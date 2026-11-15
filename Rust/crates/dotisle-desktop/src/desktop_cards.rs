use crate::{draw::{self,rect,Text},model::{Model,Action},views::{View,ViewState},widgets::Ui};
use dotisle_domain::desktop_widgets::{Date,Kind,Size,Style};
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
    pointer: (f64, f64),
    menu: bool,
    time: f64,
    media_position: Option<f64>,
) -> View {
    let (w, h) = size.dimensions();
    let mut scene = vello::Scene::new();
    let s = style_scheme(Style::Original,m.prefs.dark);
    let mut ui = Ui::new(&mut scene, text, s, pointer, rect(0.,0.,w,h));

    crate::apple_desktop::render(&mut ui,m,kind,rect(8.,8.,w-16.,h-16.),size,id,&ViewState{time,media_position,..Default::default()});
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
        let action_width = (bar.width() - 20.) / 2.;
        ui.button(
            rect(16., h - 52., action_width, 36.),
            "编辑",
            None,
            Action::WidgetConfigure(id.to_owned()),
            true,
            true,
        );
        ui.button(
            rect(20. + action_width, h - 52., action_width, 36.),
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