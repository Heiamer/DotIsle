use crate::{draw::{self,rect,Icon},model::{Model,Action},widgets::{Ui,Hit}};
use vello::{Scene,kurbo::Rect};
#[derive(Default)]pub struct ViewState{pub time:f64,pub media_position:Option<f64>}
pub struct View{pub scene:Scene,pub hits:Vec<Hit>}
pub fn record(ui:&mut Ui,m:&Model,r:Rect,_flower:bool){if let Some(a)=&m.artwork{draw::image(ui.scene,&a.image,r,12.);}else{draw::fill(ui.scene,r,12.,ui.s.high);ui.icon(Icon::Music,r.center().x-10.,r.center().y-10.,20.,ui.s.on_surface_variant);}}
pub fn transport_control(ui:&mut Ui,m:&Model,r:Rect,id:i32,label:&str,action:&'static str){let icon=match id{3=>Icon::Prev,5=>Icon::Next,_=>if m.playing(){Icon::Pause}else{Icon::Play}};ui.icon_button(r,icon,label,Action::Media(action),id==4,true);}
pub fn progress(ui:&mut Ui,m:&Model,r:Rect,state:&ViewState){let duration=m.snapshot["track"]["durationSeconds"].as_f64().unwrap_or(0.);let position=state.media_position.unwrap_or(0.);let amount=if duration>0.{(position/duration).clamp(0.,1.)}else{0.};draw::fill_rounded(ui.scene,rect(r.x0,r.y0+8.,r.width(),4.),2.,ui.s.high);if amount>0.{draw::fill_rounded(ui.scene,rect(r.x0,r.y0+8.,r.width()*amount,4.),2.,ui.s.primary);}}
