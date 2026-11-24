//! 精工桌面组件家族：柔和光学渐层外壳、双层微光描边、微容器层次排版与全尺寸自适应几何。
use crate::{
    draw::{self, rect, Icon},
    model::{Action, Model, Page},
    themes::mix,
    views::{self, ViewState},
    widget_extras::text_lines,
    widgets::Ui,
};
use dotisle_domain::{
    desktop_widgets::{Kind, Metrics, Size},
    widget_config,
};
use vello::{
    kurbo::{Affine, RoundedRect, Stroke, Rect},
    peniko::{Fill, Gradient},
};

fn is_dark_scheme(s: &dotisle_domain::monet::Scheme) -> bool {
    let r = (s.surface >> 16) & 0xff;
    let g = (s.surface >> 8) & 0xff;
    let b = s.surface & 0xff;
    (r * 299 + g * 587 + b * 114) < 128_000
}

fn kind_accent(kind: Kind, blue: u32) -> u32 {
    match kind {
        Kind::Focus | Kind::FocusStats => 0xFF7B61,
        Kind::ScreenTime => blue,
        Kind::Todos | Kind::Today => 0x2FB894,
        Kind::Calendar => 0xEA5858,
        Kind::Weather => 0x389BFF,
        Kind::Note => 0xE2AF38,
        Kind::Battery => 0x34C77B,
        _ => blue,
    }
}

/// 精工桌面小组件光学底面：细腻垂直光学渐层 + 外轮廓边线 + 顶部柔和内高光边
fn paint_shell(ui: &mut Ui, r: Rect, accent: u32, dark: bool) {
    let s = ui.s;
    let top_rgb = if dark {
        mix(mix(s.container, 0xFFFFFF, 0.055), accent, 0.04)
    } else {
        mix(mix(s.container, 0xFFFFFF, 0.56), accent, 0.025)
    };
    let bot_rgb = if dark {
        mix(s.container, 0x05070B, 0.14)
    } else {
        mix(s.container, 0xE2E8F0, 0.22)
    };
    let grad = Gradient::new_linear((r.x0, r.y0), (r.x0, r.y1)).with_stops([
        (0.0, draw::color(top_rgb)),
        (1.0, draw::color(bot_rgb)),
    ]);
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &grad,
        None,
        &RoundedRect::from_rect(r, 24.),
    );
    let inner = r.inset(-1.0);
    let inner_rim = if dark {
        draw::alpha(0xFFFFFF, 0.09)
    } else {
        draw::alpha(0xFFFFFF, 0.85)
    };
    ui.scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        inner_rim,
        None,
        &RoundedRect::from_rect(inner, 23.),
    );
    let outer_rim = if dark {
        draw::alpha(0xFFFFFF, 0.11)
    } else {
        draw::alpha(0x0F172A, 0.09)
    };
    ui.scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        outer_rim,
        None,
        &RoundedRect::from_rect(r, 24.),
    );
}

fn subtle_tile(ui: &mut Ui, r: Rect, radius: f64, accent: u32, dark: bool) {
    let s = ui.s;
    let fill = if dark {
        mix(mix(s.high, 0xFFFFFF, 0.04), accent, 0.05)
    } else {
        mix(mix(s.surface, 0x0F172A, 0.032), accent, 0.04)
    };
    let rim = if dark {
        draw::alpha(0xFFFFFF, 0.08)
    } else {
        draw::alpha(0x0F172A, 0.06)
    };
    draw::fill_rounded(ui.scene, r, radius, fill);
    ui.scene.stroke(
        &Stroke::new(0.8),
        Affine::IDENTITY,
        rim,
        None,
        &RoundedRect::from_rect(r, radius),
    );
}

