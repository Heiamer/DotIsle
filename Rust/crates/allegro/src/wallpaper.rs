//! 桌面壁纸感知、亚克力磨砂颗粒与真液态玻璃折射采样缓存引擎。
use crate::snap::Rect2D;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
    time::SystemTime,
};
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

#[derive(Clone, Hash, PartialEq, Eq)]
struct CropKey {
    monitor_key: String,
    mode: u8,
    x: i32,
    y: i32,
    w: u32,
    h: u32,
    passes: u8,
    radius_q: u16,
}

struct LoadedWallpaper {
    path: PathBuf,
    modified: Option<SystemTime>,
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

/// 裁切后的桌面壁纸光学采样结果（含平均相对亮度用于对比度自适应）。
#[derive(Clone)]
pub struct WallpaperSample {
    pub image: ImageData,
    pub avg_luma: f32,
}

/// 桌面壁纸读取、亚克力磨砂与真液态玻璃折射缓存器。
pub struct WallpaperCache {
    wallpaper: Option<LoadedWallpaper>,
    crops: HashMap<CropKey, WallpaperSample>,
    last_probe_sec: f64,
}

impl Default for WallpaperCache {
    fn default() -> Self {
        Self::new()
    }
}

impl WallpaperCache {
    pub fn new() -> Self {
        Self {
            wallpaper: None,
            crops: HashMap::new(),
            last_probe_sec: -10.0,
        }
    }

    /// 每隔 5 秒轻检测一次系统桌面壁纸路径与修改时间，壁纸变更时自动失效缓存。
    pub fn poll_system(&mut self, now_sec: f64) {
        if now_sec - self.last_probe_sec < 5.0 {
            return;
        }
        self.last_probe_sec = now_sec;
        self.probe_system();
    }

    fn probe_system(&mut self) -> bool {
        let Some(path) = query_system_wallpaper() else {
            return false;
        };
        let modified = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        let unchanged = self.wallpaper.as_ref().is_some_and(|w| {
            w.path == path && w.modified == modified
        });
        if unchanged {
            return false;
        }
        self.load_from_path(&path, modified)
    }

    /// 直接从指定路径加载壁纸（同步更新缓存，也用于单元测试或用户自定义）。
    pub fn load_from_path(&mut self, path: &Path, modified: Option<SystemTime>) -> bool {
        let Ok(decoded) = image::open(path) else {
            return false;
        };
        let resized = decoded.thumbnail(1920, 1200).to_rgba8();
        let (width, height) = resized.dimensions();
        self.wallpaper = Some(LoadedWallpaper {
            path: path.to_path_buf(),
            modified,
            width: width.max(1),
            height: height.max(1),
            rgba: resized.into_raw(),
        });
        self.crops.clear();
        true
    }

    /// 在纯内存下注入一张测试用底图，方便离线单测验证裁切、模糊与折射逻辑。
    pub fn set_synthetic_wallpaper(&mut self, width: u32, height: u32, rgba: Vec<u8>) {
        let w = width.max(1);
        let h = height.max(1);
        if rgba.len() == (w * h * 4) as usize {
            self.wallpaper = Some(LoadedWallpaper {
                path: PathBuf::from("<synthetic>"),
                modified: None,
                width: w,
                height: h,
                rgba,
            });
            self.crops.clear();
        }
    }

    /// 向后兼容的高斯模糊壁纸裁切接口。
    pub fn crop_blurred(
        &mut self,
        monitor_key: &str,
        monitor_size: (f64, f64),
        card: Rect2D,
        passes: u8,
    ) -> Option<ImageData> {
        self.crop_acrylic_inner(monitor_key, monitor_size, card, passes, false)
            .map(|s| s.image)
    }

    /// 提取并烘焙**亚克力**底图：3 次分离盒式模糊 + Luminosity 微妙噪点质感。
    pub fn crop_acrylic(
        &mut self,
        monitor_key: &str,
        monitor_size: (f64, f64),
        card: Rect2D,
    ) -> Option<WallpaperSample> {
        self.crop_acrylic_inner(monitor_key, monitor_size, card, 3, true)
    }

