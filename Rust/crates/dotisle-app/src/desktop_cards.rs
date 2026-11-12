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
    let s = ui.s;
    if crate::widget_extras::render(ui, m, kind, r, size, "preview") {
        return;
    }
    ui.panel(
        r,
        if kind == Kind::Focus {
            s.tertiary_container
        } else if kind == Kind::Music {
            s.primary_container
        } else {
            s.container
        },
    );
    let x = r.x0 + 16.;
    let y = r.y0 + 16.;
    let w = r.width() - 32.;
    let h = r.height();
    let big = size == Size::Large;
    let small = size == Size::Small;
    ui.label(kind.label(), x, y, 12., s.on_surface_variant, true, w);
    match kind {
        Kind::Clock => {
            let d = date();
            ui.label(
                &format!("{:02}:{:02}", d.hour, d.minute),
                x,
                y + 31.,
                if small { 34. } else { 48. },
                s.on_surface,
                true,
                w,
            );
            ui.label(
                &format!("{}月{}日 · {}", d.month, d.day, d.weekday_label()),
                x,
                y + if small { 88. } else { 102. },
                11.,
                s.on_surface_variant,
                false,
                w,
            );
            if big {
                let cx = r.center().x;
                let cy = r.y0 + 239.;
                draw::circle(ui.scene, cx, cy, 66., s.high);
                for i in 0..12 {
                    let a = i as f64 * std::f64::consts::TAU / 12.;
                    draw::line(
                        ui.scene,
                        (cx + a.sin() * 57., cy - a.cos() * 57.),
                        (cx + a.sin() * 61., cy - a.cos() * 61.),
                        s.on_surface_variant,
                        2.,
                    );
                }
                for (angle, len, width, color) in [
                    (
                        ((d.hour % 12) as f64 + d.minute as f64 / 60.) * std::f64::consts::TAU
                            / 12.,
                        34.,
                        4.,
                        s.on_surface,
                    ),
                    (
                        d.minute as f64 * std::f64::consts::TAU / 60.,
                        49.,
                        3.,
                        s.primary,
                    ),
                ] {
                    draw::line(
                        ui.scene,
                        (cx, cy),
                        (cx + angle.sin() * len, cy - angle.cos() * len),
                        color,
                        width,
                    );
                }
                draw::circle(ui.scene, cx, cy, 4., s.primary);
            }
        }
        Kind::Calendar => {
            let d = date();
            ui.label(
                &format!("{} 年 {} 月", d.year, d.month),
                x,
                y + 23.,
                if small { 13. } else { 18. },
                s.primary,
                true,
                w,
            );
            let cw = w / 7.;
            let ch = if big { 35. } else { 14. };
            let top = y + if big { 70. } else { 47. };
            for (i, label) in ["一", "二", "三", "四", "五", "六", "日"]
                .iter()
                .enumerate()
            {
                ui.text.line_fixed(
                    ui.scene,
                    label,
                    rect(x + i as f64 * cw, top, cw, ch),
                    10.,
                    0xff000000 | s.on_surface_variant,
                    400,
                    0.5,
                );
            }
            for day in 1..=d.days() {
                let n = d.first_weekday() + day as usize - 1;
                let rr = rect(
                    x + (n % 7) as f64 * cw,
                    top + ch + (n / 7) as f64 * ch,
                    cw,
                    ch,
                );
                if day == d.day {
                    draw::fill(ui.scene, rr.inset((1., 0.)), ch / 2., s.primary);
                }
                ui.text.line_fixed(
                    ui.scene,
                    &day.to_string(),
                    rr,
                    if big { 15. } else { 10. },
                    0xff000000
                        | if day == d.day {
                            s.on_primary
                        } else {
                            s.on_surface
                        },
                    if day == d.day { 600 } else { 400 },
                    0.5,
                );
            }
        }
        Kind::Todos => {
            let now = dotisle_domain::todos::Now::local();
            let mut items = m
                .todos
                .items
                .iter()
                .filter(|t| !t.done(&now))
                .collect::<Vec<_>>();
            items.sort_by_key(|t| (&t.date, &t.time));
            ui.label(
                &format!("{} 件小事", items.len()),
                x,
                y + 24.,
                if small { 22. } else { 27. },
                s.on_surface,
                true,
                w,
            );
            if items.is_empty() {
                ui.label(
                    "清爽的一天",
                    x,
                    y + 71.,
                    14.,
                    s.on_surface_variant,
                    false,
                    w,
                );
            }
            for (i, t) in items.iter().take(if big { 4 } else { 1 }).enumerate() {
                let ty = y + 67. + i as f64 * 50.;
                ui.icon_button(
                    rect(x - 8., ty, 36., 36.),
                    Icon::Check,
                    "完成待办",
                    Action::ToggleTodo(t.id.clone()),
                    false,
                    true,
                );
                ui.label(&t.title, x + 33., ty + 7., 13., s.on_surface, true, w - 37.);
            }
            ui.button(
                rect(x, r.y1 - 44., w, 30.),
                "添加待办",
                Some(Icon::Plus),
                Action::EditTodo(None),
                true,
                true,
            );
        }
        Kind::Focus => {
            let t = m.current_timer();
            ui.label(
                &t.map(|t| t.clock()).unwrap_or("25:00".into()),
                x,
                y + 27.,
                if small { 32. } else { 43. },
                s.on_surface,
                true,
                w,
            );
            if big {
                ui.label(
                    &t.map(|t| t.subtitle())
                        .unwrap_or("25 分钟专注 · 5 分钟休息".into()),
                    x,
                    y + 111.,
                    13.,
                    s.on_surface_variant,
                    false,
                    w,
                );
            }
            let label = t
                .map(|t| match t.primary_action() {
                    "pause" => "暂停",
                    "resume" => "继续",
                    "next" => "下一阶段",
                    _ => "重新开始",
                })
                .unwrap_or("开始专注");
            let primary = t
                .map(|t| Action::Timer(t.id.clone(), t.primary_action()))
                .unwrap_or(Action::QuickFocus);
            ui.button(
                rect(x, r.y1 - 53., w - if small { 0. } else { 44. }, 38.),
                label,
                Some(if t.is_some_and(|t| t.running) {
                    Icon::Pause
                } else {
                    Icon::Play
                }),
                primary,
                false,
                true,
            );
            if !small {
                ui.icon_button(
                    rect(r.x1 - 52., r.y1 - 55., 38., 38.),
                    Icon::Reset,
                    "重置计时",
                    t.map(|t| Action::Timer(t.id.clone(), "reset"))
                        .unwrap_or(Action::Preset(25)),
                    false,
                    t.is_some(),
                );
            }
        }
        Kind::Music => {
            let cover = if small { 42. } else { 64. };
            views::record(ui, m, rect(x, y + 27., cover, cover), false);
            let tx = if small { x } else { x + cover + 12. };
            let ty = if small { y + 73. } else { y + 30. };
            ui.label(
                m.title(),
                tx,
                ty,
                if small { 13. } else { 17. },
                s.on_primary_container,
                true,
                if small { w } else { w - cover - 12. },
            );
            if !small {
                ui.label(
                    m.artist(),
                    tx,
                    ty + 30.,
                    12.,
                    s.on_primary_container,
                    false,
                    w - cover - 12.,
                );
            }
            if big {
                ui.label(
                    m.current_lyric(),
                    x,
                    y + 125.,
                    16.,
                    s.on_primary_container,
                    false,
                    w,
                );
                views::progress(ui, m, rect(x, y + 185., w, 24.), state);
            }
            let by = r.y1 - 49.;
            for (i, (icon, label, action)) in [
                (Icon::Prev, "上一首", "previous"),
                (
                    if m.playing() { Icon::Pause } else { Icon::Play },
                    "播放或暂停",
                    "toggle",
                ),
                (Icon::Next, "下一首", "next"),
            ]
            .iter()
            .enumerate()
            {
                let bw = if small { 36. } else { 42. };
                let gap = if small { 8. } else { 18. };
                let total = 3. * bw + 2. * gap;
                views::transport_control(
                    ui,
                    m,
                    rect(
                        r.center().x - total / 2. + i as f64 * (bw + gap),
                        by,
                        bw,
                        bw,
                    ),
                    [3, 4, 5][i],
                    label,
                    action,
                );
                let _ = icon;
            }
        }
        Kind::System => {
            let metrics: Metrics =
                serde_json::from_value(m.native_state["systemMetrics"].clone()).unwrap_or_default();
            let entries = [
                ("CPU", metrics.cpu_percent),
                ("内存", metrics.memory_percent),
                ("电池", metrics.battery_percent.map(|v| v as f64)),
            ];
            for (i, (title, value)) in entries.iter().take(if small { 2 } else { 3 }).enumerate() {
                let ty = y + 31. + i as f64 * if big { 78. } else { 34. };
                ui.label(title, x, ty, 12., s.on_surface_variant, false, 65.);
                ui.label(
                    &value
                        .map(|v| format!("{v:.0}%"))
                        .unwrap_or("暂无读数".into()),
                    x + if small { 54. } else { 82. },
                    ty,
                    if big { 22. } else { 15. },
                    s.on_surface,
                    true,
                    w - 60.,
                );
                if big {
                    draw::fill(ui.scene, rect(x, ty + 36., w, 7.), 3., s.high);
                    if let Some(v) = value {
                        draw::fill(
                            ui.scene,
                            rect(x, ty + 36., w * v.clamp(0., 100.) / 100., 7.),
                            3.,
                            s.primary,
                        );
                    }
                }
            }
            if big {
                ui.label(
                    &match (metrics.memory_used_gib, metrics.memory_total_gib) {
                        (Some(a), Some(b)) => format!("内存 {a:.1} / {b:.1} GiB"),
                        _ => "等待系统读数".into(),
                    },
                    x,
                    r.y1 - 43.,
                    12.,
                    s.on_surface_variant,
                    false,
                    w,
                );
            }
        }
        _ => {}
    }
    let _ = h;
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
) -> View {
    let style = m
        .desktop_widgets
        .items
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.style)
        .unwrap_or_default();
    render_style(m, text, kind, size, style, id, pointer, menu, time)
}
fn style_scheme(style: Style) -> dotisle_domain::monet::Scheme {
    match style {
        Style::Original => dotisle_domain::monet::scheme(0x5264a8, false),
        Style::Miuix => crate::miuix::scheme(false),
        Style::Terminal => crate::terminal::scheme(true),
        Style::Pixel => crate::pixel::scheme(false),
        Style::Gallery => crate::swiss::scheme(false),
    }
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
) -> View {
    let (w, h) = size.dimensions();
    let mut scene = vello::Scene::new();
    let s = style_scheme(style);
    let face = match style {
        Style::Original => None,
        Style::Miuix => {
            Some(Text::system_family("MiSans").unwrap_or_else(|| "Noto Sans SC".into()))
        }
        Style::Terminal => Some("Space Mono".into()),
        Style::Pixel => Some(crate::pixel::FACE.into()),
        Style::Gallery => Some("Google Sans Flex".into()),
    };
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
        miuix: style == Style::Miuix,
        terminal: style == Style::Terminal,
        pixel: style == Style::Pixel,
        swiss: style == Style::Gallery,
        face,
    };

    if !crate::widget_extras::render(&mut ui, m, kind, rect(8., 8., w - 16., h - 16.), size, id) {
        card(
            &mut ui,
            m,
            kind,
            rect(8., 8., w - 16., h - 16.),
            size,
            &ViewState {
                time,
                ..Default::default()
            },
        );
    }
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