fn bar_chart_bars(
    ui: &mut Ui,
    x: f64,
    y_bot: f64,
    w: f64,
    chart_h: f64,
    values: Option<&Vec<serde_json::Value>>,
    peak: f64,
    accent: u32,
    dark: bool,
) {
    let s = ui.s;
    let grid_col = if dark {
        draw::alpha(0xFFFFFF, 0.06)
    } else {
        draw::alpha(0x0F172A, 0.06)
    };
    for frac in [0.5, 1.0] {
        let gy = y_bot - chart_h * frac;
        ui.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            grid_col,
            None,
            &RoundedRect::from_rect(rect(x, gy, w, 1.), 0.5),
        );
    }
    let bw = ((w - 36.) / 7.).max(6.);
    let gap = ((w - bw * 7.) / 6.).max(3.);
    let track_bg = if dark {
        mix(s.high, 0xFFFFFF, 0.04)
    } else {
        mix(s.high, 0x0F172A, 0.04)
    };
    for i in 0..7 {
        let bx = x + i as f64 * (bw + gap);
        draw::fill_rounded(ui.scene, rect(bx, y_bot - chart_h, bw, chart_h), 4., track_bg);
        let n = values.and_then(|a| a.get(i)).and_then(|v| v.as_f64());
        let h = (4. + (n.unwrap_or(0.) / peak).clamp(0., 1.) * (chart_h - 4.)).min(chart_h);
        let col = if i == 6 {
            accent
        } else if n.is_some_and(|v| v > 0.) {
            mix(s.high, accent, 0.45)
        } else {
            mix(s.high, accent, 0.14)
        };
        draw::fill_rounded(ui.scene, rect(bx, y_bot - h, bw, h), 4., col);
        if i == 6 {
            draw::circle(ui.scene, bx + bw / 2., y_bot - chart_h - 6., 2., accent);
        }
    }
}

