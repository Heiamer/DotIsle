//! Vello island painter. The v2 layout, activity model, hit targets and motion are retained.
use crate::{
    layout::{Layout, Rect},
    motion::Frame,
};
use dotisle_render::draw::{Icon, LyricHalo, Text};
use std::{
    collections::HashMap,
    path::Path,
    sync::Arc,
    time::{Duration, Instant},
};
use vello::{
    kurbo::{Affine, BezPath, Cap, Circle, Line, RoundedRect, Stroke},
    peniko::{Blob, Color, Fill, Gradient, ImageAlphaType, ImageData, ImageFormat, Mix},
    Scene as VScene,
};
use windows::{
    core::{Error, Result, HRESULT},
    Win32::Foundation::HWND,
};
fn err(e: impl ToString) -> Error {
    Error::new(HRESULT(0x80004005u32 as i32), e.to_string())
}
fn vr(r: Rect) -> vello::kurbo::Rect {
    vello::kurbo::Rect::new(r.x, r.y, r.x + r.w, r.y + r.h)
}
pub fn color(argb: u32) -> Color {
    Color::from_rgba8(
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
        (argb >> 24) as u8,
    )
}
fn mix_rgb(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0., 1.);
    let ch = |shift: u32| -> u32 {
        let av = ((a >> shift) & 255) as f64;
        let bv = ((b >> shift) & 255) as f64;
        (av + (bv - av) * t).round().clamp(0., 255.) as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

pub struct PreparedCover {
    image: ImageData,
    pub seed: Option<u32>,
}
impl PreparedCover {
    pub fn save_preview(&self, path: &Path) -> std::result::Result<(), String> {
        image::save_buffer(
            path,
            self.image.data.data(),
            self.image.width,
            self.image.height,
            image::ColorType::Rgba8,
        )
        .map_err(|e| e.to_string())
    }
}
pub struct Renderer {
    transport_ink: Option<(u32, Vec<u32>, u32)>,
    window: HWND,
    gpu: crate::vello_surface::Surface,
    canvas: VScene,
    text: Text,
    lyric_halo: LyricHalo,
    blurred_cover: Option<(u32, ImageData)>,
    cover: Option<ImageData>,
    previous_cover: Option<ImageData>,
    notification_icon: Option<ImageData>,
    device_images: HashMap<String, ImageData>,
    cover_palette: crate::cover_color::CoverPalette,
    paint_palette: crate::cover_color::CoverPalette,
    cover_seed: Option<u32>,
    dynamic_resolved: Option<dotisle_domain::dynamic_color::Resolved>,
    color_motion: dotisle_domain::dynamic_color::RoleMotion,
    m3_cache: Option<(u32, bool, dotisle_domain::monet::Scheme)>,
    cover_changed: Instant,
    flip_from: f64,
    pub body_line_spacing: f64,
    pub body_baseline: f64,
    pub width: u32,
    pub height: u32,
    pub dpi: f32,
    liquid_glass_shapes: Vec<dotisle_render::liquid_glass::GlassShape>,
    liquid_glass_backdrop_luma: Option<f32>,
    active_lg_refraction: f32,
    active_lg_dispersion: f32,
    active_lg_blur: f32,
    active_lg_tint: f32,
    active_lg_opacity: f32,
    active_lg_scale: f32,
}
impl Renderer {
    pub unsafe fn new(hwnd: HWND, width: u32, height: u32, dpi: f32, font: &Path) -> Result<Self> {
        let gpu = crate::vello_surface::Surface::new_island(hwnd, width, height).map_err(err)?;
        let mut text = Text::empty();
        text.register(font).map_err(err)?;
        // Google Sans Flex covers Latin and digits; every weight is bundled.
        for name in dotisle_render::draw::LATIN_FONTS {
            let _ = text.register(&font.with_file_name(name));
        }
        // Register the Japanese fallback only when text first requires it.
        let japanese = font.with_file_name("NotoSansJP.ttf");
        text.defer_japanese(japanese);

        Ok(Self {
            transport_ink: None,
            window: hwnd,
            gpu,
            canvas: VScene::new(),
            text,
            lyric_halo: LyricHalo::default(),
            color_motion: Default::default(),
            m3_cache: None,
            blurred_cover: None,
            cover: None,
            previous_cover: None,
            notification_icon: None,
            device_images: HashMap::new(),
            cover_palette: Default::default(),
            paint_palette: Default::default(),
            cover_seed: None,
            dynamic_resolved: None,
            cover_changed: Instant::now() - Duration::from_secs(1),
            flip_from: 0.,
            body_line_spacing: 1.448,
            body_baseline: 1.16,
            width,
            height,
            dpi,
            liquid_glass_shapes: Vec::new(),
            liquid_glass_backdrop_luma: None,
            active_lg_refraction: 0.76,
            active_lg_dispersion: 0.46,
            active_lg_blur: 0.08,
            active_lg_tint: 0.06,
            active_lg_opacity: 1.0,
            active_lg_scale: dpi / 96.,
        })
    }
    /// 当前液态玻璃背后是否为浅色/亮色背景（需切换深墨前景色以确保清晰可读）。
    fn is_liquid_glass_light(&self) -> bool {
        self.liquid_glass_backdrop_luma.unwrap_or(0.0) > 0.58
    }
    fn light_glass_ink(&self, ink: u32) -> u32 {
        // Preserve the requested hue/alpha, but avoid near-white ink on a bright pane.
        let channels = [(ink >> 16) & 255, (ink >> 8) & 255, ink & 255];
        let peak = channels[0].max(channels[1]).max(channels[2]);
        if peak <= 60 { return ink; }
        (ink & 0xff000000) | ((channels[0] * 60 / peak) << 16)
            | ((channels[1] * 60 / peak) << 8) | (channels[2] * 60 / peak)
    }
    /// 向液态玻璃 Beta 管线更新裁剪后的桌面/实时画面背景纹理，并轻量采样平均亮度供前景对比度自适应。
    pub fn update_liquid_glass_backdrop(&mut self, rgba: &[u8], w: u32, h: u32) {
        if w > 0 && h > 0 && rgba.len() >= (w as usize) * (h as usize) * 4 {
            let mut sum = 0.0f32;
            let mut count = 0.0f32;
            let gx = 8u32;
            let gy = 8u32;
            for iy in 0..gy {
                let y = ((iy * 2 + 1) * h / (gy * 2)).min(h - 1);
                for ix in 0..gx {
                    let x = ((ix * 2 + 1) * w / (gx * 2)).min(w - 1);
                    let idx = ((y * w + x) * 4) as usize;
                    let r = rgba[idx] as f32 / 255.0;
                    let g = rgba[idx + 1] as f32 / 255.0;
                    let b = rgba[idx + 2] as f32 / 255.0;
                    sum += 0.2126 * r + 0.7152 * g + 0.0722 * b;
                    count += 1.0;
                }
            }
            if count > 0.0 {
                let luma = sum / count;
                self.liquid_glass_backdrop_luma = Some(match self.liquid_glass_backdrop_luma {
                    Some(prev) => prev * 0.65 + luma * 0.35,
                    None => luma,
                });
            }
        }
        self.gpu.update_liquid_glass_backdrop(rgba, w, h);
    }
    /// 切回其他材质时主动释放液态玻璃离屏纹理与着色器管线。
    pub fn release_liquid_glass(&mut self) {
        self.liquid_glass_shapes.clear();
        self.liquid_glass_backdrop_luma = None;
        self.gpu.release_liquid_glass();
    }
    fn fill(&mut self, r: Rect, radius: f64, c: u32) {
        if r.w > 0. && r.h > 0. {
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                color(c),
                None,
                &RoundedRect::from_rect(vr(r), radius),
            );
        }
    }
    fn outline(&mut self, r: Rect, radius: f64, c: u32, width: f64) {
        self.canvas.stroke(
            &Stroke::new(width),
            Affine::IDENTITY,
            color(c),
            None,
            &RoundedRect::from_rect(vr(r), radius),
        );
    }
    fn line(&mut self, a: (f64, f64), b: (f64, f64), c: u32, width: f64) {
        self.canvas.stroke(
            &Stroke::new(width).with_caps(vello::kurbo::Cap::Round),
            Affine::IDENTITY,
            color(c),
            None,
            &Line::new(a, b),
        );
    }
    fn ring(&mut self, x: f64, y: f64, r: f64, c: u32, width: f64) {
        self.canvas.stroke(
            &Stroke::new(width),
            Affine::IDENTITY,
            color(c),
            None,
            &Circle::new((x, y), r),
        );
    }
    /// Determinate byte progress or an indeterminate rotating arc; never a fake percentage.
    fn update_ring(
        &mut self,
        x: f64,
        y: f64,
        radius: f64,
        progress: f64,
        time: f64,
        animate: bool,
        accent: u32,
    ) {
        let track = self.m3_cache.map(|(_, _, c)| c.outline).unwrap_or(0x43534c);
        self.ring(x, y, radius, 0xff000000 | track, 3.);
        let fraction = if progress < 0. {
            0.22
        } else {
            progress.clamp(0., 1.)
        };
        let start = if progress < 0. && animate {
            time / 1200. * std::f64::consts::TAU
        } else {
            -std::f64::consts::FRAC_PI_2
        };
        if fraction >= 0.9999 {
            self.ring(x, y, radius, accent, 3.);
            return;
        }
        if fraction <= 0. {
            return;
        }
        let mut path = BezPath::new();
        let steps = (fraction * 120.).ceil().max(2.) as usize;
        for i in 0..=steps {
            let angle = start + fraction * std::f64::consts::TAU * i as f64 / steps as f64;
            let p = (x + radius * angle.cos(), y + radius * angle.sin());
            if i == 0 {
                path.move_to(p)
            } else {
                path.line_to(p)
            }
        }
        self.canvas.stroke(
            &Stroke::new(3.).with_caps(Cap::Round),
            Affine::IDENTITY,
            color(accent),
            None,
            &path,
        );
    }
    fn clip(&mut self, r: Rect, radius: f64, alpha: f32) {
        self.canvas.push_layer(
            Mix::Normal,
            alpha.clamp(0., 1.),
            Affine::IDENTITY,
            &RoundedRect::from_rect(vr(r), radius),
        );
    }
    fn text_at(&mut self, text: &str, r: Rect, size: f64, weight: i32, c: u32, align: f64) {
        if r.w <= 0. || r.h <= 0. {
            return;
        }
        // Baseline anchoring keeps every installed family at the same optical centre.
        self.text.line_fixed(
            &mut self.canvas,
            text,
            vr(r),
            size as f32,
            c,
            weight.clamp(0, 1000) as u16,
            align,
        );
    }
    fn text_mono(&mut self, text: &str, r: Rect, size: f64, weight: i32, c: u32, align: f64) {
        if r.w <= 0. || r.h <= 0. {
            return;
        }
        self.text.line_face(
            &mut self.canvas,
            "Space Mono",
            text,
            vr(r),
            size as f32,
            c,
            weight.clamp(0, 1000) as u16,
            align,
        );
    }
    fn text_line(&mut self, text: &str, r: Rect, size: f64, weight: i32, c: u32) {
        self.text_at(text, r, size, weight, c, 0.);
    }
    fn image(&mut self, image: &ImageData, r: Rect, radius: f64, opacity: f32) {
        self.clip(r, radius, opacity);
        dotisle_render::draw::image(&mut self.canvas, image, vr(r), radius);
        self.canvas.pop_layer();
    }
    fn icon(&mut self, kind: Icon, r: Rect, c: u32) {
        self.clip(r, 0., (c >> 24) as f32 / 255.);
        if matches!(kind, Icon::Focus) {
            dotisle_render::control_icons::icon(
                &mut self.canvas,
                kind,
                r.x,
                r.y,
                r.w,
                c & 0xffffff,
            );
        } else {
            dotisle_render::draw::icon(&mut self.canvas, kind, r.x, r.y, r.w, c & 0xffffff);
        }
        self.canvas.pop_layer();
    }
    fn collect(&mut self, draw: impl FnOnce(&mut Self)) -> VScene {
        let outer = std::mem::take(&mut self.canvas);
        draw(self);
        std::mem::replace(&mut self.canvas, outer)
    }
    pub fn set_font_family(&mut self, family: &str) {
        self.text.set_family(family);
    }
    pub unsafe fn measure_line(&mut self, text: &str, size: f64) -> Result<f64> {
        Ok(self.text.measure(text, size as f32, false))
    }
    /// Weight-aware width for text that is painted at a non-default weight.
    pub unsafe fn measure_line_weighted(&mut self, text: &str, size: f64, weight: u16) -> Result<f64> {
        Ok(self.text.measure_weighted(text, size as f32, weight))
    }
    pub fn status_indicator(&mut self, _label: &str, dot: Rect, text: Rect) -> Rect {
        let center = text.y + text.h / 2.;
        Rect::new(dot.x, center - dot.h / 2., dot.w, dot.h)
    }
    pub fn compact_metrics(&self, font: f64, double: bool, scale: f64) -> (f64, f64, f64) {
        crate::compact::group_metrics(font, self.body_line_spacing, scale, double)
    }
    pub fn compact_height(&self, font: f64, double: bool, scale: f64) -> f64 {
        crate::compact::capsule_height(font, self.body_line_spacing, scale, double)
    }
    fn decode(
        bytes: &[u8],
        max_side: u32,
    ) -> std::result::Result<ImageData, Box<dyn std::error::Error>> {
        let mut reader =
            image::ImageReader::new(std::io::Cursor::new(bytes)).with_guessed_format()?;
        reader.limits(image::Limits::default());
        let image = reader.decode()?;
        let image = if image.width().max(image.height()) > max_side {
            image.thumbnail(max_side, max_side)
        } else {
            image
        }.into_rgba8();
        let (w, h) = image.dimensions();
        Ok(ImageData {
            data: Blob::new(Arc::new(image.into_raw())),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::Alpha,
            width: w,
            height: h,
        })
    }
    pub fn prepare_cover(
        bytes: Option<&[u8]>,
    ) -> std::result::Result<Option<PreparedCover>, String> {
        let decoded = bytes
            .map(|bytes| Self::decode(bytes, 512))
            .transpose()
            .map_err(|e| e.to_string())?;
        Ok(decoded.map(|image| {
            let small = image::ImageBuffer::<image::Rgba<u8>, _>::from_raw(image.width, image.height, image.data.data())
                .map(|rgba| image::imageops::thumbnail(&rgba, 64, 64));
            let seed = small
                .as_ref()
                .and_then(|p| dotisle_domain::monet::seed(p.as_raw()));
            PreparedCover {
                image,
                seed,
            }
        }))
    }
    pub fn apply_cover(&mut self, prepared: Option<PreparedCover>) {
        let phase = (self.flip_from
            + (1. - self.flip_from) * self.cover_changed.elapsed().as_secs_f64() / 0.28)
            .clamp(0., 1.);
        if phase >= 0.5 || self.previous_cover.is_none() {
            self.previous_cover = self.cover.clone();
            self.flip_from = 1. - phase;
        } else {
            self.flip_from = phase;
        }
        self.cover = None;
        self.blurred_cover = None;
        self.cover_palette = Default::default();
        self.cover_seed = None;
        if let Some(p) = prepared {
            self.cover_seed = p.seed;
            if let Some(seed) = p.seed {
                let roles = dotisle_domain::dynamic_color::roles(seed, true, true, false);
                self.cover_palette = crate::cover_color::CoverPalette {
                    background: roles.surface_container_high,
                    accent: roles.primary,
                };
            }
            self.paint_palette = self.cover_palette;
            self.cover = Some(p.image);
        }
        self.cover_changed = Instant::now();
    }
    pub fn clear_track_cover(&mut self) {
        self.cover = None;
        self.previous_cover = None;
        self.blurred_cover = None;
        self.cover_seed = None;
        self.cover_palette = Default::default();
        self.paint_palette = Default::default();
        self.flip_from = 1.;
    }
    pub unsafe fn set_cover(
        &mut self,
        bytes: Option<&[u8]>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let prepared = Self::prepare_cover(bytes)?;
        self.apply_cover(prepared);
        Ok(())
    }
    pub(crate) fn decode_notification_icon(bytes: &[u8]) -> std::result::Result<ImageData, Box<dyn std::error::Error>> {
        Self::decode(bytes, 128)
    }
    pub unsafe fn set_notification_icon(
        &mut self,
        bytes: Option<&[u8]>,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.notification_icon = bytes.map(Self::decode_notification_icon).transpose()?;
        Ok(())
    }
    pub fn material_theme(&self, material: Material, cover_background: bool) -> MaterialTheme {
        MaterialTheme {
            roles: self.dynamic_resolved.map(|v| v.roles),
            material,
            cover_background,
            palette: self.paint_palette,
            cover: if cover_background { self.cover.clone() } else { None },
        }
    }
    pub unsafe fn set_material_theme(&mut self, theme: &MaterialTheme) -> Result<()> {
        if let Some(roles) = theme.roles {
            let mut resolved = dotisle_domain::dynamic_color::resolve(
                dotisle_domain::dynamic_color::Source::Custom,
                None,
                None,
                0x496b55,
                true,
                false,
            );
            resolved.roles = roles;
            self.dynamic_resolved = Some(resolved);
            self.m3_cache = Some((resolved.seed, true, roles.ui()));
        }
        self.cover_palette = theme.palette;
        self.paint_palette = theme.palette;
        self.blurred_cover = None;
        self.cover = theme.cover.clone();
        Ok(())
    }
    pub fn preview_palette(&self) -> (u32, u32) {
        (self.paint_palette.background, self.paint_palette.accent)
    }
    pub fn preview_art(&self, path: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
        if let Some(image) = &self.cover {
            if let Some(p) = path.parent() {
                std::fs::create_dir_all(p)?;
            }
            image::save_buffer(
                path,
                image.data.data(),
                image.width,
                image.height,
                image::ColorType::Rgba8,
            )?;
        }
        Ok(())
    }
    fn cover(&mut self, r: Rect, radius: f64, opacity: f32) {
        if let Some(image) = self.cover.clone() {
            self.image(&image, r, radius, opacity);
        } else {
            let tones = self
                .m3_cache
                .map(|(_, _, c)| c)
                .unwrap_or_else(|| dotisle_domain::dynamic_color::scheme(0x496b55, true));
            self.fill(r, radius, 0xff000000 | tones.secondary_container);
            let size = r.w * 0.56;
            self.icon(
                Icon::Music,
                Rect::new(r.x + (r.w - size) / 2., r.y + (r.h - size) / 2., size, size),
                0xff000000 | tones.on_secondary_container,
            );
        }
    }
    fn flipping_cover(&mut self, r: Rect, s: &Scene<'_>) {
        let radius = crate::layout::cover_corner_radius(s.radius, r.w, r.h);
        let t = if s.animate {
            (self.flip_from + (1. - self.flip_from) * s.cover_age_ms / 280.).clamp(0., 1.)
        } else {
            1.
        };
        if t >= 1. || self.previous_cover.is_none() {
            self.cover(r, radius, s.cover_opacity as f32);
            return;
        }
        let sx = (std::f64::consts::PI * t).cos().abs().max(0.018);
        let sy = 1. - 0.06 * (std::f64::consts::PI * t).sin();
        let image = if t < 0.5 {
            self.previous_cover.clone()
        } else {
            self.cover.clone()
        };
        let part = self.collect(|a| {
            if let Some(image) = image {
                a.image(&image, r, radius, s.cover_opacity as f32);
            } else {
                a.cover(r, radius, s.cover_opacity as f32);
            }
        });
        let cx = r.x + r.w / 2.;
        let cy = r.y + r.h / 2.;
        self.canvas.append(
            &part,
            Some(
                Affine::translate((cx, cy))
                    * Affine::scale_non_uniform(sx, sy)
                    * Affine::translate((-cx, -cy)),
            ),
        );
    }
    fn surface_spec(s: &Scene<'_>) -> crate::surface::Surface {
        crate::surface::Surface {
            material: s.material,
            compact: if s.mini {
                s.mini_capsule
            } else {
                s.classic_capsule
            },
            reveal: s.frame.card_reveal,
            overlay: s.notification_mix.max(s.osd_mix).max(s.alert_mix),
            mini: s.mini,
            transparent: s.transparent,
            cover_background: s.expanded_cover_background,
            compact_cover_background: s.cover_background,
            cover_overlay: s.cover_overlay && !s.dynamic_high_contrast,
            card_frost: s.acrylic_card_frost,
            card_opacity: if s.dynamic_high_contrast {
                0.
            } else {
                s.acrylic_card_opacity
            },
            capsule_frost: s.acrylic_capsule_frost,
            capsule_opacity: if s.dynamic_high_contrast {
                0.
            } else {
                s.acrylic_capsule_opacity
            },
        }
    }
    fn music_surface_spec(s: &Scene<'_>) -> crate::surface::Surface {
        let mut surface = Self::surface_spec(s);
        let immersion = lyric_core::music_layout::immersion_amount(
            s.music_layout,
            s.music_mix,
            s.custom_geometry.is_some(),
        );
        if s.panel.is_none()
            && s.notification.is_none()
            && s.osd.is_none() && s.alert.is_none()
            && immersion > 0.
            && surface.material != Material::LiquidGlass
            && !s.dynamic_high_contrast
        {
            surface.material = Material::Acrylic;
            surface.cover_background = s.expanded_cover_background;
            surface.cover_overlay = s.cover_overlay;
            surface.card_frost = surface.card_frost + (64. - surface.card_frost) * immersion;
            surface.card_opacity = surface.card_opacity + (62. - surface.card_opacity) * immersion;
        }
        surface
    }
    fn material_surface(&mut self, r: Rect, radius: f64, s: crate::surface::Surface) {
        self.material_surface_impl(r, radius, s, None, LightFieldCtx::default());
    }
    fn material_surface_impl(&mut self, r: Rect, radius: f64, s: crate::surface::Surface, curve: Option<&BezPath>, ctx: LightFieldCtx) {
        if let Some(path) = curve {
            self.canvas.push_clip_layer(Affine::IDENTITY, path);
        } else {
            self.clip(r, radius, 1.);
        }
        let clipped_rect = curve.is_none();
        let shape_radius = 0.;
        let p = s.paint(self.paint_palette);
        if p.liquid_glass > 0.01
            && self.liquid_glass_shapes.len() < dotisle_render::liquid_glass::MAX_GLASS_SHAPES
        {
            let scale = self.active_lg_scale;
            let mut shape = dotisle_render::liquid_glass::GlassShape::new(
                (r.x as f32) * scale,
                (r.y as f32) * scale,
                (r.w as f32) * scale,
                (r.h as f32) * scale,
                (radius as f32) * scale,
                self.active_lg_opacity,
                self.active_lg_refraction,
                self.active_lg_dispersion,
                self.active_lg_blur,
                self.active_lg_tint,
                p.liquid_glass as f32,
            );
            if let Some(resolved) = self.dynamic_resolved {
                let dye_mix = s.liquid_glass_dye_mix(p.amount);
                if dye_mix > 0. && self.active_lg_tint > 0. {
                    shape = shape.with_body_dye(resolved.seed, self.active_lg_tint, dye_mix);
                }
            }
            self.liquid_glass_shapes.push(shape);
        }
        let argb = |alpha: f64, rgb: u32| -> Color {
            color((((alpha.clamp(0., 1.) * 255.).round() as u32) << 24) | (rgb & 0xffffff))
        };
        if p.liquid_glass > 0.99 {
            // 纯液态玻璃底面完全交给 WGSL 光学折射/色散/高光管线渲染，Vello 仅保留克制专辑叠层（如开启）与前景内容
        } else if p.light_field > 0.99 {
            // 曜光视效（Lumen Glass, LightField）基底：上墨黑丝绒 -> 下通透冰雾玻璃（垂直纵深渐透）
            let gradient = if r.h > 64. {
                Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                    (0.00, color(0xF3050609)),
                    (0.42, color(0xE607090F)),
                    (0.74, color(0xB812182B)),
                    (1.00, color(0x941E2844)),
                ])
            } else {
                Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                    (0.00, color(0xED050609)),
                    (0.55, color(0xD60A0D16)),
                    (1.00, color(0xB8182138)),
                ])
            };
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &gradient,
                None,
                &RoundedRect::from_rect(vr(r), shape_radius),
            );
        } else {
            let gradient = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h))
                .with_stops([color(p.top), color(p.bottom)]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &gradient,
                None,
                &RoundedRect::from_rect(vr(r), shape_radius),
            );
            // 升级版亚克力物理环境光层：左上柔和天光散射 + 极克制主色氛围辉光
            if matches!(s.material, Material::Acrylic) || matches!(s.compact, Material::Acrylic) {
                let sky_sheen = Gradient::new_linear((r.x, r.y), (r.x + r.w * 0.45, r.y + r.h))
                    .with_stops([
                        (0.00, color(0x16FFFFFF)),
                        (0.38, color(0x06FFFFFF)),
                        (1.00, color(0x00FFFFFF)),
                    ]);
                self.canvas.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &sky_sheen,
                    None,
                    &RoundedRect::from_rect(vr(r), shape_radius),
                );
                if p.tint > 0.01 {
                    let aura = Gradient::new_radial(
                        (r.x + r.w * 0.18, r.y + r.h * 0.24),
                        (r.w * 0.65).max(110.) as f32,
                    )
                    .with_stops([
                        (0.0, color((((0.14 * p.tint).clamp(0., 0.24) * 255.) as u32) << 24 | (self.paint_palette.accent & 0xffffff))),
                        (1.0, color(0x00000000)),
                    ]);
                    self.canvas.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &aura,
                        None,
                        &RoundedRect::from_rect(vr(r), shape_radius),
                    );
                }
            }
        }
        if p.cover_alpha > 0.001 {
            if let Some(image) = self.cover.clone() {
                // 根据当前展开态选择对应滑块磨砂度（收起态读 capsule_frost，展开态读 card_frost）
                let target_frost = active_cover_frost(p.amount, s.card_frost, s.capsule_frost);
                let blur_key = target_frost.round() as u32;
                if self
                    .blurred_cover
                    .as_ref()
                    .is_none_or(|(key, _)| *key != blur_key)
                {
                    if let Some(rgba) = image::RgbaImage::from_raw(
                        image.width,
                        image.height,
                        image.data.data().to_vec(),
                    ) {
                        let small = if rgba.width().max(rgba.height()) > 192 {
                            image::imageops::thumbnail(&rgba, 192, 192)
                        } else {
                            rgba
                        };
                        let sigma = acrylic_cover_sigma(target_frost, small.width().max(small.height()));
                        let blurred = if sigma > 0.0 {
                            image::imageops::blur(&small, sigma)
                        } else {
                            small
                        };
                        let (w, h) = blurred.dimensions();
                        let mut pixels = blurred.into_raw();
                        for c in pixels.chunks_exact_mut(4) {
                            let r_f = c[0] as f32 / 255.;
                            let g_f = c[1] as f32 / 255.;
                            let b_f = c[2] as f32 / 255.;
                            let lum = 0.2126 * r_f + 0.7152 * g_f + 0.0722 * b_f;
                            c[0] = tone_acrylic_cover_channel(lum, r_f);
                            c[1] = tone_acrylic_cover_channel(lum, g_f);
                            c[2] = tone_acrylic_cover_channel(lum, b_f);
                        }
                        self.blurred_cover = Some((
                            blur_key,
                            ImageData {
                                data: Blob::new(Arc::new(pixels)),
                                width: w,
                                height: h,
                                ..image.clone()
                            },
                        ));
                    }
                }
                let image = self
                    .blurred_cover
                    .as_ref()
                    .map(|(_, i)| i.clone())
                    .unwrap_or(image);
                self.image(&image, r, shape_radius, p.cover_alpha as f32);
                // 按封面叠层强度比例缩放的柔和可读性护罩，避免低叠层压黑或完全遮盖封面细节；液态玻璃大幅减弱暗灰护罩保持通透
                let scrim_w = p.cover_alpha.powf(0.65) * (1. - 0.14 * p.light_field) * (1. - 0.78 * p.liquid_glass);
                let strip_add = 0.10 * (1. - p.amount);
                let stop_a = |base: f64, cap: f64| -> u32 {
                    (((base + strip_add) * scrim_w).clamp(0., cap) * 255.).round() as u32
                };
                let base_rgb = crate::cover_color::mix(0x0B0F16, self.paint_palette.background, (0.34 * p.tint).clamp(0., 0.45));
                let deep_rgb = crate::cover_color::mix(0x06080D, self.paint_palette.background, (0.20 * p.tint).clamp(0., 0.30));
                let scrim = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                    (0.00, color((stop_a(0.14, 0.62) << 24) | base_rgb)),
                    (0.52, color((stop_a(0.24, 0.70) << 24) | base_rgb)),
                    (1.00, color((stop_a(0.38, 0.78) << 24) | deep_rgb)),
                ]);
                self.canvas.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &scrim,
                    None,
                    &RoundedRect::from_rect(vr(r), shape_radius),
                );
            }
        }
        // 曜光玻璃与亚克力统一享切歌「曜光环」扩散波纹 + 展开呼吸光效
        let morph_env = if ctx.reveal > 0.02 && ctx.reveal < 0.98 {
            (std::f64::consts::PI * ctx.reveal).sin().powf(0.85)
        } else {
            0.
        };
        let switch_t = if ctx.animate && ctx.cover_age_ms >= 0. && ctx.cover_age_ms < 780. {
            (ctx.cover_age_ms / 780.).clamp(0., 1.)
        } else {
            1.
        };
        let switch_env = if switch_t < 1. {
            (std::f64::consts::PI * switch_t).sin().powf(0.80)
        } else {
            0.
        };
        let pulse = morph_env.max(switch_env);
        let box_shape = RoundedRect::from_rect(vr(r), shape_radius);
        let color_mix = (p.tint.max(p.cover_alpha)).clamp(0., 1.);
        let album_glow_hi = mix_rgb(0xB9D4FF, self.paint_palette.accent, 0.78 * color_mix);
        let album_glow_mid = mix_rgb(0x8BAEFF, self.paint_palette.accent, 0.82 * color_mix);
        if p.light_field > 0.001 {
            let lf = p.light_field;
            // 1. 曜光玻璃底部透镜内部全反射集光（开启封面取色/叠层时平滑融入专辑主题辉光）
            let bottom_glow = Gradient::new_radial(
                (r.x + r.w * 0.5, r.y + r.h * 1.06),
                (r.w * 0.58).max(90.) as f32,
            )
            .with_stops([
                (0.00, argb(0.18 * lf, album_glow_hi)),
                (0.48, argb(0.075 * lf, album_glow_mid)),
                (1.00, argb(0.0, album_glow_mid)),
            ]);
            self.canvas.fill(Fill::NonZero, Affine::IDENTITY, &bottom_glow, None, &box_shape);
        }
        let ripple_eff = p
            .light_field
            .max(if matches!(s.material, Material::Acrylic) { 0.80 } else { 0. });
        if ripple_eff > 0.001 {
            let ripple_tint = if p.light_field > 0.5 {
                mix_rgb(0xCFEFFF, self.paint_palette.accent, 0.70 * color_mix)
            } else {
                mix_rgb(0xFFFFFF, self.paint_palette.accent, 0.50)
            };
            // 2. 切歌/封面切换「曜光环 (Ring Ripple)」：向全卡柔光扩散波纹
            if switch_t < 1. && r.h > 64. {
                let eased = 1. - (1. - switch_t).powi(3);
                let origin = (r.x + (r.w * 0.20).min(78.), r.y + (r.h * 0.34).min(74.));
                let ring_r = 34. + eased * (r.w * 0.66).max(160.);
                let ring = Circle::new(origin, ring_r);
                for (w_px, alpha, rgb) in [
                    (22.0, 0.055, ripple_tint),
                    (11.0, 0.11, ripple_tint),
                    (4.0, 0.18, 0xFFFFFF),
                    (1.3, 0.36, 0xFFFFFF),
                ] {
                    self.canvas.stroke(
                        &Stroke::new(w_px),
                        Affine::IDENTITY,
                        argb(alpha * switch_env * ripple_eff, rgb),
                        None,
                        &ring,
                    );
                }
            } else if morph_env > 0.02 {
                let breath = Gradient::new_radial(
                    (r.x + r.w * 0.5, r.y + r.h * 0.5),
                    (r.w * 0.55).max(80.) as f32,
                )
                .with_stops([
                    (0.0, argb(0.10 * morph_env * ripple_eff, ripple_tint)),
                    (1.0, argb(0.0, ripple_tint)),
                ]);
                self.canvas.fill(Fill::NonZero, Affine::IDENTITY, &breath, None, &box_shape);
            }
        }
        if p.frost > 0.001 {
            let f_top = (p.frost * 0.075).clamp(0., 0.09);
            let f_mid = f_top * 0.32;
            let frost = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.00, argb(f_top, 0xFFFFFF)),
                (0.38, argb(f_mid, 0xFFFFFF)),
                (1.00, color(0x00ffffff)),
            ]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &frost,
                None,
                &RoundedRect::from_rect(vr(r), shape_radius),
            );
        }
        let edge = Rect::new(r.x + 0.5, r.y + 0.5, r.w - 1., r.h - 1.);
        if curve.is_some() || clipped_rect {
            self.canvas.pop_layer();
        }
        if p.liquid_glass > 0.99 {
            return;
        }
        if p.light_field > 0.001 {
            let lf = p.light_field;
            let boost = (1.0 + 0.32 * pulse) * lf;
            let rim_col = |base_a: f64, rgb: u32| -> Color {
                color((((base_a * boost).clamp(0., 0.85) * 255.).round() as u32) << 24 | (rgb & 0xffffff))
            };
            // 纯净克制的曲面透镜双棱折射边：顶缘微亮冰白、中腰隐入暗透、底缘凝聚清冷折射边
            let rim_grad = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.00, rim_col(0.22, 0xFFFFFF)),
                (0.35, rim_col(0.06, 0xFFFFFF)),
                (0.75, rim_col(0.18, 0xCFE3FF)),
                (1.00, rim_col(0.46, 0xDCEEFF)),
            ]);
            let inner_rim = color((((0.06 * lf).clamp(0., 1.) * 255.).round() as u32) << 24 | 0xFFFFFF);
            if let Some(path) = curve {
                self.canvas.stroke(&Stroke::new(1.05), Affine::IDENTITY, &rim_grad, None, path);
                self.canvas.stroke(&Stroke::new(0.6), Affine::IDENTITY, inner_rim, None, path);
            } else {
                let outer_rr = RoundedRect::from_rect(vr(edge), (radius - 0.5).max(0.));
                let inner_rr = RoundedRect::from_rect(
                    vr(Rect::new(r.x + 1.1, r.y + 1.1, (r.w - 2.2).max(1.), (r.h - 2.2).max(1.))),
                    (radius - 1.1).max(0.),
                );
                self.canvas.stroke(&Stroke::new(1.05), Affine::IDENTITY, &rim_grad, None, &outer_rr);
                self.canvas.stroke(&Stroke::new(0.6), Affine::IDENTITY, inner_rim, None, &inner_rr);
            }
            return;
        }
        if let Some(path) = curve {
            let top_rim = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.00, color(((46. + 22. * p.reflection) as u32) << 24 | 0xFFFFFF)),
                (0.35, color(((16. + 10. * p.reflection) as u32) << 24 | 0xFFFFFF)),
                (1.00, color(((22. + 18. * p.reflection) as u32) << 24 | 0xDCE8FF)),
            ]);
            self.canvas.stroke(&Stroke::new(0.95), Affine::IDENTITY, &top_rim, None, path);
            return;
        }
        // 精细双层物理内倒角高光棱边（提升亚克力与弹窗玻璃质感）
        let acrylic_rim = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
            (0.00, color(((44. + 26. * p.reflection) as u32) << 24 | 0xFFFFFF)),
            (0.45, color(((14. + 12. * p.reflection) as u32) << 24 | 0xFFFFFF)),
            (1.00, color(((22. + 22. * p.reflection) as u32) << 24 | 0xDCE8FF)),
        ]);
        self.canvas.stroke(
            &Stroke::new(0.95),
            Affine::IDENTITY,
            &acrylic_rim,
            None,
            &RoundedRect::from_rect(vr(edge), (radius - 0.5).max(0.)),
        );
        self.outline(
            Rect::new(r.x + 1.3, r.y + 1.3, (r.w - 2.6).max(1.), (r.h - 2.6).max(1.)),
            (radius - 1.3).max(0.),
            0x14000000,
            0.7,
        );
    }
    fn music_lyric(&mut self, text: &str, r: Rect, size: f64, align: f64, ink: u32, s: &Scene<'_>) {
        if text.trim().is_empty() || r.w <= 0. || r.h <= 0. {
            return;
        }
        // Use the registered Medium face for the card and SemiBold for large immersive lyrics.
        let weight = if size >= 20. { 600 } else { 500 };
        let width = self
            .text
            .measure_weighted(text, size as f32, weight)
            .min(r.w);
        let x = r.x + (r.w - width) * align;
        let raw_phase = if s.animate {
            (s.lyric_age_ms / 240.).clamp(0., 1.)
        } else {
            1.
        };
        let phase = 1. - (1. - raw_phase).powi(4);
        let y = r.y + 5. * (1. - phase);
        let glyph_box = vr(Rect::new(x, y, width, r.h));
        let boundary = x + width * s.highlight.clamp(0., 1.);
        if s.lyric_glow && s.has_track && width > 0. && phase > 0.001 {
            self.lyric_halo.draw(
                &mut self.canvas, &mut self.text, text, glyph_box,
                size as f32, weight, phase as f32,
            );
        }
        let is_lf = matches!(s.material, Material::LightField);
        // 未演唱部分只在逐字涂色确实进行中时才压暗；未开启涂色或本行尚未开始涂色时保持正常墨色，避免整行发灰。
        let unplayed_ink = if is_lf && s.has_track && s.highlight > 0.01 {
            0x78000000 | (ink & 0xffffff)
        } else {
            ink
        };
        if is_lf && s.has_track && s.highlight > 0.01 && s.highlight < 0.99 && width > 4. {
            let cy = y + r.h * 0.52;
            let glow_rgb = s.progress_color & 0xffffff;
            let tint = crate::cover_color::mix(0xC8EEFF, glow_rgb, if glow_rgb != 0xFFFFFF && glow_rgb != 0xDDF6FF { 0.72 } else { 0. });
            let glow = Gradient::new_radial((boundary, cy), 14.0).with_stops([
                (0.0, color(0x34FFFFFF)),
                (0.55, color(0x18000000 | tint)),
                (1.0, color(0x00000000 | tint)),
            ]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &glow,
                None,
                &Circle::new((boundary, cy), 14.0),
            );
        }
        for (clip, c) in [
            (
                Rect::new(r.x, r.y, (boundary - r.x).max(0.), r.h + 5.),
                s.progress_color,
            ),
            (
                Rect::new(boundary, r.y, (r.x + r.w - boundary).max(0.), r.h + 5.),
                unplayed_ink,
            ),
        ] {
            if clip.w > 0. {
                self.clip(clip, 0., phase as f32);
                self.text.line_fixed(
                    &mut self.canvas,
                    text,
                    glyph_box,
                    size as f32,
                    c,
                    weight,
                    0.,
                );
                self.canvas.pop_layer();
            }
        }
    }
    fn compact_music(&mut self, s: &Scene<'_>) {
        let tones = self.m3(s);
        let c = s.layout.capsule;
        let cover = s.layout.compact_cover(s.layout_scale);
        self.cover(cover, 6., s.cover_opacity as f32);
        let is_lf = matches!(s.classic_capsule, Material::LightField);
        if is_lf {
            self.outline(cover, 6., 0x38FFFFFF, 0.85);
        }
        let double = !s.compact_secondary.is_empty();
        let (h, line, second) = self.compact_metrics(s.font_size, double, s.layout_scale);
        let y = c.y + (c.h - h) / 2.;
        let right_reserve = 70.;
        let viewport = Rect::new(c.x + 40., y, (c.w - right_reserve).max(20.), line);
        let raw_phase = if s.animate {
            (s.lyric_age_ms / 240.).clamp(0., 1.)
        } else {
            1.
        };
        let phase = 1. - (1. - raw_phase).powi(4);
        let lg_light = matches!(s.classic_capsule, Material::LiquidGlass) && self.is_liquid_glass_light();
        let primary_ink = if lg_light {
            self.light_glass_ink(s.text_color)
        } else {
            0xff000000 | tones.on_surface
        };
        let secondary_ink = if lg_light {
            0xCB000000 | (primary_ink & 0xffffff)
        } else {
            0xff000000 | tones.on_surface_variant
        };
        if phase < 1. && !s.previous_lyric.is_empty() {
            self.clip(
                Rect::new(viewport.x, c.y, viewport.w, c.h),
                0.,
                (1. - phase) as f32,
            );
            self.text_at(
                s.previous_lyric,
                Rect::new(viewport.x, y - 5.5 * phase, viewport.w, line),
                s.font_size,
                500,
                primary_ink,
                if double { 0.5 } else { 0. },
            );
            self.canvas.pop_layer();
        }
        self.music_lyric(
            s.lyric,
            viewport,
            s.font_size,
            if double { 0.5 } else { 0. },
            primary_ink,
            s,
        );
        if double {
            self.text_at(
                s.compact_secondary,
                Rect::new(viewport.x, y + line + 1., viewport.w, second),
                (s.font_size * 0.78).clamp(9., 11.5),
                400,
                secondary_ink,
                0.5,
            );
        }
        if is_lf {
            // 曜光视效胶囊态 4 柱律动声波（开启专辑动态取色时柔和承接专辑亮色，否则保持冰白）
            let scales = [
                s.activity_scales[0],
                s.activity_scales[0] * 0.4 + s.activity_scales[1] * 0.6,
                s.activity_scales[1],
                s.activity_scales[2],
            ];
            let heights = [9., 14., 11., 7.];
            let bar_tint = s.progress_color & 0xffffff;
            let custom_accent = bar_tint != 0xFFFFFF && bar_tint != 0xDDF6FF;
            let palette = if custom_accent {
                [
                    crate::cover_color::mix(bar_tint, 0xFFFFFF, 0.55),
                    crate::cover_color::mix(bar_tint, 0xFFFFFF, 0.35),
                    crate::cover_color::mix(bar_tint, 0xFFFFFF, 0.15),
                    bar_tint,
                ]
            } else {
                [0xFFFFFFu32, 0xF0F9FF, 0xDFF3FF, 0xCBEBFF]
            };
            let a = ((255. * s.activity_opacity) as u32) << 24;
            for i in 0..4 {
                let bh = heights[i] * scales[i];
                self.fill(
                    Rect::new(c.x + c.w - 25. + i as f64 * 4., c.y + c.h / 2. - bh / 2., 2.1, bh),
                    1.05,
                    a | palette[i],
                );
            }
        } else {
            for (i, h) in [10., 14., 8.].iter().enumerate() {
                let scale = s.activity_scales[i];
                self.fill(
                    Rect::new(
                        c.x + c.w - 22. + i as f64 * 4.,
                        c.y + c.h / 2. - h * scale / 2.,
                        2.,
                        h * scale,
                    ),
                    1.,
                    ((255. * s.activity_opacity) as u32) << 24 | (s.progress_color & 0xffffff),
                );
            }
        }
    }
    fn resident_compact(&mut self, s: &Scene<'_>) {
        let c = s.layout.capsule;
        let v = s.resident;
        let tones = self.m3(s);
        let material = if s.mini { s.mini_capsule } else { s.classic_capsule };
        let ink = if material == Material::LiquidGlass && self.is_liquid_glass_light() {
            self.light_glass_ink(s.text_color)
        } else if s.mini && s.mini_light_field {
            s.mini_ink
        } else {
            0xff000000 | tones.on_surface
        };
        let font = if s.mini { crate::compact::mini_font_size(s.font_size) } else { 14. };
        let gutter = if s.mini { 10. } else { 16. };
        if v.page == crate::resident::Page::Time && !s.mini {
            let size = 18.;
            self.icon(Icon::Island, Rect::new(c.x + gutter, c.y + (c.h - size) / 2., size, size), 0xff000000 | tones.primary);
        }
        if matches!(v.page, crate::resident::Page::Device(_)) {
            let right = if s.mini { 36. } else { 52. };
            let name = self.elided_line(&v.label, c.w - gutter * 2. - right, font as f32, 500);
            self.text_at(&name, Rect::new(c.x + gutter, c.y, c.w - gutter * 2. - right, c.h), font, 500, ink, 0.);
            self.text_at(&v.battery, Rect::new(c.x + c.w - gutter - right, c.y, right, c.h), font, 600, ink, 1.);
        } else {
            self.text_at(&v.label, Rect::new(c.x + gutter, c.y, c.w - 2. * gutter, c.h), font, 600, ink, 0.5);
        }
    }
    /// 任务栏模式底面：支持完全原生融入 Windows 任务栏的「透明融入」及「轻透胶囊」两种风格
    fn taskbar_widget_surface(&mut self, r: Rect, radius: f64, s: &Scene<'_>) {
        let hovered = s.taskbar_hovered;
        let pressed = s.pressed > 0;
        if s.taskbar_transparent {
            // 透明融入：常态完全零底框融于系统任务栏，仅悬停/按下时浮现 Fluent 微玻璃触控高光
            if hovered || pressed {
                if s.taskbar_light {
                    self.fill(r, radius, if pressed { 0x82ffffff } else { 0x62ffffff });
                    self.outline(r, radius, 0x1c0f172a, 0.75);
                } else {
                    self.fill(r, radius, if pressed { 0x26ffffff } else { 0x18ffffff });
                    self.outline(r, radius, 0x28ffffff, 0.75);
                }
            }
            return;
        }
        let accent = s.component_color & 0xffffff;
        if s.taskbar_light {
            let top_a: u32 = if pressed { 0xf0 } else if hovered { 0xe6 } else { 0xd8 };
            let bot_a: u32 = if pressed { 0xf6 } else if hovered { 0xec } else { 0xe0 };
            let top_rgb = crate::cover_color::mix(0xffffff, accent, 0.05);
            let bot_rgb = crate::cover_color::mix(0xf1f5f9, accent, 0.08);
            let grad = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.0, color((top_a << 24) | top_rgb)),
                (1.0, color((bot_a << 24) | bot_rgb)),
            ]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &grad,
                None,
                &RoundedRect::from_rect(vr(r), radius),
            );
            self.outline(r, radius, if hovered { 0x280f172a } else { 0x180f172a }, 0.75);
        } else {
            let top_a: u32 = if pressed { 0xe6 } else if hovered { 0xdc } else { 0xc8 };
            let bot_a: u32 = if pressed { 0xee } else if hovered { 0xe4 } else { 0xd4 };
            let top_rgb = crate::cover_color::mix(if hovered { 0x1f2633 } else { 0x151a24 }, accent, 0.10);
            let bot_rgb = crate::cover_color::mix(if hovered { 0x171c26 } else { 0x0e121a }, accent, 0.14);
            let grad = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.0, color((top_a << 24) | top_rgb)),
                (1.0, color((bot_a << 24) | bot_rgb)),
            ]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &grad,
                None,
                &RoundedRect::from_rect(vr(r), radius),
            );
            self.outline(r, radius, if hovered { 0x32ffffff } else { 0x1fffffff }, 0.75);
        }
    }
    /// 任务栏模式高颜值微条内容：播放时直显实时逐字歌词与副行译文/歌曲名，未播时可常驻显示时间与系统状态
    fn taskbar_widget_view(&mut self, s: &Scene<'_>) {
        let c = s.layout.capsule;
        let light_ui = s.taskbar_light;
        let ink: u32 = if light_ui { 0xff0f172a } else { 0xfff8fbff };
        let muted: u32 = if light_ui { 0xb8334155 } else { 0xc4dce8f8 };
        let soft_accent = s.progress_color & 0xffffff;
        let has_custom_tint = soft_accent != 0xffffff && soft_accent != 0xddf6ff;
        let accent_rgb = if light_ui {
            if has_custom_tint {
                crate::cover_color::mix(soft_accent, 0x0b57d0, 0.45)
            } else {
                0x0b6bcb
            }
        } else if has_custom_tint {
            crate::cover_color::mix(soft_accent, 0xffffff, 0.30)
        } else {
            0x8ad8ff
        };
        let accent_col = 0xff000000 | accent_rgb;
        let icon_badge_bg = if light_ui { 0x120f172a } else { 0x16ffffff };
        let icon_badge_rim = if light_ui { 0x1c0f172a } else { 0x22ffffff };

        // 1. 活动/常驻状态面板（番茄钟/待办/更新/常驻时钟状态）
        if let Some(p) = s.panel {
            if p.kind == crate::resident::KIND || !s.has_track && p.kind != 1 && p.kind != 2 && p.kind != 3 {
                let badge_sz = (c.h - 12.).clamp(24., 26.);
                let badge_r = Rect::new(c.x + 6., c.y + (c.h - badge_sz) / 2., badge_sz, badge_sz);
                self.fill(badge_r, 6., icon_badge_bg);
                self.outline(badge_r, 6., icon_badge_rim, 0.75);
                self.icon(
                    Icon::Island,
                    Rect::new(badge_r.x + (badge_sz - 16.) / 2., badge_r.y + (badge_sz - 16.) / 2., 16., 16.),
                    accent_col,
                );
                let text_x = badge_r.x + badge_r.w + 8.;
                let tw = (c.x + c.w - text_x - 22.).max(48.);
                let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
                let week = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][(st.wDayOfWeek as usize).min(6)];
                let live_clock = format!("{:02}:{:02} · {}月{}日 {}", st.wHour, st.wMinute, st.wMonth, st.wDay, week);
                let top_raw = if !s.resident.clock.is_empty() && s.resident.page == crate::resident::Page::Time {
                    format!("{} · {}月{}日 {}", s.resident.clock, st.wMonth, st.wDay, week)
                } else if matches!(s.resident.page, crate::resident::Page::Device(_)) && !s.resident.label.is_empty() {
                    s.resident.label.clone()
                } else {
                    live_clock
                };
                let sub_raw = if matches!(s.resident.page, crate::resident::Page::Device(_)) && !s.resident.battery.is_empty() {
                    format!("电量 {}", s.resident.battery)
                } else if !s.resident.cpu.is_empty() && !s.resident.memory.is_empty() {
                    format!("CPU {} · 内存 {}", s.resident.cpu, s.resident.memory)
                } else {
                    "点屿常驻 · 点击展开面板".into()
                };
                let top_line = self.elided_line(&top_raw, tw, 12.0, 600);
                let bot_line = self.elided_line(&sub_raw, tw, 10.0, 400);
                self.text_at(&top_line, Rect::new(text_x, c.y + 4.0, tw, 16.5), 12.0, 600, ink, 0.);
                self.text_at(&bot_line, Rect::new(text_x, c.y + 20.5, tw, 13.5), 10.0, 400, muted, 0.);

                // 右侧呼吸在线小圆点
                let pulse = if s.animate { (0.65 + 0.35 * (s.time_ms * 0.0025).sin()).clamp(0.3, 1.0) } else { 1.0 };
                let pa = ((pulse * 220.) as u32) << 24;
                self.fill(
                    Rect::new(c.x + c.w - 13., c.y + (c.h - 5.) / 2., 5., 5.),
                    2.5,
                    pa | accent_rgb,
                );
                return;
            }
            let (glyph, tint) = if p.kind == 1 {
                (Icon::Focus, 0xffff9e8e)
            } else if p.kind == 2 {
                (Icon::Check, 0xff7bdcb5)
            } else {
                (Icon::Import, accent_col)
            };
            let badge_sz = (c.h - 12.).clamp(24., 26.);
            let badge_r = Rect::new(c.x + 6., c.y + (c.h - badge_sz) / 2., badge_sz, badge_sz);
            self.fill(badge_r, 6., icon_badge_bg);
            self.outline(badge_r, 6., icon_badge_rim, 0.75);
            self.icon(
                glyph,
                Rect::new(badge_r.x + (badge_sz - 16.) / 2., badge_r.y + (badge_sz - 16.) / 2., 16., 16.),
                tint,
            );
            let tw = (c.w - 46.).max(40.);
            let top_txt = self.elided_line(&p.value, tw, 12.0, 600);
            let sub_txt = self.elided_line(&p.title, tw, 10.0, 400);
            if p.kind == 1 && s.timer_mono {
                self.text_mono(
                    &p.value,
                    Rect::new(badge_r.x + badge_r.w + 8., c.y + 4.0, tw, 16.5),
                    12.0,
                    600,
                    ink,
                    0.,
                );
            } else {
                self.text_at(&top_txt, Rect::new(badge_r.x + badge_r.w + 8., c.y + 4.0, tw, 16.5), 12.0, 600, ink, 0.);
            }
            self.text_at(&sub_txt, Rect::new(badge_r.x + badge_r.w + 8., c.y + 20.5, tw, 13.5), 10.0, 400, muted, 0.);
            return;
        }

        // 2. 无曲目未开常驻时的兜底展示（保证在任意瞬间都不为空白）
        if !s.has_track {
            let badge_sz = (c.h - 12.).clamp(24., 26.);
            let badge_r = Rect::new(c.x + 6., c.y + (c.h - badge_sz) / 2., badge_sz, badge_sz);
            self.fill(badge_r, 6., icon_badge_bg);
            self.icon(
                Icon::Music,
                Rect::new(badge_r.x + (badge_sz - 15.) / 2., badge_r.y + (badge_sz - 15.) / 2., 15., 15.),
                accent_col,
            );
            let text_x = badge_r.x + badge_r.w + 8.;
            let tw = (c.x + c.w - text_x - 10.).max(48.);
            self.text_at("等待音乐播放", Rect::new(text_x, c.y + 4.0, tw, 16.5), 12.0, 600, ink, 0.);
            self.text_at("点击展开控制面板", Rect::new(text_x, c.y + 20.5, tw, 13.5), 10.0, 400, muted, 0.);
            return;
        }

        // 3. 播放态高颜值「任务栏逐字歌词条」：左侧圆角小封面 + 中间宽幅逐字扫光主歌词与译文/曲目 + 右侧声波（悬停切控制钮）
        let cover_sz = (c.h - 10.).clamp(24., 28.);
        let cover_rect = Rect::new(c.x + 6., c.y + (c.h - cover_sz) / 2., cover_sz, cover_sz);
        if self.cover.is_some() && s.cover_opacity > 0.01 {
            self.cover(cover_rect, 5.5, s.cover_opacity as f32);
        } else {
            self.fill(cover_rect, 5.5, icon_badge_bg);
            self.icon(
                Icon::Music,
                Rect::new(cover_rect.x + 6., cover_rect.y + 6., cover_sz - 12., cover_sz - 12.),
                accent_col,
            );
        }
        self.outline(cover_rect, 5.5, if light_ui { 0x1e0f172a } else { 0x26ffffff }, 0.75);

        let show_controls = s.taskbar_hovered;
        let btns = crate::layout::taskbar_widget_buttons(c);
        let right_bound = if show_controls {
            for (id, r) in btns {
                let hovered = s.hover == id;
                let pressed = s.pressed == id;
                if hovered || pressed {
                    let bg = if light_ui {
                        if pressed { 0x220f172a } else { 0x120f172a }
                    } else if pressed {
                        0x30ffffff
                    } else {
                        0x1effffff
                    };
                    self.fill(r, 5., bg);
                }
                let glyph = match id {
                    3 => dotisle_render::transport_icons::Glyph::Prev,
                    5 => dotisle_render::transport_icons::Glyph::Next,
                    _ if s.playing => dotisle_render::transport_icons::Glyph::Pause,
                    _ => dotisle_render::transport_icons::Glyph::Play,
                };
                let g_sz = if id == 4 { 14. } else { 13. };
                let g_rect = Rect::new(r.x + (r.w - g_sz) / 2., r.y + (r.h - g_sz) / 2., g_sz, g_sz);
                let icon_col = if hovered { ink } else { muted };
                dotisle_render::transport_icons::paint_icon(&mut self.canvas, glyph, s.transport_theme, vr(g_rect), icon_col);
            }
            btns[0].1.x - 6.
        } else {
            // 常态右缘绘 4 柱纤细律动声波，把全部宽度留给歌词
            let scales = [
                s.activity_scales[0],
                s.activity_scales[0] * 0.4 + s.activity_scales[1] * 0.6,
                s.activity_scales[1],
                s.activity_scales[2],
            ];
            let heights = [8., 13., 10., 6.5];
            let wave_a = ((235. * s.activity_opacity.max(if s.playing { 0.85 } else { 0.35 })) as u32) << 24;
            for i in 0..4 {
                let bh = (heights[i] * if s.playing { scales[i] } else { 0.45 }).max(3.0);
                self.fill(
                    Rect::new(c.x + c.w - 23. + i as f64 * 4.0, c.y + c.h / 2. - bh / 2., 2.0, bh),
                    1.0,
                    wave_a | accent_rgb,
                );
            }
            c.x + c.w - 29.
        };

        // 中间宽幅逐字歌词舞台
        let text_x = cover_rect.x + cover_rect.w + 9.;
        let text_w = (right_bound - text_x).max(48.);
        let has_real_lyric = !s.lyric.trim().is_empty() && s.lyric != "打开播放器，开始一首音乐";
        let main_raw = if has_real_lyric {
            s.lyric
        } else if !s.title.is_empty() {
            s.title
        } else {
            "点屿音乐"
        };
        let sub_owned: String;
        let sub_raw: &str = if has_real_lyric && !s.compact_secondary.trim().is_empty() && s.compact_secondary.trim() != main_raw.trim() {
            s.compact_secondary
        } else if has_real_lyric && !s.title.is_empty() {
            if !s.artist.is_empty() {
                sub_owned = format!("{} · {}", s.title, s.artist);
                &sub_owned
            } else {
                s.title
            }
        } else if !s.artist.is_empty() {
            s.artist
        } else {
            "点击展开大卡片"
        };

        let main_elided = self.elided_line(main_raw, text_w, 12.5, 600);
        let sub_elided = self.elided_line(sub_raw, text_w, 10.0, 400);
        let raw_phase = if s.animate {
            (s.lyric_age_ms / 240.).clamp(0., 1.)
        } else {
            1.
        };
        let phase = 1. - (1. - raw_phase).powi(4);
        let main_y = c.y + 3.5 + 3.0 * (1. - phase);
        let main_box = Rect::new(text_x, main_y, text_w, 16.5);

        if has_real_lyric && s.highlight > 0.005 {
            let line_w = self.text.measure_weighted(&main_elided, 12.5, 600).min(text_w);
            let boundary = text_x + line_w * s.highlight.clamp(0., 1.);
            let unplayed = if light_ui { 0x90334155 } else { 0x86dce8f8 };
            for (clip, col) in [
                (Rect::new(text_x, c.y, (boundary - text_x).max(0.), c.h), accent_col),
                (Rect::new(boundary, c.y, (text_x + text_w - boundary).max(0.), c.h), unplayed),
            ] {
                if clip.w > 0. {
                    self.clip(clip, 0., phase as f32);
                    self.text.line_fixed(&mut self.canvas, &main_elided, vr(main_box), 12.5, col, 600, 0.);
                    self.canvas.pop_layer();
                }
            }
        } else {
            self.text_at(&main_elided, main_box, 12.5, 600, ink, 0.);
        }
        self.text_at(&sub_elided, Rect::new(text_x, c.y + 20.5, text_w, 13.5), 10.0, 400, muted, 0.);

        // 沿着文本下方的 1.5px 极细进度微光线
        let rail = Rect::new(text_x, c.y + c.h - 2.6, (c.x + c.w - 8. - text_x).max(20.), 1.4);
        self.fill(rail, 0.7, if light_ui { 0x160f172a } else { 0x18ffffff });
        let pw = (rail.w * s.progress.clamp(0., 1.)).max(2.);
        let prog = Rect::new(rail.x, rail.y, pw, rail.h);
        let pgrad = Gradient::new_linear((prog.x, prog.y), (prog.x + prog.w, prog.y)).with_stops([
            (0.0, color(0x88000000 | accent_rgb)),
            (1.0, color(accent_col)),
        ]);
        self.canvas.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &pgrad,
            None,
            &RoundedRect::from_rect(vr(prog), 0.7),
        );
    }
    fn quota_arc(&mut self, x: f64, y: f64, radius: f64, width: f64, value: Option<f64>, tone: u32, track: u32) {
        self.ring(x,y,radius,track,width);
        let Some(percent)=value.filter(|p|*p>0.) else{return;};
        if percent>=100. {self.ring(x,y,radius,tone,width);return;}
        let arc=vello::kurbo::Arc::new((x,y),(radius,radius),-std::f64::consts::FRAC_PI_2,std::f64::consts::TAU*percent/100.,0.);
        self.canvas.stroke(&Stroke::new(width).with_caps(Cap::Round),Affine::IDENTITY,color(tone),None,&arc);
    }
    fn quota_tile(&mut self, tile: &crate::agent_island::Tile, x: f64, y: f64, r: f64, ink: u32, hour: u32, week: u32, track: u32) {
        use crate::agent_island::Agent;
        if tile.agent == Agent::Harness {
            self.ring(x,y,r,track,(r*0.12).clamp(1.6,3.));
        } else if let Some(token) = tile.fallback_tokens() {
            self.quota_arc(x,y,r,(r*0.14).clamp(1.8,4.),Some(100. * token.remaining as f64 / token.total as f64),hour,track);
        } else {
            self.quota_arc(x,y,r,(r*0.14).clamp(1.8,4.),tile.hour,hour,track);
            self.quota_arc(x,y,r*0.71,(r*0.12).clamp(1.6,3.),tile.week,week,track);
        }
        let k=r*0.35;
        match tile.agent {
            Agent::Codex => {
                let rect=Rect::new(x-k,y-k*0.72,k*2.,k*1.44);
                self.outline(rect,k*0.36,ink, (r*0.075).max(1.));
                self.line((x-k*0.51,y-k*0.25),(x-k*0.16,y),ink,1.);
                self.line((x-k*0.16,y),(x-k*0.51,y+k*0.25),ink,1.);
                self.line((x+k*0.08,y+k*0.25),(x+k*0.51,y+k*0.25),ink,1.);
            }
            Agent::ZCode => {
                let x0=x-k*0.72;let x1=x+k*0.72;let y0=y-k*0.62;let y1=y+k*0.62;
                let stroke=(r*0.092).max(1.);
                self.line((x0,y0),(x1,y0),ink,stroke);
                self.line((x1,y0),(x0,y1),ink,stroke);
                self.line((x0,y1),(x1,y1),ink,stroke);
            }
            Agent::Harness => {
                let w = (r * 0.105).max(1.);
                self.line((x-k,y+k*0.15),(x-k*0.62,y-k*0.21),ink,w);
                self.line((x-k*0.62,y-k*0.21),(x+k*0.45,y-k*0.25),ink,w);
                self.line((x+k*0.45,y-k*0.25),(x+k,y-k*0.72),ink,w);
                self.line((x+k*0.45,y-k*0.25),(x+k*0.9,y+k*0.54),ink,w);
                self.line((x+k*0.45,y-k*0.25),(x+k*0.18,y+k*0.6),ink,w);
                self.line((x+k*0.18,y+k*0.6),(x-k*0.65,y+k*0.55),ink,w);
            }
            Agent::Cursor => {
                let w = (r * 0.102).max(1.);
                self.line((x-k*0.7,y-k*0.82),(x+k*0.84,y),ink,w);
                self.line((x+k*0.84,y),(x-k*0.65,y+k*0.82),ink,w);
                self.line((x-k*0.65,y+k*0.82),(x-k*0.7,y-k*0.82),ink,w);
            }
            Agent::Claude => {
                for i in 0..10 {
                    let a=i as f64*std::f64::consts::TAU/10.;
                    let (dy,dx)=a.sin_cos();
                    let l=k*(if i%3==0 {0.78}else{1.});
                    self.line((x+dx*k*0.18,y+dy*k*0.18),(x+dx*l,y+dy*l),ink,(r*0.075).max(1.));
                }
            }
        }
    }
    fn quota_label(v: Option<f64>) -> String { v.map_or_else(||"—".to_owned(),|p|format!("{p:.0}%")) }
    fn agent_compact(&mut self, s: &Scene<'_>) {
        let r=s.layout.capsule;
        let dark=!s.layout.anchor_bottom || !s.taskbar_light;
        let (ink,muted,hour,week,track)=Self::quota_colors(dark);
        if s.agent.tiles.is_empty() {
            self.text_at("Codex 等待回答",r,12.,600,ink,0.5);
            return;
        }
        if r.h<25. { self.text_at("AI 额度",r,9.,600,ink,0.5); return; }
        let n=s.agent.tiles.len() as f64; let lane=(r.w/n).min(165.); let left=r.x+(r.w-lane*n)/2.;
        for (i,tile) in s.agent.tiles.iter().enumerate(){
            let x=left+i as f64*lane;
            if lane<100. {self.quota_tile(tile,x+lane/2.,r.y+r.h/2.,13.,ink,hour,week,track);continue;}
            self.quota_tile(tile,x+23.,r.y+r.h/2.,13.,ink,hour,week,track);
            let text_w=(lane-39.).max(1.);
            let harness = tile.agent == crate::agent_island::Agent::Harness;
            let title=if harness { "Harness" } else if s.agent.prompt.is_some()&&tile.agent==crate::agent_island::Agent::Codex {"Codex · 待答"}else{tile.agent.name()};
            self.text_line(title,Rect::new(x+44.,r.y+r.h/2.-14.,text_w,15.),11.,600,ink);
            let value = if harness {
                tile.wallet.first().cloned().unwrap_or_else(|| "余额 —".into())
            } else if let Some(token) = tile.fallback_tokens() {
                token.compact_label()
            } else if tile.agent == crate::agent_island::Agent::ZCode && tile.hour.is_none() && tile.week.is_none() {
                "—".into()
            } else { format!("5h {} · 周 {}",Self::quota_label(tile.hour),Self::quota_label(tile.week)) };
            self.text_line(&value,Rect::new(x+44.,r.y+r.h/2.+1.,text_w,14.),9.,500,muted);
        }
    }
    fn quota_colors(dark: bool) -> (u32,u32,u32,u32,u32) {
        if dark {(0xfff5f6fb,0xffb4bed1,0xff78b8ff,0xff9e9df4,0x477e8ba5)}
        else {(0xff1d2835,0xff4a5b71,0xff176ac5,0xff6454c2,0x38819ab2)}
    }
    fn agent_card(&mut self, s: &Scene<'_>) {
        use crate::agent_island as a;
        let r=Rect::new(0.,0.,s.layout.card.w,s.layout.card.h);
        let dark=s.expressive_dark;
        let (ink,muted,hour,week,track)=Self::quota_colors(dark);
        let background=if dark{0x36687486}else{0x227b90a7};
        let selection=if dark{0x625085c2}else{0x3f6aa7e4};
        let v=s.agent;
        let close=a::close(r);
        self.fill(close,9.,if s.hover==a::COLLAPSE{selection}else{background});
        let (cx,cy)=(close.x+close.w/2.,close.y+close.h/2.);
        self.line((cx-4.,cy-2.),(cx,cy+2.),ink,1.3);
        self.line((cx,cy+2.),(cx+4.,cy-2.),ink,1.3);
        if let Some(q)=v.question(){
            let count=v.prompt.as_ref().map_or(1,|p|p.questions.len());
            let head=format!("Codex 等待回答  ·  {}/{}",v.page+1,count);
            self.text_line(&head,Rect::new(16.,12.,r.w-70.,26.),15.,650,ink);
            // Fit the actual request, not a generic confirmation label.
            let mut lines=Vec::new();
            for paragraph in q.text.split('\n') {
                let mut line=String::new();
                for c in paragraph.chars() {
                    line.push(c);
                    if self.text.measure_weighted(&line,13.,550)>r.w-34. && line.chars().count()>1 {
                        line.pop(); lines.push(line); line=String::from(c);
                    }
                }
                lines.push(line);
            }
            for (i,line) in lines.iter().take(3).enumerate() {
                self.text_line(line,Rect::new(16.,42.+i as f64*17.,r.w-32.,17.),13.,550,ink);
            }
            for (i,label) in q.labels.iter().enumerate(){
                let bounds=a::option(r,i);
                let selected=!v.editing&&v.answer().contains(label);
                self.fill(bounds,8.,if selected||s.hover==a::OPTION+i as i32 {selection}else{background});
                if selected{self.icon(Icon::Check,Rect::new(bounds.x+9.,bounds.y+6.,15.,15.),hour);}
                self.text_line(label,Rect::new(bounds.x+31.,bounds.y,bounds.w-43.,bounds.h),11.5,550,ink);
            }
            if q.custom {
                let bounds=a::custom(r);
                self.fill(bounds,9.,if v.editing{selection}else{background});
                if v.editing {self.outline(bounds,9.,hour,1.);}
                let line=if v.editing {
                    let value=String::from_utf16_lossy(&v.draft);
                    let tail: String=value.chars().rev().take(32).collect::<Vec<_>>().into_iter().rev().collect();
                    format!("{tail}│")
                }else{v.answer().iter().find(|val|!q.labels.contains(val)).cloned().unwrap_or_else(||"输入自定义回复…".into())};
                self.text_line(&line,Rect::new(bounds.x+12.,bounds.y,bounds.w-24.,bounds.h),12.,500,if v.editing{ink}else{muted});
            }
            for (label,bounds,enabled) in [("‹",a::prev(r),v.page>0),("›",a::next(r),v.page+1<count),("发送回复",a::send(r),v.can_send())] {
                let is_send=label=="发送回复";
                if !is_send&&count==1 {continue;}
                self.fill(bounds,9.,if is_send&&enabled{hour}else{background});
                self.text_at(label,bounds,if is_send{11.5}else{18.},650,if is_send&&enabled{if dark{0xff14283b}else{0xffffffff}}else if enabled{ink}else{muted},0.5);
            }
            if !v.feedback.is_empty(){
                let x=if count>1 {114.}else{16.};
                self.text_line(v.feedback,Rect::new(x,r.h-44.,(r.w-x-124.).max(1.),31.),10.5,500,muted);
            }
            return;
        }
        self.text_line("AI 额度",Rect::new(16.,12.,r.w-62.,27.),16.,650,ink);
        for (i,tile) in v.tiles.iter().enumerate(){
            let b = a::tile_rect(r, i, v.tiles.len());
            self.fill(b,14.,background);
            self.quota_tile(tile,b.x+32.,b.y+40.,22.,ink,hour,week,track);
            let w = (b.w - 69.).max(0.);
            let harness = tile.agent == a::Agent::Harness;
            let title = self.elided_line(tile.agent.name(), w, if harness { 12. } else {13.}, 650);
            self.text_line(&title,Rect::new(b.x+60.,b.y+9.,w,22.),if harness { 12. } else {13.},650,ink);
            if harness {
                let line1 = format!("余额  {}", tile.wallet.first().map(String::as_str).unwrap_or("—"));
                self.text_line(&line1,Rect::new(b.x+60.,b.y+33.,w,16.),11.,550,hour);
                if let Some(balance) = tile.wallet.get(1) {
                    self.text_line(balance,Rect::new(b.x+60.,b.y+52.,w,16.),11.,550,muted);
                }
            } else if let Some(token) = tile.fallback_tokens() {
                let exact = token.remaining_label();
                let line1 = self.elided_line(&exact, w, 11., 550);
                let line1 = if line1 == exact { line1 } else { token.compact_label() };
                self.text_line(&line1,Rect::new(b.x+60.,b.y+33.,w,16.),11.,550,hour);
                let line2 = format!("总量 {}", token.total);
                let line2 = self.elided_line(&line2,w,11.,550);
                self.text_line(&line2,Rect::new(b.x+60.,b.y+52.,w,16.),11.,550,muted);
            } else if tile.agent == a::Agent::ZCode && tile.hour.is_none() && tile.week.is_none() {
                self.text_line("—",Rect::new(b.x+60.,b.y+33.,w,16.),11.,550,muted);
            } else {
                self.text_line(&format!("5 小时  {}",Self::quota_label(tile.hour)),Rect::new(b.x+60.,b.y+33.,w,16.),11.,550,hour);
                self.text_line(&format!("本周    {}",Self::quota_label(tile.week)),Rect::new(b.x+60.,b.y+52.,w,16.),11.,550,week);
            }
        }
    }
    fn compact_view(&mut self, s: &Scene<'_>) {
        if s.panel.is_some_and(|p|p.kind==crate::agent_island::KIND) {
            self.agent_compact(s);
            return;
        }
        if s.layout.anchor_bottom {
            self.taskbar_widget_view(s);
            return;
        }
        let mut compact = *s;
        compact.expressive_dark = true;
        let s = &compact;
        if s.panel.is_some_and(|p| p.kind == crate::resident::KIND) {
            self.resident_compact(s);
            return;
        }
        if let Some(panel) = s.panel.filter(|p| p.kind == 1) {
            if let Some(timer) = panel.timer.as_ref().filter(|timer| !timer.indefinite) {
                let tones = self.m3(s);
                crate::liquid_timer::paint(
                    &mut self.canvas,
                    vr(s.layout.capsule),
                    s.capsule_radius,
                    panel.progress,
                    timer.elapsed_ms as f64,
                    0xff000000 | tones.primary,
                    s.animate,
                );
            }
        }
        if s.mini {
            let tones = self.m3(s);
            let c = s.layout.capsule;
            let accent = if s.panel.is_some() {
                0xff000000 | tones.primary
            } else {
                s.progress_color
            };
            let moving = s.panel.map(|p| p.running).unwrap_or(s.playing);
            let breath = if moving && s.animate {
                0.68 + 0.32 * (s.time_ms / 2800. * std::f64::consts::TAU).sin()
            } else {
                0.8
            };
            let dot = 3.2 + breath * 1.2;
            self.fill(
                Rect::new(c.x + 11. - dot / 2., c.y + (c.h - dot) / 2., dot, dot),
                dot / 2.,
                ((255. * breath) as u32) << 24 | (accent & 0xffffff),
            );
            let label = s
                .panel
                .map(|p| format!("{}  {}", p.title, p.value))
                .unwrap_or_else(|| s.lyric.to_string());
            let mini_font = crate::compact::mini_font_size(s.font_size);
            let mini_ink = if s.mini_capsule == Material::LiquidGlass && self.is_liquid_glass_light() {
                self.light_glass_ink(s.text_color)
            } else if s.mini_light_field {
                s.mini_ink
            } else {
                0xff000000 | tones.on_surface
            };
            if s.timer_mono && s.panel.is_some_and(|p| p.kind == 1) {
                self.text_mono(
                    &label,
                    Rect::new(c.x + 22., c.y, c.w - 44., c.h),
                    mini_font,
                    500,
                    mini_ink,
                    0.5,
                );
            } else {
                self.text_at(
                    &label,
                    Rect::new(c.x + 22., c.y, c.w - 44., c.h),
                    mini_font,
                    500,
                    mini_ink,
                    0.5,
                );
            }
            return;
        }
        if let Some(p) = s.panel {
            let tones = self.m3(s);
            let c = s.layout.capsule;
            let cy = c.y + c.h / 2.;
            let accent = 0xff000000 | tones.primary;
            if p.kind == 3 {
                self.update_ring(c.x + 22., cy, 10., p.progress, s.time_ms, s.animate, accent);
            } else {
                self.icon(
                    if p.kind == 1 {
                        Icon::Focus
                    } else if p.kind == 2 {
                        Icon::Check
                    } else {
                        Icon::Import
                    },
                    Rect::new(c.x + 12., cy - 10., 20., 20.),
                    accent,
                );
            }
            if p.kind == 3 {
                self.text_at(
                    &p.title,
                    Rect::new(c.x + 42., cy - 16., c.w - 56., 19.),
                    11.,
                    600,
                    0xff000000 | tones.on_surface,
                    0.,
                );
                self.text_at(
                    &p.subtitle,
                    Rect::new(c.x + 42., cy + 4., c.w - 56., 14.),
                    9.,
                    400,
                    accent,
                    0.,
                );
            } else {
                if p.kind == 1 && s.timer_mono {
                    self.text_mono(
                        &p.value,
                        Rect::new(c.x + 40., cy - 17., c.w - 70., 19.),
                        14.,
                        600,
                        0xff000000 | tones.on_surface,
                        0.5,
                    );
                } else {
                    self.text_at(
                        &p.value,
                        Rect::new(c.x + 40., cy - 17., c.w - 70., 19.),
                        14.,
                        600,
                        0xff000000 | tones.on_surface,
                        0.5,
                    );
                }
                self.text_at(
                    &p.title,
                    Rect::new(c.x + 40., cy + 3., c.w - 70., 13.),
                    9.,
                    400,
                    accent,
                    0.5,
                );
            }
            if p.kind != 3 {
                let a = if p.running && s.animate {
                    0.7 + 0.3 * (s.time_ms / 2400. * std::f64::consts::TAU).sin()
                } else {
                    0.65
                };
                self.fill(
                    Rect::new(c.x + c.w - 18., cy - 3., 6., 6.),
                    3.,
                    ((255. * a) as u32) << 24 | (accent & 0xffffff),
                );
            }
        } else {
            let tones = self.m3(s);
            let mut music = *s;
            music.progress_color = s.progress_color;
            if s.lyric_cover_color {
                let mut accent = s.progress_color;
                for _ in 0..12 {
                    if crate::cover_color::contrast(accent, tones.container) >= 3. {
                        break;
                    }
                    accent = 0xff000000 | crate::cover_color::mix(accent, tones.on_surface, 0.15);
                }
                music.progress_color = accent;
            }
            self.compact_music(&music);
        }
    }
    fn music_surface_samples(&self, s: &Scene<'_>) -> Vec<u32> {
        let p = Self::music_surface_spec(s).paint(self.paint_palette);
        let (shape, _) =
            s.layout
                .morph_shape_with_radii(s.frame.card_reveal, s.capsule_radius, s.radius);
        let scale = (shape.w / s.layout.card.w)
            .min(shape.h / s.layout.card.h)
            .max(0.0001);
        let tx = shape.x + (shape.w - s.layout.card.w * scale) / 2.;
        let ty =
            shape.y + (shape.h - s.layout.card.h * scale) / 2. + s.frame.content_offset_y * scale;
        let g = s.custom_geometry.unwrap_or_else(|| {
            crate::layout::music_geometry(
                s.layout.card.w,
                s.music_layout,
                s.music_mix,
                s.layout_scale,
                s.font_size,
            )
        });
        let image = if p.cover_alpha > 0.001 {
            self.blurred_cover
                .as_ref()
                .map(|(_, i)| i)
                .or(self.cover.as_ref())
        } else {
            None
        };
        let mut samples = Vec::new();
        for (id, r) in g.buttons {
            if ![3, 5].contains(&id) {
                continue;
            }
            for dx in [-7., 0., 7.] {
                for dy in [-7., 0., 7.] {
                    let x = tx + (r.x + r.w / 2. + dx) * scale;
                    let y = ty + (r.y + r.h / 2. + dy) * scale;
                    let t = ((y - shape.y) / shape.h.max(1.)).clamp(0., 1.);
                    let base = crate::cover_color::mix(p.top, p.bottom, t);
                    let alpha = (p.top >> 24) as f64 / 255.;
                    for under in [0x000000, 0xffffff] {
                        let mut bg = crate::cover_color::mix(under, base, alpha);
                        if let Some(image) = image {
                            let fit = (shape.w / image.width as f64)
                                .max(shape.h / image.height as f64)
                                .max(0.0001);
                            let ix = ((x
                                - (shape.x + shape.w / 2. - image.width as f64 * fit / 2.))
                                / fit)
                                .floor()
                                .clamp(0., image.width.saturating_sub(1) as f64)
                                as usize;
                            let iy = ((y
                                - (shape.y + shape.h / 2. - image.height as f64 * fit / 2.))
                                / fit)
                                .floor()
                                .clamp(0., image.height.saturating_sub(1) as f64)
                                as usize;
                            let at = (iy * image.width as usize + ix) * 4;
                            if let Some(pixel) = image.data.data().get(at..at + 4) {
                                let rgb = (pixel[0] as u32) << 16
                                    | (pixel[1] as u32) << 8
                                    | pixel[2] as u32;
                                bg = crate::cover_color::mix(
                                    bg,
                                    rgb,
                                    p.cover_alpha * pixel[3] as f64 / 255.,
                                );
                            }
                            let scrim_w = p.cover_alpha.powf(0.65) * (1. - 0.14 * p.light_field);
                            let scrim_a = ((0.14 + 0.24 * t + 0.10 * (1. - p.amount)) * scrim_w).clamp(0., 0.78);
                            let scrim = crate::cover_color::mix(
                                0x0B0F16,
                                self.paint_palette.background,
                                (0.34 * p.tint).clamp(0., 0.45),
                            );
                            bg = crate::cover_color::mix(bg, scrim, scrim_a);
                        }
                        bg = crate::cover_color::mix(bg, 0xffffff, p.frost * 15. / 255. * (1. - t));
                        samples.push(bg);
                        if alpha >= 1. {
                            break;
                        }
                    }
                }
            }
        }
        samples
    }
    fn music_control_ink(&mut self, s: &Scene<'_>) -> u32 {
        let preferred = s.component_color & 0xffffff;
        if matches!(s.material, Material::LiquidGlass) && self.is_liquid_glass_light() {
            return preferred;
        }
        let samples = self.music_surface_samples(s);
        if let Some((old, bg, ink)) = &self.transport_ink {
            if *old == preferred && *bg == samples {
                return *ink;
            }
        }
        let ink = dotisle_domain::music_controls::tinted_on_surfaces(&samples, preferred);
        self.transport_ink = Some((preferred, samples, ink));
        ink
    }
    pub fn transport_diagnostics(&self) -> serde_json::Value {
        self.transport_ink.as_ref().map(|(preferred,samples,ink)|serde_json::json!({"preferred":format!("#{preferred:06X}"),"foreground":format!("#{ink:06X}"),"backgrounds":samples,"minimumContrast":samples.iter().map(|bg|crate::cover_color::contrast(*ink,*bg)).fold(f64::INFINITY,f64::min),"minimumStateContrast":samples.iter().flat_map(|bg|[0.,0.08,0.12].map(move|alpha|crate::cover_color::contrast(*ink,crate::cover_color::mix(*bg,*ink,alpha)))).fold(f64::INFINITY,f64::min)})).unwrap_or(serde_json::Value::Null)
    }
    fn button(&mut self, id: i32, r: Rect, s: &Scene<'_>) {
        let disabled = ([3, 4, 5].contains(&id) && !s.has_track)
            || (id == 2 && !s.floating)
            || (id == 9 && !s.favorite_enabled)
            || (id == 6 && !s.translation_button);
        let mut c = s.component_color;
        if disabled {
            c = 0x4d000000 | (c & 0xffffff);
        }
        let feedback = s.feedback.get(id as usize).copied().unwrap_or(0.);
        let press = if s.pressed == id { 0.96 } else { 1. };
        let control = if [3, 4, 5].contains(&id) || id == 6 {
            0xff000000 | self.music_control_ink(s)
        } else {
            c
        };
        let on_accent = if crate::cover_color::contrast(c, 0x17201d)
            > crate::cover_color::contrast(c, 0xffffff)
        {
            0xff17201d
        } else {
            0xffffffff
        };
        let tonal = if matches!(s.material, Material::LiquidGlass) && self.is_liquid_glass_light() {
            0x18141820
        } else {
            0xff000000 | self.m3(s).secondary_container
        };
        let mix = if s.animate {
            s.play_shape.clamp(0., 1.)
        } else {
            if s.playing {
                1.
            } else {
                0.
            }
        };
        let inset = if s.transport_theme == dotisle_render::transport::TransportTheme::Reef {
            (feedback as f64 * 9.).min(1.5)
        } else {
            0.
        };
        let bg = Rect::new(r.x + inset, r.y + inset, r.w - inset * 2., r.h - inset * 2.);
        let ink_fill = 0xff000000 | self.m3(s).on_surface;
        let light_field_btn = matches!(s.material, Material::LightField | Material::Acrylic);
        if id == 4 && light_field_btn && s.transport_theme == dotisle_render::transport::TransportTheme::Reef {
            // 曜光视效主控球：极通透液态光学聚光圆镜（中心透底、底缘聚光微染专辑或冰蓝、上下受光玻璃细环）
            let center = vr(r).center();
            let rad = r.w.min(r.h) * 0.5;
            let boost = if s.pressed == id { 1.45 } else if s.hover == id { 1.22 } else { 1.0 };
            let c_a = |a: f64, rgb: u32| -> Color {
                color((((a * boost).clamp(0., 0.92) * 255.).round() as u32) << 24 | (rgb & 0xffffff))
            };
            let accent_rgb = s.component_color & 0xffffff;
            let has_tint = accent_rgb != 0xFFFFFF && accent_rgb != 0xDDF6FF;
            let glow_mid = if has_tint {
                crate::cover_color::mix(0x768FD8, accent_rgb, 0.58)
            } else {
                0x768FD8
            };
            let glow_bot = if has_tint {
                crate::cover_color::mix(0x9BB5FF, accent_rgb, 0.65)
            } else {
                0x9BB5FF
            };
            let rim_bot = if has_tint {
                crate::cover_color::mix(0xC8DCFF, accent_rgb, 0.45)
            } else {
                0xC8DCFF
            };
            let lens_fill = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.00, c_a(0.05, 0xFFFFFF)),
                (0.48, c_a(0.12, glow_mid)),
                (1.00, c_a(0.28, glow_bot)),
            ]);
            self.canvas.fill(Fill::NonZero, Affine::IDENTITY, &lens_fill, None, &Circle::new(center, rad));
            let rim = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                (0.00, c_a(0.28, 0xFFFFFF)),
                (0.48, c_a(0.09, 0xFFFFFF)),
                (1.00, c_a(0.48, rim_bot)),
            ]);
            self.canvas.stroke(&Stroke::new(1.0), Affine::IDENTITY, &rim, None, &Circle::new(center, rad - 0.5));
            if feedback > 0.001 && !disabled {
                // 曜光柔点 + 曜光扩散环组合按压反馈
                let glow_r = (r.w * 0.86) as f32;
                let fa = (feedback as f64 * 2.4).clamp(0., 0.42);
                let g = Gradient::new_radial(center, glow_r).with_stops([
                    color((((fa * 255.) as u32) << 24) | 0xFFFFFF),
                    color((((fa * 0.42 * 255.) as u32) << 24) | rim_bot),
                    color(0x00000000 | rim_bot),
                ]);
                self.canvas.fill(Fill::NonZero, Affine::IDENTITY, &g, None, &Circle::new(center, glow_r as f64));
                let ring_r = rad + (feedback as f64 / 0.16).clamp(0., 1.) * 9.0;
                self.canvas.stroke(
                    &Stroke::new(1.15),
                    Affine::IDENTITY,
                    color((((fa * 0.85 * 255.) as u32) << 24) | 0xFFFFFF),
                    None,
                    &Circle::new(center, ring_r),
                );
            }
            let size = 24. * press;
            let icon = Rect::new(r.x + (r.w - size) / 2., r.y + (r.h - size) / 2., size, size);
            use dotisle_render::transport_icons::{paint_icon, Glyph};
            if mix < 0.999 {
                self.clip(icon, 0., (1. - mix) as f32);
                paint_icon(&mut self.canvas, Glyph::Play, s.transport_theme, vr(icon), (((1. - mix) * 255.) as u32) << 24 | 0xffffff);
                self.canvas.pop_layer();
            }
            if mix > 0.001 {
                self.clip(icon, 0., mix as f32);
                paint_icon(&mut self.canvas, Glyph::Pause, s.transport_theme, vr(icon), ((mix * 255.) as u32) << 24 | 0xffffff);
                self.canvas.pop_layer();
            }
            return;
        }
        let play_ink = dotisle_render::transport::background(
            &mut self.canvas,
            id,
            vr(bg),
            s.transport_theme,
            dotisle_render::transport::Colors {
                accent: c,
                on_accent,
                control,
                tonal,
                ink_fill,
            },
            mix,
        );
        if feedback > 0.001 && !disabled && light_field_btn {
            // ColorOS 17 按键纯白凝光聚点反馈
            let rad = (r.w * 0.72).max(15.) as f32;
            let fa = (feedback as f64 * 2.2).clamp(0., 0.34);
            let ripple = Gradient::new_radial(vr(r).center(), rad).with_stops([
                color((((fa * 255.) as u32) << 24) | 0xFFFFFF),
                color((((fa * 0.35 * 255.) as u32) << 24) | 0xD8F0FF),
                color(0x00D8F0FF),
            ]);
            self.canvas.fill(Fill::NonZero, Affine::IDENTITY, &ripple, None, &Circle::new(vr(r).center(), rad as f64));
        } else {
            if feedback > 0.001 && !disabled && ![3, 5].contains(&id) {
                self.fill(r, 12., ((feedback * 255.) as u32) << 24 | 0xffffff);
            }
            if [3, 5].contains(&id) && !disabled && feedback > 0.001 {
                self.fill(
                    r,
                    12.,
                    ((feedback.min(0.12) * 255.) as u32) << 24 | (control & 0xffffff),
                );
            }
        }
        let size = if id == 4 {
            24.
        } else {
            if [3, 5].contains(&id) {
                22.
            } else {
                20.
            }
        } * press;
        let icon = Rect::new(r.x + (r.w - size) / 2., r.y + (r.h - size) / 2., size, size);
        use dotisle_render::transport_icons::{paint_icon, Glyph};
        if id == 4 {
            let ink = play_ink;
            if mix < 0.999 {
                self.clip(icon, 0., (1. - mix) as f32);
                paint_icon(
                    &mut self.canvas,
                    Glyph::Play,
                    s.transport_theme,
                    vr(icon),
                    (((1. - mix) * 255.) as u32) << 24 | (ink & 0xffffff),
                );
                self.canvas.pop_layer();
            }
            if mix > 0.001 {
                self.clip(icon, 0., mix as f32);
                paint_icon(
                    &mut self.canvas,
                    Glyph::Pause,
                    s.transport_theme,
                    vr(icon),
                    ((mix * 255.) as u32) << 24 | (ink & 0xffffff),
                );
                self.canvas.pop_layer();
            }
            return;
        }
        let (glyph, ink) = match id {
            3 => (Glyph::Prev, control),
            5 => (Glyph::Next, control),
            7 => (Glyph::Settings, c),
            2 => (Glyph::Shuffle, c),
            6 => (Glyph::Translate, control),
            9 => (Glyph::Heart, c),
            16 => (Glyph::Volume, c),
            _ => return,
        };
        if id == 6 && disabled {
            return;
        }
        paint_icon(&mut self.canvas, glyph, s.transport_theme, vr(icon), ink);
    }
    fn music_expanded(&mut self, s: &Scene<'_>) {
        let w = s.layout.card.w;
        let g = s.custom_geometry.unwrap_or_else(|| {
            crate::layout::music_geometry(
                w,
                s.music_layout,
                s.music_mix,
                s.layout_scale,
                s.font_size,
            )
        });
        let secondary = 0xb8000000 | (s.text_color & 0xffffff);
        let immersion = lyric_core::music_layout::immersion_amount(
            s.music_layout,
            s.music_mix,
            s.custom_geometry.is_some(),
        );
        let fluid_layout = s.custom_geometry.is_none() && s.music_mix < 0.35;
        let side_layout = false;
        let separated_layout = s.custom_geometry.is_none() && (s.music_mix - 1.0).abs() < 0.35;
        let focus_layout = s.custom_geometry.is_none() && (s.music_mix - 2.0).abs() < 0.35;
        let light_field_mode = matches!(s.material, Material::LightField);
        if g.cover.w > 0. && immersion < 0.999 {
            let cr = crate::layout::cover_corner_radius(s.radius, g.cover.w, g.cover.h);
            if fluid_layout || light_field_mode {
                self.canvas.draw_blurred_rounded_rect(
                    Affine::translate((0., 4.)),
                    vr(g.cover),
                    color(0x4A000000),
                    cr,
                    8.,
                );
            }
            // 切歌封面切换曜光/液态玻璃/亚克力通用外发光脉冲
            let cover_pulse = if matches!(s.material, Material::LightField | Material::Acrylic) && s.animate && s.cover_age_ms >= 0. && s.cover_age_ms < 680. {
                let t = (s.cover_age_ms / 680.).clamp(0., 1.);
                (std::f64::consts::PI * t).sin().powf(0.8)
            } else {
                0.
            };
            if cover_pulse > 0.01 {
                let pa = ((cover_pulse * 0.52 * 255.).round() as u32) << 24;
                self.canvas.draw_blurred_rounded_rect(
                    Affine::IDENTITY,
                    vr(g.cover),
                    color(pa | 0xE6F6FF),
                    cr,
                    7.5,
                );
            }
            self.clip(g.cover, 0., (1. - immersion) as f32);
            self.flipping_cover(g.cover, s);
            // 切歌瞬间在封面玻璃表层斜掠一道极清透折射高光
            if cover_pulse > 0.01 {
                let t = (s.cover_age_ms / 680.).clamp(0., 1.);
                let eased = 1. - (1. - t).powi(3);
                let sx = g.cover.x - g.cover.w * 0.55 + eased * g.cover.w * 2.1;
                let bw = (g.cover.w * 0.34).max(20.);
                let ga = ((cover_pulse * 0.32 * 255.).round() as u32) << 24;
                let sheen = Gradient::new_linear((sx - bw, g.cover.y), (sx + bw, g.cover.y + g.cover.h))
                    .with_stops([
                        (0.0, color(0x00FFFFFF)),
                        (0.5, color(ga | 0xFFFFFF)),
                        (1.0, color(0x00FFFFFF)),
                    ]);
                self.canvas.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &sheen,
                    None,
                    &RoundedRect::from_rect(vr(g.cover), cr),
                );
            }
            self.canvas.pop_layer();
            if fluid_layout || light_field_mode {
                let edge_a = ((0.24 + 0.55 * cover_pulse).clamp(0., 0.9) * 255.).round() as u32;
                self.outline(g.cover, cr, (edge_a << 24) | 0xFFFFFF, if cover_pulse > 0.05 { 1.25 } else { 0.85 });
            }
            if fluid_layout || side_layout {
                let gloss = Rect::new(g.cover.x + 10., g.cover.y + g.cover.h + 2.5, (g.cover.w - 20.).max(20.), 1.3);
                let shimmer = Gradient::new_linear((gloss.x, gloss.y), (gloss.x + gloss.w, gloss.y))
                    .with_stops([
                        (0.0, color(0x00FFFFFF)),
                        (0.5, color(0x3CFFFFFF)),
                        (1.0, color(0x00FFFFFF)),
                    ]);
                self.canvas.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    &shimmer,
                    None,
                    &RoundedRect::from_rect(vr(gloss), 0.65),
                );
            }
        }
        let head_align = if focus_layout { 0. } else { g.align };
        let title_sz = if fluid_layout || side_layout {
            18.
        } else if focus_layout {
            14.5
        } else {
            16. - 2. * immersion
        };
        self.text_at(
            s.title,
            g.title,
            title_sz,
            600,
            0xff000000 | (s.text_color & 0xffffff),
            head_align,
        );
        if !s.mini {
            self.text_at(
                s.artist,
                g.artist,
                if fluid_layout || side_layout { 11.5 } else if focus_layout { 10.5 } else { 11. },
                400,
                secondary,
                head_align,
            );
        }
        if fluid_layout && g.lyric.w > 0. {
            // 曜光动态引导竖线：播放态随音乐节奏呼吸伸缩 + 游走流光焦点 + 切句弹入闪烁；暂停时温柔收敛
            let t = s.time_ms / 1000.;
            let enter = if s.animate {
                let p = (s.lyric_age_ms / 320.).clamp(0., 1.);
                1. - (1. - p).powi(3)
            } else {
                1.
            };
            let flash = if s.animate && s.lyric_age_ms < 420. {
                (1. - s.lyric_age_ms / 420.).clamp(0., 1.)
            } else {
                0.
            };
            let pulse = if s.playing && s.animate {
                0.5 + 0.5 * (t * 3.1).sin()
            } else {
                0.
            };
            let beat = if s.playing {
                (s.activity_scales[1] * 0.65 + pulse * 0.35).clamp(0.25, 1.15)
            } else {
                0.
            };
            let spine_h = if s.playing {
                (26. + 8.5 * beat) * (0.45 + 0.55 * enter)
            } else {
                20.
            };
            let spine_y = g.lyric.y + 3. + (34. - spine_h) * 0.5;
            let spine = Rect::new(135.5, spine_y, 1.8, spine_h.max(8.));
            let glow_rgb = s.progress_color & 0xffffff;
            let accent_col = if glow_rgb != 0xFFFFFF && glow_rgb != 0xDDF6FF {
                crate::cover_color::mix(0xDDF4FF, glow_rgb, 0.55)
            } else {
                0xDDF4FF
            };
            let halo_a = if s.playing {
                ((0.22 + 0.20 * beat + 0.24 * flash).clamp(0., 0.65) * 255.) as u32
            } else {
                0x14
            };
            self.canvas.draw_blurred_rounded_rect(
                Affine::IDENTITY,
                vr(spine),
                color((halo_a << 24) | accent_col),
                0.9,
                3.8 + 1.5 * flash,
            );
            let focus = if s.playing && s.animate {
                (0.5 + 0.35 * (t * 2.1).sin()).clamp(0.16, 0.84)
            } else {
                0.28
            };
            let fade_mul = if s.playing { 1.0 } else { 0.55 };
            let core_a = (((0.94 * fade_mul) * 255.) as u32) << 24;
            let edge_a = (((0.62 * fade_mul) * 255.) as u32) << 24;
            let beam = Gradient::new_linear((spine.x, spine.y), (spine.x, spine.y + spine.h))
                .with_stops([
                    (0.0, color(0x00000000 | accent_col)),
                    ((focus - 0.14).max(0.05) as f32, color(edge_a | accent_col)),
                    (focus as f32, color(core_a | 0xFFFFFF)),
                    ((focus + 0.16).min(0.95) as f32, color(edge_a | accent_col)),
                    (1.0, color(0x00000000 | accent_col)),
                ]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &beam,
                None,
                &RoundedRect::from_rect(vr(spine), 0.9),
            );
        } else if separated_layout && g.lyric.w > 0. {
            // 「流光分栏」电影顶眉与全宽歌词剧场之间的极细居中渐隐玻璃分隔线
            let div = Rect::new(22., 79., (w - 44.).max(40.), 1.0);
            let sep_line = Gradient::new_linear((div.x, div.y), (div.x + div.w, div.y)).with_stops([
                (0.0, color(0x00FFFFFF)),
                (0.5, color(0x22FFFFFF)),
                (1.0, color(0x00FFFFFF)),
            ]);
            self.canvas.fill(Fill::NonZero, Affine::IDENTITY, &sep_line, None, &RoundedRect::from_rect(vr(div), 0.5));
        }
        if immersion > 0.5 {
            let size = (s.font_size + 2.) + (s.font_size + 6.).clamp(20., 23.) * immersion
                - (s.font_size + 2.) * immersion;
            let chars = s.lyric.chars().collect::<Vec<_>>();
            let mut split = chars.len();
            if self.text.measure_weighted(s.lyric, size as f32, 500) > g.lyric.w {
                let (mut low, mut high) = (1, chars.len());
                while low < high {
                    let n = (low + high + 1) / 2;
                    let part = chars[..n].iter().collect::<String>();
                    if self.text.measure_weighted(&part, size as f32, 500) <= g.lyric.w {
                        low = n
                    } else {
                        high = n - 1
                    }
                }
                split = low;
                if let Some(i) = chars[..split].iter().rposition(|c| c.is_whitespace()) {
                    if i > split / 2 {
                        split = i;
                    }
                }
            }
            let first = chars[..split].iter().collect::<String>();
            let second = chars[split..].iter().collect::<String>();
            let rows = if second.trim().is_empty() { 1 } else { 2 };
            let line = 30.;
            let context = (1. - s.controls_opacity) as f32;
            let next = if s.next_context != s.secondary {
                s.next_context
            } else {
                ""
            };
            let text = lyric_core::music_layout::immersive_text_geometry(
                g.height,
                rows,
                !s.previous_context.is_empty(),
                !s.secondary.is_empty(),
                !next.is_empty(),
                s.controls_opacity,
            );
            let mut band = g.lyric;
            band.y = text.current_y;
            if context > 0.001 {
                self.clip(Rect::new(0., 0., w, g.height), 0., context);
                self.text_line(
                    s.previous_context,
                    Rect::new(24., text.previous_y, w - 48., 18.),
                    12.,
                    400,
                    0x60000000 | (s.text_color & 0xffffff),
                );
                self.text_line(
                    next,
                    Rect::new(24., text.next_y, w - 48., 22.),
                    14.,
                    400,
                    0x78000000 | (s.text_color & 0xffffff),
                );
                self.canvas.pop_layer();
            }
            let mut lyric = *s;
            if rows == 1 {
                lyric.highlight = s.highlight;
                self.music_lyric(
                    &first,
                    Rect::new(band.x, band.y, band.w, line),
                    size,
                    0.,
                    s.text_color,
                    &lyric,
                );
            } else {
                let weight = split as f64 / chars.len().max(1) as f64;
                lyric.highlight = (s.highlight / weight.max(0.001)).clamp(0., 1.);
                self.music_lyric(
                    &first,
                    Rect::new(band.x, band.y, band.w, line),
                    size,
                    0.,
                    s.text_color,
                    &lyric,
                );
                lyric.highlight = ((s.highlight - weight) / (1. - weight).max(0.001)).clamp(0., 1.);
                self.music_lyric(
                    second.trim(),
                    Rect::new(band.x, band.y + line + 2., band.w, line),
                    size,
                    0.,
                    s.text_color,
                    &lyric,
                );
            }
            self.text_at(
                s.secondary,
                Rect::new(g.secondary.x, text.secondary_y, g.secondary.w, 18.),
                10.5,
                400,
                secondary,
                0.,
            );
        } else if g.lyric.w > 0. {
            let lyric_sz = if focus_layout {
                s.font_size + 4.5
            } else {
                s.font_size + 2.
            };
            self.music_lyric(s.lyric, g.lyric, lyric_sz, g.align, s.text_color, s);
        }
        if !s.mini {
            if immersion <= 0.5 && g.secondary.w > 0. {
                self.text_at(s.secondary, g.secondary, 10.5, 400, secondary, g.align);
            }
            if g.status_text.w > 0. {
                let status = if !s.has_track {
                    "等待播放"
                } else if s.playing {
                    "正在播放"
                } else {
                    "已暂停"
                };
                if fluid_layout || ((side_layout || separated_layout) && light_field_mode) {
                    let lg_light = matches!(s.material, Material::LiquidGlass) && self.is_liquid_glass_light();
                    let wave_rgb = s.progress_color & 0xffffff;
                    let active_wave = if lg_light {
                        0xEB000000 | wave_rgb
                    } else if wave_rgb != 0xFFFFFF && wave_rgb != 0xDDF6FF {
                        0xEB000000 | crate::cover_color::mix(wave_rgb, 0xFFFFFF, 0.35)
                    } else {
                        0xEBF2FAFF
                    };
                    let paused_wave = if lg_light {
                        0x70000000 | (s.text_color & 0xffffff)
                    } else {
                        0x70FFFFFF
                    };
                    let status_ink = if lg_light {
                        0xCC000000 | (s.text_color & 0xffffff)
                    } else {
                        0xCCFFFFFF
                    };
                    for (i, bh) in [5.5, 9., 6.].iter().enumerate() {
                        let fallback_pulse = if s.animate {
                            0.42 + 0.56 * ((s.time_ms / 240. + i as f64 * 1.35).sin().abs())
                        } else {
                            0.65
                        };
                        let sc = if s.playing {
                            s.activity_scales[i].max(fallback_pulse)
                        } else {
                            0.42
                        };
                        let h = bh * sc;
                        self.fill(
                            Rect::new(
                                g.status_dot.x + i as f64 * 3.5,
                                g.status_text.y + g.status_text.h / 2. - h / 2.,
                                1.8,
                                h,
                            ),
                            0.9,
                            if s.playing { active_wave } else { paused_wave },
                        );
                    }
                    let tx = g.status_dot.x + 15.;
                    let tw = (g.status_text.x + g.status_text.w - tx).max(20.);
                    self.text_line(
                        status,
                        Rect::new(tx, g.status_text.y, tw, g.status_text.h),
                        9.5,
                        500,
                        status_ink,
                    );
                } else if g.align >= 0.5 {
                    let tw = self
                        .text
                        .measure_weighted(status, 9., 400)
                        .min(g.status_text.w);
                    let total = 4. + 6. + tw;
                    let x = g.status_text.x + (g.status_text.w - total) * 0.5;
                    let dot = self.status_indicator(
                        status,
                        Rect::new(x, g.status_dot.y, 4., 4.),
                        Rect::new(x + 10., g.status_text.y, tw + 2., g.status_text.h),
                    );
                    self.fill(dot, dot.w / 2., s.progress_color);
                    self.text_line(
                        status,
                        Rect::new(x + 10., g.status_text.y, tw + 2., g.status_text.h),
                        9.,
                        400,
                        secondary,
                    );
                } else {
                    let dot = self.status_indicator(status, g.status_dot, g.status_text);
                    self.fill(dot, dot.w / 2., s.progress_color);
                    self.text_line(status, g.status_text, 9., 400, secondary);
                }
            }
        }
        if s.controls_opacity <= 0.001 {
            return;
        }
        self.clip(
            Rect::new(0., 0., w, g.height),
            0.,
            s.controls_opacity as f32,
        );
        let rail = if s.seek_offer {
            0x2e000000 | (s.component_color & 0xffffff)
        } else {
            0x48000000 | (s.component_color & 0xffffff)
        };
        // The same shared painter keeps the island and the control centre previews identical.
        let look = dotisle_render::progress::Look {
            fg: s.component_color,
            track: rail,
            dot: 0x8a000000 | (s.component_color & 0xffffff),
        };
        let wave = if s.animate {
            s.play_shape.clamp(0., 1.)
        } else {
            0.
        };
        let phase = if s.animate {
            s.time_ms / 1000. * 2.
        } else {
            0.
        };
        let spin = if s.progress_thumb_spin && s.animate {
            s.time_ms / 1000. * 0.25
        } else {
            0.
        };
        let thumb = if s.pressed == 10 { 16. } else { 14. };
        let light_field_bar = matches!(s.material, Material::LightField | Material::Acrylic);
        let progress_active = s.hover == 10 || s.pressed == 10;
        if g.track.w > 0. {
            paint_playback_progress(&mut self.canvas, g.track, s.progress, phase, thumb, wave, spin,
                look, s.progress_bar, s.progress_thumb, s.progress_glow && !s.seek_offer,
                light_field_bar, progress_active, s.pressed == 10);
            self.text_line(&time(s.elapsed), g.elapsed, 9., 400, secondary);
            self.text_at(
                &if s.duration > 0. {
                    format!("−{}", time((s.duration - s.elapsed).max(0.)))
                } else {
                    "--:--".into()
                },
                g.total,
                9.,
                400,
                secondary,
                1.,
            );
            if s.seek_offer && g.progress_hit.w > 0. {
                let tones = self.m3(s);
                let r = g.seek_chip;
                if s.seek_confirm {
                    let (cancel, confirm) = lyric_core::music_layout::seek_confirm_pair(r);
                    let fb_cancel = s.feedback.get(14).copied().unwrap_or(0.);
                    let fb_confirm = s.feedback.get(15).copied().unwrap_or(0.);
                    self.fill(
                        cancel,
                        cancel.h / 2.,
                        0xff000000 | tones.secondary_container,
                    );
                    if fb_cancel > 0.001 {
                        self.fill(
                            cancel,
                            cancel.h / 2.,
                            ((fb_cancel * 255.) as u32) << 24
                                | (tones.on_secondary_container & 0xffffff),
                        );
                    }
                    self.text_at(
                        "取消",
                        cancel,
                        10.5,
                        600,
                        0xff000000 | tones.on_secondary_container,
                        0.5,
                    );
                    self.fill(confirm, confirm.h / 2., 0xff000000 | tones.primary);
                    if fb_confirm > 0.001 {
                        self.fill(
                            confirm,
                            confirm.h / 2.,
                            ((fb_confirm * 255.) as u32) << 24 | (tones.on_primary & 0xffffff),
                        );
                    }
                    self.text_at(
                        "查看说明",
                        confirm,
                        10.5,
                        600,
                        0xff000000 | tones.on_primary,
                        0.5,
                    );
                } else {
                    let fb = s.feedback.get(14).copied().unwrap_or(0.);
                    self.fill(r, r.h / 2., 0xff000000 | tones.secondary_container);
                    if fb > 0.001 {
                        self.fill(
                            r,
                            r.h / 2.,
                            ((fb * 255.) as u32) << 24 | (tones.on_secondary_container & 0xffffff),
                        );
                    }
                    self.text_at(
                        "连接说明",
                        Rect::new(r.x, r.y, r.w, r.h),
                        11.,
                        600,
                        0xff000000 | tones.on_secondary_container,
                        0.5,
                    );
                }
            }
        }
        if fluid_layout {
            // 保持底部操控区通透无多余灰框，仅在右上两枚轻量辅助钮留极细折射暗玻璃圈
            for (id, r) in g.buttons {
                if (id == 6 || id == 7) && r.w > 0. {
                    self.fill(r, r.w / 2., 0x0CFFFFFF);
                    self.outline(r, r.w / 2., 0x22FFFFFF, 0.7);
                }
            }
        }
        for (id, r) in g.buttons {
            if r.w > 0. {
                self.button(id, r, s);
            }
        }
        if s.translation_tooltip && (s.hover == 16 || (s.hover == 6 && s.translation_button)) {
            if let Some((_, r)) = g.buttons.iter().find(|(id, _)| *id == s.hover) {
                let label=if s.hover==16 {match s.volume_level {Some(level)=>format!("滚轮调节音量 · {level}%{}",if s.volume_muted{" · 静音"}else{""}),None=>"暂无可用的音频设备".into()}}else if s.translation_enabled{"隐藏翻译 · 已开启".into()}else{"显示翻译 · 已关闭".into()};
                let tw=if s.hover==16{if s.volume_muted{204.}else{174.}}else{128.};
                let x=(r.x+r.w/2.-tw/2.).clamp(12.,w-tw-12.);
                let tip=Rect::new(x,r.y-38.,tw,30.);
                self.fill(tip,10.,0xffe9edef);
                self.text_at(&label,Rect::new(tip.x,tip.y+6.,tip.w,18.),11.,500,0xff20272b,0.5);
            }
        }
        self.canvas.pop_layer();
    }
    fn tool_button(&mut self, id: i32, r: Rect, label: &str, s: &Scene<'_>) {
        self.m3_button(id, r, label, s, id == 30)
    }
    fn resident_expanded(&mut self, s: &Scene<'_>) {
        let v = s.resident;
        let w = s.layout.card.w;
        let tones = self.m3(s);
        let ink = 0xff000000 | tones.on_surface;
        let muted = 0xff000000 | tones.on_surface_variant;
        self.text_at(&v.clock, Rect::new(20., 13., w - 88., 34.), 24., 600, ink, 0.);
        self.activity_close_button(s, crate::resident::COLLAPSE, crate::layout::activity_close(w));
        for (i, (label, value)) in [("CPU", &v.cpu), ("内存", &v.memory)].into_iter().enumerate() {
            let width = (w - 50.) / 2.;
            let x = 20. + i as f64 * (width + 10.);
            self.fill(Rect::new(x, 53., width, 35.), 12., 0x16777777);
            self.text_at(label, Rect::new(x + 12., 53., width - 24., 35.), 12., 500, muted, 0.);
            self.text_at(value, Rect::new(x + 12., 53., width - 24., 35.), 13., 600, ink, 1.);
        }
        for (id, rect, label) in crate::resident::buttons(w) {
            self.m3_button(id, rect, label, s, id == crate::resident::FOCUS);
        }
        if !v.devices.is_empty() {
            self.fill(Rect::new(20., 146., w - 40., 1.), 0.5, 0x22777777);
            self.text_at("已连接设备", Rect::new(20., 152., w - 40., 23.), 11., 500, muted, 0.);
            for (i, row) in v.devices.iter().enumerate() {
                let y = 174. + i as f64 * 23.;
                let name = self.elided_line(&row.name, w - 100., 12., 500);
                self.text_at(&name, Rect::new(20., y, w - 100., 23.), 12., 500, ink, 0.);
                self.text_at(&row.battery, Rect::new(w - 70., y, 50., 23.), 12., 500, muted, 1.);
            }
            if v.more_devices > 0 {
                self.text_at(&format!("另有 {} 台已连接设备", v.more_devices),
                    Rect::new(20., 175. + v.devices.len() as f64 * 23., w - 40., 22.), 11., 400, muted, 0.);
            }
        }
    }
    fn tool_expanded(&mut self, s: &Scene<'_>, p: &crate::activities::Panel) {
        if p.kind == crate::gesture_practice::KIND {
            let w = s.layout.card.w;
            let c = self.m3(s);
            self.text_at(&p.title, Rect::new(20., 22., w - 40., 32.), 22., 600, 0xff000000 | c.on_surface, 0.);
            self.text_at("请按控制中心提示操作", Rect::new(20., 68., w - 40., 24.), 13., 400, 0xff000000 | c.on_surface_variant, 0.);
            if s.practice_move { self.m3_button(2, crate::gesture_practice::move_button(s.layout.card.h), "拖动移动", s, true); }
            return;
        }
        if p.kind == crate::resident::KIND {
            self.resident_expanded(s);
            return;
        }
        if p.kind==4 {
            let c=self.m3(s);let w=s.layout.card.w;let ink=0xff000000|c.on_surface;let muted=0xff000000|c.on_surface_variant;let accent=0xff000000|c.primary;
            self.activity_close_button(s,crate::layout::ACTIVITY_CLOSE,crate::layout::activity_close(w));
            self.text_at(&p.title,Rect::new(20.,18.,w-80.,22.),14.,600,ink,0.);let mut y=52.;
            for b in &p.blocks {let height=match b.kind.as_str(){"metric"=>56.,"progress"=>40.,"divider"=>12.,_=>28.};
                if b.kind=="divider"{self.fill(Rect::new(20.,y+5.,w-40.,1.),0.5,0x22777777);}
                else if b.kind=="progress"{self.text_at(&b.label,Rect::new(20.,y,w-40.,18.),11.,400,muted,0.);self.fill(Rect::new(20.,y+25.,w-40.,4.),2.,0x22777777);if b.value>0.{self.fill(Rect::new(20.,y+25.,(w-40.)*b.value,4.),2.,accent);}}
                else {self.text_at(&b.text,Rect::new(20.,y,w-40.,if b.kind=="metric"{32.}else{22.}),if b.kind=="metric"{26.}else{13.},500,ink,0.);if b.kind=="metric"{self.text_at(&b.label,Rect::new(20.,y+34.,w-40.,18.),11.,400,muted,0.);}}y+=height;
            }
        }else{self.expressive_tools(s, p)}
        let id=if p.kind==3{51}else{crate::layout::ACTIVITY_CLOSE};
        if s.translation_tooltip && s.hover==id {
            let w=s.layout.card.w;let close=if p.kind==3{crate::layout::update_close(w)}else{crate::layout::activity_close(w)};
            let label=p.delete_hint();let tw=(self.text.measure_weighted(label,11.,500)+24.).min(w-40.);
            let tip=Rect::new(w-20.-tw,close.y+close.h+6.,tw,30.);
            self.fill(tip,10.,0xffe9edef);
            self.text_at(label,Rect::new(tip.x+8.,tip.y+6.,tip.w-16.,18.),11.,500,0xff20272b,0.5);
        }
    }
    fn expanded_view(&mut self, s: &Scene<'_>) {
        let mut expanded = *s;
        expanded.mini = false;
        let s = &expanded;
        if s.panel.is_some_and(|p|p.kind==crate::agent_island::KIND) {
            self.agent_card(s);
        } else if let Some(p) = s.panel {
            self.tool_expanded(s, p)
        } else {
            self.music_expanded(s)
        }
    }
    fn satellite(&mut self, s: &Scene<'_>) {
        let mut compact = *s;
        compact.expressive_dark = true;
        let s = &compact;
        let r = s.layout.satellite();
        let reveal = (1. - s.frame.card_reveal / 0.06).clamp(0., 1.);
        if reveal <= 0. {
            return;
        }
        self.clip(
            Rect::new(r.x - 24., r.y - 24., r.w + 48., r.h + 48.),
            0.,
            reveal as f32,
        );
        let tones = self.m3(s);
        let mut surface = Self::surface_spec(s);
        surface.reveal = 0.;
        surface.overlay = 0.;
        self.canvas.draw_blurred_rounded_rect(
            Affine::translate((0., 5.)),
            vr(r),
            color(0x38000000),
            r.h / 2.,
            22. / 3.,
        );
        self.material_surface(r, r.h / 2., surface);
        if let Some((progress, elapsed)) = s.auxiliary_timer {
            crate::liquid_timer::paint_rising(
                &mut self.canvas,
                vr(r),
                r.h / 2.,
                progress,
                elapsed,
                0xff000000 | tones.primary,
                s.animate,
            );
        }
        let cx = r.x + r.w / 2.;
        let cy = r.y + r.h / 2.;
        let c = 0xff000000 | tones.on_secondary_container;
        // Secondary activity state is supplied by the owner; the icon never guesses from the primary panel.
        match s.auxiliary_kind {
            3 => {
                if s.auxiliary_update_completed {
                    self.icon(
                        Icon::Check,
                        Rect::new(cx - 9., cy - 9., 18., 18.),
                        0xff9bd5ae,
                    );
                } else {
                    self.update_ring(
                        cx,
                        cy,
                        9.,
                        s.auxiliary_update_progress,
                        s.time_ms,
                        s.animate && s.auxiliary_running,
                        0xff9bd5ae,
                    );
                }
            }
            1 => {
                self.ring(cx, cy, 8., c, 1.4);
                let angle = if s.animate && s.auxiliary_running {
                    -std::f64::consts::FRAC_PI_2
                        + s.auxiliary_elapsed / 60000. * std::f64::consts::TAU
                } else {
                    -std::f64::consts::FRAC_PI_2
                };
                self.line(
                    (cx, cy),
                    (cx + angle.cos() * 5., cy + angle.sin() * 5.),
                    c,
                    1.4,
                );
                self.line((cx, cy), (cx + 4., cy + 2.), c, 1.4);
            }
            2 => self.icon(Icon::Check, Rect::new(cx - 9., cy - 9., 18., 18.), c),
            _ => {
                for (i, base) in [8., 14., 10.].iter().enumerate() {
                    let h = if s.animate && s.auxiliary_running {
                        6. + (s.time_ms / 270. + i as f64 * 1.4).sin().abs() * 9.
                    } else {
                        *base * 0.65
                    };
                    self.fill(
                        Rect::new(cx - 6. + i as f64 * 5., cy - h / 2., 2., h),
                        1.,
                        c,
                    );
                }
            }
        }
        self.canvas.pop_layer();
    }
    fn elided_line(&mut self,text:&str,width:f64,size:f32,weight:u16)->String{
        if self.text.measure_weighted(text,size,weight)<=width{return text.into()}
        let ends=text.char_indices().map(|(i,_)|i).chain(Some(text.len())).collect::<Vec<_>>();
        let available=(width-self.text.measure_weighted("…",size,weight)).max(0.);
        let(mut lo,mut hi)=(0,ends.len()-1);
        while lo<hi{let mid=(lo+hi+1)/2;if self.text.measure_weighted(&text[..ends[mid]],size,weight)<=available{lo=mid}else{hi=mid-1}}
        format!("{}…",&text[..ends[lo]])
    }
    fn copy_glyph(&mut self,r:Rect,ink:u32){
        let x=r.x+r.w*0.16;let y=r.y+r.h*0.12;
        self.line((x+r.w*0.48,y),(x,y),ink,1.5);
        self.line((x,y),(x,y+r.h*0.62),ink,1.5);
        self.outline(Rect::new(r.x+r.w*0.35,r.y+r.h*0.3,r.w*0.52,r.h*0.58),1.5,ink,1.5);
    }
    fn verification_content(&mut self,a:&crate::capsule_alert::Alert,g:crate::verification_code::Layout){
        let Some(code)=a.code.as_deref()else{return};
        let copied=a.detail=="已复制验证码";let failed=!copied&&a.detail!="点击复制验证码";
        let ink=0xfff5f5f7;let blue=0xff75b4ff;
        let label=if copied{"已复制"}else if failed{"重试复制"}else{"点击复制"};
        let source=if failed{a.detail.as_str()}else if a.title.trim().is_empty(){"验证码"}else{a.title.as_str()};
        let source=self.elided_line(source,g.source.w,12.,500);
        self.fill(g.icon,10.,0xff172536);
        if copied{self.icon(Icon::Check,Rect::new(g.icon.x+6.,g.icon.y+6.,18.,18.),blue)}else{self.copy_glyph(Rect::new(g.icon.x+6.,g.icon.y+6.,18.,18.),blue)}
        self.text_line(&source,g.source,12.,500,0xffaeb4c0);
        // Fit the complete code to its own lane; never ellipsize or clip a digit.
        let measured=self.text.measure_weighted(code,28.,600).max(1.);
        let size=28.*(g.digits.w/measured).min(1.);
        self.text_line(code,g.digits,size,600,ink);
        self.fill(g.action,14.,if copied{0xff19354d}else{0xff24272d});
        if g.compact_action{
            if copied{self.icon(Icon::Check,Rect::new(g.action.x+6.,g.action.y+6.,16.,16.),blue)}else{self.copy_glyph(Rect::new(g.action.x+6.,g.action.y+6.,16.,16.),blue)}
        }else{self.text_at(label,g.action,11.,500,if copied{blue}else{ink},0.5);}
    }
    fn quota_alert_content(&mut self, title: &str, state: crate::capsule_alert::QuotaAlert, r: Rect) {
        let ink = 0xfff5f5f7;
        let muted = 0xffbec6d5;
        let low = 0xffffbd81;
        let track = 0x30b6c5dc;
        let x = r.x + 16.;
        let y = r.y + 14.;
        let title_width = (r.w - 58.).max(0.);
        let title = self.elided_line(title, title_width, 14., 650);
        self.quota_arc(x + 11., y + 11., 8.5, 2., Some(65.), low, track);
        self.text_line(&title, Rect::new(x + 28., y, title_width, 22.), 14., 650, ink);
        if r.h < 116. || r.w < 260. { return; }
        let lane = (r.w - 40.) * 0.5;
        for (i, label) in ["5 小时剩余", "本周剩余"].into_iter().enumerate() {
            let cell = Rect::new(x + i as f64 * (lane + 8.), y + 32., lane, 62.);
            self.fill(cell, 12., 0x16c4d1e8);
            let value = state.remaining[i];
            let tone = if state.low[i] { low } else if i == 0 { 0xff83c5ff } else { 0xffbfb5ff };
            self.quota_arc(cell.x + 26., cell.y + 31., 13., 2.6, value, tone, track);
            let tx = cell.x + 49.;
            let tw = lane - 57.;
            self.text_line(label, Rect::new(tx, cell.y + 10., tw, 17.), 10., 450, muted);
            let value_text = Self::quota_label(value);
            self.text_line(&value_text, Rect::new(tx, cell.y + 28., tw, 25.), 17., 650, if value.is_some() { tone } else { muted });
        }
    }
    fn capsule_alert_content(&mut self,a:&crate::capsule_alert::Alert,r:Rect){
        if let Some(balance) = a.balance {
            let amber = 0xffffbd81;
            let x = r.x + 18.;
            let w = (r.w - 64.).max(0.);
            self.quota_arc(x + 10., r.y + 25., 8., 2., Some(65.), amber, 0x30b6c5dc);
            let title = self.elided_line(&a.title, w, 15., 650);
            self.text_line(&title, Rect::new(x + 27.,r.y + 14.,w,22.),15.,650,0xfff5f5f7);
            if r.h < 103. || r.w < 298. { return; }
            let amount = balance.amount_label();
            let label = if balance.key { "密钥可用余额" } else { "账户可用余额" };
            self.text_line(label,Rect::new(x,r.y+48.,144.,17.),11.,500,0xffbec6d5);
            let amount = self.elided_line(&amount, (r.w-165.).max(0.), 23., 650);
            self.text_line(&amount,Rect::new(x,r.y+69.,r.w-165.,27.),23.,650,amber);
            let pill=Rect::new(r.x+r.w-132.,r.y+66.,114.,30.);
            self.fill(pill,11.,0x24ffbd81);
            let threshold = if balance.currency == crate::ai_quota::RelayUnit::Points { "低于 1 积分".to_owned() }
                else { format!("低于 {}{}", balance.currency.symbol(), crate::ai_quota::RELAY_WARNING) };
            self.text_at(&threshold,pill,11.,600,amber,0.5);
            return;
        }
        if let Some(quota) = a.quota {
            self.quota_alert_content(&a.title, quota, r);
            return;
        }
        let ink=0xfff5f5f7;let muted=0xffaeb4c0;let blue=0xff75b4ff;
        let wellness = matches!(a.icon.as_str(), "WaterReminder" | "StandReminder");
        let glyph=Rect::new(r.x+17.,if wellness {r.y+16.} else {r.y+(r.h-26.)/2.},26.,26.);
        if let Some(image)=a.image.clone(){self.image(&image,glyph,8.,1.);self.outline(glyph,8.,0x2AFFFFFF,0.75);}else{match a.icon.as_str(){
            kind if kind.starts_with("Message:")=>{
                // App identity only: no sender, preview, unread count or chat text.
                let label=match kind.trim_start_matches("Message:"){"WeChat"=>"微","Feishu"=>"飞","DingTalk"=>"钉","WXWork"=>"企",_=>"信"};
                self.fill(glyph,8.,0xff2688ed);
                self.outline(glyph,8.,0x2AFFFFFF,0.75);
                self.text_at(label,Rect::new(glyph.x,glyph.y+1.,glyph.w,glyph.h),14.,600,ink,0.5);
            },
            "Sun"=>self.icon(Icon::Sun,glyph,blue),
            "Rain"|"Snow"=>{
                self.fill(Rect::new(glyph.x,glyph.y+6.,25.,11.),5.5,blue);
                self.fill(Rect::new(glyph.x+5.,glyph.y,15.,18.),7.5,blue);
                for i in 0..3{let x=glyph.x+5.+i as f64*7.;
                    if a.icon=="Snow"{self.line((x-2.,glyph.y+23.),(x+2.,glyph.y+23.),ink,1.4);self.line((x,glyph.y+21.),(x,glyph.y+25.),ink,1.4);}
                    else{self.line((x,glyph.y+20.),(x-2.,glyph.y+25.),blue,1.8);}
                }
            },
            "Code"=>self.copy_glyph(glyph,blue),

            "WaterReminder"=>{
                let x=glyph.x;let y=glyph.y;
                for (a,b) in [( (12.,2.),(5.,12.)),((5.,12.),(4.,17.)),((4.,17.),(7.,21.)),
                    ((7.,21.),(12.,23.)),((12.,23.),(17.,21.)),((17.,21.),(20.,17.)),
                    ((20.,17.),(19.,12.)),((19.,12.),(12.,2.))]{
                    self.line((x+a.0,y+a.1),(x+b.0,y+b.1),blue,1.7);
                }
            },
            "StandReminder"=>{
                let x=glyph.x;let y=glyph.y;
                self.fill(Rect::new(x+10.,y+1.,5.,5.),2.5,blue);
                for (a,b) in [((12.5,9.),(12.5,16.)),((12.5,9.),(5.,12.)),((12.5,9.),(20.,12.)),
                    ((12.5,16.),(7.,23.)),((12.5,16.),(18.,23.))]{
                    self.line((x+a.0,y+a.1),(x+b.0,y+b.1),blue,1.7);
                }
            },
            kind=>self.device_glyph(glyph,kind,blue),
        }}
        let x=r.x+56.;let w=(r.w-76.).max(0.);
        if wellness {
            let accent = if a.icon == "WaterReminder" { 0xff85c7ff } else { 0xffffc27b };
            self.text_line(&a.title,Rect::new(x,r.y+14.,w,25.),17.,650,ink);
            if let Some(water) = a.hydration {
                let amount = format!("今日已喝 {} / {} mL", water.drank_ml, water.goal_ml);
                self.text_line(&amount, Rect::new(x, r.y+44., w, 20.), 13., 500, muted);
                let Some(crate::capsule_alert::WaterLayout {track,later,record}) =
                    crate::capsule_alert::WaterLayout::in_card(r) else { return; };
                self.fill(track, 2.5, 0x36ffffff);
                let fraction = (water.drank_ml as f64 / water.goal_ml as f64).min(1.);
                self.fill(Rect::new(track.x, track.y, track.w*fraction, track.h), 2.5, accent);
                self.fill(later, 11., 0x22ffffff);
                self.fill(record, 11., accent);
                self.text_at("稍后", later, 12., 550, ink, 0.5);
                self.text_at(&format!("已喝 {} mL",water.serving_ml), record, 12., 650, 0xff112033, 0.5);
                return;
            }
            self.text_line(&a.detail,Rect::new(x,r.y+43.,w,20.),12.,450,muted);
            let button = Rect::new(r.x + r.w - 102., r.y + 69., 84., 25.);
            self.fill(button, 12.5, 0x26ffffff);
            self.text_at("知道了",button,11.,550,accent,0.5);
            return;
        }
        if a.detail.is_empty(){
            self.text_line(&a.title,Rect::new(x,r.y+(r.h-20.)/2.,if a.trailing.is_empty(){w}else{w-65.},22.),13.,500,ink);
            if !a.trailing.is_empty(){self.text_at(&a.trailing,Rect::new(r.x+r.w-74.,r.y+(r.h-22.)/2.,54.,24.),15.,600,ink,1.);}
        }else{
            self.text_line(&a.title,Rect::new(x,r.y+13.,w,24.),17.,600,ink);
            for(i,line)in a.detail.lines().take(2).enumerate(){self.text_line(line,Rect::new(x,r.y+43.+i as f64*23.,w,20.),11.5,400,muted);}
        }
    }
    fn notification_content(&mut self, n: &crate::notifications::Notice, r: Rect, dark: bool) {
        let tones = self
            .m3_cache
            .map(|(_, _, c)| c)
            .unwrap_or_else(|| dotisle_domain::dynamic_color::scheme(0x496b55, dark));
        // 统一升级图标容器：居中 42x42 圆角玻璃徽章 + 柔光描边，视觉更稳重立体
        let icon = Rect::new(r.x + 18., r.y + (r.h - 42.) / 2., 42., 42.);
        self.canvas.draw_blurred_rounded_rect(
            Affine::translate((0., 2.)),
            vr(icon),
            color(0x28000000),
            12.,
            4.5,
        );
        if let Some(kind) = n.app_id.strip_prefix("LyricIsland.Device.") {
            let tones = self
                .m3_cache
                .map(|(_, _, c)| c)
                .unwrap_or_else(|| dotisle_domain::monet::scheme(0x496b55, dark));
            self.fill(icon, 13., 0xff000000 | tones.secondary_container);
            self.device_glyph(
                Rect::new(icon.x + 8., icon.y + 8., 26., 26.),
                kind,
                0xff000000 | tones.on_secondary_container,
            );
        } else if let Some(im) = self.notification_icon.clone() {
            self.image(&im, icon, 11., 1.);
        } else {
            self.fill(icon, 11., 0xff3188ee);
            self.text_at(
                &n.app_name.chars().next().unwrap_or('•').to_string(),
                Rect::new(icon.x, icon.y + 3., 42., 34.),
                22.,
                600,
                0xffffffff,
                0.5,
            );
        }
        self.outline(icon, 11., 0x2AFFFFFF, 0.75);
        // 计算正文实际换行，动态按文本组总高垂直居中（修复通知内容视觉偏上）
        let text_x = r.x + 74.;
        let text_w = (r.w - 102.).max(60.);
        let lines = if n.body.trim().is_empty() {
            Vec::new()
        } else {
            self.wrap(&n.body, 12.5, text_w)
        };
        let shown_lines = lines.len().min(2);
        let text_h = match shown_lines {
            0 => 38.,
            1 => 59.,
            _ => 76.,
        };
        let top = r.y + ((r.h - text_h) * 0.5).max(8.);
        self.text_line(
            &n.app_name,
            Rect::new(text_x, top, (r.w - 166.).max(40.), 16.),
            11.5,
            500,
            0xb8000000 | (tones.on_surface_variant & 0xffffff),
        );
        self.text_line(
            "现在",
            Rect::new(r.x + r.w - 74., top + 1., 34., 14.),
            10.,
            400,
            0xa8000000 | (tones.on_surface_variant & 0xffffff),
        );
        let close_box = Rect::new(r.x + r.w - 34., top - 2., 20., 20.);
        self.fill(close_box, 10., 0x14FFFFFF);
        self.icon(
            Icon::Close,
            Rect::new(close_box.x + 3., close_box.y + 3., 14., 14.),
            0xd0000000 | (tones.on_surface_variant & 0xffffff),
        );
        self.text_line(
            &n.title,
            Rect::new(text_x, top + 17., text_w, 21.),
            14.5,
            600,
            0xff000000 | tones.on_surface,
        );
        for (i, line) in lines.iter().take(2).enumerate() {
            self.text_line(
                line,
                Rect::new(text_x, top + 40. + i as f64 * 16.5, text_w, 18.),
                12.5,
                400,
                0xd8000000 | (tones.on_surface_variant & 0xffffff),
            );
        }
    }
    fn wrap(&mut self, text: &str, size: f64, width: f64) -> Vec<String> {
        let mut lines = vec![];
        let mut line = String::new();
        for c in text.chars() {
            let mut next = line.clone();
            next.push(c);
            if !line.is_empty()
                && (c == '\n' || self.text.measure(&next, size as f32, false) > width)
            {
                lines.push(std::mem::take(&mut line));
            }
            if c != '\n' {
                line.push(c);
            }
        }
        if !line.is_empty() {
            lines.push(line);
        }
        lines
    }
    pub unsafe fn notification_height(
        &mut self,
        n: &crate::notifications::Notice,
        width: f64,
    ) -> Result<f64> {
        Ok(if n.body.trim().is_empty() {
            68.
        } else if self.wrap(&n.body, 12.5, (width - 102.).max(60.)).len() <= 1 {
            84.
        } else {
            98.
        })
    }
    fn osd_content(&mut self, n: &crate::notifications::Notice, r: Rect, dark: bool) {
        if n.app_id == "LyricIsland.Greeting" {
            let tones = self
                .m3_cache
                .map(|(_, _, c)| c)
                .unwrap_or_else(|| dotisle_domain::monet::scheme(0x496b55, dark));
            let accent = 0xff000000 | tones.primary;
            let badge = Rect::new(r.x + 12., r.y + (r.h - 26.) / 2., 26., 26.);
            self.fill(badge, 8., 0x18FFFFFF);
            self.outline(badge, 8., 0x28FFFFFF, 0.7);
            self.icon(
                Icon::Island,
                Rect::new(badge.x + 4., badge.y + 4., 18., 18.),
                accent,
            );
            self.text_line(
                &n.title,
                Rect::new(r.x + 46., r.y + (r.h - 19.) / 2. - 0.5, 76., 20.),
                13.,
                600,
                0xff000000 | tones.on_surface,
            );
            self.text_line(
                &n.body,
                Rect::new(r.x + 122., r.y + (r.h - 16.) / 2., (r.w - 134.).max(40.), 16.),
                10.5,
                400,
                0xff000000 | tones.on_surface_variant,
            );
            return;
        }
        let roles = self
            .dynamic_resolved
            .map(|v| v.roles)
            .unwrap_or_else(|| dotisle_domain::dynamic_color::roles(0x496b55, true, false, false));
        let icon = Rect::new(r.x + 12., r.y + (r.h - 30.) / 2., 30., 30.);
        self.fill(icon, 10., 0x1cffffff);
        self.outline(icon, 10., 0x26ffffff, 0.7);
        let kind = n.app_id.strip_prefix("LyricIsland.System.").unwrap_or("");
        let on = if kind == "Mic" {
            n.body == "已静音"
        } else {
            n.body == "已开启"
        };
        let tint = 0xff000000
            | if kind == "Mic" && on {
                roles.error
            } else if on {
                roles.primary
            } else {
                roles.on_surface_variant
            };
        self.device_glyph(
            Rect::new(icon.x + 5., icon.y + 5., 20., 20.),
            if kind == "Mic" && on { "MicOff" } else { kind },
            tint,
        );
        self.text_line(
            &n.title,
            Rect::new(r.x + 50., r.y + (r.h - 18.) / 2., 70., 18.),
            12.5,
            600,
            0xff000000 | roles.on_surface,
        );
        self.text_line(
            &n.body,
            Rect::new(r.x + 122., r.y + (r.h - 15.) / 2., (r.w - 134.).max(40.), 15.),
            10.5,
            500,
            tint,
        );
    }
    pub unsafe fn render(&mut self, s: &Scene<'_>, present: bool) -> Result<()> {
        self.paint_scene(s);
        if present {
            crate::frame_stat::tick();
        }
        self.gpu
            .draw_with_liquid_glass(
                &self.canvas,
                self.dpi as f64 / 96.,
                present,
                &self.liquid_glass_shapes,
            )
            .map_err(err)
    }
    pub fn capture_layout_frame(&mut self, scene: &Scene<'_>) -> Result<Vec<u8>> {
        self.paint_scene(scene);
        let mut scale = scene.layout_scale.min(1.5);
        let mut png = self.gpu.capture_frame(&self.canvas, scale,
            (scene.layout.host.w * scale).ceil() as u32,
            (scene.layout.host.h * scale).ceil() as u32).map_err(err)?;
        if png.len() > 750_000 {
            // Stay within the settings pipe's 1 MiB JSON limit even with noisy artwork.
            scale = scale.min((170_000. / (scene.layout.host.w * scene.layout.host.h)).sqrt());
            png = self.gpu.capture_frame(&self.canvas, scale,
                (scene.layout.host.w * scale).ceil() as u32,
                (scene.layout.host.h * scale).ceil() as u32).map_err(err)?;
        }
        Ok(png)
    }
    pub fn capture_progress_frame(&self, width: u32, bar: dotisle_render::progress::BarStyle,
        thumb: dotisle_render::progress::ThumbStyle, glow: bool, spin: bool, animate: bool,
        ink: u32, rail: u32) -> Result<Vec<u8>> {
        let mut canvas = VScene::new();
        let track = Rect::new(8., 0., f64::from(width)-16., 28.);
        let look = dotisle_render::progress::Look { fg: 0xff000000 | ink, track: 0xff000000 | rail, dot: 0x8a000000 | ink };
        // A representative playing frame. The settings page does not poll the GPU to animate it.
        paint_playback_progress(&mut canvas, track, 0.38, 0., 14., if animate {1.} else {0.},
            if spin && animate {0.28} else {0.}, look, bar, thumb, glow, false, false, false);
        self.gpu.capture_frame(&canvas, 1.5, (f64::from(width)*1.5).ceil() as u32, 42).map_err(err)
    }
    fn paint_scene(&mut self, s: &Scene<'_>) {
        self.liquid_glass_shapes.clear();
        self.active_lg_refraction = (s.liquid_glass_refraction as f32 / 100.).clamp(0., 1.);
        self.active_lg_dispersion = (s.liquid_glass_dispersion as f32 / 100.).clamp(0., 1.);
        self.active_lg_blur = (s.liquid_glass_blur as f32 / 100.).clamp(0., 1.);
        self.active_lg_tint = (s.liquid_glass_tint as f32 / 100.).clamp(0., 1.);
        self.active_lg_opacity = (s.opacity * s.background_opacity).clamp(0., 1.);
        self.active_lg_scale = self.dpi / 96.;
        if s.hidden_edge && !s.morph.active {
            self.canvas = VScene::new();
            let c = s.layout.capsule;
            let width = 56.0_f64.min(c.w.max(1.));
            let bar = Rect::new(c.x + (c.w - width) / 2., c.y, width, 5.);
            let color = 0xd8000000 | (s.tool_theme.fill & 0x00ff_ffff);
            dotisle_render::draw::fill_rounded(&mut self.canvas, vr(bar), 2.5, color);
            return;
        }
        self.text.set_family(s.font_family);
        self.prepare_dynamic(s);
        let (mut themed, compact) = cover_scenes(s, Some(self.paint_palette));
        if s.expanded_cover_background {
            if let Some(resolved) = self.dynamic_resolved {
                if s.material != Material::LightField {
                    themed.text_color = 0xff000000 | resolved.roles.on_surface;
                }
            }
        }
        if s.material == Material::LiquidGlass && self.is_liquid_glass_light() {
            themed.text_color = self.light_glass_ink(themed.text_color);
            themed.component_color = self.light_glass_ink(themed.component_color);
        }
        let s = &themed;
        if s.cover_age_ms >= 280. && self.cover_changed.elapsed() >= Duration::from_millis(280) {
            self.previous_cover = None;
        }
        self.canvas = VScene::new();
        let is_popup = s.notification.is_some() || s.osd.is_some() || s.alert.is_some();
        let morph_live = s.morph.active && !is_popup;
        let (base, base_radius) = if morph_live {
            (s.morph.rect, s.morph.radius)
        } else {
            s.layout
                .morph_shape_with_radii(s.frame.card_reveal, s.capsule_radius, s.radius)
        };
        let p = if morph_live {
            s.morph.card_mix.clamp(0., 1.)
        } else {
            s.frame.card_reveal
        };
        let (card_softness, compact_softness) = if morph_live {
            (
                if s.morph.card_mix > 0.02 { s.morph.softness } else { 0. },
                s.morph.softness,
            )
        } else {
            crate::motion::content_softness(
                p,
                s.animate && !s.drag_active && s.notification.is_none() && s.osd.is_none() && s.alert.is_none(),
            )
        };
        let (shape, radius) = if s.alert.is_some(){
            (s.alert_rect,s.alert_radius)
        }else if s.notification.is_some() {
            (s.notification_rect, s.notification_radius)
        } else if s.osd.is_some() {
            (s.osd_rect, s.osd_radius)
        } else {
            (base, base_radius)
        };
        let music_alpha = if s.transient_only {
            0.
        } else if s.alert.is_some(){
            1.-s.alert_mix
        } else if s.notification.is_some() {
            1. - s.notification_mix
        } else if s.osd.is_some() {
            1. - s.osd_mix
        } else {
            1.
        };
        let transient_alpha = if s.transient_only {
            if s.alert.is_some(){s.alert_mix}else if s.notification.is_some() {
                s.notification_mix
            } else if s.osd.is_some() {
                s.osd_mix
            } else {
                1.
            }
        } else {
            1.
        };
        let curve = (p > 0. && s.alert.is_none() && s.notification.is_none() && s.osd.is_none())
            .then(|| expanded_card_path(shape, radius, p));
        self.clip(s.layout.host, 0., s.opacity * transient_alpha as f32);
        self.clip(s.layout.host, 0., s.background_opacity);
        let is_taskbar_widget = s.layout.anchor_bottom && p < 0.01 && !is_popup;
        let shadow_mul = if is_taskbar_widget {
            0.
        } else if s.layout.anchor_bottom {
            (p * 1.4).clamp(0., 1.)
        } else if morph_live {
            s.morph.shadow_mul
        } else {
            1.
        };
        let base_shadow_a = if morph_live {
            0x38
        } else if s.mini && p < 0.01 {
            0x0a
        } else {
            0x38
        };
        let shadow = color(((base_shadow_a as f64 * shadow_mul).round() as u32) << 24);
        let mut surface = Self::music_surface_spec(s);
        if is_popup {
            // 弹窗与系统提示/提醒采用独立设置材质（默认亚克力）
            surface.material = s.popup_material;
            surface.compact = s.popup_material;
        }
        let using_pure_lg = surface.paint(self.paint_palette).liquid_glass > 0.99;
        if shadow_mul > 0.01 && !using_pure_lg {
            // 将外投影晕散半径收束至 24dp 宿主安全边距以内，彻底消除左右与底部矩形裁切黑线并降低离屏开销
            self.canvas.draw_blurred_rounded_rect(
                Affine::translate((0., 3.5)),
                vr(shape),
                shadow,
                radius,
                13. / 3.,
            );
        }
        let allow_glow = if morph_live {
            s.morph.strip_mix < 0.45 && s.morph.mini_mix < 0.45
        } else {
            !s.mini
        };
        if allow_glow && !is_taskbar_widget && !is_popup {
            let accent = self.m3(s).primary;
            crate::island_glow::paint(&mut self.canvas, vr(shape), radius, accent, s.glow);
        }
        let saved_palette = self.paint_palette;
        if s.panel.is_some() || is_popup {
            surface.cover_overlay = false;
            surface.card_opacity = 0.;
            surface.cover_background = true;
            if let Some(r) = self.dynamic_resolved {
                self.paint_palette.background = r.roles.surface_container;
            }
        }
        if s.alert.is_some() {
            surface.mini = false;
        }
        if is_taskbar_widget {
            self.taskbar_widget_surface(shape, radius, s);
        } else if morph_live && s.morph.strip_mix > 0.002 {
            // 灵动岛与蓝条之间的液态材质交叉融合：避免出现硬切闪烁，同时按当前 mini_mix 匹配外壳材质
            let island_alpha = (1. - s.morph.strip_mix).clamp(0., 1.) as f32;
            if island_alpha > 0.004 {
                let mut island_surface = surface;
                let is_mini_shell = s.morph.mini_mix > 0.5;
                island_surface.mini = is_mini_shell;
                island_surface.compact = if is_mini_shell {
                    s.mini_capsule
                } else {
                    s.classic_capsule
                };
                self.clip(shape, radius, island_alpha);
                self.material_surface_impl(
                    shape,
                    radius,
                    island_surface,
                    curve.as_ref(),
                    LightFieldCtx {
                        time_ms: s.time_ms,
                        reveal: s.frame.card_reveal,
                        cover_age_ms: s.cover_age_ms,
                        animate: s.animate,
                    },
                );
                self.canvas.pop_layer();
            }
            let strip_alpha = s.morph.strip_mix.clamp(0., 1.) as f32;
            if strip_alpha > 0.004 {
                self.clip(shape, radius, strip_alpha);
                let bar_color = 0xd8000000 | (s.tool_theme.fill & 0x00ff_ffff);
                dotisle_render::draw::fill_rounded(&mut self.canvas, vr(shape), radius, bar_color);
                if s.morph.strip_mix < 0.96 {
                    dotisle_render::draw::outline_rounded(
                        &mut self.canvas,
                        vr(shape),
                        radius,
                        0x38ffffff,
                        0.75,
                    );
                }
                self.canvas.pop_layer();
            }
        } else if morph_live && s.morph.mini_mix > 0.002 && s.morph.mini_mix < 0.998 {
            // 经典灵动岛与 Mini 岛之间的外壳材质平滑交叉融合，避免透明度和材质突变
            let classic_alpha = (1. - s.morph.mini_mix).clamp(0., 1.) as f32;
            let mini_alpha = s.morph.mini_mix.clamp(0., 1.) as f32;
            if classic_alpha > 0.004 {
                let mut classic_surface = surface;
                classic_surface.mini = false;
                classic_surface.compact = s.classic_capsule;
                self.clip(shape, radius, classic_alpha);
                self.material_surface_impl(
                    shape,
                    radius,
                    classic_surface,
                    curve.as_ref(),
                    LightFieldCtx {
                        time_ms: s.time_ms,
                        reveal: s.frame.card_reveal,
                        cover_age_ms: s.cover_age_ms,
                        animate: s.animate,
                    },
                );
                self.canvas.pop_layer();
            }
            if mini_alpha > 0.004 {
                let mut mini_surface = surface;
                mini_surface.mini = true;
                mini_surface.compact = s.mini_capsule;
                self.clip(shape, radius, mini_alpha);
                self.material_surface_impl(
                    shape,
                    radius,
                    mini_surface,
                    curve.as_ref(),
                    LightFieldCtx {
                        time_ms: s.time_ms,
                        reveal: s.frame.card_reveal,
                        cover_age_ms: s.cover_age_ms,
                        animate: s.animate,
                    },
                );
                self.canvas.pop_layer();
            }
        } else {
            self.material_surface_impl(
                shape,
                radius,
                surface,
                curve.as_ref(),
                LightFieldCtx {
                    time_ms: s.time_ms,
                    reveal: s.frame.card_reveal,
                    cover_age_ms: s.cover_age_ms,
                    animate: s.animate,
                },
            );
        }
        self.paint_palette = saved_palette;
        self.canvas.pop_layer();
        let alpha = if morph_live {
            s.morph.card_alpha as f64 * music_alpha
        } else {
            crate::motion::content_weights(s.frame).0 * music_alpha
        };
        if alpha > 0.001 {
            let part = self.collect(|a| {
                let w = s.layout.card.w;
                if s.drag_active {
                    let mut neighbor = *s;
                    neighbor.panel = s.drag_panel;
                    let layer = a.collect(|a| a.expanded_view(&neighbor));
                    a.canvas.append(
                        &layer,
                        Some(Affine::translate((
                            s.swipe_offset - s.swipe_offset.signum() * w,
                            s.frame.content_offset_y,
                        ))),
                    );
                }
                if s.switching && s.switch_outgoing {
                    let mut old = *s;
                    old.panel = s.previous_panel;
                    let layer = a.collect(|a| a.expanded_view(&old));
                    a.canvas.append(
                        &layer,
                        Some(Affine::translate((
                            s.switch_offset - s.switch_direction * w,
                            s.frame.content_offset_y,
                        ))),
                    );
                }
                let layer = a.collect(|a| a.expanded_view(s));
                a.canvas.append(
                    &layer,
                    Some(Affine::translate((
                        s.switch_offset + s.swipe_offset,
                        s.frame.content_offset_y,
                    ))),
                );
            });
            let scale = (base.w / s.layout.card.w).min(base.h / s.layout.card.h);
            let tx = base.x + (base.w - s.layout.card.w * scale) / 2.;
            let ty = base.y + (base.h - s.layout.card.h * scale) / 2.;
            if let Some(path) = &curve {
                self.canvas.push_layer(Mix::Normal, alpha as f32, Affine::IDENTITY, path);
            } else {
                self.clip(shape, radius, alpha as f32);
            }
            if card_softness > 0. {
                self.canvas.push_gaussian_layer(
                    Affine::IDENTITY,
                    &vr(shape).inflate(5., 5.),
                    card_softness,
                );
            }
            self.canvas.append(
                &part,
                Some(Affine::translate((tx, ty)) * Affine::scale(scale)),
            );
            if card_softness > 0. {
                self.canvas.pop_layer();
            }
            self.canvas.pop_layer();
        }
        let alpha = if morph_live {
            music_alpha
        } else {
            crate::motion::content_weights(s.frame).1 * music_alpha
        };
        if alpha > 0.001 {
            if morph_live {
                // 双轨交叉淡化：经典胶囊与 Mini 胶囊在各自的稳定排版框内渲染，
                // 再由实时形变轮廓居中裁剪并辅以微柔焦，文字绝不被挤扁或跳行。
                let paint_track = |this: &mut Self, mut track_scene: Scene<'_>, is_mini: bool, track_alpha: f32| {
                    if track_alpha <= 0.004 {
                        return;
                    }
                    track_scene.mini = is_mini;
                    let target_w = s.layout.capsule.w.max(shape.w);
                    let target_h = if is_mini { 14. } else { s.layout.capsule.h.max(40.) };
                    let ref_box = Rect::new(
                        shape.x + (shape.w - target_w) / 2.,
                        shape.y + (shape.h - target_h) / 2.,
                        target_w,
                        target_h,
                    );
                    track_scene.layout.capsule = ref_box;
                    track_scene.capsule_radius = target_h / 2.;
                    let part = this.collect(|a| a.compact_view(&track_scene));
                    this.clip(shape, radius, track_alpha);
                    if compact_softness > 0. {
                        this.canvas.push_gaussian_layer(
                            Affine::IDENTITY,
                            &vr(shape).inflate(5., 5.),
                            compact_softness,
                        );
                    }
                    this.canvas.append(&part, None);
                    if compact_softness > 0. {
                        this.canvas.pop_layer();
                    }
                    this.canvas.pop_layer();
                };
                paint_track(self, compact, false, (alpha as f32) * s.morph.classic_alpha);
                paint_track(self, compact, true, (alpha as f32) * s.morph.mini_alpha);
            } else {
                let part = self.collect(|a| {
                    let w = s.layout.capsule.w;
                    if s.drag_active {
                        let mut neighbor = compact;
                        neighbor.panel = s.drag_panel;
                        let layer = a.collect(|a| a.compact_view(&neighbor));
                        a.canvas.append(
                            &layer,
                            Some(Affine::translate((
                                s.swipe_offset - s.swipe_offset.signum() * w,
                                0.,
                            ))),
                        );
                    }
                    if s.switching && s.switch_outgoing {
                        let mut old = compact;
                        old.panel = s.previous_panel;
                        let layer = a.collect(|a| a.compact_view(&old));
                        a.canvas.append(
                            &layer,
                            Some(Affine::translate((
                                s.switch_offset - s.switch_direction * w + s.swipe_offset,
                                0.,
                            ))),
                        );
                    }
                    let layer = a.collect(|a| a.compact_view(&compact));
                    a.canvas.append(
                        &layer,
                        Some(Affine::translate((s.switch_offset + s.swipe_offset, 0.))),
                    );
                });
                if let Some(path) = &curve {
                    self.canvas.push_layer(Mix::Normal, alpha as f32, Affine::IDENTITY, path);
                } else {
                    self.clip(shape, radius, alpha as f32);
                }
                if compact_softness > 0. {
                    self.canvas.push_gaussian_layer(
                        Affine::IDENTITY,
                        &vr(shape).inflate(5., 5.),
                        compact_softness,
                    );
                }
                self.canvas.append(
                    &part,
                    Some(Affine::translate((
                        base.x - s.layout.capsule.x,
                        base.y - s.layout.capsule.y,
                    ))),
                );
                if compact_softness > 0. {
                    self.canvas.pop_layer();
                }
                self.canvas.pop_layer();
            }
        }
        let sat_vis = if morph_live {
            ((1. - s.morph.strip_mix) * (1. - s.morph.mini_mix) * (1. - s.morph.card_mix))
                .clamp(0., 1.) as f32
        } else if s.mini {
            0.
        } else {
            1.
        };
        if sat_vis > 0.01 && s.component_count > 1 && s.notification.is_none() && s.osd.is_none() && s.alert.is_none() {
            self.clip(s.layout.host, 0., s.background_opacity * sat_vis);
            self.satellite(&compact);
            self.canvas.pop_layer();
        }
        if let Some(n) = s.notification {
            self.clip(
                shape,
                radius,
                if s.transient_only {
                    1.
                } else {
                    s.notification_mix as f32
                },
            );
            self.notification_content(n, shape, s.expressive_dark);
            self.canvas.pop_layer();
        }
        if let Some(a)=s.alert{
            if a.code.is_some(){
                self.clip(shape,radius,crate::verification_code::content_alpha(shape.h,s.alert_mix));
                self.verification_content(a,crate::verification_code::layout(shape,s.layout.host.w));
            }else{
                self.clip(shape,radius,if s.transient_only{1.}else{s.alert_mix as f32});
                self.capsule_alert_content(a,shape);
            }
            self.canvas.pop_layer();
        }
        if let Some(n) = s.osd {
            self.clip(
                shape,
                radius,
                if s.transient_only {
                    1.
                } else {
                    s.osd_mix as f32
                },
            );
            self.osd_content(n, shape, s.expressive_dark);
            self.canvas.pop_layer();
        }
        self.canvas.pop_layer();
        self.lyric_halo.end_frame();
    }
    pub unsafe fn render_bottom_osd(
        &mut self,
        n: &crate::notifications::Notice,
        opacity: f64,
        offset: f64,
        surface: crate::surface::Surface,
        present: bool,
    ) -> Result<()> {
        self.canvas = VScene::new();
        let r = Rect::new(16., 12. + offset, 240., 44.);
        self.clip(Rect::new(0., 0., 272., 80.), 0., opacity as f32);
        self.material_surface(r, 22., surface);
        self.osd_content(n, r, true);
        self.canvas.pop_layer();
        let result = self
            .gpu
            .draw(&self.canvas, self.dpi as f64 / 96., present)
            .map_err(err);

        result
    }
    /// 任务栏模式下独立的屏幕正上方弹窗渲染入口（共用 PopupMaterial 与居中排版）
    pub unsafe fn render_top_popup(
        &mut self,
        notice: Option<&crate::notifications::Notice>,
        alert: Option<&crate::capsule_alert::Alert>,
        osd: Option<&crate::notifications::Notice>,
        rect: Rect,
        radius: f64,
        opacity: f64,
        surface: crate::surface::Surface,
        present: bool,
    ) -> Result<()> {
        self.canvas = VScene::new();
        self.clip(Rect::new(0., 0., 488., 146.), 0., opacity as f32);
        self.canvas.draw_blurred_rounded_rect(
            Affine::translate((0., 5.)),
            vr(rect),
            color(0x38000000),
            radius,
            22. / 3.,
        );
        self.material_surface(rect, radius, surface);
        if let Some(n) = notice {
            self.clip(rect, radius, 1.);
            self.notification_content(n, rect, true);
            self.canvas.pop_layer();
        } else if let Some(a) = alert {
            self.clip(rect, radius, 1.);
            if a.code.is_some() {
                self.verification_content(a, crate::verification_code::layout(rect, 488.));
            } else {
                self.capsule_alert_content(a, rect);
            }
            self.canvas.pop_layer();
        } else if let Some(n) = osd {
            self.clip(rect, radius, 1.);
            self.osd_content(n, rect, true);
            self.canvas.pop_layer();
        }
        self.canvas.pop_layer();
        self.gpu
            .draw(&self.canvas, self.dpi as f64 / 96., present)
            .map_err(err)
    }
    pub fn resize(&mut self, width: u32, height: u32, dpi: f32) {
        self.gpu.resize(width, height);
        self.width = width;
        self.height = height;
        self.dpi = dpi;
        self.active_lg_scale = dpi / 96.;
    }
    pub(crate) fn preview_glass_rect(&self) -> Option<[f32; 4]> {
        self.liquid_glass_shapes.first().map(|s| s.rect)
    }
    pub fn wait_idle(&self) {
        self.gpu.wait_idle();
    }
    pub fn colors_moving(&self, now: f64) -> bool {
        self.color_motion.moving(now)
    }
    pub unsafe fn snapshot(
        &self,
        path: &Path,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.gpu.snapshot(path)
    }
    /// The takeover window cannot rebuild Material roles, so the app hands it the window-sized
    /// material plus the roles, geometry and font that painted the island card.
    pub unsafe fn capture_update_appearance(
        &mut self,
        s: &Scene<'_>,
        directory: &Path,
        window: (i32, i32, i32, i32),
        job: &str,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        self.text.set_family(s.font_family);
        self.prepare_dynamic(s);
        self.canvas = VScene::new();
        let saved_palette = self.paint_palette;
        self.canvas.draw_blurred_rounded_rect(
            Affine::translate((0., 5.)),
            vr(s.layout.card),
            color(0x38000000),
            s.radius,
            22. / 3.,
        );
        let mut surface = Self::surface_spec(s);
        let immersion = lyric_core::music_layout::immersion_amount(
            s.music_layout,
            s.music_mix,
            s.custom_geometry.is_some(),
        );
        if s.panel.is_none()
            && s.notification.is_none()
            && s.osd.is_none() && s.alert.is_none()
            && immersion > 0.
            && surface.material != Material::LiquidGlass
            && !s.dynamic_high_contrast
        {
            surface.material = Material::Acrylic;
            surface.cover_background = s.expanded_cover_background;
            surface.cover_overlay = s.cover_overlay;
            surface.card_frost = surface.card_frost + (64. - surface.card_frost) * immersion;
            surface.card_opacity = surface.card_opacity + (62. - surface.card_opacity) * immersion;
        }
        surface.cover_overlay = false;
        surface.card_opacity = 0.;
        surface.cover_background = true;
        if let Some(r) = self.dynamic_resolved {
            self.paint_palette.background = r.roles.surface_container;
        }
        self.material_surface(s.layout.card, s.radius, surface);
        self.paint_palette = saved_palette;
        self.gpu.draw(&self.canvas, self.dpi as f64 / 96., false)?;
        std::fs::create_dir_all(directory)?;
        self.gpu.snapshot(&directory.join("background.png"))?;
        let c = self.m3(s);
        let hex = |value: u32| format!("#{:06x}", value & 0xffffff);
        let exported = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let appearance = serde_json::json!({"version":1,"jobId":job,"exportedAtMs":exported,
            "scale":self.dpi as f64/96.,"window":{"x":window.0,"y":window.1,"width":window.2,"height":window.3},
            "card":{"x":s.layout.card.x,"y":s.layout.card.y,"width":s.layout.card.w,"height":s.layout.card.h},
            "radius":s.radius,"fontFamily":s.font_family,"animate":true,"timeMs":0.,
            "roles":{"primary":hex(c.primary),"onPrimary":hex(c.on_primary),"onSurface":hex(c.on_surface),"onSurfaceVariant":hex(c.on_surface_variant),"outline":hex(c.outline)}});
        std::fs::write(
            directory.join("appearance.json"),
            serde_json::to_vec_pretty(&appearance)?,
        )?;
        Ok(())
    }
    pub unsafe fn verify_font(&mut self) -> Result<serde_json::Value> {
        let samples = [
            "晚风轻轻，音乐刚好 DotIsle 0123456789 译",
            "景色は移ろい瞬く間に変わる",
            "ひらがな カタカナ 日本語 한국어",
        ];
        Ok(
            serde_json::json!({"renderer":"Vello + Parley","family":"Noto Sans SC","lineSpacing":self.body_line_spacing,"baseline":self.body_baseline,"samples":samples.map(|s|serde_json::json!({"text":s,"width":self.text.measure(s,16.,false)}))}),
        )
    }
    pub unsafe fn icon_bounds(&self) -> Result<serde_json::Value> {
        Ok(serde_json::json!({"renderer":"Vello paths","transportSize":24,"secondarySize":16}))
    }
}
fn time(s: f64) -> String {
    let s = s.max(0.) as u64;
    format!("{:02}:{:02}", s / 60, s % 60)
}
#[derive(Clone, Copy)]
struct LightFieldCtx {
    time_ms: f64,
    reveal: f64,
    cover_age_ms: f64,
    animate: bool,
}
impl Default for LightFieldCtx {
    fn default() -> Self {
        Self { time_ms: 0., reveal: 1., cover_age_ms: 1000., animate: false }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Acrylic,
    LightField,
    LiquidGlass,
}
impl Material {
    /// Both capsule modes open the same card and use the same material preference.
    pub fn expanded(settings: &serde_json::Value, _mini: bool) -> Self {
        Self::from_setting(settings["IslandMaterial"].as_str().unwrap_or("LightField"))
    }
    pub fn from_setting(value: &str) -> Self {
        match value {
            "LiquidGlass" => Self::LiquidGlass,
            "Glass" | "LightField" => Self::LightField,
            _ => Self::Acrylic,
        }
    }
}
#[derive(Clone, PartialEq)]
pub struct MaterialTheme {
    pub roles: Option<dotisle_domain::dynamic_color::Roles>,
    pub material: Material,
    pub cover_background: bool,
    pub palette: crate::cover_color::CoverPalette,
    pub cover: Option<ImageData>,
}
impl MaterialTheme {
    pub fn new(material: Material, cover_background: bool) -> Self {
        Self {
            roles: None,
            material,
            cover_background,
            palette: Default::default(),
            cover: None,
        }
    }
}
#[derive(Clone, Copy)]
pub struct Scene<'a> {
    pub agent: &'a crate::agent_island::View,
    pub background_opacity: f32,
    pub alert:Option<&'a crate::capsule_alert::Alert>,
    pub alert_rect:Rect,pub alert_radius:f64,pub alert_mix:f64,
    pub glow: crate::island_glow::Frame,
    pub transient_only: bool,
    pub custom_geometry: Option<lyric_core::music_layout::MusicGeometry>,
    pub play_shape: f64,
    pub capsule_radius: f64,
    pub update_phase: &'a str,
    pub feedback: [f32; 64],
    pub dynamic_source: &'a str,
    pub dynamic_system_seed: Option<u32>,
    pub dynamic_custom_seed: u32,
    pub dynamic_high_contrast: bool,
    pub font_family: &'a str,
    pub timer_mono: bool,
    pub hidden_edge: bool,
    pub morph: crate::morph_transition::MorphPresentation,
    pub expressive_dark: bool,
    pub mini: bool,
    pub mini_light_field: bool,
    pub classic_capsule: Material,
    pub mini_capsule: Material,
    pub mini_ink: u32,
    pub mini_outline: u32,
    /// Hides the expanded card's 译 button in painting and in hit testing together.
    pub translation_button: bool,
    pub translation_enabled: bool,
    pub translation_tooltip: bool,
    pub volume_level: Option<u8>,
    pub volume_muted: bool,
    pub tool_theme: crate::theme_accent::Accent,
    pub drag_panel: Option<&'a crate::activities::Panel>,
    pub drag_active: bool,
    pub switch_outgoing: bool,
    pub switch_direction: f64,
    pub panel: Option<&'a crate::activities::Panel>,
    pub previous_panel: Option<&'a crate::activities::Panel>,
    pub resident: &'a crate::resident::View,
    pub component_count: usize,
    pub auxiliary_update_progress: f64,
    pub auxiliary_update_completed: bool,
    pub auxiliary_kind: u8,
    pub auxiliary_timer: Option<(f64, f64)>,
    pub auxiliary_running: bool,
    pub auxiliary_elapsed: f64,
    pub switching: bool,
    pub switch_offset: f64,
    pub swipe_offset: f64,
    pub todo_scroll: f64,
    pub material: Material,
    pub popup_material: Material,
    pub lyric_cover_color: bool,
    pub cover_background: bool,
    pub expanded_cover_background: bool,
    pub cover_overlay: bool,
    pub demo: bool,
    pub animate: bool,
    pub acrylic_card_frost: f64,
    pub acrylic_card_opacity: f64,
    pub acrylic_capsule_frost: f64,
    pub acrylic_capsule_opacity: f64,
    pub liquid_glass_refraction: f64,
    pub liquid_glass_dispersion: f64,
    pub liquid_glass_blur: f64,
    pub liquid_glass_tint: f64,
    pub notification: Option<&'a crate::notifications::Notice>,
    pub notification_rect: Rect,
    pub notification_radius: f64,
    pub notification_mix: f64,
    pub osd: Option<&'a crate::notifications::Notice>,
    pub osd_rect: Rect,
    pub osd_radius: f64,
    pub osd_mix: f64,
    pub layout: Layout,
    pub frame: Frame,
    pub radius: f64,
    pub resizing: bool,
    pub time_ms: f64,
    pub previous_context: &'a str,
    pub next_context: &'a str,
    pub controls_opacity: f64,
    pub lyric: &'a str,
    pub previous_lyric: &'a str,
    pub compact_secondary: &'a str,
    pub previous_compact_secondary: &'a str,
    pub lyric_age_ms: f64,
    pub secondary: &'a str,
    pub title: &'a str,
    pub artist: &'a str,
    pub playing: bool,
    pub highlight: f64,
    /// The player reports a live timeline but cannot be seeked; the card offers an enable flow.
    pub seek_offer: bool,
    /// The offer is expanded into a clear restart/cancel decision.
    pub seek_confirm: bool,
    pub progress: f64,
    pub elapsed: f64,
    pub duration: f64,
    pub font_size: f64,
    pub music_layout: crate::layout::MusicLayout,
    pub music_mix: f64,
    pub text_color: u32,
    pub component_color: u32,
    pub progress_color: u32,
    /// Seekable playback bars share these with the control centre; passive bars ignore them.
    pub progress_bar: dotisle_render::progress::BarStyle,
    pub progress_thumb: dotisle_render::progress::ThumbStyle,
    pub progress_thumb_spin: bool,
    pub progress_glow: bool,
    pub lyric_glow: bool,
    pub transport_theme: dotisle_render::transport::TransportTheme,
    pub hover: i32,
    pub pressed: i32,
    pub opacity: f32,
    pub transparent: bool,
    pub floating: bool,
    pub practice_move: bool,
    pub favorite_enabled: bool,
    pub has_track: bool,
    pub layout_scale: f64,
    pub activity_scales: [f64; 3],
    pub activity_opacity: f64,
    pub cover_opacity: f64,
    pub cover_age_ms: f64,
    pub outgoing_opacity: f64,
    pub outgoing_y: f64,
    pub taskbar_transparent: bool,
    pub taskbar_light: bool,
    pub taskbar_hovered: bool,
}

