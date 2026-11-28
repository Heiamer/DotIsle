//! Windows 11/10 任务栏几何探测、自由停靠定位与 Z-Order 保活引擎。
use windows::core::w;
use windows::Win32::{
    Foundation::{HWND, LPARAM, RECT},
    Graphics::Gdi::{MonitorFromRect, MonitorFromWindow, GetMonitorInfoW, MONITORINFO, HMONITOR, MONITOR_DEFAULTTONEAREST},
    System::Registry::{RegGetValueW, HKEY_CURRENT_USER, RRF_RT_REG_DWORD},
    UI::Shell::{SHAppBarMessage, APPBARDATA, ABM_GETSTATE, ABS_AUTOHIDE},
    UI::WindowsAndMessaging::{
        EnumWindows, FindWindowExW, FindWindowW, GetClassNameW, GetWindow, GetWindowRect,
        IsWindowVisible, GetForegroundWindow, IsIconic, GW_HWNDPREV,
        GetWindowLongPtrW, GetWindowThreadProcessId, IsZoomed, GWL_STYLE, WS_CAPTION,
    },
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TaskbarProbe {
    pub hwnd: isize,
    pub taskbar: RECT,
    pub tray: Option<RECT>,
    pub centered: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DockPlacement {
    pub win_x: i32,
    pub win_bottom: i32,
    pub taskbar_height_dp: f64,
    pub capsule_center_in_host_dp: f64,
    pub card_center_in_host_dp: f64,
}

/// 读取 Windows 11 任务栏是否居中对齐（HKCU\...\Advanced\TaskbarAl，默认 1 为居中，0 为靠左）。
pub fn is_taskbar_centered() -> bool {
    let mut val = 1u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Explorer\Advanced"),
            w!("TaskbarAl"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut val as *mut u32).cast()),
            Some(&mut size),
        )
    };
    if status.is_ok() {
        val != 0
    } else {
        true
    }
}

/// 读取 Windows 任务栏当前是否为浅色主题（SystemUsesLightTheme，0 为深色，非 0 为浅色）。
pub fn is_system_light_taskbar() -> bool {
    let mut val = 0u32;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER,
            w!(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize"),
            w!("SystemUsesLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut val as *mut u32).cast()),
            Some(&mut size),
        )
    };
    status.is_ok() && val != 0
}

unsafe extern "system" fn find_secondary_cb(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let target = &mut *(lparam.0 as *mut (HMONITOR, Option<(isize, RECT)>));
    if !IsWindowVisible(hwnd).as_bool() {
        return true.into();
    }
    let mut cls = [0u16; 64];
    let n = GetClassNameW(hwnd, &mut cls);
    if n > 0 && String::from_utf16_lossy(&cls[..n as usize]) == "Shell_SecondaryTrayWnd" {
        let mut r = RECT::default();
        if GetWindowRect(hwnd, &mut r).is_ok() {
            let m = MonitorFromRect(&r, MONITOR_DEFAULTTONEAREST);
            if m == target.0 {
                target.1 = Some((hwnd.0 as isize, r));
                return false.into();
            }
        }
    }
    true.into()
}

/// True when at most a reveal strip of the taskbar intersects the monitor.
fn reveal_strip(taskbar: RECT, monitor: RECT) -> bool {
    let width = taskbar.right.min(monitor.right) - taskbar.left.max(monitor.left);
    let height = taskbar.bottom.min(monitor.bottom) - taskbar.top.max(monitor.top);
    width.min(height) <= 4
}
fn covers_monitor(window: RECT, monitor: RECT) -> bool {
    window.left <= monitor.left + 2 && window.top <= monitor.top + 2 &&
        window.right >= monitor.right - 2 && window.bottom >= monitor.bottom - 2
}
/// Auto-hide only for the foreground fullscreen app on the island's display.
pub unsafe fn foreground_fullscreen_on(monitor: HMONITOR) -> bool {
    use windows::Win32::UI::Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN};
    let Some(window) = foreground_app() else { return false; };
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() { return false; }
    if frame_bounds(window).is_some_and(|bounds| fullscreen_frame(bounds, info.rcMonitor, captioned_maximized(window))) {
        return true;
    }
    // The shell's exclusive-game signal is desktop-wide; associate it with
    // the foreground window's display rather than hiding the island everywhere.
    MonitorFromWindow(window, MONITOR_DEFAULTTONEAREST) == monitor
        && SHQueryUserNotificationState().ok() == Some(QUNS_RUNNING_D3D_FULL_SCREEN)
}

