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
        if let Some(p) = m.desktop_widgets.items.iter().find(|p| p.id == id) {
            let mx = ((w - CONTEXT_MENU_W) * 0.5).max(4.0);
            let my = ((h - CONTEXT_MENU_H) * 0.5).max(4.0);
            let _ = paint_apple_context_menu(&mut ui, p, (mx, my), m.prefs.dark);
        } else {
            paint_classic_menu(&mut ui, id, w, h, s.high);
        }
    }
    View {
        hits: ui.hits,
        scene,
    }
}

fn paint_classic_menu(ui: &mut Ui, id: &str, w: f64, h: f64, high: u32) {
    let bar = rect(8., h - 100., w - 16., 92.);
    draw::fill(ui.scene, bar, 16., high);
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

/// 渲染 Allegro 单个小组件时传入的画布环境上下文。
pub struct AllegroCardContext<'a> {
    pub placement: &'a dotisle_domain::desktop_widgets::Placement,
    pub width: f64,
    pub height: f64,
    pub pointer: (f64, f64),
    pub menu: bool,
    pub scroll: f64,
    pub todo_edit: Option<&'a TodoEdit>,
    pub note_edit: Option<&'a allegro::efficiency::NoteEdit>,
    pub time: f64,
    pub media_position: Option<f64>,
    pub wallpaper_crop: Option<&'a vello::peniko::ImageData>,
    /// 局部桌面壁纸平均亮度（0.0..=1.0），用于亚克力/真液态玻璃下自适应字体对比度。
    pub wallpaper_luma: Option<f32>,
    pub dropzone_items: &'a [allegro::efficiency::DropItem],
    pub photo_image: Option<&'a vello::peniko::ImageData>,
    pub photo_meta: (usize, usize),
    pub pro: Option<&'a allegro::telemetry::ProTelemetry>,
    pub stack_info: Option<(usize, usize)>,
    pub quote_index: usize,
}

fn scheme_from_tokens(t: allegro::theme::ThemeTokens) -> dotisle_domain::monet::Scheme {
    let mut s = dotisle_domain::monet::scheme(t.primary, false);
    s.surface = t.surface;
    s.container = t.container;
    s.high = t.high;
    s.on_surface = t.on_surface;
    s.on_surface_variant = t.on_surface_variant;
    s.primary = t.primary;
    s.on_primary = t.on_primary;
    s
}

/// 使用 Allegro 耀光/亚克力/真液态玻璃材质、智能叠放与效率扩展渲染单个小组件。
pub fn render_allegro_card(
    m: &Model,
    text: &mut Text,
    ctx: &AllegroCardContext<'_>,
) -> View {
    let p = ctx.placement;
    let (w, h) = (ctx.width.max(1.), ctx.height.max(1.));
    let mut scene = vello::Scene::new();

    let mat = allegro::theme::WidgetMaterial::from_str_opt(&p.material);
    let adaptive_dark = match ctx.wallpaper_luma {
        Some(luma) => luma < 0.56,
        None => m.prefs.dark,
    };

    let accent_override = allegro::theme::parse_hex_rgb(&p.config.accent);
    let tokens = allegro::theme::resolve_theme(Style::Original, adaptive_dark, accent_override);
    let s = scheme_from_tokens(tokens);

    if p.hidden {
        let mut ui = Ui::new(&mut scene, text, s, ctx.pointer, rect(0., 0., w, h));
        let bar_color = 0xC0000000 | (s.primary & 0x00FFFFFF);
        draw::fill_rounded(ui.scene, rect(0., 0., w, h), (h / 2.).min(8.), bar_color);
        ui.center(p.kind.label(), rect(0., 0., w, h), 8.5, s.on_primary, true);
        ui.hit(rect(0., 0., w, h), "展开组件", Action::WidgetRestore(p.id.clone()), true);
        return View { hits: ui.hits, scene };
    }

    let opacity = if p.opacity > 0.05 { p.opacity } else { 1.0 };
    let appearance = allegro::theme::WidgetAppearance {
        material: mat,
        style: Style::Original,
        opacity,
        radius: tokens.radius,
        accent: accent_override,
        blur_wallpaper: true,
    };

    let mut ui = Ui::new(&mut scene, text, s, ctx.pointer, rect(0., 0., w, h));
    let state = ViewState {
        time: ctx.time,
        media_position: ctx.media_position,
        scroll: ctx.scroll,
        editor: ctx.todo_edit.cloned(),
    };
    let extras = apple_desktop::CardExtras {
        use_allegro_shell: true,
        appearance,
        wallpaper_crop: ctx.wallpaper_crop,
        note_edit: ctx.note_edit,
        dropzone_items: ctx.dropzone_items,
        photo_image: ctx.photo_image,
        photo_meta: ctx.photo_meta,
        pro: ctx.pro,
        stack_info: ctx.stack_info,
        quote_index: ctx.quote_index,
        config_override: Some(&p.config),
    };
    apple_desktop::render_with_extras(
        &mut ui,
        m,
        p.kind,
        rect(0., 0., w, h),
        p.size,
        &p.id,
        &state,
        &extras,
    );

    if ctx.menu {
        let m_x = ((w - CONTEXT_MENU_W) * 0.5).max(4.0);
        let m_y = ((h - CONTEXT_MENU_H) * 0.5).max(4.0);
        let _ = paint_apple_context_menu(&mut ui, p, (m_x, m_y), adaptive_dark);
    }

    View {
        hits: ui.hits,
        scene,
    }
}