/// Lyric highlighting follows its own preference, independently of surface tint.
pub(crate) fn cover_scenes<'a>(
    s: &Scene<'a>,
    palette: Option<crate::cover_color::CoverPalette>,
) -> (Scene<'a>, Scene<'a>) {
    let mut expanded = *s;
    let mut compact = *s;
    // The lyric effect belongs only to the expanded music card.
    compact.lyric_glow = false;
    if let Some(palette) = palette {
        let accent = 0xff000000 | palette.accent;
        // 尊重用户在设置中的动态色彩开关，曜光玻璃通过表面基底保持纯净，同时允许专辑色点缀控件强调与歌词高亮
        if s.expanded_cover_background {
            expanded.component_color = accent;
        }
        if s.cover_background {
            compact.component_color = accent;
        }
        if s.lyric_cover_color {
            expanded.progress_color = accent;
            compact.progress_color = accent;
        }
    }
    (expanded, compact)
}

include!("expressive.rs");

include!("dynamic_painter.rs");

fn paint_playback_progress(canvas: &mut VScene, track: Rect, fraction: f64, phase: f64, thumb_size: f64,
    wave: f64, spin: f64, look: dotisle_render::progress::Look, bar: dotisle_render::progress::BarStyle,
    thumb: dotisle_render::progress::ThumbStyle, glow: bool, light_field: bool, progress_active: bool, scrub_pressed: bool) {
    let fraction = fraction.clamp(0., 1.);
    let played = track.w * fraction;
    // 默认风格 (Normal / Thin 且未启用自定义滑块，或标准线条状态) 下启用追光彗星与凝光圆镜；
    // 若用户选择了波浪 (Wavy)、分段、圆点、自定义形状滑块或旋转动画，完整保留原版动效与样式
    let use_lumen_comet = light_field
        && matches!(bar, dotisle_render::progress::BarStyle::Normal | dotisle_render::progress::BarStyle::Thin)
        && thumb == dotisle_render::progress::ThumbStyle::None;
    if use_lumen_comet {
        // 曜光视效（Lumen Glass）进度条：
        // 1) 常态播放：「追光 (Chasing Light)」锥形彗星光束 + 头部弥散光晕（若开启专辑/自定义强调色则柔和融光）
        // 2) 悬停/滑动：「曜光玻璃圆镜 (Condensing Lens)」饱满发光条穿过通透白边圆镜
        let ky = track.y + track.h / 2.;
        let kx = track.x + played;
        let accent_rgb = look.fg & 0xffffff;
        let has_tint = accent_rgb != 0xFFFFFF && accent_rgb != 0xDDF6FF;
        let mix_tint = |base: u32, w: f64| -> u32 {
            if has_tint { crate::cover_color::mix(base, accent_rgb, w) } else { base }
        };
        let glow_tint = mix_tint(0xB8F1FA, 0.72);
        let tail_tint = mix_tint(0xA4DCFF, 0.78);
        let mid_tint = mix_tint(0xD8F2FF, 0.56);
        let rim_tint = mix_tint(0xB8ECFF, 0.68);
        // 极克制暗透底轨（无右端多余点）
        let rail_h = if progress_active || scrub_pressed { 4.2 } else { 3.6 };
        let rail_rect = RoundedRect::from_rect(
            vr(Rect::new(track.x, ky - rail_h / 2., track.w, rail_h)),
            rail_h / 2.,
        );
        canvas.fill(Fill::NonZero, Affine::IDENTITY, color(0x28FFFFFF), None, &rail_rect);

        if progress_active || scrub_pressed {
            // 滑动/悬停态：饱满发光条 + 通透白边液态玻璃圆镜
            if played > 1. {
                canvas.draw_blurred_rounded_rect(
                    Affine::IDENTITY,
                    vr(Rect::new(track.x, ky - 3.6, played, 7.2)),
                    color(0x52000000 | glow_tint),
                    3.6,
                    6.0,
                );
                let grad = Gradient::new_linear((track.x, ky), (kx, ky)).with_stops([
                    (0.00, color(0xEB000000 | tail_tint)),
                    (0.75, color(0xF8000000 | mid_tint)),
                    (1.00, color(0xFFFFFFFF)),
                ]);
                let bar = RoundedRect::from_rect(
                    vr(Rect::new(track.x, ky - 2.5, played, 5.0)),
                    2.5,
                );
                canvas.fill(Fill::NonZero, Affine::IDENTITY, &grad, None, &bar);
            }
            let lens_r = if scrub_pressed { 10.5 } else { 8.5 };
            // 圆镜外围柔光漫射
            let halo = Gradient::new_radial((kx, ky), (lens_r * 1.95) as f32).with_stops([
                (0.0, color(0x3CDDF6FF)),
                (0.55, color(0x18000000 | rim_tint)),
                (1.0, color(0x00000000 | rim_tint)),
            ]);
            canvas.fill(Fill::NonZero, Affine::IDENTITY, &halo, None, &Circle::new((kx, ky), lens_r * 1.95));
            // 半透明冷雾玻璃圆镜本体（可清晰透出底下穿过的发光进度条）
            let lens_body = Gradient::new_radial((kx, ky - lens_r * 0.25), (lens_r * 1.1) as f32).with_stops([
                (0.0, color(0x2EFFFFFF)),
                (0.78, color(0x3C000000 | mid_tint)),
                (1.0, color(0x4CFFFFFF)),
            ]);
            canvas.fill(Fill::NonZero, Affine::IDENTITY, &lens_body, None, &Circle::new((kx, ky), lens_r));
            // 清晰锐利 1.25px 纯白玻璃折射环
            let lens_rim = Gradient::new_linear((kx, ky - lens_r), (kx, ky + lens_r)).with_stops([
                (0.00, color(0xECFFFFFF)),
                (0.50, color(0x8AFFFFFF)),
                (1.00, color(0xDCE2F2FF)),
            ]);
            canvas.stroke(&Stroke::new(1.25), Affine::IDENTITY, &lens_rim, None, &Circle::new((kx, ky), lens_r - 0.6));
        } else if played > 1.5 {
            // 常态播放：「追光」锥形彗星流光束 + 头部弥散光晕
            let breath = 0.88 + 0.12 * (phase * 1.8).sin();
            let glow_w = played.min(34.) * breath;
            canvas.draw_blurred_rounded_rect(
                Affine::IDENTITY,
                vr(Rect::new((kx - glow_w * 0.72).max(track.x), ky - 3.6, glow_w, 7.2)),
                color(0x7C000000 | glow_tint),
                3.6,
                6.8,
            );
            // 构造左细右聚的锥形彗尾 BezPath
            let tail_r = 0.9;
            let head_r = 2.35;
            let x0 = track.x;
            let x1 = (kx - head_r * 0.4).max(x0 + 1.);
            let mut comet = BezPath::new();
            comet.move_to((x0, ky - tail_r));
            comet.line_to((x1, ky - head_r));
            comet.quad_to((kx + head_r * 0.85, ky), (x1, ky + head_r));
            comet.line_to((x0, ky + tail_r));
            comet.close_path();
            let comet_grad = Gradient::new_linear((x0, ky), (kx + 1.5, ky)).with_stops([
                (0.00, color(0x4C000000 | tail_tint)),
                (0.45, color(0xC2000000 | mid_tint)),
                (0.84, color(0xF5EBF9FF)),
                (1.00, color(0xFFFFFFFF)),
            ]);
            canvas.fill(Fill::NonZero, Affine::IDENTITY, &comet_grad, None, &comet);
            // 尖端炽白核心梭光
            let core_w = played.min(14.);
            let core = RoundedRect::from_rect(
                vr(Rect::new(kx - core_w, ky - 1.35, core_w + 0.8, 2.7)),
                1.35,
            );
            let core_grad = Gradient::new_linear((kx - core_w, ky), (kx + 0.8, ky)).with_stops([
                (0.0, color(0x00FFFFFF)),
                (0.55, color(0xDCFFFFFF)),
                (1.0, color(0xFFFFFFFF)),
            ]);
            canvas.fill(Fill::NonZero, Affine::IDENTITY, &core_grad, None, &core);
        }
        return;
    }
    if glow && played > 0.5 {
        let y = track.y + (track.h - 5.6) / 2.;
        canvas.draw_blurred_rounded_rect(Affine::IDENTITY, vr(Rect::new(track.x, y, played, 5.6)),
            color(0x66000000 | (look.fg & 0xffffff)), 2.8, 5.);
    }
    dotisle_render::progress::draw(canvas, vr(track), fraction, phase, 0.7, thumb_size, wave, spin, look, bar, thumb);
    if light_field && (progress_active || scrub_pressed) {
        let ky = track.y + track.h / 2.;
        let kx = track.x + played;
        let hr = if scrub_pressed { 14. } else { 11. };
        let halo = Gradient::new_radial((kx, ky), hr as f32).with_stops([
            (0.0, color(0x46FFFFFF)),
            (0.6, color(0x1C000000 | (look.fg & 0xffffff))),
            (1.0, color(0x00000000)),
        ]);
        canvas.fill(Fill::NonZero, Affine::IDENTITY, &halo, None, &Circle::new((kx, ky), hr));
    }
}