pub fn render(
    ui: &mut Ui,
    m: &Model,
    kind: Kind,
    r: Rect,
    size: Size,
    id: &str,
    state: &ViewState,
) {
    let s = ui.s;
    let dark = is_dark_scheme(&s);
    let fg = s.on_surface;
    let muted = s.on_surface_variant;
    let blue = s.primary;
    let accent = kind_accent(kind, blue);

    paint_shell(ui, r, accent, dark);

    let (x, y, w) = (r.x0 + 16., r.y0 + 15., r.width() - 32.);
    let small = size == Size::Small;
    let large = size == Size::Large;
    let placement = m.desktop_widgets.items.iter().find(|p| p.id == id);
    let config = placement.map(|p| p.config.clone()).unwrap_or_default();
    let data = &m.native_state["widgetData"][id];
    let title = if config.title.is_empty() {
        kind.label()
    } else {
        &config.title
    };

    // 标题旁精致色标点缀，增强组件识别度与细节质感
    draw::fill_rounded(ui.scene, rect(x, y + 3.5, 3.5, 11.), 1.75, accent);
    ui.label(title, x + 9., y, 12., muted, true, w - 12.);

    let metrics: Metrics =
        serde_json::from_value(m.native_state["systemMetrics"].clone()).unwrap_or_default();
    let clock_size = if small { 34. } else { 44. };

    match kind {
        Kind::Clock => {
            let d = crate::desktop_cards::date();
            ui.label(
                &format!("{:02}:{:02}", d.hour, d.minute),
                x,
                y + 28.,
                clock_size,
                fg,
                true,
                w,
            );
            subtle_tile(ui, rect(x, r.y1 - 40., w, 24.), 8., accent, dark);
            ui.label(
                &format!("{}月{}日 · {}", d.month, d.day, d.weekday_label()),
                x + 8.,
                r.y1 - 36.,
                11.,
                muted,
                false,
                w - 16.,
            );
            if large {
                week(ui, x, y + 114., w, d, fg, blue, dark);
            }
        }
        Kind::Calendar => {
            let d = crate::desktop_cards::date();
            if small {
                subtle_tile(ui, rect(x, y + 27., 52., 20.), 10., accent, dark);
                ui.center(d.weekday_label(), rect(x, y + 27., 52., 20.), 10., accent, true);
                ui.label(&d.day.to_string(), x, y + 52., 54., fg, true, w);
            } else {
                ui.label(&format!("{}月", d.month), x, y + 28., 18., accent, true, 70.);
                ui.label(&d.day.to_string(), x, y + 54., 46., fg, true, 74.);
                if large {
                    subtle_tile(ui, rect(x, y + 102., 66., 20.), 10., accent, dark);
                    ui.center(d.weekday_label(), rect(x, y + 102., 66., 20.), 10., accent, true);
                }
                let gx = if large { x } else { x + 94. };
                let gy = if large { y + 130. } else { y + 26. };
                let gw = if large { w } else { w - 94. };
                let cell = gw / 7.;
                let step = if large { 24. } else { 17. };
                for (i, name) in ["一", "二", "三", "四", "五", "六", "日"].iter().enumerate() {
                    let hc = if i >= 5 { mix(muted, accent, 0.35) } else { muted };
                    ui.center(name, rect(gx + i as f64 * cell, gy, cell, 15.), 9., hc, i >= 5);
                }
                for day in 1..=d.days() {
                    let n = day as usize - 1 + d.first_weekday();
                    let col = n % 7;
                    let xx = gx + col as f64 * cell;
                    let yy = gy + 19. + (n / 7) as f64 * step;
                    let today = day == d.day;
                    if today {
                        draw::circle(
                            ui.scene,
                            xx + cell / 2.,
                            yy + step / 2.,
                            step / 2. + 2.5,
                            mix(s.container, accent, 0.22),
                        );
                        draw::circle(ui.scene, xx + cell / 2., yy + step / 2., step / 2., accent);
                    }
                    let dfg = if today {
                        0xFFFFFF
                    } else if col >= 5 {
                        mix(fg, muted, 0.38)
                    } else {
                        fg
                    };
                    ui.center(&day.to_string(), rect(xx, yy, cell, step), 10., dfg, today);
                }
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
            if items.is_empty() {
                subtle_tile(
                    ui,
                    rect(x, y + 34., w, if small { 76. } else { 88. }),
                    16.,
                    accent,
                    dark,
                );
                draw::circle(ui.scene, x + 24., y + 66., 12., mix(s.container, accent, 0.16));
                ui.icon(Icon::Check, x + 15., y + 57., 18., accent);
                ui.label("暂无待办任务", x + 46., y + 52., 13., fg, true, w - 58.);
                ui.label("一切安排就绪", x + 46., y + 73., 10., muted, false, w - 58.);
            } else {
                let rows = if large { 4 } else if small { 1 } else { 2 };
                for (i, todo) in items.iter().take(rows).enumerate() {
                    let yy = y + 27. + i as f64 * 55.;
                    let row = rect(x - 4., yy, w + 8., 49.);
                    let row_accent = [0x57BFFF, 0x9A8BEB, 0xFF9EAD, 0x34C77B][i % 4];
                    subtle_tile(ui, row, 15., row_accent, dark);
                    draw::fill_rounded(ui.scene, rect(x + 4., yy + 15., 3.5, 19.), 1.75, row_accent);
                    ui.label(&todo.title, x + 16., yy + 7., 12., fg, true, w - 48.);
                    ui.label(
                        &format!("{} {}", todo.date, todo.time),
                        x + 16.,
                        yy + 27.,
                        10.,
                        muted,
                        false,
                        w - 48.,
                    );
                    ui.hit(
                        rect(x + 12., yy, w - 44., 49.),
                        "编辑待办",
                        Action::EditTodo(Some(todo.id.clone())),
                        true,
                    );
                    ui.icon_button(
                        rect(r.x1 - 44., yy + 9., 30., 30.),
                        Icon::Check,
                        "完成待办",
                        Action::ToggleTodo(todo.id.clone()),
                        false,
                        true,
                    );
                }
            }
            if large {
                ui.button(
                    rect(x, r.y1 - 48., w, 34.),
                    "新建待办",
                    Some(Icon::Plus),
                    Action::EditTodo(None),
                    true,
                    true,
                );
            }
        }
        Kind::Focus => {
            let timer = m.current_timer();
            let (clock, detail, progress) = if let Some(t) = timer {
                (t.clock(), t.subtitle(), t.progress())
            } else {
                ("25:00".into(), "准备开始".into(), 0.)
            };
            let ring = if large { 174. } else if small { 30. } else { 70. };
            let cx = if large { r.center().x } else { r.x1 - 22. - ring / 2. };
            let cy = if large { y + 128. } else if small { y + 46. } else { y + 58. };
            let track_col = if dark { mix(s.high, 0xFFFFFF, 0.06) } else { mix(s.high, 0x0F172A, 0.05) };
            // 柔和内衬圆盘与细环增强空间深邃感
            draw::circle(ui.scene, cx, cy, ring / 2., mix(s.container, accent, if dark { 0.07 } else { 0.05 }));
            draw::ring(ui.scene, (cx, cy), ring / 2., 1., track_col, if large { 5. } else { 4. });
            draw::ring(ui.scene, (cx, cy), ring / 2., progress, accent, if large { 5. } else { 4. });
            if large {
                ui.center(&clock, rect(x, cy - 30., w, 56.), 42., fg, true);
                ui.center(&detail, rect(x, cy + 28., w, 20.), 11., muted, false);
            } else {
                if !small {
                    let center_txt = if progress > 0. {
                        format!("{:.0}%", progress * 100.)
                    } else if timer.is_some_and(|t| t.running) {
                        "专注".into()
                    } else {
                        "就绪".into()
                    };
                    ui.center(&center_txt, rect(cx - ring / 2., cy - 10., ring, 20.), 10., muted, true);
                }
                ui.label(
                    &clock,
                    x,
                    y + 23.,
                    if small { 28. } else { 37. },
                    fg,
                    true,
                    if small { w - ring - 8. } else { w - ring - 14. },
                );
                ui.label(
                    &detail,
                    x,
                    if small { y + 66. } else { y + 76. },
                    10.5,
                    muted,
                    false,
                    if small { w - ring - 8. } else { w - ring - 14. },
                );
            }
            let action = timer
                .map(|t| Action::Timer(t.id.clone(), t.primary_action()))
                .unwrap_or(Action::QuickFocus);
            ui.button(
                rect(x, r.y1 - 46., w, 32.),
                if timer.is_some_and(|t| t.running) { "暂停" } else { "开始专注" },
                Some(if timer.is_some_and(|t| t.running) {
                    Icon::Pause
                } else {
                    Icon::Play
                }),
                action,
                false,
                true,
            );
        }
        Kind::Music => {
            let cover = if large { 100. } else if small { 38. } else { 62. };
            views::record(ui, m, rect(x, y + 27., cover, cover), false);
            let tx = if small || large { x } else { x + cover + 12. };
            let ty = if large { y + 143. } else if small { y + 71. } else { y + 30. };
            ui.label(
                m.title(),
                tx,
                ty,
                if small { 12. } else { 16. },
                fg,
                true,
                if small || large { w } else { w - cover - 12. },
            );
            if !small {
                ui.label(
                    m.artist(),
                    tx,
                    ty + 26.,
                    11.,
                    muted,
                    false,
                    if large { w } else { w - cover - 12. },
                );
            }
            if large {
                ui.label(m.current_lyric(), x, y + 206., 13., muted, false, w);
                views::progress(ui, m, rect(x, y + 242., w, 20.), state);
            }
            for (i, action) in ["previous", "toggle", "next"].iter().enumerate() {
                let bw = 32.;
                let gap = if small { 6. } else { 18. };
                let total = bw * 3. + gap * 2.;
                views::transport_control(
                    ui,
                    m,
                    rect(r.center().x - total / 2. + i as f64 * (bw + gap), r.y1 - 44., bw, bw),
                    [3, 4, 5][i],
                    ["上一首", "播放或暂停", "下一首"][i],
                    action,
                );
            }
        }
        Kind::System => {
            for (i, (label, value)) in [("CPU", metrics.cpu_percent), ("内存", metrics.memory_percent)]
                .iter()
                .enumerate()
            {
                let xx = x + if small { 0. } else { i as f64 * (w / 2. + 4.) };
                let yy = y + if small { 27. + i as f64 * 52. } else { 30. };
                let cw = if small { w } else { w / 2. - 4. };
                let ch = if small { 46. } else if large { 246. } else { 104. };
                let tint = if i == 0 { blue } else { 0x8B5CF6 };
                subtle_tile(ui, rect(xx, yy, cw, ch), 14., tint, dark);
                ui.label(label, xx + 10., yy + 8., 10.5, muted, true, cw - 20.);
                ui.label(
                    &value.map(|v| format!("{v:.0}%")).unwrap_or("—".into()),
                    xx + 10.,
                    yy + if small { 21. } else { 27. },
                    if small { 19. } else { 30. },
                    fg,
                    true,
                    cw - 20.,
                );
                if large {
                    let cy = yy + 168.;
                    draw::ring(ui.scene, (xx + cw / 2., cy), 44., 1., s.high, 5.);
                    if let Some(value) = value {
                        draw::ring(ui.scene, (xx + cw / 2., cy), 44., value / 100., tint, 5.);
                    }
                    ui.center(
                        &value.map(|v| format!("{v:.0}%")).unwrap_or("—".into()),
                        rect(xx, cy - 12., cw, 24.),
                        13.,
                        muted,
                        true,
                    );
                }
            }
        }
        Kind::Wireless => {
            let devices = metrics
                .wireless_devices
                .iter()
                .filter(|d| d.connected)
                .collect::<Vec<_>>();
            if devices.is_empty() {
                subtle_tile(ui, rect(x, y + 30., w, r.height() - 60.), 16., accent, dark);
                ui.icon(Icon::Volume, x + 16., y + 48., 26., muted);
                ui.label(
                    "暂无连接的设备",
                    x + 16.,
                    y + 84.,
                    if small { 13. } else { 16. },
                    fg,
                    true,
                    w - 32.,
                );
            }
            let cols = if small { 1 } else { 2 };
            let cw = w / cols as f64;
            for (i, device) in devices.iter().take(if large { 4 } else { cols }).enumerate() {
                let xx = x + (i % cols) as f64 * cw;
                let yy = y + 28. + (i / cols) as f64 * 136.;
                let cx = xx + cw / 2.;
                let radius = if small { 25. } else { 30. };
                draw::ring(ui.scene, (cx, yy + radius), radius, 1., s.high, 4.);
                if let Some(level) = device.battery_percent {
                    draw::ring(
                        ui.scene,
                        (cx, yy + radius),
                        radius,
                        level as f64 / 100.,
                        if level <= 20 { 0xF29B49 } else { accent },
                        4.,
                    );
                }
                device_icon(ui, (cx - 12., yy + radius - 12.), &device.kind, fg);
                ui.center(
                    &device.battery_percent.map(|v| format!("{v}%")).unwrap_or("—".into()),
                    rect(xx, yy + radius * 2. + 7., cw, 26.),
                    19.,
                    fg,
                    true,
                );
                ui.center(&device.name, rect(xx, yy + radius * 2. + 33., cw, 18.), 10., muted, false);
            }
        }
        Kind::Note => {
            subtle_tile(ui, rect(x, y + 26., w, r.height() - 56.), 14., accent, dark);
            draw::fill_rounded(ui.scene, rect(x + 10., y + 36., 3., 16.), 1.5, accent);
            text_lines(
                ui,
                &config.text,
                x + 19.,
                y + 34.,
                w - 28.,
                if large { 10 } else { 3 },
                if small { 13. } else { 14.5 },
                fg,
            );
        }
        Kind::Anniversary => {
            if let Some(date) = widget_config::date_ordinal(&config.date) {
                let delta = date - widget_config::ordinal(crate::desktop_cards::date());
                ui.label(&delta.abs().to_string(), x, y + 28., if small { 44. } else { 58. }, fg, true, w);
                ui.label(
                    if delta >= 0 { "天后，迎接这一天" } else { "天，已经一起走过" },
                    x,
                    y + 102.,
                    11.,
                    muted,
                    false,
                    w,
                );
                if large {
                    subtle_tile(ui, rect(x, r.y1 - 44., w, 28.), 10., accent, dark);
                    ui.label(&config.date, x + 10., r.y1 - 39., 13., accent, true, w - 20.);
                }
            } else {
                ui.label("设置重要日期", x, y + 49., if small { 16. } else { 22. }, fg, true, w);
            }
        }
        Kind::Weather => {
            if let Some(temperature) = data["temperature"].as_f64() {
                ui.label(data["city"].as_str().unwrap_or(&config.city), x, y + 25., 12., muted, false, w);
                ui.label(&format!("{temperature:.0}°"), x, y + 44., if small { 46. } else { 56. }, fg, true, w);
                if !small {
                    subtle_tile(ui, rect(r.x1 - 74., y + 30., 56., 56.), 16., accent, dark);
                    ui.icon(Icon::Sun, r.x1 - 63., y + 41., 34., accent);
                }
                ui.label(
                    if data["stale"] == true { "数据待更新" } else { "实时天气" },
                    x,
                    r.y1 - 34.,
                    11.,
                    muted,
                    false,
                    w,
                );
                if large {
                    if let (Some(high), Some(low)) = (data["high"].as_f64(), data["low"].as_f64()) {
                        subtle_tile(ui, rect(x, y + 144., w, 30.), 10., accent, dark);
                        ui.label(&format!("最高 {high:.0}° · 最低 {low:.0}°"), x + 10., y + 150., 12.5, muted, false, w - 20.);
                    }
                }
            } else {
                ui.icon(Icon::Sun, x, y + 32., 32., accent);
                ui.label("等待天气数据", x, y + 83., if small { 14. } else { 18. }, fg, true, w);
            }
        }
        Kind::Today => {
            let pending = m.todos.items.iter().filter(|t| !t.done(&dotisle_domain::todos::Now::local())).count();
            let timers = m.timers.items.iter().filter(|t| t.running).count();
            for (i, (value, label, tint)) in [(pending, "待办事项", 0x2FB894), (timers, "运行中计时", 0xFF7B61)].iter().enumerate() {
                let xx = x + if small { 0. } else { i as f64 * (w / 2. + 4.) };
                let yy = y + if small { 27. + i as f64 * 48. } else { 30. };
                let cw = if small { w } else { w / 2. - 4. };
                let ch = if small { 42. } else { 104. };
                subtle_tile(ui, rect(xx, yy, cw, ch), 14., *tint, dark);
                ui.label(&value.to_string(), xx + 12., yy + if small { 5. } else { 12. }, if small { 24. } else { 40. }, *tint, true, cw - 24.);
                ui.label(label, xx + if small { 48. } else { 12. }, yy + if small { 13. } else { 66. }, 11., muted, false, cw - 24.);
            }
            if large {
                ui.button(rect(x, r.y1 - 48., w, 34.), "查看待办事项", None, Action::Navigate(Page::Todos), true, true);
            }
        }
        Kind::Battery => {
            ui.label(
                &metrics.battery_percent.map(|v| format!("{v}%")).unwrap_or("—".into()),
                x,
                y + 30.,
                clock_size,
                fg,
                true,
                w,
            );
            let bat_col = if metrics.battery_percent.is_some_and(|v| v <= 20) { 0xF29B49 } else { accent };
            ui.label(
                if metrics.battery_percent.is_none() { "未检测到电池" } else if metrics.external_power == Some(true) { "已连接电源" } else { "正在使用电池" },
                x,
                y + 92.,
                11.,
                muted,
                false,
                w,
            );
            if large {
                draw::ring(ui.scene, (r.center().x, y + 216.), 60., 1., s.high, 6.);
                if let Some(value) = metrics.battery_percent {
                    draw::ring(ui.scene, (r.center().x, y + 216.), 60., value as f64 / 100., bat_col, 6.);
                }
            }
        }
        Kind::Storage => {
            if let (Some(free), Some(total)) = (data["free"].as_f64(), data["total"].as_f64().filter(|v| *v > 0.)) {
                ui.label(&format!("{:.0}", free / 1e9), x, y + 28., clock_size, fg, true, w);
                ui.label(&format!("GB 可用 · 共 {:.0} GB", total / 1e9), x, y + 90., 11., muted, false, w);
                let track = rect(x, r.y1 - 28., w, 6.);
                draw::fill_rounded(ui.scene, track, 3., s.high);
                draw::fill_rounded(ui.scene, rect(x, track.y0, w * (1. - free / total).clamp(0., 1.), 6.), 3., accent);
            } else {
                ui.label("暂无磁盘读数", x, y + 57., if small { 16. } else { 22. }, fg, true, w);
            }
        }
        Kind::WorldClock => {
            ui.label(data["clock"].as_str().unwrap_or("—"), x, y + 30., clock_size, fg, true, w);
            subtle_tile(ui, rect(x, y + 90., w, 26.), 9., accent, dark);
            ui.label(
                data["city"].as_str().unwrap_or(if config.city.is_empty() { "选择城市" } else { &config.city }),
                x + 9.,
                y + 95.,
                11.5,
                muted,
                false,
                w - 18.,
            );
            if large {
                ui.label(data["date"].as_str().unwrap_or("等待时间数据"), x, y + 153., 13., muted, false, w);
            }
        }
        Kind::Launcher => {
            draw::circle(ui.scene, x + 25., y + 58., 25., mix(s.container, accent, 0.14));
            ui.icon(Icon::Arrow, x + 12., y + 45., 26., accent);
            ui.label(
                if config.title.is_empty() { "常用入口" } else { &config.title },
                x,
                y + 95.,
                if small { 16. } else { 22. },
                fg,
                true,
                w,
            );
            if !config.target.is_empty() {
                ui.hit(rect(r.x0, r.y0 + 32., r.width(), r.height() - 32.), "打开快捷入口", Action::WidgetLaunch(id.into()), true);
            } else {
                ui.label("右键设置目标", x, r.y1 - 29., 10., muted, false, w);
            }
        }
        Kind::Schedule => {
            let now = crate::desktop_cards::date();
            let minute = widget_config::ordinal(now) * 1440 + now.hour as i64 * 60 + now.minute as i64;
            if let Some(at) = widget_config::deadline_minutes(&config.deadline) {
                let left = (at - minute).max(0);
                let value = if left == 0 {
                    "已到时间".into()
                } else if left >= 1440 {
                    format!("{}天 {}小时", left / 1440, left % 1440 / 60)
                } else if left >= 60 {
                    format!("{}小时 {}分", left / 60, left % 60)
                } else {
                    format!("{left}分")
                };
                ui.icon(Icon::Focus, x, y + 29., 22., accent);
                ui.label(&value, x, y + 64., if small { 22. } else { 30. }, fg, true, w);
                ui.label(&config.deadline, x, y + 110., 11., muted, false, w);
                if large {
                    subtle_tile(ui, rect(x, r.y1 - 44., w, 28.), 10., accent, dark);
                    ui.label(
                        if left == 0 { "重要日程已到时间" } else { "每一刻，都更近一点" },
                        x + 10.,
                        r.y1 - 39.,
                        12.,
                        muted,
                        false,
                        w - 20.,
                    );
                }
            } else {
                ui.icon(Icon::Focus, x, y + 30., 24., accent);
                ui.label("设置日程", x, y + 73., 22., fg, true, w);
            }
        }
        Kind::Plugin => {
            let entry = m.native_state["plugins"]
                .as_array()
                .and_then(|p| p.iter().find(|p| p["id"] == config.plugin_id && p["enabled"] == true && p["error"] == ""));
            if let Some(entry) = entry {
                let blocks: Vec<dotisle_domain::plugins::ResolvedBlock> =
                    serde_json::from_value(entry["views"]["desktop"].clone()).unwrap_or_default();
                let total = blocks
                    .iter()
                    .map(|b| match b.kind.as_str() {
                        "metric" => 56.,
                        "progress" => 40.,
                        "divider" => 12.,
                        _ => 28.,
                    })
                    .sum::<f64>();
                let scale = ((r.height() - 64.) / total.max(1.)).min(1.);
                let mut yy = y + 28.;
                for b in blocks {
                    let hh = match b.kind.as_str() {
                        "metric" => 56.,
                        "progress" => 40.,
                        "divider" => 12.,
                        _ => 28.,
                    };
                    match b.kind.as_str() {
                        "divider" => draw::fill_rounded(ui.scene, rect(x, yy + 5. * scale, w, 1.), 0.5, s.high),
                        "progress" => {
                            ui.label(&b.label, x, yy, (11. * scale).max(9.) as f32, muted, false, w);
                            draw::fill_rounded(ui.scene, rect(x, yy + 25. * scale, w, 4.), 2., s.high);
                            if b.value > 0. {
                                draw::fill_rounded(ui.scene, rect(x, yy + 25. * scale, w * b.value, 4.), 2., accent);
                            }
                        }
                        _ => {
                            ui.label(&b.text, x, yy, (if b.kind == "metric" { 26. * scale } else { 13. * scale }).max(9.) as f32, fg, b.kind == "metric", w);
                            if b.kind == "metric" {
                                ui.label(&b.label, x, yy + 34. * scale, (11. * scale).max(9.) as f32, muted, false, w);
                            }
                        }
                    }
                    yy += hh * scale;
                }
            } else {
                ui.label("插件未启用或已移除", x, y + 48., 12., muted, false, w);
            }
        }
        Kind::ScreenTime => {
            let stats = &m.native_state["screenTime"];
            let value = stats["todayMs"].as_f64().map(usage_duration).unwrap_or("正在连接".into());
            ui.label(&value, x, y + 29., if small { 24. } else { 32. }, fg, true, w);
            ui.label("今日活跃用机", x, y + 73., 11., muted, false, w);
            let yesterday = stats["yesterdayMs"].as_f64().map(|v| format!("昨日 {}", usage_duration(v))).unwrap_or("昨日暂无记录".into());
            subtle_tile(ui, rect(x, y + 98., w, 26.), 9., accent, dark);
            ui.label(&yesterday, x + 9., y + 103., 11., muted, false, w - 18.);
            if large {
                let values = stats["week"].as_array();
                let peak = values.map(|a| a.iter().filter_map(|v| v.as_f64()).fold(60000., f64::max)).unwrap_or(60000.);
                bar_chart_bars(ui, x, r.y1 - 44., w, 108., values, peak, accent, dark);
                draw::circle(ui.scene, x + 3., r.y1 - 21., 2.5, accent);
                ui.label("近7天 · 仅统计点屿运行期间", x + 10., r.y1 - 27., 9.5, muted, false, w - 12.);
            }
        }
        Kind::FocusStats => {
            let stats = &m.native_state["widgetData"]["focusStats"];
            let minutes = stats["todayMs"].as_f64().unwrap_or(0.) / 60000.;
            ui.label(&format!("{minutes:.0}"), x, y + 27., clock_size, fg, true, w);
            ui.label("今日专注 · 分钟", x, y + 88., 11., muted, false, w);
            if large {
                let values = stats["week"].as_array();
                let maximum = values.map(|a| a.iter().filter_map(|v| v.as_f64()).fold(1., f64::max)).unwrap_or(1.);
                bar_chart_bars(ui, x, r.y1 - 36., w, 108., values, maximum, accent, dark);
            }
        }
    }

    if placement.is_some()
        && matches!(
            kind,
            Kind::Note | Kind::Anniversary | Kind::Schedule | Kind::Weather | Kind::Storage | Kind::WorldClock
        )
    {
        ui.hit(
            rect(r.x0, r.y0 + 28., r.width(), r.height() - 28.),
            "编辑组件内容",
            Action::WidgetConfigure(id.into()),
            true,
        );
    }
}

pub(crate) fn week_days(date: dotisle_domain::desktop_widgets::Date) -> [u32; 7] {
    let start = date.day as i32 - date.weekday() as i32;
    let previous = dotisle_domain::desktop_widgets::Date {
        year: if date.month == 1 { date.year - 1 } else { date.year },
        month: if date.month == 1 { 12 } else { date.month - 1 },
        ..date
    }
    .days();
    std::array::from_fn(|i| {
        let day = start + i as i32;
        if day < 1 {
            (previous as i32 + day) as u32
        } else if day > date.days() as i32 {
            day as u32 - date.days()
        } else {
            day as u32
        }
    })
}

fn week(ui: &mut Ui, x: f64, y: f64, w: f64, date: dotisle_domain::desktop_widgets::Date, fg: u32, blue: u32, dark: bool) {
    let cw = w / 7.;
    for (i, day) in week_days(date).iter().enumerate() {
        let xx = x + i as f64 * cw;
        let today = i == date.weekday();
        if today {
            draw::fill_rounded(
                ui.scene,
                rect(xx + 2., y, cw - 4., 68.),
                14.,
                mix(ui.s.container, blue, if dark { 0.22 } else { 0.14 }),
            );
        }
        ui.center(["一", "二", "三", "四", "五", "六", "日"][i], rect(xx, y + 7., cw, 18.), 10., fg, false);
        ui.center(&day.to_string(), rect(xx, y + 32., cw, 26.), 17., if today { blue } else { fg }, true);
    }
}

fn device_icon(ui: &mut Ui, (x, y): (f64, f64), kind: &str, ink: u32) {
    use vello::{kurbo::BezPath, peniko::Fill};
    let mut path = BezPath::new();
    match kind {
        "mouse" => {
            draw::outline_rounded(ui.scene, rect(x + 5., y, 14., 24.), 7., ink, 1.6);
            path.move_to((x + 12., y + 3.));
            path.line_to((x + 12., y + 9.));
        }
        "keyboard" => {
            draw::outline_rounded(ui.scene, rect(x, y + 4., 24., 16.), 3., ink, 1.5);
            for i in 0..4 {
                for j in 0..2 {
                    draw::circle(ui.scene, x + 5. + i as f64 * 4.5, y + 9. + j as f64 * 4., 0.8, ink);
                }
            }
            path.move_to((x + 6., y + 17.));
            path.line_to((x + 18., y + 17.));
        }
        _ => {
            path.move_to((x + 4., y + 14.));
            path.line_to((x + 4., y + 9.));
            path.curve_to((x + 4., y - 1.), (x + 20., y - 1.), (x + 20., y + 9.));
            path.line_to((x + 20., y + 14.));
            draw::fill_rounded(ui.scene, rect(x + 2., y + 10., 5., 11.), 2.5, ink);
            draw::fill_rounded(ui.scene, rect(x + 17., y + 10., 5., 11.), 2.5, ink);
        }
    }
    ui.scene.stroke(&Stroke::new(1.6), Affine::IDENTITY, draw::color(ink), None, &path);
    let _ = Fill::NonZero;
}

fn usage_duration(ms: f64) -> String {
    let m = (ms.max(0.) / 60000.) as u64;
    if m >= 60 {
        format!("{}小时 {}分", m / 60, m % 60)
    } else {
        format!("{m}分钟")
    }
}
