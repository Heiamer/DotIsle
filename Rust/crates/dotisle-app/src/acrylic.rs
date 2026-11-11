//! One acrylic material for the overview deck and capsule navigation.
use crate::{
    draw::{self, rect},
    widgets::Ui,
};
use std::sync::{Arc, OnceLock};
use vello::{
    kurbo::{Affine, Rect},
    peniko::{Blob, Fill, Gradient, ImageAlphaType, ImageBrush, ImageData, ImageFormat},
    Scene,
};
fn grain() -> ImageData {
    static IMAGE: OnceLock<ImageData> = OnceLock::new();
    IMAGE
        .get_or_init(|| {
            let mut pixels = Vec::with_capacity(64 * 64 * 4);
            let mut seed = 0x71ac492bu32;
            for _ in 0..64 * 64 {
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let ink = if seed & 0x10000 == 0 { 0 } else { 255 };
                pixels.extend_from_slice(&[ink, ink, ink, ((seed >> 24) % 3) as u8]);
            }
            ImageData {
                data: Blob::new(Arc::new(pixels)),
                format: ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,
                width: 64,
                height: 64,
            }
        })
        .clone()
}
pub fn surface(ui: &mut Ui, r: Rect, radius: f64, backdrop: &Scene, source: Affine, dark: bool) {
    let shape = draw::continuous_rect(r, radius);
    let s = ui.s;
    ui.scene.draw_blurred_rounded_rect(
        Affine::translate((0., 5.)),
        r,
        draw::alpha(0, if dark { 0.22 } else { 0.065 }),
        radius,
        14.,
    );
    ui.scene.push_isolated_clip_layer(Affine::IDENTITY, &shape);
    ui.scene
        .push_frost_layer(Affine::IDENTITY, &r.inflate(28., 28.), 12.);
    ui.scene.append(backdrop, Some(source));
    ui.scene.pop_layer();
    let tint = Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([
        draw::alpha(s.container, if dark { 0.78 } else { 0.76 }),
        draw::alpha(s.surface, if dark { 0.72 } else { 0.60 }),
    ]);
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &tint, None, &shape);
    let light = Gradient::new_linear((r.x0, r.y0), (r.x0, r.y1)).with_stops([
        draw::alpha(0xffffff, if dark { 0.035 } else { 0.14 }),
        draw::alpha(0xffffff, 0.),
    ]);
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &light, None, &shape);
    let grain = ImageBrush::from(grain());
    for row in 0..(r.height() / 128.).ceil() as usize {
        for col in 0..(r.width() / 128.).ceil() as usize {
            ui.scene.draw_image(
                &grain,
                Affine::translate((r.x0 + col as f64 * 128., r.y0 + row as f64 * 128.))
                    * Affine::scale(2.),
            );
        }
    }
    ui.scene.pop_layer();
    ui.scene.stroke(
        &vello::kurbo::Stroke::new(0.8),
        Affine::IDENTITY,
        draw::alpha(
            if dark { 0xffffff } else { s.on_surface },
            if dark { 0.10 } else { 0.065 },
        ),
        None,
        &shape,
    );
}
pub fn inset(ui: &mut Ui, r: Rect) {
    let s = ui.s;
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        draw::alpha(s.on_surface, 0.035),
        None,
        &draw::continuous_rect(r, 16.),
    );
}

/// Album art remains visible in the backdrop; only the dedicated cover stays sharp.
pub fn album(ui: &mut Ui, r: Rect, radius: f64, image: &ImageData) {
    let shape = draw::continuous_rect(r, radius);
    ui.scene.push_isolated_clip_layer(Affine::IDENTITY, &shape);
    ui.scene
        .push_frost_layer(Affine::IDENTITY, &r.inflate(30., 30.), 10.);
    draw::image(ui.scene, image, r.inflate(28., 28.), 0.);
    ui.scene.pop_layer();
    let shade = Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1))
        .with_stops([draw::alpha(0x080b14, 0.75), draw::alpha(0x080b14, 0.32)]);
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &shade, None, &shape);
    let lower = rect(r.x0, r.y1 - 150., r.width(), 150.);
    let shade = Gradient::new_linear((lower.x0, lower.y0), (lower.x0, lower.y1))
        .with_stops([draw::alpha(0x080b14, 0.), draw::alpha(0x080b14, 0.56)]);
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &shade, None, &lower);
    ui.scene.pop_layer();
}

/// Fade a sampled, genuinely blurred content edge into the unobscured deck.
pub fn scroll_edge(ui: &mut Ui, r: Rect, top: bool) {
    use vello::peniko::{BlendMode, Compose, Mix};
    let source = ui.scene.snapshot();
    let (start, end) = if top { (r.y0, r.y1) } else { (r.y1, r.y0) };
    ui.scene.push_isolated_clip_layer(Affine::IDENTITY, &r);
    ui.scene
        .push_frost_layer(Affine::IDENTITY, &r.inflate(30., 30.), 10.);
    ui.scene.append(&source, None);
    ui.scene.pop_layer();
    let mask = Gradient::new_linear((r.x0, start), (r.x0, end))
        .with_stops([draw::alpha(0xffffff, 1.), draw::alpha(0xffffff, 0.)]);
    ui.scene.push_layer(
        BlendMode::new(Mix::Normal, Compose::DestIn),
        1.,
        Affine::IDENTITY,
        &r,
    );
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &mask, None, &r);
    ui.scene.pop_layer();
    let veil = Gradient::new_linear((r.x0, start), (r.x0, end)).with_stops([
        draw::alpha(ui.s.surface, 0.82),
        draw::alpha(ui.s.surface, 0.),
    ]);
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &veil, None, &r);
    ui.scene.pop_layer();
}