// Start with the circular quarter-arc's two cubic halves and ease into the G2
// contour. Matching endpoints/tangents keeps the opening morph interruption-free.
fn expanded_card_path(r: Rect, radius: f64, reveal: f64) -> BezPath {
    let t = reveal.clamp(0., 1.);
    let t = t*t*(3.-2.*t);
    let lerp = |a: f64, b: f64| a+(b-a)*t;
    let k = 4./3. * (std::f64::consts::PI / 16.).tan();
    let diagonal = std::f64::consts::FRAC_1_SQRT_2;
    let mid = lerp(1.-diagonal, 0.20);
    let outer = lerp(1.-k, 0.62);
    let inner_x = lerp(1.-diagonal+k*diagonal, 0.40);
    let inner_y = lerp(1.-diagonal-k*diagonal, 0.);
    let a = (radius * lerp(1., 1.22)).max(0.).min(r.w/2.).min(r.h/2.);
    let right = r.x+r.w;
    let bottom = r.y+r.h;
    let corner = |p: &mut BezPath, cx, cy, xx: f64, xy: f64, yx: f64, yy: f64| {
        let point = |x, y| (cx+(xx*x+yx*y)*a, cy+(xy*x+yy*y)*a);
        p.curve_to(point(outer,0.), point(inner_x,inner_y), point(mid,mid));
        p.curve_to(point(inner_y,inner_x), point(0.,outer), point(0.,1.));
    };
    let mut path = BezPath::new();
    path.move_to((right-a,r.y));
    corner(&mut path,right,r.y,-1.,0.,0.,1.);
    path.line_to((right,bottom-a));
    corner(&mut path,right,bottom,0.,-1.,-1.,0.);
    path.line_to((r.x+a,bottom));
    corner(&mut path,r.x,bottom,1.,0.,0.,-1.);
    path.line_to((r.x,r.y+a));
    corner(&mut path,r.x,r.y,0.,1.,1.,0.);
    path.close_path();
    path
}