/// Apple 原生右键玻璃浮层尺寸常量。
pub const CONTEXT_MENU_W: f64 = 196.0;
pub const CONTEXT_MENU_H: f64 = 172.0;

/// 绘制 Apple/macOS 桌面小组件原生玻璃质感右键菜单，并返回实际绘制的浮层边界框。
pub fn paint_apple_context_menu(
    ui: &mut crate::widgets::Ui,
    p: &dotisle_domain::desktop_widgets::Placement,
    origin: (f64, f64),
    dark: bool,
) -> vello::kurbo::Rect {
    use allegro::theme::{rgba, squircle_path, WidgetMaterial};
    use vello::kurbo::{Affine, Stroke};
    use vello::peniko::Fill;

    let panel = rect(origin.0, origin.1, CONTEXT_MENU_W, CONTEXT_MENU_H);
    let rad = 14.0;
    let shape = squircle_path(panel, rad);

    // 1. macOS 级多层柔和阴影
    ui.scene.fill(
        Fill::NonZero,
        Affine::translate((0., 8.)),
        rgba(0x000000, if dark { 0.42 } else { 0.18 }),
        None,
        &squircle_path(panel, rad),
    );
    ui.scene.fill(
        Fill::NonZero,
        Affine::translate((0., 2.)),
        rgba(0x000000, if dark { 0.24 } else { 0.10 }),
        None,
        &squircle_path(panel, rad),
    );

    // 2. 高透透明亚克力面板底 + 360° 镜面微边
    let bg = if dark { rgba(0x161A24, 0.80) } else { rgba(0xF8FAFE, 0.84) };
    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, bg, None, &shape);
    ui.scene.stroke(
        &Stroke::new(0.8),
        Affine::IDENTITY,
        rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.16 } else { 0.08 }),
        None,
        &shape,
    );
    ui.scene.stroke(
        &Stroke::new(0.9),
        Affine::IDENTITY,
        rgba(0xFFFFFF, if dark { 0.22 } else { 0.72 }),
        None,
        &squircle_path(rect(panel.x0 + 1.0, panel.y0 + 1.0, panel.width() - 2.0, panel.height() - 2.0), rad - 1.0),
    );

    let fg = if dark { 0xF5F7FA } else { 0x1D1D1F };
    let muted = if dark { 0xA1A8B8 } else { 0x6E737D };
    let groove_col = rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.08 } else { 0.055 });
    let pill_active = rgba(if dark { 0x3A3F50 } else { 0xFFFFFF }, 1.0);
    let pad = 8.0;
    let inner_w = CONTEXT_MENU_W - pad * 2.0;

    // 3. 分段控制器 1：小 / 中 / 大 尺寸
    let seg1_y = panel.y0 + 7.0;
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        groove_col,
        None,
        &squircle_path(rect(panel.x0 + pad, seg1_y, inner_w, 24.0), 8.0),
    );
    let sw = inner_w / 3.0;
    for (i, choice) in Size::ALL.iter().enumerate() {
        let r = rect(panel.x0 + pad + i as f64 * sw + 2.0, seg1_y + 2.0, sw - 4.0, 20.0);
        let active = p.size == *choice;
        let hovered = r.contains(ui.pointer);
        if active {
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, pill_active, None, &squircle_path(r, 6.0));
        } else if hovered {
            ui.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                rgba(if dark { 0xFFFFFF } else { 0x000000 }, 0.06),
                None,
                &squircle_path(r, 6.0),
            );
        }
        ui.center(choice.label(), r, 10.5, if active { fg } else { muted }, active);
        ui.hit(r, choice.label(), Action::WidgetResize(p.id.clone(), *choice), true);
    }

    // 4. 分段控制器 2：柔雾 / 清透 透明亚克力挡位
    let seg2_y = seg1_y + 28.0;
    let cur_mat = WidgetMaterial::from_str_opt(&p.material);
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        groove_col,
        None,
        &squircle_path(rect(panel.x0 + pad, seg2_y, inner_w, 24.0), 8.0),
    );
    let mat_items = [
        (WidgetMaterial::Lumen, "柔雾"),
        (WidgetMaterial::Acrylic, "清透"),
    ];
    let mw = inner_w / mat_items.len() as f64;
    for (i, (m_kind, lbl)) in mat_items.iter().enumerate() {
        let r = rect(panel.x0 + pad + i as f64 * mw + 2.0, seg2_y + 2.0, mw - 4.0, 20.0);
        let active = cur_mat == *m_kind;
        let hovered = r.contains(ui.pointer);
        if active {
            ui.scene.fill(Fill::NonZero, Affine::IDENTITY, pill_active, None, &squircle_path(r, 6.0));
        } else if hovered {
            ui.scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                rgba(if dark { 0xFFFFFF } else { 0x000000 }, 0.06),
                None,
                &squircle_path(r, 6.0),
            );
        }
        ui.center(lbl, r, 10.5, if active { fg } else { muted }, active);
        ui.hit(r, lbl, Action::WidgetMaterial(p.id.clone(), m_kind.as_str().into()), true);
    }

    // 5. 细分隔线
    let div_col = rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.11 } else { 0.09 });
    let div1_y = seg2_y + 28.0;
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        div_col,
        None,
        &rect(panel.x0 + 10.0, div1_y, CONTEXT_MENU_W - 20.0, 0.7),
    );

    // 6. Apple 系统蓝悬停菜单项（紧凑排布 4 项，严守 172px 边界）
    let row_h = 22.0;
    let stacked = !p.stack_id.is_empty();
    let row1_y = div1_y + 3.0;
    if p.kind == Kind::AppFolder {
        let is_nine = p.config.caption.trim() == "9";
        let toggle_lbl = if is_nine {
            "切换为 4 宫格 (2×2)"
        } else {
            "切换为 9 宫格 (3×3)"
        };
        menu_row(
            ui,
            rect(panel.x0 + 6.0, row1_y, CONTEXT_MENU_W - 12.0, row_h),
            toggle_lbl,
            Action::AppFolderToggleGrid(p.id.clone()),
            fg,
            false,
            dark,
        );
    } else {
        menu_row(
            ui,
            rect(panel.x0 + 6.0, row1_y, CONTEXT_MENU_W - 12.0, row_h),
            "编辑组件…",
            Action::WidgetConfigure(p.id.clone()),
            fg,
            false,
            dark,
        );
    }
    let row2_y = row1_y + row_h + 1.5;
    menu_row(
        ui,
        rect(panel.x0 + 6.0, row2_y, CONTEXT_MENU_W - 12.0, row_h),
        "网格排布编辑…",
        Action::EditModeToggle,
        fg,
        false,
        dark,
    );
    let row3_y = row2_y + row_h + 1.5;
    menu_row(
        ui,
        rect(panel.x0 + 6.0, row3_y, CONTEXT_MENU_W - 12.0, row_h),
        if stacked { "从叠放移出" } else { "收起到顶栏" },
        if stacked { Action::StackUnpack(p.id.clone()) } else { Action::WidgetHide(p.id.clone()) },
        fg,
        false,
        dark,
    );

    let div2_y = row3_y + row_h + 3.0;
    ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        div_col,
        None,
        &rect(panel.x0 + 10.0, div2_y, CONTEXT_MENU_W - 20.0, 0.7),
    );

    let row4_y = div2_y + 3.0;
    menu_row(
        ui,
        rect(panel.x0 + 6.0, row4_y, CONTEXT_MENU_W - 12.0, row_h),
        "移除小组件",
        Action::WidgetRemove(p.id.clone()),
        if dark { 0xFF453A } else { 0xFF3B30 },
        true,
        dark,
    );

    panel
}

/// Draw one row in the desktop widget context menu.
fn menu_row(
    ui: &mut crate::widgets::Ui,
    r: vello::kurbo::Rect,
    label: &str,
    action: Action,
    fg: u32,
    destructive: bool,
    dark: bool,
) {
    use allegro::theme::{rgba, squircle_path};
    use vello::kurbo::Affine;
    use vello::peniko::Fill;

    let hovered = r.contains(ui.pointer);
    if hovered {
        let hi = if destructive {
            rgba(0xFF453A, if dark { 0.85 } else { 0.92 })
        } else {
            rgba(0x0A84FF, 0.94)
        };
        ui.scene.fill(Fill::NonZero, Affine::IDENTITY, hi, None, &squircle_path(r, 7.0));
    }
    let text_col = if hovered { 0xFFFFFF } else { fg };
    ui.label(label, r.x0 + 10.0, r.y0 + 3.0, 11.5, text_col, false, r.width() - 20.0);
    ui.hit(r, label, action, true);
}