//! Shared live geometry for the native island, settings preview and layout thumbnails.
use serde::Serialize;
#[derive(Clone,Copy,Default,Debug,Serialize,PartialEq)]
pub struct Rect {pub x:f64,pub y:f64,pub w:f64,pub h:f64}
impl Rect {pub fn new(x:f64,y:f64,w:f64,h:f64)->Self{Self{x,y,w,h}} pub fn contains(self,x:f64,y:f64)->bool{x>=self.x&&y>=self.y&&x<self.x+self.w&&y<self.y+self.h}}
/// Expanded artwork uses half the card's configured radius, bounded by its own short edge.
pub fn cover_corner_radius(card_radius:f64,width:f64,height:f64)->f64{
    let radius=if card_radius.is_finite(){card_radius.max(0.)}else{30.};
    (radius*0.5).min(width.max(0.).min(height.max(0.))/2.)
}
fn buttons_at(width:f64,scale:f64,center_y:f64)->[(i32,Rect);8]{
    let snap=|v:f64|(v*scale).round_ties_even()/scale;let center=snap(width/2.);let y=snap(center_y);
    [(2,Rect::new(18.,y-14.,28.,28.)),(9,Rect::new(52.,y-14.,28.,28.)),(3,Rect::new(center-58.,y-16.,32.,32.)),(4,Rect::new(center-20.,y-20.,40.,40.)),(5,Rect::new(center+26.,y-16.,32.,32.)),(6,Rect::new(width-80.,y-14.,28.,28.)),(7,Rect::new(width-46.,y-14.,28.,28.)),(16,Rect::new(86.,y-14.,28.,28.))]
}
// 三套全新重制桌面展开布局平滑高度曲线：
// 0=光屿画廊(SideBySide, 244), 1=流光分栏(Separated, 240), 2=星幕歌词(LyricFocus, 244), 3/4=右图/沉浸兼容(236)
pub fn music_height(mix:f64,font:f64)->f64{
    let extra=(font+8.-24.).max(0.);
    if mix<=1.{244.-4.*mix.clamp(0.,1.)+extra}
    else if mix<=2.{240.+4.*(mix-1.)+extra}
    else if mix<=3.{244.-8.*(mix-2.)+extra}
    else{236.+extra}
}
/// The v2.1.6 native layout transition: 230 ms, with zero speed at both ends.
/// Both the island and the settings preview use this exact timing.
pub fn music_layout_mix(from:f64,target:f64,elapsed_seconds:f64)->f64{
    let t=(elapsed_seconds/0.23).clamp(0.,1.);
    if t>=1.{return target}
    let e=t*t*(3.-2.*t);
    from+(target-from)*e
}
/// The expanded music layouts. This is a layout type, not a set of switches.
#[derive(Clone,Copy,Debug,PartialEq,Eq)]
pub enum MusicLayout { Separated, SideBySide, LyricFocus, LyricRight, Immersive, FluidCloud }
impl MusicLayout {
    pub fn position(self)->f64{match self{Self::SideBySide|Self::FluidCloud=>0.,Self::Separated=>1.,Self::LyricFocus=>2.,Self::LyricRight=>3.,Self::Immersive=>4.}}
    pub fn from_setting(value:&str)->Self{match value{"Separated"=>Self::Separated,"LyricFocus"=>Self::LyricFocus,"Immersive"=>Self::Immersive,"FluidCloud"=>Self::SideBySide,_=>Self::SideBySide}}
}
/// 判断当前是否处于全新「光屿画廊 (Prism Stage)」首套主布局（SideBySide / FluidCloud 兼容别名）。
pub fn is_fluid_cloud(layout:MusicLayout,mix:f64)->bool{
    matches!(layout,MusicLayout::SideBySide|MusicLayout::FluidCloud)&&mix<0.35
}
/// 计算 Immersive 歌词沉浸布局激活权重。
pub fn immersion_amount(layout:MusicLayout,mix:f64,custom:bool)->f64{
    if custom||(layout!=MusicLayout::Immersive&&mix<3.01){0.}else{(mix-3.).clamp(0.,1.)}
}
/// Everything drawing, hit testing and seek share for one expanded music card.
///
/// `align` is the one horizontal anchor every consumer reads: 0 hugs the left edge,
/// 0.5 centres in the free space, 1 hugs the right edge. Drawing, the settings
/// preview and the clip rects all use this single number, so the preview can never
/// disagree with the island about where a line sits.
#[derive(Clone,Copy)]
pub struct MusicGeometry {
    pub cover:Rect,pub title:Rect,pub artist:Rect,pub lyric:Rect,pub secondary:Rect,
    pub status_dot:Rect,pub status_text:Rect,pub track:Rect,pub elapsed:Rect,pub total:Rect,
    pub buttons:[(i32,Rect);8],pub height:f64,pub progress_hit:Rect,pub text_hit:Rect,pub split:f64,pub align:f64,
    /// Small centred action shown when the player cannot seek; offers the enable flow.
    pub seek_chip:Rect,
}
fn lerp_rect(a:Rect,b:Rect,t:f64)->Rect{Rect::new(a.x+(b.x-a.x)*t,a.y+(b.y-a.y)*t,a.w+(b.w-a.w)*t,a.h+(b.h-a.h)*t)}
impl MusicGeometry {
    /// Blend two layouts. Geometry is the only thing that interpolates, so drawing and
    /// hit testing stay in lockstep on every frame of the transition.
    pub fn between(a:&Self,b:&Self,t:f64)->Self{
        let mut buttons=a.buttons;
        for (slot,other) in buttons.iter_mut().zip(b.buttons){slot.1=lerp_rect(slot.1,other.1,t);}
        Self{seek_chip:lerp_rect(a.seek_chip,b.seek_chip,t),cover:lerp_rect(a.cover,b.cover,t),title:lerp_rect(a.title,b.title,t),artist:lerp_rect(a.artist,b.artist,t),lyric:lerp_rect(a.lyric,b.lyric,t),secondary:lerp_rect(a.secondary,b.secondary,t),status_dot:lerp_rect(a.status_dot,b.status_dot,t),status_text:lerp_rect(a.status_text,b.status_text,t),track:lerp_rect(a.track,b.track,t),elapsed:lerp_rect(a.elapsed,b.elapsed,t),total:lerp_rect(a.total,b.total,t),buttons,height:a.height+(b.height-a.height)*t,progress_hit:lerp_rect(a.progress_hit,b.progress_hit,t),text_hit:lerp_rect(a.text_hit,b.text_hit,t),split:a.split+(b.split-a.split)*t,align:a.align+(b.align-a.align)*t}
    }
}
/// `mix` is the live position: 0 side by side, 1 separated, 2 lyric focus, 3 lyric right.
pub fn music_geometry(width:f64,_layout:MusicLayout,mix:f64,scale:f64,font:f64)->MusicGeometry{
    let extra=(font+8.-24.).max(0.);let line=24.+extra;
    // 布局 1（重制默认）：「光屿画廊 · SideBySide」—— 非对称左翼 104x104 大唱片 + 中缝冰晶线 + 右上微圈双钮
    let center=(width/2.*scale).round_ties_even()/scale;
    let cy=((212.+extra)*scale).round_ties_even()/scale;
    let side=MusicGeometry{
        cover:Rect::new(22.,22.,104.,104.),
        title:Rect::new(144.,22.,(width-230.).max(80.),26.),
        artist:Rect::new(144.,50.,(width-166.).max(80.),18.),
        lyric:Rect::new(144.,80.,(width-166.).max(80.),line),
        secondary:Rect::new(144.,108.+extra,(width-166.).max(80.),18.),
        status_dot:Rect::new(22.,137.,10.,10.),
        status_text:Rect::new(37.,133.,89.,18.),
        track:Rect::new(22.,162.+extra,width-44.,4.),
        elapsed:Rect::new(22.,171.+extra,70.,14.),
        total:Rect::new(width-92.,171.+extra,70.,14.),
        buttons:[
            (2,Rect::new(22.,cy-13.,26.,26.)),
            (9,Rect::new(54.,cy-13.,26.,26.)),
            (3,Rect::new(center-62.,cy-16.,32.,32.)),
            (4,Rect::new(center-19.,cy-19.,38.,38.)),
            (5,Rect::new(center+30.,cy-16.,32.,32.)),
            (6,Rect::new(width-78.,20.,26.,26.)),
            (7,Rect::new(width-48.,20.,26.,26.)),
            (16,Rect::new(88.,cy-13.,26.,26.)),
        ],
        height:music_height(0.,font),
        progress_hit:Rect::new(18.,155.+extra,width-36.,18.),
        text_hit:Rect::new(144.,22.,(width-230.).max(80.),50.),
        split:0.,align:0.,
        seek_chip:Rect::new(width/2.-65.,167.+extra,130.,24.),
    };
    // 布局 2（重制）：「流光分栏 · Separated」—— 电影顶眉信息带 + 全宽宽幕歌词剧场
    let sep_head_w=(width-196.).max(80.);
    let separated=MusicGeometry{
        cover:Rect::new(22.,18.,54.,54.),
        title:Rect::new(90.,21.,sep_head_w,24.),
        artist:Rect::new(90.,47.,sep_head_w,18.),
        lyric:Rect::new(22.,88.,width-44.,line),
        secondary:Rect::new(22.,116.+extra,width-44.,18.),
        status_dot:Rect::new(width-98.,38.,10.,10.),
        status_text:Rect::new(width-83.,34.,61.,18.),
        track:Rect::new(22.,156.+extra,width-44.,4.),
        elapsed:Rect::new(22.,165.+extra,70.,14.),
        total:Rect::new(width-92.,165.+extra,70.,14.),
        buttons:buttons_at(width,scale,206.+extra),
        height:music_height(1.,font),
        progress_hit:Rect::new(18.,150.+extra,width-36.,18.),
        text_hit:Rect::new(90.,21.,sep_head_w,44.),
        split:1.,align:0.,
        seek_chip:Rect::new(width/2.-65.,161.+extra,130.,24.),
    };
    // 布局 3（重制）：「星幕歌词 · LyricFocus」—— 极简居中微封面头组 + 中轴超大居中歌词剧场
    let head_x=(width/2.-122.).max(22.);
    let head_w=(width-head_x-166.).min(180.).max(80.);
    let focus=MusicGeometry{
        cover:Rect::new(head_x,18.,46.,46.),
        title:Rect::new(head_x+58.,20.,head_w,22.),
        artist:Rect::new(head_x+58.,44.,head_w,16.),
        lyric:Rect::new(22.,82.,width-44.,line+6.),
        secondary:Rect::new(22.,118.+extra,width-44.,18.),
        status_dot:Rect::new(width-98.,38.,0.,0.),
        status_text:Rect::new(width-83.,34.,0.,0.),
        track:Rect::new(22.,160.+extra,width-44.,4.),
        elapsed:Rect::new(22.,169.+extra,70.,14.),
        total:Rect::new(width-92.,169.+extra,70.,14.),
        buttons:buttons_at(width,scale,211.+extra),
        height:music_height(2.,font),
        progress_hit:Rect::new(18.,154.+extra,width-36.,18.),
        text_hit:Rect::new(head_x+58.,20.,head_w,40.),
        split:2.,align:0.5,
        seek_chip:Rect::new(width/2.-65.,165.+extra,130.,24.),
    };
    // 兼容右置封面旧位（供内部过渡与历史兼容）
    let right=MusicGeometry{
        cover:Rect::new(width-88.,20.,68.,68.),
        title:Rect::new(20.,18.,width-128.,24.),
        artist:Rect::new(20.,45.,width-128.,18.),
        lyric:Rect::new(20.,68.,width-128.,line),
        secondary:Rect::new(20.,94.+extra,width-128.,18.),
        status_dot:Rect::new(20.,120.+extra,4.,4.),
        status_text:Rect::new(20.,114.+extra,width-128.,16.),
        track:Rect::new(20.,150.+extra,width-40.,4.),
        elapsed:Rect::new(20.,160.+extra,70.,14.),
        total:Rect::new(width-90.,160.+extra,70.,14.),
        buttons:buttons_at(width,scale,201.+extra),
        height:music_height(3.,font),
        progress_hit:Rect::new(16.,144.+extra,width-32.,14.),
        text_hit:Rect::new(20.,18.,width-128.,46.),
        split:3.,align:0.5,
        seek_chip:Rect::new(width/2.-65.,155.+extra,130.,24.),
    };
    let immersive=MusicGeometry{
        cover:Rect::new(24.,20.,0.,0.),title:Rect::new(24.,18.,width-48.,24.),artist:Rect::new(24.,44.,width-142.,17.),
        lyric:Rect::new(24.,70.,width-48.,62.),secondary:Rect::new(24.,134.,width-48.,18.),
        status_dot:Rect::new(width-85.,50.,3.,3.),status_text:Rect::new(width-76.,44.,52.,17.),
        track:Rect::new(24.,164.+extra,width-48.,4.),elapsed:Rect::new(24.,174.+extra,70.,12.),total:Rect::new(width-94.,174.+extra,70.,12.),
        buttons:buttons_at(width,scale,207.+extra),height:music_height(4.,font),
        progress_hit:Rect::new(20.,158.+extra,width-40.,16.),text_hit:Rect::new(24.,18.,width-48.,43.),split:4.,align:0.,seek_chip:Rect::new(width/2.-65.,169.+extra,130.,24.),
    };
    if mix<=1.{
        // 光屿画廊(左大封面) -> 流光分栏(左上小封面+全宽歌词)：先缩小封面避让后再横向展开全宽歌词
        let mut stacked=separated;
        stacked.lyric.x=side.lyric.x;stacked.lyric.w=side.lyric.w;
        stacked.secondary.x=side.secondary.x;stacked.secondary.w=side.secondary.w;
        let t=mix.clamp(0.,1.);
        let mut g=if t<=0.5{MusicGeometry::between(&side,&stacked,t*2.)}
        else{MusicGeometry::between(&stacked,&separated,(t-0.5)*2.)};
        g.height=music_height(mix,font);
        g
    }
    else if mix<=2.{MusicGeometry::between(&separated,&focus,(mix-1.).clamp(0.,1.))}
    else if mix<=3.{
        let t=(mix-2.).clamp(0.,1.);
        let mut g=MusicGeometry::between(&focus,&right,t);
        let(anchor,sc)=if t<0.5{(focus.cover,1.-t*2.)}else{(right.cover,(t-0.5)*2.)};
        let cx=anchor.x+anchor.w*0.5;let cy=anchor.y+anchor.h*0.5;
        g.cover=Rect::new(cx-anchor.w*sc*0.5,cy-anchor.h*sc*0.5,anchor.w*sc,anchor.h*sc);
        g
    }
    else{MusicGeometry::between(&right,&immersive,(mix-3.).clamp(0.,1.))}
}
/// Two decision chips replace the single offer while the user confirms a player restart.
pub fn seek_confirm_pair(chip:Rect)->(Rect,Rect){let w=96.;let gap=8.;let total=w*2.+gap;let x=chip.x+chip.w*0.5-total*0.5;let y=chip.y-1.;(Rect::new(x,y,w,26.),Rect::new(x+w+gap,y,w,26.))}
/// Compatibility entry point: both capsule modes share one expanded composition.
pub fn mini_music_geometry(width:f64,layout:MusicLayout,mix:f64,scale:f64,font:f64)->MusicGeometry{music_geometry(width,layout,mix,scale,font)}
#[derive(Debug,PartialEq)]
pub struct MusicCopy<'a>{pub header_title:&'a str,pub header_artist:&'a str,pub primary:&'a str,pub secondary:&'a str}
pub fn music_copy<'a>(title:&'a str,artist:&'a str,lyric:&'a str,next:&'a str)->MusicCopy<'a>{
    MusicCopy{header_title:title,header_artist:artist,primary:lyric,secondary:next}
}

