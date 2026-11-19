//! C ABI for desktop widgets embedded in the Go control center.
//! The caller owns all buffers; JSON is copied before returning across the ABI.
mod apple_desktop;
mod desktop_cards;
mod desktop_host;
mod draw;
mod gpu;
mod model;
mod platform;
mod themes;
mod timeline;
mod views;
mod widget_extras;
mod widgets;
mod verify;
#[cfg(test)]mod abi_tests;
use std::{path::PathBuf,sync::{Mutex,OnceLock,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use winit::{application::ApplicationHandler,event::WindowEvent,event_loop::{ActiveEventLoop,ControlFlow,EventLoop,EventLoopProxy},platform::windows::EventLoopBuilderExtWindows,window::WindowId};

static SNAPSHOT:Mutex<Option<serde_json::Value>>=Mutex::new(None);
static PROXY:OnceLock<EventLoopProxy<()>>=OnceLock::new();
static STARTED:AtomicBool=AtomicBool::new(false);
static RUNNING:AtomicBool=AtomicBool::new(false);
static STOP:AtomicBool=AtomicBool::new(false);

fn wake(){if let Some(proxy)=PROXY.get(){let _=proxy.send_event(());}}
unsafe fn input<'a>(data:*const u8,len:usize)->Result<&'a [u8],()> {if data.is_null()||len==0||len>8*1024*1024{return Err(())}Ok(std::slice::from_raw_parts(data,len))}

#[no_mangle]pub extern "C" fn dotisle_desktop_abi()->u32{1}
#[no_mangle]pub extern "C" fn dotisle_desktop_running()->u32{RUNNING.load(Ordering::Acquire)as u32}
#[no_mangle]pub extern "C" fn dotisle_desktop_stop(){STOP.store(true,Ordering::Release);wake();}
#[no_mangle]pub unsafe extern "C" fn dotisle_desktop_snapshot(data:*const u8,len:usize)->i32{
 let Ok(bytes)=input(data,len)else{return 1};let Ok(value)=serde_json::from_slice::<serde_json::Value>(bytes)else{return 1};
 if !value.is_object(){return 1}let Ok(mut slot)=SNAPSHOT.lock()else{return 2};*slot=Some(value);drop(slot);wake();0
}
#[no_mangle]pub unsafe extern "C" fn dotisle_desktop_run(data:*const u8,len:usize)->i32{
 let Ok(bytes)=input(data,len)else{return 1};let Ok(path)=std::str::from_utf8(bytes)else{return 1};let path=PathBuf::from(path);
 if STARTED.swap(true,Ordering::AcqRel){return 2}
 RUNNING.store(true,Ordering::Release);
 let result=std::panic::catch_unwind(||run(path));RUNNING.store(false,Ordering::Release);
 match result{Ok(Ok(()))=>0,Ok(Err(error))=>{eprintln!("desktop runtime: {error}");3},Err(_)=>4}
}
#[no_mangle]pub unsafe extern "C" fn dotisle_desktop_verify(data:*const u8,len:usize)->i32{
 let Ok(bytes)=input(data,len)else{return 1};let Ok(path)=std::str::from_utf8(bytes)else{return 1};
 match std::panic::catch_unwind(||verify::capture(std::path::Path::new(path))){Ok(Ok(()))=>0,Ok(Err(e))=>{eprintln!("desktop verification: {e}");3},Err(_)=>4}
}
// Shared geometry prevents the Go editor from drifting from the native island.
#[no_mangle]pub unsafe extern "C" fn dotisle_desktop_layout(data:*const u8,len:usize,out:*mut u8,capacity:usize)->isize{
 let result=std::panic::catch_unwind(||->Result<Vec<u8>,String>{
  let bytes=input(data,len).map_err(|_|"invalid buffer")?;
  let value:serde_json::Value=serde_json::from_slice(bytes).map_err(|e|e.to_string())?;
  let draft=value.get("draft").unwrap_or(&value);
  let mix=lyric_core::music_layout::MusicLayout::from_setting(value["baseLayout"].as_str().unwrap_or("")).position();
  let font=value["fontSize"].as_f64().unwrap_or(14.).clamp(11.,20.);
  let layout=if draft.is_null(){lyric_core::custom_layout::Layout::default()}else{lyric_core::custom_layout::Layout::from_value(draft)?};
  let geometry=layout.geometry(400.,1.,font,mix);
  let elements=lyric_core::custom_layout::elements(&geometry).into_iter().map(|(id,r)|serde_json::json!({"id":id,"label":lyric_core::custom_layout::element_label(id),"resizable":lyric_core::custom_layout::resizable(id),"x":r.x,"y":r.y,"w":r.w,"h":r.h})).collect::<Vec<_>>();
  serde_json::to_vec(&serde_json::json!({"layout":layout,"height":geometry.height,"elements":elements})).map_err(|e|e.to_string())
 });
 let bytes=match result{Ok(Ok(bytes))=>bytes,Ok(Err(error))=>return write_output(serde_json::json!({"error":error}).to_string().as_bytes(),out,capacity),Err(_)=>return -1};write_output(&bytes,out,capacity)
}
unsafe fn write_output(bytes:&[u8],out:*mut u8,capacity:usize)->isize{if out.is_null()||bytes.len()>capacity{return -(bytes.len()as isize)}std::ptr::copy_nonoverlapping(bytes.as_ptr(),out,bytes.len());bytes.len()as isize}

