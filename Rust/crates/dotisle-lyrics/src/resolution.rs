//! Source-independent lookup: identity gates are evaluated before any lyric download.
use lyric_core::{models::*,identity};
pub trait Source:Send+Sync{fn search(&self,track:&Track)->Result<Vec<Hit>,String>;fn fetch(&self,hit:&Hit)->Result<Option<Payload>,String>;}
pub fn resolve(track:&Track,source:&dyn Source)->Lookup{
 let hits=match source.search(track){Ok(h)=>h,Err(_)=>return Lookup{lyrics:None,status:LookupStatus::TransientFailure}};
 let mut rated=hits.into_iter().filter_map(|h|identity::rate(track,&h).map(|score|(score,h))).collect::<Vec<_>>();rated.sort_by(|a,b|b.0.cmp(&a.0).then_with(||a.1.id.cmp(&b.1.id)));
 let mut failed=false;
 for(score,hit)in rated.into_iter().take(4){match source.fetch(&hit){Ok(Some(p))=>{if let Ok(content)=crate::formats::parse(&p){if content.usable(){return Lookup::from_lyrics(Some(Selection{content,provider_id:hit.provider,candidate_id:hit.id,score}))}}},Err(_)=>failed=true,Ok(None)=>{}}}
 Lookup{lyrics:None,status:if failed{LookupStatus::TransientFailure}else{LookupStatus::Missing}}
}
#[cfg(test)]mod tests{use super::*;use std::{sync::atomic::{AtomicUsize,Ordering},time::Duration};
 struct Mock{count:AtomicUsize,fail:bool}impl Source for Mock{fn search(&self,t:&Track)->Result<Vec<Hit>,String>{Ok(vec![Hit{provider:Provider::new("Local"),id:"a".into(),title:t.title.clone(),artists:vec![t.artist.clone()],album:String::new(),duration:t.duration,extra:Default::default()},Hit{provider:Provider::new("Local"),id:"b".into(),title:format!("{} (Live)",t.title),artists:vec![t.artist.clone()],album:String::new(),duration:t.duration,extra:Default::default()}])}fn fetch(&self,_:&Hit)->Result<Option<Payload>,String>{self.count.fetch_add(1,Ordering::SeqCst);if self.fail{return Err("timeout".into())}Ok(Some(Payload{format:Format::Lrc,original:"[00:02.00]独立测试".into(),translation:String::new(),instrumental:false}))}}
 #[test]fn downloads_only_admitted_candidates_and_retries_fetch_failures(){let t=Track::new("x","歌声","甲","","LRCLIB",Duration::from_secs(93));let m=Mock{count:AtomicUsize::new(0),fail:false};assert_eq!(resolve(&t,&m).status,LookupStatus::Found);assert_eq!(m.count.load(Ordering::SeqCst),1);let m=Mock{count:AtomicUsize::new(0),fail:true};assert_eq!(resolve(&t,&m).status,LookupStatus::TransientFailure);assert_eq!(m.count.load(Ordering::SeqCst),1);}
}
