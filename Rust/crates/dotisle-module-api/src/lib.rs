//! Stable, bounded C/JSON service ABI. DLL-owned buffers are freed by that DLL.
use serde_json::{json,Value};
use std::{ffi::{c_void,CString},path::PathBuf,sync::{Arc,Mutex,OnceLock},collections::BTreeMap};
pub const ABI:u32=1;
pub const CORE_API:u32=1;
pub const LIMIT:usize=4*1024*1024;
static INITIALIZATION_ERRORS:OnceLock<Mutex<BTreeMap<String,String>>>=OnceLock::new();
pub fn initialization_errors()->Value{INITIALIZATION_ERRORS.get_or_init(Default::default).lock().map(|m|json!(*m)).unwrap_or_else(|_|json!({"registry":"error registry unavailable"}))}
pub const MODULES:&[(&str,&str)]=&[("core","CORE 核心"),("settings","控制中心"),("focus","计时与流程"),("todos","待办与快捷提醒"),("music","音乐与播放器"),("lyrics","歌词检索与同步"),("notifications","通知接收"),("history","通知历史"),("automation","应用提醒规则"),("devices","设备连接提醒"),("osd","系统状态提示"),("sound","音效"),("widgets","桌面组件"),("themes","主题与配色"),("glow","外围光效")];
pub fn metadata(id:&str,version:&str)->Value {let dependencies=match id{"music"=>vec!["lyrics"],"widgets"=>vec!["focus"],_=>vec![]};json!({"id":id,"version":version,"abi":ABI,"coreApi":CORE_API,"coreMin":CORE_API,"coreMax":CORE_API,"dataSchema":1,"dependencies":dependencies})}
pub fn filename(id:&str)->Option<String>{MODULES.iter().any(|(key,_)|*key==id).then(||format!("dotisle_{id}.dll"))}
#[repr(C)]
pub struct Buffer {pub ptr:*mut u8,pub len:usize}
impl Buffer {
 pub fn from_value(value:Value)->Self {let bytes=serde_json::to_vec(&value).unwrap_or_else(|_|b"null".to_vec()).into_boxed_slice();let len=bytes.len();Self{ptr:Box::into_raw(bytes)as *mut u8,len}}
 pub unsafe fn release(self) {if !self.ptr.is_null(){drop(Box::from_raw(std::ptr::slice_from_raw_parts_mut(self.ptr,self.len)));}}
}
pub unsafe fn serve(ptr:*const u8,len:usize,dispatch:impl FnOnce(Value)->Result<Value,String>+std::panic::UnwindSafe)->Buffer {
 if ptr.is_null()||len>LIMIT{return Buffer::from_value(json!({"ok":false,"error":"invalid module request"}));}
 let result=std::panic::catch_unwind(||->Result<Value,String>{let value=serde_json::from_slice(std::slice::from_raw_parts(ptr,len)).map_err(|e|e.to_string())?;dispatch(value)});
 Buffer::from_value(match result{Ok(Ok(value))=>json!({"ok":true,"value":value}),Ok(Err(error))=>json!({"ok":false,"error":error}),Err(_)=>json!({"ok":false,"error":"module panicked"})})
}
#[macro_export]
macro_rules! export_service {($id:expr,$dispatch:path)=>{
 #[no_mangle]pub unsafe extern "C" fn dotisle_service_call(ptr:*const u8,len:usize)->$crate::Buffer {
  $crate::serve(ptr,len,|value|{if value["method"]=="describe"{Ok($crate::metadata($id,env!("CARGO_PKG_VERSION")))}else if value["method"]=="health"{$crate::health::run($id,$dispatch)}else{$dispatch(value)}})
 }
 #[no_mangle]pub unsafe extern "C" fn dotisle_service_free(buffer:$crate::Buffer){buffer.release();}
};}
type Call=unsafe extern "C" fn(*const u8,usize)->Buffer;
type Free=unsafe extern "C" fn(Buffer);
struct Library { _handle:usize,call:Call,free:Free }
#[link(name="kernel32")]extern "system" {fn LoadLibraryExW(path:*const u16,file:*mut c_void,flags:u32)->*mut c_void;fn GetProcAddress(module:*mut c_void,name:*const u8)->*mut c_void;}
impl Library {
 fn invoke(&self,value:&Value)->Result<Value,String> {
  let input=serde_json::to_vec(value).map_err(|e|e.to_string())?;if input.len()>LIMIT{return Err("模块请求过大".into());}
  unsafe {let buffer=(self.call)(input.as_ptr(),input.len());if buffer.ptr.is_null(){return Err("模块返回空缓冲区".into());}
   let result=if buffer.len>LIMIT{Err("模块响应过大".into())}else{serde_json::from_slice::<Value>(std::slice::from_raw_parts(buffer.ptr,buffer.len)).map_err(|e|e.to_string())};(self.free)(buffer);
   let reply=result?;if reply["ok"]==true{Ok(reply["value"].clone())}else{Err(reply["error"].as_str().unwrap_or("模块操作失败").into())}
  }
 }
 fn open(id:&str)->Result<Self,String>{
  use std::os::windows::ffi::OsStrExt;
  if !id.bytes().all(|b|b.is_ascii_lowercase()||b==b'_'){return Err("模块标识无效".into());}
  let directory=std::env::var_os("DOTISLE_MODULE_DIR").map(PathBuf::from).or_else(||std::env::current_exe().ok().and_then(|p|p.parent().map(PathBuf::from))).ok_or("模块目录不可用")?;
  let path=directory.join(format!("dotisle_{id}.dll"));let wide=path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();
  unsafe {let handle=LoadLibraryExW(wide.as_ptr(),std::ptr::null_mut(),0x100|0x1000);if handle.is_null(){return Err(format!("无法加载 {}：{}",path.display(),std::io::Error::last_os_error()));}
   let symbol=|name:&str|GetProcAddress(handle,CString::new(name).unwrap().as_ptr().cast());let call=symbol("dotisle_service_call");let free=symbol("dotisle_service_free");
   if call.is_null()||free.is_null(){return Err("模块缺少服务接口".into());}
   let library=Self{_handle:handle as usize,call:std::mem::transmute(call),free:std::mem::transmute(free)};
   let manifest=library.invoke(&json!({"method":"describe"}))?;
   if manifest["abi"]!=ABI||manifest["id"]!=id||manifest["coreMin"].as_u64().is_none_or(|n|n>CORE_API as u64)||manifest["coreMax"].as_u64().is_none_or(|n|n<CORE_API as u64){return Err("模块与 CORE 不兼容".into());}
   Ok(library)
  }
 }
}
#[derive(Clone)]pub struct Client(Arc<Library>);
impl Client {
 pub fn open(id:&str)->Result<Self,String>{
  static LIBRARIES:OnceLock<Mutex<BTreeMap<String,Arc<Library>>>>=OnceLock::new();let mut libraries=LIBRARIES.get_or_init(Default::default).lock().map_err(|_|"模块列表不可用")?;
  if let Some(l)=libraries.get(id){return Ok(Self(l.clone()));}let library=Arc::new(Library::open(id)?);libraries.insert(id.into(),library.clone());Ok(Self(library))
 }
 pub fn call(&self,method:&str,args:Value)->Result<Value,String>{self.0.invoke(&json!({"method":method,"args":args}))}
 pub fn describe(&self)->Result<Value,String>{self.0.invoke(&json!({"method":"describe"}))}
}
pub fn call(id:&str,method:&str,args:Value)->Result<Value,String>{Client::open(id)?.call(method,args)}
pub fn versions()->Value{static V:OnceLock<Value>=OnceLock::new();V.get_or_init(||{let mut result=json!({});for(id,_)in MODULES{if *id=="core"{continue;}if let Ok(v)=Client::open(id).and_then(|c|c.describe()){result[*id]=v["version"].clone();}}result}).clone()}
pub struct Session {client:Result<Client,String>,pub handle:u64,service:&'static str,owner:std::thread::ThreadId,shared:bool}
impl Session {
 pub fn new(module:&str,service:&'static str,args:Value)->(Self,Result<Value,String>){
  let client=Client::open(module);let reply=client.as_ref().map_err(Clone::clone).and_then(|c|c.call(&format!("{service}.new"),args)).and_then(|v|if v["handle"].as_u64().is_some_and(|h|h!=0){Ok(v)}else{Err("模块没有创建有效会话".into())});
  let key=format!("{module}.{service}");if let Ok(mut errors)=INITIALIZATION_ERRORS.get_or_init(Default::default).lock(){match &reply{Ok(_)=>{errors.remove(&key);},Err(e)=>{errors.insert(key,e.clone());}}}
  let handle=reply.as_ref().ok().and_then(|v|v["handle"].as_u64()).unwrap_or(0);(Self{client,handle,service,owner:std::thread::current().id(),shared:matches!(module,"music"|"lyrics")},reply)
 }
 pub fn invoke(&self,method:&str,mut args:Value)->Result<Value,String>{if !self.shared&&std::thread::current().id()!=self.owner{return Err("线程绑定的模块会话不能在其他线程使用".into());}args["handle"]=json!(self.handle);self.client.as_ref().map_err(Clone::clone)?.call(&format!("{}.{method}",self.service),args)}
}
impl Drop for Session {fn drop(&mut self){if self.handle!=0{let _=self.invoke("drop",json!({}));}}}
#[cfg(test)]mod tests {
 use super::*;
 #[test]fn response_ownership_and_panic_are_contained(){let bytes=b"{}";let b=unsafe{serve(bytes.as_ptr(),bytes.len(),|_|Ok(json!({"test":7})))};let value:Value=unsafe{serde_json::from_slice(std::slice::from_raw_parts(b.ptr,b.len)).unwrap()};assert_eq!(value["value"]["test"],7);unsafe{b.release();}let b=unsafe{serve(bytes.as_ptr(),bytes.len(),|_|panic!("contained"))};let value:Value=unsafe{serde_json::from_slice(std::slice::from_raw_parts(b.ptr,b.len)).unwrap()};assert_eq!(value["ok"],false);unsafe{b.release();}}
}

pub mod inspection;

pub mod health;
