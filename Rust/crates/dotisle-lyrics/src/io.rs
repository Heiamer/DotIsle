//! Bounded lyric lookup diagnostics.
use std::{path::{Path,PathBuf},io::Write,time::{Instant,Duration},sync::{Mutex,OnceLock}};
const LOG_LIMIT:u64=2*1024*1024;
const LOG_FILES:usize=8;
#[derive(Default)]struct LogState{last:std::collections::HashMap<(PathBuf,String), (Instant,usize)>,pruned:Option<Instant>}
static LOG_LOCK:OnceLock<Mutex<LogState>>=OnceLock::new();
fn owned_log(name:&str)->bool{(name.starts_with("app_debug-")||name.starts_with("app_error-"))&&name.ends_with(".log")}
pub fn redact(message:&str)->String{
    let mut out=message.chars().take(4096).filter(|c|*c!='\r'&&*c!='\n'&&*c!='\0').collect::<String>();
    if let Some(home)=std::env::var_os("USERPROFILE"){let home=home.to_string_lossy();if !home.is_empty(){out=out.replace(home.as_ref(),"%USERPROFILE%");}}
    static QUERY:OnceLock<regex::Regex>=OnceLock::new();
    out=QUERY.get_or_init(||regex::Regex::new(r"(https?://[^\s?]+)\?[^\s]+" ).unwrap()).replace_all(&out,"$1?[redacted]").into_owned();
    static SECRET:OnceLock<regex::Regex>=OnceLock::new();
    SECRET.get_or_init(||regex::Regex::new(r"(?i)(authorization|access_token|refresh_token|cookie|password|secret)\s*[:=]\s*[^\s,;]+").unwrap()).replace_all(&out,"$1=[redacted]").into_owned()
}
fn prune(directory:&Path){
    let Ok(files)=std::fs::read_dir(directory)else{return};
    let mut files=files.flatten().filter_map(|entry|{let name=entry.file_name().to_string_lossy().to_string();if !owned_log(&name){return None}let meta=entry.metadata().ok()?;if !meta.is_file(){return None}Some((meta.modified().ok()?,entry.path()))}).collect::<Vec<_>>();
    files.sort_by(|a,b|b.0.cmp(&a.0));
    let mut bytes=0u64;
    for(index,(modified,path))in files.into_iter().enumerate(){bytes+=std::fs::metadata(&path).map(|m|m.len()).unwrap_or(0);if index>=LOG_FILES||bytes>18*1024*1024||modified.elapsed().is_ok_and(|age|age>Duration::from_secs(7*86400)){let _=std::fs::remove_file(path);}}
}
pub fn log(directory:&Path,category:&str,message:&str){
    let Ok(mut state)=LOG_LOCK.get_or_init(Default::default).lock()else{return};
    let now=Instant::now();let message=redact(message);let category=category.chars().take(48).collect::<String>();
    let key=(directory.to_path_buf(),format!("{category}|{message}"));let mut repeats=0;
    if let Some((last,count))=state.last.get_mut(&key){if last.elapsed()<Duration::from_secs(15){*count+=1;return}repeats=*count;*last=now;*count=0;}
    else{if state.last.len()>=128{state.last.retain(|_,(time,_)|time.elapsed()<Duration::from_secs(60));if state.last.len()>=128{state.last.clear();}}state.last.insert(key,(now,0));}
    let _=std::fs::create_dir_all(directory);
    if state.pruned.is_none_or(|last|last.elapsed()>Duration::from_secs(60)){prune(directory);state.pruned=Some(now);}
    let time=unsafe{windows::Win32::System::SystemInformation::GetLocalTime()};let today=format!("{:04}-{:02}-{:02}",time.wYear,time.wMonth,time.wDay);
    let prefix=if category.contains("ERROR")||category=="NATIVE_RENDER"{"app_error"}else{"app_debug"};
    let path=directory.join(format!("{prefix}-{today}.log"));
    if std::fs::metadata(&path).is_ok_and(|m|m.len()>=LOG_LIMIT){let stamp=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();let _=std::fs::rename(&path,directory.join(format!("{prefix}-{today}.{stamp}.log")));prune(directory);}
    if let Ok(mut file)=std::fs::OpenOptions::new().create(true).append(true).open(path){let _=writeln!(file,"[{} {:02}:{:02}:{:02}.{:03}] [{} pid={}] {}{}",today,time.wHour,time.wMinute,time.wSecond,time.wMilliseconds,category,std::process::id(),message,if repeats>0{format!(" (suppressed {repeats} identical repeats)")}else{String::new()});}
}
#[cfg(test)]mod log_tests{use super::*;
#[test]fn redacts_secrets_and_limits_each_event(){let s=redact("access_token=abc cookie=x https://example.com/a?token=hidden\nnext");assert!(!s.contains("abc"));assert!(!s.contains("hidden"));assert!(!s.contains('\n'));assert_eq!(redact(&"x".repeat(10000)).len(),4096);}
#[test]fn bounds_owned_logs_and_keeps_other_files(){let dir=std::env::temp_dir().join(format!("dotisle-log-budget-{}",std::process::id()));std::fs::create_dir_all(&dir).unwrap();for i in 0..15{std::fs::write(dir.join(format!("app_debug-test.{i}.log")),"test").unwrap();}std::fs::write(dir.join("user.txt"),"keep").unwrap();prune(&dir);assert!(std::fs::read_dir(&dir).unwrap().count()<=9);assert!(dir.join("user.txt").exists());let _=std::fs::remove_dir_all(dir);}
}
