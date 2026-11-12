use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap,path::PathBuf};mod widget_data;mod widget_metrics;mod weather_http;mod timers;mod activities;pub mod desktop_widgets;pub mod widget_config;
struct State{data:widget_data::Hub,metrics:widget_metrics::Hub,placements:desktop_widgets::Store}
thread_local!{static STATES:RefCell<BTreeMap<u64,State>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn parse<T:serde::de::DeserializeOwned>(a:&Value,k:&str)->Result<T,String>{serde_json::from_value(a[k].clone()).map_err(|e|e.to_string())}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;
match method{
"launch"=>return Ok(widget_data::launch(&parse::<Vec<desktop_widgets::Placement>>(a,"placements")?,a["id"].as_str().ok_or("id")?)),
"host_running"=>return Ok(json!(widget_metrics::host_running(std::path::Path::new(a["data"].as_str().ok_or("data")?)))),
_=>{}}
if method.ends_with(".new") {let path=PathBuf::from(a["path"].as_str().unwrap_or(""));let id=NEXT.with(|n|{let id=n.get();n.set(id+1);id});let s=State{data:widget_data::Hub::new(if method=="data.new"{path.clone()}else{PathBuf::new()}),metrics:Default::default(),placements:desktop_widgets::Store::new(if method=="placements.new"{path}else{PathBuf::new()})};let result=json!({"handle":id,"value":s.data.value,"items":s.placements.items,"error":s.placements.error});STATES.with(|m|m.borrow_mut().insert(id,s));return Ok(result);}
if method.ends_with(".drop"){STATES.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));return Ok(Value::Null);}
STATES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("widget session")?;
if method.starts_with("placements."){
 if a["items"].is_array(){s.placements.items=parse(a,"items")?;}
 let p=&mut s.placements;let result=match method{"placements.add"=>p.add(&a["value"]).map(|v|json!(v)),"placements.update"=>p.update(&a["value"]).map(|v|json!(v)),"placements.remove"=>p.remove(a["id"].as_str().ok_or("id")?).map(|v|json!(v)),"placements.persist"=>p.persist().map(|v|json!(v)),"placements.list"=>Ok(p.list()),_=>Err("unknown widget placement method".into())};let(result,error)=match result{Ok(v)=>(v,None),Err(e)=>(Value::Null,Some(e))};return Ok(json!({"items":p.items,"error":p.error,"result":result,"operationError":error}));
}
match method{
"data.advance_focus"=>s.data.advance_focus(&parse::<Vec<timers::Timer>>(a,"timers")?,parse(a,"elapsed")?),"data.flush"=>s.data.flush(),"data.poll"=>s.data.poll(&parse::<Vec<desktop_widgets::Placement>>(a,"placements")?),
"metrics.poll"=>{s.metrics.poll();return Ok(json!(s.metrics.value));},_=>return Err("unknown widget method".into())};Ok(json!({"value":s.data.value}))
})}
dotisle_module_api::export_service!("widgets",dispatch);
