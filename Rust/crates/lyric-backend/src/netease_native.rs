//! Independently derived, read-only native timeline for an ordinary player launch.
//! RTTI identifies the audio object; its progress callback identifies the clock.
//! A verified image profile gates private field/instruction layouts. No injection,
//! remote execution, memory writes, debug switch, or third-party scan patterns.
use std::{path::PathBuf,sync::Mutex,time::{Duration,Instant}};
use sha2::{Digest,Sha256};
use windows::Win32::{Foundation::{CloseHandle,HANDLE},System::{Diagnostics::{Debug::ReadProcessMemory,ToolHelp::*},Threading::{OpenProcess,GetExitCodeProcess,PROCESS_QUERY_LIMITED_INFORMATION,PROCESS_VM_READ}}};
use crate::{netease::TimingSample,netease_control::Target,media::unix_ms};

const PROFILE_SHA:&str="48f4b79fc0eb7fbcb955835463a71cd866c81055b0a546990e8ab644e886b920";
const AUDIO:&[u8]=b".?AVAudioPlayer@audio@@\0";
const INFO:&[u8]=b".?AVAudioPlayInfo@player@@\0";
const READ:u32=0x40000000;const WRITE:u32=0x80000000;const EXEC:u32=0x20000000;
type Result<T>=std::result::Result<T,String>;
fn bad()->String{"网易云原生进度结构尚未就绪，等待自动连接".into()}
fn bytes(b:&[u8],at:usize,n:usize)->Result<&[u8]>{b.get(at..at.checked_add(n).ok_or_else(bad)?).ok_or_else(bad)}
fn u16_at(b:&[u8],at:usize)->Result<u16>{Ok(u16::from_le_bytes(bytes(b,at,2)?.try_into().unwrap()))}
fn u32_at(b:&[u8],at:usize)->Result<u32>{Ok(u32::from_le_bytes(bytes(b,at,4)?.try_into().unwrap()))}
fn u64_at(b:&[u8],at:usize)->Result<u64>{Ok(u64::from_le_bytes(bytes(b,at,8)?.try_into().unwrap()))}
fn f64_at(b:&[u8],at:usize)->Result<f64>{Ok(f64::from_le_bytes(bytes(b,at,8)?.try_into().unwrap()))}
fn unique<T>(mut values:Vec<T>)->Result<T>{if values.len()!=1{return Err(bad())}Ok(values.remove(0))}
fn user_pointer(p:u64,n:usize)->bool{p>=0x10000&&p.checked_add(n as u64).is_some_and(|end|end<0x0000800000000000)}

