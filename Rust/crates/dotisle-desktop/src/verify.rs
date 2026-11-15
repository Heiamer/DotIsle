//! Offscreen acceptance uses exactly the desktop window renderer.
use crate::{desktop_cards,draw::Text,gpu::Gpu,model::Model};
use dotisle_domain::desktop_widgets::{Kind,Size};
use serde_json::json;
use std::path::Path;

pub fn capture(output:&Path)->Result<(),String>{
 std::fs::create_dir_all(output).map_err(|e|e.to_string())?;
 let result=(||{
  let mut model=Model::new();let mut text=Text::new_lazy_japanese();let mut gpu=Gpu::new();let mut captures=vec![];
  for dark in [false,true]{for kind in Kind::ALL.into_iter().chain([Kind::Plugin]){for size in Size::ALL{
   model.prefs.dark=dark;
   let(w,h)=size.dimensions();let name=format!("{:?}-{:?}-{dark}.png",kind,size);
   let view=desktop_cards::render(&model,&mut text,kind,size,"verification",(-1.,-1.),false,0.,Some(0.));
   for hit in &view.hits{if hit.rect.x0<0.||hit.rect.y0<0.||hit.rect.x1>w||hit.rect.y1>h{return Err(format!("hit outside {name}: {}",hit.label));}}
   gpu.capture(&view.scene,w as u32,h as u32,&output.join(&name))?;
   captures.push(json!({"file":name,"kind":kind,"size":size,"dark":dark,"width":w,"height":h,"hits":view.hits.len()}));
  }}}
  Ok(json!({"success":true,"renderer":"dotisle-render / production desktop_cards::render","captures":captures}))
 })();
 let report=match &result{Ok(v)=>v.clone(),Err(e)=>json!({"success":false,"error":e})};
 std::fs::write(output.join("report.json"),serde_json::to_vec_pretty(&report).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
 result.map(|_|())
}
