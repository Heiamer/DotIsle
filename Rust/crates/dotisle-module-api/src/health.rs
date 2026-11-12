//! Deterministic, side-effect-free compatibility probes, executed before replacing a DLL.
use serde_json::{json,Value};
pub fn run(id:&str,dispatch:impl Fn(Value)->Result<Value,String>)->Result<Value,String>{
 let mut count=0;let mut invoke=|method:&str,args:Value|{count+=1;dispatch(json!({"method":method,"args":args}))};
 let (service,new_args,commands):(&str,Value,Vec<(&str,Value)>)=match id {
 "core"|"settings"=>return Ok(json!({"success":true,"commands":0})),
 "focus"=>("store",json!({"path":""}),vec![("create",json!({"v":{"mode":"countdown","duration":60}})),("advance",json!({"ms":1000})),("list",json!({}))]),
 "todos"=>("store",json!({"path":""}),vec![("list",json!({"now":{"date":"2000-01-01","time":"12:00"}}))]),
 "history"=>("store",json!({"path":""}),vec![("list",json!({}))]),
 "automation"=>("store",json!({"rules_path":"","usage_path":"","today":"2000-01-01"}),vec![("list",json!({"today":"2000-01-01"}))]),
 "devices"=>("hub",json!({}),vec![("poll",json!({"gates":{"master":false,"disconnect":false,"headphones":false,"speaker":false,"gamepad":false,"mouse":false,"keyboard":false}}))]),
 "osd"=>("hub",json!({}),vec![("status",json!({}))]),
 "widgets"=>("placements",json!({"path":""}),vec![("list",json!({}))]),
 "lyrics"=>("timeline",json!({}),vec![("frame",json!({"track":null,"positionMs":0}))]),
 "glow"=>("state",json!({}),vec![("tick",json!({"dt":0.016,"target":"Off","animate":false})),("frame",json!({"time_ms":0.,"animate":false}))]),
 "music"=>{invoke("attention",json!({}))?;return Ok(json!({"success":true,"commands":count}));},
 "notifications"=>{let p=invoke("policy",json!({"NotificationsEnabled":false}))?;let enabled=invoke("should_show",p)?;if enabled!=false{return Err("通知停用策略不兼容".into());}return Ok(json!({"success":true,"commands":count}));},
 "sound"=>{invoke("select",json!({"settings":{},"prefix":"Notification","source":"Builtin","kind":"Soft"}))?;return Ok(json!({"success":true,"commands":count}));},
 "themes"=>{let contrast=invoke("contrast",json!({"a":0,"b":16777215}))?;if contrast.as_f64().is_none_or(|n|(n-21.).abs()>0.01){return Err("主题对比度接口不兼容".into());}return Ok(json!({"success":true,"commands":count}));},
 _=>return Err("未知模块".into())};
 let opened=invoke(&format!("{service}.new"),new_args)?;let handle=opened["handle"].as_u64().ok_or("模块没有创建有效测试会话")?;
 let result=(||{for(method,mut args)in commands{args["handle"]=json!(handle);let value=invoke(&format!("{service}.{method}"),args)?;if value["operationError"].is_string(){return Err(value["operationError"].as_str().unwrap().to_owned());}}Ok(())})();
 let closed=invoke(&format!("{service}.drop"),json!({"handle":handle}));result?;closed?;
 Ok(json!({"success":true,"commands":count}))
}
