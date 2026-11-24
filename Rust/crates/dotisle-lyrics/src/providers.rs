use crate::{resolution::Source, worker::{self, RequestContext}};
use lyric_core::{cache::Cache, identity, models::*};
use serde::Deserialize;
use serde_json::json;
use std::{collections::BTreeMap, sync::Arc, time::Duration};

pub struct OnlineSource { pub id:Provider, pub context:RequestContext, pub cache:Option<Arc<Cache>> }

#[derive(Deserialize)]
#[serde(rename_all="PascalCase")]
struct RemoteHit {
 #[serde(rename="ID")]
 id:String,
 title:String,
 artists:Vec<String>,
 album:String,
 #[serde(rename="DurationMS")]
 duration_ms:u64,
 extra:Option<BTreeMap<String,String>>,
}

impl OnlineSource {
 pub fn new(id:&str,context:RequestContext)->Self{Self{id:Provider::new(id),context,cache:None}}
 fn query(&self,t:&Track,query:&str,structured:bool)->Result<Vec<Hit>,String>{
  let v=worker::call(json!({"Op":"search","Source":self.id.value(),"Query":query,"Structured":structured,
    "Title":t.title,"Artist":t.artist,"Album":t.album,"DurationMS":t.duration.as_millis().min(86_400_000)}),&self.context)?;
  if v["hits"].is_null(){return Ok(Vec::new())}
  let hits=serde_json::from_value::<Vec<RemoteHit>>(v["hits"].clone()).map_err(|e|e.to_string())?;
  Ok(hits.into_iter().map(|h|{
   let mut hit=Hit{provider:self.id.clone(),id:h.id,title:h.title,artists:h.artists,album:h.album,
     duration:Duration::from_millis(h.duration_ms.min(86_400_000)),extra:h.extra.unwrap_or_default()};
   if self.id.value()=="SodaMusic"{crate::soda::set_preview(t,&mut hit)}
   hit
  }).collect())
 }
 fn download(&self,h:&Hit,op:&str)->Result<Option<Payload>,String>{
  let mut v=worker::call(json!({"Op":op,"Source":self.id.value(),"Hit":{"ID":h.id,"Extra":h.extra}}),&self.context)?;
  let value=v["payload"].take();if value.is_null(){return Ok(None)}
  #[derive(Deserialize)] #[serde(rename_all="PascalCase")]
  struct RemotePayload{format:Format,original:String,translation:String,instrumental:bool}
  let p:RemotePayload=serde_json::from_value(value).map_err(|e|e.to_string())?;
  let translation=if p.format==Format::Qrc{crate::crypto::qrc_text(&p.translation).unwrap_or_default()}else{p.translation};
  Ok(Some(Payload{format:p.format,original:p.original,translation,instrumental:p.instrumental}))
 }
}

impl Source for OnlineSource {
 fn search(&self,t:&Track)->Result<Vec<Hit>,String>{
  self.context.check()?;
  if identity::official(&t.source_app).as_ref()==Some(&self.id)&&matches!(self.id.value(),"Netease"|"QQMusic"|"Kuwo"){
   if let Some(id)=t.song_id.as_ref().filter(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit())){
    return Ok(vec![Hit{provider:self.id.clone(),id:id.clone(),title:t.title.clone(),artists:identity::artists(&t.artist),
     album:t.album.clone(),duration:t.duration,extra:Default::default()}]);
   }
  }
  let clean_title = identity::search_title(&t.title);
  let primary_artist = identity::artists(&t.artist);
  let query_artist = primary_artist.first().map(String::as_str).unwrap_or(&t.artist);
  let clean_q = search_query(&identity::search_text(clean_title), &identity::search_text(query_artist));
  let initial = if clean_q.trim().is_empty() { search_query(&t.title, &t.artist) } else { clean_q };
  self.query(t, &initial, true)
 }
 fn fallback(&self,t:&Track)->Result<Vec<Hit>,String>{
  if self.id.value()=="AMLL"{
   let title=identity::search_text(identity::search_title(&t.title));
   return self.query(t,&title,false);
  }
  let mut hits=Vec::new();let mut error=None;
  for query in fallback_queries(t,self.id.value()=="LRCLIB"){
   if let Err(e)=self.context.check(){error=Some(e);break}
   match self.query(t,&query,false){Ok(rows)=>{
     let admitted=rows.iter().any(|h|identity::rate(t,h).is_some_and(|n|n>=96));
     hits.extend(rows);
     if admitted{break}
   },Err(e)=>error=Some(e)}
  }
  if hits.is_empty(){if let Some(e)=error{return Err(e)}}
  Ok(hits)
 }
 fn align(&self,t:&Track,h:&Hit,content:&mut Lyrics){if self.id.value()=="SodaMusic"{crate::soda::align(t,h,content)}}
 fn fetch(&self,h:&Hit)->Result<Option<Payload>,String>{
  self.context.check()?;
  let key=serde_json::to_string(&("payload-3",self.id.value(),&h.id)).unwrap_or_default();
  if let Some(value)=self.cache.as_ref().and_then(|c|c.get(&key)){return Ok(Some(value))}
  let mut payload=self.download(h,"fetch")?;
  if payload.as_ref().is_some_and(|p|p.format==Format::Qrc&&!crate::formats::parse(p).is_ok_and(|l|l.usable())){
   payload=self.download(h,"fetch-lrc")?;
  }
  if let(Some(cache),Some(payload))=(&self.cache,&payload){cache.put(&key,payload)}
  Ok(payload)
 }
}

fn search_query(title:&str,artist:&str)->String{
 let title=title.trim();let artist=artist.trim();
 if artist.is_empty()||title.to_lowercase().ends_with(&format!(" - {}",artist.to_lowercase())){title.to_owned()}else{format!("{title} {artist}")}
}

fn fallback_queries(t:&Track,unstructured_exact:bool)->Vec<String>{
 let exact=search_query(&t.title,&t.artist);let title=identity::search_title(&t.title);let artists=identity::artists(&t.artist);
 let artist=artists.first().map(String::as_str).unwrap_or("");let mut queries=Vec::new();
 for query in [unstructured_exact.then_some(exact.clone()),Some(search_query(&identity::search_text(title),&identity::search_text(&t.artist))),
   Some(search_query(&identity::search_text(title),&identity::search_text(artist))),Some(search_query(title,artist)),Some(identity::search_text(title)),Some(title.trim().to_owned())].into_iter().flatten(){
  if !query.is_empty()&&(unstructured_exact||query!=exact)&&!queries.contains(&query){queries.push(query);}
 }
 queries.truncate(4);queries
}

#[cfg(test)]
#[test]
fn fallback_queries_cover_normalization_and_primary_artist(){
 let t=Track::new("x","夜航（Official Video）","甲 feat. 乙","","Browser",Duration::from_secs(200));
 let queries=fallback_queries(&t,false);
 assert!(queries.contains(&"夜航 甲".into())&&queries.contains(&"夜航".into()));
 assert!(!queries.iter().any(|q|q.contains("Official Video")));
 assert_eq!(fallback_queries(&t,true)[0],search_query(&t.title,&t.artist));
}
