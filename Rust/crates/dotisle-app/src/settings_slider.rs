//! Native settings-only slider, matching the MIUIX Slider proportions.
//! Design reference: compose-miuix-ui/miuix basic/Slider.kt (no Compose runtime).
use crate::{
    draw::{self, rect},
    model::Action,
    widgets::Ui,
};
use vello::kurbo::Rect;

pub fn render(
    ui: &mut Ui,
    bounds: Rect,
    title: &str,
    key: &'static str,
    value: f64,
    min: f64,
    max: f64,
) {
    let height = 28.;
    let radius = height / 2.;
    let track = rect(
        bounds.x0,
        bounds.center().y - radius,
        bounds.width(),
        height,
    );
    // The endpoints belong to the thumb centres, so minimum and maximum stay
    // inside the rounded track. Pointer projection uses the identical interval.
    let travel = rect(track.x0 + radius, track.y0, track.width() - height, height);
    let fraction = if value.is_finite() && max > min {
        ((value - min) / (max - min)).clamp(0., 1.)
    } else {
        0.
    };
    let thumb_x = travel.x0 + travel.width() * fraction;
    let hit = ui.hit(
        bounds.inflate(0., 8.),
        title,
        Action::RangeHandle(
            key,
            min,
            max,
            [travel.x0, travel.y0, travel.width(), travel.height()],
        ),
        true,
    );
    let engaged = bounds.inflate(0., 8.).contains(ui.pointer)
        || hit.is_some_and(|i| ui.pressed == Some(i) || ui.focus == Some(i));
    let motion_key = format!("settings-slider:{key}:engagement");
    let target = if engaged { 1. } else { 0. };
    ui.motion_targets.push((motion_key.clone(), target));
    let amount = ui
        .selection_values
        .get(&motion_key)
        .copied()
        .unwrap_or(target)
        .clamp(0., 1.);
    draw::fill(ui.scene, track, radius, ui.s.outline_variant);
    let filled = rect(track.x0, track.y0, thumb_x - track.x0 + radius, height);
    draw::fill(ui.scene, filled, radius, ui.s.primary);
    let thumb_radius = height * 0.36 * (1. + 0.127 * amount);
    let thumb = rect(
        thumb_x - thumb_radius,
        track.center().y - thumb_radius,
        thumb_radius * 2.,
        thumb_radius * 2.,
    );
    ui.scene.draw_blurred_rounded_rect(
        vello::kurbo::Affine::IDENTITY,
        thumb,
        draw::alpha(0, 0.18),
        thumb_radius,
        1.5,
    );
    draw::circle(ui.scene, thumb_x, track.center().y, thumb_radius, 0xffffff);
    if hit.is_some() && ui.focus == hit {
        draw::outline(
            ui.scene,
            track.inflate(3., 3.),
            radius + 3.,
            ui.s.primary,
            1.5,
        );
    }
}
