//! Online parser port from taskbar-lyrics-displayer 3fae928 (Apache-2.0).
//! Local-file parsing remains the host application's existing implementation.
use lyric_core::models::*;
use regex::Regex;
use std::sync::LazyLock;

static HEAD:LazyLock<Regex>=LazyLock::new(||Regex::new(r"^\[(\d+),(\d+)\]").unwrap());
static YRC:LazyLock<Regex>=LazyLock::new(||Regex::new(r"\((-?\d+),(-?\d+),[^)]*\)").unwrap());
static KRC:LazyLock<Regex>=LazyLock::new(||Regex::new(r"<(-?\d+),(-?\d+),[^>]*>").unwrap());
static QRC:LazyLock<Regex>=LazyLock::new(||Regex::new(r"\((\d+),(\d+)\)").unwrap());
static FLOAT:LazyLock<Regex>=LazyLock::new(||Regex::new(r"^[ \t]*[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?").unwrap());

pub fn stamp(s:&str)->Option<u64>{let(m,s)=s.split_once(':')?;if m.is_empty()||!m.bytes().all(|b|b.is_ascii_digit()){return None}let n=FLOAT.find(s)?.as_str().trim().parse::<f64>().ok()?;let ms=(m.parse::<f64>().ok()?*60.+n)*1000.+0.5;(ms.is_finite()&&ms>=0.&&ms<(u64::MAX as f64)).then_some(ms as u64)}
fn trim(s:&str)->&str{s.trim_matches([' ','\t'])}
pub fn finish(mut l:Lyrics)->Lyrics{l.lines.sort_by_key(|c|c.at);for i in 0..l.lines.len(){let end=l.lines.get(i+1).map(|c|c.at).unwrap_or(l.lines[i].at.saturating_add(6000));let c=&mut l.lines[i];c.until=c.words.last().map(|w|w.until).unwrap_or(end).max(c.at.saturating_add(1));}l.timed=true;l}
pub fn lrc(input:&str)->Lyrics{
 let mut l=Lyrics::default();for raw in input.lines(){let raw=raw.trim_end_matches('\r');let mut rest=raw;let mut times=Vec::new();while rest.starts_with('['){let Some(end)=rest.find(']')else{break};let Some(ms)=stamp(&rest[1..end])else{times.clear();break};times.push(ms);rest=&rest[end+1..];}let text=trim(rest);if text.is_empty(){continue}for at in times{l.lines.push(Cue{at,text:text.into(),..Default::default()});}}finish(l)
}
fn unwrapped(mut s:String)->String{if s.starts_with('\u{feff}'){s=s.trim_start_matches('\u{feff}').into()}if s.contains("<Lyric_1"){let Some((_,rest))=s.split_once("LyricContent=\"")else{return String::new()};let Some((content,_))=rest.split_once('"')else{return String::new()};s=content.into();for(a,b)in[("&quot;","\""),("&apos;","'"),("&lt;","<"),("&gt;",">"),("&amp;","&")]{s=s.replace(a,b)}}s}
pub fn word(input:&str,format:Format)->Lyrics{
 let s=if format==Format::Qrc{unwrapped(input.into())}else{input.trim_start_matches('\u{feff}').to_owned()};let s=if format==Format::Krc{s.replace("&apos;","'").replace("&quot;","\"").replace("&amp;","&")}else{s};let mut result=Lyrics::default();
 for raw in s.lines(){let raw=raw.trim_end_matches('\r');let raw=if format==Format::Yrc{raw.trim_start_matches([' ','\t'])}else{raw};let Some(head)=HEAD.captures(raw)else{if format==Format::Yrc {if let Some(rest)=raw.strip_prefix('['){if let Some((ts,text))=rest.split_once(']'){if let Some(at)=stamp(ts){let text=text.trim_end_matches([' ','\t']);if !text.is_empty(){result.lines.push(Cue{at,text:text.into(),..Default::default()})}}}}}continue};let Some(at)=head[1].parse::<u64>().ok()else{continue};let length=head[2].parse::<u64>().unwrap_or(0);let body=&raw[head.get(0).unwrap().end()..];let re=match format{Format::Krc=>&*KRC,Format::Yrc=>&*YRC,_=>&*QRC};let tags=re.captures_iter(body).collect::<Vec<_>>();let mut words=Vec::new();let mut text=String::new();let mut pos=0;
  for(i,tag)in tags.iter().enumerate(){let m=tag.get(0).unwrap();if format!=Format::Qrc&&m.start()!=pos{break}let start=tag[1].parse::<i64>().unwrap_or(-1);let duration=tag[2].parse::<i64>().unwrap_or(0);let token=if format==Format::Qrc{&body[pos..m.start()]}else{&body[m.end()..tags.get(i+1).map(|t|t.get(0).unwrap().start()).unwrap_or(body.len())]};pos=if format==Format::Qrc{m.end()}else{tags.get(i+1).map(|t|t.get(0).unwrap().start()).unwrap_or(body.len())};if token.is_empty(){continue}let start=if format==Format::Krc{at.saturating_add_signed(start)}else{start.max(0) as u64};text.push_str(token);words.push(Word{at:start,until:start.saturating_add(duration.max(1) as u64),text:token.into()});}
  if words.is_empty()&&format==Format::Krc{let mut cleaned=String::new();let mut rest=body;while !rest.is_empty(){if let Some(after)=rest.strip_prefix('<'){let Some((_,tail))=after.split_once('>')else{break};rest=tail}else{let c=rest.chars().next().unwrap();cleaned.push(c);rest=&rest[c.len_utf8()..]}}text=trim(&cleaned).into();let units=text.encode_utf16().collect::<Vec<_>>();let total=length.max(units.len() as u64*120);for(i,c)in units.iter().enumerate(){let start=at+i as u64*total/units.len() as u64;let end=at+(i+1)as u64*total/units.len() as u64;words.push(Word{at:start,until:end.max(start+1),text:String::from_utf16_lossy(&[*c])});}}
  if format!=Format::Qrc{text=text.trim_end_matches([' ','\t']).into()}if !text.is_empty()&&!words.is_empty(){result.lines.push(Cue{at,text,words,..Default::default()});}
 }
 let mut result=finish(result);
 if format==Format::Krc{use base64::Engine;if let Some(encoded)=s.lines().find_map(|l|l.strip_prefix("[language:").and_then(|v|v.strip_suffix(']'))){if let Ok(bytes)=base64::engine::general_purpose::STANDARD.decode(encoded){if let Ok(v)=serde_json::from_slice::<serde_json::Value>(&bytes){if let Some(groups)=v["content"].as_array(){for g in groups{let kind=g["type"].as_i64().unwrap_or(-1);if kind!=0&&kind!=1{continue}if let Some(rows)=g["lyricContent"].as_array(){for(c,row)in result.lines.iter_mut().zip(rows){let mut text=String::new();if let Some(parts)=row.as_array(){for part in parts.iter().filter_map(|p|p.as_str()){if kind==0&&!text.is_empty()&&!part.is_empty()&&!text.ends_with(' ')&&!part.starts_with(' '){text.push(' ')}text.push_str(part)}}let text=text.trim_matches([' ','\t','\r','\n']).to_owned();if kind==0{c.romanization=Some(text)}else{c.translation=Some(text)}}}}}}}}}
 result
}
pub fn secondary(main:&mut Lyrics,extra:&Lyrics,romanization:bool){if main.lines.is_empty(){return}let mut cursor=0;for e in &extra.lines{while cursor+1<main.lines.len()&&main.lines[cursor+1].at.abs_diff(e.at)<=main.lines[cursor].at.abs_diff(e.at){cursor+=1}if main.lines[cursor].at.abs_diff(e.at)>1500{continue}let target=if romanization{&mut main.lines[cursor].romanization}else{&mut main.lines[cursor].translation};if target.as_deref().unwrap_or("").is_empty(){*target=Some(e.text.clone())}}}

