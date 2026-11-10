//! Versioned, validated music modules shared by the editor, native paint and hit testing.
use serde::{Deserialize,Serialize};
use crate::music_layout::{MusicGeometry,MusicLayout,Rect,music_geometry};

#[derive(Clone,Copy,Debug,Serialize,Deserialize,PartialEq,Eq)]
#[serde(rename_all="camelCase")]
pub enum Module { Header, Lyrics, Translation, Status, Progress, Controls }
impl Module {
 pub const ALL:[Self;6]=[Self::Header,Self::Lyrics,Self::Translation,Self::Status,Self::Progress,Self::Controls];
 pub fn label(self)->&'static str{match self{Self::Header=>"封面与歌曲信息",Self::Lyrics=>"当前歌词",Self::Translation=>"翻译与下一句",Self::Status=>"播放状态",Self::Progress=>"播放进度",Self::Controls=>"播放控制"}}
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Slot {pub kind:Module,pub visible:bool}
/// Positions are stored in a 400 DIP design space; native widths resolve proportionally.
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Placement {pub element:u8,pub x:f64,pub y:f64,#[serde(default,skip_serializing_if="Option::is_none")]pub width:Option<f64>}
pub fn element_label(id:u8)->&'static str{match id{0=>"专辑封面",1=>"歌曲标题",2=>"歌手",3=>"当前歌词",4=>"翻译与下一句",5=>"播放状态",6=>"播放进度条",7=>"当前时间",8=>"歌曲时长",34=>"播放模式",41=>"喜欢",35=>"上一首",36=>"播放 / 暂停",37=>"下一首",38=>"翻译按钮",39=>"设置按钮",_=>"元素"}}
pub fn resizable(id:u8)->bool{[1,2,3,4,5,6].contains(&id)}
pub fn elements(g:&MusicGeometry)->Vec<(u8,Rect)>{
 let mut out=vec![(0,g.cover),(1,g.title),(2,g.artist),(3,g.lyric),(4,g.secondary),(5,Rect::new(g.status_dot.x,g.status_text.y,(g.status_text.x+g.status_text.w-g.status_dot.x).max(0.),g.status_text.h)),(6,g.progress_hit),(7,g.elapsed),(8,g.total)];
 out.extend(g.buttons.iter().map(|(id,r)|((32+*id)as u8,*r)));out.retain(|(_,r)|r.w>0.&&r.h>0.);out
}
fn translate(r:&mut Rect,dx:f64,dy:f64){if r.w>0.&&r.h>0.{r.x+=dx;r.y+=dy;}}
fn position(g:&mut MusicGeometry,p:&Placement,width:f64){
 let Some((_,old))=elements(g).into_iter().find(|(id,_)|*id==p.element)else{return};
 let span=width/400.;let target_w=if resizable(p.element){p.width.map(|w|w*span).unwrap_or(old.w).clamp(24.,width-24.)}else{old.w};
 let x=(p.x*span).clamp(12.,(width-target_w-12.).max(12.));let y=p.y.clamp(12.,600.);let(dx,dy)=(x-old.x,y-old.y);
 match p.element{
  0=>translate(&mut g.cover,dx,dy),1=>{translate(&mut g.title,dx,dy);g.title.w=target_w;},2=>{translate(&mut g.artist,dx,dy);g.artist.w=target_w;},3=>{translate(&mut g.lyric,dx,dy);g.lyric.w=target_w;},4=>{translate(&mut g.secondary,dx,dy);g.secondary.w=target_w;},
  5=>{translate(&mut g.status_dot,dx,dy);translate(&mut g.status_text,dx,dy);g.status_text.w=(target_w-10.).max(14.);},
  6=>{translate(&mut g.track,dx,dy);translate(&mut g.progress_hit,dx,dy);translate(&mut g.seek_chip,dx,dy);g.progress_hit.w=target_w;g.track.w=(target_w-8.).max(1.);g.seek_chip.x=g.track.x+(g.track.w-g.seek_chip.w)/2.;},
  7=>translate(&mut g.elapsed,dx,dy),8=>translate(&mut g.total,dx,dy),id=>{if let Some((_,r))=g.buttons.iter_mut().find(|(n,_)|32+*n==id as i32){translate(r,dx,dy);}},
 }
}
#[derive(Clone,Debug,Serialize,Deserialize,PartialEq)]
#[serde(rename_all="camelCase",deny_unknown_fields)]
pub struct Layout {
 #[serde(default,skip_serializing_if="Vec::is_empty")]pub elements:Vec<Placement>,
 pub version:u8,pub enabled:bool,pub modules:Vec<Slot>,pub alignment:u8,pub gap:f64,pub cover_size:f64,pub show_cover:bool,
 pub controls:Vec<i32>,pub transport_theme:String,pub bar_style:String,pub thumb_style:String,pub thumb_enabled:bool,pub thumb_spin:bool,
}
impl Default for Layout {
 fn default()->Self{Self{elements:vec![],version:1,enabled:false,modules:Module::ALL.map(|kind|Slot{kind,visible:true}).to_vec(),alignment:0,gap:8.,cover_size:56.,show_cover:true,controls:vec![2,9,3,4,5,6,7],transport_theme:"Reef".into(),bar_style:"Wavy".into(),thumb_style:"Pill".into(),thumb_enabled:false,thumb_spin:false}}
}
impl Layout {
 pub fn from_value(value:&serde_json::Value)->Result<Self,String>{let layout:Self=serde_json::from_value(value.clone()).map_err(|_|"布局格式无效，请恢复推荐布局".to_string())?;layout.validate()?;Ok(layout)}
 pub fn validate(&self)->Result<(),String>{
  if self.version!=1||self.alignment>2||!self.gap.is_finite()||!(4. ..=16.).contains(&self.gap)||!self.cover_size.is_finite()||!(40. ..=80.).contains(&self.cover_size){return Err("布局版本、间距或封面尺寸无效".into())}
  if self.modules.len()!=6||Module::ALL.iter().any(|kind|self.modules.iter().filter(|s|s.kind==*kind).count()!=1){return Err("每个布局模块必须且只能出现一次".into())}
  if !self.modules.iter().any(|s|s.visible&&s.kind==Module::Header)||!self.modules.iter().any(|s|s.visible&&s.kind==Module::Controls){return Err("歌曲信息与播放控制需要保留".into())}
  if self.controls.is_empty()||self.controls.len()>7||!self.controls.contains(&4)||self.controls.iter().any(|id|![2,9,3,4,5,6,7].contains(id))||self.controls.iter().enumerate().any(|(i,id)|self.controls[..i].contains(id)){return Err("播放按钮排列无效，必须保留播放按钮".into())}
  if !["Reef","Ripple","Sand","Mist","Glaze","InkIsle","Tide","Whitespace","Halo","Layered"].contains(&self.transport_theme.as_str())||!["Normal","Wavy","Rounded","Thin","Thick","Gradient","Segmented","Dots"].contains(&self.bar_style.as_str())||!["Default","Circle","Square","Pill","Diamond","Flower","Heart","Cookie","Puffy","Clover","Clover8","Gem","Triangle","Pentagon","Cookie12","Clam"].contains(&self.thumb_style.as_str()){return Err("播放组件样式无效".into())}
  if self.elements.len()>16||self.elements.iter().enumerate().any(|(i,p)|![0,1,2,3,4,5,6,7,8,34,35,36,37,38,39,41].contains(&p.element)||self.elements[..i].iter().any(|q|q.element==p.element)||!p.x.is_finite()||!p.y.is_finite()||!(12. ..=388.).contains(&p.x)||!(12. ..=600.).contains(&p.y)||p.width.is_some_and(|w|!resizable(p.element)||!w.is_finite()||!(24. ..=376.).contains(&w))){return Err("画布元素的位置或宽度无效".into())}
  Ok(())
 }
 pub fn move_module(&mut self,from:usize,to:usize){if from<self.modules.len()&&to<self.modules.len()&&from!=to{let item=self.modules.remove(from);self.modules.insert(to,item);}}
 pub fn set_element(&mut self,element:u8,x:f64,y:f64,width:Option<f64>){
  let mut p=self.elements.iter().find(|p|p.element==element).cloned().unwrap_or(Placement{element,x:12.,y:12.,width:None});p.x=x.clamp(12.,388.);p.y=y.clamp(12.,600.);if let Some(w)=width{if resizable(element){p.width=Some(w.clamp(24.,376.));}}
  self.elements.retain(|p|p.element!=element);self.elements.push(p);self.enabled=true;
 }
 pub fn geometry(&self,width:f64,scale:f64,font:f64)->MusicGeometry{
  let mut g=music_geometry(width,MusicLayout::SideBySide,0.,scale,font);let zero=Rect::default();g.cover=zero;g.title=zero;g.artist=zero;g.lyric=zero;g.secondary=zero;g.status_dot=zero;g.status_text=zero;g.track=zero;g.elapsed=zero;g.total=zero;g.progress_hit=zero;g.seek_chip=zero;g.text_hit=zero;for(_,r)in &mut g.buttons{*r=zero;}
  let mut y=18.;let align=self.alignment as f64/2.;g.align=align;g.split=0.;
  for slot in self.modules.iter().filter(|s|s.visible){match slot.kind{
   Module::Header=>{let size=if self.show_cover{self.cover_size}else{0.};let tx=if size>0.{size+36.}else{20.};let tw=(width-tx-20.).max(1.);if size>0.{g.cover=Rect::new(20.,y,size,size);}let h=size.max(48.);g.title=Rect::new(tx,y+(h-46.)/2.,tw,24.);g.artist=Rect::new(tx,g.title.y+28.,tw,18.);g.text_hit=Rect::new(tx,y,tw,h);y+=h;},
   Module::Lyrics=>{let h=font+12.;g.lyric=Rect::new(20.,y,width-40.,h);y+=h;},
   Module::Translation=>{g.secondary=Rect::new(20.,y,width-40.,18.);y+=18.;},
   Module::Status=>{g.status_dot=Rect::new(20.,y+6.,4.,4.);g.status_text=Rect::new(30.,y,width-50.,16.);y+=16.;},
   Module::Progress=>{g.track=Rect::new(20.,y+8.,width-40.,4.);g.progress_hit=Rect::new(16.,y,width-32.,22.);g.elapsed=Rect::new(20.,y+20.,70.,14.);g.total=Rect::new(width-90.,y+20.,70.,14.);g.seek_chip=Rect::new(width/2.-65.,y+19.,130.,24.);y+=46.;},
   Module::Controls=>{let sizes=self.controls.iter().map(|id|if *id==4{40.}else{28.}).collect::<Vec<_>>();let total=sizes.iter().sum::<f64>()+12.*(sizes.len()-1)as f64;let mut bx=20.+(width-40.-total)*align;for(id,sz)in self.controls.iter().zip(sizes){if let Some((_,r))=g.buttons.iter_mut().find(|(n,_)|n==id){*r=Rect::new(bx,y+(44.-sz)/2.,sz,sz);}bx+=sz+12.;}y+=44.;},
  }y+=self.gap;}
  g.height=(y-self.gap+18.).max(110.);
  for p in &self.elements{position(&mut g,p,width);}
  for(_,r)in elements(&g){g.height=g.height.max(r.y+r.h+18.);}
  g.text_hit=Rect::new(g.title.x.min(g.artist.x),g.title.y.min(g.artist.y),(g.title.x+g.title.w).max(g.artist.x+g.artist.w)-g.title.x.min(g.artist.x),(g.title.y+g.title.h).max(g.artist.y+g.artist.h)-g.title.y.min(g.artist.y));g
 }
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn rejects_duplicated_modules_unknown_controls_and_unsupported_versions(){let mut l=Layout::default();l.modules[1].kind=Module::Header;assert!(l.validate().is_err());let mut l=Layout::default();l.controls.push(4);assert!(l.validate().is_err());let mut l=Layout::default();l.version=2;assert!(l.validate().is_err());}
 #[test]fn every_module_order_keeps_visible_content_and_targets_apart(){for align in 0..=2{for gap in [4.,8.,16.]{for cover in [40.,56.,80.]{let mut l=Layout{alignment:align,gap,cover_size:cover,..Default::default()};for i in 0..6{l.move_module(0,5);for width in [400.,592.]{let g=l.geometry(width,1.5,20.);let mut rows=vec![g.cover,g.title,g.artist,g.lyric,g.secondary,g.status_text,g.progress_hit];rows.extend(g.buttons.map(|(_,r)|r));for r in &rows{assert!(r.x>=0.&&r.y>=0.&&r.x+r.w<=width&&r.y+r.h<=g.height,"{i}: {r:?}");}for(a,r)in rows.iter().enumerate(){for q in rows.iter().skip(a+1){assert!(!(r.x<q.x+q.w&&q.x<r.x+r.w&&r.y<q.y+q.h&&q.y<r.y+r.h),"overlap {r:?} {q:?}");}}}}}}}}
 #[test]fn hidden_modules_have_no_target_and_json_roundtrips(){let mut l=Layout::default();for s in &mut l.modules{if ![Module::Header,Module::Controls].contains(&s.kind){s.visible=false;}}let g=l.geometry(400.,1.,14.);assert_eq!(g.progress_hit.w,0.);assert_eq!(g.lyric.h,0.);assert_eq!(Layout::from_value(&serde_json::to_value(&l).unwrap()).unwrap(),l);}
}

#[cfg(test)]mod canvas_tests{use super::*;
 #[test]fn legacy_layouts_open_without_positions_and_reject_invalid_placements(){let l=Layout::default();let v=serde_json::to_value(&l).unwrap();assert!(v.get("elements").is_none());assert!(Layout::from_value(&v).is_ok());let mut l=l;l.elements.push(Placement{element:255,x:20.,y:20.,width:None});assert!(l.validate().is_err());l.elements[0].element=0;l.elements[0].x=f64::NAN;assert!(l.validate().is_err());}
 #[test]fn moved_elements_and_seek_targets_share_native_geometry(){let mut l=Layout::default();l.set_element(0,300.,210.,None);l.set_element(6,40.,120.,Some(300.));l.set_element(36,220.,300.,None);assert!(l.validate().is_ok());for width in [400.,592.]{let g=l.geometry(width,1.5,14.);let span=width/400.;assert_eq!(g.cover.x,300.*span);assert_eq!(g.cover.y,210.);assert_eq!(g.progress_hit.x,40.*span);assert_eq!(g.track.x,g.progress_hit.x+4.);assert_eq!(g.progress_hit.w,300.*span);let play=g.buttons.iter().find(|(id,_)|*id==4).unwrap().1;assert_eq!(play.x,220.*span);assert_eq!(play.y,300.);assert!(g.height>=play.y+play.h+18.);for(_,r)in elements(&g){assert!(r.x>=0.&&r.x+r.w<=width+0.01&&r.y+r.h<=g.height);}}assert_eq!(Layout::from_value(&serde_json::to_value(&l).unwrap()).unwrap(),l);}
 #[test]fn hidden_positioned_modules_never_reappear(){let mut l=Layout::default();l.set_element(6,40.,120.,Some(200.));l.modules.iter_mut().find(|s|s.kind==Module::Progress).unwrap().visible=false;let g=l.geometry(400.,1.,14.);assert_eq!(g.track.w,0.);assert_eq!(g.progress_hit.w,0.);assert!(!elements(&g).iter().any(|(id,_)|*id==6));}
}
