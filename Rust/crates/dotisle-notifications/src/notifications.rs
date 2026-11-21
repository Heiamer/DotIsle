// Read-only current-user notification receiver; presentation uses Vello.
use std::{sync::{Arc,Mutex,mpsc,atomic::{AtomicBool,AtomicU64,Ordering}},time::Duration};
use serde_json::{Value,json};
use windows::Win32::System::WinRT::{RoInitialize,RoUninitialize,RO_INIT_MULTITHREADED};
#[derive(Clone,Debug,Default,serde::Serialize,serde::Deserialize)]
pub struct Notice {pub app_id:String,pub app_name:String,pub title:String,pub body:String,pub icon:Option<Vec<u8>>}
impl Notice {
    pub fn example(base:&std::path::Path)->Self {Self{app_id:"LyricIsland".into(),app_name:"点屿".into(),title:"通知预览".into(),body:"这是一条通知预览。音乐继续，重要消息也不会错过。".into(),icon:std::fs::read(base.join("Assets/LyricIsland-v3.png")).ok()}}
}
#[derive(Clone,Default,serde::Serialize,serde::Deserialize)]
pub struct Policy {pub enabled:bool,pub paused:bool,pub whitelist_enabled:bool,pub allowed:Vec<String>,pub blacklist_enabled:bool,#[serde(default)]pub message_apps:Vec<String>,pub blocked:Vec<String>}
impl Policy {
    pub fn from_settings(v:&Value)->Self {Self{message_apps:[crate::message_hints::App::WeChat,crate::message_hints::App::Feishu,crate::message_hints::App::DingTalk,crate::message_hints::App::WeCom].into_iter().filter(|app|v[app.setting()].as_bool().unwrap_or(true)).map(|app|app.id().to_owned()).collect(),blacklist_enabled:v["NotificationBlacklistEnabled"].as_bool().unwrap_or(false),blocked:v["NotificationBlockedApps"].as_array().map(|a|a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_default(),enabled:v["NotificationsEnabled"].as_bool().unwrap_or(true),paused:v["PauseNotifications"].as_bool().unwrap_or(false),whitelist_enabled:v["NotificationWhitelistEnabled"].as_bool().unwrap_or(false),allowed:v["NotificationAllowedApps"].as_array().map(|a|a.iter().filter_map(Value::as_str).map(str::to_owned).collect()).unwrap_or_else(||vec!["QQNT".into(),"QQ".into()])}}
    fn matches(rule:&str,id:&str,name:&str)->bool{
        if rule.eq_ignore_ascii_case(id){return true}
        if let Some(app)=crate::message_hints::identify(id,name){if crate::message_hints::identify(rule,rule)==Some(app){return true}}
        let id=id.to_lowercase();let name=name.trim().to_lowercase();let rule=rule.trim().to_lowercase();
        let tokens=id.split(['.','!','/','\\',':','_']).collect::<Vec<_>>();
        let nt=name=="qqnt"||name=="qq nt"||tokens.contains(&"qqnt")||(tokens.contains(&"qq")&&tokens.contains(&"nt"));
        if rule=="qqnt"{return nt}if rule=="qq"{return !nt&&(["qq","腾讯qq","腾讯 qq"].contains(&name.as_str())||tokens.contains(&"qq"))}false
    }
    pub fn permits(&self,id:&str,name:&str)->bool{
        if let Some(app)=crate::message_hints::identify(id,name){if !self.message_apps.iter().any(|id|id==app.id()){return false}}
        if self.blacklist_enabled&&self.blocked.iter().any(|rule|Self::matches(rule,id,name)){return false}
        !self.whitelist_enabled||self.allowed.iter().any(|rule|Self::matches(rule,id,name))
    }
    pub fn should_show(&self)->bool{self.enabled&&!self.paused}
}
#[derive(serde::Serialize,serde::Deserialize)]pub enum Event { Notice(Notice), MessageHint(crate::message_hints::Hint), State(Value) }
pub struct Listener {pub events:mpsc::Receiver<Event>,policy:Arc<Mutex<Policy>>,stop:Arc<AtomicBool>,epoch:Arc<AtomicU64>,thread:Option<std::thread::JoinHandle<()>>}
impl Drop for Listener {fn drop(&mut self){self.stop.store(true,Ordering::Release);if let Some(t)=self.thread.take(){let _=t.join();}}}
impl Listener {
    pub fn new(policy:Policy)->Self {
        let policy=Arc::new(Mutex::new(policy));let stop=Arc::new(AtomicBool::new(false));let (tx,events)=mpsc::sync_channel(128);let epoch=Arc::new(AtomicU64::new(0));
        let shared=policy.clone();let stopping=stop.clone();let generation=epoch.clone();
        let thread=std::thread::spawn(move||{
            let initialized=unsafe{RoInitialize(RO_INIT_MULTITHREADED)}.is_ok();
            let wechat=crate::wechat::Reader::new(shared.clone(),generation.clone());let mut wechat_state=json!({"status":"starting","ready":false});
            let shell=crate::message_hints::Shell::new();let mut gate=crate::message_hints::Gate::default();let mut icons=std::collections::HashMap::new();
            let mut last_state=Value::Null;let mut last_enabled=false;let mut last_paused=false;let mut apps_cache=json!([]);let mut store=crate::notification_store::Store::new();
            while !stopping.load(Ordering::Acquire) {
                let p=shared.lock().map(|p|p.clone()).unwrap_or_default();
                if p.enabled!=last_enabled || (last_paused&&!p.paused){store.reset();}
                last_enabled=p.enabled;last_paused=p.paused;
                let mut state=json!({"enabled":p.enabled,"paused":p.paused,"access":"disabled","source":"database","message":"通知提醒已关闭","apps":apps_cache});
                if p.enabled {
                    match store.poll(&p){
                        Ok(notices)=>{state["access"]=json!("compatible");state["message"]=json!(if p.paused{"通知已暂停，恢复后只接收新消息"}else if p.blacklist_enabled{"通知接收已连接，屏蔽黑名单中的应用"}else if p.whitelist_enabled{"通知接收已连接，按白名单提醒新通知"}else{"通知接收已连接，提醒各应用的新通知"});
                            for notice in notices{if !apps_cache.as_array().is_some_and(|a|a.iter().any(|a|a["id"]==notice.app_id)){if apps_cache.as_array().is_none_or(|a|a.len()>=32){apps_cache=json!([]);}apps_cache.as_array_mut().unwrap().push(json!({"id":notice.app_id,"name":notice.app_name}));}if let Some(app)=crate::message_hints::identify(&notice.app_id,&notice.app_name){
                                if app.content_only(){if p.should_show()&&app!=crate::message_hints::App::WeChat&&!notice.body.trim().is_empty(){let _=tx.try_send(Event::Notice(content_notice(app,notice)));}continue}
                                if p.should_show()&&gate.accept(app,std::time::Instant::now(),false,crate::message_hints::foreground()==Some(app)){let mut hint:crate::message_hints::Hint=app.into();hint.icon=notice.icon;let _=tx.try_send(Event::MessageHint(hint));}
                            }else{let _=tx.try_send(Event::Notice(notice));}}
                            state["apps"]=apps_cache.clone();
                        },Err(error)=>{state["access"]=json!("unavailable");state["message"]=json!(error);}
                    }
                }
                state["messageHints"]=json!({"taskbar":shell.is_some(),"contentRead":wechat_state["ready"],"wechat":wechat_state,"apps":p.message_apps});
                if state!=last_state {let _=tx.try_send(Event::State(state.clone()));last_state=state;}
                for _ in 0..8 {if stopping.load(Ordering::Acquire){break}
                    for update in wechat.events.try_iter().take(32){match update{crate::wechat::Update::Notice(mut n,epoch)=>{if n.icon.is_none(){n.icon=Some(crate::app_icon::wechat_icon());}let current=shared.lock().map(|p|p.clone()).unwrap_or_default();if epoch==generation.load(Ordering::Acquire)&&current.should_show()&&current.permits(&n.app_id,&n.app_name){let _=tx.try_send(Event::Notice(n));}},crate::wechat::Update::State(s)=>wechat_state=s}}
                    if let Some(shell)=&shell{for signal in shell.drain(){let app=signal.app;if app.content_only(){continue}
                        let current=shared.lock().map(|p|p.clone()).unwrap_or_default();
                        let fresh=gate.accept(app,std::time::Instant::now(),true,crate::message_hints::foreground()==Some(app));
                        if current.should_show()&&current.permits(app.id(),app.name())&&fresh{let icon=icons.entry(app).or_insert_with(||crate::app_icon::shell_icon(&signal.executable)).clone();let mut hint:crate::message_hints::Hint=app.into();hint.icon=icon;let _=tx.try_send(Event::MessageHint(hint));}
                    }}std::thread::sleep(Duration::from_millis(50));}
            }
            if initialized {unsafe{RoUninitialize()}}
        });Self{events,policy,stop,epoch,thread:Some(thread)}
    }
    pub fn configure(&self,p:Policy){if let Ok(mut old)=self.policy.lock(){if old.should_show()!=p.should_show()||old.permits("WeChat","微信")!=p.permits("WeChat","微信"){self.epoch.fetch_add(1,Ordering::AcqRel);}*old=p;}}
}
fn content_notice(app:crate::message_hints::App,mut notice:Notice)->Notice{notice.app_id=app.id().into();notice.app_name=app.name().into();notice}
pub fn diagnostics()->Value {json!({"packaged":windows::ApplicationModel::Package::Current().is_ok(),"readOnly":true,"source":"database","rendering":"Vello Hybrid 0.2 / wgpu 29"})}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn messaging_toggles_and_alias_filters_do_not_fall_back_to_content(){
        let mut p=Policy::from_settings(&json!({"WeChatMessageHintsEnabled":false}));
        assert!(!p.permits("Tencent.WeChat","微信"));assert!(p.permits("Feishu","飞书"));
        p.blacklist_enabled=true;p.blocked=vec!["企业微信".into()];assert!(!p.permits("WXWork","企业微信"));
        p.whitelist_enabled=true;p.allowed=vec!["飞书".into()];assert!(p.permits("com.bytedance.Feishu","飞书"));assert!(!p.permits("DingTalk","钉钉"));
        let n=crate::message_hints::App::WeChat.notice();assert!(n.title.is_empty());assert!(n.body.is_empty());
    }
    #[test] fn qq_rules_do_not_admit_qq_music_or_lookalikes(){
        let p=Policy{whitelist_enabled:true,allowed:vec!["QQ".into(),"QQNT".into()],..Default::default()};
        assert!(p.permits("Tencent.QQ.NT","QQ"));assert!(p.permits("QQNT","QQNT"));assert!(p.permits("C:\\QQ\\QQ.exe","QQ"));
        assert!(!p.permits("Tencent.QQMusic","QQ音乐"));assert!(!p.permits("FakeQQ","NotQQ"));
    }
    #[test] fn custom_apps_use_exact_identifiers(){let p=Policy{whitelist_enabled:true,allowed:vec!["Example.App_123!Main".into()],..Default::default()};assert!(p.permits("Example.App_123!Main","Example"));assert!(!p.permits("Example.App_123!Other","Example"));}
    #[test] fn pause_defaults_off_and_is_independent_of_removed_dnd_option(){
        let mut p=Policy::from_settings(&json!({"NotificationsEnabled":true,"FollowWindowsDnd":true}));assert!(!p.paused);assert!(p.should_show());p.paused=true;assert!(!p.should_show());p.paused=false;assert!(p.should_show());p.enabled=false;assert!(!p.should_show());
    }

    #[test] fn disabling_qqnt_does_not_leave_it_allowed_by_legacy_rule(){let p=Policy{whitelist_enabled:true,allowed:vec!["QQ".into()],..Default::default()};assert!(!p.permits("Tencent.QQ.NT","QQ"));assert!(p.permits("Tencent.QQ","QQ"));}
}

