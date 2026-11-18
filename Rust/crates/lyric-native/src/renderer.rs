//! Vello island painter. The v2 layout, activity model, hit targets and motion are retained.
use crate::{
    layout::{Layout, Rect},
    motion::Frame,
};
use dotisle_render::draw::{Icon, Text};
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

pub struct PreparedCover {
    image: ImageData,
    seed: Option<u32>,
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
    glass: crate::glass::Sampler,
    glass_live: bool,
    glass_allowed: bool,
    glass_active: bool,
    gpu: crate::vello_surface::Surface,
    canvas: VScene,
    text: Text,
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
        if let Some(dir) = font.parent() { text.defer_harmony(dir); }
        Ok(Self {
            transport_ink: None,
            window: hwnd,
            glass: Default::default(),
            glass_live: false,
            glass_allowed: true,
            glass_active: false,
            gpu,
            canvas: VScene::new(),
            text,
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
        })
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
            glass_refraction: self.glass_allowed,
            roles: self.dynamic_resolved.map(|v| v.roles),
            material,
            cover_background,
            palette: self.paint_palette,
            cover: if cover_background { self.cover.clone() } else { None },
        }
    }
    pub unsafe fn set_material_theme(&mut self, theme: &MaterialTheme) -> Result<()> {
        self.glass_allowed = theme.glass_refraction;
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
    pub fn glass_tick(&mut self) -> bool {
        self.glass_active && self.glass_live && (self.glass.poll() || self.glass.due())
    }
    pub fn glass_status(&self) -> serde_json::Value {
        serde_json::json!({"active":self.glass_active,"samples":self.glass.captures,"enabled":self.glass_allowed})
    }
    pub fn glass_fixture(&mut self) {
        self.glass.enable_fixture(self.width, self.height);
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
        let immersion = if s.custom_geometry.is_some() {
            0.
        } else {
            (s.music_mix - 3.).clamp(0., 1.)
        };
        if s.panel.is_none()
            && s.notification.is_none()
            && s.osd.is_none() && s.alert.is_none()
            && immersion > 0.
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
        let p = s.paint(self.paint_palette);
        let gradient = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h))
            .with_stops([color(p.top), color(p.bottom)]);
        self.canvas.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            &gradient,
            None,
            &RoundedRect::from_rect(vr(r), radius),
        );
        if p.cover_alpha > 0.001 {
            if let Some(image) = self.cover.clone() {
                let blur = p.cover_blur.max(0.).round() as u32;
                if self
                    .blurred_cover
                    .as_ref()
                    .is_none_or(|(key, _)| *key != blur)
                {
                    if let Some(rgba) = image::RgbaImage::from_raw(
                        image.width,
                        image.height,
                        image.data.data().to_vec(),
                    ) {
                        let small = image::imageops::thumbnail(&rgba, 128, 128);
                        let ratio = small.width() as f32 / image.width as f32;
                        let blurred = image::imageops::blur(&small, blur as f32 * ratio);
                        let (w, h) = blurred.dimensions();
                        let mut pixels = blurred.into_raw();
                        // Keep the frosted artwork a dark M3E surface: a near-white cover would otherwise lift the card past white text contrast.
                        for c in pixels.chunks_exact_mut(4) {
                            c[0] = (c[0] as u32 * 58 / 100) as u8;
                            c[1] = (c[1] as u32 * 58 / 100) as u8;
                            c[2] = (c[2] as u32 * 58 / 100) as u8;
                        }
                        self.blurred_cover = Some((
                            blur,
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
                self.image(&image, r, radius, p.cover_alpha as f32);
                let scrim = ((80. + 36. * (1. - p.amount)) as u32) << 24;
                self.fill(
                    r,
                    radius,
                    scrim
                        | crate::cover_color::mix(
                            0x10171a,
                            self.paint_palette.background,
                            0.4 * p.tint,
                        ),
                );
            }
        }
        if p.frost > 0.001 {
            let frost = Gradient::new_linear((r.x, r.y), (r.x, r.y + r.h)).with_stops([
                color(((p.frost * 15.) as u32) << 24 | 0xffffff),
                color(0x00ffffff),
            ]);
            self.canvas.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &frost,
                None,
                &RoundedRect::from_rect(vr(r), radius),
            );
        }
        if self.glass_allowed
            && ((s.material == Material::Glass && p.amount > 0.001)
                || (s.compact == Material::Glass && p.amount < 0.999))
        {
            self.glass_active = true;
            if let Some(edge) = self
                .glass
                .image(self.window, r, radius, self.dpi, self.glass_live)
            {
                self.image(&edge, r, radius, 1.);
            }
        }
        let edge = Rect::new(r.x + 0.5, r.y + 0.5, r.w - 1., r.h - 1.);
        self.outline(
            edge,
            (radius - 0.5).max(0.),
            ((24. * p.reflection) as u32) << 24 | 0xffffff,
            0.8,
        );
    }
    fn music_lyric(&mut self, text: &str, r: Rect, size: f64, align: f64, ink: u32, s: &Scene<'_>) {
        // 5.0.1: the old 400-weight lyric looked thin on frosted covers. Use the
        // registered Medium face for the card and SemiBold for large immersive lyrics.
        let weight = if size >= 20. { 600 } else { 500 };
        let width = self
            .text
            .measure_weighted(text, size as f32, weight)
            .min(r.w);
        let x = r.x + (r.w - width) * align;
        let phase = if s.animate {
            (s.lyric_age_ms / 180.).clamp(0., 1.)
        } else {
            1.
        };
        let y = r.y + 4. * (1. - phase).powi(3);
        let boundary = x + width * s.highlight.clamp(0., 1.);
        for (clip, c) in [
            (
                Rect::new(r.x, r.y, (boundary - r.x).max(0.), r.h + 5.),
                s.progress_color,
            ),
            (
                Rect::new(boundary, r.y, (r.x + r.w - boundary).max(0.), r.h + 5.),
                ink,
            ),
        ] {
            if clip.w > 0. {
                self.clip(clip, 0., phase as f32);
                self.text.line_fixed(
                    &mut self.canvas,
                    text,
                    vr(Rect::new(x, y, width, r.h)),
                    size as f32,
                    c,
                    weight as u16,
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
        let double = !s.compact_secondary.is_empty();
        let (h, line, second) = self.compact_metrics(s.font_size, double, s.layout_scale);
        let y = c.y + (c.h - h) / 2.;
        let viewport = Rect::new(c.x + 42., y, c.w - 76., line);
        let phase = if s.animate {
            (s.lyric_age_ms / 180.).clamp(0., 1.)
        } else {
            1.
        };
        if phase < 1. && !s.previous_lyric.is_empty() {
            self.clip(
                Rect::new(viewport.x, c.y, viewport.w, c.h),
                0.,
                (1. - phase) as f32,
            );
            self.text_at(
                s.previous_lyric,
                Rect::new(viewport.x, y - 5. * phase, viewport.w, line),
                s.font_size,
                500,
                0xff000000 | tones.on_surface,
                if double { 0.5 } else { 0. },
            );
            self.canvas.pop_layer();
        }
        self.music_lyric(
            s.lyric,
            viewport,
            s.font_size,
            if double { 0.5 } else { 0. },
            0xff000000 | tones.on_surface,
            s,
        );
        if double {
            self.text_at(
                s.compact_secondary,
                Rect::new(viewport.x, y + line + 1., viewport.w, second),
                (s.font_size * 0.78).clamp(9., 11.5),
                400,
                0xff000000 | tones.on_surface_variant,
                0.5,
            );
        }
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
    fn resident_compact(&mut self, s: &Scene<'_>) {
        let c = s.layout.capsule;
        let v = s.resident;
        let tones = self.m3(s);
        let ink = if s.mini && s.mini_glass { s.mini_ink } else { 0xff000000 | tones.on_surface };
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
    fn compact_view(&mut self, s: &Scene<'_>) {
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
            self.text_at(
                &label,
                Rect::new(c.x + 22., c.y, c.w - 44., c.h),
                crate::compact::mini_font_size(s.font_size),
                500,
                if s.mini_glass {
                    s.mini_ink
                } else {
                    0xff000000 | tones.on_surface
                },
                0.5,
            );
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
                self.text_at(
                    &p.value,
                    Rect::new(c.x + 40., cy - 17., c.w - 70., 19.),
                    14.,
                    600,
                    0xff000000 | tones.on_surface,
                    0.5,
                );
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
                            let scrim = crate::cover_color::mix(
                                0x10171a,
                                self.paint_palette.background,
                                0.4 * p.tint,
                            );
                            bg = crate::cover_color::mix(
                                bg,
                                scrim,
                                (80. + 36. * (1. - p.amount)) / 255.,
                            );
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
        let tonal = 0xff000000 | self.m3(s).secondary_container;
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
        let immersion = if s.custom_geometry.is_some() {
            0.
        } else {
            (s.music_mix - 3.).clamp(0., 1.)
        };
        if g.cover.w > 0. && immersion < 0.999 {
            self.clip(g.cover, 0., (1. - immersion) as f32);
            self.flipping_cover(g.cover, s);
            self.canvas.pop_layer();
        }
        self.text_at(
            s.title,
            g.title,
            16. - 2. * immersion,
            600,
            0xff000000 | (s.text_color & 0xffffff),
            g.align,
        );
        if !s.mini {
            self.text_at(s.artist, g.artist, 10., 400, secondary, g.align);
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
            self.music_lyric(s.lyric, g.lyric, s.font_size + 2., g.align, s.text_color, s);
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
                if g.align >= 0.5 {
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
        if g.track.w > 0. {
            dotisle_render::progress::draw(
                &mut self.canvas,
                vr(g.track),
                s.progress.clamp(0., 1.),
                phase,
                0.7,
                thumb,
                wave,
                spin,
                look,
                s.progress_bar,
                s.progress_thumb,
            );
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
        if let Some(p) = s.panel {
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
    fn capsule_alert_content(&mut self,a:&crate::capsule_alert::Alert,r:Rect){
        let ink=0xfff5f5f7;let muted=0xffaeb4c0;let blue=0xff75b4ff;
        let glyph=Rect::new(r.x+17.,r.y+(r.h-25.)/2.,25.,25.);
        if let Some(image)=a.image.clone(){self.image(&image,glyph,7.,1.);}else{match a.icon.as_str(){
            kind if kind.starts_with("Message:")=>{
                // App identity only: no sender, preview, unread count or chat text.
                let label=match kind.trim_start_matches("Message:"){"WeChat"=>"微","Feishu"=>"飞","DingTalk"=>"钉","WXWork"=>"企",_=>"信"};
                self.fill(glyph,7.,0xff2688ed);
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
        let icon = Rect::new(r.x + 18., r.y + (r.h - 40.) / 2., 40., 40.);
        if let Some(kind) = n.app_id.strip_prefix("LyricIsland.Device.") {
            let tones = self
                .m3_cache
                .map(|(_, _, c)| c)
                .unwrap_or_else(|| dotisle_domain::monet::scheme(0x496b55, dark));
            self.fill(icon, 14., 0xff000000 | tones.secondary_container);
            self.device_glyph(
                Rect::new(icon.x + 7., icon.y + 7., 26., 26.),
                kind,
                0xff000000 | tones.on_secondary_container,
            );
        } else if let Some(im) = self.notification_icon.clone() {
            self.image(&im, icon, 10., 1.);
        } else {
            self.fill(icon, 10., 0xff3188ee);
            self.text_at(
                &n.app_name.chars().next().unwrap_or('•').to_string(),
                Rect::new(icon.x, icon.y + 2., 40., 34.),
                22.,
                600,
                0xffffffff,
                0.5,
            );
        }
        self.text_line(
            &n.app_name,
            Rect::new(r.x + 76., r.y + 10., r.w - 160., 17.),
            11.5,
            500,
            (0xff000000 | tones.on_surface_variant),
        );
        self.text_line(
            "现在",
            Rect::new(r.x + r.w - 70., r.y + 12., 32., 14.),
            10.,
            400,
            (0xff000000 | tones.on_surface_variant),
        );
        self.text_line(
            &n.title,
            Rect::new(r.x + 76., r.y + 27., r.w - 100., 21.),
            14.,
            600,
            (0xff000000 | tones.on_surface),
        );
        let lines = self.wrap(&n.body, 12.5, r.w - 100.);
        for (i, line) in lines.iter().take(2).enumerate() {
            self.text_line(
                line,
                Rect::new(r.x + 76., r.y + 49. + i as f64 * 16., r.w - 100., 18.),
                12.5,
                400,
                (0xff000000 | tones.on_surface_variant),
            );
        }
        self.icon(
            Icon::Close,
            Rect::new(r.x + r.w - 28., r.y + 11., 16., 16.),
            (0xff000000 | tones.on_surface_variant),
        );
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
            64.
        } else if self.wrap(&n.body, 12.5, width - 100.).len() <= 1 {
            80.
        } else {
            96.
        })
    }
    fn osd_content(&mut self, n: &crate::notifications::Notice, r: Rect, dark: bool) {
        if n.app_id == "LyricIsland.Greeting" {
            let tones = self
                .m3_cache
                .map(|(_, _, c)| c)
                .unwrap_or_else(|| dotisle_domain::monet::scheme(0x496b55, true));
            let accent = 0xff000000 | tones.primary;
            self.icon(
                Icon::Island,
                Rect::new(r.x + 14., r.y + (r.h - 20.) / 2., 20., 20.),
                accent,
            );
            self.text_line(
                &n.title,
                Rect::new(r.x + 44., r.y + 12., 74., 20.),
                13.,
                600,
                0xff000000 | tones.on_surface,
            );
            self.text_line(
                &n.body,
                Rect::new(r.x + 116., r.y + 14., r.w - 130., 16.),
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
        self.fill(icon, 10., 0x1affffff);
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
            Rect::new(r.x + 50., r.y + 13., 70., 18.),
            12.5,
            600,
            (0xff000000 | roles.on_surface),
        );
        self.text_line(
            &n.body,
            Rect::new(r.x + 122., r.y + 14., r.w - 134., 15.),
            10.5,
            500,
            tint,
        );
    }
    pub unsafe fn render(&mut self, s: &Scene<'_>, present: bool) -> Result<()> {
        self.paint_scene(s, present);
        if present {
            crate::frame_stat::tick();
        }
        self.gpu.draw(&self.canvas, self.dpi as f64 / 96., present).map_err(err)
    }
    pub fn capture_layout_frame(&mut self, scene: &Scene<'_>) -> Result<Vec<u8>> {
        let (active, live, allowed) = (self.glass_active, self.glass_live, self.glass_allowed);
        self.paint_scene_inner(scene, false, false);
        (self.glass_active, self.glass_live, self.glass_allowed) = (active, live, allowed);
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
    fn paint_scene(&mut self, s: &Scene<'_>, present: bool) {
        self.paint_scene_inner(s, present, true);
    }
    fn paint_scene_inner(&mut self, s: &Scene<'_>, present: bool, include_desktop: bool) {
        self.glass_active = false;
        self.glass_live = present && !s.resizing && !s.drag_active;
        self.glass_allowed = include_desktop && s.glass_refraction && !s.dynamic_high_contrast && s.opacity > 0.;
        self.text.set_family(s.font_family);
        self.prepare_dynamic(s);
        let (mut themed, compact) = cover_scenes(s, Some(self.paint_palette));
        if s.expanded_cover_background {
            if let Some(resolved) = self.dynamic_resolved {
                themed.text_color = 0xff000000 | resolved.roles.on_surface;
            }
        }
        let s = &themed;
        if s.cover_age_ms >= 280. && self.cover_changed.elapsed() >= Duration::from_millis(280) {
            self.previous_cover = None;
        }
        self.canvas = VScene::new();
        if include_desktop {
            if let Some(background) = self.glass.background() {
                self.image(&background, s.layout.host, 0., 1.);
            }
        }
        let (base, base_radius) =
            s.layout
                .morph_shape_with_radii(s.frame.card_reveal, s.capsule_radius, s.radius);
        let p = s.frame.card_reveal;
        let (card_softness, compact_softness) = crate::motion::content_softness(
            p,
            s.animate && !s.drag_active && s.notification.is_none() && s.osd.is_none() && s.alert.is_none(),
        );
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
        self.clip(s.layout.host, 0., s.opacity * transient_alpha as f32);
        self.clip(s.layout.host, 0., s.background_opacity);
        self.canvas.draw_blurred_rounded_rect(
            Affine::translate((0., 5.)),
            vr(shape),
            color(if s.mini && p < 0.01 {
                0x0a000000
            } else {
                0x38000000
            }),
            radius,
            22. / 3.,
        );
        if !s.mini && s.notification.is_none() && s.osd.is_none() && s.alert.is_none() {
            let accent = self.m3(s).primary;
            crate::island_glow::paint(&mut self.canvas, vr(shape), radius, accent, s.glow);
        }
        let mut surface = Self::music_surface_spec(s);
        let saved_palette = self.paint_palette;
        if s.panel.is_some() || s.notification.is_some() || s.osd.is_some() || s.alert.is_some() {
            surface.cover_overlay = false;
            surface.card_opacity = 0.;
            surface.cover_background = true;
            if let Some(r) = self.dynamic_resolved {
                self.paint_palette.background = r.roles.surface_container;
            }
        }
        if s.alert.is_some(){surface.mini=false;surface.compact=s.classic_capsule;}
        self.material_surface(shape, radius, surface);
        self.paint_palette = saved_palette;
        self.canvas.pop_layer();
        let alpha = crate::motion::content_weights(s.frame).0 * music_alpha;
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
            self.clip(shape, radius, alpha as f32);
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
        let alpha = crate::motion::content_weights(s.frame).1 * music_alpha;
        if alpha > 0.001 {
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
            self.clip(shape, radius, alpha as f32);
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
        if !s.mini && s.component_count > 1 && s.notification.is_none() && s.osd.is_none() && s.alert.is_none() {
            self.clip(s.layout.host, 0., s.background_opacity);
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
    }
    pub unsafe fn render_bottom_osd(
        &mut self,
        n: &crate::notifications::Notice,
        opacity: f64,
        offset: f64,
        surface: crate::surface::Surface,
        present: bool,
    ) -> Result<()> {
        self.glass_live = present;
        self.glass_active = false;
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
    pub fn glass_captures(&self) -> u64 {
        self.glass.captures
    }
    pub fn resize(&mut self, width: u32, height: u32, dpi: f32) {
        self.gpu.resize(width, height);
        self.width = width;
        self.height = height;
        self.dpi = dpi;
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
        let immersion = if s.custom_geometry.is_some() {
            0.
        } else {
            (s.music_mix - 3.).clamp(0., 1.)
        };
        if s.panel.is_none()
            && s.notification.is_none()
            && s.osd.is_none() && s.alert.is_none()
            && immersion > 0.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Material {
    Acrylic,
    Glass,
}
impl Material {
    /// Both capsule modes open the same card and use the same material preference.
    pub fn expanded(settings: &serde_json::Value, _mini: bool) -> Self {
        Self::from_setting(settings["IslandMaterial"].as_str().unwrap_or("Acrylic"))
    }
    pub fn from_setting(value: &str) -> Self {
        match value {
            "Glass" => Self::Glass,
            _ => Self::Acrylic,
        }
    }
}
#[derive(Clone, PartialEq)]
pub struct MaterialTheme {
    pub glass_refraction: bool,
    pub roles: Option<dotisle_domain::dynamic_color::Roles>,
    pub material: Material,
    pub cover_background: bool,
    pub palette: crate::cover_color::CoverPalette,
    pub cover: Option<ImageData>,
}
impl MaterialTheme {
    pub fn new(material: Material, cover_background: bool) -> Self {
        Self {
            glass_refraction: true,
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
    pub background_opacity: f32,
    pub alert:Option<&'a crate::capsule_alert::Alert>,
    pub alert_rect:Rect,pub alert_radius:f64,pub alert_mix:f64,
    pub glow: crate::island_glow::Frame,
    pub transient_only: bool,
    pub custom_geometry: Option<lyric_core::music_layout::MusicGeometry>,
    pub glass_refraction: bool,
    pub play_shape: f64,
    pub capsule_radius: f64,
    pub update_phase: &'a str,
    pub feedback: [f32; 64],
    pub dynamic_source: &'a str,
    pub dynamic_system_seed: Option<u32>,
    pub dynamic_custom_seed: u32,
    pub dynamic_high_contrast: bool,
    pub font_family: &'a str,
    pub expressive_dark: bool,
    pub mini: bool,
    pub mini_glass: bool,
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
    pub material: Material,
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
    pub transport_theme: dotisle_render::transport::TransportTheme,
    pub hover: i32,
    pub pressed: i32,
    pub opacity: f32,
    pub transparent: bool,
    pub floating: bool,
    pub favorite_enabled: bool,
    pub has_track: bool,
    pub layout_scale: f64,
    pub activity_scales: [f64; 3],
    pub activity_opacity: f64,
    pub cover_opacity: f64,
    pub cover_age_ms: f64,
    pub outgoing_opacity: f64,
    pub outgoing_y: f64,
}

/// Lyric highlighting follows its own preference, independently of surface tint.
pub(crate) fn cover_scenes<'a>(
    s: &Scene<'a>,
    palette: Option<crate::cover_color::CoverPalette>,
) -> (Scene<'a>, Scene<'a>) {
    let mut expanded = *s;
    let mut compact = *s;
    if let Some(palette) = palette {
        let accent = 0xff000000 | palette.accent;
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