struct Runtime{model:model::Model,host:desktop_host::Host,gpu:gpu::Gpu,text:draw::Text,timeline:timeline::Timeline,last:Instant,started:Instant}
impl ApplicationHandler for Runtime{
 fn resumed(&mut self,_:&ActiveEventLoop){}
 fn window_event(&mut self,_:&ActiveEventLoop,id:WindowId,event:WindowEvent){
  if let Some(action)=self.host.event(id,event,&mut self.model,&mut self.gpu,&mut self.text,self.started.elapsed().as_secs_f64(),Some(self.timeline.position)){self.model.dispatch(action);}
 }
 fn about_to_wait(&mut self,event:&ActiveEventLoop){
  if STOP.load(Ordering::Acquire){event.exit();return}
  let now=Instant::now();let dt=now.duration_since(self.last).as_secs_f64();self.last=now;
  let snapshot=SNAPSHOT.lock().ok().and_then(|mut s|s.take());let changed=snapshot.is_some();
  if let Some(snapshot)=snapshot{self.model.apply(snapshot);}
  let moving=self.timeline.tick(&self.model,dt)&&self.model.desktop_widgets.items.iter().any(|p|p.kind==dotisle_domain::desktop_widgets::Kind::Music);
  self.host.sync(event,&mut self.model,&mut self.gpu,changed,moving);
  // Native surface presentation paces moving widgets; static widgets wake once a second.
  event.set_control_flow(if moving{ControlFlow::Poll}else{ControlFlow::WaitUntil(now+Duration::from_secs(1))});
 }
}
fn run(data:PathBuf)->Result<(),String>{
 let Some(_guard)=platform::center_instance(&data)?else{return Ok(())};
 let event=EventLoop::builder().with_any_thread(true).build().map_err(|e|e.to_string())?;
 PROXY.set(event.create_proxy()).map_err(|_|"desktop event loop already initialized")?;
 let mut app=Runtime{model:model::Model::new(),host:Default::default(),gpu:gpu::Gpu::new(),text:draw::Text::new_lazy_japanese(),timeline:Default::default(),last:Instant::now(),started:Instant::now()};
 event.run_app(&mut app).map_err(|e|e.to_string())
}

fn describe_only(_:serde_json::Value)->Result<serde_json::Value,String>{Err("desktop renderer has no background service commands".into())}
dotisle_module_api::export_service!("desktop",describe_only);
