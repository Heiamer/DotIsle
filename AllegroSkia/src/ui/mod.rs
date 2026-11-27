pub mod glass;
pub mod island;
pub mod sidebar;
pub mod widgets;
use crate::{
    async_io::Snapshot,
    file_shelf::FileShelf,
    reminders::{ActiveAlert, ReminderState},
    state::{Board, ExtraWidgetState, HotkeyPreset, SearchResult, SidebarEdge, SidebarSettings, TodoList},
};
use skia_safe::{
    Canvas, Color, Contains, FontMgr, FontStyle, Paint, Point, Rect, TileMode,
    font_style::{Slant, Weight, Width},
    gradient_shader,
    paint::Style,
    surfaces,
    textlayout::{FontCollection, ParagraphBuilder, ParagraphStyle, TextAlign, TextStyle},
};
use std::path::Path;

#[derive(Clone, Debug)]
pub enum Action {
    Search,
    Close,
    Edit,
    Theme,
    ToggleSettings,
    SetSidebarEdge(SidebarEdge),
    SetSidebarHotkey(HotkeyPreset),
    RefreshAiQuota,
    OpenAiQuotaConfig,
    ShelfDragAll,
    ShelfDragItem(usize),
    ShelfRemove(usize),
    ShelfClear,
    AddDrawer,
    DrawerDrag(String),
    Add(String),
    Card(u64),
    Remove(u64),
    Resize(u64),
    Acrylic(u64),
    Move(u64, i32),
    Turn(u64, i32),
    Merge(u64),
    Split(u64),
    Result(usize),
    Copy(String),
    Context(u64),
    FocusToggle,
    FocusPreset(u32),
    TodoToggle(usize),
    QuoteNext,
    NoteFinish,
    NoteClear(u64),
    ToggleSidebarEnabled,
    ToggleHotkeyHint,
    OpenTutorial,
    TutorialNext,
    TutorialPrev,
    TutorialComplete,
    TutorialDismiss,
    WaterDrink,
    WaterCupDelta(i32),
    WaterGoalDelta(i32),
    WaterCycleInterval,
    WaterToggle,
    WaterPreviewIsland,
    StandAck,
    StandCycleInterval,
    StandToggle,
    StandPreviewIsland,
    IslandAckWater,
    IslandAckStand,
    HabitToggle(usize),
    AmbientToggle,
    AmbientNext,
    PaletteNext,
    WoodFishTap,
    BreathToggle,
    CalcKey(String),
}

