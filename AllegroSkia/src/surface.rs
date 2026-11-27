use crate::platform::{Hwnd, Point, WinRect};
use skia_safe::{AlphaType, Canvas, Color, ColorType, ImageInfo};
use std::{ffi::c_void, path::Path};

#[repr(C)]
struct BitmapHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bits: u16,
    compression: u32,
    image_size: u32,
    x: i32,
    y: i32,
    used: u32,
    important: u32,
}
#[repr(C)]
struct BitmapInfo {
    header: BitmapHeader,
    colors: [u32; 1],
}
#[repr(C)]
struct Size {
    x: i32,
    y: i32,
}
#[repr(C)]
struct Blend {
    operation: u8,
    flags: u8,
    alpha: u8,
    format: u8,
}
#[link(name = "gdi32")]
unsafe extern "system" {
    fn CreateCompatibleDC(dc: Hwnd) -> Hwnd;
    fn DeleteDC(dc: Hwnd) -> i32;
    fn CreateDIBSection(
        dc: Hwnd,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: Hwnd,
        offset: u32,
    ) -> Hwnd;
    fn SelectObject(dc: Hwnd, obj: Hwnd) -> Hwnd;
}
#[link(name = "user32")]
unsafe extern "system" {
    fn UpdateLayeredWindow(
        hwnd: Hwnd,
        dc: Hwnd,
        pos: *const Point,
        size: *const Size,
        source: Hwnd,
        origin: *const Point,
        key: u32,
        blend: *const Blend,
        flags: u32,
    ) -> i32;
}

// Skia 直接零拷贝绘制至顶向下 Win32 DIB 缓冲；彻底消除每帧 read_pixels 整屏内存拷贝。
pub struct SkiaSurface {
    width: i32,
    height: i32,
    pub scale: f32,
    dc: Hwnd,
    bitmap: Hwnd,
    old: Hwnd,
    bits: *mut c_void,
}
impl SkiaSurface {
    pub fn new(width: u32, height: u32, scale: f32) -> Result<Self, String> {
        let (w, h) = checked_dimensions(width, height, 1.)?;
        unsafe {
            let dc = CreateCompatibleDC(std::ptr::null_mut());
            if dc.is_null() {
                return Err("创建内存 DC 失败".into());
            }
            let info = BitmapInfo {
                header: BitmapHeader {
                    size: 40,
                    width: w,
                    height: -h,
                    planes: 1,
                    bits: 32,
                    compression: 0,
                    image_size: 0,
                    x: 0,
                    y: 0,
                    used: 0,
                    important: 0,
                },
                colors: [0],
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(dc, &info, 0, &mut bits, std::ptr::null_mut(), 0);
            if bitmap.is_null() || bits.is_null() {
                if !bitmap.is_null() {
                    crate::platform::DeleteObject(bitmap);
                }
                DeleteDC(dc);
                return Err("创建透明 DIB 失败".into());
            }
            let old = SelectObject(dc, bitmap);
            if old.is_null() || old as isize == -1 {
                crate::platform::DeleteObject(bitmap);
                DeleteDC(dc);
                return Err("选择透明 DIB 失败".into());
            }
            Ok(Self {
                width: w,
                height: h,
                scale,
                dc,
                bitmap,
                old,
                bits,
            })
        }
    }
    pub fn resize(&mut self, w: u32, h: u32, scale: f32) -> Result<(), String> {
        if self.width != w as i32 || self.height != h as i32 {
            *self = Self::new(w, h, scale)?;
        }
        self.scale = scale;
        Ok(())
    }
    pub fn draw(&mut self, draw: impl FnOnce(&Canvas)) -> Result<(), String> {
        let info = ImageInfo::new(
            (self.width, self.height),
            ColorType::BGRA8888,
            AlphaType::Premul,
            None,
        );
        let row_bytes = self.width as usize * 4;
        let len = row_bytes * self.height as usize;
        let pixels = unsafe { std::slice::from_raw_parts_mut(self.bits.cast::<u8>(), len) };
        let mut surface = skia_safe::surfaces::wrap_pixels(&info, pixels, row_bytes, None)
            .ok_or("包装 DIB 到 Skia 表面失败")?;
        let c = surface.canvas();
        c.clear(Color::TRANSPARENT);
        c.save();
        c.scale((self.scale, self.scale));
        draw(c);
        c.restore();
        Ok(())
    }
    pub fn present(&mut self, hwnd: Hwnd) -> Result<(), String> {
        unsafe {
            let mut rect = WinRect::default();
            crate::platform::GetWindowRect(hwnd, &mut rect);
            let pos = Point {
                x: rect.left,
                y: rect.top,
            };
            let size = Size {
                x: self.width,
                y: self.height,
            };
            let origin = Point::default();
            if UpdateLayeredWindow(
                hwnd,
                std::ptr::null_mut(),
                &pos,
                &size,
                self.dc,
                &origin,
                0,
                &Blend {
                    operation: 0,
                    flags: 0,
                    alpha: 255,
                    format: 1,
                },
                2,
            ) == 0
            {
                return Err(format!(
                    "UpdateLayeredWindow: {}",
                    std::io::Error::last_os_error()
                ));
            }
            Ok(())
        }
    }
}
impl Drop for SkiaSurface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.old);
            crate::platform::DeleteObject(self.bitmap);
            DeleteDC(self.dc);
        }
    }
}
fn checked_dimensions(w: u32, h: u32, scale: f32) -> Result<(i32, i32), String> {
    if w == 0 || h == 0 || !scale.is_finite() || scale <= 0. {
        return Err("无效渲染尺寸".into());
    }
    let pw = (w as f64 * scale as f64).ceil();
    let ph = (h as f64 * scale as f64).ceil();
    if pw > 16384. || ph > 16384. || pw * ph > 64_000_000. {
        return Err("渲染尺寸超出限制".into());
    }
    Ok((pw as i32, ph as i32))
}
pub fn render_offscreen_png(
    width: u32,
    height: u32,
    scale: f32,
    draw_fn: impl FnOnce(&Canvas),
) -> Result<Vec<u8>, String> {
    let dimensions = checked_dimensions(width, height, scale)?;
    let mut s = skia_safe::surfaces::raster_n32_premul(dimensions).ok_or("创建离屏表面失败")?;
    let c = s.canvas();
    c.clear(Color::TRANSPARENT);
    c.scale((scale, scale));
    draw_fn(c);
    let image = s.image_snapshot();
    let data = image
        .encode(None, skia_safe::EncodedImageFormat::PNG, None)
        .ok_or("PNG 编码失败")?;
    Ok(data.as_bytes().to_vec())
}
pub fn write_png_file(
    path: &Path,
    width: u32,
    height: u32,
    scale: f32,
    draw: impl FnOnce(&Canvas),
) -> Result<(), String> {
    let data = render_offscreen_png(width, height, scale, draw)?;
    std::fs::write(path, data).map_err(|e| e.to_string())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn png_alpha_and_content() {
        let bytes = render_offscreen_png(64, 64, 2., |c| {
            let mut p = skia_safe::Paint::default();
            p.set_color(Color::from_argb(180, 30, 100, 200));
            c.draw_circle((32., 32.), 20., &p);
        })
        .unwrap();
        let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
        assert_eq!(img.dimensions(), (128, 128));
        assert_eq!(img.get_pixel(0, 0)[3], 0);
        assert!(img.get_pixel(64, 64)[3] > 0);
        assert!(render_offscreen_png(0, 1, 1., |_| {}).is_err());
    }
}
