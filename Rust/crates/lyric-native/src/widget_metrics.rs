use serde_json::json;use dotisle_domain::desktop_widgets::Metrics;
pub struct Hub{session:dotisle_module_api::Session,pub value:Metrics}
impl Default for Hub{fn default()->Self{let(session,_)=dotisle_module_api::Session::new("widgets","metrics",json!({}));Self{session,value:Default::default()}}}
impl Hub{pub fn poll(&mut self){if let Ok(v)=self.session.invoke("poll",json!({})){if let Ok(value)=serde_json::from_value(v){self.value=value;}}}}
pub fn host_running(data:&std::path::Path)->bool{dotisle_module_api::call("widgets","host_running",json!({"data":data})).ok().and_then(|v|v.as_bool()).unwrap_or(false)}
