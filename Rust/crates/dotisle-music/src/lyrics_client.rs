use serde_json::json;use lyric_core::models::*;
pub struct Timeline{session:dotisle_module_api::Session,previous:String,source:Option<String>}
impl Timeline{
pub fn new()->Self{let(session,_)=dotisle_module_api::Session::new("lyrics","timeline",json!({}));Self{session,previous:String::new(),source:None}}
pub fn build_stable_track_identity(t:&Track)->String{lyric_core::identity::track_key(t)}
pub fn begin_track(&mut self,t:&Track)->bool{self.session.invoke("begin",json!({"track":t})).ok().and_then(|v|v.as_bool()).unwrap_or(false)}
pub fn complete_search(&mut self,t:&Track,selection:Option<&Selection>){let _=self.session.invoke("complete",json!({"track":t,"selection":selection}));}
pub fn get_display_frame(&mut self,p:&Playback)->DisplayFrame{let reply=self.session.invoke("frame",json!({"track":p.track,"positionMs":p.position.as_millis() as u64}));match reply{Ok(v)=>{self.previous=v["previous"].as_str().unwrap_or("").into();self.source=v["source"].as_str().map(str::to_owned);serde_json::from_value(v["frame"].clone()).unwrap_or_default()},Err(e)=>DisplayFrame{current_line:format!("歌词模块不可用：{e}"),..Default::default()}}}
pub fn previous_line(&self)->&str{&self.previous}
pub fn current_lyric_source_app(&self)->Option<&str>{self.source.as_deref()}
}
