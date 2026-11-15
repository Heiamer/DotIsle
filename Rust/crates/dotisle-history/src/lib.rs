use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod engine;
fn strarg<'a>(a:&'a Value,k:&str)->Result<&'a str,String>{a[k].as_str().ok_or_else(||format!("missing {k}"))}
thread_local!{static STORES:RefCell<BTreeMap<u64,engine::Store>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;match method{
"store.new"=>{let s=engine::Store::new(std::path::PathBuf::from(strarg(a,"path")?));let h=NEXT.with(|n|{let h=n.get();n.set(h+1);h});let value=json!({"handle":h,"items":s.items,"error":s.error});STORES.with(|m|m.borrow_mut().insert(h,s));Ok(value)},
"store.drop"=>{STORES.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"store.list"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok(s.list());let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.record"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok({s.record(strarg(a,"kind")?,strarg(a,"app_id")?,strarg(a,"app_name")?,strarg(a,"title")?,strarg(a,"body")?,a["todo_id"].as_str().map(str::to_owned));});let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.prune"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok({s.prune(a["days"].as_u64().ok_or("days")? as u32,a["now"].as_i64().ok_or("now")?);});let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
"store.clear"=>STORES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("module session missing")?;if !a["items"].is_null(){s.items=serde_json::from_value(a["items"].clone()).map_err(|e|e.to_string())?;}
let operation:Result<_,String>=Ok({s.clear(a["kind"].as_str());});let(result,error)=match operation{Ok(value)=>(json!(value),None),Err(e)=>(Value::Null,Some(e))};Ok(json!({"items":s.items,"error":s.error,"result":result,"operationError":error}))}),
 _=>Err(format!("unknown module method: {method}"))}}
dotisle_module_api::export_service!("history",dispatch);
mod todos;