#[inline]
fn active_cover_frost(amount: f64, card_frost: f64, capsule_frost: f64) -> f64 {
    (if amount >= 0.5 { card_frost } else { capsule_frost }).clamp(0., 100.)
}

#[inline]
fn acrylic_cover_sigma(target_frost: f64, max_side: u32) -> f32 {
    if target_frost < 0.5 {
        0.0
    } else {
        (14.0 * (target_frost as f32 / 100.0).powf(1.55) * (max_side.max(1) as f32 / 192.0)).max(0.35)
    }
}

#[inline]
fn tone_acrylic_cover_channel(lum: f32, v: f32) -> u8 {
    let s_v = (lum + (v - lum) * 1.16).clamp(0., 1.);
    let tone = (s_v * 0.78) / (0.86 + 0.28 * s_v);
    (tone.clamp(0., 1.) * 255.).round() as u8
}

#[cfg(test)]
mod acrylic_cover_tests {
    use super::*;

    #[test]
    fn acrylic_cover_frost_is_zero_at_zero_and_independent_of_source_resolution() {
        assert_eq!(acrylic_cover_sigma(0., 64), 0.0);
        assert_eq!(acrylic_cover_sigma(0.4, 512), 0.0);
        let s64 = acrylic_cover_sigma(40., 64) / 64.0;
        let s128 = acrylic_cover_sigma(40., 128) / 128.0;
        let s192 = acrylic_cover_sigma(40., 192) / 192.0;
        assert!((s64 - s128).abs() < 1e-4 && (s128 - s192).abs() < 1e-4);
        assert!(acrylic_cover_sigma(80., 192) > acrylic_cover_sigma(40., 192));
        assert!(acrylic_cover_sigma(40., 192) > acrylic_cover_sigma(10., 192));
    }

    #[test]
    fn acrylic_cover_uses_capsule_frost_when_collapsed_and_card_frost_when_expanded() {
        assert_eq!(active_cover_frost(0.0, 95.0, 25.0), 25.0);
        assert_eq!(active_cover_frost(0.49, 95.0, 25.0), 25.0);
        assert_eq!(active_cover_frost(0.50, 95.0, 25.0), 95.0);
        assert_eq!(active_cover_frost(1.0, 95.0, 25.0), 95.0);
        assert_eq!(active_cover_frost(1.0, 140.0, -5.0), 100.0);
        assert_eq!(active_cover_frost(0.0, 95.0, -5.0), 0.0);
    }

    #[test]
    fn acrylic_cover_tone_preserves_midtones_and_caps_pure_white_for_contrast() {
        let mid = tone_acrylic_cover_channel(0.5, 0.5);
        let white = tone_acrylic_cover_channel(1.0, 1.0);
        assert!((90..=112).contains(&mid), "midtone unexpectedly crushed/blown: {mid}");
        assert!(white <= 180, "pure white exceeds contrast cap: {white}");
        assert!(white > mid);
    }
}
