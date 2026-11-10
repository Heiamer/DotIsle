//! Deterministic frames from the production greeting, notification and update painters.
use crate::{App,banner::Banner,notifications::Notice,renderer::Renderer,motion::Frame};
use std::path::Path;
use serde_json::json;
pub unsafe fn run(a:&mut App,out:&Path)->Result<(),Box<dyn std::error::Error>>{
 std::fs::create_dir_all(out)?;a.opacity=1.;a.fade_to=1.;a.frame=Frame::hidden(a.options());a.snapshot=json!({"track":null,"isPlaying":false});a.held=false;a.manual_until=None;
 let mut r=a.renderer.take().unwrap();let(base,radius)=a.shape();let mut report=vec![];
 for restore in [false,true]{
  let dir=out.join(if restore{"notice-playing"}else{"notice-idle"});std::fs::create_dir_all(&dir)?;a.opacity=1.;a.snapshot=if restore{json!({"track":{"id":"preview","title":"海边来信"},"isPlaying":true})}else{json!({"track":null,"isPlaying":false})};
  a.banner=Banner::default();a.banner.show(Notice::example(&a.base),base,radius,a.layout.host.w,80.,0.,false);a.banner.dismiss(base,radius,0.,true);
  for frame in 0..49{let now=frame as f64/60.;a.banner.tick(base,radius,now,false,true);if !a.banner.visible(){a.finish_transient();}let(rect,rad,mix)=a.banner.presentation(now);let mut scene=a.scene();scene.notification_rect=rect;scene.notification_radius=rad;scene.notification_mix=mix;scene.time_ms=now*1000.;r.render(&scene,false)?;r.snapshot(&dir.join(format!("{frame:03}.png")))?;report.push(json!({"restore":restore,"frame":frame,"mix":mix,"opacity":a.opacity,"transientOnly":scene.transient_only}));}
 }
 a.banner=Banner::default();a.snapshot=json!({"track":null,"isPlaying":false});a.opacity=1.;a.osd_banner=Banner::default();let n=Notice{app_id:"LyricIsland.Greeting".into(),title:"下午好".into(),body:"点屿已就绪".into(),..Default::default()};a.osd_banner.show_compact(n,base,radius,a.layout.host.w,0.,false);let(rect,rad,mix)=a.osd_banner.presentation(1.);let mut scene=a.scene();scene.osd_rect=rect;scene.osd_radius=rad;scene.osd_mix=mix;r.render(&scene,false)?;r.snapshot(&out.join("greeting.png"))?;a.osd_banner=Banner::default();
 a.updates.state=json!({"phase":"applying","version":env!("CARGO_PKG_VERSION"),"message":"正在保存数据并安装，请勿关闭…"});a.selected="update".into();a.panel=Some(a.updates.panel());a.frame=Frame::expanded();a.layout.card.h=a.card_height();a.opacity=1.;
 for material in ["Acrylic","Glass"]{a.settings["IslandMaterial"]=json!(material);let dir=out.join(format!("update-{material}"));std::fs::create_dir_all(&dir)?;let mut scene=a.scene();scene.time_ms=0.;scene.lyric_age_ms=1000.;r.render(&scene,false)?;r.snapshot(&dir.join("native.png"))?;let host=a.layout.host;let scale=r.dpi as f64/96.;let export=dir.join("Cache/UpdateAppearance");r.capture_update_appearance(&scene,&export,(0,0,(host.w*scale)as i32,(host.h*scale)as i32),"preview350")?;}
 std::fs::write(out.join("report.json"),serde_json::to_vec_pretty(&json!({"success":true,"frames":report}))?)?;a.renderer=Some(r);Ok(())
}
