//! Conservative song identity matching, based on Unicode text and edit distance.
use crate::models::*;
use unicode_normalization::UnicodeNormalization;
use std::collections::BTreeSet;
pub fn normalized(s:&str)->String{
 let s=simplified(s);s.nfkd().flat_map(char::to_lowercase).filter(|c|c.is_alphanumeric()).collect()
}
#[cfg(windows)]fn simplified(s:&str)->String{
 #[link(name="kernel32")]extern "system"{fn LCMapStringW(locale:u32,flags:u32,src:*const u16,n:i32,dst:*mut u16,m:i32)->i32;}
 let src=s.encode_utf16().collect::<Vec<_>>();if src.is_empty(){return String::new()}let mut dst=vec![0;src.len()*3];let n=unsafe{LCMapStringW(0x0804,0x02000000,src.as_ptr(),src.len()as i32,dst.as_mut_ptr(),dst.len()as i32)};if n>0{String::from_utf16_lossy(&dst[..n as usize])}else{s.into()}
}
#[cfg(not(windows))]fn simplified(s:&str)->String{s.into()}
pub fn artists(s:&str)->Vec<String>{s.split(['/', '&', '、', ',', '，',';']).map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned).collect()}
fn similarity(a:&str,b:&str)->f64{
 let a=normalized(a).chars().take(256).collect::<Vec<_>>();let b=normalized(b).chars().take(256).collect::<Vec<_>>();if a.is_empty()||b.is_empty(){return 0.}let mut row=(0..=b.len()).collect::<Vec<_>>();
 for(i,x)in a.iter().enumerate(){let mut prev=row[0];row[0]=i+1;for(j,y)in b.iter().enumerate(){let old=row[j+1];row[j+1]=(prev+usize::from(x!=y)).min(row[j]+1).min(old+1);prev=old;}}1.-row[b.len()]as f64/a.len().max(b.len())as f64
}
fn versions(s:&str)->BTreeSet<&'static str>{let s=s.to_lowercase();[("live","live"),("现场","live"),("remix","remix"),("伴奏","instrumental"),("instrumental","instrumental"),("acoustic","acoustic"),("demo","demo"),("翻唱","cover"),("cover","cover"),("sped up","speed"),("slowed","slow")].into_iter().filter_map(|(word,tag)|s.contains(word).then_some(tag)).collect()}
pub fn official(source:&str)->Option<Provider>{let source=source.to_lowercase();[("netease","Netease"),("cloudmusic","Netease"),("qqmusic","QQMusic"),("kugou","Kugou"),("kuwo","Kuwo"),("migu","Migu")].into_iter().find_map(|(s,p)|source.contains(s).then(||Provider::new(p)))}
pub fn track_key(t:&Track)->String{serde_json::to_string(&(normalized(&t.source_app),&t.song_id,normalized(&t.title),normalized(&t.artist),normalized(&t.album))).unwrap_or_default()}
pub fn rate(t:&Track,h:&Hit)->Option<u8>{
 if versions(&t.title)!=versions(&h.title){return None}
 let title=similarity(&t.title,&h.title);if title<0.83{return None}
 let wanted=artists(&t.artist);let artist=if wanted.is_empty(){0.7}else{wanted.iter().map(|a|h.artists.iter().map(|b|similarity(a,b)).fold(0.,f64::max)).sum::<f64>()/wanted.len()as f64};if artist<0.8{return None}
 let duration=if t.duration.is_zero()||h.duration.is_zero(){0.6}else{let delta=t.duration.as_secs_f64().abs_diff(h.duration.as_secs_f64());if delta>12_f64.max(t.duration.as_secs_f64()*0.07){return None}1.-(delta/20.).min(1.)};
 let album=if t.album.is_empty()||h.album.is_empty(){0.5}else{similarity(&t.album,&h.album)};
 Some((title*60.+artist*30.+duration*7.+album*3.).round().clamp(0.,100.)as u8)
}
trait Difference{fn abs_diff(self,other:Self)->Self;}impl Difference for f64{fn abs_diff(self,other:Self)->Self{(self-other).abs()}}
#[cfg(test)]mod tests{use super::*;use std::time::Duration;
 #[test]fn identity_rejects_variants_and_performers(){let t=Track::new("a","轨道","甲","","QQMusic",Duration::from_secs(204));let mut h=Hit{provider:Provider::new("QQMusic"),id:"b".into(),title:"轨道".into(),artists:vec!["甲".into()],album:String::new(),duration:t.duration,extra:Default::default()};assert!(rate(&t,&h).unwrap()>=95);h.title="轨道 (Live)".into();assert!(rate(&t,&h).is_none());h.title=t.title.clone();h.artists=vec!["乙".into()];assert!(rate(&t,&h).is_none());}
 #[test]fn punctuation_diacritics_and_collaborations(){assert_eq!(normalized("Café：夜！"),normalized("cafe 夜"));assert_eq!(artists("甲 / 乙、丙"),["甲","乙","丙"]);}
 #[test]fn identity_changes_with_song_and_artist(){let mut t=Track::new("a","同名","甲","","Netease",Duration::ZERO);let a=track_key(&t);t.artist="乙".into();assert_ne!(a,track_key(&t));}
}
