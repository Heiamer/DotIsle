use std::{path::PathBuf,sync::{Arc,atomic::{AtomicBool,Ordering}}};
use serde_json::Value;
use lyric_core::models::*;
use crate::{local::LocalLibrary,storage::{MappingDb,FinalCache},io};
pub struct Resolver{pub settings:Value,pub data:PathBuf,pub logs:PathBuf,pub final_cache:FinalCache,pub closed:AtomicBool}
impl Resolver{pub fn resolve(&self,track:&Track,cancel:Arc<AtomicBool>)->Lookup{
 let mapping=MappingDb(self.data.join("database/song_maps.db")).resolve(track);
 if cancel.load(Ordering::Acquire)||self.closed.load(Ordering::Acquire){return Lookup::unavailable();}
 if mapping.is_pure_music{return Lookup::from_lyrics(Some(Selection{content:Lyrics::instrumental(),provider_id:Provider::new("Local"),candidate_id:format!("mapping:{}",track.id),score:100}));}
 let mut mapped=track.clone();mapped.title=mapping.title;mapped.artist=mapping.artist;
 if let Some(album)=mapping.album{mapped.album=album;}
 let folders=self.settings["LocalFolders"].as_array().map(|a|a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default();
 if let Some(content)=LocalLibrary::new(folders).get(&mapped){return Lookup::from_lyrics(Some(Selection{content,provider_id:Provider::new("Local"),candidate_id:track.id.clone(),score:100}));}
 // Versioned identity prevents old heuristic results from surviving the engine change.
 let context=serde_json::to_string(&("taskbar-3fae928-v1",&mapped.title,&mapped.artist,&mapped.album,&mapped.source_app,&mapped.song_id,mapped.duration.as_secs())).unwrap_or_default();
 if let Some(cached)=self.final_cache.get(&context){return Lookup::from_lyrics(Some(cached));}
 match crate::taskbar::resolve(&mapped,cancel.clone()){
  Ok(result)=>{
   if cancel.load(Ordering::Acquire)||self.closed.load(Ordering::Acquire){return Lookup::unavailable();}
   if let Some(selection)=&result.lyrics{self.final_cache.put(&context,selection);}
   io::log(&self.logs,"LYRIC_UPSTREAM",&format!("title={:?} artist={:?} status={:?} source={:?}",track.title,track.artist,result.status,result.lyrics.as_ref().map(|s|s.provider_id.value())));
   result
  },
  Err(error)=>{io::log(&self.logs,"LYRIC_ERROR",&error);Lookup{lyrics:None,status:LookupStatus::TransientFailure}}
 }
}}