    /// 提取并烘焙**右侧侧边栏丝绸柔雾背景**：大半径 5 次平滑高斯模糊、无粗糙颗粒噪点，
    /// 将壁纸色彩化作类似 HyperOS / iOS 负一屏的温润丝绸散景光晕。
    pub fn crop_sidebar_frost(
        &mut self,
        monitor_key: &str,
        monitor_size: (f64, f64),
        panel: Rect2D,
    ) -> Option<WallpaperSample> {
        let wall = self.wallpaper.as_ref()?;
        let mon_w = monitor_size.0.max(1.0);
        let mon_h = monitor_size.1.max(1.0);
        let target_w = ((panel.w * 0.22).round() as u32).clamp(48, 128);
        let target_h = ((panel.h * 0.22).round() as u32).clamp(96, 240);

        let key = CropKey {
            monitor_key: monitor_key.to_string(),
            mode: 3,
            x: (panel.x / 8.0).round() as i32,
            y: (panel.y / 8.0).round() as i32,
            w: target_w,
            h: target_h,
            passes: 5,
            radius_q: 0,
        };
        if let Some(cached) = self.crops.get(&key) {
            return Some(cached.clone());
        }

        let mut buf = vec![0u8; (target_w * target_h * 4) as usize];
        let u0 = (panel.x / mon_w).clamp(0.0, 1.0);
        let v0 = (panel.y / mon_h).clamp(0.0, 1.0);
        let u1 = ((panel.x + panel.w) / mon_w).clamp(0.0, 1.0);
        let v1 = ((panel.y + panel.h) / mon_h).clamp(0.0, 1.0);

        for py in 0..target_h {
            let v = v0 + (v1 - v0) * ((py as f64 + 0.5) / target_h as f64);
            for px in 0..target_w {
                let u = u0 + (u1 - u0) * ((px as f64 + 0.5) / target_w as f64);
                let c = sample_bilinear(wall, u, v);
                let di = ((py * target_w + px) * 4) as usize;
                buf[di] = c[0].round().clamp(0.0, 255.0) as u8;
                buf[di + 1] = c[1].round().clamp(0.0, 255.0) as u8;
                buf[di + 2] = c[2].round().clamp(0.0, 255.0) as u8;
                buf[di + 3] = 255;
            }
        }

        // 大半径多轮分离模糊，完全抹平锐利纹理，留下柔滑奶油雾面光晕
        let radius = ((target_w.min(target_h) / 5) as usize).clamp(8, 22);
        for _ in 0..5 {
            box_blur_rgba(&mut buf, target_w as usize, target_h as usize, radius);
        }

        for px in buf.chunks_exact_mut(4) {
            let r = px[0] as f32;
            let g = px[1] as f32;
            let b = px[2] as f32;
            let lum = 0.2126 * r + 0.7152 * g + 0.0722 * b;
            // 柔和降饱和 0.78，避免高对比壁纸干扰白色卡片阅读
            px[0] = (lum + (r - lum) * 0.78).clamp(0.0, 255.0) as u8;
            px[1] = (lum + (g - lum) * 0.78).clamp(0.0, 255.0) as u8;
            px[2] = (lum + (b - lum) * 0.78).clamp(0.0, 255.0) as u8;
        }
        let avg_luma = compute_luma(&buf);

        let sample = WallpaperSample {
            image: ImageData {
                data: Blob::new(Arc::new(buf)),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: target_w,
                height: target_h,
            },
            avg_luma,
        };
        if self.crops.len() > 128 {
            self.crops.clear();
        }
        self.crops.insert(key, sample.clone());
        Some(sample)
    }

