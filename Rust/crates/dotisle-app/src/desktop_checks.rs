//! Capture the real desktop renderer without legacy control-center assertions.
use crate::{draw::Text,gpu::Gpu,model::{Action,Model}};
use dotisle_domain::desktop_widgets::{Date,Kind,Size};
use serde_json::json;

pub fn run(out:&std::path::Path)->Result<(),String>{
 std::fs::create_dir_all(out).map_err(|e|e.to_string())?;
 let mut text=Text::new();let mut gpu=Gpu::new();let mut m=Model::new(out.join("fixture-data"),true);
 m.dispatch(Action::QuickFocus);
 m.native_state["systemMetrics"]=json!({"cpuPercent":38.0,"memoryPercent":62.0,"batteryPercent":76,"externalPower":true,"wirelessDevices":[{"id":"fixture-mouse","name":"无线鼠标","kind":"mouse","connected":true,"batteryPercent":76},{"id":"fixture-headphones","name":"蓝牙耳机","kind":"headphones","connected":true,"batteryPercent":null}]});
 let mut captures=vec![];
 for dark in [false,true]{m.prefs.dark=dark;
  for kind in Kind::ALL{
   m.dispatch(Action::WidgetAdd(kind,kind.default_size()));
   let p=m.desktop_widgets.items.iter_mut().find(|p|p.kind==kind).ok_or_else(||format!("desktop fixture was not added: {kind:?}"))?;
   p.config.title=String::new();
   if kind==Kind::Note{p.config.text="这是一段需要自动换行的长便签。记录想法、安排明天的行程，并保留每一个重要的小细节。\n第二段保留显式换行。".into();}
   if kind==Kind::Anniversary{p.config.date="2027-01-01".into();}
   let id=p.id.clone();
   for size in Size::ALL{
   let view=crate::desktop_cards::render(&m,&mut text,kind,size,&id,(-1.,-1.),false,0.,None);
   let(w,h)=size.dimensions();
   if view.hits.iter().any(|v|v.rect.x0<0.||v.rect.y0<0.||v.rect.x1>w||v.rect.y1>h){return Err(format!("hit outside {kind:?}/{size:?}"))}
   let name=format!("desktop-{kind:?}-{size:?}-{}.png",if dark{"dark"}else{"light"});
   gpu.capture(&view.scene,w as u32,h as u32,&out.join(&name))?;captures.push(name);
   }
   m.dispatch(Action::WidgetRemove(id));
  }
 }
 for(year,month,day,expected)in [(2026,12,31,[28,29,30,31,1,2,3]),(2026,1,1,[29,30,31,1,2,3,4]),(2028,3,1,[28,29,1,2,3,4,5])]{
  let date=Date{year,month,day,hour:0,minute:0,second:0};
  if crate::apple_desktop::week_days(date)!=expected{return Err("week crossed a month incorrectly".into())}
 }
 std::fs::write(out.join("report.json"),json!({"success":true,"captures":captures,"count":captures.len(),"renderer":"production Rust desktop renderer; fixture data; offscreen GPU capture","hardwareBatteryValidated":false}).to_string()).map_err(|e|e.to_string())?;
 Ok(())
}
