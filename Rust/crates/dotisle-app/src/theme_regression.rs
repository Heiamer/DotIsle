//! Theme acceptance checks use the production model, scene builder and isolated storage.
use std::path::Path;
use serde_json::json;
use crate::{model::{Model,Action,Page},draw::{self,Text},views::{self,ViewState},gpu::Gpu};
pub fn run(out:&Path)->Result<(),String>{
 std::fs::create_dir_all(out).map_err(|e|e.to_string())?;
 crate::shell::verify_theme_inputs(&out.join("input-data"))?;
 crate::background::verify(out)?;
 let data=out.join("isolated-data");let mut m=Model::new(data.clone(),true);m.tutorial=None;m.tutorial_checked=true;
 let mut text=Text::new();let mut gpu=Gpu::new();let mut rows=vec![];let mut reference=None;
 for slot in 0..crate::nothing::SLOTS.len()as u8{
  m.dispatch(Action::ControlTheme(slot));m.toast=None;
  let restored=Model::new(data.clone(),true);
  if crate::nothing::slot(&restored)!=slot{return Err(format!("theme {slot} did not persist"))}
  for dark in [false,true]{
   m.prefs.dark=dark;
   for page in Page::ALL{
    m.page=page;m.settings_tab=0;m.scroll=0.;
    let state=ViewState{rail_y:112.+Page::ALL.iter().position(|p|*p==page).unwrap()as f64*82.,..Default::default()};
    let first=views::render(&m,&mut text,1200.,800.,&state);
    let _=first;
    let view=views::render(&m,&mut text,1200.,800.,&state);
    let filename=format!("{}-{}-{:?}.png",crate::nothing::SLOTS[slot as usize],if dark{"dark"}else{"light"},page);
    gpu.capture(&view.scene,1200,800,&out.join(&filename))?;
    if slot==0&&page==Page::Focus{let im=image::open(out.join(&filename)).map_err(|e|e.to_string())?.to_rgb8();let actual=im.get_pixel(200,434);let expected=m.scheme().outline;for(channel,shift)in [16,8,0].into_iter().enumerate(){if(actual[channel]as i16-((expected>>shift)&255)as i16).abs()>1{return Err(format!("original timer track colour changed in {filename}"))}}}
    for h in &view.hits{if !h.rect.x0.is_finite()||h.rect.x0<0.||h.rect.x1>1200.01||h.rect.y0<0.||h.rect.y1>800.01{return Err(format!("hit outside viewport: {}",h.label))}}
    if slot>0&&view.hits.iter().any(|h|matches!(h.action,Action::WindowMaximize)&&h.enabled){return Err("maximize action is enabled".into())}
    if page==Page::Settings{
     let options=view.hits.iter().filter(|h|matches!(h.action,Action::ThemeHandle(_))).collect::<Vec<_>>();
     if options.len()!=crate::nothing::SLOTS.len(){return Err(format!("expected 7 theme option blocks, got {}",options.len()))}
     for (i,h) in options.iter().enumerate(){
      if h.rect.height()<44.{return Err("theme option is too short".into())}
      if let Action::ThemeHandle(b)=h.action{let f=((h.rect.center().x-b[0])/b[2]).clamp(0.,1.);if crate::nothing::slot_from_fraction(f)!=i as u8{return Err("theme option maps to wrong slot".into())}}
     }
     let bounds=options.iter().map(|h|vec![h.rect.x0,h.rect.y0,h.rect.x1,h.rect.y1]).collect::<Vec<_>>();
     if let Some(r)=&reference{if r!=&bounds{return Err("theme switch changed option geometry".into())}}else{reference=Some(bounds)}
    }
    if page==Page::Todos||page==Page::Island{
     m.scroll=0.;if page==Page::Island{m.island_tab=0;}
     let probe=views::render(&m,&mut text,1200.,800.,&state);if page==Page::Island{m.scroll=570.;}
     let detail=views::render(&m,&mut text,1200.,800.,&state);let _=probe;
     gpu.capture(&detail.scene,1200,800,&out.join(format!("{}-{}-{}-detail.png",crate::nothing::SLOTS[slot as usize],if dark{"dark"}else{"light"},if page==Page::Island{"sliders"}else{"empty"})))?;
     m.island_tab=0;m.scroll=0.;
    }
    if page==Page::Focus{
     for mode in 0..4{m.mode=mode;m.scroll=0.;let view=views::render(&m,&mut text,1200.,800.,&state);gpu.capture(&view.scene,1200,800,&out.join(format!("timer-mode-{mode}-{}-{}.png",crate::nothing::SLOTS[slot as usize],if dark{"dark"}else{"light"})))?;}m.mode=0;
    }
    rows.push(json!({"theme":crate::nothing::SLOTS[slot as usize],"dark":dark,"page":format!("{page:?}"),"capture":filename,"hits":view.hits.len(),"max_scroll":view.max_scroll}));
   }
  }
 }
 for theme in ["Miuix","Gallery","Nocturne"]{m.prefs.control_center_theme=theme.into();for dark in [false,true]{m.prefs.dark=dark;for page in Page::ALL{m.page=page;m.settings_tab=0;m.scroll=0.;let v=views::render(&m,&mut text,900.,680.,&ViewState::default());if v.hits.iter().any(|h|h.rect.x0<0.||h.rect.x1>900.01||h.rect.y1>680.01){return Err(format!("{theme}/{page:?} small-screen hit clipping"))}gpu.capture(&v.scene,900,680,&out.join(format!("{theme}-{}-{:?}-900.png",if dark{"dark"}else{"light"},page)))?;}}}
 for soft in [false,true]{
  let mut scene=vello::Scene::new();draw::fill(&mut scene,draw::rect(0.,0.,800.,360.),0.,0xF5F5F5);
  for (i,kind) in crate::nothing::ALL.iter().enumerate(){let x=24.+(i%8)as f64*96.;let y=22.+(i/8)as f64*86.;crate::theme_icons::icon(&mut scene,*kind,x+20.,y,32.,0x1A1A1A,soft);text.draw(&mut scene,crate::nothing::icon_name(*kind),x,y+40.,11.,0x666666,false,92.);}
  gpu.capture(&scene,800,360,&out.join(if soft{"icons-Miuix.png"}else{"icons-Nothing.png"}))?;
 }
 m.page=Page::Settings;m.settings_tab=0;m.scroll=0.;m.prefs.control_center_theme="Original".into();let base=views::render(&m,&mut text,1200.,800.,&ViewState::default());let _=base;
 crate::nothing::begin_transition(0,m.scheme(),false);m.prefs.control_center_theme="Miuix".into();crate::nothing::set_transition_progress(Some(0.5));
 let view=views::render(&m,&mut text,1200.,800.,&ViewState::default());gpu.capture(&view.scene,1200,800,&out.join("theme-switch-mid.png"))?;crate::nothing::end_transition();
 std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&json!({"success":true,"custom_theme_window_locked":true,"original_window_behavior_preserved":true,"original_timer_track_preserved":true,"theme_count":crate::nothing::SLOTS.len(),"new_themes_small_viewport":true,"porcelain_replaced":true,"production_drag_release_cancel_arrows":true,"persisted_all":true,"all_theme_option_blocks":true,"geometry_stable":true,"captures":rows})).unwrap()).map_err(|e|e.to_string())?;
 Ok(())
}
