pub use lyric_core::music_layout::{Rect,MusicLayout,music_geometry,mini_music_geometry,music_height,cover_corner_radius};
pub use lyric_core::island_layout::Layout;
/// Digital timer controls share their painted bounds with pointer hit testing.
pub fn timer_buttons(w:f64,h:f64)->[(i32,Rect);3]{let y=h-56.;[(31,Rect::new(20.,y,72.,40.)),(30,Rect::new(100.,y,w-200.,40.)),(7,Rect::new(w-92.,y,72.,40.))]}
pub fn todo_rows(width:f64,count:usize)->impl Iterator<Item=Rect>{(0..count.min(2)).map(move|i|Rect::new(20.,54.+i as f64*50.,width-40.,42.))}
/// 全量待办视口区域，位于顶栏与底部按钮之间，留足呼吸边距避免底行裁剪。
pub fn todo_viewport(w:f64,h:f64)->Rect{Rect::new(16.,48.,(w-32.).max(40.),(h-100.).max(44.))}
/// 任务栏微组件右侧 3 枚 Fluent 2 原生感媒体控制钮（上一首 3、播放暂停 4、下一首 5）。
pub fn taskbar_widget_buttons(capsule:Rect)->[(i32,Rect);3]{
    let size=24.;let gap=2.;let y=capsule.y+(capsule.h-size)/2.;let right=capsule.x+capsule.w-6.;
    [
        (3,Rect::new(right-size*3.-gap*2.,y,size,size)),
        (4,Rect::new(right-size*2.-gap,y,size,size)),
        (5,Rect::new(right-size,y,size,size)),
    ]
}
/// 指定待办条目在当前滚动偏移 `scroll` 下的虚拟卡片坐标。
pub fn todo_row_rect(w:f64,index:usize,scroll:f64)->Rect{Rect::new(16.,52.+index as f64*46.-scroll,(w-42.).max(40.),40.)}
/// 根据任务数量计算待办列表最大可滚动距离。
pub fn todo_max_scroll(h:f64,count:usize)->f64{if count==0{0.}else{let total=count as f64*46.-6.;(total-todo_viewport(400.,h).h+4.).max(0.)}}
/// 结合滚动偏移量判定待办项点击命中索引（视口外自动屏蔽）。
pub fn todo_hit(w:f64,h:f64,count:usize,scroll:f64,x:f64,y:f64)->Option<usize>{
    let vp=todo_viewport(w,h);
    if !vp.contains(x,y){return None;}
    (0..count).find(|&i|todo_row_rect(w,i,scroll).contains(x,y))
}
pub fn todo_footer(w:f64,h:f64)->Rect{Rect::new(20.,h-48.,w-40.,32.)}
/// Mini shares readable geometry with pointer hit testing, at half the classic area.
pub fn mini_track(w:f64,h:f64)->Rect{Rect::new(20.,h-69.,w-40.,4.)}
pub fn mini_music_buttons(w:f64,h:f64)->[(i32,Rect);5]{let y=h-43.;[(9,Rect::new(20.,y+4.,28.,28.)),(3,Rect::new(w/2.-58.,y+2.,32.,32.)),(4,Rect::new(w/2.-18.,y,36.,36.)),(5,Rect::new(w/2.+26.,y+2.,32.,32.)),(7,Rect::new(w-48.,y+4.,28.,28.))]}
pub fn mini_timer_buttons(w:f64,h:f64)->[(i32,Rect);3]{let y=h-48.;[(31,Rect::new(20.,y,72.,36.)),(30,Rect::new(100.,y,w-200.,36.)),(7,Rect::new(w-92.,y,72.,36.))]}
pub fn mini_todos(w:f64)->[Rect;1]{[Rect::new(20.,38.,w-40.,30.)]}
pub fn mini_primary(w:f64,h:f64,_kind:u8)->Rect{Rect::new(20.,h-44.,w-40.,32.)}
pub fn mini_hit(w:f64,h:f64,panel:Option<&crate::activities::Panel>,x:f64,y:f64)->i32{
    if panel.is_some_and(|p|p.kind!=3)&&activity_close(w).contains(x,y){return ACTIVITY_CLOSE}
    match panel.map(|p|p.kind){
        None=>{let t=mini_track(w,h);if Rect::new(t.x,t.y-5.,t.w,13.).contains(x,y){return 10}for(id,r)in mini_music_buttons(w,h){if r.contains(x,y){return id}}},
        Some(2)=>{for(i,r)in mini_todos(w).iter().enumerate().take(panel.map(|p|p.tasks.len()).unwrap_or(0)){if r.contains(x,y){return 40+i as i32}}if mini_primary(w,h,2).contains(x,y){return 7}},
        Some(4)=>{},
        Some(3)=>{if update_close(w).contains(x,y){return 51}if panel.is_some_and(|p|!p.tasks.is_empty())&&update_primary(w,h).contains(x,y){return 50}},
        _=>{for(id,r)in mini_timer_buttons(w,h){if r.contains(x,y){return id}}}
    }0
}
#[cfg(test)]mod mini_tests{use super::*;
    #[test]fn mini_music_targets_stay_clear_of_timeline(){for w in [400.,480.,592.]{for h in [110.,118.,122.]{for(id,r)in mini_music_buttons(w,h){assert!(r.x>=20.&&r.x+r.w<=w-20.&&r.y+r.h<=h-7.);assert!(r.y>mini_track(w,h).y+mini_track(w,h).h+6.);assert_eq!(mini_hit(w,h,None,r.x+r.w/2.,r.y+r.h/2.),id);}}}}
    #[test]fn mini_timer_hits_match_painted_buttons(){let p=crate::activities::Panel{kind:1,..Default::default()};for w in [400.,592.]{for(id,r)in mini_timer_buttons(w,132.){assert!(r.y>=80.&&r.y+r.h<=120.);assert_eq!(mini_hit(w,132.,Some(&p),r.x+r.w/2.,r.y+r.h/2.),id);}}}
    #[test]fn mini_update_close_and_primary_targets_match_the_painted_card(){let p=crate::activities::Panel{kind:3,tasks:vec![crate::activities::Task{title:"取消下载".into(),..Default::default()}],..Default::default()};for w in [400.,592.]{assert_eq!(mini_hit(w,142.,Some(&p),w-26.,22.),51);let r=update_primary(w,142.);assert_eq!(mini_hit(w,142.,Some(&p),r.x+r.w/2.,r.y+r.h/2.),50);}}
}
#[cfg(test)]mod tests{use super::*;
#[test]fn todo_content_has_safe_insets_and_nonoverlapping_targets(){for width in [340.,420.,528.]{let rows=todo_rows(width,3).collect::<Vec<_>>();let footer=todo_footer(width,220.);assert_eq!(rows[1].y+rows[1].h+26.,footer.y);assert_eq!(footer.y+footer.h,220.-16.);assert_eq!(rows[0].x,footer.x);for r in rows{assert!(!r.contains(footer.x+1.,footer.y+1.));}}}
#[test]fn todo_scroll_viewport_reveals_all_items_and_translates_hit_targets(){
    let h=220.;let w=420.;let count=7;
    let max=todo_max_scroll(h,count);
    assert!(max>100.);
    // 顶部未滚动时无法命中第 6 条，滚到底部后可精准命中第 6 条 (index 5) 与第 7 条 (index 6)
    let r5_top=todo_row_rect(w,5,0.);
    assert_eq!(todo_hit(w,h,count,0.,r5_top.x+20.,r5_top.y+10.),None);
    let r6_bot=todo_row_rect(w,6,max);
    let vp=todo_viewport(w,h);
    assert!(r6_bot.y>=vp.y && r6_bot.y+r6_bot.h<=vp.y+vp.h+2.);
    assert_eq!(todo_hit(w,h,count,max,r6_bot.x+20.,r6_bot.y+20.),Some(6));
    // 点击落入底部按钮区时不得穿透命中列表项
    let footer=todo_footer(w,h);
    assert_eq!(todo_hit(w,h,count,max,footer.x+20.,footer.y+10.),None);
}
}

