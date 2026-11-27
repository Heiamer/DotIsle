use super::*;
use crate::state::{BlockSize, Card};
use skia_safe::{Point, paint::Cap};

pub const PANEL_WIDTH: f32 = 416.;
pub const DRAWER_WIDTH: f32 = 356.;
pub const DRAWER_GAP: f32 = 12.;
pub const WINDOW_WIDTH: f32 = PANEL_WIDTH + DRAWER_WIDTH + DRAWER_GAP + 32.;

/// 小组件库目录（34 类完整 Apple 原生与日常效用组件）：(组件类型, 中文名称, 是否为宽卡 2×1)。
pub const GALLERY_ITEMS: &[(&str, &str, bool)] = &[
    ("hydration", "喝水提醒", true),
    ("sedentary", "久坐提醒", true),
    ("todo", "待办事项", true),
    ("ai_quota", "AI 额度", true),
    ("file_shelf", "文件中转站", true),
    ("analog_clock", "时钟", false),
    ("weather", "天气", false),
    ("focus", "计时器", false),
    ("apps", "快捷指令", false),
    ("note", "便签", false),
    ("clipboard", "剪贴板历史", false),
    ("anniversary", "纪念日倒数", false),
    ("schedule", "日程倒计时", false),
    ("quote", "杂志日签", false),
    ("countdown_ring", "下班倒计时", false),
    ("color_picker", "灵感色板", false),
    ("wooden_fish", "解压木鱼", false),
    ("breathing", "呼吸引导", false),
    ("quick_calc", "迷你计算器", false),
    ("system", "电池与性能", true),
    ("calendar", "日历", true),
    ("today", "今日概览", true),
    ("music", "音乐", true),
    ("habit", "习惯打卡", true),
    ("pomodoro_wave", "白噪音氛围", true),
    ("net_speed", "实时网速", true),
    ("converter", "汇率快查", true),
    ("github_grid", "贡献热力图", true),
    ("wireless", "无线设备电量", true),
    ("storage", "存储空间", true),
    ("screen_time", "用机时长", true),
    ("focus_stats", "专注统计", true),
    ("activity", "健身圆环", true),
    ("clock", "世界时钟", true),
];

/// 小组件库排版列表，返回 `(index, x, y, w, h)`（相对抽屉内容视口）。
fn gallery_layout() -> (Vec<(usize, f32, f32, f32, f32)>, f32) {
    let mut items = Vec::with_capacity(GALLERY_ITEMS.len());
    let mut y = 0.0_f32;
    let mut col = 0usize;
    let gap = 14.0_f32;
    let small_w = 152.0_f32;
    let wide_w = small_w * 2. + gap;
    let item_h = 174.0_f32; // 146 缩略图高度 + 28 标签高度

    for (idx, &(_, _, wide)) in GALLERY_ITEMS.iter().enumerate() {
        if wide {
            if col > 0 {
                y += item_h + gap;
                col = 0;
            }
            items.push((idx, 0., y, wide_w, item_h));
            y += item_h + gap;
        } else {
            let x = col as f32 * (small_w + gap);
            items.push((idx, x, y, small_w, item_h));
            col += 1;
            if col == 2 {
                col = 0;
                y += item_h + gap;
            }
        }
    }
    if col > 0 {
        y += item_h + gap;
    }
    (items, y)
}

/// 返回组件库总高度，供滚轮边界计算。
pub fn drawer_content_height() -> f32 {
    gallery_layout().1
}

/// 返回本机实时日期与时间：(年, 月, 日, 星期0=日..6=六, 时, 分, 秒)。
pub fn local_datetime() -> (i32, u32, u32, u32, u32, u32, u32) {
    let (y, m, d, dow, h, min, sec_f) = local_datetime_precise();
    (y, m, d, dow, h, min, sec_f.floor() as u32)
}

/// 返回含亚秒级毫秒分量的本机实时日期与时间，供指针时钟连续扫秒使用。
pub fn local_datetime_precise() -> (i32, u32, u32, u32, u32, u32, f32) {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let sec_f = (st.wSecond as f32 + st.wMilliseconds as f32 / 1000.0).clamp(0.0, 59.999);
    (
        st.wYear as i32,
        st.wMonth as u32,
        st.wDay as u32,
        st.wDayOfWeek as u32,
        st.wHour as u32,
        st.wMinute as u32,
        sec_f,
    )
}

/// 计算连续扫秒秒针弧度（12 点方向为 `-PI/2`）。
pub fn sweep_second_angle(sec_f: f32) -> f32 {
    sec_f * std::f32::consts::TAU / 60.0 - std::f32::consts::FRAC_PI_2
}

