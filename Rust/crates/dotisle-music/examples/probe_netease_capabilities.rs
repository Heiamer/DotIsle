//! Read-only diagnosis: no playback commands, focus changes or player writes.
use std::{path::PathBuf,time::Duration};
use serde_json::json;
fn main()->Result<(),Box<dyn std::error::Error>>{
 let out=PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);std::fs::create_dir_all(&out)?;
 let targets=dotisle_music::netease_control::players();let targets=targets.iter().map(|t|json!({"pid":t.pid,"backgroundAvailable":t.background_available()})).collect::<Vec<_>>();
 let b=dotisle_music::runtime::Backend::new(json!({"AllowedSources":["Netease"],"IgnoredSources":[],"OnlineSongInfo":false,"LyricSourceOrder":[]}),out.join("Data"),PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()),out.join("Logs"));
 let mut samples=vec![];
 for _ in 0..24{std::thread::sleep(Duration::from_millis(250));let s=b.snapshot(u64::MAX);samples.push(json!({"track":s["track"],"canSeek":s["canSeek"],"rawSeconds":s["rawSeconds"],"isPlaying":s["isPlaying"],"nativeStatus":s["nativeStatus"],"nativeError":s["nativeError"]}));}
 b.close();let timeline=dotisle_music::netease::timeline_diagnostics();let report=json!({"targets":targets,"samples":samples,"timeline":timeline,"readonly":true});std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report)?)?;println!("{}",json!({"targets":targets,"last":samples.last(),"timeline":timeline}));Ok(())
}
