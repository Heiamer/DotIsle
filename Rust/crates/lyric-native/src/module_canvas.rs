//! Bounded primitive scene replay. Feature DLLs never receive a Rust renderer pointer.
use serde_json::Value;
use vello::{Scene,kurbo::{Affine,Rect,RoundedRect,Stroke},peniko::Gradient};
pub fn paint(scene:&mut Scene,commands:&Value){
 for c in commands.as_array().into_iter().flatten().take(256){
  if c["op"]!="roundedStroke"{continue;}
  let Some(b)=c["bounds"].as_array().filter(|b|b.len()==4)else{continue;};let b=b.iter().filter_map(Value::as_f64).collect::<Vec<_>>();if b.len()!=4||b.iter().any(|n|!n.is_finite()){continue;}
  let r=Rect::new(b[0],b[1],b[2],b[3]);let inflate=c["inflate"].as_f64().unwrap_or(0.).clamp(0.,64.);let radius=c["radius"].as_f64().unwrap_or(0.).clamp(0.,1000.);let width=c["width"].as_f64().unwrap_or(1.).clamp(0.,128.);
  let shape=RoundedRect::from_rect(r.inflate(inflate,inflate),radius+inflate);let alpha=(255.*c["alpha"].as_f64().unwrap_or(0.).clamp(0.,1.)).round()as u32;
  let colors=c["colors"].as_array().into_iter().flatten().take(16).filter_map(Value::as_u64).map(|rgb|crate::renderer::color((alpha<<24)|(rgb as u32&0xffffff))).collect::<Vec<_>>();
  if colors.len()==1{scene.stroke(&Stroke::new(width),Affine::IDENTITY,colors[0],None,&shape);}else if colors.len()>1{
   let center=r.center();let transform=Affine::translate((center.x,center.y))*Affine::rotate(c["phase"].as_f64().unwrap_or(0.))*Affine::translate((-center.x,-center.y));
   let gradient=Gradient::new_sweep(center,0.,std::f32::consts::TAU).with_stops(colors.as_slice());scene.stroke(&Stroke::new(width),Affine::IDENTITY,&gradient,Some(transform),&shape);
  }
 }
}
