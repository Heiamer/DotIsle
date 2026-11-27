use std::{
    ffi::c_void,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

// 仅使用 Windows 系统库；句柄不跨线程传递其所有权。
pub type Hwnd = *mut c_void;
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}
#[repr(C)]
#[derive(Default)]
pub struct WinRect {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}
#[repr(C)]
#[derive(Default)]
pub struct Msg {
    pub hwnd: Hwnd,
    pub message: u32,
    pub wparam: usize,
    pub lparam: isize,
    pub time: u32,
    pub pt: Point,
    pub private: u32,
}
#[link(name = "user32")]
unsafe extern "system" {
    fn PeekMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32, remove: u32) -> i32;
    fn GetMessageW(msg: *mut Msg, hwnd: Hwnd, min: u32, max: u32) -> i32;
    fn RegisterHotKey(hwnd: Hwnd, id: i32, mods: u32, key: u32) -> i32;
    fn UnregisterHotKey(hwnd: Hwnd, id: i32) -> i32;
    pub fn PostThreadMessageW(tid: u32, msg: u32, w: usize, l: isize) -> i32;
    fn GetAsyncKeyState(key: i32) -> i16;
    pub fn SetWindowPos(hwnd: Hwnd, after: Hwnd, x: i32, y: i32, w: i32, h: i32, flags: u32)
    -> i32;
    pub fn ShowWindow(hwnd: Hwnd, cmd: i32) -> i32;
    fn SetWindowRgn(hwnd: Hwnd, rgn: *mut c_void, redraw: i32) -> i32;
    pub fn GetForegroundWindow() -> Hwnd;
    pub fn SetForegroundWindow(hwnd: Hwnd) -> i32;
    pub fn GetWindowLongPtrW(hwnd: Hwnd, index: i32) -> isize;
    pub fn SetWindowLongPtrW(hwnd: Hwnd, index: i32, value: isize) -> isize;
    pub fn GetWindowRect(hwnd: Hwnd, rect: *mut WinRect) -> i32;
    pub fn SystemParametersInfoW(action: u32, param: u32, data: *mut c_void, flags: u32) -> i32;
    pub fn OpenClipboard(hwnd: Hwnd) -> i32;
    pub fn CloseClipboard() -> i32;
    pub fn GetClipboardData(format: u32) -> *mut c_void;
    pub fn GetClipboardSequenceNumber() -> u32;
    pub fn EmptyClipboard() -> i32;
    pub fn SetClipboardData(format: u32, data: *mut c_void) -> *mut c_void;
    pub fn CreateWindowExW(
        ex: u32,
        class: *const u16,
        title: *const u16,
        style: u32,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
        parent: Hwnd,
        menu: Hwnd,
        instance: Hwnd,
        param: *mut c_void,
    ) -> Hwnd;
    pub fn DestroyWindow(hwnd: Hwnd) -> i32;
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateRectRgn(l: i32, t: i32, r: i32, b: i32) -> *mut c_void;
    fn CombineRgn(dst: *mut c_void, src1: *mut c_void, src2: *mut c_void, mode: i32) -> i32;
    pub fn DeleteObject(obj: *mut c_void) -> i32;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetCurrentThreadId() -> u32;
    pub fn GlobalAlloc(flags: u32, bytes: usize) -> *mut c_void;
    pub fn GlobalLock(mem: *mut c_void) -> *mut c_void;
    pub fn GlobalUnlock(mem: *mut c_void) -> i32;
    pub fn GlobalFree(mem: *mut c_void) -> *mut c_void;
}
#[link(name = "kernel32")]
unsafe extern "system" {
    pub fn GlobalSize(mem: *mut c_void) -> usize;
}

pub type Wake = Arc<dyn Fn() + Send + Sync>;
#[derive(Default)]
pub struct HotkeyEvents {
    pub sidebar: AtomicU64,
    pub edit: AtomicU64,
}
#[derive(Default)]
pub struct EdgeDetector {
    down: bool,
}
impl EdgeDetector {
    pub fn sample(&mut self, mods_ok: bool, key: bool, other: bool) -> bool {
        let now = mods_ok && key;
        let trigger = now && !self.down && !other;
        self.down = now;
        trigger
    }
}
pub struct HotkeyListener {
    pub events: Arc<HotkeyEvents>,
    stop: Arc<AtomicBool>,
    tid: u32,
    sidebar_native: Arc<AtomicBool>,
    edit_native: Arc<AtomicBool>,
    join: Option<JoinHandle<()>>,
}
impl HotkeyListener {
    #[allow(dead_code)]
    pub fn start(wake: Wake) -> Result<Self, String> {
        Self::start_with(wake, 0x4001, 0xC0, 0x4001, 0x45)
    }