#[cfg(test)] mod music_spacing_tests {use super::*;
#[test]fn compact_layouts_keep_text_controls_and_pointer_targets_disjoint(){
 for width in [400.,440.,592.]{for scale in [1.,1.25,1.5,2.]{for font in [11.,12.5,17.]{for mix in [0.,0.25,0.5,0.75,1.]{
  let g=music_geometry(width,MusicLayout::Separated,mix,scale,font);
  assert!(g.secondary.y>=g.lyric.y+g.lyric.h);
  assert!(g.track.y>=g.secondary.y+g.secondary.h+12.);
  assert!(g.buttons[3].1.y>=g.elapsed.y+g.elapsed.h+6.);
  let center=g.buttons[3].1.y+g.buttons[3].1.h/2.;
  for(id,r)in g.buttons{
   if mix>0.35||![6,7].contains(&id){assert!((r.y+r.h/2.-center).abs()<0.01,"btn {id} off baseline at mix={mix}");}
   assert!(r.y+r.h<=g.height-12.);assert!(!r.contains(g.track.x+1.,g.track.y));
  }
  assert_eq!(g.height,music_height(mix,font));
  if mix==0.{assert!(g.lyric.x>=g.cover.x+g.cover.w+16.);assert_eq!(g.title.x,g.lyric.x);}
  if mix==1.{assert!(g.lyric.y>=g.cover.y+g.cover.h+10.);assert_eq!(g.artist.y-g.title.y-g.title.h,2.);assert!(g.title.x+g.title.w+8.<=g.status_dot.x);}
 }}}}
}
#[test]fn switching_layouts_uses_the_same_live_geometry_in_both_directions(){for mix in [0.,0.2,0.5,0.8,1.]{let a=music_geometry(400.,MusicLayout::Separated,mix,1.5,12.5);let b=music_geometry(400.,MusicLayout::SideBySide,mix,1.5,12.5);assert_eq!(a.lyric,b.lyric);assert_eq!(a.cover,b.cover);assert_eq!(a.buttons,b.buttons);}}
}

