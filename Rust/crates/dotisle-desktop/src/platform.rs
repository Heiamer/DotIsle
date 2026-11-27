use raw_window_handle::{HasWindowHandle,RawWindowHandle};
use windows::core::PCWSTR;
fn native_handle(w:&winit::window::Window)->isize{match w.window_handle().unwrap().as_raw(){RawWindowHandle::Win32(h)=>h.hwnd.get(),_=>0}}
#[link(name="user32")]extern "system"{
 fn FindWindowW(class:*const u16,title:*const u16)->isize;
 fn SetWindowLongPtrW(hwnd:isize,index:i32,value:isize)->isize;
 fn GetWindowLongPtrW(hwnd:isize,index:i32)->isize;
 fn GetWindow(hwnd:isize,cmd:u32)->isize;
 fn IsWindow(hwnd:isize)->i32;
 fn IsWindowVisible(hwnd:isize)->i32;
 fn GetClassNameW(hwnd:isize,name:*mut u16,count:i32)->i32;
 fn SetWindowPos(hwnd:isize,after:isize,x:i32,y:i32,w:i32,h:i32,flags:u32)->i32;
 fn SetCapture(hwnd:isize)->isize;
 fn ReleaseCapture()->i32;
 fn GetForegroundWindow()->isize;
 fn SetForegroundWindow(hwnd:isize)->i32;
 fn SetFocus(hwnd:isize)->isize;
 fn GetWindowThreadProcessId(hwnd:isize,pid:*mut u32)->u32;
 fn AttachThreadInput(attach:u32,attach_to:u32,on:i32)->i32;
 fn BringWindowToTop(hwnd:isize)->i32;
 fn ShowWindow(hwnd:isize,cmd:i32)->i32;
 fn GetAsyncKeyState(vkey:i32)->i16;
}
#[link(name="kernel32")]extern "system"{
 fn GetCurrentThreadId()->u32;
 fn LoadLibraryW(name:*const u16)->isize;
 fn GetProcAddress(module:isize,name:*const u8)->isize;
 fn FreeLibrary(module:isize)->i32;
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
pub fn activate(window: &winit::window::Window) {
    unsafe {
        let hwnd = native_handle(window);
        // Editing puts the card above applications so the keyboard and IME can reach it.
        SetWindowPos(hwnd, -1, 0, 0, 0, 0, 0x0001 | 0x0002 | 0x0040);
        BringWindowToTop(hwnd);
        let foreground = GetForegroundWindow();
        let foreground_thread = if foreground != 0 {
            GetWindowThreadProcessId(foreground, std::ptr::null_mut())
        } else {
            0
        };
        let current = GetCurrentThreadId();
        if foreground_thread != 0 && foreground_thread != current {
            AttachThreadInput(current, foreground_thread, 1);
        }
        SetForegroundWindow(hwnd);
        SetFocus(hwnd);
        if foreground_thread != 0 && foreground_thread != current {
            AttachThreadInput(current, foreground_thread, 0);
        }
    }
}
pub fn sink(window: &winit::window::Window) {
    unsafe {
        SetWindowPos(native_handle(window), 1, 0, 0, 0, 0, 0x0001 | 0x0002);
    }
}

