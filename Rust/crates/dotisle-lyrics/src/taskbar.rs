//! Sequential automatic chain from H0308/taskbar-lyrics-displayer 3fae928.
//! Apache-2.0. HTTP/search/matching live in the existing native Go worker.
use lyric_core::models::*;
use serde::Deserialize;
use serde_json::{json,Value};
use std::{collections::HashSet,sync::{Arc,atomic::AtomicBool},time::{Duration,Instant}};
use crate::{taskbar_formats as parser,worker::{self,RequestContext}};

#[derive(Default,Deserialize)]#[serde(default,rename_all="PascalCase")]
struct Raw{format:String,original:String,translation:String,romanization:String,fallback:String,document:Option<Lyrics>}
type Gateway<'a>=dyn FnMut(Value)->Result<Value,String>+'a;
struct Engine<'a>{track:&'a Track,gateway:&'a mut Gateway<'a>,errors:Vec<String>}
impl Engine<'_>{
 fn call(&mut self,mut v:Value)->Option<Value>{v["Title"]=json!(self.track.title);v["Artist"]=json!(self.track.artist);v["Album"]=json!(self.track.album);v["DurationMS"]=json!(self.track.duration.as_millis());let source=v["Source"].as_str().unwrap_or("plan").to_owned();match (self.gateway)(v){Ok(v)=>Some(v),Err(e)=>{self.errors.push(format!("{source}: {e}"));None}}}
 fn plan(&mut self)->Vec<Value>{self.call(json!({"Op":"taskbar-plan"})).and_then(|v|v["hits"].as_array().cloned()).unwrap_or_default()}
 fn search(&mut self,source:&str,variant:&Value)->Option<Vec<Value>>{self.call(json!({"Op":"taskbar-search","Source":source,"Hit":variant})).map(|v|v["hits"].as_array().cloned().unwrap_or_default())}
 fn fetch(&mut self,source:&str,hit:&Value,qrc:bool)->Option<Lyrics>{let response=self.call(json!({"Op":if qrc{"taskbar-qrc"}else{"taskbar-fetch"},"Source":source,"Hit":hit}))?;let raw=serde_json::from_value::<Raw>(response["payload"].clone()).ok()?;let mut l=if let Some(doc)=raw.document.filter(|d|!d.lines.is_empty()){parser::finish(doc)}else{match raw.format.as_str(){
   "Qrc"=>parser::word(&crate::crypto::qrc_text(&raw.original).ok()?,Format::Qrc),
   "Krc"=>{let text=if raw.original.trim_start_matches('\u{feff}').lines().any(|l|l.starts_with('[')&&l.split_once(']').is_some_and(|(h,_)|h.contains(','))){raw.original.clone()}else{crate::crypto::krc_text(raw.original.as_bytes()).ok()?};parser::word(&text,Format::Krc)},
   "Yrc"=>{let y=parser::word(&raw.original,Format::Yrc);if y.lines.is_empty(){parser::lrc(&raw.fallback)}else{y}},
   _=>parser::lrc(&raw.original),
  }};
  if l.lines.is_empty(){return None}for(text,roma)in[(&raw.translation,false),(&raw.romanization,true)]{let mut extra=parser::word(text,Format::Yrc);if extra.lines.is_empty(){extra=parser::lrc(text)}parser::secondary(&mut l,&extra,roma)}
  if source=="SodaMusic"{align_preview(self.track,hit,&mut l)}
  (!l.lines.is_empty()).then_some(l)
 }
 fn selection(&self,source:&str,hit:&Value,content:Lyrics)->Selection{let score=hit["Extra"]["score"].as_str().and_then(|s|s.parse().ok()).unwrap_or(100);Selection{content,provider_id:Provider::new(source),candidate_id:format!("taskbar:{source}:{}:{}",hit["ID"].as_str().unwrap_or(""),hit["Extra"]["mid"].as_str().unwrap_or("")),score}}
 fn metadata(&mut self,source:&str,variants:&[Value])->Option<Selection>{let mut searched=HashSet::new();let mut attempted=HashSet::new();for variant in variants{let query=variant["Extra"]["query"].as_str().unwrap_or("");if !searched.insert(query){continue}let ranked=self.search(source,variant)?;for hit in ranked.iter().filter(|h|!attempted.contains(h["ID"].as_str().unwrap_or(""))).take(if source=="SodaMusic"{3}else{usize::MAX}).cloned().collect::<Vec<_>>(){if !attempted.insert(hit["ID"].as_str().unwrap_or("").to_owned()){continue}if let Some(content)=self.fetch(source,&hit,false){return Some(self.selection(source,&hit,content))}}}None}
 fn qq_secondary(&mut self,hit:&Value,content:&mut Lyrics){if let Some(l)=self.fetch("QQMusic",hit,false){let translation=Lyrics{lines:l.lines.iter().filter_map(|l|l.translation.as_ref().filter(|s|!s.is_empty()).map(|text|Cue{at:l.at,text:text.clone(),..Default::default()})).collect(),..Default::default()};let roma=Lyrics{lines:l.lines.iter().filter_map(|l|l.romanization.as_ref().filter(|s|!s.is_empty()).map(|text|Cue{at:l.at,text:text.clone(),..Default::default()})).collect(),..Default::default()};parser::secondary(content,&translation,false);parser::secondary(content,&roma,true)}}
 fn online(&mut self)->Option<Selection>{
  if self.track.title.is_empty(){return None}let variants=self.plan();let player=lyric_core::identity::official(&self.track.source_app).map(|p|p.value().to_owned()).unwrap_or_default();
  if player=="Netease"{if let Some(id)=&self.track.song_id{let hit=json!({"ID":id});if let Some(content)=self.fetch("Netease",&hit,false){return Some(self.selection("Netease",&hit,content))}}}
  if player=="SodaMusic"{if let Some(s)=self.metadata("SodaMusic",&variants){return Some(s)}}
  for variant in &variants{let Some(ranked)=self.search("Kugou",variant)else{continue};for hit in ranked{let duration=if self.track.duration.is_zero(){hit["DurationMS"].as_u64().unwrap_or(0)as u128}else{self.track.duration.as_millis()};let response=(self.gateway)(json!({"Op":"taskbar-kugou-candidates","Source":"Kugou","Hit":hit,"DurationMS":duration}));let Some(candidates)=response.ok().and_then(|v|v["hits"].as_array().cloned())else{continue};for candidate in candidates{if let Some(mut content)=self.fetch("Kugou",&candidate,false){for v in &variants{if let Some(best)=self.search("QQMusic",v).and_then(|h|h.into_iter().next()){self.qq_secondary(&best,&mut content);break}}return Some(self.selection("Kugou",&hit,content))}}}}
  // QRC searches once per variant; LRC reuses those exact candidate lists.
  let mut qq=Vec::new();for variant in &variants{let candidates=self.search("QQMusic",variant).unwrap_or_default();for hit in &candidates{if hit["ID"].as_str().unwrap_or("").is_empty(){continue}if let Some(mut content)=self.fetch("QQMusic",hit,true){self.qq_secondary(hit,&mut content);return Some(self.selection("QQMusic",hit,content))}}qq.push(candidates)}
  for candidates in &qq{for hit in candidates{if let Some(content)=self.fetch("QQMusic",hit,false){return Some(self.selection("QQMusic",hit,content))}}}
  for simple in[false,true]{if let Some(response)=self.call(json!({"Op":"taskbar-fetch","Source":"LRCLIB","Structured":simple})){if let Ok(raw)=serde_json::from_value::<Raw>(response["payload"].clone()){let content=raw.document.filter(|d|!d.lines.is_empty()).map(parser::finish).unwrap_or_else(||parser::lrc(&raw.original));if !content.lines.is_empty(){return Some(self.selection("LRCLIB",&Value::Null,content))}}}}
  if player!="Netease"{if let Some(s)=self.metadata("Netease",&variants){return Some(s)}}
  if player!="SodaMusic"{if let Some(s)=self.metadata("SodaMusic",&variants){return Some(s)}}None
 }
}
fn align_preview(track:&Track,hit:&Value,l:&mut Lyrics){let value=|key|hit["Extra"][key].as_str().and_then(|s|s.parse::<u64>().ok()).unwrap_or(0);let offset=value("previewOffsetMs");if offset==0{return}let full=value("fullDurationMs");let duration=track.duration.as_millis() as u64;let end=offset.saturating_add(duration);let starts=l.lines.iter().map(|c|c.at).collect::<Vec<_>>();let mut i=0;l.lines.retain_mut(|c|{let line_end=c.words.last().map(|w|w.until).unwrap_or_else(||starts.get(i+1).copied().unwrap_or(full));i+=1;if line_end<=offset||c.at>=end{return false}if !c.words.is_empty(){c.words.retain_mut(|w|{if w.until<=offset||w.at>=end{return false}w.at=w.at.saturating_sub(offset);w.until=(w.until-offset).min(duration);true});if c.words.is_empty(){return false}c.text=c.words.iter().map(|w|w.text.as_str()).collect()}c.at=c.at.saturating_sub(offset);c.until=line_end.saturating_sub(offset).min(duration);true});}
pub fn resolve(track:&Track,cancel:Arc<AtomicBool>)->Result<Lookup,String>{
 // The upstream has per-request timeouts and generation cancellation, no total
 // search cutoff that could prevent later sources from being reached.
 let ctx=RequestContext{cancelled:cancel,deadline:Instant::now()+Duration::from_secs(86400)};
 let mut gateway=|v|worker::call(v,&ctx);let mut engine=Engine{track,gateway:&mut gateway,errors:Vec::new()};let selection=engine.online();ctx.check()?;if selection.is_none()&&!engine.errors.is_empty(){return Err(engine.errors.join("; "))}Ok(Lookup::from_lyrics(selection))
}

