//! Allegro Sidebar 右侧独立效率侧边栏（旗舰负一屏美学）：
//! 贴屏幕右侧独立置顶窗平滑滑出，展开后左侧桌面与应用 100% 正常操作；
//! 采用 5 层指数空间阴影、壁纸 5 次大半径丝绸柔雾渐变底、顶部微状态眉与空气感胶囊搜索栏，
//! 强制按 Apple 2 列标准区块网格排布（1×1 小 / 2×1 大 / 2×2 特大，与桌面完全独立、互不同步）。
use crate::{
    desktop_cards::{self, AllegroCardContext},
    draw::{self, rect, Icon, Text},
    model::{Action, Model},
    widgets::Ui,
};
use allegro::{
    stage::{pack_sidebar_blocks, ClipboardItem, StageResult, StageResultKind, StageState, SIDEBAR_COLS},
    telemetry::ProTelemetry,
    theme::{rgba, squircle_path},
};
use dotisle_domain::desktop_widgets::{Kind, Size, WIDGET_CELL, WIDGET_GAP};
use std::collections::HashMap;
use vello::{
    kurbo::{Affine, Circle, Line, Rect, RoundedRect, Stroke},
    peniko::{BlendMode, Fill, Gradient, ImageBrush, ImageData},
    Scene,
};

/// 侧边栏交互热区条目。
pub struct StageOverlayHit {
    pub rect: Rect,
    pub action: Action,
    pub label: String,
}

/// 侧边栏布局度量参数。
#[derive(Clone, Copy, Debug)]
pub struct SidebarMetrics {
    pub panel_w: f64,
    pub win_w: f64,
    pub shadow_left: f64,
    pub margin_right: f64,
    pub pad: f64,
    pub cell: f64,
    pub gap: f64,
}

/// 计算侧边栏在目标逻辑屏幕宽度下的严谨 Apple 区块尺寸。
pub fn sidebar_layout_metrics(screen_w: f64) -> SidebarMetrics {
    let base_pad = 16.0;
    let base_panel_w = base_pad * 2.0 + WIDGET_CELL * 2.0 + WIDGET_GAP; // 400.0 px
    let fit_scale = ((screen_w - 40.0) / base_panel_w).clamp(0.72, 1.0);
    let pad = (base_pad * fit_scale).round();
    let cell = (WIDGET_CELL * fit_scale).round();
    let gap = (WIDGET_GAP * fit_scale).round();
    let panel_w = pad * 2.0 + cell * 2.0 + gap;
    let shadow_left = 18.0;
    let margin_right = 12.0;
    SidebarMetrics {
        panel_w,
        win_w: panel_w + shadow_left + margin_right,
        shadow_left,
        margin_right,
        pad,
        cell,
        gap,
    }
}

/// 侧边栏单帧渲染输出（供宿主做滚轮滚动限位与右键卡片命中）。
pub struct SidebarRenderOutput {
    pub panel: Rect,
    pub viewport: Rect,
    pub max_scroll: f64,
    pub hits: Vec<StageOverlayHit>,
    pub card_slots: Vec<(String, Rect)>,
}

