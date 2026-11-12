//! Automatically derived, read-only native timeline for x86 and x64 clients.
//! Client RTTI and instruction argument flow locate the live object and clocks.
//! No version list, hash gate, private member constants, injection or memory writes.
use std::{path::{Path,PathBuf},sync::Mutex,time::{Duration,Instant}};
use windows::Win32::{Foundation::{CloseHandle,HANDLE},System::{Diagnostics::{Debug::ReadProcessMemory,ToolHelp::*},Threading::{OpenProcess,GetExitCodeProcess,PROCESS_QUERY_LIMITED_INFORMATION,PROCESS_VM_READ}}};
use crate::{netease::TimingSample,netease_control::Target,media::unix_ms};
mod image;
use image::{Image,Layout,WRITE,EXEC};
type Result<T>=std::result::Result<T,String>;
fn bad()->String{"网易云原生进度结构尚未就绪，等待自动连接".into()}
fn bytes(b:&[u8],at:usize,n:usize)->Result<&[u8]>{b.get(at..at.checked_add(n).ok_or_else(bad)?).ok_or_else(bad)}
fn u16_at(b:&[u8],at:usize)->Result<u16>{Ok(u16::from_le_bytes(bytes(b,at,2)?.try_into().unwrap()))}
fn u32_at(b:&[u8],at:usize)->Result<u32>{Ok(u32::from_le_bytes(bytes(b,at,4)?.try_into().unwrap()))}
fn u64_at(b:&[u8],at:usize)->Result<u64>{Ok(u64::from_le_bytes(bytes(b,at,8)?.try_into().unwrap()))}
fn f64_at(b:&[u8],at:usize)->Result<f64>{Ok(f64::from_le_bytes(bytes(b,at,8)?.try_into().unwrap()))}
fn unique<T>(mut values:Vec<T>)->Result<T>{if values.len()!=1{return Err(bad())}Ok(values.remove(0))}
fn user_pointer(p:u64,n:usize)->bool{p>=0x10000&&p.checked_add(n as u64).is_some_and(|end|end<0x0000800000000000)}
struct Process(HANDLE);
// A process handle is an OS-owned reference, never a borrowed remote pointer.
unsafe impl Send for Process{}
impl Drop for Process{fn drop(&mut self){unsafe{let _=CloseHandle(self.0);}}}
impl Process{
 fn open(pid:u32)->Result<Self>{unsafe{OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION|PROCESS_VM_READ,false,pid).map(Self).map_err(|_|"无法只读连接网易云，请保持两款应用的运行权限一致".into())}}
 fn alive(&self)->bool{let mut code=0;unsafe{GetExitCodeProcess(self.0,&mut code).is_ok()&&code==259}}
 fn read(&self,address:u64,n:usize)->Result<Vec<u8>>{
  if n>16*1024*1024||!user_pointer(address,n)||!self.alive(){return Err(bad())}let mut data=vec![0;n];let mut actual=0;
  unsafe{ReadProcessMemory(self.0,address as*const _,data.as_mut_ptr()as*mut _,n,Some(&mut actual))}.map_err(|_|bad())?;if actual!=n{return Err(bad())}Ok(data)
 }
}
fn module(target:&Target)->Result<(u64,usize,PathBuf)>{unsafe{
 let snapshot=Process(CreateToolhelp32Snapshot(TH32CS_SNAPMODULE|TH32CS_SNAPMODULE32,target.pid).map_err(|_|bad())?);
 let mut e=MODULEENTRY32W{dwSize:std::mem::size_of::<MODULEENTRY32W>()as u32,..Default::default()};let mut found=Vec::new();
 if Module32FirstW(snapshot.0,&mut e).is_ok(){loop{let name=String::from_utf16_lossy(e.szModule.split(|c|*c==0).next().unwrap_or(&[]));if name.eq_ignore_ascii_case("cloudmusic.dll"){let path=PathBuf::from(String::from_utf16_lossy(e.szExePath.split(|c|*c==0).next().unwrap_or(&[])));if path.parent()!=target.executable.parent(){return Err(bad())}found.push((e.modBaseAddr as u64,e.modBaseSize as usize,path));}if Module32NextW(snapshot.0,&mut e).is_err(){break}}}unique(found)
}}

