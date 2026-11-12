use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod engine;mod notifications;
thread_local!{static HUBS:RefCell<BTreeMap<u64,engine::Hub>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;match method{
"hub.new"=>{let id=NEXT.with(|n|{let id=n.get();n.set(id+1);id});HUBS.with(|m|m.borrow_mut().insert(id,engine::Hub::default()));Ok(json!({"handle":id}))},
"hub.drop"=>{HUBS.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"notice"=>{let event:engine::Event=serde_json::from_value(a.clone()).map_err(|e|e.to_string())?;Ok(json!(event.notice()))},
_=>HUBS.with(|m|{let mut m=m.borrow_mut();let h=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("hub missing")?;match method{
"hub.register"=>{unsafe{h.register(windows::Win32::Foundation::HWND(a["hwnd"].as_u64().ok_or("hwnd")? as *mut std::ffi::c_void));}Ok(Value::Null)},
"hub.input_change"=>{unsafe{h.input_change(a["action"].as_u64().ok_or("action")? as u32,a["rawHandle"].as_i64().ok_or("rawHandle")? as isize);}Ok(Value::Null)},
"hub.message"=>{unsafe{h.message(a["action"].as_u64().ok_or("action")? as usize,a["data"].as_i64().ok_or("data")? as isize);}Ok(Value::Null)},
"hub.record"=>{h.record(serde_json::from_value(a["event"].clone()).map_err(|e|e.to_string())?);Ok(Value::Null)},
"hub.poll"=>{Ok(json!(h.poll(&serde_json::from_value(a["gates"].clone()).map_err(|e|e.to_string())?)))},
 _=>Err("unknown hub method".into())}})}}
dotisle_module_api::export_service!("devices",dispatch);
