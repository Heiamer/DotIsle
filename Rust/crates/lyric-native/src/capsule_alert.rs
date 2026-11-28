//! Dedicated island-body activities; these never enter the notification banner.
use crate::{banner::Banner,layout::Rect,notifications::Notice};
use std::{collections::{HashMap,VecDeque},time::{Duration,Instant}};
use dotisle_domain::desktop_widgets::WirelessDevice;
use serde_json::Value;

#[derive(Clone, Copy, Debug)]
pub struct QuotaAlert {
    pub remaining: [Option<f64>; 2],
    pub low: [bool; 2],
}

#[derive(Clone,Debug)]
pub struct Alert{
    pub key:String,pub icon:String,pub title:String,pub detail:String,pub trailing:String,
    pub image:Option<vello::peniko::ImageData>,pub code:Option<String>,pub quota:Option<QuotaAlert>,pub balance:Option<crate::ai_quota::RelayBalance>,pub hydration:Option<crate::reminder_plugins::Hydration>,pub expires:Instant,pub hold:f64,pub width:f64,pub height:f64,
}
impl Alert{
    pub fn new(key:String,icon:&str,title:String,detail:String)->Self{
        Self{key,icon:icon.into(),title,detail,trailing:String::new(),image:None,code:None,quota:None,balance:None,hydration:None,
            expires:Instant::now()+Duration::from_secs(90),hold:5.,width:400.,height:48.}
    }
    pub fn island_only(&self) -> bool {
        self.key.starts_with("reminder:") || self.key.starts_with("quota:")
    }
    pub fn quota(id: &str, name: &str, remaining: [Option<f64>; 2], thresholds: [u8; 2]) -> Self {
        let mut alert = Self::new(format!("quota:{id}"), "QuotaReminder", format!("{name} 额度偏低"), String::new());
        alert.quota = Some(QuotaAlert {
            remaining,
            low: std::array::from_fn(|i| remaining[i].is_some_and(|n| n < f64::from(thresholds[i]))),
        });
        alert.width = 356.;
        alert.height = 122.;
        alert.hold = 9.;
        alert.expires = Instant::now() + Duration::from_secs(15);
        alert
    }
    pub fn relay_balance(id: &str, name: &str, balance: crate::ai_quota::RelayBalance) -> Self {
        let mut alert = Self::new(format!("quota:balance:{id}:{}", balance.currency.code()), "QuotaReminder", format!("{name} · 余额偏低"), String::new());
        alert.balance = Some(balance);
        alert.width = 356.;
        alert.height = 112.;
        alert.hold = 9.;
        alert.expires = Instant::now() + Duration::from_secs(15);
        alert
    }
    pub fn wellness(kind: crate::reminder_plugins::Kind) -> Self {
        use crate::reminder_plugins::Kind;
        let (icon, title, detail) = match kind {
            Kind::Stand => ("StandReminder", "起来活动一下", "离开座位走几步，放松肩颈。"),
            Kind::Water => ("WaterReminder", "喝口水，休息一下", "补一杯水，再继续手头的事。"),
        };
        let mut alert = Self::new(kind.alert_key().into(), icon, title.into(), detail.into());
        alert.width = 356.; alert.height = 104.; alert.hold = 9.;
        alert.expires = Instant::now() + Duration::from_secs(15);
        alert
    }
    pub fn with_hydration(mut self, progress: crate::reminder_plugins::Hydration) -> Self {
        self.height = 148.;
        self.hold = 15.;
        self.expires = Instant::now() + Duration::from_secs(20);
        self.hydration = Some(progress);
        self
    }
    pub fn verification(id:&str,name:&str,code:String)->Self{
        let mut a=Self::new(format!("otp:{id}:{code}"),"Code",name.into(),"点击复制验证码".into());
        a.code=Some(code);a.hold=12.;a.width=crate::verification_code::WIDTH;a.height=crate::verification_code::HEIGHT;a
    }
    pub fn message(id:&str,name:&str,icon:Option<&[u8]>)->Self{
        let epoch=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let mut a=Self::new(format!("message:{id}:{epoch}"),&format!("Message:{id}"),format!("{name}有新消息"),String::new());
        a.image=icon.and_then(|raw|crate::renderer::Renderer::decode_notification_icon(raw).ok());
        a.width=320.;a.hold=5.;a.expires=Instant::now()+Duration::from_secs(15);a
    }
    pub fn device(id:&str,icon:&str,name:&str,connected:bool,level:Option<u8>,low:bool)->Self{
        let mut a=Self::new(format!("device:{id}:{connected}:{low}"),icon,
            format!("{}{}",name,if low{"电量偏低"}else if connected{"已连接"}else{"已断开"}),String::new());
        a.trailing=if connected{level.map(|v|format!("{v}%")).unwrap_or_default()}else{String::new()};
        a
    }
}
/// A single layout shared by painting and hit testing during the island morph.
pub struct WaterLayout {
    pub track: Rect,
    pub later: Rect,
    pub record: Rect,
}
impl WaterLayout {
    pub fn in_card(r: Rect) -> Option<Self> {
        // At first the compact capsule is narrower and shorter than the controls.
        // Wait for both axes so neither buttons nor hit regions overlap mid-morph.
        if r.w < 306. || r.h < 134. { return None; }
        Some(Self {
            track: Rect::new(r.x + 56., r.y + 72., r.w - 76., 5.),
            later: Rect::new(r.x + 56., r.y + r.h - 45., 78., 32.),
            record: Rect::new(r.x + r.w - 164., r.y + r.h - 45., 146., 32.),
        })
    }
}

