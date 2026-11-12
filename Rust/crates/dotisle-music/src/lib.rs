pub mod updates;
pub mod http;
mod lyrics_client;pub mod providers;
pub mod storage;
pub mod media;
pub mod netease;
pub mod netease_control;
mod netease_window;
mod netease_attention;
mod netease_cdp;
mod netease_native;
mod media_commands;
pub mod clock;
pub mod io;
pub mod runtime;
pub mod duration;
pub mod shell;
pub mod ipc;


mod artwork;

use serde_json::{json,Value};use std::{collections::BTreeMap,sync::{Mutex,OnceLock,Arc},sync::atomic::{AtomicU64,Ordering}};
fn dispatch(v:Value)->Result<Value,String>{
 static BACKENDS:OnceLock<Mutex<BTreeMap<u64,Arc<runtime::Backend>>>>=OnceLock::new();static NEXT:AtomicU64=AtomicU64::new(1);
 let a=&v["args"];let method=v["method"].as_str().ok_or("method")?;
 if method=="attention"{return Ok(netease_control::attention_diagnostics());}
 if method=="diagnostics"{return media::control_diagnostics().map_err(|e|e.to_string());}
 let mut backends=BACKENDS.get_or_init(Default::default).lock().map_err(|_|"music registry lock")?;
 if method=="backend.new"{let p=|key:&str|a[key].as_str().map(std::path::PathBuf::from).ok_or_else(||format!("missing {key}"));let backend=runtime::Backend::new(a["settings"].clone(),p("data")?,p("local")?,p("logs")?);let id=NEXT.fetch_add(1,Ordering::Relaxed);backends.insert(id,backend);return Ok(json!({"handle":id}));}
 let id=a["handle"].as_u64().ok_or("music session")?;
 if method=="backend.drop"{if let Some(backend)=backends.remove(&id){backend.close();}return Ok(Value::Null);}
 let b=backends.get(&id).ok_or("music session closed")?;match method{
 "backend.close"=>{b.close();Ok(Value::Null)},"backend.apply"=>{b.apply(a["settings"].clone());Ok(Value::Null)},
 "backend.snapshot"=>Ok(b.snapshot(a["last_cover"].as_u64().unwrap_or(0))),
 "backend.enqueue"=>Ok(json!(b.enqueue(a["action"].as_str().ok_or("action")?,a["seconds"].as_f64().ok_or("seconds")?)?)),
 "backend.command"=>Ok(json!(b.command(a["action"].as_str().ok_or("action")?,a["seconds"].as_f64().ok_or("seconds")?)?)),
 _=>Err("unknown music method".into())}
}
dotisle_module_api::export_service!("music",dispatch);
