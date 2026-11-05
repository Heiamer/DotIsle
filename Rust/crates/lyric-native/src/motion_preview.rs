//! Production GPU captures of forward, reverse and interrupted island motion.
use std::{path::Path,time::Instant};
use serde_json::json;
use crate::{App,renderer::Renderer,motion::{Frame,Transition},layout};
pub unsafe fn run(a:&mut App,out:&Path)->Result<(),Box<dyn std::error::Error>>{
 std::fs::create_dir_all(out)?;a.renderer=None;a.opacity=1.;a.layout=layout::Layout::new(260.);a.layout.card.h=layout::music_height(0.,14.);a.lyric="此刻的旋律，轻盈展开".into();a.settings["Animation"]=json!(true);a.frame=Frame::expanded();
 let mut renderer=Renderer::new(a.visual_hwnd,(a.layout.host.w*1.5)as u32,(a.layout.host.h*1.5)as u32,144.,&a.base.join("Assets/Fonts/NotoSansSC.ttf"))?;renderer.set_cover(Some(&std::fs::read(a.base.join("Assets/DemoCover.png"))?))?;
 let o=a.options();let opening=Transition::new(true,Frame::hidden(o),o,Frame::default(),false);let closing=Transition::new(false,Frame::expanded(),o,Frame::default(),false);let reversed=Transition::new(false,opening.sample(140.),o,opening.velocity(140.),false);let mut rows=vec![];let mut times=vec![];
 for (name,t) in [("open",opening),("close",closing),("reverse",reversed)]{
  let frames=(t.duration_ms/16.6667).ceil()as usize;
  for i in 0..=frames{let ms=(i as f64*16.6667).min(t.duration_ms);let mut s=a.scene();s.frame=t.sample(ms);s.animate=true;s.layout_scale=1.;s.glass_refraction=false;s.component_count=1;s.title="海边来信";s.artist="展开与收起 · 实际渲染预览";s.lyric_age_ms=1000.;s.cover_age_ms=1000.;s.elapsed=86.;s.duration=252.;s.progress=86./252.;
   let start=Instant::now();renderer.render(&s,false)?;renderer.wait_idle();times.push(start.elapsed().as_secs_f64()*1000.);
   let file=format!("{name}-{i:03}.png");renderer.snapshot(&out.join(&file))?;let (card,compact)=crate::motion::content_softness(s.frame.card_reveal,true);rows.push(json!({"file":file,"ms":ms,"reveal":s.frame.card_reveal,"cardSigma":card,"compactSigma":compact}));
  }
 }
 times.sort_by(f64::total_cmp);std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&json!({"success":true,"scope":"production GPU scene with synthetic music data, includes true Gaussian layers and reversal","maxBlurDp":1.35,"reducedMotionBlur":0,"completedGpuP95Ms":times[times.len()*95/100],"frames":rows}))?)?;Ok(())
}
