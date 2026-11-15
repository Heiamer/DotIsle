//! Public data-driven plugin contract shared by all three native hosts.
//! Producers run independently; the host reads bounded JSON and never executes code.
use serde::{Deserialize,Serialize};
use serde_json::{json,Value};
use std::{collections::BTreeMap,fs,io::Read,path::{Path,PathBuf},time::{Duration,Instant,SystemTime}};

pub fn valid_id(id:&str)->bool{!id.is_empty()&&id.len()<=64&&id.bytes().all(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||c==b'-'||c==b'.')&&!id.starts_with('.')&&!id.ends_with('.')&&!id.contains("..")}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]
#[serde(default,rename_all="camelCase",deny_unknown_fields)]
pub struct Block{pub kind:String,pub text:String,pub label:String,pub bind:String,pub value:f64}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Manifest{pub api_version:u32,pub id:String,pub name:String,pub version:String,#[serde(default)]pub description:String,pub views:BTreeMap<String,Vec<Block>>}
#[derive(Clone,Debug,Default,Serialize,Deserialize,PartialEq)]
#[serde(default,rename_all="camelCase")]
pub struct ResolvedBlock{pub kind:String,pub text:String,pub label:String,pub value:f64}
fn clean(text:&str,max:usize)->String{text.chars().filter(|c|!c.is_control()).take(max).collect()}
impl Manifest{
 pub fn validate(&self)->Result<(),String>{
  if self.api_version!=1{return Err("插件 API 版本不受支持".into())}
  if !valid_id(&self.id)||self.name.trim().is_empty()||self.name.chars().count()>40||self.version.len()>32||self.description.chars().count()>180{return Err("插件名称、标识或版本无效".into())}
  if self.views.is_empty()||self.views.keys().any(|k|!["island","overview","desktop"].contains(&k.as_str())){return Err("插件展示位置无效".into())}
  for blocks in self.views.values(){
   if blocks.is_empty()||blocks.len()>8{return Err("每个展示位置需要 1–8 块内容".into())}
   let mut height=0;
   for b in blocks{
    height+=match b.kind.as_str(){"metric"=>56,"text"=>28,"progress"=>40,"divider"=>12,_=>return Err("不支持的插件积木类型".into())};
    if b.text.chars().count()>160||b.label.chars().count()>40||b.bind.len()>80||!b.value.is_finite()||(!b.bind.is_empty()&&!b.bind.split('.').all(|p|!p.is_empty()&&p.bytes().all(|c|c.is_ascii_alphanumeric()||c==b'_'))){return Err("插件积木内容无效".into())}
   }
   if height>260{return Err("插件内容过高，请减少积木数量".into())}
  }Ok(())
 }
 pub fn resolve(&self,surface:&str,data:&Value)->Vec<ResolvedBlock>{
  self.views.get(surface).into_iter().flatten().map(|b|{
   let v=if b.bind.is_empty(){None}else{let mut v=data;for key in b.bind.split('.'){v=&v[key]}Some(v)};
   let text=match v{Some(Value::String(s))=>clean(s,160),Some(Value::Number(n))=>n.to_string(),Some(Value::Bool(v))=>v.to_string(),Some(_)=>"—".into(),None=>clean(&b.text,160)};
   ResolvedBlock{kind:b.kind.clone(),text,label:clean(&b.label,40),value:v.and_then(Value::as_f64).unwrap_or(b.value).clamp(0.,1.)}
  }).collect()
 }
}
fn read_json(path:&Path,max:u64)->Result<Value,String>{
 let meta=fs::symlink_metadata(path).map_err(|_|format!("无法读取 {}",path.file_name().unwrap_or_default().to_string_lossy()))?;
 if !meta.is_file()||meta.file_type().is_symlink()||meta.len()>max{return Err("插件文件类型或大小无效".into())}
 let mut bytes=Vec::new();fs::File::open(path).map_err(|e|e.to_string())?.take(max+1).read_to_end(&mut bytes).map_err(|e|e.to_string())?;
 if bytes.len() as u64>max{return Err("插件文件过大".into())}serde_json::from_slice(&bytes).map_err(|_|"插件 JSON 格式无效".into())
}
#[derive(Clone,Serialize,Deserialize)]
#[serde(rename_all="camelCase")]
struct Registration{id:String,path:PathBuf,enabled:bool}
pub struct Registry{path:PathBuf,entries:Vec<Registration>,pub state:Value,last:Instant,stamps:BTreeMap<String,[Option<(SystemTime,u64)>;2]>,pub error:String}
impl Registry{
 pub fn new(path:PathBuf)->Self{
  let (entries,error)=if path.is_file(){match read_json(&path,65536).and_then(|v|serde_json::from_value::<Vec<Registration>>(v).map_err(|e|e.to_string())){Ok(v) if v.len()<=32=>(v,String::new()),_=> (vec![],"插件注册表读取失败，原文件已保留".into())}}else{(vec![],String::new())};
  let mut r=Self{path,entries,state:json!([]),last:Instant::now()-Duration::from_secs(2),stamps:BTreeMap::new(),error};r.refresh(true);r
 }
 fn load(folder:&Path)->Result<(Manifest,Value),String>{
  let m:Manifest=serde_json::from_value(read_json(&folder.join("manifest.json"),65536)?).map_err(|_|"插件清单格式无效".to_string())?;m.validate()?;
  let state=folder.join("state.json");let data=if state.exists(){read_json(&state,131072)?}else{json!({})};if !data.is_object(){return Err("插件数据必须为 JSON 对象".into())}Ok((m,data))
 }
 fn save(&self,entries:&Vec<Registration>)->Result<(),String>{if !self.error.is_empty(){return Err(self.error.clone())}lyric_backend::io::save_settings(&self.path,&json!(entries))}
 pub fn install(&mut self,folder:&Path)->Result<(),String>{
  if self.entries.len()>=32{return Err("最多安装 32 个插件".into())}
  let folder=folder.canonicalize().map_err(|_|"请选择有效插件文件夹".to_string())?;let(m,_)=Self::load(&folder)?;
  if self.entries.iter().any(|e|e.id==m.id){return Err("同一插件已经安装，请移除后重新添加".into())}
  let mut entries=self.entries.clone();entries.push(Registration{id:m.id,path:folder,enabled:true});self.save(&entries)?;self.entries=entries;self.refresh(true);Ok(())
 }
 pub fn change(&mut self,id:&str,enabled:Option<bool>)->Result<(),String>{
  let mut entries=self.entries.clone();let Some(i)=entries.iter().position(|e|e.id==id)else{return Err("插件不存在".into())};
  if let Some(v)=enabled{entries[i].enabled=v}else{entries.remove(i);}
  self.save(&entries)?;self.entries=entries;self.refresh(true);Ok(())
 }
 pub fn refresh(&mut self,force:bool)->bool{
  if !force&&self.last.elapsed()<Duration::from_secs(1){return false} self.last=Instant::now();
  let mut stamps=BTreeMap::new();
  let next=Value::Array(self.entries.iter().map(|e|{
   let fingerprint=["manifest.json","state.json"].map(|name|fs::symlink_metadata(e.path.join(name)).ok().and_then(|m|m.modified().ok().map(|at|(at,m.len()))));
   stamps.insert(e.id.clone(),fingerprint);
   if !force&&self.stamps.get(&e.id)==Some(&fingerprint){if let Some(previous)=self.state.as_array().and_then(|a|a.iter().find(|p|p["id"]==e.id)){return previous.clone()}}
   match Self::load(&e.path){Ok((m,data)) if m.id==e.id=>{let views=m.views.keys().map(|k|(k.clone(),json!(m.resolve(k,&data)))).collect::<serde_json::Map<_,_>>();json!({"id":m.id,"name":m.name,"version":m.version,"description":m.description,"enabled":e.enabled,"views":views,"error":""})},
   _=>json!({"id":e.id,"name":e.id,"enabled":e.enabled,"views":{},"error":"插件文件损坏、缺失或格式不受支持"})}
  }).collect());let changed=self.state!=next;self.state=next;self.stamps=stamps;changed
 }
 pub fn island_ids(&self)->Vec<String>{self.state.as_array().into_iter().flatten().filter(|p|p["enabled"]==true&&p["views"]["island"].is_array()).filter_map(|p|p["id"].as_str().map(|id|format!("plugin:{id}"))).collect()}
 pub fn get(&self,id:&str)->Option<&Value>{self.state.as_array()?.iter().find(|p|p["id"]==id&&p["enabled"]==true&&p["error"]=="")}
}
#[cfg(test)]mod tests{use super::*;
 fn manifest()->Manifest{serde_json::from_value(json!({"apiVersion":1,"id":"example.hello","name":"Hello","version":"1","views":{"island":[{"kind":"metric","bind":"count","label":"剩余"}],"overview":[{"kind":"text","bind":"message"}],"desktop":[{"kind":"progress","bind":"progress"}]}})).unwrap()}
 #[test]fn validates_and_resolves_every_host(){let m=manifest();m.validate().unwrap();let data=json!({"count":7,"message":"你好\n世界","progress":5});assert_eq!(m.resolve("island",&data)[0].text,"7");assert_eq!(m.resolve("overview",&data)[0].text,"你好世界");assert_eq!(m.resolve("desktop",&data)[0].value,1.);assert_eq!(m.resolve("island",&json!({}))[0].text,"—");}
 #[test]fn rejects_unknown_api_bad_ids_and_expensive_trees(){let mut m=manifest();m.api_version=2;assert!(m.validate().is_err());m.api_version=1;m.id="../../bad".into();assert!(m.validate().is_err());m.id="valid".into();m.views.get_mut("island").unwrap().resize(9,Block::default());assert!(m.validate().is_err());}
 #[test]fn install_toggle_remove_and_broken_state_are_isolated(){
  let dir=std::env::temp_dir().join(format!("dotisle-plugin-test-{}-{}",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));fs::create_dir_all(&dir).unwrap();let package=dir.join("package");fs::create_dir(&package).unwrap();fs::write(package.join("manifest.json"),serde_json::to_vec(&manifest()).unwrap()).unwrap();
  let path=dir.join("registry.json");let mut r=Registry::new(path.clone());r.install(&package).unwrap();assert_eq!(r.island_ids(),vec!["plugin:example.hello"]);assert!(r.install(&package).is_err());r.change("example.hello",Some(false)).unwrap();assert!(r.island_ids().is_empty());r=Registry::new(path);assert!(r.island_ids().is_empty());r.change("example.hello",Some(true)).unwrap();fs::write(package.join("state.json"),b"bad").unwrap();r.refresh(true);assert!(r.island_ids().is_empty());r.change("example.hello",None).unwrap();assert_eq!(r.state,json!([]));let _=fs::remove_dir_all(dir);
 }
}
