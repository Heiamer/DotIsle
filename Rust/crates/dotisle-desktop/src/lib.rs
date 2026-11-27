//! C ABI for desktop widgets embedded in the Go control center.
//! The caller owns all buffers; JSON is copied before returning across the ABI.
mod apple_desktop;
mod desktop_cards;
mod desktop_host;
mod draw;
mod gpu;
mod model;
mod platform;
mod stage_overlay;
mod sidebar_io;
mod themes;
mod timeline;
mod views;
mod widget_extras;
mod widgets;
mod verify;
#[cfg(test)]mod abi_tests;
#[cfg(test)]mod render_tests;
use std::{path::PathBuf,sync::{Mutex,OnceLock,atomic::{AtomicBool,Ordering}},time::{Duration,Instant}};
use winit::{application::ApplicationHandler,event::WindowEvent,event_loop::{ActiveEventLoop,ControlFlow,EventLoop,EventLoopProxy},platform::windows::EventLoopBuilderExtWindows,window::WindowId};

static SNAPSHOT:Mutex<Option<serde_json::Value>>=Mutex::new(None);
static PROXY:OnceLock<EventLoopProxy<()>>=OnceLock::new();
static STARTED:AtomicBool=AtomicBool::new(false);
static RUNNING:AtomicBool=AtomicBool::new(false);
static STOP:AtomicBool=AtomicBool::new(false);

pub(crate) fn wake(){if let Some(proxy)=PROXY.get(){let _=proxy.send_event(());}}
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
 fn resumed(&mut self,_:&ActiveEventLoop){wake();}
 fn window_event(&mut self,_:&ActiveEventLoop,id:WindowId,event:WindowEvent){
  if let Some(action)=self.host.event(id,event,&mut self.model,&mut self.gpu,&mut self.text,self.started.elapsed().as_secs_f64(),Some(self.timeline.position)){self.model.dispatch(action);}
 }
 fn about_to_wait(&mut self,event:&ActiveEventLoop){
  if STOP.load(Ordering::Acquire){event.exit();return}
  let now=Instant::now();let dt=now.duration_since(self.last).as_secs_f64();self.last=now;
  let snapshot=SNAPSHOT.try_lock().ok().and_then(|mut s|s.take());
  let demo_changed=self.model.flush_demo_commands();
  let changed=snapshot.is_some()||demo_changed;
  if let Some(snapshot)=snapshot{
   self.model.apply(snapshot);
  }
  let moving=self.timeline.tick(&self.model,dt)&&self.model.desktop_widgets.items.iter().any(|p|p.kind==dotisle_domain::desktop_widgets::Kind::Music);
  let active=self.host.sync(event,&mut self.model,&mut self.gpu,changed,moving,dt,self.started.elapsed().as_secs_f64());
  // 活动时按帧等待，不空转；空闲的秒级截止时间只服务周期维护，热键与后台结果直接唤醒。
  let interval=if active{Duration::from_millis(16)}else{Duration::from_secs(1)};
  event.set_control_flow(ControlFlow::WaitUntil(now+interval));
 }
}
fn run(data:PathBuf)->Result<(),String>{
 let Some(_guard)=platform::center_instance(&data)?else{return Ok(())};
 let event=EventLoop::builder().with_any_thread(true).build().map_err(|e|e.to_string())?;
 PROXY.set(event.create_proxy()).map_err(|_|"desktop event loop already initialized")?;
 let mut app=Runtime{model:model::Model::new(),host:desktop_host::Host::with_data_dir(data),gpu:gpu::Gpu::new(),text:draw::Text::new_lazy_japanese(),timeline:Default::default(),last:Instant::now(),started:Instant::now()};
 event.run_app(&mut app).map_err(|e|e.to_string())
}

fn run_with_pipe(data:PathBuf,pipe:String,demo:bool)->Result<(),String>{
 // --demo 预览模式使用独立的单实例锁路径，避免与正在运行的正式版 LyricIsland.Center.exe 互斥冲突而静默退出。
 let lock_dir = if demo { data.join("allegro-demo") } else { data.clone() };
 let Some(_guard)=platform::center_instance(&lock_dir)?else{return Ok(())};
 let event=EventLoop::builder().with_any_thread(true).build().map_err(|e|e.to_string())?;
 let _=PROXY.set(event.create_proxy());
 let mut model=model::Model::new_with_pipe(pipe);
 model.demo=demo;
 let mut app=Runtime{model,host:desktop_host::Host::with_data_dir(data),gpu:gpu::Gpu::new(),text:draw::Text::new_lazy_japanese(),timeline:Default::default(),last:Instant::now(),started:Instant::now()};
 event.run_app(&mut app).map_err(|e|e.to_string())
}

