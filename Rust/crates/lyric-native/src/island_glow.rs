use serde_json::{json,Value};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Kind {
    #[default]
    Off,
    MusicBreath,
    MusicRhythm,
    Timer,
    Update,
}
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
pub struct Frame {
    pub kind: Kind,
    pub strength: f64,
    pub phase: f64,
}
pub struct State{session:dotisle_module_api::Session}
impl Default for State{fn default()->Self{let(session,_)=dotisle_module_api::Session::new("glow","state",json!({}));Self{session}}}
impl State{pub fn tick(&mut self,dt:f64,target:Kind,animate:bool)->bool{self.session.invoke("tick",json!({"dt":dt,"target":target,"animate":animate})).ok().and_then(|v|v.as_bool()).unwrap_or(false)}pub fn active(&self)->bool{self.session.invoke("active",json!({})).ok().and_then(|v|v.as_bool()).unwrap_or(false)}pub fn frame(&self,time_ms:f64,animate:bool)->Frame{self.session.invoke("frame",json!({"time_ms":time_ms,"animate":animate})).ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}}
pub fn paint(scene:&mut vello::Scene,bounds:vello::kurbo::Rect,radius:f64,accent:u32,frame:Frame){if let Ok(commands)=dotisle_module_api::call("glow","paint",json!({"bounds":[bounds.x0,bounds.y0,bounds.x1,bounds.y1],"radius":radius,"accent":accent,"frame":frame})){crate::module_canvas::paint(scene,&commands);}}