    fn crop_acrylic_inner(
        &mut self,
        monitor_key: &str,
        monitor_size: (f64, f64),
        card: Rect2D,
        passes: u8,
        add_grain: bool,
    ) -> Option<WallpaperSample> {
        let wall = self.wallpaper.as_ref()?;
        let mon_w = monitor_size.0.max(1.0);
        let mon_h = monitor_size.1.max(1.0);
        let target_w = ((card.w * 0.32).round() as u32).clamp(24, 200);
        let target_h = ((card.h * 0.32).round() as u32).clamp(24, 200);

        let key = CropKey {
            monitor_key: monitor_key.to_string(),
            mode: if add_grain { 1 } else { 0 },
            x: (card.x / 4.0).round() as i32,
            y: (card.y / 4.0).round() as i32,
            w: target_w,
            h: target_h,
            passes,
            radius_q: 0,
        };
        if let Some(cached) = self.crops.get(&key) {
            return Some(cached.clone());
        }

        let mut buf = vec![0u8; (target_w * target_h * 4) as usize];
        let u0 = (card.x / mon_w).clamp(0.0, 1.0);
        let v0 = (card.y / mon_h).clamp(0.0, 1.0);
        let u1 = ((card.x + card.w) / mon_w).clamp(0.0, 1.0);
        let v1 = ((card.y + card.h) / mon_h).clamp(0.0, 1.0);

        for py in 0..target_h {
            let v = v0 + (v1 - v0) * ((py as f64 + 0.5) / target_h as f64);
            for px in 0..target_w {
                let u = u0 + (u1 - u0) * ((px as f64 + 0.5) / target_w as f64);
                let c = sample_bilinear(wall, u, v);
                let di = ((py * target_w + px) * 4) as usize;
                buf[di] = c[0].round().clamp(0.0, 255.0) as u8;
                buf[di + 1] = c[1].round().clamp(0.0, 255.0) as u8;
                buf[di + 2] = c[2].round().clamp(0.0, 255.0) as u8;
                buf[di + 3] = 255;
            }
        }

        for _ in 0..passes.clamp(1, 4) {
            box_blur_rgba(&mut buf, target_w as usize, target_h as usize, 4);
        }

        if add_grain {
            // 确定性亚克力微颗粒噪点（静止零重算且无条带伪影）
            for py in 0..target_h {
                for px in 0..target_w {
                    let h = px
                        .wrapping_mul(374_761_393)
                        .wrapping_add(py.wrapping_mul(668_265_263))
                        ^ (px ^ py).wrapping_mul(1_274_126_177);
                    let jitter = ((h >> 24) & 7) as i16 - 3; // -3..=+4 轻微颗粒
                    let di = ((py * target_w + px) * 4) as usize;
                    for ch in 0..3 {
                        buf[di + ch] = (buf[di + ch] as i16 + jitter).clamp(0, 255) as u8;
                    }
                }
            }
        }

        let avg_luma = compute_luma(&buf);
        let sample = WallpaperSample {
            image: ImageData {
                data: Blob::new(Arc::new(buf)),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: target_w,
                height: target_h,
            },
            avg_luma,
        };
        if self.crops.len() > 128 {
            self.crops.clear();
        }
        self.crops.insert(key, sample.clone());
        Some(sample)
    }

