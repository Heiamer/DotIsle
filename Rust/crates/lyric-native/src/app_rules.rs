//! Wire adapter; business logic is supplied by the corresponding internal DLL.
use serde::{Serialize,Deserialize};use serde_json::{json,Value};use std::{path::PathBuf,collections::HashMap};
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct Rule{
 pub id:String,pub name:String,pub executable_path:String,pub trigger:String,pub message:String,
 #[serde(default)]pub cooldown_minutes:u32,#[serde(default)]pub threshold_minutes:u32,#[serde(default)]pub enabled:bool,
 #[serde(default)]pub last_fired:Option<String>,#[serde(default)]pub disabled_date:Option<String>,#[serde(default)]pub revision:u64,
}
#[derive(Clone,Debug,Serialize,Deserialize,Default)]#[serde(rename_all="camelCase")]
pub struct Usage{
 #[serde(default)]pub accumulated_ms:u64,#[serde(default)]pub snooze_target_ms:u64,#[serde(default)]pub fired_date:Option<String>,
}
pub type Hit=(String,String,String);
pub fn same_path(a:&str,b:&str)->bool{dotisle_module_api::call("automation","same_path",json!({"a":a,"b":b})).ok().and_then(|v|v.as_bool()).unwrap_or(false)}
pub fn valid_path(path:&str)->bool{dotisle_module_api::call("automation","valid_path",json!({"path":path})).ok().and_then(|v|v.as_bool()).unwrap_or(false)}
pub struct Store {session:dotisle_module_api::Session,pub items:Vec<Rule>,pub usage:HashMap<String,Usage>,pub usage_date:String,pub baseline:HashMap<String,u64>,pub error:Option<String>}
impl Store{
pub fn new(rules_path:PathBuf,usage_path:PathBuf,today:&str)->Self{let (session,reply)=dotisle_module_api::Session::new("automation","store",json!({"rules_path":rules_path,"usage_path":usage_path,"today":today}));let mut this=Self{session,items:Default::default(),usage:Default::default(),usage_date:Default::default(),baseline:Default::default(),error:Default::default()};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(value)=serde_json::from_value(v["items"].clone()){self.items=value;}if let Ok(value)=serde_json::from_value(v["usage"].clone()){self.usage=value;}if let Ok(value)=serde_json::from_value(v["usage_date"].clone()){self.usage_date=value;}if let Ok(value)=serde_json::from_value(v["baseline"].clone()){self.baseline=value;}if let Ok(value)=serde_json::from_value(v["error"].clone()){self.error=value;}}
fn args(&self,mut a:Value)->Value{a["items"]=json!(self.items);a["usage"]=json!(self.usage);a["usage_date"]=json!(self.usage_date);a["baseline"]=json!(self.baseline);a}
fn invoke(&mut self,method:&str,args:Value)->Result<Value,String>{let reply=self.session.invoke(method,self.args(args));let v=match reply{Ok(v)=>v,Err(e)=>{self.error=Some(e.clone());return Err(e)}};self.sync(&v);if let Some(e)=v["operationError"].as_str(){return Err(e.into());}Ok(v["result"].clone())}
pub fn persist_usage(&self)->Result<(),String>{let value=self.session.invoke("persist_usage",self.args(json!({}))).and_then(|v|{if let Some(e)=v["operationError"].as_str(){Err(e.into())}else{Ok(v["result"].clone())}});value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn list(&self,today:&str)->Value{let value=self.session.invoke("list",self.args(json!({"today":today}))).and_then(|v|{if let Some(e)=v["operationError"].as_str(){Err(e.into())}else{Ok(v["result"].clone())}});value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn save(&mut self,v:&Value,today:&str,current:Option<&str>,segment:u64)->Result<String,String>{let value=self.invoke("save",json!({"v":v,"today":today,"current":current,"segment":segment}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn roll_day(&mut self,today:&str)->(){let value=self.invoke("roll_day",json!({"today":today}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn baseline_now(&mut self,path:Option<&str>,segment:u64)->(){let value=self.invoke("baseline_now",json!({"path":path,"segment":segment}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn action(&mut self,id:&str,action:&str,revision:Option<u64>,today:&str)->Result<(),String>{let value=self.invoke("action",json!({"id":id,"action":action,"revision":revision,"today":today}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn accumulate(&mut self,path:&str,delta_ms:u64,today:&str)->(){let value=self.invoke("accumulate",json!({"path":path,"delta_ms":delta_ms,"today":today}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn due_usage(&mut self,path:&str,today:&str)->Vec<Hit>{let value=self.invoke("due_usage",json!({"path":path,"today":today}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn enter_hits(&mut self,path:&str,segment:u64,today:&str,now:&crate::todos::Now,allow:bool)->Vec<Hit>{let value=self.invoke("enter_hits",json!({"path":path,"segment":segment,"today":today,"now":now,"allow":allow}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn clear_usage(&mut self)->Result<(),String>{let value=self.invoke("clear_usage",json!({}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
}
