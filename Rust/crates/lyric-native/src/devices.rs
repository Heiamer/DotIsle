use serde_json::{json,Value};
use std::time::Instant;
#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash,serde::Serialize,serde::Deserialize)]
pub enum Kind{Usb,Volume,Phone,Headphones,Speaker,Gamepad,Mouse,Keyboard}
impl Kind{
    pub fn key(self)->&'static str{match self{Self::Usb=>"Usb",Self::Volume=>"Volume",Self::Phone=>"Phone",Self::Headphones=>"Headphones",Self::Speaker=>"Speaker",Self::Gamepad=>"Gamepad",Self::Mouse=>"Mouse",Self::Keyboard=>"Keyboard"}}
    pub fn label(self)->&'static str{match self{Self::Usb=>"USB 设备",Self::Volume=>"移动磁盘",Self::Phone=>"手机 / 便携设备",Self::Headphones=>"耳机",Self::Speaker=>"音响",Self::Gamepad=>"手柄",Self::Mouse=>"鼠标",Self::Keyboard=>"键盘"}}
    /// The three families that existed before 2.0 keep their own filters and disconnect behaviour.
    pub fn legacy(self)->bool{matches!(self,Self::Usb|Self::Volume|Self::Phone)}
}
#[derive(serde::Serialize,serde::Deserialize)]pub struct Gates{pub master:bool,pub disconnect:bool,pub headphones:bool,pub speaker:bool,pub gamepad:bool,pub mouse:bool,pub keyboard:bool}
#[derive(Clone,Debug,serde::Serialize,serde::Deserialize)]pub struct Event{pub key:String,pub kind:Kind,pub connected:bool,pub label:String,#[serde(skip,default="Instant::now")]at:Instant}
impl Event{pub fn new(key:String,kind:Kind,connected:bool,label:String)->Self{Self{key,kind,connected,label,at:Instant::now()}}pub fn notice(&self)->crate::notifications::Notice{dotisle_module_api::call("devices","notice",json!(self)).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}}

pub struct Hub{session:dotisle_module_api::Session}
impl Default for Hub{fn default()->Self{let(session,_)=dotisle_module_api::Session::new("devices","hub",json!({}));Self{session}}}
impl Hub{
pub unsafe fn register(&mut self,hwnd:windows::Win32::Foundation::HWND)->(){self.session.invoke("register",json!({"hwnd":hwnd.0 as usize})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub unsafe fn input_change(&mut self,action:u32,handle:isize)->(){self.session.invoke("input_change",json!({"action":action,"rawHandle":handle})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub unsafe fn message(&mut self,action:usize,data:isize)->(){self.session.invoke("message",json!({"action":action,"data":data})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn record(&mut self,event:Event)->(){self.session.invoke("record",json!({"event":event})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn poll(&mut self,gates:&Gates)->Vec<Event>{self.session.invoke("poll",json!({"gates":gates})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
}
