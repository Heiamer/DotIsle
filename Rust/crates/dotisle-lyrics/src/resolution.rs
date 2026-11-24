//! Source-independent lookup: identity gates are evaluated before any lyric download.
use lyric_core::{models::*,identity};
pub trait Source:Send+Sync{
 fn search(&self,track:&Track)->Result<Vec<Hit>,String>;
 fn fallback(&self,_track:&Track)->Result<Vec<Hit>,String>{Ok(Vec::new())}
 fn fetch(&self,hit:&Hit)->Result<Option<Payload>,String>;
 fn align(&self,_track:&Track,_hit:&Hit,_content:&mut Lyrics){}
}
/// 过滤“纯音乐，请欣赏”或仅有作词/作曲/制作人名单而无实质歌词的空壳歌词。
fn is_placeholder_lyrics(content: &Lyrics) -> bool {
 if content.instrumental || content.lines.is_empty() {
  return false;
 }
 let non_empty: Vec<&str> = content
  .lines
  .iter()
  .map(|l| l.text.trim())
  .filter(|t| !t.is_empty())
  .collect();
 if non_empty.is_empty() {
  return true;
 }
 if non_empty.len() > 6 {
  return false;
 }
 let meta_count = non_empty
  .iter()
  .filter(|t| {
   let low = t.to_lowercase();
   low.contains("纯音乐")
    || low.contains("没有填词")
    || low.contains("此歌曲为")
    || low.contains("作词")
    || low.contains("作曲")
    || low.contains("编曲")
    || low.contains("制作人")
    || low.contains("lyricist")
    || low.contains("composer")
  })
  .count();
 meta_count == non_empty.len() || (non_empty.len() <= 3 && meta_count * 2 >= non_empty.len() && non_empty.iter().any(|t| t.contains("纯音乐") || t.contains("没有填词")))
}

pub fn resolve(track:&Track,source:&dyn Source)->Lookup{
 let mut tried=std::collections::HashSet::new();
 let mut failed=false;
 let mut best=None::<Selection>;
 for fallback in [false,true]{
     let hits=match if fallback {source.fallback(track)} else {source.search(track)}{
         Ok(h)=>h, Err(_)=>{failed=true;continue}
     };
     let mut rated=hits.into_iter().filter_map(|h|identity::rate(track,&h).map(|score|(score,h))).collect::<Vec<_>>();
     rated.sort_by(|a,b|b.0.cmp(&a.0));
     for (score, hit) in rated.into_iter().filter(|(_, hit)| {
         // The desktop ID can fetch QRC, but the web LRC endpoint requires the MID.
         let mid = hit.extra.get("mid").cloned().unwrap_or_default();
         tried.insert((hit.id.clone(), mid))
     }).take(5) {
         match source.fetch(&hit){
             Ok(Some(p))=>if let Ok(mut content)=crate::formats::parse(&p){
                 source.align(track,&hit,&mut content);
                 if !content.usable() || is_placeholder_lyrics(&content){continue}
                 if content.instrumental&&score<95{continue}
                 let selection=Selection{content,provider_id:hit.provider,candidate_id:hit.id,score};
                 if best.as_ref().is_none_or(|old|quality(&selection)>quality(old)){best=Some(selection);}
                 // 仅当既高匹配度、又是时间轴对齐且最好含逐字或翻译时才早退
                 let has_words = best.as_ref().is_some_and(|s| s.content.lines.iter().any(|l| !l.words.is_empty()));
                 if best.as_ref().is_some_and(|s|s.content.timed&&s.score>=95&&has_words){break}
             }, Err(_)=>failed=true, Ok(None)=>{}
         }
     }
     if best.as_ref().is_some_and(|s|(s.content.timed||s.content.instrumental)&&s.score>=95){break}
 }
 if best.is_some(){return Lookup::from_lyrics(best)}
 Lookup{lyrics:None,status:if failed{LookupStatus::TransientFailure}else{LookupStatus::Missing}}
}

