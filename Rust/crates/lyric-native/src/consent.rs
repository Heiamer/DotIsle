// The same document and record format are shared with the standalone center.
use std::{path::Path,time::{SystemTime,UNIX_EPOCH}};
use sha2::{Digest,Sha256};
#[path="consent_dialog.rs"]mod dialog;
use windows::{core::PCWSTR,Win32::{Foundation::CloseHandle,System::Threading::{CreateMutexW,WaitForSingleObject,ReleaseMutex,INFINITE},UI::WindowsAndMessaging::{MessageBoxW,MB_SETFOREGROUND,MB_OK,MB_ICONERROR}}};
pub const REVISION:&str="2026-11-17.1";
pub const TEXT:&str=include_str!("../../../../Go/settings-center/consent/disclaimer.zh-CN.txt");
fn digest()->String{format!("{:x}",Sha256::digest(TEXT.replace("\r\n","\n").as_bytes()))}
fn accepted(path:&Path)->bool{
 std::fs::read(path.join("consent.json")).ok().and_then(|b|serde_json::from_slice::<serde_json::Value>(&b).ok()).is_some_and(|r|r["accepted"]==true&&r["acceptedAtUnix"].as_u64().is_some_and(|t|t>0))
}
// A minimal state-only endpoint lets existing OTA helpers recognize the new
// executable while its mandatory consent dialog waits. It exposes no user data
// and accepts no settings, media, notification or agreement mutations.
unsafe extern "C" fn pending(request:*const std::ffi::c_char)->*mut std::ffi::c_char{
 use serde_json::json;
 let command=if request.is_null(){None}else{serde_json::from_slice::<serde_json::Value>(std::ffi::CStr::from_ptr(request).to_bytes()).ok()};
 let value=if command.as_ref().and_then(|v|v["command"].as_str())==Some("state"){json!({"success":true,"version":crate::DISPLAY_VERSION,"bundleVersion":crate::DISPLAY_VERSION,"processId":std::process::id(),"consentRequired":true,"moduleReady":false,"notificationState":{"enabled":false,"access":"consent-required"}})}else{json!({"success":false,"error":"请先阅读并同意使用免责声明"})};
 let text=std::ffi::CString::new(value.to_string()).unwrap();let bytes=text.as_bytes_with_nul();let out=windows::Win32::System::Com::CoTaskMemAlloc(bytes.len());if out.is_null(){return std::ptr::null_mut()}std::ptr::copy_nonoverlapping(bytes.as_ptr(),out.cast(),bytes.len());out.cast()
}
pub fn ensure(data:&Path)->Result<bool,Box<dyn std::error::Error>>{unsafe{
 let absolute=std::path::absolute(data)?;let name=format!("Local.DotIsle.Consent.{:x}",Sha256::digest(absolute.to_string_lossy().replace('/',"\\").to_lowercase().as_bytes()));let name=name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
 let mutex=CreateMutexW(None,false,PCWSTR(name.as_ptr()))?;
 struct Guard(windows::Win32::Foundation::HANDLE);impl Drop for Guard{fn drop(&mut self){unsafe{let _=ReleaseMutex(self.0);let _=CloseHandle(self.0);}}}let _guard=Guard(mutex);
 let wait=WaitForSingleObject(mutex,INFINITE);if wait.0!=0&&wait.0!=0x80{return Err("无法锁定本机协议记录".into())}
 if accepted(data){return Ok(true)}
 let pipe=std::env::var("LYRICISLAND_TEST_PIPE").unwrap_or_else(|_|"LyricIsland.Native.Settings.v1".into());let bootstrap=lyric_backend::ipc::Server::new(&pipe,pending);
 struct Bootstrap(lyric_backend::ipc::Server);impl Drop for Bootstrap{fn drop(&mut self){self.0.close();}}let _bootstrap=Bootstrap(bootstrap);
 let title="点屿 · 使用声明与免责条款".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
 if !dialog::show(TEXT){return Ok(false)}
 let result=(||->Result<(),Box<dyn std::error::Error>>{
  std::fs::create_dir_all(data)?;let path=data.join("consent.json");let tmp=data.join(format!(".consent-{}.tmp",std::process::id()));let value=serde_json::json!({"appVersion":crate::DISPLAY_VERSION,"revision":REVISION,"sha256":digest(),"accepted":true,"acceptedAtUnix":SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs()});
  use std::io::Write;let mut file=std::fs::File::create(&tmp)?;file.write_all(&serde_json::to_vec_pretty(&value)?)?;file.sync_all()?;drop(file);
  use windows::Win32::Storage::FileSystem::{MoveFileExW,MOVEFILE_REPLACE_EXISTING,MOVEFILE_WRITE_THROUGH};let from=tmp.to_string_lossy().encode_utf16().chain(Some(0)).collect::<Vec<_>>();let to=path.to_string_lossy().encode_utf16().chain(Some(0)).collect::<Vec<_>>();let moved=MoveFileExW(PCWSTR(from.as_ptr()),PCWSTR(to.as_ptr()),MOVEFILE_REPLACE_EXISTING|MOVEFILE_WRITE_THROUGH);let _=std::fs::remove_file(tmp);moved?;Ok(())
 })();
 if result.is_err(){let text="无法保存本机同意记录，软件不会启动。请检查数据目录是否可写后重试。".encode_utf16().chain(Some(0)).collect::<Vec<_>>();MessageBoxW(None,PCWSTR(text.as_ptr()),PCWSTR(title.as_ptr()),MB_OK|MB_ICONERROR|MB_SETFOREGROUND);result?;}Ok(true)
}}
#[cfg(test)]mod tests{use super::*;#[test]fn acceptance_survives_updates_but_requires_explicit_decision(){let path=std::env::temp_dir().join(format!("dotisle-consent-unit-{}",std::process::id()));std::fs::create_dir_all(&path).unwrap();assert!(!accepted(&path));let mut record=serde_json::json!({"appVersion":crate::DISPLAY_VERSION,"revision":REVISION,"sha256":digest(),"accepted":true,"acceptedAtUnix":1});std::fs::write(path.join("consent.json"),record.to_string()).unwrap();assert!(accepted(&path));record["sha256"]=serde_json::json!("old-document");std::fs::write(path.join("consent.json"),record.to_string()).unwrap();assert!(accepted(&path));record["appVersion"]=serde_json::json!("old-version");record["revision"]=serde_json::json!("old-revision");std::fs::write(path.join("consent.json"),record.to_string()).unwrap();assert!(accepted(&path));record["accepted"]=serde_json::json!(false);std::fs::write(path.join("consent.json"),record.to_string()).unwrap();assert!(!accepted(&path));record["accepted"]=serde_json::json!(true);record["acceptedAtUnix"]=serde_json::json!(0);std::fs::write(path.join("consent.json"),record.to_string()).unwrap();assert!(!accepted(&path));std::fs::remove_dir_all(path).unwrap();}}