// Duration enrichment uses the same taskbar metadata adapter as lyric lookup.
pub fn search_metadata(a:&Value)->Result<Value,String>{
 let track:Track=serde_json::from_value(a["track"].clone()).map_err(|e|e.to_string())?;
 let source=a["provider"].as_str().ok_or("provider")?;
 if !matches!(source,"Kugou"|"QQMusic"|"Netease"|"SodaMusic"){return Ok(json!([]))}
 let ctx=RequestContext::new(Duration::from_millis(a["timeoutMs"].as_u64().unwrap_or(5000).min(10000)));
 let args=json!({"Title":track.title,"Artist":track.artist,"Album":track.album,"DurationMS":track.duration.as_millis(),"Op":"taskbar-plan"});
 let plan=worker::call(args,&ctx)?;let Some(variant)=plan["hits"].as_array().and_then(|h|h.first())else{return Ok(json!([]))};
 let response=worker::call(json!({"Op":"taskbar-search","Source":source,"Hit":variant,"Title":track.title,"Artist":track.artist,"DurationMS":track.duration.as_millis()}),&ctx)?;
 let rows=response["hits"].as_array().cloned().unwrap_or_default().into_iter().map(|h|Hit{provider:Provider::new(source),id:h["ID"].as_str().unwrap_or_default().into(),title:h["Title"].as_str().unwrap_or_default().into(),artists:serde_json::from_value(h["Artists"].clone()).unwrap_or_default(),album:h["Album"].as_str().unwrap_or_default().into(),duration:Duration::from_millis(h["DurationMS"].as_u64().unwrap_or(0)),extra:serde_json::from_value(h["Extra"].clone()).unwrap_or_default()}).collect::<Vec<_>>();Ok(json!(rows))
}

