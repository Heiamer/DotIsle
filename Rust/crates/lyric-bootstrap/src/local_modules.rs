//! Local first-party DLL transactions; inspection executes in a disposable child process.
use crate::{error::{Error,Result},paths,updater,win};
use serde_json::{json,Value};
use std::{path::{Path,PathBuf},process::{Command,Stdio},time::{Duration,Instant},os::windows::process::CommandExt};
fn err(message:impl Into<String>)->Error{Error(message.into())}
struct StageGuard{work:PathBuf,root:PathBuf,keep:bool}
impl Drop for StageGuard{fn drop(&mut self){if !self.keep{if let(Ok(work),Ok(root))=(self.work.canonicalize(),self.root.canonicalize()){if work.starts_with(&root)&&work!=root{let _=std::fs::remove_dir_all(work);}}}}}
fn run_bounded(command:&mut Command,timeout:Duration)->Result<()> {
 let mut child=command.creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;let start=Instant::now();
 loop {if let Some(status)=child.try_wait()?{return if status.success(){Ok(())}else{Err(err(format!("模块校验进程退出：{status}")))};}if start.elapsed()>timeout{let _=child.kill();let _=child.wait();return Err(err("模块校验超时，旧程序未修改"));}std::thread::sleep(Duration::from_millis(20));}
}
fn inspect(path:&Path,work:&Path,index:usize)->Result<Value>{
 let report=work.join(format!("inspect-{index}.json"));let _=std::fs::remove_file(&report);
 run_bounded(Command::new(std::env::current_exe()?).arg("--module-inspect").arg(path).arg(&report),Duration::from_secs(20))?;
 let result=paths::read_json(&report)?;if result["success"]!=true{return Err(err(result["message"].as_str().unwrap_or("DLL 校验失败")));}Ok(result["module"].clone())
}
pub fn catalog(target:&Path,work:&Path,override_file:Option<(&str,&Path)>)->Result<Vec<Value>>{
 let mut items=vec![];for(i,(id,label))in dotisle_module_api::MODULES.iter().enumerate(){
  let filename=dotisle_module_api::filename(id).ok_or_else(||err("模块标识"))?;let path=target.join(&filename);let file=override_file.filter(|(key,_)|*key==*id).map(|(_,path)|path).unwrap_or(&path);
  let mut info=inspect(file,work,i)?;if info["id"]!=*id{return Err(err(format!("{filename} 的模块角色不符")));}info["label"]=json!(label);info["file"]=json!(filename);items.push(info);
 }dotisle_module_api::inspection::validate_set(&items).map_err(err)?;Ok(items)
}
pub fn validate_overlay(target:&Path,stage:&Path,work:&Path)->Result<Vec<Value>>{
 let mut items=vec![];for(i,(id,label))in dotisle_module_api::MODULES.iter().enumerate(){let name=dotisle_module_api::filename(id).ok_or_else(||err("模块标识"))?;let replacement=stage.join(&name);let existing=target.join(&name);let mut info=inspect(if replacement.is_file(){&replacement}else{&existing},work,200+i)?;if info["id"]!=*id{return Err(err("更新包中的模块角色不一致"));}info["label"]=json!(label);items.push(info);}dotisle_module_api::inspection::validate_set(&items).map_err(err)?;Ok(items)
}
fn stage(source:&Path,target:&Path,data:&Path,expected:&str)->Result<Value>{
 if dotisle_module_api::filename(expected).is_none(){return Err(err("未知内部模块"));}
 let source=source.canonicalize()?;if !source.is_file()||!source.extension().is_some_and(|e|e.eq_ignore_ascii_case("dll"))||std::fs::metadata(&source)?.len()>128*1024*1024{return Err(err("请选择小于 128 MB 的内部 DLL"));}
 let work=data.join("Cache/ModuleUpdates").join(format!("job-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));std::fs::create_dir_all(&work)?;
 let mut cleanup=StageGuard{work:work.clone(),root:data.join("Cache/ModuleUpdates"),keep:false};
 let candidate=work.join("candidate.dll");std::fs::copy(source,&candidate)?;
 let info=inspect(&candidate,&work,100)?;if info["id"]!=expected{return Err(err("选择的 DLL 不属于该功能模块"));}
 let items=catalog(target,&work,Some((expected,&candidate)))?;
 let pending=work.join("pending.json");let record=json!({"schemaVersion":1,"id":expected,"file":dotisle_module_api::filename(expected),"sha256":lyric_archive::sha256_file(&candidate)?,"module":info,"candidate":candidate,"target":target});paths::write_json(&pending,&record)?;
 paths::write_json(&data.join("module-pending.json"),&json!({"pending":pending,"id":expected,"module":info}))?;
 cleanup.keep=true;
 Ok(json!({"success":true,"phase":"staged","message":"模块已校验并暂存，点击安装并重启后生效","pending":pending,"selected":info,"items":items}))
}
fn same_path(a:&Path,b:&Path)->bool{a.canonicalize().ok().zip(b.canonicalize().ok()).is_some_and(|(a,b)|a==b)}
struct RestartGuard{target:PathBuf,data:PathBuf,active:bool}
impl Drop for RestartGuard{fn drop(&mut self){if self.active{let _=updater::start(&self.target,&self.data);}}}
fn apply(pending:&Path,target:&Path,data:&Path,pid:u32,settings:u32,pipe:&str)->Result<Value>{
 let _gate=win::NamedMutex::acquire(&format!("Local.LyricIsland.OTA.{}",updater::executable_hash(target)?))?.ok_or_else(||err("已有更新事务正在进行"))?;
 let root=data.join("Cache/ModuleUpdates").canonicalize()?;let pending=pending.canonicalize()?;
 if !pending.starts_with(&root){return Err(err("模块暂存路径无效"));}let work=pending.parent().ok_or_else(||err("暂存目录无效"))?;
 let record=paths::read_json(&pending)?;if record["schemaVersion"]!=1{return Err(err("模块事务版本无效"));}
 let id=record["id"].as_str().ok_or_else(||err("模块标识缺失"))?;let filename=dotisle_module_api::filename(id).ok_or_else(||err("模块标识无效"))?;
 if !same_path(Path::new(record["target"].as_str().ok_or_else(||err("目标缺失"))?),target){return Err(err("模块事务安装目录不一致"));}
 let candidate=work.join("candidate.dll");updater::verify(&candidate,record["sha256"].as_str().ok_or_else(||err("摘要缺失"))?)?;
 catalog(target,work,Some((id,&candidate)))?;
 let before=win::pipe_request(pipe,&json!({"version":1,"command":"state"}),Duration::from_secs(3))?;
 if before["processId"].as_u64()!=Some(pid as u64)||!win::process_image(pid).is_some_and(|p|same_path(&p,&target.join("LyricIsland.exe"))){return Err(err("运行实例不匹配，未安装模块"));}
 let stage_dir=work.join("install");std::fs::create_dir_all(&stage_dir)?;std::fs::copy(&candidate,stage_dir.join(&filename))?;
 let journal=data.join("module-update-result.json");paths::write_json(&journal,&json!({"phase":"applying","id":id,"message":"正在保存并替换模块"}))?;
 // A separately opened settings window must also release its DLL, and only its verified process is closed.
 if settings!=0&&win::process_image(settings).is_some_and(|p|same_path(&p,&target.join("LyricIsland.Settings.exe"))){win::close_main_windows(settings);}
 updater::stop_app(pipe)?;let mut recovery=RestartGuard{target:target.into(),data:data.into(),active:true};updater::wait(pid as i32)?;if settings!=0{updater::wait(settings as i32)?;}
 let backup=work.join("backup");let mut restarted=None;
 let mut after=||->Result<()> {
  run_bounded(Command::new(target.join("LyricIsland.Settings.exe")).arg("--module-health").arg(work.join("settings-health")),Duration::from_secs(30))?;
  let mut child=Command::new(target.join("LyricIsland.exe")).arg("--data-dir").arg(data).current_dir(target).creation_flags(0x08000000).spawn()?;
  let deadline=Instant::now()+Duration::from_secs(20);let mut ready=false;
  while Instant::now()<deadline {if child.try_wait()?.is_some(){break;}if let Ok(v)=win::pipe_request(pipe,&json!({"version":1,"command":"state"}),Duration::from_secs(1)){
    if v["processId"].as_u64()==Some(child.id()as u64)&&v["moduleVersions"][id]==record["module"]["version"]&&v["moduleReady"]==true {ready=true;break;}
   }std::thread::sleep(Duration::from_millis(100));}
  if !ready{let _=child.kill();let _=child.wait();return Err(err("新模块未通过启动确认，已回滚"));}restarted=Some(child.id());Ok(())
 };
 let old_hash=lyric_archive::sha256_file(&target.join(&filename))?;
 let outcome=updater::apply(&stage_dir,target,&backup,0,Some(&mut after));
 if let Err(e)=outcome {
  let rolled_back=lyric_archive::sha256_file(&target.join(&filename)).is_ok_and(|h|h==old_hash);if !rolled_back{recovery.active=false;}let result=json!({"success":false,"phase":"failed","rolledBack":rolled_back,"id":id,"message":e.to_string()});let _=paths::write_json(&journal,&result);let _=std::fs::remove_file(data.join("module-pending.json"));return Err(e);
 }
 recovery.active=false;
 let previous=data.join("Cache/ModuleBackups").join(id);std::fs::create_dir_all(&previous)?;
 if backup.join(&filename).is_file(){std::fs::copy(backup.join(&filename),previous.join("previous.dll"))?;}
 let result=json!({"success":true,"phase":"completed","id":id,"message":"模块已替换，新版本启动确认通过","module":record["module"],"processId":restarted});paths::write_json(&journal,&result)?;let _=std::fs::remove_file(data.join("module-pending.json"));let _=std::fs::remove_dir_all(work);Ok(result)
}
pub fn cli(args:&[String])->Option<i32>{
 if !args.first().is_some_and(|a|a.starts_with("--module-")){return None;}
 let result=(||->Result<Value>{match args.first().map(String::as_str){
  Some("--module-inspect") if args.len()==3=>{let value=dotisle_module_api::inspection::inspect(Path::new(&args[1])).map_err(err)?;Ok(json!({"success":true,"module":value}))},
  Some("--module-list") if args.len()==3=>{let report=Path::new(&args[2]);let work=report.parent().ok_or_else(||err("输出目录"))?;std::fs::create_dir_all(work)?;let items=catalog(Path::new(&args[1]),work,None)?;Ok(json!({"success":true,"phase":"idle","items":items}))},
  Some("--module-stage") if args.len()==6=>stage(Path::new(&args[1]),Path::new(&args[2]),Path::new(&args[3]),&args[4]),
  Some("--module-apply") if args.len()==7=>apply(Path::new(&args[1]),Path::new(&args[2]),Path::new(&args[3]),args[4].parse().map_err(|_|err("主进程标识无效"))?,args[5].parse().map_err(|_|err("设置进程标识无效"))?,&args[6]),
  _=>Err(err("模块操作参数无效"))}
 })();
 if args.first().is_some_and(|a|a=="--module-apply")&&args.len()==7{if let Err(e)=&result{let _=paths::write_json(&Path::new(&args[3]).join("module-update-result.json"),&json!({"success":false,"phase":"failed","message":e.to_string()}));}}
 let code=if result.is_ok(){0}else{1};let value=result.unwrap_or_else(|e|json!({"success":false,"phase":"failed","message":e.to_string()}));
 if let Some(path)=match args.first().map(String::as_str){Some("--module-stage") if args.len()==6=>args.get(5),Some("--module-list"|"--module-inspect") if args.len()==3=>args.get(2),_=>None}{let _=paths::write_json(Path::new(path),&value);}
 Some(code)
}