fn weekday_cn(dow: u32) -> &'static str {
    match dow {
        1 => "星期一",
        2 => "星期二",
        3 => "星期三",
        4 => "星期四",
        5 => "星期五",
        6 => "星期六",
        _ => "星期日",
    }
}

/// 绘制极简液态玻璃圆形图标按钮。
fn glass_circle_btn(
    c: &Canvas,
    cx: f32,
    cy: f32,
    r: f32,
    active: bool,
    action: Action,
    label: &str,
    v: &View<'_>,
    hits: &mut Vec<HitItem>,
    draw_glyph: impl FnOnce(&Canvas, Color),
) {
    let rect = Rect::from_xywh(cx - r, cy - r, r * 2., r * 2.);
    let hovered = v.hover.is_some_and(|p| rect.contains(Point::from(p)));
    if active {
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        fill.set_color(accent_blue(v.dark));
        c.draw_circle((cx, cy), r, &fill);
        glass::draw_specular_rim(c, rect, r, v.dark, true);
    } else {
        glass::liquid_circle(c, cx, cy, r, v.dark, hovered);
    }

    let fg = if active { Color::WHITE } else { ink(v.dark) };
    draw_glyph(c, fg);
    hits.push(HitItem::new(rect, label, action));
}

/// 绘制停靠在侧边栏外侧的 Apple 小组件库抽屉（支持靠左/靠右展开方向自适应，支持直接拖入侧边栏）。
fn draw_widget_drawer(
    c: &Canvas,
    width: f32,
    height: f32,
    sidebar_left: f32,
    v: &View<'_>,
    hits: &mut Vec<HitItem>,
) {
    let prog = v.drawer_progress.clamp(0., 1.);
    if prog <= 0.01 {
        return;
    }
    let is_left = v.settings.edge == crate::state::SidebarEdge::Left;
    let dl = if is_left {
        sidebar_left + PANEL_WIDTH + DRAWER_GAP - (DRAWER_WIDTH + DRAWER_GAP) * (1. - prog) - 8.
    } else {
        sidebar_left - (DRAWER_WIDTH + DRAWER_GAP) * prog + 8.
    };
    let dr = Rect::from_xywh(dl, 16., DRAWER_WIDTH, (height - 32.).max(320.));

    // 限制在侧边栏外侧绘制，动画展开/收起时呈现“从侧边栏背后平滑滑出”的效果
    let clip_rect = if is_left {
        Rect::from_xywh(
            sidebar_left + PANEL_WIDTH - 8.,
            0.,
            (width - (sidebar_left + PANEL_WIDTH - 8.)).max(0.),
            height,
        )
    } else {
        Rect::from_xywh(0., 0., (sidebar_left + 8.).max(0.), height)
    };
    c.save();
    c.clip_rect(clip_rect, None, true);

    glass::panel_radius(
        c,
        dr,
        28.,
        v.dark,
        1,
        v.data.blurred_wallpaper(1).map(|img| (img, (width, height))),
    );

    // 抽屉顶眉：标题 + 操作指引 + 右上关闭圆按钮
    text_styled(
        c,
        "小组件库",
        dr.left + 20.,
        dr.top + 18.,
        160.,
        17.,
        ink(v.dark),
        Weight::BOLD,
        TextAlign::Left,
        1,
    );
    text_styled(
        c,
        if is_left {
            "按住拖到左侧任意位置，或点击添加"
        } else {
            "按住拖到右侧任意位置，或点击添加"
        },
        dr.left + 20.,
        dr.top + 41.,
        260.,
        10.5,
        muted(v.dark),
        Weight::NORMAL,
        TextAlign::Left,
        1,
    );

    let close_r = Rect::from_xywh(dr.right - 46., dr.top + 18., 28., 28.);
    let close_hov = v.hover.is_some_and(|p| close_r.contains(Point::from(p)));
    glass::liquid_circle(c, close_r.center_x(), close_r.center_y(), 14., v.dark, close_hov);
    let mut xp = Paint::default();
    xp.set_anti_alias(true);
    xp.set_style(Style::Stroke);
    xp.set_stroke_width(1.5);
    xp.set_stroke_cap(Cap::Round);
    xp.set_color(ink(v.dark));
    c.draw_line((close_r.center_x() - 4., close_r.center_y() - 4.), (close_r.center_x() + 4., close_r.center_y() + 4.), &xp);
    c.draw_line((close_r.center_x() + 4., close_r.center_y() - 4.), (close_r.center_x() - 4., close_r.center_y() + 4.), &xp);
    hits.push(HitItem::new(close_r, "收起小组件库", Action::AddDrawer));

    // 可滚动小组件库预览区
    let vp = Rect::from_xywh(
        dr.left + 14.,
        dr.top + 68.,
        DRAWER_WIDTH - 28.,
        (dr.height() - 84.).max(140.),
    );
    c.save();
    c.clip_rect(vp, None, true);
    let hit_start = hits.len();

    let (items, _) = gallery_layout();
    for (idx, ix, iy, iw, ih) in items {
        let (kind, title, wide) = GALLERY_ITEMS[idx];
        let ox = dr.left + 19. + ix;
        let oy = vp.top + 4. + iy - v.drawer_scroll;
        if oy + ih < vp.top - 10. || oy > vp.bottom + 10. {
            continue;
        }
        let item_rect = Rect::from_xywh(ox, oy, iw, ih);
        let preview_h = 146.0_f32;
        let preview_rect = Rect::from_xywh(ox, oy, iw, preview_h);
        let hovered = v.dragging_card.is_none() && v.hover.is_some_and(|p| item_rect.contains(Point::from(p)));

        // 真实按比例缩放绘制 Apple 原生组件缩略图（不注册卡片内部点击，只注册外层抽屉拖拽热区）
        let (raw_w, raw_h) = if wide {
            BlockSize::Medium.dimensions()
        } else {
            BlockSize::Small.dimensions()
        };
        let scale = (iw / raw_w).min(preview_h / raw_h);
        let draw_x = ox + (iw - raw_w * scale) * 0.5;
        let draw_y = oy + (preview_h - raw_h * scale) * 0.5;

        let preview_card = v.board.make_card(kind);
        let mut dummy_hits = Vec::new();
        c.save();
        c.translate((draw_x, draw_y));
        c.scale((scale, scale));
        widgets::card(
            c,
            Rect::from_xywh(0., 0., raw_w, raw_h),
            &preview_card,
            v,
            &mut dummy_hits,
            None,
        );
        c.restore();

        if hovered {
            let mut hrim = Paint::default();
            hrim.set_anti_alias(true);
            hrim.set_style(Style::Stroke);
            hrim.set_stroke_width(2.0);
            hrim.set_color(accent_blue(v.dark));
            c.draw_round_rect(
                Rect::from_xywh(draw_x, draw_y, raw_w * scale, raw_h * scale),
                22. * scale,
                22. * scale,
                &hrim,
            );
        }

        // 缩略图下方标题 + 尺寸小徽标
        let badge = if wide { "2×1" } else { "1×1" };
        text_styled(
            c,
            title,
            preview_rect.left + 4.,
            preview_rect.bottom + 5.,
            iw - 40.,
            11.5,
            if hovered { accent_blue(v.dark) } else { ink(v.dark) },
            Weight::SEMI_BOLD,
            TextAlign::Left,
            1,
        );
        text_styled(
            c,
            badge,
            preview_rect.right - 34.,
            preview_rect.bottom + 6.,
            30.,
            9.5,
            muted(v.dark),
            Weight::MEDIUM,
            TextAlign::Right,
            1,
        );

        hits.push(HitItem::new(
            item_rect,
            &format!("拖拽或点击添加「{title}」"),
            Action::DrawerDrag(kind.into()),
        ));
    }

    for hit in &mut hits[hit_start..] {
        if !hit.rect.intersect(vp) {
            hit.rect = Rect::new_empty();
        }
    }
    c.restore();
    c.restore();
}

