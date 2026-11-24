//! Read-only service interoperability checks; never issues player controls.
use std::time::{Duration,Instant};
use lyric_core::models::*;use dotisle_lyrics::resolution::{self,Source};
use dotisle_lyrics::{providers::OnlineSource,worker::RequestContext};
fn main()->Result<(),Box<dyn std::error::Error>>{
 let args=std::env::args().skip(1).collect::<Vec<_>>();
 let out=args.first().ok_or("report path required")?;
 let track=Track::new("service-check",args.get(2).map_or("海阔天空",String::as_str),args.get(3).map_or("BEYOND",String::as_str),args.get(4).map_or("",String::as_str),"Generic",Duration::from_secs(args.get(5).and_then(|s|s.parse().ok()).unwrap_or(0)));
 let providers=providers().into_iter().filter(|p|args.get(1).is_none_or(|a|a=="all"||p.eq_ignore_ascii_case(a))).collect::<Vec<_>>();
 if providers.is_empty(){return Err("unknown provider".into())}
 let reports=std::thread::scope(|scope|providers.into_iter().map(|provider|{
  let t=&track;
  scope.spawn(move||{
   let start=Instant::now();let source=OnlineSource::new(provider.value(),RequestContext::new(Duration::from_secs(10)));
   let search=match source.search(t){Ok(hits)=>serde_json::json!({"hits":hits.len(),"admitted":hits.iter().filter(|h|lyric_core::identity::rate(t,h).is_some()).count()}),Err(e)=>serde_json::json!({"error":e})};
   let result=resolution::resolve(t,&source);
   serde_json::json!({"provider":provider.value(),"search":search,"status":format!("{:?}",result.status),"score":result.lyrics.as_ref().map(|s|s.score),"timed":result.lyrics.as_ref().is_some_and(|s|s.content.timed),"wordTimedLines":result.lyrics.as_ref().map(|s|s.content.lines.iter().filter(|l|!l.words.is_empty()).count()),"elapsedMs":start.elapsed().as_millis()})
  })
 }).collect::<Vec<_>>().into_iter().map(|job|job.join().unwrap()).collect::<Vec<_>>());
 std::fs::write(out,serde_json::to_vec_pretty(&reports)?)?;Ok(())
}
