//! Bounded primitive scene replay. Feature DLLs never receive a Rust renderer pointer.
use serde_json::Value;
use vello::{
    kurbo::{Affine, Rect, RoundedRect, Stroke},
    peniko::Gradient,
    Scene,
};

pub fn paint(scene: &mut Scene, commands: &Value) {
    for c in commands.as_array().into_iter().flatten().take(256) {
        let Some(op) = c["op"].as_str() else { continue };
        let Some(b) = c["bounds"].as_array().filter(|b| b.len() == 4) else {
            continue;
        };
        let b = b.iter().filter_map(Value::as_f64).collect::<Vec<_>>();
        if b.len() != 4 || b.iter().any(|n| !n.is_finite()) {
            continue;
        }
        let r = Rect::new(b[0], b[1], b[2], b[3]);
        // 外扩半径严格控制在 14dp 以内，确保 24dp 安全边距内不会产生直角切边黑线
        let inflate = c["inflate"].as_f64().unwrap_or(0.).clamp(0., 14.);
        let radius = c["radius"].as_f64().unwrap_or(0.).clamp(0., 1000.);
        let alpha = (255. * c["alpha"].as_f64().unwrap_or(0.).clamp(0., 1.)).round() as u32;
        if alpha == 0 {
            continue;
        }
        if op == "blurredHalo" {
            let rgb = c["color"].as_u64().unwrap_or(0x88ccff) as u32 & 0xffffff;
            let blur_std = c["blur"].as_f64().unwrap_or(8.0).clamp(1.0, 14.0);
            let shape = r.inflate(inflate, inflate);
            scene.draw_blurred_rounded_rect(
                Affine::IDENTITY,
                shape,
                crate::renderer::color((alpha << 24) | rgb),
                radius + inflate,
                blur_std,
            );
            continue;
        }
        if op != "roundedStroke" {
            continue;
        }
        let width = c["width"].as_f64().unwrap_or(1.).clamp(0., 64.);
        let shape = RoundedRect::from_rect(r.inflate(inflate, inflate), radius + inflate);
        let colors = c["colors"]
            .as_array()
            .into_iter()
            .flatten()
            .take(16)
            .filter_map(Value::as_u64)
            .map(|rgb| crate::renderer::color((alpha << 24) | (rgb as u32 & 0xffffff)))
            .collect::<Vec<_>>();
        if colors.len() == 1 {
            scene.stroke(&Stroke::new(width), Affine::IDENTITY, colors[0], None, &shape);
        } else if colors.len() > 1 {
            let center = r.center();
            let transform = Affine::translate((center.x, center.y))
                * Affine::rotate(c["phase"].as_f64().unwrap_or(0.))
                * Affine::translate((-center.x, -center.y));
            let gradient =
                Gradient::new_sweep(center, 0., std::f32::consts::TAU).with_stops(colors.as_slice());
            scene.stroke(
                &Stroke::new(width),
                Affine::IDENTITY,
                &gradient,
                Some(transform),
                &shape,
            );
        }
    }
}