/// 在独立侧边栏窗口局部坐标系内渲染旗舰负一屏风格极简区块侧边栏。
#[allow(clippy::too_many_arguments)]
pub fn paint_stage_overlay(
    scene: &mut Scene,
    text: &mut Text,
    model: &Model,
    win_size: (f64, f64),
    metrics: SidebarMetrics,
    pointer: (f64, f64),
    dark: bool,
    opacity: f32,
    time: f64,
    media_position: Option<f64>,
    backdrop: Option<&ImageData>,
    stage: &StageState,
    results: &[StageResult],
    clipboard: &[ClipboardItem],
    pro: Option<&ProTelemetry>,
    quote_indices: &HashMap<String, usize>,
) -> SidebarRenderOutput {
    let op = opacity.clamp(0.05, 1.0);
    let mut scheme = dotisle_domain::monet::scheme(0x0A84FF, dark);
    if dark {
        scheme.surface = 0x161923;
        scheme.container = 0x1E2230;
        scheme.high = 0x2B3042;
        scheme.on_surface = 0xF5F7FA;
        scheme.on_surface_variant = 0xA6AEBF;
        scheme.primary = 0x0A84FF;
    } else {
        scheme.surface = 0xF5F7FC;
        scheme.container = 0xFFFFFF;
        scheme.high = 0xEBF0F8;
        scheme.on_surface = 0x1D1D1F;
        scheme.on_surface_variant = 0x525866;
        scheme.primary = 0x007AFF;
    }

    let fg = scheme.on_surface;
    let muted = scheme.on_surface_variant;
    let accent = scheme.primary;
    let pad = metrics.pad;
    let cell = metrics.cell;
    let gap = metrics.gap;
    let panel_w = metrics.panel_w;
    let slide_x = (1.0 - op as f64) * 36.0;
    let panel_x = metrics.shadow_left + slide_x;
    let panel_y = 12.0;
    let panel_h = (win_size.1 - 24.0).max(360.0);
    let panel = rect(panel_x, panel_y, panel_w, panel_h);

    paint_sidebar_shell(scene, panel, 28.0, op, dark, backdrop);

    let mut out_hits: Vec<StageOverlayHit> = Vec::new();
    let mut card_slots: Vec<(String, Rect)> = Vec::new();

    // 1. 顶部系统感状态眉（20px：实时时间 + 日期星期 + Allegro 在线微标）
    let mut header_ui = Ui::new(
        scene,
        text,
        scheme,
        pointer,
        rect(0., 0., win_size.0, win_size.1),
    );
    let now = desktop_cards::date();
    let status_y = panel.y0 + 11.0;
    let status_fg = if dark { 0xEAEFF8 } else { 0x1D1D1F };
    let status_muted = if dark { 0x9BA3B4 } else { 0x4E5564 };
    header_ui.label(
        &format!("{:02}:{:02}", now.hour, now.minute),
        panel.x0 + pad + 2.0,
        status_y,
        11.5,
        status_fg,
        true,
        46.0,
    );
    header_ui.label(
        &format!("{}月{}日 · {}", now.month, now.day, now.weekday_label()),
        panel.x0 + pad + 44.0,
        status_y + 0.5,
        10.5,
        status_muted,
        false,
        160.0,
    );
    draw::circle(header_ui.scene, panel.x1 - pad - 54.0, status_y + 7.5, 2.8, 0x30D158);
    header_ui.label(
        "Allegro",
        panel.x1 - pad - 47.0,
        status_y + 0.5,
        10.5,
        status_muted,
        true,
        48.0,
    );

    // 2. Deskset 极简空气感胶囊栏：透明亚克力微光徽标 + 微凹玻璃搜索胶囊 + [区块编辑 / + 添加 / 关闭]
    let top_y = status_y + 24.0;
    let btn_sz = 34.0;
    let btn_gap = 8.0;

    let avatar_r = rect(panel.x0 + pad, top_y, btn_sz, btn_sz);
    let badge_bg = if dark { rgba(0xFFFFFF, 0.14) } else { rgba(0xFFFFFF, 0.56) };
    header_ui.scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        badge_bg,
        None,
        &Circle::new(avatar_r.center(), btn_sz * 0.5),
    );
    header_ui.scene.stroke(
        &Stroke::new(0.95),
        Affine::IDENTITY,
        rgba(0xFFFFFF, if dark { 0.28 } else { 0.82 }),
        None,
        &Circle::new(avatar_r.center(), btn_sz * 0.5 - 0.5),
    );
    header_ui.icon(
        Icon::Island,
        avatar_r.center().x - 8.0,
        avatar_r.center().y - 8.0,
        16.0,
        accent,
    );

    let close_r = rect(panel.x1 - pad - btn_sz, top_y, btn_sz, btn_sz);
    let add_r = rect(close_r.x0 - btn_gap - btn_sz, top_y, btn_sz, btn_sz);
    let edit_r = rect(add_r.x0 - btn_gap - btn_sz, top_y, btn_sz, btn_sz);

    paint_circle_btn(
        &mut header_ui,
        edit_r,
        stage.sidebar_edit,
        edit_r.contains(pointer),
        dark,
        accent,
    );
    paint_grid_glyph(
        header_ui.scene,
        edit_r.center().into(),
        if stage.sidebar_edit { 0xFFFFFF } else { fg },
    );
    header_ui.hit(edit_r, "编辑侧边栏区块", Action::SidebarToggleEdit, true);

    paint_circle_btn(
        &mut header_ui,
        add_r,
        stage.sidebar_add_open,
        add_r.contains(pointer),
        dark,
        accent,
    );
    header_ui.icon(
        Icon::Plus,
        add_r.center().x - 8.0,
        add_r.center().y - 8.0,
        16.0,
        if stage.sidebar_add_open { 0xFFFFFF } else { fg },
    );
    header_ui.hit(add_r, "添加侧边栏组件", Action::SidebarToggleAdd, true);

    paint_circle_btn(&mut header_ui, close_r, false, close_r.contains(pointer), dark, accent);
    header_ui.icon(
        Icon::Close,
        close_r.center().x - 7.0,
        close_r.center().y - 7.0,
        14.0,
        muted,
    );
    header_ui.hit(close_r, "收起侧边栏", Action::SidebarClose, true);

    // 中间极简空气感胶囊搜索框（微凹压感内影 + 斜杠指令徽标）
    let search_x = avatar_r.x1 + 10.0;
    let search_w = (edit_r.x0 - 10.0 - search_x).max(96.0);
    let search_r = rect(search_x, top_y - 1.0, search_w, 36.0);
    let search_shape = squircle_path(search_r, 18.0);
    let search_fill = if stage.search_focused {
        rgba(if dark { 0xFFFFFF } else { 0xFFFFFF }, if dark { 0.16 } else { 0.78 })
    } else if search_r.contains(pointer) {
        rgba(0xFFFFFF, if dark { 0.14 } else { 0.60 })
    } else {
        rgba(0xFFFFFF, if dark { 0.10 } else { 0.44 })
    };
    header_ui.scene.fill(Fill::NonZero, Affine::IDENTITY, search_fill, None, &search_shape);
    // 物理微凹槽边框：上缘微暗压边 + 聚焦系统蓝光环
    let search_rim = if stage.search_focused {
        rgba(accent, 0.75)
    } else {
        rgba(if dark { 0xFFFFFF } else { 0xFFFFFF }, if dark { 0.16 } else { 0.56 })
    };
    header_ui.scene.stroke(&Stroke::new(0.9), Affine::IDENTITY, search_rim, None, &search_shape);
    paint_search_glyph(header_ui.scene, (search_r.x0 + 16.0, search_r.center().y), muted);

    let composed = if stage.preedit.is_empty() {
        stage.query.clone()
    } else {
        format!("{}{}", stage.query, stage.preedit)
    };
    let text_x = search_r.x0 + 30.0;
    let text_max = (search_r.width() - 56.0).max(40.0);
    if composed.is_empty() {
        header_ui.label("搜索", text_x, search_r.y0 + 9.5, 13.0, muted, false, text_max);
        let slash_r = rect(search_r.x1 - 24.0, search_r.y0 + 9.0, 17.0, 18.0);
        header_ui.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.10 } else { 0.06 }),
            None,
            &squircle_path(slash_r, 5.5),
        );
        header_ui.center("/", slash_r, 10.0, muted, true);
    } else {
        header_ui.label(&composed, text_x, search_r.y0 + 9.0, 13.0, fg, true, text_max);
    }
    if stage.search_focused && (time * 2.0).rem_euclid(2.0) < 1.0 {
        let tw = if composed.is_empty() {
            0.0
        } else {
            header_ui.measure_alpha(&composed, 13.0, true).min(text_max)
        };
        header_ui.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            rgba(accent, 0.95),
            None,
            &RoundedRect::from_rect(rect(text_x + tw + 2.0, search_r.y0 + 9.0, 1.8, 18.0), 1.0),
        );
    }
    header_ui.hit(search_r, "聚焦侧边栏搜索框", Action::SidebarFocusSearch, true);

    let mut content_top = search_r.y1 + 14.0;

    // 3. 若点击右上「+」或处于「编辑区块」且未在搜索：展开独立组件添加面板
    if (stage.sidebar_add_open || stage.sidebar_edit) && composed.is_empty() {
        let drawer_h = 84.0;
        let drawer_r = rect(panel.x0 + pad, content_top, panel_w - pad * 2.0, drawer_h);
        paint_sub_card(&mut header_ui, drawer_r, 18.0, false, dark, accent);
        header_ui.label(
            "添加独立组件（强制 1×1 / 2×1 / 2×2 区块，不与桌面同步）",
            drawer_r.x0 + 12.0,
            drawer_r.y0 + 7.0,
            10.0,
            muted,
            true,
            drawer_r.width() - 24.0,
        );
        let quick_items = [
            (Kind::AppFolder, Size::Small, "4", "应用 1×1"),
            (Kind::Weather, Size::Small, "", "天气 1×1"),
            (Kind::AnalogClock, Size::Small, "", "表盘 1×1"),
            (Kind::ActivityRings, Size::Small, "", "三环 1×1"),
            (Kind::SystemPro, Size::Medium, "", "探针 2×1"),
            (Kind::Note, Size::Medium, "", "便签 2×1"),
            (Kind::Quote, Size::Medium, "", "日签 2×1"),
            (Kind::Todos, Size::Large, "", "待办 2×2"),
        ];
        let chip_gap = 6.0;
        let cols = 4usize;
        let chip_w = (drawer_r.width() - 24.0 - chip_gap * (cols as f64 - 1.0)) / cols as f64;
        for (i, (kind, size, cap, lbl)) in quick_items.into_iter().enumerate() {
            let c = i % cols;
            let r = i / cols;
            let cr = rect(
                drawer_r.x0 + 12.0 + c as f64 * (chip_w + chip_gap),
                drawer_r.y0 + 26.0 + r as f64 * 26.0,
                chip_w,
                22.0,
            );
            paint_sub_card(&mut header_ui, cr, 8.0, cr.contains(pointer), dark, accent);
            header_ui.center(lbl, cr, 9.8, fg, true);
            header_ui.hit(cr, lbl, Action::SidebarAddWidget(kind, size, cap.into()), true);
        }
        content_top = drawer_r.y1 + 12.0;
    } else if stage.search_focused && composed.is_empty() && !clipboard.is_empty() {
        let cb_h = 34.0;
        let cb_r = rect(panel.x0 + pad, content_top, panel_w - pad * 2.0, cb_h);
        paint_sub_card(&mut header_ui, cb_r, 12.0, false, dark, accent);
        header_ui.label("剪贴板", cb_r.x0 + 10.0, cb_r.y0 + 10.0, 10.0, muted, true, 36.0);
        let cnt = clipboard.len().min(2);
        let avail = (cb_r.width() - 56.0).max(60.0);
        let cw = (avail - (cnt as f64 - 1.0) * 6.0) / cnt as f64;
        for (idx, item) in clipboard.iter().take(cnt).enumerate() {
            let cr = rect(cb_r.x0 + 50.0 + idx as f64 * (cw + 6.0), cb_r.y0 + 5.0, cw, 24.0);
            paint_sub_card(&mut header_ui, cr, 8.0, cr.contains(pointer), dark, accent);
            header_ui.label(&item.preview, cr.x0 + 7.0, cr.y0 + 5.0, 9.8, fg, false, (cw - 14.0).max(20.0));
            header_ui.hit(cr, "回贴剪贴板", Action::StageClipboardCopy(idx), true);
        }
        content_top = cb_r.y1 + 10.0;
    }

    out_hits.extend(collect_hits(header_ui));

    let viewport = rect(
        panel.x0 + pad,
        content_top,
        panel_w - pad * 2.0,
        (panel.y1 - 14.0 - content_top).max(120.0),
    );
    let mut max_scroll = 0.0;

    // 4. 主体区域：搜索结果 OR 强制 2 列 Apple 独立区块卡片流
    if !composed.is_empty() {
        let mut search_ui = Ui::new(
            scene,
            text,
            scheme,
            pointer,
            rect(0., 0., win_size.0, win_size.1),
        );
        paint_sub_card(&mut search_ui, viewport, 20.0, false, dark, accent);
        if results.is_empty() {
            search_ui.center(
                "未找到匹配的应用或计算式 · 试试 128*36 或 /focus 25",
                viewport,
                12.0,
                muted,
                false,
            );
        } else {
            let row_h = 46.0;
            let row_gap = 6.0;
            for (idx, item) in results.iter().take(8).enumerate() {
                let ry = viewport.y0 + 10.0 + idx as f64 * (row_h + row_gap);
                if ry + row_h > viewport.y1 - 8.0 {
                    break;
                }
                let rr = rect(viewport.x0 + 8.0, ry, viewport.width() - 16.0, row_h);
                let active = idx == stage.selected || rr.contains(pointer);
                if active {
                    search_ui.scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        rgba(accent, if dark { 0.88 } else { 0.94 }),
                        None,
                        &squircle_path(rr, 12.0),
                    );
                }
                let t_col = if active { 0xFFFFFF } else { fg };
                let s_col = if active { 0xE5F0FF } else { muted };
                let b_bg = if active {
                    rgba(0xFFFFFF, 0.22)
                } else {
                    rgba(accent, if dark { 0.20 } else { 0.12 })
                };
                let badge_r = rect(rr.x0 + 8.0, ry + 12.0, 40.0, 22.0);
                search_ui.scene.fill(Fill::NonZero, Affine::IDENTITY, b_bg, None, &squircle_path(badge_r, 7.0));
                search_ui.center(item.badge, badge_r, 9.5, if active { 0xFFFFFF } else { accent }, true);
                search_ui.label(&item.title, badge_r.x1 + 10.0, ry + 7.0, 12.5, t_col, true, (rr.width() - 110.0).max(60.0));
                search_ui.label(&item.subtitle, badge_r.x1 + 10.0, ry + 25.0, 9.5, s_col, false, (rr.width() - 110.0).max(60.0));
                if idx == stage.selected {
                    search_ui.label("↵", rr.x1 - 24.0, ry + 14.0, 12.0, 0xFFFFFF, true, 20.0);
                }
                let act = match &item.kind {
                    StageResultKind::MathCopy(val) => Action::StageCopyText(val.clone()),
                    StageResultKind::LaunchApp { target, .. } => Action::StageRunApp(target.clone()),
                    StageResultKind::RunCommand(cmd) => Action::StageSlash(cmd.clone()),
                };
                search_ui.hit(rr, &item.title, act, true);
            }
        }
        out_hits.extend(collect_hits(search_ui));
    } else {
        let (raw_slots, total_h) = pack_sidebar_blocks(
            &stage.sidebar_widgets,
            viewport.x0,
            viewport.y0,
            cell,
            gap,
        );
        max_scroll = (total_h + 12.0 - viewport.height()).max(0.0);
        let scroll = stage.sidebar_scroll.clamp(0.0, max_scroll);

        scene.push_layer(BlendMode::default(), 1.0, Affine::IDENTITY, &viewport);

        if stage.sidebar_edit {
            let vis_rows = (((viewport.height() + scroll + gap) / (cell + gap)).ceil() as usize).max(3);
            let cell_fill = if dark { rgba(0xFFFFFF, 0.05) } else { rgba(0xFFFFFF, 0.22) };
            let cell_rim = if dark { rgba(0xFFFFFF, 0.16) } else { rgba(0xFFFFFF, 0.46) };
            for r in 0..vis_rows {
                for c in 0..SIDEBAR_COLS {
                    let sx = viewport.x0 + c as f64 * (cell + gap);
                    let sy = viewport.y0 - scroll + r as f64 * (cell + gap);
                    if sy + cell < viewport.y0 || sy > viewport.y1 {
                        continue;
                    }
                    let sr = rect(sx, sy, cell, cell);
                    let sh = squircle_path(sr, 24.0);
                    scene.fill(Fill::NonZero, Affine::IDENTITY, cell_fill, None, &sh);
                    scene.stroke(&Stroke::new(0.9), Affine::IDENTITY, cell_rim, None, &sh);
                    let cx = sr.center().x;
                    let cy = sr.center().y;
                    scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, cell_rim, None, &Line::new((cx - 4., cy), (cx + 4., cy)));
                    scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, cell_rim, None, &Line::new((cx, cy - 4.), (cx, cy + 4.)));
                }
            }
        }

        if stage.sidebar_widgets.is_empty() {
            text.draw_face_fixed(
                scene,
                "Noto Sans SC",
                "侧边栏暂无独立组件，点击右上「+」添加",
                viewport.x0 + 36.0,
                viewport.center().y - 8.0,
                12.0,
                muted,
                500,
                viewport.width() - 72.0,
            );
        }

        for slot in &raw_slots {
            let Some(p) = stage.sidebar_widgets.get(slot.index) else {
                continue;
            };
            let card_y = slot.rect.y - scroll;
            let card_r = rect(slot.rect.x, card_y, slot.rect.w, slot.rect.h);
            if card_r.y1 < viewport.y0 - 8.0 || card_r.y0 > viewport.y1 + 8.0 {
                continue;
            }
            let clipped_card = card_r.intersect(viewport);
            if clipped_card.width() > 2.0 && clipped_card.height() > 2.0 {
                card_slots.push((p.id.clone(), clipped_card));
            }

            let quote_idx = quote_indices.get(&p.id).copied().unwrap_or(0);
            let ctx = AllegroCardContext {
                placement: p,
                width: card_r.width(),
                height: card_r.height(),
                pointer: (pointer.0 - card_r.x0, pointer.1 - card_r.y0),
                menu: false,
                scroll: 0.0,
                todo_edit: None,
                note_edit: None,
                time,
                media_position,
                wallpaper_crop: None,
                wallpaper_luma: None,
                dropzone_items: &[],
                photo_image: None,
                photo_meta: (0, 0),
                pro,
                stack_info: None,
                quote_index: quote_idx,
                in_sidebar: true,
            };
            let view = desktop_cards::render_allegro_card(model, text, &ctx);
            scene.append(&view.scene, Some(Affine::translate((card_r.x0, card_r.y0))));

            // 悬停柔光微交互（轻量提亮卡片边缘）
            if !stage.sidebar_edit && card_r.contains(pointer) {
                scene.stroke(
                    &Stroke::new(1.1),
                    Affine::IDENTITY,
                    rgba(if dark { 0xFFFFFF } else { accent }, if dark { 0.30 } else { 0.28 }),
                    None,
                    &squircle_path(card_r, 24.0),
                );
            }

            if !stage.sidebar_edit {
                for hit in view.hits {
                    let Some(mapped) = remap_sidebar_action(hit.action) else {
                        continue;
                    };
                    let hr = rect(
                        hit.rect.x0 + card_r.x0,
                        hit.rect.y0 + card_r.y0,
                        hit.rect.width(),
                        hit.rect.height(),
                    )
                    .intersect(viewport);
                    if hr.width() > 2.0 && hr.height() > 2.0 {
                        out_hits.push(StageOverlayHit {
                            rect: hr,
                            action: mapped,
                            label: hit.label,
                        });
                    }
                }
            } else {
                scene.stroke(
                    &Stroke::new(1.4),
                    Affine::IDENTITY,
                    rgba(accent, 0.72),
                    None,
                    &squircle_path(card_r, 24.0),
                );
                let del_c = (card_r.x0 + 12.0, card_r.y0 + 12.0);
                scene.fill(Fill::NonZero, Affine::IDENTITY, rgba(0xFF3B30, 0.96), None, &Circle::new(del_c, 10.0));
                scene.stroke(&Stroke::new(1.1), Affine::IDENTITY, rgba(0xFFFFFF, 0.92), None, &Circle::new(del_c, 10.0));
                scene.fill(Fill::NonZero, Affine::IDENTITY, rgba(0xFFFFFF, 0.98), None, &rect(del_c.0 - 4.0, del_c.1 - 1.0, 8.0, 2.0));
                let del_hit = rect(del_c.0 - 11.0, del_c.1 - 11.0, 22.0, 22.0).intersect(viewport);
                if del_hit.width() > 2.0 && del_hit.height() > 2.0 {
                    out_hits.push(StageOverlayHit {
                        rect: del_hit,
                        action: Action::SidebarRemoveWidget(p.id.clone()),
                        label: "移除侧边栏组件".into(),
                    });
                }

                // 高级深曜石磨砂胶囊尺寸切换钮
                let size_lbl = match p.size {
                    Size::Small => "1×1 小",
                    Size::Medium => "2×1 大",
                    Size::Large => "2×2 特大",
                };
                let pill_w = if p.size == Size::Large { 62.0 } else { 54.0 };
                let size_r = rect(card_r.x1 - pill_w - 8.0, card_r.y0 + 8.0, pill_w, 22.0);
                let sz_hov = size_r.contains(pointer);
                let pill_col = if sz_hov {
                    rgba(accent, 0.96)
                } else if dark {
                    rgba(0xFFFFFF, 0.22)
                } else {
                    rgba(0x1D1D1F, 0.86)
                };
                scene.fill(Fill::NonZero, Affine::IDENTITY, pill_col, None, &squircle_path(size_r, 11.0));
                text.draw_face_fixed(
                    scene,
                    "Noto Sans SC",
                    size_lbl,
                    size_r.x0 + 7.0,
                    size_r.y0 + 4.0,
                    10.0,
                    0xFFFFFF,
                    600,
                    pill_w - 10.0,
                );
                let sz_hit = size_r.intersect(viewport);
                if sz_hit.width() > 2.0 && sz_hit.height() > 2.0 {
                    out_hits.push(StageOverlayHit {
                        rect: sz_hit,
                        action: Action::SidebarCycleSize(p.id.clone()),
                        label: "切换区块尺寸".into(),
                    });
                }

                let up_r = rect(size_r.x0 - 26.0, card_r.y0 + 8.0, 22.0, 22.0);
                let up_hov = up_r.contains(pointer);
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    rgba(if dark { 0x161923 } else { 0xFFFFFF }, if up_hov { 0.98 } else { 0.90 }),
                    None,
                    &squircle_path(up_r, 11.0),
                );
                scene.stroke(&Stroke::new(0.8), Affine::IDENTITY, rgba(0x1D1D1F, 0.12), None, &squircle_path(up_r, 11.0));
                text.draw_face_fixed(
                    scene,
                    "Noto Sans SC",
                    "↑",
                    up_r.x0 + 6.5,
                    up_r.y0 + 3.0,
                    11.0,
                    if dark { 0xFFFFFF } else { 0x1D1D1F },
                    700,
                    14.0,
                );
                let up_hit = up_r.intersect(viewport);
                if up_hit.width() > 2.0 && up_hit.height() > 2.0 {
                    out_hits.push(StageOverlayHit {
                        rect: up_hit,
                        action: Action::SidebarMove(p.id.clone(), -1),
                        label: "区块前移".into(),
                    });
                }
            }
        }

        scene.pop_layer();

        // 5. Smart Stack 经典胶囊分页指示器（半透玻璃细胶囊底座 + 实心高亮指示点）
        if max_scroll > 1.0 && total_h > 1.0 {
            let dot_count = 5usize;
            let active_idx = ((scroll / max_scroll) * (dot_count - 1) as f64).round() as usize;
            let dot_x = panel.x1 - 8.0;
            let mid_y = viewport.center().y;
            let spacing = 9.0;
            let pill_h = (dot_count as f64 - 1.0) * spacing + 14.0;
            let capsule = rect(dot_x - 5.0, mid_y - pill_h * 0.5, 10.0, pill_h);
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                rgba(if dark { 0xFFFFFF } else { 0xFFFFFF }, if dark { 0.14 } else { 0.48 }),
                None,
                &RoundedRect::from_rect(capsule, 5.0),
            );
            scene.stroke(
                &Stroke::new(0.7),
                Affine::IDENTITY,
                rgba(0xFFFFFF, if dark { 0.24 } else { 0.72 }),
                None,
                &RoundedRect::from_rect(capsule, 5.0),
            );
            let start_y = mid_y - ((dot_count - 1) as f64 * spacing) * 0.5;
            for i in 0..dot_count {
                let cy = start_y + i as f64 * spacing;
                if i == active_idx {
                    let pill = rect(dot_x - 2.0, cy - 4.0, 4.0, 8.0);
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        rgba(if dark { 0xFFFFFF } else { 0x1D1D1F }, 0.82),
                        None,
                        &RoundedRect::from_rect(pill, 2.0),
                    );
                } else {
                    scene.fill(
                        Fill::NonZero,
                        Affine::IDENTITY,
                        rgba(if dark { 0xFFFFFF } else { 0x1D1D1F }, 0.26),
                        None,
                        &Circle::new((dot_x, cy), 1.6),
                    );
                }
            }
        }
    }

    // 6. 轻提示胶囊 Toast
    if let Some((msg, _)) = &stage.toast {
        let mut toast_ui = Ui::new(
            scene,
            text,
            scheme,
            pointer,
            rect(0., 0., win_size.0, win_size.1),
        );
        let tw = (toast_ui.measure_alpha(msg, 11.5, true) + 28.0).clamp(120.0, panel_w - 32.0);
        let tr = rect(panel.center().x - tw * 0.5, panel.y1 - 44.0, tw, 28.0);
        toast_ui.scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            rgba(0x161923, 0.90),
            None,
            &squircle_path(tr, 14.0),
        );
        toast_ui.center(msg, tr, 11.0, 0xFFFFFF, true);
    }

    SidebarRenderOutput {
        panel,
        viewport,
        max_scroll,
        hits: out_hits,
        card_slots,
    }
}

