//! Local active-session accounting. No application names or input contents.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::BTreeMap, path::PathBuf, time::{Duration, Instant}};
use dotisle_domain::{desktop_widgets::Date, widget_config::{ordinal, date_ordinal}};
use windows::Win32::{System::SystemInformation::{GetLocalTime, GetTickCount64},
    UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO}};

#[derive(Default, Serialize, Deserialize)]
#[serde(default, rename_all="camelCase")]
struct Day { active_ms: u64, observed_ms: u64 }
pub struct Tracker {
    path: PathBuf, days: BTreeMap<String,Day>, last: Instant, checkpoint: Instant,
    awake: u64, date: Date, locked: bool, dirty: bool,
}
#[derive(Default)]
pub struct Sample { pub active_ms: u64, pub continuous: bool }
fn local() -> Date {
    let t=unsafe { GetLocalTime() };
    Date{year:t.wYear as i32,month:t.wMonth as u32,day:t.wDay as u32,
        hour:t.wHour as u32,minute:t.wMinute as u32,second:t.wSecond as u32}
}
fn key(d:Date)->String { format!("{:04}-{:02}-{:02}",d.year,d.month,d.day) }
fn idle_ms()->Option<u64>{
    let mut i=LASTINPUTINFO{cbSize:std::mem::size_of::<LASTINPUTINFO>() as u32,..Default::default()};
    unsafe { GetLastInputInfo(&mut i).as_bool().then(|| (GetTickCount64() as u32).wrapping_sub(i.dwTime) as u64) }
}
impl Tracker {
    pub fn new(path:PathBuf,awake:u64)->Self {
        let days=std::fs::read(&path).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or_default();
        Self{path,days,last:Instant::now(),checkpoint:Instant::now(),awake,date:local(),locked:false,dirty:false}
    }
    pub fn poll(&mut self,at:Instant,awake:u64,locked:bool)->Sample{
        let elapsed=at.saturating_duration_since(self.last).as_millis().min(u64::MAX as u128) as u64;
        self.last=at;
        let now=local();
        // Unbiased time excludes sleep. Long scheduler gaps are deliberately uncounted.
        let observed=if elapsed<=90_000 {awake.saturating_sub(self.awake).min(elapsed)}else{0};
        let present=!locked&&!self.locked&&idle_ms().is_some_and(|v|v<crate::reminder_plugins::AWAY_MS);
        let active=if present{observed}else{0};
        self.record(now,observed,active);
        self.awake=awake;self.locked=locked;
        if self.checkpoint.elapsed()>=Duration::from_secs(30){self.flush();self.checkpoint=Instant::now();}
        Sample{active_ms:active,continuous:present&&elapsed.saturating_sub(observed)<5_000}
    }
    fn record(&mut self,now:Date,observed:u64,active:u64){
        let day=ordinal(now);let previous=ordinal(self.date);
        let since_midnight=(now.hour as u64*3600+now.minute as u64*60+now.second as u64)*1000;
        let spill=if day==previous+1{observed.saturating_sub(since_midnight)}else{0};
        if spill>0{let d=self.days.entry(key(self.date)).or_default();d.observed_ms=d.observed_ms.saturating_add(spill);d.active_ms=d.active_ms.saturating_add(active.min(spill));}
        if observed>spill{let d=self.days.entry(key(now)).or_default();d.observed_ms=d.observed_ms.saturating_add(observed-spill);d.active_ms=d.active_ms.saturating_add(active.saturating_sub(spill));}
        self.days.retain(|k,_|date_ordinal(k).is_some_and(|v|v<=day&&day-v<35));
        self.date=now;self.dirty|=observed>0;
    }
    pub fn flush(&mut self){
        if self.dirty&&!self.path.as_os_str().is_empty(){
            if lyric_backend::io::save_settings(&self.path,&json!(self.days)).is_ok(){self.dirty=false;}
        }
    }
    pub fn snapshot(&self)->Value {
        let today=ordinal(local());
        let get=|date:i64|self.days.iter().find(|(k,_)|date_ordinal(k)==Some(date)).map(|(_,d)|d);
        let week=(0..7).map(|i|get(today-6+i).map(|d|d.active_ms)).collect::<Vec<_>>();
        json!({"todayMs":get(today).map_or(0,|d|d.active_ms),"yesterdayMs":get(today-1).map(|d|d.active_ms),
            "todayObservedMs":get(today).map_or(0,|d|d.observed_ms),"week":week,
            "recording":true,"definition":"点屿运行期间，解锁且最近 5 分钟有操作的时间；不含锁屏与休眠"})
    }
}
impl Drop for Tracker{fn drop(&mut self){self.flush();}}
pub fn duration(ms:u64)->String{
    let m=ms/60_000; if m>=60{format!("{} 小时 {} 分钟",m/60,m%60)}else{format!("{m} 分钟")}
}
#[cfg(test)] mod tests{
    use super::*;
    #[test] fn splits_midnight_and_never_invents_yesterday(){
        let mut t=Tracker::new(PathBuf::new(),0);t.days.clear();
        t.date=Date{year:2026,month:11,day:14,hour:23,minute:59,second:59};
        t.record(Date{day:15,hour:0,minute:0,second:1,..t.date},2000,2000);
        assert_eq!(t.days["2026-11-14"].active_ms,1000);
        assert_eq!(t.days["2026-11-15"].active_ms,1000);
    }
    #[test] fn inactive_sample_does_not_increase_active_time(){
        let mut t=Tracker::new(PathBuf::new(),0);t.days.clear();let d=t.date;
        t.record(d,1000,0);assert_eq!(t.days[&key(d)].active_ms,0);
        assert_eq!(t.days[&key(d)].observed_ms,1000);
        assert_eq!(duration(7260000),"2 小时 1 分钟");
    }
}
