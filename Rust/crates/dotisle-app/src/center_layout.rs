//! Shared header and navigation geometry, in logical pixels.
use crate::{draw::rect, widgets::Ui};
use vello::{
    kurbo::{Affine, Rect},
    peniko::{BlendMode, Compose, Fill, Gradient, Mix},
};

pub const TITLE_Y: f64 = 64.;
pub const SUBTITLE_Y: f64 = 110.;
pub const CONTENT_TOP: f64 = 151.;
pub const EDITOR_TOP: f64 = 68.;
pub const TODO_ACTION_Y: f64 = 79.;
pub const NAV_BOTTOM: f64 = 22.;
pub const NAV_HEIGHT: f64 = 72.;

/// Keep the established card interiors independent of the extended edge clip.
/// 369px is the original header, bottom reserve and stack inset combined.
pub fn deck_card_height(viewport_height: f64) -> f64 {
    (viewport_height - 369.).clamp(280., 430.)
}

pub fn navigation(width: f64, height: f64) -> Rect {
    let w = (width - 96.).min(744.);
    rect(
        (width - w) / 2.,
        height - NAV_BOTTOM - NAV_HEIGHT,
        w,
        NAV_HEIGHT,
    )
}

/// Mask the already-isolated deck once. Transparent pixels reveal the original
/// theme/wallpaper; a solid-colour veil cannot match that backdrop at its edge.
/// Card acrylic and velocity blur remain inside the deck's existing scene.
pub fn fade_deck_edges(ui: &mut Ui, clip: Rect, height: f64) {
    let top_end = clip.y0 + 58.;
    let bottom_start = height - 139.;
    let bottom_end = height - NAV_BOTTOM;
    let position = |y: f64| ((y - clip.y0) / clip.height()).clamp(0., 1.) as f32;
    let mask = Gradient::new_linear((clip.x0, clip.y0), (clip.x0, clip.y1)).with_stops([
        (0., crate::draw::alpha(0xffffff, 0.)),
        (position(top_end), crate::draw::alpha(0xffffff, 1.)),
        (position(bottom_start), crate::draw::alpha(0xffffff, 1.)),
        (position(bottom_end), crate::draw::alpha(0xffffff, 0.)),
        (1., crate::draw::alpha(0xffffff, 0.)),
    ]);
    ui.scene.push_layer(
        BlendMode::new(Mix::Normal, Compose::DestIn),
        1.,
        Affine::IDENTITY,
        &clip,
    );
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &mask, None, &clip);
    ui.scene.pop_layer();
}
