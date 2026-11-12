use serde_json::{json,Value};use dotisle_domain::desktop_widgets::Placement;
pub struct Hub{session:dotisle_module_api::Session,pub value:Value}
impl Hub{
pub fn new(path:std::path::PathBuf)->Self{let(session,reply)=dotisle_module_api::Session::new("widgets","data",json!({"path":path}));Self{session,value:reply.ok().map(|v|v["value"].clone()).unwrap_or(Value::Null)}}
fn invoke(&mut self,method:&str,args:Value){if let Ok(v)=self.session.invoke(method,args){self.value=v["value"].clone();}}
pub fn advance_focus(&mut self,timers:&[crate::timers::Timer],elapsed:u64){self.invoke("advance_focus",json!({"timers":timers,"elapsed":elapsed}));}
pub fn flush(&mut self){self.invoke("flush",json!({}));}
pub fn poll(&mut self,placements:&[Placement]){self.invoke("poll",json!({"placements":placements}));}
}
pub fn launch(placements:&[Placement],id:&str)->Value{dotisle_module_api::call("widgets","launch",json!({"placements":placements,"id":id})).unwrap_or_else(|e|json!({"success":false,"message":e}))}