pub fn quality(s:&Selection)->(u16,u8){
 let timed_bonus = u16::from(s.content.timed) * 4;
 let word_bonus = u16::from(s.content.lines.iter().any(|l| !l.words.is_empty())) * 3;
 let trans_bonus = u16::from(s.content.lines.iter().any(|l| l.translation.as_ref().is_some_and(|t| !t.trim().is_empty()))) * 2;
 let unverified_duration = u16::from(s.provider_id.value() == "AMLL") * 4;
 // 身份匹配分权重放大 2 倍，确保 98 分精准 LRC 不会被 86 分错歌用逐字特效反超
 let total = (u16::from(s.score) * 2)
  .saturating_add(timed_bonus + word_bonus + trans_bonus)
  .saturating_sub(unverified_duration);
 (total, s.score)
}
#[cfg(test)]mod tests{use super::*;use std::{sync::atomic::{AtomicUsize,Ordering},time::Duration};
 struct Mock{count:AtomicUsize,fail:bool}impl Source for Mock{fn search(&self,t:&Track)->Result<Vec<Hit>,String>{Ok(vec![Hit{provider:Provider::new("Local"),id:"a".into(),title:t.title.clone(),artists:vec![t.artist.clone()],album:String::new(),duration:t.duration,extra:Default::default()},Hit{provider:Provider::new("Local"),id:"b".into(),title:format!("{} (Live)",t.title),artists:vec![t.artist.clone()],album:String::new(),duration:t.duration,extra:Default::default()}])}fn fetch(&self,_:&Hit)->Result<Option<Payload>,String>{self.count.fetch_add(1,Ordering::SeqCst);if self.fail{return Err("timeout".into())}Ok(Some(Payload{format:Format::Lrc,original:"[00:02.00]独立测试".into(),translation:String::new(),instrumental:false}))}}
 #[test]fn downloads_only_admitted_candidates_and_retries_fetch_failures(){let t=Track::new("x","歌声","甲","","LRCLIB",Duration::from_secs(93));let m=Mock{count:AtomicUsize::new(0),fail:false};assert_eq!(resolve(&t,&m).status,LookupStatus::Found);assert_eq!(m.count.load(Ordering::SeqCst),1);let m=Mock{count:AtomicUsize::new(0),fail:true};assert_eq!(resolve(&t,&m).status,LookupStatus::TransientFailure);assert_eq!(m.count.load(Ordering::SeqCst),1);}
 #[test]
 fn qq_can_retry_the_same_song_when_search_adds_the_web_mid() {
     struct Qq;
     impl Source for Qq {
         fn search(&self, track: &Track) -> Result<Vec<Hit>, String> {
             Ok(vec![Hit { provider: Provider::new("QQMusic"), id: "123".into(), title: track.title.clone(),
                 artists: vec![track.artist.clone()], album: track.album.clone(), duration: track.duration, extra: Default::default() }])
         }
         fn fallback(&self, track: &Track) -> Result<Vec<Hit>, String> {
             let mut rows = self.search(track)?;
             rows[0].extra.insert("mid".into(), "abc".into());
             Ok(rows)
         }
         fn fetch(&self, hit: &Hit) -> Result<Option<Payload>, String> {
             Ok(hit.extra.contains_key("mid").then(|| Payload { format: Format::Lrc, original: "[00:02.00]逐行歌词".into(),
                 translation: String::new(), instrumental: false }))
         }
     }
     let track = Track::new("x", "歌声", "甲", "", "QQMusic", Duration::from_secs(93));
     assert_eq!(resolve(&track, &Qq).status, LookupStatus::Found);
 }
 #[test]
 fn placeholder_credit_only_lyrics_are_skipped_for_real_lyrics() {
     struct FakeFirst;
     impl Source for FakeFirst {
         fn search(&self, track: &Track) -> Result<Vec<Hit>, String> {
             Ok(vec![
                 Hit { provider: Provider::new("Netease"), id: "1".into(), title: track.title.clone(),
                     artists: vec![track.artist.clone()], album: String::new(), duration: track.duration, extra: Default::default() },
                 Hit { provider: Provider::new("Netease"), id: "2".into(), title: track.title.clone(),
                     artists: vec![track.artist.clone()], album: String::new(), duration: track.duration, extra: Default::default() },
             ])
         }
         fn fetch(&self, hit: &Hit) -> Result<Option<Payload>, String> {
             Ok(Some(if hit.id == "1" {
                 Payload { format: Format::Lrc, original: "[00:00.00]作词 : 甲\n[00:01.00]纯音乐，请欣赏".into(), translation: String::new(), instrumental: false }
             } else {
                 Payload { format: Format::Lrc, original: "[00:05.00]海风吹过灯塔\n[00:10.00]故事才刚刚开场".into(), translation: "[00:05.00]Sea breeze".into(), instrumental: false }
             }))
         }
     }
     let track = Track::new("x", "灯塔", "甲", "", "Netease", Duration::from_secs(210));
     let res = resolve(&track, &FakeFirst);
     assert_eq!(res.lyrics.unwrap().candidate_id, "2");
 }
 #[test]
 fn high_identity_match_beats_lower_score_word_timed_candidate() {
     let mut word_lyrics = Lyrics { timed: true, instrumental: false, lines: vec![Cue { at: 1000, until: 4000, text: "错曲".into(), translation: None, words: vec![Word { at: 1000, until: 2000, text: "错".into() }] }] };
     word_lyrics.finalize();
     let mut exact_lrc = Lyrics { timed: true, instrumental: false, lines: vec![Cue { at: 1000, until: 4000, text: "正曲".into(), translation: Some("Translation".into()), words: vec![] }] };
     exact_lrc.finalize();
     let low_word = Selection { content: word_lyrics, provider_id: Provider::new("Netease"), candidate_id: "low".into(), score: 86 };
     let high_lrc = Selection { content: exact_lrc, provider_id: Provider::new("QQMusic"), candidate_id: "high".into(), score: 98 };
     assert!(quality(&high_lrc) > quality(&low_word));
 }
}
