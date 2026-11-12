//! Wire adapter; business logic is supplied by the corresponding internal DLL.
use serde::{Serialize,Deserialize};use serde_json::{json,Value};use std::{path::PathBuf,collections::HashMap};
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct Todo{pub id:String,pub title:String,pub content:String,pub date:String,pub time:String,pub daily:bool,#[serde(default="default_color")]pub color:String,#[serde(default)]pub completed:bool,#[serde(default)]pub completed_date:Option<String>,#[serde(default)]pub last_reminded_date:Option<String>,#[serde(default)]pub snooze_until:Option<String>}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Now{pub date:String,pub time:String}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->Result<T,String>{serde_json::from_value(dotisle_module_api::call("todos",method,args)?).map_err(|e|e.to_string())}
pub fn default_color()->String{request("default_color",json!({})).unwrap_or_else(|_|"#F3D79B".into())}
impl Now{pub fn local()->Self{request("now",json!({})).expect("todo time service unavailable")}}
impl Todo{pub fn done(&self,now:&Now)->bool{request("done",json!({"todo":self,"now":now})).unwrap_or(false)}}
pub fn valid_date(s:&str)->bool{request("valid_date",json!({"text":s})).unwrap_or(false)}
pub fn valid_time(s:&str)->bool{request("valid_time",json!({"text":s})).unwrap_or(false)}
pub fn add_minutes(date:&str,time:&str,minutes:u32)->String{request("add_minutes",json!({"date":date,"time":time,"minutes":minutes})).unwrap_or_default()}
pub struct Store {session:dotisle_module_api::Session,pub items:Vec<Todo>,pub error:Option<String>}
impl Store{
pub fn new(path:PathBuf)->Self{let (session,reply)=dotisle_module_api::Session::new("todos","store",json!({"path":path}));let mut this=Self{session,items:Default::default(),error:Default::default()};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(value)=serde_json::from_value(v["items"].clone()){self.items=value;}if let Ok(value)=serde_json::from_value(v["error"].clone()){self.error=value;}}
fn args(&self,mut a:Value)->Value{a["items"]=json!(self.items);a}
fn invoke(&mut self,method:&str,args:Value)->Result<Value,String>{let reply=self.session.invoke(method,self.args(args));let v=match reply{Ok(v)=>v,Err(e)=>{self.error=Some(e.clone());return Err(e)}};self.sync(&v);if let Some(e)=v["operationError"].as_str(){return Err(e.into());}Ok(v["result"].clone())}
pub fn list(&self,n:&Now)->Value{let value=self.session.invoke("list",self.args(json!({"now":n}))).and_then(|v|{if let Some(e)=v["operationError"].as_str(){Err(e.into())}else{Ok(v["result"].clone())}});value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn save(&mut self,v:&Value)->Result<String,String>{let value=self.invoke("save",json!({"v":v}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn complete(&mut self,id:&str,done:bool,n:&Now)->Result<(),String>{let value=self.invoke("complete",json!({"id":id,"done":done,"now":n}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn snooze(&mut self,id:&str,minutes:u32,n:&Now)->Result<(),String>{let value=self.invoke("snooze",json!({"id":id,"minutes":minutes,"now":n}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn restore(&mut self,value:&Value)->Result<(),String>{let value=self.invoke("restore",json!({"v":value}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn delete(&mut self,id:&str)->Result<(),String>{let value=self.invoke("delete",json!({"id":id}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn poll(&mut self,n:&Now)->Result<Vec<Todo>,String>{let value=self.invoke("poll",json!({"now":n}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
}