    /// 提取并合成借鉴 Apple / kube.io 物理光学模型的**真液态玻璃 (Liquid Glass)** 采样图：
    /// 1) 轻度预柔化 + 1.65x Vibrant 色彩饱和度增强（消除透明底纱发灰问题）；
    /// 2) Convex Squircle 凸起斜面轮廓 $h(x)=(1-(1-x)^4)^{1/4}$ 计算 3D 表面法线；
    /// 3) Snell's Law 三通道折射率色散 ($n_R=1.48, n_G=1.52, n_B=1.57$)；
    /// 4) 双角 Blinn-Phong 镜面反射高光带 Specular Rim。
    pub fn crop_liquid_glass(
        &mut self,
        monitor_key: &str,
        monitor_size: (f64, f64),
        card: Rect2D,
        radius: f64,
    ) -> Option<WallpaperSample> {
        let wall = self.wallpaper.as_ref()?;
        let mon_w = monitor_size.0.max(1.0);
        let mon_h = monitor_size.1.max(1.0);
        let target_w = ((card.w * 0.45).round() as u32).clamp(32, 240);
        let target_h = ((card.h * 0.45).round() as u32).clamp(32, 240);
        let rad_q = radius.round().clamp(0.0, 200.0) as u16;

        let key = CropKey {
            monitor_key: monitor_key.to_string(),
            mode: 2,
            x: (card.x / 4.0).round() as i32,
            y: (card.y / 4.0).round() as i32,
            w: target_w,
            h: target_h,
            passes: 1,
            radius_q: rad_q,
        };
        if let Some(cached) = self.crops.get(&key) {
            return Some(cached.clone());
        }

        // 1. 预采样微扩边距背景缓冲，轻柔模糊后增强饱和度以还原苹果玻璃通透彩感
        let pad_u = (card.w / mon_w) * 0.06;
        let pad_v = (card.h / mon_h) * 0.06;
        let su0 = (card.x / mon_w - pad_u).clamp(0.0, 1.0);
        let sv0 = (card.y / mon_h - pad_v).clamp(0.0, 1.0);
        let su1 = ((card.x + card.w) / mon_w + pad_u).clamp(0.0, 1.0);
        let sv1 = ((card.y + card.h) / mon_h + pad_v).clamp(0.0, 1.0);

        let mut src_buf = vec![0u8; (target_w * target_h * 4) as usize];
        for py in 0..target_h {
            let v = sv0 + (sv1 - sv0) * ((py as f64 + 0.5) / target_h as f64);
            for px in 0..target_w {
                let u = su0 + (su1 - su0) * ((px as f64 + 0.5) / target_w as f64);
                let c = sample_bilinear(wall, u, v);
                let di = ((py * target_w + px) * 4) as usize;
                src_buf[di] = c[0].round().clamp(0.0, 255.0) as u8;
                src_buf[di + 1] = c[1].round().clamp(0.0, 255.0) as u8;
                src_buf[di + 2] = c[2].round().clamp(0.0, 255.0) as u8;
                src_buf[di + 3] = 255;
            }
        }
        box_blur_rgba(&mut src_buf, target_w as usize, target_h as usize, 2);
        box_blur_rgba(&mut src_buf, target_w as usize, target_h as usize, 2);
        boost_saturation(&mut src_buf, 1.65);

        let tw = target_w as f64;
        let th = target_h as f64;
        let half_w = tw * 0.5;
        let half_h = th * 0.5;
        let scale_px = tw / card.w.max(1.0);
        let rad_px = (radius * scale_px).min(half_w.min(half_h) - 1.0).max(2.0);
        let bezel_w = (rad_px * 1.15).clamp(14.0, 28.0);
        let thickness = 0.085;

        // 归一化光照与半程方向（左上主光源 + 右下环境回光）
        let (l1x, l1y, l1z) = norm3(-0.707, -0.707, 0.58);
        let (l2x, l2y, l2z) = norm3(0.707, 0.707, 0.50);
        let (h1x, h1y, h1z) = norm3(l1x, l1y, l1z + 1.0);
        let (h2x, h2y, h2z) = norm3(l2x, l2y, l2z + 1.0);

        let mut buf = vec![0u8; (target_w * target_h * 4) as usize];

        for py in 0..target_h {
            let fy = py as f64 + 0.5;
            let ry = fy - half_h;
            for px in 0..target_w {
                let fx = px as f64 + 0.5;
                let rx = fx - half_w;

                // 计算圆角矩形边界距离与指向内部的单位梯度 (gx, gy)
                let qx = rx.abs() - (half_w - rad_px);
                let qy = ry.abs() - (half_h - rad_px);
                let dist_out = (qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0)) - rad_px;
                let depth_in = (-dist_out).max(0.0);

                let (gx, gy) = if qx > 0.0 || qy > 0.0 {
                    let dx = qx.max(0.0) * rx.signum();
                    let dy = qy.max(0.0) * ry.signum();
                    let len = dx.hypot(dy).max(1e-4);
                    (-dx / len, -dy / len)
                } else if qx > qy {
                    (-rx.signum(), 0.0)
                } else {
                    (0.0, -ry.signum())
                };

                // 中心区域轻微凸透镜聚焦 (0.985)
                let base_u = 0.5 + (fx / tw - 0.5) * 0.985;
                let base_v = 0.5 + (fy / th - 0.5) * 0.985;

                let (nx, ny, nz, bezel_factor) = if depth_in < bezel_w {
                    let x = (depth_in / bezel_w).clamp(0.0, 1.0);
                    let one_minus_x = 1.0 - x;
                    // Convex Squircle 高度导数 h'(x) = (1 - x)^3 / max(1 - (1 - x)^4, 0.008)^(3/4)
                    let base_val = (1.0 - one_minus_x.powi(4)).max(0.008);
                    let slope = (one_minus_x.powi(3) / base_val.powf(0.75)).min(3.6);
                    let (nx, ny, nz) = norm3(gx * slope * 0.62, gy * slope * 0.62, 1.0);
                    (nx, ny, nz, one_minus_x)
                } else {
                    (0.0, 0.0, 1.0, 0.0)
                };

                // 按 Snell's Law 计算 R/G/B 差异折射偏移（n_R=1.48, n_G=1.52, n_B=1.57）
                let (ru, rv) = snell_offset(nx, ny, nz, 1.0 / 1.48, thickness);
                let (gu, gv) = snell_offset(nx, ny, nz, 1.0 / 1.52, thickness);
                let (bu, bv) = snell_offset(nx, ny, nz, 1.0 / 1.57, thickness);

                let r = sample_buf_bilinear(&src_buf, target_w, target_h, base_u + ru, base_v + rv, 0);
                let g = sample_buf_bilinear(&src_buf, target_w, target_h, base_u + gu, base_v + gv, 1);
                let b = sample_buf_bilinear(&src_buf, target_w, target_h, base_u + bu, base_v + bv, 2);

                // Blinn-Phong 镜面双角高光 Specular Rim
                let ndoth1 = (nx * h1x + ny * h1y + nz * h1z).max(0.0);
                let ndoth2 = (nx * h2x + ny * h2y + nz * h2z).max(0.0);
                let spec = (ndoth1.powi(20) * 0.68 + ndoth2.powi(18) * 0.36) * bezel_factor;
                let rim_dark = if depth_in < 1.5 { 0.94 } else { 1.0 };

                let di = ((py * target_w + px) * 4) as usize;
                buf[di] = (r * rim_dark + spec * 255.0).round().clamp(0.0, 255.0) as u8;
                buf[di + 1] = (g * rim_dark + spec * 255.0).round().clamp(0.0, 255.0) as u8;
                buf[di + 2] = (b * rim_dark + spec * 255.0).round().clamp(0.0, 255.0) as u8;
                buf[di + 3] = 255;
            }
        }

