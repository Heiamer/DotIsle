use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod notifications;mod notification_store;mod app_icon;
thread_local!{static LISTENERS:RefCell<BTreeMap<u64,notifications::Listener>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];match v["method"].as_str().unwrap_or(""){
"example"=>Ok(json!(notifications::Notice::example(std::path::Path::new(a["base"].as_str().ok_or("base")?)))),
"policy"=>Ok(json!(notifications::Policy::from_settings(a))),"permits"=>{let p:notifications::Policy=serde_json::from_value(a["policy"].clone()).map_err(|e|e.to_string())?;Ok(json!(p.permits(a["id"].as_str().ok_or("id")?,a["name"].as_str().ok_or("name")?)))},
"should_show"=>{let p:notifications::Policy=serde_json::from_value(a.clone()).map_err(|e|e.to_string())?;Ok(json!(p.should_show()))},"diagnostics"=>Ok(notifications::diagnostics()),
"shell_icon"=>Ok(json!(app_icon::shell_icon(a["id"].as_str().ok_or("id")?))),
"listener.new"=>{let p=serde_json::from_value(a.clone()).map_err(|e|e.to_string())?;let id=NEXT.with(|n|{let id=n.get();n.set(id+1);id});LISTENERS.with(|m|m.borrow_mut().insert(id,notifications::Listener::new(p)));Ok(json!({"handle":id}))},
"listener.drop"=>{LISTENERS.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
method=>LISTENERS.with(|m|{let m=m.borrow();let listener=m.get(&a["handle"].as_u64().ok_or("handle")?).ok_or("listener missing")?;match method{
"listener.configure"=>{listener.configure(serde_json::from_value(a["policy"].clone()).map_err(|e|e.to_string())?);Ok(Value::Null)},"listener.poll"=>Ok(json!(listener.events.try_iter().take(32).collect::<Vec<_>>())),_=>Err("unknown notification method".into())}})
}}
dotisle_module_api::export_service!("notifications",dispatch);