/// A compact content stack; absent context has no reserved row.
#[derive(Clone,Copy,Debug)]pub struct ImmersiveTextGeometry{pub previous_y:f64,pub current_y:f64,pub secondary_y:f64,pub next_y:f64}
pub fn immersive_text_geometry(height:f64,rows:u8,previous:bool,secondary:bool,next:bool,reveal:f64)->ImmersiveTextGeometry{
 let main=if rows>1{62.}else{30.};let before=if previous{26.}else{0.};let after=if secondary{26.}else{0.};let tail=if next{32.}else{0.};let total=before+main+after+tail;let top=70.+((height-86.-total)/2.).max(0.);let current=top+before;let subtitle=current+main+8.;let next_y=if secondary{subtitle+28.}else{current+main+10.};let t=reveal.clamp(0.,1.);let hover=if rows>1{70.}else{86.};
 ImmersiveTextGeometry{previous_y:top,current_y:current+(hover-current)*t,secondary_y:subtitle+(134.-subtitle)*t,next_y}
}
#[cfg(test)]mod immersive_text_tests{use super::*;
 #[test]fn context_has_compact_gaps_for_one_or_two_lines(){for rows in [1,2]{let g=immersive_text_geometry(236.,rows,true,true,true,0.);let h=if rows==1{30.}else{62.};assert_eq!(g.secondary_y-(g.current_y+h),8.);assert_eq!(g.next_y-(g.secondary_y+18.),10.);assert!(g.next_y+22.<=220.);}}
 #[test]fn control_reveal_can_reverse_without_changing_outer_geometry(){for rows in [1,2]{let a=immersive_text_geometry(236.,rows,true,true,true,0.);let b=immersive_text_geometry(236.,rows,true,true,true,1.);let mid=immersive_text_geometry(236.,rows,true,true,true,0.5);assert!((mid.current_y-(a.current_y+b.current_y)/2.).abs()<0.001);assert_eq!(b.secondary_y,134.);}}
}
#[cfg(test)]mod immersive_geometry_tests{use super::*;
 #[test]fn immersive_has_the_shared_height_without_a_header_artwork_target(){for width in [400.,440.,592.]{for font in [11.,14.,17.,20.]{for scale in [1.,1.25,1.5,2.]{let g=music_geometry(width,MusicLayout::Immersive,4.,scale,font);assert_eq!(g.height,music_height(3.,font));assert_eq!(g.cover.w,0.);assert_eq!(g.cover.h,0.);assert!(!g.cover.contains(g.cover.x,g.cover.y));assert_eq!(g.title.x,24.);assert!(g.title.y+g.title.h<=g.artist.y);assert!(g.artist.y+g.artist.h<g.lyric.y);assert!(g.lyric.y+g.lyric.h<=g.secondary.y);assert!(g.secondary.y+g.secondary.h+12.<=g.track.y);for(id,r)in g.buttons{assert!(r.x>=18.&&r.x+r.w<=width-18.);assert!(r.y+r.h<=g.height-8.);assert!(r.y>=g.elapsed.y+g.elapsed.h);assert!(!r.contains(g.track.x+1.,g.track.y),"{id}");}}}}}
 #[test]fn immersive_transition_does_not_grow_the_card_or_snap_in_reverse(){for font in [11.,14.,20.]{for i in 0..=100{let mix=3.+i as f64/100.;let a=music_geometry(400.,MusicLayout::Immersive,mix,1.5,font);let b=music_geometry(400.,MusicLayout::LyricRight,mix,1.5,font);assert_eq!(a.height,music_height(3.,font));assert_eq!(a.cover,b.cover);assert_eq!(a.title,b.title);assert_eq!(a.buttons,b.buttons);assert!(a.cover.w>=0.&&a.cover.w<=68.);}}}
}
#[cfg(test)]mod copy_tests{use super::*;
    #[test]fn the_header_and_lyric_lines_keep_their_classic_sources(){
        let lines=music_copy("歌名","作者","当前歌词","下一行歌词");
        assert_eq!(lines,MusicCopy{header_title:"歌名",header_artist:"作者",primary:"当前歌词",secondary:"下一行歌词"});
    }
}
#[cfg(test)]
mod tests{
    use super::*;
    #[test]fn every_layout_and_transition_keeps_uniform_artwork_and_text_clear(){
        let overlaps=|a:Rect,b:Rect|a.x<b.x+b.w&&b.x<a.x+a.w&&a.y<b.y+b.h&&b.y<a.y+a.h;
        for width in [400.,440.,592.]{for font in [11.,12.5,17.,20.]{
            for (layout,mix) in [(MusicLayout::SideBySide,0.),(MusicLayout::Separated,1.),(MusicLayout::LyricFocus,2.),(MusicLayout::LyricRight,3.)]{
                let g=music_geometry(width,layout,mix,1.5,font);
                assert_eq!(g.track.h,4.);
                assert_eq!(g.cover.w,g.cover.h);
                assert!(g.cover.w>=46.&&g.cover.w<=104.);
                for text in [g.title,g.artist,g.lyric,g.secondary]{assert!(!overlaps(g.cover,text),"cover overlaps text in {layout:?}");}
                assert!(g.title.y+g.title.h<=g.artist.y);
                assert!(g.artist.y+g.artist.h<=g.lyric.y);
                assert!(g.lyric.y+g.lyric.h<=g.secondary.y);
                assert!(g.track.y>=g.secondary.y+g.secondary.h+12.);
                assert!(g.lyric.x>=20.&&g.lyric.x+g.lyric.w<=width-20.);
                assert!(g.buttons[3].1.y+g.buttons[3].1.h<=g.height-8.);
            }
            for step in 0..=24{
                let mix=step as f64/8.;
                let g=music_geometry(width,MusicLayout::SideBySide,mix,1.5,font);
                assert_eq!(g.track.h,4.);
                assert_eq!(g.cover.w,g.cover.h);
                assert!(g.buttons[3].1.y+g.buttons[3].1.h<=g.height-8.);
            }
        }}
        for (layout,align) in [(MusicLayout::SideBySide,0.),(MusicLayout::Separated,0.),(MusicLayout::LyricFocus,0.5),(MusicLayout::LyricRight,0.5)]{
            assert_eq!(music_geometry(400.,layout,layout.position(),1.5,12.5).align,align);
        }
    }
    #[test]fn the_right_layout_mirrors_the_side_artwork(){let _a=music_geometry(400.,MusicLayout::SideBySide,0.,1.5,12.5);let b=music_geometry(400.,MusicLayout::LyricRight,3.,1.5,12.5);
        let(cover,title,artist,lyric,secondary)=(b.cover,b.title,b.artist,b.lyric,b.secondary);
        assert_eq!((cover.x,cover.y,cover.w,cover.h),(312.,20.,68.,68.));
        assert_eq!((title.x,artist.x,lyric.x,secondary.x),(20.,20.,20.,20.));
        assert_eq!(b.align,0.5);
        assert!(cover.x>=title.x+title.w);assert!(cover.x>=lyric.x+lyric.w);
        assert!(b.buttons[3].1.y>180.&&b.buttons[3].1.y+b.buttons[3].1.h<=b.height-8.);assert_eq!(b.height,236.);
    }
}

