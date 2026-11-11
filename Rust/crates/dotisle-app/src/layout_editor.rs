//! Local canvas drafts share their geometry and painters with the native island.
use crate::{model::{Model,Page,Action},widgets::Ui,draw::{self,rect,Icon},views::ViewState};
use lyric_core::custom_layout::{self,Layout,Module};
use serde_json::{json,Value};
use vello::kurbo::{Affine,Rect};
#[derive(Clone)]pub struct Editor{pub draft:Layout,pub baseline:Layout,pub tab:u8,pub selected:Option<u8>,pub pending:Option<Value>,pub error:String,pub undo:Vec<Layout>,pub redo:Vec<Layout>,drag:Option<(u8,f64,f64,bool)>}
impl Editor{
 pub fn open(m:&Model)->Self{let mut draft=Layout::from_value(&m.setting("MusicCustomLayout")).unwrap_or_default();if !draft.enabled{draft.transport_theme=m.setting("TransportTheme").as_str().unwrap_or("Reef").into();draft.bar_style=m.setting("ProgressBarStyle").as_str().unwrap_or("Wavy").into();draft.thumb_style=m.setting("ProgressBarThumb").as_str().unwrap_or("Pill").into();draft.thumb_enabled=m.setting("ProgressBarThumbEnabled").as_bool().unwrap_or(false);draft.thumb_spin=m.setting("ProgressBarThumbSpin").as_bool().unwrap_or(false);}Self{baseline:draft.clone(),draft,tab:0,selected:None,pending:None,error:String::new(),undo:vec![],redo:vec![],drag:None}}
 pub fn reply(&mut self,success:bool,stored:&Value,message:&str){if self.pending.is_none(){return}if !success{self.error=message.into();self.pending=None;}else if self.pending.as_ref()==Some(stored){self.baseline=self.draft.clone();self.pending=None;self.error.clear();}}
 pub fn checkpoint(&mut self){if self.undo.last()!=Some(&self.draft){self.undo.push(self.draft.clone());if self.undo.len()>40{self.undo.remove(0);}}self.redo.clear();}
 pub fn begin_drag(&mut self,id:Option<u8>){if self.pending.is_some(){return}self.checkpoint();self.selected=id;self.drag=None;}
 pub fn end_drag(&mut self,cancel:bool){if self.drag.take().is_some()&&cancel{if let Some(before)=self.undo.pop(){self.draft=before;}}}
 pub fn drag_to(&mut self,id:u8,b:[f64;3],pointer:(f64,f64),resize:bool,font:f64){
  if self.pending.is_some(){return}let Some((_,r))=custom_layout::elements(&self.draft.geometry(400.,1.,font)).into_iter().find(|(n,_)|*n==id)else{return};let(px,py)=((pointer.0-b[0])/b[2],(pointer.1-b[1])/b[2]);
  let(_,gx,gy,_)=*self.drag.get_or_insert((id,px-r.x,py-r.y,resize));self.selected=Some(id);
  if resize{let width=((px-r.x)/4.).round()*4.;self.draft.set_element(id,r.x,r.y,Some(width.clamp(24.,(388.-r.x).max(24.))));}
  else{let mut x=((px-gx)/4.).round()*4.;let y=((py-gy)/4.).round()*4.;if (x+r.w/2.-200.).abs()<6.{x=200.-r.w/2.;}x=x.clamp(12.,(388.-r.w).max(12.));self.draft.set_element(id,x,y,None);}
  self.error.clear();
 }
}
pub fn dispatch(m:&mut Model,a:&Action)->bool{
 if matches!(a,Action::LayoutOpen){if m.layout_editor.is_none(){m.layout_editor=Some(Editor::open(m));}m.page=Page::Settings;m.settings_tab=135;m.scroll=0.;return true}
 if matches!(a,Action::SettingsTab(135)){if m.layout_editor.is_none(){m.layout_editor=Some(Editor::open(m));}return false}
 let is_layout=matches!(a,Action::LayoutTab(_)|Action::LayoutMove(..)|Action::LayoutDrag(_)|Action::LayoutElement(..)|Action::LayoutResize(..)|Action::LayoutNudge(..)|Action::LayoutWidth(_)|Action::LayoutUndo|Action::LayoutRedo|Action::LayoutAuto|Action::LayoutResetElement|Action::LayoutModule(_)|Action::LayoutToggle(_)|Action::LayoutAlign(_)|Action::LayoutGap(_)|Action::LayoutCover(_)|Action::LayoutControl(_)|Action::LayoutStyle(..)|Action::LayoutReset|Action::LayoutDiscard|Action::LayoutApply);
 if !is_layout{return false}let Some(e)=m.layout_editor.as_mut()else{return true};if e.pending.is_some(){return true}
 let editing=!matches!(a,Action::LayoutTab(_)|Action::LayoutDrag(_)|Action::LayoutElement(..)|Action::LayoutResize(..)|Action::LayoutUndo|Action::LayoutRedo|Action::LayoutDiscard|Action::LayoutApply);if editing{e.checkpoint();e.draft.enabled=true;e.error.clear();}
 match a{
  Action::LayoutElement(id,_)|Action::LayoutResize(id,_)=>e.selected=Some(*id),Action::LayoutTab(tab)=>{e.tab=*tab;m.scroll=0.;},Action::LayoutDrag(_)=>{},
  Action::LayoutUndo=>{if let Some(l)=e.undo.pop(){e.redo.push(e.draft.clone());e.draft=l;}},Action::LayoutRedo=>{if let Some(l)=e.redo.pop(){e.undo.push(e.draft.clone());e.draft=l;}},
  Action::LayoutAuto=>e.draft.elements.clear(),Action::LayoutResetElement=>{if let Some(id)=e.selected{e.draft.elements.retain(|p|p.element!=id);}},
  Action::LayoutNudge(dx,dy)=>{if let Some(id)=e.selected{if let Some((_,r))=custom_layout::elements(&e.draft.geometry(400.,1.,14.)).into_iter().find(|(n,_)|*n==id){e.draft.set_element(id,(r.x+*dx as f64).clamp(12.,(388.-r.w).max(12.)),r.y+*dy as f64,None);}}},
  Action::LayoutWidth(delta)=>{if let Some(id)=e.selected{if let Some((_,r))=custom_layout::elements(&e.draft.geometry(400.,1.,14.)).into_iter().find(|(n,_)|*n==id){e.draft.set_element(id,r.x,r.y,Some((r.w+*delta as f64).clamp(24.,(388.-r.x).max(24.))));}}},
  Action::LayoutMove(from,delta)=>{let to=(*from as isize+*delta as isize).clamp(0,5)as usize;e.draft.move_module(*from,to);},
  Action::LayoutModule(i)=>{if let Some(s)=e.draft.modules.get_mut(*i){if ![Module::Header,Module::Controls].contains(&s.kind){s.visible=!s.visible;}}},
  Action::LayoutAlign(v)=>e.draft.alignment=(*v).min(2),Action::LayoutGap(d)=>e.draft.gap=(e.draft.gap+*d as f64).clamp(4.,16.),Action::LayoutCover(d)=>e.draft.cover_size=(e.draft.cover_size+*d as f64).clamp(40.,80.),
  Action::LayoutToggle(i)=>match i{0=>e.draft.enabled=!e.draft.enabled,1=>e.draft.show_cover=!e.draft.show_cover,2=>e.draft.thumb_enabled=!e.draft.thumb_enabled,3=>e.draft.thumb_spin=!e.draft.thumb_spin,_=>{}},
  Action::LayoutControl(id)=>{if *id!=4{if e.draft.controls.contains(id){e.draft.controls.retain(|n|n!=id);}else{e.draft.controls.push(*id);}}},
  Action::LayoutStyle(kind,key)=>match kind{0=>e.draft.transport_theme=key.clone(),1=>e.draft.bar_style=key.clone(),2=>e.draft.thumb_style=key.clone(),_=>{}},
  Action::LayoutReset=>{e.draft=Layout{enabled:true,..Default::default()};e.selected=None;},Action::LayoutDiscard=>{e.checkpoint();e.draft=e.baseline.clone();e.error.clear();},
  Action::LayoutApply=>{if let Err(error)=e.draft.validate(){e.error=error;return true}let value=serde_json::to_value(&e.draft).unwrap();if m.remote.is_some(){e.pending=Some(value.clone());m.native_update("MusicCustomLayout",value);}else{e.baseline=e.draft.clone();m.native_state["settings"]["MusicCustomLayout"]=value;m.message("布局已应用到设计预览");}},_=>{}
 }true
}
pub fn render(ui:&mut Ui,m:&Model,x:f64,y:f64,w:f64,state:&ViewState)->f64{
 let Some(e)=m.layout_editor.as_ref()else{return y};let s=ui.s;let busy=e.pending.is_some();let dirty=e.draft!=e.baseline;let outer=ui.clip;let toolbar=outer.y0+9.;
 ui.button(rect(x,toolbar,144.,38.),"全部设置",Some(Icon::Back),Action::SettingsTab(255),true,true);ui.label("灵动岛布局编辑器",x+164.,toolbar+6.,22.,s.on_surface,true,w-170.);
 let actions=toolbar+48.;ui.button(rect(x,actions,74.,36.),"撤销",None,Action::LayoutUndo,true,!busy&&!e.undo.is_empty());ui.button(rect(x+82.,actions,74.,36.),"重做",None,Action::LayoutRedo,true,!busy&&!e.redo.is_empty());
 ui.button(rect(x+w-350.,actions,104.,36.),"恢复推荐",None,Action::LayoutReset,true,!busy);ui.button(rect(x+w-236.,actions,104.,36.),"还原修改",None,Action::LayoutDiscard,true,dirty&&!busy);ui.button(rect(x+w-122.,actions,122.,36.),if busy{"应用中…"}else{"应用布局"},None,Action::LayoutApply,false,!busy&&dirty);
 let left=(w*0.56).min(656.);let ix=x+left+24.;let iw=w-left-24.;let py=toolbar+104.;let ph=(outer.y1-py-12.).max(260.);

 ui.clip(Rect::new(x,py,x+left,outer.y1));ui.panel(rect(x,py,left,ph),s.high);ui.label("编辑画布",x+20.,py+14.,13.,s.on_surface,true,100.);ui.label("拖动元素排版 · 4 px 吸附",x+116.,py+16.,11.,s.on_surface_variant,false,left-132.);
 let g=e.draft.geometry(400.,1.,m.setting("FontSize").as_f64().unwrap_or(14.));let scale=((left-36.)/400.).min((ph-86.)/g.height).min(1.30);let ox=x+(left-400.*scale)/2.;let oy=py+46.+(ph-86.-g.height*scale).max(0.)/2.;let tr=Affine::translate((ox,oy))*Affine::scale(scale);
 for gx in 0..((left-24.)/16.)as usize{for gy in 0..((ph-74.)/16.)as usize{draw::circle(ui.scene,x+12.+gx as f64*16.,py+44.+gy as f64*16.,0.65,s.outline_variant);}}
 preview(ui,m,&e.draft,tr,state);ui.scene.stroke(&vello::kurbo::Stroke::new(1.),Affine::IDENTITY,draw::color(s.outline_variant),None,&vello::kurbo::RoundedRect::from_rect(tr.transform_rect_bbox(rect(0.,0.,400.,g.height)),30.*scale));
 let pointer=tr.inverse()*vello::kurbo::Point::from(ui.pointer);let mut selected_rect=None;
 for(id,r)in custom_layout::elements(&g){let local=rect(r.x,r.y,r.w,r.h);let visual=tr.transform_rect_bbox(local);let hover=local.inflate(2.,2.).contains(pointer);let selected=e.selected==Some(id);
  ui.hit(visual.inflate(2.,2.),custom_layout::element_label(id),Action::LayoutElement(id,[ox,oy,scale]),!busy);
  if selected||hover{draw::outline(ui.scene,visual.inflate(2.,2.),3.,if selected{s.primary}else{s.outline},if selected{1.6}else{0.8});}
  if selected{selected_rect=Some((id,visual));}
 }
 if let Some((id,r))=selected_rect{if custom_layout::resizable(id){let handle=rect(r.x1-5.,r.center().y-5.,10.,10.);draw::fill(ui.scene,handle,3.,s.primary);draw::outline(ui.scene,handle,3.,s.surface,1.5);ui.hit(handle.inflate(5.,7.),"拖动调整元素宽度",Action::LayoutResize(id,[ox,oy,scale]),!busy);}}
 let label=if let Some(id)=e.selected{format!("{} · 方向键微调 · Ctrl+Z 撤销",custom_layout::element_label(id))}else{"点击选择元素，拖动移动；右侧选择独立样式。".into()};ui.label(&label,x+18.,py+ph-30.,11.,s.on_surface_variant,false,left-36.);ui.end_clip(outer);
 let tabs=["排版","播放样式","进度样式"];let tw=(iw-12.)/3.;for(i,label)in tabs.iter().enumerate(){ui.chip(rect(ix+i as f64*(tw+6.),py,tw,38.),label,e.tab==i as u8,Action::LayoutTab(i as u8));}
 let inspector=Rect::new(ix-3.,py+50.,outer.x1,outer.y1);ui.clip(inspector);let mut cy=py+54.-(toolbar-y);
 if !e.error.is_empty(){ui.label(&e.error,ix,cy,12.,s.error,false,iw);cy+=36.;}
 match e.tab{
 0=>{
  if let Some((id,r))=e.selected.and_then(|id|custom_layout::elements(&g).into_iter().find(|(n,_)|*n==id)){
   ui.label(custom_layout::element_label(id),ix+4.,cy,16.,s.on_surface,true,iw-8.);cy+=30.;ui.label(&format!("X {:.0}   Y {:.0}   {:.0} × {:.0}",r.x,r.y,r.w,r.h),ix+4.,cy,12.,s.on_surface_variant,false,iw-8.);cy+=32.;
   if custom_layout::resizable(id){stepper(ui,ix,&mut cy,iw,"元素宽度",r.w,"px",Action::LayoutWidth(-16),Action::LayoutWidth(16),!busy);}
   ui.button(rect(ix,cy,iw,36.),"还原此元素位置",None,Action::LayoutResetElement,true,!busy);cy+=52.;
  }
  ui.label("模块顺序与显示",ix+4.,cy,16.,s.on_surface,true,iw-8.);cy+=36.;for(i,slot)in e.draft.modules.iter().enumerate(){let r=rect(ix,cy,iw,44.);ui.panel(r,s.container);ui.hit(rect(ix+4.,cy,28.,44.),"拖动布局模块",Action::LayoutDrag(i),!busy);ui.icon(Icon::Move,ix+10.,cy+15.,14.,s.on_surface_variant);ui.label(slot.kind.label(),ix+36.,cy+12.,12.,if slot.visible{s.on_surface}else{s.on_surface_variant},true,iw-158.);let fixed=[Module::Header,Module::Controls].contains(&slot.kind);ui.button(rect(ix+iw-120.,cy+4.,32.,36.),"↑",None,Action::LayoutMove(i,-1),true,!busy&&i>0);ui.button(rect(ix+iw-82.,cy+4.,32.,36.),"↓",None,Action::LayoutMove(i,1),true,!busy&&i<5);ui.icon_button(rect(ix+iw-42.,cy+4.,36.,36.),if slot.visible{Icon::Check}else{Icon::Plus},"显示或隐藏模块",Action::LayoutModule(i),slot.visible,!fixed&&!busy);cy+=52.;}
  ui.button(rect(ix,cy,iw,38.),"清除自由位置，自动排列",None,Action::LayoutAuto,true,!busy&&!e.draft.elements.is_empty());cy+=54.;
  ui.label("自动排列的对齐方式",ix+4.,cy,13.,s.on_surface,true,iw);cy+=34.;let cw=(iw-12.)/3.;for(i,label)in ["左对齐","居中","右对齐"].iter().enumerate(){ui.chip(rect(ix+i as f64*(cw+6.),cy,cw,36.),label,e.draft.alignment==i as u8,Action::LayoutAlign(i as u8));}cy+=52.;
  stepper(ui,ix,&mut cy,iw,"模块间距",e.draft.gap,"px",Action::LayoutGap(-2),Action::LayoutGap(2),!busy);stepper(ui,ix,&mut cy,iw,"封面大小",e.draft.cover_size,"px",Action::LayoutCover(-8),Action::LayoutCover(8),!busy);ui.toggle(rect(ix,cy,iw,68.),"显示专辑封面","封面可以在画布中单独移动。",e.draft.show_cover,Action::LayoutToggle(1));cy+=78.;ui.toggle(rect(ix,cy,iw,68.),"使用自定义布局","应用后在灵动岛中使用当前设计。",e.draft.enabled,Action::LayoutToggle(0));cy+=80.;
 },
 1=>{
  ui.label("播放按钮主题",ix+4.,cy,16.,s.on_surface,true,iw);cy+=29.;ui.label("全部 10 套 · 独立于控制中心主题",ix+4.,cy,11.,s.on_surface_variant,false,iw);cy+=29.;let cw=(iw-10.)/2.;
  for(i,t)in dotisle_render::transport::TransportTheme::ALL.iter().enumerate(){let r=rect(ix+(i%2)as f64*(cw+10.),cy+(i/2)as f64*92.,cw,82.);let selected=e.draft.transport_theme==t.key();ui.panel(r,if selected{s.secondary_container}else{s.container});ui.hit(r,t.label(),Action::LayoutStyle(0,t.key().into()),!busy);transport_sample(ui,r,*t);ui.center(t.label(),rect(r.x0,r.y1-28.,cw,22.),11.,s.on_surface,true);if selected{draw::outline(ui.scene,r,20.,s.primary,1.5);}}
  cy+=470.;ui.label("显示哪些按钮",ix+4.,cy,16.,s.on_surface,true,iw);cy+=36.;for(i,(id,label))in [(2,"播放模式"),(9,"喜欢"),(3,"上一首"),(4,"播放 / 暂停"),(5,"下一首"),(6,"翻译"),(7,"设置")].iter().enumerate(){ui.chip(rect(ix+(i%2)as f64*(cw+10.),cy+(i/2)as f64*46.,cw,38.),label,e.draft.controls.contains(id),Action::LayoutControl(*id));}cy+=194.;
 },
 _=>{
  ui.label("进度条主题",ix+4.,cy,16.,s.on_surface,true,iw);cy+=32.;let cw=(iw-10.)/2.;
  for(i,b)in dotisle_render::progress::BarStyle::ALL.iter().enumerate(){let r=rect(ix+(i%2)as f64*(cw+10.),cy+(i/2)as f64*78.,cw,68.);let selected=e.draft.bar_style==b.key();ui.panel(r,if selected{s.secondary_container}else{s.container});ui.hit(r,b.detail(),Action::LayoutStyle(1,b.key().into()),!busy);bar_sample(ui,m,r,*b,dotisle_render::progress::ThumbStyle::None,state);ui.center(b.detail(),rect(r.x0+4.,r.y1-28.,cw-8.,22.),11.,s.on_surface,true);if selected{draw::outline(ui.scene,r,20.,s.primary,1.5);}}cy+=322.;
  ui.toggle(rect(ix,cy,iw,68.),"显示滑块","启用后选择滑块造型。",e.draft.thumb_enabled,Action::LayoutToggle(2));cy+=78.;if e.draft.thumb_enabled{ui.toggle(rect(ix,cy,iw,68.),"滑块缓慢旋转","减少动态效果开启时保持静止。",e.draft.thumb_spin,Action::LayoutToggle(3));cy+=80.;
   for(i,t)in dotisle_render::progress::ThumbStyle::ALL.iter().enumerate(){let r=rect(ix+(i%2)as f64*(cw+10.),cy+(i/2)as f64*76.,cw,66.);let selected=e.draft.thumb_style==t.key();ui.panel(r,if selected{s.secondary_container}else{s.container});ui.hit(r,t.label(),Action::LayoutStyle(2,t.key().into()),!busy);bar_sample(ui,m,r,dotisle_render::progress::BarStyle::Thin,*t,state);ui.center(t.label(),rect(r.x0,r.y1-26.,cw,22.),11.,s.on_surface,true);if selected{draw::outline(ui.scene,r,20.,s.primary,1.5);}}cy+=618.;
  }
 },
 }ui.end_clip(outer);cy.max(y+158.)+18.
}
fn component_scheme()->dotisle_domain::monet::Scheme{dotisle_domain::monet::scheme(0x5264a8,true)}
fn transport_sample(ui:&mut Ui,r:Rect,theme:dotisle_render::transport::TransportTheme){
 let s=component_scheme();draw::fill(ui.scene,rect(r.x0+6.,r.y0+5.,r.width()-12.,44.),12.,s.primary_container);let c=dotisle_render::transport::Colors{accent:0xff000000|s.primary,on_accent:0xff000000|s.on_primary,control:0xff000000|s.on_surface,tonal:0xff000000|s.secondary_container,ink_fill:0xff000000|s.on_surface};let center=r.center().x;
 for(id,dx,glyph)in [(3,-37.,dotisle_render::transport_icons::Glyph::Prev),(4,0.,dotisle_render::transport_icons::Glyph::Play),(5,37.,dotisle_render::transport_icons::Glyph::Next)]{let size=if id==4{34.}else{26.};let box_=rect(center+dx-size/2.,r.y0+10.+(34.-size)/2.,size,size);let ink=dotisle_render::transport::background(ui.scene,id,box_,theme,c,0.7);let edge=if id==4{20.}else{16.};dotisle_render::transport_icons::paint_icon(ui.scene,glyph,theme,rect(box_.center().x-edge/2.,box_.center().y-edge/2.,edge,edge),ink);}
}
fn bar_sample(ui:&mut Ui,m:&Model,r:Rect,bar:dotisle_render::progress::BarStyle,thumb:dotisle_render::progress::ThumbStyle,state:&ViewState){draw::fill(ui.scene,rect(r.x0+6.,r.y0+5.,r.width()-12.,33.),10.,component_scheme().primary_container);dotisle_render::progress::draw(ui.scene,rect(r.x0+15.,r.y0+14.,r.width()-30.,18.),0.52,if m.prefs.reduced_motion{0.}else{state.time*2.},0.65,12.,1.,0.,dotisle_render::progress::Look{fg:0xff000000|component_scheme().primary,track:0xff000000|component_scheme().outline,dot:0xff000000|component_scheme().primary},bar,thumb);}

