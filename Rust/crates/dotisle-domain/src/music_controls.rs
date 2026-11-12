use serde_json::json;
#[derive(Clone,Copy,Debug,serde::Serialize,serde::Deserialize)]pub struct SkipColors{pub background:u32,pub foreground:u32}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:serde_json::Value)->T{serde_json::from_value(dotisle_module_api::call("themes",method,args).expect("theme module unavailable")).expect("theme response")}
pub fn on_surface(surface:u32,preferred:u32)->u32{request("controls.on_surface",json!({"surface":surface,"preferred":preferred}))}
pub fn tinted_on_surfaces(surfaces:&[u32],preferred:u32)->u32{request("controls.tinted",json!({"surfaces":surfaces,"preferred":preferred}))}
pub fn skip_colors(surface:u32,preferred:u32)->SkipColors{request("controls.skip",json!({"surface":surface,"preferred":preferred}))}
