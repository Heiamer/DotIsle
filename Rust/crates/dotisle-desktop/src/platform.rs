use raw_window_handle::{HasWindowHandle,RawWindowHandle};
use windows::core::PCWSTR;
fn native_handle(w:&winit::window::Window)->isize{match w.window_handle().unwrap().as_raw(){RawWindowHandle::Win32(h)=>h.hwnd.get(),_=>0}}
#[link(name="user32")]extern "system"{
 fn FindWindowW(class:*const u16,title:*const u16)->isize;
 fn SetWindowLongPtrW(hwnd:isize,index:i32,value:isize)->isize;
 fn GetWindowLongPtrW(hwnd:isize,index:i32)->isize;
 fn GetWindow(hwnd:isize,cmd:u32)->isize;
 fn IsWindowVisible(hwnd:isize)->i32;
 fn GetClassNameW(hwnd:isize,name:*mut u16,count:i32)->i32;
 fn SetWindowPos(hwnd:isize,after:isize,x:i32,y:i32,w:i32,h:i32,flags:u32)->i32;
 fn SetCapture(hwnd:isize)->isize;
 fn ReleaseCapture()->i32;
}
pub struct CenterGuard(windows::Win32::Foundation::HANDLE);
impl Drop for CenterGuard{fn drop(&mut self){unsafe{let _=windows::Win32::Foundation::CloseHandle(self.0);}}}
pub fn center_instance(data: &std::path::Path) -> Result<Option<CenterGuard>, String> {
    use windows::Win32::{
        Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS},
        System::Threading::CreateMutexW,
    };
    let name = dotisle_domain::desktop_widgets::host_key(data)
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    unsafe {
        let handle = CreateMutexW(None, false, PCWSTR(name.as_ptr())).map_err(|e| e.to_string())?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            Ok(None)
        } else {
            Ok(Some(CenterGuard(handle)))
        }
    }
}
pub fn desktop_owner(window: &winit::window::Window) {
    unsafe {
        let progman: Vec<u16> = "Progman".encode_utf16().chain(Some(0)).collect();
        let owner = FindWindowW(progman.as_ptr(), std::ptr::null());
        if owner != 0 {
            SetWindowLongPtrW(native_handle(window), -8, owner);
        }
    }
}
pub fn capture_pointer(window: &winit::window::Window, on: bool) {
    unsafe {
        if on {
            SetCapture(native_handle(window));
        } else {
            ReleaseCapture();
        }
    }
}
pub fn desktop_floor(window: &winit::window::Window) {
    unsafe {
        // Walk upward from the desktop to the lowest visible application. Owned popup
        // windows avoid the cross-process DPI reset that SetParent would cause.
        let hwnd = native_handle(window);
        let mut cursor = GetWindow(hwnd, 1);
        let mut after = 0;
        let mut visited = 0;
        while cursor != 0 && visited < 4096 {
            visited += 1;
            let mut class = [0u16; 128];
            let length = GetClassNameW(cursor, class.as_mut_ptr(), 128).max(0) as usize;
            let name = String::from_utf16_lossy(&class[..length]);
            if cursor != hwnd
                && IsWindowVisible(cursor) != 0
                && name != "Progman"
                && name != "WorkerW"
                && GetWindowLongPtrW(cursor, -8) != GetWindowLongPtrW(hwnd, -8)
            {
                after = cursor;
                break;
            }
            cursor = GetWindow(cursor, 3);
        }
        if after != 0 && after != hwnd {
            SetWindowPos(hwnd, after, 0, 0, 0, 0, 0x0010 | 0x0001 | 0x0002);
        } else {
            SetWindowPos(hwnd, 1, 0, 0, 0, 0, 0x0010 | 0x0001 | 0x0002);
        }
    }
}