#[derive(Clone, Debug)]
pub struct HitItem {
    pub rect: Rect,
    pub label: String,
    pub action: Action,
}
impl HitItem {
    pub fn new(rect: Rect, label: impl Into<String>, action: Action) -> Self {
        Self {
            rect,
            label: label.into(),
            action,
        }
    }
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub board: &'a Board,
    pub data: &'a Snapshot,
    pub dark: bool,
    pub edit: bool,
    pub query: &'a str,
    pub preedit: &'a str,
    pub results: &'a [SearchResult],
    pub scroll: f32,
    pub hover: Option<(f32, f32)>,
    pub menu: Option<(u64, (f32, f32))>,
    pub drawer: bool,
    pub drawer_progress: f32,
    pub drawer_scroll: f32,
    pub dragging_card: Option<(&'a str, (f32, f32), Option<usize>)>,
    pub settings: SidebarSettings,
    pub settings_open: bool,
    pub hotkey_native_ok: bool,
    pub file_shelf: &'a FileShelf,
    pub file_hovering: bool,
    pub todos: &'a TodoList,
    pub quote_index: usize,
    pub editing_note: Option<u64>,
    pub note_preedit: &'a str,
    pub reminders: &'a ReminderState,
    pub extra: &'a ExtraWidgetState,
    pub tutorial_open: bool,
    pub tutorial_step: u8,
    #[allow(dead_code)]
    pub time_sec: f32,
    pub focus_running: bool,
    pub focus_left: u32,
    pub focus_total: u32,
}

pub fn ink(dark: bool) -> Color {
    if dark {
        Color::from_rgb(245, 247, 252)
    } else {
        Color::from_rgb(24, 28, 36)
    }
}

pub fn muted(dark: bool) -> Color {
    if dark {
        Color::from_rgb(164, 174, 194)
    } else {
        Color::from_rgb(92, 102, 120)
    }
}

pub fn accent_blue(dark: bool) -> Color {
    if dark {
        Color::from_rgb(10, 132, 255)
    } else {
        Color::from_rgb(0, 122, 255)
    }
}

/// 精排版文字输出（默认常规字重、左对齐）。
pub fn text(c: &Canvas, s: &str, x: f32, y: f32, width: f32, size: f32, color: Color) {
    text_styled(c, s, x, y, width, size, color, Weight::NORMAL, TextAlign::Left, 3);
}

/// 完整排版文字函数：支持字重、对齐与行数。
#[allow(clippy::too_many_arguments)]
pub fn text_styled(
    c: &Canvas,
    s: &str,
    x: f32,
    y: f32,
    width: f32,
    size: f32,
    color: Color,
    weight: Weight,
    align: TextAlign,
    max_lines: usize,
) {
    thread_local! {
        static FONTS: FontCollection = {
            let mut f = FontCollection::new();
            f.set_default_font_manager(FontMgr::default(), None);
            f
        };
    }
    let fonts = FONTS.with(Clone::clone);
    let mut style = TextStyle::new();
    style.set_font_families(&[
        "SF Pro Display",
        "Segoe UI Variable Display",
        "HarmonyOS Sans SC",
        "Microsoft YaHei UI",
        "Segoe UI",
    ]);
    style.set_font_style(FontStyle::new(weight, Width::NORMAL, Slant::Upright));
    style.set_font_size(size);
    style.set_color(color);
    let mut paragraph = ParagraphStyle::new();
    paragraph.set_text_style(&style);
    paragraph.set_text_align(align);
    paragraph.set_max_lines(max_lines);
    paragraph.set_ellipsis("…");
    let mut builder = ParagraphBuilder::new(&paragraph, fonts);
    builder.add_text(s);
    let mut p = builder.build();
    p.layout(width.max(1.));
    p.paint(c, (x, y));
}

pub fn button(
    c: &Canvas,
    r: Rect,
    label: &str,
    action: Action,
    v: &View<'_>,
    hits: &mut Vec<HitItem>,
) {
    let hovered = v.hover.is_some_and(|p| r.contains(Point::from(p)));
    glass::liquid_pill(c, r, 11., v.dark, hovered);

    text_styled(
        c,
        label,
        r.left + 8.,
        r.top + (r.height() - 16.).max(4.) * 0.5,
        (r.width() - 16.).max(8.),
        12.5,
        if hovered { accent_blue(v.dark) } else { ink(v.dark) },
        Weight::MEDIUM,
        TextAlign::Center,
        1,
    );
    hits.push(HitItem::new(r, label, action));
}

pub fn overlays(
    c: &Canvas,
    v: &View<'_>,
    width: f32,
    height: f32,
    hits: &mut Vec<HitItem>,
    backdrop: Option<(&skia_safe::Image, (f32, f32))>,
) {
    if let Some((id, (mx, my))) = v.menu {
        let x = mx.clamp(12., (width - 236.).max(12.));
        let y = my.clamp(12., (height - 326.).max(12.));
        let r = Rect::from_xywh(x, y, 224., 314.);
        glass::panel_radius(c, r, 18., v.dark, 2, backdrop);
        let options = [
            ("切换尺寸 (小 / 大 / 特大)", Action::Resize(id)),
            ("亚克力通透挡位", Action::Acrylic(id)),
            ("区块顺序上移", Action::Move(id, -1)),
            ("区块顺序下移", Action::Move(id, 1)),
            ("与同尺寸卡片智能叠放", Action::Merge(id)),
            ("拆出当前叠放", Action::Split(id)),
            ("移除小组件", Action::Remove(id)),
        ];
        for (i, (label, action)) in options.into_iter().enumerate() {
            let br = Rect::from_xywh(x + 12., y + 12. + i as f32 * 41., 200., 35.);
            if i == 6 {
                let hovered = v.hover.is_some_and(|p| br.contains(Point::from(p)));
                let mut rp = Paint::default();
                rp.set_anti_alias(true);
                rp.set_color(Color::from_argb(
                    if hovered { 52 } else { 28 },
                    255,
                    59,
                    48,
                ));
                c.draw_round_rect(br, 11., 11., &rp);
                text_styled(
                    c,
                    label,
                    br.left + 8.,
                    br.top + 9.5,
                    (br.width() - 16.).max(8.),
                    12.5,
                    Color::from_rgb(255, 59, 48),
                    Weight::SEMI_BOLD,
                    TextAlign::Center,
                    1,
                );
                hits.push(HitItem::new(br, label, action));
            } else {
                button(c, br, label, action, v, hits);
            }
        }
    }
    if v.drawer {
        let panel_w = 668.0_f32;
        let items = sidebar::GALLERY_ITEMS;
        let cols = 4usize;
        let rows = items.len().div_ceil(cols);
        let panel_h = 62. + rows as f32 * 40. + 14.;
        let x = ((width - panel_w) * 0.5).clamp(12., (width - panel_w - 12.).max(12.));
        let y = 74.0_f32.min((height - panel_h - 12.).max(16.));
        let dr = Rect::from_xywh(x, y, panel_w, panel_h);
        glass::panel_radius(c, dr, 22., v.dark, 2, backdrop);
        hits.push(HitItem::new(dr, "", Action::Card(0)));
        text_styled(
            c,
            &format!("添加桌面组件（共 {} 款）", items.len()),
            x + 18.,
            y + 15.,
            360.,
            15.,
            ink(v.dark),
            Weight::BOLD,
            TextAlign::Left,
            1,
        );
        button(
            c,
            Rect::from_xywh(dr.right - 84., y + 11., 68., 28.),
            "收起",
            Action::AddDrawer,
            v,
            hits,
        );
        for (i, &(kind, title, wide)) in items.iter().enumerate() {
            let col = i % cols;
            let row = i / cols;
            let tag = if wide { "2×1" } else { "1×1" };
            let label = format!("{title} {tag}");
            button(
                c,
                Rect::from_xywh(
                    x + 16. + col as f32 * 160.,
                    y + 52. + row as f32 * 40.,
                    152.,
                    33.,
                ),
                &label,
                Action::Add(kind.into()),
                v,
                hits,
            );
        }
    }
    if v.tutorial_open {
        // 半透明压暗遮罩 + 与天气设置一致的 Apple 引导设置模态弹窗
        let mut dim = Paint::default();
        dim.set_color(Color::from_argb(if v.dark { 112 } else { 72 }, 10, 14, 24));
        c.draw_rect(Rect::from_xywh(0., 0., width, height), &dim);

        let tw = 492.0_f32.min((width - 32.).max(320.));
        let th = 468.0_f32.min((height - 32.).max(360.));
        let tx = (width - tw) * 0.5;
        let ty = (height - th) * 0.5;
        let tr = Rect::from_xywh(tx, ty, tw, th);
        glass::panel_radius(c, tr, 26., v.dark, 2, backdrop);
        hits.push(HitItem::new(tr, "", Action::Card(0)));

        // 顶部步进胶囊
        let step = v.tutorial_step.min(1);
        for (si, lbl) in ["1 使用与召唤教程", "2 启用与热键设置"].into_iter().enumerate() {
            let sr = Rect::from_xywh(tx + 22. + si as f32 * 144., ty + 20., 136., 24.);
            let cur = step as usize == si;
            let mut sp = Paint::default();
            sp.set_anti_alias(true);
            sp.set_color(if cur {
                accent_blue(v.dark)
            } else {
                Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128)
            });
            c.draw_round_rect(sr, 12., 12., &sp);
            text_styled(
                c,
                lbl,
                sr.left,
                sr.top + 4.5,
                sr.width(),
                10.,
                if cur { Color::WHITE } else { muted(v.dark) },
                Weight::SEMI_BOLD,
                TextAlign::Center,
                1,
            );
            hits.push(HitItem::new(
                sr,
                lbl,
                if si == 0 { Action::TutorialPrev } else { Action::TutorialNext },
            ));
        }
        let close_r = Rect::from_xywh(tr.right - 72., ty + 19., 50., 26.);
        glass::liquid_pill(c, close_r, 13., v.dark, false);
        text_styled(c, "跳过", close_r.left, close_r.top + 5.5, 50., 10.5, muted(v.dark), Weight::MEDIUM, TextAlign::Center, 1);
        hits.push(HitItem::new(close_r, "稍后再说", Action::TutorialDismiss));

        if step == 0 {
            text_styled(c, "欢迎使用全新侧边栏与桌面组件", tx + 24., ty + 62., tw - 48., 19., ink(v.dark), Weight::BOLD, TextAlign::Left, 1);
            text_styled(
                c,
                "三步上手高效桌面工作台",
                tx + 24.,
                ty + 89.,
                tw - 48.,
                11.5,
                muted(v.dark),
                Weight::NORMAL,
                TextAlign::Left,
                1,
            );

            let tips = [
                (
                    Color::from_rgb(0, 122, 255),
                    "1. 热键一键召唤与靠边停靠",
                    format!("随时按下 {}（可在下一步修改）呼出或收起侧边栏，支持屏幕左侧或右侧展开。", v.settings.hotkey_label()),
                ),
                (
                    Color::from_rgb(52, 199, 89),
                    "2. 组件库抽屉自由拖放与桌面互动",
                    "点顶部 ＋ 展开 34 款 Apple 风格组件抽屉，按住拖到任意位置；桌面组件直接拖动、右键菜单、便签可双击写字。".to_string(),
                ),
                (
                    Color::from_rgb(10, 182, 255),
                    "3. 喝水 / 久坐提醒灵动岛",
                    "设定每次喝水毫升数与每日目标，到点顶部自动弹出黑色灵动岛提醒，喝完点一下立即消失并在下周期再弹。".to_string(),
                ),
            ];
            for (i, (col, head, body)) in tips.into_iter().enumerate() {
                let cr = Rect::from_xywh(tx + 24., ty + 124. + i as f32 * 88., tw - 48., 78.);
                glass::liquid_pill(c, cr, 16., v.dark, false);
                let mut cp = Paint::default();
                cp.set_anti_alias(true);
                cp.set_color(col);
                c.draw_round_rect(Rect::from_xywh(cr.left + 12., cr.top + 16., 4., 46.), 2., 2., &cp);
                text_styled(c, head, cr.left + 26., cr.top + 12., cr.width() - 38., 13., ink(v.dark), Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &body, cr.left + 26., cr.top + 33., cr.width() - 38., 10.8, muted(v.dark), Weight::NORMAL, TextAlign::Left, 2);
            }

            button(
                c,
                Rect::from_xywh(tr.right - 196., tr.bottom - 56., 172., 36.),
                "下一步：启用与热键设置",
                Action::TutorialNext,
                v,
                hits,
            );
        } else {
            text_styled(c, "配置你的侧边栏呼出方式", tx + 24., ty + 62., tw - 48., 18.5, ink(v.dark), Weight::BOLD, TextAlign::Left, 1);
            text_styled(
                c,
                "跟天气弹窗一样轻松选好开关与热键，之后仍可在设置中随时二次修改",
                tx + 24.,
                ty + 88.,
                tw - 48.,
                11.,
                muted(v.dark),
                Weight::NORMAL,
                TextAlign::Left,
                1,
            );

            // 是否启用侧边栏
            let en_r = Rect::from_xywh(tx + 24., ty + 122., tw - 48., 44.);
            glass::liquid_pill(c, en_r, 14., v.dark, false);
            text_styled(c, "启用侧边栏与全局热键", en_r.left + 14., en_r.top + 13., 220., 13., ink(v.dark), Weight::SEMI_BOLD, TextAlign::Left, 1);
            let sw_r = Rect::from_xywh(en_r.right - 88., en_r.top + 8., 76., 28.);
            let mut swp = Paint::default();
            swp.set_anti_alias(true);
            swp.set_color(if v.settings.enabled { Color::from_rgb(52, 199, 89) } else { Color::from_argb(60, 120, 120, 128) });
            c.draw_round_rect(sw_r, 14., 14., &swp);
            text_styled(c, if v.settings.enabled { "已启用" } else { "已关闭" }, sw_r.left, sw_r.top + 6.5, sw_r.width(), 11., Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(en_r, "切换启用侧边栏", Action::ToggleSidebarEnabled));

            // 展开方向
            text_styled(c, "展开方向", tx + 24., ty + 180., 120., 11., muted(v.dark), Weight::SEMI_BOLD, TextAlign::Left, 1);
            let ew = (tw - 58.) * 0.5;
            for (i, (edge, lbl)) in [(SidebarEdge::Left, "靠左展开"), (SidebarEdge::Right, "靠右展开")].into_iter().enumerate() {
                let er = Rect::from_xywh(tx + 24. + i as f32 * (ew + 10.), ty + 200., ew, 32.);
                let active = v.settings.edge == edge;
                if active {
                    let mut fp = Paint::default();
                    fp.set_anti_alias(true);
                    fp.set_color(accent_blue(v.dark));
                    c.draw_round_rect(er, 10., 10., &fp);
                } else {
                    glass::liquid_pill(c, er, 10., v.dark, false);
                }
                text_styled(c, lbl, er.left, er.top + 8., er.width(), 12., if active { Color::WHITE } else { ink(v.dark) }, Weight::SEMI_BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(er, lbl, Action::SetSidebarEdge(edge)));
            }

            // 呼出热键 5 选 1
            text_styled(c, "呼出热键（可二次在顶部齿轮设置更改）", tx + 24., ty + 244., tw - 48., 11., muted(v.dark), Weight::SEMI_BOLD, TextAlign::Left, 1);
            let presets = HotkeyPreset::all();
            let pw3 = (tw - 48. - 16.) / 3.;
            let pw2 = (tw - 48. - 8.) * 0.5;
            for (i, preset) in presets.into_iter().enumerate() {
                let pr = if i < 3 {
                    Rect::from_xywh(tx + 24. + i as f32 * (pw3 + 8.), ty + 264., pw3, 29.)
                } else {
                    Rect::from_xywh(tx + 24. + (i - 3) as f32 * (pw2 + 8.), ty + 299., pw2, 29.)
                };
                let active = v.settings.hotkey == preset;
                if active {
                    let mut fp = Paint::default();
                    fp.set_anti_alias(true);
                    fp.set_color(accent_blue(v.dark));
                    c.draw_round_rect(pr, 9., 9., &fp);
                } else {
                    glass::liquid_pill(c, pr, 9., v.dark, false);
                }
                text_styled(c, preset.label(), pr.left, pr.top + 7., pr.width(), 11., if active { Color::WHITE } else { ink(v.dark) }, Weight::SEMI_BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(pr, preset.label(), Action::SetSidebarHotkey(preset)));
            }

            // 热键提醒开关
            let hr = Rect::from_xywh(tx + 24., ty + 340., tw - 48., 38.);
            glass::liquid_pill(c, hr, 12., v.dark, false);
            text_styled(c, "在顶栏展示热键提醒胶囊", hr.left + 14., hr.top + 10.5, 220., 12., ink(v.dark), Weight::MEDIUM, TextAlign::Left, 1);
            text_styled(
                c,
                if v.settings.show_hotkey_hint { "开启" } else { "关闭" },
                hr.right - 68.,
                hr.top + 10.5,
                54.,
                11.5,
                if v.settings.show_hotkey_hint { accent_blue(v.dark) } else { muted(v.dark) },
                Weight::BOLD,
                TextAlign::Right,
                1,
            );
            hits.push(HitItem::new(hr, "切换热键提醒显示", Action::ToggleHotkeyHint));

            button(c, Rect::from_xywh(tx + 24., tr.bottom - 54., 110., 34.), "← 上一步", Action::TutorialPrev, v, hits);
            let save_r = Rect::from_xywh(tr.right - 178., tr.bottom - 54., 154., 34.);
            let mut sp = Paint::default();
            sp.set_anti_alias(true);
            sp.set_color(accent_blue(v.dark));
            c.draw_round_rect(save_r, 12., 12., &sp);
            text_styled(c, "保存并开始使用", save_r.left, save_r.top + 8.5, save_r.width(), 12.5, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(save_r, "保存并完成引导", Action::TutorialComplete));
        }
    }
}

