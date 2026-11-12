//! Gallery (留白): monochrome surfaces, editorial serif titles and generous spacing.
use crate::{
    draw::{self, rect, Icon},
    model::{Action, Model, Page},
    views::{self, ViewState},
    widgets::Ui,
};
use dotisle_domain::monet::Scheme;
use vello::{kurbo::Rect, Scene};
pub fn active(m: &Model) -> bool {
    matches!(m.prefs.control_center_theme.as_str(), "Gallery" | "Swiss")
}
pub fn scheme(dark: bool) -> Scheme {
    if dark {
        Scheme {
            primary: 0xF9F9F7,
            on_primary: 0x151515,
            primary_container: 0x1B1B1A,
            on_primary_container: 0xF5F5F3,
            secondary_container: 0x232321,
            on_secondary_container: 0xF5F5F3,
            tertiary_container: 0x1E1E1C,
            on_tertiary_container: 0xF5F5F3,
            surface: 0x111110,
            container: 0x1A1A19,
            high: 0x282826,
            on_surface: 0xF5F5F3,
            on_surface_variant: 0xAEAEAA,
            outline: 0x777773,
            outline_variant: 0x393937,
            error: 0xE88686,
        }
    } else {
        Scheme {
            primary: 0x1A1A1A,
            on_primary: 0xFFFFFF,
            primary_container: 0xFFFFFF,
            on_primary_container: 0x1A1A1A,
            secondary_container: 0xF0F0ED,
            on_secondary_container: 0x1A1A1A,
            tertiary_container: 0xF4F4F1,
            on_tertiary_container: 0x1A1A1A,
            surface: 0xFAFAFA,
            container: 0xFFFFFF,
            high: 0xF0F0EE,
            on_surface: 0x1A1A1A,
            on_surface_variant: 0x686865,
            outline: 0x999995,
            outline_variant: 0xDEDEDA,
            error: 0xAD3535,
        }
    }
}
pub fn page(p: Page) -> (&'static str, &'static str) {
    match p {
        Page::Home => ("OVERVIEW", "01"),
        Page::Island => ("THE ISLAND", "02"),
        Page::Music => ("NOW PLAYING", "03"),
        Page::Widgets => ("DESKTOP WIDGETS", "03"),
        Page::Focus => ("FOCUS / TIME", "04"),
        Page::Todos => ("THINGS TO DO", "05"),
        Page::Notifications => ("NOTIFICATIONS", "05"),
        Page::Settings => ("PREFERENCES", "06"),
    }
}
pub fn panel(scene: &mut Scene, r: Rect, bg: u32, edge: u32) {
    draw::fill(scene, r, 22., bg);
    draw::outline(scene, r, 22., edge, 0.75);
}
pub fn header(ui: &mut Ui, m: &Model, x: f64, w: f64) {
    let s = ui.s;
    let (eyebrow, _) = page(m.page);
    ui.center(
        eyebrow,
        rect(x, 58., w, 18.),
        10.,
        s.on_surface_variant,
        false,
    );
    ui.center(
        m.page.label(),
        rect(x, 78., w, 36.),
        30.,
        s.on_surface,
        true,
    );
    let description = if m.page == Page::Home {
        "音乐、时间与小事，留一处安静的空间。"
    } else {
        m.page.description()
    };
    ui.center(
        description,
        rect(x, 120., w, 20.),
        12.,
        s.on_surface_variant,
        false,
    );
}
pub fn icon(scene: &mut Scene, i: Icon, x: f64, y: f64, size: f64, color: u32) {
    draw::icon(scene, i, x, y, size, color);
}
fn section(ui: &mut Ui, title: &str, x: f64, y: f64, w: f64) {
    let s = ui.s;
    draw::fill(ui.scene, rect(x, y, w, 2.), 0., s.on_surface);
    ui.label(title, x, y + 10., 12., s.primary, true, w);
}
pub fn music_head(ui: &mut Ui, m: &Model, x: f64, y: f64, w: f64, state: &ViewState) {
    let s = ui.s;
    section(ui, "01 / THE RECORD", x, y, w);
    let cover = if w > 900. { 288. } else { 224. };
    let px = x + cover + 32.;
    let avail = w - cover - 32.;
    views::record(ui, m, rect(x, y + 38., cover, cover), false);
    ui.label(
        if m.track() {
            "NOW PLAYING"
        } else {
            "WAITING FOR MUSIC"
        },
        px,
        y + 39.,
        12.,
        s.primary,
        true,
        avail,
    );
    ui.label(
        m.title(),
        px,
        y + 75.,
        if avail > 500. { 44. } else { 32. },
        s.on_surface,
        true,
        avail,
    );
    ui.label(
        m.artist(),
        px,
        y + 141.,
        17.,
        s.on_surface_variant,
        false,
        avail,
    );
    views::progress(ui, m, rect(px, y + 195., avail, 24.), state);
    views::player_controls(ui, m, px, y + 253., true);
}

pub fn music_card(ui: &mut Ui, m: &Model, w: f64, h: f64, state: &ViewState) {
    let s = ui.s;
    let cover = if h < 300. { 142. } else { 174. };
    views::record(ui, m, rect(w - cover - 28., 68., cover, cover), false);
    let max = w - cover - 88.;
    ui.label(m.title(), 24., 72., 30., s.on_primary_container, true, max);
    ui.label(m.artist(), 26., 116., 14., s.on_surface_variant, false, max);
    ui.label(
        m.current_lyric(),
        26.,
        149.,
        14.,
        s.on_surface_variant,
        false,
        max,
    );
    views::player_controls(ui, m, 18., h - 110., false);
    views::progress(ui, m, rect(246., h - 98., w - cover - 306., 24.), state);
    ui.label(
        "歌词同步",
        26.,
        h - 34.,
        11.,
        s.on_surface_variant,
        false,
        66.,
    );
    ui.icon_button(
        rect(98., h - 46., 36., 36.),
        Icon::Back,
        "歌词提前 100 毫秒",
        Action::Offset(-100),
        false,
        true,
    );
    ui.center(
        &format!("{:+} ms", m.prefs.offset_ms),
        rect(138., h - 46., 88., 36.),
        12.,
        s.on_primary_container,
        true,
    );
    ui.icon_button(
        rect(230., h - 46., 36., 36.),
        Icon::Arrow,
        "歌词延后 100 毫秒",
        Action::Offset(100),
        false,
        true,
    );
    ui.button(
        rect(w - 208., h - 48., 184., 36.),
        "布局与播放组件",
        Some(Icon::Settings),
        Action::LayoutOpen,
        true,
        true,
    );
}