#[cfg(test)]mod cover_corner_tests{use super::*;
 #[test]fn artwork_uses_half_card_radius_in_both_sizes_and_stays_within_its_bounds(){
  for edge in [40.,56.,68.]{for radius in [8.,16.,30.,48.]{assert_eq!(cover_corner_radius(radius,edge,edge),(radius*0.5).min(edge/2.));}}
  assert_eq!(cover_corner_radius(30.,68.,68.),15.);assert_eq!(cover_corner_radius(30.,40.,40.),15.);
 }
}

#[cfg(test)]mod volume_geometry_tests{use super::*;
 #[test]fn volume_targets_share_the_baseline_and_clear_every_other_button(){
  let overlaps=|a:Rect,b:Rect|a.x<b.x+b.w&&b.x<a.x+a.w&&a.y<b.y+b.h&&b.y<a.y+a.h;
  for width in [400.,440.,592.]{for scale in [1.,1.25,1.5,2.]{for i in 0..=32{
   let mix=i as f64/8.;
   let g=music_geometry(width,MusicLayout::SideBySide,mix,scale,20.);
   let volume=g.buttons.iter().find(|(id,_)|*id==16).unwrap().1;
   let heart=g.buttons.iter().find(|(id,_)|*id==9).unwrap().1;
   assert!((volume.x-heart.x-34.).abs()<0.01);
   assert!((heart.y+heart.h/2.-(volume.y+volume.h/2.)).abs()<0.001);
   for(id,r)in g.buttons{if id!=16{assert!(!overlaps(r,volume),"btn {id} overlaps volume at mix={mix}");}}
  }}}
 }
}

