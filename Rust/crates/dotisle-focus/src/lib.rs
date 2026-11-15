use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod engine;mod activities;
fn strarg<'a>(a:&'a Value,k:&str)->Result<&'a str,String>{a[k].as_str().ok_or_else(||format!("missing {k}"))}
thread_local!{static STORES:RefCell<BTreeMap<u64,engine::Store>>=RefCell::new(BTreeMap::new());static TEMPLATES:RefCell<BTreeMap<u64,engine::Templates>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn next()->u64{NEXT.with(|n|{let id=n.get();n.set(id+1);id})}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;match method{
"format_time"=>Ok(json!(engine::format_time(a["s"].as_u64().ok_or("s")?))),"stages_from"=>Ok(json!(engine::stages_from(a)?)),"stages_ok"=>Ok(json!(engine::stages_ok(&serde_json::from_value::<Vec<engine::Stage>>(a.clone()).map_err(|e|e.to_string())?))),
"timer.primary_action"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.primary_action()))},
"timer.panel"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.panel()))},
"timer.limit_ms"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.limit_ms()))},
"timer.remaining_ms"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.remaining_ms()))},
"timer.clock"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.clock()))},
"timer.progress"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.progress()))},
"timer.subtitle"=>{let t:engine::Timer=serde_json::from_value(a["timer"].clone()).map_err(|e|e.to_string())?;Ok(json!(t.subtitle()))},
"store.new"=>{if cfg!(feature="module-broken")&&a["path"].as_str().is_some_and(|p|!p.is_empty()){return Err("injected startup failure for rollback acceptance".into());}let s=engine::Store::new(std::path::PathBuf::from(strarg(a,"path")?));let h=next();let result=json!({"handle":h,"items":s.items,"error":s.error});STORES.with(|m|m.borrow_mut().insert(h,s));Ok(result)},
"store.drop"=>{STORES.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"store.persist"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.persist();let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.list"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok(probe_list(s.list()));let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.create"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.create(&a["v"]);let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.action"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.action(strarg(a,"id")?,strarg(a,"action")?);let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.act"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.act(strarg(a,"id")?,strarg(a,"action")?,a["revision"].as_u64());let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.set_direction"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.set_direction(strarg(a,"id")?,a["count_up"]==true,a["revision"].as_u64());let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.advance"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok(s.advance(a["ms"].as_u64().ok_or("ms")?));let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.pause_for_lock"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.pause_for_lock(a["generation"].as_u64().ok_or("generation")?);let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.resume_focus"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.resume_focus(&serde_json::from_value::<Vec<String>>(a["ids"].clone()).map_err(|e|e.to_string())?,a["generation"].as_u64().ok_or("generation")?);let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.clear_lock_pauses"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.clear_lock_pauses();let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"templates.new"=>{let s=engine::Templates::new(std::path::PathBuf::from(strarg(a,"path")?));let h=next();let result=json!({"handle":h,"items":s.items,"error":s.error});TEMPLATES.with(|m|m.borrow_mut().insert(h,s));Ok(result)},
"templates.drop"=>{TEMPLATES.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"templates.persist"=>TEMPLATES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.persist();let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"templates.list"=>TEMPLATES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok(probe_list(s.list()));let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"templates.save"=>TEMPLATES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.save(&a["v"]);let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"templates.delete"=>TEMPLATES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;
if a["items"].is_array(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.delete(strarg(a,"id")?);let (result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};
Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
 _=>Err(format!("unknown focus method: {method}"))}}
dotisle_module_api::export_service!("focus",dispatch);

fn probe_list(value:Value)->Value{
 #[cfg(feature="module-probe")]{let mut value=value;for t in value.as_array_mut().into_iter().flatten(){let text=t["subtitle"].as_str().unwrap_or("");t["subtitle"]=json!(format!("{text} [DLL-PROBE]"));}value}
 #[cfg(not(feature="module-probe"))]{value}
}
