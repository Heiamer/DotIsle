//! Native ports of the service protocols used by Lyricify Lyrics Helper (Apache-2.0).
//! Reimplemented in Rust; request construction, transport and decryption are native.
use std::{sync::mpsc,collections::{BTreeMap,HashSet},time::{Duration,SystemTime,UNIX_EPOCH}};
use aes::{Aes128,cipher::{BlockEncrypt,KeyInit,generic_array::GenericArray}};
use md5::{Md5,Digest};
use base64::{Engine,engine::general_purpose::STANDARD};
use serde_json::{Value,json};
use lyric_core::{models::*, resolution::LyricSource, policy::{SourceFailure,ProviderSongIdPolicy},crypto};
use crate::http::{self,RequestContext};

#[derive(Clone)]pub struct OnlineSource { pub id:LyricProviderId, pub context:RequestContext }
pub fn text(v:&Value,key:&str)->String{match &v[key]{Value::String(s)=>s.clone(),Value::Number(n)=>n.to_string(),_=>String::new()}}
fn artists(v:&Value)->Vec<String>{v.as_array().map(|a|a.iter().map(|i|text(i,"name")).filter(|s|!s.is_empty()).collect()).unwrap_or_default()}
fn items<'a>(v:&'a Value,pointer:&str)->&'a [Value]{v.pointer(pointer).and_then(Value::as_array).map(Vec::as_slice).unwrap_or(&[])}
fn failure(e:String)->SourceFailure{if e=="cancelled"{SourceFailure::Other(e.clone())}else if e=="timeout"{SourceFailure::Timeout}else{SourceFailure::Connection}}

