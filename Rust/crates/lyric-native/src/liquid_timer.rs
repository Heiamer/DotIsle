//! Determinate liquid fill inside the compact timer capsule.
//! Two inexpensive vector fronts share the existing capsule clip. Timer elapsed
//! time is the phase source, so pausing freezes both progress and flow exactly.
use vello::{
    kurbo::{Affine, BezPath, Rect, RoundedRect},
    peniko::{Fill, Gradient},
    Scene,
};

fn fraction(value: f64) -> f64 {
    if value.is_finite() {
        value.clamp(0., 1.)
    } else {
        0.
    }
}

fn front(bounds: Rect, amount: f64, phase: f64, amplitude: f64) -> BezPath {
    let x = bounds.x0 + bounds.width() * amount;
    let mut path = BezPath::new();
    path.move_to((bounds.x0, bounds.y0));
    path.line_to((x, bounds.y0));
    let segments = 12;
    for i in 1..=segments {
        let t = i as f64 / segments as f64;
        let wave = (t * std::f64::consts::TAU + phase).sin()
            + 0.28 * (t * std::f64::consts::TAU * 2. - phase * 0.7).sin();
        path.line_to((x + amplitude * wave, bounds.y0 + bounds.height() * t));
    }
    path.line_to((bounds.x0, bounds.y1));
    path.close_path();
    path
}

pub fn paint(
    scene: &mut Scene,
    bounds: Rect,
    radius: f64,
    progress: f64,
    elapsed_ms: f64,
    accent: u32,
    animate: bool,
) {
    let amount = fraction(progress);
    if amount == 0. || bounds.is_zero_area() {
        return;
    }
    let phase = if animate && elapsed_ms.is_finite() {
        elapsed_ms / 1800. * std::f64::consts::TAU
    } else {
        0.
    };
    let amplitude = if animate {
        (bounds.height() * 0.055).min(2.4) * (amount * 12.).min(1.) * ((1. - amount) * 12.).min(1.)
    } else {
        0.
    };
    let clip = RoundedRect::from_rect(bounds, radius.min(bounds.height() / 2.));
    scene.push_clip_layer(Affine::IDENTITY, &clip);
    let tint = |alpha| crate::renderer::color((accent & 0xffffff) | alpha);
    // Full progress fills every pixel of the capsule; no moving gap remains.
    if amount >= 1. {
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            tint(0x48000000),
            None,
            &bounds,
        );
    } else {
        let trail = front(bounds, amount, phase + 1.1, amplitude * 1.55);
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            tint(0x18000000),
            None,
            &trail,
        );
        let main = front(bounds, amount, phase, amplitude);
        let gradient = Gradient::new_linear((bounds.x0, bounds.y0), (bounds.x1, bounds.y1))
            .with_stops([tint(0x22000000), tint(0x52000000)]);
        scene.fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &main);
    }
    scene.pop_layer();
}

/// The satellite fills upward. Rotate the shared horizontal painter so its
/// zero edge sits on the bottom, keeping the same wave and endpoint behavior.
pub fn paint_rising(
    scene: &mut Scene,
    bounds: Rect,
    radius: f64,
    progress: f64,
    elapsed_ms: f64,
    accent: u32,
    animate: bool,
) {
    let mut local = Scene::new();
    paint(
        &mut local,
        Rect::new(0., 0., bounds.height(), bounds.width()),
        radius,
        progress,
        elapsed_ms,
        accent,
        animate,
    );
    scene.append(
        &local,
        Some(
            Affine::translate((bounds.x0, bounds.y1))
                * Affine::rotate(-std::f64::consts::FRAC_PI_2),
        ),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_and_endpoint_progress_is_bounded() {
        assert_eq!(fraction(f64::NAN), 0.);
        assert_eq!(fraction(f64::INFINITY), 0.);
        assert_eq!(fraction(-0.2), 0.);
        assert_eq!(fraction(1.8), 1.);
    }
    #[test]
    fn liquid_geometry_stays_inside_its_capsule_height() {
        let bounds = Rect::new(0., 0., 260., 40.);
        for amount in [0.1, 0.5, 0.9] {
            use vello::kurbo::Shape;
            let r = front(bounds, amount, 0.7, 2.).bounding_box();
            assert!(r.y0 >= bounds.y0 && r.y1 <= bounds.y1);
            assert!(r.x0 >= bounds.x0 && r.x1 <= bounds.x1);
        }
    }
}
