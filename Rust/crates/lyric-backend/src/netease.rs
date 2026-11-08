//! Read-only local player-page timeline. No process-memory patterns or offsets.
use std::{path::PathBuf,sync::mpsc,time::{Duration,Instant}};
use serde_json::{Value,json};
use lyric_core::{models::Track,identity};
use crate::{media::unix_ms,netease_control::{Target,players},netease_cdp,http::{self,RequestContext},providers::text};
#[derive(Clone,Debug)]pub struct TimingSample{pub song_id:String,pub title:String,pub artist:String,pub album:String,pub position:f64,pub duration:f64,pub playing:bool,pub control_ready:bool,pub captured_ms:i64,pub cover_url:Option<String>}
fn parse(v:&Value,now:i64)->Result<TimingSample,String>{
 let song_id=text(v,"songId");let position=v["position"].as_f64().ok_or("position missing")?;let duration=v["duration"].as_f64().ok_or("duration missing")?;let captured_ms=v["capturedMs"].as_i64().ok_or("timestamp missing")?;
 if song_id.is_empty()||song_id.len()>32||!song_id.bytes().all(|b|b.is_ascii_digit())||!position.is_finite()||!duration.is_finite()||!(0.0..=86400.).contains(&duration)||duration==0.||position<0.||position>duration||!(0..800).contains(&(now-captured_ms)){return Err("invalid or stale player timeline".into())}
 let playing=v["playing"].as_bool().ok_or("playback state missing")?;let cover=text(v,"cover");
 Ok(TimingSample{song_id,title:text(v,"title"),artist:text(v,"artist"),album:text(v,"album"),position,duration,playing,control_ready:v["controlReady"].as_bool().unwrap_or(false),captured_ms,cover_url:(!cover.is_empty()).then_some(cover)})
}
type ReadResult=Result<(Target,TimingSample),String>;
fn sample(port:u16)->ReadResult{
 for target in players(){if !netease_cdp::available(target.pid,port){continue}let value=netease_cdp::read_timeline(target.pid,port)?;return parse(&value,unix_ms()).map(|sample|(target,sample))}
 Err(format!("本机进度通道未连接（端口 {port}），需退出播放器后运行包内连接脚本；当前使用系统媒体会话"))
}
pub struct NeteaseTimeline{pub latest:Option<TimingSample>,pub status:String,pub error:Option<String>,pub port:u16,target:Option<Target>,pending:Option<(u16,mpsc::Receiver<ReadResult>)>,retry:Instant}
impl NeteaseTimeline{
 pub fn new(_local:PathBuf)->Self{Self{latest:None,status:"等待网易云".into(),error:None,port:netease_cdp::DEFAULT_PORT,target:None,pending:None,retry:Instant::now()}}
 pub fn read(&mut self,_online:bool){
  if self.latest.as_ref().is_some_and(|s|!(0..2000).contains(&(unix_ms()-s.captured_ms))){self.latest=None;self.target=None;}
  if let Some((port,rx))=&self.pending{match rx.try_recv(){Ok(result)=>{let same=*port==self.port;self.pending=None;if same{match result{Ok((target,sample))=>{self.target=Some(target);self.latest=Some(sample);self.status="网易云本机页面进度".into();self.error=None;self.retry=Instant::now();},Err(e)=>{self.target=None;self.status="使用系统媒体会话".into();self.error=Some(e);self.retry=Instant::now()+Duration::from_millis(750);}}}},Err(mpsc::TryRecvError::Disconnected)=>{self.pending=None;self.target=None;self.retry=Instant::now()+Duration::from_millis(750)},Err(mpsc::TryRecvError::Empty)=>{}}}
  if self.pending.is_none()&&Instant::now()>=self.retry{let port=self.port;let(tx,rx)=mpsc::channel();self.pending=Some((port,rx));std::thread::spawn(move||{let _=tx.send(sample(port));});}
 }
 pub fn matching(&self,track:&Track)->Option<TimingSample>{let sample=self.display_matching(track)?;if !(0..800).contains(&(unix_ms()-sample.captured_ms)){return None}Some(sample)}
 pub fn display_matching(&self,track:&Track)->Option<TimingSample>{let sample=self.latest.as_ref()?;if identity::official(&track.source_app)!=Some(lyric_core::models::Provider::new("Netease"))||!(0..2000).contains(&(unix_ms()-sample.captured_ms)){return None}if let Some(id)=&track.song_id{if id!=&sample.song_id{return None}}else if identity::normalized(&track.title)!=identity::normalized(&sample.title)||(!track.artist.is_empty()&&identity::normalized(&track.artist)!=identity::normalized(&sample.artist)){return None}Some(sample.clone())}
 pub fn control_target(&self,track:&Track)->Option<Target>{if !self.matching(track)?.control_ready{return None}self.target.clone()}
}
pub fn timeline_diagnostics()->Value{let port=netease_cdp::DEFAULT_PORT;match sample(port){Ok((target,s))=>json!({"route":"player-page","pid":target.pid,"songId":s.song_id,"position":s.position,"duration":s.duration,"playing":s.playing,"capturedMs":s.captured_ms}),Err(e)=>json!({"route":"player-page","error":e})}}
pub fn track_detail(id:&str)->Result<Value,String>{if id.is_empty()||id.len()>32||!id.bytes().all(|b|b.is_ascii_digit()){return Err("歌曲 ID 无效".into())}let response=http::get(&format!("https://music.163.com/api/song/detail/?ids=%5B{id}%5D"),&RequestContext::new(Duration::from_secs(4)))?;response["songs"].as_array().and_then(|a|a.iter().find(|s|text(s,"id")==id)).cloned().ok_or("歌曲详情 ID 不匹配".into())}
#[cfg(test)]mod tests{use super::*;
 #[test]fn short_page_gaps_hold_display_but_never_control(){
  let now=unix_ms();let v=json!({"songId":"51","title":"远行","artist":"甲","position":12.5,"duration":150.,"playing":false,"capturedMs":now-1000,"controlReady":true});
  let mut reader=NeteaseTimeline::new(PathBuf::new());reader.latest=Some(parse(&v,now-1000).unwrap());reader.target=Some(Target{pid:42,executable:PathBuf::from("cloudmusic.exe")});
  let track=Track::new("Netease|51","远行","甲","","Netease",Duration::from_secs(150));assert_eq!(reader.display_matching(&track).unwrap().position,12.5);assert!(reader.matching(&track).is_none());assert!(reader.control_target(&track).is_none());
  let other=Track::new("Netease|52","另一首","甲","","Netease",Duration::from_secs(150));assert!(reader.display_matching(&other).is_none());reader.latest.as_mut().unwrap().captured_ms=now-2100;assert!(reader.display_matching(&track).is_none());
 }
 #[test]fn restored_readonly_samples_do_not_advertise_control(){
  let now=unix_ms();let mut v=json!({"songId":"51","title":"远行","artist":"甲","position":12.5,"duration":150.,"playing":false,"capturedMs":now,"controlReady":false});
  let track=Track::new("Netease|51","远行","甲","","Netease",Duration::from_secs(150));
  let mut reader=NeteaseTimeline::new(PathBuf::new());reader.latest=Some(parse(&v,now).unwrap());reader.target=Some(Target{pid:42,executable:PathBuf::from("cloudmusic.exe")});
  assert!(reader.matching(&track).is_some());assert!(reader.control_target(&track).is_none());
  v["controlReady"]=json!(true);reader.latest=Some(parse(&v,now).unwrap());assert!(reader.control_target(&track).is_some());
 }
 #[test]fn reader_rejects_delays_future_and_impossible_samples(){let mut v=json!({"songId":"51","title":"远行","artist":"甲","position":12.5,"duration":150.,"playing":false,"capturedMs":2000});assert!(parse(&v,2200).is_ok());assert!(parse(&v,3000).is_err());assert!(parse(&v,1900).is_err());v["position"]=json!(151.);assert!(parse(&v,2200).is_err());v["position"]=json!(10.);v["playing"]=Value::Null;assert!(parse(&v,2200).is_err());}
}