    pub fn start_with(wake: Wake, sidebar_mods: u32, sidebar_vk: u32, edit_mods: u32, edit_vk: u32) -> Result<Self, String> {
        let events = Arc::new(HotkeyEvents::default());
        let stop = Arc::new(AtomicBool::new(false));
        let sidebar_native = Arc::new(AtomicBool::new(false));
        let edit_native = Arc::new(AtomicBool::new(false));
        let (e, s, sn, en) = (events.clone(), stop.clone(), sidebar_native.clone(), edit_native.clone());
        let (tx, rx) = mpsc::sync_channel(1);
        let join = thread::Builder::new()
            .name("allegro-skia-hotkeys".into())
            .spawn(move || unsafe {
                let mut msg = Msg::default();
                PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 0);
                let tid = GetCurrentThreadId();
                let mods = [sidebar_mods, edit_mods];
                let keys = [sidebar_vk, edit_vk];
                let mut registered = [false; 2];
                for i in 0..2 {
                    registered[i] = keys[i] != 0 &&
                        RegisterHotKey(std::ptr::null_mut(), 0x5A10 + i as i32, mods[i], keys[i])
                            != 0;
                }
                sn.store(registered[0], Ordering::Release);
                en.store(registered[1], Ordering::Release);
                thread::scope(|_| {
                    let _ = tx.send(tid);
                    loop {
                        let status = GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0);
                        if status <= 0 || s.load(Ordering::Acquire) {
                            break;
                        }
                        if msg.message == 0x312 {
                            match msg.wparam {
                                0x5A10 => {
                                    e.sidebar.fetch_add(1, Ordering::Release);
                                    wake();
                                }
                                0x5A11 => {
                                    e.edit.fetch_add(1, Ordering::Release);
                                    wake();
                                }
                                _ => {}
                            }
                        }
                    }
                    s.store(true, Ordering::Release);
                });
                for (i, ok) in registered.iter().enumerate() {
                    if *ok {
                        UnregisterHotKey(std::ptr::null_mut(), 0x5A10 + i as i32);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let tid = rx.recv().map_err(|e| e.to_string())?;
        Ok(Self {
            events,
            stop,
            tid,
            sidebar_native,
            edit_native,
            join: Some(join),
        })
    }

    pub fn sidebar_registered(&self) -> bool {
        self.sidebar_native.load(Ordering::Acquire)
    }

    pub fn edit_registered(&self) -> bool { self.edit_native.load(Ordering::Acquire) }

    pub fn rebind(&mut self, wake: Wake, mods: u32, vk: u32, edit_mods: u32, edit_vk: u32) -> Result<(), String> {
        self.stop.store(true, Ordering::Release);
        unsafe {
            PostThreadMessageW(self.tid, 0x12, 0, 0);
        }
        if let Some(t) = self.join.take() {
            t.thread().unpark();
            let _ = t.join();
        }
        *self = Self::start_with(wake, mods, vk, edit_mods, edit_vk)?;
        Ok(())
    }

    pub fn take(&self) -> (u64, u64) {
        (
            self.events.sidebar.swap(0, Ordering::AcqRel),
            self.events.edit.swap(0, Ordering::AcqRel),
        )
    }
}
impl Drop for HotkeyListener {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        unsafe {
            PostThreadMessageW(self.tid, 0x12, 0, 0);
        }
        if let Some(t) = self.join.take() {
            t.thread().unpark();
            let _ = t.join();
        }
    }
}
// 区域之外由系统直接穿透，避免透明全屏窗口吞掉左屏点击。
pub fn input_region(
    hwnd: Hwnd,
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
) -> Result<(), String> {
    unsafe {
        let r = CreateRectRgn(left, top, right, bottom);
        if r.is_null() {
            return Err("创建输入区域失败".into());
        }
        if SetWindowRgn(hwnd, r, 1) == 0 {
            DeleteObject(r);
            return Err("更新输入区域失败".into());
        }
        Ok(())
    }
}

/// 合并多个物理像素矩形作为窗口可点击输入区域，其余空白完全穿透桌面。
pub fn input_regions(hwnd: Hwnd, rects: &[(i32, i32, i32, i32)]) -> Result<(), String> {
    const RGN_OR: i32 = 2;
    unsafe {
        let mut iter = rects.iter().copied();
        let acc = match iter.next() {
            Some((l, t, r, b)) => {
                let h = CreateRectRgn(l, t, r, b);
                if h.is_null() {
                    return Err("创建主输入区域失败".into());
                }
                h
            }
            None => {
                let h = CreateRectRgn(0, 0, 0, 0);
                if h.is_null() {
                    return Err("创建空输入区域失败".into());
                }
                h
            }
        };
        for (l, t, r, b) in iter {
            let part = CreateRectRgn(l, t, r, b);
            if !part.is_null() {
                CombineRgn(acc, acc, part, RGN_OR);
                DeleteObject(part);
            }
        }
        if SetWindowRgn(hwnd, acc, 1) == 0 {
            DeleteObject(acc);
            return Err("更新组合输入区域失败".into());
        }
        Ok(())
    }
}
pub fn layered(hwnd: Hwnd) {
    unsafe {
        let style = GetWindowLongPtrW(hwnd, -20);
        SetWindowLongPtrW(hwnd, -20, style | 0x80000 | 0x08000000 | 0x80);
    }
}
pub fn z_order(hwnd: Hwnd, top: bool) {
    unsafe {
        SetWindowPos(
            hwnd,
            if top { -1isize as Hwnd } else { 1isize as Hwnd },
            0,
            0,
            0,
            0,
            0x13,
        );
    }
}
pub fn allow_focus(hwnd: Hwnd, allow: bool) {
    unsafe {
        let s = GetWindowLongPtrW(hwnd, -20);
        SetWindowLongPtrW(
            hwnd,
            -20,
            if allow {
                s & !0x08000000
            } else {
                s | 0x08000000
            },
        );
        if allow {
            SetForegroundWindow(hwnd);
        }
    }
}
pub fn restore_focus(hwnd: Hwnd, previous: Hwnd) {
    unsafe {
        if GetForegroundWindow() == hwnd && !previous.is_null() {
            SetForegroundWindow(previous);
        }
        allow_focus(hwnd, false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fallback_edges() {
        let mut e = EdgeDetector::default();
        assert!(!e.sample(false, true, false));
        assert!(e.sample(true, true, false));
        assert!(!e.sample(true, true, false));
        assert!(!e.sample(true, true, true));
        assert!(!e.sample(true, true, false));
        assert!(!e.sample(false, false, false));
        assert!(e.sample(true, true, false));
    }
    #[test]
    fn custom_edit_rebind_releases_previous_chord() {
        let free = |vk| unsafe {
            let ok=RegisterHotKey(std::ptr::null_mut(),0x5b70,0x4007,vk)!=0;
            if ok { UnregisterHotKey(std::ptr::null_mut(),0x5b70); }
            ok
        };
        let keys:Vec<u32>=(0x70..=0x7b).filter(|vk|free(*vk)).take(2).collect();
        assert_eq!(keys.len(),2);
        let (tx,rx)=mpsc::channel();
        let wake:Wake=Arc::new(move || {let _=tx.send(());});
        let mut l=HotkeyListener::start_with(wake.clone(),0x4001,0,0x4007,keys[0]).unwrap();
        assert!(l.edit_registered());
        assert!(!l.sidebar_registered());
        assert!(!free(keys[0]));
        unsafe {assert_ne!(PostThreadMessageW(l.tid,0x312,0x5A11,0),0);}
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(l.take(),(0,1));
        l.rebind(wake,0x4001,0,0x4007,keys[1]).unwrap();
        assert!(l.edit_registered());
        assert!(free(keys[0]));
        assert!(!free(keys[1]));
        drop(l);
        assert!(free(keys[1]));
    }
    #[test]
    fn native_messages_and_drop() {
        let (tx, rx) = mpsc::channel();
        let l = HotkeyListener::start(Arc::new(move || {
            let _ = tx.send(());
        }))
        .unwrap();
        unsafe {
            assert_ne!(PostThreadMessageW(l.tid, 0x312, 0x5A10, 0), 0);
            assert_ne!(PostThreadMessageW(l.tid, 0x312, 0x5A11, 0), 0);
        }
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(l.take(), (1, 1));
        assert_eq!(l.take(), (0, 0));
        let now = std::time::Instant::now();
        drop(l);
        assert!(now.elapsed() < Duration::from_secs(2));
    }
}