pub fn eapi_body(path:&str,data:Value)->String{
    let body=data.to_string();let digest=Md5::digest(format!("nobody{path}use{body}md5forencrypt"));
    let digest=digest.iter().map(|b|format!("{b:02x}")).collect::<String>();
    let mut message=format!("{path}-36cd479b6b5-{body}-36cd479b6b5-{digest}").into_bytes();
    let pad=16-message.len()%16;message.extend(std::iter::repeat_n(pad as u8,pad));
    let cipher=Aes128::new(GenericArray::from_slice(b"e82ckenh8dichen8"));
    for block in message.chunks_exact_mut(16){cipher.encrypt_block(GenericArray::from_mut_slice(block));}
    format!("params={}",message.iter().map(|b|format!("{b:02X}")).collect::<String>())
}
fn eapi(path:&str,mut data:Value,ctx:&RequestContext)->Result<Value,String>{
    let now=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    data["header"]=Value::String(json!({"__csrf":"","appver":"8.0.0","buildver":now.as_secs().to_string(),"channel":"","deviceId":"","mobilename":"","resolution":"1920x1080","os":"android","osver":"","requestId":format!("{}_0001",now.as_millis()),"versioncode":"140","MUSIC_U":""}).to_string());
    let response=http::post(&format!("https://interface.music.163.com/e{}",&path[1..]),&eapi_body(path,data),"application/x-www-form-urlencoded","https://music.163.com/",ctx)?;if response["code"].as_i64()!=Some(200){return Err(format!("Netease service status {}",response["code"]))}Ok(response)
}
impl OnlineSource{
    pub fn new(id:&str,context:RequestContext)->Self{Self{id:LyricProviderId::try_new(id).unwrap(),context}}
    fn candidate(&self,id:String,title:String,artists:Vec<String>,album:String,duration:f64,variant:&str,metadata:BTreeMap<String,String>)->SourceTrackCandidate{
        SourceTrackCandidate{provider_id:self.id.clone(),candidate_id:id,title,artists,album,duration:Duration::from_secs_f64(duration.max(0.).min(86400.)),query_variant_id:variant.into(),fetch_metadata:metadata}
    }
    fn variant(&self,v:&SearchQueryVariant)->Result<Vec<SourceTrackCandidate>,String>{
        self.context.check()?;let query=std::iter::once(v.title.as_str()).chain(v.artists.iter().map(String::as_str)).collect::<Vec<_>>().join(" ");
        let mut out=vec![];
        match self.id.value(){
            "Netease"=>{
                let response=eapi("/api/cloudsearch/pc",json!({"s":query,"type":"1","limit":"30","offset":"0","total":"true"}),&self.context)?;
                for s in items(&response,"/result/songs"){
                    out.push(self.candidate(text(s,"id"),text(s,"name"),artists(s.get("ar").unwrap_or(&s["artists"])),text(s.get("al").unwrap_or(&s["album"]),"name"),s.get("dt").unwrap_or(&s["duration"]).as_f64().unwrap_or(0.)/1000.,&v.id,BTreeMap::new()));
                }
            }
            "QQMusic"=>{
                let data=json!({"req_1":{"method":"DoSearchForQQMusicDesktop","module":"music.search.SearchCgiService","param":{"num_per_page":"20","page_num":"1","query":query,"search_type":0}}});
                let response=http::post("https://u.y.qq.com/cgi-bin/musicu.fcg",&data.to_string(),"application/json","https://y.qq.com/",&self.context)?;
                for s in items(&response,"/req_1/data/body/song/list"){
                    let title=if text(s,"title").is_empty(){text(s,"name")}else{text(s,"title")};
                    out.push(self.candidate(text(s,"id"),title,artists(&s["singer"]),text(&s["album"],"name"),s["interval"].as_f64().unwrap_or(0.),&v.id,BTreeMap::from([("mid".into(),text(s,"mid"))])));
                }
            }
            "LRCLIB"=>{
                let response=http::get(&format!("https://lrclib.net/api/search?track_name={}&artist_name={}&album_name={}",http::encode(&v.title),http::encode(&v.artists.join(", ")),http::encode(&v.album)),&self.context)?;
                for s in response.as_array().map(Vec::as_slice).unwrap_or(&[]){out.push(self.candidate(text(s,"id"),text(s,"trackName"),vec![text(s,"artistName")],text(s,"albumName"),s["duration"].as_f64().unwrap_or(0.),&v.id,BTreeMap::new()));}
            }
            "Kugou"=>{
                if let Ok(response)=http::get(&format!("https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&keyword={}&duration={}",http::encode(&query),v.duration.as_millis()),&self.context){out=self.parse_kugou(&response,"",&v.title,&v.artists.join(" / "),&v.album,&v.id);}
                if out.is_empty(){
                    let response=http::get(&format!("https://mobilecdn.kugou.com/api/v3/search/song?format=json&keyword={}&page=1&pagesize=8&showtype=1",http::encode(&query)),&self.context)?;
                    let(tx,rx)=mpsc::channel();
                    for song in items(&response,"/data/info").iter().take(4){let source=self.clone();let song=song.clone();let variant=v.id.clone();let tx=tx.clone();std::thread::spawn(move||{let _=tx.send(source.kugou_candidates(&text(&song,"hash"),&text(&song,"songname"),&text(&song,"singername"),&text(&song,"album_name"),&variant));});}
                    drop(tx);for result in rx{if let Ok(found)=result{out.extend(found)}}
                }
            }
            "Kuwo"=>{
                let bytes=http::request(&format!("https://search.kuwo.cn/r.s?client=kt&all={}&pn=0&rn=20&ft=music&encoding=utf8&rformat=json&vipver=1&show_copyright_off=1&newver=1",http::encode(&query)),"GET",&[],"","https://www.kuwo.cn/",&self.context,8*1024*1024)?;let response=parse_kuwo_search(&bytes)?;
                for song in items(&response,"/abslist"){out.push(self.candidate(text(song,"MUSICRID").trim_start_matches("MUSIC_").into(),clean_kuwo_title(&decode_entities(&text(song,"SONGNAME"))),split_artists(&decode_entities(&text(song,"ARTIST"))),decode_entities(&text(song,"ALBUM")),number(song,"DURATION"),&v.id,BTreeMap::new()));}
            }
            "Migu"=>{
                let time=SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis().to_string();let device="963B7AA0D21511ED807EE5846EC87D20";
                let sign=Md5::digest(format!("{query}6cdc72a439cef99a3418d2a78aa28c73yyapp2d16148780a1dcc7408e06336b98cfd50{device}{time}")).iter().map(|b|format!("{b:02x}")).collect::<String>();
                let response=http::get_headers(&format!("https://jadeite.migu.cn/music_search/v3/search/searchAll?isCorrect=0&isCopyright=1&searchSwitch=%7B%22song%22%3A1%7D&pageSize=20&text={}&pageNo=1&sort=0&sid=USS",http::encode(&query)),&[("uiVersion","A_music_3.6.1"),("deviceId",device),("timestamp",&time),("sign",&sign),("channel","0146921")],&self.context)?;
                if response["code"]!="000000"{return Err(format!("Migu service status {}",response["code"]))}
                for group in items(&response,"/songResultData/resultList"){for song in group.as_array().map(Vec::as_slice).unwrap_or(&[]){let id=text(song,"copyrightId");if id.is_empty(){continue}out.push(self.candidate(id,text(song,"name"),song["singerList"].as_array().map(|a|a.iter().map(|v|{let name=text(v,"name");if name.is_empty(){text(v,"singerName")}else{name}}).collect()).unwrap_or_default(),text(song,"album"),number(song,"duration"),&v.id,BTreeMap::from([("lrc-url".into(),text(song,"lrcUrl")),("translation-url".into(),text(song,"trcUrl"))])));}}
            }
            _=>{}
        }
        out.retain(|c|!c.candidate_id.is_empty());Ok(out)
    }
    fn kugou_candidates(&self,hash:&str,title:&str,artist:&str,album:&str,variant:&str)->Result<Vec<SourceTrackCandidate>,String>{
        let url=format!("https://lyrics.kugou.com/search?ver=1&man=yes&client=pc&hash={}",http::encode(hash));
        let mut response=http::get(&url,&self.context)?;
        if items(&response,"/candidates").is_empty(){self.context.check()?;response=http::get(&url,&self.context)?;}
        Ok(self.parse_kugou(&response,hash,title,artist,album,variant))
    }
    fn parse_kugou(&self,response:&Value,hash:&str,title:&str,artist:&str,album:&str,variant:&str)->Vec<SourceTrackCandidate>{
        items(response,"/candidates").iter().filter(|s|!text(s,"id").is_empty()&&!text(s,"accesskey").is_empty()).map(|s|self.candidate(text(s,"id"),if text(s,"song").is_empty(){title.into()}else{text(s,"song")},split_artists(if text(s,"singer").is_empty(){artist}else{s["singer"].as_str().unwrap_or(artist)}),album.into(),number(s,"duration")/1000.,variant,BTreeMap::from([("access-key".into(),text(s,"accesskey")),("song-hash".into(),hash.into())]))).collect()
    }
    fn payload(&self,c:&SourceTrackCandidate)->Result<Option<RawLyricPayload>,String>{
        let (format,original,translation,pure)=match self.id.value(){
            "Netease"=>{
                let response=eapi("/api/song/lyric/v1",json!({"id":c.candidate_id,"cp":"false","lv":"0","kv":"0","tv":"0","rv":"0","yv":"0","ytv":"0","yrv":"0","csrf_token":""}),&self.context)?;
                if response["nolyric"].as_bool()==Some(true)||response["uncollected"].as_bool()==Some(true){return Ok(None)}
                if !text(&response["yrc"],"lyric").is_empty(){(LyricPayloadFormat::Yrc,text(&response["yrc"],"lyric"),if !text(&response["ytlrc"],"lyric").is_empty(){text(&response["ytlrc"],"lyric")}else{text(&response["tlyric"],"lyric")},false)}
                else{(LyricPayloadFormat::Lrc,text(&response["lrc"],"lyric"),text(&response["tlyric"],"lyric"),false)}
            }
            "LRCLIB"=>{
                if c.candidate_id.parse::<u64>().is_err(){return Ok(None)}
                let response=http::get(&format!("https://lrclib.net/api/get/{}",c.candidate_id),&self.context)?;
                if !text(&response,"syncedLyrics").is_empty(){(LyricPayloadFormat::Lrc,text(&response,"syncedLyrics"),String::new(),response["instrumental"].as_bool().unwrap_or(false))}
                else{(LyricPayloadFormat::PlainText,text(&response,"plainLyrics"),String::new(),response["instrumental"].as_bool().unwrap_or(false))}
            }
            "Kugou"=>{
                let key=c.fetch_metadata.get("access-key").ok_or("missing access-key")?;
                let response=http::get(&format!("https://lyrics.kugou.com/download?ver=1&client=pc&id={}&accesskey={}&fmt=krc&charset=utf8",http::encode(&c.candidate_id),http::encode(key)),&self.context)?;
                (LyricPayloadFormat::Krc,crypto::decrypt_krc(&text(&response,"content")).map_err(|e|e.to_string())?,String::new(),false)
            }
            "Kuwo"=>{
                if c.candidate_id.parse::<u64>().is_err(){return Ok(None)}
                let response=http::get(&format!("https://m.kuwo.cn/newh5/singles/songinfoandlrc?musicId={}",c.candidate_id),&self.context)?;
                (LyricPayloadFormat::Lrc,kuwo_lrc(&response),String::new(),false)
            }
            "Migu"=>{
                let original=fetch_migu_text(c.fetch_metadata.get("lrc-url"),&self.context)?;
                let translated=fetch_migu_text(c.fetch_metadata.get("translation-url"),&self.context).unwrap_or_default();
                (LyricPayloadFormat::Lrc,original,translated,false)
            }
            "QQMusic"=>{
                let body=format!("version=15&miniversion=82&lrctype=4&musicid={}",http::encode(&c.candidate_id));
                let xml=String::from_utf8(http::request("https://c.y.qq.com/qqmusic/fcgi-bin/lyric_download.fcg","POST",body.as_bytes(),"application/x-www-form-urlencoded","https://y.qq.com/",&self.context,8*1024*1024)?).map_err(|e|e.to_string())?;
                let xml=xml.replace("<!--","").replace("-->","");
                let extract=|tag:&str|->String{
                    let re=regex::Regex::new(&format!(r"(?s)<{tag}[^>]*>(.*?)</{tag}>" )).unwrap();
                    let raw=re.captures(&xml).map(|c|c[1].trim().trim_start_matches("<![CDATA[").trim_end_matches("]]>").to_string()).unwrap_or_default();
                    let decoded=crypto::decrypt_qrc(&raw).unwrap_or(raw);
                    let attribute=regex::Regex::new(r#"(?s)<Lyric_1\b[^>]*\bLyricContent="(.*?)""#).unwrap();
                    attribute.captures(&decoded).map(|c|c[1].replace("&#13;","\r").replace("&#10;","\n").replace("&quot;","\"").replace("&apos;","'").replace("&lt;","<").replace("&gt;",">").replace("&amp;","&")).unwrap_or(decoded)
                };
                let qrc=extract("content");let trans=extract("contentts");
                if !qrc.is_empty()&&qrc.lines().any(|line|line.starts_with('[')){let format=if regex::Regex::new(r"\[\d+,\d+\]").unwrap().is_match(&qrc){LyricPayloadFormat::Qrc}else{LyricPayloadFormat::Lrc};(format,qrc,trans,false)}else{
                    let Some(mid)=c.fetch_metadata.get("mid")else{return Ok(None)};
                    let body=format!("songmid={}&g_tk=5381&loginUin=0&hostUin=0&format=json&inCharset=utf8&outCharset=utf8&platform=yqq&needNewCode=0",http::encode(mid));
                    let response=http::post("https://c.y.qq.com/lyric/fcgi-bin/fcg_query_lyric_new.fcg",&body,"application/x-www-form-urlencoded","https://y.qq.com/",&self.context)?;
                    let decode=|s:String|String::from_utf8(STANDARD.decode(&s).unwrap_or_default()).unwrap_or_default();
                    (LyricPayloadFormat::Lrc,decode(text(&response,"lyric")),decode(text(&response,"trans")),false)
                }
            }
            _=>return Ok(None)
        };
        if original.trim().is_empty()&&!pure{return Ok(None)}
        Ok(Some(RawLyricPayload{provider_id:self.id.clone(),candidate_id:c.candidate_id.clone(),format,original_lyrics:Some(original),translation_lyrics:if translation.is_empty(){None}else{Some(translation)},is_encrypted:false,is_pure_music:pure,diagnostics:BTreeMap::new(),has_stable_identity:true}))
    }
}
impl LyricSource for OnlineSource{
    fn provider_id(&self)->&LyricProviderId{&self.id}
    fn search(&self,plan:&LyricSearchPlan)->Result<Vec<SourceTrackCandidate>,SourceFailure>{
        if ProviderSongIdPolicy::can_use_direct_song_id(&plan.original_track,&self.id){
            let t=&plan.original_track;let id=t.song_id.clone().unwrap_or_default();
            if self.id.value()=="Kugou"{if let Ok(found)=self.kugou_candidates(&id,&t.title,&t.artists.join(", "),&t.album,"direct-song-id"){if !found.is_empty(){return Ok(found)}}}
            else if self.id.value()!="Migu"{let metadata=if self.id.value()=="QQMusic"&&id.parse::<u64>().is_err(){BTreeMap::from([("mid".into(),id.clone())])}else{BTreeMap::from([("direct".into(),"true".into())])};return Ok(vec![self.candidate(id,t.title.clone(),t.artists.clone(),t.album.clone(),t.duration.as_secs_f64(),"direct-song-id",metadata)]);}
        }
        let mut all=vec![];let mut seen=HashSet::new();
        let mut last_error=None;
        for variant in plan.variants.iter().take(3) {
            let found=match self.variant(variant){Ok(v)=>v,Err(e)=>{last_error=Some(e);continue}};
            for candidate in found{if seen.insert(candidate.candidate_id.clone()){all.push(candidate)}}
            if all.iter().any(|c|c.title.trim().eq_ignore_ascii_case(&variant.title)){break}
        }
        if all.is_empty(){if let Some(e)=last_error{return Err(failure(e))}}
        Ok(all)
    }
    fn fetch(&self,c:&SourceTrackCandidate)->Result<Option<RawLyricPayload>,SourceFailure>{self.payload(c).map_err(failure)}
}
#[cfg(test)]mod tests{use super::*;#[test]fn eapi_cipher_is_reproducible_and_form_safe(){let s=eapi_body("/api/song/lyric/v1",json!({"id":"1"}));assert!(s.starts_with("params="));assert!(s[7..].bytes().all(|b|b.is_ascii_hexdigit()));assert_eq!((s.len()-7)%32,0);assert_eq!(s,eapi_body("/api/song/lyric/v1",json!({"id":"1"})));}}



fn number(v:&Value,key:&str)->f64{v[key].as_f64().or_else(||v[key].as_str().and_then(|s|s.parse().ok())).unwrap_or(0.)}
fn split_artists(s:&str)->Vec<String>{s.split(['&','/','、','，',',']).map(str::trim).filter(|s|!s.is_empty()).map(str::to_owned).collect()}
fn decode_entities(s:&str)->String{s.replace("&amp;","&").replace("&quot;","\"").replace("&apos;","'").replace("&lt;","<").replace("&gt;",">").replace("&nbsp;"," ")}
fn kuwo_lrc(response:&Value)->String{items(response,"/data/lrclist").iter().filter_map(|v|{let t=number(v,"time");if !t.is_finite()||t<0.||t>86400.{return None}let ms=(t*1000.).round()as u64;let line=decode_entities(&text(v,"lineLyric"));Some(format!("[{:02}:{:02}.{:03}]{}",ms/60000,ms/1000%60,ms%1000,line))}).collect::<Vec<_>>().join("\n")}
fn fetch_migu_text(url:Option<&String>,ctx:&RequestContext)->Result<String,String>{let Some(url)=url.filter(|s|!s.is_empty())else{return Ok(String::new())};let rest=url.strip_prefix("https://").or_else(||url.strip_prefix("http://")).ok_or("invalid lyric URL")?;let host=rest.split('/').next().unwrap_or("").to_ascii_lowercase();if !["migu.cn","migumusic.cn"].iter().any(|d|host==*d||host.ends_with(&format!(".{d}"))){return Err("invalid lyric host".into())}String::from_utf8(http::request(&format!("https://{rest}"),"GET",&[],"","https://app.c.nf.migu.cn/",ctx,2*1024*1024)?).map_err(|e|e.to_string())}
#[cfg(test)]mod fallback_tests{use super::*;
 #[test]fn keyword_lyric_candidates_keep_identity_duration_and_access_key(){let source=OnlineSource::new("Kugou",RequestContext::new(Duration::from_secs(1)));let found=source.parse_kugou(&json!({"candidates":[{"id":42,"song":"歌名","singer":"歌手A、歌手B","duration":"123456","accesskey":"token"},{"id":43}]}),"","wrong","wrong","album","exact");assert_eq!(found.len(),1);assert_eq!(found[0].title,"歌名");assert_eq!(found[0].artists,["歌手A","歌手B"]);assert_eq!(found[0].duration.as_millis(),123456);assert_eq!(found[0].fetch_metadata["access-key"],"token");}
 #[test]fn kuwo_timestamps_handle_rounding_entities_and_empty_lines(){let lrc=kuwo_lrc(&json!({"data":{"lrclist":[{"time":"59.9996","lineLyric":"你&amp;我"},{"time":"61.25","lineLyric":"下一句"}]}}));assert_eq!(lrc,"[01:00.000]你&我\n[01:01.250]下一句");}
}

