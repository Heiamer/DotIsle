use serde_json::{json,Value};mod engine;
fn strarg<'a>(a:&'a Value,k:&str)->Result<&'a str,String>{a[k].as_str().ok_or_else(||format!("missing {k}"))}
fn dispatch(v:Value)->Result<Value,String>{let a=&v["args"];match v["method"].as_str().ok_or("method")?{
"chime"=>{let result:Result<_,String>=Ok(engine::chime(strarg(a,"kind")?,a["volume"].as_f64().ok_or("volume")?));Ok(json!(result?))},
"custom"=>{let result:Result<_,String>=engine::custom(&serde_json::from_value::<Vec<u8>>(a["bytes"].clone()).map_err(|e|e.to_string())?,a["volume"].as_f64().ok_or("volume")?);Ok(json!(result?))},
"validate_file"=>{let result:Result<_,String>=engine::validate_file(strarg(a,"path")?);Ok(json!(result?))},
"play"=>{let result:Result<_,String>=engine::play(&a["settings"],a["todo"]==true,a["preview"]==true);Ok(json!(result?))},
"play_device"=>{let result:Result<_,String>=engine::play_device(&a["settings"],a["preview"]==true);Ok(json!(result?))},
"select"=>{let result:Result<_,String>=engine::select(&a["settings"],strarg(a,"prefix")?,strarg(a,"source")?,strarg(a,"kind")?);Ok(json!(result?))},
"reset_builtin"=>{let result:Result<_,String>=Ok(engine::reset_builtin(&a["settings"]));Ok(json!(result?))},
"preview"=>{let result:Result<_,String>=engine::preview(&a["settings"],strarg(a,"prefix")?,a["kind"].as_str());Ok(json!(result?))},
 _=>Err("unknown sound method".into())}}
dotisle_module_api::export_service!("sound",dispatch);
