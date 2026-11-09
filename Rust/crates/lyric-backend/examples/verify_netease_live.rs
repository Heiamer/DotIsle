//! Authorized live-player acceptance with production commands and guarded restoration.
use std::{path::PathBuf,sync::Arc,time::{Duration,Instant}};
use serde_json::{json,Value};
use lyric_backend::runtime::Backend;

fn wait(b:&Backend,check:impl Fn(&Value)->bool,seconds:u64)->Value{
 let until=Instant::now()+Duration::from_secs(seconds);
 loop{let s=b.snapshot(u64::MAX);if check(&s){return s}if Instant::now()>=until{return json!({"timeout":true,"last":s})}std::thread::sleep(Duration::from_millis(30));}
}
fn compact(s:&Value)->Value{json!({"track":s["track"],"playing":s["isPlaying"],"position":s["positionSeconds"],"raw":s["rawSeconds"],"canSeek":s["canSeek"],"frame":s["frame"],"lyricSource":s["lyricSource"],"nativeStatus":s["nativeStatus"],"error":s["nativeError"]})}
fn window_state(v:&Value)->Value{json!(v.as_array().map(|rows|rows.iter().map(|r|json!({"pid":r["pid"],"visible":r["visible"],"minimized":r["minimized"],"foreground":r["foreground"]})).collect::<Vec<_>>()))}
fn windows_preserved(v:&Value)->bool{
 if v["windowsBefore"].is_array()&&window_state(&v["windowsBefore"])!=window_state(&v["windowsAfter"]){return false}
 match v{Value::Object(values)=>values.values().all(windows_preserved),Value::Array(values)=>values.iter().all(windows_preserved),_=>true}
}
fn seek(b:&Arc<Backend>,seconds:f64)->Value{
 let windows_before=lyric_backend::netease_control::window_diagnostics();let start=Instant::now();match b.enqueue("seek",seconds){Ok(id)=>{let s=wait(b,|s|s["mediaCommand"]["id"]==id&&matches!(s["mediaCommand"]["state"].as_str(),Some("confirmed"|"failed")),7);json!({"elapsedMs":start.elapsed().as_millis(),"command":s["mediaCommand"],"state":compact(&s),"windowsBefore":windows_before,"windowsAfter":lyric_backend::netease_control::window_diagnostics()})},Err(e)=>json!({"error":e})}
}
fn playback(b:&Arc<Backend>,playing:bool)->Value{
 let windows_before=lyric_backend::netease_control::window_diagnostics();
 let sent=b.command(if playing{"play"}else{"pause"},0.);
 let s=wait(b,|s|s["isPlaying"]==playing,4);
 let confirmed=sent.unwrap_or(false);json!({"sent":confirmed,"state":compact(&s),"matched":confirmed&&s["isPlaying"]==playing,"windowsBefore":windows_before,"windowsAfter":lyric_backend::netease_control::window_diagnostics()})
}
struct Restore{b:Arc<Backend>,song:Value,position:f64,playing:bool,armed:bool}
impl Restore{
 fn run(&mut self)->Value{
  if !self.armed{return json!({"alreadyRestored":true})}self.armed=false;
  if self.b.snapshot(u64::MAX)["track"]["songId"]!=self.song{return json!({"skipped":"track_changed"})}
  let paused=playback(&self.b,false);let position=seek(&self.b,self.position);
  let playing=playback(&self.b,self.playing);
  json!({"pause":paused,"seek":position,"playback":playing,"success":position["command"]["state"]=="confirmed"&&playing["matched"]==true})
 }
}
impl Drop for Restore{fn drop(&mut self){if self.armed{eprintln!("Emergency restore: {}",self.run());}self.b.close();}}
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args:Vec<String>=std::env::args().collect();if args.get(1).map(String::as_str)!=Some("--authorized-and-restore"){return Err("authorization flag required".into())}
 let out=PathBuf::from(args.get(2).ok_or("output required")?);std::fs::create_dir_all(&out)?;
 let b=Backend::new(json!({"AllowedSources":["Netease"],"IgnoredSources":[],"OnlineSongInfo":false,"LyricSourceOrder":["Netease","QQ"],"LocalLyric":false}),out.join("Data"),PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap()),out.join("Logs"));
 let before=wait(&b,|s|s["canSeek"]==true&&s["track"]["songId"].as_str().is_some(),12);
 let windows_before=lyric_backend::netease_control::window_diagnostics();
 if before["canSeek"]!=true{b.close();std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&json!({"success":false,"precheck":before}))?)?;return Err("live channel unavailable".into())}
 let duration=before["track"]["durationSeconds"].as_f64().unwrap();
 let original=args.get(3).and_then(|s|s.parse::<f64>().ok()).unwrap_or_else(||before["positionSeconds"].as_f64().unwrap());
 let mut restore=Restore{b:b.clone(),song:before["track"]["songId"].clone(),position:original,playing:before["isPlaying"].as_bool().unwrap_or(false),armed:true};
 let anchor=original.clamp(15.,duration-30.);let paused=playback(&b,false);
 let mut boundaries=Vec::new();for target in [0.,0.8,duration-0.8,duration-0.05]{let r=seek(&b,target);let ok=r["command"]["state"]=="confirmed";boundaries.push(r);if !ok{break}}
 let mut rows=Vec::new();
 for delta in [0.,3.,-3.,6.,-5.,8.,-7.,4.,-4.,5.,-2.]{if b.snapshot(u64::MAX)["track"]["songId"]!=restore.song{break}let r=seek(&b,anchor+delta);let ok=r["command"]["state"]=="confirmed";rows.push(r);if !ok{break}}
 let mut burst_id=0;for delta in [9.,3.,-2.,7.,1.]{if let Ok(id)=b.enqueue("seek",anchor+delta){burst_id=id}std::thread::sleep(Duration::from_millis(15));}
 let burst=wait(&b,|s|s["mediaCommand"]["id"]==burst_id&&matches!(s["mediaCommand"]["state"].as_str(),Some("confirmed"|"failed")),7);
 let stable_before=b.snapshot(u64::MAX);let mut pause_samples=vec![compact(&stable_before)];for _ in 0..20{std::thread::sleep(Duration::from_millis(100));pause_samples.push(compact(&b.snapshot(u64::MAX)));}let stable_after=b.snapshot(u64::MAX);
 let paused_drift=(stable_after["positionSeconds"].as_f64().unwrap_or(0.)-stable_before["positionSeconds"].as_f64().unwrap_or(999.)).abs();
 let playing=playback(&b,true);let start=b.snapshot(u64::MAX);let mut frames=Vec::new();
 for _ in 0..30{std::thread::sleep(Duration::from_millis(200));frames.push(compact(&b.snapshot(u64::MAX)));}
 let end=b.snapshot(u64::MAX);let advanced=end["positionSeconds"].as_f64().unwrap_or(0.)-start["positionSeconds"].as_f64().unwrap_or(999.);
 let playing_seek=seek(&b,anchor-10.);let stopped=playback(&b,false);
 let restored=restore.run();let after=b.snapshot(u64::MAX);
 let lyrics=frames.iter().any(|s|(s["frame"]["currentLineIndex"].as_i64().is_some_and(|i|i>=0)||s["frame"]["isPureMusic"]==true)&&s["lyricSource"].as_str().is_some());
 let success=paused["matched"]==true&&boundaries.len()==4&&boundaries.iter().all(|r|r["command"]["state"]=="confirmed"&&r["state"]["playing"]==false)&&rows.len()==11&&rows.iter().all(|r|r["command"]["state"]=="confirmed"&&r["state"]["playing"]==false)&&burst["mediaCommand"]["state"]=="confirmed"&&paused_drift<0.1&&playing["matched"]==true&&advanced>4.&&advanced<9.&&playing_seek["command"]["state"]=="confirmed"&&playing_seek["state"]["playing"]==true&&stopped["matched"]==true&&restored["success"]==true&&lyrics;
 let report=json!({"success":success,"scope":"current real NetEase client; production backend, command queue, online lyrics and automatic native readback","before":compact(&before),"restoreTargetSeconds":original,"initialPause":paused,"pausedSeeks":rows,"burst":{"command":burst["mediaCommand"],"state":compact(&burst)},"pauseSamples":pause_samples,"pauseDriftSeconds":paused_drift,"play":playing,"advancedSeconds":advanced,"playingFrames":frames,"realLyricReceived":lyrics,"playingSeek":playing_seek,"pause":stopped,"restore":restored,"after":compact(&after),"attention":lyric_backend::netease_control::attention_diagnostics()});
 std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report)?)?;
 let mut report=report;report["boundaries"]=json!(boundaries);report["windowsBefore"]=windows_before;report["windowsAfter"]=lyric_backend::netease_control::window_diagnostics();report["timeline"]=lyric_backend::netease::timeline_diagnostics();std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report)?)?;
 let window_preserved=windows_preserved(&report);let success=success&&window_preserved;report["windowStatePreserved"]=json!(window_preserved);report["success"]=json!(success);std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&report)?)?;
 println!("{}",json!({"success":success,"pausedSeeks":report["pausedSeeks"].as_array().map(Vec::len),"pauseDriftSeconds":paused_drift,"advancedSeconds":advanced,"realLyricReceived":lyrics,"restore":report["restore"]["success"]}));
 if !success{return Err("live acceptance failed; inspect report".into())}Ok(())
}