fn remap_sidebar_action(act: Action) -> Option<Action> {
    match act {
        Action::AppFolderLaunch(id, idx) => Some(Action::SidebarAppFolderLaunch(id, idx)),
        Action::NoteToggleCheck(id, line) => Some(Action::SidebarNoteToggleCheck(id, line)),
        Action::QuoteNext(id) => Some(Action::SidebarQuoteNext(id)),
        Action::ToggleTodo(id) => Some(Action::ToggleTodo(id)),
        Action::Timer(id, cmd) => Some(Action::Timer(id, cmd)),
        Action::QuickFocus => Some(Action::QuickFocus),
        Action::Media(cmd) => Some(Action::Media(cmd)),
        _ => None,
    }
}

fn paint_sidebar_shell(
    scene: &mut Scene,
    r: Rect,
    rad: f64,
    op: f32,
    dark: bool,
    backdrop: Option<&ImageData>,
) {
    let shape = squircle_path(r, rad);
    // 5 层柔和空间扩展左阴影，轻盈悬浮贴合屏幕边缘
    for (dx, dy, a) in [
        (-13., 10., 0.065),
        (-8., 6., 0.055),
        (-4., 3., 0.045),
        (-2., 1.5, 0.032),
        (-1., 0.8, 0.024),
    ] {
        scene.fill(
            Fill::NonZero,
            Affine::translate((dx, dy)),
            rgba(0x050810, a * op),
            None,
            &shape,
        );
    }
    scene.push_layer(BlendMode::default(), op, Affine::IDENTITY, &shape);

    if let Some(bg) = backdrop {
        let sx = r.width() / (bg.width as f64).max(1.);
        let sy = r.height() / (bg.height as f64).max(1.);
        let tr = Affine::translate((r.x0, r.y0)) * Affine::scale_non_uniform(sx, sy);
        scene.draw_image(&ImageBrush::from(bg.clone()), tr);
    } else {
        // 无壁纸切片时直接采用高通透透明亚克力渐变底（非不透明实色底板）
        let fallback_grad = if dark {
            Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([
                (0.0, rgba(0x161A26, 0.72)),
                (1.0, rgba(0x10141E, 0.76)),
            ])
        } else {
            Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([
                (0.0, rgba(0xF6F8FC, 0.68)),
                (1.0, rgba(0xEDF1F8, 0.72)),
            ])
        };
        scene.fill(Fill::NonZero, Affine::IDENTITY, &fallback_grad, None, &shape);
    }

    // 极轻左上天光漫射 + 透明亚克力柔光纱（有壁纸时保持背景充分透出）
    let sky_glow = Gradient::new_radial(
        (r.x0 + r.width() * 0.25, r.y0 + r.height() * 0.14),
        r.width() as f32 * 1.15,
    )
    .with_stops([
        (0.0, rgba(0xFFFFFF, if dark { 0.06 } else { 0.24 })),
        (1.0, rgba(0xFFFFFF, 0.0)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &sky_glow, None, &shape);

    let veil_grad = if dark {
        Gradient::new_linear((r.x0, r.y0), (r.x0, r.y1)).with_stops([
            (0.0, rgba(0x121620, if backdrop.is_some() { 0.42 } else { 0.20 })),
            (1.0, rgba(0x0D111A, if backdrop.is_some() { 0.54 } else { 0.28 })),
        ])
    } else {
        Gradient::new_linear((r.x0, r.y0), (r.x0, r.y1)).with_stops([
            (0.0, rgba(0xF7F9FD, if backdrop.is_some() { 0.26 } else { 0.14 })),
            (1.0, rgba(0xEFF3FA, if backdrop.is_some() { 0.36 } else { 0.20 })),
        ])
    };
    scene.fill(Fill::NonZero, Affine::IDENTITY, &veil_grad, None, &shape);

    scene.stroke(
        &Stroke::new(0.85),
        Affine::IDENTITY,
        rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.18 } else { 0.075 }),
        None,
        &shape,
    );
    // Smart Stack 360° 物理斜向玻璃内棱高光边
    let inner = rect(r.x0 + 1., r.y0 + 1., (r.width() - 2.).max(2.), (r.height() - 2.).max(2.));
    let rim = Gradient::new_linear((inner.x0, inner.y0), (inner.x1, inner.y1)).with_stops([
        (0.0, rgba(0xFFFFFF, if dark { 0.34 } else { 0.88 })),
        (0.42, rgba(0xFFFFFF, if dark { 0.08 } else { 0.20 })),
        (1.0, rgba(0xFFFFFF, if dark { 0.20 } else { 0.52 })),
    ]);
    scene.stroke(
        &Stroke::new(1.05),
        Affine::IDENTITY,
        &rim,
        None,
        &squircle_path(inner, (rad - 1.).max(2.)),
    );
    scene.pop_layer();
}

