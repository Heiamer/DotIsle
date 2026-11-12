use serde_json::{json,Value};
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct Scheme {
    pub primary:u32, pub on_primary:u32, pub primary_container:u32, pub on_primary_container:u32,
    pub secondary_container:u32, pub on_secondary_container:u32, pub tertiary_container:u32,
    pub on_tertiary_container:u32, pub surface:u32, pub container:u32, pub high:u32,
    pub on_surface:u32, pub on_surface_variant:u32, pub outline:u32, pub outline_variant:u32, pub error:u32,
}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->T{serde_json::from_value(dotisle_module_api::call("themes",method,args).expect("theme module unavailable")).expect("theme module response schema")}
pub use crate::dynamic_color::scheme;
pub fn seed(rgba:&[u8])->Option<u32>{request("seed",json!({"rgba":rgba}))}
pub fn contrast(a:u32,b:u32)->f64{request("contrast",json!({"a":a,"b":b}))}