/// 以独立 `DotIsle-Allegro.exe` 进程模式运行桌面小组件引擎：
/// 开启 Windows 能效模式，并通过 Named Pipe 与点屿主进程同步状态快照。
pub fn run_standalone(
 data: PathBuf,
 pipe_name: String,
 keep_alive: bool,
 demo: bool,
) -> Result<(), String> {
 platform::enable_efficiency_mode();
 if STARTED.swap(true, Ordering::AcqRel) {
  return Err("DotIsle-Allegro 事件循环已启动".into());
 }
 STOP.store(false, Ordering::Release);
 RUNNING.store(true, Ordering::Release);

 if demo {
  let snapshot = demo_snapshot();
  if let Ok(mut slot) = SNAPSHOT.lock() {
   *slot = Some(snapshot);
  }
 } else {
  let pipe = pipe_name.clone();
  std::thread::spawn(move || {
   let mut last_payload: Option<Vec<u8>> = None;
   let mut missed_polls: u32 = 0;
   while !STOP.load(Ordering::Acquire) {
    let req = serde_json::json!({"version": 1, "command": "state"});
    let mut playing = false;
    match lyric_backend::io::pipe_request(&pipe, &req) {
     Ok(state) if state.is_object() => {
      missed_polls = 0;
      playing = state["isPlaying"] == true;
      if let Ok(bytes) = serde_json::to_vec(&state) {
       if last_payload.as_ref() != Some(&bytes) {
        last_payload = Some(bytes);
        if let Ok(mut slot) = SNAPSHOT.lock() {
         *slot = Some(state);
        }
        wake();
       }
      }
     }
     _ => {
      missed_polls = missed_polls.saturating_add(1);
      if !keep_alive && missed_polls >= 12 {
       // 主进程已退出，独立 Allegro 自动平滑收尾释放资源。
       dotisle_desktop_stop();
       break;
      }
     }
    }
    let sleep_ms = if playing { 250 } else { 500 };
    std::thread::sleep(Duration::from_millis(sleep_ms));
   }
  });
 }

 let result = std::panic::catch_unwind(|| run_with_pipe(data, pipe_name, demo));
 RUNNING.store(false, Ordering::Release);
 STOP.store(true, Ordering::Release);
 match result {
  Ok(r) => r,
  Err(payload) => {
   let detail = payload
    .downcast_ref::<&str>()
    .map(|s| (*s).to_string())
    .or_else(|| payload.downcast_ref::<String>().cloned())
    .unwrap_or_else(|| "未知内部异常".into());
   Err(format!("DotIsle-Allegro 发生内部异常: {detail}"))
  }
 }
}

fn demo_snapshot() -> serde_json::Value {
 // 桌面默认演示严格按 Apple 176/16 步距（192px）网格排布：小=176×176，大=368×176，特大=368×368
 serde_json::json!({
  "settingsDark": false,
  "settings": { "SettingsTheme": "Light", "Animation": true },
  "desktopWidgets": [
   { "id": "demo-folder-4", "kind": "appFolder", "size": "small", "style": "original", "material": "acrylic", "x": 48, "y": 52, "config": { "title": "常用应用", "caption": "4" } },
   { "id": "demo-analog", "kind": "analogClock", "size": "small", "style": "original", "material": "lumen", "x": 240, "y": 52 },
   { "id": "demo-cal", "kind": "calendar", "size": "small", "style": "original", "material": "lumen", "x": 432, "y": 52 },
   { "id": "demo-weather", "kind": "weather", "size": "medium", "style": "original", "material": "acrylic", "x": 624, "y": 52, "config": { "city": "上海" } },
   { "id": "demo-pro", "kind": "systemPro", "size": "medium", "style": "original", "material": "lumen", "x": 48, "y": 244 },
   { "id": "demo-rings", "kind": "activityRings", "size": "small", "style": "original", "material": "lumen", "x": 432, "y": 244 },
   { "id": "demo-note", "kind": "note", "size": "medium", "style": "original", "material": "acrylic", "x": 624, "y": 244, "config": { "text": "- [ ] 右键卡片一键切换 小 / 大 / 特大\n- [ ] Alt+~ 滑出右侧独立区块侧边栏\n- [x] Apple 标准 1×1 / 2×1 / 2×2 比例已就绪" } },
   { "id": "demo-quote", "kind": "quote", "size": "medium", "style": "original", "material": "lumen", "x": 48, "y": 436 },
   { "id": "demo-folder-9", "kind": "appFolder", "size": "small", "style": "original", "material": "acrylic", "x": 432, "y": 436, "config": { "title": "快捷百宝箱", "caption": "9" } }
  ],
  "widgetData": {
   "demo-weather": { "city": "上海", "temperature": 22.0, "high": 25.0, "low": 16.0, "condition": "晴间多云 · 体感舒适", "stale": false },
   "demo-focusStats": { "todayMs": 2700000 }
  },
  "screenTime": { "todayMs": 12240000, "yesterdayMs": 10800000 },
  "systemMetrics": { "cpuPercent": 18.0, "memoryPercent": 46.0, "gpuPercent": 14.0, "netRxKbps": 420.0, "netTxKbps": 64.0 }
 })
}

fn describe_only(_:serde_json::Value)->Result<serde_json::Value,String>{Err("desktop renderer has no background service commands".into())}
dotisle_module_api::export_service!("desktop",describe_only);
