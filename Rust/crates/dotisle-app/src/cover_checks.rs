//! Real rendering checks for cover conversion, replacement and segmented playback.
use std::path::Path;
use serde_json::json;
use crate::{cover_art,draw::{self,Text,rect},gpu::Gpu,model::{Model,Artwork,Page},views::{self,ViewState}};
pub fn run(out:&Path)->Result<(),String>{
 std::fs::create_dir_all(out).map_err(|e|e.to_string())?;cover_art::verify()?;
 let mut m=Model::new(out.join("isolated-data"),true);m.tutorial=None;m.tutorial_checked=true;
 let real=std::env::var_os("DOTISLE_QA_COVER");
 if let Some(path)=real.as_ref(){let im=image::open(path).map_err(|e|e.to_string())?;im.save(out.join("source-cover.png")).map_err(|e|e.to_string())?;m.artwork=Some(Artwork::new(im));m.snapshot["track"]["title"]=json!("当前网易云封面");m.snapshot["track"]["artist"]=json!("色块与半块字符转换预览");}
 if let Some(a)=&m.artwork{for(name,im)in [("terminal-cover.png",&a.covers.terminal),("pixel-cover.png",&a.covers.pixel)]{image::save_buffer(out.join(name),im.data.as_ref(),im.width,im.height,image::ColorType::Rgba8).map_err(|e|e.to_string())?;}}
 let mut text=Text::new();let mut gpu=Gpu::new();let mut captures=vec![];let mut meters=vec![];
 for theme in ["Terminal","Pixel"]{m.prefs.control_center_theme=theme.into();for dark in [false,true]{m.prefs.dark=dark;let s=if theme=="Pixel"{crate::pixel::scheme(dark)}else{crate::terminal::scheme(dark)};let mut counts=vec![];
  for fraction in [0.,0.37,1.]{m.page=Page::Home;m.snapshot["positionSeconds"]=json!(252.*fraction);m.snapshot["isPlaying"]=json!(fraction>0.);let view=views::render(&m,&mut text,1200.,800.,&ViewState::default());let name=format!("{theme}-{dark}-{}-home.png",(fraction*100.)as u32);gpu.capture(&view.scene,1200,800,&out.join(&name))?;captures.push(name);
   let mut scene=vello::Scene::new();draw::fill(&mut scene,rect(0.,0.,380.,56.),0.,s.primary_container);let r=rect(12.,12.,356.,32.);if theme=="Pixel"{crate::pixel::meter(&mut scene,r,fraction,s.primary,s.outline);}else{crate::terminal::meter(&mut scene,r,fraction,s.primary,s.outline);}
   let name=format!("meter-{theme}-{dark}-{}.png",(fraction*100.)as u32);gpu.capture(&scene,380,56,&out.join(&name))?;let im=image::open(out.join(&name)).map_err(|e|e.to_string())?.to_rgb8();let count=im.pixels().filter(|p|p[0]as u32==s.primary>>16&&p[1]as u32==(s.primary>>8)&255&&p[2]as u32==s.primary&255).count();counts.push(count);captures.push(name);
  }
  if counts[0]!=0||counts[1]<=counts[0]||counts[2]<=counts[1]*2{return Err(format!("played/unplayed blocks are indistinguishable: {theme}/{dark}/{counts:?}"))}
  meters.push(json!({"theme":theme,"dark":dark,"foregroundPixelsAt0_37_100":counts}));
  m.page=Page::Music;m.snapshot["positionSeconds"]=json!(93.24);m.snapshot["isPlaying"]=json!(false);let view=views::render(&m,&mut text,1200.,800.,&ViewState::default());let name=format!("{theme}-{dark}-paused-music.png");gpu.capture(&view.scene,1200,800,&out.join(&name))?;captures.push(name);
 }}
 // Missing art gets a placeholder; late art immediately returns to the same cover painter.
 let art=m.artwork.take();m.page=Page::Home;let missing=views::render(&m,&mut text,1200.,800.,&ViewState::default());gpu.capture(&missing.scene,1200,800,&out.join("missing-cover.png"))?;m.artwork=art;let late=views::render(&m,&mut text,1200.,800.,&ViewState::default());gpu.capture(&late.scene,1200,800,&out.join("late-cover.png"))?;
 std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&json!({"success":true,"sourceCoverUsed":real.is_some(),"realCoverOrDemoNeverFixedPlaceholder":true,"sourceColourAndOrientation":true,"artworkReplacement":true,"nearestSampling":true,"meters":meters,"captures":captures})).unwrap()).map_err(|e|e.to_string())?;Ok(())
}
