//! 精工桌面组件家族：柔和光学渐层外壳、双层微光描边、微容器层次排版与全尺寸自适应几何。
use crate::{
    draw::{self, rect, Icon},
    model::{Action, Model, Page},
    themes::mix,
    views::{self, ViewState},
    widget_extras::text_lines,
    widgets::Ui,
};
use allegro::{
    efficiency::{DropItem, NoteEdit},
    stack::{paint_stack_depth_layers, paint_stack_dots},
    telemetry::ProTelemetry,
    theme::{self as allegro_theme, paint_micro_dot_grid, WidgetAppearance},
};
use dotisle_domain::{
    desktop_widgets::{Kind, Metrics, Size},
    widget_config,
};
use vello::{
    kurbo::{Affine, BezPath, Rect, RoundedRect, Stroke},
    peniko::{Fill, Gradient, ImageData},
};

/// Allegro 画布注入到桌面组件渲染管线的扩展上下文。
#[derive(Default)]
pub struct CardExtras<'a> {
    /// 是否由 Allegro 引擎接管材质底壳与五套视觉主题绘制。
    pub use_allegro_shell: bool,
    /// 组件的材质、皮肤与不透明度外观配置。
    pub appearance: WidgetAppearance,
    /// 为 `Acrylic` 材质裁切模糊后的桌面壁纸区域纹理。
    pub wallpaper_crop: Option<&'a ImageData>,
    /// 当前正在进行原地编辑的便签草稿。
    pub note_edit: Option<&'a NoteEdit>,
    /// 当前文件中的 `Dropzone` 暂存条目切片。
    pub dropzone_items: &'a [DropItem],
    /// `PhotoFrame` 当前展示的图片数据。
    pub photo_image: Option<&'a ImageData>,
    /// `PhotoFrame` 游标与总数 `(current_index, total_count)`。
    pub photo_meta: (usize, usize),
    /// 实时性能探针状态（GPU、网卡吞吐与 30 点波形）。
    pub pro: Option<&'a ProTelemetry>,
    /// 智能叠放状态 `(active_index, total_members)`。
    pub stack_info: Option<(usize, usize)>,
    /// 杂志日签当前语录索引。
    pub quote_index: usize,
    /// 直接透传的组件配置（用于独立侧边栏卡片，不依赖桌面列表查找）。
    pub config_override: Option<&'a widget_config::Config>,
}

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
        Kind::PhotoFrame => 0xF59E0B,
        Kind::Dropzone => 0x06B6D4,
        Kind::SystemPro => 0x10B981,
        Kind::AnalogClock => 0xFF9F0A,
        Kind::ActivityRings => 0xFA114F,
        Kind::QuickDock => 0x0A84FF,
        Kind::Quote => 0x8E8E93,
        Kind::AppFolder => 0x007AFF,
        _ => blue,
    }
}

/// 应用文件夹内单个可启动应用条目（图标配色与内置矢量类型）。
#[derive(Clone, Debug)]
pub struct AppFolderEntry {
    pub name: String,
    pub target: String,
    pub grad_top: u32,
    pub grad_bot: u32,
    pub glyph: AppGlyph,
}

/// 内置手绘 Apple 风格应用图标矢量类型。
#[derive(Clone, Copy, Debug)]
pub enum AppGlyph {
    Folder,
    Terminal,
    Browser,
    Settings,
    Note,
    Calc,
    Capture,
    Monitor,
    Clock,
    Initial(char),
}

/// 解析应用文件夹的展示与启动条目（默认内置 9 款常用 Windows 高频应用，支持自定义/拖放列表）。
pub fn app_folder_entries(config: &widget_config::Config) -> Vec<AppFolderEntry> {
    let defaults = [
        ("文件", "explorer.exe", 0x5AC8FA, 0x007AFF, AppGlyph::Folder),
        ("终端", "wt.exe", 0x48484A, 0x1C1C1E, AppGlyph::Terminal),
        ("浏览器", "msedge.exe", 0x34D399, 0x0284C7, AppGlyph::Browser),
        ("设置", "ms-settings:", 0xAEAEB2, 0x636366, AppGlyph::Settings),
        ("记事本", "notepad.exe", 0xFCD34D, 0xF59E0B, AppGlyph::Note),
        ("计算器", "calc.exe", 0x6B7280, 0x1F2937, AppGlyph::Calc),
        ("截图", "ms-screenclip:", 0xFB7185, 0x9333EA, AppGlyph::Capture),
        ("性能", "taskmgr.exe", 0x34D399, 0x059669, AppGlyph::Monitor),
        ("时钟", "ms-clock:", 0x818CF8, 0x4F46E5, AppGlyph::Clock),
    ];
    const CUSTOM_PALETTE: [(u32, u32); 6] = [
        (0x60A5FA, 0x2563EB),
        (0x34D399, 0x059669),
        (0xFBBF24, 0xEA580C),
        (0xF472B6, 0xDB2777),
        (0xA78BFA, 0x7C3AED),
        (0x38BDF8, 0x0284C7),
    ];

    let mut out = Vec::new();
    for raw in config.items.iter().take(9) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let (name, target) = if let Some((n, t)) = trimmed.split_once('|') {
            (n.trim().to_string(), t.trim().to_string())
        } else {
            let label = std::path::Path::new(trimmed)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(trimmed)
                .to_string();
            (label, trimmed.to_string())
        };
        let hash = name.bytes().fold(0usize, |acc, b| acc.wrapping_mul(31).wrapping_add(b as usize));
        let (gt, gb) = CUSTOM_PALETTE[hash % CUSTOM_PALETTE.len()];
        let ch = name.chars().next().unwrap_or('A').to_ascii_uppercase();
        out.push(AppFolderEntry {
            name,
            target,
            grad_top: gt,
            grad_bot: gb,
            glyph: AppGlyph::Initial(ch),
        });
    }

    for (name, target, gt, gb, glyph) in defaults {
        if out.len() >= 9 {
            break;
        }
        if !out.iter().any(|e| e.target.eq_ignore_ascii_case(target)) {
            out.push(AppFolderEntry {
                name: name.into(),
                target: target.into(),
                grad_top: gt,
                grad_bot: gb,
                glyph,
            });
        }
    }
    out
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
        &draw::continuous_rect(r, 24.),
    );
}

fn subtle_tile(ui: &mut Ui, r: Rect, radius: f64, accent: u32, dark: bool) {
    let s = ui.s;
    let rim = if ui.glass {
        draw::alpha(0xFFFFFF, if dark { 0.18 } else { 0.56 })
    } else if dark {
        draw::alpha(0xFFFFFF, 0.085)
    } else {
        draw::alpha(0x0F172A, 0.055)
    };
    if ui.glass {
        let glass_fill = if dark {
            allegro_theme::rgba(0xFFFFFF, 0.085)
        } else {
            allegro_theme::rgba(0xFFFFFF, 0.38)
        };
        ui.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            glass_fill,
            None,
            &RoundedRect::from_rect(r, radius),
        );
    } else {
        let fill = if dark {
            mix(mix(s.high, 0xFFFFFF, 0.04), accent, 0.05)
        } else {
            mix(mix(s.surface, 0x0F172A, 0.025), accent, 0.035)
        };
        draw::fill_rounded(ui.scene, r, radius, fill);
    }
    ui.scene.stroke(
        &Stroke::new(0.75),
        Affine::IDENTITY,
        rim,
        None,
        &RoundedRect::from_rect(r, radius),
    );
}

/// 根据当前材质返回适配玻璃态或耀光的底层轨道色（避免玻璃材质下出现实心灰块）。
fn glass_track_color(ui: &Ui, tint: u32, dark: bool) -> vello::peniko::Color {
    if ui.glass {
        if dark {
            allegro_theme::rgba(0xFFFFFF, 0.14)
        } else {
            allegro_theme::rgba(0x0F172A, 0.09)
        }
    } else {
        let col = mix(ui.s.high, tint, if dark { 0.12 } else { 0.08 });
        draw::color(col)
    }
}

/// 使用圆角矩形填充支持 Alpha 通道的轨道或色块。
fn fill_alpha_rounded(ui: &mut Ui, r: Rect, radius: f64, col: vello::peniko::Color) {
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        col,
        None,
        &RoundedRect::from_rect(r, radius),
    );
}

/// 绘制兼容光学玻璃材质的柔和圆环底轨（完整 360° 透明描边）。
fn ring_track(ui: &mut Ui, center: (f64, f64), radius: f64, thickness: f64, tint: u32, dark: bool) {
    use vello::kurbo::Circle;
    let col = if ui.glass {
        if dark {
            allegro_theme::rgba(0xFFFFFF, 0.13)
        } else {
            allegro_theme::rgba(0x0F172A, 0.095)
        }
    } else {
        let base = mix(ui.s.high, tint, if dark { 0.15 } else { 0.10 });
        draw::alpha(base, 0.92)
    };
    ui.scene.stroke(
        &Stroke::new(thickness),
        Affine::IDENTITY,
        col,
        None,
        &Circle::new(center, radius),
    );
}

/// 绘制 Apple 风格轻量渐变或纯色胶囊进度条（含透明轨道）。
fn paint_apple_meter(
    ui: &mut Ui,
    r: Rect,
    progress: f64,
    start_rgb: u32,
    end_rgb: u32,
    dark: bool,
) {
    let rad = r.height() / 2.;
    fill_alpha_rounded(ui, r, rad, glass_track_color(ui, end_rgb, dark));
    let ratio = progress.clamp(0., 1.);
    if ratio <= 0.005 {
        return;
    }
    let fill_w = (r.width() * ratio).max(r.height());
    let fill_r = rect(r.x0, r.y0, fill_w.min(r.width()), r.height());
    let grad = Gradient::new_linear((fill_r.x0, fill_r.y0), (fill_r.x1, fill_r.y0)).with_stops([
        (0.0, draw::color(start_rgb)),
        (1.0, draw::color(end_rgb)),
    ]);
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        &grad,
        None,
        &RoundedRect::from_rect(fill_r, rad),
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
    let grid_col = if dark {
        draw::alpha(0xFFFFFF, 0.07)
    } else {
        draw::alpha(0x0F172A, 0.06)
    };
    for frac in [0.5, 1.0] {
        let gy = y_bot - chart_h * frac;
        fill_alpha_rounded(ui, rect(x, gy, w, 0.8), 0.4, grid_col);
    }
    let bw = ((w - 42.) / 7.).max(7.);
    let gap = ((w - bw * 7.) / 6.).max(4.);
    let track_bg = glass_track_color(ui, accent, dark);
    for i in 0..7 {
        let bx = x + i as f64 * (bw + gap);
        fill_alpha_rounded(ui, rect(bx, y_bot - chart_h, bw, chart_h), 4.5, track_bg);
        let n = values.and_then(|a| a.get(i)).and_then(|v| v.as_f64());
        let h = (5. + (n.unwrap_or(0.) / peak).clamp(0., 1.) * (chart_h - 5.)).min(chart_h);
        let bar_r = rect(bx, y_bot - h, bw, h);
        if i == 6 {
            let grad = Gradient::new_linear((bar_r.x0, bar_r.y0), (bar_r.x0, bar_r.y1)).with_stops([
                (0.0, draw::color(mix(accent, 0xFFFFFF, 0.18))),
                (1.0, draw::color(accent)),
            ]);
            ui.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &grad,
                None,
                &RoundedRect::from_rect(bar_r, 4.5),
            );
            draw::circle(ui.scene, bx + bw / 2., y_bot - chart_h - 6., 2.2, accent);
        } else {
            let alpha = if n.is_some_and(|v| v > 0.) { 0.48 } else { 0.16 };
            fill_alpha_rounded(ui, bar_r, 4.5, draw::alpha(accent, alpha));
        }
    }
}

/// 将 Allegro 视觉令牌同步到渲染管线使用的配色方案，并保留未启用 Allegro 时的兼容行为。
fn apply_theme_to_scheme(
    base: dotisle_domain::monet::Scheme,
    t: allegro_theme::ThemeTokens,
) -> dotisle_domain::monet::Scheme {
    let mut s = base;
    s.surface = t.surface;
    s.container = t.container;
    s.high = t.high;
    s.on_surface = t.on_surface;
    s.on_surface_variant = t.on_surface_variant;
    s.primary = t.primary;
    s.on_primary = t.on_primary;
    s
}

/// 兼容旧版调用签名的默认桌面组件渲染入口。
pub fn render(
    ui: &mut Ui,
    m: &Model,
    kind: Kind,
    r: Rect,
    size: Size,
    id: &str,
    state: &ViewState,
) {
    render_with_extras(ui, m, kind, r, size, id, state, &CardExtras::default());
}

