//! Nothing design-system theme for the control centre.
//! Only colour, material and icon ink change here; every rectangle comes from the layout code.
use std::sync::{Mutex,OnceLock};
use std::time::Instant;
use vello::{Scene,kurbo::{Affine,Point,Shape},peniko::Fill};
use dotisle_domain::monet::Scheme;
use dotisle_render::draw::Icon;
use crate::model::Model;

pub const ACCENT:u32=0xD71921;
pub const ORIGINAL:&str="Original";
pub const NOTHING:&str="Nothing";
pub const SLOTS:[&str;5]=[ORIGINAL,NOTHING,"Porcelain","Terminal","Pixel"];
/// Main grid for 20dp and above: 16×16 cells, dots at cell centres.
pub const GRID:f64=16.;
/// Below 16dp the same drawing is sampled on 8×8 cells so the ink can still be read.
pub const SMALL_GRID:f64=8.;
/// Sizes at or below this switch to the compact grid; 20/24/32 stay on the main grid.
pub const SMALL_MAX:f64=16.5;
/// Dot diameter relative to a cell edge. Identical for every icon and both grids.
pub const DOT_RATIO:f64=0.55;
/// Theme colour transition length; the brief caps it at 200 ms.
pub const TRANSITION_MS:f64=200.;

pub fn active(m:&Model)->bool{m.prefs.control_center_theme.as_str()==NOTHING}
pub fn name(m:&Model)->&'static str{SLOTS[slot(m)as usize]}
pub fn slot(m:&Model)->u8{match m.prefs.control_center_theme.as_str(){NOTHING=>1,"Porcelain"=>2,"Terminal"=>3,"Pixel"=>4,_=>0}}
pub fn slot_from_fraction(f:f64)->u8{(f.clamp(0.,1.)*4.).round()as u8}
pub fn label(slot:u8)->&'static str{["默认","Nothing","云瓷","终端","像素"][slot.min(4)as usize]}

// ---- theme transition --------------------------------------------------------------------
struct Transition{from:u8,from_scheme:Scheme,dark:bool,start:Instant,progress:Option<f64>}
static TRANS:OnceLock<Mutex<Option<Transition>>>=OnceLock::new();
fn trans()->&'static Mutex<Option<Transition>>{TRANS.get_or_init(||Mutex::new(None))}
/// Start a colour/outline/text-colour transition from the palette in force before the
/// switch. Nothing global is repainted in one step.
pub fn begin_transition(from:u8,from_scheme:Scheme,dark:bool){
    if let Ok(mut g)=trans().lock(){*g=Some(Transition{from,from_scheme,dark,start:Instant::now(),progress:None});}
}
pub fn mix(a:u32,b:u32,t:f64)->u32{
    let t=t.clamp(0.,1.);let c=|n:u32|{let x=((a>>n)&255)as f64;let y=((b>>n)&255)as f64;(x+(y-x)*t).round()as u32};
    c(16)<<16|c(8)<<8|c(0)
}
fn mix_scheme(a:Scheme,b:Scheme,t:f64)->Scheme{Scheme{
    primary:mix(a.primary,b.primary,t),on_primary:mix(a.on_primary,b.on_primary,t),
    primary_container:mix(a.primary_container,b.primary_container,t),on_primary_container:mix(a.on_primary_container,b.on_primary_container,t),
    secondary_container:mix(a.secondary_container,b.secondary_container,t),on_secondary_container:mix(a.on_secondary_container,b.on_secondary_container,t),
    tertiary_container:mix(a.tertiary_container,b.tertiary_container,t),on_tertiary_container:mix(a.on_tertiary_container,b.on_tertiary_container,t),
    surface:mix(a.surface,b.surface,t),container:mix(a.container,b.container,t),high:mix(a.high,b.high,t),
    on_surface:mix(a.on_surface,b.on_surface,t),on_surface_variant:mix(a.on_surface_variant,b.on_surface_variant,t),
    outline:mix(a.outline,b.outline,t),outline_variant:mix(a.outline_variant,b.outline_variant,t),error:mix(a.error,b.error,t),
}}
fn progress()->Option<f64>{
    let Ok(mut g)=trans().lock()else{return None};
    let Some(t)=g.as_mut()else{return None};
    if let Some(p)=t.progress{return Some(p.clamp(0.,1.));}
    let p=(t.start.elapsed().as_secs_f64()*1000./TRANSITION_MS).clamp(0.,1.);
    if p>=1.{g.take();return None}
    Some(p)
}
/// Advance/clamp the transition. Returns true while a frame is still needed.
pub fn finish_transition()->bool{progress().is_some()}
/// Snapshot of the palette in force before the switch while the transition is running.
/// Progress is computed before taking the lock again so this can never self-deadlock.
fn active_from()->Option<(Scheme,f64)>{
    let p=progress()?;
    if p>=1.{return None}
    let g=trans().lock().ok()?;
    g.as_ref().map(|t|(t.from_scheme,p))
}
/// Blend the target palette with the palette that was on screen before the switch.
pub fn position(target:u8)->f64{
    let Some(p)=progress() else{return target as f64};
    let from=trans().lock().ok().and_then(|g|g.as_ref().map(|t|t.from)).unwrap_or(target);
    let e=1.-(1.-p).powi(3);from as f64+(target as f64-from as f64)*e
}
pub fn resolved(m:&Model)->Scheme{if active(m){let mut s=scheme(m.prefs.dark);if m.prefs.nothing_dynamic{let colors=m.scheme();s.primary=colors.primary;s.on_primary=colors.on_primary;s.secondary_container=colors.primary;s.on_secondary_container=colors.on_primary;}s}else if crate::porcelain::active(m){crate::porcelain::scheme(m.prefs.dark)}else if crate::terminal::active(m){crate::terminal::scheme(m.prefs.dark)}else if crate::pixel::active(m){crate::pixel::scheme(m.prefs.dark)}else{m.scheme()}}
pub fn blend_to(target:Scheme)->Scheme{
    match active_from(){Some((from,p))=>mix_scheme(from,target,p),None=>target}
}
/// Headless verification hook: pin the transition at a given fraction.
pub fn set_transition_progress(p:Option<f64>){if let Ok(mut g)=trans().lock(){if let Some(t)=&mut*g{t.progress=p;}}}
pub fn end_transition(){if let Ok(mut g)=trans().lock(){*g=None;}}