#[derive(PartialEq,Eq)]struct Identity{pointer:u64,id:String,field:Vec<u8>,header:Vec<u8>}
struct Reader{target:Target,process:Process,base:u64,image:Image,layout:Layout,object:Option<u64>,link:Option<(usize,usize)>,id_field:Option<usize>,last_song:Option<(String,Instant)>}
impl Reader{
 fn connect(target:&Target)->Result<Self>{
  let(base,size,path)=module(target)?;let file=std::fs::read(path).map_err(|_|bad())?;
  let image=Image::parse(file)?;if image.size!=size{return Err(bad())}let process=Process::open(target.pid)?;
  let header=process.read(base,4096)?;let pe=u32_at(&header,0x3c)?as usize;
  if u32_at(&header,pe+8)?!=image.timestamp||u32_at(&header,pe+24+56)?as usize!=image.size{return Err(bad())}
  let layout=image.discover().map_err(|_|bad())?;
  Ok(Self{target:target.clone(),process,base,image,layout,object:None,link:None,id_field:None,last_song:None})
 }
 fn locate(&self)->Result<u64>{
  let ptr=self.image.ptr();let needle=self.base+self.layout.audio as u64;let mut found=Vec::new();
  for s in &self.image.sections{if s.flags&WRITE==0||s.flags&EXEC!=0||s.virtual_size>16*1024*1024{continue}let data=self.process.read(self.base+s.rva as u64,s.virtual_size)?;
   for at in (0..data.len().saturating_sub(ptr-1)).step_by(ptr){if self.image.word(&data,at)?==needle{found.push(self.base+s.rva as u64+at as u64)}}
  }unique(found)
 }
 fn info_header(&self,p:u64)->Result<Vec<u8>>{
  let ptr=self.image.ptr();let data=self.process.read(p,self.layout.secondary_offset+ptr)?;
  if self.image.word(&data,0)?!=self.base+self.layout.info as u64||self.image.word(&data,self.layout.secondary_offset)?!=self.base+self.layout.secondary as u64{return Err(bad())}Ok(data)
 }
 fn identity(&mut self,object:&[u8])->Result<Identity>{
  if self.image.word(object,0)?!=self.base+self.layout.audio as u64{return Err(bad())}
  if self.link.is_none(){
   let mut candidates=Vec::new();let ptr=self.image.ptr();
   for at in (ptr..object.len().saturating_sub(ptr-1).min(512)).step_by(ptr){let value=self.image.word(object,at)?;
    for adjustment in [0,self.layout.secondary_offset]{let Some(p)=value.checked_sub(adjustment as u64)else{continue};if !user_pointer(p,ptr){continue}
     if self.info_header(p).is_ok(){candidates.push((p,at,adjustment))}
    }
   }
   candidates.sort_unstable();candidates.dedup_by_key(|c|c.0);let(_,at,adjustment)=unique(candidates)?;self.link=Some((at,adjustment));
  }
  let(at,adjustment)=self.link.ok_or_else(bad)?;let info=self.image.word(object,at)?.checked_sub(adjustment as u64).ok_or_else(bad)?;let header=self.info_header(info)?;
  let ptr=self.image.ptr();let string_size=16+2*ptr;
  if self.id_field.is_none(){
   let mut fields=Vec::new();
   for &offset in &self.layout.id_fields{let Ok(data)=self.process.read(info+offset as u64,string_size)else{continue};
    if let Ok(id)=play_id(&data,ptr,|p,n|self.process.read(p,n)){fields.push((offset,id));}
   }
   // The client interface's first string getter is the play identity. Other
   // metadata may also be all-numeric (for example a song title), so require
   // that this earliest derived field validates rather than guessing another.
   let first=*self.layout.id_fields.first().ok_or_else(bad)?;
   if !fields.iter().any(|(offset,_)|*offset==first){return Err(bad())}self.id_field=Some(first);
  }
  let field=self.process.read(info+self.id_field.ok_or_else(bad)?as u64,string_size)?;
  let id=play_id(&field,ptr,|p,n|self.process.read(p,n))?;
  Ok(Identity{pointer:info,id,field,header})
 }
 fn sample(&mut self)->Result<TimingSample>{
  if !self.process.alive(){return Err(bad())}let object=match self.object{Some(p)=>p,None=>{let p=self.locate()?;self.object=Some(p);p}};
  let size=(self.layout.state+4).max(self.layout.duration+8).max(512);let before=self.process.read(object,size)?;let identity=self.identity(&before)?;
  let duration=f64_at(&before,self.layout.duration)?;
  let position=f64_at(&self.process.read(self.base+self.layout.position as u64,8)?,0)?;
  let after=self.process.read(object,size)?;let after_identity=self.identity(&after)?;
  let duration_after=f64_at(&after,self.layout.duration)?;
  if identity!=after_identity||duration.to_bits()!=duration_after.to_bits()||u32_at(&before,self.layout.state)?!=u32_at(&after,self.layout.state)?{return Err(bad())}
  let song_id=identity.id;let playing=playback(u32_at(&after,self.layout.state)?,position,duration)?;
  if self.last_song.as_ref().is_none_or(|(id,_)|id!=&song_id){self.last_song=Some((song_id,Instant::now()));return Err(bad())}
  if self.last_song.as_ref().is_some_and(|(_,since)|since.elapsed()<Duration::from_millis(160)){return Err(bad())}
  Ok(TimingSample{song_id,title:String::new(),artist:String::new(),album:String::new(),position,duration,playing,control_ready:true,captured_ms:unix_ms(),cover_url:None})
 }
}
fn playback(state:u32,position:f64,duration:f64)->Result<bool>{if !position.is_finite()||!duration.is_finite()||!(0.0..=86400.).contains(&duration)||duration==0.||position<0.||position>duration{return Err(bad())}match state{1=>Ok(true),2=>Ok(false),_=>Err(bad())}}
fn play_id(data:&[u8],ptr:usize,read:impl Fn(u64,usize)->Result<Vec<u8>>)->Result<String>{
 let word=|at|if ptr==8{u64_at(data,at)}else{u32_at(data,at).map(u64::from)};
 let len=usize::try_from(word(16)?).map_err(|_|bad())?;let capacity=word(16+ptr)?;
 if len==0||len>96||capacity<len as u64||capacity>4096{return Err(bad())}
 let content=if capacity<16{if len>15{return Err(bad())}bytes(data,0,len)?.to_vec()}else{let p=word(0)?;if !user_pointer(p,len){return Err(bad())}read(p,len)?};
 if content.len()!=len{return Err(bad())}let text=std::str::from_utf8(&content).map_err(|_|bad())?;let id=text.split('_').next().unwrap_or("");
 if id.is_empty()||id.len()>32||!id.bytes().all(|b|b.is_ascii_digit())||!text.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'){return Err(bad())}Ok(id.into())
}
static READER:Mutex<Option<Reader>>=Mutex::new(None);
pub fn read(target:&Target)->Result<TimingSample>{
 let mut cache=READER.lock().map_err(|_|bad())?;
 if cache.as_ref().is_none_or(|r|r.target!=*target||!r.process.alive()){*cache=Some(Reader::connect(target)?)}
 let reader=cache.as_mut().ok_or_else(bad)?;let result=reader.sample();if result.is_err(){reader.object=None}result
}
pub fn inspect(path:&Path)->Result<serde_json::Value>{let image=Image::parse(std::fs::read(path).map_err(|e|e.to_string())?)?;let layout=image.discover()?;Ok(serde_json::json!({"bits":image.bits,"size":image.size,"layout":layout,"versionGate":false,"evidence":"static-structure-only"}))}
pub fn diagnostics()->serde_json::Value{match READER.lock(){Ok(cache)=>cache.as_ref().map(|r|serde_json::json!({"bits":r.image.bits,"layout":r.layout,"versionGate":false})).unwrap_or_default(),Err(_)=>serde_json::Value::Null}}