/// 生成经典的 macOS Sonoma / Sequoia 山海蓝青暖曦桌面壁纸。
fn synthetic_aurora_wallpaper(width: i32, height: i32, dark: bool) -> Option<skia_safe::Image> {
    let mut s = surfaces::raster_n32_premul((width, height))?;
    let c = s.canvas();
    let cols = if dark {
        [
            Color::from_rgb(12, 24, 52),
            Color::from_rgb(24, 56, 98),
            Color::from_rgb(14, 28, 56),
        ]
    } else {
        [
            Color::from_rgb(132, 188, 242),
            Color::from_rgb(198, 224, 248),
            Color::from_rgb(232, 214, 206),
        ]
    };
    let pos = [0.0_f32, 0.55, 1.0];
    if let Some(sh) = gradient_shader::linear(
        (Point::new(0., 0.), Point::new(width as f32, height as f32)),
        &cols[..],
        &pos[..],
        TileMode::Clamp,
        None,
        None,
    ) {
        let mut p = Paint::default();
        p.set_shader(sh);
        c.draw_rect(Rect::from_xywh(0., 0., width as f32, height as f32), &p);
    }
    let rect = Rect::from_xywh(0., 0., width as f32, height as f32);
    if dark {
        glass::glow_blob(c, rect, 0., (56, 189, 248), 0.72, 0.28, 0.72, 115);
        glass::glow_blob(c, rect, 0., (245, 158, 11), 0.24, 0.78, 0.56, 65);
        glass::glow_blob(c, rect, 0., (52, 211, 153), 0.85, 0.80, 0.58, 72);
    } else {
        glass::glow_blob(c, rect, 0., (56, 155, 255), 0.76, 0.24, 0.70, 90);
        glass::glow_blob(c, rect, 0., (251, 191, 36), 0.25, 0.76, 0.58, 78);
        glass::glow_blob(c, rect, 0., (99, 102, 241), 0.82, 0.82, 0.56, 48);
    }
    Some(s.image_snapshot())
}

