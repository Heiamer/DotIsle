//! Cached, cover-derived block art. Conversion happens once per artwork update.
use std::sync::Arc;
use image::{DynamicImage,RgbaImage,imageops::FilterType};
use vello::{Scene,kurbo::{Affine,Rect},peniko::{Blob,ImageData,ImageFormat,ImageAlphaType,ImageBrush,ImageQuality}};

pub struct Covers{pub terminal:ImageData,pub pixel:ImageData}
pub fn data(rgba:RgbaImage)->ImageData{let(width,height)=rgba.dimensions();ImageData{data:Blob::new(Arc::new(rgba.into_raw())),format:ImageFormat::Rgba8,alpha_type:ImageAlphaType::Alpha,width,height}}
impl Covers{
 pub fn new(source:&DynamicImage)->Self{
  let sample=source.resize_to_fill(32,32,FilterType::Triangle).to_rgba8();
  // RGB565 is the actual 16-bit colour representation, with no interpolated edges.
  let mut pixel=sample.clone();
  for p in pixel.pixels_mut(){p[0]=((p[0]as u16>>3)*255/31)as u8;p[1]=((p[1]as u16>>2)*255/63)as u8;p[2]=((p[2]as u16>>3)*255/31)as u8;}
  // Each 6x12 terminal cell contains a coloured upper/lower half-block pair (▀/▄).
  // Transparent gutters expose the active theme rather than baking in one background.
  let terminal=RgbaImage::from_fn(192,192,|x,y|{
   let mut p=*sample.get_pixel(x/6,(y/12)*2+u32::from(y%12>=6));
   if x%6==5||y%12==11{p[3]=0;}
   p
  });
  Self{terminal:data(terminal),pixel:data(pixel)}
 }
}
pub fn paint(scene:&mut Scene,image:&ImageData,r:Rect){
 let side=r.width().min(r.height()).floor();let square=Rect::new((r.center().x-side/2.).round(),(r.center().y-side/2.).round(),(r.center().x-side/2.).round()+side,(r.center().y-side/2.).round()+side);
 let transform=Affine::translate((square.x0,square.y0))*Affine::scale_non_uniform(side/image.width as f64,side/image.height as f64);
 scene.draw_image(&ImageBrush::from(image.clone()).with_quality(ImageQuality::Low),transform);
}

pub fn verify()->Result<(),String>{
 // Asymmetric colours detect fixed placeholders, wrong row order and theme tinting.
 let source=DynamicImage::ImageRgba8(RgbaImage::from_fn(32,32,|x,y|image::Rgba([if x<16{255}else{0},if y<16{0}else{255},if x>=16{255}else{0},255])));
 let art=Covers::new(&source);let p=art.pixel.data.as_ref();let t=art.terminal.data.as_ref();
 if p[..4]!=[255,0,0,255]||p[(31*4)..(32*4)]!=[0,0,255,255]||p[(31*32*4)..(31*32*4+4)]!=[255,255,0,255]{return Err("pixel cover lost source colours or orientation".into())}
 if t[..4]!=[255,0,0,255]||t[5*4+3]!=0||t[(191*192+191)*4+3]!=0{return Err("terminal half-block colours/gutters differ from source".into())}
 let next=Covers::new(&DynamicImage::ImageRgba8(RgbaImage::from_pixel(32,32,image::Rgba([0,255,0,255]))));
 if art.pixel.data.as_ref()==next.pixel.data.as_ref()||art.terminal.data.as_ref()==next.terminal.data.as_ref(){return Err("artwork replacement retained the old mosaic".into())}
 Ok(())
}