        let avg_luma = compute_luma(&buf);
        let sample = WallpaperSample {
            image: ImageData {
                data: Blob::new(Arc::new(buf)),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: target_w,
                height: target_h,
            },
            avg_luma,
        };
        if self.crops.len() > 128 {
            self.crops.clear();
        }
        self.crops.insert(key, sample.clone());
        Some(sample)
    }
}

#[inline]
fn norm3(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let len = (x * x + y * y + z * z).sqrt().max(1e-6);
    (x / len, y / len, z / len)
}

#[inline]
fn snell_offset(nx: f64, ny: f64, nz: f64, eta: f64, thickness: f64) -> (f64, f64) {
    // 视线向量 I = (0, 0, -1)，折射法线朝 +Z 时点积 I·N = -nz
    let cos1 = nz.clamp(0.0, 1.0);
    let k = 1.0 - eta * eta * (1.0 - cos1 * cos1);
    if k <= 0.0 {
        return (nx * thickness, ny * thickness);
    }
    let factor = eta * cos1 - k.sqrt();
    // 水平折射方向取 R = eta * I + factor * N 的 XY 分量
    (factor * nx * thickness, factor * ny * thickness)
}

#[inline]
fn boost_saturation(buf: &mut [u8], sat: f32) {
    for px in buf.chunks_exact_mut(4) {
        let r = px[0] as f32;
        let g = px[1] as f32;
        let b = px[2] as f32;
        let luma = 0.299 * r + 0.587 * g + 0.114 * b;
        px[0] = (luma + (r - luma) * sat).clamp(0.0, 255.0) as u8;
        px[1] = (luma + (g - luma) * sat).clamp(0.0, 255.0) as u8;
        px[2] = (luma + (b - luma) * sat).clamp(0.0, 255.0) as u8;
    }
}

#[inline]
fn sample_buf_bilinear(buf: &[u8], w: u32, h: u32, u: f64, v: f64, ch: usize) -> f64 {
    let u = u.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let x = u * w.saturating_sub(1) as f64;
    let y = v * h.saturating_sub(1) as f64;
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(w - 1);
    let y1 = (y0 + 1).min(h - 1);
    let fx = x - x0 as f64;
    let fy = y - y0 as f64;
    let read = |px: u32, py: u32| -> f64 { buf[((py * w + px) * 4) as usize + ch] as f64 };
    let top = read(x0, y0) + (read(x1, y0) - read(x0, y0)) * fx;
    let bot = read(x0, y1) + (read(x1, y1) - read(x0, y1)) * fx;
    top + (bot - top) * fy
}

