//! Versioned, bounded lyric cache. New keys never reuse historic pipeline payloads.
use std::{collections::BTreeMap,path::PathBuf,sync::Mutex,time::{SystemTime,UNIX_EPOCH},io::Write};
use serde::{de::DeserializeOwned,Serialize,Deserialize};
#[derive(Serialize,Deserialize)]struct Record{written:u64,value:serde_json::Value}
pub struct Cache{path:PathBuf,entries:Mutex<Option<BTreeMap<String,Record>>>}
fn now()->u64{SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()}
impl Cache{
 pub fn new(path:PathBuf)->Self{Self{path,entries:Mutex::new(None)}}
 fn load(&self)->BTreeMap<String,Record>{if std::fs::metadata(&self.path).is_ok_and(|m|m.len()<=32*1024*1024){std::fs::read(&self.path).ok().and_then(|b|serde_json::from_slice(&b).ok()).unwrap_or_default()}else{BTreeMap::new()}}
 pub fn get<T:DeserializeOwned>(&self,key:&str)->Option<T>{let mut g=self.entries.lock().ok()?;let rows=g.get_or_insert_with(||self.load());let r=rows.get(key)?;if now().saturating_sub(r.written)>30*86400{return None}serde_json::from_value(r.value.clone()).ok()}
 pub fn put<T:Serialize>(&self,key:&str,value:&T){let Ok(value)=serde_json::to_value(value)else{return};let Ok(mut g)=self.entries.lock()else{return};let rows=g.get_or_insert_with(||self.load());rows.insert(key.into(),Record{written:now(),value});while rows.len()>512{if let Some(k)=rows.iter().min_by_key(|(_,v)|v.written).map(|(k,_)|k.clone()){rows.remove(&k);}}let Ok(data)=serde_json::to_vec(rows)else{return};if data.len()>32*1024*1024{return}let _=persist(&self.path,&data);}
}
pub fn persist(path:&std::path::Path,data:&[u8])->std::io::Result<()>{
 static SEQUENCE:std::sync::atomic::AtomicU64=std::sync::atomic::AtomicU64::new(0);
 if let Some(parent)=path.parent(){std::fs::create_dir_all(parent)?}let temp=path.with_extension(format!("{}.{}.tmp",std::process::id(),SEQUENCE.fetch_add(1,std::sync::atomic::Ordering::Relaxed)));
 let result=(||{let mut file=std::fs::OpenOptions::new().write(true).create_new(true).open(&temp)?;file.write_all(data)?;file.sync_all()?;drop(file);
 #[cfg(windows)]{use std::os::windows::ffi::OsStrExt;#[link(name="kernel32")]extern "system"{fn MoveFileExW(from:*const u16,to:*const u16,flags:u32)->i32;}let from=temp.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();let to=path.as_os_str().encode_wide().chain(Some(0)).collect::<Vec<_>>();if unsafe{MoveFileExW(from.as_ptr(),to.as_ptr(),9)}==0{return Err(std::io::Error::last_os_error())}}
 #[cfg(not(windows))]std::fs::rename(&temp,path)?;
 Ok(())})();if result.is_err(){let _=std::fs::remove_file(temp);}result
}
#[cfg(test)]mod tests{use super::*;#[test]fn disk_cache_survives_replace_and_ignores_invalid_json(){let p=std::env::temp_dir().join(format!("dotisle-v3-cache-{}.json",std::process::id()));let c=Cache::new(p.clone());c.put("a",&vec![13,24]);c.put("a",&vec![31,42]);assert_eq!(Cache::new(p.clone()).get::<Vec<i32>>("a"),Some(vec![31,42]));std::fs::write(&p,b"broken").unwrap();assert!(Cache::new(p.clone()).get::<Vec<i32>>("a").is_none());let _=std::fs::remove_file(p);}}
