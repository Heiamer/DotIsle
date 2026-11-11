//! Local, per-theme wallpaper with cached image processing and opaque content surfaces.
use std::{path::Path,sync::Arc};
use vello::{Scene,kurbo::{Affine,Rect},peniko::{ImageData,ImageFormat,ImageAlphaType,Blob,ImageBrush,Mix}};
use crate::{draw::{self,rect,Icon},widgets::Ui,model::{Model,Action}};
struct Tile{image:ImageData,origin:(u32,u32),core:(u32,u32,u32,u32)}
#[derive(Default)]pub struct Cache{opaque:bool,tiles:Vec<Tile>,width:u32,height:u32,pub image:Option<ImageData>,pub error:Option<String>,path:String,blur:f64,source:Option<image::RgbaImage>}
fn decode(path:&Path)->Result<image::RgbaImage,String>{
 let meta=std::fs::metadata(path).map_err(|_|"找不到图片，请重新选择".to_string())?;
 if meta.len()>64*1024*1024{return Err("图片超过 64 MB，请选择较小的图片".into())}
 let mut reader=image::ImageReader::open(path).map_err(|_|"无法打开图片")?.with_guessed_format().map_err(|_|"无法识别图片格式")?;
 let mut limits=image::Limits::default();limits.max_image_width=Some(16384);limits.max_image_height=Some(16384);limits.max_alloc=Some(256*1024*1024);reader.limits(limits);
 let image=reader.decode().map_err(|_|"图片损坏或格式不支持，请选择 PNG、JPEG 或 BMP")?;
 if image.width()==0||image.height()==0{return Err("图片尺寸无效".into())}
 Ok(if image.width()>2560||image.height()>1600{image.thumbnail(2560,1600).to_rgba8()}else{image.to_rgba8()})
}
fn frosted(source:&image::RgbaImage,blur:f64)->image::RgbaImage{
 if blur<=0.{return source.clone()}
 let mut pixels=source.clone();
 // Premultiply before filtering, then return straight alpha for the image brush.
 for p in pixels.pixels_mut(){for c in 0..3{p[c]=((p[c]as u32*p[3]as u32+127)/255)as u8;}}
 let mut pixels=image::imageops::fast_blur(&pixels,blur as f32);
 for p in pixels.pixels_mut(){if p[3]>0{for c in 0..3{p[c]=((p[c]as u32*255)/p[3]as u32).min(255)as u8;}}}
 pixels
}
impl Model{
 pub fn refresh_background(&mut self){
  let bg=&self.prefs.backgrounds[crate::nothing::slot(self)as usize];
  if bg.path.is_empty()||!bg.enabled{self.background=Cache::default();return}
  if self.background.path!=bg.path||self.background.source.is_none(){
   match decode(Path::new(&bg.path)){Ok(im)=>{self.background.source=Some(im);self.background.path=bg.path.clone();self.background.blur=-1.;self.background.error=None;},Err(e)=>{self.background=Cache{error:Some(e),..Default::default()};return}}
  }
  if self.background.blur!=bg.blur{
   let im=frosted(self.background.source.as_ref().unwrap(),bg.blur);let(w,h)=im.dimensions();
   self.background.opaque=im.pixels().all(|p|p[3]==255);self.background.tiles.clear();self.background.width=w;self.background.height=h;
   // The hybrid atlas accepts <=1024px textures. Keep photo detail in padded tiles.
   for y in (0..h).step_by(1000){for x in (0..w).step_by(1000){
    let(x0,y0,x1,y1)=(x.saturating_sub(1),y.saturating_sub(1),(x+1001).min(w),(y+1001).min(h));
    let tile=image::imageops::crop_imm(&im,x0,y0,x1-x0,y1-y0).to_image();
    self.background.tiles.push(Tile{image:ImageData{data:Blob::new(Arc::new(tile.into_raw())),format:ImageFormat::Rgba8,alpha_type:ImageAlphaType::Alpha,width:x1-x0,height:y1-y0},origin:(x0,y0),core:(x,y,(x+1000).min(w),(y+1000).min(h))});
   }}
   self.background.image=self.background.tiles.first().map(|t|t.image.clone());self.background.blur=bg.blur;
  }
 }
 pub fn import_background(&mut self,path:&Path)->Result<(),String>{
  let image=decode(path)?;let slot=crate::nothing::slot(self)as usize;
  let dir=self.data.join("Backgrounds");std::fs::create_dir_all(&dir).map_err(|_|"无法保存图片目录")?;
  let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos();
  let destination=dir.join(format!("theme-{slot}-{stamp}.png"));image.save(&destination).map_err(|_|"无法保存背景图片")?;
  let old=self.prefs.clone();self.prefs.backgrounds[slot].path=destination.to_string_lossy().into_owned();self.prefs.backgrounds[slot].enabled=true;
  if let Err(e)=self.persist(){self.prefs=old;let _=std::fs::remove_file(destination);return Err(e)}
  self.refresh_background();Ok(())
 }
 pub fn background_value(&mut self,key:&str,value:f64){
  if !value.is_finite(){return}
  let old=self.prefs.clone();let i=crate::nothing::slot(self)as usize;let bg=&mut self.prefs.backgrounds[i];
  match key{"BackgroundOpacity"=>bg.opacity=(value/100.).clamp(0.,1.),"BackgroundBrightness"=>bg.brightness=(value/100.).clamp(0.,2.),"BackgroundBlur"=>bg.blur=value.clamp(0.,40.),_=>return}
  if let Err(e)=self.persist(){self.prefs=old;self.background.error=Some(e);return}self.refresh_background();
 }
}
fn value(m:&Model,key:&str,fallback:f64)->f64{m.preview_values.get(key).and_then(|v|v.as_f64()).unwrap_or(fallback)}
pub fn paint(scene:&mut Scene,m:&Model,r:Rect){
 let bg=&m.prefs.backgrounds[crate::nothing::slot(m)as usize];if !bg.enabled{return}if m.background.image.is_none(){return}let(width,height)=(m.background.width as f64,m.background.height as f64);
 let opacity=value(m,"BackgroundOpacity",bg.opacity*100.)/100.;let brightness=value(m,"BackgroundBrightness",bg.brightness*100.)/100.;
 scene.push_layer(Mix::Normal,opacity.clamp(0.,1.)as f32,Affine::IDENTITY,&r);
 let fit=if bg.mode=="Contain"{f64::min}else{f64::max};let scale=fit(r.width()/width,r.height()/height);
 let target=rect(r.center().x-width*scale/2.,r.center().y-height*scale/2.,width*scale,height*scale);
 for tile in &m.background.tiles{
  let(a,b,c,d)=tile.core;if !m.background.opaque{scene.push_clip_layer(Affine::IDENTITY,&Rect::new(target.x0+a as f64*scale,target.y0+b as f64*scale,target.x0+c as f64*scale,target.y0+d as f64*scale));}
  let transform=Affine::translate((target.x0+tile.origin.0 as f64*scale,target.y0+tile.origin.1 as f64*scale))*Affine::scale(scale);scene.draw_image(&ImageBrush::from(tile.image.clone()),transform);if !m.background.opaque{scene.pop_layer();}
 }
 if brightness<1.{scene.fill(vello::peniko::Fill::NonZero,Affine::IDENTITY,draw::alpha(0x000000,(1.-brightness)as f32),None,&target);}else if brightness>1.{scene.fill(vello::peniko::Fill::NonZero,Affine::IDENTITY,draw::alpha(0xFFFFFF,(brightness-1.)as f32),None,&target);}
 scene.pop_layer();
 // Protect page titles even when a high-contrast photograph is selected.
 let s=crate::nothing::resolved(m);scene.fill(vello::peniko::Fill::NonZero,Affine::IDENTITY,draw::alpha(s.surface,0.86),None,&rect(r.x0,r.y0,r.width(),148.));
}
pub fn settings(ui:&mut Ui,m:&Model,x:f64,y:&mut f64,w:f64){
 let s=ui.s;let bg=&m.prefs.backgrounds[crate::nothing::slot(m)as usize];let loaded=!bg.path.is_empty();
 ui.panel(rect(x,*y,w,if loaded{488.}else{132.}),s.container);
 ui.label("自定义图片背景",x+24.,*y+16.,16.,s.on_surface,true,w-380.);
 ui.label(&format!("仅用于「{}」 · 支持 PNG / JPEG / BMP",crate::nothing::label(crate::nothing::slot(m))),x+24.,*y+46.,12.,s.on_surface_variant,false,w-380.);
 ui.button(rect(x+w-300.,*y+20.,140.,44.),if loaded{"更换图片"}else{"选择图片"},Some(Icon::Folder),Action::BackgroundPick,false,true);
 ui.button(rect(x+w-148.,*y+20.,124.,44.),"恢复默认",None,Action::BackgroundReset,true,loaded);
 let status=m.background.error.as_deref().unwrap_or(if loaded{"图片已保存在本机，切换主题后仍会保留"}else{"选择自己的图片，预览后调整透明度、明暗与磨砂度"});
 ui.label(status,x+24.,*y+90.,12.,if m.background.error.is_some(){s.error}else{s.on_surface_variant},false,w-48.);
 if !loaded{*y+=148.;return}
 ui.toggle(rect(x+24.,*y+120.,w-48.,70.),"使用图片背景","关闭后保留图片与参数",bg.enabled,Action::BackgroundEnable(!bg.enabled));
 let cw=(w-56.)/2.;for (i,(label,contain))in [("填满 · 居中裁切",false),("适应 · 完整显示",true)].iter().enumerate(){ui.chip(rect(x+24.+i as f64*(cw+8.),*y+200.,cw,40.),label,(bg.mode=="Contain")==*contain,Action::BackgroundMode(*contain));}
 for (i,(title,key,v,max,unit))in [("图片透明度","BackgroundOpacity",bg.opacity*100.,100.,"%"),("图片明暗","BackgroundBrightness",bg.brightness*100.,200.,"%"),("图片磨砂度","BackgroundBlur",bg.blur,40.,"")].iter().enumerate(){
  let yy=*y+256.+i as f64*72.;let v=value(m,key,*v);ui.label(title,x+24.,yy,14.,s.on_surface,false,w-200.);ui.trailing_label(&format!("{v:.0}{unit}"),x+w-24.,yy,12.,s.on_surface_variant,false,88.);
  let r=rect(x+24.,yy+30.,w-48.,28.);crate::settings_slider::render(ui,r,title,key,v,0.,*max);
 }
 *y+=504.;
}

