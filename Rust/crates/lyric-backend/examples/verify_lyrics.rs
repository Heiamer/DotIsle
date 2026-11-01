//! Real provider checks use metadata only and never operate a music player.
use std::time::{Duration,Instant};
use lyric_core::{models::*,resolution::{LyricResolutionCoordinator,LyricResolutionCoordinatorBuilder},policy::LyricProviderTrustPolicy};
use lyric_backend::{providers::OnlineSource,http::RequestContext};
fn main()->Result<(),Box<dyn std::error::Error>>{
    let out=std::env::args().nth(1).ok_or("report path required")?;let mut reports=vec![];
    for provider in known_providers::online_trust_order(){
        let began=Instant::now();let source=OnlineSource::new(provider.value(),RequestContext::new(Duration::from_secs(15)));
        let policy=LyricProviderTrustPolicy::new(vec![provider.clone()],vec![provider.clone()])?;
        let coordinator=LyricResolutionCoordinator::new(LyricResolutionCoordinatorBuilder{sources:vec![Box::new(source)],trust_policy:Some(policy),source_timeout:Some(Duration::from_secs(15)),..Default::default()})?;
        let duration=if provider.value()=="Migu"{0}else{326};let track=TrackInfo::new("fixture","海阔天空","BEYOND","乐与怒","Kugou",Duration::from_secs(duration));let result=coordinator.resolve(&track);
        let lyrics=result.lyrics.as_ref();reports.push(serde_json::json!({"provider":provider.value(),"durationProvided":duration>0,"status":format!("{:?}",result.status),"lyrics":lyrics.is_some(),"score":lyrics.and_then(ResolvedLyrics::identity_score),"elapsedMs":began.elapsed().as_millis()}));
        std::fs::write(&out,serde_json::to_vec_pretty(&reports)?)?;
    }
    Ok(())
}
