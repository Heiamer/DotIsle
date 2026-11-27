//! Desktop-only primitives; no settings pages or alternate UI themes.
use crate::{draw::{self, Icon,Text}, model::Action};
use dotisle_domain::monet::Scheme;
use vello::{Scene,kurbo::Rect};
#[derive(Clone,Debug)]pub struct Hit{pub rect:Rect,pub action:Action,pub label:String,pub enabled:bool}
pub struct Ui<'a>{pub scene:&'a mut Scene,pub text:&'a mut Text,pub s:Scheme,pub hits:Vec<Hit>,pub pointer:(f64,f64),pub clip:Rect,pub glass:bool}
impl<'a> Ui<'a>{
 pub fn new(scene:&'a mut Scene,text:&'a mut Text,s:Scheme,pointer:(f64,f64),clip:Rect)->Self{Self{scene,text,s,pointer,clip,hits:vec![],glass:false}}
 pub fn label(&mut self,text:&str,x:f64,y:f64,size:f32,color:u32,bold:bool,max:f64){self.text.draw_face_fixed(self.scene,"Noto Sans SC",text,x,y,size,0xff000000|color,if bold{600}else{400},max);}
 pub fn center(&mut self,text:&str,r:Rect,size:f32,color:u32,bold:bool){let width=self.text.face_width("Noto Sans SC",text,size,if bold{600}else{400}).min((r.width()-8.).max(1.));self.label(text,r.x0+(r.width()-width)/2.,r.y0+(r.height()-size as f64*1.35)/2.,size,color,bold,(r.width()-8.).max(1.));}
 pub fn label_face(&mut self,family:&str,text:&str,x:f64,y:f64,size:f32,color:u32,bold:bool,max:f64){self.text.draw_face_fixed(self.scene,family,text,x,y,size,0xff000000|color,if bold{600}else{400},max);}
 pub fn center_face(&mut self,family:&str,text:&str,r:Rect,size:f32,color:u32,bold:bool){let width=self.text.face_width(family,text,size,if bold{600}else{400}).min((r.width()-8.).max(1.));self.label_face(family,text,r.x0+(r.width()-width)/2.,r.y0+(r.height()-size as f64*1.35)/2.,size,color,bold,(r.width()-8.).max(1.));}
 pub fn measure_alpha(&mut self,text:&str,size:f32,bold:bool)->f64{self.text.face_width("Noto Sans SC",text,size,if bold{600}else{400})}
 pub fn icon(&mut self,kind:Icon,x:f64,y:f64,size:f64,color:u32){draw::icon(self.scene,kind,x,y,size,color);}
 pub fn hit(&mut self,r:Rect,label:&str,action:Action,enabled:bool)->Option<usize>{let r=r.intersect(self.clip);if r.width()<=0.||r.height()<=0.{return None}let index=self.hits.len();self.hits.push(Hit{rect:r,label:label.into(),action,enabled});Some(index)}
 pub fn button(&mut self,r:Rect,label:&str,icon:Option<Icon>,action:Action,tonal:bool,enabled:bool){
  self.hit(r,label,action,enabled);draw::fill(self.scene,r,12.,if tonal{self.s.high}else{self.s.primary});
  let color=if tonal{self.s.on_surface}else{self.s.on_primary};
  if let Some(icon)=icon{self.icon(icon,r.x0+12.,r.center().y-8.,16.,color);self.center(label,Rect::new(r.x0+23.,r.y0,r.x1,r.y1),11.,color,true);}else{self.center(label,r,11.,color,true);}
 }
 pub fn icon_button(&mut self,r:Rect,icon:Icon,label:&str,action:Action,filled:bool,enabled:bool){self.hit(r,label,action,enabled);if filled||r.contains(self.pointer){draw::fill(self.scene,r,10.,self.s.high)}self.icon(icon,r.center().x-9.,r.center().y-9.,18.,self.s.on_surface);}
}
