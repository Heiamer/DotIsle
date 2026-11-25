use crate::{draw::{self,rect,Icon},model::{Model,Action},widgets::{Ui,Hit}};
use vello::{Scene,kurbo::Rect};
#[derive(Clone,Copy,Debug,PartialEq,Eq,Default)]
pub enum TodoField{#[default]Title,Date,Time}
impl TodoField{pub fn next(self)->Self{match self{Self::Title=>Self::Date,Self::Date=>Self::Time,Self::Time=>Self::Title}}}
/// In-card draft used by the large todo widget. The native process still owns persistence.
#[derive(Clone,Debug)]
pub struct TodoEdit{pub id:Option<String>,pub title:String,pub date:String,pub time:String,pub content:String,pub daily:bool,pub color:String,pub field:TodoField,pub preedit:String,pub error:Option<String>}
impl Default for TodoEdit{fn default()->Self{Self{id:None,title:String::new(),date:String::new(),time:String::new(),content:String::new(),daily:false,color:"#F3D79B".into(),field:TodoField::Title,preedit:String::new(),error:None}}}
pub struct ViewState{pub time:f64,pub media_position:Option<f64>,pub scroll:f64,pub editor:Option<TodoEdit>}
impl Default for ViewState{fn default()->Self{Self{time:0.,media_position:None,scroll:0.,editor:None}}}
pub struct View{pub scene:Scene,pub hits:Vec<Hit>}
pub fn record(ui:&mut Ui,m:&Model,r:Rect,_flower:bool){if let Some(a)=&m.artwork{draw::image(ui.scene,&a.image,r,12.);}else{draw::fill(ui.scene,r,12.,ui.s.high);ui.icon(Icon::Music,r.center().x-10.,r.center().y-10.,20.,ui.s.on_surface_variant);}}
pub fn transport_control(ui:&mut Ui,m:&Model,r:Rect,id:i32,label:&str,action:&'static str){let icon=match id{3=>Icon::Prev,5=>Icon::Next,_=>if m.playing(){Icon::Pause}else{Icon::Play}};ui.icon_button(r,icon,label,Action::Media(action),id==4,true);}
pub fn progress(ui:&mut Ui,m:&Model,r:Rect,state:&ViewState){let duration=m.snapshot["track"]["durationSeconds"].as_f64().unwrap_or(0.);let position=state.media_position.unwrap_or(0.);let amount=if duration>0.{(position/duration).clamp(0.,1.)}else{0.};draw::fill_rounded(ui.scene,rect(r.x0,r.y0+8.,r.width(),4.),2.,ui.s.high);if amount>0.{draw::fill_rounded(ui.scene,rect(r.x0,r.y0+8.,r.width()*amount,4.),2.,ui.s.primary);}}