#[cfg(test)]mod tests{use super::*;
 #[test]fn private_state_numbers_and_invalid_positions_are_rejected(){assert_eq!(playback(1,12.,180.),Ok(true));assert_eq!(playback(2,12.,180.),Ok(false));for state in [0,3,4,u32::MAX]{assert!(playback(state,12.,180.).is_err())}for(p,d)in[(f64::NAN,180.),(0.,f64::INFINITY),(-1.,180.),(181.,180.),(0.,0.),(0.,86401.)]{assert!(playback(1,p,d).is_err())}}
 #[test]fn play_identity_handles_both_msvc_pointer_widths(){for ptr in [4,8]{let mut b=vec![0u8;16+2*ptr];b[..12].copy_from_slice(b"12345_ABC123");b[16]=12;b[16+ptr]=15;assert_eq!(play_id(&b,ptr,|_,_|panic!()).unwrap(),"12345");b[0]=b'x';assert!(play_id(&b,ptr,|_,_|panic!()).is_err());b[..ptr].copy_from_slice(&0x123400u64.to_le_bytes()[..ptr]);b[16]=21;b[16+ptr]=31;assert_eq!(play_id(&b,ptr,|p,n|{assert_eq!((p,n),(0x123400,21));Ok(b"2680959706_ABCDEFGHIJ".to_vec())}).unwrap(),"2680959706");b[16+ptr..18+ptr].copy_from_slice(&4097u16.to_le_bytes());assert!(play_id(&b,ptr,|_,_|panic!()).is_err());}}
 #[test]fn pointers_and_ambiguous_objects_are_checked(){assert!(!user_pointer(0,8));assert!(!user_pointer(u64::MAX,8));assert!(!user_pointer(0x7ffffffffffe,8));assert!(unique::<usize>(vec![]).is_err());assert!(unique(vec![1,2]).is_err());assert!(bytes(&[0;8],usize::MAX,2).is_err());}
}