/// Nothing palette mapped onto the existing Scheme slot names. Nothing changes geometry.
pub fn scheme(dark:bool)->Scheme{
    if dark{Scheme{
        primary:0xFFFFFF,on_primary:0x000000,primary_container:0x111111,on_primary_container:0xE8E8E8,
        secondary_container:0xFFFFFF,on_secondary_container:0x000000,tertiary_container:0x111111,on_tertiary_container:0xE8E8E8,
        surface:0x000000,container:0x111111,high:0x1A1A1A,
        on_surface:0xE8E8E8,on_surface_variant:0x999999,outline:0x222222,outline_variant:0x333333,error:ACCENT,
    }}else{Scheme{
        primary:0x000000,on_primary:0xFFFFFF,primary_container:0xFFFFFF,on_primary_container:0x1A1A1A,
        secondary_container:0x000000,on_secondary_container:0xFFFFFF,tertiary_container:0xFFFFFF,on_tertiary_container:0x1A1A1A,
        surface:0xF5F5F5,container:0xFFFFFF,high:0xF0F0F0,
        on_surface:0x1A1A1A,on_surface_variant:0x666666,outline:0xD8D8D8,outline_variant:0xCCCCCC,error:ACCENT,
    }}
}
/// Dark switch-off track lifted one step (#333333 -> #3A3A3A) and thumb #666666 -> #888888;
/// #888 on #3A is 3.21:1, and the off thumb stays darker than the white on track.
pub fn track_off(dark:bool)->u32{
    let base=if dark{0x3A3A3A}else{0xCCCCCC};
    match active_from(){Some((from,p))=>mix(from.high,base,p),None=>base}
}
pub fn track_thumb(dark:bool)->u32{
    let base=if dark{0x888888}else{0x999999};
    match active_from(){Some((from,p))=>mix(from.on_surface_variant,base,p),None=>base}
}
pub fn border(dark:bool)->u32{
    let base=if dark{0x222222}else{0xC4C4C4};
    match active_from(){Some((from,p))=>mix(from.outline,base,p),None=>base}
}

