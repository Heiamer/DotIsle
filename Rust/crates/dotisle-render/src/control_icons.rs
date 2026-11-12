//! A consistent 24px outline family for the Glass controls.
//! All glyphs share round caps, 1.75px strokes and an optical 20px content box.
use crate::draw::{self, Icon};
use vello::{
    kurbo::{Affine, BezPath, Cap, Join, Stroke},
    Scene,
};

pub fn icon(scene: &mut Scene, kind: Icon, x: f64, y: f64, size: f64, rgb: u32) {
    let data = match kind {
        Icon::Home => "M3 10L12 3L21 10 M5 9V20A1 1 0 0 0 6 21H10V15H14V21H18A1 1 0 0 0 19 20V9",
        Icon::Music => "M9 18V5L20 3V16 M9 9L20 7 M9 18A3 2.5 0 1 1 3 18A3 2.5 0 1 1 9 18 M20 16A3 2.5 0 1 1 14 16A3 2.5 0 1 1 20 16",
        Icon::Focus => "M9 2H15 M12 2V5 M18.5 5.5L20 4 M12 6A8 8 0 1 1 12 22A8 8 0 1 1 12 6 M12 10V14L15 16",
        Icon::Todo => "M3 6L5 8L8 4 M11 6H21 M3 13L5 15L8 11 M11 13H21 M4 20H8 M11 20H21",
        Icon::Settings => "M4 6H9 M15 6H21 M4 12H14 M20 12H21 M4 18H7 M13 18H21 M12 3A3 3 0 1 1 12 9A3 3 0 1 1 12 3 M17 9A3 3 0 1 1 17 15A3 3 0 1 1 17 9 M10 15A3 3 0 1 1 10 21A3 3 0 1 1 10 15",
        Icon::Play => "M8 4L21 12L8 20Z",
        Icon::Pause => "M7 4H10V20H7Z M15 4H18V20H15Z",
        Icon::Next => "M4 5L16 12L4 19Z M20 5V19",
        Icon::Prev => "M20 5L8 12L20 19Z M4 5V19",
        Icon::Plus => "M12 4V20 M4 12H20",
        Icon::Check => "M4 12L9 17L20 6",
        Icon::Close => "M6 6L18 18 M18 6L6 18",
        Icon::Arrow => "M9 5L16 12L9 19",
        Icon::Back => "M15 5L8 12L15 19",
        Icon::Island => "M8 7H16A5 5 0 0 1 16 17H8A5 5 0 0 1 8 7Z",
        Icon::Sun => "M12 8A4 4 0 1 1 12 16A4 4 0 1 1 12 8 M12 2V4 M12 20V22 M2 12H4 M20 12H22 M5 5L6.5 6.5 M17.5 17.5L19 19 M5 19L6.5 17.5 M17.5 6.5L19 5",
        Icon::Moon => "M20.5 13A8.5 8.5 0 1 1 11 3.5A7 7 0 0 0 20.5 13Z",
        Icon::Trash => "M3 6H21 M9 6V3H15V6 M6 6L7 21H17L18 6 M10 10V17 M14 10V17",
        Icon::Reset => "M4 10A8 8 0 1 1 5 18 M4 4V10H10",
        Icon::Search => "M10.5 3A7.5 7.5 0 1 1 10.5 18A7.5 7.5 0 1 1 10.5 3 M16 16L21 21",
        Icon::Volume => "M3 9H7L12 5V19L7 15H3Z M16 8A6 6 0 0 1 16 16 M19 5A10 10 0 0 1 19 19",
        Icon::Folder => "M3 6A2 2 0 0 1 5 4H10L13 7H20A1 1 0 0 1 21 8V19A1 1 0 0 1 20 20H4A1 1 0 0 1 3 19Z",
        Icon::Import => "M12 3V15 M7 10L12 15L17 10 M4 16V20A1 1 0 0 0 5 21H19A1 1 0 0 0 20 20V16",
        Icon::Leaf => "M20 3C7 2 1 8 5 16C13 23 21 15 20 3Z M4 21L15 9",
        Icon::Stop => "M6 4H18A2 2 0 0 1 20 6V18A2 2 0 0 1 18 20H6A2 2 0 0 1 4 18V6A2 2 0 0 1 6 4Z",
        Icon::Minimize => "M5 12H19",
        Icon::Maximize => "M5 5H19V19H5Z",
        Icon::Restore => "M8 4H20V16 M4 8H16V20H4Z",
        Icon::Translate => "M3 5H14 M8 3V5 M5 8C7 12 10 15 14 16 M12 5C11 11 8 15 3 17 M14 21L18 10L22 21 M15.5 17H20.5",
        Icon::Heart => "M12 21L3.5 12C-1 6 6 0 12 6C18 0 25 6 20.5 12Z",
        Icon::Move => "M12 2V22 M2 12H22 M8 6L12 2L16 6 M8 18L12 22L16 18 M6 8L2 12L6 16 M18 8L22 12L18 16",
    };
    // The paths are constant literals, parsed once per glyph and retained.
    use std::sync::OnceLock;
    static PATHS: OnceLock<std::sync::Mutex<std::collections::HashMap<String, BezPath>>> =
        OnceLock::new();
    let paths = PATHS.get_or_init(Default::default);
    let mut cache = paths.lock().unwrap_or_else(|e| e.into_inner());
    let path = cache
        .entry(data.into())
        .or_insert_with(|| BezPath::from_svg(data).expect("valid control icon path"));
    let stroke = Stroke::new(1.75)
        .with_caps(Cap::Round)
        .with_join(Join::Round);
    scene.stroke(
        &stroke,
        Affine::translate((x, y)) * Affine::scale(size / 24.),
        draw::color(rgb),
        None,
        path,
    );
}