#[derive(Default)]
pub struct Hub{pub banner:Banner,pub active:Option<Alert>,queue:VecDeque<Alert>,seen:HashMap<String,Instant>}
impl Hub{
    pub fn visible(&self)->bool{self.banner.visible()}
    pub fn pending(&self)->bool{!self.queue.is_empty()}
    pub fn has_wellness(&self) -> bool {
        self.active.iter().chain(self.queue.iter()).any(|a| a.key.starts_with("reminder:"))
    }
    pub fn cancel_wellness(&mut self) -> bool {
        self.cancel_prefix("reminder:")
    }
    pub fn cancel_prefix(&mut self, prefix: &str) -> bool {
        self.remove_pending_prefix(prefix);
        if self.active.as_ref().is_some_and(|a| a.key.starts_with(prefix)) {
            self.banner = Default::default();
            self.active = None;
            return true;
        }
        false
    }
    pub fn next_wellness(&self) -> bool {
        self.queue.iter().find(|a| a.expires > Instant::now()).is_some_and(|a| a.key.starts_with("reminder:"))
    }
    pub fn enqueue(&mut self,a:Alert){
        let window=if a.key.starts_with("otp:"){30}else{120};
        self.seen.retain(|key,at|at.elapsed()<Duration::from_secs(if key.starts_with("device:")||key.starts_with("reminder:"){3}else if key.starts_with("otp:"){30}else{120}));
        if a.expires<=Instant::now()||self.seen.get(&a.key).is_some_and(|at|at.elapsed()<Duration::from_secs(window)){return}
        self.seen.insert(a.key.clone(),Instant::now());
        if self.queue.len()>=8{self.queue.pop_front();}
        if a.code.is_some(){self.queue.push_front(a)}else{self.queue.push_back(a)}
    }
    /// 验证码高优直达：若当前胶囊已展示旧提醒或旧验证码，就地无缝接管并重设计时；否则入队头。
    pub fn preempt_or_enqueue(&mut self,a:Alert,base:Rect,radius:f64,host_width:f64,y:f64,now:f64,animate:bool)->bool{
        self.seen.retain(|key,at|at.elapsed()<Duration::from_secs(if key.starts_with("device:")||key.starts_with("reminder:"){3}else if key.starts_with("otp:"){30}else{120}));
        if a.expires<=Instant::now()||self.seen.get(&a.key).is_some_and(|at|at.elapsed()<Duration::from_secs(30)){return false}
        self.seen.insert(a.key.clone(),Instant::now());
        if self.visible()&&self.active.is_some(){
            self.banner.show_capsule(Notice::default(),base,radius,host_width,y,a.width,a.height,now,animate);
            self.banner.hold_until(now+a.hold);
            self.active=Some(a);
            return true;
        }
        if self.queue.len()>=8{self.queue.pop_front();}
        self.queue.push_front(a);
        false
    }
    // Telemetry may arrive after the connection event. Update the existing
    // surface in place and canonicalize its identity before deduplicating it.
    pub fn refresh_devices(&mut self,watch:&DeviceWatch)->bool{
        let mut changed=false;
        for alert in self.active.iter_mut().chain(self.queue.iter_mut()){
            let Some(rest)=alert.key.strip_prefix("device:")else{continue};
            let mut parts=rest.rsplitn(3,':');
            let low=parts.next()==Some("true");let connected=parts.next()==Some("true");
            let Some(device)=parts.next().and_then(|id|watch.lookup(id))else{continue};
            if !connected{continue}
            let updated=Alert::device(&device.id,device_icon(&device.kind),&device.name,true,device.battery_percent,low);
            if alert.trailing!=updated.trailing||alert.title!=updated.title||alert.icon!=updated.icon{changed=true;}
            alert.trailing=updated.trailing;alert.title=updated.title;alert.icon=updated.icon;
            if alert.key!=updated.key{self.seen.insert(updated.key.clone(),Instant::now());alert.key=updated.key;}
        }
        changed
    }
    pub fn begin(&mut self,base:Rect,radius:f64,host:Rect,y:f64,now:f64,animate:bool)->bool{
        self.begin_at(base, radius, host, y, now, animate, false)
    }
    pub fn begin_at(&mut self,base:Rect,radius:f64,host:Rect,y:f64,now:f64,animate:bool,bottom:bool)->bool{
        if self.visible(){return false}
        while let Some(a)=self.queue.pop_front(){
            if a.expires<=Instant::now(){continue}
            let island = a.island_only();
            let h = (if island { a.height } else { a.height.clamp(40.,112.) }).min(host.h);
            let margin = 12_f64.min(host.w / 2.);
            let w = a.width.min(host.w - 48.).max(140.).min(host.w - 2.*margin);
            let center = if island { base.x + base.w/2. } else { host.x + host.w/2. };
            let x = (center - w/2.).clamp(host.x + margin, (host.x + host.w - w - margin).max(host.x + margin));
            let top = if bottom { base.y + base.h - h } else { y };
            let top = top.clamp(host.y, host.y + host.h - h);
            self.banner.show_fitted_capsule(Notice::default(),base,radius,Rect::new(x,top,w,h),now,animate);
            self.banner.hold_until(now+a.hold);self.active=Some(a);return true
        }
        false
    }
    /// 取出下一条尚未过期的提醒（供任务栏模式路由到屏幕顶部独立弹窗使用）。
    pub fn pop_next(&mut self)->Option<Alert>{
        self.queue.retain(|a| a.expires > Instant::now());
        if self.next_island_only() { return None; }
        self.queue.pop_front()
    }
    pub fn next_island_only(&self) -> bool {
        self.queue.iter().find(|a| a.expires > Instant::now()).is_some_and(Alert::island_only)
    }
    pub fn tick(&mut self,base:Rect,radius:f64,now:f64,animate:bool){
        // Always return to the current base, including Mini. Visibility is retired by the host.
        self.banner.tick(base,radius,now,false,animate,true);
        if !self.visible(){self.active=None;}
    }
    pub fn dismiss(&mut self,base:Rect,radius:f64,now:f64,animate:bool){self.banner.dismiss(base,radius,now,animate,true);}
    pub fn clear(&mut self,base:Rect,radius:f64,now:f64,animate:bool){self.queue.clear();self.dismiss(base,radius,now,animate);}
    pub fn remove_pending(&mut self,key:&str){self.queue.retain(|a|a.key!=key);self.seen.remove(key);}
    pub fn remove_pending_prefix(&mut self,prefix:&str){self.queue.retain(|a|!a.key.starts_with(prefix));self.seen.retain(|key,_|!key.starts_with(prefix));}
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
            if self.baseline&&d.battery_percent.is_some_and(|v|v<20)&&!warned{
                warned=true;out.push(Alert::device(&d.id,device_icon(&d.kind),&d.name,true,d.battery_percent,true));
            }
            if !self.baseline&&d.battery_percent.is_some_and(|v|v<20){warned=true;}
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
            let begin=if ongoing{"预计处于降水时段".to_owned()}else{format!("预计 {} 开始",w["startLabel"].as_str().unwrap_or("稍后"))};
            let finish=w["endLabel"].as_str().map(|v|format!("预计 {v} 结束")).unwrap_or("结束时间暂不确定".into());
            let resolution=w["resolutionMinutes"].as_u64().or_else(||weather["forecastResolutionMinutes"].as_u64()).unwrap_or(60);
            let precision=if resolution>=180{"约三小时预报"}else{"小时预报"};
            let mut a=Alert::new(key,if kind=="snow"{"Snow"}else{"Rain"},title,format!("{begin} · {finish} · {precision}"));
            a.width=452.;a.height=72.;a.hold=8.;alerts.push(a);break
        }
        alerts
    }
}

