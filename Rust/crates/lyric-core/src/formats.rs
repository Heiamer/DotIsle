//! Parsers produce one millisecond timeline; no intermediate compatibility document.
use crate::models::*;
use regex::Regex;
use std::sync::LazyLock;
static LRC:LazyLock<Regex>=LazyLock::new(||Regex::new(r"\[(\d{1,3}):(\d{2}(?:[.:]\d{1,3})?)\]").unwrap());
static HEAD:LazyLock<Regex>=LazyLock::new(||Regex::new(r"^\[(\d+),(\d+)\]").unwrap());
static ANGLE:LazyLock<Regex>=LazyLock::new(||Regex::new(r"<(\d{1,3}:\d{2}(?:\.\d{1,3})?)>").unwrap());
const LIMIT:usize=8*1024*1024;
pub fn time(s:&str)->Option<u64>{
 let s=s.trim();let factor=if s.ends_with("ms"){1.}else if s.ends_with('s'){1000.}else if s.ends_with('m'){60000.}else if s.ends_with('h'){3600000.}else{0.};
 let n=if factor>0.{s.trim_end_matches(|c:char|c.is_ascii_alphabetic()).parse::<f64>().ok()?*factor}else{let mut n=0.;for p in s.split(':'){n=n*60.+p.parse::<f64>().ok()?;}n*1000.};
 (n.is_finite()&&(0.0..=86_400_000.).contains(&n)).then_some(n.round()as u64)
}
fn number(s:&str)->Option<u64>{s.parse().ok().filter(|v|*v<=86_400_000)}
fn adjusted(t:u64,offset:i64)->u64{t.saturating_add_signed(offset)}
pub fn parse(payload:&Payload)->Result<Lyrics,String>{
 if payload.instrumental{return Ok(Lyrics::instrumental())}if payload.original.len()>LIMIT{return Err("lyric exceeds size limit".into())}
 let plain=match payload.format{Format::Krc=>crate::crypto::krc_text(payload.original.as_bytes())?,Format::Qrc=>{if payload.original.trim().bytes().all(|b|b.is_ascii_hexdigit())&&!payload.original.trim().is_empty(){crate::crypto::qrc_text(payload.original.trim())?}else{payload.original.clone()}},_=>payload.original.clone()};
 let plain=if payload.format==Format::Qrc&&plain.contains("LyricContent="){qrc_body(&plain)?}else{plain};
 let mut lyrics=match payload.format{Format::Lrc=>lrc(&plain),Format::Qrc if !plain.lines().any(|l|HEAD.is_match(l))=>lrc(&plain),Format::Qrc|Format::Yrc|Format::Krc=>word_lines(&plain,payload.format),Format::Ttml=>ttml(&plain)?,Format::Text=>Lyrics{lines:plain.lines().filter(|l|!l.trim().is_empty()).map(|l|Cue{text:l.to_owned(),..Default::default()}).collect(),timed:false,instrumental:false}};
 lyrics.finalize();if !payload.translation.trim().is_empty(){let translated=if payload.translation.contains("LyricContent="){qrc_body(&payload.translation).unwrap_or_default()}else{payload.translation.clone()};let mut translated=if translated.lines().any(|l|HEAD.is_match(l)){word_lines(&translated,Format::Qrc)}else{lrc(&translated)};translated.finalize();merge_translations(&mut lyrics,&translated)}
 if payload.format==Format::Krc{if let Some(language)=plain.lines().find_map(|l|l.strip_prefix("[language:").and_then(|s|s.strip_suffix(']'))){krc_translations(&mut lyrics,language);}}
 if lyrics.lines.len()<=2&&lyrics.lines.iter().any(|l|l.text.contains("纯音乐")||l.text.to_lowercase().contains("instrumental")){lyrics.instrumental=true;}
 Ok(lyrics)
}
fn qrc_body(xml:&str)->Result<String,String>{
 // QQ stores literal newlines in an attribute; XML would otherwise normalize them to spaces.
 let mut escaped=String::with_capacity(xml.len());let mut quote=None;
 for c in xml.chars(){if c=='"'||c=='\''{if quote==Some(c){quote=None}else if quote.is_none(){quote=Some(c)}}if quote.is_some()&&c=='\n'{escaped.push_str("&#10;")}else if quote.is_some()&&c=='\r'{escaped.push_str("&#13;")}else{escaped.push(c)}}
 let doc=roxmltree::Document::parse(&escaped).map_err(|e|e.to_string())?;doc.descendants().find_map(|n|n.attribute("LyricContent")).map(str::to_owned).ok_or("QRC content missing".into())
}
pub fn lrc(input:&str)->Lyrics{
 let offset=input.lines().find_map(|l|l.trim().strip_prefix("[offset:").and_then(|s|s.strip_suffix(']')).and_then(|s|s.parse::<i64>().ok())).unwrap_or(0).clamp(-86_400_000,86_400_000);
 let mut lines=Vec::new();
 for row in input.trim_start_matches('\u{feff}').lines().take(50_000){
  let stamps=LRC.captures_iter(row).collect::<Vec<_>>();let Some(last)=stamps.last()else{continue};let body=row[last.get(0).unwrap().end()..].trim();
  for stamp in &stamps{let Some(at)=time(&format!("{}:{}",&stamp[1],stamp[2].replacen('.',".",1).replacen(':',".",1)))else{continue};let at=adjusted(at,offset);let tags=ANGLE.captures_iter(body).collect::<Vec<_>>();let mut words=Vec::new();
   for(i,tag)in tags.iter().enumerate(){let text_end=tags.get(i+1).map(|t|t.get(0).unwrap().start()).unwrap_or(body.len());let text=body[tag.get(0).unwrap().end()..text_end].to_owned();let Some(start)=time(&tag[1])else{continue};let end=tags.get(i+1).and_then(|t|time(&t[1])).unwrap_or(start+500);if !text.is_empty(){words.push(Word{at:adjusted(start,offset),until:adjusted(end,offset),text});}}
   let text=ANGLE.replace_all(body,"").to_string();if let Some(old)=lines.iter_mut().find(|l:&&mut Cue|l.at==at){if !text.is_empty()&&old.text!=text{old.translation=Some(text)}}else{lines.push(Cue{at,text,words,..Default::default()});}
  }
 }
 let mut result=Lyrics{lines,timed:true,instrumental:false};result.finalize();result
}
fn word_lines(input:&str,format:Format)->Lyrics{
 let pattern=match format{Format::Krc=>r"<(\d+),(\d+),\d+>",Format::Yrc=>r"\((\d+),(\d+),\d+\)",_=>r"\((\d+),(\d+)\)"};let word=Regex::new(pattern).unwrap();let mut lines=Vec::new();
 for row in input.lines().take(50_000){let Some(head)=HEAD.captures(row)else{continue};let Some(at)=number(&head[1])else{continue};let Some(length)=number(&head[2])else{continue};let body=&row[head.get(0).unwrap().end()..];let tags=word.captures_iter(body).collect::<Vec<_>>();let mut words=Vec::new();
  for(i,tag)in tags.iter().enumerate(){let Some(start)=number(&tag[1])else{continue};let Some(length)=number(&tag[2])else{continue};let (text,start)=if format==Format::Qrc{let from=if i==0{0}else{tags[i-1].get(0).unwrap().end()};(body[from..tag.get(0).unwrap().start()].to_string(),start)}else{let to=tags.get(i+1).map(|c|c.get(0).unwrap().start()).unwrap_or(body.len());(body[tag.get(0).unwrap().end()..to].to_string(),if format==Format::Krc{at.saturating_add(start)}else{start})};words.push(Word{at:start,until:start.saturating_add(length),text});
  }
  let text=word.replace_all(body,"").to_string();lines.push(Cue{at,until:at.saturating_add(length),text,words,translation:None});
 }
 Lyrics{lines,timed:true,instrumental:false}
}
fn merge_translations(target:&mut Lyrics,translated:&Lyrics){for line in &mut target.lines{if let Some(t)=translated.lines.iter().min_by_key(|t|t.at.abs_diff(line.at)).filter(|t|t.at.abs_diff(line.at)<=200){if !t.text.trim().is_empty()&&t.text!=line.text{line.translation=Some(t.text.clone());}}}}
fn krc_translations(target:&mut Lyrics,encoded:&str){use base64::Engine;let Ok(bytes)=base64::engine::general_purpose::STANDARD.decode(encoded)else{return};let Ok(data)=serde_json::from_slice::<serde_json::Value>(&bytes)else{return};if let Some(rows)=data["content"].as_array(){for group in rows{if group["type"].as_i64()!=Some(1){continue}if let Some(rows)=group["lyricContent"].as_array(){for(line,row)in target.lines.iter_mut().zip(rows){let text=row.as_array().map(|a|a.iter().filter_map(|v|v.as_str()).collect::<String>()).unwrap_or_default();if !text.is_empty(){line.translation=Some(text)}}}}}}
fn inherited(n:roxmltree::Node,key:&str)->Option<u64>{n.ancestors().find_map(|n|n.attribute(key).and_then(time))}
fn xml_text(n:roxmltree::Node)->String{n.descendants().filter(|d|d.is_text()).filter_map(|d|d.text()).collect::<String>()}
fn ttml(input:&str)->Result<Lyrics,String>{
 let doc=roxmltree::Document::parse(input).map_err(|e|e.to_string())?;let mut result=Lyrics{timed:true,..Default::default()};
 for p in doc.descendants().filter(|n|n.is_element()&&n.tag_name().name()=="p").take(50_000){let Some(at)=inherited(p,"begin")else{continue};let until=p.attribute("end").and_then(time).or_else(||p.attribute("dur").and_then(time).map(|d|at.saturating_add(d))).or_else(||inherited(p,"end")).unwrap_or(at+6000);let mut cue=Cue{at,until,..Default::default()};
  for node in p.children(){if node.is_text(){cue.text.push_str(node.text().unwrap_or(""));continue}if !node.is_element(){continue}if node.tag_name().name()=="br"{cue.text.push('\n');continue}let role=node.attributes().find(|a|a.name()=="role").map(|a|a.value()).unwrap_or("");if role.contains("translation"){cue.translation=Some(xml_text(node));continue}if role.contains("romanization"){continue}let text=xml_text(node);cue.text.push_str(&text);if let Some(start)=node.attribute("begin").and_then(time){let end=node.attribute("end").and_then(time).unwrap_or(until);cue.words.push(Word{at:start,until:end,text});}}
  cue.text=cue.text.trim().to_owned();result.lines.push(cue);
 }
 Ok(result)
}
#[cfg(test)]mod tests{use super::*;
 fn p(format:Format,s:&str)->Lyrics{parse(&Payload{format,original:s.into(),translation:String::new(),instrumental:false}).unwrap()}
 #[test]fn repeated_timestamps_offsets_and_translation(){let mut l=p(Format::Lrc,"[offset:-125]\n[00:03.750][00:09.100]星光\n[00:03.750]Starlight\n[00:14.000]终点");assert_eq!(l.lines.len(),3);assert_eq!(l.lines[0].at,3625);assert_eq!(l.lines[0].translation.as_deref(),Some("Starlight"));assert_eq!(l.lines[0].until,8975);l.finalize();}
 #[test]fn formats_keep_real_word_timing(){for(format,s)in [(Format::Yrc,"[4250,2100](4250,700,0)夜(4950,1400,0)航"),(Format::Krc,"[4250,2100]<0,700,0>夜<700,1400,0>航"),(Format::Qrc,"[4250,2100]夜(4250,700)航(4950,1400)")]{let l=p(format,s);assert_eq!(l.lines[0].text,"夜航");assert_eq!(l.lines[0].words[1].at,4950);assert_eq!(l.lines[0].words[1].until,6350);}}
 #[test]fn xml_entities_namespaces_and_translated_spans(){let l=p(Format::Ttml,r#"<tt xmlns="http://www.w3.org/ns/ttml" xmlns:t="http://www.w3.org/ns/ttml#metadata"><body><div><p begin="2.4s" end="5s"><span begin="2.4s" end="3.6s">A&amp;B</span><span t:role="x-translation">甲与乙</span></p></div></body></tt>"#);assert_eq!(l.lines[0].text,"A&B");assert_eq!(l.lines[0].translation.as_deref(),Some("甲与乙"));assert_eq!(l.lines[0].words[0].until,3600);}
 #[test]fn qrc_xml_envelope_is_decoded(){let l=p(Format::Qrc,r#"<QrcInfos><Lyric_1 LyricContent="[2400,900]岸(2400,900)&#10;"/></QrcInfos>"#);assert_eq!(l.lines[0].text,"岸");}
 #[test]fn qrc_literal_newlines_survive_xml_attribute_rules(){let l=p(Format::Qrc,"<QrcInfos><Lyric_1 LyricContent=\"[ti:晨雾]\n[2400,900]岸(2400,900)\n[5000,800]灯(5000,800)\"/></QrcInfos>");assert_eq!(l.lines.len(),2);assert_eq!(l.lines[1].at,5000);}
 #[test]fn enhanced_lrc_preserves_absolute_boundaries(){let l=p(Format::Lrc,"[00:07.20]<00:07.20>轻<00:07.85>风<00:08.60>\n[00:10.00]下一句");assert_eq!(l.lines[0].words[1].until,8600);assert_eq!(l.lines[0].text,"轻风");}
 #[test]fn malformed_or_oversized_times_do_not_panic(){assert!(p(Format::Lrc,"[99999999:23.2]x\n[offset:no]").lines.is_empty());assert!(p(Format::Yrc,"[23,10](bad)x").lines[0].words.is_empty());assert_eq!(time("01:02:03.004"),Some(3723004));assert_eq!(time("-2s"),None);assert_eq!(time("NaNs"),None);}
}