// ---- small geometry predicates, all measured in the 16-unit design space ----------------
fn seg(x:f64,y:f64,a:(f64,f64),b:(f64,f64),t:f64)->bool{
    let(dx,dy)=(b.0-a.0,b.1-a.1);let l=dx*dx+dy*dy;
    let s=if l<=0.{0.}else{((x-a.0)*dx+(y-a.1)*dy)/l};
    let s=s.clamp(0.,1.);let(px,py)=(a.0+s*dx,a.1+s*dy);
    ((x-px).powi(2)+(y-py).powi(2)).sqrt()<=t
}
fn dist(x:f64,y:f64,a:(f64,f64),b:(f64,f64))->f64{
    let(dx,dy)=(b.0-a.0,b.1-a.1);let l=dx*dx+dy*dy;
    let s=if l<=0.{0.}else{((x-a.0)*dx+(y-a.1)*dy)/l};let s=s.clamp(0.,1.);
    let(px,py)=(a.0+s*dx,a.1+s*dy);((x-px).powi(2)+(y-py).powi(2)).sqrt()
}
fn disc(x:f64,y:f64,cx:f64,cy:f64,r:f64)->bool{(x-cx).powi(2)+(y-cy).powi(2)<=r*r}
fn ring(x:f64,y:f64,cx:f64,cy:f64,r:f64,t:f64)->bool{let d=((x-cx).powi(2)+(y-cy).powi(2)).sqrt();d>=r-t&&d<=r+t}
fn area(x:f64,y:f64,x0:f64,y0:f64,x1:f64,y1:f64)->bool{x>=x0&&x<=x1&&y>=y0&&y<=y1}
fn poly(x:f64,y:f64,pts:&[(f64,f64)])->bool{
    let mut inside=false;let n=pts.len();let mut j=n-1;
    for i in 0..n{let(xi,yi)=pts[i];let(xj,yj)=pts[j];if((yi>y)!=(yj>y))&&(x<(xj-xi)*(y-yi)/(yj-yi)+xi){inside=!inside;}j=i;}
    inside
}
fn path(x:f64,y:f64,pts:&[(f64,f64)],t:f64)->bool{pts.windows(2).any(|w|seg(x,y,w[0],w[1],t))}
fn rect_line(x:f64,y:f64,x0:f64,y0:f64,x1:f64,y1:f64,t:f64)->bool{path(x,y,&[(x0,y0),(x1,y0),(x1,y1),(x0,y1),(x0,y0)],t)}
/// Continuous rounded-rect outline: distance to the corner-rounded centreline, so the
/// contour never opens at the corners.
fn frame(x:f64,y:f64,x0:f64,y0:f64,x1:f64,y1:f64,rad:f64,t:f64)->bool{
    let(cx0,cy0,cx1,cy1)=(x0+rad,y0+rad,x1-rad,y1-rad);
    let qx=x.clamp(cx0,cx1);let qy=y.clamp(cy0,cy1);
    ((((x-qx).powi(2)+(y-qy).powi(2)).sqrt()-rad)).abs()<=t
}
/// Capsule outline (the Dynamic Island body): centreline is a horizontal segment.
fn pill(x:f64,y:f64,x0:f64,x1:f64,cy:f64,h:f64,t:f64)->bool{(dist(x,y,(x0,cy),(x1,cy))-h).abs()<=t}

/// The six navigation-rail glyphs. They share one 14×14 visual box on the 16×16 lattice
/// (columns/rows 1..14), so no glyph keeps its own Material aspect ratio any more.
pub fn rail(icon:Icon)->bool{matches!(icon,Icon::Home|Icon::Island|Icon::Music|Icon::Focus|Icon::Todo|Icon::Settings)}

