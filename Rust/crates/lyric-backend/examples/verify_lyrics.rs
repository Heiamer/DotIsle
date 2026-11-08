//! Read-only service interoperability checks; never issues player controls.
use std::time::{Duration,Instant};
use lyric_core::{models::*,resolution::{self,Source}};
use lyric_backend::{providers::OnlineSource,http::RequestContext};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let out=std::env::args().nth(1).ok_or("report path required")?;let mut reports=Vec::new();
 for provider in providers(){let start=Instant::now();let source=OnlineSource::new(provider.value(),RequestContext::new(Duration::from_secs(15)));let track=Track::new("service-check","海阔天空","BEYOND","乐与怒","Generic",Duration::ZERO);let search=source.search(&track);let details=match &search{Ok(hits)=>serde_json::json!({"hits":hits.len(),"candidates":hits.iter().take(6).map(|h|serde_json::json!({"title":h.title,"artists":h.artists,"id":h.id,"score":lyric_core::identity::rate(&track,h)})).collect::<Vec<_>>()}),Err(e)=>serde_json::json!({"error":e})};let result=resolution::resolve(&track,&source);reports.push(serde_json::json!({"provider":provider.value(),"search":details,"status":format!("{:?}",result.status),"lyrics":result.lyrics.is_some(),"score":result.lyrics.as_ref().map(|s|s.score),"formatWords":result.lyrics.as_ref().map(|s|s.content.lines.iter().filter(|l|!l.words.is_empty()).count()),"elapsedMs":start.elapsed().as_millis()}));std::fs::write(&out,serde_json::to_vec_pretty(&reports)?)?;}Ok(())
}
