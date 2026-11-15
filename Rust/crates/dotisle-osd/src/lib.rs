use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod engine;mod notifications;
thread_local!{static HUBS:RefCell<BTreeMap<u64,engine::Hub>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;match method{
"hub.new"=>{let id=NEXT.with(|n|{let id=n.get();n.set(id+1);id});HUBS.with(|m|m.borrow_mut().insert(id,engine::Hub::default()));Ok(json!({"handle":id}))},
"hub.drop"=>{HUBS.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"notice"=>{let hint:engine::Hint=serde_json::from_value(a.clone()).map_err(|e|e.to_string())?;Ok(json!(hint.notice()))},
_=>HUBS.with(|m|{let mut m=m.borrow_mut();let h=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("hub missing")?;match method{
"hub.install"=>{unsafe{h.install(windows::Win32::Foundation::HWND(a["hwnd"].as_u64().ok_or("hwnd")? as *mut std::ffi::c_void));}Ok(Value::Null)},
"hub.uninstall"=>{h.uninstall();Ok(Value::Null)},
"hub.resync"=>{h.resync();Ok(Value::Null)},
"hub.flush"=>{Ok(json!(h.flush(a["pending"].as_u64().ok_or("pending")? as u32,a["caps"]==true,a["num"]==true,a["mic"]==true)))},
"hub.poll"=>{Ok(json!(h.poll(a["mic"]==true,a["touch"]==true)))},
"hub.status"=>{Ok(h.status())},
 _=>Err("unknown hub method".into())}})}}
dotisle_module_api::export_service!("osd",dispatch);
