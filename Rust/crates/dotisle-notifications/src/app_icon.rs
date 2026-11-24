// Use the Windows Shell registration for desktop applications whose WinRT
// logo stream is unavailable. Work stays off the rendering thread.
use windows::{core::{PCWSTR,Result},Win32::{Foundation::SIZE,Graphics::Gdi::*,UI::Shell::*}};

// A bundled recognizable fallback also covers missing Shell registrations.
pub fn wechat_icon()->Vec<u8>{
 static ICON:std::sync::OnceLock<Vec<u8>>=std::sync::OnceLock::new();ICON.get_or_init(||{
  let ellipse=|x:f32,y:f32,cx:f32,cy:f32,rx:f32,ry:f32|((x-cx)/rx).powi(2)+((y-cy)/ry).powi(2)<=1.;
  let pixels=image::RgbaImage::from_fn(256,256,|x,y|{
   let (x,y)=((x as f32+0.5)/4.,(y as f32+0.5)/4.);let green=[7,193,96,255];let white=[255,255,255,255];
   let dx=(x-32.).abs()-22.;let dy=(y-32.).abs()-22.;if dx.max(0.).powi(2)+dy.max(0.).powi(2)>100.{return image::Rgba([0,0,0,0])}
   let mut color=green;
   if ellipse(x,y,25.,26.,18.,14.)||(y>35.&&y<43.&&x>12.&&x<24.&&(x+y)<56.){color=white;}
   if ellipse(x,y,20.,23.,2.,2.)||ellipse(x,y,31.,23.,2.,2.){color=green;}
   if ellipse(x,y,42.,40.,16.,13.){color=green;}
   if ellipse(x,y,42.,40.,14.5,11.5)||(y>47.&&y<53.&&x>44.&&x<52.&&(x-y)>-2.){color=white;}
   if ellipse(x,y,37.,37.,1.7,1.7)||ellipse(x,y,47.,37.,1.7,1.7){color=green;}
   image::Rgba(color)
  });let small=image::imageops::resize(&pixels,64,64,image::imageops::FilterType::Lanczos3);let mut out=std::io::Cursor::new(vec![]);let _=image::DynamicImage::ImageRgba8(small).write_to(&mut out,image::ImageFormat::Png);out.into_inner()
 }).clone()
}

pub fn shell_icon(id:&str)->Option<Vec<u8>>{unsafe{
    let path=if std::path::Path::new(id).is_absolute(){id.to_owned()}else{format!("shell:AppsFolder\\{id}")};let path=path.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let factory:IShellItemImageFactory=SHCreateItemFromParsingName(PCWSTR(path.as_ptr()),None).ok()?;
    let bitmap=factory.GetImage(SIZE{cx:64,cy:64},SIIGBF_ICONONLY|SIIGBF_BIGGERSIZEOK).ok()?;
    let result=bitmap_png(bitmap);let _=DeleteObject(bitmap.into());result.ok()
}}

unsafe fn bitmap_png(bitmap:HBITMAP)->Result<Vec<u8>>{
    let bad=||windows::core::Error::from_hresult(windows::core::HRESULT(0x80070057u32 as i32));
    let mut info=BITMAP::default();
    if GetObjectW(bitmap.into(),std::mem::size_of::<BITMAP>()as i32,Some((&mut info as *mut BITMAP).cast()))==0{return Err(bad())}
    let width=info.bmWidth;let height=info.bmHeight.abs();
    if width<=0||height<=0||width>1024||height>1024{return Err(bad())}
    let mut header=BITMAPINFO::default();header.bmiHeader=BITMAPINFOHEADER{biSize:std::mem::size_of::<BITMAPINFOHEADER>()as u32,biWidth:width,biHeight:-height,biPlanes:1,biBitCount:32,biCompression:BI_RGB.0,..Default::default()};
    let mut pixels=vec![0u8;(width*height*4)as usize];let dc=GetDC(None);
    let rows=GetDIBits(dc,bitmap,0,height as u32,Some(pixels.as_mut_ptr().cast()),&mut header,DIB_RGB_COLORS);let _=ReleaseDC(None,dc);
    if rows!=height{return Err(bad())}
    let alpha=pixels.chunks_exact(4).any(|p|p[3]!=0);
    for p in pixels.chunks_exact_mut(4){p.swap(0,2);if !alpha{p[3]=255}else if p[3]>0{let a=p[3]as u32;for c in &mut p[..3]{*c=((*c as u32*255+a/2)/a).min(255)as u8;}}}
    let image=image::RgbaImage::from_raw(width as u32,height as u32,pixels).ok_or_else(bad)?;
    let mut output=std::io::Cursor::new(Vec::new());image::DynamicImage::ImageRgba8(image).write_to(&mut output,image::ImageFormat::Png).map_err(|_|bad())?;Ok(output.into_inner())
}
