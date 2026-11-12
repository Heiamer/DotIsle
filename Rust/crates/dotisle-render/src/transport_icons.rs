//! Embedded open-source icon sets for the ten transport themes.
//! Reef keeps the shipped Material Symbols; the other nine read MIT/ISC/Apache-licensed SVGs.
use std::{collections::HashMap,sync::OnceLock};
use vello::{Scene,kurbo::{Affine,BezPath,Circle,Rect,RoundedRect,Shape,Stroke},peniko::Fill};
use crate::{draw::{self,Icon},transport::TransportTheme};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum Glyph{Shuffle,Heart,Prev,Play,Pause,Next,Translate,Settings}

struct Part{path:BezPath,stroke:Option<f64>,even_odd:bool}
struct Asset{parts:Vec<Part>,view:(f64,f64,f64,f64)}

const ASSETS:&[(&str,Glyph,&str)]=&[
    ("Glaze",Glyph::Shuffle,"TransportIcons/Glaze/shuffle.svg"),
    ("Glaze",Glyph::Heart,"TransportIcons/Glaze/heart.svg"),
    ("Glaze",Glyph::Prev,"TransportIcons/Glaze/prev.svg"),
    ("Glaze",Glyph::Play,"TransportIcons/Glaze/play.svg"),
    ("Glaze",Glyph::Pause,"TransportIcons/Glaze/pause.svg"),
    ("Glaze",Glyph::Next,"TransportIcons/Glaze/next.svg"),
    ("Glaze",Glyph::Translate,"TransportIcons/Glaze/translate.svg"),
    ("Glaze",Glyph::Settings,"TransportIcons/Glaze/settings.svg"),
    ("Halo",Glyph::Shuffle,"TransportIcons/Halo/shuffle.svg"),
    ("Halo",Glyph::Heart,"TransportIcons/Halo/heart.svg"),
    ("Halo",Glyph::Prev,"TransportIcons/Halo/prev.svg"),
    ("Halo",Glyph::Play,"TransportIcons/Halo/play.svg"),
    ("Halo",Glyph::Pause,"TransportIcons/Halo/pause.svg"),
    ("Halo",Glyph::Next,"TransportIcons/Halo/next.svg"),
    ("Halo",Glyph::Translate,"TransportIcons/Halo/translate.svg"),
    ("Halo",Glyph::Settings,"TransportIcons/Halo/settings.svg"),
    ("InkIsle",Glyph::Shuffle,"TransportIcons/InkIsle/shuffle.svg"),
    ("InkIsle",Glyph::Heart,"TransportIcons/InkIsle/heart.svg"),
    ("InkIsle",Glyph::Prev,"TransportIcons/InkIsle/prev.svg"),
    ("InkIsle",Glyph::Play,"TransportIcons/InkIsle/play.svg"),
    ("InkIsle",Glyph::Pause,"TransportIcons/InkIsle/pause.svg"),
    ("InkIsle",Glyph::Next,"TransportIcons/InkIsle/next.svg"),
    ("InkIsle",Glyph::Translate,"TransportIcons/InkIsle/translate.svg"),
    ("InkIsle",Glyph::Settings,"TransportIcons/InkIsle/settings.svg"),
    ("Layered",Glyph::Shuffle,"TransportIcons/Layered/shuffle.svg"),
    ("Layered",Glyph::Heart,"TransportIcons/Layered/heart.svg"),
    ("Layered",Glyph::Prev,"TransportIcons/Layered/prev.svg"),
    ("Layered",Glyph::Play,"TransportIcons/Layered/play.svg"),
    ("Layered",Glyph::Pause,"TransportIcons/Layered/pause.svg"),
    ("Layered",Glyph::Next,"TransportIcons/Layered/next.svg"),
    ("Layered",Glyph::Translate,"TransportIcons/Layered/translate.svg"),
    ("Layered",Glyph::Settings,"TransportIcons/Layered/settings.svg"),
    ("Mist",Glyph::Shuffle,"TransportIcons/Mist/shuffle.svg"),
    ("Mist",Glyph::Heart,"TransportIcons/Mist/heart.svg"),
    ("Mist",Glyph::Prev,"TransportIcons/Mist/prev.svg"),
    ("Mist",Glyph::Play,"TransportIcons/Mist/play.svg"),
    ("Mist",Glyph::Pause,"TransportIcons/Mist/pause.svg"),
    ("Mist",Glyph::Next,"TransportIcons/Mist/next.svg"),
    ("Mist",Glyph::Translate,"TransportIcons/Mist/translate.svg"),
    ("Mist",Glyph::Settings,"TransportIcons/Mist/settings.svg"),
    ("Ripple",Glyph::Shuffle,"TransportIcons/Ripple/shuffle.svg"),
    ("Ripple",Glyph::Heart,"TransportIcons/Ripple/heart.svg"),
    ("Ripple",Glyph::Prev,"TransportIcons/Ripple/prev.svg"),
    ("Ripple",Glyph::Play,"TransportIcons/Ripple/play.svg"),
    ("Ripple",Glyph::Pause,"TransportIcons/Ripple/pause.svg"),
    ("Ripple",Glyph::Next,"TransportIcons/Ripple/next.svg"),
    ("Ripple",Glyph::Translate,"TransportIcons/Ripple/translate.svg"),
    ("Ripple",Glyph::Settings,"TransportIcons/Ripple/settings.svg"),
    ("Sand",Glyph::Shuffle,"TransportIcons/Sand/shuffle.svg"),
    ("Sand",Glyph::Heart,"TransportIcons/Sand/heart.svg"),
    ("Sand",Glyph::Prev,"TransportIcons/Sand/prev.svg"),
    ("Sand",Glyph::Play,"TransportIcons/Sand/play.svg"),
    ("Sand",Glyph::Pause,"TransportIcons/Sand/pause.svg"),
    ("Sand",Glyph::Next,"TransportIcons/Sand/next.svg"),
    ("Sand",Glyph::Translate,"TransportIcons/Sand/translate.svg"),
    ("Sand",Glyph::Settings,"TransportIcons/Sand/settings.svg"),
    ("Tide",Glyph::Shuffle,"TransportIcons/Tide/shuffle.svg"),
    ("Tide",Glyph::Heart,"TransportIcons/Tide/heart.svg"),
    ("Tide",Glyph::Prev,"TransportIcons/Tide/prev.svg"),
    ("Tide",Glyph::Play,"TransportIcons/Tide/play.svg"),
    ("Tide",Glyph::Pause,"TransportIcons/Tide/pause.svg"),
    ("Tide",Glyph::Next,"TransportIcons/Tide/next.svg"),
    ("Tide",Glyph::Translate,"TransportIcons/Tide/translate.svg"),
    ("Tide",Glyph::Settings,"TransportIcons/Tide/settings.svg"),
    ("Whitespace",Glyph::Shuffle,"TransportIcons/Whitespace/shuffle.svg"),
    ("Whitespace",Glyph::Heart,"TransportIcons/Whitespace/heart.svg"),
    ("Whitespace",Glyph::Prev,"TransportIcons/Whitespace/prev.svg"),
    ("Whitespace",Glyph::Play,"TransportIcons/Whitespace/play.svg"),
    ("Whitespace",Glyph::Pause,"TransportIcons/Whitespace/pause.svg"),
    ("Whitespace",Glyph::Next,"TransportIcons/Whitespace/next.svg"),
    ("Whitespace",Glyph::Translate,"TransportIcons/Whitespace/translate.svg"),
    ("Whitespace",Glyph::Settings,"TransportIcons/Whitespace/settings.svg"),];

