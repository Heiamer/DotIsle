use crate::{apple_desktop, draw::{self,rect,Text}, model::{Action,Model}, views::{TodoEdit,View,ViewState}, widgets::Ui};
use dotisle_domain::desktop_widgets::{Date,Kind,Size,Style};
pub use crate::views::TodoField;

pub const HIDDEN_BAR: f64 = 12.;
pub const TODO_ROW: f64 = 55.;

/// Visible rows and maximum scroll offset for the current window and preset.
pub fn todo_metrics(_width: f64, height: f64, size: Size, count: usize) -> (usize, f64) {
    let card_h = (height - 16.).max(1.);
    let large = size == Size::Large;
    let small = size == Size::Small;
    let top = 15. + 27.;
    let bottom = if large { card_h - 50. } else { card_h - 6. };
    let visible = if small {
        1
    } else {
        (((bottom - top) / TODO_ROW).floor().max(1.)) as usize
    };
    let max = (count as f64 * TODO_ROW - (bottom - top) - 2.).max(0.);
    (visible, max)
}

pub fn date() -> Date {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    Date {
        year: t.wYear as i32,
        month: t.wMonth as u32,
        day: t.wDay as u32,
        hour: t.wHour as u32,
        minute: t.wMinute as u32,
        second: t.wSecond as u32,
    }
}
pub fn valid_date(s: &str) -> bool {
    let b = s.as_bytes();
    if b.len() != 10 || b[4] != b'-' || b[7] != b'-' || !b.iter().enumerate().all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit()) {
        return false;
    }
    let y = s[0..4].parse::<u32>().unwrap_or(0);
    let m = s[5..7].parse::<u32>().unwrap_or(0);
    let d = s[8..10].parse::<u32>().unwrap_or(0);
    let max = match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => if y % 400 == 0 || (y % 4 == 0 && y % 100 != 0) { 29 } else { 28 },
        _ => 0,
    };
    (1900..=9999).contains(&y) && d > 0 && d <= max
}
pub fn valid_time(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 5 && b[2] == b':' && b.iter().enumerate().all(|(i, c)| i == 2 || c.is_ascii_digit()) && s[..2].parse::<u32>().unwrap_or(99) < 24 && s[3..].parse::<u32>().unwrap_or(99) < 60
}
/// Field rectangles shared by painting and IME placement.
pub fn editor_field_rect(width: f64, field: TodoField) -> vello::kurbo::Rect {
    let x = 34.;
    let fw = (width - 68.).max(80.);
    let half = ((fw - 8.) / 2.).max(40.);
    match field {
        TodoField::Title => rect(x, 103., fw, 44.),
        TodoField::Date => rect(x, 153., half, 34.),
        TodoField::Time => rect(x + 8. + half, 153., half, 34.),
    }
}
fn style_scheme(_style: Style, dark: bool) -> dotisle_domain::monet::Scheme {
    let mut s=dotisle_domain::monet::scheme(0x0088FF,false);
    s.surface=0xF5F5F7;s.container=0xFFFFFF;s.high=0xF0F0F2;s.on_surface=0x1D1D1F;s.on_surface_variant=0x86868B;s.primary=0x0088FF;s.on_primary=0xFFFFFF;
    if dark{s.surface=0x1C1C1E;s.container=0x2C2C2E;s.high=0x3A3A3C;s.on_surface=0xF5F5F7;s.on_surface_variant=0xABABB0;s.primary=0x0A84FF;}
    s
}
pub fn render(
    m: &Model,
    text: &mut Text,
    kind: Kind,
    size: Size,
    id: &str,
    width: f64,
    height: f64,
    pointer: (f64, f64),
    menu: bool,
    hidden: bool,
    scroll: f64,
    editor: Option<&TodoEdit>,
    time: f64,
    media_position: Option<f64>,
) -> View {
    let (w, h) = (width.max(1.), height.max(1.));
    let mut scene = vello::Scene::new();
    let s = style_scheme(Style::Original,m.prefs.dark);
    if hidden {
        let mut ui = Ui::new(&mut scene, text, s, pointer, rect(0.,0.,w,h));
        let bar_color = 0xC0000000 | (s.primary & 0x00FFFFFF);
        draw::fill_rounded(ui.scene, rect(0., 0., w, h), (h / 2.).min(8.), bar_color);
        ui.center(kind.label(), rect(0., 0., w, h), 8.5, s.on_primary, true);
        ui.hit(rect(0., 0., w, h), "展开组件", Action::WidgetRestore(id.to_owned()), true);
        return View { hits: ui.hits, scene };
    }
    let mut ui = Ui::new(&mut scene, text, s, pointer, rect(0.,0.,w,h));
    let state = ViewState { time, media_position, scroll, editor: editor.cloned() };
    apple_desktop::render(&mut ui,m,kind,rect(8.,8.,w-16.,h-16.),size,id,&state);
    if menu {
        let bar = rect(8., h - 100., w - 16., 92.);
        draw::fill(ui.scene, bar, 16., s.high);
        let cw = (bar.width() - 16.) / 3.;
        for (i, choice) in Size::ALL.iter().enumerate() {
            ui.button(
                rect(16. + i as f64 * cw, h - 94., cw - 4., 36.),
                choice.label(),
                None,
                Action::WidgetResize(id.to_owned(), *choice),
                true,
                true,
            );
        }
        let action_width = (bar.width() - 20.) / 3.;
        ui.button(
            rect(16., h - 52., action_width, 36.),
            "编辑",
            None,
            Action::WidgetConfigure(id.to_owned()),
            true,
            true,
        );
        ui.button(
            rect(18. + action_width, h - 52., action_width, 36.),
            "收起",
            None,
            Action::WidgetHide(id.to_owned()),
            true,
            true,
        );
        ui.button(
            rect(20. + action_width * 2., h - 52., action_width, 36.),
            "移除",
            None,
            Action::WidgetRemove(id.to_owned()),
            true,
            true,
        );
    }
    View {
        hits: ui.hits,
        scene,
    }
}