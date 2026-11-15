//! Six native desktop cards, with one data model and one command path.
use crate::{
    draw::{self, rect, Icon, Text},
    model::{Action, Model, Page},
    views::{self, View, ViewState},
    widgets::Ui,
};
use dotisle_domain::desktop_widgets::{Date, Kind, Metrics, Size, Style};
use serde_json::json;
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
pub fn card(
    ui: &mut Ui,
    m: &Model,
    kind: Kind,
    r: vello::kurbo::Rect,
    size: Size,
    state: &ViewState,
) {
    crate::apple_desktop::render(ui,m,kind,r,size,"preview",state);
}
pub fn gallery(ui: &mut Ui, m: &Model, x: f64, mut y: f64, w: f64, state: &ViewState) -> f64 {
    let s = ui.s;
    ui.label(
        &format!("每种组件，{} 套独立风格", Style::ALL.len()),
        x,
        y,
        22.,
        s.on_surface,
        true,
        w - 8.,
    );
    y += 38.;
    ui.label(
        "直接挑选喜欢的样式；控制中心切换主题时，桌面上的组件保持原样。",
        x,
        y,
        12.,
        s.on_surface_variant,
        false,
        w - 8.,
    );
    y += 42.;
    let cols = if w >= 1000. { 3 } else { 2 };
    let cw = (w - (cols - 1) as f64 * 18.) / cols as f64;
    for kind in Kind::ALL {
        let size = m.widget_sizes[kind.index()];
        ui.label(kind.label(), x, y + 6., 21., s.on_surface, true, w - 290.);
        for (j, choice) in Size::ALL.iter().enumerate() {
            ui.chip(
                rect(x + w - 230. + j as f64 * 78., y, 70., 34.),
                choice.label(),
                size == *choice,
                Action::WidgetSize(kind, *choice),
            );
        }
        y += 50.;
        for (i, style) in Style::ALL.iter().enumerate() {
            let left = x + (i % cols) as f64 * (cw + 18.);
            let top = y + (i / cols) as f64 * 266.;
            if top + 250. < ui.clip.y0 || top > ui.clip.y1 {
                continue;
            }
            ui.panel(rect(left, top, cw, 250.), s.high);
            let (pw, ph) = size.dimensions();
            let scale = ((cw - 32.) / pw).min(150. / ph);
            let preview = render_style(
                m,
                ui.text,
                kind,
                size,
                *style,
                "preview",
                (-1., -1.),
                false,
                state.time,
                state.media_position,
            );
            ui.scene.append(
                &preview.scene,
                Some(
                    vello::kurbo::Affine::translate((left + (cw - pw * scale) / 2., top + 12.))
                        * vello::kurbo::Affine::scale(scale),
                ),
            );
            ui.label(
                style.label(),
                left + 18.,
                top + 172.,
                15.,
                s.on_surface,
                true,
                cw - 36.,
            );
            ui.button(
                rect(left + 16., top + 206., cw - 32., 32.),
                "添加到桌面",
                Some(Icon::Plus),
                Action::WidgetAddStyle(kind, size, *style),
                false,
                m.desktop_widgets.items.len() < 16,
            );
        }
        y += (Style::ALL.len().div_ceil(cols)) as f64 * 266. + 24.;
    }

    if let Some(e) = &m.desktop_widgets.error {
        ui.label(e, x, y, 13., s.error, false, w);
        y += 44.;
    }
    ui.label(
        &format!("桌面上的组件 · {}", m.desktop_widgets.items.len()),
        x,
        y,
        21.,
        s.on_surface,
        true,
        w,
    );
    y += 48.;
    for p in &m.desktop_widgets.items {
        ui.panel(rect(x, y, w, 76.), s.container);
        ui.label(
            &format!("{} · {}", p.kind.label(), p.style.label()),
            x + 22.,
            y + 16.,
            17.,
            s.on_surface,
            true,
            w - 430.,
        );
        ui.label(
            "拖动调整位置 · 右键编辑、调整尺寸或移除",
            x + 22.,
            y + 46.,
            11.,
            s.on_surface_variant,
            false,
            w - 430.,
        );
        for (i, size) in Size::ALL.iter().enumerate() {
            ui.chip(
                rect(x + w - 340. + i as f64 * 76., y + 18., 68., 38.),
                size.label(),
                p.size == *size,
                Action::WidgetResize(p.id.clone(), *size),
            );
        }
        ui.icon_button(
            rect(x + w - 62., y + 16., 44., 44.),
            Icon::Trash,
            "移除桌面组件",
            Action::WidgetRemove(p.id.clone()),
            false,
            true,
        );
        y += 90.;
    }
    y
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
    let style = m
        .desktop_widgets
        .items
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.style)
        .unwrap_or_default();
    render_style(m, text, kind, size, style, id, pointer, menu, time, media_position)
}
fn style_scheme(_style: Style, dark: bool) -> dotisle_domain::monet::Scheme {
    let mut s=dotisle_domain::monet::scheme(0x0088FF,false);
    s.surface=0xF5F5F7;s.container=0xFFFFFF;s.high=0xF0F0F2;s.on_surface=0x1D1D1F;s.on_surface_variant=0x86868B;s.primary=0x0088FF;s.on_primary=0xFFFFFF;
    if dark{s.surface=0x1C1C1E;s.container=0x2C2C2E;s.high=0x3A3A3C;s.on_surface=0xF5F5F7;s.on_surface_variant=0xABABB0;s.primary=0x0A84FF;}
    s
}
pub(crate) fn render_style(
    m: &Model,
    text: &mut Text,
    kind: Kind,
    size: Size,
    style: Style,
    id: &str,
    pointer: (f64, f64),
    menu: bool,
    time: f64,
    media_position: Option<f64>,
) -> View {
    let (w, h) = size.dimensions();
    let mut scene = vello::Scene::new();
    let s = style_scheme(style,m.prefs.dark);
    let face=Some("HarmonyOS Sans SC".into());
    let mut ui = Ui {
        selection_values: Default::default(),
        motion_targets: vec![],
        press_layers: vec![],
        switches: Default::default(),
        state_layers: vec![],
        scene: &mut scene,
        text,
        s,
        hits: vec![],
        pointer,
        focus: None,
        pressed: None,
        clip: rect(0., 0., w, h),
        miuix: false,
        terminal: false,
        pixel: false,
        swiss: false,
        face,
    };

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
        overlay_scene: None,
        motion_targets: ui.motion_targets,
        hits: ui.hits,
        scene,
        max_scroll: 0.,
    }
}
impl Model {
    pub(crate) fn widget_action(&mut self, a: &Action) -> bool {
        let result = match a {
            Action::QuickFocus => {
                self.quick_focus();
                return true;
            }
            Action::NoticeFilter(f) => {
                self.notice_filter = (*f).min(4);
                self.scroll = 0.;
                return true;
            }
            Action::WidgetSize(k, s) => {
                self.widget_sizes[k.index()] = *s;
                return true;
            }
            Action::WidgetConfigure(id) => {
                self.edit_widget_config(id);
                return true;
            }
            Action::WidgetLaunch(id) => {
                self.native_command("widgetLaunch", json!({"id":id}));
                return true;
            }
            Action::WidgetFocus => return true,
            Action::WidgetAdd(k, s) => {
                let v = json!({"kind":k,"size":s});
                if self.remote.is_some() {
                    self.native_command("widgetAdd", v);
                    return true;
                }
                self.desktop_widgets.add(&v).map(|_| ())
            }
            Action::WidgetAddStyle(k, s, style) => {
                let v = json!({"kind":k,"size":s,"style":style});
                if self.remote.is_some() {
                    self.native_command("widgetAdd", v);
                    return true;
                }
                self.desktop_widgets.add(&v).map(|_| ())
            }
            Action::WidgetResize(id, s) => {
                let v = json!({"id":id,"size":s});
                if self.remote.is_some() {
                    self.native_command("widgetUpdate", v);
                    return true;
                }
                self.desktop_widgets.update(&v)
            }
            Action::WidgetRemove(id) => {
                if self.remote.is_some() {
                    self.native_command("widgetRemove", json!({"id":id}));
                    return true;
                }
                self.desktop_widgets.remove(id)
            }
            _ => return false,
        };
        if let Err(e) = result {
            self.message(e);
        }
        true
    }
}
