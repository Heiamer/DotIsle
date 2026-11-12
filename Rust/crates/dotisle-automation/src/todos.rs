use serde::{Serialize,Deserialize};
#[derive(Clone,Debug,serde::Serialize,serde::Deserialize)]pub struct Now{pub date:String,pub time:String}
impl Now{pub fn local()->Self{let t=unsafe{windows::Win32::System::SystemInformation::GetLocalTime()};Self{date:format!("{:04}-{:02}-{:02}",t.wYear,t.wMonth,t.wDay),time:format!("{:02}:{:02}",t.wHour,t.wMinute)}}}