/// 绘制侧边栏二次设置中心浮层（支持启用开关、展开方向、5 档热键、热键提醒与重看教程）。
fn draw_settings_panel(c: &Canvas, v: &View<'_>, hits: &mut Vec<HitItem>) {
    let pr = Rect::from_xywh(24., 102., 368., 284.);
    glass::panel_radius(c, pr, 20., v.dark, 2, None);
    let mut backing = Paint::default();
    backing.set_anti_alias(true);
    backing.set_color(Color::new(if v.dark { 0xF51A202C } else { 0xF5F7FAFE }));
    c.draw_rrect(skia_safe::RRect::new_rect_xy(pr, 20., 20.), &backing);
    // 防止穿透到底层卡片热区
    hits.push(HitItem::new(pr, "", Action::Card(0)));

    text_styled(
        c,
        "侧边栏与热键设置",
        pr.left + 16.,
        pr.top + 13.,
        210.,
        14.,
        ink(v.dark),
        Weight::BOLD,
        TextAlign::Left,
        1,
    );
    let done_r = Rect::from_xywh(pr.right - 68., pr.top + 10., 54., 24.);
    let done_hov = v.hover.is_some_and(|p| done_r.contains(Point::from(p)));
    glass::liquid_pill(c, done_r, 12., v.dark, done_hov);
    text_styled(
        c,
        "完成",
        done_r.left,
        done_r.top + 4.5,
        done_r.width(),
        11.,
        accent_blue(v.dark),
        Weight::SEMI_BOLD,
        TextAlign::Center,
        1,
    );
    hits.push(HitItem::new(done_r, "收起设置", Action::ToggleSettings));

    // 0. 是否启用侧边栏
    let en_r = Rect::from_xywh(pr.left + 16., pr.top + 44., pr.width() - 32., 34.);
    glass::liquid_pill(c, en_r, 10., v.dark, false);
    text_styled(c, "启用侧边栏热键呼出", en_r.left + 10., en_r.top + 9., 180., 11.5, ink(v.dark), Weight::SEMI_BOLD, TextAlign::Left, 1);
    let badge_r = Rect::from_xywh(en_r.right - 64., en_r.top + 5., 56., 24.);
    let mut bp = Paint::default();
    bp.set_anti_alias(true);
    bp.set_color(if v.settings.enabled { Color::from_rgb(52, 199, 89) } else { Color::from_argb(60, 120, 120, 128) });
    c.draw_round_rect(badge_r, 12., 12., &bp);
    text_styled(c, if v.settings.enabled { "已启用" } else { "已停用" }, badge_r.left, badge_r.top + 4.5, badge_r.width(), 10., Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
    hits.push(HitItem::new(en_r, "切换启用侧边栏", Action::ToggleSidebarEnabled));

    // 1. 展开位置分段选择
    text_styled(c, "展开位置", pr.left + 16., pr.top + 86., 120., 10.5, muted(v.dark), Weight::SEMI_BOLD, TextAlign::Left, 1);
    let edges = [
        (crate::state::SidebarEdge::Left, "靠左展开"),
        (crate::state::SidebarEdge::Right, "靠右展开"),
    ];
    let ew = (pr.width() - 40.) * 0.5;
    for (i, (edge, lbl)) in edges.into_iter().enumerate() {
        let er = Rect::from_xywh(pr.left + 16. + i as f32 * (ew + 8.), pr.top + 105., ew, 28.);
        let active = v.settings.edge == edge;
        let hov = v.hover.is_some_and(|p| er.contains(Point::from(p)));
        if active {
            let mut fp = Paint::default();
            fp.set_anti_alias(true);
            fp.set_color(accent_blue(v.dark));
            c.draw_round_rect(er, 9., 9., &fp);
        } else {
            glass::liquid_pill(c, er, 9., v.dark, hov);
        }
        text_styled(c, lbl, er.left, er.top + 6.5, er.width(), 11.5, if active { Color::WHITE } else { ink(v.dark) }, Weight::SEMI_BOLD, TextAlign::Center, 1);
        hits.push(HitItem::new(er, lbl, Action::SetSidebarEdge(edge)));
    }

    // 2. 呼出热键预设 5 选 1
    text_styled(c, "呼出热键", pr.left + 16., pr.top + 141., 120., 10.5, muted(v.dark), Weight::SEMI_BOLD, TextAlign::Left, 1);
    let presets = crate::state::HotkeyPreset::all();
    let row_gap = 7.0_f32;
    let w3 = (pr.width() - 32. - row_gap * 2.) / 3.;
    let w2 = (pr.width() - 32. - row_gap) * 0.5;
    for (i, preset) in presets.into_iter().enumerate() {
        let rr = if i < 3 {
            Rect::from_xywh(pr.left + 16. + i as f32 * (w3 + row_gap), pr.top + 160., w3, 26.)
        } else {
            Rect::from_xywh(pr.left + 16. + (i - 3) as f32 * (w2 + row_gap), pr.top + 191., w2, 26.)
        };
        let active = v.settings.hotkey == preset;
        let hov = v.hover.is_some_and(|p| rr.contains(Point::from(p)));
        if active {
            let mut fp = Paint::default();
            fp.set_anti_alias(true);
            fp.set_color(accent_blue(v.dark));
            c.draw_round_rect(rr, 8., 8., &fp);
        } else {
            glass::liquid_pill(c, rr, 8., v.dark, hov);
        }
        text_styled(c, preset.label(), rr.left + 4., rr.top + 6., rr.width() - 8., 10.2, if active { Color::WHITE } else { ink(v.dark) }, Weight::SEMI_BOLD, TextAlign::Center, 1);
        hits.push(HitItem::new(rr, &format!("切换热键为 {}", preset.label()), Action::SetSidebarHotkey(preset)));
    }

    // 3. 热键提醒 & 重看教程入口
    let hint_r = Rect::from_xywh(pr.left + 16., pr.top + 226., 184., 26.);
    glass::liquid_pill(c, hint_r, 8., v.dark, false);
    text_styled(
        c,
        if v.settings.show_hotkey_hint { "热键提醒：已开启" } else { "热键提醒：已隐藏" },
        hint_r.left,
        hint_r.top + 5.5,
        hint_r.width(),
        10.2,
        if v.settings.show_hotkey_hint { accent_blue(v.dark) } else { muted(v.dark) },
        Weight::SEMI_BOLD,
        TextAlign::Center,
        1,
    );
    hits.push(HitItem::new(hint_r, "切换热键提醒显示", Action::ToggleHotkeyHint));

    let tut_r = Rect::from_xywh(pr.right - 160., pr.top + 226., 144., 26.);
    glass::liquid_pill(c, tut_r, 8., v.dark, false);
    text_styled(c, "重看使用教程", tut_r.left, tut_r.top + 5.5, tut_r.width(), 10.2, ink(v.dark), Weight::MEDIUM, TextAlign::Center, 1);
    hits.push(HitItem::new(tut_r, "重看侧边栏使用教程", Action::OpenTutorial));

    let (dot_col, status_msg) = if !v.settings.enabled {
        (Color::from_rgb(142, 142, 147), "热键已随侧边栏停用（随时可重新开启）")
    } else if v.hotkey_native_ok {
        (Color::from_rgb(52, 199, 89), "全局热键已生效")
    } else {
        (Color::from_rgb(255, 149, 0), "原热键被占用，已自动启用兼容监听")
    };
    let mut dp = Paint::default();
    dp.set_anti_alias(true);
    dp.set_color(dot_col);
    c.draw_circle((pr.left + 21., pr.bottom - 14.), 3.5, &dp);
    text_styled(c, status_msg, pr.left + 30., pr.bottom - 20., pr.width() - 46., 9.8, muted(v.dark), Weight::MEDIUM, TextAlign::Left, 1);
}

pub fn draw(c: &Canvas, width: f32, height: f32, progress: f32, v: &View<'_>) -> Vec<HitItem> {
    let is_left = v.settings.edge == crate::state::SidebarEdge::Left;
    let left = if is_left {
        -PANEL_WIDTH * (1. - progress)
    } else {
        width - PANEL_WIDTH + PANEL_WIDTH * (1. - progress)
    };
    let mut hits = vec![];

    // 0. 外侧可视化小组件库抽屉（靠右模式向左滑出，靠左模式向右滑出）
    draw_widget_drawer(c, width, height, left, v, &mut hits);

    let panel_rect = Rect::from_xywh(left + 8., 16., 400., (height - 32.).max(320.));

    // 1. 整面高透透明亚克力侧边栏主面板（按侧边栏实际坐标精准切片映射背景壁纸）
    glass::panel_radius(
        c,
        panel_rect,
        28.,
        v.dark,
        1,
        v.data.blurred_wallpaper(1).map(|img| (img, (width, height))),
    );

    c.save();
    c.translate((left, 0.));
    let local_hits_start = hits.len();

    // 2. 顶部实时时间与日期眉栏
    let (_, mon, day, dow, hr, min, _) = local_datetime();
    text_styled(
        c,
        &format!("{hr:02}:{min:02}"),
        28.,
        27.,
        60.,
        12.,
        ink(v.dark),
        Weight::SEMI_BOLD,
        TextAlign::Left,
        1,
    );
    text_styled(
        c,
        &format!("{mon}月{day}日 {}", weekday_cn(dow)),
        70.,
        27.5,
        180.,
        11.,
        muted(v.dark),
        Weight::NORMAL,
        TextAlign::Left,
        1,
    );
    if v.settings.show_hotkey_hint {
        let hint_r = Rect::from_xywh(292., 25., 98., 19.);
        glass::liquid_pill(c, hint_r, 9.5, v.dark, false);
        text_styled(
            c,
            &v.settings.hotkey_label(),
            hint_r.left,
            hint_r.top + 3.,
            hint_r.width(),
            9.2,
            muted(v.dark),
            Weight::MEDIUM,
            TextAlign::Center,
            1,
        );
        hits.push(HitItem::new(hint_r, "打开热键设置", Action::ToggleSettings));
    }

    // 3. 顶部工具栏：[侧边栏设置] + 搜索胶囊 + [外观 / 编辑 / 添加组件库 / 关闭]
    let top_cy = 68.0;
    let local_view = View {
        hover: v.hover.map(|(hx, hy)| (hx - left, hy)),
        ..*v
    };
    glass_circle_btn(
        c,
        41.,
        top_cy,
        17.,
        v.settings_open,
        Action::ToggleSettings,
        "侧边栏与热键设置",
        &local_view,
        &mut hits,
        |c, fg| {
            let mut gp = Paint::default();
            gp.set_anti_alias(true);
            gp.set_style(Style::Stroke);
            gp.set_stroke_width(1.5);
            gp.set_stroke_cap(Cap::Round);
            gp.set_color(fg);
            c.draw_circle((41., top_cy), 3.4, &gp);
            for k in 0..6 {
                let ang = k as f32 * std::f32::consts::FRAC_PI_3;
                c.draw_line(
                    (41. + ang.cos() * 4.8, top_cy + ang.sin() * 4.8),
                    (41. + ang.cos() * 7.0, top_cy + ang.sin() * 7.0),
                    &gp,
                );
            }
        },
    );

    let search_r = Rect::from_xywh(66., top_cy - 17., 184., 34.);
    let search_hov = v
        .hover
        .is_some_and(|p| search_r.contains(Point::from((p.0 - left, p.1))));
    glass::liquid_pill(c, search_r, 17., v.dark, search_hov || !v.query.is_empty());

    let mut g = Paint::default();
    g.set_anti_alias(true);
    g.set_style(Style::Stroke);
    g.set_stroke_width(1.5);
    g.set_stroke_cap(Cap::Round);
    g.set_color(muted(v.dark));
    c.draw_circle((81., top_cy - 1.), 4.5, &g);
    c.draw_line((84.5, top_cy + 2.5), (87.8, top_cy + 5.8), &g);

    let search_label = if v.query.is_empty() && v.preedit.is_empty() {
        "搜索应用或算式"
    } else {
        v.query
    };
    text_styled(
        c,
        search_label,
        94.,
        top_cy - 7.,
        124.,
        12.,
        if v.query.is_empty() {
            muted(v.dark)
        } else {
            ink(v.dark)
        },
        if v.query.is_empty() {
            Weight::NORMAL
        } else {
            Weight::SEMI_BOLD
        },
        TextAlign::Left,
        1,
    );
    if v.query.is_empty() {
        glass::liquid_pill(
            c,
            Rect::from_xywh(search_r.right - 25., top_cy - 9., 17., 18.),
            5.5,
            v.dark,
            false,
        );
        text_styled(
            c,
            "/",
            search_r.right - 25.,
            top_cy - 6.,
            17.,
            10.,
            muted(v.dark),
            Weight::BOLD,
            TextAlign::Center,
            1,
        );
    }
    hits.push(HitItem::new(search_r, "搜索应用或算式", Action::Search));
    if !v.preedit.is_empty() {
        text(c, v.preedit, 94., top_cy + 18., 150., 12., ink(v.dark));
    }
    glass_circle_btn(
        c,
        270.,
        top_cy,
        15.,
        v.dark,
        Action::Theme,
        if v.dark { "切至浅色" } else { "切至深色" },
        &local_view,
        &mut hits,
        |c, fg| {
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_style(Style::Stroke);
            p.set_stroke_width(1.5);
            p.set_color(fg);
            c.draw_circle((270., top_cy), 5.2, &p);
            let mut core = Paint::default();
            core.set_anti_alias(true);
            core.set_color(fg);
            c.draw_circle((270., top_cy), 2.2, &core);
        },
    );
    glass_circle_btn(
        c,
        305.,
        top_cy,
        15.,
        v.edit,
        Action::Edit,
        if v.edit { "完成编辑" } else { "编辑区块" },
        &local_view,
        &mut hits,
        |c, fg| {
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_style(Style::Stroke);
            p.set_stroke_width(1.3);
            p.set_color(fg);
            for row in 0..2 {
                for col in 0..2 {
                    c.draw_round_rect(
                        Rect::from_xywh(
                            299.5 + col as f32 * 6.5,
                            top_cy - 5.5 + row as f32 * 6.5,
                            4.5,
                            4.5,
                        ),
                        1.2,
                        1.2,
                        &p,
                    );
                }
            }
        },
    );
    glass_circle_btn(
        c,
        340.,
        top_cy,
        15.,
        v.drawer,
        Action::AddDrawer,
        if v.drawer { "关闭小组件库" } else { "打开小组件库抽屉" },
        &local_view,
        &mut hits,
        |c, fg| {
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_style(Style::Stroke);
            p.set_stroke_width(1.7);
            p.set_stroke_cap(Cap::Round);
            p.set_color(fg);
            c.draw_line((335., top_cy), (345., top_cy), &p);
            c.draw_line((340., top_cy - 5.), (340., top_cy + 5.), &p);
        },
    );
    glass_circle_btn(
        c,
        375.,
        top_cy,
        15.,
        false,
        Action::Close,
        "收起侧边栏",
        &local_view,
        &mut hits,
        |c, fg| {
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_style(Style::Stroke);
            p.set_stroke_width(1.6);
            p.set_stroke_cap(Cap::Round);
            p.set_color(fg);
            c.draw_line((371., top_cy - 4.), (379., top_cy + 4.), &p);
            c.draw_line((379., top_cy - 4.), (371., top_cy + 4.), &p);
        },
    );

    // 4. 内容视口（Apple 标准 2 列 176px 区块流 + 拖拽实时让位插槽预览）
    let vp_top = 100.0;
    let vp_bot = (height - 28.).max(260.);
    let clip = Rect::from_xywh(16., vp_top, 384., (vp_bot - vp_top).max(120.));
    c.save();
    c.clip_rect(clip, None, true);
    let start = hits.len();
    let mut max_bottom = vp_top;

    if !v.query.is_empty() {
        glass::panel_radius(
            c,
            Rect::from_xywh(24., vp_top + 6., 368., (clip.height() - 16.).max(140.)),
            20.,
            v.dark,
            1,
            None,
        );
        if v.results.is_empty() {
            text_styled(
                c,
                "未找到匹配的应用或算式 · 试试 128*36 或 /dark",
                36.,
                vp_top + 56.,
                344.,
                12.5,
                muted(v.dark),
                Weight::NORMAL,
                TextAlign::Center,
                2,
            );
        } else {
            for (i, result) in v.results.iter().enumerate() {
                let label = match result {
                    SearchResult::Math(s) => format!("= {s}    回车复制计算结果"),
                    SearchResult::Command(s) => format!("指令  {s}"),
                    SearchResult::Application(a) => a.name.clone(),
                };
                button(
                    c,
                    Rect::from_xywh(34., vp_top + 16. + i as f32 * 50. - v.scroll, 348., 42.),
                    &label,
                    Action::Result(i),
                    &local_view,
                    &mut hits,
                );
            }
        }
    } else {
        if v.edit {
            for row in 0..5 {
                for col in 0..2 {
                    let gr = Rect::from_xywh(
                        24. + col as f32 * 192.,
                        vp_top + 6. + row as f32 * 192. - v.scroll,
                        176.,
                        176.,
                    );
                    if gr.bottom >= vp_top && gr.top <= vp_bot {
                        glass::liquid_pill(c, gr, 22., v.dark, false);
                    }
                }
            }
        }

        // 若正在从抽屉拖入卡片到侧边栏落点，构造临时插入列表实现实时让位排版
        let mut preview_cards: Vec<Card> = v.board.cards.clone();
        let placeholder_id = u64::MAX;
        let has_placeholder = if let Some((drag_kind, _, Some(ins))) = v.dragging_card {
            let mut ghost = v.board.make_card(drag_kind);
            ghost.id = placeholder_id;
            let pos = ins.min(preview_cards.len());
            preview_cards.insert(pos, ghost);
            true
        } else {
            false
        };

        for p in crate::state::pack_sidebar_blocks(&preview_cards) {
            let cy = p.y + vp_top + 6. - v.scroll;
            max_bottom = max_bottom.max(p.y + vp_top + 6. + p.h);
            if cy + p.h < vp_top - 12. || cy > vp_bot + 12. {
                continue;
            }
            let rect = Rect::from_xywh(24. + p.x, cy, p.w, p.h);
            if has_placeholder && p.id == placeholder_id {
                // 绘制极具辨识度的 Apple 蓝半透高亮落点占位框
                let mut fill = Paint::default();
                fill.set_anti_alias(true);
                fill.set_color(Color::from_argb(if v.dark { 46 } else { 32 }, 0, 122, 255));
                c.draw_round_rect(rect, 22., 22., &fill);

                let mut border = Paint::default();
                border.set_anti_alias(true);
                border.set_style(Style::Stroke);
                border.set_stroke_width(2.2);
                border.set_color(accent_blue(v.dark));
                c.draw_round_rect(rect, 22., 22., &border);

                text_styled(
                    c,
                    "＋ 松手放置在此处",
                    rect.left + 8.,
                    rect.center_y() - 9.,
                    rect.width() - 16.,
                    13.5,
                    accent_blue(v.dark),
                    Weight::BOLD,
                    TextAlign::Center,
                    1,
                );
            } else if let Some(item) = preview_cards.iter().find(|c| c.id == p.id) {
                widgets::card(c, rect, item, &local_view, &mut hits, None);
            }
        }
    }

    for hit in &mut hits[start..] {
        if !hit.rect.intersect(clip) {
            hit.rect = Rect::new_empty();
        }
    }
    c.restore();

    // 5. 右侧滚动分页指示器
    let max_scroll = (max_bottom + 14. - vp_bot).max(0.);
    if max_scroll > 4.0 && v.query.is_empty() {
        let dot_cnt = 5usize;
        let active_idx = ((v.scroll / max_scroll) * (dot_cnt - 1) as f32)
            .clamp(0., (dot_cnt - 1) as f32)
            .round() as usize;
        let dx = 403.5;
        let mid_y = (vp_top + vp_bot) * 0.5;
        let step = 9.0;
        let cap_h = (dot_cnt as f32 - 1.) * step + 14.;
        glass::liquid_pill(
            c,
            Rect::from_xywh(dx - 5., mid_y - cap_h * 0.5, 10., cap_h),
            5.,
            v.dark,
            false,
        );
        let sy = mid_y - (dot_cnt as f32 - 1.) * step * 0.5;
        for i in 0..dot_cnt {
            let cy = sy + i as f32 * step;
            let mut dp = Paint::default();
            dp.set_anti_alias(true);
            if i == active_idx {
                dp.set_color(ink(v.dark));
                c.draw_round_rect(Rect::from_xywh(dx - 1.8, cy - 4., 3.6, 8.), 1.8, 1.8, &dp);
            } else {
                dp.set_color(Color::from_argb(if v.dark { 85 } else { 90 }, 100, 116, 139));
                c.draw_circle((dx, cy), 1.6, &dp);
            }
        }
    }

    // 6. 设置浮层面板与右键菜单叠加层
    if v.settings_open {
        draw_settings_panel(c, &local_view, &mut hits);
    }
    let mut no_drawer_view = local_view;
    no_drawer_view.drawer = false;
    no_drawer_view.menu = v.menu.map(|(id, (mx, my))| (id, (mx - left, my)));
    overlays(c, &no_drawer_view, 416., height, &mut hits, None);
    c.restore();

    for h in &mut hits[local_hits_start..] {
        h.rect.offset((left, 0.));
    }

    // 6. 顶层鼠标跟随拖拽幽灵预览卡（脱离平移动画，直接按窗口坐标跟随指针）
    if let Some((drag_kind, (mx, my), _)) = v.dragging_card {
        let card = v.board.make_card(drag_kind);
        let (rw, rh) = card.size.dimensions();
        let ghost_scale = 0.72_f32;
        let gx = mx - rw * ghost_scale * 0.5;
        let gy = my - rh * ghost_scale * 0.5;

        // 抬起拖影
        let mut sh = Paint::default();
        sh.set_anti_alias(true);
        sh.set_color(Color::from_argb(if v.dark { 72 } else { 42 }, 0, 0, 0));
        c.draw_round_rect(
            Rect::from_xywh(
                gx - 4.,
                gy + 8.,
                rw * ghost_scale + 8.,
                rh * ghost_scale + 4.,
            ),
            20.,
            20.,
            &sh,
        );

        let mut dummy_hits = Vec::new();
        c.save();
        c.translate((gx, gy));
        c.scale((ghost_scale, ghost_scale));
        widgets::card(
            c,
            Rect::from_xywh(0., 0., rw, rh),
            &card,
            v,
            &mut dummy_hits,
            None,
        );
        c.restore();

        let mut rim = Paint::default();
        rim.set_anti_alias(true);
        rim.set_style(Style::Stroke);
        rim.set_stroke_width(1.6);
        rim.set_color(accent_blue(v.dark));
        c.draw_round_rect(
            Rect::from_xywh(gx, gy, rw * ghost_scale, rh * ghost_scale),
            22. * ghost_scale,
            22. * ghost_scale,
            &rim,
        );
    }

    hits
}

#[cfg(test)]
mod clock_tests {
    use super::*;

    #[test]
    fn analog_clock_sweep_angle_is_continuous_with_subseconds() {
        let a0 = sweep_second_angle(15.0);
        let a_mid = sweep_second_angle(15.5);
        let a1 = sweep_second_angle(16.0);
        assert!((a0 - 0.0).abs() < 1e-4);
        assert!(a_mid > a0 && a_mid < a1);
        assert!(((a_mid - a0) - (a1 - a_mid)).abs() < 1e-5);
        let (_, _, _, _, _, _, sec_f) = local_datetime_precise();
        assert!((0.0..60.0).contains(&sec_f));
    }
}
