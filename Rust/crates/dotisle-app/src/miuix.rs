//! Native interpretation of Miuix: grouped surfaces, continuous corners and calm blue controls.
use dotisle_domain::monet::Scheme;
use vello::{Scene,kurbo::{Affine,BezPath,Rect},peniko::Fill};
use crate::{model::Model,draw};
pub fn active(m:&Model)->bool{m.prefs.control_center_theme=="Miuix"}
pub fn scheme(dark:bool)->Scheme{if dark{Scheme{
 primary:0x5D9BFF,on_primary:0x0B1E36,primary_container:0x202020,on_primary_container:0xF0F0F0,secondary_container:0x17365D,on_secondary_container:0xB7D5FF,tertiary_container:0x202020,on_tertiary_container:0xF0F0F0,surface:0x101010,container:0x202020,high:0x343434,on_surface:0xF0F0F0,on_surface_variant:0xAAAAAA,outline:0x525252,outline_variant:0x383838,error:0xFF8074,
}}else{Scheme{
 primary:0x3482FF,on_primary:0xFFFFFF,primary_container:0xFFFFFF,on_primary_container:0x151515,secondary_container:0xEAF2FF,on_secondary_container:0x2467D0,tertiary_container:0xFFFFFF,on_tertiary_container:0x151515,surface:0xF7F7F7,container:0xFFFFFF,high:0xF0F0F0,on_surface:0x151515,on_surface_variant:0x666666,outline:0xB2B2B2,outline_variant:0xE6E6E6,error:0xD83D2E,
}}}
/// Superelliptic corners have flat tangents at the adjoining straight edges.
pub fn surface(scene:&mut Scene,r:Rect,radius:f64,color:u32){
 let a=radius.min(r.width()/2.).min(r.height()/2.);let mut p=BezPath::new();
 for (cx,cy,start)in [(r.x1-a,r.y0+a,-90_f64),(r.x1-a,r.y1-a,0.),(r.x0+a,r.y1-a,90.),(r.x0+a,r.y0+a,180.)]{
  for i in 0..=24{let t=(start+i as f64*90./24.).to_radians();let(c,s)=(t.cos(),t.sin());let point=(cx+a*c.signum()*c.abs().powf(0.625),cy+a*s.signum()*s.abs().powf(0.625));if p.elements().is_empty(){p.move_to(point);}else{p.line_to(point);}}
 }p.close_path();scene.fill(Fill::NonZero,Affine::IDENTITY,draw::color(color),None,&p);
}