/// 带 Allegro 材质、主题皮肤、智能叠放、原地便签与性能波形的完整组件渲染入口。
pub fn render_with_extras(
    ui: &mut Ui,
    m: &Model,
    kind: Kind,
    r: Rect,
    size: Size,
    id: &str,
    state: &ViewState,
    extras: &CardExtras<'_>,
) {
    let dark = is_dark_scheme(&ui.s);
    let saved_scheme = ui.s;
    let saved_glass = ui.glass;
    let tokens = allegro_theme::resolve_theme(extras.appearance.style, dark, extras.appearance.accent);
    if extras.use_allegro_shell {
        ui.s = apply_theme_to_scheme(ui.s, tokens);
        ui.glass = extras.appearance.material == allegro_theme::WidgetMaterial::Acrylic;
    }
    let s = ui.s;
    let fg = s.on_surface;
    let muted = s.on_surface_variant;
    let blue = s.primary;
    let accent = if extras.use_allegro_shell {
        extras
            .appearance
            .accent
            .map(|c| c & 0x00FF_FFFF)
            .unwrap_or_else(|| kind_accent(kind, blue))
    } else {
        kind_accent(kind, blue)
    };

    if let Some((_, total)) = extras.stack_info.filter(|(_, tot)| *tot >= 2) {
        paint_stack_depth_layers(
            ui.scene,
            allegro::snap::Rect2D::new(r.x0, r.y0, r.width(), r.height()),
            total,
            dark,
        );
    }

    if extras.use_allegro_shell {
        allegro_theme::paint_shell(
            ui.scene,
            r,
            extras.appearance,
            tokens,
            accent,
            dark,
            extras.wallpaper_crop,
        );
    } else {
        paint_shell(ui, r, accent, dark);
    }

    let (x, y, w) = (r.x0 + 16., r.y0 + 15., r.width() - 32.);
    let small = size == Size::Small;
    let large = size == Size::Large;
    let placement = m.desktop_widgets.items.iter().find(|p| p.id == id);
    let config = extras
        .config_override
        .cloned()
        .or_else(|| placement.map(|p| p.config.clone()))
        .unwrap_or_default();
    let data = &m.native_state["widgetData"][id];
    let title = if config.title.is_empty() {
        kind.label()
    } else {
        &config.title
    };

    // 无顶栏全幅展示的 Deskset × Smart Stack 原生比例卡片（极净克制，仅工具类卡片保留顶部轻标签）
    let frameless = matches!(
        kind,
        Kind::AppFolder
            | Kind::AnalogClock
            | Kind::Calendar
            | Kind::Weather
            | Kind::PhotoFrame
            | Kind::ActivityRings
            | Kind::Note
            | Kind::Clock
            | Kind::Focus
            | Kind::Music
    );

    if !frameless {
        draw::circle(ui.scene, x + 3.5, y + 8.5, 2.5, accent);
        let title_max = if extras.stack_info.is_some_and(|(_, tot)| tot >= 2) {
            (w - 64.).max(36.)
        } else {
            w - 14.
        };
        ui.label(title, x + 11.5, y, 11.5, muted, true, title_max);
    }

    if let Some((active, total)) = extras.stack_info.filter(|(_, tot)| *tot >= 2) {
        paint_stack_dots(
            ui.scene,
            allegro::snap::Rect2D::new(r.x0, r.y0, r.width(), r.height()),
            active,
            total,
            accent,
        );
        let pill_w = 40.;
        let px = r.x1 - 14. - pill_w;
        let py = y - 2.;
        subtle_tile(ui, rect(px, py, pill_w, 20.), 10., accent, dark);
        ui.center("<", rect(px, py, pill_w / 2., 20.), 10., muted, true);
        ui.center(">", rect(px + pill_w / 2., py, pill_w / 2., 20.), 10., muted, true);
        ui.hit(rect(px, py, pill_w / 2., 20.), "叠放上一页", Action::StackPrev(id.into()), true);
        ui.hit(rect(px + pill_w / 2., py, pill_w / 2., 20.), "叠放下一页", Action::StackNext(id.into()), true);
    }

    let metrics: Metrics =
        serde_json::from_value(m.native_state["systemMetrics"].clone()).unwrap_or_default();
    let clock_size = if small { 36. } else { 46. };

    match kind {
        Kind::Clock => {
            let d = crate::desktop_cards::date();
            // Deskset 极净居中数字时钟：轻量星期日期 + 超粗居中时间 + 发光日出日落点阵轨
            let date_top = if large { y + 10. } else { y + 6. };
            ui.center(
                &format!("{}月{}日 · {}", d.month, d.day, d.weekday_label()),
                rect(x, date_top, w, 18.),
                if small { 10.5 } else { 11.5 },
                muted,
                false,
            );
            let time_size = if large { 62. } else if small { 42. } else { 54. };
            let time_y = if large { date_top + 26. } else if small { date_top + 28. } else { date_top + 22. };
            ui.center(
                &format!("{:02}:{:02}", d.hour, d.minute),
                rect(x, time_y, w, time_size + 12.),
                time_size as f32,
                fg,
                true,
            );

            // 点阵日照轨迹 + 暖橙发光太阳节点 + 日出日落时间
            let track_y = if large { time_y + 78. } else { r.y1 - 36. };
            let track_pad = if small { 8. } else { 22. };
            let tx0 = x + track_pad;
            let tx1 = x + w - track_pad;
            let tw = (tx1 - tx0).max(40.);
            let day_mins = (d.hour as f64 * 60.0 + d.minute as f64).clamp(0.0, 1440.0);
            let sun_frac = ((day_mins - 402.0) / (1038.0 - 402.0)).clamp(0.08, 0.92);
            let sun_x = tx0 + tw * sun_frac;
            let dot_col = draw::alpha(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.22 } else { 0.16 });
            let mut dx = tx0;
            while dx <= tx1 {
                ui.scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    dot_col,
                    None,
                    &vello::kurbo::Circle::new((dx, track_y), 1.0),
                );
                dx += 6.0;
            }
            let bar_grad = Gradient::new_linear((tx0, track_y), (sun_x, track_y)).with_stops([
                (0.0, draw::alpha(0xFF9F0A, 0.25)),
                (1.0, draw::alpha(0xFF9500, 0.92)),
            ]);
            ui.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &bar_grad,
                None,
                &RoundedRect::from_rect(rect(tx0, track_y - 1.2, (sun_x - tx0).max(4.), 2.4), 1.2),
            );
            let sun_halo = Gradient::new_radial((sun_x, track_y), 10.0).with_stops([
                (0.0, draw::alpha(0xFFB340, 0.55)),
                (1.0, draw::alpha(0xFF9500, 0.0)),
            ]);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &sun_halo, None, &vello::kurbo::Circle::new((sun_x, track_y), 10.0));
            draw::circle(ui.scene, sun_x, track_y, 4.2, 0xFF9500);
            draw::circle(ui.scene, sun_x, track_y, 1.8, 0xFFF7E0);

            let label_y = track_y + 8.;
            ui.label("日出 06:42", tx0, label_y, 9.5, muted, false, tw * 0.5);
            ui.label("日落 17:18", tx1 - if small { 46. } else { 52. }, label_y, 9.5, muted, false, 56.);

            if large {
                week(ui, x, track_y + 42., w, d, fg, blue, dark);
            }
        }
        Kind::Calendar => {
            let d = crate::desktop_cards::date();
            let wk = (d.month * 4 + d.day / 7).clamp(1, 52);
            if small {
                // Deskset 极简红标日历：珊瑚红星期 + 右上轻量月份 + 62px 超大日号 + 周数底标
                ui.label(d.weekday_label(), x, y + 3., 13.5, accent, true, w - 42.);
                ui.label(&format!("{}月", d.month), r.x1 - 48., y + 4., 11.5, muted, true, 32.);
                ui.label(&d.day.to_string(), x - 2., y + 26., 62., fg, true, w);
                let foot = rect(x, r.y1 - 35., w, 21.);
                subtle_tile(ui, foot, 10.5, accent, dark);
                draw::circle(ui.scene, x + 10., r.y1 - 24.5, 2.4, accent);
                ui.label(
                    &format!("{}年 · 第{wk}周", d.year),
                    x + 18.,
                    r.y1 - 31.5,
                    10.,
                    muted,
                    true,
                    w - 24.,
                );
            } else {
                ui.label(d.weekday_label(), x, y + 4., 13., accent, true, 78.);
                ui.label(&d.day.to_string(), x - 2., y + 28., 54., fg, true, 82.);
                ui.label(&format!("第 {wk} 周"), x, y + 92., 11., muted, false, 78.);
                if large {
                    let pill = rect(x, y + 114., 72., 22.);
                    fill_alpha_rounded(ui, pill, 11., draw::alpha(accent, if dark { 0.22 } else { 0.14 }));
                    ui.center(&format!("{}年", d.year), pill, 10.5, accent, true);
                }
                let gx = if large { x } else { x + 94. };
                let gy = if large { y + 144. } else { y + 20. };
                let gw = if large { w } else { w - 94. };
                ui.label(&format!("{} 年 {} 月", d.year, d.month), gx, gy - 17., 10.5, accent, true, gw);
                let cell = gw / 7.;
                let step = if large { 24. } else { 18. };
                for (i, name) in ["一", "二", "三", "四", "五", "六", "日"].iter().enumerate() {
                    let hc = if i >= 5 { mix(muted, accent, 0.45) } else { muted };
                    ui.center(name, rect(gx + i as f64 * cell, gy, cell, 14.), 8.5, hc, i >= 5);
                }
                for day in 1..=d.days() {
                    let n = day as usize - 1 + d.first_weekday();
                    let col = n % 7;
                    let xx = gx + col as f64 * cell;
                    let yy = gy + 17. + (n / 7) as f64 * step;
                    let today = day == d.day;
                    if today {
                        use vello::kurbo::Circle;
                        let center = (xx + cell / 2., yy + step / 2.);
                        ui.scene.fill(
                            Fill::NonZero,
                            Affine::IDENTITY,
                            draw::alpha(accent, 0.22),
                            None,
                            &Circle::new(center, step / 2. + 2.5),
                        );
                        draw::circle(ui.scene, center.0, center.1, step / 2., accent);
                    }
                    let dfg = if today {
                        0xFFFFFF
                    } else if col >= 5 {
                        mix(fg, muted, 0.38)
                    } else {
                        fg
                    };
                    ui.center(&day.to_string(), rect(xx, yy, cell, step), 9.5, dfg, today);
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
            if let Some(editor) = state.editor.as_ref() {
                todo_editor(ui, r, x, y, w, editor, accent, dark, fg, muted, state.time);
            } else if items.is_empty() {
                subtle_tile(
                    ui,
                    rect(x, y + 32., w, if small { 78. } else { 92. }),
                    16.,
                    accent,
                    dark,
                );
                {
                    use vello::kurbo::Circle;
                    let c = (x + 25., y + 66.);
                    ui.scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        draw::alpha(accent, if dark { 0.24 } else { 0.16 }),
                        None,
                        &Circle::new(c, 13.0),
                    );
                }
                ui.icon(Icon::Check, x + 16., y + 57., 18., accent);
                ui.label("暂无待办任务", x + 48., y + 51., 13., fg, true, w - 58.);
                ui.label("一切安排就绪", x + 48., y + 72., 10.5, muted, false, w - 58.);
                if large {
                    ui.button(
                        rect(x, r.y1 - 48., w, 34.),
                        "新建待办",
                        Some(Icon::Plus),
                        Action::TodoInlineEdit(None),
                        true,
                        true,
                    );
                }
            } else {
                let row_h = crate::desktop_cards::TODO_ROW;
                let top = y + 27.;
                let bottom = if large { r.y1 - 50. } else { r.y1 - 6. };
                let (visible, max_scroll) = crate::desktop_cards::todo_metrics(
                    r.width() + 16.,
                    r.height() + 16.,
                    size,
                    items.len(),
                );
                let scroll = state.scroll.clamp(0., max_scroll);
                let vp = rect(x - 4., top - 4., w + 8., (bottom - top).max(24.));
                // 列表滚动时裁掉越过卡片内容区的行，滚动条固定在右侧。
                ui.scene
                    .push_clip_layer(Affine::IDENTITY, &RoundedRect::from_rect(vp, 14.));
                let first = (scroll / row_h).floor() as usize;
                for (i, todo) in items.iter().enumerate().skip(first).take(visible + 1) {
                    let yy = top + i as f64 * row_h - scroll;
                    if yy > bottom {
                        break;
                    }
                    let row = rect(x - 4., yy, w + 8., 49.);
                    let row_accent = [0x0A84FF, 0xAF52DE, 0xFF375F, 0x30D158][i % 4];
                    subtle_tile(ui, row, 15., row_accent, dark);
                    draw::circle(ui.scene, x + 8., yy + 24.5, 3.8, row_accent);
                    ui.label(&todo.title, x + 19., yy + 7., 12., fg, true, w - 52.);
                    ui.label(
                        &format!("{} {}", todo.date, todo.time),
                        x + 19.,
                        yy + 27.,
                        10.,
                        muted,
                        false,
                        w - 52.,
                    );
                    let edit_action = if large {
                        Action::TodoInlineEdit(Some(todo.id.clone()))
                    } else {
                        Action::EditTodo(Some(todo.id.clone()))
                    };
                    ui.hit(
                        rect(x + 12., yy, w - 50., 49.).intersect(vp),
                        "编辑待办",
                        edit_action,
                        true,
                    );
                    let check = rect(r.x1 - 44., yy + 9., 30., 30.);
                    if check.y1 <= bottom + 4. {
                        ui.icon_button(
                            check,
                            Icon::Check,
                            "完成待办",
                            Action::ToggleTodo(todo.id.clone()),
                            false,
                            true,
                        );
                    }
                }
                ui.scene.pop_layer();
                if max_scroll > 0.5 {
                    let bar_x = r.x1 - 5.;
                    let track_top = top + 2.;
                    let track_h = (bottom - top - 4.).max(20.);
                    let ratio = ((bottom - top) / ((bottom - top) + max_scroll)).clamp(0.18, 0.9);
                    let thumb_h = (track_h * ratio).max(18.);
                    let thumb_y = track_top + (track_h - thumb_h) * (scroll / max_scroll);
                    let track = if dark { draw::alpha(0xFFFFFF, 0.12) } else { draw::alpha(0x0F172A, 0.08) };
                    let bar = if dark { draw::alpha(0xFFFFFF, 0.42) } else { draw::alpha(0x0F172A, 0.36) };
                    fill_alpha_rounded(ui, rect(bar_x, track_top, 4., track_h), 2., track);
                    fill_alpha_rounded(ui, rect(bar_x, thumb_y, 4., thumb_h), 2., bar);
                }
                if large {
                    ui.button(
                        rect(x, r.y1 - 48., w, 34.),
                        "新建待办",
                        Some(Icon::Plus),
                        Action::TodoInlineEdit(None),
                        true,
                        true,
                    );
                }
            }
        }
        Kind::Focus => {
            // Deskset 极简琥珀橙圆环番茄钟 + 底部 5/15/25/45 预设胶囊条
            let timer = m.current_timer();
            let mono = m.native_state["settings"]["TimerMonoDigits"]
                .as_bool()
                .unwrap_or(false);
            let clock_face = if mono { "Space Mono" } else { "Noto Sans SC" };
            let (clock, _detail, progress) = if let Some(t) = timer {
                (t.clock(), t.subtitle(), t.progress())
            } else {
                ("25:00".into(), "准备开始".into(), 0.25)
            };
            let focus_orange = 0xFF8A00;
            let ring = if large { 132. } else if small { 58. } else { 68. };
            let thick = if large { 10.5 } else if small { 6.5 } else { 7.5 };
            let cx = if small || large { r.center().x } else { x + ring * 0.5 + 18. };
            let cy = if large { y + 98. } else if small { y + 40. } else { r.center().y - 8. };

            ring_track(ui, (cx, cy), ring / 2., thick, focus_orange, dark);
            draw::ring(ui.scene, (cx, cy), ring / 2., progress.max(0.06), focus_orange, thick);

            let action = timer
                .map(|t| Action::Timer(t.id.clone(), t.primary_action()))
                .unwrap_or(Action::QuickFocus);
            let running = timer.is_some_and(|t| t.running);
            let btn_icon = if running { Icon::Pause } else { Icon::Play };
            let btn_sz = (ring * 0.44).clamp(18., 38.);
            let btn_r = rect(cx - btn_sz * 0.5, cy - btn_sz * 0.5, btn_sz, btn_sz);
            ui.icon(btn_icon, btn_r.x0 + 2., btn_r.y0 + 2., btn_sz - 4., fg);
            ui.hit(
                rect(cx - ring * 0.55, cy - ring * 0.55, ring * 1.1, ring * 1.1),
                if running { "暂停专注" } else { "开始专注" },
                action,
                true,
            );

            if small {
                let cs = 23.;
                let cy_clk = cy + ring * 0.5 + 12.;
                if mono {
                    ui.center_face(clock_face, &clock, rect(x, cy_clk, w, 28.), cs, fg, true);
                } else {
                    ui.center(&clock, rect(x, cy_clk, w, 28.), cs, fg, true);
                }
            } else if large {
                let cy_clk = cy + ring * 0.5 + 24.;
                if mono {
                    ui.center_face(clock_face, &clock, rect(x, cy_clk, w, 46.), 38., fg, true);
                } else {
                    ui.center(&clock, rect(x, cy_clk, w, 46.), 38., fg, true);
                }
            } else {
                let rx = cx + ring * 0.5 + 22.;
                let rw = (x + w - rx).max(80.);
                if mono {
                    ui.center_face(clock_face, &clock, rect(rx, cy - 22., rw, 44.), 38., fg, true);
                } else {
                    ui.center(&clock, rect(rx, cy - 22., rw, 44.), 38., fg, true);
                }
            }

            // Deskset 底部 5 / 15 / 25 / 45 分钟预设胶囊栏
            let bar_w = if small { w - 8. } else if large { (w * 0.72).max(160.) } else { (x + w - (cx + ring * 0.5 + 22.)).max(140.) };
            let bar_x = if small || large { r.center().x - bar_w * 0.5 } else { cx + ring * 0.5 + 22. };
            let bar_y = if large { r.y1 - 46. } else { r.y1 - 38. };
            let bar_r = rect(bar_x, bar_y, bar_w, 24.);
            subtle_tile(ui, bar_r, 12., focus_orange, dark);
            let presets = ["5", "15", "25", "45"];
            let pw = (bar_w - 6.) / presets.len() as f64;
            for (idx, lbl) in presets.iter().enumerate() {
                let pr = rect(bar_x + 3. + idx as f64 * pw, bar_y + 2.5, pw, 19.);
                if idx == 2 {
                    fill_alpha_rounded(
                        ui,
                        pr,
                        9.5,
                        allegro_theme::rgba(0xFFFFFF, if dark { 0.22 } else { 0.90 }),
                    );
                }
                ui.center(lbl, pr, 10.5, if idx == 2 { fg } else { muted }, idx == 2);
            }
        }
        Kind::Music => {
            // Deskset 黑胶唱机 + 高饱和洋红圆角柱状频谱 + 透明亚克力播控条
            let vinyl_sz = if large { 142. } else if small { 76. } else { (r.height() - 40.).clamp(88., 128.) };
            let vinyl_x = if small { r.center().x - vinyl_sz * 0.5 } else { x };
            let vinyl_y = if large { y + 10. } else if small { y + 2. } else { r.center().y - vinyl_sz * 0.5 };
            let disk_r = vinyl_sz * 0.44;
            let cx = vinyl_x + vinyl_sz * 0.46;
            let cy = vinyl_y + vinyl_sz * 0.50;

            // 唱片悬浮外微影 + 深邃黑胶纹理环
            ui.scene.fill(
                Fill::NonZero,
                Affine::translate((0., 2.5)),
                draw::alpha(0x050810, if dark { 0.36 } else { 0.16 }),
                None,
                &vello::kurbo::Circle::new((cx, cy), disk_r),
            );
            let vinyl_grad = Gradient::new_linear((cx, cy - disk_r), (cx, cy + disk_r)).with_stops([
                (0.0, draw::color(0x272A33)),
                (0.5, draw::color(0x111318)),
                (1.0, draw::color(0x1C1F27)),
            ]);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &vinyl_grad, None, &vello::kurbo::Circle::new((cx, cy), disk_r));
            for ring_k in [0.88, 0.74, 0.60] {
                ui.scene.stroke(
                    &Stroke::new(0.65),
                    Affine::IDENTITY,
                    draw::alpha(0xFFFFFF, 0.085),
                    None,
                    &vello::kurbo::Circle::new((cx, cy), disk_r * ring_k),
                );
            }
            // 唱片中心标签：若有专辑图则在中心圆标绘制，否则绘制 Deskset 青蓝渐变条纹标
            let lbl_r = disk_r * 0.39;
            let lbl_shape = vello::kurbo::Circle::new((cx, cy), lbl_r);
            if let Some(art) = &m.artwork {
                ui.scene.push_clip_layer(Affine::IDENTITY, &lbl_shape);
                draw::image(ui.scene, &art.image, rect(cx - lbl_r, cy - lbl_r, lbl_r * 2., lbl_r * 2.), lbl_r);
                ui.scene.pop_layer();
            } else {
                let lbl_grad = Gradient::new_linear((cx - lbl_r, cy), (cx + lbl_r, cy)).with_stops([
                    (0.0, draw::color(0x0284C7)),
                    (1.0, draw::color(0x38BDF8)),
                ]);
                ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &lbl_grad, None, &lbl_shape);
                for b in 0..4 {
                    let bw = lbl_r * (0.62 - (b as f64) * 0.08);
                    let by = cy - lbl_r * 0.42 + b as f64 * (lbl_r * 0.28);
                    fill_alpha_rounded(ui, rect(cx - bw * 0.5, by, bw, 2.4), 1.2, draw::alpha(0xFFFFFF, 0.90));
                }
            }
            draw::circle(ui.scene, cx, cy, (lbl_r * 0.22).max(2.6), if dark { 0x161923 } else { 0xF8FAFC });

            // 右上精工银色金属唱臂
            let arm_piv = (vinyl_x + vinyl_sz * 0.90, vinyl_y + vinyl_sz * 0.16);
            let arm_tip = (cx + disk_r * 0.52, cy + disk_r * 0.34);
            draw::circle(ui.scene, arm_piv.0, arm_piv.1, (vinyl_sz * 0.065).max(3.5), 0xCBD5E1);
            ui.scene.stroke(
                &Stroke::new(2.0).with_caps(vello::kurbo::Cap::Round),
                Affine::IDENTITY,
                draw::alpha(if dark { 0xE2E8F0 } else { 0x64748B }, 0.92),
                None,
                &vello::kurbo::Line::new(arm_piv, arm_tip),
            );
            fill_alpha_rounded(ui, rect(arm_tip.0 - 3., arm_tip.1 - 2., 6., 9.), 2., draw::color(0x334155));

            if small {
                let title_y = vinyl_y + vinyl_sz + 4.;
                ui.center(m.title(), rect(x, title_y, w, 18.), 12., fg, true);
                let bw = 28.;
                let gap = 8.;
                let total = bw * 3. + gap * 2.;
                for (i, action) in ["previous", "toggle", "next"].iter().enumerate() {
                    views::transport_control(
                        ui,
                        m,
                        rect(r.center().x - total / 2. + i as f64 * (bw + gap), r.y1 - 40., bw, bw),
                        [3, 4, 5][i],
                        ["上一首", "播放或暂停", "下一首"][i],
                        action,
                    );
                }
            } else {
                let rx = if large { x } else { vinyl_x + vinyl_sz + 14. };
                let rw = if large { w } else { (x + w - rx).max(100.) };
                let info_y = if large { vinyl_y + vinyl_sz + 12. } else { y + 6. };
                ui.label(m.title(), rx, info_y, if large { 17. } else { 15. }, fg, true, rw);
                ui.label(m.artist(), rx, info_y + 23., 11., muted, false, rw);

                // Deskset 标志性高饱和洋红圆头音频频谱柱
                let spec_y = info_y + 46.;
                let spec_h = if large { 56. } else { 34. };
                let bar_cnt = if large { 22 } else { 16 };
                let bar_gap = 3.0;
                let bar_w = ((rw - bar_gap * (bar_cnt as f64 - 1.0)) / bar_cnt as f64).clamp(3.0, 7.5);
                let amp_scale = if m.playing() { 1.0 } else { 0.42 };
                for bi in 0..bar_cnt {
                    let phase = state.time * (if m.playing() { 3.8 } else { 0.8 }) + bi as f64 * 0.48;
                    let env = 0.25 + 0.75 * (0.5 + 0.5 * (phase.sin() * 0.65 + (phase * 1.7 + 1.1).cos() * 0.35));
                    let bh = (spec_h * env * amp_scale).clamp(4.0, spec_h);
                    let bx = rx + bi as f64 * (bar_w + bar_gap);
                    let br = rect(bx, spec_y + spec_h - bh, bar_w, bh);
                    let spec_grad = Gradient::new_linear((br.x0, br.y0), (br.x0, br.y1)).with_stops([
                        (0.0, draw::color(0xFB7185)),
                        (1.0, draw::color(0xF43F5E)),
                    ]);
                    ui.scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        &spec_grad,
                        None,
                        &RoundedRect::from_rect(br, bar_w * 0.5),
                    );
                }

                if large {
                    ui.label(m.current_lyric(), rx, spec_y + spec_h + 12., 12., muted, false, rw);
                    views::progress(ui, m, rect(rx, r.y1 - 74., rw, 16.), state);
                }
                let bw = 30.;
                let gap = if large { 18. } else { 12. };
                let total = bw * 3. + gap * 2.;
                let dock_cx = if large { r.center().x } else { rx + rw * 0.5 };
                subtle_tile(ui, rect(dock_cx - total / 2. - 8., r.y1 - 45., total + 16., 36.), 18., 0xF43F5E, dark);
                for (i, action) in ["previous", "toggle", "next"].iter().enumerate() {
                    views::transport_control(
                        ui,
                        m,
                        rect(dock_cx - total / 2. + i as f64 * (bw + gap), r.y1 - 42., bw, bw),
                        [3, 4, 5][i],
                        ["上一首", "播放或暂停", "下一首"][i],
                        action,
                    );
                }
            }
        }
        Kind::System => {
            for (i, (label, value)) in [("CPU", metrics.cpu_percent), ("内存", metrics.memory_percent)]
                .iter()
                .enumerate()
            {
                let xx = x + if small { 0. } else { i as f64 * (w / 2. + 4.) };
                let yy = y + if small { 27. + i as f64 * 52. } else { 29. };
                let cw = if small { w } else { w / 2. - 4. };
                let ch = if small { 46. } else if large { 246. } else { 106. };
                let tint = if i == 0 { blue } else { 0xAF52DE };
                subtle_tile(ui, rect(xx, yy, cw, ch), 14., tint, dark);
                draw::circle(ui.scene, xx + 12., yy + 14., 3.0, tint);
                ui.label(label, xx + 19., yy + 7., 10.5, muted, true, cw - 28.);
                ui.label(
                    &value.map(|v| format!("{v:.0}%")).unwrap_or("—".into()),
                    xx + 10.,
                    yy + if small { 21. } else { 28. },
                    if small { 18. } else { 30. },
                    fg,
                    true,
                    cw - 20.,
                );
                if !large {
                    let frac = value.unwrap_or(0.) / 100.;
                    let bar_y = yy + ch - if small { 8. } else { 14. };
                    let bar_x = if small { xx + cw * 0.56 } else { xx + 10. };
                    let bar_w = if small { cw * 0.44 - 10. } else { cw - 20. };
                    let bar_top = if small { yy + 20. } else { bar_y };
                    paint_apple_meter(ui, rect(bar_x, bar_top, bar_w, 5.5), frac, mix(tint, 0xFFFFFF, 0.2), tint, dark);
                } else {
                    let cy = yy + 168.;
                    ring_track(ui, (xx + cw / 2., cy), 45., 6.0, tint, dark);
                    if let Some(value) = value {
                        draw::ring(ui.scene, (xx + cw / 2., cy), 45., value / 100., tint, 6.0);
                    }
                    ui.center(
                        &value.map(|v| format!("{v:.0}%")).unwrap_or("—".into()),
                        rect(xx, cy - 12., cw, 24.),
                        14.,
                        fg,
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
                subtle_tile(ui, rect(x, y + 28., w, r.height() - 58.), 16., accent, dark);
                {
                    use vello::kurbo::Circle;
                    ui.scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        draw::alpha(accent, if dark { 0.20 } else { 0.14 }),
                        None,
                        &Circle::new((r.center().x, y + 66.), 20.),
                    );
                }
                ui.icon(Icon::Volume, r.center().x - 12., y + 54., 24., accent);
                ui.center(
                    "暂无已连接无线设备",
                    rect(x + 8., y + 92., w - 16., 22.),
                    if small { 12. } else { 14. },
                    fg,
                    true,
                );
            }
            let cols = if small { 1 } else { 2 };
            let gap = 8.;
            let cw = (w - gap * (cols - 1) as f64) / cols as f64;
            let ch = if large { 126. } else { (r.height() - 56.).max(88.) };
            for (i, device) in devices.iter().take(if large { 4 } else { cols }).enumerate() {
                let xx = x + (i % cols) as f64 * (cw + gap);
                let yy = y + 27. + (i / cols) as f64 * (ch + 8.);
                let cell_r = rect(xx, yy, cw, ch);
                let dev_col = match device.battery_percent {
                    Some(v) if v <= 20 => 0xFF9F0A,
                    _ => 0x30D158,
                };
                subtle_tile(ui, cell_r, 15., dev_col, dark);
                let cx = xx + cw / 2.;
                let radius = if small { 22. } else { 26. };
                let cy = yy + 10. + radius;
                ring_track(ui, (cx, cy), radius, 4.5, dev_col, dark);
                if let Some(level) = device.battery_percent {
                    draw::ring(ui.scene, (cx, cy), radius, level as f64 / 100., dev_col, 4.5);
                }
                device_icon(ui, (cx - 12., cy - 12.), &device.kind, fg);
                ui.center(
                    &device.battery_percent.map(|v| format!("{v}%")).unwrap_or("—".into()),
                    rect(xx + 4., cy + radius + 5., cw - 8., 22.),
                    16.,
                    fg,
                    true,
                );
                ui.center(&device.name, rect(xx + 4., cy + radius + 27., cw - 8., 16.), 10., muted, false);
            }
        }
        Kind::Note => {
            // 纯净瓷白旗舰速记卡：释放内部嵌套灰框，顶部极简标题 + 琥珀进度徽章 + 透气清单行
            let head_r = rect(x, y, w, 22.);
            draw::circle(ui.scene, x + 4.5, y + 10.5, 3.2, 0xF59E0B);
            ui.label(title, x + 14., y + 2., 12.5, fg, true, (w - 64.).max(40.));

            let checks_all: Vec<&str> = config
                .text
                .lines()
                .map(|l| l.trim_start())
                .filter(|t| t.starts_with("- [ ] ") || t.starts_with("- [x] ") || t.starts_with("- [X] "))
                .collect();
            let done_cnt = checks_all
                .iter()
                .filter(|t| t.starts_with("- [x] ") || t.starts_with("- [X] "))
                .count();
            if !checks_all.is_empty() {
                let badge_txt = format!("{done_cnt}/{}", checks_all.len());
                let br = rect(head_r.x1 - 40., y + 1., 40., 19.);
                subtle_tile(ui, br, 9.5, 0xF59E0B, dark);
                ui.center(&badge_txt, br, 10., 0xD97706, true);
            }
            // 极淡水平发丝分隔线
            fill_alpha_rounded(
                ui,
                rect(x, y + 26., w, 0.8),
                0.4,
                draw::alpha(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.08 } else { 0.055 }),
            );

            let box_rect = rect(x, y + 32., w, (r.height() - 60.).max(64.));
            if let Some(edit) = extras.note_edit.filter(|e| e.widget_id == id) {
                let composed = if edit.preedit.is_empty() {
                    edit.text.clone()
                } else {
                    format!("{}{}", edit.text, edit.preedit)
                };
                if composed.is_empty() {
                    ui.label("直接键入便签内容，支持 - [ ] 任务行", x + 4., y + 38., 12., muted, false, w - 8.);
                } else {
                    text_lines(ui, &composed, x + 4., y + 38., w - 8., if large { 8 } else { 3 }, 12.5, fg);
                }
                if (state.time * 2.).rem_euclid(2.) < 1. {
                    let last_line = composed.lines().last().unwrap_or("");
                    let cursor_x = (x + 4. + ui.measure_alpha(last_line, 12.5, false)).min(box_rect.x1 - 6.);
                    let line_cnt = composed.lines().count().clamp(1, if large { 8 } else { 3 });
                    let cursor_y = y + 38. + (line_cnt.saturating_sub(1)) as f64 * 18.;
                    draw::fill_rounded(ui.scene, rect(cursor_x, cursor_y, 1.6, 15.), 0.8, accent);
                }
                let bw = ((box_rect.width() - 16.) / 2.).max(40.);
                let by = box_rect.y1 - 30.;
                ui.button(rect(box_rect.x0, by, bw, 28.), "取消", None, Action::NoteCancel, true, true);
                ui.button(rect(box_rect.x0 + 12. + bw, by, bw, 28.), "保存", None, Action::NoteSave, false, true);
            } else if !checks_all.is_empty() {
                let max_rows = if large { 8 } else { 3 };
                let row_h = 27.0;
                let mut drawn_bottom = y + 34.;
                for (line_idx, raw_line) in config.text.lines().take(max_rows).enumerate() {
                    let ry = y + 34. + line_idx as f64 * row_h;
                    drawn_bottom = ry + row_h;
                    let row_r = rect(x - 4., ry, w + 8., row_h - 2.);
                    if row_r.contains(ui.pointer) {
                        fill_alpha_rounded(
                            ui,
                            row_r,
                            8.,
                            draw::alpha(0x0A84FF, if dark { 0.12 } else { 0.06 }),
                        );
                    }
                    let trimmed = raw_line.trim_start();
                    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
                        draw::outline_rounded(ui.scene, rect(x + 2., ry + 5., 14.5, 14.5), 7.2, 0xF59E0B, 1.5);
                        ui.label(rest, x + 24., ry + 2.5, 12.5, fg, false, (w - 28.).max(40.));
                        ui.hit(row_r.intersect(r), "勾选便签待办", Action::NoteToggleCheck(id.into(), line_idx), true);
                    } else if let Some(rest) = trimmed.strip_prefix("- [x] ").or_else(|| trimmed.strip_prefix("- [X] ")) {
                        draw::fill_rounded(ui.scene, rect(x + 2., ry + 5., 14.5, 14.5), 7.2, 0x30D158);
                        ui.icon(Icon::Check, x + 4.2, ry + 7.2, 10., 0xFFFFFF);
                        ui.label(rest, x + 24., ry + 2.5, 12.5, muted, false, (w - 28.).max(40.));
                        ui.hit(row_r.intersect(r), "取消勾选便签待办", Action::NoteToggleCheck(id.into(), line_idx), true);
                    } else {
                        ui.label(raw_line, x + 2., ry + 2.5, 12.5, fg, false, w - 4.);
                    }
                }
                if extras.use_allegro_shell && box_rect.y1 - drawn_bottom > 12. {
                    ui.hit(
                        rect(box_rect.x0, drawn_bottom, box_rect.width(), (box_rect.y1 - drawn_bottom).max(12.)).intersect(r),
                        "原地编辑便签",
                        Action::NoteInlineEdit(id.into()),
                        true,
                    );
                }
            } else {
                text_lines(
                    ui,
                    if config.text.is_empty() { "点击此处快速记录想法或 - [ ] 任务" } else { &config.text },
                    x + 2.,
                    y + 36.,
                    w - 4.,
                    if large { 8 } else { 3 },
                    if small { 12.5 } else { 13.5 },
                    if config.text.is_empty() { muted } else { fg },
                );
                if extras.use_allegro_shell {
                    ui.hit(box_rect.intersect(r), "原地编辑便签", Action::NoteInlineEdit(id.into()), true);
                }
            }
        }
        Kind::Anniversary => {
            if let Some(date) = widget_config::date_ordinal(&config.date) {
                let delta = date - widget_config::ordinal(crate::desktop_cards::date());
                ui.label(&delta.abs().to_string(), x, y + 28., if small { 46. } else { 58. }, fg, true, w);
                ui.label(
                    if delta >= 0 { "天后 · 迎接这一天" } else { "天 · 已经一起走过" },
                    x,
                    y + 92.,
                    11.,
                    muted,
                    false,
                    w,
                );
                let pill_y = r.y1 - 40.;
                subtle_tile(ui, rect(x, pill_y, w, 24.), 12., accent, dark);
                draw::circle(ui.scene, x + 11., pill_y + 12., 2.8, accent);
                ui.label(&config.date, x + 20., pill_y + 4.5, 11.5, accent, true, w - 28.);
            } else {
                subtle_tile(ui, rect(x, y + 32., w, (r.height() - 64.).max(72.)), 16., accent, dark);
                ui.center("设置重要日期", rect(x, y + 32., w, (r.height() - 64.).max(72.)), if small { 15. } else { 18. }, fg, true);
            }
        }
        Kind::Weather => {
            // Deskset 透明亚克力天气卡（移除不透明蓝底，保留轻柔天空散射微晕 + 温度面积曲线）
            let sky_shape = allegro_theme::squircle_path(r, 24.0);
            let sky_glaze = Gradient::new_radial(
                (r.x1 - 24., r.y0 + 24.),
                r.width().max(r.height()) as f32 * 0.90,
            )
            .with_stops([
                (0.0, allegro_theme::rgba(0x389BFF, if dark { 0.14 } else { 0.12 })),
                (0.55, allegro_theme::rgba(0xFBBF24, if dark { 0.05 } else { 0.06 })),
                (1.0, allegro_theme::rgba(0xFFFFFF, 0.0)),
            ]);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &sky_glaze, None, &sky_shape);

            let city = data["city"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(if config.city.is_empty() { "上海市" } else { &config.city });
            let temp_opt = data["temperature"].as_f64();
            let temp_val = temp_opt.unwrap_or(22.0);
            let high_val = data["high"].as_f64().unwrap_or(temp_val + 4.0);
            let low_val = data["low"].as_f64().unwrap_or(temp_val - 6.0);
            let cond = data["condition"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(if temp_opt.is_some() { "晴 · 体感舒适" } else { "晴 · 微风轻拂" });

            draw::circle(ui.scene, x + 4.5, y + 8.5, 3.0, 0x389BFF);
            ui.label(city, x + 13., y + 1., 12., muted, true, (w * 0.55).max(48.));

            if small {
                paint_weather_sun_cloud(ui.scene, (r.x1 - 36., y + 26.), dark);
                ui.label(&format!("{temp_val:.0}°"), x - 2., y + 22., 42., fg, true, w - 52.);
                ui.label(&format!("最高{high_val:.0}° · 最低{low_val:.0}°"), x, y + 74., 10.5, muted, false, w);
                let curve_r = rect(x, r.y1 - 44., w, 28.);
                paint_weather_curve(ui.scene, curve_r, 0x34C759, dark);
            } else {
                let left_w = (w * 0.44).max(110.);
                paint_weather_sun_cloud(ui.scene, (x + left_w - 28., y + 36.), dark);
                ui.label(&format!("{temp_val:.0}°"), x - 2., y + 20., 46., fg, true, left_w - 46.);
                ui.label(cond, x, y + 80., 11.5, fg, true, left_w);
                ui.label(&format!("最高{high_val:.0}°  最低{low_val:.0}°"), x, y + 98., 10.5, muted, false, left_w);

                let rx = x + left_w + 10.;
                let rw = (x + w - rx).max(110.);
                let col_w = rw / 4.;
                let hours = [("现在", temp_val), ("14时", temp_val + 2.), ("16时", high_val), ("18时", temp_val - 1.)];
                for (hi, (lbl, t)) in hours.iter().enumerate() {
                    let cx = rx + hi as f64 * col_w;
                    ui.center(lbl, rect(cx, y + 2., col_w, 16.), 9.5, muted, false);
                    draw::circle(ui.scene, cx + col_w * 0.5, y + 32., 5.0, 0xFF9F0A);
                    ui.center(&format!("{t:.0}°"), rect(cx, y + 44., col_w, 18.), 11.5, fg, true);
                }
                let curve_y = y + 70.;
                let curve_h = (r.y1 - 24. - curve_y).clamp(28., 110.);
                paint_weather_curve(ui.scene, rect(rx, curve_y, rw, curve_h), 0x34C759, dark);
            }
        }
        Kind::Today => {
            let pending = m.todos.items.iter().filter(|t| !t.done(&dotisle_domain::todos::Now::local())).count();
            let timers = m.timers.items.iter().filter(|t| t.running).count();
            for (i, (value, label, tint)) in [(pending, "待办事项", 0x30D158), (timers, "运行中计时", 0xFF7B61)].iter().enumerate() {
                let xx = x + if small { 0. } else { i as f64 * (w / 2. + 4.) };
                let yy = y + if small { 27. + i as f64 * 48. } else { 30. };
                let cw = if small { w } else { w / 2. - 4. };
                let ch = if small { 42. } else { 104. };
                subtle_tile(ui, rect(xx, yy, cw, ch), 15., *tint, dark);
                if small {
                    draw::circle(ui.scene, xx + 12., yy + ch / 2., 3.2, *tint);
                    ui.label(&value.to_string(), xx + 22., yy + 5., 24., *tint, true, 44.);
                    ui.label(label, xx + 62., yy + 13., 11., muted, false, cw - 72.);
                } else {
                    draw::circle(ui.scene, xx + cw - 16., yy + 18., 4.2, *tint);
                    ui.label(&value.to_string(), xx + 14., yy + 12., 40., *tint, true, cw - 28.);
                    ui.label(label, xx + 14., yy + 68., 11.5, muted, true, cw - 28.);
                }
            }
            if large {
                ui.button(rect(x, r.y1 - 48., w, 34.), "查看待办事项", None, Action::Navigate(Page::Todos), true, true);
            }
        }
        Kind::Battery => {
            let pct = metrics.battery_percent;
            let bat_col = if pct.is_some_and(|v| v <= 20) { 0xFF9F0A } else { 0x30D158 };
            let status_txt = if pct.is_none() {
                "未检测到电池"
            } else if metrics.external_power == Some(true) {
                "已连接电源"
            } else {
                "正在使用电池"
            };
            if large {
                let cx = r.center().x;
                let cy = y + 122.;
                ring_track(ui, (cx, cy), 64., 8.5, bat_col, dark);
                if let Some(value) = pct {
                    draw::ring(ui.scene, (cx, cy), 64., value as f64 / 100., bat_col, 8.5);
                }
                ui.center(&pct.map(|v| format!("{v}%")).unwrap_or("—".into()), rect(x, cy - 26., w, 52.), 38., fg, true);
                subtle_tile(ui, rect(x, r.y1 - 44., w, 28.), 14., bat_col, dark);
                draw::circle(ui.scene, x + 12., r.y1 - 30., 3.2, bat_col);
                ui.label(status_txt, x + 22., r.y1 - 37.5, 11.5, muted, true, w - 32.);
            } else {
                let ring = if small { 34. } else { 64. };
                let cx = r.x1 - 22. - ring / 2.;
                let cy = y + if small { 46. } else { 64. };
                ring_track(ui, (cx, cy), ring / 2., 5.0, bat_col, dark);
                if let Some(value) = pct {
                    draw::ring(ui.scene, (cx, cy), ring / 2., value as f64 / 100., bat_col, 5.0);
                }
                ui.label(
                    &pct.map(|v| format!("{v}%")).unwrap_or("—".into()),
                    x,
                    y + 28.,
                    if small { 34. } else { 44. },
                    fg,
                    true,
                    w - ring - 14.,
                );
                let pill_y = r.y1 - 38.;
                subtle_tile(ui, rect(x, pill_y, (w - ring - 14.).max(92.), 22.), 11., bat_col, dark);
                ui.label(status_txt, x + 9., pill_y + 4., 10.5, muted, false, (w - ring - 28.).max(72.));
            }
        }
        Kind::Storage => {
            if let (Some(free), Some(total)) = (data["free"].as_f64(), data["total"].as_f64().filter(|v| *v > 0.)) {
                let used_ratio = (1. - free / total).clamp(0., 1.);
                ui.label(&format!("{:.0}", free / 1e9), x, y + 26., clock_size, fg, true, w);
                ui.label(&format!("GB 可用 · 共 {:.0} GB", total / 1e9), x, y + 86., 11., muted, false, w);
                paint_apple_meter(
                    ui,
                    rect(x, r.y1 - 30., w, 8.),
                    used_ratio,
                    mix(accent, 0x38BDF8, 0.35),
                    accent,
                    dark,
                );
                if large {
                    subtle_tile(ui, rect(x, y + 126., w, 46.), 14., accent, dark);
                    ui.label("已使用空间比例", x + 12., y + 141., 12., muted, false, w - 80.);
                    ui.label(&format!("{:.0}%", used_ratio * 100.), r.x1 - 64., y + 139., 15., accent, true, 52.);
                }
            } else {
                subtle_tile(ui, rect(x, y + 32., w, (r.height() - 64.).max(72.)), 16., accent, dark);
                ui.center("暂无磁盘读数", rect(x, y + 32., w, (r.height() - 64.).max(72.)), if small { 14. } else { 18. }, fg, true);
            }
        }
        Kind::WorldClock => {
            ui.label(data["clock"].as_str().unwrap_or("—"), x, y + 28., clock_size, fg, true, w);
            subtle_tile(ui, rect(x, y + 90., w, 26.), 13., accent, dark);
            draw::circle(ui.scene, x + 11., y + 103., 2.6, accent);
            ui.label(
                data["city"].as_str().unwrap_or(if config.city.is_empty() { "选择城市" } else { &config.city }),
                x + 20.,
                y + 95.,
                11.5,
                fg,
                true,
                w - 28.,
            );
            if large {
                subtle_tile(ui, rect(x, y + 128., w, 34.), 12., accent, dark);
                ui.label(data["date"].as_str().unwrap_or("等待时间数据"), x + 12., y + 137., 12.5, muted, false, w - 24.);
            }
        }
        Kind::Launcher => {
            {
                use vello::kurbo::Circle;
                let c = (x + 25., y + 58.);
                ui.scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    draw::alpha(accent, if dark { 0.24 } else { 0.16 }),
                    None,
                    &Circle::new(c, 25.),
                );
                ui.scene.stroke(
                    &Stroke::new(0.9),
                    Affine::IDENTITY,
                    draw::alpha(accent, 0.35),
                    None,
                    &Circle::new(c, 25.),
                );
            }
            ui.icon(Icon::Arrow, x + 12., y + 45., 26., accent);
            ui.label(
                if config.title.is_empty() { "常用入口" } else { &config.title },
                x,
                y + 94.,
                if small { 16. } else { 21. },
                fg,
                true,
                w,
            );
            if !config.target.is_empty() {
                ui.hit(rect(r.x0, r.y0 + 32., r.width(), r.height() - 32.), "打开快捷入口", Action::WidgetLaunch(id.into()), true);
            } else {
                ui.label("右键编辑设置目标", x, r.y1 - 29., 10.5, muted, false, w);
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
                {
                    use vello::kurbo::Circle;
                    ui.scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        draw::alpha(accent, if dark { 0.22 } else { 0.14 }),
                        None,
                        &Circle::new((x + 15., y + 39.), 14.),
                    );
                }
                ui.icon(Icon::Focus, x + 5., y + 29., 20., accent);
                ui.label(&value, x, y + 63., if small { 22. } else { 30. }, fg, true, w);
                ui.label(&config.deadline, x, y + 106., 11., muted, false, w);
                if large {
                    subtle_tile(ui, rect(x, r.y1 - 44., w, 28.), 14., accent, dark);
                    ui.label(
                        if left == 0 { "重要日程已到时间" } else { "每一刻，都更近一点" },
                        x + 12.,
                        r.y1 - 37.5,
                        12.,
                        muted,
                        false,
                        w - 24.,
                    );
                }
            } else {
                subtle_tile(ui, rect(x, y + 32., w, (r.height() - 64.).max(72.)), 16., accent, dark);
                ui.icon(Icon::Focus, x + 14., y + 46., 24., accent);
                ui.label("设置日程倒数", x + 46., y + 48., 17., fg, true, w - 58.);
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
                        "divider" => fill_alpha_rounded(ui, rect(x, yy + 5. * scale, w, 1.), 0.5, glass_track_color(ui, accent, dark)),
                        "progress" => {
                            ui.label(&b.label, x, yy, (11. * scale).max(9.) as f32, muted, false, w);
                            paint_apple_meter(ui, rect(x, yy + 24. * scale, w, 5.), b.value, mix(accent, 0xFFFFFF, 0.15), accent, dark);
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
            ui.label(&value, x, y + 28., if small { 24. } else { 32. }, fg, true, w);
            ui.label("今日活跃用机", x, y + 70., 11., muted, false, w);
            let yesterday = stats["yesterdayMs"].as_f64().map(|v| format!("昨日 {}", usage_duration(v))).unwrap_or("昨日暂无记录".into());
            subtle_tile(ui, rect(x, y + 96., w, 26.), 13., accent, dark);
            draw::circle(ui.scene, x + 11., y + 109., 2.6, accent);
            ui.label(&yesterday, x + 20., y + 101.5, 11., muted, false, w - 28.);
            if large {
                let values = stats["week"].as_array();
                let peak = values.map(|a| a.iter().filter_map(|v| v.as_f64()).fold(60000., f64::max)).unwrap_or(60000.);
                bar_chart_bars(ui, x, r.y1 - 44., w, 108., values, peak, accent, dark);
                draw::circle(ui.scene, x + 4., r.y1 - 21., 2.5, accent);
                ui.label("近7天 · 仅统计点屿运行期间", x + 11., r.y1 - 27., 9.5, muted, false, w - 12.);
            }
        }
        Kind::FocusStats => {
            let stats = &m.native_state["widgetData"]["focusStats"];
            let minutes = stats["todayMs"].as_f64().unwrap_or(0.) / 60000.;
            ui.label(&format!("{minutes:.0}"), x, y + 26., clock_size, fg, true, w);
            let sub_y = y + 86.;
            subtle_tile(ui, rect(x, sub_y, w, 24.), 12., accent, dark);
            draw::circle(ui.scene, x + 11., sub_y + 12., 2.6, accent);
            ui.label("今日专注 · 分钟", x + 20., sub_y + 4.5, 11., muted, false, w - 28.);
            if large {
                let values = stats["week"].as_array();
                let maximum = values.map(|a| a.iter().filter_map(|v| v.as_f64()).fold(1., f64::max)).unwrap_or(1.);
                bar_chart_bars(ui, x, r.y1 - 36., w, 108., values, maximum, accent, dark);
            }
        }
        Kind::PhotoFrame => {
            let bottom_pad = 26.;
            let frame_r = rect(x, y + 2., w, (r.height() - 34. - bottom_pad).max(64.));
            if let Some(img) = extras.photo_image {
                draw::image(ui.scene, img, frame_r, 14.);
                ui.scene.stroke(
                    &Stroke::new(0.8),
                    Affine::IDENTITY,
                    draw::alpha(if dark { 0xFFFFFF } else { 0x0F172A }, 0.14),
                    None,
                    &RoundedRect::from_rect(frame_r, 14.),
                );
                ui.hit(frame_r, "切换下一张照片", Action::PhotoNext(id.into()), true);
                let caption = if config.caption.is_empty() { "桌面相框 · 点击切换" } else { &config.caption };
                let (cur, tot) = extras.photo_meta;
                ui.label(caption, x + 2., frame_r.y1 + 7., 11., fg, true, (w - 42.).max(40.));
                if tot > 0 {
                    ui.label(&format!("{}/{}", cur + 1, tot), r.x1 - 52., frame_r.y1 + 7., 10.5, muted, true, 38.);
                }
            } else {
                subtle_tile(ui, frame_r, 15., accent, dark);
                ui.icon(Icon::Sun, frame_r.center().x - 14., frame_r.center().y - 22., 28., accent);
                ui.center(
                    if config.folder.is_empty() { "拖入照片或目录" } else { "目录中暂无图片" },
                    rect(frame_r.x0 + 8., frame_r.center().y + 10., frame_r.width() - 16., 20.),
                    11.5,
                    fg,
                    true,
                );
                let cap = if config.caption.is_empty() { "拍立得相框 · PNG / JPG" } else { &config.caption };
                ui.label(cap, x + 2., frame_r.y1 + 7., 10.5, muted, false, w - 4.);
            }
        }
        Kind::Dropzone => {
            let items = extras.dropzone_items;
            if items.is_empty() && config.items.is_empty() {
                let zone_r = rect(x, y + 26., w, (r.height() - 56.).max(72.));
                subtle_tile(ui, zone_r, 16., accent, dark);
                ui.icon(Icon::Arrow, zone_r.center().x - 12., zone_r.center().y - 24., 24., accent);
                ui.center(
                    "拖放文件至此暂存",
                    rect(zone_r.x0 + 8., zone_r.center().y + 4., zone_r.width() - 16., 20.),
                    if small { 12. } else { 13.5 },
                    fg,
                    true,
                );
                if !small {
                    ui.center(
                        "点击条目快捷打开，无需堆满桌面",
                        rect(zone_r.x0 + 8., zone_r.center().y + 24., zone_r.width() - 16., 18.),
                        10.,
                        muted,
                        false,
                    );
                }
            } else {
                let count = if !items.is_empty() { items.len() } else { config.items.len() };
                ui.label(&format!("已暂存 {count} 项"), x, y + 26., 10.5, muted, false, (w - 56.).max(40.));
                ui.button(rect(r.x1 - 64., y + 23., 48., 20.), "清空", None, Action::DropzoneClear(id.into()), true, true);
                let max_rows = if large { 6 } else { 2 };
                let row_h = if large { 38. } else { 36. };
                for idx in 0..count.min(max_rows) {
                    let ry = y + 48. + idx as f64 * (row_h + 6.);
                    if ry + row_h > r.y1 - 8. {
                        break;
                    }
                    let row_r = rect(x, ry, w, row_h);
                    subtle_tile(ui, row_r, 10., accent, dark);
                    let (tag, name, size_lbl) = if let Some(it) = items.get(idx) {
                        (it.tag, it.name.as_str(), it.size_label.as_str())
                    } else {
                        ("FILE", config.items[idx].as_str(), "已保存")
                    };
                    draw::fill_rounded(ui.scene, rect(x + 8., ry + 8., 34., 20.), 6., mix(s.container, accent, 0.22));
                    ui.center(tag, rect(x + 8., ry + 8., 34., 20.), 9., accent, true);
                    ui.label(name, x + 48., ry + 9., 12., fg, true, (w - 110.).max(40.));
                    ui.label(size_lbl, row_r.x1 - 56., ry + 10., 10., muted, false, 48.);
                    ui.hit(row_r, "打开暂存文件", Action::DropzoneOpen(id.into(), idx), true);
                }
            }
        }
        Kind::SystemPro => {
            // Deskset 四色圆环仪表盘（CPU / MEMORY / DISK / GPU）+ Smart Stack 微点阵与脉冲波形
            paint_micro_dot_grid(ui.scene, r, 24.0, dark);
            let cpu = metrics.cpu_percent;
            let gpu = extras.pro.and_then(|p| p.gpu_percent).or(metrics.gpu_percent);
            let mem = metrics.memory_percent;
            let rx = extras.pro.and_then(|p| p.net_rx_kbps).or(metrics.net_rx_kbps);
            let tx = extras.pro.and_then(|p| p.net_tx_kbps).or(metrics.net_tx_kbps);

            let net_text = format!("↓ {}  ↑ {}", format_kbps(rx), format_kbps(tx));
            ui.label(&net_text, x + 92., y + 1., 9.5, muted, false, (w - 92.).max(48.));

            let gauges = [
                ("CPU", cpu.unwrap_or(27.0), 0x0A84FF_u32, 0usize),
                ("MEMORY", mem.unwrap_or(48.0), 0x30D158_u32, 1usize),
                ("DISK", 55.0, 0xFF9F0A_u32, 2usize),
                ("GPU", gpu.unwrap_or(23.0), 0xAF52DE_u32, 3usize),
            ];

            if small {
                let gx_step = w / 2.;
                let top_y = y + 26.;
                let row_h = (r.y1 - 14. - top_y) / 2.;
                let ring_r = (row_h * 0.25).clamp(11., 14.);
                for (i, (lbl, val, tint, glyph)) in gauges.iter().enumerate() {
                    let col = i % 2;
                    let row = i / 2;
                    let cell_x = x + col as f64 * gx_step;
                    let cell_y = top_y + row as f64 * row_h;
                    let rcx = cell_x + gx_step * 0.5;
                    let rcy = cell_y + ring_r + 2.;
                    ring_track(ui, (rcx, rcy), ring_r, 3.8, *tint, dark);
                    draw::ring(ui.scene, (rcx, rcy), ring_r, (*val / 100.).clamp(0.04, 1.), *tint, 3.8);
                    paint_pro_glyph(ui.scene, (rcx, rcy), *glyph, *tint);
                    ui.center(&format!("{val:.0}%"), rect(cell_x, rcy + ring_r + 2., gx_step, 14.), 11.5, fg, true);
                    ui.center(lbl, rect(cell_x, rcy + ring_r + 16., gx_step, 11.), 8., muted, false);
                }
            } else {
                let col_w = w / 4.;
                let ring_r = if large { 27. } else { 23. };
                let rcy = y + 28. + ring_r;
                for (i, (lbl, val, tint, glyph)) in gauges.iter().enumerate() {
                    let cell_x = x + i as f64 * col_w;
                    let rcx = cell_x + col_w * 0.5;
                    ring_track(ui, (rcx, rcy), ring_r, 5.2, *tint, dark);
                    draw::ring(ui.scene, (rcx, rcy), ring_r, (*val / 100.).clamp(0.04, 1.), *tint, 5.2);
                    paint_pro_glyph(ui.scene, (rcx, rcy), *glyph, *tint);
                    ui.center(&format!("{val:.0}%"), rect(cell_x, rcy + ring_r + 10., col_w, 20.), 16.5, fg, true);
                    ui.center(lbl, rect(cell_x, rcy + ring_r + 32., col_w, 14.), 9., muted, false);
                }
                if large {
                    let wave_top = rcy + ring_r + 58.;
                    let wave_h = (r.y1 - 18. - wave_top).max(48.);
                    let wave_r = rect(x, wave_top, w, wave_h);
                    subtle_tile(ui, wave_r, 15., 0x0A84FF, dark);
                    if let Some(pro) = extras.pro {
                        paint_area_sparkline(ui.scene, wave_r, &pro.cpu_history.samples, 100., 0x0A84FF);
                        paint_area_sparkline(ui.scene, wave_r, &pro.gpu_history.samples, 100., 0xAF52DE);
                    } else if let Some(v) = cpu {
                        paint_apple_meter(ui, rect(wave_r.x0 + 12., wave_r.center().y - 3., wave_r.width() - 24., 6.), v / 100., 0x60A5FA, 0x0A84FF, dark);
                    }
                }
            }
        }
        Kind::AnalogClock => {
            use std::f64::consts::FRAC_PI_2;
            use vello::kurbo::{Cap, Circle, Line};
            let d = crate::desktop_cards::date();
            let rad = if large {
                ((w.min(r.height() - 28.)) * 0.45).clamp(52., 128.)
            } else if small {
                ((r.height() - 20.) * 0.48).clamp(50., 76.)
            } else {
                ((r.height() - 24.) * 0.46).clamp(50., 66.)
            };
            let cx = if small || large { r.center().x } else { x + rad + 6. };
            let cy = r.center().y;

            // Deskset 精密机械微仪表盘表盘（哑光银白 / 深炭灰表盘 + 双层金属表圈）
            ui.scene.fill(
                Fill::NonZero,
                Affine::translate((0., 3.5)),
                draw::alpha(0x090D16, if dark { 0.34 } else { 0.12 }),
                None,
                &Circle::new((cx, cy), rad),
            );
            let dial_grad = Gradient::new_linear((cx, cy - rad), (cx, cy + rad)).with_stops(if dark {
                [(0.0, draw::color(0x20242E)), (1.0, draw::color(0x14171F))]
            } else {
                [(0.0, draw::color(0xFFFFFF)), (1.0, draw::color(0xF1F4F9))]
            });
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &dial_grad, None, &Circle::new((cx, cy), rad));
            ui.scene.stroke(
                &Stroke::new(1.4),
                Affine::IDENTITY,
                draw::alpha(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.18 } else { 0.08 }),
                None,
                &Circle::new((cx, cy), rad),
            );
            ui.scene.stroke(
                &Stroke::new(0.85),
                Affine::IDENTITY,
                draw::alpha(0xFFFFFF, if dark { 0.10 } else { 0.92 }),
                None,
                &Circle::new((cx, cy), rad - 1.4),
            );

            // 60 刻度（12 点采用标志性双竖粗刻度）
            for i in 0..60 {
                let ang = (i as f64) * std::f64::consts::TAU / 60.0 - FRAC_PI_2;
                let major = i % 5 == 0;
                let r_outer = rad - 4.0;
                let r_inner = rad - if major { 10.5 } else { 5.8 };
                if i == 0 {
                    for off in [-2.0, 2.0] {
                        let tick = Line::new((cx + off, cy - r_outer), (cx + off, cy - r_inner - 1.5));
                        ui.scene.stroke(
                            &Stroke::new(2.0).with_caps(Cap::Round),
                            Affine::IDENTITY,
                            draw::alpha(if dark { 0xF5F7FA } else { 0x1D1D1F }, 0.90),
                            None,
                            &tick,
                        );
                    }
                } else {
                    let tick = Line::new(
                        (cx + ang.cos() * r_inner, cy + ang.sin() * r_inner),
                        (cx + ang.cos() * r_outer, cy + ang.sin() * r_outer),
                    );
                    ui.scene.stroke(
                        &Stroke::new(if major { 2.0 } else { 0.85 }).with_caps(Cap::Round),
                        Affine::IDENTITY,
                        draw::alpha(if dark { 0xF5F7FA } else { 0x1D1D1F }, if major { 0.86 } else { 0.24 }),
                        None,
                        &tick,
                    );
                }
            }

            // 12 点下方 Deskset 胶囊日期窗 + 下沉式双微子盘
            if rad >= 48. {
                let win_w = (rad * 0.62).clamp(36., 56.);
                let win_h = (rad * 0.22).clamp(13., 18.);
                let win_r = rect(cx - win_w * 0.5, cy - rad * 0.44 - win_h * 0.5, win_w, win_h);
                fill_alpha_rounded(
                    ui,
                    win_r,
                    4.5,
                    draw::color(if dark { 0x2C3344 } else { 0x1E293B }),
                );
                let wk_short = match d.weekday() {
                    0 => "一",
                    1 => "二",
                    2 => "三",
                    3 => "四",
                    4 => "五",
                    5 => "六",
                    _ => "日",
                };
                ui.center(&format!("周{wk_short} {}", d.day), win_r, (win_h * 0.66).clamp(8., 11.) as f32, 0xFFFFFF, true);

                // 左右两个下沉式微子盘（左：温度绿弧，右：活跃度绿弧）
                let sub_r = (rad * 0.23).clamp(11., 24.);
                let sub_y = cy + rad * 0.16;
                let sub_fill = draw::color(if dark { 0x12151C } else { 0x1F2430 });
                let temp_val = m.native_state["widgetData"].as_object()
                    .and_then(|o| o.values().find_map(|v| v["temperature"].as_f64()))
                    .unwrap_or(22.0);
                let sub_specs = [
                    (cx - rad * 0.36, 0x30D158_u32, 0.72, format!("{temp_val:.0}°")),
                    (cx + rad * 0.36, 0x30D158_u32, 0.76, "76%".into()),
                ];
                for (scx, col, frac, txt) in sub_specs {
                    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, sub_fill, None, &Circle::new((scx, sub_y), sub_r));
                    ring_track(ui, (scx, sub_y), sub_r - 2.2, 2.2, col, true);
                    draw::ring(ui.scene, (scx, sub_y), sub_r - 2.2, frac, col, 2.2);
                    ui.center(&txt, rect(scx - sub_r, sub_y - 6., sub_r * 2., 12.), (sub_r * 0.56).clamp(7.5, 11.) as f32, 0xF8FAFC, true);
                }
            }

            let sec = d.second as f64;
            let min = d.minute as f64 + sec / 60.0;
            let hr = (d.hour % 12) as f64 + min / 60.0;
            let hand_hr = hr * std::f64::consts::TAU / 12.0 - FRAC_PI_2;
            let hand_min = min * std::f64::consts::TAU / 60.0 - FRAC_PI_2;
            let hand_sec = sec * std::f64::consts::TAU / 60.0 - FRAC_PI_2;

            let hand_col = if dark { 0xF5F7FA } else { 0x1D1D1F };
            let round_stroke = |w: f64| Stroke::new(w).with_caps(Cap::Round);
            ui.scene.stroke(
                &round_stroke(3.6),
                Affine::translate((0., 1.2)),
                draw::alpha(0x000000, 0.14),
                None,
                &Line::new((cx, cy), (cx + hand_hr.cos() * rad * 0.50, cy + hand_hr.sin() * rad * 0.50)),
            );
            ui.scene.stroke(
                &round_stroke(3.6),
                Affine::IDENTITY,
                draw::color(hand_col),
                None,
                &Line::new((cx, cy), (cx + hand_hr.cos() * rad * 0.50, cy + hand_hr.sin() * rad * 0.50)),
            );
            ui.scene.stroke(
                &round_stroke(2.4),
                Affine::translate((0., 1.2)),
                draw::alpha(0x000000, 0.12),
                None,
                &Line::new((cx, cy), (cx + hand_min.cos() * rad * 0.76, cy + hand_min.sin() * rad * 0.76)),
            );
            ui.scene.stroke(
                &round_stroke(2.4),
                Affine::IDENTITY,
                draw::color(hand_col),
                None,
                &Line::new((cx, cy), (cx + hand_min.cos() * rad * 0.76, cy + hand_min.sin() * rad * 0.76)),
            );
            ui.scene.stroke(
                &round_stroke(1.4),
                Affine::IDENTITY,
                draw::color(0xFF9F0A),
                None,
                &Line::new(
                    (cx - hand_sec.cos() * rad * 0.20, cy - hand_sec.sin() * rad * 0.20),
                    (cx + hand_sec.cos() * rad * 0.86, cy + hand_sec.sin() * rad * 0.86),
                ),
            );
            draw::circle(ui.scene, cx, cy, 3.8, 0xFF9F0A);
            draw::circle(ui.scene, cx, cy, 1.4, if dark { 0x1E222D } else { 0xFFFFFF });

            if !small && !large {
                let rx = cx + rad + 22.;
                let rw = (r.x1 - 16. - rx).max(60.);
                ui.label(d.weekday_label(), rx, y + 18., 14., 0xFF9F0A, true, rw);
                ui.label(&format!("{}月{}日", d.month, d.day), rx, y + 44., 26., fg, true, rw);
                ui.label(&format!("{:02}:{:02}:{:02}", d.hour, d.minute, d.second), rx, y + 86., 14., muted, true, rw);
            }
        }
        Kind::ActivityRings => {
            // Smart Stack 透明烟熏玻璃霓虹三环（保留外层透明亚克力通透，叠加微点阵与霓虹三环发光）
            let dark_shape = allegro_theme::squircle_path(r, 24.0);
            let obsidian_veil = Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([
                (0.0, allegro_theme::rgba(0x141824, if dark { 0.44 } else { 0.56 })),
                (1.0, allegro_theme::rgba(0x0B0E17, if dark { 0.56 } else { 0.66 })),
            ]);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &obsidian_veil, None, &dark_shape);
            paint_micro_dot_grid(ui.scene, r, 24.0, true);
            let inner_glow = Gradient::new_radial(
                (r.center().x, r.center().y),
                r.width().max(r.height()) as f32 * 0.65,
            )
            .with_stops([
                (0.0, allegro_theme::rgba(0xFA114F, 0.14)),
                (0.55, allegro_theme::rgba(0x00F0FF, 0.06)),
                (1.0, allegro_theme::rgba(0xFFFFFF, 0.0)),
            ]);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &inner_glow, None, &dark_shape);

            let today_focus = m.native_state["widgetData"]["focusStats"]["todayMs"].as_f64().unwrap_or(0.) / 60000.;
            let p_focus = if m.timers.items.is_empty() && today_focus == 0. {
                0.76
            } else {
                (today_focus / 60.0).clamp(0.08, 1.0)
            };
            let tot_todo = m.todos.items.len();
            let done_todo = m.todos.items.iter().filter(|t| t.done(&dotisle_domain::todos::Now::local())).count();
            let p_todo = if tot_todo == 0 { 0.64 } else { (done_todo as f64 / tot_todo as f64).clamp(0.08, 1.0) };
            let screen_ms = m.native_state["screenTime"]["todayMs"].as_f64().unwrap_or(0.);
            let p_active = if screen_ms == 0. { 0.86 } else { (screen_ms / (4. * 3_600_000.)).clamp(0.08, 1.0) };

            let outer_r = if large { 74. } else if small { 61. } else { 46. };
            let thick = if large { 13. } else if small { 11.5 } else { 9. };
            let gap = thick + 3.2;
            let cx = if small || large { r.center().x } else { x + outer_r + 12. };
            let cy = if large { y + 106. } else if small { r.center().y } else { y + 28. + outer_r };

            let rings = [
                (outer_r, p_focus, 0xFA114F_u32, "专注", format!("{today_focus:.0}/60m")),
                (outer_r - gap, p_todo, 0xA6FF00_u32, "待办", if tot_todo == 0 { "3/5".into() } else { format!("{done_todo}/{tot_todo}") }),
                (outer_r - gap * 2., p_active, 0x00F0FF_u32, "活跃", if screen_ms == 0. { "3.4h".into() } else { usage_duration(screen_ms) }),
            ];
            for (rad_i, prog, col, _, _) in &rings {
                ring_track(ui, (cx, cy), *rad_i, thick, *col, true);
                draw::ring(ui.scene, (cx, cy), *rad_i, *prog, *col, thick);
            }

            if !small {
                draw::circle(ui.scene, x + 3.5, y + 8.5, 2.5, 0xFA114F);
                ui.label(title, x + 11.5, y, 11.5, 0xA8B0C2, true, (w - 16.).max(40.));
                let lx = if large { x } else { cx + outer_r + 22. };
                let lw = if large { w } else { (r.x1 - 16. - lx).max(64.) };
                let base_y = if large { cy + outer_r + 24. } else { y + 32. };
                for (i, (_, prog, col, name, detail)) in rings.iter().enumerate() {
                    let ry = base_y + i as f64 * 32.;
                    draw::circle(ui.scene, lx + 5., ry + 9., 4.0, *col);
                    ui.label(name, lx + 15., ry + 1., 11., 0x9CA3AF, true, 46.);
                    ui.label(detail, lx + 54., ry + 1., 11.5, 0xF5F7FA, true, (lw - 84.).max(32.));
                    ui.label(&format!("{:.0}%", prog * 100.), lx + lw - 34., ry + 1., 11., *col, true, 34.);
                }
            }
        }
        Kind::QuickDock => {
            let tiles = [
                ("快速专注", Icon::Focus, 0xFF7B61_u32, Action::QuickFocus),
                (if m.playing() { "暂停音乐" } else { "播放音乐" }, if m.playing() { Icon::Pause } else { Icon::Play }, 0x0A84FF_u32, Action::Media("playPause")),
                ("下一曲", Icon::Next, 0x5E5CE6_u32, Action::Media("next")),
                ("待办清单", Icon::Check, 0x30D158_u32, Action::Navigate(Page::Todos)),
            ];
            let cols = 2;
            let gap = 8.;
            let top = y + 26.;
            let avail_h = (r.y1 - 14. - top).max(60.);
            let tw = (w - gap) / cols as f64;
            let th = ((avail_h - gap) / 2.).max(32.);
            for (i, (label, icon, tint, act)) in tiles.into_iter().enumerate() {
                let col = i % cols;
                let row = i / cols;
                let tx = x + col as f64 * (tw + gap);
                let ty = top + row as f64 * (th + gap);
                let tr = rect(tx, ty, tw, th);
                let hovered = tr.contains(ui.pointer);
                if hovered {
                    fill_alpha_rounded(ui, tr, 14., draw::alpha(tint, if dark { 0.26 } else { 0.16 }));
                } else {
                    subtle_tile(ui, tr, 14., tint, dark);
                }
                if small {
                    let ir = (th * 0.34).clamp(11., 15.);
                    draw::circle(ui.scene, tr.center().x, tr.center().y, ir, tint);
                    ui.icon(icon, tr.center().x - 8., tr.center().y - 8., 16., 0xFFFFFF);
                } else {
                    draw::circle(ui.scene, tx + 22., ty + th / 2., 14., tint);
                    ui.icon(icon, tx + 14., ty + th / 2. - 8., 16., 0xFFFFFF);
                    ui.label(label, tx + 42., ty + th / 2. - 8., 12., fg, true, (tw - 48.).max(32.));
                }
                ui.hit(tr, label, act, true);
            }
        }
        Kind::Quote => {
            const QUOTES: [(&str, &str); 6] = [
                ("少即是多，让桌面留出呼吸的秩序。", "Dieter Rams · 设计格言"),
                ("把注意留给创造，琐碎交由顺手的工具。", "Allegro · 桌搭日记"),
                ("好的界面隐于背景之中，只在需要时亮起。", "Apple Design"),
                ("慢慢来，稳稳做完今天最重要的一件事。", "Today Focus"),
                ("光线穿过透明的边缘，时间也有了形状。", "Lumen & Glass"),
                ("整齐的桌面，是清醒思绪的起点。", "Studio Notes"),
            ];
            let (body, author) = if config.text.trim().is_empty() {
                QUOTES[extras.quote_index % QUOTES.len()]
            } else {
                (config.text.trim(), "个人日签")
            };
            let box_r = rect(x, y + 26., w, (r.height() - 54.).max(64.));
            // 左侧 3px Apple 优雅渐变竖引线
            let bar_grad = Gradient::new_linear((x + 2., y + 32.), (x + 2., r.y1 - 36.)).with_stops([
                (0.0, draw::alpha(accent, 0.85)),
                (1.0, draw::alpha(0x0A84FF, 0.35)),
            ]);
            ui.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &bar_grad,
                None,
                &RoundedRect::from_rect(rect(x + 2., y + 32., 3., (r.height() - 86.).max(32.)), 1.5),
            );
            ui.label("“", x + 12., y + 24., 26., accent, true, 24.);
            text_lines(
                ui,
                body,
                x + 30.,
                y + 34.,
                (w - 34.).max(60.),
                if large { 6 } else { 3 },
                if small { 13.5 } else { 15.5 },
                fg,
            );
            ui.label(author, x + 12., r.y1 - 28., 10.5, muted, true, w - 16.);
            ui.hit(box_r, "切换日签语录", Action::QuoteNext(id.into()), true);
        }
        Kind::AppFolder => {
            let entries = app_folder_entries(&config);
            let is_nine = config.caption.trim() == "9";
            if is_nine {
                if small {
                    // iOS 3×3 九宫格文件夹（无顶栏铺满）
                    let pad = 15.;
                    let gap = 9.;
                    let icon_s = ((r.width() - pad * 2. - gap * 2.) / 3.).clamp(30., 46.);
                    let ox = r.x0 + (r.width() - (icon_s * 3. + gap * 2.)) / 2.;
                    let oy = r.y0 + (r.height() - (icon_s * 3. + gap * 2.)) / 2.;
                    for (idx, ent) in entries.iter().take(9).enumerate() {
                        let col = idx % 3;
                        let row = idx / 3;
                        let ir = rect(ox + col as f64 * (icon_s + gap), oy + row as f64 * (icon_s + gap), icon_s, icon_s);
                        let hovered = ir.contains(ui.pointer);
                        paint_app_squircle(ui, ir, 11., ent, hovered);
                        ui.hit(ir, &format!("打开 {}", ent.name), Action::AppFolderLaunch(id.into(), idx), true);
                    }
                } else if large {
                    // 3×3 大号带清晰应用名与右上切换胶囊
                    ui.label(if config.title.is_empty() { "应用文件夹" } else { &config.title }, x, y + 2., 14., fg, true, w - 92.);
                    let sw_r = rect(r.x1 - 86., y, 70., 22.);
                    subtle_tile(ui, sw_r, 11., accent, dark);
                    ui.center("9格 ⇄ 4格", sw_r, 10., accent, true);
                    ui.hit(sw_r, "切换为 4 宫格", Action::AppFolderToggleGrid(id.into()), true);

                    let top = y + 32.;
                    let gap_x = 16.;
                    let gap_y = 8.;
                    let cw = (w - gap_x * 2.) / 3.;
                    let rh = ((r.y1 - 14. - top - gap_y * 2.) / 3.).max(66.);
                    let icon_s = (rh - 22.).min(66.).max(44.);
                    for (idx, ent) in entries.iter().take(9).enumerate() {
                        let col = idx % 3;
                        let row = idx / 3;
                        let cell_x = x + col as f64 * (cw + gap_x);
                        let cell_y = top + row as f64 * (rh + gap_y);
                        let ix = cell_x + (cw - icon_s) / 2.;
                        let ir = rect(ix, cell_y, icon_s, icon_s);
                        let hovered = ir.contains(ui.pointer);
                        paint_app_squircle(ui, ir, 15., ent, hovered);
                        ui.center(&ent.name, rect(cell_x, cell_y + icon_s + 4., cw, 16.), 10.5, fg, true);
                        ui.hit(rect(cell_x, cell_y, cw, rh), &format!("打开 {}", ent.name), Action::AppFolderLaunch(id.into(), idx), true);
                    }
                } else {
                    // Medium：5列×2行（前 9 个应用 + 第 10 格右下“4格切换按钮”）
                    let pad = 16.;
                    let gap_x = 12.;
                    let gap_y = 12.;
                    let icon_s = ((r.height() - pad * 2. - gap_y) / 2.).clamp(40., 62.);
                    let ox = r.x0 + (r.width() - (icon_s * 5. + gap_x * 4.)) / 2.;
                    let oy = r.y0 + (r.height() - (icon_s * 2. + gap_y)) / 2.;
                    for idx in 0..10 {
                        let col = idx % 5;
                        let row = idx / 5;
                        let ir = rect(ox + col as f64 * (icon_s + gap_x), oy + row as f64 * (icon_s + gap_y), icon_s, icon_s);
                        if idx < 9 {
                            if let Some(ent) = entries.get(idx) {
                                let hovered = ir.contains(ui.pointer);
                                paint_app_squircle(ui, ir, 13., ent, hovered);
                                ui.hit(ir, &format!("打开 {}", ent.name), Action::AppFolderLaunch(id.into(), idx), true);
                            }
                        } else {
                            subtle_tile(ui, ir, 13., accent, dark);
                            ui.center("2×2", rect(ir.x0, ir.y0 + 10., ir.width(), 18.), 12., accent, true);
                            ui.center("切回4格", rect(ir.x0, ir.y0 + 30., ir.width(), 14.), 8.5, muted, false);
                            ui.hit(ir, "切换为 4 宫格", Action::AppFolderToggleGrid(id.into()), true);
                        }
                    }
                }
            } else if small {
                // iOS 默认 2×2 四宫格大图标文件夹（无顶栏铺满，单格 66×66 质感超椭圆）
                let pad = 16.;
                let gap = 12.;
                let icon_s = ((r.width() - pad * 2. - gap) / 2.).clamp(48., 68.);
                let ox = r.x0 + (r.width() - (icon_s * 2. + gap)) / 2.;
                let oy = r.y0 + (r.height() - (icon_s * 2. + gap)) / 2.;
                for (idx, ent) in entries.iter().take(4).enumerate() {
                    let col = idx % 2;
                    let row = idx / 2;
                    let ir = rect(ox + col as f64 * (icon_s + gap), oy + row as f64 * (icon_s + gap), icon_s, icon_s);
                    let hovered = ir.contains(ui.pointer);
                    paint_app_squircle(ui, ir, 16., ent, hovered);
                    ui.hit(ir, &format!("打开 {}", ent.name), Action::AppFolderLaunch(id.into(), idx), true);
                }
            } else if large {
                // Large 2×2 豪华四格应用卡
                ui.label(if config.title.is_empty() { "应用文件夹" } else { &config.title }, x, y + 2., 14., fg, true, w - 92.);
                let sw_r = rect(r.x1 - 86., y, 70., 22.);
                subtle_tile(ui, sw_r, 11., accent, dark);
                ui.center("4格 ⇄ 9格", sw_r, 10., accent, true);
                ui.hit(sw_r, "切换为 9 宫格", Action::AppFolderToggleGrid(id.into()), true);

                let top = y + 34.;
                let gap = 12.;
                let cw = (w - gap) / 2.;
                let ch = ((r.y1 - 16. - top - gap) / 2.).max(92.);
                for (idx, ent) in entries.iter().take(4).enumerate() {
                    let col = idx % 2;
                    let row = idx / 2;
                    let card_r = rect(x + col as f64 * (cw + gap), top + row as f64 * (ch + gap), cw, ch);
                    subtle_tile(ui, card_r, 16., ent.grad_bot, dark);
                    let icon_s = 68.;
                    let ir = rect(card_r.center().x - icon_s / 2., card_r.y0 + 16., icon_s, icon_s);
                    let hovered = card_r.contains(ui.pointer);
                    paint_app_squircle(ui, ir, 16., ent, hovered);
                    ui.center(&ent.name, rect(card_r.x0 + 8., ir.y1 + 10., cw - 16., 18.), 12.5, fg, true);
                    ui.hit(card_r, &format!("打开 {}", ent.name), Action::AppFolderLaunch(id.into(), idx), true);
                }
            } else {
                // Medium 1×4 横向排布 4 枚精致图标 + 下方应用名 + 底部 9 格切换胶囊
                let top = y + 6.;
                let gap = 14.;
                let cw = (w - gap * 3.) / 4.;
                let icon_s = cw.min(64.).max(44.);
                for (idx, ent) in entries.iter().take(4).enumerate() {
                    let cx = x + idx as f64 * (cw + gap);
                    let ix = cx + (cw - icon_s) / 2.;
                    let ir = rect(ix, top + 4., icon_s, icon_s);
                    let hovered = ir.contains(ui.pointer);
                    paint_app_squircle(ui, ir, 15., ent, hovered);
                    ui.center(&ent.name, rect(cx, ir.y1 + 8., cw, 16.), 11., fg, true);
                    ui.hit(rect(cx, top, cw, icon_s + 28.), &format!("打开 {}", ent.name), Action::AppFolderLaunch(id.into(), idx), true);
                }
                let foot_r = rect(r.x1 - 92., r.y1 - 32., 76., 18.);
                subtle_tile(ui, foot_r, 9., accent, dark);
                ui.center("切换 9 宫格", foot_r, 9.5, accent, true);
                ui.hit(foot_r, "切换为 9 宫格", Action::AppFolderToggleGrid(id.into()), true);
            }
        }
    }

    if extras.use_allegro_shell {
        ui.s = saved_scheme;
        ui.glass = saved_glass;
    }

    if placement.is_some()
        && ((!extras.use_allegro_shell && kind == Kind::Note)
            || matches!(
                kind,
                Kind::Anniversary
                    | Kind::Schedule
                    | Kind::Weather
                    | Kind::Storage
                    | Kind::WorldClock
                    | Kind::PhotoFrame
            ))
        && (kind != Kind::PhotoFrame || extras.photo_image.is_none())
    {
        ui.hit(
            rect(r.x0, r.y0 + 28., r.width(), r.height() - 28.),
            "编辑组件内容",
            Action::WidgetConfigure(id.into()),
            true,
        );
    }
}

