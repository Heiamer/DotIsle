//! Nonblocking module management. The independent updater owns DLL inspection and writes.
use serde_json::{json,Value};use std::{path::{Path,PathBuf},sync::mpsc,time::{Duration,Instant},process::{Command,Stdio},os::windows::process::CommandExt};
pub struct Manager{pub state:Value,base:PathBuf,data:PathBuf,rx:Option<mpsc::Receiver<Result<Value,String>>>,last:Instant,last_result:Value}
impl Manager{
 pub fn new(base:&Path,data:&Path,demo:bool)->Self{let mut s=Self{state:json!({"phase":"idle","items":[],"message":"内部功能模块支持独立替换，安装时应用会重启"}),base:base.into(),data:data.into(),rx:None,last:Instant::now(),last_result:Value::Null};if !demo{s.refresh();}s}
 fn worker(&mut self,mut args:Vec<String>)->Result<(),String>{if self.rx.is_some(){return Err("模块校验正在进行".into());}let root=self.data.join("Cache/ModuleReports");std::fs::create_dir_all(&root).map_err(|e|e.to_string())?;let report=root.join(format!("report-{}.json",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));args.push(report.to_string_lossy().into_owned());let helper=self.base.join("LyricIsland.Updater.exe");let(tx,rx)=mpsc::channel();self.rx=Some(rx);self.state["phase"]=json!("checking");std::thread::spawn(move||{let result=(||{let mut child=Command::new(helper).args(args).creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;let start=Instant::now();loop{if let Some(status)=child.try_wait().map_err(|e|e.to_string())?{let v=std::fs::read(&report).ok().and_then(|b|serde_json::from_slice::<Value>(&b).ok()).ok_or("模块助手没有返回有效报告")?;if !status.success()||v["success"]!=true{return Err(v["message"].as_str().unwrap_or("模块校验失败").into());}return Ok(v);}if start.elapsed()>Duration::from_secs(120){let _=child.kill();let _=child.wait();return Err("模块检查超时，现有模块未修改".into());}std::thread::sleep(Duration::from_millis(30));}})();let _=std::fs::remove_file(report);let _=tx.send(result);});Ok(())}
 pub fn refresh(&mut self){let _=self.worker(vec!["--module-list".into(),self.base.to_string_lossy().into_owned()]);}
 // A journal written before a new local action describes an older transaction; consume it so the
 // one-second journal poll cannot overwrite the newer staged/applying state.
 fn consume_journal(&mut self){if let Ok(bytes)=std::fs::read(self.data.join("module-update-result.json")){if let Ok(v)=serde_json::from_slice::<Value>(&bytes){self.last_result=v;}}}
 pub fn stage(&mut self,id:&str,path:&str)->Result<(),String>{if dotisle_module_api::filename(id).is_none(){return Err("未知模块".into());}self.consume_journal();self.worker(vec!["--module-stage".into(),path.into(),self.base.to_string_lossy().into_owned(),self.data.to_string_lossy().into_owned(),id.into()])}
 pub fn rollback(&mut self,id:&str)->Result<(),String>{if dotisle_module_api::filename(id).is_none(){return Err("未知模块".into());}let path=self.data.join("Cache/ModuleBackups").join(id).join("previous.dll");self.stage(id,&path.to_string_lossy())}
 pub fn cancel(&mut self)->Result<(),String>{if self.rx.is_some(){return Err("请等待校验完成后再取消".into());}self.consume_journal();let _=std::fs::remove_file(self.data.join("module-pending.json"));self.state=json!({"phase":"idle","items":self.state["items"],"message":"已取消暂存模块"});Ok(())}
 pub fn apply(&mut self,settings:u32)->Result<(),String>{if self.rx.is_some(){return Err("模块正在校验".into());}self.consume_journal();let pending=std::fs::read(self.data.join("module-pending.json")).map_err(|_|"没有已校验的待安装模块")?;let value:Value=serde_json::from_slice(&pending).map_err(|e|e.to_string())?;let path=value["pending"].as_str().ok_or("模块事务缺失")?;let pipe=std::env::var("LYRICISLAND_TEST_PIPE").unwrap_or_else(|_|"LyricIsland.Native.Settings.v1".into());Command::new(self.base.join("LyricIsland.Updater.exe")).arg("--module-apply").arg(path).arg(&self.base).arg(&self.data).arg(std::process::id().to_string()).arg(settings.to_string()).arg(pipe).creation_flags(0x08000000).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map_err(|e|e.to_string())?;self.state["phase"]=json!("applying");self.state["message"]=json!("正在保存并安装，应用将自动重启");Ok(())}
 pub fn tick(&mut self)->bool{let mut changed=false;if let Some(result)=self.rx.as_ref().and_then(|rx|rx.try_recv().ok()){self.rx=None;match result{Ok(mut v)=>{if v["phase"]=="idle"{if let Ok(bytes)=std::fs::read(self.data.join("module-pending.json")){if let Ok(p)=serde_json::from_slice::<Value>(&bytes){v["phase"]=json!("staged");v["selected"]=p["module"].clone();v["message"]=json!("有一个已校验模块等待安装");}}}self.state=v;},Err(e)=>{self.state["phase"]=json!("failed");self.state["message"]=json!(e);}}changed=true;}
 if self.last.elapsed()>Duration::from_secs(1){self.last=Instant::now();if let Ok(bytes)=std::fs::read(self.data.join("module-update-result.json")){if let Ok(result)=serde_json::from_slice::<Value>(&bytes){if result!=self.last_result{self.last_result=result.clone();self.state["phase"]=result["phase"].clone();self.state["message"]=result["message"].clone();changed=true;}}}}
 changed}
}
#[cfg(test)]
mod tests{
 use super::*;
 #[test]
 fn consumed_journal_cannot_replace_newer_module_state(){
  let data=std::env::temp_dir().join(format!("dotisle-module-journal-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));std::fs::create_dir_all(&data).unwrap();
  let journal=data.join("module-update-result.json");std::fs::write(&journal,r#"{"success":true,"phase":"completed","id":"focus","message":"模块已替换"}"#).unwrap();
  let mut m=Manager::new(Path::new("."),&data,true);m.state=json!({"phase":"staged","items":[],"message":"有一个已校验模块等待安装"});m.consume_journal();m.last=Instant::now()-Duration::from_secs(2);
  assert!(!m.tick());assert_eq!(m.state["phase"],"staged");
  std::fs::write(&journal,r#"{"success":false,"phase":"failed","id":"focus","message":"安装失败"}"#).unwrap();m.last=Instant::now()-Duration::from_secs(2);
  assert!(m.tick());assert_eq!(m.state["phase"],"failed");
  let _=std::fs::remove_dir_all(&data);
 }
}
