//! Album artwork is displayed directly. Theme styling never changes its pixels.
use std::sync::Arc;
use image::RgbaImage;
use vello::peniko::{Blob,ImageData,ImageFormat,ImageAlphaType};
pub fn data(rgba:RgbaImage)->ImageData{let(width,height)=rgba.dimensions();ImageData{data:Blob::new(Arc::new(rgba.into_raw())),format:ImageFormat::Rgba8,alpha_type:ImageAlphaType::Alpha,width,height}}
pub fn verify()->Result<(),String>{
 let source=RgbaImage::from_fn(64,64,|x,y|image::Rgba([(x*3)as u8,(y*3)as u8,(x^y)as u8,if x==0{0}else{255}]));
 let expected=source.as_raw().clone();let artwork=crate::model::Artwork::new(image::DynamicImage::ImageRgba8(source));
 if artwork.image.width!=64||artwork.image.height!=64||artwork.image.data.as_ref()!=expected.as_slice(){return Err("theme processing altered original artwork".into())}
 let next=crate::model::Artwork::new(image::DynamicImage::ImageRgba8(RgbaImage::from_pixel(64,64,image::Rgba([1,2,3,255]))));
 if next.image.data.as_ref()==artwork.image.data.as_ref(){return Err("new artwork retained previous pixels".into())}Ok(())
}