#[cfg(test)]mod tests{
    use super::*;
    #[test]
    fn dedicated_card_geometry_fits_host_and_bottom_anchor() {
        use crate::reminder_plugins::{Hydration, Kind};
        let mut layout = crate::layout::Layout::new(260.);
        for bottom in [false, true] {
            layout.configure_anchor(bottom, 48.);
            let base = layout.capsule;
            let mut ordinary = Alert::new("test".into(), "Sun", "测试".into(), String::new());
            ordinary.height = 180.;
            for (alert, height) in [
                (Alert::wellness(Kind::Water).with_hydration(Hydration {goal_ml:2000, serving_ml:250, drank_ml:750}), 148.),
                (Alert::quota("codex", "Codex", [Some(18.), Some(8.)], [20,10]), 122.),
                (ordinary, 112.),
            ] {
                let water = alert.hydration.is_some();
                let mut hub = Hub::default();
                hub.enqueue(alert);
                assert!(hub.begin_at(base, base.h/2., layout.host, base.y, 0., true, bottom));
                for i in 0..100 {
                    let (r, _, _) = hub.banner.presentation(i as f64 / 100.);
                    assert!(r.y >= 0. && r.y + r.h <= layout.host.h + 0.001);
                    if bottom { assert!((r.y + r.h - base.y - base.h).abs() < 0.001); }
                    if water {
                        if let Some(g) = WaterLayout::in_card(r) {
                            assert!(g.track.y + g.track.h + 12. <= g.record.y);
                            assert!(g.later.x + g.later.w + 8. <= g.record.x);
                            assert!(g.record.y + g.record.h + 12. <= r.y + r.h);
                        }
                    }
                }
                let r = hub.banner.presentation(1.).0;
                assert!((r.h - height).abs() < 0.001);
                if water { assert!(WaterLayout::in_card(r).is_some()); }
            }
        }
    }
    #[test]fn message_capsule_restores_classic_and_mini_without_content(){
        for base in [Rect::new(170.,24.,260.,40.),Rect::new(250.,24.,100.,18.)] {
            let mut h=Hub::default();let a=Alert::message("WXWork","企业微信",None);
            let wechat=Alert::message("WeChat","微信",None);
            assert_eq!(wechat.title,"微信有新消息");
            assert!(wechat.detail.is_empty()&&wechat.trailing.is_empty()&&wechat.code.is_none());
            assert_eq!(a.title,"企业微信有新消息");assert!(a.detail.is_empty()&&a.trailing.is_empty()&&a.code.is_none());
            h.enqueue(a);assert!(h.begin(base,base.h/2.,Rect::new(0.,0.,600.,332.),24.,0.,true));
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
    #[test]fn delayed_battery_refreshes_existing_connection_without_second_popup(){
        let mut watch=DeviceWatch::default();watch.update(&[],1,true);
        let mut hub=Hub::default();hub.enqueue(Alert::device("{physical-a}","Headphones","耳机",true,None,false));
        let mut device=d(Some(60));device.container_id=Some("physical-a".into());
        let arrivals=watch.update(&[device],2,true);assert!(hub.refresh_devices(&watch));
        for alert in arrivals{hub.enqueue(alert);}
        assert_eq!(hub.queue.len(),1);assert_eq!(hub.queue[0].trailing,"60%");
        let base=Rect::new(200.,24.,100.,18.);hub.begin(base,9.,Rect::new(0.,0.,600.,332.),24.,0.,false);
        let expires=hub.active.as_ref().unwrap().expires;
        let mut device=d(Some(59));device.container_id=Some("physical-a".into());watch.update(&[device],3,true);
        assert!(hub.refresh_devices(&watch));assert_eq!(hub.active.as_ref().unwrap().trailing,"59%");
        assert_eq!(hub.active.as_ref().unwrap().expires,expires);
    }
    #[test]fn reconnect_is_not_suppressed_for_two_minutes(){
        let mut hub=Hub::default();let alert=Alert::device("a","Mouse","鼠标",true,Some(50),false);
        hub.enqueue(alert.clone());hub.queue.clear();
        hub.seen.insert(alert.key.clone(),Instant::now()-Duration::from_secs(4));hub.enqueue(alert);
        assert_eq!(hub.queue.len(),1);
        let mut watch=DeviceWatch::default();assert!(watch.update(&[d(Some(10))],1,true).is_empty());
    }
    #[test]fn queue_is_bounded_and_one_message_uses_one_surface(){
        let mut h=Hub::default();for i in 0..20{h.enqueue(Alert::new(i.to_string(),"Sun","测试".into(),String::new()));}
        assert_eq!(h.queue.len(),8);let r=Rect::new(200.,24.,100.,18.);
        let host=Rect::new(0.,0.,600.,332.);
        assert!(h.begin(r,9.,host,24.,0.,true));assert!(!h.begin(r,9.,host,24.,0.,true));
        assert_eq!(h.banner.presentation(0.).0,r);
        h.dismiss(r,9.,0.12,true);h.tick(r,9.,2.,true);assert!(!h.visible());
        assert!((h.banner.presentation(2.).0.h-18.).abs()<0.01);
    }
    #[test]fn precipitation_reminders_are_deduplicated_and_stale_is_silent(){
        let mut h=WeatherWatch::default();let mut v=serde_json::json!({"city":"上海","time":1000,"precipitation":[{"startsAt":3600,"endsAt":7200,"kind":"snow","startLabel":"今天09:00","endLabel":"今天10:00"}]});
        let a=h.poll(&v,1000);assert_eq!(a.len(),1);assert!(a[0].detail.contains("10:00"));
        assert!(h.poll(&v,1100).is_empty());v["stale"]=true.into();assert!(h.poll(&v,3700).is_empty());
    }
    #[test]fn verification_preempts_active_alert_immediately_and_dedups_within_30s(){
        let mut h=Hub::default();let base=Rect::new(114.,24.,260.,40.);
        h.enqueue(Alert::device("d1","Headphones","耳机",true,Some(80),false));
        assert!(h.begin(base,20.,Rect::new(0.,0.,488.,332.),24.,0.,true));
        let otp=Alert::verification("sms","短信","582914".into());
        assert!(h.preempt_or_enqueue(otp.clone(),base,20.,488.,24.,0.5,true));
        assert_eq!(h.active.as_ref().and_then(|a|a.code.as_deref()),Some("582914"));
        // 同验证码在 30 秒去重窗口内不会反复打断
        assert!(!h.preempt_or_enqueue(otp,base,20.,488.,24.,1.0,true));
        // 新验证码可立即再次抢占旧验证码
        let otp2=Alert::verification("sms","短信","990123".into());
        assert!(h.preempt_or_enqueue(otp2,base,20.,488.,24.,2.0,true));
        assert_eq!(h.active.as_ref().and_then(|a|a.code.as_deref()),Some("990123"));
    }
}
