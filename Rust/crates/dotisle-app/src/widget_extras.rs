//! Apple-style information hierarchy for the additional desktop widgets.
use crate::{
    draw::{self, rect, Icon},
    model::{Action, Dialog, DialogKind, Field, Model},
    widgets::Ui,
};
use dotisle_domain::{
    desktop_widgets::{Kind, Metrics, Size},
    widget_config::{self, Config},
};
use serde_json::json;
use vello::{
    kurbo::{Affine, Rect},
    peniko::{Fill, Gradient},
};

fn colors(kind: Kind, dark: bool) -> (u32, u32, u32) {
    if dark {
        return (
            0x20232B,
            0xF4F5F8,
            match kind {
                Kind::Note => 0xF4D475,
                Kind::Battery => 0x77DDA8,
                Kind::Weather => 0x94C9FF,
                _ => 0xA7BFFF,
            },
        );
    }
    match kind {
        Kind::Note => (0xFFF1BC, 0x40331E, 0x9D7119),
        Kind::Anniversary => (0xF1E9FC, 0x352C49, 0x8861BD),
        Kind::Weather => (0x336CB3, 0xFFFFFF, 0xDAEBFF),
        Kind::Battery => (0xEDF8F1, 0x193A2A, 0x268E58),
        Kind::FocusStats => (0xF0EDFC, 0x362C50, 0x8964D3),
        _ => (0xFAFBFD, 0x262A33, 0x5786C9),
    }
}
fn text_lines(
    ui: &mut Ui,
    value: &str,
    x: f64,
    y: f64,
    width: f64,
    lines: usize,
    size: f32,
    color: u32,
) {
    let mut line = String::new();
    let mut row = 0;
    for ch in value.chars().chain(std::iter::once('\n')) {
        let mut next = line.clone();
        next.push(ch);
        if ch == '\n' || ui.measure_alpha(&next, size, false) > width {
            ui.label(
                &line,
                x,
                y + row as f64 * (size as f64 * 1.5),
                size,
                color,
                false,
                width,
            );
            row += 1;
            if row >= lines {
                return;
            }
            line.clear();
        }
        if ch != '\n' {
            line.push(ch);
        }
    }
}
fn meter(ui: &mut Ui, bounds: Rect, progress: f64, accent: u32, track: u32) {
    draw::fill_rounded(ui.scene, bounds, bounds.height() / 2., track);
    if progress > 0. {
        draw::fill_rounded(
            ui.scene,
            rect(
                bounds.x0,
                bounds.y0,
                bounds.width() * progress.clamp(0., 1.),
                bounds.height(),
            ),
            bounds.height() / 2.,
            accent,
        );
    }
}
pub fn render(ui: &mut Ui, m: &Model, kind: Kind, r: Rect, size: Size, id: &str) -> bool {
    if matches!(
        kind,
        Kind::Clock | Kind::Calendar | Kind::Todos | Kind::Focus | Kind::Music | Kind::System
    ) {
        return false;
    }
    let config = m
        .desktop_widgets
        .items
        .iter()
        .find(|p| p.id == id)
        .map(|p| p.config.clone())
        .unwrap_or_default();
    let data = &m.native_state["widgetData"][id];
    let dark = ui.s.surface < 0x808080;
    let (bg, fg, accent) = colors(kind, dark);
    let shape = draw::rounded_rect(r, 24.);
    let gradient = Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([
        draw::color(bg),
        draw::color(crate::themes::mix(
            bg,
            accent,
            if dark { 0.08 } else { 0.05 },
        )),
    ]);
    ui.scene.draw_blurred_rounded_rect(
        Affine::translate((0., 3.)),
        r,
        draw::alpha(0, 0.09),
        24.,
        7.,
    );
    ui.scene
        .fill(Fill::NonZero, Affine::IDENTITY, &gradient, None, &shape);
    let (x, y, w) = (r.x0 + 16., r.y0 + 16., r.width() - 32.);
    let small = size == Size::Small;
    let large = size == Size::Large;
    let title = if config.title.is_empty() {
        match kind {
            Kind::Weather => data["city"].as_str().filter(|s| !s.is_empty()).unwrap_or(
                if config.city.is_empty() {
                    "天气"
                } else {
                    &config.city
                },
            ),
            Kind::WorldClock => {
                if config.city.is_empty() {
                    "UTC"
                } else {
                    &config.city
                }
            }
            _ => kind.label(),
        }
    } else {
        &config.title
    };
    ui.label(title, x, y, 14., accent, true, w);
    let baseline = y + 24.;
    let configured = m.desktop_widgets.items.iter().any(|p| p.id == id);
    if configured
        && !matches!(
            kind,
            Kind::Launcher | Kind::Today | Kind::FocusStats | Kind::Battery
        )
    {
        ui.hit(
            Rect::new(r.x0, r.y0 + 42., r.x1, r.y1),
            "编辑组件内容",
            Action::WidgetConfigure(id.into()),
            true,
        );
    }
    match kind {
        Kind::Note => {
            let text = if config.text.is_empty() {
                "记下一点想法\n点击编辑便签"
            } else {
                &config.text
            };
            text_lines(
                ui,
                text,
                x,
                baseline,
                w,
                if large { 10 } else { 4 },
                if small { 14. } else { 16. },
                fg,
            );
        }
        Kind::Anniversary => {
            let today = widget_config::ordinal(crate::desktop_cards::date());
            if let Some(target) = widget_config::date_ordinal(&config.date) {
                let days = target - today;
                ui.label(
                    &days.abs().to_string(),
                    x,
                    baseline,
                    if small { 45. } else { 54. },
                    fg,
                    true,
                    w - 28.,
                );
                ui.label(
                    if days >= 0 {
                        "天后到来"
                    } else {
                        "天已经过去"
                    },
                    x,
                    baseline + 68.,
                    12.,
                    accent,
                    false,
                    w,
                );
                if large {
                    ui.label(&config.date, x, r.y1 - 48., 16., fg, true, w);
                }
            } else {
                ui.label(
                    "值得期待",
                    x,
                    baseline + 8.,
                    if small { 22. } else { 28. },
                    fg,
                    true,
                    w,
                );
                text_lines(
                    ui,
                    "点击设置一个重要日期",
                    x,
                    baseline + 66.,
                    w,
                    2,
                    12.,
                    accent,
                );
            }
        }
        Kind::Weather => {
            if let Some(temperature) = data["temperature"].as_f64() {
                ui.label(
                    &format!("{temperature:.0}°"),
                    x,
                    baseline + 6.,
                    if small { 44. } else { 52. },
                    fg,
                    false,
                    w,
                );
                let code = data["code"].as_u64().unwrap_or(0);
                let label = match code {
                    0 => "晴",
                    1..=3 => "多云",
                    45 | 48 => "有雾",
                    51..=67 => "有雨",
                    71..=77 => "有雪",
                    80..=86 => "阵雨",
                    95..=99 => "雷雨",
                    _ => "天气更新中",
                };
                let suffix = if data["stale"] == true {
                    " · 待更新"
                } else {
                    ""
                };
                ui.label(
                    &format!("{label}{suffix}"),
                    x,
                    r.y1 - 38.,
                    12.,
                    fg,
                    false,
                    w,
                );
                if large {
                    ui.label(
                        &format!(
                            "最高 {:.0}° / 最低 {:.0}°",
                            data["high"].as_f64().unwrap_or(temperature),
                            data["low"].as_f64().unwrap_or(temperature)
                        ),
                        x,
                        baseline + 112.,
                        14.,
                        fg,
                        false,
                        w,
                    );
                    ui.label("Open-Meteo", x, r.y1 - 62., 10., accent, false, w);
                }
                if !small {
                    ui.icon(
                        if data["day"] == 0 {
                            Icon::Moon
                        } else {
                            Icon::Sun
                        },
                        r.x1 - 77.,
                        baseline + 7.,
                        44.,
                        fg,
                    );
                }
            } else {
                ui.icon(Icon::Sun, x, baseline + 5., 34., fg);
                ui.label(
                    if config.city.is_empty() {
                        "选择城市"
                    } else {
                        "天气更新中"
                    },
                    x,
                    baseline + 57.,
                    if small { 18. } else { 23. },
                    fg,
                    true,
                    w,
                );
                let status = data["error"].as_str().unwrap_or(if config.city.is_empty() {
                    "点击设置城市"
                } else {
                    "正在获取天气"
                });
                text_lines(ui, status, x, r.y1 - 36., w, 1, 11., accent);
            }
        }
        Kind::Today => {
            let date = crate::desktop_cards::date();
            ui.label(
                &format!("{}月{}日", date.month, date.day),
                x,
                baseline,
                if small { 24. } else { 30. },
                fg,
                true,
                w,
            );
            let pending = m.pending();
            let running = m.timers.items.iter().filter(|t| t.running).count();
            ui.label(
                &format!("{pending} 件待办"),
                x,
                baseline + 52.,
                17.,
                fg,
                true,
                w,
            );
            ui.label(
                &format!("{running} 个计时进行中"),
                x,
                baseline + 82.,
                12.,
                accent,
                false,
                w,
            );
            if large {
                for (i, todo) in m
                    .todos
                    .items
                    .iter()
                    .filter(|t| !t.completed)
                    .take(4)
                    .enumerate()
                {
                    ui.label(
                        &todo.title,
                        x,
                        baseline + 132. + i as f64 * 30.,
                        13.,
                        fg,
                        false,
                        w,
                    );
                }
            }
            if configured {
                ui.hit(
                    r,
                    "查看今日待办",
                    Action::Navigate(crate::model::Page::Todos),
                    true,
                );
            }
        }
        Kind::Battery => {
            let metrics: Metrics =
                serde_json::from_value(m.native_state["systemMetrics"].clone()).unwrap_or_default();
            ui.label(
                &metrics
                    .battery_percent
                    .map(|v| format!("{v}%"))
                    .unwrap_or("—".into()),
                x,
                baseline,
                if small { 44. } else { 52. },
                fg,
                true,
                w,
            );
            let track = rect(x, r.y1 - 52., w - 4., 13.);
            meter(
                ui,
                track,
                metrics.battery_percent.unwrap_or(0) as f64 / 100.,
                accent,
                crate::themes::mix(bg, accent, 0.18),
            );
            ui.label(
                if metrics.battery_percent.is_none() {
                    "未检测到电池"
                } else if metrics.external_power == Some(true) {
                    "已连接电源"
                } else {
                    "正在使用电池"
                },
                x,
                r.y1 - 31.,
                11.,
                accent,
                false,
                w,
            );
        }
        Kind::Storage => {
            let root = if config.drive.is_empty() {
                "C:"
            } else {
                &config.drive
            };
            if let (Some(free), Some(total)) = (data["free"].as_u64(), data["total"].as_u64()) {
                ui.label(
                    &format!("{:.0} GB", free as f64 / 1_000_000_000.),
                    x,
                    baseline,
                    if small { 30. } else { 40. },
                    fg,
                    true,
                    w,
                );
                ui.label(
                    &format!("{root} 可用空间"),
                    x,
                    baseline + 53.,
                    12.,
                    accent,
                    false,
                    w,
                );
                meter(
                    ui,
                    rect(x, r.y1 - 39., w, 8.),
                    1. - free as f64 / total.max(1) as f64,
                    accent,
                    crate::themes::mix(bg, accent, 0.16),
                );
                if large {
                    ui.label(
                        &format!("总容量 {:.0} GB", total as f64 / 1_000_000_000.),
                        x,
                        baseline + 110.,
                        15.,
                        fg,
                        false,
                        w,
                    );
                }
            } else {
                ui.label(root, x, baseline, 36., fg, true, w);
                text_lines(
                    ui,
                    data["error"].as_str().unwrap_or("等待磁盘读数"),
                    x,
                    baseline + 67.,
                    w,
                    2,
                    12.,
                    accent,
                );
            }
        }
        Kind::WorldClock => {
            ui.label(
                data["clock"].as_str().unwrap_or("—:—"),
                x,
                baseline + 4.,
                if small { 36. } else { 56. },
                fg,
                true,
                w,
            );
            ui.label(
                data["date"].as_str().unwrap_or("点击选择城市"),
                x,
                r.y1 - 35.,
                12.,
                accent,
                false,
                w,
            );
            if large {
                ui.icon(
                    if data["hour"].as_u64().is_some_and(|v| (6..18).contains(&v)) {
                        Icon::Sun
                    } else {
                        Icon::Moon
                    },
                    x,
                    baseline + 108.,
                    52.,
                    accent,
                );
            }
        }
        Kind::Launcher => {
            let ready = !config.target.is_empty();
            let name = if let Some(url) = config.target.strip_prefix("https://") {
                url.split('/').next().unwrap_or(url).to_string()
            } else {
                std::path::Path::new(&config.target)
                    .file_stem()
                    .map(|v| v.to_string_lossy().into_owned())
                    .unwrap_or_default()
            };
            ui.label(
                if ready { &name } else { "添加入口" },
                x,
                baseline + 8.,
                if small { 20. } else { 28. },
                fg,
                true,
                w,
            );
            if !small && ready {
                ui.label(&config.target, x, baseline + 58., 11., accent, false, w);
            }
            ui.label(
                if ready {
                    "点击打开"
                } else {
                    "点击选择常用程序或网页"
                },
                x,
                r.y1 - 32.,
                11.,
                accent,
                false,
                w,
            );
            if configured {
                ui.hit(
                    Rect::new(r.x0, r.y0 + 42., r.x1, r.y1),
                    if ready {
                        "打开快捷入口"
                    } else {
                        "配置快捷入口"
                    },
                    if ready {
                        Action::WidgetLaunch(id.into())
                    } else {
                        Action::WidgetConfigure(id.into())
                    },
                    true,
                );
            }
        }
        Kind::FocusStats => {
            let stats = &m.native_state["widgetData"]["focusStats"];
            let minutes = stats["todayMs"].as_u64().unwrap_or(0) / 60000;
            ui.label(
                &format!("{minutes}"),
                x,
                baseline,
                if small { 39. } else { 48. },
                fg,
                true,
                w,
            );
            ui.label("分钟 · 今日专注", x, baseline + 58., 11., accent, false, w);
            if large {
                let week = stats["week"].as_array();
                let max = week
                    .map(|v| {
                        v.iter()
                            .filter_map(|n| n.as_u64())
                            .max()
                            .unwrap_or(1)
                            .max(1)
                    })
                    .unwrap_or(1) as f64;
                let cw = w / 7.;
                for i in 0..7 {
                    let amount = week
                        .and_then(|v| v.get(i))
                        .and_then(|n| n.as_u64())
                        .unwrap_or(0) as f64
                        / max;
                    let h = (amount * 98.).max(3.);
                    draw::fill_rounded(
                        ui.scene,
                        rect(x + i as f64 * cw, r.y1 - 51. - h, cw - 8., h),
                        5.,
                        accent,
                    );
                }
                ui.label(
                    "近七天 · 番茄专注与流程计时",
                    x,
                    r.y1 - 32.,
                    11.,
                    accent,
                    false,
                    w,
                );
            }
        }
        _ => {}
    }
    true
}