/// One shape definition for both grids; the coordinates never change with the resolution.
fn contains(icon:Icon,x:f64,y:f64)->bool{match icon{
    Icon::Play=>poly(x,y,&[(3.,2.),(13.5,8.),(3.,14.)]),
    Icon::Pause=>area(x,y,4.,2.5,6.8,13.5)||area(x,y,9.2,2.5,12.,13.5),
    Icon::Next=>poly(x,y,&[(2.,3.),(9.5,8.),(2.,13.)])||area(x,y,10.5,3.,12.8,13.),
    Icon::Prev=>poly(x,y,&[(14.,3.),(6.5,8.),(14.,13.)])||area(x,y,3.2,3.,5.5,13.),
    Icon::Plus=>seg(x,y,(3.5,8.),(12.5,8.),1.)||seg(x,y,(8.,3.5),(8.,12.5),1.),
    Icon::Check=>path(x,y,&[(3.,8.5),(6.5,11.5),(13.,4.)],1.),
    Icon::Close=>seg(x,y,(4.5,4.5),(11.5,11.5),1.)||seg(x,y,(11.5,4.5),(4.5,11.5),1.),
    Icon::Arrow=>seg(x,y,(4.,8.),(12.5,8.),1.)||path(x,y,&[(8.5,3.8),(12.8,8.),(8.5,12.2)],1.),
    Icon::Home=>path(x,y,&[(1.05,8.95),(8.0,1.55),(14.95,8.95)],0.6)||seg(x,y,(3.35,9.1),(3.35,14.45),0.6)||seg(x,y,(12.65,9.1),(12.65,14.45),0.6)||seg(x,y,(3.35,14.45),(12.65,14.45),0.6),
    Icon::Music=>seg(x,y,(8.15,1.95),(8.15,11.7),0.6)||seg(x,y,(8.15,1.95),(14.45,3.75),0.6)||ring(x,y,4.75,11.9,2.95,0.6),
    Icon::Focus=>ring(x,y,8.,8.,5.35,0.6)||(0..4).any(|k|{let a=k as f64*std::f64::consts::FRAC_PI_2;seg(x,y,(8.+6.*a.cos(),8.+6.*a.sin()),(8.+7.*a.cos(),8.+7.*a.sin()),0.7)})||disc(x,y,8.,8.,1.),
    Icon::Todo=>frame(x,y,1.35,1.35,14.65,14.65,1.6,0.5)||path(x,y,&[(4.5,7.8),(6.9,10.6),(11.4,4.4)],0.5),
    Icon::Settings=>[(2.5,10.0),(8.5,5.0),(14.5,11.0)].iter().any(|(cy,kx)|seg(x,y,(1.5,*cy),(14.5,*cy),0.45)||disc(x,y,*kx,*cy-0.5,1.)),
    Icon::Island=>frame(x,y,1.35,1.35,14.65,14.65,3.6,0.5)||pill(x,y,5.5,10.5,4.6,0.5,0.45),
    Icon::Sun=>disc(x,y,8.,8.,3.2)||(0..8).any(|i|{let a=i as f64*std::f64::consts::TAU/8.;seg(x,y,(8.+4.8*a.cos(),8.+4.8*a.sin()),(8.+6.6*a.cos(),8.+6.6*a.sin()),0.6)}),
    Icon::Moon=>disc(x,y,7.5,8.,4.)&&!disc(x,y,10.4,6.2,3.9),
    Icon::Back=>path(x,y,&[(11.5,3.5),(5.,8.),(11.5,12.5)],1.1),
    Icon::Trash=>seg(x,y,(2.8,5.),(13.2,5.),0.9)||path(x,y,&[(4.,5.),(4.,13.),(12.,13.),(12.,5.)],0.9)||seg(x,y,(6.,2.8),(10.,2.8),0.9)||seg(x,y,(8.,3.),(8.,5.),0.8),
    Icon::Reset=>(ring(x,y,8.,8.,4.6,0.95)&&!(x>9.6&&y<6.4))||poly(x,y,&[(12.,1.4),(15.2,4.6),(10.8,6.6)]),
    Icon::Search=>ring(x,y,7.,7.,3.4,1.1)||seg(x,y,(9.4,9.4),(13.,13.),1.),
    Icon::Volume=>poly(x,y,&[(2.5,6.),(5.5,6.),(9.,2.8),(9.,13.2),(5.5,10.),(2.5,10.)])||(x>9.&&ring(x,y,9.5,8.,3.2,0.75))||(x>11.&&ring(x,y,9.5,8.,5.2,0.7)),
    Icon::Folder=>path(x,y,&[(2.,5.),(6.,5.),(7.2,7.),(14.,7.),(14.,12.5),(2.,12.5),(2.,5.)],0.95),
    Icon::Import=>path(x,y,&[(3.,11.),(3.,14.),(13.,14.),(13.,11.)],0.9)||seg(x,y,(8.,2.8),(8.,9.6),0.9)||poly(x,y,&[(5.3,7.6),(8.,10.6),(10.7,7.6)]),
    Icon::Leaf=>{let(dx,dy)=(x-8.,y-8.);let u=(dx*0.766+dy*0.643)/5.2;let v=(-dx*0.643+dy*0.766)/2.6;(u*u+v*v-1.).abs()<=0.28||seg(x,y,(3.,13.),(8.,8.),0.6)},
    Icon::Stop=>area(x,y,4.,4.,12.,12.),
    Icon::Minimize=>seg(x,y,(4.,8.),(12.,8.),1.),
    Icon::Maximize=>rect_line(x,y,4.,4.,12.,12.,1.),
    Icon::Restore=>rect_line(x,y,3.,5.,10.,12.,0.85)||rect_line(x,y,6.,3.,13.,10.,0.85),
    Icon::Translate=>path(x,y,&[(3.5,12.),(8.5,4.),(13.5,12.)],0.9)||seg(x,y,(5.7,9.2),(11.3,9.2),0.8)||seg(x,y,(2.5,13.6),(13.5,13.6),0.7),
    Icon::Heart=>disc(x,y,5.2,6.,3.)||disc(x,y,10.8,6.,3.)||poly(x,y,&[(2.4,7.),(13.6,7.),(8.,14.)]),
    Icon::Move=>seg(x,y,(3.5,8.),(12.5,8.),0.8)||seg(x,y,(8.,3.5),(8.,12.5),0.8)||path(x,y,&[(4.5,6.),(2.8,8.),(4.5,10.)],0.8)||path(x,y,&[(11.5,6.),(13.2,8.),(11.5,10.)],0.8)||path(x,y,&[(6.,4.5),(8.,2.8),(10.,4.5)],0.8)||path(x,y,&[(6.,11.5),(8.,13.2),(10.,11.5)],0.8),
    _=>true,
}}
/// Hand-authored 8×8 pixel art for the icons that actually appear at 16dp. Every stroke is
/// two cells wide and every closed outline is continuous, which is impossible to get by
/// blindly down-sampling the 16-lattice.
fn small_art(icon:Icon)->Option<[&'static str;8]>{Some(match icon{
    Icon::Home=>["........","...XX...","..XXXX..",".XX..XX.",".XX..XX.",".XX..XX.",".XXXXXX.","........"],
    Icon::Check=>["........",".......X","......X.","XX...XX.","XXX.XX..",".XXXX...","........","........"],
    Icon::Island=>["........","..XXXX..",".XXXXXX.",".XX..XX.",".XX..XX.",".XXXXXX.","..XXXX..","........"],
    Icon::Plus=>["........","...XX...","...XX...","...XX...",".XXXXXX.",".XXXXXX.","...XX...","...XX..."],
    Icon::Music=>["........","....XXX.","....XX..","....XX..","....XX..",".XXXX...",".XX.XX..","........"],
    Icon::Todo=>["........",".XXXXXX.",".X....X.",".X.X..X.",".X..X.X.",".X...XX.",".XXXXXX.","........"],
    Icon::Settings=>["........",".XXXXXX.",".XX.....",".XXXXXX.","....XX..",".XXXXXX.","...XX...","........"],
    Icon::Search=>["..XXXX..",".XX..XX.",".XX..XX.",".XX..XX.","..XXXX..","....XX..",".....XX.","......XX"],
    Icon::Back=>["........","....XX..","...XX...","..XX....",".XX.....",".XX.....","..XX....","...XX..."],
    Icon::Arrow=>["........","........",".....XX.","...XX...","..XX....","XX......","XX......","........"],
    Icon::Close=>["........","..XX.XX.",".XX..XX.","XX....XX","XX....XX",".XX..XX.","..XX.XX.","........"],
    Icon::Play=>["........","...XX...","...XXX..","...XXXX.","...XXXXX","...XXXX.","...XXX..","........"],
    Icon::Pause=>["........","..XX....","..XX....","..XX....","..XX....","..XX....","..XX....","........"],
    Icon::Sun=>["...XX...","........","XX....XX","........","...XX...","........","XX....XX","........"],
    Icon::Moon=>["........","..XXXX..",".XX..XX.",".XX..XX.",".XX..XX.","..XXXX..","........","........"],
    Icon::Volume=>["........","....XX..","...XX...","..XX....","XX......","XX......","XXXXXX..","XXXXXX.."],
    Icon::Folder=>["........","XXXX....","XXXXXX..","XX..XXXX","XX..XXXX","XX..XXXX","XXXXXX..","........"],
    Icon::Import=>["........","....XX..","....XX..","....XX..","..XXXX..",".XX..XX.","XX....XX","XXXXXX.."],
    Icon::Reset=>["....XX..","..XX.XX.","XX...XX.","XX..XX..",".XX.XX..","..XXX...","...XX...","....X..."],
    Icon::Trash=>["........","XXXXXX..","........","XX..XX..","XX..XX..","XX..XX..","XX..XX..","XXXXXX.."],
    Icon::Translate=>["........","....XX..","...XXXX.","..XXXXXX","...XXXX.","....XX..","........","........"],
    Icon::Heart=>["........","..XX.XX.",".XXXXXX.","XXXXXXXX","XXXXXXXX",".XXXXXX.","..XXXX..","........"],
    Icon::Focus=>["........",".XXXXXX.",".XX..XX.",".XX..XX.",".XX..XX.",".XX..XX.",".XXXXXX.","........"],
    Icon::Move=>["...XX...","....XX..",".....XX.","XX...XX.","XX...XX.",".....XX.","....XX..","...XX..."],
    Icon::Stop=>["........","..XXXX..","..XXXX..","..XXXX..","..XXXX..","..XXXX..","........","........"],
    Icon::Minimize=>["........","........","........",".XXXXXX.","..XXXX..","........","........","........"],
    Icon::Maximize=>[".XXXXXX.",".XX..XX.",".XX..XX.",".XX..XX.",".XX..XX.",".XX..XX.",".XX..XX.",".XXXXXX."],
    Icon::Restore=>["..XXXX..",".XX..XX.",".XX..XX.",".XXXXXX.",".XX..XX.",".XX..XX.",".XX..XX.",".XXXXXX."],
    _=>return None,
})}

