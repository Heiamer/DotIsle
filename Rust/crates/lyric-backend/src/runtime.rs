//! Music/lyrics service wire adapter. No player workers are linked into CORE.
use std::{sync::Arc,path::PathBuf};use serde_json::{json,Value};
pub struct Backend{session:dotisle_module_api::Session}
impl Backend {
pub fn new(settings:Value,data:PathBuf,local:PathBuf,logs:PathBuf)->Arc<Self>{let(session,_)=dotisle_module_api::Session::new("music","backend",json!({"settings":settings,"data":data,"local":local,"logs":logs}));Arc::new(Self{session})}
pub fn close(&self){let _=self.session.invoke("close",json!({}));}
pub fn apply(&self,settings:Value){let _=self.session.invoke("apply",json!({"settings":settings}));}
pub fn snapshot(&self,last_cover:u64)->Value{self.session.invoke("snapshot",json!({"last_cover":last_cover})).unwrap_or_else(|e|json!({"nativeStatus":"音乐模块不可用","nativeError":e,"isPlaying":false}))}
pub fn enqueue(self:&Arc<Self>,action:&str,seconds:f64)->Result<u64,String>{serde_json::from_value(self.session.invoke("enqueue",json!({"action":action,"seconds":seconds}))?).map_err(|e|e.to_string())}
pub fn command(&self,action:&str,seconds:f64)->Result<bool,String>{serde_json::from_value(self.session.invoke("command",json!({"action":action,"seconds":seconds}))?).map_err(|e|e.to_string())}
}