fn format_kbps(kbps: Option<f64>) -> String {
    match kbps {
        None => "—".into(),
        Some(v) if v >= 1024.0 => format!("{:.1} MB/s", v / 1024.0),
        Some(v) => format!("{:.0} KB/s", v),
    }
}

/// 绘制带垂直半透明渐变面积填充的硬件波形线。
fn paint_area_sparkline(
    scene: &mut vello::Scene,
    r: Rect,
    samples: &std::collections::VecDeque<f64>,
    peak: f64,
    color: u32,
) {
    if samples.len() < 2 {
        return;
    }
    let pad = 8.;
    let inner_w = (r.width() - pad * 2.).max(8.);
    let inner_h = (r.height() - 12.).max(8.);
    let base_y = r.y1 - 6.;
    let max_val = peak.max(1.);
    let n = (samples.len() - 1) as f64;
    let mut line_path = BezPath::new();
    let mut area_path = BezPath::new();
    for (i, val) in samples.iter().enumerate() {
        let px = r.x0 + pad + (i as f64 / n) * inner_w;
        let py = base_y - (val / max_val).clamp(0., 1.) * inner_h;
        if i == 0 {
            line_path.move_to((px, py));
            area_path.move_to((px, base_y));
            area_path.line_to((px, py));
        } else {
            line_path.line_to((px, py));
            area_path.line_to((px, py));
        }
    }
    area_path.line_to((r.x0 + pad + inner_w, base_y));
    area_path.close_path();

    let area_grad = Gradient::new_linear((r.x0, r.y0 + 4.), (r.x0, base_y)).with_stops([
        (0.0, draw::alpha(color, 0.28)),
        (1.0, draw::alpha(color, 0.02)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &area_grad, None, &area_path);
    scene.stroke(&Stroke::new(1.8), Affine::IDENTITY, draw::alpha(color, 0.92), None, &line_path);
}

/// Deskset 天气温度平滑折线与柔和渐变面积图。
fn paint_weather_curve(scene: &mut vello::Scene, r: Rect, color: u32, dark: bool) {
    let pts = [0.22, 0.34, 0.58, 0.72, 0.76, 0.62, 0.48];
    let pad = 4.;
    let iw = (r.width() - pad * 2.).max(8.);
    let ih = (r.height() - 10.).max(8.);
    let base_y = r.y1 - 3.;
    let n = (pts.len() - 1) as f64;
    let mut line = BezPath::new();
    let mut area = BezPath::new();
    for (i, &v) in pts.iter().enumerate() {
        let px = r.x0 + pad + (i as f64 / n) * iw;
        let py = base_y - v * ih;
        if i == 0 {
            line.move_to((px, py));
            area.move_to((px, base_y));
            area.line_to((px, py));
        } else {
            line.line_to((px, py));
            area.line_to((px, py));
        }
    }
    area.line_to((r.x0 + pad + iw, base_y));
    area.close_path();
    let grad = Gradient::new_linear((r.x0, r.y0), (r.x0, base_y)).with_stops([
        (0.0, draw::alpha(color, if dark { 0.32 } else { 0.26 })),
        (1.0, draw::alpha(color, 0.0)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &grad, None, &area);
    scene.stroke(&Stroke::new(1.8), Affine::IDENTITY, draw::alpha(color, 0.92), None, &line);
}

/// 绘制 SystemPro 圆环中心精密矢量微图标（0=CPU芯片, 1=内存, 2=磁盘, 3=GPU核心）。
fn paint_pro_glyph(scene: &mut vello::Scene, c: (f64, f64), glyph: usize, tint: u32) {
    let col = draw::alpha(tint, 0.92);
    let stroke = Stroke::new(1.25).with_caps(vello::kurbo::Cap::Round);
    match glyph {
        0 => {
            scene.stroke(&stroke, Affine::IDENTITY, col, None, &RoundedRect::from_rect(rect(c.0 - 5., c.1 - 5., 10., 10.), 2.));
            scene.stroke(&stroke, Affine::IDENTITY, col, None, &RoundedRect::from_rect(rect(c.0 - 2.2, c.1 - 2.2, 4.4, 4.4), 0.8));
        }
        1 => {
            scene.stroke(&stroke, Affine::IDENTITY, col, None, &RoundedRect::from_rect(rect(c.0 - 5.5, c.1 - 3.5, 11., 7.), 1.5));
        }
        2 => {
            scene.stroke(&stroke, Affine::IDENTITY, col, None, &RoundedRect::from_rect(rect(c.0 - 5., c.1 - 4.5, 10., 9.), 2.));
            scene.stroke(&stroke, Affine::IDENTITY, col, None, &vello::kurbo::Line::new((c.0 - 3., c.1 + 1.5), (c.0 + 3., c.1 + 1.5)));
        }
        _ => {
            let mut hex = BezPath::new();
            for k in 0..6 {
                let a = k as f64 * std::f64::consts::TAU / 6.0;
                let pt = (c.0 + a.cos() * 5.2, c.1 + a.sin() * 5.2);
                if k == 0 { hex.move_to(pt); } else { hex.line_to(pt); }
            }
            hex.close_path();
            scene.stroke(&stroke, Affine::IDENTITY, col, None, &hex);
            draw::circle(scene, c.0, c.1, 1.4, tint);
        }
    }
}

/// 手绘右上角发光暖阳与纯洁蓬松立体白云矢量图标。
fn paint_weather_sun_cloud(scene: &mut vello::Scene, center: (f64, f64), dark: bool) {
    use vello::kurbo::Circle;
    let sun_c = (center.0 + 9., center.1 - 7.);
    let halo = Gradient::new_radial(sun_c, 26.0).with_stops([
        (0.0, draw::alpha(0xFFE066, if dark { 0.52 } else { 0.44 })),
        (1.0, draw::alpha(0xFF9F0A, 0.0)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &halo, None, &Circle::new(sun_c, 26.));
    let sun_grad = Gradient::new_linear((sun_c.0, sun_c.1 - 12.), (sun_c.0, sun_c.1 + 12.)).with_stops([
        (0.0, draw::color(0xFFE048)),
        (1.0, draw::color(0xFF9500)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &sun_grad, None, &Circle::new(sun_c, 12.));

    // 通透蓬松立体白云（去掉生硬灰线，用轻柔和阴影塑造体积感）
    let cloud_base = (center.0 - 4., center.1 + 6.);
    let pill = rect(cloud_base.0 - 17., cloud_base.1 - 3., 34., 15.);
    let c_col = draw::alpha(0xFFFFFF, 0.96);
    scene.fill(Fill::NonZero, Affine::IDENTITY, c_col, None, &Circle::new((cloud_base.0 - 6., cloud_base.1 - 1.), 9.2));
    scene.fill(Fill::NonZero, Affine::IDENTITY, c_col, None, &Circle::new((cloud_base.0 + 3., cloud_base.1 - 4.), 11.2));
    scene.fill(Fill::NonZero, Affine::IDENTITY, c_col, None, &Circle::new((cloud_base.0 + 10., cloud_base.1 + 0.5), 7.8));
    scene.fill(Fill::NonZero, Affine::IDENTITY, c_col, None, &RoundedRect::from_rect(pill, 7.5));
}

/// 绘制 macOS / iOS 风格晶莹立体超椭圆渐变应用图标与居中矢量图案。
fn paint_app_squircle(
    ui: &mut Ui,
    r: Rect,
    radius: f64,
    entry: &AppFolderEntry,
    hovered: bool,
) {
    use allegro_theme::{rgba, squircle_path};
    use vello::kurbo::{Cap, Circle, Line};

    let shape = squircle_path(r, radius);
    // 1. 同色系彩色漫射柔和投影
    ui.scene.fill(
        Fill::NonZero,
        Affine::translate((0., 2.8)),
        rgba(entry.grad_bot, 0.28),
        None,
        &shape,
    );
    ui.scene.fill(
        Fill::NonZero,
        Affine::translate((0., 1.0)),
        rgba(0x090D16, 0.14),
        None,
        &shape,
    );
    // 2. 饱满 Apple 垂直色彩渐变底
    let grad = Gradient::new_linear((r.x0, r.y0), (r.x0, r.y1)).with_stops([
        (0.0, draw::color(entry.grad_top)),
        (1.0, draw::color(entry.grad_bot)),
    ]);
    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &grad, None, &shape);

    // 3. 顶部弧面晶莹光泽 + 悬停聚焦环
    let sheen = Gradient::new_linear((r.x0, r.y0), (r.x0, r.y0 + r.height() * 0.52)).with_stops([
        (0.0, rgba(0xFFFFFF, 0.30)),
        (1.0, rgba(0xFFFFFF, 0.0)),
    ]);
    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, &sheen, None, &shape);

    ui.scene.stroke(
        &Stroke::new(if hovered { 2.0 } else { 0.9 }),
        Affine::IDENTITY,
        rgba(0xFFFFFF, if hovered { 0.94 } else { 0.46 }),
        None,
        &shape,
    );

    // 4. 居中精工纯白矢量徽标
    let cx = r.center().x;
    let cy = r.center().y;
    let s = (r.width().min(r.height()) * 0.46).clamp(14., 32.);
    let white = draw::alpha(0xFFFFFF, 0.96);
    let soft_w = draw::alpha(0xFFFFFF, 0.28);
    let stroke = Stroke::new((s * 0.11).clamp(1.6, 3.0)).with_caps(Cap::Round);

    match entry.glyph {
        AppGlyph::Folder => {
            let tab = rect(cx - s * 0.50, cy - s * 0.36, s * 0.42, s * 0.18);
            let body = rect(cx - s * 0.50, cy - s * 0.24, s, s * 0.64);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, white, None, &RoundedRect::from_rect(tab, s * 0.08));
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, white, None, &RoundedRect::from_rect(body, s * 0.14));
        }
        AppGlyph::Terminal => {
            let mut prompt = BezPath::new();
            prompt.move_to((cx - s * 0.36, cy - s * 0.24));
            prompt.line_to((cx - s * 0.06, cy));
            prompt.line_to((cx - s * 0.36, cy + s * 0.24));
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &prompt);
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &Line::new((cx + s * 0.05, cy + s * 0.24), (cx + s * 0.38, cy + s * 0.24)));
        }
        AppGlyph::Browser => {
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &Circle::new((cx, cy), s * 0.46));
            let mut needle = BezPath::new();
            needle.move_to((cx + s * 0.24, cy - s * 0.24));
            needle.line_to((cx + s * 0.06, cy + s * 0.06));
            needle.line_to((cx - s * 0.24, cy + s * 0.24));
            needle.line_to((cx - s * 0.06, cy - s * 0.06));
            needle.close_path();
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, white, None, &needle);
        }
        AppGlyph::Settings => {
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &Circle::new((cx, cy), s * 0.28));
            for k in 0..6 {
                let a = k as f64 * std::f64::consts::TAU / 6.0;
                ui.scene.stroke(
                    &stroke,
                    Affine::IDENTITY,
                    white,
                    None,
                    &Line::new(
                        (cx + a.cos() * s * 0.32, cy + a.sin() * s * 0.32),
                        (cx + a.cos() * s * 0.46, cy + a.sin() * s * 0.46),
                    ),
                );
            }
        }
        AppGlyph::Note => {
            let page = rect(cx - s * 0.38, cy - s * 0.44, s * 0.76, s * 0.88);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, white, None, &RoundedRect::from_rect(page, s * 0.12));
            for dy in [-0.14, 0.06, 0.24] {
                ui.scene.stroke(
                    &Stroke::new(1.5).with_caps(Cap::Round),
                    Affine::IDENTITY,
                    draw::alpha(entry.grad_bot, 0.72),
                    None,
                    &Line::new((cx - s * 0.20, cy + s * dy), (cx + s * 0.20, cy + s * dy)),
                );
            }
        }
        AppGlyph::Calc => {
            let disp = rect(cx - s * 0.34, cy - s * 0.38, s * 0.68, s * 0.22);
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, soft_w, None, &RoundedRect::from_rect(disp, s * 0.06));
            for row in 0..2 {
                for col in 0..2 {
                    let kc = (cx - s * 0.18 + col as f64 * s * 0.36, cy + s * 0.05 + row as f64 * s * 0.28);
                    let dot_c = if row == 1 && col == 1 { draw::color(0xFF9F0A) } else { white };
                    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, dot_c, None, &Circle::new(kc, s * 0.11));
                }
            }
        }
        AppGlyph::Capture => {
            let o = s * 0.36;
            let l = s * 0.20;
            for (sx, sy) in [(-1., -1.), (1., -1.), (-1., 1.), (1., 1.)] {
                let mut p = BezPath::new();
                p.move_to((cx + sx * o - sx * l, cy + sy * o));
                p.line_to((cx + sx * o, cy + sy * o));
                p.line_to((cx + sx * o, cy + sy * o - sy * l));
                ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &p);
            }
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, white, None, &Circle::new((cx, cy), s * 0.14));
        }
        AppGlyph::Monitor => {
            let mut wave = BezPath::new();
            wave.move_to((cx - s * 0.44, cy + s * 0.06));
            wave.line_to((cx - s * 0.18, cy + s * 0.06));
            wave.line_to((cx - s * 0.04, cy - s * 0.28));
            wave.line_to((cx + s * 0.12, cy + s * 0.26));
            wave.line_to((cx + s * 0.24, cy + s * 0.02));
            wave.line_to((cx + s * 0.44, cy + s * 0.02));
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &wave);
        }
        AppGlyph::Clock => {
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &Circle::new((cx, cy), s * 0.44));
            ui.scene.stroke(&stroke, Affine::IDENTITY, white, None, &Line::new((cx, cy), (cx, cy - s * 0.24)));
            ui.scene.stroke(&stroke, Affine::IDENTITY, draw::color(0xFF9F0A), None, &Line::new((cx, cy), (cx + s * 0.20, cy + s * 0.10)));
        }
        AppGlyph::Initial(ch) => {
            let txt = ch.to_string();
            let fs = (r.height() * 0.42).clamp(14., 28.) as f32;
            ui.center(&txt, r, fs, 0xFFFFFF, true);
        }
    }
}