/// 以不抢占前台焦点的方式强制显示桌面画布窗口。
pub fn show_inactive(window: &winit::window::Window) {
    const SW_SHOWNOACTIVATE: i32 = 4;
    unsafe {
        let hwnd = native_handle(window);
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        SetWindowPos(hwnd, 1, 0, 0, 0, 0, 0x0040 | 0x0010 | 0x0001 | 0x0002);
    }
}
pub fn remove_dwm_frame(window: &winit::window::Window) {
    unsafe {
        // Windows 11 adds its own rounded clip, accent border and shadow to top-level
        // windows; on a transparent desktop card those read as a second corner and a
        // white halo outside the real card. Load dwmapi.dll lazily so the GNU link
        // does not require an import library that the toolchain does not ship.
        let module_name: Vec<u16> = "dwmapi.dll".encode_utf16().chain(Some(0)).collect();
        let module = LoadLibraryW(module_name.as_ptr());
        if module == 0 {
            return;
        }
        let address = GetProcAddress(module, b"DwmSetWindowAttribute\0".as_ptr());
        if address != 0 {
            let set: extern "system" fn(isize, u32, *const std::ffi::c_void, u32) -> i32 =
                std::mem::transmute(address);
            let hwnd = native_handle(window);
            let corner: u32 = 1; // DWMWCP_DONOTROUND
            let border: u32 = 0xFFFF_FFFE; // DWMWA_COLOR_NONE
            let rendering: u32 = 1; // DWMNCRP_DISABLED
            let size = std::mem::size_of::<u32>() as u32;
            let _ = set(hwnd, 33, &corner as *const _ as *const _, size);
            let _ = set(hwnd, 34, &border as *const _ as *const _, size);
            let _ = set(hwnd, 2, &rendering as *const _ as *const _, size);
        }
        let _ = FreeLibrary(module);
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

/// 为单个全屏合批画布设置精确可交互选区（其余桌面空白处 100% 鼠标穿透）。
/// 当 `allow_full` 为 `true`（热键仪表盘开启或正在拖拽）时解除选区裁剪。
pub fn apply_input_region(
    window: &winit::window::Window,
    rects: &[(i32, i32, i32, i32, i32)],
    allow_full: bool,
) {
    use windows::Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, SetWindowRgn, HRGN,
            RGN_OR,
        },
    };
    unsafe {
        let hwnd = HWND(native_handle(window) as *mut std::ffi::c_void);
        if hwnd.0.is_null() {
            return;
        }
        if allow_full {
            let _ = SetWindowRgn(hwnd, None, true);
            return;
        }
        if rects.is_empty() {
            let empty = CreateRectRgn(0, 0, 0, 0);
            if SetWindowRgn(hwnd, Some(empty), true) == 0 {
                let _ = DeleteObject(empty.into());
            }
            return;
        }
        let make_rgn = |r: &(i32, i32, i32, i32, i32)| -> HRGN {
            let (x0, y0, x1, y1, rad) = *r;
            if rad > 1 {
                let d = rad * 2 + 4;
                CreateRoundRectRgn(x0 - 2, y0 - 2, x1 + 2, y1 + 2, d, d)
            } else {
                CreateRectRgn(x0 - 2, y0 - 2, x1 + 2, y1 + 2)
            }
        };
        let acc = make_rgn(&rects[0]);
        for rect in &rects[1..] {
            let sub = make_rgn(rect);
            let _ = CombineRgn(Some(acc), Some(acc), Some(sub), RGN_OR);
            let _ = DeleteObject(sub.into());
        }
        // SetWindowRgn 成功后系统接管 acc 句柄所有权，不可手动 DeleteObject。
        if SetWindowRgn(hwnd, Some(acc), true) == 0 {
            let _ = DeleteObject(acc.into());
        }
    }
}

/// 侧栏每帧自行绘制，无需由 HRGN 更新再发 WM_PAINT，避免动画空转。
pub fn apply_sidebar_region(window: &winit::window::Window, region: (i32, i32, i32, i32, i32)) -> bool {
    use windows::Win32::{Foundation::HWND, Graphics::Gdi::{CreateRoundRectRgn, DeleteObject, SetWindowRgn}};
    let (x0, y0, x1, y1, r) = region;
    unsafe {
        let hwnd = HWND(native_handle(window) as _);
        let d = r * 2 + 4;
        let handle = CreateRoundRectRgn(x0 - 2, y0 - 2, x1 + 2, y1 + 2, d, d);
        if handle.is_invalid() { return false; }
        if SetWindowRgn(hwnd, Some(handle), false) == 0 {
            let _ = DeleteObject(handle.into());
            return false;
        }
        true
    }
}

/// 在全屏编辑模式（置顶）与日常桌面壁纸层（置底）之间切换窗口 Z 序。
pub fn set_dashboard_layer(window: &winit::window::Window, topmost: bool) {
    if topmost {
        apply_input_region(window, &[], true);
        activate(window);
    } else {
        sink(window);
        desktop_floor(window);
    }
}

/// 将独立右侧边栏窗口置顶显示：默认 `focus = false` 以免抢占用户正在操作的电脑窗口焦点；
/// 点击搜索框时传入 `focus = true` 激活键盘与中文 IME。
pub fn show_sidebar_topmost(window: &winit::window::Window, focus: bool) {
    const SW_SHOWNOACTIVATE: i32 = 4;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOACTIVATE: u32 = 0x0010;
    const SWP_SHOWWINDOW: u32 = 0x0040;
    unsafe {
        let hwnd = native_handle(window);
        // 保留 NOACTIVATE：只有搜索框使用显式前台激活，其他鼠标点击不抢焦点。
        const GWL_EXSTYLE: i32 = -20;
        const WS_EX_NOACTIVATE: isize = 0x0800_0000;
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        if style & WS_EX_NOACTIVATE == 0 {
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style | WS_EX_NOACTIVATE);
        }
        ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let flags = SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW | SWP_NOACTIVATE;
        SetWindowPos(hwnd, -1, 0, 0, 0, 0, flags);
        // 此处只在真实搜索框点击时调用；不绑定其他进程的输入队列。
        if focus && SetForegroundWindow(hwnd) != 0 {
            SetFocus(hwnd);
        }
    }
}

