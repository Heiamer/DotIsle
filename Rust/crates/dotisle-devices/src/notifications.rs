#[derive(Clone,Debug,Default,serde::Serialize,serde::Deserialize)]
pub struct Notice {pub app_id:String,pub app_name:String,pub title:String,pub body:String,pub icon:Option<Vec<u8>>}