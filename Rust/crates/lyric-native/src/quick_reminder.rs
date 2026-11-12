use crate::todos::Now;use serde_json::json;
#[derive(Clone,Debug,PartialEq,Eq,serde::Serialize)]
pub struct Draft{pub title:String,pub date:String,pub time:String}
#[derive(Clone,Debug,PartialEq,Eq)]
pub struct ParseError{pub code:&'static str,pub message:String}
pub fn parse(input:&str,now:&Now)->Result<Draft,ParseError>{
let reply=dotisle_module_api::call("todos","parse",json!({"text":input,"now":now})).map_err(|e|ParseError{code:"module_unavailable",message:e})?;
if !reply["draft"].is_null(){let d=&reply["draft"];return Ok(Draft{title:d["title"].as_str().unwrap_or("").into(),date:d["date"].as_str().unwrap_or("").into(),time:d["time"].as_str().unwrap_or("").into()});}
let code=match reply["error"].as_str().unwrap_or(""){"empty_title"=>"empty_title","invalid_characters"=>"invalid_characters","invalid_date"=>"invalid_date","invalid_time"=>"invalid_time","out_of_range"=>"out_of_range","past_time"=>"past_time","too_long"=>"too_long","unsupported_grammar"=>"unsupported_grammar",_=>"invalid_input"};Err(ParseError{code,message:reply["message"].as_str().unwrap_or("无法解析提醒").into()})}
