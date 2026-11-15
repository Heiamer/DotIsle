//! Wire adapter; business logic is supplied by the corresponding internal DLL.
use serde::{Serialize,Deserialize};use serde_json::{json,Value};use std::{path::PathBuf,collections::HashMap};
#[derive(Clone,Debug,Serialize,Deserialize)]#[serde(rename_all="camelCase")]
pub struct Event{pub id:String,pub kind:String,pub app_id:String,pub app_name:String,pub title:String,pub body:String,pub date:String,pub clock:String,pub t:i64,#[serde(default)]pub todo_id:Option<String>}
pub const LIMIT:usize=200;
pub fn kinds()->[&'static str;5]{["notice","todo","device","timer","automation"]}
pub struct Store {session:dotisle_module_api::Session,pub items:Vec<Event>,pub error:Option<String>}
impl Store{
pub fn new(path:PathBuf)->Self{let (session,reply)=dotisle_module_api::Session::new("history","store",json!({"path":path}));let mut this=Self{session,items:Default::default(),error:Default::default()};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(value)=serde_json::from_value(v["items"].clone()){self.items=value;}if let Ok(value)=serde_json::from_value(v["error"].clone()){self.error=value;}}
fn args(&self,mut a:Value)->Value{a["items"]=json!(self.items);a}
fn invoke(&mut self,method:&str,args:Value)->Result<Value,String>{let reply=self.session.invoke(method,self.args(args));let v=match reply{Ok(v)=>v,Err(e)=>{self.error=Some(e.clone());return Err(e)}};self.sync(&v);if let Some(e)=v["operationError"].as_str(){return Err(e.into());}Ok(v["result"].clone())}
pub fn list(&self)->Value{let value=self.session.invoke("list",self.args(json!({}))).and_then(|v|{if let Some(e)=v["operationError"].as_str(){Err(e.into())}else{Ok(v["result"].clone())}});value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn record(&mut self,kind:&str,app_id:&str,app_name:&str,title:&str,body:&str,todo_id:Option<String>)->(){let value=self.invoke("record",json!({"kind":kind,"app_id":app_id,"app_name":app_name,"title":title,"body":body,"todo_id":todo_id}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn prune(&mut self,days:u32,now:i64)->(){let value=self.invoke("prune",json!({"days":days,"now":now}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn clear(&mut self,kind:Option<&str>)->(){let value=self.invoke("clear",json!({"kind":kind}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
}
