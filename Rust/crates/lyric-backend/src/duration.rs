//! Native bounded duration enrichment, preserving the managed v1 cache format.
use std::{path::PathBuf,collections::VecDeque,sync::mpsc,time::{Duration,Instant}};
use serde_json::{Value,json};
use lyric_core::{models::*,identity,resolution::Source,sync::Timeline};
use crate::{io,storage::{duration_from_text,to_managed},providers::OnlineSource,http::RequestContext};
#[derive(Clone)]struct Entry{duration:Duration,provider:String,id:String}
pub struct DurationResolver{path:PathBuf,cache:VecDeque<(String,Entry)>,identity:String,pending:Option<mpsc::Receiver<Option<Entry>>>,retry:Instant,pub source:String}
impl DurationResolver{
    pub fn new(path:PathBuf)->Self{
        let mut cache=VecDeque::new();let value=io::read_settings(&path);
        if let Some(entries)=value.as_object(){for(key,value)in entries{if let Some(duration)=value["Duration"].as_str().and_then(duration_from_text).filter(|d|valid(*d)){cache.push_back((key.clone(),Entry{duration,provider:value["Provider"].as_str().unwrap_or("").into(),id:value["CandidateId"].as_str().unwrap_or("").into()}));while cache.len()>256{cache.pop_front();}}}}
        Self{path,cache,identity:String::new(),pending:None,retry:Instant::now(),source:"等待歌曲信息".into()}
    }
    pub fn enrich(&mut self,playback:&mut Playback){
        let key=playback.track.as_ref().map(Timeline::build_stable_track_identity).unwrap_or_default();
        if key!=self.identity{self.identity=key.clone();self.pending=None;self.retry=Instant::now()}
        let Some(track)=playback.track.as_mut()else{self.source="等待歌曲信息".into();return};
        if valid(track.duration){self.source="播放器".into();return}
        if let Some(receiver)=&self.pending{match receiver.try_recv(){Ok(result)=>{self.pending=None;self.retry=Instant::now()+Duration::from_secs(30);if let Some(entry)=result{self.cache.retain(|(id,_)|*id!=key);self.cache.push_back((key.clone(),entry));while self.cache.len()>256{self.cache.pop_front();}self.save();}},Err(mpsc::TryRecvError::Disconnected)=>{self.pending=None;self.retry=Instant::now()+Duration::from_secs(30)},Err(_)=>{}}}
        if let Some((_,entry))=self.cache.iter().find(|(id,_)|*id==key){track.duration=entry.duration;self.source=format!("{}歌曲信息",entry.provider);return}
        self.source="正在补取歌曲时长".into();
        if self.pending.is_none()&&Instant::now()>=self.retry{let(tx,rx)=mpsc::channel();self.pending=Some(rx);let track=track.clone();std::thread::spawn(move||{let _=tx.send(lookup(&track));});}
    }
    fn save(&self){let map=self.cache.iter().map(|(key,e)|(key.clone(),json!({"Duration":to_managed(json!({"duration":{"secs":e.duration.as_secs(),"nanos":e.duration.subsec_nanos()}}),true)["Duration"],"Provider":e.provider,"CandidateId":e.id}))).collect::<serde_json::Map<_,_>>();let _=io::save_settings(&self.path,&Value::Object(map));}
}
fn valid(duration:Duration)->bool{duration>Duration::ZERO&&duration<=Duration::from_secs(86400)}
fn lookup(track:&Track)->Option<Entry>{
 if track.title.trim().is_empty(){return None}let mut query=track.clone();query.song_id=None;query.duration=Duration::ZERO;
 let mut order=providers();if let Some(primary)=identity::official(&track.source_app){if let Some(i)=order.iter().position(|p|*p==primary){order.remove(i);order.insert(0,primary);}}
 for provider in order{let source=OnlineSource::new(provider.value(),RequestContext::new(Duration::from_secs(5)));if let Ok(hits)=source.search(&query){let best=hits.into_iter().filter(|h|valid(h.duration)).filter_map(|h|identity::rate(&query,&h).filter(|score|*score>=95).map(|score|(score,h))).max_by_key(|(score,_)|*score);if let Some((_,hit))=best{return Some(Entry{duration:hit.duration,provider:provider.value().into(),id:hit.id})}}}None
}
