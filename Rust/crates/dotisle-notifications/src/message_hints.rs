//! Content-free messaging signals. Never inspect window titles, accessibility
//! text, client databases, process memory, input or notification payloads.
use std::{collections::HashMap,time::{Duration,Instant}};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Hash)]
pub enum App { WeChat, Feishu, DingTalk, WeCom }
impl App {
    pub fn id(self)->&'static str { match self { Self::WeChat=>"WeChat",Self::Feishu=>"Feishu",Self::DingTalk=>"DingTalk",Self::WeCom=>"WXWork" } }
    pub fn name(self)->&'static str { match self { Self::WeChat=>"微信",Self::Feishu=>"飞书",Self::DingTalk=>"钉钉",Self::WeCom=>"企业微信" } }
    pub fn setting(self)->&'static str { match self { Self::WeChat=>"WeChatMessageHintsEnabled",Self::Feishu=>"FeishuMessageHintsEnabled",Self::DingTalk=>"DingTalkMessageHintsEnabled",Self::WeCom=>"WeComMessageHintsEnabled" } }
    pub fn notice(self)->crate::notifications::Notice { crate::notifications::Notice { app_id:self.id().into(),app_name:self.name().into(),title:"有新消息".into(),..Default::default() } }
}

pub fn from_executable(path:&str)->Option<App> {
    let file=path.rsplit(['\\','/']).next()?.to_ascii_lowercase();
    match file.as_str() { "wechat.exe"|"weixin.exe"=>Some(App::WeChat),"feishu.exe"|"lark.exe"=>Some(App::Feishu),"dingtalk.exe"=>Some(App::DingTalk),"wxwork.exe"=>Some(App::WeCom),_=>None }
}

pub fn identify(id:&str,name:&str)->Option<App> {
    if let Some(app)=from_executable(id) { return Some(app) }
    // Exact product tokens only. Substring matches would admit WeChat tools,
    // DingTalk updaters and lookalike applications.
    for token in id.split(['.','!','\\','/',':','_']).chain(std::iter::once(name.trim())) {
        let app=match token.to_ascii_lowercase().as_str() {
            "wechat"|"weixin"|"微信"=>App::WeChat,
            "feishu"|"lark"|"飞书"=>App::Feishu,
            "dingtalk"|"钉钉"=>App::DingTalk,
            "wxwork"|"wecom"|"企业微信"=>App::WeCom,
            _=>continue,
        }; return Some(app)
    } None
}

#[derive(Clone,Debug,serde::Serialize,serde::Deserialize)]
pub struct Hint { pub app_id:String,pub app_name:String,#[serde(default)]pub icon:Option<Vec<u8>> }
impl From<App> for Hint { fn from(app:App)->Self {Self{app_id:app.id().into(),app_name:app.name().into(),icon:None}} }

#[derive(Default)]
pub struct Gate { emitted:HashMap<App,Instant>,flashes:HashMap<App,Instant> }
impl Gate {
    pub fn accept(&mut self,app:App,now:Instant,flash:bool,foreground:bool)->bool {
        if foreground { self.flashes.remove(&app);return false }
        if flash {
            let previous=self.flashes.insert(app,now);
            if previous.is_some_and(|at|now.saturating_duration_since(at)<Duration::from_secs(3)){return false}
        }
        if self.emitted.get(&app).is_some_and(|at|now.saturating_duration_since(*at)<Duration::from_secs(8)){return false}
        self.emitted.insert(app,now);true
    }
}

// Hidden top-level receiver, owned and pumped by the notification worker.
// RegisterShellHookWindow observes Explorer's flash signal without DLL injection.
pub struct Signal {pub app:App,pub executable:String}
pub struct Shell { window:windows::Win32::Foundation::HWND,message:u32 }
impl Shell {
    pub fn new()->Option<Self> { unsafe {
        use windows::{core::w,Win32::UI::WindowsAndMessaging::*};
        // STATIC is a system class: no process-global WNDCLASS lifetime to race.
        let window=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE,w!("STATIC"),w!("DotIsle.MessageSignal"),WS_POPUP,0,0,0,0,None,None,None,None).ok()?;
        let message=RegisterWindowMessageW(w!("SHELLHOOK"));
        if message==0||!RegisterShellHookWindow(window).as_bool(){let _=DestroyWindow(window);return None}
        Some(Self{window,message})
    }}
    pub fn drain(&self)->Vec<Signal> { unsafe {
        use windows::{Win32::{Foundation::HWND,UI::WindowsAndMessaging::*}};
        let mut result=Vec::new();let mut msg=MSG::default();
        for _ in 0..256 { if !PeekMessageW(&mut msg,Some(self.window),0,0,PM_REMOVE).as_bool(){break}
            if msg.message==self.message&&msg.wParam.0==0x8006 {
                if let Some((app,executable))=window_identity(HWND(msg.lParam.0 as *mut _)) { if !result.iter().any(|s:&Signal|s.app==app){result.push(Signal{app,executable})} }
            }
            let _=DispatchMessageW(&msg);
        } result
    }}
}
impl Drop for Shell {fn drop(&mut self){unsafe{use windows::Win32::UI::WindowsAndMessaging::*;let _=DeregisterShellHookWindow(self.window);let _=DestroyWindow(self.window);}}}

pub fn foreground()->Option<App> {unsafe{window_identity(windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow()).map(|(app,_)|app)}}
unsafe fn window_identity(window:windows::Win32::Foundation::HWND)->Option<(App,String)> {
    use windows::{core::PWSTR,Win32::{Foundation::CloseHandle,System::Threading::*,UI::WindowsAndMessaging::GetWindowThreadProcessId}};
    if window.is_invalid(){return None}
    let mut pid=0;GetWindowThreadProcessId(window,Some(&mut pid));
    if pid==0{return None}
    let process=OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION,false,pid).ok()?;
    let mut path=[0u16;32768];let mut size=path.len() as u32;
    let read=QueryFullProcessImageNameW(process,PROCESS_NAME_WIN32,PWSTR(path.as_mut_ptr()),&mut size);
    let _=CloseHandle(process);
    if read.is_err(){return None}
    let path=String::from_utf16_lossy(&path[..size as usize]);
    Some((from_executable(&path)?,path))
}

#[cfg(test)] mod tests {
    use super::*;
    #[test] fn exact_processes_and_product_ids_only() {
        assert_eq!(from_executable(r"C:\Program Files\Tencent\Weixin.exe"),Some(App::WeChat));
        assert_eq!(identify("com.bytedance.Feishu",""),Some(App::Feishu));
        assert_eq!(identify("Tencent.WXWork",""),Some(App::WeCom));
        assert_eq!(identify("DingTalk",""),Some(App::DingTalk));
        for name in ["FakeWeChat.exe","WeChatAppEx.exe","DingTalkUpdater.exe","QQMusic.exe"] {assert_eq!(from_executable(name),None)}
        assert_eq!(identify("FakeWeChat",""),None);
    }
    #[test] fn flashing_and_toast_share_cooldown_without_replaying_foreground() {
        let mut gate=Gate::default();let now=Instant::now();let a=App::WeChat;
        assert!(gate.accept(a,now,true,false));
        assert!(!gate.accept(a,now+Duration::from_millis(400),false,false));
        for n in 1..15 {assert!(!gate.accept(a,now+Duration::from_secs(n),true,false))}
        assert!(!gate.accept(a,now+Duration::from_secs(16),true,true));
        assert!(gate.accept(a,now+Duration::from_secs(20),true,false));
        assert!(gate.accept(App::Feishu,now+Duration::from_secs(20),false,false));
    }
}
