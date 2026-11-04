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
    ("Glaze",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Glaze/shuffle.svg")),
    ("Glaze",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Glaze/heart.svg")),
    ("Glaze",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Glaze/prev.svg")),
    ("Glaze",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Glaze/play.svg")),
    ("Glaze",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Glaze/pause.svg")),
    ("Glaze",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Glaze/next.svg")),
    ("Glaze",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Glaze/translate.svg")),
    ("Glaze",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Glaze/settings.svg")),
    ("Halo",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Halo/shuffle.svg")),
    ("Halo",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Halo/heart.svg")),
    ("Halo",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Halo/prev.svg")),
    ("Halo",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Halo/play.svg")),
    ("Halo",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Halo/pause.svg")),
    ("Halo",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Halo/next.svg")),
    ("Halo",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Halo/translate.svg")),
    ("Halo",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Halo/settings.svg")),
    ("InkIsle",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/InkIsle/shuffle.svg")),
    ("InkIsle",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/InkIsle/heart.svg")),
    ("InkIsle",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/InkIsle/prev.svg")),
    ("InkIsle",Glyph::Play,include_str!("../../../../Assets/TransportIcons/InkIsle/play.svg")),
    ("InkIsle",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/InkIsle/pause.svg")),
    ("InkIsle",Glyph::Next,include_str!("../../../../Assets/TransportIcons/InkIsle/next.svg")),
    ("InkIsle",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/InkIsle/translate.svg")),
    ("InkIsle",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/InkIsle/settings.svg")),
    ("Layered",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Layered/shuffle.svg")),
    ("Layered",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Layered/heart.svg")),
    ("Layered",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Layered/prev.svg")),
    ("Layered",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Layered/play.svg")),
    ("Layered",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Layered/pause.svg")),
    ("Layered",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Layered/next.svg")),
    ("Layered",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Layered/translate.svg")),
    ("Layered",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Layered/settings.svg")),
    ("Mist",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Mist/shuffle.svg")),
    ("Mist",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Mist/heart.svg")),
    ("Mist",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Mist/prev.svg")),
    ("Mist",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Mist/play.svg")),
    ("Mist",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Mist/pause.svg")),
    ("Mist",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Mist/next.svg")),
    ("Mist",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Mist/translate.svg")),
    ("Mist",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Mist/settings.svg")),
    ("Ripple",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Ripple/shuffle.svg")),
    ("Ripple",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Ripple/heart.svg")),
    ("Ripple",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Ripple/prev.svg")),
    ("Ripple",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Ripple/play.svg")),
    ("Ripple",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Ripple/pause.svg")),
    ("Ripple",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Ripple/next.svg")),
    ("Ripple",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Ripple/translate.svg")),
    ("Ripple",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Ripple/settings.svg")),
    ("Sand",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Sand/shuffle.svg")),
    ("Sand",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Sand/heart.svg")),
    ("Sand",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Sand/prev.svg")),
    ("Sand",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Sand/play.svg")),
    ("Sand",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Sand/pause.svg")),
    ("Sand",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Sand/next.svg")),
    ("Sand",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Sand/translate.svg")),
    ("Sand",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Sand/settings.svg")),
    ("Tide",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Tide/shuffle.svg")),
    ("Tide",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Tide/heart.svg")),
    ("Tide",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Tide/prev.svg")),
    ("Tide",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Tide/play.svg")),
    ("Tide",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Tide/pause.svg")),
    ("Tide",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Tide/next.svg")),
    ("Tide",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Tide/translate.svg")),
    ("Tide",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Tide/settings.svg")),
    ("Whitespace",Glyph::Shuffle,include_str!("../../../../Assets/TransportIcons/Whitespace/shuffle.svg")),
    ("Whitespace",Glyph::Heart,include_str!("../../../../Assets/TransportIcons/Whitespace/heart.svg")),
    ("Whitespace",Glyph::Prev,include_str!("../../../../Assets/TransportIcons/Whitespace/prev.svg")),
    ("Whitespace",Glyph::Play,include_str!("../../../../Assets/TransportIcons/Whitespace/play.svg")),
    ("Whitespace",Glyph::Pause,include_str!("../../../../Assets/TransportIcons/Whitespace/pause.svg")),
    ("Whitespace",Glyph::Next,include_str!("../../../../Assets/TransportIcons/Whitespace/next.svg")),
    ("Whitespace",Glyph::Translate,include_str!("../../../../Assets/TransportIcons/Whitespace/translate.svg")),
    ("Whitespace",Glyph::Settings,include_str!("../../../../Assets/TransportIcons/Whitespace/settings.svg")),];

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
        for(set,glyph,svg)in ASSETS{if let Some(asset)=parse(svg){map.insert((*set,*glyph),asset);}}
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