fn todo_editor(
    ui: &mut Ui,
    r: Rect,
    x: f64,
    y: f64,
    w: f64,
    editor: &crate::views::TodoEdit,
    accent: u32,
    dark: bool,
    fg: u32,
    muted: u32,
    time: f64,
) {
    let panel_top = y + 30.;
    let panel_h = (r.y1 - panel_top - 10.).max(110.);
    subtle_tile(ui, rect(x, panel_top, w, panel_h), 16., accent, dark);
    ui.label(
        if editor.id.is_some() { "编辑待办" } else { "新建待办" },
        x + 12.,
        panel_top + 10.,
        13.,
        fg,
        true,
        w - 24.,
    );
    let fw = (w - 20.).max(60.);
    let title = rect(x + 10., panel_top + 50., fw, 44.);
    editor_field(
        ui,
        title,
        crate::views::TodoField::Title,
        &editor.title,
        editor.field,
        &editor.preedit,
        accent,
        dark,
        fg,
        muted,
        time,
    );
    if panel_h >= 200. {
        let half = ((fw - 8.) / 2.).max(40.);
        let date = rect(x + 10., panel_top + 100., half, 34.);
        let clock = rect(x + 18. + half, panel_top + 100., half, 34.);
        editor_field(
            ui,
            date,
            crate::views::TodoField::Date,
            &editor.date,
            editor.field,
            &editor.preedit,
            accent,
            dark,
            fg,
            muted,
            time,
        );
        editor_field(
            ui,
            clock,
            crate::views::TodoField::Time,
            &editor.time,
            editor.field,
            &editor.preedit,
            accent,
            dark,
            fg,
            muted,
            time,
        );
    }
    if let Some(error) = &editor.error {
        ui.label(error, x + 12., r.y1 - 66., 10., 0xD93025, false, w - 24.);
    }
    let bw = (fw - 8.) / 2.;
    ui.button(
        rect(x + 10., r.y1 - 44., bw, 34.),
        "取消",
        None,
        Action::TodoCancel,
        true,
        true,
    );
    ui.button(
        rect(x + 18. + bw, r.y1 - 44., bw, 34.),
        "保存",
        None,
        Action::TodoSave,
        false,
        true,
    );
}

