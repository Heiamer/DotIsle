//! Rossela wordmark shared with the Go center and README as vector outlines.
use std::sync::OnceLock;
use vello::{Scene,kurbo::{Affine,BezPath,Rect},peniko::{Color,Fill}};
mod outlines { include!("wordmark_outlines.rs"); }
pub const WRITE_SECONDS:f64=1.;
pub const HOLD_SECONDS:f64=0.28;
pub const EXIT_SECONDS:f64=0.22;
pub const TOTAL_SECONDS:f64=WRITE_SECONDS+HOLD_SECONDS+EXIT_SECONDS;
fn paths()->&'static Vec<BezPath>{
    static PATHS:OnceLock<Vec<BezPath>>=OnceLock::new();
    PATHS.get_or_init(||outlines::OUTLINES.iter().map(|s|BezPath::from_svg(s).expect("Rossela wordmark outline")).collect())
}
pub fn duration(reduced:bool)->f64{if reduced{0.24}else{TOTAL_SECONDS}}
pub fn ink(elapsed:f64,reduced:bool)->(f64,f64){
    if elapsed>=duration(reduced){return (1.,0.)}
    if reduced{return (1.,(1.-((elapsed-0.08)/0.16).clamp(0.,1.)).clamp(0.,1.))}
    ((elapsed/WRITE_SECONDS).clamp(0.,1.),1.-((elapsed-WRITE_SECONDS-HOLD_SECONDS)/EXIT_SECONDS).clamp(0.,1.))
}
pub fn draw(scene:&mut Scene,bounds:Rect,progress:f64,ink:Color){
    if !progress.is_finite()||progress<=0.{return}
    let scale=(bounds.width()/outlines::WIDTH).min(bounds.height()/outlines::HEIGHT);
    let tr=Affine::translate((bounds.center().x-outlines::WIDTH*scale/2.,bounds.center().y-outlines::HEIGHT*scale/2.))*Affine::scale(scale);
    if progress<1.{scene.push_clip_layer(tr,&Rect::new(0.,0.,outlines::WIDTH*progress,outlines::HEIGHT));}
    for path in paths(){scene.fill(Fill::NonZero,tr,ink,None,path);}
    if progress<1.{scene.pop_layer();}
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn rossela_wordmark_has_seven_valid_glyphs(){assert_eq!(paths().len(),7);for p in paths(){assert!(!p.elements().is_empty());}}
    #[test]fn reduced_motion_has_a_short_exit(){assert_eq!(ink(0.,true).0,1.);assert!(duration(true)<0.3);assert_eq!(ink(duration(false),false),(1.,0.));assert_eq!(ink(0.,false),(0.,1.));}
}
