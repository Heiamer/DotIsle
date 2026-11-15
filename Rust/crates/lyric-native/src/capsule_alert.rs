//! Dedicated island-body activities; these never enter the notification banner.
use crate::{banner::Banner,layout::Rect,notifications::Notice};
use std::{collections::{HashMap,VecDeque},time::{Duration,Instant}};
use dotisle_domain::desktop_widgets::WirelessDevice;
use serde_json::Value;

#[derive(Clone,Debug)]
pub struct Alert{
    pub key:String,pub icon:String,pub title:String,pub detail:String,pub trailing:String,
    pub image:Option<vello::peniko::ImageData>,pub code:Option<String>,pub expires:Instant,pub hold:f64,pub width:f64,pub height:f64,
}
impl Alert{
    pub fn new(key:String,icon:&str,title:String,detail:String)->Self{
        Self{key,icon:icon.into(),title,detail,trailing:String::new(),image:None,code:None,
            expires:Instant::now()+Duration::from_secs(90),hold:5.,width:400.,height:48.}
    }
    pub fn message(id:&str,name:&str,icon:Option<&[u8]>)->Self{
        let epoch=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let mut a=Self::new(format!("message:{id}:{epoch}"),&format!("Message:{id}"),format!("{name}有新消息"),String::new());
        a.image=icon.and_then(|raw|crate::renderer::Renderer::decode(raw).ok()).map(|v|v.0);
        a.width=320.;a.hold=5.;a.expires=Instant::now()+Duration::from_secs(15);a
    }
    pub fn device(id:&str,icon:&str,name:&str,connected:bool,level:Option<u8>,low:bool)->Self{
        let mut a=Self::new(format!("device:{id}:{connected}:{low}"),icon,
            format!("{}{}",name,if low{"电量偏低"}else if connected{"已连接"}else{"已断开"}),String::new());
        a.trailing=level.map(|v|format!("{v}%")).unwrap_or_else(||"—".into());
        a
    }
}
#[derive(Default)]
pub struct Hub{pub banner:Banner,pub active:Option<Alert>,queue:VecDeque<Alert>,seen:HashMap<String,Instant>}
impl Hub{
    pub fn visible(&self)->bool{self.banner.visible()}
    pub fn pending(&self)->bool{!self.queue.is_empty()}
    pub fn enqueue(&mut self,a:Alert){
        self.seen.retain(|_,at|at.elapsed()<Duration::from_secs(120));
        if a.expires<=Instant::now()||self.seen.contains_key(&a.key){return}
        self.seen.insert(a.key.clone(),Instant::now());
        if self.queue.len()>=8{self.queue.pop_front();}
        if a.code.is_some(){self.queue.push_front(a)}else{self.queue.push_back(a)}
    }
    pub fn begin(&mut self,base:Rect,radius:f64,host_width:f64,y:f64,now:f64,animate:bool)->bool{
        if self.visible(){return false}
        while let Some(a)=self.queue.pop_front(){
            if a.expires<=Instant::now(){continue}
            self.banner.show_capsule(Notice::default(),base,radius,host_width,y,a.width,a.height,now,animate);
            self.banner.hold_until(now+a.hold);self.active=Some(a);return true
        }
        false
    }
    pub fn tick(&mut self,base:Rect,radius:f64,now:f64,animate:bool){
        // Always return to the current base, including Mini. Visibility is retired by the host.
        self.banner.tick(base,radius,now,false,animate,true);
        if !self.visible(){self.active=None;}
    }
    pub fn dismiss(&mut self,base:Rect,radius:f64,now:f64,animate:bool){self.banner.dismiss(base,radius,now,animate,true);}
    pub fn clear(&mut self,base:Rect,radius:f64,now:f64,animate:bool){self.queue.clear();self.dismiss(base,radius,now,animate);}
}

#[derive(Default)]
pub struct DeviceWatch{levels:HashMap<String,(WirelessDevice,bool)>,baseline:bool,last_stamp:u64}
impl DeviceWatch{
    pub fn update(&mut self,devices:&[WirelessDevice],stamp:u64,ready:bool)->Vec<Alert>{
        if !ready||stamp==0||stamp==self.last_stamp{return vec![]}
        self.last_stamp=stamp;
        let mut out=Vec::new();let mut next=HashMap::new();
        for d in devices.iter().filter(|d|d.connected){
            let old=self.levels.get(&d.id);
            let mut warned=old.is_some_and(|(_,v)|*v);
            if d.battery_percent.is_some_and(|v|v>=25){warned=false;}
            if self.baseline&&old.is_none(){out.push(Alert::device(&d.id,device_icon(&d.kind),&d.name,true,d.battery_percent,false));}
            if d.battery_percent.is_some_and(|v|v<20)&&!warned{
                warned=true;out.push(Alert::device(&d.id,device_icon(&d.kind),&d.name,true,d.battery_percent,true));
            }
            next.insert(d.id.clone(),(d.clone(),warned));
        }
        // A successful empty snapshot is a disconnect; telemetry errors never are.
        self.levels=next;self.baseline=true;out
    }
    pub fn lookup(&self,key:&str)->Option<&WirelessDevice>{
        let norm=|s:&str|s.trim_matches(['{','}']).to_ascii_lowercase();let k=norm(key);
        self.levels.values().map(|(d,_)|d).find(|d|norm(&d.id)==k||d.container_id.as_ref().is_some_and(|c|norm(c)==k))
    }
}
pub fn device_icon(kind:&str)->&'static str{match kind{"headphones"=>"Headphones","speaker"=>"Speaker","mouse"=>"Mouse","keyboard"=>"Keyboard","gamepad"=>"Gamepad",_=>"Usb"}}