/// Persistence, image import, processing, error recovery, and cross-theme isolation.
pub fn verify(out:&Path)->Result<(),String>{
 let data=out.join(format!("wallpaper-data-{}",std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_nanos()));let mut m=Model::new(data.clone(),true);m.tutorial=None;m.tutorial_checked=true;
 m.dispatch(Action::ControlTheme(1));let file=out.join("wallpaper-source.png");std::fs::write(&file,include_bytes!("../../../../Assets/DemoCover.png")).map_err(|e|e.to_string())?;
 m.import_background(&file)?;m.dispatch(Action::BackgroundValue("BackgroundOpacity",65.));m.dispatch(Action::BackgroundValue("BackgroundBrightness",75.));m.dispatch(Action::BackgroundValue("BackgroundBlur",18.));
 std::fs::remove_file(&file).map_err(|e|e.to_string())?;
 let restored=Model::new(data.clone(),true);if restored.background.image.is_none()||restored.prefs.backgrounds[1].opacity!=0.65||restored.prefs.backgrounds[1].blur!=18.{return Err("background did not survive restart/source removal".into())}
 let old=m.prefs.backgrounds[1].path.clone();let invalid=out.join("invalid-image.png");std::fs::write(&invalid,b"bad image").map_err(|e|e.to_string())?;if m.import_background(&invalid).is_ok()||m.prefs.backgrounds[1].path!=old{return Err("invalid image replaced good background".into())}
 m.dispatch(Action::ControlTheme(2));if m.background.image.is_some(){return Err("background leaked between themes".into())}
 m.import_background(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Assets/DemoCover.png"))?;
 m.dispatch(Action::BackgroundValue("BackgroundOpacity",55.));m.dispatch(Action::BackgroundValue("BackgroundBlur",12.));
 let mut text=crate::draw::Text::new();let mut gpu=crate::gpu::Gpu::new();
 for slot in 0..crate::nothing::SLOTS.len()as u8{
  m.dispatch(Action::ControlTheme(slot));if slot==0||slot>=3{m.import_background(&Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../Assets/DemoCover.png"))?;}
  for dark in [false,true]{m.prefs.dark=dark;m.page=crate::model::Page::Home;m.toast=None;m.scroll=0.;let v=crate::views::render(&m,&mut text,1200.,800.,&Default::default());let name=format!("wallpaper-{}-{}.png",crate::nothing::SLOTS[slot as usize],if dark{"dark"}else{"light"});gpu.capture(&v.scene,1200,800,&out.join(&name))?;
   m.prefs.backgrounds[slot as usize].enabled=false;let plain=crate::views::render(&m,&mut text,1200.,800.,&Default::default());let baseline=out.join("wallpaper-cover-baseline.png");gpu.capture(&plain.scene,1200,800,&baseline)?;m.prefs.backgrounds[slot as usize].enabled=true;
   let a=image::open(out.join(&name)).map_err(|e|e.to_string())?.to_rgba8();let b=image::open(&baseline).map_err(|e|e.to_string())?.to_rgba8();
   for (x,y) in [(610,310),(650,350),(700,390)]{let pa=a.get_pixel(x,y);let pb=b.get_pixel(x,y);if pa.0.iter().zip(pb.0.iter()).any(|(a,b)|(*a as i16-*b as i16).abs()>3){return Err("wallpaper atlas corrupted the album cover".into())}}
}
  m.page=crate::model::Page::Settings;m.settings_tab=0;m.scroll=300.;let view=crate::views::render(&m,&mut text,1200.,800.,&Default::default());
  for key in ["BackgroundOpacity","BackgroundBrightness","BackgroundBlur"]{if !view.hits.iter().any(|h|matches!(&h.action,Action::RangeHandle(k,_,_,_)if *k==key)){return Err(format!("missing background control {key}"))}}
  gpu.capture(&view.scene,1200,800,&out.join(format!("wallpaper-settings-{}.png",crate::nothing::SLOTS[slot as usize])))?;
  m.dispatch(Action::BackgroundEnable(false));if m.background.image.is_some(){return Err("disabled background retained visible texture".into())}m.dispatch(Action::BackgroundEnable(true));
 }
 m.dispatch(Action::BackgroundReset);if m.background.image.is_some()||m.prefs.backgrounds[crate::nothing::slot(&m)as usize].enabled{return Err("background reset failed".into())}
 std::fs::write(out.join("background-report.json"),serde_json::to_vec_pretty(&serde_json::json!({"success":true,"per_theme":true,"import_is_local_copy":true,"source_can_be_removed":true,"corrupt_image_keeps_previous":true,"restart_restores_image_and_controls":true,"opacity_brightness_blur_controls":true,"disable_and_reset":true})).unwrap()).map_err(|e|e.to_string())?;Ok(())
}
