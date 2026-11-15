//! Version-independent CEF background seek. No top-level show, focus, global
//! input, debug port, process launch or player-memory writes are involved.
use std::time::{Duration,Instant};
use windows::{core::{BOOL,PCWSTR},Win32::{Foundation::{CloseHandle,HANDLE,HWND,LPARAM,WPARAM,RECT,WAIT_OBJECT_0,WAIT_ABANDONED},System::Threading::{CreateMutexW,ReleaseMutex,WaitForSingleObject},UI::{HiDpi::*,Input::KeyboardAndMouse::IsWindowEnabled,WindowsAndMessaging::*}}};
use crate::netease_control::Target;
mod accessibility;
mod resources;

// Detect the running player and its CEF window tree, never a version or package hash.
fn class(window:HWND,name:&str)->bool{unsafe{let mut text=[0u16;256];let n=GetClassNameW(window,&mut text);n>0&&String::from_utf16_lossy(&text[..n as usize])==name}}
fn owned(window:HWND,pid:u32)->bool{unsafe{let mut owner=0;GetWindowThreadProcessId(window,Some(&mut owner));owner==pid&&IsWindow(Some(window)).as_bool()}}
#[derive(Default)]struct Windows{pid:u32,roots:Vec<HWND>,hosts:Vec<HWND>,widgets:Vec<HWND>,renderers:Vec<HWND>}
unsafe extern "system" fn top(window:HWND,p:LPARAM)->BOOL{
 let found=&mut*(p.0 as*mut Windows);
 if owned(window,found.pid)&&class(window,"OrpheusBrowserHost")&&!GetWindow(window,GW_OWNER).is_ok_and(|h|!h.0.is_null()){found.roots.push(window);}BOOL(1)
}
unsafe extern "system" fn children(window:HWND,p:LPARAM)->BOOL{
 let found=&mut*(p.0 as*mut Windows);
 if owned(window,found.pid){if class(window,"CefBrowserWindow"){found.hosts.push(window);}if class(window,"Chrome_WidgetWin_0"){found.widgets.push(window);}if class(window,"Chrome_RenderWidgetHostHWND"){found.renderers.push(window);}}BOOL(1)
}
fn windows(pid:u32)->Option<(HWND,HWND,HWND,Option<HWND>)>{unsafe{
 let mut found=Windows{pid,..Default::default()};let p=LPARAM(&mut found as*mut _ as isize);let _=EnumWindows(Some(top),p);
 if found.roots.len()!=1{return None}let root=found.roots[0];if !IsWindowEnabled(root).as_bool(){return None}
 let _=EnumChildWindows(Some(root),Some(children),p);if found.hosts.len()!=1||found.widgets.len()!=1||found.renderers.len()>1{return None}
 Some((root,found.hosts[0],found.widgets[0],found.renderers.first().copied()))
}}
struct Dpi(DPI_AWARENESS_CONTEXT);
impl Dpi{fn enter()->Self{Self(unsafe{SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)})}}
impl Drop for Dpi{fn drop(&mut self){unsafe{let _=SetThreadDpiAwarenessContext(self.0);}}}
struct Serial(HANDLE);
impl Serial{fn enter()->Result<Self,String>{unsafe{
 let name="Local\\DotIsle.NetEase.BackgroundSeek.v1".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
 let handle=CreateMutexW(None,false,PCWSTR(name.as_ptr())).map_err(|e|e.to_string())?;
 let status=WaitForSingleObject(handle,0);if status!=WAIT_OBJECT_0&&status!=WAIT_ABANDONED{let _=CloseHandle(handle);return Err("网易云正在处理另一条进度操作，请稍后重试".into())}Ok(Self(handle))
}}}
impl Drop for Serial{fn drop(&mut self){unsafe{let _=ReleaseMutex(self.0);let _=CloseHandle(self.0);}}}
/// Restores only a temporarily resized child, and yields if the user changes the main window.
struct ChildSize{pid:u32,root:HWND,host:HWND,width:i32,height:i32,iconic:bool,visible:bool}
impl Drop for ChildSize{fn drop(&mut self){unsafe{
 if owned(self.root,self.pid)&&owned(self.host,self.pid)&&IsIconic(self.root).as_bool()==self.iconic&&IsWindowVisible(self.root).as_bool()==self.visible{
  let _=SetWindowPos(self.host,None,0,0,self.width,self.height,SWP_NOMOVE|SWP_NOZORDER|SWP_NOACTIVATE|SWP_ASYNCWINDOWPOS);
 }
}}}
/// Prevent background mouse focus at both native root and CEF widget boundaries.
/// Only restore the property bit added by this transaction; never disable a window.
struct NoActivate{pid:u32,widget:HWND,kind:&'static str,added:bool}
impl NoActivate{fn enter(pid:u32,host:HWND,widget:HWND)->Result<Self,String>{unsafe{
 if !owned(widget,pid)||!class(widget,"Chrome_WidgetWin_0")||!IsChild(host,widget).as_bool(){return Err("网易云后台控件层级已变化，已取消进度操作".into())}
 Self::protect(pid,widget,"Chrome_WidgetWin_0")
}}fn root(pid:u32,root:HWND)->Result<Self,String>{unsafe{
 if !owned(root,pid)||!class(root,"OrpheusBrowserHost"){return Err("网易云主窗口已变化，已取消后台操作".into())}
 Self::protect(pid,root,"OrpheusBrowserHost")
}}unsafe fn protect(pid:u32,widget:HWND,kind:&'static str)->Result<Self,String>{
 // Refuse a hung browser before making any temporary property change.
 let mut result=0;SendMessageTimeoutW(widget,WM_NULL,WPARAM(0),LPARAM(0),SMTO_ABORTIFHUNG|SMTO_BLOCK,600,Some(&mut result)).0.ne(&0).then_some(()).ok_or("网易云正在忙，已取消后台操作")?;
 let old=GetWindowLongPtrW(widget,GWL_EXSTYLE);let added=old&WS_EX_NOACTIVATE.0 as isize==0;
 let guard=Self{pid,widget,kind,added};
 if added{SetWindowLongPtrW(widget,GWL_EXSTYLE,old|WS_EX_NOACTIVATE.0 as isize);if GetWindowLongPtrW(widget,GWL_EXSTYLE)&WS_EX_NOACTIVATE.0 as isize==0{return Err("无法保护网易云后台焦点，已取消进度操作".into())}}
 Ok(guard)
}}
impl Drop for NoActivate{fn drop(&mut self){unsafe{
 if self.added&&owned(self.widget,self.pid)&&class(self.widget,self.kind){
  let style=GetWindowLongPtrW(self.widget,GWL_EXSTYLE);SetWindowLongPtrW(self.widget,GWL_EXSTYLE,style&!(WS_EX_NOACTIVATE.0 as isize));
 }
}}}
fn valid_size(width:i32,height:i32,dpi:u32)->bool{let scale=dpi as f64/96.;(700.0..=6000.0).contains(&(width as f64/scale))&&(400.0..=3000.0).contains(&(height as f64/scale))}
#[derive(Debug,PartialEq)]struct Point{x:i32,y:i32,start:i32,actual:f64}
fn point(track:(f64,f64,f64),width:i32,height:i32,dpi:u32,fraction:f64)->Option<Point>{
 let(left,right,center)=track;let scale=dpi as f64/96.;
 if !valid_size(width,height,dpi)||!fraction.is_finite()||!(0.0..=1.).contains(&fraction)||![left,right,center].iter().all(|v|v.is_finite())||left< -2.||right>width as f64+2.||right-left<160.*scale||center<1.||center>=height as f64-1.{return None}
 // UIA's native renderer boundary can differ from CEF's row by a physical
 // border pixel while hidden. Clip only this bounded provider rounding.
 let(left,right)=(left.max(0.),right.min(width as f64));
 let x=(left+fraction*(right-left)).round().clamp((left+1.).ceil(),(right-2.).floor())as i32;
 let inset=16.*scale;let start=(x as f64).clamp((left+inset).ceil(),(right-inset).floor())as i32;
 Some(Point{x,y:center.round()as i32,start,actual:(x as f64-left)/(right-left)})
}
fn geometry(target:&Target,root:HWND,renderer:HWND,dpi:u32,width:i32,height:i32)->Result<(f64,f64,f64),String>{
 if let Some(s)=accessibility::slider(root,renderer,dpi){let track=(s.left as f64,s.right as f64,(s.top+s.bottom)as f64/2.);if point(track,width,height,dpi,0.5).is_some(){return Ok(track)}}
 match resources::installed(&target.executable)?{resources::Layout::Legacy(l)=>Ok(l.track(width,height,dpi)),resources::Layout::SemanticFooter=>Err("网易云后台进度区域尚未就绪，正在等待控件连接".into())}
}
pub fn inspect_resources(path:&std::path::Path)->Result<serde_json::Value,String>{resources::inspect(path).and_then(|v|serde_json::to_value(v).map_err(|e|e.to_string()))}
pub fn available(target:&Target)->bool{target.available()&&windows(target.pid).is_some()}
pub fn diagnostics(target:&Target)->serde_json::Value{let _dpi=Dpi::enter();unsafe{match windows(target.pid){Some((root,_,_,renderer))=>serde_json::json!({"pid":target.pid,"visible":IsWindowVisible(root).as_bool(),"minimized":IsIconic(root).as_bool(),"foreground":GetForegroundWindow()==root,"slider":renderer.and_then(|renderer|accessibility::slider(root,renderer,GetDpiForWindow(root).max(96)))}),None=>serde_json::json!({"pid":target.pid,"available":false})}}}
pub fn metadata(target:&Target)->Option<(String,String)>{let(root,_,_,_)=windows(target.pid)?;let mut text=[0u16;1024];let n=unsafe{GetWindowTextW(root,&mut text)};if n<=0{return None}let title=String::from_utf16_lossy(&text[..n as usize]);let(song,artist)=title.rsplit_once(" - ")?;let(song,artist)=(song.trim(),artist.trim());if song.is_empty()||artist.is_empty(){return None}Some((song.into(),artist.into()))}

pub fn seek(target:&Target,seconds:f64,duration:f64,valid:&dyn Fn()->bool)->Result<f64,String>{
 let seconds=crate::netease_control::bounded_target(seconds,duration)?;
 if !available(target){return Err("网易云后台窗口尚未就绪，请稍后重试".into())}
 let _serial=Serial::enter()?;let _dpi=Dpi::enter();
 let (root,host,widget,mut renderer)=windows(target.pid).ok_or("网易云窗口已变化，已取消进度操作")?;
 let root_focus=NoActivate::root(target.pid,root)?;
 let focus=NoActivate::enter(target.pid,host,widget)?;
 let mut original=RECT::default();unsafe{GetClientRect(host,&mut original)}.map_err(|e|e.to_string())?;
 let mut resize=None;
 if original.right==0||original.bottom==0{
  let mut placement=WINDOWPLACEMENT{length:std::mem::size_of::<WINDOWPLACEMENT>()as u32,..Default::default()};
  unsafe{GetWindowPlacement(root,&mut placement)}.map_err(|e|e.to_string())?;
  let width=placement.rcNormalPosition.right-placement.rcNormalPosition.left;let height=placement.rcNormalPosition.bottom-placement.rcNormalPosition.top;
  if !valid_size(width,height,unsafe{GetDpiForWindow(root)}.max(96)){return Err("网易云保存的窗口尺寸无效，已取消后台操作".into())}
  resize=Some(ChildSize{pid:target.pid,root,host,width:original.right,height:original.bottom,iconic:unsafe{IsIconic(root)}.as_bool(),visible:unsafe{IsWindowVisible(root)}.as_bool()});
  // A minimized CEF host collapses to 0x0. Resize only that child; leave the
  // main window minimized/hidden. This recreates the renderer without showing it.
  unsafe{SetWindowPos(host,None,0,0,width,height,SWP_NOMOVE|SWP_NOZORDER|SWP_NOACTIVATE|SWP_ASYNCWINDOWPOS)}.map_err(|e|e.to_string())?;
  let until=Instant::now()+Duration::from_millis(900);
  while Instant::now()<until{
   if !valid(){return Err("歌曲已变化，已取消后台进度操作".into())}
   let Some((same,_,_,next))=windows(target.pid)else{return Err("网易云窗口已变化，已取消后台操作".into())};
   if same!=root{return Err("网易云窗口已变化，已取消后台操作".into())}
   if let Some(next)=next{let mut r=RECT::default();if unsafe{GetClientRect(next,&mut r)}.is_ok()&&r.right>width/2&&r.bottom>height/2{renderer=Some(next);std::thread::sleep(Duration::from_millis(60));break}}
   std::thread::sleep(Duration::from_millis(20));
  }
 }
 let renderer=renderer.filter(|w|owned(*w,target.pid)&&class(*w,"Chrome_RenderWidgetHostHWND")).ok_or("网易云后台播放控件未就绪，请稍后重试")?;
 let mut rect=RECT::default();unsafe{GetClientRect(renderer,&mut rect)}.map_err(|e|e.to_string())?;
 let dpi=unsafe{GetDpiForWindow(root)}.max(96);let track=geometry(target,root,renderer,dpi,rect.right,rect.bottom)?;
 let point=point(track,rect.right,rect.bottom,dpi,seconds/duration).ok_or("网易云进度控件尺寸不受支持")?;let(x,y)=(point.x,point.y);
 let pos=LPARAM(((y as u32)<<16|x as u32)as isize);
 // The rounded edge does not accept a new mouse-down. Capture inside the
 // bar, then drag to an edge target before releasing the same gesture.
 let start_x=point.start;let start=LPARAM(((y as u32)<<16|start_x as u32)as isize);
 if !valid(){return Err("歌曲已变化，已取消后台进度操作".into())}
 let focus_after_resize=NoActivate::enter(target.pid,host,widget)?;
 crate::netease_attention::arm(root,target.pid)?;
 unsafe{
  PostMessageW(Some(renderer),WM_MOUSEMOVE,WPARAM(0),start).map_err(|e|e.to_string())?;
  let down=PostMessageW(Some(renderer),WM_LBUTTONDOWN,WPARAM(1),start);
  let moved=if start_x!=x{PostMessageW(Some(renderer),WM_MOUSEMOVE,WPARAM(1),pos)}else{Ok(())};
  let up=PostMessageW(Some(renderer),WM_LBUTTONUP,WPARAM(0),pos);
  down.map_err(|e|e.to_string())?;moved.map_err(|e|e.to_string())?;up.map_err(|e|e.to_string())?;
 }
 // CEF consumes the complete posted click under NOACTIVATE before cleanup.
 std::thread::sleep(Duration::from_millis(180));
 let restored_size=resize.as_ref().map(|r|(r.width,r.height));drop(resize);
 if let Some((width,height))=restored_size{let until=Instant::now()+Duration::from_millis(900);while Instant::now()<until{let mut r=RECT::default();if !owned(host,target.pid)||unsafe{GetClientRect(host,&mut r)}.is_err()||r.right==width&&r.bottom==height{break}std::thread::sleep(Duration::from_millis(20));}}
 drop(focus_after_resize);drop(focus);drop(root_focus);Ok(point.actual*duration)
}
#[cfg(test)]mod tests{use super::*;
 #[test]fn edge_drags_begin_inside_the_resolved_bar_at_each_scale(){for dpi in [96,144,192]{let scale=dpi as f64/96.;let width=(1056.*scale)as i32;let height=(752.*scale)as i32;let track=(300.*scale,756.*scale,735.5*scale);let a=point(track,width,height,dpi,0.).unwrap();let b=point(track,width,height,dpi,1.).unwrap();assert_eq!(a.start,(316.*scale).ceil()as i32);assert_eq!(b.start,(740.*scale).floor()as i32);assert!(a.actual>=0.&&b.actual<=1.);}}
 #[test]fn physical_coordinates_use_the_actual_timeline_width_and_dpi(){assert_eq!(point((335.,687.,735.5),1022,752,96,0.5),Some(Point{x:511,y:736,start:511,actual:0.5}));assert_eq!(point((0.,1584.,1004.5),1584,1128,144,0.5),Some(Point{x:792,y:1005,start:792,actual:0.5}));}
 #[test]fn malformed_rows_and_mini_player_geometry_never_produce_input(){for(w,h,d,f)in[(0,0,96,0.5),(300,100,96,0.5),(1000,30,96,0.5),(1000,800,96,f64::NAN),(1000,800,96,-0.1)]{assert!(point((0.,w as f64,h as f64-80.),w,h,d,f).is_none());}for track in [(f64::NAN,1000.,700.),(-3.,1000.,700.),(0.,1100.,700.),(0.,1000.,900.)]{assert!(point(track,1000,800,96,0.5).is_none());}}
 #[test]fn hidden_provider_border_rounding_is_clipped_inside_the_renderer(){for fraction in [0.,0.5,1.]{let p=point((-1.,1001.,700.),1000,800,96,fraction).unwrap();assert!((1..=998).contains(&p.x));assert!((16..=984).contains(&p.start));assert!((0. ..=1.).contains(&p.actual));}assert!(point((-2.1,1000.,700.),1000,800,96,0.5).is_none());assert!(point((0.,1002.1,700.),1000,800,96,0.5).is_none());}

}