fn stepper(ui:&mut Ui,x:f64,y:&mut f64,w:f64,label:&str,value:f64,unit:&str,minus:Action,plus:Action,enabled:bool){let s=ui.s;ui.panel(rect(x,*y,w,54.),s.container);ui.label(label,x+16.,*y+17.,13.,s.on_surface,true,w-180.);ui.button(rect(x+w-158.,*y+7.,38.,38.),"−",None,minus,true,enabled);ui.center(&format!("{value:.0} {unit}"),rect(x+w-120.,*y+7.,68.,38.),12.,s.on_surface,true);ui.icon_button(rect(x+w-48.,*y+7.,38.,38.),Icon::Plus,"增加",plus,false,enabled);*y+=64.;}
fn preview(ui:&mut Ui,m:&Model,l:&Layout,tr:Affine,state:&ViewState){let old=std::mem::take(ui.scene);let g=l.geometry(400.,1.,m.setting("FontSize").as_f64().unwrap_or(14.));let s=component_scheme();ui.scene.fill(vello::peniko::Fill::NonZero,Affine::IDENTITY,draw::color(s.primary_container),None,&vello::kurbo::RoundedRect::from_rect(rect(0.,0.,400.,g.height),30.));let rr=|r:lyric_core::music_layout::Rect|rect(r.x,r.y,r.w,r.h);if g.cover.w>0.{let r=rr(g.cover);if let Some(a)=&m.artwork{draw::image(ui.scene,&a.image,r,14.);}else{draw::fill(ui.scene,r,14.,s.high);draw::icon(ui.scene,Icon::Music,r.center().x-10.,r.center().y-10.,20.,s.on_surface_variant);}}let labels=[(m.title(),g.title,16.,true),(m.artist(),g.artist,10.,false),(m.current_lyric(),g.lyric,16.,false),(m.snapshot["frame"]["currentTranslation"].as_str().unwrap_or(""),g.secondary,10.5,false),(if m.playing(){"正在播放"}else{"已暂停"},g.status_text,9.,false)];for(text,r,size,bold)in labels{if r.w>0.{ui.text.line_fixed(ui.scene,text,rr(r),size,0xff000000|s.on_primary_container,if bold{600}else{400},l.alignment as f64/2.);}}if g.status_dot.w>0.{draw::circle(ui.scene,g.status_dot.x+2.,g.status_dot.y+2.,2.,s.primary);}if g.track.w>0.{let duration=m.snapshot["track"]["durationSeconds"].as_f64().unwrap_or(0.);let position=state.media_position.unwrap_or_else(||m.snapshot["positionSeconds"].as_f64().unwrap_or(0.));dotisle_render::progress::draw(ui.scene,rr(g.track),if duration>0.{position/duration}else{0.},if m.prefs.reduced_motion{0.}else{state.time*2.},0.7,14.,if m.playing()&&!m.prefs.reduced_motion{1.}else{0.},if l.thumb_spin&&!m.prefs.reduced_motion{state.time*0.25}else{0.},dotisle_render::progress::Look{fg:0xff000000|s.primary,track:0xff000000|s.outline,dot:0xff000000|s.primary},dotisle_render::progress::BarStyle::from_setting(&l.bar_style),if l.thumb_enabled{dotisle_render::progress::ThumbStyle::from_setting(&l.thumb_style)}else{dotisle_render::progress::ThumbStyle::None});ui.text.line_fixed(ui.scene,&dotisle_domain::timers::format_time(position.max(0.)as u64),rr(g.elapsed),9.,0xff000000|s.on_primary_container,400,0.);ui.text.line_fixed(ui.scene,&dotisle_domain::timers::format_time(duration.max(0.)as u64),rr(g.total),9.,0xff000000|s.on_primary_container,400,1.);}
 let theme=dotisle_render::transport::TransportTheme::from_setting(&l.transport_theme);let colors=dotisle_render::transport::Colors{accent:0xff000000|s.primary,on_accent:0xff000000|s.on_primary,control:0xff000000|s.on_primary_container,tonal:0xff000000|s.secondary_container,ink_fill:0xff000000|s.on_surface};for(id,r)in g.buttons{if r.w<=0.{continue}let r=rr(r);let ink=dotisle_render::transport::background(ui.scene,id,r,theme,colors,if m.playing(){1.}else{0.});let glyph=match id{2=>dotisle_render::transport_icons::Glyph::Shuffle,9=>dotisle_render::transport_icons::Glyph::Heart,3=>dotisle_render::transport_icons::Glyph::Prev,4=>if m.playing(){dotisle_render::transport_icons::Glyph::Pause}else{dotisle_render::transport_icons::Glyph::Play},5=>dotisle_render::transport_icons::Glyph::Next,6=>dotisle_render::transport_icons::Glyph::Translate,_=>dotisle_render::transport_icons::Glyph::Settings};let edge=if id==4{24.}else{18.};dotisle_render::transport_icons::paint_icon(ui.scene,glyph,theme,rect(r.center().x-edge/2.,r.center().y-edge/2.,edge,edge),ink);}
 let part=std::mem::replace(ui.scene,old);ui.scene.append(&part,Some(tr));
}
