//! Windows native HTTP. Music uses direct access; update requests use the system proxy.
use std::ffi::c_void;
use std::ptr::{null, null_mut};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};
use std::time::{Duration,Instant};

type Handle = *mut c_void;
#[link(name="winhttp")]
extern "system" {
    fn WinHttpOpen(agent:*const u16,access:u32,proxy:*const u16,bypass:*const u16,flags:u32)->Handle;
    fn WinHttpConnect(session:Handle,server:*const u16,port:u16,reserved:u32)->Handle;
    fn WinHttpOpenRequest(connection:Handle,verb:*const u16,path:*const u16,version:*const u16,refer:*const u16,accept:*const *const u16,flags:u32)->Handle;
    fn WinHttpSetTimeouts(handle:Handle,resolve:i32,connect:i32,send:i32,receive:i32)->i32;
    fn WinHttpSetOption(handle:Handle,option:u32,value:*const c_void,length:u32)->i32;
    fn WinHttpSendRequest(handle:Handle,headers:*const u16,header_len:u32,body:*mut c_void,body_len:u32,total:u32,context:usize)->i32;
    fn WinHttpReceiveResponse(handle:Handle,reserved:*mut c_void)->i32;
    fn WinHttpQueryHeaders(handle:Handle,level:u32,name:*const u16,buffer:*mut c_void,len:*mut u32,index:*mut u32)->i32;
    fn WinHttpReadData(handle:Handle,buffer:*mut c_void,len:u32,read:*mut u32)->i32;
    fn WinHttpCloseHandle(handle:Handle)->i32;
}
struct Owned(Handle);
impl Owned { fn new(h:Handle)->Result<Self,String> { if h.is_null(){Err(std::io::Error::last_os_error().to_string())}else{Ok(Self(h))} } }
impl Drop for Owned { fn drop(&mut self){unsafe{WinHttpCloseHandle(self.0);}} }
fn wide(s:&str)->Vec<u16>{s.encode_utf16().chain(Some(0)).collect()}
#[derive(Clone)]
pub struct RequestContext { pub cancelled:Arc<AtomicBool>, pub deadline:Instant }
impl RequestContext {
    pub fn new(timeout:Duration)->Self{Self{cancelled:Arc::new(AtomicBool::new(false)),deadline:Instant::now()+timeout}}
    pub fn check(&self)->Result<(),String>{if self.cancelled.load(Ordering::Acquire){Err("cancelled".into())}else if Instant::now()>self.deadline{Err("timeout".into())}else{Ok(())}}
}
pub fn encode(s:&str)->String { s.bytes().map(|b| if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {(b as char).to_string()}else{format!("%{b:02X}")}).collect() }
pub fn request(url:&str,method:&str,body:&[u8],content_type:&str,refer:&str,ctx:&RequestContext,limit:usize)->Result<Vec<u8>,String>{
    request_access(url,method,body,content_type,refer,ctx,limit,1,&[],false)
}
/// Update requests follow system/user proxy settings; music providers retain direct requests.
pub fn request_system(url:&str,ctx:&RequestContext,limit:usize)->Result<Vec<u8>,String>{request_access(url,"GET",&[],"application/json","",ctx,limit,4,&[],false)}
pub fn get_system(url:&str,ctx:&RequestContext)->Result<serde_json::Value,String>{serde_json::from_slice(&request_system(url,ctx,8*1024*1024)?).map_err(|e|e.to_string())}
fn request_access(url:&str,method:&str,body:&[u8],content_type:&str,refer:&str,ctx:&RequestContext,limit:usize,access:u32,extra:&[(&str,&str)],authenticated:bool)->Result<Vec<u8>,String>{
    ctx.check()?;
    let (scheme,rest)=url.split_once("://").ok_or("invalid URL")?;
    if scheme!="https" && scheme!="http"{return Err("unsupported URL".into())}
    let (authority,path)=rest.split_once('/').map(|(a,p)|(a,format!("/{p}"))).unwrap_or((rest,"/".into()));
    if authority.contains('@') || authority.is_empty(){return Err("invalid host".into())}
    let (host,port)=authority.split_once(':').map(|(h,p)|Ok::<_,String>((h,p.parse::<u16>().map_err(|_|"invalid port")?))).transpose()?.unwrap_or((authority,if scheme=="https"{443}else{80}));
    let agent=wide("Mozilla/5.0 (Windows NT 10.0; Win64; x64) LyricIsland/1.2");
    unsafe{
        let session=Owned::new(WinHttpOpen(agent.as_ptr(),access,null(),null(),0))?; // 1: direct; 4: system/user proxy
        let remaining=ctx.deadline.saturating_duration_since(Instant::now()).as_millis().clamp(1,if access==4{10_000}else{5000}) as i32;
        // System update sources can take several seconds to prepare a release list.
        // Use their request budget instead of the low-latency music timeout.
        if access==4{WinHttpSetTimeouts(session.0,remaining,remaining,remaining,remaining);}else{
            WinHttpSetTimeouts(session.0,remaining.min(2000),remaining.min(2000),remaining.min(2000),remaining.min(1500));
        }
        let decompression=3u32;WinHttpSetOption(session.0,118,&decompression as *const _ as _,4);
        let host=wide(host);let connection=Owned::new(WinHttpConnect(session.0,host.as_ptr(),port,0))?;
        let path=wide(&path);let method=wide(method);let refer_w=wide(refer);
        let request=Owned::new(WinHttpOpenRequest(connection.0,method.as_ptr(),path.as_ptr(),null(),if refer.is_empty(){null()}else{refer_w.as_ptr()},null(),if scheme=="https"{0x800000}else{0}))?;
        if authenticated {
            // Credentialed sources may never follow redirects, including same-site redirects.
            // Reject any failure to set the option rather than sending credentials unsafely.
            const WINHTTP_OPTION_REDIRECT_POLICY:u32=88;
            const REDIRECT_POLICY_NEVER:u32=0;
            if WinHttpSetOption(request.0,WINHTTP_OPTION_REDIRECT_POLICY,&REDIRECT_POLICY_NEVER as *const _ as _,4)==0 {
                return Err("failed to disable redirects".into());
            }
        }
        let has_header=|key:&str|extra.iter().any(|(name,_)|name.eq_ignore_ascii_case(key));
        let mut headers=String::new();
        if !has_header("Content-Type") {headers.push_str(&format!("Content-Type: {content_type}\r\n"));}
        if !has_header("Accept") {headers.push_str("Accept: */*\r\n");}
        for(name,value)in extra{if name.is_empty()||!name.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'-')||value.contains(['\r','\n']){return Err("invalid HTTP header".into())}headers.push_str(&format!("{name}: {value}\r\n"));}let headers=wide(&headers);
        if WinHttpSendRequest(request.0,headers.as_ptr(),u32::MAX,if body.is_empty(){null_mut()}else{body.as_ptr() as _},body.len() as u32,body.len() as u32,0)==0{return Err(std::io::Error::last_os_error().to_string())}
        ctx.check()?;
        if WinHttpReceiveResponse(request.0,null_mut())==0{return Err(std::io::Error::last_os_error().to_string())}
        let mut status=0u32;let mut len=4;
        if WinHttpQueryHeaders(request.0,19|0x20000000,null(),&mut status as *mut _ as _,&mut len,null_mut())==0 || !(200..300).contains(&status){return Err(format!("HTTP {status}"))}
        let mut output=Vec::new();let mut buffer=[0u8;65536];
        loop{
            ctx.check()?;let mut count=0;
            if WinHttpReadData(request.0,buffer.as_mut_ptr() as _,buffer.len() as u32,&mut count)==0{return Err(std::io::Error::last_os_error().to_string())}
            if count==0{break}if output.len()+count as usize>limit{return Err("response exceeds limit".into())}
            output.extend_from_slice(&buffer[..count as usize]);
        }
        Ok(output)
    }
}
pub fn get(url:&str,ctx:&RequestContext)->Result<serde_json::Value,String>{serde_json::from_slice(&request(url,"GET",&[],"application/json","",ctx,8*1024*1024)?).map_err(|e|e.to_string())}
pub fn post(url:&str,body:&str,content_type:&str,refer:&str,ctx:&RequestContext)->Result<serde_json::Value,String>{serde_json::from_slice(&request(url,"POST",body.as_bytes(),content_type,refer,ctx,8*1024*1024)?).map_err(|e|e.to_string())}
#[cfg(test)] mod tests{use super::*;#[test]fn url_encoding_is_utf8_and_never_injects_query(){assert_eq!(encode("a&中 "),"a%26%E4%B8%AD%20");}#[test]fn expired_and_cancelled_never_open_network(){let c=RequestContext::new(Duration::ZERO);assert!(request("https://invalid.test","GET",&[],"","",&c,10).is_err());}}