impl Model {
    pub(crate) fn edit_widget_config(&mut self, id: &str) {
        let Some(p) = self.desktop_widgets.items.iter().find(|p| p.id == id) else {
            return;
        };
        let c = &p.config;
        let mut fields = vec![Field::new("标题", c.title.clone(), 60)];
        match p.kind {
            Kind::Note => fields.push(Field::new("便签内容", c.text.clone(), 500)),
            Kind::Anniversary => fields.push(Field::new("日期 · YYYY-MM-DD", c.date.clone(), 10)),
            Kind::Weather => fields.push(Field::new("城市 · 可附省份或国家", c.city.clone(), 100)),
            Kind::WorldClock => fields.push(Field::new(
                "城市 · 北京 / 东京 / 伦敦 / 纽约 / 悉尼 / UTC",
                c.city.clone(),
                20,
            )),
            Kind::Storage => fields.push(Field::new("盘符 · 如 C: 或 D:", c.drive.clone(), 3)),
            Kind::Launcher => fields.push(Field::new(
                "程序、文件夹路径或 HTTPS 网页",
                c.target.clone(),
                2048,
            )),
            _ => {}
        }
        self.show_center = true;
        self.dialog = Some(Dialog {
            kind: DialogKind::Widget(id.into()),
            fields,
            active: 0,
            daily: false,
            error: String::new(),
            preedit: String::new(),
        });
    }
    pub(crate) fn save_widget_config(&mut self, id: &str, dialog: &Dialog) -> Result<(), String> {
        let p = self
            .desktop_widgets
            .items
            .iter()
            .find(|p| p.id == id)
            .ok_or("组件已移除")?;
        let mut config: Config = p.config.clone();
        config.title = dialog.fields[0].value.trim().into();
        let value = dialog
            .fields
            .get(1)
            .map(|f| f.value.trim())
            .unwrap_or("")
            .to_owned();
        match p.kind {
            Kind::Note => config.text = value,
            Kind::Anniversary => config.date = value,
            Kind::Weather | Kind::WorldClock => config.city = value,
            Kind::Storage => config.drive = value,
            Kind::Launcher => config.target = value,
            _ => {}
        }
        config.validate(p.kind)?;
        let update = json!({"id":id,"config":config});
        if self.remote.is_some() {
            self.native_command("widgetUpdate", update);
            Ok(())
        } else {
            self.desktop_widgets.update(&update)
        }
    }
}