pub fn render_previews(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let desktop_board = Board::demo();
    let sidebar_board = Board::sidebar_demo();
    let demo_shelf = FileShelf::demo();
    let demo_todos = TodoList::demo();
    let demo_rem = ReminderState::demo();
    let demo_extra = ExtraWidgetState::default();
    for dark in [false, true] {
        let mode = if dark { "dark" } else { "light" };
        let wall_sidebar = synthetic_aurora_wallpaper(448, 1220, dark);
        let wall_widgets = synthetic_aurora_wallpaper(840, 680, dark);

        let mut sb_data = Snapshot::default();
        if let Some(bg) = &wall_sidebar {
            sb_data.wallpaper_blurred = crate::async_io::preblur_wallpaper(bg);
        }
        sb_data.wallpaper = wall_sidebar.clone();
        sb_data.wallpaper_status = "极光壁纸".into();
        sb_data.indexed = true;
        sb_data.metrics.cpu = Some(27.0);
        sb_data.metrics.memory = Some(54.0);
        sb_data.metrics.disk = Some(55.0);
        sb_data.metrics.battery = Some(86);
        sb_data.metrics.download = 285.0 * 1024.0;
        sb_data.metrics.upload = 42.0 * 1024.0;
        sb_data.ai_quota = crate::ai_quota::demo_snapshot();
        sb_data.clipboard.push("cargo +stable-x86_64-pc-windows-msvc build --release".into());

        let vs = View {
            board: &sidebar_board,
            data: &sb_data,
            dark,
            edit: false,
            query: "",
            preedit: "",
            results: &[],
            scroll: 0.,
            hover: None,
            menu: None,
            drawer: false,
            drawer_progress: 0.,
            drawer_scroll: 0.,
            dragging_card: None,
            settings: SidebarSettings::default(),
            settings_open: false,
            hotkey_native_ok: true,
            file_shelf: &demo_shelf,
            file_hovering: false,
            todos: &demo_todos,
            quote_index: 0,
            editing_note: None,
            note_preedit: "",
            reminders: &demo_rem,
            extra: &demo_extra,
            tutorial_open: false,
            tutorial_step: 0,
            time_sec: 1.4,
            focus_running: true,
            focus_left: 25 * 60 - 185,
            focus_total: 25 * 60,
        };
        crate::surface::write_png_file(
            &dir.join(format!("sidebar-{mode}.png")),
            448,
            1220,
            2.,
            |c| {
                if let Some(bg) = &wall_sidebar {
                    c.draw_image(bg, (0., 0.), None);
                }
                sidebar::draw(c, 448., 1220., 1., &vs);
            },
        )?;

        // 导出包含可视化组件库抽屉及实时拖放占位效果的高清图
        let wall_drawer = synthetic_aurora_wallpaper(816, 1080, dark);
        let mut dw_data = Snapshot::default();
        if let Some(bg) = &wall_drawer {
            dw_data.wallpaper_blurred = crate::async_io::preblur_wallpaper(bg);
        }
        dw_data.wallpaper = wall_drawer.clone();
        dw_data.metrics = sb_data.metrics.clone();
        dw_data.clipboard = sb_data.clipboard.clone();
        dw_data.ai_quota = sb_data.ai_quota.clone();
        let vd = View {
            board: &sidebar_board,
            data: &dw_data,
            dark,
            edit: false,
            query: "",
            preedit: "",
            results: &[],
            scroll: 0.,
            hover: None,
            menu: None,
            drawer: true,
            drawer_progress: 1.,
            drawer_scroll: 0.,
            dragging_card: Some(("music", (520., 360.), Some(2))),
            settings: SidebarSettings::default(),
            settings_open: false,
            hotkey_native_ok: true,
            file_shelf: &demo_shelf,
            file_hovering: false,
            todos: &demo_todos,
            quote_index: 0,
            editing_note: None,
            note_preedit: "",
            reminders: &demo_rem,
            extra: &demo_extra,
            tutorial_open: false,
            tutorial_step: 0,
            time_sec: 1.4,
            focus_running: true,
            focus_left: 25 * 60 - 185,
            focus_total: 25 * 60,
        };
        crate::surface::write_png_file(
            &dir.join(format!("sidebar-drawer-{mode}.png")),
            816,
            1080,
            2.,
            |c| {
                if let Some(bg) = &wall_drawer {
                    c.draw_image(bg, (0., 0.), None);
                }
                sidebar::draw(c, 816., 1080., 1., &vd);
            },
        )?;

        if !dark {
            // 额外导出：靠左展开 + 打开侧边栏设置面板 + 组件库右侧展开完整图
            let vl = View {
                settings: SidebarSettings {
                    edge: SidebarEdge::Left,
                    hotkey: HotkeyPreset::AltSpace,
                    ..Default::default()
                },
                settings_open: true,
                dragging_card: None,
                ..vd
            };
            crate::surface::write_png_file(
                &dir.join("sidebar-settings-left-light.png"),
                816,
                1080,
                2.,
                |c| {
                    if let Some(bg) = &wall_drawer {
                        c.draw_image(bg, (0., 0.), None);
                    }
                    sidebar::draw(c, 816., 1080., 1., &vl);
                },
            )?;
        }

        if let Some(bg) = &wall_widgets {
            sb_data.wallpaper_blurred = crate::async_io::preblur_wallpaper(bg);
        }
        sb_data.wallpaper = wall_widgets.clone();
        let vw = View {
            board: &desktop_board,
            data: &sb_data,
            dark,
            edit: false,
            query: "",
            preedit: "",
            results: &[],
            scroll: 0.,
            hover: None,
            menu: None,
            drawer: false,
            drawer_progress: 0.,
            drawer_scroll: 0.,
            dragging_card: None,
            settings: SidebarSettings::default(),
            settings_open: false,
            hotkey_native_ok: true,
            file_shelf: &demo_shelf,
            file_hovering: false,
            todos: &demo_todos,
            quote_index: 0,
            editing_note: None,
            note_preedit: "",
            reminders: &demo_rem,
            extra: &demo_extra,
            tutorial_open: false,
            tutorial_step: 0,
            time_sec: 1.4,
            focus_running: true,
            focus_left: 25 * 60 - 185,
            focus_total: 25 * 60,
        };
        for &(kind,title,_) in sidebar::GALLERY_ITEMS {
            for (size_name,size) in [("small",crate::state::BlockSize::Small),("medium",crate::state::BlockSize::Medium),("large",crate::state::BlockSize::Large)] {
                let mut card=desktop_board.make_card(kind);card.title=title.into();card.size=size;
                let (w,h)=size.dimensions();
                crate::surface::write_png_file(&dir.join(format!("{kind}-{size_name}-{mode}.png")),(w+40.) as u32,(h+40.) as u32,2.,|c|{
                    widgets::card(c,Rect::from_xywh(20.,16.,w,h),&card,&vw,&mut vec![],None);
                })?;
            }
        }
        crate::surface::write_png_file(
            &dir.join(format!("widgets-{mode}.png")),
            840,
            680,
            2.,
            |c| {
                if let Some(bg) = &wall_widgets {
                    c.draw_image(bg, (0., 0.), None);
                }
                widgets::desktop(c, 840., 680., &vw);
            },
        )?;

        if !dark {
            let vt = View {
                tutorial_open: true,
                tutorial_step: 1,
                ..vw
            };
            crate::surface::write_png_file(
                &dir.join("sidebar-tutorial-light.png"),
                840,
                680,
                2.,
                |c| {
                    if let Some(bg) = &wall_widgets {
                        c.draw_image(bg, (0., 0.), None);
                    }
                    widgets::desktop(c, 840., 680., &vt);
                },
            )?;

            let wall_isl = synthetic_aurora_wallpaper(420, 112, false);
            crate::surface::write_png_file(
                &dir.join("reminder-island-water.png"),
                420,
                112,
                2.,
                |c| {
                    if let Some(bg) = &wall_isl {
                        c.draw_image(bg, (0., 0.), None);
                    }
                    island::draw(c, 420., 112., 1., ActiveAlert::Water, &demo_rem, None);
                },
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn render_frame_fast() {
        let wall = synthetic_aurora_wallpaper(1920, 1080, false).unwrap();
        let mut data = Snapshot::default();
        data.wallpaper_blurred = crate::async_io::preblur_wallpaper(&wall);
        data.wallpaper = Some(wall);
        data.metrics.cpu = Some(27.0);
        data.metrics.memory = Some(54.0);
        data.metrics.disk = Some(55.0);
        data.metrics.battery = Some(86);
        let board = Board::sidebar_demo();
        let demo_shelf = FileShelf::demo();
        let demo_todos = TodoList::demo();
        let demo_rem = ReminderState::demo();
        let demo_extra = ExtraWidgetState::default();
        let v = View {
            board: &board,
            data: &data,
            dark: false,
            edit: false,
            query: "",
            preedit: "",
            results: &[],
            scroll: 0.,
            hover: None,
            menu: None,
            drawer: false,
            drawer_progress: 0.,
            drawer_scroll: 0.,
            dragging_card: None,
            settings: SidebarSettings::default(),
            settings_open: false,
            hotkey_native_ok: true,
            file_shelf: &demo_shelf,
            file_hovering: false,
            todos: &demo_todos,
            quote_index: 0,
            editing_note: None,
            note_preedit: "",
            reminders: &demo_rem,
            extra: &demo_extra,
            tutorial_open: false,
            tutorial_step: 0,
            time_sec: 1.4,
            focus_running: true,
            focus_left: 1200,
            focus_total: 1500,
        };
        let mut s = surfaces::raster_n32_premul((896, 2160)).unwrap();
        let t0 = std::time::Instant::now();
        let c = s.canvas();
        c.clear(Color::TRANSPARENT);
        c.save();
        c.scale((2., 2.));
        let hits = sidebar::draw(c, 448., 1080., 1., &v);
        c.restore();
        let elapsed = t0.elapsed();
        eprintln!("AllegroSkia 2x 侧边栏单帧渲染耗时：{elapsed:?}（Debug 含文字排印）");
        assert!(!hits.is_empty());
        assert!(elapsed.as_millis() < 250);
    }
}