pub fn foreground_window() -> isize { unsafe { GetForegroundWindow() } }

/// 用户结束输入时交还焦点；若用户已切到第三方窗口，绝不抢回来。
pub fn restore_sidebar_focus(window: &winit::window::Window, previous: isize) {
    unsafe {
        if previous != 0 && previous != native_handle(window) && GetForegroundWindow() == native_handle(window)
            && IsWindow(previous) != 0
        {
            SetForegroundWindow(previous);
        }
    }
}

/// 隐藏窗口（用于彻底收起独立右侧边栏）。
pub fn hide_window(window: &winit::window::Window) {
    unsafe {
        ShowWindow(native_handle(window), 0);
    }
}

use std::{
    sync::{Arc, atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering}},
    thread::{self, JoinHandle},
    time::Duration,
};

const HOTKEYS: [u32; 2] = [0xC0, 0x45]; // VK_OEM_3、VK_E

#[derive(Default)]
struct HotkeyEvents([AtomicU64; 2]);

impl HotkeyEvents {
    fn push(&self, key: usize) {
        self.0[key].fetch_add(1, Ordering::Relaxed);
    }

    fn take(&self) -> [u64; 2] {
        self.0.each_ref().map(|n| n.swap(0, Ordering::Relaxed))
    }
}

#[derive(Default)]
struct HotkeyControl {
    events: HotkeyEvents,
    stopping: AtomicBool,
    thread_id: AtomicU32,
}

/// 线程消息无需 HWND；注册成功的组合键不参与轮询。
pub struct HotkeyListener {
    control: Arc<HotkeyControl>,
    worker: Option<JoinHandle<()>>,
}

impl HotkeyListener {
    pub fn start() -> Self {
        let control = Arc::new(HotkeyControl::default());
        let worker_control = control.clone();
        let worker = thread::Builder::new().name("allegro-hotkeys".into())
            .spawn(move || listen_hotkeys(&worker_control))
            .map_err(|err| eprintln!("Allegro hotkey thread: {err}"))
            .ok();
        Self { control, worker }
    }

    /// 取走累计次数；即使唤醒消息合并，连续两次切换也不会被当作一次。
    pub fn take(&self) -> [u64; 2] {
        self.control.events.take()
    }
}

impl Drop for HotkeyListener {
    fn drop(&mut self) {
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_QUIT};
        self.control.stopping.store(true, Ordering::SeqCst);
        let id = self.control.thread_id.load(Ordering::SeqCst);
        if id != 0 {
            // 队列先于 ID 发布；若线程尚未发布 ID，它会自行看到停止标记。
            unsafe { let _ = PostThreadMessageW(id, WM_QUIT, Default::default(), Default::default()); }
        }
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

fn listen_hotkeys(control: &HotkeyControl) {
    use windows::Win32::UI::{
        Input::KeyboardAndMouse::{RegisterHotKey, UnregisterHotKey, MOD_ALT, MOD_NOREPEAT},
        WindowsAndMessaging::{GetMessageW, PeekMessageW, MSG, PM_NOREMOVE, WM_HOTKEY},
    };
    let mut msg = MSG::default();
    unsafe { let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE); }
    control.thread_id.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
    if control.stopping.load(Ordering::SeqCst) {
        return;
    }
    let failed = std::array::from_fn(|i| unsafe {
        match RegisterHotKey(None, i as i32 + 1, MOD_ALT | MOD_NOREPEAT, HOTKEYS[i]) {
            Ok(()) => false,
            Err(err) => {
                eprintln!("Allegro hotkey VK={:#x}: {err}; using fallback", HOTKEYS[i]);
                true
            }
        }
    });
    thread::scope(|scope| {
        let fallback = failed.iter().any(|&v| v).then(|| {
            thread::Builder::new().name("allegro-hotkey-fallback".into())
                .spawn_scoped(scope, || poll_failed_hotkeys(control, failed))
                .map_err(|err| eprintln!("Allegro hotkey fallback thread: {err}"))
                .ok()
        }).flatten();
        while !control.stopping.load(Ordering::SeqCst)
            && unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0
        {
            if msg.message == WM_HOTKEY && (1..=2).contains(&msg.wParam.0) {
                control.events.push(msg.wParam.0 - 1);
                crate::wake();
            }
        }
        control.stopping.store(true, Ordering::SeqCst);
        if let Some(worker) = fallback {
            worker.thread().unpark();
            let _ = worker.join();
        }
    });
    for (i, failed) in failed.into_iter().enumerate() {
        if !failed {
            unsafe { let _ = UnregisterHotKey(None, i as i32 + 1); }
        }
    }
    control.thread_id.store(0, Ordering::SeqCst);
}