/// Exercise populated layouts with explicitly synthetic content. Runtime cards
/// continue to show real readings or their unconfigured/offline state.
pub fn verify(
    out: &std::path::Path,
    text: &mut crate::draw::Text,
    gpu: &mut crate::gpu::Gpu,
) -> Result<Vec<String>, String> {
    use dotisle_domain::desktop_widgets::Style;
    let mut model = Model::new(out.join("extra-widget-data"), true);
    let mut files = Vec::new();
    for kind in [
        Kind::Note,
        Kind::Anniversary,
        Kind::Weather,
        Kind::Today,
        Kind::Battery,
        Kind::Storage,
        Kind::WorldClock,
        Kind::Launcher,
        Kind::FocusStats,
    ] {
        let id=model.desktop_widgets.add(&json!({"kind":kind,"config":{"title":if matches!(kind,Kind::Weather|Kind::WorldClock){""}else{kind.label()},"text":"把今天的想法记在这里。\n一步一步，把事情做好。","date":"2027-01-01","city":if kind==Kind::WorldClock{"伦敦"}else{"北京"},"target":"https://example.com","drive":"C:"}}))?;
        model.native_state["widgetData"][&id] = json!({"city":"北京","temperature":18.,"code":2,"day":1,"high":22.,"low":14.,"source":"layout-fixture","free":128000000000u64,"total":512000000000u64,"clock":"09:41","date":"11月12日","hour":9});
        model.native_state["widgetData"]["focusStats"] = json!({"todayMs":4500000,"week":[1200000,2400000,1800000,3600000,2700000,4200000,4500000]});
        model.native_state["systemMetrics"] = json!({"batteryPercent":78,"externalPower":true});
        model.edit_widget_config(&id);
        if model
            .dialog
            .as_ref()
            .is_none_or(|dialog| !matches!(dialog.kind, DialogKind::Widget(_)))
        {
            return Err("widget configuration dialog did not open".into());
        }
        model.dialog = None;
        for size in Size::ALL {
            let view = crate::desktop_cards::render_style(
                &model,
                text,
                kind,
                size,
                Style::Original,
                &id,
                (-1., -1.),
                false,
                0.,
            );
            let (w, h) = size.dimensions();
            if view.hits.iter().any(|hit| {
                hit.rect.x0 < 0. || hit.rect.y0 < 0. || hit.rect.x1 > w || hit.rect.y1 > h
            }) {
                return Err("extra widget target left its bounds".into());
            }
            let name = format!("extra-{kind:?}-{size:?}.png");
            gpu.capture(&view.scene, w as u32, h as u32, &out.join(&name))?;
            files.push(name);
        }
    }
    Ok(files)
}
