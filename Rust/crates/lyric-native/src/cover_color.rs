use serde_json::{json,Value};
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
pub struct CoverPalette{pub background:u32,pub accent:u32}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->T{serde_json::from_value(dotisle_module_api::call("themes",method,args).expect("theme module unavailable")).expect("theme module response schema")}
pub const NEUTRAL_BACKGROUND:u32=0x1b1f25;
impl Default for CoverPalette{fn default()->Self{request("cover.default",json!({}))}}
pub fn palette(bgra:&[u8])->CoverPalette{request("cover.palette",json!({"bgra":bgra}))}
pub fn mix(a:u32,b:u32,t:f64)->u32{request("cover.mix",json!({"a":a,"b":b,"t":t}))}
pub fn luminance(c:u32)->f64{request("cover.luminance",json!({"c":c}))}
pub fn contrast(a:u32,b:u32)->f64{request("cover.contrast",json!({"a":a,"b":b}))}
