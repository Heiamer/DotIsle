//! A 16-bit player shell: Chinese pixel lettering, stepped bezels and cassette sprites.
use crate::{draw::{self,Icon},model::Model};
use dotisle_domain::monet::Scheme;
use vello::{Scene,kurbo::{Affine,BezPath,Rect,Stroke},peniko::Fill};
pub const FACE:&str="Fusion Pixel 12px Prop zh-Hans";
pub fn active(m:&Model)->bool{m.prefs.control_center_theme=="Pixel"}
pub fn scheme(dark:bool)->Scheme{if dark{Scheme{
 primary:0xF3CF72,on_primary:0x28213C,primary_container:0x2A2542,on_primary_container:0xF7EFD5,secondary_container:0x655A90,on_secondary_container:0xFFF8E3,tertiary_container:0x2A2542,on_tertiary_container:0xF7EFD5,surface:0x191629,container:0x2A2542,high:0x39314F,on_surface:0xF7EFD5,on_surface_variant:0xC2B4D7,outline:0x716187,outline_variant:0x504260,error:0xFF9B93,
}}else{Scheme{
 primary:0x5D55A5,on_primary:0xFFF9E5,primary_container:0xFFF8E4,on_primary_container:0x302B48,secondary_container:0x5D55A5,on_secondary_container:0xFFF9E5,tertiary_container:0xFFF8E4,on_tertiary_container:0x302B48,surface:0xEAE4EE,container:0xFFF8E4,high:0xDCD2E4,on_surface:0x302B48,on_surface_variant:0x695A76,outline:0x8C7DA0,outline_variant:0xC3B6D1,error:0xAD4248,
}}}
pub fn panel(scene:&mut Scene,r:Rect,bg:u32,edge:u32){let c=4.;let mut p=BezPath::new();p.move_to((r.x0+c,r.y0));for point in [(r.x1-c,r.y0),(r.x1-c,r.y0+c),(r.x1,r.y0+c),(r.x1,r.y1-c),(r.x1-c,r.y1-c),(r.x1-c,r.y1),(r.x0+c,r.y1),(r.x0+c,r.y1-c),(r.x0,r.y1-c),(r.x0,r.y0+c),(r.x0+c,r.y0+c)]{p.line_to(point);}p.close_path();scene.fill(Fill::NonZero,Affine::IDENTITY,draw::color(bg),None,&p);scene.stroke(&Stroke::new(2.),Affine::IDENTITY,draw::color(edge),None,&p);let light=crate::nothing::mix(bg,0xFFFFFF,0.35);draw::fill(scene,draw::rect(r.x0+6.,r.y0+3.,(r.width()-12.).max(0.),2.),0.,light);}
pub fn icon(scene:&mut Scene,i:Icon,x:f64,y:f64,size:f64,rgb:u32){crate::nothing::pixel_icon(scene,i,x,y,size,rgb);}
pub fn meter(scene:&mut Scene,r:Rect,value:f64,fg:u32,track:u32){
 let n=(r.width()/12.).floor().max(1.)as usize;let cell=(r.width()-(n-1)as f64*3.)/n as f64;let progress=if value.is_finite(){value.clamp(0.,1.)*n as f64}else{0.};
 for i in 0..n{let block=draw::rect(r.x0+i as f64*(cell+3.),r.center().y-5.,cell,10.);if i as f64+1.<=progress{draw::fill(scene,block,0.,fg);}else{draw::outline(scene,block,0.,track,1.);let part=(progress-i as f64).clamp(0.,1.);if part>0.{draw::fill(scene,draw::rect(block.x0,block.y0,cell*part,10.),0.,fg);}}}
}
pub fn record(scene:&mut Scene,r:Rect,accent:u32,ink:u32){let cell=(r.width().min(r.height())-20.)/16.;let x=r.center().x-cell*8.;let y=r.center().y-cell*8.;for row in 0..16{for col in 0..16{let frame=(2..14).contains(&col)&&(3..13).contains(&row);if !frame{continue}let edge=col==2||col==13||row==3||row==12;let reels=((col as f64-5.).powi(2)+(row as f64-7.).powi(2)<=4.)||((col as f64-10.).powi(2)+(row as f64-7.).powi(2)<=4.);let label=row==10&&(5..11).contains(&col);let color=if edge||label{ink}else if reels{accent}else{crate::nothing::mix(accent,0xFFF8E4,0.7)};draw::fill(scene,draw::rect(x+col as f64*cell,y+row as f64*cell,cell,cell),0.,color);}}}
