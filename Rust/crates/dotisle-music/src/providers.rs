//! Provider transport adapter; network protocols and lyric parsing live in the lyrics DLL.
use lyric_core::models::*;use serde_json::{Value,json};use crate::http::RequestContext;
pub fn text(v:&Value,key:&str)->String{v.get(key).map(|v|match v{Value::String(s)=>s.clone(),Value::Number(n)=>n.to_string(),_=>String::new()}).unwrap_or_default()}
pub struct OnlineSource{pub id:Provider,pub context:RequestContext}
impl OnlineSource{
pub fn new(id:&str,context:RequestContext)->Self{Self{id:Provider::new(id),context}}
pub fn search(&self,track:&Track)->Result<Vec<Hit>,String>{self.context.check()?;let millis=self.context.deadline.saturating_duration_since(std::time::Instant::now()).as_millis().min(10000)as u64;let value=dotisle_module_api::call("lyrics","provider.search",json!({"provider":self.id.value(),"track":track,"timeoutMs":millis}))?;self.context.check()?;serde_json::from_value(value).map_err(|e|e.to_string())}
}