unsafe fn foreground_app() -> Option<HWND> {
    let window = GetForegroundWindow();
    if window.is_invalid() || !IsWindowVisible(window).as_bool() || IsIconic(window).as_bool() { return None; }
    let mut process = 0;
    GetWindowThreadProcessId(window, Some(&mut process));
    if process == 0 || process == std::process::id() { return None; }
    let mut class = [0u16; 64];
    let n = GetClassNameW(window, &mut class).max(0) as usize;
    if matches!(String::from_utf16_lossy(&class[..n]).as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") { return None; }
    Some(window)
}

unsafe fn captioned_maximized(window: HWND) -> bool {
    IsZoomed(window).as_bool() && (GetWindowLongPtrW(window, GWL_STYLE) as u32 & WS_CAPTION.0) == WS_CAPTION.0
}

/// Protect exclusive Direct3D, borderless fullscreen on any monitor, and a
/// foreground application that has confined the pointer. Only read OS state.
pub fn blocks_wellness_interrupt() -> bool {
    use windows::Win32::UI::{
        Shell::{SHQueryUserNotificationState, QUNS_RUNNING_D3D_FULL_SCREEN},
        WindowsAndMessaging::{GetClipCursor, GetSystemMetrics, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, SM_CXVIRTUALSCREEN, SM_CYVIRTUALSCREEN},
    };
    unsafe {
        if SHQueryUserNotificationState().ok() == Some(QUNS_RUNNING_D3D_FULL_SCREEN) { return true; }
        let Some(foreground) = foreground_app() else { return false; };
        let mut clip = RECT::default();
        if GetClipCursor(&mut clip).is_ok() {
            let (left, top) = (GetSystemMetrics(SM_XVIRTUALSCREEN), GetSystemMetrics(SM_YVIRTUALSCREEN));
            let desktop = RECT { left, top, right: left + GetSystemMetrics(SM_CXVIRTUALSCREEN), bottom: top + GetSystemMetrics(SM_CYVIRTUALSCREEN) };
            if desktop.right > left && desktop.bottom > top && clipped_within_desktop(clip, desktop) { return true; }
        }
        let Some(bounds) = frame_bounds(foreground) else { return false; };
        let monitor = MonitorFromRect(&bounds, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() { return false; }
        fullscreen_frame(bounds, info.rcMonitor, captioned_maximized(foreground))
    }
}
fn fullscreen_frame(bounds: RECT, monitor: RECT, captioned_maximized: bool) -> bool {
    !captioned_maximized && covers_monitor(bounds, monitor)
}
fn clipped_within_desktop(clip: RECT, desktop: RECT) -> bool {
    // A border at only one edge is sufficient (e.g. game on an adjacent display).
    clip.left > desktop.left + 2 || clip.top > desktop.top + 2 || clip.right < desktop.right - 2 || clip.bottom < desktop.bottom - 2
}
unsafe fn frame_bounds(window: HWND) -> Option<RECT> {
    use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_EXTENDED_FRAME_BOUNDS};
    let mut bounds = RECT::default();
    if DwmGetWindowAttribute(window, DWMWA_EXTENDED_FRAME_BOUNDS, (&mut bounds as *mut RECT).cast(), std::mem::size_of::<RECT>() as u32).is_err()
        && GetWindowRect(window, &mut bounds).is_err() { return None; }
    Some(bounds)
}
pub fn top_dock(taskbar: RECT, monitor: RECT) -> bool {
    taskbar.bottom - taskbar.top < taskbar.right - taskbar.left &&
        (taskbar.top - monitor.top).abs() < (taskbar.bottom - monitor.bottom).abs()
}
pub unsafe fn should_suppress(monitor: HMONITOR, probe: Option<TaskbarProbe>, input: HWND, visual: HWND) -> bool {
    let mut info = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
    if !GetMonitorInfoW(monitor, &mut info).as_bool() { return false; }
    let mut data = APPBARDATA { cbSize: std::mem::size_of::<APPBARDATA>() as u32, ..Default::default() };
    let auto_hide = SHAppBarMessage(ABM_GETSTATE, &mut data) & ABS_AUTOHIDE as usize != 0;
    if auto_hide && probe.is_none_or(|p| reveal_strip(p.taskbar, info.rcMonitor)) { return true; }
    let foreground = GetForegroundWindow();
    if foreground.is_invalid() || foreground == input || foreground == visual || IsIconic(foreground).as_bool() { return false; }
    let mut class = [0u16; 64];
    let n = GetClassNameW(foreground,&mut class).max(0) as usize;
    if matches!(String::from_utf16_lossy(&class[..n]).as_str(),"Progman"|"WorkerW"|"Shell_TrayWnd"|"Shell_SecondaryTrayWnd") { return false; }
    frame_bounds(foreground).is_some_and(|bounds| covers_monitor(bounds, info.rcMonitor))
}

/// 探测目标显示器上正在运行的任务栏及系统托盘安全矩形。
pub fn probe_for_monitor(target_monitor: HMONITOR) -> Option<TaskbarProbe> {
    let centered = is_taskbar_centered();
    unsafe {
        if let Ok(primary_tb) = FindWindowW(w!("Shell_TrayWnd"), None) {
            if !primary_tb.is_invalid() && IsWindowVisible(primary_tb).as_bool() {
                let mut tb_rect = RECT::default();
                if GetWindowRect(primary_tb, &mut tb_rect).is_ok() {
                    let tb_mon = MonitorFromRect(&tb_rect, MONITOR_DEFAULTTONEAREST);
                    if tb_mon == target_monitor || target_monitor.is_invalid() {
                        let tray = FindWindowExW(Some(primary_tb), None, w!("TrayNotifyWnd"), None)
                            .ok()
                            .filter(|h| !h.is_invalid())
                            .and_then(|h| {
                                let mut r = RECT::default();
                                GetWindowRect(h, &mut r).ok().map(|_| r)
                            });
                        return Some(TaskbarProbe {
                            hwnd: primary_tb.0 as isize,
                            taskbar: tb_rect,
                            tray,
                            centered,
                        });
                    }
                }
            }
        }
        // 查找副屏任务栏
        let mut state = (target_monitor, None::<(isize, RECT)>);
        let _ = EnumWindows(
            Some(find_secondary_cb),
            LPARAM((&mut state as *mut (HMONITOR, Option<(isize, RECT)>)) as isize),
        );
        if let Some((sec_hwnd, tb_rect)) = state.1 {
            return Some(TaskbarProbe {
                hwnd: sec_hwnd,
                taskbar: tb_rect,
                tray: None,
                centered,
            });
        }
    }
    None
}

/// 检查目标窗口是否被任务栏（Shell_TrayWnd / Shell_SecondaryTrayWnd）压在了更低的 Z-Order 层级。
pub unsafe fn is_behind_taskbar(win: HWND, tb_hwnd: isize) -> bool {
    if win.0.is_null() {
        return false;
    }
    let mut cur = win;
    for _ in 0..64 {
        let Ok(prev) = GetWindow(cur, GW_HWNDPREV) else {
            break;
        };
        if prev.0.is_null() || prev == cur {
            break;
        }
        if tb_hwnd != 0 && prev.0 as isize == tb_hwnd && IsWindowVisible(prev).as_bool() {
            return true;
        }
        let mut cls = [0u16; 48];
        let n = GetClassNameW(prev, &mut cls);
        if n > 0 && IsWindowVisible(prev).as_bool() {
            let name = String::from_utf16_lossy(&cls[..n as usize]);
            if name == "Shell_TrayWnd" || name == "Shell_SecondaryTrayWnd" {
                return true;
            }
        }
        cur = prev;
    }
    false
}

/// 根据任务栏几何、停靠基准、自由滑块百分比与防溢出规则，精准计算停靠槽位与向上弹出浮窗位置。
pub fn compute_dock_placement(
    probe: TaskbarProbe,
    align_mode: &str,
    position_pct: f64,
    host_w_dp: f64,
    capsule_w_dp: f64,
    card_w_dp: f64,
    scale: f64,
) -> DockPlacement {
    let scale = scale.max(1.0);
    let tb = probe.taskbar;
    let tb_w = (tb.right - tb.left).max(320) as f64;
    let tb_h_dp = ((tb.bottom - tb.top) as f64 / scale).clamp(36., 76.);
    let cap_px = capsule_w_dp * scale;
    let card_px = card_w_dp * scale;
    let host_px = host_w_dp * scale;

    let pad_px = 12. * scale;
    let tray_left = probe
        .tray
        .map(|r| r.left as f64)
        .filter(|&l| l > tb.left as f64 + cap_px && l <= tb.right as f64)
        .unwrap_or(tb.right as f64 - 160. * scale);

    let min_center = (tb.left as f64 + pad_px + cap_px / 2.).min(tb.right as f64 - cap_px / 2. - 8. * scale);
    let max_center = (tray_left - pad_px - cap_px / 2.).max(min_center);

    let target_capsule_center_px = match align_mode {
        "Left" => min_center,
        "Right" => max_center,
        "Free" => {
            let t = (position_pct / 100.).clamp(0., 1.);
            min_center + (max_center - min_center) * t
        }
        _ => {
            if probe.centered {
                min_center
            } else {
                max_center
            }
        }
    };

    // 浮窗优先与任务栏微条对齐中心；靠近屏幕边角时自动向屏内推移防止被裁切
    let desired_card_left = target_capsule_center_px - card_px / 2.;
    let clamped_card_left = desired_card_left.clamp(
        tb.left as f64 + 8. * scale,
        (tb.right as f64 - card_px - 8. * scale).max(tb.left as f64),
    );
    let card_center_px = clamped_card_left + card_px / 2.;

    let min_host_left = tb.left as f64;
    let max_host_left = (tb.left as f64 + tb_w - host_px).max(min_host_left);
    let win_left_px = ((target_capsule_center_px + card_center_px) * 0.5 - host_px / 2.)
        .clamp(min_host_left, max_host_left);

    DockPlacement {
        win_x: win_left_px.round() as i32,
        win_bottom: tb.bottom,
        taskbar_height_dp: tb_h_dp,
        capsule_center_in_host_dp: (target_capsule_center_px - win_left_px) / scale,
        card_center_in_host_dp: (card_center_px - win_left_px) / scale,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn win11_centered_auto_docks_left_without_clipping_card() {
        let probe = TaskbarProbe {
            hwnd: 1,
            taskbar: RECT { left: 0, top: 1032, right: 1920, bottom: 1080 },
            tray: Some(RECT { left: 1680, top: 1032, right: 1920, bottom: 1080 }),
            centered: true,
        };
        let p = compute_dock_placement(probe, "Auto", 18., 560., 340., 400., 1.0);
        assert_eq!(p.win_bottom, 1080);
        assert_eq!(p.taskbar_height_dp, 48.);
        let card_left = p.win_x as f64 + p.card_center_in_host_dp - 200.;
        assert!(card_left >= 7.5);
        assert!(p.capsule_center_in_host_dp < 240.);
    }

    #[test]
    fn left_aligned_or_right_mode_docks_beside_system_tray() {
        let probe = TaskbarProbe {
            hwnd: 1,
            taskbar: RECT { left: 0, top: 1032, right: 1920, bottom: 1080 },
            tray: Some(RECT { left: 1650, top: 1032, right: 1920, bottom: 1080 }),
            centered: false,
        };
        let p = compute_dock_placement(probe, "Auto", 18., 560., 340., 400., 1.0);
        let cap_right = p.win_x as f64 + p.capsule_center_in_host_dp + 170.;
        assert!(cap_right < 1650.);
        let card_right = p.win_x as f64 + p.card_center_in_host_dp + 200.;
        assert!(card_right <= 1920.);
    }

    #[test]
    fn free_mode_interpolates_across_safe_taskbar_span() {
        let probe = TaskbarProbe {
            hwnd: 1,
            taskbar: RECT { left: 0, top: 1032, right: 1920, bottom: 1080 },
            tray: Some(RECT { left: 1680, top: 1032, right: 1920, bottom: 1080 }),
            centered: true,
        };
        let left = compute_dock_placement(probe, "Free", 0., 560., 340., 400., 1.0);
        let mid = compute_dock_placement(probe, "Free", 50., 560., 340., 400., 1.0);
        let right = compute_dock_placement(probe, "Free", 100., 560., 340., 400., 1.0);

        let left_cx = left.win_x as f64 + left.capsule_center_in_host_dp;
        let mid_cx = mid.win_x as f64 + mid.capsule_center_in_host_dp;
        let right_cx = right.win_x as f64 + right.capsule_center_in_host_dp;
        assert!(left_cx < mid_cx && mid_cx < right_cx);
        assert!(left_cx - 170. >= 0.);
        assert!(right_cx + 170. <= 1680.);
    }
}

#[cfg(test)]
mod visibility_tests {
    use super::*;
    #[test] fn top_and_bottom_reveal_strips_are_not_visible_taskbars() {
        let m=RECT{left:-1920,top:0,right:0,bottom:1080};
        for (top,bottom) in [(-46,2),(1078,1126)] { assert!(reveal_strip(RECT{top,bottom,..m},m)); }
        assert!(!reveal_strip(RECT{bottom:48,..m},m));
        assert!(top_dock(RECT{bottom:48,..m},m));
        assert!(!top_dock(RECT{top:1032,..m},m));
        assert!(covers_monitor(m,m));
        assert!(!covers_monitor(RECT{top:48,..m},m));
    }
}

#[cfg(test)]
mod foreground_guard_tests {
    use super::*;
    fn r(left:i32, top:i32, right:i32, bottom:i32)->RECT { RECT{left,top,right,bottom} }

    #[test]
    fn recognizes_borderless_fullscreen_on_an_extended_monitor_without_treating_maximized_apps_as_games() {
        let monitor = r(-1920,120,0,1200);
        assert!(fullscreen_frame(r(-1921,120,0,1200), monitor, false));
        assert!(!fullscreen_frame(monitor, monitor, true));
        assert!(!fullscreen_frame(r(-1920,155,0,1200), monitor, false));
        assert!(!fullscreen_frame(r(0,0,2560,1440), monitor, false));
    }

    #[test]
    fn confinement_to_either_display_is_not_the_full_virtual_desktop() {
        let desktop = r(-1920,0,2560,1440);
        assert!(!clipped_within_desktop(desktop,desktop));
        assert!(clipped_within_desktop(r(-1920,120,0,1200),desktop));
        assert!(clipped_within_desktop(r(0,0,2560,1440),desktop));
        assert!(clipped_within_desktop(r(20,30,21,31),desktop));
    }
}
