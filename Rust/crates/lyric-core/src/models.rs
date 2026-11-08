//! DotIsle's playback contract and single lyric timeline representation.
use serde::{Deserialize,Serialize};
use std::{collections::BTreeMap,time::Duration};
#[derive(Clone,Debug,Default,Serialize,Deserialize)]
pub struct Track{pub id:String,pub title:String,pub artist:String,pub album:String,pub source_app:String,pub duration:Duration,pub song_id:Option<String>}
impl Track{
 pub fn new(id:impl Into<String>,title:impl Into<String>,artist:impl Into<String>,album:impl Into<String>,source:impl Into<String>,duration:Duration)->Self{Self{id:id.into(),title:title.into(),artist:artist.into(),album:album.into(),source_app:source.into(),duration,song_id:None}}
 pub fn with_song_id(mut self,id:impl Into<String>)->Self{self.song_id=Some(id.into());self}
}
#[derive(Clone,Debug,Default)]
pub struct Playback{pub track:Option<Track>,pub is_playing:bool,pub position:Duration,pub raw_position:Option<Duration>,pub extrapolated_position:Option<Duration>,pub cover_image_bytes:Option<Vec<u8>>,pub is_cover_loading:bool,pub source_app_user_model_id:String,pub can_seek:bool}
impl Playback{pub fn idle()->Self{Self::default()}pub fn playing(track:Track,position:Duration)->Self{Self{track:Some(track),position,is_playing:true,..Default::default()}}}
#[derive(Clone,Debug,Eq,PartialEq,Ord,PartialOrd,Hash,Serialize,Deserialize)]
#[serde(transparent)]pub struct Provider(String);
impl Provider{pub fn new(id:&str)->Self{Self(id.to_owned())}pub fn value(&self)->&str{&self.0}pub fn eq_ignore_ascii_case(&self,s:&str)->bool{self.0.eq_ignore_ascii_case(s)}}
impl std::fmt::Display for Provider{fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result{self.0.fmt(f)}}
pub fn providers()->Vec<Provider>{["Netease","QQMusic","Kugou","LRCLIB","Kuwo","Migu"].map(Provider::new).to_vec()}
#[derive(Clone,Copy,Debug,Default,Serialize,Deserialize,PartialEq,Eq)]pub enum Format{#[default]Lrc,Qrc,Krc,Yrc,Ttml,Text}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]pub struct Word{pub at:u64,pub until:u64,pub text:String}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]pub struct Cue{pub at:u64,pub until:u64,pub text:String,pub translation:Option<String>,pub words:Vec<Word>}
#[derive(Clone,Debug,Default,Serialize,Deserialize)]pub struct Lyrics{pub lines:Vec<Cue>,pub instrumental:bool,pub timed:bool}
impl Lyrics{
 pub fn instrumental()->Self{Self{instrumental:true,..Default::default()}}
 pub fn usable(&self)->bool{self.instrumental||self.lines.iter().any(|l|!l.text.trim().is_empty())}
 pub fn finalize(&mut self){
  self.lines.retain(|c|c.at<=86_400_000&&!c.text.trim().is_empty());self.lines.sort_by_key(|c|c.at);
  for i in 0..self.lines.len(){let next=self.lines.get(i+1).map(|c|c.at);let c=&mut self.lines[i];if c.until<=c.at{c.until=next.filter(|n|*n>c.at).unwrap_or(c.at+6000)}if let Some(n)=next.filter(|n|*n>c.at){c.until=c.until.min(n)}c.words.retain(|w|!w.text.is_empty()&&w.until>=w.at&&w.at>=c.at&&w.at<=c.until);c.words.sort_by_key(|w|w.at);}
 }
}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Selection{pub content:Lyrics,pub provider_id:Provider,pub candidate_id:String,pub score:u8}
impl Selection{pub fn identity_score(&self)->Option<i32>{Some(self.score as i32)}}
#[derive(Clone,Copy,Debug,PartialEq,Eq)]pub enum LookupStatus{Found,Missing,TransientFailure}
#[derive(Clone,Debug)]pub struct Lookup{pub lyrics:Option<Selection>,pub status:LookupStatus}
impl Lookup{pub fn from_lyrics(lyrics:Option<Selection>)->Self{Self{status:if lyrics.is_some(){LookupStatus::Found}else{LookupStatus::Missing},lyrics}}pub fn unavailable()->Self{Self::from_lyrics(None)}}
#[derive(Clone,Debug)]pub struct Hit{pub provider:Provider,pub id:String,pub title:String,pub artists:Vec<String>,pub album:String,pub duration:Duration,pub extra:BTreeMap<String,String>}
#[derive(Clone,Debug,Serialize,Deserialize)]pub struct Payload{pub format:Format,pub original:String,pub translation:String,pub instrumental:bool}
#[derive(Default)]pub struct DisplayFrame{pub current_line:String,pub next_line:String,pub title:String,pub line_progress:f64,pub current_line_index:i32,pub is_pure_music:bool,pub word_scan_progress:Option<f64>,pub current_translation:Option<String>,pub next_translation:Option<String>,pub has_track_translation:bool}