fn sample_bilinear(wall: &LoadedWallpaper, u: f64, v: f64) -> [f32; 4] {
    let u = u.clamp(0.0, 1.0);
    let v = v.clamp(0.0, 1.0);
    let x = u * (wall.width.saturating_sub(1)) as f64;
    let y = v * (wall.height.saturating_sub(1)) as f64;
    let x0 = x.floor() as u32;
    let y0 = y.floor() as u32;
    let x1 = (x0 + 1).min(wall.width - 1);
    let y1 = (y0 + 1).min(wall.height - 1);
    let fx = (x - x0 as f64) as f32;
    let fy = (y - y0 as f64) as f32;

    let read_px = |px: u32, py: u32| -> [f32; 4] {
        let i = ((py * wall.width + px) * 4) as usize;
        [
            wall.rgba[i] as f32,
            wall.rgba[i + 1] as f32,
            wall.rgba[i + 2] as f32,
            wall.rgba[i + 3] as f32,
        ]
    };

    let c00 = read_px(x0, y0);
    let c10 = read_px(x1, y0);
    let c01 = read_px(x0, y1);
    let c11 = read_px(x1, y1);
    let mut out = [0.0f32; 4];
    for ch in 0..4 {
        let top = c00[ch] + (c10[ch] - c00[ch]) * fx;
        let bot = c01[ch] + (c11[ch] - c01[ch]) * fx;
        out[ch] = top + (bot - top) * fy;
    }
    out
}

fn compute_luma(buf: &[u8]) -> f32 {
    if buf.len() < 4 {
        return 0.5;
    }
    let count = (buf.len() / 4) as f64;
    let mut sum = 0.0_f64;
    for px in buf.chunks_exact(4) {
        sum += 0.299 * px[0] as f64 + 0.587 * px[1] as f64 + 0.114 * px[2] as f64;
    }
    (sum / count / 255.0) as f32
}

/// 高速 O(W×H) 分离式双向盒式模糊（多次迭代逼近真高斯模糊）。
fn box_blur_rgba(buf: &mut [u8], w: usize, h: usize, radius: usize) {
    if w == 0 || h == 0 || radius == 0 {
        return;
    }
    let mut tmp = vec![0u8; buf.len()];
    let r = radius as isize;
    let div = (2 * radius + 1) as u32;

    for y in 0..h {
        let row = y * w * 4;
        for x in 0..w {
            let mut sum = [0u32; 4];
            for k in -r..=r {
                let sx = (x as isize + k).clamp(0, (w - 1) as isize) as usize;
                let idx = row + sx * 4;
                sum[0] += buf[idx] as u32;
                sum[1] += buf[idx + 1] as u32;
                sum[2] += buf[idx + 2] as u32;
                sum[3] += buf[idx + 3] as u32;
            }
            let di = row + x * 4;
            tmp[di] = (sum[0] / div) as u8;
            tmp[di + 1] = (sum[1] / div) as u8;
            tmp[di + 2] = (sum[2] / div) as u8;
            tmp[di + 3] = (sum[3] / div) as u8;
        }
    }

    for x in 0..w {
        for y in 0..h {
            let mut sum = [0u32; 4];
            for k in -r..=r {
                let sy = (y as isize + k).clamp(0, (h - 1) as isize) as usize;
                let idx = (sy * w + x) * 4;
                sum[0] += tmp[idx] as u32;
                sum[1] += tmp[idx + 1] as u32;
                sum[2] += tmp[idx + 2] as u32;
                sum[3] += tmp[idx + 3] as u32;
            }
            let di = (y * w + x) * 4;
            buf[di] = (sum[0] / div) as u8;
            buf[di + 1] = (sum[1] / div) as u8;
            buf[di + 2] = (sum[2] / div) as u8;
            buf[di + 3] = (sum[3] / div) as u8;
        }
    }
}

