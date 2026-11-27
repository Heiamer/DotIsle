//! Production DLL ABI -> asynchronous resolver -> Go process -> display timeline.
use std::{path::PathBuf,time::{Duration,Instant}};
use lyric_core::models::{Selection,Track};
use serde_json::json;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let out=PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);std::fs::create_dir_all(&out)?;
 let client=dotisle_module_api::Client::open("lyrics")?;let mut report=Vec::new();
 for(title,artist,seconds,app,id)in[("晴天","周杰伦",269,"QQMusic",None),("起风了","买辣椒也用券",325,"Generic",None),("Blinding Lights","The Weeknd",200,"Spotify",None),("Hello","Adele",295,"Generic",None),("海阔天空","BEYOND",326,"Netease",Some("347230"))]{
  let mut track=Track::new("online-module-fixture",title,artist,"",app,Duration::from_secs(seconds));track.song_id=id.map(str::to_owned);let start=Instant::now();let job=client.call("search.start",json!({"track":track,"settings":{"LocalFolders":[]},"data":out.join(title).join("Data"),"logs":out.join(title).join("Logs")}))?;let until=Instant::now()+Duration::from_secs(180);
  let result=loop{let v=client.call("search.poll",json!({"id":job}))?;if v["ready"]==true{break v}if Instant::now()>until{let _=client.call("search.cancel",json!({"id":job}));break json!({"status":"timeout"})}std::thread::sleep(Duration::from_millis(40));};
  let parsed=serde_json::from_value::<Selection>(result["lyrics"].clone());let mut row=json!({"title":title,"artist":artist,"player":app,"songId":id,"status":result["status"],"elapsedMs":start.elapsed().as_millis()});
  if let Ok(s)=parsed{let timeline=client.call("timeline.new",json!({}))?;let handle=timeline["handle"].clone();client.call("timeline.begin",json!({"handle":handle,"track":track}))?;client.call("timeline.complete",json!({"handle":handle,"track":track,"selection":s}))?;let at=s.content.lines.iter().find(|l|!l.words.is_empty()).or(s.content.lines.first()).map(|l|l.at+100).unwrap_or(0);let frame=client.call("timeline.frame",json!({"handle":handle,"track":track,"positionMs":at}))?;client.call("timeline.drop",json!({"handle":handle}))?;row["provider"]=json!(s.provider_id);row["lineCount"]=json!(s.content.lines.len());row["wordTimedLines"]=json!(s.content.lines.iter().filter(|c|!c.words.is_empty()).count());row["frame"]=frame;}
  println!("{}",serde_json::to_string(&row)?);report.push(row);std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report)?)?;
 }
 if report.iter().any(|v|v["status"]!="found"){return Err("one or more online module fixtures failed; inspect report.json".into())}Ok(())
}
