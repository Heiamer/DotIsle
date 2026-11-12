// Transport colours are paired against their actual filled target, including state layers.
use crate::monet::contrast;
use material_colors::{color::Argb,hct::Hct,palette::TonalPalette};
pub fn on_surface(surface:u32,preferred:u32)->u32{if contrast(surface,preferred)>=4.5{preferred&0xffffff}else if contrast(surface,0xffffff)>contrast(surface,0){0xffffff}else{0}}
/// Unfilled controls keep their source hue against the composited reading plane.
pub fn tinted_on_surfaces(surfaces:&[u32],preferred:u32)->u32{
 let surfaces=if surfaces.is_empty(){&[0x1b1f25][..]}else{surfaces};let preferred=preferred&0xffffff;
 let quality=|ink:u32|{let base=surfaces.iter().map(|bg|contrast(ink,*bg)).fold(f64::INFINITY,f64::min);let state=surfaces.iter().flat_map(|bg|[0.08,0.12].map(move|a|contrast(ink,mix(*bg,ink,a)))).fold(f64::INFINITY,f64::min);(base/4.5).min(state/3.)};
 if quality(preferred)>=1.{return preferred}
 let hct=Hct::new(Argb::from_u32(0xff000000|preferred));let palette=TonalPalette::from_hue_and_chroma(hct.get_hue(),hct.get_chroma());
 let mut chosen=preferred;let mut best=quality(preferred);let mut distance=f64::INFINITY;
 for tone in 10..=98{let c=palette.tone(tone);let ink=(c.red as u32)<<16|(c.green as u32)<<8|c.blue as u32;let q=quality(ink);let d=(tone as f64-hct.get_tone()).abs();if q>=1.&&(best<1.||d<distance)||best<1.&&q>best{chosen=ink;best=q;distance=d;}}
 chosen
}
#[derive(Clone,Copy,Debug,serde::Serialize,serde::Deserialize)]pub struct SkipColors{pub background:u32,pub foreground:u32}
fn mix(a:u32,b:u32,t:f64)->u32{let c=|s:u32|{let a=((a>>s)&255)as f64;let b=((b>>s)&255)as f64;(a+(b-a)*t).round()as u32};c(16)<<16|c(8)<<8|c(0)}
pub fn skip_colors(surface:u32,preferred:u32)->SkipColors{
 let background=mix(surface,preferred,0.14);
 let worst=|ink:u32|[0.,0.08,0.12,0.16].into_iter().map(|a|contrast(ink,mix(background,ink,a))).fold(f64::INFINITY,f64::min);
 let foreground=if contrast(preferred,background)>=4.5&&worst(preferred)>=3.{preferred&0xffffff}else if contrast(0xffffff,background)>=4.5&&(contrast(0,background)<4.5||worst(0xffffff)>worst(0)){0xffffff}else{0};
 SkipColors{background,foreground}
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn olive_screenshot_keeps_tinted_controls_instead_of_black(){let backgrounds=[0x524e2e,0x646141,0x62653f];let ink=tinted_on_surfaces(&backgrounds,0xbfe957);assert_ne!(ink,0);for bg in backgrounds{assert!(contrast(ink,bg)>=4.5);for a in [0.,0.08,0.12,0.16]{assert!(contrast(ink,mix(bg,ink,a))>=3.);}}}
 #[test]fn extreme_covers_and_missing_artwork_keep_legible_hued_controls(){for preferred in [0xc5eb5d,0xffffff,0x010101,0xffff00,0xff00ff,0x00ff00,0x0000ff]{for backgrounds in [[0x646141,0x524e2e],[0x1b1f25,0x0c0f14],[0xe6dfb5,0xd7d0a6]]{let ink=tinted_on_surfaces(&backgrounds,preferred);assert_ne!(ink,0);for bg in backgrounds{assert!(contrast(ink,bg)>=4.45,"{preferred:06x}/{bg:06x}/{ink:06x}");for a in [0.,0.08,0.12,0.16]{assert!(contrast(ink,mix(bg,ink,a))>=2.95);}}}}}
 #[test]fn screenshot_regressions_and_custom_colours_stay_readable(){for(bg,ink)in[(0x1d2e33,0x41494c),(0x7d2b85,0x514158),(0x351a35,0xf6a9fb),(0xf5e5dc,0xffeeee),(0x050505,0x101010)]{let c=skip_colors(bg,ink);for state in [0.,0.08,0.12,0.16]{assert!(contrast(c.foreground,mix(c.background,c.foreground,state))>=3.,"{bg:06x}/{ink:06x}/{state}");}assert!(contrast(c.foreground,c.background)>=4.5);}}
 #[test]fn material_and_cover_colours_keep_legible_skip_targets(){for seed in [0,0xffffff,0xff00ff,0xffff00,0x00ff00,0x0000ff,0x1d2e33,0x9135a7]{for dark in [false,true]{for content in [false,true]{let r=crate::dynamic_color::roles(seed,dark,content,false);let c=skip_colors(r.primary_container,r.on_primary_container);assert!(contrast(c.foreground,c.background)>=4.5,"{seed:06x}/{dark}/{content}");for state in [0.,0.08,0.12,0.16]{assert!(contrast(c.foreground,mix(c.background,c.foreground,state))>=3.);}}}}}
}