#[cfg(test)]mod focus_guard_tests{use super::*;use windows::core::w;
 unsafe extern "system" fn wndproc(h:HWND,m:u32,w:WPARAM,l:LPARAM)->windows::Win32::Foundation::LRESULT{DefWindowProcW(h,m,w,l)}
 #[test]fn native_root_guard_preserves_visibility_and_unrelated_style_bits(){unsafe{
  let wc=WNDCLASSW{lpfnWndProc:Some(wndproc),lpszClassName:w!("OrpheusBrowserHost"),..Default::default()};let _=RegisterClassW(&wc);
  let root=CreateWindowExW(WS_EX_TOOLWINDOW,w!("OrpheusBrowserHost"),w!(""),WS_POPUP,0,0,20,20,None,None,None,None).unwrap();
  let mut pid=0;GetWindowThreadProcessId(root,Some(&mut pid));let style=GetWindowLongPtrW(root,GWL_EXSTYLE);
  {let _guard=NoActivate::root(pid,root).unwrap();assert_ne!(GetWindowLongPtrW(root,GWL_EXSTYLE)&WS_EX_NOACTIVATE.0 as isize,0);assert!(!IsWindowVisible(root).as_bool());assert!(!IsIconic(root).as_bool());SetWindowLongPtrW(root,GWL_EXSTYLE,GetWindowLongPtrW(root,GWL_EXSTYLE)|WS_EX_TRANSPARENT.0 as isize);}
  assert_eq!(GetWindowLongPtrW(root,GWL_EXSTYLE),style|WS_EX_TRANSPARENT.0 as isize);assert!(IsWindowEnabled(root).as_bool());let _=DestroyWindow(root);
 }}
 #[test]fn focus_guard_is_child_only_and_restores_only_its_added_bit(){unsafe{
  let wc=WNDCLASSW{lpfnWndProc:Some(wndproc),lpszClassName:w!("Chrome_WidgetWin_0"),..Default::default()};let _=RegisterClassW(&wc);
  let host=CreateWindowExW(WINDOW_EX_STYLE(0),w!("STATIC"),w!(""),WS_POPUP,0,0,20,20,None,None,None,None).unwrap();
  let widget=CreateWindowExW(WS_EX_TOOLWINDOW,w!("Chrome_WidgetWin_0"),w!(""),WS_CHILD,0,0,10,10,Some(host),None,None,None).unwrap();
  let renderer=CreateWindowExW(WINDOW_EX_STYLE(0),w!("STATIC"),w!(""),WS_CHILD,0,0,10,10,Some(widget),None,None,None).unwrap();
  let mut pid=0;GetWindowThreadProcessId(widget,Some(&mut pid));let root_style=GetWindowLongPtrW(host,GWL_EXSTYLE);let old=GetWindowLongPtrW(widget,GWL_EXSTYLE);
  {let _guard=NoActivate::enter(pid,host,widget).unwrap();assert_ne!(GetWindowLongPtrW(widget,GWL_EXSTYLE)&WS_EX_NOACTIVATE.0 as isize,0);assert_eq!(GetWindowLongPtrW(host,GWL_EXSTYLE),root_style);assert!(IsWindowEnabled(widget).as_bool());SetWindowLongPtrW(widget,GWL_EXSTYLE,GetWindowLongPtrW(widget,GWL_EXSTYLE)|WS_EX_TRANSPARENT.0 as isize);}
  assert_eq!(GetWindowLongPtrW(widget,GWL_EXSTYLE),old|WS_EX_TRANSPARENT.0 as isize);
  SetWindowLongPtrW(widget,GWL_EXSTYLE,old|WS_EX_NOACTIVATE.0 as isize);{let _guard=NoActivate::enter(pid,host,widget).unwrap();}assert_ne!(GetWindowLongPtrW(widget,GWL_EXSTYLE)&WS_EX_NOACTIVATE.0 as isize,0);
  assert!(NoActivate::enter(pid,host,renderer).is_err());let _=DestroyWindow(host);
 }}
}