#[derive(Clone)]struct Section{rva:usize,virtual_size:usize,raw:usize,raw_size:usize,flags:u32}
impl Section{fn contains(&self,rva:usize,n:usize)->bool{rva>=self.rva&&rva.checked_add(n).is_some_and(|end|end<=self.rva+self.virtual_size)}}
struct Image{file:Vec<u8>,base:u64,size:usize,timestamp:u32,sections:Vec<Section>,functions:Vec<(usize,usize)>}
impl Image{
 fn parse(file:Vec<u8>)->Result<Self>{
  if bytes(&file,0,2)?!=b"MZ"{return Err(bad())}let pe=u32_at(&file,0x3c)? as usize;
  if bytes(&file,pe,4)?!=b"PE\0\0"||u16_at(&file,pe+4)?!=0x8664||u16_at(&file,pe+24)?!=0x20b{return Err(bad())}
  let count=u16_at(&file,pe+6)? as usize;let optional=u16_at(&file,pe+20)? as usize;
  if count==0||count>96||optional<240{return Err(bad())}
  let size=u32_at(&file,pe+24+56)? as usize;if !(4096..=128*1024*1024).contains(&size){return Err(bad())}
  let mut sections=Vec::new();
  for i in 0..count{let at=pe+24+optional+i*40;let s=Section{virtual_size:u32_at(&file,at+8)?as usize,rva:u32_at(&file,at+12)?as usize,raw_size:u32_at(&file,at+16)?as usize,raw:u32_at(&file,at+20)?as usize,flags:u32_at(&file,at+36)?};
   if s.rva.checked_add(s.virtual_size).is_none_or(|end|end>size){return Err(bad())}bytes(&file,s.raw,s.raw_size)?;
   if sections.iter().any(|o:&Section|s.rva<o.rva+o.virtual_size&&o.rva<s.rva+s.virtual_size){return Err(bad())}sections.push(s);
  }
  let mut image=Self{base:u64_at(&file,pe+24+24)?,timestamp:u32_at(&file,pe+8)?,size,file,sections,functions:Vec::new()};
  let table=u32_at(&image.file,pe+24+112+3*8)? as usize;let table_size=u32_at(&image.file,pe+24+116+3*8)? as usize;
  if table_size==0||table_size%12!=0||table_size>8*1024*1024{return Err(bad())}
  let data=image.at(table,table_size)?;let mut functions=Vec::new();
  for at in (0..data.len()).step_by(12){let start=u32_at(data,at)?as usize;let end=u32_at(data,at+4)?as usize;if start>=end||!image.section(start,end-start).is_some_and(|s|s.flags&EXEC!=0){return Err(bad())}functions.push((start,end));}image.functions=functions;
  Ok(image)
 }
 fn section(&self,rva:usize,n:usize)->Option<&Section>{self.sections.iter().find(|s|s.contains(rva,n))}
 fn at(&self,rva:usize,n:usize)->Result<&[u8]>{let s=self.section(rva,n).ok_or_else(bad)?;let delta=rva-s.rva;if delta.checked_add(n).is_none_or(|end|end>s.raw_size){return Err(bad())}bytes(&self.file,s.raw+delta,n)}
 fn rva(&self,p:u64)->Result<usize>{let r=usize::try_from(p.checked_sub(self.base).ok_or_else(bad)?).map_err(|_|bad())?;self.section(r,1).map(|_|r).ok_or_else(bad)}
 fn function(&self,rva:usize)->Result<&[u8]>{let(_,end)=unique(self.functions.iter().copied().filter(|(start,_)|*start==rva).collect())?;if end-rva>4096{return Err(bad())}self.at(rva,end-rva)}
 fn vtable(&self,name:&[u8],object_offset:u32)->Result<usize>{
  let mut descriptors=Vec::new();
  for s in &self.sections{if s.flags&READ==0{continue}for(i,w)in self.file[s.raw..s.raw+s.raw_size].windows(name.len()).enumerate(){if w==name&&i>=16{descriptors.push(s.rva+i-16)}}}
  let td=unique(descriptors)?;let mut locators=Vec::new();
  for s in &self.sections{if s.flags&READ==0||s.flags&(WRITE|EXEC)!=0{continue}let data=&self.file[s.raw..s.raw+s.raw_size];
   for at in (0..data.len().saturating_sub(23)).step_by(4){if u32_at(data,at)?==1&&u32_at(data,at+4)?==object_offset&&u32_at(data,at+8)?==0&&u32_at(data,at+12)?as usize==td&&u32_at(data,at+20)?as usize==s.rva+at{locators.push(s.rva+at)}}
  }
  let col=unique(locators)?;let needle=(self.base+col as u64).to_le_bytes();let mut tables=Vec::new();
  for s in &self.sections{if s.flags&READ==0||s.flags&(WRITE|EXEC)!=0{continue}let data=&self.file[s.raw..s.raw+s.raw_size];for at in (0..data.len().saturating_sub(7)).step_by(8){if data[at..at+8]==needle{tables.push(s.rva+at+8)}}}
  unique(tables)
 }
 fn progress(&self,info_vtable:usize)->Result<usize>{
  let callback=self.rva(u64_at(self.at(info_vtable+0x28,8)?,0)?)?;
  // In this verified build the callback forwards XMM1/XMM2 to a UI-thread
  // notifier. Resolve CALL and RIP-relative operands with instruction checks
  // for this image profile, without retaining an absolute clock address.
  let callback_body=self.function(callback)?;let call=bytes(callback_body,0x32,5)?;
  if call[0]!=0xe8{return Err(bad())}let notifier=relative(callback+0x32,5,&call[1..])?;
  let body=self.function(notifier)?;
  if bytes(body,0x22,3)?!=[0x0f,0x28,0xf2]||bytes(body,0x25,3)?!=[0x0f,0x28,0xf9]{return Err(bad())}
  let compare=bytes(body,0x2b,8)?;let store=bytes(body,0x47,8)?;
  if compare[..4]!=[0x66,0x0f,0x2e,0x0d]||store[..4]!=[0xf2,0x0f,0x11,0x3d]{return Err(bad())}
  let position=relative(notifier+0x47,8,&store[4..])?;
  if relative(notifier+0x2b,8,&compare[4..])?!=position||!self.section(position,8).is_some_and(|s|s.flags&WRITE!=0&&s.flags&EXEC==0){return Err(bad())}
  Ok(position)
 }
}
fn relative(rva:usize,len:usize,disp:&[u8])->Result<usize>{let d=i32::from_le_bytes(disp.try_into().map_err(|_|bad())?)as i64;usize::try_from((rva as i64).checked_add(len as i64).and_then(|v|v.checked_add(d)).ok_or_else(bad)?).map_err(|_|bad())}

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
struct Reader{target:Target,process:Process,base:u64,image:Image,audio_vtable:usize,info_vtable:usize,position:usize,object:Option<u64>,last_song:Option<(String,Instant)>}
impl Reader{
 fn connect(target:&Target)->Result<Self>{
  let(base,size,path)=module(target)?;let file=std::fs::read(path).map_err(|_|bad())?;
  if format!("{:x}",Sha256::digest(&file))!=PROFILE_SHA{return Err("当前网易云版本暂用系统媒体进度，原生进度适配将随点屿更新".into())}
  let image=Image::parse(file)?;if image.size!=size{return Err(bad())}let process=Process::open(target.pid)?;
  let header=process.read(base,4096)?;let pe=u32_at(&header,0x3c)?as usize;
  if u32_at(&header,pe+8)?!=image.timestamp||u32_at(&header,pe+24+56)?as usize!=image.size{return Err(bad())}
  let audio_vtable=image.vtable(AUDIO,0)?;let info_vtable=image.vtable(INFO,8)?;let position=image.progress(info_vtable)?;
  Ok(Self{target:target.clone(),process,base,image,audio_vtable,info_vtable,position,object:None,last_song:None})
 }
 fn locate(&self)->Result<u64>{
  let needle=(self.base+self.audio_vtable as u64).to_le_bytes();let mut found=Vec::new();
  for s in &self.image.sections{if s.flags&WRITE==0||s.flags&EXEC!=0||s.virtual_size>16*1024*1024{continue}let data=self.process.read(self.base+s.rva as u64,s.virtual_size)?;
   for at in (0..data.len().saturating_sub(7)).step_by(8){if data[at..at+8]==needle{found.push(self.base+s.rva as u64+at as u64)}}
  }unique(found)
 }
 fn identity(&self,object:&[u8])->Result<(u64,Vec<u8>,String)>{
  if u64_at(object,0)?!=self.base+self.audio_vtable as u64{return Err(bad())}
  let info=u64_at(object,0x50)?;if !user_pointer(info,0x30)||u64_at(object,0x58)?!=info+8{return Err(bad())}
  let data=self.process.read(info,0x30)?;
  if u64_at(&data,8)?!=self.base+self.info_vtable as u64{return Err(bad())}
  let id=play_id(&data[0x10..],|p,n|self.process.read(p,n))?;Ok((info,data,id))
 }
 fn sample(&mut self)->Result<TimingSample>{
  if !self.process.alive(){return Err(bad())}let object=match self.object{Some(p)=>p,None=>{let p=self.locate()?;self.object=Some(p);p}};
  let before=self.process.read(object,0xb0)?;let(info,info_before,song_id)=self.identity(&before)?;
  let position=f64_at(&self.process.read(self.base+self.position as u64,8)?,0)?;
  let after=self.process.read(object,0xb0)?;let(info_after,info_bytes,after_id)=self.identity(&after)?;
  if !coherent(&before,&after,info==info_after&&info_before==info_bytes&&song_id==after_id){return Err(bad())}
  let duration=f64_at(&after,0xa8)?;let state=u32_at(&after,0x60)?;let playing=playback(state,position,duration)?;
  if self.last_song.as_ref().is_none_or(|(id,_)|id!=&song_id){self.last_song=Some((song_id,Instant::now()));return Err(bad())}
  if self.last_song.as_ref().is_some_and(|(_,since)|since.elapsed()<Duration::from_millis(160)){return Err(bad())}
  Ok(TimingSample{song_id,title:String::new(),artist:String::new(),album:String::new(),position,duration,playing,control_ready:true,captured_ms:unix_ms(),cover_url:None})
 }
}
fn coherent(before:&[u8],after:&[u8],identity_same:bool)->bool{identity_same&&before.get(0..8)==after.get(0..8)&&before.get(0x50..0x64)==after.get(0x50..0x64)&&before.get(0xa8..0xb0)==after.get(0xa8..0xb0)}
fn playback(state:u32,position:f64,duration:f64)->Result<bool>{if !position.is_finite()||!duration.is_finite()||!(0.0..=86400.).contains(&duration)||duration==0.||position<0.||position>duration{return Err(bad())}match state{1=>Ok(true),2=>Ok(false),_=>Err(bad())}}
fn play_id(data:&[u8],read:impl Fn(u64,usize)->Result<Vec<u8>>)->Result<String>{
 let len=usize::try_from(u64_at(data,16)?).map_err(|_|bad())?;let capacity=u64_at(data,24)?;
 if len==0||len>96||capacity<len as u64||capacity>4096{return Err(bad())}
 let content=if capacity<16{if len>15{return Err(bad())}bytes(data,0,len)?.to_vec()}else{let p=u64_at(data,0)?;if !user_pointer(p,len){return Err(bad())}read(p,len)?};
 if content.len()!=len{return Err(bad())}let text=std::str::from_utf8(&content).map_err(|_|bad())?;let id=text.split('_').next().unwrap_or("");
 if id.is_empty()||id.len()>32||!id.bytes().all(|b|b.is_ascii_digit())||!text.bytes().all(|b|b.is_ascii_alphanumeric()||b==b'_'){return Err(bad())}Ok(id.into())
}
static READER:Mutex<Option<Reader>>=Mutex::new(None);
pub fn read(target:&Target)->Result<TimingSample>{
 let mut cache=READER.lock().map_err(|_|bad())?;
 if cache.as_ref().is_none_or(|r|r.target!=*target||!r.process.alive()){*cache=Some(Reader::connect(target)?)}
 let reader=cache.as_mut().ok_or_else(bad)?;let result=reader.sample();if result.is_err(){reader.object=None}result
}

