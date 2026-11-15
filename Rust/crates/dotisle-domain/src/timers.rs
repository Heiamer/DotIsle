//! Host-side focus wire adapter. Business logic lives in dotisle_focus.dll.
use serde::{Serialize,Deserialize};use serde_json::{json,Value};use std::path::PathBuf;
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    pub name: String,
    pub seconds: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Workflow {
    #[serde(default)]
    pub stages: Vec<Stage>,
    #[serde(default)]
    pub stage_index: usize,
    #[serde(default)]
    pub waiting: bool,
    #[serde(default)]
    pub template_id: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Timer {
    pub id: String,
    pub name: String,
    pub mode: String,
    pub duration: u64,
    pub focus: u64,
    pub rest: u64,
    pub rounds: u32,
    pub round: u32,
    pub phase: String,
    pub elapsed_ms: u64,
    pub running: bool,
    pub finished: bool,
    #[serde(default)]
    pub todo_id: Option<String>,
    #[serde(default)]
    pub workflow: Option<Workflow>,
    #[serde(default)]
    pub pause_reason: Option<String>,
    #[serde(default)]
    pub revision: u64,
    #[serde(default)]
    pub laps: Vec<u64>,
    #[serde(default)]
    pub count_up: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Template {
    pub id: String,
    pub name: String,
    pub stages: Vec<Stage>,
}
fn request<T:serde::de::DeserializeOwned>(method:&str,args:Value)->Result<T,String>{serde_json::from_value(dotisle_module_api::call("focus",method,args)?).map_err(|e|e.to_string())}
impl Timer {
pub fn primary_action(&self)->&'static str{match request::<String>("timer.primary_action",json!({"timer":self})).unwrap_or_default().as_str(){"restart"=>"restart","next"=>"next","pause"=>"pause",_=>"resume"}}
pub fn limit_ms(&self)->u64{request("timer.limit_ms",json!({"timer":self})).unwrap_or_else(|_|0)}
pub fn remaining_ms(&self)->u64{request("timer.remaining_ms",json!({"timer":self})).unwrap_or_else(|_|0)}
pub fn clock(&self)->String{request("timer.clock",json!({"timer":self})).unwrap_or_else(|_|String::new())}
pub fn progress(&self)->f64{request("timer.progress",json!({"timer":self})).unwrap_or_else(|_|0.)}
pub fn subtitle(&self)->String{request("timer.subtitle",json!({"timer":self})).unwrap_or_else(|_|String::new())}
}
pub fn format_time(s:u64)->String{request("format_time",json!({"s":s})).unwrap_or_default()}
pub fn stages_from(value:&Value)->Result<Vec<Stage>,String>{request("stages_from",value.clone())}
pub fn stages_ok(stages:&[Stage])->bool{request("stages_ok",json!(stages)).unwrap_or_default()}
pub struct Store{session:dotisle_module_api::Session,pub items:Vec<Timer>,pub error:Option<String>}
impl Store{pub fn new(path:PathBuf)->Self{let (session,reply)=dotisle_module_api::Session::new("focus","store",json!({"path":path}));let mut this=Self{session,items:vec![],error:None};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(items)=serde_json::from_value(v["items"].clone()){self.items=items;}self.error=v["error"].as_str().map(str::to_owned);}
fn invoke(&mut self,method:&str,mut args:Value)->Result<Value,String>{args["items"]=json!(self.items);let reply=self.session.invoke(method,args)?;self.sync(&reply);if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}Ok(reply["result"].clone())}
pub fn persist(&self)->Result<(),String>{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("persist",a)?;if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}serde_json::from_value(reply["result"].clone()).map_err(|e|e.to_string())}
pub fn list(&self)->Value{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("list",a);reply.ok().map(|v|v["result"].clone()).unwrap_or(Value::Null)}
pub fn create(&mut self,v:&Value)->Result<String,String>{let value=self.invoke("create",json!({"v":v}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn action(&mut self,id:&str,action:&str)->Result<(),String>{let value=self.invoke("action",json!({"id":id,"action":action}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn act(&mut self,id:&str,action:&str,revision:Option<u64>)->Result<(),String>{let value=self.invoke("act",json!({"id":id,"action":action,"revision":revision}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn set_direction(&mut self,id:&str,count_up:bool,revision:Option<u64>)->Result<(),String>{let value=self.invoke("set_direction",json!({"id":id,"count_up":count_up,"revision":revision}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn advance(&mut self,ms:u64)->Vec<Timer>{let value=self.invoke("advance",json!({"ms":ms}));value.ok().and_then(|v|serde_json::from_value(v).ok()).unwrap_or_default()}
pub fn pause_for_lock(&mut self,generation:u64)->Result<Vec<String>,String>{let value=self.invoke("pause_for_lock",json!({"generation":generation}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn resume_focus(&mut self,ids:&[String],generation:u64)->Result<Vec<String>,String>{let value=self.invoke("resume_focus",json!({"ids":ids,"generation":generation}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn clear_lock_pauses(&mut self)->Result<(),String>{let value=self.invoke("clear_lock_pauses",json!({}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
}
pub struct Templates{session:dotisle_module_api::Session,pub items:Vec<Template>,pub error:Option<String>}
impl Templates{pub fn new(path:PathBuf)->Self{let (session,reply)=dotisle_module_api::Session::new("focus","templates",json!({"path":path}));let mut this=Self{session,items:vec![],error:None};match reply{Ok(v)=>this.sync(&v),Err(e)=>this.error=Some(e)};this}
fn sync(&mut self,v:&Value){if let Ok(items)=serde_json::from_value(v["items"].clone()){self.items=items;}self.error=v["error"].as_str().map(str::to_owned);}
fn invoke(&mut self,method:&str,mut args:Value)->Result<Value,String>{args["items"]=json!(self.items);let reply=self.session.invoke(method,args)?;self.sync(&reply);if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}Ok(reply["result"].clone())}
pub fn persist(&self)->Result<(),String>{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("persist",a)?;if let Some(e)=reply["operationError"].as_str(){return Err(e.into());}serde_json::from_value(reply["result"].clone()).map_err(|e|e.to_string())}
pub fn list(&self)->Value{let mut a=json!({});a["items"]=json!(self.items);let reply=self.session.invoke("list",a);reply.ok().map(|v|v["result"].clone()).unwrap_or(Value::Null)}
pub fn save(&mut self,v:&Value)->Result<String,String>{let value=self.invoke("save",json!({"v":v}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
pub fn delete(&mut self,id:&str)->Result<(),String>{let value=self.invoke("delete",json!({"id":id}));value.and_then(|v|serde_json::from_value(v).map_err(|e|e.to_string()))}
}

#[cfg(test)]mod tests{use super::*;
 fn store()->Store{Store::new(PathBuf::new())}
 fn stages()->Value{json!([{"name":"整理","seconds":600},{"name":"学习","seconds":1500},{"name":"休息","seconds":300}])}
 #[test]fn pause_resume_and_countdown_complete_exactly_once(){let mut s=store();let id=s.create(&json!({"mode":"countdown","duration":2})).unwrap();s.advance(900);s.action(&id,"toggle").unwrap();s.advance(5000);assert_eq!(s.items[0].clock(),"00:02");s.action(&id,"toggle").unwrap();assert_eq!(s.advance(1100).len(),1);assert!(s.advance(5000).is_empty());assert_eq!(s.items[0].clock(),"00:00");}
 #[test]fn pomodoro_sleep_gap_starts_a_full_rest_instead_of_skipping_it(){
  let mut s=store();s.create(&json!({"mode":"pomodoro","focus":2,"rest":1,"rounds":3})).unwrap();
  let events=s.advance(7000);assert_eq!(events.len(),1);assert_eq!(events[0].phase,"rest");
  assert_eq!(s.items[0].round,1);assert_eq!(s.items[0].elapsed_ms,0);assert_eq!(s.items[0].clock(),"00:01");assert!(s.items[0].running&&!s.items[0].finished);
  assert!(s.advance(999).is_empty());assert_eq!(s.items[0].phase,"rest");
  assert_eq!(s.advance(1).len(),1);assert_eq!(s.items[0].phase,"focus");assert_eq!(s.items[0].round,2);assert_eq!(s.items[0].elapsed_ms,0);
 }
 #[test]fn concurrent_clocks_keep_independent_state_and_validate_input(){let mut s=store();let id=s.create(&json!({"mode":"stopwatch"})).unwrap();s.create(&json!({"duration":5})).unwrap();s.advance(5000);assert_eq!(s.items[0].clock(),"00:05");assert!(s.items[1].finished);s.action(&id,"reset").unwrap();assert_eq!(s.items[0].clock(),"00:00");assert!(s.create(&json!({"duration":0})).is_err());assert!(s.create(&json!({"rounds":17})).is_err());}
 #[test]fn todo_link_persists_and_invalid_ids_are_rejected(){let p=std::env::temp_dir().join(format!("LyricIsland.TimerLink.{}.{}.json",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let mut s=Store::new(p.clone());let id=s.create(&json!({"mode":"pomodoro","focus":2,"name":"整理产品方案","todoId":"todo-123"})).unwrap();assert!(s.create(&json!({"todoId":"bad id!"})).is_err());let s=Store::new(p.clone());assert_eq!(s.items.iter().find(|t|t.id==id).and_then(|t|t.todo_id.as_deref()),Some("todo-123"));let _=std::fs::remove_file(p);}
 #[test]fn workflow_waits_after_each_stage_even_across_a_sleep_gap(){let mut s=store();let id=s.create(&json!({"mode":"workflow","name":"学习流程","stages":stages()})).unwrap();assert_eq!(s.items[0].clock(),"10:00");assert!(s.advance(599000).is_empty());assert_eq!(s.advance(1000).len(),1);assert!(s.items[0].workflow.as_ref().unwrap().waiting);assert!(!s.items[0].running);assert_eq!(s.items[0].clock(),"00:00");assert!(s.advance(24*60*60*1000).is_empty());assert_eq!(s.items[0].workflow.as_ref().unwrap().stage_index,0);let revision=s.items[0].revision;s.act(&id,"next",Some(revision)).unwrap();assert_eq!(s.items[0].workflow.as_ref().unwrap().stage_index,1);assert_eq!(s.items[0].clock(),"25:00");assert!(s.items[0].running);}
 #[test]fn stale_revision_prevents_a_double_next(){let mut s=store();let id=s.create(&json!({"mode":"workflow","stages":stages()})).unwrap();s.advance(600000);let revision=s.items[0].revision;s.act(&id,"next",Some(revision)).unwrap();assert!(s.act(&id,"next",Some(revision)).is_err());assert_eq!(s.items[0].workflow.as_ref().unwrap().stage_index,1);}
 #[test]fn skip_last_stage_ends_and_extend_is_capped(){let mut s=store();let id=s.create(&json!({"mode":"workflow","stages":stages()})).unwrap();for _ in 0..3{s.act(&id,"skip",None).unwrap();}assert!(s.items[0].finished);assert!(s.act(&id,"skip",None).is_err());let id=s.create(&json!({"mode":"workflow","stages":[{"name":"长","seconds":60},{"name":"末","seconds":60}]})).unwrap();s.advance(60000);for _ in 0..48{s.act(&id,"extend5",None).unwrap();}assert_eq!(s.items[1].workflow.as_ref().unwrap().stages[0].seconds,14400);assert!(s.act(&id,"extend5",None).is_err());}
 #[test]fn workflow_restarts_paused_at_the_same_stage_and_templates_are_copied(){let p=std::env::temp_dir().join(format!("LyricIsland.Workflow.{}.{}.json",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));let mut s=Store::new(p.clone());let id=s.create(&json!({"mode":"workflow","stages":stages(),"templateId":"flow-1"})).unwrap();s.act(&id,"skip",None).unwrap();let s=Store::new(p.clone());let t=s.items.iter().find(|t|t.id==id).unwrap();assert!(!t.running);assert_eq!(t.workflow.as_ref().unwrap().stage_index,1);assert_eq!(s.error,None);let instance=t.workflow.as_ref().unwrap().stages.clone();let mut templates=Templates::new(PathBuf::new());let template=templates.save(&json!({"name":"学习流程","stages":stages()})).unwrap();templates.save(&json!({"id":template,"name":"改名","stages":[{"name":"a","seconds":60},{"name":"b","seconds":60}]})).unwrap();assert_eq!(s.items[0].workflow.as_ref().unwrap().stages,instance);assert!(templates.delete(&template).is_ok());let _=std::fs::remove_file(p);}
 #[test]fn lock_pauses_only_running_focus_and_resume_needs_the_generation(){let mut s=store();let focus=s.create(&json!({"mode":"pomodoro","focus":60})).unwrap();let rest=s.create(&json!({"mode":"pomodoro","focus":60,"rest":30})).unwrap();let countdown=s.create(&json!({"mode":"countdown","duration":60})).unwrap();s.items.iter_mut().find(|t|t.id==rest).unwrap().phase="rest".into();assert_eq!(s.pause_for_lock(3).unwrap(),vec![focus.clone()]);assert!(s.items.iter().find(|t|t.id==rest).unwrap().running);assert!(s.items.iter().find(|t|t.id==countdown).unwrap().running);assert!(s.resume_focus(&[focus.clone()],2).is_err());assert_eq!(s.resume_focus(&[focus.clone()],3).unwrap(),vec![focus.clone()]);assert!(s.resume_focus(&[focus.clone()],3).is_err());s.pause_for_lock(4).unwrap();s.clear_lock_pauses().unwrap();assert!(s.resume_focus(&[focus],4).is_err());}
 #[test]fn old_three_mode_files_still_load(){let p=std::env::temp_dir().join(format!("LyricIsland.OldTimers.{}.{}.json",std::process::id(),std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));std::fs::write(&p,r#"[{"id":"clock-1","name":"倒计时","mode":"countdown","duration":300,"focus":1500,"rest":300,"rounds":4,"round":1,"phase":"focus","elapsedMs":0,"running":true,"finished":false}]"#).unwrap();let s=Store::new(p.clone());assert_eq!(s.error,None);assert_eq!(s.items.len(),1);assert!(!s.items[0].running);assert!(s.items[0].pause_reason.is_some());let _=std::fs::remove_file(p);}
}
#[cfg(test)]mod lap_tests{use super::*;
#[test]fn stopwatch_laps_are_authoritative_independent_and_resettable(){let mut s=Store::new(PathBuf::new());let id=s.create(&json!({"mode":"stopwatch"})).unwrap();s.advance(3180);s.act(&id,"lap",Some(1)).unwrap();s.advance(650);let rev=s.items[0].revision;s.act(&id,"lap",Some(rev)).unwrap();assert_eq!(s.items[0].laps,vec![3180,3830]);s.action(&id,"pause").unwrap();assert!(s.action(&id,"lap").is_err());s.action(&id,"reset").unwrap();assert!(s.items[0].laps.is_empty());assert_eq!(s.items[0].elapsed_ms,0);let other=s.create(&json!({"mode":"countdown"})).unwrap();assert!(s.action(&other,"lap").is_err());}
}
#[cfg(test)]mod direction_tests{use super::*;
#[test]fn reversing_display_keeps_phase_and_completion_deadline(){let mut s=Store::new(PathBuf::new());let id=s.create(&json!({"mode":"pomodoro","focus":10,"rounds":1})).unwrap();s.advance(3100);assert_eq!(s.items[0].clock(),"00:07");s.set_direction(&id,true,Some(1)).unwrap();assert_eq!(s.items[0].clock(),"00:03");assert_eq!(s.items[0].elapsed_ms,3100);assert!(s.items[0].running);assert!(s.set_direction(&id,false,Some(1)).is_err());assert_eq!(s.advance(6900).len(),1);assert!(s.items[0].finished);assert_eq!(s.items[0].clock(),"00:10");assert!(s.advance(5000).is_empty());}
#[test]fn count_up_workflow_still_waits_and_direction_survives_restart(){let p=std::env::temp_dir().join(format!("DotIsle.Direction.{}.json",std::process::id()));let mut s=Store::new(p.clone());let id=s.create(&json!({"mode":"workflow","countUp":true,"stages":[{"name":"专注","seconds":60},{"name":"休息","seconds":60}]})).unwrap();s.advance(60000);assert_eq!(s.items[0].clock(),"01:00");assert!(s.items[0].workflow.as_ref().unwrap().waiting);s.set_direction(&id,false,None).unwrap();assert_eq!(s.items[0].clock(),"00:00");assert!(s.items[0].workflow.as_ref().unwrap().waiting);let copy=Store::new(p.clone());assert_eq!(copy.error,None);assert!(!copy.items[0].count_up);assert!(copy.items[0].workflow.as_ref().unwrap().waiting);let _=std::fs::remove_file(p);assert!(s.create(&json!({"mode":"countdown","countUp":true})).is_err());assert!(s.create(&json!({"mode":"pomodoro","countUp":"true"})).is_err());}
}
#[cfg(test)]mod island_presentation_tests{use super::*;
#[test]fn island_actions_match_finished_and_waiting_states(){let mut s=Store::new(PathBuf::new());let id=s.create(&json!({"duration":1})).unwrap();s.advance(1000);assert_eq!(s.items[0].primary_action(),"restart");s.action(&id,"restart").unwrap();assert!(s.items[0].running&&!s.items[0].finished);assert_eq!(s.items[0].elapsed_ms,0);let flow=s.create(&json!({"mode":"workflow","stages":[{"name":"a","seconds":60},{"name":"b","seconds":60}]})).unwrap();s.advance(60000);assert_eq!(s.items[1].primary_action(),"next");s.action(&flow,"next").unwrap();assert_eq!(s.items[1].workflow.as_ref().unwrap().stage_index,1);}
#[test]fn stopwatch_preserves_fractional_elapsed_time(){let mut s=Store::new(PathBuf::new());s.create(&json!({"mode":"stopwatch"})).unwrap();s.advance(3830);assert_eq!(s.items[0].clock(),"00:03");assert_eq!(s.items[0].elapsed_ms,3830);assert_eq!(s.items[0].primary_action(),"pause");}
}
#[cfg(test)]mod phase_regression_tests{use super::*;
 #[test]fn delayed_rest_cannot_consume_the_next_focus(){
  let mut s=Store::new(PathBuf::new());s.create(&json!({"mode":"pomodoro","focus":2,"rest":1,"rounds":3})).unwrap();s.advance(2000);
  let events=s.advance(u64::MAX);assert_eq!(events.len(),1);assert_eq!(events[0].phase,"focus");assert_eq!(s.items[0].round,2);assert_eq!(s.items[0].elapsed_ms,0);assert_eq!(s.items[0].clock(),"00:02");assert!(!s.items[0].finished);
 }
 #[test]fn all_rounds_keep_their_full_phases_and_finish_once(){
  for count_up in [false,true]{let mut s=Store::new(PathBuf::new());s.create(&json!({"mode":"pomodoro","focus":2,"rest":1,"rounds":4,"countUp":count_up})).unwrap();
   for round in 1..=4{assert_eq!(s.items[0].round,round);assert_eq!(s.items[0].phase,"focus");assert_eq!(s.items[0].elapsed_ms,0);assert!(s.advance(1999).is_empty());assert_eq!(s.advance(1).len(),1);
    if round<4{assert_eq!(s.items[0].phase,"rest");assert_eq!(s.items[0].round,round);assert_eq!(s.items[0].elapsed_ms,0);assert!(s.advance(999).is_empty());assert_eq!(s.advance(1).len(),1);}
   }assert!(s.items[0].finished&&!s.items[0].running);assert_eq!(s.items[0].elapsed_ms,2000);assert!(s.advance(u64::MAX).is_empty());
  }
 }
 #[test]fn late_tick_does_not_shorten_rest_or_other_clocks(){
  let mut s=Store::new(PathBuf::new());s.create(&json!({"mode":"pomodoro","focus":2,"rest":1})).unwrap();s.create(&json!({"mode":"countdown","duration":10})).unwrap();s.create(&json!({"mode":"stopwatch"})).unwrap();
  assert_eq!(s.advance(2450).len(),1);assert_eq!(s.items[0].phase,"rest");assert_eq!(s.items[0].elapsed_ms,0);assert_eq!(s.items[1].elapsed_ms,2450);assert_eq!(s.items[2].elapsed_ms,2450);
  assert!(s.advance(999).is_empty());assert_eq!(s.items[0].phase,"rest");assert_eq!(s.advance(1).len(),1);assert_eq!(s.items[0].round,2);
 }
 #[test]fn rest_pause_and_display_direction_preserve_the_phase(){
  let mut s=Store::new(PathBuf::new());let id=s.create(&json!({"mode":"pomodoro","focus":2,"rest":1})).unwrap();s.advance(7000);s.advance(400);s.action(&id,"pause").unwrap();assert!(s.advance(u64::MAX).is_empty());s.set_direction(&id,true,None).unwrap();s.action(&id,"resume").unwrap();
  assert_eq!(s.items[0].phase,"rest");assert_eq!(s.items[0].elapsed_ms,400);assert!(s.advance(599).is_empty());assert_eq!(s.advance(1).len(),1);assert_eq!(s.items[0].round,2);assert_eq!(s.items[0].elapsed_ms,0);
 }
 #[test]fn duplicate_skip_cannot_skip_the_new_rest_stage(){
  let mut s=Store::new(PathBuf::new());let id=s.create(&json!({"mode":"workflow","stages":[{"name":"学习","seconds":60},{"name":"休息","seconds":60},{"name":"学习","seconds":60}]})).unwrap();let revision=s.items[0].revision;
  s.act(&id,"skip",Some(revision)).unwrap();assert!(s.act(&id,"skip",Some(revision)).is_err());assert_eq!(s.items[0].workflow.as_ref().unwrap().stage_index,1);assert_eq!(s.items[0].elapsed_ms,0);assert_eq!(s.items[0].clock(),"01:00");
 }
}