/// 调用 Win32 `SystemParametersInfoW` 查询当前用户桌面壁纸路径。
fn query_system_wallpaper() -> Option<PathBuf> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use windows::Win32::UI::WindowsAndMessaging::{
        SystemParametersInfoW, SPI_GETDESKWALLPAPER, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
    };
    let mut wide = vec![0u16; 520];
    unsafe {
        if SystemParametersInfoW(
            SPI_GETDESKWALLPAPER,
            wide.len() as u32,
            Some(wide.as_mut_ptr() as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_err()
        {
            return None;
        }
    }
    let len = wide.iter().position(|&c| c == 0).unwrap_or(wide.len());
    if len == 0 {
        return None;
    }
    let path = PathBuf::from(OsString::from_wide(&wide[..len]));
    path.is_file().then_some(path)
}

use std::{sync::mpsc, thread};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
enum Slot {
    Sidebar,
    Card(String),
}

/// 包含显示器尺寸与量化位置；DPI 或屏幕布局改变后不会命中旧切片。
#[derive(Clone, PartialEq, Eq)]
struct SliceKey {
    monitor: String,
    size: (u64, u64),
    area: [i32; 4],
}

struct SliceRequest {
    slot: Slot,
    key: SliceKey,
    area: Rect2D,
    size: (f64, f64),
}

#[derive(Default)]
struct SliceCache {
    pending: bool,
    ready: Option<(SliceKey, u64)>,
    last: Option<WallpaperSample>,
}

enum Work {
    Probe,
    Slice(SliceRequest),
}

enum Ready {
    Probe(bool),
    Slice(Slot, SliceKey, Option<WallpaperSample>),
}

/// 渲染端只提交有限的请求并取走结果；系统探测、解码和模糊均由单个后台线程串行处理。
pub struct AsyncWallpaperCache {
    work: Option<mpsc::SyncSender<Work>>,
    worker: Option<thread::JoinHandle<()>>,
    ready: mpsc::Receiver<Ready>,
    slots: HashMap<Slot, SliceCache>,
    revision: u64,
    last_probe_sec: f64,
    probing: bool,
    stopping: Arc<std::sync::atomic::AtomicBool>,
}

impl AsyncWallpaperCache {
    pub fn new(wake: fn()) -> Self {
        Self::spawn(WallpaperCache::new(), wake, WallpaperCache::probe_system)
    }

    fn spawn(
        mut wallpaper: WallpaperCache,
        wake: impl Fn() + Send + 'static,
        mut probe: impl FnMut(&mut WallpaperCache) -> bool + Send + 'static,
    ) -> Self {
        let (work, requests) = mpsc::sync_channel(16);
        let (results, ready) = mpsc::channel();
        let stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop = stopping.clone();
        let worker = thread::Builder::new().name("allegro-wallpaper".into()).spawn(move || {
            while let Ok(request) = requests.recv() {
                if stop.load(std::sync::atomic::Ordering::Acquire) {
                    break;
                }
                let result = match request {
                    Work::Probe => Ready::Probe(probe(&mut wallpaper)),
                    Work::Slice(req) => {
                        let monitor = format!("{}:{:?}", req.key.monitor, req.key.size);
                        let sample = match req.slot {
                            Slot::Sidebar => wallpaper.crop_sidebar_frost(&monitor, req.size, req.area),
                            Slot::Card(_) => wallpaper.crop_acrylic(&monitor, req.size, req.area),
                        };
                        Ready::Slice(req.slot, req.key, sample)
                    }
                };
                if results.send(result).is_err() {
                    break;
                }
                wake();
            }
        }).map_err(|err| eprintln!("Allegro wallpaper thread: {err}")).ok();
        Self { work: Some(work), worker, ready, slots: HashMap::new(), revision: 0, last_probe_sec: -5.0, probing: false, stopping }
    }

    /// 消费已完成任务（包括无壁纸的结果），然后按 5 秒间隔异步探测；失败时保留旧底图。
    pub fn poll(&mut self, now_sec: f64) -> bool {
        let mut redraw = false;
        while let Ok(result) = self.ready.try_recv() {
            match result {
                Ready::Probe(replaced) => {
                    self.probing = false;
                    if replaced {
                        self.revision = self.revision.wrapping_add(1);
                    }
                    redraw |= replaced;
                }
                Ready::Slice(slot, key, sample) => {
                    if let Some(cache) = self.slots.get_mut(&slot) {
                        cache.pending = false;
                        cache.ready = Some((key, self.revision));
                        if sample.is_some() {
                            cache.last = sample;
                        }
                    }
                    // 即使没有壁纸，也要让此前队列满时未提交的卡片有机会补交。
                    redraw = true;
                }
            }
        }
        if !self.probing && now_sec - self.last_probe_sec >= 5.0 && self.work.as_ref().is_some_and(|tx| tx.try_send(Work::Probe).is_ok()) {
            self.probing = true;
            self.last_probe_sec = now_sec;
        }
        redraw
    }

    pub fn sidebar(&mut self, monitor: &str, size: (f64, f64), area: Rect2D) -> Option<WallpaperSample> {
        self.sample(Slot::Sidebar, monitor, size, area, true)
    }

    pub fn acrylic(&mut self, id: &str, monitor: &str, size: (f64, f64), area: Rect2D, moving: bool) -> Option<WallpaperSample> {
        self.sample(Slot::Card(id.into()), monitor, size, area, !moving)
    }

    pub fn retain_cards(&mut self, keep: impl Fn(&str) -> bool) {
        // 先保留仍在途的槽位，防止同一 ID 重建时把旧结果误认成新请求。
        self.slots.retain(|slot, cache| cache.pending || match slot {
            Slot::Sidebar => true,
            Slot::Card(id) => keep(id),
        });
    }

    fn sample(&mut self, slot: Slot, monitor: &str, size: (f64, f64), area: Rect2D, request: bool) -> Option<WallpaperSample> {
        let cache = self.slots.entry(slot.clone()).or_default();
        if request && !cache.pending {
            let (step, density, w_range, h_range) = match slot {
                Slot::Sidebar => (8.0, 0.22, (48, 128), (96, 240)),
                Slot::Card(_) => (4.0, 0.32, (24, 200), (24, 200)),
            };
            let key = SliceKey {
                monitor: monitor.into(),
                size: (size.0.to_bits(), size.1.to_bits()),
                area: [(area.x / step).round() as i32, (area.y / step).round() as i32,
                    ((area.w * density).round() as i32).clamp(w_range.0, w_range.1),
                    ((area.h * density).round() as i32).clamp(h_range.0, h_range.1)],
            };
            if !cache.ready.as_ref().is_some_and(|(old, revision)| *old == key && *revision == self.revision) {
                cache.pending = self.work.as_ref().is_some_and(|tx| tx.try_send(Work::Slice(SliceRequest { slot, key, area, size })).is_ok());
            }
        }
        // 首次无缓存时直接使用既有的纯色材质；后台替换前可沿用上次成功的纹理。
        cache.last.clone()
    }
}

impl Drop for AsyncWallpaperCache {
    fn drop(&mut self) {
        // 事件循环退出后才回收线程，并跳过剩余切片任务。
        self.stopping.store(true, std::sync::atomic::Ordering::Release);
        self.work.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

#[cfg(test)]
mod async_tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn first_frame_falls_back_and_reprobe_keeps_last_slice_until_new_pixels_arrive() {
        let (allow, permits) = mpsc::channel();
        let (wake, woken) = mpsc::channel();
        let mut cache = AsyncWallpaperCache::spawn(WallpaperCache::new(), move || { let _ = wake.send(()); }, move |wall| {
            let shade = permits.recv_timeout(Duration::from_secs(3)).unwrap();
            let rgba = [shade, shade, shade, 255].repeat(64 * 64);
            wall.set_synthetic_wallpaper(64, 64, rgba);
            true
        });
        let size = (1920.0, 1080.0);
        let area = Rect2D::new(1500.0, 12.0, 400.0, 1056.0);
        let await_wake = || woken.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(!cache.poll(0.0));
        assert!(cache.sidebar("m1", size, area).is_none());
        // 同一个槽位只允许一个切片在途，而不是每帧排队计算。
        assert!(cache.sidebar("m1", size, area).is_none());
        allow.send(30).unwrap();
        await_wake(); // 探测与降采样完成
        await_wake(); // 切片完成
        assert!(cache.poll(0.0));
        let first = cache.sidebar("m1", size, area).unwrap();
        assert_eq!(cache.revision, 1);

        assert!(!cache.poll(5.0));
        assert_eq!(cache.sidebar("m1", size, area).unwrap().avg_luma, first.avg_luma);
        allow.send(230).unwrap();
        await_wake();
        assert!(cache.poll(5.0));
        assert_eq!(cache.sidebar("m1", size, area).unwrap().avg_luma, first.avg_luma);
        await_wake();
        assert!(cache.poll(5.0));
        assert!(cache.sidebar("m1", size, area).unwrap().avg_luma > first.avg_luma);
    }

    #[test]
    fn missing_wallpaper_does_not_resubmit_same_slice_every_frame() {
        let (wake, woken) = mpsc::channel();
        let mut cache = AsyncWallpaperCache::spawn(WallpaperCache::new(), move || { let _ = wake.send(()); }, |_| false);
        let area = Rect2D::new(12.0, 12.0, 176.0, 176.0);
        let size = (1920.0, 1080.0);
        assert!(cache.acrylic("a", "m", size, area, false).is_none());
        woken.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(cache.poll(-1.0)); // 不启动系统探测
        assert!(cache.acrylic("a", "m", size, area, false).is_none());
        assert!(!cache.slots[&Slot::Card("a".into())].pending);
        assert!(cache.acrylic("a", "m", (1280.0, 720.0), area, false).is_none());
        assert!(cache.slots[&Slot::Card("a".into())].pending);
    }
}
