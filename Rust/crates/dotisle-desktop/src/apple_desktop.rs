//! One rebuilt desktop widget family. Placement/content and actions keep their
//! existing contracts; legacy style IDs all render this same neutral family.
use crate::{draw::{self,rect,Icon},widgets::Ui,model::{Action,Model,Page},views::{self,ViewState}};
use dotisle_domain::{desktop_widgets::{Kind,Size,Metrics},widget_config};
use vello::kurbo::Rect;

pub fn render(ui:&mut Ui,m:&Model,kind:Kind,r:Rect,size:Size,id:&str,state:&ViewState){
 let s=ui.s;let fg=s.on_surface;let muted=s.on_surface_variant;let blue=s.primary;
 draw::fill(ui.scene,r,24.,s.container);
 draw::outline(ui.scene,r,24.,s.high,0.6);
 let(x,y,w)=(r.x0+16.,r.y0+16.,r.width()-32.);let small=size==Size::Small;let large=size==Size::Large;
 let placement=m.desktop_widgets.items.iter().find(|p|p.id==id);
 let config=placement.map(|p|p.config.clone()).unwrap_or_default();let data=&m.native_state["widgetData"][id];
 let title=if config.title.is_empty(){kind.label()}else{&config.title};
 ui.label(title,x,y,12.,muted,true,w);
 let metrics:Metrics=serde_json::from_value(m.native_state["systemMetrics"].clone()).unwrap_or_default();
 let clock_size=if small{36.}else{46.};
 match kind {
 Kind::Clock=>{
  let d=crate::desktop_cards::date();ui.label(&format!("{:02}:{:02}",d.hour,d.minute),x,y+29.,clock_size,fg,true,w);
  ui.label(&format!("{}月{}日 · {}",d.month,d.day,d.weekday_label()),x,r.y1-36.,11.,muted,false,w);
  if large{week(ui,x,y+118.,w,d,fg,blue)}
 },
 Kind::Calendar=>{
  let d=crate::desktop_cards::date();
  if small {ui.label(d.weekday_label(),x,y+28.,13.,blue,true,w);ui.label(&d.day.to_string(),x,y+48.,58.,fg,false,w);}
  else {ui.label(&format!("{}月",d.month),x,y+29.,18.,blue,true,70.);ui.label(&d.day.to_string(),x,y+56.,48.,fg,false,74.);
   let gx=if large{x}else{x+98.};let gy=if large{y+126.}else{y+28.};let gw=if large{w}else{w-98.};let cell=gw/7.;let step=if large{25.}else{16.};
   for(i,name)in ["一","二","三","四","五","六","日"].iter().enumerate(){ui.center(name,rect(gx+i as f64*cell,gy,cell,16.),9.,muted,false)}
   for day in 1..=d.days(){let n=day as usize-1+d.first_weekday();let xx=gx+(n%7)as f64*cell;let yy=gy+20.+(n/7)as f64*step;let today=day==d.day;
    if today{draw::circle(ui.scene,xx+cell/2.,yy+step/2.,step/2.,blue)}
    ui.center(&day.to_string(),rect(xx,yy,cell,step),10.,if today{s.on_primary}else{fg},today);
   }
  }
 },
 Kind::Todos=>{
  let now=dotisle_domain::todos::Now::local();let mut items=m.todos.items.iter().filter(|t|!t.done(&now)).collect::<Vec<_>>();items.sort_by_key(|t|(&t.date,&t.time));
  if items.is_empty(){ui.label("今天，轻松一点。",x,y+45.,if small{17.}else{22.},fg,true,w)}
  let rows=if large{4}else if small{1}else{2};
  for(i,todo)in items.iter().take(rows).enumerate(){
   let yy=y+27.+i as f64*55.;let row=rect(x-6.,yy,w+12.,49.);draw::fill_rounded(ui.scene,row,16.,if i==0{s.high}else{crate::themes::mix(s.container,fg,0.025)});
   let accent=[0x57BFFF,0x9A8BEB,0xFF9EAD][i%3];draw::fill_rounded(ui.scene,rect(x+2.,yy+16.,3.,18.),1.5,accent);
   ui.label(&todo.title,x+15.,yy+7.,12.,fg,true,w-42.);ui.label(&format!("{} {}",todo.date,todo.time),x+15.,yy+27.,10.,muted,false,w-42.);
   ui.hit(rect(x+12.,yy,w-44.,49.),"编辑待办",Action::EditTodo(Some(todo.id.clone())),true);
   ui.icon_button(rect(r.x1-46.,yy+8.,30.,30.),Icon::Check,"完成待办",Action::ToggleTodo(todo.id.clone()),false,true);
  }
  if large{ui.button(rect(x,r.y1-48.,w,32.),"新建待办",Some(Icon::Plus),Action::EditTodo(None),true,true);}
 },
 Kind::Focus=>{
  let timer=m.current_timer();let(clock,detail,progress)=if let Some(t)=timer{(t.clock(),t.subtitle(),t.progress())}else{("25:00".into(),"准备开始".into(),0.)};
  let ring=if large{180.}else if small{26.}else{72.};let cx=if large{r.center().x}else{r.x1-24.-ring/2.};let cy=if large{y+132.}else if small{y+45.}else{y+60.};
  draw::ring(ui.scene,(cx,cy),ring/2.,1.,s.high,4.);draw::ring(ui.scene,(cx,cy),ring/2.,progress,blue,4.);
  if large{ui.center(&clock,rect(x,cy-30.,w,60.),42.,fg,true);}else{ui.label(&clock,x,y+23.,if small{30.}else{38.},fg,true,if small{w}else{w-ring-10.});}
  ui.label(&detail,x,if large{y+237.}else if small{y+69.}else{y+80.},10.,muted,false,if small{w-ring-8.}else{w});
  let action=timer.map(|t|Action::Timer(t.id.clone(),t.primary_action())).unwrap_or(Action::QuickFocus);
  ui.button(rect(x,r.y1-44.,w,30.),if timer.is_some_and(|t|t.running){"暂停"}else{"开始专注"},Some(if timer.is_some_and(|t|t.running){Icon::Pause}else{Icon::Play}),action,false,true);
 },
 Kind::Music=>{
  let cover=if large{100.}else if small{38.}else{60.};views::record(ui,m,rect(x,y+27.,cover,cover),false);
  let tx=if small||large{x}else{x+cover+12.};let ty=if large{y+143.}else if small{y+71.}else{y+29.};
  ui.label(m.title(),tx,ty,if small{12.}else{16.},fg,true,if small||large{w}else{w-cover-12.});
  if !small{ui.label(m.artist(),tx,ty+26.,11.,muted,false,if large{w}else{w-cover-12.});}
  if large{ui.label(m.current_lyric(),x,y+208.,13.,muted,false,w);views::progress(ui,m,rect(x,y+243.,w,20.),state);}
  for(i,action)in ["previous","toggle","next"].iter().enumerate(){let bw=32.;let gap=if small{6.}else{18.};let total=bw*3.+gap*2.;views::transport_control(ui,m,rect(r.center().x-total/2.+i as f64*(bw+gap),r.y1-43.,bw,bw),[3,4,5][i],["上一首","播放或暂停","下一首"][i],action);}
 },
 Kind::System=>{
  for(i,(title,value))in [("CPU",metrics.cpu_percent),("内存",metrics.memory_percent)].iter().enumerate(){let xx=x+if small{0.}else{i as f64*w/2.};let yy=y+if small{28.+i as f64*50.}else{32.};let cw=if small{w}else{w/2.-10.};
   ui.label(title,xx,yy,11.,muted,false,cw);ui.label(&value.map(|v|format!("{v:.0}%")).unwrap_or("—".into()),xx,yy+19.,if small{25.}else{34.},fg,true,cw);
   if large{draw::ring(ui.scene,(xx+cw/2.,y+205.),45.,1.,s.high,5.);if let Some(value)=value{draw::ring(ui.scene,(xx+cw/2.,y+205.),45.,value/100.,blue,5.);}}
  }
 },
 Kind::Wireless=>{
  let devices=metrics.wireless_devices.iter().filter(|d|d.connected).collect::<Vec<_>>();
  if devices.is_empty(){ui.icon(Icon::Volume,x,y+34.,28.,muted);ui.label("暂无连接的设备",x,y+78.,if small{14.}else{18.},fg,true,w);}
  let cols=if small{1}else{2};let cw=w/cols as f64;
  for(i,device)in devices.iter().take(if large{4}else{cols}).enumerate(){let xx=x+(i%cols)as f64*cw;let yy=y+28.+(i/cols)as f64*137.;let cx=xx+cw/2.;let radius=if small{25.}else{30.};
   draw::ring(ui.scene,(cx,yy+radius),radius,1.,s.high,4.);if let Some(level)=device.battery_percent{draw::ring(ui.scene,(cx,yy+radius),radius,level as f64/100.,if level<=20{0xF29B49}else{blue},4.);}
   device_icon(ui,(cx-12.,yy+radius-12.),&device.kind,fg);
   ui.center(&device.battery_percent.map(|v|format!("{v}%")).unwrap_or("—".into()),rect(xx,yy+radius*2.+7.,cw,27.),20.,fg,true);
   ui.center(&device.name,rect(xx,yy+radius*2.+34.,cw,18.),10.,muted,false);
  }
 },
 Kind::Note=>{
  draw::fill_rounded(ui.scene,rect(x,y+27.,w,3.),1.5,0xE9BD51);
  text_lines(ui,if config.text.is_empty(){"记下一点想法\n点击编辑便签"}else{&config.text},x,y+43.,w,if large{10}else{3},if small{13.}else{15.},fg);
 },
 Kind::Anniversary=>{
  if let Some(date)=widget_config::date_ordinal(&config.date){let delta=date-widget_config::ordinal(crate::desktop_cards::date());
   ui.label(&delta.abs().to_string(),x,y+28.,if small{44.}else{58.},fg,false,w);ui.label(if delta>=0{"天后，迎接这一天"}else{"天，已经一起走过"},x,y+104.,11.,muted,false,w);
   if large{ui.label(&config.date,x,r.y1-40.,14.,blue,false,w)}
  }else{ui.label("设置重要日期",x,y+49.,if small{16.}else{22.},fg,true,w);}
 },
 Kind::Weather=>{
  if let Some(temperature)=data["temperature"].as_f64(){ui.label(data["city"].as_str().unwrap_or(&config.city),x,y+25.,12.,muted,false,w);ui.label(&format!("{temperature:.0}°"),x,y+44.,if small{46.}else{58.},fg,false,w);if !small{ui.icon(Icon::Sun,r.x1-76.,y+40.,40.,blue)}
   ui.label(if data["stale"]==true{"数据待更新"}else{"天气"},x,r.y1-34.,11.,muted,false,w);
   if large{if let(Some(high),Some(low))=(data["high"].as_f64(),data["low"].as_f64()){ui.label(&format!("最高 {high:.0}° · 最低 {low:.0}°"),x,y+146.,13.,muted,false,w)}}
  }else{ui.icon(Icon::Sun,x,y+32.,32.,blue);ui.label("等待天气数据",x,y+83.,if small{14.}else{18.},fg,true,w);}
 },
 Kind::Today=>{
  let pending=m.todos.items.iter().filter(|t|!t.done(&dotisle_domain::todos::Now::local())).count();let timers=m.timers.items.iter().filter(|t|t.running).count();
  for(i,(value,label))in [(pending,"待办事项"),(timers,"运行中计时")].iter().enumerate(){let xx=x+if small{0.}else{i as f64*w/2.};let yy=y+if small{26.+i as f64*48.}else{30.};let cw=if small{w}else{w/2.};ui.label(&value.to_string(),xx,yy,if small{27.}else{44.},blue,true,cw);ui.label(label,xx+if small{44.}else{0.},yy+if small{12.}else{60.},11.,muted,false,cw-8.);}
  if large{ui.button(rect(x,r.y1-48.,w,32.),"查看待办事项",None,Action::Navigate(Page::Todos),true,true);}
 },
 Kind::Battery=>{
  ui.label(&metrics.battery_percent.map(|v|format!("{v}%")).unwrap_or("—".into()),x,y+30.,clock_size,fg,true,w);
  ui.label(if metrics.battery_percent.is_none(){"未检测到电池"}else if metrics.external_power==Some(true){"已连接电源"}else{"正在使用电池"},x,y+95.,11.,muted,false,w);
  if large {draw::ring(ui.scene,(r.center().x,y+220.),62.,1.,s.high,6.);if let Some(value)=metrics.battery_percent{draw::ring(ui.scene,(r.center().x,y+220.),62.,value as f64/100.,blue,6.);}}
 },
 Kind::Storage=>{
  if let(Some(free),Some(total))=(data["free"].as_f64(),data["total"].as_f64().filter(|v|*v>0.)){
   ui.label(&format!("{:.0}",free/1e9),x,y+28.,clock_size,fg,true,w);ui.label(&format!("GB 可用 · 共 {:.0} GB",total/1e9),x,y+92.,11.,muted,false,w);
   let track=rect(x,r.y1-25.,w,4.);draw::fill_rounded(ui.scene,track,2.,s.high);draw::fill_rounded(ui.scene,rect(x,track.y0,w*(1.-free/total).clamp(0.,1.),4.),2.,blue);
  }else{ui.label("暂无磁盘读数",x,y+57.,if small{16.}else{22.},fg,true,w)}
 },
 Kind::WorldClock=>{
  ui.label(data["clock"].as_str().unwrap_or("—"),x,y+30.,clock_size,fg,true,w);ui.label(data["city"].as_str().unwrap_or(if config.city.is_empty(){"选择城市"}else{&config.city}),x,y+95.,12.,muted,false,w);
  if large{ui.label(data["date"].as_str().unwrap_or("等待时间数据"),x,y+153.,13.,muted,false,w)}
 },
 Kind::Launcher=>{
  draw::circle(ui.scene,x+25.,y+58.,25.,s.high);ui.icon(Icon::Arrow,x+12.,y+45.,26.,blue);
  ui.label(if config.title.is_empty(){"常用入口"}else{&config.title},x,y+95.,if small{16.}else{22.},fg,true,w);
  if !config.target.is_empty(){ui.hit(rect(r.x0,r.y0+32.,r.width(),r.height()-32.),"打开快捷入口",Action::WidgetLaunch(id.into()),true);}else{ui.label("右键设置目标",x,r.y1-29.,10.,muted,false,w)}
 },
 Kind::Schedule=>{
  let now=crate::desktop_cards::date();let minute=widget_config::ordinal(now)*1440+now.hour as i64*60+now.minute as i64;
  if let Some(at)=widget_config::deadline_minutes(&config.deadline){
   let left=(at-minute).max(0);let value=if left==0{"已到时间".into()}else if left>=1440{format!("{}天 {}小时",left/1440,left%1440/60)}else if left>=60{format!("{}小时 {}分",left/60,left%60)}else{format!("{left}分")};
   ui.icon(Icon::Focus,x,y+29.,22.,blue);ui.label(&value,x,y+64.,if small{22.}else{30.},fg,true,w);
   ui.label(&config.deadline,x,y+110.,11.,muted,false,w);
   if large{ui.label(if left==0{"重要日程已到时间"}else{"每一刻，都更近一点"},x,r.y1-40.,12.,muted,false,w)}
  }else{ui.icon(Icon::Focus,x,y+30.,24.,blue);ui.label("设置日程",x,y+73.,22.,fg,true,w);}
 },
 Kind::Plugin=>{
  let entry=m.native_state["plugins"].as_array().and_then(|p|p.iter().find(|p|p["id"]==config.plugin_id&&p["enabled"]==true&&p["error"]==""));
  if let Some(entry)=entry{let blocks:Vec<dotisle_domain::plugins::ResolvedBlock>=serde_json::from_value(entry["views"]["desktop"].clone()).unwrap_or_default();let total=blocks.iter().map(|b|match b.kind.as_str(){"metric"=>56.,"progress"=>40.,"divider"=>12.,_=>28.}).sum::<f64>();let scale=((r.height()-64.)/total.max(1.)).min(1.);let mut yy=y+28.;
   for b in blocks{let hh=match b.kind.as_str(){"metric"=>56.,"progress"=>40.,"divider"=>12.,_=>28.};match b.kind.as_str(){
    "divider"=>draw::fill_rounded(ui.scene,rect(x,yy+5.*scale,w,1.),0.5,s.high),
    "progress"=>{ui.label(&b.label,x,yy,(11.*scale).max(9.) as f32,muted,false,w);draw::fill_rounded(ui.scene,rect(x,yy+25.*scale,w,4.),2.,s.high);if b.value>0.{draw::fill_rounded(ui.scene,rect(x,yy+25.*scale,w*b.value,4.),2.,blue);}},
    _=>{ui.label(&b.text,x,yy,(if b.kind=="metric"{26.*scale}else{13.*scale}).max(9.) as f32,fg,b.kind=="metric",w);if b.kind=="metric"{ui.label(&b.label,x,yy+34.*scale,(11.*scale).max(9.) as f32,muted,false,w);}}
   }yy+=hh*scale;}
  }else{ui.label("插件未启用或已移除",x,y+48.,12.,muted,false,w);}
 },
 Kind::ScreenTime=>{
  let stats=&m.native_state["screenTime"];
  let value=stats["todayMs"].as_f64().map(usage_duration).unwrap_or("正在连接".into());
  ui.label(&value,x,y+29.,if small{25.}else{34.},fg,true,w);ui.label("今日活跃用机",x,y+76.,11.,muted,false,w);
  let yesterday=stats["yesterdayMs"].as_f64().map(|v|format!("昨日 {}",usage_duration(v))).unwrap_or("昨日暂无记录".into());
  ui.label(&yesterday,x,y+105.,11.,muted,false,w);
  if large{
   let values=stats["week"].as_array();let peak=values.map(|a|a.iter().filter_map(|v|v.as_f64()).fold(60000.,f64::max)).unwrap_or(60000.);let bw=(w-36.)/7.;
   for i in 0..7{let n=values.and_then(|a|a.get(i)).and_then(|v|v.as_f64());let h=3.+n.unwrap_or(0.)/peak*110.;draw::fill_rounded(ui.scene,rect(x+i as f64*(bw+6.),r.y1-48.-h,bw,h),3.,if n.is_none(){s.high}else if i==6{blue}else{crate::themes::mix(s.high,blue,0.4)});}
   ui.label("近7天 · 仅统计点屿运行期间",x,r.y1-28.,9.,muted,false,w);
  }
 },
 Kind::FocusStats=>{
  let stats=&m.native_state["widgetData"]["focusStats"];let minutes=stats["todayMs"].as_f64().unwrap_or(0.)/60000.;ui.label(&format!("{minutes:.0}"),x,y+27.,clock_size,fg,true,w);ui.label("今日专注 · 分钟",x,y+90.,11.,muted,false,w);
  if large{let values=stats["week"].as_array();let maximum=values.map(|a|a.iter().filter_map(|v|v.as_f64()).fold(1.,f64::max)).unwrap_or(1.);let bw=(w-36.)/7.;for i in 0..7{let amount=values.and_then(|a|a.get(i)).and_then(|v|v.as_f64()).unwrap_or(0.);let h=4.+amount/maximum*100.;draw::fill_rounded(ui.scene,rect(x+i as f64*(bw+6.),r.y1-40.-h,bw,h),4.,if i==6{blue}else{crate::themes::mix(s.high,blue,0.3)});}}
 }
 }
 if placement.is_some()&&matches!(kind,Kind::Note|Kind::Anniversary|Kind::Schedule|Kind::Weather|Kind::Storage|Kind::WorldClock){ui.hit(rect(r.x0,r.y0+28.,r.width(),r.height()-28.),"编辑组件内容",Action::WidgetConfigure(id.into()),true);}
}
use crate::widget_extras::text_lines;
pub(crate) fn week_days(date:dotisle_domain::desktop_widgets::Date)->[u32;7]{
 let start=date.day as i32-date.weekday() as i32;
 let previous=dotisle_domain::desktop_widgets::Date{year:if date.month==1{date.year-1}else{date.year},month:if date.month==1{12}else{date.month-1},..date}.days();
 std::array::from_fn(|i|{let day=start+i as i32;if day<1{(previous as i32+day)as u32}else if day>date.days()as i32{day as u32-date.days()}else{day as u32}})
}
fn week(ui:&mut Ui,x:f64,y:f64,w:f64,date:dotisle_domain::desktop_widgets::Date,fg:u32,blue:u32){
 let cw=w/7.;for(i,day)in week_days(date).iter().enumerate(){let xx=x+i as f64*cw;let today=i==date.weekday();
 if today{draw::fill_rounded(ui.scene,rect(xx+2.,y,cw-4.,68.),14.,ui.s.high)}
 ui.center(["一","二","三","四","五","六","日"][i],rect(xx,y+7.,cw,18.),10.,fg,false);
 ui.center(&day.to_string(),rect(xx,y+32.,cw,26.),17.,if today{blue}else{fg},true);}
}

