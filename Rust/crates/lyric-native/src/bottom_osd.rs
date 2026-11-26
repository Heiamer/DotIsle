//! A separate, non-activating system hint surface. It never replaces the island activity.
use std::{path::Path,time::{Instant,Duration}};
use windows::{core::{w,Result},Win32::{Foundation::*,Graphics::Gdi::*,System::LibraryLoader::GetModuleHandleW,UI::{WindowsAndMessaging::*,HiDpi::*}}};
use crate::{renderer::{Renderer,MaterialTheme,Material},notifications::Notice};
pub struct Overlay{theme:MaterialTheme,hwnd:HWND,renderer:Option<Renderer>,notice:Option<Notice>,opacity:f64,alpha_vel:f64,motion:f64,motion_vel:f64,target:f64,last_tick:Instant,until:Instant,animate:bool,dirty:bool,idle:Instant}
impl Overlay{
 pub fn new()->Self{let now=Instant::now();Self{theme:MaterialTheme::new(Material::LightField,false),hwnd:HWND::default(),renderer:None,notice:None,opacity:0.,alpha_vel:0.,motion:0.,motion_vel:0.,target:0.,last_tick:now,until:now,animate:true,dirty:false,idle:now}}
 pub fn visible(&self)->bool{self.notice.is_some()&&(self.opacity>0.001||self.target>0.)}
 pub unsafe fn show(&mut self,notice:Notice,anchor:HWND,font:&Path,animate:bool,theme:MaterialTheme)->Result<()>{
  let dpi=GetDpiForWindow(anchor).max(96)as f32;let scale=dpi as f64/96.;let width=(272.*scale).round()as i32;let height=(80.*scale).round()as i32;
  let mut info=MONITORINFO{cbSize:std::mem::size_of::<MONITORINFO>()as u32,..Default::default()};GetMonitorInfoW(MonitorFromWindow(anchor,MONITOR_DEFAULTTONEAREST),&mut info).ok()?;
  let work=info.rcWork;let x=work.left+(work.right-work.left-width)/2;let y=work.bottom-height-(40.*scale).round()as i32;
  if self.hwnd==HWND::default(){let instance=GetModuleHandleW(None)?;let class=w!("DotIsle.BottomOSD.v201");let wc=WNDCLASSW{lpfnWndProc:Some(proc),hInstance:instance.into(),lpszClassName:class,..Default::default()};RegisterClassW(&wc);
   self.hwnd=CreateWindowExW(WS_EX_TOOLWINDOW|WS_EX_NOACTIVATE|WS_EX_NOREDIRECTIONBITMAP|WS_EX_TRANSPARENT,class,w!("点屿 · 系统状态"),WS_POPUP,x,y,width,height,None,None,Some(instance.into()),None)?;
  }
  SetWindowPos(self.hwnd,Some(HWND_TOPMOST),x,y,width,height,SWP_NOACTIVATE|SWP_SHOWWINDOW)?;
  if !self.renderer.as_ref().is_some_and(|r|r.width==width as u32&&r.height==height as u32&&(r.dpi-dpi).abs()<0.01){self.renderer=None;self.renderer=Some(Renderer::new(self.hwnd,width as u32,height as u32,dpi,font)?);if let Some(renderer)=&mut self.renderer{renderer.set_material_theme(&self.theme)?;}}
  self.set_theme(theme)?;
  let now=Instant::now();self.notice=Some(notice);self.animate=animate;self.target=1.;self.last_tick=now;self.until=now+Duration::from_millis(1600);self.dirty=true;self.tick()
 }
 pub unsafe fn set_theme(&mut self,theme:MaterialTheme)->Result<()>{
  if self.theme!=theme{if let Some(renderer)=&mut self.renderer{renderer.set_material_theme(&theme)?;}self.theme=theme;self.dirty=true;}Ok(())
 }
 pub unsafe fn clear(&mut self){self.notice=None;self.opacity=0.;self.alpha_vel=0.;self.motion=0.;self.motion_vel=0.;self.target=0.;self.dirty=false;self.idle=Instant::now();if self.hwnd!=HWND::default(){let _=ShowWindow(self.hwnd,SW_HIDE);}}
 pub unsafe fn tick(&mut self)->Result<()>{
  if self.notice.is_none(){
   // The overlay keeps its own full wgpu + Vello renderer. Hints are rare, so it
   // is retired after half a hidden minute instead of holding a second GPU
   // context alive for the rest of the session; the next hint rebuilds it.
   if self.renderer.is_some()&&self.idle.elapsed()>=Duration::from_secs(30){self.renderer=None;self.theme.cover=None;}
   return Ok(())}let now=Instant::now();let dt=now.duration_since(self.last_tick).as_secs_f64().clamp(0.,0.05);self.last_tick=now;
  if now>=self.until&&self.target>0.{self.target=0.;}
  let (next_a,next_m)=if self.animate{
   let step=|val:&mut f64,vel:&mut f64,target:f64,w:f64,z:f64|{
    if dt<=0.{return}let d=*val-target;
    if z<1.{let wd=w*(1.-z*z).sqrt();let b=(*vel+z*w*d)/wd;let e=(-z*w*dt).exp();let(s,c)=(wd*dt).sin_cos();let env=d*c+b*s;*val=target+e*env;*vel=e*(-z*w*env+wd*(b*c-d*s));}
    else{let b=*vel+w*d;let e=(-w*dt).exp();*val=target+(d+b*dt)*e;*vel=(*vel-w*b*dt)*e;}
   };
   let(mw,mz)=if self.target>0.{(21.0,0.78)}else{(25.0,0.95)};
   let(aw,az)=if self.target>0.{(24.0,1.0)}else{(28.0,1.0)};
   let(mut a,mut av,mut m,mut mv)=(self.opacity,self.alpha_vel,self.motion,self.motion_vel);
   step(&mut a,&mut av,self.target,aw,az);step(&mut m,&mut mv,self.target,mw,mz);
   self.alpha_vel=av;self.motion_vel=mv;(a.clamp(0.,1.),m)
  }else{self.alpha_vel=0.;self.motion_vel=0.;(self.target,self.target)};
  let settled=(next_a-self.target).abs()<0.002&&self.alpha_vel.abs()<0.02&&(next_m-self.target).abs()<0.002&&self.motion_vel.abs()<0.02;
  if (next_a-self.opacity).abs()>0.0001||(next_m-self.motion).abs()>0.0001||self.dirty{
   self.opacity=next_a;self.motion=next_m;self.dirty=false;
   if let(Some(renderer),Some(notice))=(&mut self.renderer,&self.notice){renderer.render_bottom_osd(notice,next_a,if self.animate{12.*(1.-next_m)}else{0.},crate::surface::Surface::expanded(self.theme.material,self.theme.cover_background),true)?;}}
  if settled{self.opacity=self.target;self.motion=self.target;if self.target==0.{self.clear();}}
  Ok(())
 }
}
impl Drop for Overlay{fn drop(&mut self){self.renderer=None;if self.hwnd!=HWND::default(){unsafe{let _=DestroyWindow(self.hwnd);}}}}
unsafe extern "system" fn proc(hwnd:HWND,message:u32,w:WPARAM,l:LPARAM)->LRESULT{match message{WM_NCHITTEST=>LRESULT(HTTRANSPARENT as isize),WM_MOUSEACTIVATE=>LRESULT(MA_NOACTIVATE as isize),WM_ERASEBKGND=>LRESULT(1),_=>DefWindowProcW(hwnd,message,w,l)}}