struct Glyph{dots:Vec<(f64,f64)>,w:f64,h:f64,x0:f64,y0:f64,x1:f64,y1:f64}
/// Legacy Material ink box at 24dp, so the dot design can be fitted back into the
/// exact slot the old glyph occupied (no control grows, no visual centre shifts).
fn legacy_ink(icon:Icon)->Option<(f64,f64,f64,f64)>{
    // Rail glyphs must not be fitted into their old Material ink box: that is what made one
    // icon 14 cells wide and the next 6. They keep the shared 14×14 box instead.
    if rail(icon){return None}
    let paths=dotisle_render::material_symbols::paths_for(icon)?;
    let tr=Affine::translate((0.,24.))*Affine::scale(1./40.);
    let mut acc:Option<vello::kurbo::Rect>=None;
    for path in paths{let b=tr.transform_rect_bbox(path.bounding_box());acc=Some(match acc{None=>b,Some(a)=>a.union(b)});}
    acc.map(|r|(r.x0,r.y0,r.x1,r.y1))
}
fn glyphs()->&'static Vec<Option<Glyph>>{
    static CACHE:OnceLock<Vec<Option<Glyph>>>=OnceLock::new();
    CACHE.get_or_init(||{
        let mut out:Vec<Option<Glyph>>=(0..32).map(|_|None).collect();
        for icon in ALL{out[icon as usize]=Some(build(icon,16));}
        out
    })
}
fn glyphs_small()->&'static Vec<Option<Glyph>>{
    static CACHE:OnceLock<Vec<Option<Glyph>>>=OnceLock::new();
    CACHE.get_or_init(||{
        let mut out:Vec<Option<Glyph>>=(0..32).map(|_|None).collect();
        for icon in ALL{out[icon as usize]=Some(build_small(icon));}
        out
    })
}
/// Two-cell max-pool of the accepted 16-lattice for icons that never show at 16dp.
fn pooled(icon:Icon)->Vec<(f64,f64)>{
    let g=build(icon,16);let mut set=std::collections::BTreeSet::new();
    for(dx,dy)in g.dots{set.insert((((dx-0.5)/2.).floor()as i64,((dy-0.5)/2.).floor()as i64));}
    set.into_iter().map(|(i,j)|(i as f64+0.5,j as f64+0.5)).collect()
}
/// Compact glyph: authored art where available, otherwise a pooled 8×8 cell map. Either way
/// the dot extents are mapped onto the same legacy ink box as the 16-lattice.
fn build_small(icon:Icon)->Glyph{
    let mut dots=if let Some(rows)=small_art(icon){
        let mut v=vec![];for(j,row)in rows.iter().enumerate(){for(i,ch)in row.chars().enumerate(){if ch=='X'{v.push((i as f64+0.5,j as f64+0.5));}}}
        v
    }else{pooled(icon)};
    if dots.is_empty(){dots.push((4.,4.));}
    let(mx0,my0,mx1,my1):(f64,f64,f64,f64)=(dots.iter().map(|d|d.0).fold(8.,f64::min),dots.iter().map(|d|d.1).fold(8.,f64::min),dots.iter().map(|d|d.0).fold(0.,f64::max),dots.iter().map(|d|d.1).fold(0.,f64::max));
    if let Some((lx0,ly0,lx1,ly1))=legacy_ink(icon){
        let cell=24./SMALL_GRID;let inset=DOT_RATIO;
        let(tx0,ty0)=(lx0/cell+inset,ly0/cell+inset);
        let(tx1,ty1)=((lx1/cell-inset).max(tx0),(ly1/cell-inset).max(ty0));
        for(dx,dy)in &mut dots{
            let px=if mx1>mx0{tx0+(*dx-mx0)*(tx1-tx0)/(mx1-mx0)}else{(tx0+tx1)/2.};
            let py=if my1>my0{ty0+(*dy-my0)*(ty1-ty0)/(my1-my0)}else{(ty0+ty1)/2.};
            *dx=px;*dy=py;
        }
        return Glyph{dots,w:tx1-tx0+1.,h:ty1-ty0+1.,x0:tx0,y0:ty0,x1:tx1,y1:ty1};
    }
    Glyph{dots,w:mx1-mx0+1.,h:my1-my0+1.,x0:mx0,y0:my0,x1:mx1,y1:my1}
}
fn build(icon:Icon,n:usize)->Glyph{
    let step=16./n as f64;
    let mut dots=vec![];
    for j in 0..n{for i in 0..n{
        let cx=(i as f64+0.5)*step;let cy=(j as f64+0.5)*step;
        if contains(icon,cx,cy){dots.push((i as f64+0.5,j as f64+0.5));}
    }}
    let(mut x0,mut y0,mut x1,mut y1):(f64,f64,f64,f64)=(n as f64,0.,0.,0.);
    for(dx,dy)in &dots{x0=x0.min(*dx);y0=y0.min(*dy);x1=x1.max(*dx);y1=y1.max(*dy);}
    if let Some((lx0,ly0,lx1,ly1))=legacy_ink(icon){
        let cell=24./n as f64;let inset=DOT_RATIO;
        let(tx0,ty0)=(lx0/cell+inset,ly0/cell+inset);
        let(tx1,ty1)=((lx1/cell-inset).max(tx0),(ly1/cell-inset).max(ty0));
        if x1>x0&&y1>y0&&tx1>tx0&&ty1>ty0{
            // Re-sample on a lattice whose spacing never exceeds one cell, so chains stay
            // continuous even when the legacy ink box is not square.
            let nx=(tx1-tx0).ceil().max(1.)as usize;let ny=(ty1-ty0).ceil().max(1.)as usize;
            let mut resampled=Vec::with_capacity((nx+1)*(ny+1));
            for j in 0..=ny{for i in 0..=nx{
                let px=tx0+(tx1-tx0)*i as f64/nx as f64;let py=ty0+(ty1-ty0)*j as f64/ny as f64;
                let ox=x0+(px-tx0)*(x1-x0)/(tx1-tx0);let oy=y0+(py-ty0)*(y1-y0)/(ty1-ty0);
                if contains(icon,ox,oy){resampled.push((px,py));}
            }}
            dots=resampled;(x0,y0,x1,y1)=(tx0,ty0,tx1,ty1);
        }
    }
    Glyph{dots,w:x1-x0+1.,h:y1-y0+1.,x0,y0,x1,y1}
}
/// Half-pixel snap keeps dot centres on the pixel grid at 1× and 2× alike.
fn snap(v:f64)->f64{(v*2.).round()/2.}
/// Grid chosen purely by slot size: ≤16dp compact, 20dp and above the main lattice.
pub fn grid_for(size:f64)->f64{if size<=SMALL_MAX{SMALL_GRID}else{GRID}}
/// Monochrome dot-matrix glyph inside the caller's original icon slot.
pub fn icon(scene:&mut Scene,kind:Icon,x:f64,y:f64,size:f64,rgb:u32){
    let(n,cache)=if grid_for(size)==SMALL_GRID{(SMALL_GRID,glyphs_small())}else{(GRID,glyphs())};
    let Some(g)=cache.get(kind as usize).and_then(|g|g.as_ref())else{
        scene.fill(Fill::NonZero,Affine::IDENTITY,crate::draw::color(rgb),None,&vello::kurbo::Rect::new(x,y,x+size,y+size));return
    };
    let cell=size/n;let r=(cell*DOT_RATIO).clamp(0.5,1.6);let c=crate::draw::color(rgb);
    for(dx,dy)in &g.dots{
        let px=snap(x+dx*cell).clamp(x+r,x+size-r);let py=snap(y+dy*cell).clamp(y+r,y+size-r);
        scene.fill(Fill::NonZero,Affine::IDENTITY,c,None,&vello::kurbo::Circle::new(Point::new(px,py),r));
    }
}