fn attr<'a>(tag:&'a str,name:&str)->Option<&'a str>{
    let mut from=0;
    while let Some(i)=tag[from..].find(name){
        let start=from+i;
        let boundary=tag[..start].chars().next_back().is_none_or(|c|!c.is_ascii_alphanumeric()&&c!='-'&&c!='_');
        if boundary{
            let rest=tag[start+name.len()..].trim_start();
            if let Some(rest)=rest.strip_prefix('='){
                let rest=rest.trim_start();
                if let Some(q)=rest.chars().next().filter(|c|*c=='"'||*c=='\''){let body=&rest[q.len_utf8()..];if let Some(end)=body.find(q){return Some(&body[..end])}}
            }
        }
        from=start+name.len();
    }
    None
}
fn number(tag:&str,name:&str)->Option<f64>{attr(tag,name).and_then(|v|v.trim().parse().ok())}
fn view_box(svg:&str)->(f64,f64,f64,f64){
    let v=attr(svg,"viewBox").or_else(||attr(svg,"viewbox")).unwrap_or("0 0 24 24");
    let n=v.split(|c:char|c==','||c.is_whitespace()).filter(|s|!s.is_empty()).filter_map(|s|s.parse::<f64>().ok()).collect::<Vec<_>>();
    if n.len()==4&&n[2]>0.&&n[3]>0.{(n[0],n[1],n[2],n[3])}else{(0.,0.,24.,24.)}
}
fn points_path(raw:&str,close:bool)->Option<BezPath>{
    let nums=raw.split(|c:char|c==','||c.is_whitespace()).filter(|s|!s.is_empty()).filter_map(|s|s.parse::<f64>().ok()).collect::<Vec<_>>();
    if nums.len()<4{return None}
    let mut p=BezPath::new();p.move_to((nums[0],nums[1]));
    for pair in nums[2..].chunks_exact(2){p.line_to((pair[0],pair[1]));}
    if close{p.close_path();}
    Some(p)
}
fn parse(svg:&str)->Option<Asset>{
    let svg_start=svg.find("<svg")?;let end=svg[svg_start..].find('>')?+svg_start;let root=&svg[svg_start..=end];let view=view_box(root);
    let root_fill=attr(root,"fill").unwrap_or("black");let root_stroke=attr(root,"stroke");let root_width=number(root,"stroke-width").unwrap_or(1.);let root_rule=attr(root,"fill-rule").unwrap_or("");
    let mut parts=Vec::new();let mut at=end+1;
    while at<svg.len(){
        let next=["<path","<circle","<rect","<line","<polyline","<polygon"].iter().filter_map(|tag|svg[at..].find(tag).map(|i|(at+i,*tag))).min_by_key(|(i,_)|*i);
        let Some((start,tag_name))=next else{break};
        let close=svg[start..].find('>')?+start;let tag=&svg[start..=close];at=close+1;
        let fill=attr(tag,"fill").unwrap_or(root_fill);let stroke=attr(tag,"stroke").or(root_stroke);let width=number(tag,"stroke-width").unwrap_or(root_width);let rule=attr(tag,"fill-rule").unwrap_or(root_rule);
        let stroke_on=stroke.is_some_and(|s|s!="none"&&s!="transparent");
        let fill_on=fill!="none"&&fill!="transparent";
        if !stroke_on&&!fill_on{continue}
        let path=match tag_name{
            "<path"=>attr(tag,"d").and_then(|d|BezPath::from_svg(d).ok()),
            "<circle"=>{let cx=number(tag,"cx").unwrap_or(0.);let cy=number(tag,"cy").unwrap_or(0.);let r=number(tag,"r").unwrap_or(0.);if r>0.{Some(Circle::new((cx,cy),r).to_path(0.1))}else{None}},
            "<rect"=>{let x=number(tag,"x").unwrap_or(0.);let y=number(tag,"y").unwrap_or(0.);let w=number(tag,"width").unwrap_or(0.);let h=number(tag,"height").unwrap_or(0.);let rx=number(tag,"rx").or_else(||number(tag,"ry")).unwrap_or(0.);if w>0.&&h>0.{Some(RoundedRect::from_rect(Rect::new(x,y,x+w,y+h),rx).to_path(0.1))}else{None}},
            "<line"=>{let x1=number(tag,"x1").unwrap_or(0.);let y1=number(tag,"y1").unwrap_or(0.);let x2=number(tag,"x2").unwrap_or(0.);let y2=number(tag,"y2").unwrap_or(0.);let mut p=BezPath::new();p.move_to((x1,y1));p.line_to((x2,y2));Some(p)},
            "<polyline"=>attr(tag,"points").and_then(|p|points_path(p,false)),
            _=>attr(tag,"points").and_then(|p|points_path(p,true)),
        };
        if let Some(path)=path{parts.push(Part{path,stroke:if stroke_on&&!fill_on{Some(width)}else{None},even_odd:rule=="evenodd"});}
    }
    if parts.is_empty(){None}else{Some(Asset{parts,view})}
}
fn assets()->&'static HashMap<(&'static str,Glyph),Asset>{
    static CACHE:OnceLock<HashMap<(&'static str,Glyph),Asset>>=OnceLock::new();
    CACHE.get_or_init(||{
        let mut map=HashMap::new();
        for(set,glyph,path)in ASSETS {
            let svg=std::env::current_exe().ok().and_then(|p|p.parent().map(|p|p.join("Assets").join(path))).and_then(|p|std::fs::read_to_string(p).ok());
            if let Some(asset)=svg.as_deref().and_then(parse){map.insert((*set,*glyph),asset);}
        }
        map
    })
}
fn palette(theme:TransportTheme)->&'static str{match theme{TransportTheme::Reef=>"Reef",TransportTheme::Ripple=>"Ripple",TransportTheme::Sand=>"Sand",TransportTheme::Mist=>"Mist",TransportTheme::Glaze=>"Glaze",TransportTheme::InkIsle=>"InkIsle",TransportTheme::Tide=>"Tide",TransportTheme::Whitespace=>"Whitespace",TransportTheme::Halo=>"Halo",TransportTheme::Layered=>"Layered"}}
fn builtin(g:Glyph)->Icon{match g{Glyph::Shuffle=>Icon::Move,Glyph::Heart=>Icon::Heart,Glyph::Prev=>Icon::Prev,Glyph::Play=>Icon::Play,Glyph::Pause=>Icon::Pause,Glyph::Next=>Icon::Next,Glyph::Translate=>Icon::Translate,Glyph::Settings=>Icon::Settings}}
fn color(argb:u32)->vello::peniko::Color{vello::peniko::Color::from_rgba8((argb>>16)as u8,(argb>>8)as u8,argb as u8,(argb>>24)as u8)}

/// Paints a transport glyph inside the same square every theme uses. Reef keeps the shipped
/// Material Symbols drawing; every other theme reads its own icon library.
pub fn paint_icon(scene:&mut Scene,g:Glyph,theme:TransportTheme,r:Rect,argb:u32){
    if theme==TransportTheme::Reef{draw::icon(scene,builtin(g),r.x0,r.y0,r.width(),argb&0xffffff);return}
    let Some(asset)=assets().get(&(palette(theme),g))else{draw::icon(scene,builtin(g),r.x0,r.y0,r.width(),argb&0xffffff);return};
    let(x0,y0,w,h)=asset.view;
    let tr=Affine::translate((r.x0,r.y0))*Affine::scale(r.width()/24.)*Affine::scale_non_uniform(24./w,24./h)*Affine::translate((-x0,-y0));
    for part in &asset.parts{
        if let Some(width)=part.stroke{scene.stroke(&Stroke::new(width).with_caps(vello::kurbo::Cap::Round).with_join(vello::kurbo::Join::Round),tr,color(argb),None,&part.path);}
        else{scene.fill(if part.even_odd{Fill::EvenOdd}else{Fill::NonZero},tr,color(argb),None,&part.path);}
    }
}

#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn every_theme_has_every_glyph(){
        for theme in TransportTheme::ALL{
            for g in [Glyph::Shuffle,Glyph::Heart,Glyph::Prev,Glyph::Play,Glyph::Pause,Glyph::Next,Glyph::Translate,Glyph::Settings]{
                let mut scene=Scene::new();paint_icon(&mut scene,g,theme,Rect::new(0.,0.,24.,24.),0xffffffff);
                assert!(!scene.is_empty(),"{} {:?}",palette(theme),g);
            }
        }
    }
}