fn pressed_edge(previous: &mut bool, down: bool) -> bool {
    let was_down = std::mem::replace(previous, down);
    down && !was_down
}

fn poll_failed_hotkeys(control: &HotkeyControl, failed: [bool; 2]) {
    let mut previous = [false; 2];
    while !control.stopping.load(Ordering::SeqCst) {
        // 与 MOD_ALT 一致；避免把 AltGr（Ctrl+Alt）或其他组合误当成独占热键。
        let alt = unsafe { GetAsyncKeyState(0x12) < 0 &&
            [0x10, 0x11, 0x5B, 0x5C].into_iter().all(|vk| GetAsyncKeyState(vk) >= 0) };
        for (i, vk) in HOTKEYS.into_iter().enumerate() {
            if failed[i] && pressed_edge(&mut previous[i], alt && unsafe { GetAsyncKeyState(vk as i32) < 0 }) {
                control.events.push(i);
                crate::wake();
            }
        }
        thread::park_timeout(Duration::from_millis(25));
    }
}

#[cfg(test)]
mod hotkey_tests {
    use super::*;

    #[test]
    fn counters_keep_repeated_and_concurrent_presses() {
        let events = HotkeyEvents::default();
        thread::scope(|scope| {
            for i in [0, 0, 1] {
                let events = &events;
                scope.spawn(move || { for _ in 0..100 { events.push(i); } });
            }
        });
        assert_eq!(events.take(), [200, 100]);
        assert_eq!(events.take(), [0, 0]);
    }

    #[test]
    fn native_listener_delivers_thread_message_and_joins() {
        use windows::Win32::UI::WindowsAndMessaging::{PostThreadMessageW, WM_HOTKEY};
        let listener = HotkeyListener::start();
        let until = std::time::Instant::now() + Duration::from_secs(3);
        let id = loop {
            let id = listener.control.thread_id.load(Ordering::SeqCst);
            if id != 0 { break id; }
            assert!(std::time::Instant::now() < until, "hotkey thread did not start");
            thread::sleep(Duration::from_millis(1));
        };
        unsafe { PostThreadMessageW(id, WM_HOTKEY, windows::Win32::Foundation::WPARAM(1), Default::default()).unwrap(); }
        loop {
            if listener.take()[0] == 1 { break; }
            assert!(std::time::Instant::now() < until, "WM_HOTKEY was not delivered");
            thread::sleep(Duration::from_millis(1));
        }
        let control = listener.control.clone();
        drop(listener);
        assert_eq!(control.thread_id.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn held_fallback_key_does_not_repeat() {
        let mut state = false;
        assert_eq!([true, true, false, true, false].map(|down| pressed_edge(&mut state, down)),
                   [true, false, false, true, false]);
    }
}

/// 使用 Windows Shell 默认关联应用打开本地文件或文件夹。
pub fn open_shell_path(raw: &str) {
    use windows::Win32::{UI::Shell::ShellExecuteW, UI::WindowsAndMessaging::SW_SHOWNORMAL};
    if raw.trim().is_empty() || raw.contains('\0') {
        return;
    }
    let op: Vec<u16> = "open\0".encode_utf16().collect();
    let file: Vec<u16> = raw.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let _ = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(file.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// 保持独立桌面引擎的标准交互优先级（仅靠空闲 `WaitUntil` 节流实现低占用，避免强制小核锁频导致拖拽和动画卡顿）。
pub fn enable_efficiency_mode() {
    // Windows EcoQoS (`PROCESS_POWER_THROTTLING_EXECUTION_SPEED`) 会把进程强锁在最低频小核，
    // 严重拖慢 Vello / WGPU 全屏透明画布的实时合成与拖拽，故保持常规调度。
}