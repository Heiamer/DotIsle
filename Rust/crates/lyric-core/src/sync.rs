//! Playback is sampled without accumulating elapsed time in the lyric renderer.
use crate::{models::*,identity::track_key};
pub struct Timeline{identity:String,lyrics:Option<Selection>,waiting:bool}
impl Default for Timeline{fn default()->Self{Self::new()}}
impl Timeline{
 pub fn new()->Self{Self{identity:String::new(),lyrics:None,waiting:false}}
 pub fn build_stable_track_identity(t:&Track)->String{track_key(t)}
 pub fn begin_track(&mut self,t:&Track)->bool{let key=track_key(t);if self.identity==key{return false}self.identity=key;self.lyrics=None;self.waiting=true;true}
 pub fn complete_search(&mut self,t:&Track,result:Option<&Selection>){if self.identity==track_key(t){self.lyrics=result.filter(|r|r.content.usable()).cloned();self.waiting=false;}}
 pub fn current_document(&self)->Option<&Lyrics>{self.lyrics.as_ref().map(|r|&r.content)}
 pub fn current_lyric_source_app(&self)->Option<&str>{self.lyrics.as_ref().map(|r|r.provider_id.value())}
 pub fn get_display_frame(&mut self,playback:&Playback)->DisplayFrame{
  let mut f=DisplayFrame{current_line_index:-1,..Default::default()};let Some(track)=&playback.track else{self.identity.clear();self.lyrics=None;self.waiting=false;return f};if self.identity!=track_key(track){self.begin_track(track);}f.title=track.title.clone();
  let Some(result)=&self.lyrics else{f.current_line=if self.waiting{"正在检索歌词…"}else{"暂未找到歌词"}.into();return f};let l=&result.content;f.has_track_translation=l.lines.iter().any(|c|c.translation.is_some());f.is_pure_music=l.instrumental;if l.instrumental{f.current_line="纯音乐，请欣赏".into();return f}
  let at=playback.position.as_millis().min(u64::MAX as u128)as u64;let index=if !l.timed{Some(0)}else{l.lines.partition_point(|c|c.at<=at).checked_sub(1)};let Some(i)=index else{f.next_line=l.lines.first().map(|l|l.text.clone()).unwrap_or_default();f.next_translation=l.lines.first().and_then(|l|l.translation.clone());return f};let Some(c)=l.lines.get(i)else{return f};
  f.current_line_index=i as i32;f.current_line=c.text.clone();f.current_translation=c.translation.clone();f.line_progress=if c.until>c.at{(at.saturating_sub(c.at)as f64/(c.until-c.at)as f64).clamp(0.,1.)}else{0.};
  if let Some(next)=l.lines.get(i+1){f.next_line=next.text.clone();f.next_translation=next.translation.clone()}
  if !c.words.is_empty(){let total=c.words.iter().map(|w|w.text.chars().count()).sum::<usize>();if total>0{let scanned=c.words.iter().map(|w|{let fraction=if at<w.at{0.}else if w.until<=w.at{1.}else{(at.saturating_sub(w.at)as f64/(w.until-w.at)as f64).min(1.)};fraction*w.text.chars().count()as f64}).sum::<f64>();f.word_scan_progress=Some((scanned/total as f64).clamp(0.,1.));}}
  f
 }
}
#[cfg(test)]mod tests{use super::*;use std::time::Duration;
 fn track(id:&str)->Track{Track::new(id,id,"作者","","Netease",Duration::from_secs(120)).with_song_id(id)}
 fn selection()->Selection{Selection{content:crate::formats::lrc("[00:02.50]<00:02.50>晨<00:03.10>光<00:04.00>\n[00:05.00]海面"),provider_id:Provider::new("Local"),candidate_id:"test".into(),score:100}}
 #[test]fn seeks_and_pause_follow_snapshot_without_drift(){let t=track("11");let mut s=Timeline::new();s.begin_track(&t);s.complete_search(&t,Some(&selection()));let mut p=Playback::playing(t,Duration::from_millis(3100));p.is_playing=false;let f=s.get_display_frame(&p);assert_eq!(f.current_line,"晨光");assert_eq!(f.word_scan_progress,Some(0.5));assert_eq!(s.get_display_frame(&p).word_scan_progress,f.word_scan_progress);p.position=Duration::from_secs(6);assert_eq!(s.get_display_frame(&p).current_line,"海面");p.position=Duration::from_secs(1);let f=s.get_display_frame(&p);assert_eq!(f.current_line_index,-1);assert_eq!(f.next_line,"晨光");}
 #[test]fn late_results_cannot_cross_tracks_or_idle(){let mut s=Timeline::new();let a=track("12");let b=track("13");s.begin_track(&a);s.begin_track(&b);s.complete_search(&a,Some(&selection()));assert!(s.current_document().is_none());s.complete_search(&b,Some(&selection()));assert!(s.current_document().is_some());s.get_display_frame(&Playback::idle());assert!(s.current_document().is_none());}
}
