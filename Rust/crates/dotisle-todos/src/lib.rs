mod quick_reminder;mod todos{pub use crate::engine::*;}
use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod engine;
fn strarg<'a>(a:&'a Value,k:&str)->Result<&'a str,String>{a[k].as_str().ok_or_else(||format!("missing {k}"))}
thread_local!{static STORES:RefCell<BTreeMap<u64,engine::Store>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;match method{
"default_color"=>Ok(json!(engine::default_color())),"now"=>Ok(json!(engine::Now::local())),
"done"=>{let todo:engine::Todo=serde_json::from_value(a["todo"].clone()).map_err(|e|e.to_string())?;Ok(json!(todo.done(&serde_json::from_value(a["now"].clone()).map_err(|e|e.to_string())?)))},
"valid_date"=>Ok(json!(engine::valid_date(strarg(a,"text")?))),"valid_time"=>Ok(json!(engine::valid_time(strarg(a,"text")?))),
"add_minutes"=>Ok(json!(engine::add_minutes(strarg(a,"date")?,strarg(a,"time")?,a["minutes"].as_u64().ok_or("minutes")? as u32))),
"store.new"=>{let s=engine::Store::new(std::path::PathBuf::from(strarg(a,"path")?));let h=NEXT.with(|n|{let h=n.get();n.set(h+1);h});let value=json!({"handle":h,"items":s.items,"error":s.error});STORES.with(|m|m.borrow_mut().insert(h,s));Ok(value)},
"store.drop"=>{STORES.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"store.list"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok(s.list(&serde_json::from_value(a["now"].clone()).map_err(|e|e.to_string())?));let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.save"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.save(&a["v"]);let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.complete"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.complete(strarg(a,"id")?,a["done"]==true,&serde_json::from_value(a["now"].clone()).map_err(|e|e.to_string())?);let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.snooze"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.snooze(strarg(a,"id")?,a["minutes"].as_u64().ok_or("minutes")? as u32,&serde_json::from_value(a["now"].clone()).map_err(|e|e.to_string())?);let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.restore"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.restore(&a["v"]);let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.delete"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.delete(strarg(a,"id")?);let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.poll"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=s.poll(&serde_json::from_value(a["now"].clone()).map_err(|e|e.to_string())?);let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
 "parse"=>{let now=serde_json::from_value(a["now"].clone()).map_err(|e|e.to_string())?;match quick_reminder::parse(strarg(a,"text")?,&now){Ok(draft)=>Ok(json!({"draft":draft})),Err(e)=>Ok(json!({"error":e.code,"message":e.message}))}},
 _=>Err(format!("unknown module method: {method}"))}}
dotisle_module_api::export_service!("todos",dispatch);