fn clean_kuwo_title(title:&str)->String{if let Some((song,credit))=title.split_once("-《"){if ["插曲","主题曲","片尾曲","片头曲"].iter().any(|s|credit.contains(s)){return song.trim().into()}}title.into()}
// Kuwo's search service returns JavaScript string literals, including single-quoted keys.
// Normalize string tokens only; never evaluate a response or replace apostrophes globally.
fn parse_kuwo_search(bytes:&[u8])->Result<Value,String>{
 if let Ok(v)=serde_json::from_slice(bytes){return Ok(v)}
 let text=std::str::from_utf8(bytes).map_err(|e|e.to_string())?;let mut chars=text.chars();let mut out=String::new();
 while let Some(ch)=chars.next(){if ch!='\''&&ch!='"'{out.push(ch);continue}let quote=ch;out.push('"');let mut closed=false;while let Some(ch)=chars.next(){if ch==quote{out.push('"');closed=true;break}if ch=='\\'{let next=chars.next().ok_or("truncated search string")?;if next=='\''{out.push('\'')}else{out.push('\\');out.push(next)}}else if ch=='"'{out.push_str("\\\"")}else{out.push(ch)}}if !closed{return Err("unterminated search string".into())}}
 serde_json::from_str(&out).map_err(|e|e.to_string())
}
#[cfg(test)]mod kuwo_protocol_tests{use super::*;
 #[test]fn single_quoted_json_keeps_apostrophes_and_escaped_quotes(){let parsed=parse_kuwo_search(br#"{'abslist':[{'SONGNAME':'Don\'t \"Stop\"','DURATION':'123'}]}"#).unwrap();assert_eq!(parsed["abslist"][0]["SONGNAME"],"Don't \"Stop\"");}
 #[test]fn soundtrack_credits_are_removed_but_live_versions_remain(){assert_eq!(clean_kuwo_title("海阔天空-《九四2班》网络电影插曲"),"海阔天空");assert_eq!(clean_kuwo_title("海阔天空 (Live)"),"海阔天空 (Live)");}
}
