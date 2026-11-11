//! Material 3 Expressive settings rendered entirely with Vello.
use vello::{Scene,kurbo::{Rect,Affine,BezPath,Stroke},peniko::{Fill}};
use crate::{draw::{self,Icon,Text,rect},widgets::{Ui,Hit},model::{Model,Page,Action,DialogKind}};
pub const TITLEBAR_H:f64=56.;
/// Control-centre face for the configured font scope. `None` keeps the bundled
/// Google Sans Flex, which also stays the pinned metric source for every rect.
pub fn center_face(m:&Model)->Option<String>{
    if crate::swiss::active(m){return Some("Google Sans Flex".into())}
    if crate::miuix::active(m){return Some(Text::system_family("MiSans").unwrap_or_else(||"Noto Sans SC".into()))}
    if crate::pixel::active(m){return Some(crate::pixel::FACE.into())}
    if crate::terminal::active(m){return Some("Space Mono".into())}
    if crate::nothing::active(m){return Some("Space Grotesk".into())}
    if m.prefs.font_scope.as_str()=="Island"{return None}
    let stored=m.setting("IslandFontFamily");let requested=stored.as_str().unwrap_or("").trim().to_string();
    if requested.is_empty()||requested=="Noto Sans SC"||requested=="Google Sans Flex"{return None}
    Some(Text::system_family(&requested).unwrap_or(requested))
}
#[derive(Default)]pub struct ViewState{pub lyric_lines:Option<Vec<crate::music_card::PresentedLine>>,pub signature_age:Option<f64>,pub scroll_blur:f64,pub tutorial_time:Option<f64>,pub selection_values:std::collections::HashMap<String,f64>,pub press_layers:Vec<f64>,pub rail_target:Option<f64>,pub switches:std::collections::HashMap<String,f64>,pub color_scheme:Option<dotisle_domain::monet::Scheme>,pub state_layers:Vec<f32>,pub page_enter:Option<f64>,pub media_position:Option<f64>,pub pointer:(f64,f64),pub focus:Option<usize>,pub pressed:Option<usize>,pub rail_y:f64,pub satellite:f64,pub cloud_open:f64,pub time:f64,pub swipe:f64,pub maximized:bool}
pub struct View{pub motion_targets:Vec<(String,f64)>,pub scene:Scene,pub hits:Vec<Hit>,pub max_scroll:f64}
fn page_icon(page:Page)->Icon{match page{Page::Home=>Icon::Home,Page::Music=>Icon::Music,Page::Focus=>Icon::Focus,Page::Todos=>Icon::Todo,Page::Island=>Icon::Island,Page::Widgets=>Icon::Folder,Page::Notifications=>Icon::Volume,Page::Settings=>Icon::Settings}}
fn caption_button(ui:&mut Ui,x:f64,kind:Icon,label:&'static str,action:Action,close:bool){
    let s=ui.s;let r=rect(x,0.,46.,TITLEBAR_H);let hovered=r.contains(ui.pointer);let locked=ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss;ui.hit(r,label,action,!(locked&&matches!(kind,Icon::Maximize|Icon::Restore)));
    if hovered&&!(locked&&matches!(kind,Icon::Maximize|Icon::Restore)){draw::fill(ui.scene,r,0.,if close{0xC42B1C}else{s.high});}
    // Window chrome uses one shared 12px optical box and a 1.4px stroke.
    // Generic symbol viewBoxes have different ink bounds and a low minimize baseline.
    let(cx,cy)=(x+23.,TITLEBAR_H/2.);let mut path=BezPath::new();
    match kind{
      Icon::Minimize=>{path.move_to((cx-6.,cy));path.line_to((cx+6.,cy));},
      Icon::Maximize=>{path.move_to((cx-6.,cy-6.));path.line_to((cx+6.,cy-6.));path.line_to((cx+6.,cy+6.));path.line_to((cx-6.,cy+6.));path.close_path();},
      Icon::Restore=>{path.move_to((cx-3.,cy-3.));path.line_to((cx-3.,cy-6.));path.line_to((cx+6.,cy-6.));path.line_to((cx+6.,cy+3.));path.line_to((cx+3.,cy+3.));path.move_to((cx-6.,cy-3.));path.line_to((cx+3.,cy-3.));path.line_to((cx+3.,cy+6.));path.line_to((cx-6.,cy+6.));path.close_path();},
      _=>{path.move_to((cx-6.,cy-6.));path.line_to((cx+6.,cy+6.));path.move_to((cx+6.,cy-6.));path.line_to((cx-6.,cy+6.));}
    }
    ui.scene.stroke(&Stroke::new(1.4),Affine::IDENTITY,draw::color(if locked&&matches!(kind,Icon::Maximize|Icon::Restore){s.outline}else if hovered&&close{0xFFFFFF}else{s.on_surface_variant}),None,&path);
}
pub fn render(model:&Model,text:&mut Text,width:f64,height:f64,state:&ViewState)->View{
    let mut scene=Scene::new();let nothing=crate::nothing::active(model);let miuix=crate::miuix::active(model);let terminal=crate::terminal::active(model);let pixel=crate::pixel::active(model);let swiss=crate::swiss::active(model);let target=if nothing||miuix||terminal||pixel||swiss{crate::nothing::resolved(model)}else{state.color_scheme.unwrap_or_else(||model.scheme())};let s=crate::nothing::blend_to(target);let window=rect(0.,0.,width,height);
    let mut ui=Ui{selection_values:state.selection_values.clone(),motion_targets:vec![],press_layers:state.press_layers.clone(),switches:state.switches.clone(),scene:&mut scene,text,s,hits:vec![],state_layers:state.state_layers.clone(),pointer:state.pointer,focus:state.focus,pressed:state.pressed,clip:window,nothing,miuix,terminal,pixel,swiss,face:center_face(model)};
    draw::fill(ui.scene,window,0.,s.surface);crate::background::paint(ui.scene,model,rect(0.,TITLEBAR_H,width,height-TITLEBAR_H));let backdrop=ui.scene.snapshot();
    ui.icon_button(rect(width-378.,10.,36.,36.),Icon::Search,"搜索与跳转 · Ctrl+K",Action::Palette,false,true);
    ui.icon_button(rect(width-334.,10.,36.,36.),if model.prefs.dark{Icon::Sun}else{Icon::Moon},"切换明暗主题",Action::Pref("dark"),false,true);
    ui.button(rect(width-290.,10.,140.,36.),if model.prefs.island{"隐藏灵动岛"}else{"显示灵动岛"},Some(Icon::Island),Action::Pref("island"),true,true);
    caption_button(&mut ui,width-138.,Icon::Minimize,"最小化窗口",Action::WindowMinimize,false);
    caption_button(&mut ui,width-92.,if state.maximized{Icon::Restore}else{Icon::Maximize},if nothing||miuix||terminal||pixel||swiss{"窗口尺寸已固定"}else if state.maximized{"还原窗口"}else{"最大化窗口"},Action::WindowMaximize,false);
    caption_button(&mut ui,width-46.,Icon::Close,"关闭窗口",Action::WindowClose,true);
    let x=36.;let w=width-72.;
    let editing=model.page==Page::Settings&&model.settings_tab==135;
    if !editing{if swiss{crate::swiss::header(&mut ui,model,x,w);}else{
    ui.label(if terminal{crate::terminal::page(model.page)}else{model.page.label()},x,85.,if terminal{28.}else if nothing||miuix||pixel{36.}else{34.},s.on_surface,true,w-220.);
    ui.label(if model.page==Page::Home{"音乐、时间与小事，都在这里。"}else{model.page.description()},x,134.,13.,s.on_surface_variant,false,w-220.);
}
}
    if model.page==Page::Todos{ui.button(rect(width-198.,102.,162.,48.),"新建待办",Some(Icon::Plus),Action::EditTodo(None),false,true);}
    let content_top=if editing{68.}else{179.};let clip=Rect::new(x-4.,content_top,width-24.,height-if model.page==Page::Home{84.}else{112.});ui.clip(clip);
    let enter=if model.page==Page::Settings||nothing||terminal||pixel||swiss||model.prefs.reduced_motion{1.}else{state.page_enter.unwrap_or(1.)};let e=enter.clamp(0.,1.);ui.scene.push_layer(vello::peniko::Mix::Normal,(0.82+0.18*e)as f32,Affine::IDENTITY,&clip);let y=content_top+9.-model.scroll+8.*(1.-e);
    let bottom=match model.page{Page::Home=>crate::overview::render(&mut ui,model,x,y,w,state,&backdrop),Page::Widgets=>crate::desktop_cards::gallery(&mut ui,model,x,y,w,state),Page::Notifications=>crate::notification_center::render(&mut ui,model,x,y,w),Page::Music=>music(&mut ui,model,x,y,w,state),Page::Focus=>focus(&mut ui,model,x,y,w),Page::Todos=>todos(&mut ui,model,x,y,w),Page::Island=>crate::island_settings::render(&mut ui,model,x,y,w,state),Page::Settings=>crate::settings_hub::render(&mut ui,model,x,y,w,state,&backdrop)};
    ui.scene.pop_layer();ui.end_clip(window);
    if model.page==Page::Home{crate::acrylic::scroll_edge(&mut ui,rect(24.,content_top-8.,width-48.,66.),true);crate::acrylic::scroll_edge(&mut ui,rect(24.,height-139.,width-48.,55.),false);}
    let max_scroll=if model.page==Page::Home{0.}else{(bottom+model.scroll-(height-120.)).max(0.)};
    if max_scroll>0.&&model.page!=Page::Home{let track=(height-content_top-120.).max(60.);let thumb=(track*track/(track+max_scroll)).max(40.);let pos=(model.scroll/max_scroll).clamp(0.,1.)*(track-thumb);draw::fill(ui.scene,rect(width-14.,content_top+9.+pos,4.,thumb),2.,s.outline);}
    bottom_navigation(&mut ui,model,width,height,state,&backdrop);
    if nothing{if let Some((message,_))=&model.toast{ui.label(&format!("[{message}]"),x,height-145.,12.,s.on_surface_variant,false,w-120.);if model.undo.is_some(){ui.button(rect(width-144.,height-156.,108.,36.),"撤销",None,Action::Undo,true,true);}}}
    if let Some((message,_))=model.toast.as_ref().filter(|_|!nothing){let tw=(ui.text.measure(message,13.,false)+if model.undo.is_some(){136.}else{44.}).clamp(160.,w.min(480.));let r=rect(x+w-tw,height-170.,tw,48.);draw::fill(ui.scene,r,16.,s.high);draw::outline(ui.scene,r,16.,s.outline_variant,1.);ui.label(message,r.x0+18.,r.y0+13.,13.,s.on_surface,false,tw-if model.undo.is_some(){120.}else{36.});if model.undo.is_some(){let r=rect(r.x1-94.,r.y0+7.,84.,44.);let i=ui.hit(r,"撤销移除",Action::Undo,true);ui.state(r,20.,i,s.primary);ui.center("撤销",r,14.,s.primary,true);}}
    if model.palette{palette(&mut ui,model,width,height);}
    if model.dialog.is_some(){dialog(&mut ui,model,width,height);}
    if model.picker.is_some(){crate::picker::panel(&mut ui,model,width,height);}
    if model.font_picker.is_some(){crate::font_picker::panel(&mut ui,model,width,height);}
    if model.tutorial.is_some(){crate::tutorial::overlay(&mut ui,model,width,height,state);}
    if let Some(age)=state.signature_age{
        let (progress,alpha)=dotisle_render::brand_signature::ink(age,model.prefs.reduced_motion);
        let cover=rect(0.,TITLEBAR_H,width,height-TITLEBAR_H);
        ui.scene.push_layer(vello::peniko::Mix::Normal,alpha as f32,Affine::IDENTITY,&cover);
        draw::fill(ui.scene,cover,0.,s.surface);
        dotisle_render::brand_signature::draw(ui.scene,rect((width-420.)/2.,height*0.44-72.,420.,144.),progress,draw::color(s.on_surface));
        ui.center(&format!("{}",env!("CARGO_PKG_VERSION")),rect(0.,height*0.44+92.,width,24.),12.,s.on_surface_variant,false);
        ui.scene.pop_layer();ui.hits.retain(|h|h.rect.y1<=TITLEBAR_H);
    }
    let max_scroll=if model.tutorial.is_some(){0.}else{max_scroll};let motion_targets=ui.motion_targets;let hits=ui.hits;View{motion_targets,scene,hits,max_scroll}
}
pub(crate) fn record(ui:&mut Ui,m:&Model,r:Rect,_flower:bool){if let Some(a)=&m.artwork{draw::image(ui.scene,&a.image,r,if ui.pixel||ui.terminal||ui.swiss{0.}else if ui.miuix{16.}else{15.});}else if ui.pixel{crate::pixel::record(ui.scene,r,ui.s.primary,ui.s.on_surface);}else if ui.terminal{crate::terminal::record(ui.scene,r,ui.s.primary,ui.s.high);}else{draw::fill(ui.scene,r,if ui.swiss{0.}else{15.},if ui.swiss{ui.s.high}else{ui.s.secondary_container});let size=r.width().min(r.height())*0.32;ui.icon(Icon::Music,r.center().x-size/2.,r.center().y-size/2.,size,ui.s.on_surface_variant);}}
pub(crate) fn player_controls(ui:&mut Ui,m:&Model,x:f64,y:f64,large:bool){
    let size=if large{64.}else{52.};let pad=(size-44.)/2.;transport_control(ui,m,rect(x,y+pad,44.,44.),3,"上一首","previous");transport_control(ui,m,rect(x+56.,y,size,size),4,if m.playing(){"暂停播放"}else{"开始播放"},"toggle");transport_control(ui,m,rect(x+size+68.,y+pad,44.,44.),5,"下一首","next");

}
pub(crate) fn transport_control(ui:&mut Ui,m:&Model,r:Rect,id:i32,label:&str,command:&'static str){use dotisle_render::{transport::{self,TransportTheme,Colors},transport_icons::{self,Glyph}};let s=ui.s;let theme=TransportTheme::from_setting(m.setting("TransportTheme").as_str().unwrap_or("Reef"));let colors=Colors{accent:0xff000000|s.primary,on_accent:0xff000000|s.on_primary,control:0xff000000|s.on_surface,tonal:0xff000000|s.high,ink_fill:0xff000000|s.on_surface};let hit=ui.hit(r,label,Action::Media(command),m.track());let ink=transport::background(ui.scene,id,r,theme,colors,if m.playing(){1.}else{0.});ui.state(r,r.height()/2.,hit,s.on_surface);let edge=if id==4{24.}else{22.};transport_icons::paint_icon(ui.scene,match id{3=>Glyph::Prev,5=>Glyph::Next,_=>if m.playing(){Glyph::Pause}else{Glyph::Play}},theme,rect(r.center().x-edge/2.,r.center().y-edge/2.,edge,edge),if m.track(){ink}else{0x80000000|s.on_surface_variant});}
pub(crate) fn progress(ui:&mut Ui,m:&Model,r:Rect,state:&ViewState){let duration=m.snapshot["track"]["durationSeconds"].as_f64().unwrap_or(0.);let pos=state.media_position.unwrap_or_else(||m.snapshot["positionSeconds"].as_f64().unwrap_or(0.));let bar=dotisle_render::progress::BarStyle::from_setting(m.setting("ProgressBarStyle").as_str().unwrap_or("Wavy"));let thumb=if m.setting("ProgressBarThumbEnabled").as_bool().unwrap_or(false){dotisle_render::progress::ThumbStyle::from_setting(m.setting("ProgressBarThumb").as_str().unwrap_or("Pill"))}else{dotisle_render::progress::ThumbStyle::None};let spin=if m.setting("ProgressBarThumbSpin").as_bool().unwrap_or(false)&&!m.prefs.reduced_motion{state.time*0.25}else{0.};ui.slider(r,if duration>0.{pos/duration}else{0.},m.track()&&m.snapshot["canSeek"].as_bool().unwrap_or(false),if m.playing()&&!m.prefs.reduced_motion{state.time*2.}else{0.},spin,bar,thumb);ui.label(&dotisle_domain::timers::format_time(pos.max(0.)as u64),r.x0,r.y1+2.,11.,ui.s.on_surface_variant,false,80.);let total=dotisle_domain::timers::format_time(duration.max(0.)as u64);let total_width=ui.measure_alpha(&total,11.,false);ui.label_alpha(&total,r.x1-total_width,r.y1+2.,11.,ui.s.on_surface_variant,1.,false,total_width+0.5);}
fn music(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64,state:&ViewState)->f64{
    let s=ui.s;if ui.swiss{crate::swiss::music_head(ui,m,x,y,w,state);}else{ui.panel(rect(x,y,w,300.),s.primary_container);record(ui,m,rect(x+24.,y+24.,252.,252.),false);let px=x+310.;let avail=w-340.;
    ui.label(if m.demo{"音乐预览"}else if m.track(){"正在播放"}else{"等待播放器"},px,y+25.,12.,s.on_primary_container,true,avail);
    ui.label(m.title(),px,y+60.,36.,s.on_primary_container,true,avail);ui.label(m.artist(),px,y+113.,16.,s.on_primary_container,false,avail);
    progress(ui,m,rect(px,y+164.,avail,28.),state);player_controls(ui,m,px+avail/2.-88.,y+218.,true);
    }let mut y=y+if ui.swiss{364.}else{320.};
    if m.track()&&m.snapshot["canSeek"].as_bool()==Some(false){
        let status=m.snapshot["seekStatus"].as_str().unwrap_or("播放器尚未提供静默进度通道");
        ui.panel(rect(x,y,w,64.),s.container);ui.label("播放进度",x+24.,y+12.,15.,s.on_surface,true,200.);ui.label(status,x+24.,y+38.,12.,s.on_surface_variant,false,w-280.);
        if status=="播放器尚未提供静默进度通道"&&m.snapshot["sourceId"].as_str().is_some_and(|id|id.eq_ignore_ascii_case("cloudmusic.exe")){ui.button(rect(x+w-216.,y+10.,192.,44.),"查看连接状态",None,Action::NativeCommand("neteaseEnableSilent",serde_json::Value::Null),true,!m.demo);}
        y+=88.;
    }
    ui.panel(rect(x,y,w,194.),s.container);ui.label("歌词",x+26.,y+22.,13.,s.on_surface_variant,true,100.);
    let line=m.snapshot["frame"]["currentLine"].as_str().filter(|s|!s.is_empty()).unwrap_or("把耳朵交给音乐，把此刻留给自己。");
    ui.label(line,x+26.,y+if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{52.}else{56.},if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{24.}else{28.},s.on_surface,true,w-52.);
    if m.prefs.word_highlight&&m.track(){let p=m.snapshot["frame"]["wordScanProgress"].as_f64().unwrap_or(0.).clamp(0.,1.);ui.scene.push_clip_layer(Affine::IDENTITY,&rect(x+26.,y+if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{50.}else{54.},(w-52.)*p,45.));ui.label(line,x+26.,y+if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{52.}else{56.},if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{24.}else{28.},s.primary,true,w-52.);ui.scene.pop_layer();}
    if m.prefs.translation{let tr=m.snapshot["frame"]["currentTranslation"].as_str().unwrap_or("");ui.label(tr,x+28.,y+if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{110.}else{105.},14.,s.on_surface_variant,false,w-56.);}
    ui.label(m.snapshot["frame"]["nextLine"].as_str().unwrap_or("歌词会在歌曲播放后自动匹配"),x+28.,y+if ui.nothing||ui.miuix||ui.terminal||ui.pixel||ui.swiss{156.}else{147.},18.,s.on_surface_variant,false,w-56.);
    let y=y+214.;ui.panel(rect(x,y,w,72.),s.high);ui.label("歌词同步",x+24.,y+13.,15.,s.on_surface,true,200.);ui.label("向左提前，向右延后 · 每次 100 ms",x+24.,y+39.,12.,s.on_surface_variant,false,w-300.);
    ui.icon_button(rect(x+w-238.,y+14.,44.,44.),Icon::Back,"歌词提前 100 毫秒",Action::Offset(-100),false,true);ui.center(&format!("{:+} ms",m.prefs.offset_ms),rect(x+w-187.,y+14.,120.,44.),15.,s.primary,true);ui.icon_button(rect(x+w-60.,y+14.,44.,44.),Icon::Arrow,"歌词延后 100 毫秒",Action::Offset(100),false,true);y+72.
}
fn focus(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64)->f64{crate::timer_surface::render(ui,m,x,y,w)}
fn todos(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64)->f64{
    let s=ui.s;ui.label(&format!("{} 件未完成 · 共 {} 件",m.pending(),m.todos.items.len()),x,y+4.,13.,s.on_surface_variant,false,w);let now=dotisle_domain::todos::Now::local();let mut items=m.todos.items.iter().collect::<Vec<_>>();items.sort_by_key(|t|(t.done(&now),t.date.clone(),t.time.clone()));
    if items.is_empty(){ui.panel(rect(x,y+45.,w,360.),s.container);if ui.nothing||ui.terminal||ui.pixel||ui.swiss{ui.icon(Icon::Leaf,x+w/2.-22.,y+136.,44.,s.on_surface);}else{draw::circle(ui.scene,x+w/2.,y+158.,42.,s.secondary_container);ui.icon(Icon::Leaf,x+w/2.-22.,y+136.,44.,if ui.miuix{s.on_secondary_container}else{s.primary});}ui.center("心里，少一点牵挂",rect(x,y+234.,w,44.),28.,s.on_surface,true);ui.center("点击右上角的新建待办，记下第一件事。",rect(x,y+293.,w,28.),14.,s.on_surface_variant,false);return y+405.}
    let mut bottom=y+42.;for t in items{let done=t.done(&now);ui.panel(rect(x,bottom,w,96.),if done{s.container}else{s.high});ui.icon_button(rect(x+16.,bottom+26.,44.,44.),Icon::Check,if done{"恢复待办"}else{"完成待办"},Action::ToggleTodo(t.id.clone()),done,true);let title=if done{format!("已完成 · {}",t.title)}else{t.title.clone()};ui.label(&title,x+78.,bottom+17.,18.,if done{s.on_surface_variant}else{s.on_surface},!done,w-248.);ui.label(&format!("{}  {}{}{}",t.date,t.time,if t.daily{" · 每天"}else{""},if t.content.is_empty(){String::new()}else{format!("  /  {}",t.content)}),x+78.,bottom+54.,13.,s.on_surface_variant,false,w-248.);ui.button(rect(x+w-156.,bottom+26.,88.,44.),"编辑",None,Action::EditTodo(Some(t.id.clone())),true,true);ui.icon_button(rect(x+w-60.,bottom+26.,44.,44.),Icon::Trash,"移除待办",Action::DeleteTodo(t.id.clone()),false,true);bottom+=108.;}bottom
}
pub(crate) fn settings(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64)->f64{
    let s=ui.s;let mut cy=y;
    let mut group=|ui:&mut Ui,title:&str,rows:Vec<(&str,&str,bool,&'static str)>|{ui.label(title,x,cy+5.,20.,s.on_surface,true,w);cy+=48.;let h=rows.len()as f64*80.+16.;ui.panel(rect(x,cy,w,h),s.container);for(i,(title,detail,on,key))in rows.iter().enumerate(){ui.toggle(rect(x+24.,cy+8.+i as f64*80.,w-48.,72.),title,detail,*on,Action::Pref(key));if i+1<rows.len(){draw::line(ui.scene,(x+24.,cy+88.+i as f64*80.),(x+w-24.,cy+88.+i as f64*80.),s.outline,0.5);}}cy+=h+28.;};
    match m.settings_tab{
        0=>{crate::preferences_ui::theme_options(ui,m,x,&mut cy,w);crate::background::settings(ui,m,x,&mut cy,w);cy=crate::appearance::render(ui,m,x,cy,w);},
        1=>{group(ui,"聆听偏好",vec![("显示翻译","有译文时在当前歌词下方显示",m.prefs.translation,"translation"),("逐字高亮","歌词随歌曲进度渐次点亮",m.prefs.word_highlight,"words"),("在线歌曲补全","使用现有音乐服务补全歌曲信息和歌词",m.prefs.online,"online")]);
            ui.panel(rect(x,cy,w,88.),s.container);ui.label("本地歌词",x+24.,cy+16.,17.,s.on_surface,true,w-220.);ui.label(&format!("{} 个文件夹 · 支持本地歌词优先匹配",m.prefs.local_folders.len()),x+24.,cy+48.,12.,s.on_surface_variant,false,w-220.);ui.button(rect(x+w-172.,cy+22.,148.,44.),"管理文件夹",Some(Icon::Folder),Action::Paths,true,true);cy+=116.;
            ui.label("同步偏移与播放器来源在「音乐与歌词」中统一管理。",x,cy,13.,s.on_surface_variant,false,w);cy+=30.;},
        2=>{},
        3=>{cy=crate::software_update::render(ui,m,x,cy,w);

            if let Some(e)=m.prefs_error.as_ref().or(m.todos.error.as_ref()).or(m.timers.error.as_ref()){ui.label(e,x,cy,14.,s.error,true,w);cy+=56.;}
            ui.button(rect(x,cy,168.,48.),"关闭设置",Some(Icon::Close),Action::Quit,true,true);cy+=60.;},_=>{}
    }crate::preferences_ui::extra(ui,m,x,cy,w,m.settings_tab)
}
fn dialog(ui:&mut Ui,m:&Model,width:f64,height:f64){
    let Some(d)=&m.dialog else{return};let s=ui.s;ui.hits.clear();ui.scene.fill(Fill::NonZero,Affine::IDENTITY,draw::alpha(0,0.38),None,&rect(0.,0.,width,height));
    if matches!(d.kind,DialogKind::WorkflowTemplates){templates_dialog(ui,m,width,height);return}if matches!(d.kind,DialogKind::WorkflowModules){crate::tool_preferences::modules_dialog(ui,m,width,height);return}
    let h=match d.kind{DialogKind::TimerPreferences=>496.,DialogKind::WorkflowModule(_)|DialogKind::WorkflowModules=>340.,DialogKind::QuickReminder=>380.,DialogKind::Rule(_)=>620.,DialogKind::Todo(_)=>552.,DialogKind::Paths=>300.,DialogKind::Import|DialogKind::Setting(_)=>320.,DialogKind::WorkflowStage(_)|DialogKind::WorkflowTemplate(_)=>360.,DialogKind::WorkflowTemplates=>360.};let w=560.;let x=(width-w)/2.;let y=(height-h)/2.;ui.panel(rect(x,y,w,h),s.surface);
    let title=match &d.kind{DialogKind::TimerPreferences=>"计时参数",DialogKind::WorkflowModules=>"流程模块库",DialogKind::WorkflowModule(index)=>if index.is_some(){"编辑模块"}else{"新建模块"},DialogKind::QuickReminder=>"一句话快速提醒",DialogKind::Rule(id)=>if id.is_some(){"编辑应用提醒"}else{"新建应用提醒"},DialogKind::Todo(id)=>if id.is_some(){"编辑待办"}else{"记下一件事"},DialogKind::Paths=>"本地歌词文件夹",DialogKind::Import=>"导入 2.x 数据",DialogKind::Setting(_)=>"调整设置",DialogKind::WorkflowStage(Some(_))=>"编辑阶段",DialogKind::WorkflowStage(None)=>"添加阶段",DialogKind::WorkflowTemplate(_)=>"保存流程模板",DialogKind::WorkflowTemplates=>"流程模板"};ui.label(title,x+28.,y+24.,28.,s.on_surface,true,w-110.);ui.icon_button(rect(x+w-68.,y+20.,44.,44.),Icon::Close,"取消",Action::CancelModal,false,true);
    if matches!(d.kind,DialogKind::Import){ui.label("从本机旧版点屿导入偏好、待办和计时器。",x+28.,y+94.,15.,s.on_surface,false,w-56.);ui.label("V3 已有数据时将拒绝导入。旧文件不会被修改。",x+28.,y+131.,13.,s.on_surface_variant,false,w-56.);ui.label("旧版材质、窗口布局和动画调参不会迁移。",x+28.,y+164.,13.,s.on_surface_variant,false,w-56.);}
    for(i,f)in d.fields.iter().enumerate(){let fy=y+88.+i as f64*if matches!(d.kind,DialogKind::Rule(_)){74.}else{84.};let r=rect(x+28.,fy,w-56.,68.);draw::fill(ui.scene,r,12.,s.high);if d.active==i{draw::outline(ui.scene,r,12.,s.primary,2.);}ui.label(f.label,r.x0+14.,fy+8.,11.,s.primary,true,w-88.);let caret=f.cursor.min(f.value.len());if f.selected&&d.active==i{draw::fill(ui.scene,rect(r.x0+10.,fy+27.,r.width()-20.,28.),6.,s.secondary_container);}let display=if d.active==i{format!("{}{}│{}",&f.value[..caret],d.preedit,&f.value[caret..])}else{f.value.clone()};let count=display.chars().count();let tail=if count>52{format!("…{}",display.chars().skip(count-52).collect::<String>())}else{display};ui.label(&tail,r.x0+14.,fy+28.,15.,s.on_surface,false,w-88.);ui.hit(r,f.label,Action::Field(i),true);}
    if matches!(d.kind,DialogKind::TimerPreferences){ui.chip(rect(x+28.,y+356.,196.,38.),"番茄钟正向计时",d.daily,Action::Daily);ui.label("时长参数用于下一次新建计时。",x+28.,y+404.,12.,s.on_surface_variant,false,w-56.);}
    if matches!(d.kind,DialogKind::QuickReminder){
        let q=&m.quick_preview;ui.label("支持几分钟后、今天 / 明天 HH:MM，以及指定日期。",x+28.,y+166.,12.,s.on_surface_variant,false,w-56.);
        if q["success"]==true{ui.label(&format!("{} {} · {}",q["date"].as_str().unwrap_or(""),q["time"].as_str().unwrap_or(""),q["title"].as_str().unwrap_or("")),x+28.,y+206.,17.,s.primary,true,w-56.);ui.button(rect(x+28.,y+250.,196.,44.),"确认创建提醒",Some(Icon::Check),Action::QuickCommit,false,true);}
    }
    if matches!(d.kind,DialogKind::Rule(_)){ui.chip(rect(x+28.,y+470.,232.,38.),"按前台累计时长触发",d.daily,Action::Daily);ui.label(if d.daily{"当前方式：每天累计前台使用时长"}else{"当前方式：进入应用时提醒"},x+280.,y+480.,12.,s.on_surface_variant,false,w-308.);}
    if matches!(d.kind,DialogKind::Todo(_)){ui.chip(rect(x+28.,y+432.,128.,40.),"每天重复",d.daily,Action::Daily);}
    if let DialogKind::WorkflowTemplate(Some(_))=&d.kind{ui.chip(rect(x+28.,y+176.,176.,40.),"覆盖已加载模板",d.daily,Action::Daily);ui.label(if d.daily{"保存后更新当前模板；正在运行的流程实例不受影响。"}else{"不覆盖时，会另存为一个新模板。"},x+28.,y+226.,12.,s.on_surface_variant,false,w-56.);}
    if !d.error.is_empty(){ui.label(&d.error,x+28.,y+h-104.,12.,s.error,true,w-56.);}
    let save=match &d.kind{DialogKind::QuickReminder=>"解析时间",DialogKind::Import=>"开始导入",DialogKind::WorkflowStage(_)=>"保存阶段",DialogKind::WorkflowTemplate(_)=>"保存模板",_=>"保存"};
    ui.button(rect(x+w-248.,y+h-70.,96.,44.),"取消",None,Action::CancelModal,true,true);ui.button(rect(x+w-140.,y+h-70.,112.,44.),save,None,Action::SaveModal,false,true);
}
fn templates_dialog(ui:&mut Ui,m:&Model,width:f64,height:f64){
    let s=ui.s;let templates=&m.workflow.templates;let shown=templates.len().min(16);let rows=shown.div_ceil(2);let w=620.;let h=(100.+rows as f64*58.+76.).max(250.).min(height-40.);let x=(width-w)/2.;let y=(height-h)/2.;
    ui.panel(rect(x,y,w,h),s.surface);
    ui.label("流程模板",x+28.,y+24.,28.,s.on_surface,true,w-110.);ui.icon_button(rect(x+w-68.,y+20.,44.,44.),Icon::Close,"关闭模板库",Action::CancelModal,false,true);
    ui.label(if templates.is_empty(){"还没有保存的模板。"}else{"加载会复制成当前草稿；删除不影响正在运行的流程。"},x+28.,y+74.,13.,s.on_surface_variant,false,w-56.);
    if templates.is_empty(){ui.label("编排好阶段后，点「保存为模板」即可在这里复用。",x+28.,y+126.,15.,s.on_surface,false,w-56.);}
    else{let cw=(w-64.)/2.;for(i,t)in templates.iter().take(16).enumerate(){let cx=x+28.+(i%2)as f64*(cw+8.);let cy=y+104.+(i/2)as f64*58.;let r=rect(cx,cy,cw,50.);draw::fill(ui.scene,r,14.,s.high);ui.label(&t.name,cx+14.,cy+6.,15.,s.on_surface,true,cw-160.);ui.label(&crate::workflow::summary(&t.stages),cx+14.,cy+28.,11.,s.on_surface_variant,false,cw-160.);ui.button(rect(cx+cw-136.,cy+7.,76.,36.),"加载",None,Action::WorkflowLoadTemplate(t.id.clone()),true,true);ui.icon_button(rect(cx+cw-52.,cy+7.,40.,36.),Icon::Trash,"删除模板",Action::WorkflowDeleteTemplate(t.id.clone()),false,true);}}
    ui.button(rect(x+w-140.,y+h-62.,112.,44.),"关闭",None,Action::CancelModal,true,true);
}
fn palette(ui:&mut Ui,m:&Model,width:f64,height:f64){let s=ui.s;ui.hits.clear();ui.scene.fill(Fill::NonZero,Affine::IDENTITY,draw::alpha(0,0.30),None,&rect(0.,0.,width,height));let w=560.;let x=(width-w)/2.;let panel_h=100.+Page::ALL.len()as f64*60.;let y=((height-panel_h)/2.).max(24.);ui.panel(rect(x,y,w,panel_h),s.surface);ui.icon(Icon::Search,x+24.,y+25.,24.,s.primary);ui.label(&format!("{}│",if m.query.is_empty(){"搜索页面或设置"}else{&m.query}),x+64.,y+23.,20.,s.on_surface,true,w-140.);ui.icon_button(rect(x+w-64.,y+16.,44.,44.),Icon::Close,"关闭搜索",Action::Palette,false,true);let mut cy=y+84.;for page in Page::ALL{if !m.query.is_empty()&&!page.label().contains(&m.query)&&!page.description().contains(&m.query){continue}let r=rect(x+16.,cy,w-32.,56.);let i=ui.hit(r,page.label(),Action::Navigate(page),true);ui.state(r,16.,i,s.on_surface);ui.icon(page_icon(page),x+30.,cy+16.,24.,s.primary);ui.label(page.label(),x+76.,cy+14.,17.,s.on_surface,false,w-120.);cy+=60.;}if cy==y+84.{ui.label("没有找到页面，试试“音乐”或“偏好”。",x+28.,cy+20.,14.,s.on_surface_variant,false,w-56.);}}

fn bottom_navigation(ui:&mut Ui,m:&Model,width:f64,height:f64,state:&ViewState,backdrop:&Scene){
 let s=ui.s;let w=(width-96.).min(744.);let r=rect((width-w)/2.,height-94.,w,72.);
 crate::acrylic::surface(ui,r,36.,backdrop,Affine::IDENTITY,m.prefs.dark);
 let cw=(w-16.)/6.;let selected=Page::ALL.iter().position(|p|*p==m.page).unwrap_or(5)as f64;
 let position=if m.prefs.reduced_motion{selected}else{if state.rail_y>=112.{((state.rail_y-112.)/82.).clamp(0.,5.)}else{selected}};
 let selected=rect(r.x0+8.+cw*position,r.y0+8.,cw,56.);let classic=!ui.nothing&&!ui.miuix&&!ui.terminal&&!ui.pixel&&!ui.swiss;
 if classic{ui.scene.draw_blurred_rounded_rect(Affine::translate((0.,2.)),selected,draw::alpha(0,if m.prefs.dark{0.15}else{0.055}),28.,5.);draw::fill(ui.scene,selected,28.,s.high);ui.scene.stroke(&Stroke::new(0.7),Affine::IDENTITY,draw::alpha(if m.prefs.dark{0xffffff}else{0x000000},if m.prefs.dark{0.06}else{0.035}),None,&draw::continuous_rect(selected,28.));}else{draw::fill(ui.scene,selected,28.,s.secondary_container);}
 for(i,page)in Page::ALL.iter().enumerate(){let item=rect(r.x0+8.+i as f64*cw,r.y0+8.,cw,56.);let hit=ui.hit(item,page.label(),Action::Navigate(*page),true);ui.state(item,28.,hit,s.on_surface);let ink=if m.page==*page{if classic{s.primary}else{s.on_secondary_container}}else{s.on_surface_variant};ui.icon(page_icon(*page),item.center().x-10.,item.y0+6.,20.,ink);ui.center(page.label(),rect(item.x0,item.y0+29.,item.width(),22.),11.,ink,m.page==*page);}
}
