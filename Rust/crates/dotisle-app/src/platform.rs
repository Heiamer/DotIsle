//! Window regions and clipboard only. Vello owns all drawing.
use std::os::windows::ffi::OsStrExt;
use windows::core::PCWSTR;
use windows::Win32::{
    Foundation::HGLOBAL,
    System::{DataExchange::*, Memory::*},
};
pub fn clipboard() -> Option<String> {
    unsafe {
        OpenClipboard(None).ok()?;
        let out = (|| {
            let handle = GetClipboardData(13).ok()?;
            let global = HGLOBAL(handle.0);
            let ptr = GlobalLock(global) as *const u16;
            if ptr.is_null() {
                return None;
            }
            let length = GlobalSize(global) / 2;
            let slice = std::slice::from_raw_parts(ptr, length.min(65536));
            let end = slice.iter().position(|&n| n == 0).unwrap_or(slice.len());
            let text = String::from_utf16_lossy(&slice[..end]);
            let _ = GlobalUnlock(global);
            Some(text)
        })();
        let _ = CloseClipboard();
        out
    }
}

/// A stable app id keeps the taskbar button distinct from the launcher shell.
pub fn set_app_id() {
    #[link(name = "shell32")]
    extern "system" {
        fn SetCurrentProcessExplicitAppUserModelID(id: *const u16) -> i32;
    }
    let id: Vec<u16> = "DotIsle.V3.Settings"
        .encode_utf16()
        .chain(Some(0))
        .collect();
    unsafe {
        let _ = SetCurrentProcessExplicitAppUserModelID(id.as_ptr());
    }
}

/// The taskbar uses ICON_BIG; the 1024 px preview PNG is not a valid HICON at that size.
pub fn set_theme_icon(window: &winit::window::Window, theme: u8) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::{HWND, LPARAM, WPARAM},
        UI::WindowsAndMessaging::{LoadImageW, SendMessageW, IMAGE_ICON, LR_LOADFROMFILE},
    };
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(h.hwnd.get() as _);
    let base = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
        .unwrap_or_default();
    let path = base.join(match theme {
        1 => "Assets/Themes/Miuix.ico",
        2 => "Assets/Themes/Terminal.ico",
        3 => "Assets/Themes/Pixel.ico",
        4 => "Assets/Themes/Gallery.ico",
        _ => "Assets/LyricIsland-v3.ico",
    });
    if !path.is_file() {
        return;
    }
    let wide: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
    unsafe {
        if let Ok(big) = LoadImageW(
            None,
            PCWSTR(wide.as_ptr()),
            IMAGE_ICON,
            64,
            64,
            LR_LOADFROMFILE,
        ) {
            SendMessageW(hwnd, 0x0080, Some(WPARAM(1)), Some(LPARAM(big.0 as isize)));
        }
        if let Ok(small) = LoadImageW(
            None,
            PCWSTR(wide.as_ptr()),
            IMAGE_ICON,
            16,
            16,
            LR_LOADFROMFILE,
        ) {
            SendMessageW(
                hwnd,
                0x0080,
                Some(WPARAM(0)),
                Some(LPARAM(small.0 as isize)),
            );
        }
    }
}

/// Re-create the taskbar tab after showing the window so it adopts the window icon
/// instead of the shell's generic executable icon.
pub fn refresh_taskbar(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{
        Foundation::HWND,
        System::Com::{
            CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{ITaskbarList, TaskbarList},
    };
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    let hwnd = HWND(h.hwnd.get() as _);
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        if let Ok(list) =
            CoCreateInstance::<_, ITaskbarList>(&TaskbarList, None, CLSCTX_INPROC_SERVER)
        {
            let _ = list.HrInit();
            let _ = list.DeleteTab(hwnd);
            let _ = list.AddTab(hwnd);
        }
    }
}

pub fn backdrop(window: &winit::window::Window) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::Win32::{Foundation::HWND, Graphics::Dwm::*, UI::Controls::MARGINS};
    let Ok(handle) = window.window_handle() else {
        return;
    };
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return;
    };
    unsafe {
        let hwnd = HWND(h.hwnd.get() as _);
        let margins = MARGINS {
            cxLeftWidth: -1,
            cxRightWidth: -1,
            cyTopHeight: -1,
            cyBottomHeight: -1,
        };
        let _ = DwmExtendFrameIntoClientArea(hwnd, &margins);
        let acrylic = 3u32;
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_SYSTEMBACKDROP_TYPE,
            &acrylic as *const _ as _,
            4,
        );
    }
}

/// The owner keeps the picker modal to the control centre and preserves its directory.
pub fn choose_background(
    window: &winit::window::Window,
) -> Result<Option<std::path::PathBuf>, String> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows::{
        core::{PCWSTR, PWSTR},
        Win32::{Foundation::HWND, UI::Controls::Dialogs::*},
    };
    let handle = window.window_handle().map_err(|e| e.to_string())?;
    let RawWindowHandle::Win32(h) = handle.as_raw() else {
        return Err("无法打开图片选择器".into());
    };
    let filter: Vec<u16> = "图片 (PNG / JPEG / BMP)\0*.png;*.jpg;*.jpeg;*.bmp\0\0"
        .encode_utf16()
        .collect();
    let title: Vec<u16> = "选择控制中心背景图片\0".encode_utf16().collect();
    let mut file = vec![0u16; 32768];
    let mut dialog = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: HWND(h.hwnd.get() as _),
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: PWSTR(file.as_mut_ptr()),
        nMaxFile: file.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST | OFN_PATHMUSTEXIST | OFN_NOCHANGEDIR | OFN_EXPLORER,
        ..Default::default()
    };
    unsafe {
        if GetOpenFileNameW(&mut dialog).as_bool() {
            let n = file.iter().position(|v| *v == 0).unwrap_or(file.len());
            return Ok(Some(std::path::PathBuf::from(String::from_utf16_lossy(
                &file[..n],
            ))));
        }
        let error = CommDlgExtendedError();
        if error.0 != 0 {
            return Err(format!("图片选择器未能打开（{}）", error.0));
        }
    }
    Ok(None)
}

pub struct CenterGuard(windows::Win32::Foundation::HANDLE);
impl Drop for CenterGuard {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.0);
        }
    }
}
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
pub fn process_alive(pid: u32) -> bool {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::Threading::{GetExitCodeProcess, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
    };
    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut code = 0;
        let active = GetExitCodeProcess(handle, &mut code).is_ok() && code == 259;
        let _ = CloseHandle(handle);
        active
    }
}
fn native_handle(window: &winit::window::Window) -> isize {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    window
        .window_handle()
        .ok()
        .and_then(|h| match h.as_raw() {
            RawWindowHandle::Win32(h) => Some(h.hwnd.get()),
            _ => None,
        })
        .unwrap_or(0)
}
#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn SetWindowPos(hwnd: isize, after: isize, x: i32, y: i32, cx: i32, cy: i32, flags: u32)
        -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn GetClassNameW(hwnd: isize, class: *mut u16, count: i32) -> i32;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetCapture(hwnd: isize) -> isize;
    fn ReleaseCapture() -> i32;
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