fn device_icon(ui:&mut Ui,(x,y):(f64,f64),kind:&str,ink:u32){
 use vello::{kurbo::{BezPath,Affine,Stroke},peniko::Fill};let mut path=BezPath::new();
 match kind{
  "mouse"=>{draw::outline_rounded(ui.scene,rect(x+5.,y,14.,24.),7.,ink,1.6);path.move_to((x+12.,y+3.));path.line_to((x+12.,y+9.));},
  "keyboard"=>{draw::outline_rounded(ui.scene,rect(x,y+4.,24.,16.),3.,ink,1.5);for i in 0..4{for j in 0..2{draw::circle(ui.scene,x+5.+i as f64*4.5,y+9.+j as f64*4.,0.8,ink)}}path.move_to((x+6.,y+17.));path.line_to((x+18.,y+17.));},
  _=>{path.move_to((x+4.,y+14.));path.line_to((x+4.,y+9.));path.curve_to((x+4.,y-1.),(x+20.,y-1.),(x+20.,y+9.));path.line_to((x+20.,y+14.));draw::fill_rounded(ui.scene,rect(x+2.,y+10.,5.,11.),2.5,ink);draw::fill_rounded(ui.scene,rect(x+17.,y+10.,5.,11.),2.5,ink);}
 }
 ui.scene.stroke(&Stroke::new(1.6),Affine::IDENTITY,draw::color(ink),None,&path);let _=Fill::NonZero;
}

fn usage_duration(ms:f64)->String{let m=(ms.max(0.)/60000.)as u64;if m>=60{format!("{}小时 {}分",m/60,m%60)}else{format!("{m}分钟")}}