#[derive(Default)]
pub struct WeatherWatch{announced:HashMap<String,Instant>}
impl WeatherWatch{
    pub fn poll(&mut self,weather:&Value,now:i64)->Vec<Alert>{
        self.announced.retain(|_,at|at.elapsed()<Duration::from_secs(24*3600));
        if weather["stale"]==true||weather["error"].is_string()
            ||!weather["time"].as_i64().is_some_and(|t|t<=now+900&&now-t<5400){return vec![]}
        let city=weather["city"].as_str().unwrap_or("");let mut alerts=Vec::new();
        for w in weather["precipitation"].as_array().into_iter().flatten(){
            let Some(start)=w["startsAt"].as_i64()else{continue};let end=w["endsAt"].as_i64();
            if start>now+7200||end.is_some_and(|t|t<=now)||start<now-86400{continue}
            let kind=w["kind"].as_str().unwrap_or("rain");let ongoing=start<=now;
            // Time revisions within the same hour should not re-announce the same forecast.
            let key=format!("weather:{city}:{kind}:{}:{ongoing}",start/3600);
            if self.announced.contains_key(&key){continue}
            self.announced.insert(key.clone(),Instant::now());
            let name=match kind{"snow"=>"降雪","mixed"=>"雨夹雪",_=>"降雨"};
            let title=if ongoing{format!("{city} · {name}时段")}else{format!("{city} · 即将{name}")};
            let begin=if ongoing{"正在降水".to_owned()}else{format!("预计 {} 开始",w["startLabel"].as_str().unwrap_or("稍后"))};
            let finish=w["endLabel"].as_str().map(|v|format!("预计 {v} 结束")).unwrap_or("结束时间暂不确定".into());
            let mut a=Alert::new(key,if kind=="snow"{"Snow"}else{"Rain"},title,format!("{begin} · {finish}"));
            a.width=452.;a.height=72.;a.hold=8.;alerts.push(a);break
        }
        alerts
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]fn message_capsule_restores_classic_and_mini_without_content(){
        for base in [Rect::new(170.,24.,260.,40.),Rect::new(250.,24.,100.,18.)] {
            let mut h=Hub::default();let a=Alert::message("WXWork","企业微信",None);
            assert_eq!(a.title,"企业微信有新消息");assert!(a.detail.is_empty()&&a.trailing.is_empty()&&a.code.is_none());
            h.enqueue(a);assert!(h.begin(base,base.h/2.,600.,24.,0.,true));
            assert_eq!(h.banner.presentation(0.).0,base);
            h.tick(base,base.h/2.,0.5,true);let expanded=h.banner.presentation(0.5).0;
            assert!(expanded.w>319.&&expanded.h>47.);
            let before=h.banner.presentation(0.6);h.dismiss(base,base.h/2.,0.6,true);
            assert_eq!(h.banner.presentation(0.6),before);
            h.tick(base,base.h/2.,2.,true);assert!(!h.visible());
            assert!((h.banner.presentation(2.).0.w-base.w).abs()<0.01);
        }
    }
    fn d(level:Option<u8>)->WirelessDevice{WirelessDevice{id:"a".into(),name:"耳机".into(),kind:"headphones".into(),connected:true,battery_percent:level,container_id:None}}
    #[test]fn silent_connections_baseline_and_low_battery_hysteresis(){
        let mut w=DeviceWatch::default();assert!(w.update(&[d(Some(70))],1,true).is_empty());
        assert_eq!(w.update(&[d(Some(19))],2,true).len(),1);
        assert!(w.update(&[d(Some(18))],3,true).is_empty());
        assert!(w.update(&[d(Some(20))],4,true).is_empty());
        assert!(w.update(&[d(None)],5,false).is_empty());
        assert!(w.update(&[d(Some(26))],6,true).is_empty());
        assert_eq!(w.update(&[d(Some(19))],7,true).len(),1);
    }
    #[test]fn unknown_and_exact_twenty_do_not_trigger(){
        let mut w=DeviceWatch::default();assert!(w.update(&[d(None)],1,true).is_empty());
        assert!(w.update(&[d(Some(20))],2,true).is_empty());
    }
    #[test]fn queue_is_bounded_and_one_message_uses_one_surface(){
        let mut h=Hub::default();for i in 0..20{h.enqueue(Alert::new(i.to_string(),"Sun","测试".into(),String::new()));}
        assert_eq!(h.queue.len(),8);let r=Rect::new(200.,24.,100.,18.);
        assert!(h.begin(r,9.,600.,24.,0.,true));assert!(!h.begin(r,9.,600.,24.,0.,true));
        assert_eq!(h.banner.presentation(0.).0,r);
        h.dismiss(r,9.,0.12,true);h.tick(r,9.,2.,true);assert!(!h.visible());
        assert!((h.banner.presentation(2.).0.h-18.).abs()<0.01);
    }
    #[test]fn precipitation_reminders_are_deduplicated_and_stale_is_silent(){
        let mut h=WeatherWatch::default();let mut v=serde_json::json!({"city":"上海","time":1000,"precipitation":[{"startsAt":3600,"endsAt":7200,"kind":"snow","startLabel":"今天09:00","endLabel":"今天10:00"}]});
        let a=h.poll(&v,1000);assert_eq!(a.len(),1);assert!(a[0].detail.contains("10:00"));
        assert!(h.poll(&v,1100).is_empty());v["stale"]=true.into();assert!(h.poll(&v,3700).is_empty());
    }
}