pub fn get_headers(url:&str,headers:&[(&str,&str)],ctx:&RequestContext)->Result<serde_json::Value,String>{serde_json::from_slice(&request_access(url,"GET",&[],"application/json","",ctx,8*1024*1024,1,headers,false)?).map_err(|e|e.to_string())}

/// JSON for a credentialed quota source. Callers must validate the endpoint against the
/// provider's fixed origin or the saved user-approved relay; WinHTTP must not redirect.
pub fn quota_get(url:&str,headers:&[(&str,&str)],ctx:&RequestContext)->Result<serde_json::Value,String>{
    serde_json::from_slice(&request_access(url,"GET",&[],"application/json","",ctx,1024*1024,1,headers,true)?).map_err(|_|"invalid JSON".into())
}

#[cfg(test)]
mod quota_transport_tests {
    use super::*;
    #[test]
    fn quota_credentials_do_not_follow_a_redirect() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local quota fixture");
        listener.set_nonblocking(true).unwrap();
        let url = format!("http://{}/quota", listener.local_addr().unwrap());
        let server = std::thread::spawn(move || {
            let start = Instant::now();
            let mut stream = loop {
                match listener.accept() {
                    Ok((stream, _)) => break stream,
                    Err(e) if e.kind()==std::io::ErrorKind::WouldBlock && start.elapsed()<Duration::from_secs(3) => std::thread::sleep(Duration::from_millis(4)),
                    other => panic!("quota fixture did not connect: {other:?}"),
                }
            };
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut auth = false;
            let mut reader = BufReader::new(&stream);
            for _ in 0..32 {
                let mut line=String::new();
                if reader.read_line(&mut line).unwrap()==0 || line=="\r\n" {break;}
                auth|=line.trim_end().eq_ignore_ascii_case("Authorization: Bearer fixture-only");
            }
            assert!(auth);
            stream.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/redirect-target\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
        });
        let res = quota_get(&url, &[("Authorization", "Bearer fixture-only")], &RequestContext::new(Duration::from_secs(3)));
        server.join().unwrap();
        assert_eq!(res.unwrap_err(), "HTTP 302");
    }
}

/// Read-only quota endpoints that use POST. Uses the same limits and no-redirect policy as quota_get.
pub fn quota_post(url:&str,body:&[u8],headers:&[(&str,&str)],ctx:&RequestContext)->Result<serde_json::Value,String>{
    serde_json::from_slice(&request_access(url,"POST",body,"application/json","",ctx,1024*1024,1,headers,true)?).map_err(|_|"invalid JSON".into())
}
