// Optional current-user compatibility reader. Never writes to the Windows store or reads
// deleted records. Notification content is transient and is never logged or uploaded.
use std::{collections::{HashSet,HashMap},ffi::{c_char,c_void,CStr,CString},path::{Path,PathBuf},time::{SystemTime,UNIX_EPOCH,Instant,Duration}};
use windows::{core::{PCSTR,PCWSTR},Win32::{Foundation::{FreeLibrary,HMODULE},System::LibraryLoader::{LoadLibraryW,GetProcAddress}}};
use crate::notifications::{Notice,Policy};
type Ptr=*mut c_void;
struct Api{library:HMODULE,open:unsafe extern "C" fn(*const c_char,*mut Ptr,i32,*const c_char)->i32,close:unsafe extern "C" fn(Ptr)->i32,prepare:unsafe extern "C" fn(Ptr,*const c_char,i32,*mut Ptr,*mut *const c_char)->i32,step:unsafe extern "C" fn(Ptr)->i32,finalize:unsafe extern "C" fn(Ptr)->i32,text:unsafe extern "C" fn(Ptr,i32)->*const u8,blob:unsafe extern "C" fn(Ptr,i32)->*const c_void,bytes:unsafe extern "C" fn(Ptr,i32)->i32,int:unsafe extern "C" fn(Ptr,i32)->i64,busy:unsafe extern "C" fn(Ptr,i32)->i32}
impl Drop for Api{fn drop(&mut self){unsafe{let _=FreeLibrary(self.library);}}}
impl Api{
    unsafe fn load()->Result<Self,String>{
        let path=std::env::current_exe().map_err(|_|"无法定位通知组件")?.parent().ok_or("无法定位通知组件")?.join("e_sqlite3.dll");
        #[cfg(test)]let path=std::env::var_os("DOTISLE_TEST_SQLITE3").map(PathBuf::from).unwrap_or(path);
        let path=path.to_string_lossy().encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let library=LoadLibraryW(PCWSTR(path.as_ptr())).map_err(|_|"通知数据库组件缺失，请完整解压程序")?;
        macro_rules! symbol{($name:literal)=>{{match GetProcAddress(library,PCSTR(concat!($name,"\0").as_ptr())){Some(p)=>std::mem::transmute(p),None=>{let _=FreeLibrary(library);return Err("通知数据库组件版本不兼容".into())}}}}}
        Ok(Self{library,open:symbol!("sqlite3_open_v2"),close:symbol!("sqlite3_close"),prepare:symbol!("sqlite3_prepare_v2"),step:symbol!("sqlite3_step"),finalize:symbol!("sqlite3_finalize"),text:symbol!("sqlite3_column_text"),blob:symbol!("sqlite3_column_blob"),bytes:symbol!("sqlite3_column_bytes"),int:symbol!("sqlite3_column_int64"),busy:symbol!("sqlite3_busy_timeout")})
    }
}
struct Db<'a>(&'a Api,Ptr);impl Drop for Db<'_>{fn drop(&mut self){unsafe{(self.0.close)(self.1);}}}
struct Statement<'a>(&'a Api,Ptr);impl Drop for Statement<'_>{fn drop(&mut self){unsafe{(self.0.finalize)(self.1);}}}
pub fn filetime()->i64{(SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_nanos()/100)as i64+116444736000000000}
fn path()->PathBuf{
    if std::env::var("LYRICISLAND_TEST_PIPE").is_ok_and(|s|s.starts_with("LyricIsland.Settings.test.")){
        if let Some(path)=std::env::var_os("LYRICISLAND_NOTIFICATION_DB"){return PathBuf::from(path)}
    }
    PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()).join("Microsoft/Windows/Notifications/wpndatabase.db")
}
type Stamp=(Option<(SystemTime,u64)>,Option<(SystemTime,u64)>,Option<(SystemTime,u64)>);
fn stamp(path:&Path)->Stamp{
    let info=|p:&Path|std::fs::metadata(p).ok().and_then(|m|m.modified().ok().map(|t|(t,m.len())));
    let raw=path.to_string_lossy();
    (info(path),info(&PathBuf::from(format!("{raw}-wal"))),info(&PathBuf::from(format!("{raw}-shm"))))
}
pub struct Store{icons:HashMap<String,Option<Vec<u8>>>,last_read:Instant,stamp:Option<Stamp>,since:i64,seen:HashSet<(i64,i64,i64)>,api:Option<Api>}
impl Store{
    pub fn new()->Self{Self{icons:HashMap::new(),last_read:Instant::now()-Duration::from_secs(2),stamp:None,since:filetime(),seen:HashSet::new(),api:None}}
    pub fn reset(&mut self){self.since=filetime();self.seen.clear();self.stamp=None;}
    pub fn poll(&mut self,policy:&Policy)->Result<Vec<Notice>,String>{unsafe{
        if self.api.is_none(){self.api=Some(Api::load()?);}
        let api=self.api.as_ref().unwrap();let path=path();
        let current=stamp(&path);
        // 文件元信息未变化时兜底周期压缩至 500ms，防止 NTFS/WAL 元数据延迟更新漏检新验证码
        if self.stamp.as_ref()==Some(&current)&&self.last_read.elapsed()<Duration::from_millis(500){return Ok(vec![])}
        let rows=read_with_retry(api,&path,self.since,policy)?;self.stamp=Some(current);self.last_read=Instant::now();
        let mut notices=vec![];let mut latest=self.since;
        for (key,id,payload) in rows{
            latest=latest.max(key.2);
            if !self.seen.insert(key)||policy.paused{continue}
            let name=app_name(&id);
            if crate::message_hints::identify(&id,&name).is_some_and(|app|app.content_only())||!policy.permits(&id,&name){continue}
            if notices.len()<8{if let Some(app)=crate::message_hints::identify(&id,&name){let mut notice=app.notice();notice.icon=crate::app_icon::shell_icon(&id).or_else(||(app==crate::message_hints::App::WeChat).then(crate::app_icon::wechat_icon));notices.push(notice);continue}if let Some(mut notice)=parse(&payload,&id){
                if notice.icon.is_none(){if !self.icons.contains_key(&id){if self.icons.len()>=64{self.icons.clear();}let icon=crate::app_icon::shell_icon(&id);self.icons.insert(id.clone(),icon);}notice.icon=self.icons.get(&id).cloned().flatten();}
                notices.push(notice);
            }}
        }
        self.since=latest;
        // 保留近 10 秒的已处理主键，配合 2 秒回看窗防止晚落盘记录漏检且零重复
        self.seen.retain(|k|k.2>=latest.saturating_sub(100_000_000));
        Ok(notices)
    }}
}
type Row=((i64,i64,i64),String,Vec<u8>);
enum ReadErr{Busy(String),Fatal(String)}
unsafe fn read_with_retry(api:&Api,path:&Path,since:i64,policy:&Policy)->Result<Vec<Row>,String>{
    for attempt in 0..3 {
        match read_inner(api,path,since,policy) {
            Ok(rows)=>return Ok(rows),
            Err(ReadErr::Busy(_)) if attempt<2 => std::thread::sleep(Duration::from_millis(25)),
            Err(ReadErr::Busy(e)|ReadErr::Fatal(e))=>return Err(e),
        }
    }
    unreachable!()
}
unsafe fn read(api:&Api,path:&Path,since:i64,policy:&Policy)->Result<Vec<Row>,String>{
    read_with_retry(api,path,since,policy)
}
unsafe fn read_inner(api:&Api,path:&Path,since:i64,policy:&Policy)->Result<Vec<Row>,ReadErr>{
    let file=CString::new(path.to_string_lossy().as_bytes()).map_err(|_|ReadErr::Fatal("通知数据库路径无效".into()))?;let mut raw=std::ptr::null_mut();
    // SQLITE_OPEN_READONLY | SQLITE_OPEN_NOMUTEX. Never CREATE, READWRITE, backup or checkpoint.
    let rc=(api.open)(file.as_ptr(),&mut raw,1|0x8000,std::ptr::null());
    if rc!=0{if !raw.is_null(){(api.close)(raw);}let msg="通知数据库暂不可读，请稍后重试或反馈错误".into();return Err(if rc==5||rc==6{ReadErr::Busy(msg)}else{ReadErr::Fatal(msg)})}
    let db=Db(api,raw);(api.busy)(db.1,250);
    // 向前回看 2 秒（20_000_000 个 100ns 刻度），防止系统时间戳乱序或晚落盘通知丢失
    let lookback=since.saturating_sub(20_000_000);
    let sql=CString::new(format!("SELECT n.Id,n.HandlerId,n.ArrivalTime,h.PrimaryId FROM Notification n JOIN NotificationHandler h ON n.HandlerId=h.RecordId WHERE n.Type='toast' AND n.ArrivalTime>={lookback} AND n.ArrivalTime<={} ORDER BY n.ArrivalTime,n.[Order] LIMIT 256",filetime()+300000000)).unwrap();
    let mut raw=std::ptr::null_mut();if (api.prepare)(db.1,sql.as_ptr(),-1,&mut raw,std::ptr::null_mut())!=0{if !raw.is_null(){(api.finalize)(raw);}return Err(ReadErr::Fatal("此 Windows 版本的通知格式尚不兼容，请稍后重试或反馈错误".into()))}
    let statement=Statement(api,raw);let mut rows=vec![];
    loop{match (api.step)(statement.1){
        100=>{
            let key=((api.int)(statement.1,0),(api.int)(statement.1,1),(api.int)(statement.1,2));
            let id=(api.text)(statement.1,3);if id.is_null(){continue}let id=CStr::from_ptr(id.cast()).to_string_lossy().into_owned();
            let name=app_name(&id);let mut payload=vec![];
            if policy.enabled&&!policy.paused&&policy.permits(&id,&name)&&crate::message_hints::identify(&id,&name).is_none(){payload=read_payload(api,&db,key).map_err(ReadErr::Fatal)?;}
            rows.push((key,id,payload));
        },
        101=>break,
        5|6=>return Err(ReadErr::Busy("通知数据库正忙，稍后自动重试".into())),
        _=>return Err(ReadErr::Fatal("通知数据库正忙或已更新，稍后自动重试".into()))
    }}Ok(rows)
}
// Read actual toast content only after the application policy permits it.
unsafe fn read_payload(api:&Api,db:&Db<'_>,key:(i64,i64,i64))->Result<Vec<u8>,String>{
    let sql=CString::new(format!("SELECT Payload FROM Notification WHERE Id={} AND HandlerId={} AND ArrivalTime={} LIMIT 1",key.0,key.1,key.2)).unwrap();
    let mut raw=std::ptr::null_mut();
    if (api.prepare)(db.1,sql.as_ptr(),-1,&mut raw,std::ptr::null_mut())!=0{if !raw.is_null(){(api.finalize)(raw);}return Err("无法读取通知记录".into())}
    let statement=Statement(api,raw);
    if (api.step)(statement.1)!=100{return Ok(vec![])}
    let size=(api.bytes)(statement.1,0);if !(1..=262144).contains(&size){return Ok(vec![])}
    let blob=(api.blob)(statement.1,0);if blob.is_null(){return Ok(vec![])}
    Ok(std::slice::from_raw_parts(blob.cast::<u8>(),size as usize).to_vec())
}
fn app_name(id:&str)->String{
    windows::ApplicationModel::AppInfo::GetFromAppUserModelId(&windows::core::HSTRING::from(id)).and_then(|a|a.DisplayInfo()?.DisplayName()).map(|n|n.to_string()).unwrap_or_else(|_|{
        let lower=id.to_ascii_lowercase();if lower.contains("qq")&&!lower.contains("qqmusic"){if lower.contains(".nt")||lower.contains("qqnt"){"QQNT".into()}else{"QQ".into()}}else{id.into()}
    })
}
fn decode(bytes:&[u8])->Option<String>{
    let text=if bytes.starts_with(&[0xFF,0xFE]){
        String::from_utf16_lossy(&bytes[2..].chunks_exact(2).map(|c|u16::from_le_bytes([c[0],c[1]])).collect::<Vec<_>>())
    }else if bytes.starts_with(&[0xFE,0xFF]){
        String::from_utf16_lossy(&bytes[2..].chunks_exact(2).map(|c|u16::from_be_bytes([c[0],c[1]])).collect::<Vec<_>>())
    }else if bytes.starts_with(&[0xEF,0xBB,0xBF]){
        std::str::from_utf8(&bytes[3..]).ok()?.to_owned()
    }else{
        std::str::from_utf8(bytes).ok()?.to_owned()
    };
    Some(text.trim_matches(['\u{feff}','\0']).to_owned())
}
fn parse(bytes:&[u8],id:&str)->Option<Notice>{
    if bytes.is_empty()||bytes.len()>262144{return None}
    let name=app_name(id);
    if crate::message_hints::identify(id,&name).is_some_and(|a|!a.uses_toast()||a.content_only()){return None}
    let xml=decode(bytes)?;let document=roxmltree::Document::parse(xml.trim_matches(['\u{feff}','\0'])).ok()?;
    if !document.root_element().has_tag_name("toast"){return None}
    let binding=document.descendants().find(|n|n.has_tag_name("binding")&&n.attribute("template")==Some("ToastGeneric")).or_else(||document.descendants().find(|n|n.has_tag_name("binding")))?;
    let mut text=binding.descendants().filter(|n|n.has_tag_name("text")).map(|n|n.descendants().filter(|c|c.is_text()).filter_map(|c|c.text()).collect::<String>()).map(|s|s.trim().to_owned()).filter(|s|!s.is_empty()).take(8).map(|s|s.chars().take(1024).collect::<String>()).collect::<Vec<_>>();
    // 部分系统/应用通知将关键主题写在 <header title="...">，缺失或独立时补充到标题前段
    if let Some(header_title)=document.descendants().find(|n|n.has_tag_name("header")).and_then(|n|n.attribute("title")).map(str::trim).filter(|s|!s.is_empty()){
        if text.is_empty(){
            text.push(header_title.to_owned());
        }else if !text[0].contains(header_title){
            text[0]=format!("{header_title} · {}",text[0]);
        }
    }
    let title=text.first()?.clone();let body=text.into_iter().skip(1).collect::<Vec<_>>().join("\n");
    // Feishu and DingTalk are handled exclusively by the WinRT listener.
    let icon = crate::toast_avatar::from_binding(id, binding);
    Some(Notice{app_id:id.into(),app_name:name,title,body,icon})
}
#[cfg(test)]mod tests{use super::*;
    #[test]fn messaging_metadata_works_even_when_payload_column_does_not_exist(){unsafe{
        if std::env::var_os("DOTISLE_TEST_SQLITE3").is_none(){eprintln!("isolated SQLite fixture not configured");return}
        let api=Api::load().unwrap();
        type Exec=unsafe extern "C" fn(Ptr,*const c_char,Option<unsafe extern "C" fn(Ptr,i32,*mut *mut c_char,*mut *mut c_char)->i32>,Ptr,*mut *mut c_char)->i32;
        let exec:Exec=std::mem::transmute(GetProcAddress(api.library,PCSTR(b"sqlite3_exec\0".as_ptr())).unwrap());
        let path=std::env::temp_dir().join(format!("dotisle-message-metadata-{}-{}.db",std::process::id(),SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()));
        let file=CString::new(path.to_string_lossy().as_bytes()).unwrap();let mut raw=std::ptr::null_mut();
        assert_eq!((api.open)(file.as_ptr(),&mut raw,2|4,std::ptr::null()),0);
        let db=Db(&api,raw);
        // Deliberately omit Payload: selecting any chat content must fail.
        let schema=CString::new("CREATE TABLE NotificationHandler(RecordId INTEGER,PrimaryId TEXT);CREATE TABLE Notification(Id INTEGER,HandlerId INTEGER,ArrivalTime INTEGER,Type TEXT,[Order] INTEGER);INSERT INTO NotificationHandler VALUES(1,'Tencent.WXWork'),(2,'com.bytedance.Feishu'),(3,'DingTalk'),(4,'Tencent.WXWork'),(5,'Tencent.WeChat'),(6,'Tencent.Weixin');INSERT INTO Notification VALUES(1,1,100,'toast',1),(2,2,101,'toast',2),(3,3,102,'toast',3),(4,4,103,'toast',4),(5,5,104,'toast',5),(6,6,105,'toast',6);").unwrap();
        assert_eq!(exec(db.1,schema.as_ptr(),None,std::ptr::null_mut(),std::ptr::null_mut()),0);drop(db);
        let rows=read(&api,&path,0,&Policy::from_settings(&serde_json::json!({"FeishuMessageHintsEnabled":false,"DingTalkMessageHintsEnabled":false}))).unwrap();
        assert_eq!(rows.len(),6);assert!(rows.iter().all(|row|row.2.is_empty()));
        std::fs::remove_file(path).unwrap();
    }}
    #[test]fn parses_escaped_xml_and_multiple_text_lines(){let n=parse(br#"<toast><visual><binding template="ToastGeneric"><text>A &amp; B</text><text>second</text></binding></visual></toast>"#,"Example.App").unwrap();assert_eq!(n.title,"A & B");assert_eq!(n.body,"second");}
    #[test]fn wechat_toasts_never_parse() {
        let xml=br#"<toast><visual><binding template="ToastGeneric"><text>Sender</text><text>Actual message</text></binding></visual></toast>"#;
        for id in ["Tencent.WeChat", "Tencent.Weixin", r"C:\Program Files\Tencent\Weixin.exe", "微信"] {
            assert!(parse(xml,id).is_none(), "{id}");
        }
    }
    #[test]fn desktop_toast_apps_do_not_use_legacy_database_payloads(){
        let xml=br#"<toast><visual><binding template="ToastGeneric"><text>Sender</text><text>Actual message</text></binding></visual></toast>"#;
        for id in ["com.bytedance.Feishu","DingTalk"] { assert!(parse(xml,id).is_none()); }
    }
    #[test]fn rejects_malformed_oversize_and_non_toast_payloads(){assert!(parse(b"<broken>","a").is_none());assert!(parse(b"<tile />","a").is_none());assert!(parse(&vec![b'x';262145],"a").is_none());assert!(parse(br#"<!DOCTYPE toast [<!ENTITY x SYSTEM "file:///private">]><toast>&x;</toast>"#,"a").is_none());}
    #[test]fn new_store_starts_at_current_time_without_replaying_history(){let s=Store::new();assert!(s.since>116444736000000000);assert!(s.seen.is_empty());}
    #[test]fn decodes_utf8_bom_utf16be_and_header_title_entities(){
        let utf8_bom=[b"\xEF\xBB\xBF".as_slice(),r#"<toast><header id="1" title="安全中心"/><visual><binding template="ToastGeneric"><text>验证码 &amp; 提醒</text><text>您的验证码为 482910</text></binding></visual></toast>"#.as_bytes()].concat();
        let n=parse(&utf8_bom,"Example.Auth").unwrap();
        assert_eq!(n.title,"安全中心 · 验证码 & 提醒");
        assert_eq!(n.body,"您的验证码为 482910");
        let utf16_be:Vec<u8>=[0xFEu8,0xFF].into_iter().chain("你好".encode_utf16().flat_map(|u|u.to_be_bytes())).collect();
        assert_eq!(decode(&utf16_be).as_deref(),Some("你好"));
    }
}
