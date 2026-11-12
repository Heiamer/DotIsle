use serde_json::{json,Value};use std::sync::Arc;
#[derive(Clone,Debug,Default,serde::Serialize,serde::Deserialize)]
pub struct Notice {pub app_id:String,pub app_name:String,pub title:String,pub body:String,pub icon:Option<Vec<u8>>}
#[derive(Clone,Default,serde::Serialize,serde::Deserialize)]
pub struct Policy {pub enabled:bool,pub paused:bool,pub whitelist_enabled:bool,pub allowed:Vec<String>,pub blacklist_enabled:bool,pub blocked:Vec<String>}
#[derive(serde::Serialize,serde::Deserialize)]pub enum Event { Notice(Notice), State(Value) }
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->Result<T,String>{serde_json::from_value(dotisle_module_api::call("notifications",method,args)?).map_err(|e|e.to_string())}
impl Notice{pub fn example(base:&std::path::Path)->Self{request("example",json!({"base":base})).unwrap_or_default()}}
impl Policy {
pub fn from_settings(v:&Value)->Self{request("policy",v.clone()).unwrap_or_default()}
pub fn permits(&self,id:&str,name:&str)->bool{request("permits",json!({"policy":self,"id":id,"name":name})).unwrap_or(false)}
pub fn should_show(&self)->bool{request("should_show",json!(self)).unwrap_or(false)}
}
pub struct EventQueue(Arc<dotisle_module_api::Session>);
impl EventQueue {pub fn try_iter(&self)->std::vec::IntoIter<Event>{self.0.invoke("poll",json!({})).ok().and_then(|v|serde_json::from_value::<Vec<Event>>(v).ok()).unwrap_or_default().into_iter()}}
pub struct Listener{pub events:EventQueue,session:Arc<dotisle_module_api::Session>}
impl Listener{pub fn new(policy:Policy)->Self{let(session,_)=dotisle_module_api::Session::new("notifications","listener",json!(policy));let session=Arc::new(session);Self{events:EventQueue(session.clone()),session}}pub fn configure(&self,policy:Policy){let _=self.session.invoke("configure",json!({"policy":policy}));}}
pub fn diagnostics()->Value{request("diagnostics",json!({})).unwrap_or_else(|e|json!({"error":e}))}