fn paint_circle_btn(
    ui: &mut Ui,
    r: Rect,
    active: bool,
    hovered: bool,
    dark: bool,
    accent: u32,
) {
    let bg = if active {
        rgba(accent, 0.94)
    } else if hovered {
        rgba(0xFFFFFF, if dark { 0.24 } else { 0.76 })
    } else {
        rgba(0xFFFFFF, if dark { 0.12 } else { 0.46 })
    };
    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, bg, None, &Circle::new(r.center(), r.width() * 0.5));
    let rim = if active {
        rgba(accent, 0.45)
    } else {
        rgba(0xFFFFFF, if dark { 0.18 } else { 0.56 })
    };
    ui.scene.stroke(&Stroke::new(0.85), Affine::IDENTITY, rim, None, &Circle::new(r.center(), r.width() * 0.5));
}

fn paint_search_glyph(scene: &mut Scene, center: (f64, f64), color: u32) {
    let (cx, cy) = (center.0 - 1.5, center.1 - 1.5);
    scene.stroke(
        &Stroke::new(1.5),
        Affine::IDENTITY,
        rgba(color, 0.85),
        None,
        &Circle::new((cx, cy), 4.8),
    );
    scene.stroke(
        &Stroke::new(1.5),
        Affine::IDENTITY,
        rgba(color, 0.85),
        None,
        &Line::new((cx + 3.4, cy + 3.4), (cx + 6.6, cy + 6.6)),
    );
}

