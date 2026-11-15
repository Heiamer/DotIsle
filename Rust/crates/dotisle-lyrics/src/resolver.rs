use std::{path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering},mpsc},time::{Duration,Instant}};
use serde_json::Value;use lyric_core::{models::*,cache::Cache};use crate::{local::LocalLibrary,providers::OnlineSource,http::RequestContext,storage::{MappingDb,FinalCache},io};
pub struct Resolver{pub settings:Value,pub data:PathBuf,pub logs:PathBuf,pub raw:Arc<Cache>,pub final_cache:FinalCache,pub closed:AtomicBool}
impl Resolver{pub fn resolve(&self,track:&Track,cancel:Arc<AtomicBool>)->Lookup{
        let settings=&self.settings;
        let mapping=MappingDb(self.data.join("database/song_maps.db")).resolve(track);
        if mapping.is_pure_music{return Lookup::from_lyrics(Some(Selection{content:Lyrics::instrumental(),provider_id:Provider::new("Local"),candidate_id:format!("mapping:{}",track.id),score:100}))}
        let mut mapped=track.clone();mapped.title=mapping.title;mapped.artist=mapping.artist;if let Some(album)=mapping.album{mapped.album=album}
        let folders=settings["LocalFolders"].as_array().map(|a|a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default();
        if let Some(content)=LocalLibrary::new(folders).get(&mapped){return Lookup::from_lyrics(Some(Selection{content,provider_id:Provider::new("Local"),candidate_id:track.id.clone(),score:100}))}
        let context=serde_json::to_string(&(lyric_core::identity::track_key(&mapped),&mapping.preferred_provider)).unwrap_or_default();
        if let Some(cached)=self.final_cache.get(&context){return Lookup::from_lyrics(Some(cached))}
        let mut order=providers();if let Some(official)=lyric_core::identity::official(&track.source_app){if let Some(i)=order.iter().position(|p|*p==official){let p=order.remove(i);order.insert(0,p);}}
        if let Some(preferred)=mapping.preferred_provider{order.retain(|p|p.eq_ignore_ascii_case(&preferred));}
        let (tx,rx)=mpsc::channel();let stopped=Arc::new(AtomicBool::new(false));
        for provider in &order{let tx=tx.clone();let provider=provider.clone();let mapped=mapped.clone();let cache=self.raw.clone();let cancel=cancel.clone();let stop=stopped.clone();std::thread::spawn(move||{if cancel.load(Ordering::Acquire){return}let mut source=OnlineSource::new(provider.value(),RequestContext{cancelled:stop,deadline:Instant::now()+Duration::from_secs(9)});source.cache=Some(cache);let result=crate::resolution::resolve(&mapped,&source);if !cancel.load(Ordering::Acquire){let _=tx.send((provider,result));}});}
        drop(tx);let mut results=Vec::new();let mut deadline=Instant::now()+Duration::from_secs(10);
        while results.len()<order.len()&&Instant::now()<deadline{
            if cancel.load(Ordering::Acquire)||self.closed.load(Ordering::Acquire){stopped.store(true,Ordering::Release);return Lookup::unavailable()}
            if let Ok((provider,result))=rx.recv_timeout(Duration::from_millis(40)){
                if result.lyrics.as_ref().is_some_and(|l|l.score>=96){if order.first()==Some(&provider){stopped.store(true,Ordering::Release);if let Some(r)=&result.lyrics{self.final_cache.put(&context,r)}return result}deadline=deadline.min(Instant::now()+Duration::from_millis(600));}
                results.push((provider,result));
            }
        }
        stopped.store(true,Ordering::Release);
        let selected=results.iter().filter_map(|(p,r)|r.lyrics.as_ref().map(|l|(p,l))).max_by_key(|(p,l)|(l.score,std::cmp::Reverse(order.iter().position(|id|id==*p).unwrap_or(usize::MAX))));
        if let Some((_,lyrics))=selected{self.final_cache.put(&context,lyrics);return Lookup::from_lyrics(Some(lyrics.clone()))}
        io::log(&self.logs,"LYRIC_MISS",&format!("title={:?} artist={:?} results={:?}",track.title,track.artist,results.iter().map(|(p,r)|(p.value(),r.status)).collect::<Vec<_>>()));
        Lookup{lyrics:None,status:if results.len()<order.len()||results.iter().any(|(_,r)|r.status==LookupStatus::TransientFailure){LookupStatus::TransientFailure}else{LookupStatus::Missing}}
    }}
