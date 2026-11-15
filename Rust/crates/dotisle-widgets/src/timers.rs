// Host-side focus wire adapter. Business logic lives in dotisle_focus.dll.
use serde::{Serialize,Deserialize};use serde_json::{json,Value};use std::path::PathBuf;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub name: String,
    pub seconds: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub stage_index: usize,
    #[serde(default)]
    pub waiting: bool,
    #[serde(default)]
    pub template_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub duration: u64,
    pub focus: u64,
    pub rest: u64,
    pub rounds: u32,
    pub round: u32,
    pub phase: String,
    pub elapsed_ms: u64,
    pub running: bool,
    pub finished: bool,
    #[serde(default)]
    pub todo_id: Option<String>,
    #[serde(default)]
    pub workflow: Option<Workflow>,
    #[serde(default)]
    pub pause_reason: Option<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub laps: Vec<u64>,
    #[serde(default)]
    pub count_up: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub stages: Vec<Stage>,
}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->Result<T,String>{serde_json::from_value(dotisle_module_api::call("focus",method,args)?).map_err(|e|e.to_string())}
impl Timer {
pub fn primary_action(&self)->&'static str{match request::<String>("timer.primary_action",json!({"timer":self})).unwrap_or_default().as_str(){"restart"=>"restart","next"=>"next","pause"=>"pause",_=>"resume"}}
pub fn panel(&self)->crate::activities::Panel{request("timer.panel",json!({"timer":self})).unwrap_or_else(|_|Default::default())}
pub fn limit_ms(&self)->u64{request("timer.limit_ms",json!({"timer":self})).unwrap_or_else(|_|0)}
pub fn remaining_ms(&self)->u64{request("timer.remaining_ms",json!({"timer":self})).unwrap_or_else(|_|0)}
pub fn clock(&self)->String{request("timer.clock",json!({"timer":self})).unwrap_or_else(|_|String::new())}
pub fn progress(&self)->f64{request("timer.progress",json!({"timer":self})).unwrap_or_else(|_|0.)}
pub fn subtitle(&self)->String{request("timer.subtitle",json!({"timer":self})).unwrap_or_else(|_|String::new())}
}
pub fn format_time(s:u64)->String{request("format_time",json!({"s":s})).unwrap_or_default()}
pub fn stages_from(value:&Value)->Result<Vec<Stage>,String>{request("stages_from",value.clone())}
pub fn stages_ok(stages:&[Stage])->bool{request("stages_ok",json!(stages)).unwrap_or_default()}
pub struct Store{session:dotisle_module_api::Session,pub items:Vec<Timer>,pub error:Option<String>}
impl Store{pub fn new(path:PathBuf)->Self{let (session,reply)=dotisle_module_api::Session::new("focus","store",json!({"path":path}));let mut this=Self{session,items:vec![],error:None};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(items)=serde_json::from_value(v["items"].clone()){self.items=items;}self.error=v["error"].as_str().map(str::to_owned);}
fn invoke(&mut self,method:&str,mut args:Value)->Result<Value,String>{args["items"]=json!(self.items);let reply=self.session.invoke(method,args)?;self.sync(&reply);if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}Ok(reply["result"].clone())}
pub fn persist(&self)->Result<(),String>{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("persist",a)?;if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}serde_json::from_value(reply["result"].clone()).map_err(|e|e.to_string())}
pub fn list(&self)->Value{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("list",a);reply.ok().map(|v|v["result"].clone()).unwrap_or(Value::Null)}
pub fn create(&mut self,v:&Value)->Result<String,String>{let value=self.invoke("create",json!({"v":v}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn action(&mut self,id:&str,action:&str)->Result<(),String>{let value=self.invoke("action",json!({"id":id,"action":action}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn act(&mut self,id:&str,action:&str,revision:Option<u64>)->Result<(),String>{let value=self.invoke("act",json!({"id":id,"action":action,"revision":revision}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn set_direction(&mut self,id:&str,count_up:bool,revision:Option<u64>)->Result<(),String>{let value=self.invoke("set_direction",json!({"id":id,"count_up":count_up,"revision":revision}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn advance(&mut self,ms:u64)->Vec<Timer>{let value=self.invoke("advance",json!({"ms":ms}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn pause_for_lock(&mut self,generation:u64)->Result<Vec<String>,String>{let value=self.invoke("pause_for_lock",json!({"generation":generation}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn resume_focus(&mut self,ids:&[String],generation:u64)->Result<Vec<String>,String>{let value=self.invoke("resume_focus",json!({"ids":ids,"generation":generation}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn clear_lock_pauses(&mut self)->Result<(),String>{let value=self.invoke("clear_lock_pauses",json!({}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
}
pub struct Templates{session:dotisle_module_api::Session,pub items:Vec<Template>,pub error:Option<String>}
impl Templates{pub fn new(path:PathBuf)->Self{let (session,reply)=dotisle_module_api::Session::new("focus","templates",json!({"path":path}));let mut this=Self{session,items:vec![],error:None};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(items)=serde_json::from_value(v["items"].clone()){self.items=items;}self.error=v["error"].as_str().map(str::to_owned);}
fn invoke(&mut self,method:&str,mut args:Value)->Result<Value,String>{args["items"]=json!(self.items);let reply=self.session.invoke(method,args)?;self.sync(&reply);if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}Ok(reply["result"].clone())}
pub fn persist(&self)->Result<(),String>{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("persist",a)?;if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}serde_json::from_value(reply["result"].clone()).map_err(|e|e.to_string())}
pub fn list(&self)->Value{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("list",a);reply.ok().map(|v|v["result"].clone()).unwrap_or(Value::Null)}
pub fn save(&mut self,v:&Value)->Result<String,String>{let value=self.invoke("save",json!({"v":v}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn delete(&mut self,id:&str)->Result<(),String>{let value=self.invoke("delete",json!({"id":id}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
}
