use serde_json::{json,Value};
pub const WM_OSD:u32=windows::Win32::UI::WindowsAndMessaging::WM_APP+3;pub const OSD_TIMER:usize=2;
#[derive(Clone,Copy,Debug,PartialEq,Eq,serde::Serialize,serde::Deserialize)]
pub enum Kind{Caps,Num,Mic,Touchpad}
impl Kind{
    pub fn key(self)->&'static str{match self{Self::Caps=>"Caps",Self::Num=>"Num",Self::Mic=>"Mic",Self::Touchpad=>"Touchpad"}}
    pub fn label(self)->&'static str{match self{Self::Caps=>"大写锁定",Self::Num=>"小键盘锁定",Self::Mic=>"麦克风",Self::Touchpad=>"触控板"}}
    pub fn state(self,on:bool)->&'static str{match self{Self::Mic=>if on{"已静音"}else{"已取消静音"},_=>if on{"已开启"}else{"已关闭"}}}
    pub fn bit(self)->u32{match self{Self::Caps=>1,Self::Num=>2,Self::Mic=>4,Self::Touchpad=>8}}
}
#[derive(Clone,Copy,Debug,serde::Serialize,serde::Deserialize)]
pub struct Hint{pub kind:Kind,pub on:bool}
impl Hint{pub fn notice(self)->crate::notifications::Notice{dotisle_module_api::call("osd","notice",json!(self)).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}}

pub struct Hub{session:dotisle_module_api::Session}
impl Default for Hub{fn default()->Self{let(session,_)=dotisle_module_api::Session::new("osd","hub",json!({}));Self{session}}}
impl Hub{
pub unsafe fn install(&mut self,hwnd:windows::Win32::Foundation::HWND)->(){self.session.invoke("install",json!({"hwnd":hwnd.0 as usize})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn uninstall(&mut self)->(){self.session.invoke("uninstall",json!({})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn resync(&mut self)->(){self.session.invoke("resync",json!({})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn flush(&mut self,pending:u32,caps_on:bool,num_on:bool,mic_on:bool)->Vec<Hint>{self.session.invoke("flush",json!({"pending":pending,"caps":caps_on,"num":num_on,"mic":mic_on})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn poll(&mut self,mic_on:bool,touch_on:bool)->Vec<Hint>{self.session.invoke("poll",json!({"mic":mic_on,"touch":touch_on})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Default::default())}
pub fn status(&self)->Value{self.session.invoke("status",json!({})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_else(||Value::Null)}
}