#[cfg(test)]mod tests{use super::*;
 #[test]fn online_lrc_preserves_duplicates_and_ignores_offset_metadata(){let l=lrc("[offset:500]\n[00:03.20][00:08.00] verse \n[00:03.20]translation");assert_eq!(l.lines.len(),3);assert_eq!(l.lines[0].at,3200);assert_eq!(l.lines[1].text,"translation");}
 #[test]fn secondary_uses_upstream_monotonic_nearest_and_empty_only(){let mut l=lrc("[00:01]a\n[00:03]b");secondary(&mut l,&lrc("[00:02]tie\n[00:03]later"),false);assert_eq!(l.lines[1].translation.as_deref(),Some("tie"));assert!(l.lines[0].translation.is_none());}
 #[test]fn yrc_keeps_mixed_lrc_and_absolute_word_times(){let l=word("[1000,300](1000,0,0)词\n[00:03]normal",Format::Yrc);assert_eq!(l.lines.len(),2);assert_eq!(l.lines[0].words[0].until,1001);}
 #[test]fn malformed_qrc_does_not_count_as_a_hit(){assert!(word("[1000,200]text without words",Format::Qrc).lines.is_empty());assert_eq!(word("[1000,200]词(1000,0)",Format::Qrc).lines[0].words[0].until,1001);}
 #[test]fn krc_falls_back_to_character_times(){let l=word("[1000,100]测试",Format::Krc);assert_eq!(l.lines[0].words.len(),2);assert_eq!(l.lines[0].words[1].until,1240);}
}