#[cfg(test)]mod fluid_cloud_layout_tests{use super::*;
 #[test]fn fluid_cloud_keeps_disjoint_artwork_wide_lyric_stage_and_safe_controls(){
  let overlaps=|a:Rect,b:Rect|a.x<b.x+b.w&&b.x<a.x+a.w&&a.y<b.y+b.h&&b.y<a.y+a.h;
  assert_eq!(MusicLayout::from_setting("FluidCloud"),MusicLayout::SideBySide);
  assert_eq!(MusicLayout::SideBySide.position(),0.);
  assert!(is_fluid_cloud(MusicLayout::SideBySide,0.));
  assert_eq!(immersion_amount(MusicLayout::SideBySide,0.,false),0.);
  for width in [400.,440.,592.]{for scale in [1.,1.25,1.5,2.]{for font in [11.,14.,17.,20.]{
   let g=music_geometry(width,MusicLayout::SideBySide,0.,scale,font);
   let extra=(font+8.-24.).max(0.);
   assert_eq!(g.height,music_height(0.,font));
   assert_eq!((g.cover.x,g.cover.y,g.cover.w,g.cover.h),(22.,22.,104.,104.));
   assert_eq!((g.status_dot.y,g.status_text.y),(137.,133.));
   assert_eq!(g.title.x,144.);
   assert_eq!(g.lyric.x,144.);
   for text in [g.title,g.artist,g.lyric,g.secondary,g.status_text]{assert!(!overlaps(g.cover,text));}
   assert!(!overlaps(g.lyric,g.secondary));
   assert!(g.status_text.y>=g.cover.y+g.cover.h+6.);
   assert!(g.track.y>=162.+extra-0.01);
   let find_btn=|target:i32|g.buttons.iter().find(|(id,_)|*id==target).unwrap().1;
   let (b2,b3,b4,b5,b6,b7,b9,b16)=(find_btn(2),find_btn(3),find_btn(4),find_btn(5),find_btn(6),find_btn(7),find_btn(9),find_btn(16));
   assert_eq!(b4.w,38.);assert_eq!(b4.h,38.);
   assert!((b3.y+b3.h/2.-(b4.y+b4.h/2.)).abs()<0.01&&(b5.y+b5.h/2.-(b4.y+b4.h/2.)).abs()<0.01);
   assert!(!overlaps(g.title,b6)&&!overlaps(g.title,b7));
   assert!(b2.y>g.progress_hit.y+g.progress_hit.h&&b9.x>b2.x+b2.w&&b16.x>b9.x+b9.w);
   assert!(b3.x>b16.x+b16.w);
   for i in 0..g.buttons.len(){for j in i+1..g.buttons.len(){assert!(!overlaps(g.buttons[i].1,g.buttons[j].1));}}
   for(_,r)in g.buttons{assert!(r.x>=20.&&r.x+r.w<=width-20.);assert!(r.y+r.h<=g.height-12.);}
  }}}
 }
}
