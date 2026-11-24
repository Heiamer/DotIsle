//! Align a verified Soda Music preview against the original track timeline.
use lyric_core::{identity, models::*};
use std::time::Duration;

fn playback_ms(track:&Track)->u64{track.duration.as_millis().min(u64::MAX as u128) as u64}
pub fn set_preview(track:&Track,hit:&mut Hit){
    let value=|key|hit.extra.get(key).and_then(|v|v.parse::<u64>().ok()).unwrap_or(0).min(86_400_000);
    let(start,length)=(value("previewStartMs"),value("previewDurationMs"));
    let full=hit.duration.as_millis() as u64;
    let playing=playback_ms(track);
    if identity::official(&track.source_app).is_some_and(|p|p.value()=="SodaMusic")
        && start>0 && start<full && length>0 && start+length<=full+1000
        && playing>0 && playing.abs_diff(full)>1000 && playing.abs_diff(length)<=1000 {
        hit.extra.insert("previewOffsetMs".into(),start.to_string());
        hit.duration=Duration::from_millis(length);
    }
}
pub fn align(track:&Track,hit:&Hit,lyrics:&mut Lyrics){
    let Some(offset)=hit.extra.get("previewOffsetMs").and_then(|v|v.parse::<u64>().ok()) else{return};
    let length=playback_ms(track);let end=offset.saturating_add(length);
    lyrics.lines.retain_mut(|line|{
        if line.until<=offset||line.at>=end{return false}
        line.at=line.at.saturating_sub(offset);line.until=(line.until-offset).min(length);
        if !line.words.is_empty(){
            line.words.retain_mut(|w|{
                if w.until<=offset||w.at>=end{return false}
                w.at=w.at.saturating_sub(offset);w.until=(w.until-offset).min(length);true
            });
            line.text=line.words.iter().map(|w|w.text.as_str()).collect();
        }
        !line.text.trim().is_empty()
    });
}

#[cfg(test)]
#[test]
fn align_only_verified_soda_preview(){
    let mut t=Track::new("x","song","artist","","Browser",Duration::from_secs(30));
    let mut h=Hit{provider:Provider::new("SodaMusic"),id:"123".into(),title:t.title.clone(),artists:vec![t.artist.clone()],album:String::new(),duration:Duration::from_secs(190),extra:[("previewStartMs".into(),"60000".into()),("previewDurationMs".into(),"30000".into())].into()};
    set_preview(&t,&mut h);assert_eq!(h.duration.as_secs(),190);
    t.source_app="SodaMusic".into();set_preview(&t,&mut h);assert_eq!(h.duration.as_secs(),30);
    let mut l=crate::formats::lrc("[00:59.00]overlap\n[01:01.00]verse\n[01:31.00]outside");
    align(&t,&h,&mut l);assert_eq!(l.lines.iter().map(|l|l.at).collect::<Vec<_>>(),[0,1000]);
}
