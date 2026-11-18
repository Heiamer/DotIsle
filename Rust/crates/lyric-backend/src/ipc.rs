//! Pipe transport stays native; the callback only dispatches GUI operations in WPF.
use std::{sync::{Arc,atomic::{AtomicBool,Ordering}},time::{Instant,Duration},ffi::{c_char,CStr}};
use windows::{core::PCWSTR,Win32::{Foundation::{HANDLE,CloseHandle,GetLastError,ERROR_PIPE_CONNECTED},System::{Pipes::*,Com::CoTaskMemFree},Storage::FileSystem::{FILE_FLAG_FIRST_PIPE_INSTANCE,PIPE_ACCESS_DUPLEX,FlushFileBuffers,ReadFile,WriteFile}}};
pub type GuiCallback=unsafe extern "C" fn(*const c_char)->*mut c_char;
pub struct Server {closed:Arc<AtomicBool>,finished:Arc<AtomicBool>}
impl Server{
    pub fn new(name:&str,callback:GuiCallback)->Self{
        let name=format!("\\\\.\\pipe\\{name}").encode_utf16().chain(Some(0)).collect::<Vec<_>>();let closed=Arc::new(AtomicBool::new(false));let worker_closed=closed.clone();
        let finished=Arc::new(AtomicBool::new(false));let done=finished.clone();
        std::thread::spawn(move||unsafe{
            // The instance is created once and reused for every request. Recreating it after each
            // reply exposed a window where a client could connect to an instance that was about to
            // be closed: CreateFile succeeded, then CloseHandle cut the connection underneath the
            // client with ERROR_PIPE_NOT_CONNECTED (0x800700E9), which made the settings process
            // believe the island was gone and spawn a second one (3.5.2 withdrawal root cause).
            let mut instance:Option<HANDLE>=None;
            while !worker_closed.load(Ordering::Acquire){
                let pipe=match instance{
                    Some(handle)=>handle,
                    None=>{
                        // Local-only pipe, remote clients rejected by the pipe type. The request is buffered
                        // first under a 16 KiB cap, then the client token SID is checked with
                        // ImpersonateNamedPipeClient before the callback runs: nothing is acted on and no
                        // data is returned until that check passes.
                        let created=CreateNamedPipeW(PCWSTR(name.as_ptr()),PIPE_ACCESS_DUPLEX|FILE_FLAG_FIRST_PIPE_INSTANCE,PIPE_TYPE_BYTE|PIPE_READMODE_BYTE|PIPE_NOWAIT|PIPE_REJECT_REMOTE_CLIENTS,1,65536,16384,1000,None);
                        if created.is_invalid(){std::thread::sleep(Duration::from_millis(150));continue}
                        instance=Some(created);created
                    }
                };
                let connection=(||{
                    loop{
                        if worker_closed.load(Ordering::Acquire){return None}
                        if ConnectNamedPipe(pipe,None).is_ok(){break}
                        if GetLastError()==ERROR_PIPE_CONNECTED{break}
                        std::thread::sleep(Duration::from_millis(20));
                    }
                    let deadline=Instant::now()+Duration::from_secs(8);let mut input=Vec::new();
                    loop{
                        if worker_closed.load(Ordering::Acquire){return None}
                        if Instant::now()>deadline{return dropped("request timeout")}
                        let mut available=0;if PeekNamedPipe(pipe,None,0,None,Some(&mut available),None).is_err(){return dropped("peek failed")}
                        if available==0{std::thread::sleep(Duration::from_millis(10));continue}
                        let mut bytes=vec![0;available.min(16384) as usize];let mut read=0;
                        if ReadFile(pipe,Some(&mut bytes),Some(&mut read),None).is_err(){return dropped("read failed")}
                        input.extend_from_slice(&bytes[..read as usize]);
                        if input.len()>16384{return dropped("request too large")}if let Some(end)=input.iter().position(|b|*b==b'\n'){input.truncate(end);break}
                    }
                    if !same_user(pipe){return dropped("client identity rejected")}
                    let Some(request)=std::ffi::CString::new(input).ok()else{return dropped("request is not a C string")};
                    let result=callback(request.as_ptr());if result.is_null(){return dropped("GUI returned no response")}
                    let mut response=CStr::from_ptr(result).to_bytes().to_vec();CoTaskMemFree(Some(result as _));response.push(b'\n');
                    if response.len()>1024*1024{return dropped("response too large")}
                    let deadline = Instant::now() + Duration::from_secs(4);
                    let mut sent = 0;
                    while sent < response.len() {
                        if worker_closed.load(Ordering::Acquire) { return None; }
                        if Instant::now() > deadline { return dropped("response timeout"); }
                        let mut written = 0;
                        let end = (sent + 32 * 1024).min(response.len());
                        if WriteFile(pipe, Some(&response[sent..end]), Some(&mut written), None).is_err() {
                            return dropped("write failed");
                        }
                        sent += written as usize;
                        if written == 0 { std::thread::sleep(Duration::from_millis(2)); }
                    }
                    // DisconnectNamedPipe discards whatever the client has not consumed yet, so the
                    // response has to be drained first: FlushFileBuffers on a server handle returns
                    // once the client has read everything. Without this the client races the
                    // disconnect and fails with ERROR_PIPE_NOT_CONNECTED (0x800700E9).
                    // Best effort: a failure here only means the client already closed its end
                    // (nothing left to wait for), so it is not reported as a dropped request.
                    let _=FlushFileBuffers(pipe);
                    Some(())
                })();
                let _=connection;
                // Disconnect resets the instance for the next client without destroying it, so the
                // pipe name never points at a doomed instance between requests. Only a failed
                // disconnect (instance unusable) forces a rebuild.
                if DisconnectNamedPipe(pipe).is_err(){let _=CloseHandle(pipe);instance=None;std::thread::sleep(Duration::from_millis(20));}
            }
            if let Some(pipe)=instance{let _=CloseHandle(pipe);}
            done.store(true,Ordering::Release);
        });Self{closed,finished}
    }
    pub fn close(&self){self.closed.store(true,Ordering::Release)}
    pub fn finished(&self)->bool{self.finished.load(Ordering::Acquire)}
}
impl Drop for Server{fn drop(&mut self){self.close()}}
/// Record why a connection was abandoned, then report it as no response.
fn dropped(reason:&str)->Option<()>{
    crate::io::log(&crate::io::data_directory().join("Logs"),"IPC_DROP",&format!("settings pipe connection dropped: {reason}"));
    None
}
unsafe fn same_user(pipe:HANDLE)->bool{
    use windows::Win32::{System::Threading::{GetCurrentProcess,GetCurrentThread,OpenProcessToken,OpenThreadToken},Security::{GetTokenInformation,TokenUser,TOKEN_QUERY,TOKEN_USER,EqualSid}};
    fn token_user(token:HANDLE)->Option<Vec<u8>>{unsafe{let mut length=0;let _=GetTokenInformation(token,TokenUser,None,0,&mut length);if length==0||length>65536{return None}let mut buffer=vec![0u8;length as usize];GetTokenInformation(token,TokenUser,Some(buffer.as_mut_ptr() as _),length,&mut length).ok()?;Some(buffer)}}
    let mut own=HANDLE::default();if OpenProcessToken(GetCurrentProcess(),TOKEN_QUERY,&mut own).is_err(){return false}let own_data=token_user(own);let _=CloseHandle(own);
    if ImpersonateNamedPipeClient(pipe).is_err(){return false}let mut client=HANDLE::default();let opened=OpenThreadToken(GetCurrentThread(),TOKEN_QUERY,true,&mut client).is_ok();
    let data=if opened{let d=token_user(client);let _=CloseHandle(client);d}else{None};let _=windows::Win32::Security::RevertToSelf();
    match (own_data,data){(Some(a),Some(b))=>{let a=&*(a.as_ptr() as *const TOKEN_USER);let b=&*(b.as_ptr() as *const TOKEN_USER);EqualSid(a.User.Sid,b.User.Sid).is_ok()},_=>false}
}
#[cfg(test)]mod ipc_tests{
    use super::*;
    use serde_json::json;
    // The server frees replies with CoTaskMemFree, so the test callback allocates there too.
    unsafe extern "C" fn echo(request:*const c_char)->*mut c_char{
        let command=std::ffi::CStr::from_ptr(request).to_bytes().iter().filter(|b|**b!=b'\n').cloned().collect::<Vec<_>>();
        let command=serde_json::from_slice::<serde_json::Value>(&command).ok().and_then(|v|v["command"].as_str().map(str::to_owned)).unwrap_or_default();
        let reply=json!({"command":command,"image":if command=="large" {"x".repeat(260_000)}else{String::new()}}).to_string();
        let len=reply.len()+1;
        let Ok(c)=std::ffi::CString::new(reply)else{return std::ptr::null_mut()};
        let buffer=windows::Win32::System::Com::CoTaskMemAlloc(len);
        if buffer.is_null(){return std::ptr::null_mut()}
        std::ptr::copy_nonoverlapping(c.as_ptr() as *const u8,buffer as *mut u8,len);
        buffer as *mut c_char
    }
    #[test]
    fn a_reply_larger_than_the_pipe_buffer_is_delivered_in_full() {
        let pipe = format!("LyricIsland.Native.Settings.test.ipc-large-{}", std::process::id());
        let server = Server::new(&pipe, echo);
        let reply = crate::io::pipe_request(&pipe, &json!({"version":1,"command":"large"})).unwrap();
        assert_eq!(reply["image"].as_str().unwrap().len(), 260_000);
        server.close();
    }
    // Regression for the 3.5.2 withdrawal: the server used to close and recreate its pipe
    // instance after every reply, so a concurrent client could connect to a doomed instance
    // and fail with ERROR_PIPE_NOT_CONNECTED (0x800700E9).
    #[test]fn concurrent_state_requests_never_hit_a_doomed_instance(){
        let pipe=format!("LyricIsland.Native.Settings.test.ipc-{}",std::process::id());
        let server=Server::new(&pipe,echo);
        let workers=(0..4).map(|_|{
            let pipe=pipe.clone();
            std::thread::spawn(move||{
                for _ in 0..50{
                    let reply=crate::io::pipe_request(&pipe,&json!({"version":1,"command":"state"})).expect("state request must succeed");
                    assert_eq!(reply["command"].as_str(),Some("state"));
                }
            })
        }).collect::<Vec<_>>();
        for worker in workers{worker.join().expect("worker must not panic");}
        server.close();
    }
}

