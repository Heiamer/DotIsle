//! 多显示器枚举与目标显示器解析，支持按显示器名称精准定位灵动岛。
use serde_json::{json, Value};
use windows::Win32::{
    Foundation::{LPARAM, RECT},
    Graphics::Gdi::{
        EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, MonitorFromWindow, HDC, HMONITOR,
        MONITORINFOEXW, MONITOR_DEFAULTTOPRIMARY, MONITOR_DEFAULTTONEAREST,
    },
};

#[derive(Clone, Debug, PartialEq)]
pub struct DisplayInfo {
    pub id: String,
    pub label: String,
    pub primary: bool,
    pub work: RECT,
    pub monitor: RECT,
    pub raw_hmonitor: isize,
}

unsafe extern "system" fn collect_cb(
    hmon: HMONITOR,
    _hdc: HDC,
    _rect: *mut RECT,
    lparam: LPARAM,
) -> windows::core::BOOL {
    let list = &mut *(lparam.0 as *mut Vec<DisplayInfo>);
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if GetMonitorInfoW(hmon, &mut info.monitorInfo).as_bool() {
        let len = info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len());
        let id = String::from_utf16_lossy(&info.szDevice[..len]);
        let primary = (info.monitorInfo.dwFlags & 1) != 0;
        let mr = info.monitorInfo.rcMonitor;
        let w = (mr.right - mr.left).abs();
        let h = (mr.bottom - mr.top).abs();
        let idx = list.len() + 1;
        let label = if primary {
            format!("显示器 {idx} · 主屏 ({w}×{h})")
        } else {
            format!("显示器 {idx} ({w}×{h})")
        };
        list.push(DisplayInfo {
            id,
            label,
            primary,
            work: info.monitorInfo.rcWork,
            monitor: mr,
            raw_hmonitor: hmon.0 as isize,
        });
    }
    true.into()
}

pub fn enumerate() -> Vec<DisplayInfo> {
    let mut list = Vec::<DisplayInfo>::new();
    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(collect_cb),
            LPARAM((&mut list as *mut Vec<DisplayInfo>) as isize),
        );
    }
    list.sort_by_key(|d| !d.primary);
    for (i, item) in list.iter_mut().enumerate() {
        let w = (item.monitor.right - item.monitor.left).abs();
        let h = (item.monitor.bottom - item.monitor.top).abs();
        item.label = if item.primary {
            format!("显示器 {} · 主屏 ({w}×{h})", i + 1)
        } else {
            format!("显示器 {} ({w}×{h})", i + 1)
        };
    }
    list
}

pub fn list_json() -> Value {
    let items: Vec<Value> = enumerate()
        .into_iter()
        .map(|d| json!({"id": d.id, "label": d.label, "primary": d.primary}))
        .collect();
    json!(items)
}

pub fn monitor_id(hmon: HMONITOR) -> String {
    let mut info = MONITORINFOEXW::default();
    info.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    if unsafe { GetMonitorInfoW(hmon, &mut info.monitorInfo).as_bool() } {
        let len = info
            .szDevice
            .iter()
            .position(|&c| c == 0)
            .unwrap_or(info.szDevice.len());
        return String::from_utf16_lossy(&info.szDevice[..len]);
    }
    String::new()
}

pub fn resolve_monitor(preferred: &str, fallback_hwnd: windows::Win32::Foundation::HWND) -> HMONITOR {
    let preferred = preferred.trim();
    if !preferred.is_empty() {
        if let Some(found) = enumerate().into_iter().find(|d| d.id.eq_ignore_ascii_case(preferred)) {
            return HMONITOR(found.raw_hmonitor as *mut core::ffi::c_void);
        }
    }
    if preferred.is_empty() {
        unsafe { MonitorFromPoint(windows::Win32::Foundation::POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY) }
    } else {
        unsafe { MonitorFromWindow(fallback_hwnd, MONITOR_DEFAULTTONEAREST) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enumerate_finds_primary_display_and_resolves_default() {
        let list = enumerate();
        assert!(!list.is_empty());
        assert!(list.iter().any(|d| d.primary));
        let first_id = list[0].id.clone();
        let h = resolve_monitor(&first_id, windows::Win32::Foundation::HWND::default());
        assert!(!h.is_invalid());
        assert_eq!(monitor_id(h), first_id);
    }
}