fn paint_grid_glyph(scene: &mut Scene, center: (f64, f64), color: u32) {
    let stroke = Stroke::new(1.3);
    for r in 0..2 {
        for c in 0..2 {
            let x = center.0 - 6.0 + c as f64 * 7.0;
            let y = center.1 - 6.0 + r as f64 * 7.0;
            scene.stroke(
                &stroke,
                Affine::IDENTITY,
                rgba(color, 0.90),
                None,
                &RoundedRect::from_rect(rect(x, y, 5.0, 5.0), 1.4),
            );
        }
    }
}

fn paint_sub_card(ui: &mut Ui, r: Rect, rad: f64, hovered: bool, dark: bool, tint: u32) {
    let fill = if hovered {
        rgba(tint, if dark { 0.22 } else { 0.14 })
    } else if dark {
        rgba(0xFFFFFF, 0.10)
    } else {
        rgba(0xFFFFFF, 0.54)
    };
    ui.scene.fill(Fill::NonZero, Affine::IDENTITY, fill, None, &squircle_path(r, rad));
    let rim = if hovered {
        rgba(tint, 0.56)
    } else {
        rgba(0xFFFFFF, if dark { 0.18 } else { 0.62 })
    };
    ui.scene.stroke(&Stroke::new(0.85), Affine::IDENTITY, rim, None, &squircle_path(r, rad));
}

fn collect_hits(ui: Ui<'_>) -> Vec<StageOverlayHit> {
    ui.hits
        .into_iter()
        .map(|h| StageOverlayHit {
            rect: h.rect,
            action: h.action,
            label: h.label,
        })
        .collect()
}