#[cfg(test)]mod whitelist_option_tests{use super::*;#[test]fn defaults_off_and_filters_only_when_enabled(){let mut p=Policy::from_settings(&json!({}));assert!(!p.whitelist_enabled);assert!(p.permits("Example.Other","Other"));p.whitelist_enabled=true;assert!(!p.permits("Example.Other","Other"));assert!(p.permits("QQNT","QQNT"));}}

#[cfg(test)]mod blacklist_tests{use super::*;
#[test]fn deny_takes_precedence_and_disabled_list_preserves_rules(){let mut policy=Policy::from_settings(&json!({"NotificationBlacklistEnabled":true,"NotificationWhitelistEnabled":true,"NotificationAllowedApps":["Example.App"],"NotificationBlockedApps":["example.app"]}));assert!(!policy.permits("Example.App","Example"));policy.blacklist_enabled=false;assert!(policy.permits("Example.App","Example"));assert_eq!(policy.blocked.len(),1);}
#[test]fn blocked_custom_ids_do_not_match_lookalikes(){let policy=Policy::from_settings(&json!({"NotificationBlacklistEnabled":true,"NotificationBlockedApps":["Example.App_1!Main"]}));assert!(!policy.permits("Example.App_1!Main","App"));assert!(policy.permits("Example.App_1!Other","App"));assert!(policy.permits("FakeExample.App_1!Main","App"));}
}
