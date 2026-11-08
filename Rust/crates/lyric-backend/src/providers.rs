//! DotIsle service adapters. Protocol facts and license provenance are recorded in REWRITE.md.
use lyric_core::{models::*,identity,resolution::Source,cache::Cache};
use serde_json::{Value,json};
use std::{collections::{BTreeMap,HashSet},sync::Arc,time::{Duration,SystemTime,UNIX_EPOCH}};
use base64::{Engine,engine::general_purpose::STANDARD};
use crate::http::{self,RequestContext};
pub fn text(v:&Value,key:&str)->String{v.get(key).map(|v|match v{Value::String(s)=>s.clone(),Value::Number(n)=>n.to_string(),_=>String::new()}).unwrap_or_default()}
fn rows<'a>(v:&'a Value,path:&str)->&'a [Value]{v.pointer(path).and_then(Value::as_array).map(Vec::as_slice).unwrap_or_default()}
fn num(v:&Value,key:&str)->f64{v[key].as_f64().or_else(||v[key].as_str()?.parse().ok()).filter(|n|n.is_finite()).unwrap_or(0.)}
fn names(v:&Value)->Vec<String>{v.as_array().map(|a|a.iter().map(|v|text(v,"name")).filter(|s|!s.is_empty()).collect()).unwrap_or_default()}
fn dur(n:f64)->Duration{Duration::from_secs_f64(n.clamp(0.,86400.))}
pub struct OnlineSource{pub id:Provider,pub context:RequestContext,pub cache:Option<Arc<Cache>>}
impl OnlineSource{
 pub fn new(id:&str,context:RequestContext)->Self{Self{id:Provider::new(id),context,cache:None}}
 fn hit(&self,id:String,title:String,artists:Vec<String>,album:String,seconds:f64,extra:BTreeMap<String,String>)->Hit{Hit{provider:self.id.clone(),id,title,artists,album,duration:dur(seconds),extra}}
 fn get(&self,url:&str)->Result<Value,String>{http::get(url,&self.context)}
 fn netease(&self,path:&str,value:Value)->Result<Value,String>{
  // EAPI wire format adapted from Binaryify's MIT NeteaseCloudMusicApi protocol.
  use aes::{Aes128,cipher::{BlockEncrypt,KeyInit,generic_array::GenericArray}};use md5::{Md5,Digest};
  let serialized=serde_json::to_string(&value).map_err(|e|e.to_string())?;let digest=Md5::digest([b"nobody",path.as_bytes(),b"use",serialized.as_bytes(),b"md5forencrypt"].concat()).iter().map(|b|format!("{b:02x}")).collect::<String>();
  let mut bytes=[path.as_bytes(),b"-36cd479b6b5-",serialized.as_bytes(),b"-36cd479b6b5-",digest.as_bytes()].concat();let padding=16-bytes.len()%16;bytes.resize(bytes.len()+padding,padding as u8);let aes=Aes128::new_from_slice(b"e82ckenh8dichen8").map_err(|e|e.to_string())?;for chunk in bytes.chunks_exact_mut(16){aes.encrypt_block(GenericArray::from_mut_slice(chunk));}
  let wire=format!("params={}",bytes.iter().map(|b|format!("{b:02X}")).collect::<String>());let value=http::post(&format!("https://interface.music.163.com/e{}",path.strip_prefix('/').ok_or("invalid API path")?),&wire,"application/x-www-form-urlencoded","https://music.163.com/",&self.context)?;
  if value["code"].as_i64()!=Some(200){return Err(format!("Netease status {}",value["code"]))}Ok(value)
 }
 fn query(&self,t:&Track,query:&str)->Result<Vec<Hit>,String>{
  let mut hits=Vec::new();let q=http::encode(query);
  match self.id.value(){
   "Netease"=>{let data=self.netease("/api/cloudsearch/pc",json!({"s":query,"type":1,"limit":24,"offset":0,"total":true})).or_else(|_|http::post("https://music.163.com/api/search/get/web",&format!("s={q}&type=1&limit=24&offset=0"),"application/x-www-form-urlencoded","https://music.163.com/",&self.context))?;if data["code"].as_i64()!=Some(200){return Err(format!("Netease search status {}",data["code"]))}for s in rows(&data,"/result/songs"){let album=s.get("al").or_else(||s.get("album")).unwrap_or(&Value::Null);hits.push(self.hit(text(s,"id"),text(s,"name"),names(s.get("ar").or_else(||s.get("artists")).unwrap_or(&Value::Null)),text(album,"name"),num(s,if s.get("dt").is_some(){"dt"}else{"duration"})/1000.,BTreeMap::new()));}}
   "QQMusic"=>{let request=json!({"req":{"module":"music.search.SearchCgiService","method":"DoSearchForQQMusicDesktop","param":{"query":query,"num_per_page":24,"page_num":1,"search_type":0}}});let data=http::post("https://u.y.qq.com/cgi-bin/musicu.fcg",&request.to_string(),"application/json","https://y.qq.com/",&self.context)?;for s in rows(&data,"/req/data/body/song/list"){hits.push(self.hit(text(s,"id"),text(s,"title"),names(&s["singer"]),text(&s["album"],"name"),num(s,"interval"),BTreeMap::from([("mid".into(),text(s,"mid"))])));}}
   "LRCLIB"=>{let data=self.get(&format!("https://lrclib.net/api/search?track_name={}&artist_name={}",http::encode(&t.title),http::encode(&t.artist)))?;for s in data.as_array().map(Vec::as_slice).unwrap_or_default(){hits.push(self.hit(text(s,"id"),text(s,"trackName"),identity::artists(&text(s,"artistName")),text(s,"albumName"),num(s,"duration"),BTreeMap::new()));}}
   "Kugou"=>{let data=self.get(&format!("https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={q}&duration={}",t.duration.as_millis()))?;hits.extend(self.kugou_hits(&data));if hits.is_empty(){let songs=self.get(&format!("https://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={q}&page=1&pagesize=4"))?;for song in rows(&songs,"/data/info").iter().take(3){self.context.check()?;let hash=text(song,"hash");if hash.is_empty(){continue}if let Ok(data)=self.get(&format!("https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&hash={}",http::encode(&hash))){hits.extend(self.kugou_hits(&data));}}}}
   "Kuwo"=>{let bytes=http::request(&format!("https://search.kuwo.cn/r.s?all={q}&pn=0&rn=24&ft=music&encoding=utf8&rformat=json&client=kt&vipver=1&show_copyright_off=1"),"GET",&[],"","https://www.kuwo.cn/",&self.context,8*1024*1024)?;let data=json_strings(&bytes)?;for s in rows(&data,"/abslist"){let title=entities(&text(s,"SONGNAME"));let title=title.split_once("-《").filter(|(_,tail)|["插曲","主题曲","片尾曲","片头曲"].iter().any(|w|tail.contains(w))).map(|(title,_)|title.to_owned()).unwrap_or(title);hits.push(self.hit(text(s,"MUSICRID").trim_start_matches("MUSIC_").into(),title,identity::artists(&entities(&text(s,"ARTIST"))),entities(&text(s,"ALBUM")),num(s,"DURATION"),BTreeMap::new()));}}
   "Migu"=>{use md5::{Md5,Digest};let time=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().to_string();let device="963B7AA0D21511ED807EE5846EC87D20";let message=format!("{query}6cdc72a439cef99a3418d2a78aa28c73yyapp2d16148780a1dcc7408e06336b98cfd50{device}{time}");let sign=Md5::digest(message).iter().map(|b|format!("{b:02x}")).collect::<String>();let data=http::get_headers(&format!("https://jadeite.migu.cn/music_search/v3/search/searchAll?isCorrect=0&isCopyright=1&searchSwitch=%7B%22song%22%3A1%7D&pageSize=24&text={q}&pageNo=1&sort=0&sid=USS"),&[("uiVersion","A_music_3.6.1"),("deviceId",device),("timestamp",&time),("sign",&sign),("channel","0146921")],&self.context)?;if data["code"]!="000000"{return Err(format!("Migu status {}",data["code"]))}for group in rows(&data,"/songResultData/resultList"){for s in group.as_array().map(Vec::as_slice).unwrap_or_default(){let artists=rows(s,"/singerList").iter().map(|v|{let n=text(v,"name");if n.is_empty(){text(v,"singerName")}else{n}}).collect();hits.push(self.hit(text(s,"copyrightId"),text(s,"name"),artists,text(s,"album"),num(s,"duration"),BTreeMap::from([("lyric".into(),text(s,"lrcUrl")),("translation".into(),text(s,"trcUrl"))])));}}}
   _=>return Err("unknown lyric provider".into())
  }hits.retain(|h|!h.id.is_empty()&&!h.title.trim().is_empty());Ok(hits)
 }
 fn kugou_hits(&self,data:&Value)->Vec<Hit>{rows(data,"/candidates").iter().filter(|s|!text(s,"accesskey").is_empty()).map(|s|self.hit(text(s,"id"),text(s,"song"),identity::artists(&text(s,"singer")),String::new(),num(s,"duration")/1000.,BTreeMap::from([("key".into(),text(s,"accesskey"))]))).collect()}
 fn download(&self,h:&Hit)->Result<Option<Payload>,String>{
  let mut p=Payload{format:Format::Lrc,original:String::new(),translation:String::new(),instrumental:false};
  match self.id.value(){
   "Netease"=>{let data=self.netease("/api/song/lyric/v1",json!({"id":h.id,"cp":false,"tv":0,"lv":0,"rv":0,"kv":0,"yv":0,"ytv":0,"yrv":0})).or_else(|_|self.get(&format!("https://music.163.com/api/song/lyric?id={}&lv=-1&kv=-1&tv=-1",http::encode(&h.id))))?;if data["code"].as_i64()!=Some(200){return Err(format!("Netease lyric status {}",data["code"]))}p.original=text(&data["yrc"],"lyric");if p.original.is_empty(){p.original=text(&data["lrc"],"lyric");p.translation=text(&data["tlyric"],"lyric");}else{p.format=Format::Yrc;p.translation=text(&data["ytlrc"],"lyric");if p.translation.is_empty(){p.translation=text(&data["tlyric"],"lyric")}}}
   "QQMusic"=>{
    // Word-timed download is optional; the ordinary LRC endpoint remains a fallback.
    if h.id.bytes().all(|b|b.is_ascii_digit()){
     let body=format!("version=15&miniversion=82&lrctype=4&musicid={}",http::encode(&h.id));if let Ok(bytes)=http::request("https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg","POST",body.as_bytes(),"application/x-www-form-urlencoded","https://y.qq.com/",&self.context,8*1024*1024){
      if let Ok(xml)=String::from_utf8(bytes){p.original=lyric_core::crypto::qrc_text(qq_content(&xml,"content")).unwrap_or_default();p.translation=lyric_core::crypto::qrc_text(qq_content(&xml,"contentts")).unwrap_or_default();if !p.original.is_empty(){p.format=Format::Qrc;}}
     }
    }
    if p.original.is_empty(){let Some(mid)=h.extra.get("mid").filter(|s|!s.is_empty())else{return Ok(None)};let url=format!("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg?songmid={}&format=json&g_tk=5381&loginUin=0&hostUin=0&inCharset=utf8&outCharset=utf8&platform=yqq&needNewCode=0",http::encode(mid));let bytes=http::request(&url,"GET",&[],"","https://y.qq.com/",&self.context,8*1024*1024)?;let data:Value=serde_json::from_slice(&bytes).map_err(|e|e.to_string())?;let decode=|s:&str|STANDARD.decode(s).ok().and_then(|b|String::from_utf8(b).ok()).unwrap_or_default();p.original=decode(&text(&data,"lyric"));p.translation=decode(&text(&data,"trans"));}
   }
   "Kugou"=>{let key=h.extra.get("key").ok_or("missing lyric token")?;let data=self.get(&format!("https://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=krc&charset=utf8",http::encode(&h.id),http::encode(key)))?;p.format=Format::Krc;p.original=text(&data,"content");}
   "LRCLIB"=>{if h.id.parse::<u64>().is_err(){return Err("invalid LRCLIB ID".into())}let data=self.get(&format!("https://lrclib.net/api/get/{}",h.id))?;p.original=text(&data,"syncedLyrics");if p.original.is_empty(){p.format=Format::Text;p.original=text(&data,"plainLyrics");}p.instrumental=data["instrumental"].as_bool().unwrap_or(false);}
   "Kuwo"=>{if h.id.parse::<u64>().is_err(){return Err("invalid Kuwo ID".into())}let data=self.get(&format!("https://m.kuwo.cn/newh5/singles/songinfoandlrc?musicId={}",h.id))?;p.original=rows(&data,"/data/lrclist").iter().filter_map(|v|{let sec=num(v,"time");if !(0.0..=86400.).contains(&sec){return None}let ms=(sec*1000.).round()as u64;Some(format!("[{:02}:{:02}.{:03}]{}",ms/60000,ms/1000%60,ms%1000,entities(&text(v,"lineLyric"))))}).collect::<Vec<_>>().join("\n");}
   "Migu"=>{p.original=self.migu_text(h.extra.get("lyric"))?;p.translation=self.migu_text(h.extra.get("translation")).unwrap_or_default();}
   _=>return Err("unknown lyric provider".into())
  }Ok((p.instrumental||!p.original.trim().is_empty()).then_some(p))
 }
 fn migu_text(&self,url:Option<&String>)->Result<String,String>{let Some(url)=url.filter(|s|!s.is_empty())else{return Ok(String::new())};let rest=url.strip_prefix("https://").or_else(||url.strip_prefix("http://")).ok_or("invalid lyric URL")?;let host=rest.split('/').next().unwrap_or("").to_ascii_lowercase();if !["migu.cn","migumusic.cn"].iter().any(|domain|host==*domain||host.ends_with(&format!(".{domain}"))){return Err("untrusted lyric host".into())}String::from_utf8(http::request(&format!("https://{rest}"),"GET",&[],"","https://app.c.nf.migu.cn/",&self.context,2*1024*1024)?).map_err(|e|e.to_string())}
}
impl Source for OnlineSource{
 fn search(&self,t:&Track)->Result<Vec<Hit>,String>{
  self.context.check()?;if identity::official(&t.source_app).as_ref()==Some(&self.id)&&matches!(self.id.value(),"Netease"|"QQMusic"|"Kuwo"){if let Some(id)=t.song_id.as_ref().filter(|s|!s.is_empty()&&s.bytes().all(|b|b.is_ascii_digit())){return Ok(vec![self.hit(id.clone(),t.title.clone(),identity::artists(&t.artist),t.album.clone(),t.duration.as_secs_f64(),BTreeMap::new())])}}
  let key=serde_json::to_string(&("search-3",self.id.value(),&t.title,&t.artist,t.duration.as_secs())).unwrap_or_default();let query=if t.artist.trim().is_empty(){t.title.clone()}else{format!("{} {}",t.title,t.artist)};
  let mut hits=self.query(t,&query)?;if hits.is_empty()&&query!=t.title{hits=self.query(t,&t.title)?;}let mut seen=HashSet::new();hits.retain(|h|seen.insert(h.id.clone()));let _=key;Ok(hits)
 }
 fn fetch(&self,h:&Hit)->Result<Option<Payload>,String>{self.context.check()?;let key=serde_json::to_string(&("payload-3",self.id.value(),&h.id)).unwrap_or_default();if let Some(value)=self.cache.as_ref().and_then(|c|c.get(&key)){return Ok(Some(value))}let p=self.download(h)?;if let(Some(cache),Some(payload))=(&self.cache,&p){cache.put(&key,payload)}Ok(p)}
}
fn entities(s:&str)->String{let xml=format!("<v>{}</v>",s.replace("&nbsp;"," "));if let Ok(doc)=roxmltree::Document::parse(&xml){doc.root_element().text().unwrap_or("").into()}else{s.into()}}
// This endpoint wraps content in an XML-like comment containing malformed sibling tags.
// Read only the named bounded field; the decrypted QRC itself uses a real XML parser.
fn qq_content<'a>(response:&'a str,name:&str)->&'a str{
 let start_tag=format!("<{name}");let mut remaining=response;
 while let Some(at)=remaining.find(&start_tag){remaining=&remaining[at+start_tag.len()..];if !remaining.starts_with(|c:char|c=='>'||c.is_ascii_whitespace()){continue}let Some(at)=remaining.find('>')else{return ""};let body=&remaining[at+1..];let Some(end)=body.find(&format!("</{name}>"))else{return ""};return body[..end].trim().trim_start_matches("<![CDATA[").trim_end_matches("]]>").trim();} ""
}
fn json_strings(bytes:&[u8])->Result<Value,String>{if let Ok(v)=serde_json::from_slice(bytes){return Ok(v)}let text=std::str::from_utf8(bytes).map_err(|e|e.to_string())?;let mut iter=text.chars();let mut result=String::new();while let Some(c)=iter.next(){if c!='\''&&c!='"'{result.push(c);continue}let quote=c;let mut token=String::new();let mut closed=false;while let Some(c)=iter.next(){if c==quote{closed=true;break}if c=='\\'{let c=iter.next().ok_or("truncated JSON escape")?;match c{'\''|'"'|'\\'=>token.push(c),'n'=>token.push('\n'),'r'=>token.push('\r'),'t'=>token.push('\t'),'u'=>{let digits=iter.by_ref().take(4).collect::<String>();let n=u32::from_str_radix(&digits,16).map_err(|e|e.to_string())?;token.push(char::from_u32(n).ok_or("invalid JSON unicode")?);},_=>return Err("unknown JSON escape".into())}}else{token.push(c)}}if !closed{return Err("unclosed JSON string".into())}result.push_str(&serde_json::to_string(&token).map_err(|e|e.to_string())?);}serde_json::from_str(&result).map_err(|e|e.to_string())}
#[cfg(test)]mod tests{use super::*;
 #[test]fn literal_parser_preserves_apostrophes_and_escapes(){let data=json_strings(br#"{'song': 'It\'s \"okay\"', 'singer': '\u7532'}"#).unwrap();assert_eq!(data["song"],"It's \"okay\"");assert_eq!(data["singer"],"甲");assert!(json_strings(b"{'a':'bad}").is_err());}
 #[test]fn kugou_hits_keep_the_download_token_and_duration(){let s=OnlineSource::new("Kugou",RequestContext::new(Duration::from_secs(1)));let h=s.kugou_hits(&json!({"candidates":[{"id":71,"song":"晴空","singer":"甲/乙","duration":123400,"accesskey":"token"},{"id":72}]}));assert_eq!(h.len(),1);assert_eq!(h[0].duration.as_millis(),123400);assert_eq!(h[0].extra["key"],"token");assert_eq!(h[0].artists,["甲","乙"]);}
 #[test]fn qq_comment_envelope_does_not_require_well_formed_siblings(){let s="<!--<cmd><miniversion=\"1\"/><contentts><![CDATA[AA]]></contentts><content type=\"1\"><![CDATA[BB]]></content></cmd>-->";assert_eq!(qq_content(s,"content"),"BB");assert_eq!(qq_content(s,"contentts"),"AA");assert_eq!(qq_content(s,"none"),"");}
}
