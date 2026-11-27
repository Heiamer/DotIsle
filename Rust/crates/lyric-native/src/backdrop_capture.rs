//! 灵动岛「液态玻璃 Beta」按需背景采样器：仅在选用 LiquidGlass 时运行，切换回原材质即刻释放并恢复窗口属性。
use std::{
    path::PathBuf,
    time::{Duration, Instant, SystemTime},
};
use windows::Win32::{
    Foundation::{HWND, RECT},
    Graphics::Gdi::{
        BitBlt, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC,
        SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    },
    UI::WindowsAndMessaging::{
        GetSystemMetrics, GetWindowRect, SetWindowDisplayAffinity, SM_CXSCREEN, SM_CYSCREEN,
        SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WDA_EXCLUDEFROMCAPTURE, WDA_NONE,
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BackdropSource {
    Live,
    Wallpaper,
}

impl BackdropSource {
    pub fn from_setting(v: &str) -> Self {
        if v.eq_ignore_ascii_case("Wallpaper") {
            Self::Wallpaper
        } else {
            Self::Live
        }
    }
}

pub struct CapturedBackdrop {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

struct CachedWallpaper {
    modified: Option<SystemTime>,
    screen_w: u32,
    screen_h: u32,
    rgba: Vec<u8>,
}

/// 负责灵动岛局部屏幕 ROI / 壁纸采样与 `WDA_EXCLUDEFROMCAPTURE` 生命周期管理。
pub struct BackdropCapturer {
    excluded_from_capture: bool,
    last_capture: Instant,
    wallpaper_cache: Option<CachedWallpaper>,
    scratch: Vec<u8>,
}

impl Default for BackdropCapturer {
    fn default() -> Self {
        Self {
            excluded_from_capture: false,
            last_capture: Instant::now() - Duration::from_secs(10),
            wallpaper_cache: None,
            scratch: Vec::new(),
        }
    }
}

impl BackdropCapturer {
    /// 当所有表面都不再使用 `LiquidGlass` 或窗口隐藏时，立刻停采并恢复正常截图可见性。
    pub unsafe fn deactivate(&mut self, hwnd: HWND) {
        if self.excluded_from_capture && !hwnd.is_invalid() {
            let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
            self.excluded_from_capture = false;
        }
        if !self.scratch.is_empty() {
            self.scratch = Vec::new();
        }
    }

    /// 按需采集灵动岛背后的局部像素区域；未到限帧间隔时返回 `None`。
    pub unsafe fn capture_if_due(
        &mut self,
        hwnd: HWND,
        source: BackdropSource,
        animating: bool,
    ) -> Option<CapturedBackdrop> {
        if hwnd.is_invalid() {
            return None;
        }
        let interval = match source {
            BackdropSource::Live => {
                if animating {
                    Duration::from_millis(16)
                } else {
                    Duration::from_millis(33)
                }
            }
            BackdropSource::Wallpaper => Duration::from_millis(if animating { 33 } else { 250 }),
        };
        if self.last_capture.elapsed() < interval {
            return None;
        }
        self.last_capture = Instant::now();

        let mut rect = RECT::default();
        if GetWindowRect(hwnd, &mut rect).is_err() {
            return None;
        }
        let w = (rect.right - rect.left).max(1) as u32;
        let h = (rect.bottom - rect.top).max(1) as u32;

        match source {
            BackdropSource::Live => {
                if !self.excluded_from_capture {
                    let _ = SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE);
                    self.excluded_from_capture = true;
                }
                self.capture_screen_roi(rect.left, rect.top, w, h)
            }
            BackdropSource::Wallpaper => {
                if self.excluded_from_capture {
                    let _ = SetWindowDisplayAffinity(hwnd, WDA_NONE);
                    self.excluded_from_capture = false;
                }
                self.capture_wallpaper_roi(rect.left, rect.top, w, h)
            }
        }
    }

    unsafe fn capture_screen_roi(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<CapturedBackdrop> {
        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            return None;
        }
        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_invalid() {
            let _ = ReleaseDC(None, screen_dc);
            return None;
        }

        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w as i32,
                biHeight: -(h as i32), // top-down DIB
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut std::ffi::c_void = std::ptr::null_mut();
        let Ok(hbmp) = CreateDIBSection(Some(screen_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0) else {
            let _ = DeleteDC(mem_dc);
            let _ = ReleaseDC(None, screen_dc);
            return None;
        };
        let old = SelectObject(mem_dc, hbmp.into());
        // 严格仅使用 SRCCOPY 尊重 WDA_EXCLUDEFROMCAPTURE 排除自身且绝不出现系统黄框
        let ok = BitBlt(mem_dc, 0, 0, w as i32, h as i32, Some(screen_dc), x, y, SRCCOPY).is_ok();

        let out = if ok && !bits.is_null() {
            let total = (w as usize) * (h as usize) * 4;
            let raw = std::slice::from_raw_parts(bits as *const u8, total);
            self.scratch.resize(total, 0);
            for (dst, src) in self.scratch.chunks_exact_mut(4).zip(raw.chunks_exact(4)) {
                dst[0] = src[2];
                dst[1] = src[1];
                dst[2] = src[0];
                dst[3] = 255;
            }
            Some(CapturedBackdrop {
                rgba: std::mem::take(&mut self.scratch),
                width: w,
                height: h,
            })
        } else {
            None
        };

        let _ = SelectObject(mem_dc, old);
        let _ = DeleteObject(hbmp.into());
        let _ = DeleteDC(mem_dc);
        let _ = ReleaseDC(None, screen_dc);
        out
    }

    unsafe fn capture_wallpaper_roi(&mut self, x: i32, y: i32, w: u32, h: u32) -> Option<CapturedBackdrop> {
        let screen_w = GetSystemMetrics(SM_CXSCREEN).max(1) as u32;
        let screen_h = GetSystemMetrics(SM_CYSCREEN).max(1) as u32;
        let wp_path = wallpaper_path()?;
        let modified = std::fs::metadata(&wp_path).ok().and_then(|m| m.modified().ok());

        let stale = self.wallpaper_cache.as_ref().is_none_or(|c| {
            c.screen_w != screen_w || c.screen_h != screen_h || c.modified != modified
        });
        if stale {
            let bytes = std::fs::read(&wp_path).ok()?;
            let img = image::load_from_memory(&bytes).ok()?;
            let resized = img.resize_to_fill(
                screen_w,
                screen_h,
                image::imageops::FilterType::Triangle,
            );
            self.wallpaper_cache = Some(CachedWallpaper {
                modified,
                screen_w,
                screen_h,
                rgba: resized.to_rgba8().into_raw(),
            });
        }

        let cache = self.wallpaper_cache.as_ref()?;
        let vx = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let vy = GetSystemMetrics(SM_YVIRTUALSCREEN);
        let base_x = (x - vx).max(0);
        let base_y = (y - vy).max(0);
        let total = (w as usize) * (h as usize) * 4;
        let mut rgba = vec![0u8; total];
        for row in 0..h as usize {
            let sy = ((base_y as usize + row).min(cache.screen_h as usize - 1)) * (cache.screen_w as usize);
            let dst_off = row * (w as usize) * 4;
            for col in 0..w as usize {
                let sx = (base_x as usize + col).min(cache.screen_w as usize - 1);
                let src_i = (sy + sx) * 4;
                let dst_i = dst_off + col * 4;
                rgba[dst_i..dst_i + 4].copy_from_slice(&cache.rgba[src_i..src_i + 4]);
            }
        }
        Some(CapturedBackdrop {
            rgba,
            width: w,
            height: h,
        })
    }
}

fn wallpaper_path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    let p = PathBuf::from(appdata)
        .join("Microsoft")
        .join("Windows")
        .join("Themes")
        .join("TranscodedWallpaper");
    if p.exists() {
        Some(p)
    } else {
        None
    }
}
