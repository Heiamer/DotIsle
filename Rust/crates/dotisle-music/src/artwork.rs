//! Track-scoped artwork cache. Identical bytes at a track boundary may be a late old thumbnail.
use std::time::{Duration,Instant};
pub(crate) struct ArtworkCache{pub key:String,pub bytes:Option<Vec<u8>>,previous:Option<Vec<u8>>,changed:Instant}
impl Default for ArtworkCache{fn default()->Self{Self{key:String::new(),bytes:None,previous:None,changed:Instant::now()}}}
impl ArtworkCache{
    pub fn select(&mut self,key:String,now:Instant)->bool{
        if key==self.key{return false}
        self.previous=self.bytes.take().or_else(||self.previous.take());self.key=key;self.changed=now;true
    }
    pub fn accept(&mut self,key:&str,bytes:Option<Vec<u8>>,now:Instant)->bool{
        if key!=self.key{return false}
        let Some(bytes)=bytes.filter(|b|!b.is_empty())else{return false};
        if self.bytes.is_none()&&self.previous.as_ref()==Some(&bytes)&&now.duration_since(self.changed)<Duration::from_millis(1500){return false}
        if self.bytes.as_ref()==Some(&bytes){return false}
        self.bytes=Some(bytes);self.previous=None;true
    }
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn delayed_old_cover_and_old_jobs_cannot_replace_the_current_track(){let now=Instant::now();let mut c=ArtworkCache::default();c.select("a".into(),now);assert!(c.accept("a",Some(vec![1]),now));c.select("b".into(),now);assert!(c.bytes.is_none());assert!(!c.accept("b",Some(vec![1]),now));assert!(!c.accept("a",Some(vec![2]),now));assert!(c.accept("b",Some(vec![3]),now));assert!(!c.accept("b",None,now));assert_eq!(c.bytes,Some(vec![3]));assert!(c.accept("b",Some(vec![4]),now));}
    #[test]fn adjacent_tracks_on_the_same_album_can_share_a_thumbnail(){let now=Instant::now();let mut c=ArtworkCache::default();c.select("a".into(),now);c.accept("a",Some(vec![1]),now);c.select("b".into(),now);assert!(c.accept("b",Some(vec![1]),now+Duration::from_secs(2)));}
}
