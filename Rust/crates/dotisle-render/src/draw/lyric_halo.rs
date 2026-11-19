//! Cache the blurred ink for the current lyric only. The sharp text is still
//! drawn by Vello, including the separately animated karaoke fill.
use super::{Affine, ImageData, Rect, Scene, Text};
use image::{imageops, GrayImage};
use parley::{Layout, PositionedLayoutItem};
use std::sync::Arc;
use swash::{
    scale::{image::Image, Render, ScaleContext, Source},
    zeno::Vector,
    FontRef,
};
use vello::peniko::{Blob, ImageAlphaType, ImageBrush, ImageFormat, Mix};

struct Row {
    text: String,
    family: String,
    width: f64,
    height: f64,
    size: f32,
    weight: u16,
    used: bool,
    mask: Option<(ImageData, u32)>,
}

/// Two rows also cover a wrapped current lyric. Neither the translation nor a
/// catalogue of previously sung lines is cached. Use one pixel per DIP: a
/// diffuse mask needs no high-DPI supersampling or extra GPU filter pipeline.
#[derive(Default)]
pub struct LyricHalo([Option<Row>; 2]);

impl LyricHalo {
    pub fn draw(
        &mut self,
        scene: &mut Scene,
        text: &mut Text,
        line: &str,
        rect: Rect,
        size: f32,
        weight: u16,
        opacity: f32,
    ) {
        let (width, height) = (rect.width(), rect.height());
        let index = match self.0.iter().position(|row| {
            row.as_ref().is_some_and(|r| {
                r.text == line
                    && r.family == text.family
                    && (r.width - width).abs() < 1e-6
                    && (r.height - height).abs() < 1e-6
                    && r.size == size
                    && r.weight == weight
            })
        }) {
            Some(index) => index,
            None => {
                let index = self
                    .0
                    .iter()
                    .position(|r| r.as_ref().is_none_or(|r| !r.used))
                    .unwrap_or(1);
                let mask = make_mask(text, line, width, height, size, weight);
                self.0[index] = Some(Row {
                    text: line.to_owned(),
                    family: text.family.clone(),
                    width,
                    height,
                    size,
                    weight,
                    used: false,
                    mask,
                });
                index
            }
        };
        let row = self.0[index].as_mut().unwrap();
        row.used = true;
        if let Some((mask, pad)) = &row.mask {
            let pad = f64::from(*pad);
            let x = rect.x0 - pad;
            let y = rect.y0 - pad;
            let bounds = Rect::new(x, y, x + mask.width as f64, y + mask.height as f64);
            // Vello Hybrid rejects a fractional alpha on an image brush, so the fade-in
            // runs as a layer and the brush stays fully opaque.
            scene.push_layer(Mix::Normal, opacity.clamp(0., 1.), Affine::IDENTITY, &bounds);
            scene.draw_image(
                &ImageBrush::new(mask.clone()),
                Affine::translate((x, y)),
            );
            scene.pop_layer();
        }
    }

    pub fn end_frame(&mut self) {
        for slot in &mut self.0 {
            if let Some(row) = slot {
                if row.used {
                    row.used = false;
                } else {
                    *slot = None;
                }
            }
        }
    }
}

fn make_mask(
    text: &mut Text,
    line: &str,
    w: f64,
    h: f64,
    size: f32,
    weight: u16,
) -> Option<(ImageData, u32)> {
    if !(0. < w && w <= 1536. && 0. < h && h <= 96.) {
        return None;
    }
    let key = text.layout_key(line, size, weight, false);
    let baseline = text.cache[&key]
        .lines()
        .next()
        .map(|l| l.metrics().baseline)
        .unwrap_or(size * 0.8);
    let fit = text.fitted_layout_key(line, size, weight, w, false)?;
    let y = h as f32 / 2. + size * 0.34 - baseline;
    let sigma = (size * 0.19).clamp(1.6, 4.0);
    let pad = (4. * sigma).ceil() as u32;
    let (w, h) = (w.ceil() as u32, h.ceil() as u32);
    let mut ink = GrayImage::new(w + 2 * pad, h + 2 * pad);
    rasterize(&text.cache[&fit], &mut ink, pad, w, h, y);
    if !ink.as_raw().iter().any(|&a| a > 0) {
        return None;
    }
    let soft = imageops::fast_blur(&ink, sigma);
    let rgba: Vec<u8> = soft
        .as_raw()
        .iter()
        .flat_map(|&a| {
            let v = (u16::from(a) * 96 / 255) as u8;
            [v; 4] // Premultiplied white.
        })
        .collect();
    Some((
        ImageData {
            data: Blob::new(Arc::new(rgba)),
            format: ImageFormat::Rgba8,
            alpha_type: ImageAlphaType::AlphaPremultiplied,
            width: soft.width(),
            height: soft.height(),
        },
        pad,
    ))
}

fn rasterize(layout: &Layout<()>, ink: &mut GrayImage, pad: u32, w: u32, h: u32, y: f32) {
    let mut ctx = ScaleContext::new();
    let mut glyph = Image::new();
    let mut render = Render::new(&[Source::Outline]);
    for l in layout.lines() {
        for item in l.items() {
            let PositionedLayoutItem::GlyphRun(g) = item else {
                continue;
            };
            let run = g.run();
            let font = run.font();
            let Some(font) = FontRef::from_index(font.data.data(), font.index as usize) else {
                continue;
            };
            let mut scaler = ctx
                .builder(font)
                .size(run.font_size())
                .normalized_coords(run.normalized_coords())
                .build();
            let mut x = g.offset();
            for p in g.glyphs() {
                let gx = x + p.x;
                let gy = y + g.baseline() - p.y;
                x += p.advance;
                let (left, baseline) = (gx.floor(), gy.floor());
                if !render
                    .offset(Vector::new(gx - left, baseline - gy))
                    .render_into(&mut scaler, p.id, &mut glyph)
                {
                    continue;
                }
                let left = left as i32 + glyph.placement.left;
                let top = baseline as i32 - glyph.placement.top;
                let stride = glyph.placement.width as usize;
                if stride == 0 {
                    continue;
                }
                for (dy, row) in glyph.data.chunks_exact(stride).enumerate() {
                    let ty = (top + dy as i32) as u32;
                    if ty >= h {
                        continue;
                    }
                    for (dx, &a) in row.iter().enumerate() {
                        let tx = (left + dx as i32) as u32;
                        if tx < w {
                            let dst = ink.get_pixel_mut(tx + pad, ty + pad);
                            dst.0[0] = dst.0[0].max(a);
                        }
                    }
                }
            }
        }
    }
}
