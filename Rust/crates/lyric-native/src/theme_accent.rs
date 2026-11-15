use serde_json::{json,Value};
#[derive(Clone,Copy,serde::Serialize,serde::Deserialize)]pub struct Accent{pub fill:u32,pub hover:u32,pub pressed:u32,pub ink:u32}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->T{serde_json::from_value(dotisle_module_api::call("themes",method,args).expect("theme module unavailable")).expect("theme module response schema")}
pub fn resolve(settings:&Value)->Accent{request("accent.resolve",settings.clone())}
pub fn system_dark()->bool{request("accent.system_dark",json!({}))}
pub fn system_accent()->Option<u32>{request("accent.system_accent",json!({}))}
