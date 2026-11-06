//! Window regions and clipboard only. Vello owns all drawing.
use windows::Win32::{Foundation::{HGLOBAL},System::{DataExchange::*,Memory::*}};
use windows::core::PCWSTR;
use std::os::windows::ffi::OsStrExt;
pub fn clipboard()->Option<String>{unsafe{OpenClipboard(None).ok()?;let out=(||{let handle=GetClipboardData(13).ok()?;let global=HGLOBAL(handle.0);let ptr=GlobalLock(global)as *const u16;if ptr.is_null(){return None}let length=GlobalSize(global)/2;let slice=std::slice::from_raw_parts(ptr,length.min(65536));let end=slice.iter().position(|&n|n==0).unwrap_or(slice.len());let text=String::from_utf16_lossy(&slice[..end]);let _=GlobalUnlock(global);Some(text)})();let _=CloseClipboard();out}}

/// A stable app id keeps the taskbar button distinct from the launcher shell.
pub fn set_app_id(){
 #[link(name="shell32")]extern "system"{fn SetCurrentProcessExplicitAppUserModelID(id:*const u16)->i32;}
 let id:Vec<u16>="DotIsle.V3.Settings".encode_utf16().chain(Some(0)).collect();
 unsafe{let _=SetCurrentProcessExplicitAppUserModelID(id.as_ptr());}
}

/// The taskbar uses ICON_BIG; the 1024 px preview PNG is not a valid HICON at that size.
pub fn set_theme_icon(window:&winit::window::Window,theme:u8){
 use raw_window_handle::{HasWindowHandle,RawWindowHandle};
 use windows::Win32::{Foundation::{HWND,WPARAM,LPARAM},UI::WindowsAndMessaging::{LoadImageW,SendMessageW,IMAGE_ICON,LR_LOADFROMFILE}};
 let Ok(handle)=window.window_handle()else{return};let RawWindowHandle::Win32(h)=handle.as_raw()else{return};let hwnd=HWND(h.hwnd.get()as _);
 let base=std::env::current_exe().ok().and_then(|p|p.parent().map(std::path::Path::to_path_buf)).unwrap_or_default();
 let path=base.join(match theme{1=>"Assets/Themes/Nothing.ico",2=>"Assets/Themes/Miuix.ico",3=>"Assets/Themes/Terminal.ico",4=>"Assets/Themes/Pixel.ico",5=>"Assets/Themes/Swiss.ico",_=>"Assets/LyricIsland-v3.ico"});if !path.is_file(){return}
 let wide:Vec<u16>=path.as_os_str().encode_wide().chain(Some(0)).collect();
 unsafe{
  if let Ok(big)=LoadImageW(None,PCWSTR(wide.as_ptr()),IMAGE_ICON,64,64,LR_LOADFROMFILE){SendMessageW(hwnd,0x0080,Some(WPARAM(1)),Some(LPARAM(big.0 as isize)));}
  if let Ok(small)=LoadImageW(None,PCWSTR(wide.as_ptr()),IMAGE_ICON,16,16,LR_LOADFROMFILE){SendMessageW(hwnd,0x0080,Some(WPARAM(0)),Some(LPARAM(small.0 as isize)));}
 }
}

/// Re-create the taskbar tab after showing the window so it adopts the window icon
/// instead of the shell's generic executable icon.
pub fn refresh_taskbar(window:&winit::window::Window){
 use raw_window_handle::{HasWindowHandle,RawWindowHandle};
 use windows::Win32::{Foundation::HWND,System::Com::{CoCreateInstance,CoInitializeEx,CLSCTX_INPROC_SERVER,COINIT_APARTMENTTHREADED},UI::Shell::{ITaskbarList,TaskbarList}};
 let Ok(handle)=window.window_handle()else{return};let RawWindowHandle::Win32(h)=handle.as_raw()else{return};let hwnd=HWND(h.hwnd.get()as _);
 unsafe{let _=CoInitializeEx(None,COINIT_APARTMENTTHREADED);if let Ok(list)=CoCreateInstance::<_,ITaskbarList>(&TaskbarList,None,CLSCTX_INPROC_SERVER){let _=list.HrInit();let _=list.DeleteTab(hwnd);let _=list.AddTab(hwnd);}}
}

pub fn backdrop(window:&winit::window::Window){use raw_window_handle::{HasWindowHandle,RawWindowHandle};use windows::Win32::{Foundation::HWND,Graphics::Dwm::*,UI::Controls::MARGINS};let Ok(handle)=window.window_handle()else{return};let RawWindowHandle::Win32(h)=handle.as_raw()else{return};unsafe{let hwnd=HWND(h.hwnd.get()as _);let margins=MARGINS{cxLeftWidth:-1,cxRightWidth:-1,cyTopHeight:-1,cyBottomHeight:-1};let _=DwmExtendFrameIntoClientArea(hwnd,&margins);let acrylic=3u32;let _=DwmSetWindowAttribute(hwnd,DWMWA_SYSTEMBACKDROP_TYPE,&acrylic as *const _ as _,4);}}

/// The owner keeps the picker modal to the control centre and preserves its directory.
pub fn choose_background(window:&winit::window::Window)->Result<Option<std::path::PathBuf>,String>{
 use raw_window_handle::{HasWindowHandle,RawWindowHandle};
 use windows::{core::{PCWSTR,PWSTR},Win32::{Foundation::HWND,UI::Controls::Dialogs::*}};
 let handle=window.window_handle().map_err(|e|e.to_string())?;let RawWindowHandle::Win32(h)=handle.as_raw()else{return Err("无法打开图片选择器".into())};
 let filter:Vec<u16>="图片 (PNG / JPEG / BMP)\0*.png;*.jpg;*.jpeg;*.bmp\0\0".encode_utf16().collect();
 let title:Vec<u16>="选择控制中心背景图片\0".encode_utf16().collect();let mut file=vec![0u16;32768];
 let mut dialog=OPENFILENAMEW{lStructSize:std::mem::size_of::<OPENFILENAMEW>()as u32,hwndOwner:HWND(h.hwnd.get()as _),lpstrFilter:PCWSTR(filter.as_ptr()),lpstrFile:PWSTR(file.as_mut_ptr()),nMaxFile:file.len()as u32,lpstrTitle:PCWSTR(title.as_ptr()),Flags:OFN_FILEMUSTEXIST|OFN_PATHMUSTEXIST|OFN_NOCHANGEDIR|OFN_EXPLORER,..Default::default()};
 unsafe{if GetOpenFileNameW(&mut dialog).as_bool(){let n=file.iter().position(|v|*v==0).unwrap_or(file.len());return Ok(Some(std::path::PathBuf::from(String::from_utf16_lossy(&file[..n]))))}let error=CommDlgExtendedError();if error.0!=0{return Err(format!("图片选择器未能打开（{}）",error.0))}}Ok(None)
}