#[cfg(test)]mod tests{use super::*;
 #[test]fn private_state_numbers_and_invalid_positions_are_rejected(){assert_eq!(playback(1,12.,180.),Ok(true));assert_eq!(playback(2,12.,180.),Ok(false));for state in [0,3,4,u32::MAX]{assert!(playback(state,12.,180.).is_err())}for(p,d)in[(f64::NAN,180.),(0.,f64::INFINITY),(-1.,180.),(181.,180.),(0.,0.),(0.,86401.)]{assert!(playback(1,p,d).is_err())}}
 #[test]fn play_identity_handles_inline_and_heap_strings_with_bounds(){let mut b=[0u8;32];b[..12].copy_from_slice(b"12345_ABC123");b[16..24].copy_from_slice(&12u64.to_le_bytes());b[24..32].copy_from_slice(&15u64.to_le_bytes());assert_eq!(play_id(&b,|_,_|panic!()).unwrap(),"12345");b[0]=b'x';assert!(play_id(&b,|_,_|panic!()).is_err());b[..8].copy_from_slice(&0x123400u64.to_le_bytes());b[16..24].copy_from_slice(&21u64.to_le_bytes());b[24..32].copy_from_slice(&31u64.to_le_bytes());assert_eq!(play_id(&b,|p,n|{assert_eq!((p,n),(0x123400,21));Ok(b"2680959706_ABCDEFGHIJ".to_vec())}).unwrap(),"2680959706");b[24..32].copy_from_slice(&4097u64.to_le_bytes());assert!(play_id(&b,|_,_|panic!()).is_err())}
 #[test]fn torn_song_state_and_duration_never_make_a_sample(){let a=vec![0;0xb0];let mut b=a.clone();assert!(coherent(&a,&b,true));assert!(!coherent(&a,&b,false));for at in [0,0x50,0x58,0x60,0xa8]{b[at]=1;assert!(!coherent(&a,&b,true));b[at]=0}}
 #[test]fn pointers_relative_targets_and_ambiguous_objects_are_checked(){assert!(!user_pointer(0,8));assert!(!user_pointer(u64::MAX,8));assert!(!user_pointer(0x7ffffffffffe,8));assert!(unique::<usize>(vec![]).is_err());assert!(unique(vec![1,2]).is_err());assert_eq!(relative(0x100,5,&(-20i32).to_le_bytes()).unwrap(),0xf1);assert!(relative(0,5,&(-20i32).to_le_bytes()).is_err());assert!(bytes(&[0;8],usize::MAX,2).is_err());assert!(Image::parse(vec![0;4096]).is_err())}
 fn fixture()->Vec<u8>{
  let mut b=vec![0u8;0xf00];
  fn set(b:&mut[u8],p:usize,v:&[u8]){b[p..p+v.len()].copy_from_slice(v)}
  set(&mut b,0,b"MZ");set(&mut b,0x3c,&0x80u32.to_le_bytes());set(&mut b,0x80,b"PE\0\0");set(&mut b,0x84,&0x8664u16.to_le_bytes());set(&mut b,0x86,&4u16.to_le_bytes());set(&mut b,0x94,&240u16.to_le_bytes());set(&mut b,0x98,&0x20bu16.to_le_bytes());set(&mut b,0xb0,&0x180000000u64.to_le_bytes());set(&mut b,0xd0,&0x5000u32.to_le_bytes());set(&mut b,0x120,&0x4000u32.to_le_bytes());set(&mut b,0x124,&24u32.to_le_bytes());
  for(i,(rva,raw,size,flags))in [(0x1000u32,0x200u32,0x400u32,READ|EXEC),(0x2000,0x600,0x400,READ),(0x3000,0xa00,0x400,READ|WRITE),(0x4000,0xe00,0x100,READ)].into_iter().enumerate(){let at=0x188+i*40;for(off,value)in [(8,size),(12,rva),(16,size),(20,raw),(36,flags)]{set(&mut b,at+off,&value.to_le_bytes())}}
  set(&mut b,0xa10,AUDIO);set(&mut b,0xa90,INFO);
  for(raw,rva,offset,td)in [(0x600,0x2000u32,0u32,0x3000u32),(0x640,0x2040,8,0x3080)]{for(i,value)in [1,offset,0,td,0x2080,rva].into_iter().enumerate(){set(&mut b,raw+i*4,&value.to_le_bytes())}}
  set(&mut b,0x700,&0x180002000u64.to_le_bytes());set(&mut b,0x780,&0x180002040u64.to_le_bytes());set(&mut b,0x7b0,&0x180001000u64.to_le_bytes());
  b[0x232]=0xe8;set(&mut b,0x233,&(0x1100i32-0x1037).to_le_bytes());set(&mut b,0x322,&[0x0f,0x28,0xf2]);set(&mut b,0x325,&[0x0f,0x28,0xf9]);set(&mut b,0x32b,&[0x66,0x0f,0x2e,0x0d]);set(&mut b,0x32f,&(0x3200i32-0x1133).to_le_bytes());set(&mut b,0x347,&[0xf2,0x0f,0x11,0x3d]);set(&mut b,0x34b,&(0x3200i32-0x114f).to_le_bytes());
  for(i,(start,end))in [(0x1000u32,0x1080u32),(0x1100,0x1200)].into_iter().enumerate(){set(&mut b,0xe00+i*12,&start.to_le_bytes());set(&mut b,0xe04+i*12,&end.to_le_bytes())}b
 }
 #[test]fn rtti_and_callback_resolve_relocated_data_without_fixed_clock_addresses(){let image=Image::parse(fixture()).unwrap();assert_eq!(image.vtable(AUDIO,0).unwrap(),0x2108);let info=image.vtable(INFO,8).unwrap();assert_eq!(info,0x2188);assert_eq!(image.progress(info).unwrap(),0x3200);assert!(image.vtable(INFO,0).is_err());let mut b=fixture();b[0x34b..0x34f].copy_from_slice(&(0x3208i32-0x114f).to_le_bytes());assert!(Image::parse(b).unwrap().progress(info).is_err())}
 #[test]fn malformed_sections_ambiguous_rtti_and_code_targets_fail_closed(){
  let mut b=fixture();b[0x1bc..0x1c0].copy_from_slice(&0x1000u32.to_le_bytes());assert!(Image::parse(b).is_err());
  let mut b=fixture();b[0x800..0x808].copy_from_slice(&0x180002000u64.to_le_bytes());assert!(Image::parse(b).unwrap().vtable(AUDIO,0).is_err());
  let mut b=fixture();b[0x233..0x237].copy_from_slice(&(0x3000i32-0x1037).to_le_bytes());assert!(Image::parse(b).unwrap().progress(0x2188).is_err());
  for n in [0,64,0x200,0xeff]{assert!(Image::parse(fixture()[..n].to_vec()).is_err())}
 }
}
