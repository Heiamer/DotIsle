//! An infinite rolling deck: periodic card identities, continuous motion coordinates.
use crate::{
    draw::{self, rect, Icon},
    model::{Action, Model, Page},
    views::{self, ViewState},
    widgets::Ui,
};
pub use dotisle_domain::card_ring::{self as ring, STEP};
use serde_json::json;
use vello::{kurbo::Affine, peniko::Mix};
pub const TITLES: [&str; 5] = ["音乐", "专注", "待办", "通知中心", "桌面组件"];
pub fn render(
    ui: &mut Ui,
    m: &Model,
    x: f64,
    _y: f64,
    w: f64,
    state: &ViewState,
    backdrop: &vello::Scene,
) -> f64 {
    let x = x + (w - w.min(980.)) / 2.;
    let w = w.min(980.);
    let clip = ui.clip;
    let top = clip.y0 + 54.;
    let h = crate::center_layout::deck_card_height(clip.y1 + crate::center_layout::NAV_BOTTOM);
    let progress = ring::phase(m.scroll);
    let base = progress.floor() as i64;
    let mut order = (base - 2..=base + 5).collect::<Vec<_>>();
    order.sort_by(|a, b| ((*b as f64 - progress).abs()).total_cmp(&(*a as f64 - progress).abs()));
    for slot in order {
        let i = ring::slot_index(slot);
        let d = slot as f64 - progress;
        let visibility = (2. - d.abs()).clamp(0., 1.);
        if visibility <= 0. {
            continue;
        }
        let dy = if d < 0. {
            d * 56.
        } else if d <= 1. {
            d * (h - 16.)
        } else {
            h - 16. + (d - 1.) * 64.
        };
        let scale = 1. - 0.035 * d.abs().min(3.);
        let tx = x + w * (1. - scale) / 2.;
        let ty = top + dy;
        let visual_h = if d.abs() >= 1. {
            72.
        } else {
            h + (72. - h) * d.abs().powi(3)
        };
        let bounds = rect(tx, ty, w * scale, visual_h * scale);
        if bounds.y1 < clip.y0 || bounds.y0 > clip.y1 {
            continue;
        }
        let tr = Affine::translate((tx, ty)) * Affine::scale(scale);
        let inverse = tr.inverse();
        let pointer = ui.pointer;
        let old = std::mem::take(ui.scene);
        let start = ui.hits.len();
        ui.pointer = (inverse * vello::kurbo::Point::from(pointer)).into();
        ui.clip = inverse.transform_rect_bbox(clip);
        let s = ui.s;
        let card_h = if d.abs() >= 1. {
            68.
        } else {
            h + (68. - h) * d.abs().powi(3)
        };
        let r = rect(0., 0., w, card_h);
        let radius = 28.;
        let art = if i == 0 { m.artwork.as_ref() } else { None };
        let mut card_scheme = if let Some(a) = art {
            dotisle_domain::monet::scheme(a.seed.unwrap_or(0x5264a8), true)
        } else {
            s
        };
        card_scheme.primary_container = card_scheme.container;
        card_scheme.on_primary_container = card_scheme.on_surface;
        card_scheme.tertiary_container = card_scheme.container;
        card_scheme.on_tertiary_container = card_scheme.on_surface;
        card_scheme.secondary_container = card_scheme.high;
        card_scheme.on_secondary_container = card_scheme.on_surface;
        ui.s = card_scheme;
        crate::acrylic::surface(
            ui,
            r,
            radius,
            backdrop,
            inverse,
            m.prefs.dark || art.is_some(),
        );
        if let Some(a) = art {
            crate::acrylic::album(ui, r, radius, &a.image);
        }
        // Blank space is a direct-manipulation target; child controls take precedence.
        ui.hit(
            rect(0., 0., w, h),
            "上下拖动浏览卡片",
            Action::OverviewDrag,
            true,
        );
        let fg = ui.s.on_surface;
        let body = (1. - d.abs() * 1.2).clamp(0., 1.);
        let hy = if d < 0. {
            20. - 4. * d.abs().min(1.)
        } else {
            20. + d.abs().min(1.) * 16.
        };
        ui.label(TITLES[i], 24., hy, 18., fg, true, w - 48.);
        if body > 0.001 {
            ui.scene.push_layer(
                Mix::Normal,
                body as f32,
                Affine::IDENTITY,
                &rect(0., 60., w, h - 60.),
            );
            content(ui, m, i, w, h, state);
            ui.scene.pop_layer();
        }
        ui.s = s;
        let card = std::mem::replace(ui.scene, old);
        ui.clip = clip;
        ui.pointer = pointer;

        if visibility < 1. {
            ui.scene
                .push_layer(Mix::Normal, visibility as f32, Affine::IDENTITY, &clip);
        }
        ui.scene.append(&card, Some(tr));
        if visibility < 1. {
            ui.scene.pop_layer();
        }
        if d.abs() > 0.46 {
            ui.hits.truncate(start);
            ui.hit(
                bounds.intersect(clip),
                &format!("浏览{}卡片", TITLES[i]),
                Action::OverviewCard(i),
                true,
            );
        } else {
            for hit in &mut ui.hits[start..] {
                hit.rect = tr.transform_rect_bbox(hit.rect).intersect(clip);
            }
        }
    }
    clip.y1
}
fn content(ui: &mut Ui, m: &Model, i: usize, w: f64, h: f64, state: &ViewState) {
    let s = ui.s;
    match i {
        0 => {
            crate::music_card::render(ui, m, w, h, state);
        }
        1 => {
            let timer = m.current_timer();
            let clock = timer
                .map(|t| t.clock())
                .unwrap_or_else(|| format!("{:02}:00", m.minutes));
            ui.label(&clock, 26., 64., 62., s.on_surface, true, w - 310.);
            ui.label(
                &timer
                    .map(|t| t.subtitle())
                    .unwrap_or("先选一段时间，再专心做一件事。".into()),
                30.,
                151.,
                13.,
                s.on_surface_variant,
                false,
                w - 320.,
            );
            let bx = w - 246.;
            ui.button(
                rect(bx, 72., 216., 48.),
                timer
                    .map(|t| match t.primary_action() {
                        "pause" => "暂停专注",
                        "resume" => "继续专注",
                        "next" => "下一阶段",
                        _ => "再来一轮",
                    })
                    .unwrap_or("开始专注"),
                Some(if timer.is_some_and(|t| t.running) {
                    Icon::Pause
                } else {
                    Icon::Play
                }),
                timer
                    .map(|t| Action::Timer(t.id.clone(), t.primary_action()))
                    .unwrap_or(Action::QuickFocus),
                false,
                true,
            );
            ui.button(
                rect(bx, 132., 102., 38.),
                "重置",
                Some(Icon::Reset),
                timer
                    .map(|t| Action::Timer(t.id.clone(), "reset"))
                    .unwrap_or(Action::Preset(25)),
                true,
                timer.is_some(),
            );
            ui.button(
                rect(bx + 114., 132., 102., 38.),
                "全部计时",
                None,
                Action::Navigate(Page::Focus),
                true,
                true,
            );
            if let Some(t) = timer {
                let track = rect(30., h - 83., w - 60., 6.);
                draw::fill(ui.scene, track, 3., s.outline_variant);
                draw::fill(
                    ui.scene,
                    rect(track.x0, track.y0, track.width() * t.progress(), 6.),
                    3.,
                    s.primary,
                );
                ui.label(
                    if t.finished {
                        "这一段，已经完成。"
                    } else if t.running {
                        "正在专注，按自己的节奏来。"
                    } else {
                        "已经暂停，准备好时再继续。"
                    },
                    30.,
                    h - 57.,
                    12.,
                    s.on_surface_variant,
                    false,
                    w - 60.,
                );
            } else {
                ui.label(
                    "专注时长",
                    30.,
                    h - 84.,
                    11.,
                    s.on_surface_variant,
                    false,
                    100.,
                );
                for (j, min) in [15, 25, 45].iter().enumerate() {
                    ui.chip(
                        rect(28. + j as f64 * 110., h - 57., 100., 34.),
                        &format!("{min} 分钟"),
                        m.minutes == *min,
                        Action::Preset(*min),
                    );
                }
                ui.label(
                    "结束后，记得休息一下。",
                    w - 246.,
                    h - 47.,
                    11.,
                    s.on_surface_variant,
                    false,
                    216.,
                );
            }
        }
        2 => {
            ui.label(
                &format!("{} 件小事，慢慢完成", m.pending()),
                28.,
                65.,
                12.,
                s.on_surface_variant,
                false,
                w - 360.,
            );
            ui.button(
                rect(w - 278., 60., 116., 34.),
                "快速提醒",
                None,
                Action::QuickReminder,
                true,
                true,
            );
            ui.button(
                rect(w - 150., 60., 124., 34.),
                "添加待办",
                Some(Icon::Plus),
                Action::EditTodo(None),
                false,
                true,
            );
            let now = dotisle_domain::todos::Now::local();
            let mut items = m
                .todos
                .items
                .iter()
                .filter(|t| !t.done(&now))
                .collect::<Vec<_>>();
            items.sort_by_key(|t| (&t.date, &t.time));
            if items.is_empty() {
                ui.icon(Icon::Leaf, 32., 127., 36., s.primary);
                ui.label("清爽的一天", 90., 119., 25., s.on_surface, true, w - 122.);
                ui.label(
                    "记下一件小事，然后安心去做。",
                    92.,
                    162.,
                    13.,
                    s.on_surface_variant,
                    false,
                    w - 124.,
                );
            }
            for (j, t) in items.iter().take(2).enumerate() {
                let y = 108. + j as f64 * 64.;
                crate::acrylic::inset(ui, rect(22., y, w - 44., 56.));
                ui.icon_button(
                    rect(28., y + 7., 40., 40.),
                    Icon::Check,
                    "完成待办",
                    Action::ToggleTodo(t.id.clone()),
                    false,
                    true,
                );
                ui.label(&t.title, 82., y + 6., 15., s.on_surface, true, w - 266.);
                ui.label(
                    &format!(
                        "{}  {}{}",
                        t.date,
                        t.time,
                        if t.daily { " · 每天" } else { "" }
                    ),
                    82.,
                    y + 32.,
                    11.,
                    s.on_surface_variant,
                    false,
                    w - 266.,
                );
                ui.button(
                    rect(w - 174., y + 10., 84., 36.),
                    "编辑",
                    None,
                    Action::EditTodo(Some(t.id.clone())),
                    true,
                    true,
                );
                ui.icon_button(
                    rect(w - 78., y + 10., 36., 36.),
                    Icon::Trash,
                    "移除待办",
                    Action::DeleteTodo(t.id.clone()),
                    false,
                    true,
                );
            }
            ui.button(
                rect(26., h - 45., 124., 32.),
                "全部待办",
                None,
                Action::Navigate(Page::Todos),
                true,
                true,
            );
        }
        3 => {
            let paused = m.setting("PauseNotifications").as_bool().unwrap_or(false);
            let events = m.native_state["notificationHistory"].as_array();
            let count = events.map(|e| e.len()).unwrap_or(0);
            ui.label(
                &if paused {
                    "通知已暂停".into()
                } else {
                    format!("最近收到 {count} 条提醒")
                },
                28.,
                65.,
                12.,
                s.on_surface_variant,
                false,
                w - 360.,
            );
            ui.button(
                rect(w - 294., 60., 130., 34.),
                if paused {
                    "恢复通知"
                } else {
                    "暂停通知"
                },
                Some(if paused { Icon::Play } else { Icon::Pause }),
                Action::NativeEdit("PauseNotifications", json!(!paused)),
                true,
                true,
            );
            ui.button(
                rect(w - 152., 60., 126., 34.),
                "查看全部",
                None,
                Action::Navigate(Page::Notifications),
                true,
                true,
            );
            if count == 0 {
                ui.icon(Icon::Volume, 32., 135., 34., s.primary);
                ui.label("此刻很安静", 90., 121., 25., s.on_surface, true, w - 122.);
                ui.label(
                    "收到的通知与到期提醒，会收拢在这里。",
                    92.,
                    165.,
                    13.,
                    s.on_surface_variant,
                    false,
                    w - 124.,
                );
            } else {
                for (j, event) in events.unwrap().iter().take(2).enumerate() {
                    let y = 108. + j as f64 * 64.;
                    crate::acrylic::inset(ui, rect(22., y, w - 44., 56.));
                    ui.icon(Icon::Volume, 36., y + 17., 20., s.on_surface_variant);
                    ui.label(
                        event["title"].as_str().unwrap_or("通知"),
                        74.,
                        y + 5.,
                        14.,
                        s.on_surface,
                        true,
                        w - 270.,
                    );
                    ui.label(
                        event["body"].as_str().unwrap_or(""),
                        74.,
                        y + 31.,
                        11.,
                        s.on_surface_variant,
                        false,
                        w - 270.,
                    );
                    ui.label(
                        event["clock"].as_str().unwrap_or(""),
                        w - 174.,
                        y + 8.,
                        10.,
                        s.on_surface_variant,
                        false,
                        132.,
                    );
                    if event["kind"] == "notice" {
                        ui.button(
                            rect(w - 174., y + 24., 132., 28.),
                            "打开来源",
                            None,
                            Action::NativeCommand("historyOpen", event["id"].clone()),
                            true,
                            true,
                        );
                    } else if let Some(id) = event["todoId"].as_str() {
                        ui.button(
                            rect(w - 174., y + 24., 132., 28.),
                            "查看待办",
                            None,
                            Action::EditTodo(Some(id.into())),
                            true,
                            true,
                        );
                    }
                }
            }
            ui.button(
                rect(26., h - 45., 132., 32.),
                "通知偏好",
                Some(Icon::Settings),
                Action::SettingsTab(242),
                true,
                true,
            );
        }
        _ => {
            ui.label(
                "把常用的小事，轻轻放到桌面。",
                28.,
                65.,
                13.,
                s.on_surface_variant,
                false,
                w - 56.,
            );
            let kinds = dotisle_domain::desktop_widgets::Kind::ALL;
            let cw = (w - 76.) / 3.;
            for (j, k) in kinds.iter().take(6).enumerate() {
                let x = 26. + (j % 3) as f64 * (cw + 12.);
                let y = 102. + (j / 3) as f64 * 64.;
                let r = rect(x, y, cw, 54.);
                crate::acrylic::inset(ui, r);
                let hit = ui.hit(
                    r,
                    &format!("添加{}桌面组件", k.label()),
                    Action::WidgetAdd(*k, k.default_size()),
                    m.desktop_widgets.items.len() < 16,
                );
                ui.state(r, 16., hit, s.on_surface);
                ui.icon(
                    match j {
                        0 | 1 => Icon::Focus,
                        2 => Icon::Todo,
                        3 => Icon::Play,
                        4 => Icon::Music,
                        _ => Icon::Settings,
                    },
                    x + 14.,
                    y + 16.,
                    22.,
                    s.primary,
                );
                ui.label(
                    k.label(),
                    x + 48.,
                    y + 8.,
                    14.,
                    s.on_surface,
                    true,
                    cw - 82.,
                );
                ui.label(
                    [
                        "一眼知晓时间",
                        "今天，也值得期待",
                        "记下重要小事",
                        "留一段时间给自己",
                        "喜欢的歌近一点",
                        "状态，一眼知晓",
                    ][j],
                    x + 48.,
                    y + 31.,
                    10.,
                    s.on_surface_variant,
                    false,
                    cw - 76.,
                );
                ui.icon(Icon::Plus, x + cw - 28., y + 19., 16., s.on_surface_variant);
            }
            ui.button(
                rect(26., h - 45., 180., 32.),
                "选择全部组件风格",
                Some(Icon::Folder),
                Action::Navigate(Page::Widgets),
                true,
                true,
            );
            ui.label(
                &format!("已放置 {} 个组件", m.desktop_widgets.items.len()),
                w - 206.,
                h - 37.,
                11.,
                s.on_surface_variant,
                false,
                180.,
            );
        }
    }
}
impl Model {
    pub(crate) fn quick_focus(&mut self) {
        if let Some(t) = self.current_timer() {
            let a = Action::Timer(t.id.clone(), t.primary_action());
            self.dispatch(a);
            return;
        }
        if self.remote.is_some() {
            self.native_command("timerCreate",json!({"mode":"pomodoro","focus":self.minutes*60,"rest":self.rest_seconds,"rounds":self.timer_rounds}));
        } else if self.demo {
            match self.timers.create(&json!({"mode":"pomodoro","focus":self.minutes*60,"rest":self.rest_seconds,"rounds":self.timer_rounds})){Ok(id)=>self.selected=Some(id),Err(e)=>self.message(e)}
        }
    }
}