fn editor_field(
    ui: &mut Ui,
    r: Rect,
    field: crate::views::TodoField,
    value: &str,
    active: crate::views::TodoField,
    preedit: &str,
    accent: u32,
    dark: bool,
    fg: u32,
    muted: u32,
    time: f64,
) {
    let on = field == active;
    let fill = if ui.glass {
        if on {
            draw::alpha(accent, if dark { 0.22 } else { 0.14 })
        } else if dark {
            draw::alpha(0xFFFFFF, 0.08)
        } else {
            draw::alpha(0xFFFFFF, 0.36)
        }
    } else {
        draw::color(if on {
            mix(ui.s.container, accent, 0.10)
        } else {
            mix(ui.s.container, 0x0F172A, if dark { 0.05 } else { 0.035 })
        })
    };
    fill_alpha_rounded(ui, r, 10., fill);
    let rim = if on {
        draw::alpha(accent, 1.)
    } else {
        draw::alpha(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.14 } else { 0.10 })
    };
    ui.scene.stroke(
        &Stroke::new(if on { 1.4 } else { 0.8 }),
        Affine::IDENTITY,
        rim,
        None,
        &RoundedRect::from_rect(r, 10.),
    );
    let ty = r.y0 + (r.height() - 16.) / 2.;
    let composed = if preedit.is_empty() {
        value.to_owned()
    } else {
        format!("{value}{preedit}")
    };
    if composed.is_empty() {
        ui.label(
            match field {
                crate::views::TodoField::Title => "输入待办标题",
                crate::views::TodoField::Date => "2026-11-25",
                crate::views::TodoField::Time => "18:30",
            },
            r.x0 + 10.,
            ty,
            12.,
            muted,
            false,
            r.width() - 20.,
        );
    } else {
        ui.label(&composed, r.x0 + 10., ty, 12., fg, false, r.width() - 20.);
    }
    if !preedit.is_empty() {
        let base = ui.measure_alpha(value, 12., false);
        let width = ui.measure_alpha(preedit, 12., false).max(1.);
        let available = (r.width() - 20. - base).max(0.);
        if available > 0.5 {
            draw::fill_rounded(
                ui.scene,
                rect(
                    r.x0 + 10. + base.min(r.width() - 20.),
                    r.y0 + r.height() - 8.,
                    width.min(available),
                    1.,
                ),
                0.5,
                accent,
            );
        }
    }
    if on && (time * 2.).rem_euclid(2.) < 1. {
        let tw = ui.measure_alpha(&composed, 12., false).min(r.width() - 22.);
        draw::fill_rounded(
            ui.scene,
            rect(r.x0 + 10. + tw, r.y0 + 9., 1.4, r.height() - 18.),
            0.7,
            accent,
        );
    }
    ui.hit(r, "编辑字段", Action::TodoField(field), true);
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
            let pill = rect(xx + 2., y, cw - 4., 68.);
            let grad = Gradient::new_linear((pill.x0, pill.y0), (pill.x0, pill.y1)).with_stops([
                (0.0, draw::alpha(mix(blue, 0xFFFFFF, 0.16), if ui.glass { 0.92 } else { 1.0 })),
                (1.0, draw::alpha(blue, if ui.glass { 0.92 } else { 1.0 })),
            ]);
            ui.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                &grad,
                None,
                &RoundedRect::from_rect(pill, 14.),
            );
        }
        let wd_col = if today {
            0xEBF5FF
        } else if i >= 5 {
            mix(fg, 0x8E8E93, if dark { 0.35 } else { 0.45 })
        } else {
            fg
        };
        ui.center(["一", "二", "三", "四", "五", "六", "日"][i], rect(xx, y + 7., cw, 18.), 10., wd_col, today);
        ui.center(&day.to_string(), rect(xx, y + 32., cw, 26.), 17., if today { 0xFFFFFF } else { fg }, true);
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
