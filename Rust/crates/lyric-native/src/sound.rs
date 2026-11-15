use serde_json::{json,Value};
pub fn chime(kind:&str,volume:f64)->Vec<u8>{dotisle_module_api::call("sound","chime",json!({"kind":kind,"volume":volume})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn custom(bytes:&[u8],volume:f64)->Result<Vec<u8>,String>{serde_json::from_value(dotisle_module_api::call("sound","custom",json!({"bytes":bytes,"volume":volume}))?).map_err(|e|e.to_string())}
pub fn validate_file(path:&str)->Result<(),String>{serde_json::from_value(dotisle_module_api::call("sound","validate_file",json!({"path":path}))?).map_err(|e|e.to_string())}
pub fn play(settings:&Value,todo:bool,preview:bool)->Result<(),String>{serde_json::from_value(dotisle_module_api::call("sound","play",json!({"settings":settings,"todo":todo,"preview":preview}))?).map_err(|e|e.to_string())}
pub fn play_device(settings:&Value,preview:bool)->Result<(),String>{serde_json::from_value(dotisle_module_api::call("sound","play_device",json!({"settings":settings,"preview":preview}))?).map_err(|e|e.to_string())}
pub fn select(settings:&Value,prefix:&str,source:&str,kind:&str)->Result<Value,String>{serde_json::from_value(dotisle_module_api::call("sound","select",json!({"settings":settings,"prefix":prefix,"source":source,"kind":kind}))?).map_err(|e|e.to_string())}
pub fn reset_builtin(settings:&Value)->Value{dotisle_module_api::call("sound","reset_builtin",json!({"settings":settings})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn preview(settings:&Value,prefix:&str,kind:Option<&str>)->Result<(),String>{serde_json::from_value(dotisle_module_api::call("sound","preview",json!({"settings":settings,"prefix":prefix,"kind":kind}))?).map_err(|e|e.to_string())}
