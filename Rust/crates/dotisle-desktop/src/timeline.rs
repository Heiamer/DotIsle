//! Bounded presentation of the real media position, with explicit seek acknowledgement.
use crate::model::Model;
#[derive(Default)]pub struct Timeline{pub position:f64,track:(String,String,String),sample:f64,age:f64,ready:bool,playing:bool,pending:Option<(f64,f64)>}
impl Timeline{
 pub fn seek(&mut self,position:f64){if position.is_finite(){self.position=position.max(0.);self.pending=Some((self.position,0.));self.age=0.;}}
 pub fn tick(&mut self,m:&Model,dt:f64)->bool{
  let before=self.position;let dt=if dt.is_finite(){dt.clamp(0.,0.1)}else{0.};
  let t=&m.snapshot["track"];let Some(sample)=m.snapshot["positionSeconds"].as_f64().filter(|v|v.is_finite()&&*v>=0.)else{self.age=0.;return false};
  let duration=t["durationSeconds"].as_f64().filter(|v|v.is_finite()&&*v>0.);
  let track=(t["id"].as_str().unwrap_or("").to_owned(),t["title"].as_str().unwrap_or("").to_owned(),t["artist"].as_str().unwrap_or("").to_owned());let playing=m.playing();
  if !self.ready||self.track!=track{self.position=sample;self.sample=sample;self.age=0.;self.track=track;self.ready=true;self.pending=None;}
  if let Some((position,age))=self.pending{let command=&m.snapshot["mediaCommand"];let matches=command["action"]=="seek"&&command["targetSeconds"].as_f64().is_some_and(|v|(v-position).abs()<0.1);let failed=matches&&command["state"]=="failed";let waiting=matches&&(command["state"]=="queued"||command["state"]=="pending");if failed||(sample-position).abs()<0.8||age>=if waiting{5.}else{1.2}{self.pending=None;self.sample=sample;self.age=0.;}else{self.pending=Some((position,age+dt));self.playing=playing;return (before-self.position).abs()>0.001}}
  if (sample-self.sample).abs()>0.0001||playing!=self.playing{self.sample=sample;self.age=0.;}else if playing{self.age=(self.age+dt).min(0.35);}else{self.age=0.;}
  let target=(sample+if playing{self.age}else{0.}).clamp(0.,duration.unwrap_or(86400.));let error=target-self.position;
  if m.prefs.reduced_motion||!playing||error.abs()>1.5{self.position=sample;}else{self.position+=error*(1.-(-dt*20.).exp());}
  self.playing=playing;
  playing&&!m.prefs.reduced_motion||(self.position-before).abs()>0.001
 }
}