#[cfg(test)]mod focus_layout_tests{use super::*;
 #[test]fn all_three_layouts_keep_lyrics_and_controls_separate(){for width in [400.,440.,592.]{for mix in [0.,0.5,1.,1.5,2.]{let g=music_geometry(width,MusicLayout::LyricFocus,mix,1.5,17.);assert!(g.lyric.y>=g.cover.y+g.cover.h||g.lyric.x>=g.cover.x+g.cover.w+16.);assert!(g.track.y>=g.secondary.y+g.secondary.h+12.);assert!(g.buttons[3].1.y>=g.elapsed.y+g.elapsed.h+6.);if mix>=1.&&g.status_dot.w>0.{assert!(g.title.x+g.title.w+8.<=g.status_dot.x);}}}}
}

#[cfg(test)] mod throw_geometry_tests {
    use super::*;
    #[test] fn toss_stays_in_host_and_retraces_the_same_path() {
        for width in [140.,260.,480.] {let l=Layout::new(width);
            assert_eq!(l.morph_shape(0.,30.),(l.capsule,l.capsule.h/2.));
            let (end,r)=l.morph_shape(1.,30.);assert!((end.x-l.card.x).abs()<1e-8);assert!((end.y-l.card.y).abs()<1e-8);assert_eq!(r,30.);
            for i in 0..=1000 {let p=i as f64/1000.;let(a,_)=l.morph_shape(p,30.);let(b,_)=l.morph_shape(1.-(1.-p),30.);
                assert!(a.x>=0.&&a.y>=0.&&a.x+a.w<=l.host.w&&a.y+a.h<=l.host.h);
                assert!((a.y-b.y).abs()<1e-8);assert!((a.w-b.w).abs()<1e-8);
            }
        }
    }
}

pub fn update_close(w:f64)->Rect{let r=dotisle_render::update_card::close(w);Rect::new(r.x0,r.y0,r.width(),r.height())}
pub fn update_skip(w:f64)->Rect{let r=dotisle_render::update_card::skip(w);Rect::new(r.x0,r.y0,r.width(),r.height())}
pub const ACTIVITY_CLOSE:i32=53;
pub fn activity_close(w:f64)->Rect{Rect::new(w-48.,14.,28.,28.)}
pub fn update_primary(w:f64,h:f64)->Rect{let r=dotisle_render::update_card::primary(w,h);Rect::new(r.x0,r.y0,r.width(),r.height())}

#[cfg(test)]mod expressive_tests{use super::*;
 #[test]fn tool_targets_remain_disjoint_inside_the_card(){for w in [340.,400.,592.]{for mini in [false,true]{let h=if mini{132.}else{220.};let buttons=if mini{mini_timer_buttons(w,h)}else{timer_buttons(w,h)};for(i,(_,a))in buttons.iter().enumerate(){assert!(a.x>=16.&&a.x+a.w<=w-16.&&a.y+a.h<=h-4.);for(_,b)in &buttons[i+1..]{assert!(a.x+a.w<=b.x||b.x+b.w<=a.x||a.y+a.h<=b.y||b.y+b.h<=a.y);}}}}
 }
 #[test]fn update_close_is_separate_from_body_and_primary(){for w in [340.,400.,592.]{for h in [132.,220.]{let a=update_close(w);let b=update_primary(w,h);assert!(a.y+a.h+8.<=b.y);assert!(b.y+b.h<=h-12.);let p=crate::activities::Panel{kind:3,tasks:vec![Default::default()],..Default::default()};assert_eq!(mini_hit(w,h,Some(&p),b.x+b.w/2.,b.y+b.h/2.),50);}}}
 #[test]fn taskbar_widget_buttons_stay_disjoint_inside_capsule(){
     let c=Rect::new(12.,280.,256.,38.);
     let btns=taskbar_widget_buttons(c);
     for (i,(id,r)) in btns.iter().enumerate(){
         assert!([3,4,5].contains(id));
         assert!(r.x>=c.x&&r.x+r.w<=c.x+c.w&&r.y>=c.y&&r.y+r.h<=c.y+c.h);
         for (_,b) in &btns[i+1..]{assert!(r.x+r.w<=b.x);}
     }
 }
}
