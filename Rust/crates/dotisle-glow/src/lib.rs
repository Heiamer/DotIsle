use serde_json::{json,Value};use std::{cell::RefCell,collections::BTreeMap};mod engine;
thread_local!{static STATES:RefCell<BTreeMap<u64,engine::State>>=RefCell::new(BTreeMap::new());static NEXT:std::cell::Cell<u64>=const{std::cell::Cell::new(1)};}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;match method{
"state.new"=>{let id=NEXT.with(|n|{let id=n.get();n.set(id+1);id});STATES.with(|m|m.borrow_mut().insert(id,engine::State::default()));Ok(json!({"handle":id}))},
"state.drop"=>{STATES.with(|m|m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));Ok(Value::Null)},
"paint"=>{let f:engine::Frame=serde_json::from_value(a["frame"].clone()).map_err(|e|e.to_string())?;if f.kind==engine::Kind::Off||f.strength<=0.001{return Ok(json!([]));}let accent=a["accent"].as_u64().ok_or("accent")? as u32;let colors=if f.kind==engine::Kind::Update{vec![0x4285F4,0xA17BFF,0xEA6A8D,0xF8C46A,0x66C49B,0x61C3E8,0x4285F4]}else{vec![accent]};
Ok(json!([(19.,0.018),(14.,0.032),(10.,0.055),(6.,0.10),(2.5,0.30)].into_iter().map(|(width,opacity)|json!({"op":"roundedStroke","bounds":a["bounds"],"radius":a["radius"],"inflate":1.,"width":width,"alpha":opacity*f.strength,"colors":colors,"phase":f.phase})).collect::<Vec<_>>()))},
_=>STATES.with(|m|{let mut m=m.borrow_mut();let s=m.get_mut(&a["handle"].as_u64().ok_or("handle")?).ok_or("glow session missing")?;match method{
"state.tick"=>Ok(json!(s.tick(a["dt"].as_f64().ok_or("dt")?,serde_json::from_value(a["target"].clone()).map_err(|e|e.to_string())?,a["animate"]==true))),"state.active"=>Ok(json!(s.active())),"state.frame"=>Ok(json!(s.frame(a["time_ms"].as_f64().ok_or("time_ms")?,a["animate"]==true))),_=>Err("unknown glow method".into())}})
}}
dotisle_module_api::export_service!("glow",dispatch);