pub fn pixel_icon(scene:&mut Scene,kind:Icon,x:f64,y:f64,size:f64,rgb:u32){
 let (n,cache)=if size<=20.{(SMALL_GRID,glyphs_small())}else{(GRID,glyphs())};let Some(g)=cache.get(kind as usize).and_then(|g|g.as_ref())else{return};let cell=size/n;
 for(dx,dy)in &g.dots{let px=(x+dx*cell-cell/2.).round();let py=(y+dy*cell-cell/2.).round();scene.fill(Fill::NonZero,Affine::IDENTITY,crate::draw::color(rgb),None,&vello::kurbo::Rect::new(px,py,px+cell.ceil(),py+cell.ceil()));}
}
pub const ALL:[Icon;31]=[Icon::Home,Icon::Music,Icon::Focus,Icon::Todo,Icon::Settings,Icon::Play,Icon::Pause,Icon::Next,Icon::Prev,Icon::Plus,Icon::Check,Icon::Close,Icon::Arrow,Icon::Island,Icon::Sun,Icon::Moon,Icon::Back,Icon::Trash,Icon::Reset,Icon::Search,Icon::Volume,Icon::Folder,Icon::Import,Icon::Leaf,Icon::Stop,Icon::Minimize,Icon::Maximize,Icon::Restore,Icon::Translate,Icon::Heart,Icon::Move];
pub fn icon_name(kind:Icon)->&'static str{match kind{
    Icon::Home=>"Home",Icon::Music=>"Music",Icon::Focus=>"Focus",Icon::Todo=>"Todo",Icon::Settings=>"Settings",Icon::Play=>"Play",Icon::Pause=>"Pause",Icon::Next=>"Next",Icon::Prev=>"Prev",Icon::Plus=>"Plus",Icon::Check=>"Check",Icon::Close=>"Close",Icon::Arrow=>"Arrow",Icon::Island=>"Island",Icon::Sun=>"Sun",Icon::Moon=>"Moon",Icon::Back=>"Back",Icon::Trash=>"Trash",Icon::Reset=>"Reset",Icon::Search=>"Search",Icon::Volume=>"Volume",Icon::Folder=>"Folder",Icon::Import=>"Import",Icon::Leaf=>"Leaf",Icon::Stop=>"Stop",Icon::Minimize=>"Minimize",Icon::Maximize=>"Maximize",Icon::Restore=>"Restore",Icon::Translate=>"Translate",Icon::Heart=>"Heart",Icon::Move=>"Move",_=>"?",
}}
/// Per-icon ink box in grid cells, used by the icon sheet and the self-check table.
pub fn ink_box(kind:Icon)->(f64,f64){glyphs().get(kind as usize).and_then(|g|g.as_ref()).map(|g|(g.w,g.h)).unwrap_or((GRID,GRID))}
/// Dot-centre extents in grid cells (x0,y0,x1,y1); used for the bounding-box audit.
pub fn ink_bounds(kind:Icon)->Option<(f64,f64,f64,f64)>{glyphs().get(kind as usize).and_then(|g|g.as_ref()).map(|g|(g.x0,g.y0,g.x1,g.y1))}
/// Extents recomputed from the dot centres themselves, so the reported ink box never
/// depends on bookkeeping in `build` (this is the number quoted in the delivery table).
pub fn dot_extent(kind:Icon)->Option<(f64,f64,f64,f64)>{
    let g=glyphs().get(kind as usize).and_then(|g|g.as_ref())?;
    if g.dots.is_empty(){return None}
    let x0=g.dots.iter().map(|d|d.0).fold(f64::INFINITY,f64::min);
    let y0=g.dots.iter().map(|d|d.1).fold(f64::INFINITY,f64::min);
    let x1=g.dots.iter().map(|d|d.0).fold(f64::NEG_INFINITY,f64::max);
    let y1=g.dots.iter().map(|d|d.1).fold(f64::NEG_INFINITY,f64::max);
    Some((x0,y0,x1,y1))
}
/// Ink box in grid cells measured from the dots: width = span + one cell.
pub fn ink_box_measured(kind:Icon)->(f64,f64){
    dot_extent(kind).map(|(x0,y0,x1,y1)|(x1-x0+1.,y1-y0+1.)).unwrap_or((GRID,GRID))
}
/// Compact-grid dot-centre extents, audited at 16dp against the same legacy ink box.
pub fn ink_bounds_small(kind:Icon)->Option<(f64,f64,f64,f64)>{glyphs_small().get(kind as usize).and_then(|g|g.as_ref()).map(|g|(g.x0,g.y0,g.x1,g.y1))}
/// Number of dots in the compact glyph (for the stroke self-check table).
pub fn dot_count_small(kind:Icon)->usize{glyphs_small().get(kind as usize).and_then(|g|g.as_ref()).map(|g|g.dots.len()).unwrap_or(0)}
/// Dot count on the main 16×16 lattice, identical for every size at or above 20dp.
pub fn dot_count(kind:Icon)->usize{glyphs().get(kind as usize).and_then(|g|g.as_ref()).map(|g|g.dots.len()).unwrap_or(0)}
/// Compact-grid ink box in 8×8 cells.
pub fn ink_box_small(kind:Icon)->(f64,f64){glyphs_small().get(kind as usize).and_then(|g|g.as_ref()).map(|g|(g.w,g.h)).unwrap_or((SMALL_GRID,SMALL_GRID))}
/// Ink fraction of the 24dp slot. Dots touch but do not overlap, so count×πr²/size² tracks a
/// rasterised mask to within 0.1 pp (measured on the six rail glyphs).
pub fn coverage(kind:Icon)->f64{
    let r=(24./GRID*DOT_RATIO).clamp(0.5,1.6);
    dot_count(kind) as f64*std::f64::consts::PI*r*r/576.
}
/// Ink fraction of the 16dp slot on the compact grid.
pub fn coverage_small(kind:Icon)->f64{
    let r=(16./SMALL_GRID*DOT_RATIO).clamp(0.5,1.6);
    dot_count_small(kind) as f64*std::f64::consts::PI*r*r/256.
}
