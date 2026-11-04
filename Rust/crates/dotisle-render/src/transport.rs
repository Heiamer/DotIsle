//! Transport-row themes. Rectangles, icon boxes and hit targets stay identical; only the
//! background treatment differs, so switching a theme cannot move or remove a control.
use vello::{Scene,kurbo::{Affine,Circle,Rect,RoundedRect,Stroke},peniko::Fill};

#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum TransportTheme{Reef,Ripple,Sand,Mist,Glaze,InkIsle,Tide,Whitespace,Halo,Layered}
impl TransportTheme{
    pub const ALL:[Self;10]=[Self::Reef,Self::Ripple,Self::Sand,Self::Mist,Self::Glaze,Self::InkIsle,Self::Tide,Self::Whitespace,Self::Halo,Self::Layered];
    pub fn key(self)->&'static str{match self{Self::Reef=>"Reef",Self::Ripple=>"Ripple",Self::Sand=>"Sand",Self::Mist=>"Mist",Self::Glaze=>"Glaze",Self::InkIsle=>"InkIsle",Self::Tide=>"Tide",Self::Whitespace=>"Whitespace",Self::Halo=>"Halo",Self::Layered=>"Layered"}}
    pub fn label(self)->&'static str{match self{Self::Reef=>"礁石",Self::Ripple=>"涟漪",Self::Sand=>"细砂",Self::Mist=>"晨雾",Self::Glaze=>"琉璃",Self::InkIsle=>"墨屿",Self::Tide=>"双潮",Self::Whitespace=>"留白",Self::Halo=>"光环",Self::Layered=>"叠影"}}
    pub fn from_setting(s:&str)->Self{Self::ALL.into_iter().find(|t|t.key()==s).unwrap_or(Self::Reef)}
}
/// All colors are ARGB. `accent` is the component color, `on_accent` a readable ink for it,
/// `control` the card-level icon color, and `tonal` the secondary container.
#[derive(Clone,Copy)]
pub struct Colors{pub accent:u32,pub on_accent:u32,pub control:u32,pub tonal:u32,pub ink_fill:u32}

fn color(argb:u32)->vello::peniko::Color{vello::peniko::Color::from_rgba8((argb>>16)as u8,(argb>>8)as u8,argb as u8,(argb>>24)as u8)}
fn alpha(argb:u32,a:f64)->vello::peniko::Color{color(((a.clamp(0.,1.)*255.)as u32)<<24|(argb&0xffffff))}
fn readable(argb:u32)->u32{let r=((argb>>16)&255)as f64;let g=((argb>>8)&255)as f64;let b=(argb&255)as f64;if (0.299*r+0.587*g+0.114*b)/255.>0.55{0xff17201d}else{0xffffffff}}
fn fill(scene:&mut Scene,r:Rect,radius:f64,c:vello::peniko::Color){scene.fill(Fill::NonZero,Affine::IDENTITY,c,None,&RoundedRect::from_rect(r,radius));}
fn outline(scene:&mut Scene,r:Rect,radius:f64,c:vello::peniko::Color,w:f64){scene.stroke(&Stroke::new(w),Affine::IDENTITY,c,None,&RoundedRect::from_rect(r,radius));}

/// Paints the background for one transport control and returns the icon color for the play
/// button. `morph` is the existing paused→playing shape value; only Classic uses it.
pub fn background(scene:&mut Scene,id:i32,r:Rect,theme:TransportTheme,c:Colors,morph:f64)->u32{
    let rounded=12.0_f64.min(r.height()/2.);
    match id{
        4=>match theme{
            TransportTheme::Reef=>{let radius=r.height()*(0.5-0.22*morph.clamp(0.,1.));fill(scene,r,radius,color(c.accent));c.on_accent}
            TransportTheme::Ripple=>{fill(scene,r,r.height()/2.,color(c.accent));c.on_accent}
            TransportTheme::Sand=>{outline(scene,r,rounded,alpha(c.accent,0.72),1.1);c.accent}
            TransportTheme::Mist=>{fill(scene,r,rounded,alpha(c.accent,0.20));outline(scene,r,rounded,alpha(c.accent,0.38),1.);c.accent}
            TransportTheme::Glaze=>{fill(scene,r,rounded,alpha(c.accent,0.30));outline(scene,r,rounded,alpha(c.accent,0.62),1.);c.accent}
            TransportTheme::InkIsle=>{fill(scene,r,rounded,color(c.ink_fill));readable(c.ink_fill)}
            TransportTheme::Tide=>{fill(scene,r,rounded,color(c.accent));c.on_accent}
            TransportTheme::Whitespace=>c.accent,
            TransportTheme::Halo=>{scene.stroke(&Stroke::new(2.2),Affine::IDENTITY,color(c.accent),None,&Circle::new((r.center().x,r.center().y),r.height()/2.-2.));c.accent}
            TransportTheme::Layered=>{fill(scene,Rect::new(r.x0,r.y0+2.,r.x1,r.y1+2.),rounded,alpha(0x000000,0.16));fill(scene,r,rounded,color(c.accent));c.on_accent}
        },
        6=>{
            match theme{
                TransportTheme::Reef=>{fill(scene,r,12.,alpha(0x000000,0.19));}
                TransportTheme::Ripple|TransportTheme::Mist|TransportTheme::Glaze|TransportTheme::InkIsle|TransportTheme::Tide|TransportTheme::Layered=>{fill(scene,r,r.height()/2.,alpha(c.tonal,0.55));}
                _=>{}
            }
            c.control
        }
        3|5 if theme==TransportTheme::Tide=>{fill(scene,r,r.height()/2.,alpha(c.tonal,0.45));c.control}
        _=>c.control,
    }
}
