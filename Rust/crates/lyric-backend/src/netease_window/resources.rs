//! Read geometry facts from the installed client package, never a version list.
//! Client assets are inspected locally and are never copied into DotIsle.
use std::{collections::BTreeMap,path::{Path,PathBuf},sync::Mutex,time::SystemTime};
use lyric_archive::ZipReader;
#[derive(Clone,Debug,serde::Serialize)]pub(super)struct Legacy{footer:f64,top:f64,padding:f64,track:f64,half:f64,wide_half:f64,breakpoint:f64}
#[derive(Clone,Debug,serde::Serialize)]pub(super)enum Layout{Legacy(Legacy),SemanticFooter}
#[derive(Debug)]struct Rule{selector:String,values:BTreeMap<String,String>,minimum:f64}
fn number(text:&str)->Option<f64>{let v=text.trim().strip_suffix("px")?.trim().parse::<f64>().ok()?;v.is_finite().then_some(v)}
fn without_comments(text:&str)->String{let mut rest=text;let mut out=String::new();while let Some(at)=rest.find("/*"){out.push_str(&rest[..at]);let Some(end)=rest[at+2..].find("*/")else{return out};rest=&rest[at+2+end+2..];}out.push_str(rest);out}
fn rules(text:&str)->Vec<Rule>{
 let css=without_comments(text);let mut out=Vec::new();let mut stack=Vec::<(String,usize)>::new();let mut start=0;
 for(at,c)in css.char_indices(){if c=='{'{stack.push((css[start..at].trim().to_owned(),at+1));start=at+1;}
  else if c=='}'{if let Some((selector,body))=stack.pop(){if !selector.starts_with('@')&&!css[body..at].contains(['{','}']){
   let minimum=stack.iter().filter_map(|(scope,_)|scope.find("min-width:").and_then(|i|scope[i+10..].split(')').next()).and_then(number)).fold(0.,f64::max);
   let values=css[body..at].split(';').filter_map(|v|v.split_once(':')).map(|(k,v)|(k.trim().to_owned(),v.trim().to_owned())).collect();
   out.push(Rule{selector,values,minimum});
  }}start=at+1;}
 }out
}
fn px(rule:&Rule,key:&str)->Option<f64>{number(rule.values.get(key)?)}
fn half(rule:&Rule)->Option<f64>{let value=rule.values.get("left")?.replace(' ',"");let n=number(value.strip_prefix("calc(50%-")?.strip_suffix(')')?)?;let right=rule.values.get("right")?.replace(' ',"");(right==value).then_some(n)}
fn parse(css:&str)->Option<Layout>{
 let rs=rules(css);
 let mut bars=rs.iter().filter(|r|r.selector.trim()==".m-player"&&r.values.get("bottom").is_some_and(|v|v=="0"||v=="0px")).filter_map(|r|px(r,"height")).collect::<Vec<_>>();bars.dedup();
 if bars.len()==1{
  let base=rs.iter().find(|r|r.selector.trim()==".m-player .prg-ply"&&r.minimum==0.&&r.values.contains_key("top"))?;
  let narrow=half(base)?;let mut wide=rs.iter().filter(|r|r.selector.trim()==".m-player .prg-ply"&&r.minimum>0.).filter_map(|r|half(r).map(|h|(r.minimum,h))).collect::<Vec<_>>();wide.sort_by(|a,b|a.0.total_cmp(&b.0));wide.dedup();if wide.len()>1{return None}
  let padding_rule=rs.iter().find(|r|r.selector.split(',').any(|s|matches!(s.trim(),".m-player .prg-ply"|".m-player .prg"))&&r.values.contains_key("padding"))?;
  let padding=number(padding_rule.values.get("padding")?.split_whitespace().next()?)?;
  let track=px(padding_rule,"height")?;let(wide_at,wide_half)=wide.first().copied().unwrap_or((6001.,narrow));
  let layout=Legacy{footer:bars[0],top:px(base,"top")?,padding,track,half:narrow,wide_half,breakpoint:wide_at};
  if !(40. ..=160.).contains(&layout.footer)||!(100. ..=400.).contains(&narrow)||layout.top+padding+track>layout.footer{return None}return Some(Layout::Legacy(layout))
 }
 // A semantic play footer distinguishes classic, full-width and lyric layouts
 // at runtime. CSS presence establishes the capability, not the active variant.
 if rs.iter().any(|r|r.selector.contains("DefaultBarWrapper_"))&&rs.iter().any(|r|r.selector.contains("StyledSlider_")||r.selector.contains("StyledSliderContainer_")){return Some(Layout::SemanticFooter)}
 None
}
pub(super)fn inspect(package:&Path)->Result<Layout,String>{
 let mut zip=ZipReader::open_embedded(package).map_err(|e|e.to_string())?;let mut css=String::new();let mut total=0u64;
 let entries=zip.entries().iter().enumerate().filter(|(_,e)|e.name=="pub/core.css"||e.name.starts_with("pub/styles/")&&e.name.ends_with(".css")).map(|(i,e)|(i,e.size,e.compressed)).collect::<Vec<_>>();
 if entries.len()>512{return Err("网易云界面资源过大".into())}
 for(index,size,compressed)in entries{total+=size;if size>8*1024*1024||compressed>8*1024*1024||total>64*1024*1024{return Err("网易云界面资源过大".into())}
  let data=zip.read(index).map_err(|e|e.to_string())?;let text=std::str::from_utf8(&data).map_err(|_|"网易云界面资源编码无效")?;css.push_str(&without_comments(text));css.push('\n');
 }
 parse(&css).ok_or_else(||"网易云界面布局暂不可识别".into())
}
struct Cached{path:PathBuf,size:u64,modified:Option<SystemTime>,result:Result<Layout,String>}
static CACHE:Mutex<Option<Cached>>=Mutex::new(None);
pub(super)fn installed(executable:&Path)->Result<Layout,String>{
 let path=executable.parent().ok_or("网易云路径无效")?.join("package/orpheus.ntpk");let meta=std::fs::metadata(&path).map_err(|_|"网易云界面资源尚未就绪")?;let modified=meta.modified().ok();let mut cache=CACHE.lock().map_err(|_|"网易云界面资源正在更新")?;
 if cache.as_ref().is_none_or(|c|c.path!=path||c.size!=meta.len()||c.modified!=modified){*cache=Some(Cached{result:inspect(&path),path,size:meta.len(),modified});}
 cache.as_ref().ok_or("网易云界面资源尚未就绪")?.result.clone()
}
impl Legacy{pub(super)fn track(&self,width:i32,height:i32,dpi:u32)->(f64,f64,f64){let scale=dpi as f64/96.;let half=if width as f64/scale>=self.breakpoint{self.wide_half}else{self.half};(width as f64/2.-half*scale,width as f64/2.+half*scale,height as f64-(self.footer-self.top-self.padding-self.track/2.)*scale)}}
#[cfg(test)]mod tests{use super::*;
 const LEGACY:&str=".m-player{height:72px;bottom:0}.m-player .prg-ply{top:50px;left:calc(50% - 176px);right:calc(50% - 176px)}@media screen and (min-width:1104px){.m-player .prg-ply{left:calc(50% - 186px);right:calc(50% - 186px)}}.m-player .prg-ply{height:3px;padding:4px 0}";
 #[test]fn legacy_track_follows_resource_dimensions_and_media_query(){let Layout::Legacy(l)=parse(LEGACY).unwrap()else{panic!()};assert_eq!(l.track(1022,752,96),(335.,687.,735.5));assert_eq!(l.track(1200,800,96),(414.,786.,783.5));assert_eq!(l.track(1533,1128,144),(502.5,1030.5,1103.25));assert!(matches!(parse(&LEGACY.replace(".m-player .prg-ply{height:",".m-player .prg{height:")),Some(Layout::Legacy(_))));}
 #[test]fn comments_unknown_resources_and_incomplete_geometry_never_select_a_layout(){assert!(parse("/* .DefaultBarWrapper_x{}.StyledSlider_x{} */").is_none());assert!(parse(".m-player{height:72px;bottom:0}").is_none());assert!(parse(&LEGACY.replace("height:72px","height:3px")).is_none());assert!(matches!(parse(".DefaultBarWrapper_any{}.StyledSlider_other{}"),Some(Layout::SemanticFooter)));}
}