#[cfg(test)]mod tests{use super::*;
 fn track()->Track{Track::new("id","song","artist","","Browser",Duration::from_secs(200))}
 fn candidate()->Value{json!({"ID":"123","Title":"song","Extra":{"mid":"mid","score":"100"}})}
 #[test]fn sequential_fallback_reuses_qq_search_and_rejects_bad_qrc(){let t=track();let mut ops=Vec::new();let mut gateway=|v:Value|{ops.push(format!("{}:{}",v["Source"].as_str().unwrap_or(""),v["Op"].as_str().unwrap()));Ok(match v["Op"].as_str().unwrap(){"taskbar-plan"=>json!({"hits":[{"Title":"song","Extra":{"query":"song artist"}}]}),"taskbar-search" if v["Source"]=="QQMusic"=>json!({"hits":[candidate()]}),"taskbar-fetch" if v["Source"]=="QQMusic"=>json!({"payload":{"Format":"Lrc","Original":"[00:01]found"}}),_=>json!({})})};let mut e=Engine{track:&t,gateway:&mut gateway,errors:vec![]};let s=e.online().unwrap();assert_eq!(s.provider_id.value(),"QQMusic");drop(e);assert_eq!(ops,[":taskbar-plan","Kugou:taskbar-search","QQMusic:taskbar-search","QQMusic:taskbar-qrc","QQMusic:taskbar-fetch"]);}
 #[test]fn netease_player_uses_known_id_first_and_no_metadata_retry(){let t=track().with_song_id("99");let mut t=t;t.source_app="Netease".into();let mut ops=Vec::new();let mut gateway=|v:Value|{ops.push(v.clone());Ok(if v["Op"]=="taskbar-plan"{json!({"hits":[]})}else{json!({"payload":{"Format":"Yrc","Original":"[1000,1](1000,1,0)x"}})})};let mut e=Engine{track:&t,gateway:&mut gateway,errors:vec![]};assert_eq!(e.online().unwrap().provider_id.value(),"Netease");drop(e);assert_eq!(ops[1]["Hit"]["ID"],"99");}
 #[test]fn all_fallback_sources_keep_upstream_order(){let t=track();let mut ops=Vec::new();let mut gateway=|v:Value|{ops.push((v["Source"].clone(),v["Op"].clone()));Ok(if v["Op"]=="taskbar-plan"{json!({"hits":[{"Extra":{"query":"song artist"}}]})}else{json!({})})};let mut e=Engine{track:&t,gateway:&mut gateway,errors:vec![]};assert!(e.online().is_none());drop(e);let sources=ops.iter().skip(1).map(|v|v.0.as_str().unwrap()).collect::<Vec<_>>();assert_eq!(sources,["Kugou","QQMusic","LRCLIB","LRCLIB","Netease","SodaMusic"]);}
}
