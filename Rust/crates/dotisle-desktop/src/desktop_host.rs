//! Allegro 单显示器全屏合批透明画布引擎：
//! 每台活跃显示器仅维护 1 个透明置底无边框窗口与 1 份 `Surface` SwapChain，
//! 配合动态 `SetWindowRgn` 精确点击穿透与全屏游戏自动休眠，实现高刷新合批渲染与极低资源占用。
use crate::{
    desktop_cards::{self, AllegroCardContext},
    draw::{rect, Text},
    gpu::Gpu,
    model::{Action, Model},
    platform,
    views::{TodoEdit, TodoField},
};
use allegro::{
    canvas::{hit_test, plan_monitor_slots, union_input_rects, CanvasSlot, MonitorDesc},
    efficiency::{
        resolve_dropped_launcher, toggle_note_checkbox, DropzoneStore, NoteEdit,
        PhotoFrameState,
    },
    snap::{
        auto_pack_grid, compute_grid, paint_edit_grid, paint_guides, snap_drag, snap_rect_to_grid,
        snap_resize, Rect2D, SnapConfig, SnapGuide, SnapPeer,
    },
    stack::{ContextSignals, StackManager},
    telemetry::ProTelemetry,
    theme::{self as allegro_theme, WidgetMaterial},
    wallpaper::AsyncWallpaperCache,
};
use dotisle_domain::desktop_widgets::Kind;
use serde_json::json;
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use vello::{
    kurbo::{Affine, Rect},
    Scene,
};
use winit::{
    dpi::{LogicalPosition, PhysicalPosition, PhysicalSize},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    platform::windows::WindowAttributesExtWindows,
    window::{CursorIcon, Window, WindowId},
};

const MIN_W: f64 = 160.;
const MIN_H: f64 = 120.;
const MAX_W: f64 = 1200.;
const MAX_H: f64 = 1200.;
const EDGE: f64 = 8.;

/// 单台显示器的合批透明画布容器。
struct MonitorCanvas {
    desc: MonitorDesc,
    window: Arc<Window>,
    surface: dotisle_render::surface::Surface,
    slots: Vec<CanvasSlot>,
    hits: Vec<HitEntry>,
    pointer: (f64, f64),
    hovered_hit: Option<usize>,
    cursor: CursorIcon,
    last_input_rects: Vec<(i32, i32, i32, i32, i32)>,
    last_allow_full: bool,
    dirty: bool,
}

#[derive(Clone, Debug)]
struct HitEntry {
    rect: Rect,
    action: Action,
    #[allow(dead_code)]
    label: String,
    enabled: bool,
    widget_id: String,
}

#[derive(Clone, Copy, Debug, Default)]
struct Edges {
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
}

impl Edges {
    fn any(self) -> bool {
        self.left || self.right || self.top || self.bottom
    }

    fn icon(self) -> CursorIcon {
        match (self.left, self.right, self.top, self.bottom) {
            (true, _, true, _) => CursorIcon::NwResize,
            (_, true, true, _) => CursorIcon::NeResize,
            (true, _, _, true) => CursorIcon::SwResize,
            (_, true, _, true) => CursorIcon::SeResize,
            (true, _, _, _) => CursorIcon::WResize,
            (_, true, _, _) => CursorIcon::EResize,
            (_, _, true, _) => CursorIcon::NResize,
            (_, _, _, true) => CursorIcon::SResize,
            _ => CursorIcon::Default,
        }
    }
}

#[derive(Clone)]
struct DragState {
    widget_id: String,
    canvas_id: WindowId,
    orig_rect: Rect2D,
    start_cursor: (f64, f64),
    preview_rect: Rect2D,
    guides: Vec<SnapGuide>,
    stack_target: Option<String>,
    moved: bool,
}

#[derive(Clone)]
struct ResizeState {
    widget_id: String,
    canvas_id: WindowId,
    edges: Edges,
    orig_rect: Rect2D,
    start_cursor: (f64, f64),
    preview_rect: Rect2D,
    guides: Vec<SnapGuide>,
}

/// 桌面小组件全屏合批宿主的全局状态机。
pub struct Host {
    canvases: Vec<MonitorCanvas>,
    wallpaper: AsyncWallpaperCache,
    hotkeys: platform::HotkeyListener,
    stacks: StackManager,
    dropzone: DropzoneStore,
    photos: PhotoFrameState,
    telemetry: ProTelemetry,
    edit_mode: bool,
    menu_for: Option<(String, (f64, f64))>,
    scroll_for: HashMap<String, f64>,
    quote_indices: HashMap<String, usize>,
    todo_edit: Option<(String, TodoEdit)>,
    note_edit: Option<NoteEdit>,
    pressed: Option<(WindowId, String, Action)>,
    drag: Option<DragState>,
    resize: Option<ResizeState>,
    last_second: Instant,
    now_sec: f64,
    pub error: Option<String>,
}

impl Default for Host {
    fn default() -> Self {
        Self::new()
    }
}

impl Host {
    pub fn new() -> Self {
        Self {
            canvases: Vec::new(),
            wallpaper: AsyncWallpaperCache::new(crate::wake),
            hotkeys: platform::HotkeyListener::start(),
            stacks: StackManager::new(),
            dropzone: DropzoneStore::default(),
            photos: PhotoFrameState::with_wake(crate::wake),
            telemetry: ProTelemetry::with_wake(crate::wake),
            edit_mode: false,
            menu_for: None,
            scroll_for: HashMap::new(),
            quote_indices: HashMap::new(),
            todo_edit: None,
            note_edit: None,
            pressed: None,
            drag: None,
            resize: None,
            last_second: Instant::now() - Duration::from_secs(2),
            now_sec: 0.0,
            error: None,
        }
    }
    /// 同步最新状态、管理每显示器画布生命周期、采集探针与处理全屏游戏休眠。
    pub fn sync(
        &mut self,
        event: &ActiveEventLoop,
        model: &mut Model,
        gpu: &mut Gpu,
        changed: bool,
        moving: bool,
        dt: f64,
        now_sec: f64,
    ) -> bool {
        self.now_sec = now_sec;
        let [edit_presses] = self.hotkeys.take();
        if edit_presses % 2 != 0 {
            self.edit_mode = !self.edit_mode;
            self.menu_for = None;
            for c in &self.canvases {
                platform::set_dashboard_layer(&c.window, self.edit_mode);
                c.window.request_redraw();
            }
        }

        let placements = &model.desktop_widgets.items;
        self.stacks.sync_from_placements(placements);
        self.dropzone.sync_from_placements(placements);
        let photos_changed = self.photos.poll();
        if changed || photos_changed {
            self.photos.retain(|id| placements.iter().any(|p| p.id == id && p.kind == Kind::PhotoFrame));
        }
        for p in placements {
            if p.kind == Kind::PhotoFrame {
                self.photos.sync_folder(&p.id, &p.config.folder);
            }
        }

        let wallpaper_changed = self.wallpaper.poll(now_sec);
        if changed || wallpaper_changed {
            self.wallpaper.retain_cards(|id| placements.iter().any(|p| p.id == id));
        }
        let cpu_now = model.native_state["systemMetrics"]["cpuPercent"].as_f64();
        let telemetry_changed = self.telemetry.poll_updates(cpu_now);

        let signals = ContextSignals {
            music_playing: model.playing(),
            focus_running: model.timers.items.iter().any(|t| t.running),
            upcoming_schedule_mins: None,
            dropzone_count: self.dropzone.total_count(),
        };
        let stack_auto_changed = self.stacks.auto_rotate(now_sec, signals, placements);

        let has_fast_clock = placements.iter().any(|p| {
            !p.hidden
                && (matches!(p.kind, Kind::AnalogClock | Kind::SystemPro)
                    || (p.kind == Kind::Music && model.playing())
                    || (p.kind == Kind::Focus && model.timers.items.iter().any(|t| t.running)))
        });
        let tick_interval = if has_fast_clock {
            Duration::from_secs(1)
        } else {
            Duration::from_secs(15)
        };
        let second_tick = self.last_second.elapsed() >= tick_interval;
        if second_tick {
            self.last_second = Instant::now();
        }

        let descs = collect_monitors(event);

        // 无任何桌面小组件时释放桌面全屏画布，做到零占用
        if placements.is_empty() {
            self.canvases.clear();
        }

        // 移除已不存在或已无任何可见/隐藏槽位的桌面画布
        let mut planned: Vec<(MonitorDesc, Vec<CanvasSlot>)> = Vec::new();
        for desc in descs {
            planned.push((desc, Vec::new()));
        }
        let all_descs: Vec<MonitorDesc> = planned.iter().map(|(d, _)| d.clone()).collect();
        for (desc, slots) in &mut planned {
            *slots = plan_monitor_slots(desc, &all_descs, placements, &self.stacks);
        }
        planned.retain(|(_, slots)| !slots.is_empty());

        self.canvases
            .retain(|c| planned.iter().any(|(d, _)| d.key == c.desc.key));

        for (desc, slots) in planned {
            if let Some(c) = self.canvases.iter_mut().find(|c| c.desc.key == desc.key) {
                if c.desc != desc {
                    c.desc = desc.clone();
                    let _ = c
                        .window
                        .request_inner_size(PhysicalSize::new(desc.phys_w, desc.phys_h));
                    c.window
                        .set_outer_position(PhysicalPosition::new(desc.phys_x, desc.phys_y));
                    c.surface.resize(desc.phys_w.max(1), desc.phys_h.max(1));
                    c.dirty = true;
                }
                if c.slots != slots {
                    c.slots = slots;
                    c.dirty = true;
                }
            } else if let Some(mut created) = create_canvas(event, gpu, desc, slots, self.edit_mode)
            {
                created.dirty = true;
                self.canvases.push(created);
            } else {
                eprintln!("Allegro create_canvas returned None");
                self.error = Some("无法创建 Allegro 全屏透明画布".into());
            }
        }

        self.refresh_input_regions();

        let editing = self.todo_edit.is_some() || self.note_edit.is_some();
        let sleeping = !self.canvases.is_empty()
            && !changed
            && !self.edit_mode
            && !editing
            && self.drag.is_none()
            && self.resize.is_none()
            && self.telemetry.sleeping();
        let force_redraw = changed || stack_auto_changed || second_tick || moving || editing
            || wallpaper_changed || photos_changed || (has_fast_clock && telemetry_changed) || edit_presses % 2 != 0;

        for c in &mut self.canvases {
            if c.dirty || (!sleeping && force_redraw) {
                c.dirty = false;
                c.window.request_redraw();
            }
        }

        moving && !sleeping
    }

    /// 计算 Apple 玻璃右键浮层在画布内的防越界逻辑矩形。
    fn clamped_menu_rect(mon_w: f64, mon_h: f64, anchor: (f64, f64)) -> Rect2D {
        let mw = desktop_cards::CONTEXT_MENU_W;
        let mh = desktop_cards::CONTEXT_MENU_H;
        let x = anchor.0.clamp(12.0, (mon_w - mw - 12.0).max(12.0));
        let y = anchor.1.clamp(12.0, (mon_h - mh - 12.0).max(12.0));
        Rect2D::new(x, y, mw, mh)
    }

    /// 同步所有桌面画布上的精确 Win32 `SetWindowRgn` 穿透选区。
    fn refresh_input_regions(&mut self) {
        let allow_full = self.edit_mode || self.drag.is_some() || self.resize.is_some();
        for c in &mut self.canvases {
            let scale = c.desc.scale.max(0.5);
            let mut rects = union_input_rects(
                &c.slots,
                false,
                c.desc.phys_w,
                c.desc.phys_h,
                scale,
            );
            if let Some((wid, anchor)) = &self.menu_for {
                if c.slots.iter().any(|s| &s.id == wid) {
                    let mr = Self::clamped_menu_rect(
                        c.desc.phys_w as f64 / scale,
                        c.desc.phys_h as f64 / scale,
                        *anchor,
                    );
                    rects.push((
                        (mr.x * scale).floor() as i32 - 6,
                        (mr.y * scale).floor() as i32 - 6,
                        (mr.right() * scale).ceil() as i32 + 6,
                        (mr.bottom() * scale).ceil() as i32 + 12,
                        (14.0 * scale).round() as i32,
                    ));
                }
            }
            if c.last_allow_full != allow_full || c.last_input_rects != rects {
                platform::apply_input_region(&c.window, &rects, allow_full);
                c.last_allow_full = allow_full;
                c.last_input_rects = rects;
            }
        }
    }

    /// 处理桌面全屏画布的窗口事件。
    pub fn event(
        &mut self,
        id: WindowId,
        event: WindowEvent,
        model: &mut Model,
        gpu: &mut Gpu,
        text: &mut Text,
        time: f64,
        media_position: Option<f64>,
    ) -> Option<Action> {
        let idx = self.canvases.iter().position(|c| c.window.id() == id)?;

        match event {
            WindowEvent::Resized(new_size) => {
                {
                    let c = &mut self.canvases[idx];
                    c.desc.phys_w = new_size.width.max(1);
                    c.desc.phys_h = new_size.height.max(1);
                    c.surface.resize(c.desc.phys_w, c.desc.phys_h);
                    c.dirty = true;
                }
                self.refresh_input_regions();
                self.canvases[idx].window.request_redraw();
                None
            }
            WindowEvent::ScaleFactorChanged { scale_factor, .. } => {
                {
                    let c = &mut self.canvases[idx];
                    c.desc.scale = scale_factor;
                    c.dirty = true;
                }
                self.refresh_input_regions();
                self.canvases[idx].window.request_redraw();
                None
            }
            WindowEvent::RedrawRequested => {
                self.redraw_canvas(idx, model, gpu, text, time, media_position);
                None
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = self.canvases[idx].desc.scale.max(0.5);
                let pt = position.to_logical::<f64>(scale);
                self.canvases[idx].pointer = (pt.x, pt.y);
                self.handle_cursor_move(idx, (pt.x, pt.y), model);
                None
            }
            WindowEvent::MouseInput { state, button, .. } => {
                self.handle_mouse(idx, id, state, button, model)
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_wheel(idx, delta, model);
                None
            }
            WindowEvent::DroppedFile(path) => self.handle_drop(idx, &path, model),
            WindowEvent::KeyboardInput { event, .. } => {
                if event.state == ElementState::Pressed {
                    return self.handle_key(event.logical_key, model);
                }
                None
            }
            WindowEvent::Ime(ime) => {
                self.handle_ime(ime, model);
                None
            }
            _ => None,
        }
    }

    fn slot_draw_rect(&self, canvas_id: WindowId, slot: &CanvasSlot) -> Rect2D {
        if let Some(d) = &self.drag {
            if d.canvas_id == canvas_id && d.widget_id == slot.id {
                return d.preview_rect;
            }
        }
        if let Some(r) = &self.resize {
            if r.canvas_id == canvas_id && r.widget_id == slot.id {
                return r.preview_rect;
            }
        }
        slot.rect
    }

    fn redraw_canvas(
        &mut self,
        idx: usize,
        model: &Model,
        gpu: &mut Gpu,
        text: &mut Text,
        time: f64,
        media_position: Option<f64>,
    ) {
        let canvas_id = self.canvases[idx].window.id();
        let scale = self.canvases[idx].desc.scale.max(0.5);
        let mon_key = self.canvases[idx].desc.key.clone();
        let screen_log = (
            self.canvases[idx].desc.phys_w as f64 / scale,
            self.canvases[idx].desc.phys_h as f64 / scale,
        );
        let pointer = self.canvases[idx].pointer;
        let slots = self.canvases[idx].slots.clone();

        let mut scene = Scene::new();
        let mut next_hits = Vec::new();

        // 全屏网格编辑模式：铺满可排布的桌面网格槽位与拖放高亮槽
        if self.edit_mode {
            use vello::peniko::Fill;
            let veil_a = if model.prefs.dark { 0.34 } else { 0.14 };
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                allegro_theme::rgba(if model.prefs.dark { 0x050810 } else { 0x0F172A }, veil_a),
                None,
                &rect(0., 0., screen_log.0, screen_log.1),
            );
            let grid = compute_grid(screen_log.0, screen_log.1);
            let hl = self.drag.as_ref().and_then(|d| {
                (d.canvas_id == canvas_id && d.stack_target.is_none()).then_some(d.preview_rect)
            });
            paint_edit_grid(&mut scene, &grid, hl, model.prefs.dark);
        }

        // 为保证被拖动或调整大小的卡片始终绘制在最上方，先排普通组件再排焦点组件。
        let active_id = self
            .drag
            .as_ref()
            .map(|d| d.widget_id.as_str())
            .or_else(|| self.resize.as_ref().map(|r| r.widget_id.as_str()));

        let mut ordered_indices: Vec<usize> = (0..slots.len()).collect();
        ordered_indices.sort_by_key(|&i| usize::from(Some(slots[i].id.as_str()) == active_id));

        for si in ordered_indices {
            let slot = &slots[si];
            let Some(placement) = model.desktop_widgets.items.get(slot.placement_index) else {
                continue;
            };
            let draw_r = self.slot_draw_rect(canvas_id, slot);
            let _mat = WidgetMaterial::from_str_opt(&placement.material);
            let is_moving_card = Some(placement.id.as_str()) == active_id;
            // 桌面全卡片统一采样透明亚克力模糊壁纸切片
            let sample = if !placement.hidden {
                self.wallpaper.acrylic(&placement.id, &mon_key, screen_log, draw_r, is_moving_card)
            } else {
                None
            };
            let todo_ref = self
                .todo_edit
                .as_ref()
                .filter(|(wid, _)| wid == &placement.id)
                .map(|(_, ed)| ed);
            let note_ref = self
                .note_edit
                .as_ref()
                .filter(|ed| ed.widget_id == placement.id);
            let photo_meta = self.photos.photo_count(&placement.id);
            let photo_img = if placement.kind == Kind::PhotoFrame {
                self.photos.current_image(&placement.id)
            } else {
                None
            };
            let drop_items = self.dropzone.items(&placement.id);
            let scroll = self.scroll_for.get(&placement.id).copied().unwrap_or(0.0);
            let quote_idx = self.quote_indices.get(&placement.id).copied().unwrap_or(0);

            let ctx = AllegroCardContext {
                placement,
                width: draw_r.w,
                height: draw_r.h,
                pointer: (pointer.0 - draw_r.x, pointer.1 - draw_r.y),
                menu: false,
                scroll,
                todo_edit: todo_ref,
                note_edit: note_ref,
                time,
                media_position,
                wallpaper_crop: sample.as_ref().map(|s| &s.image),
                wallpaper_luma: sample.as_ref().map(|s| s.avg_luma),
                dropzone_items: drop_items,
                photo_image: photo_img,
                photo_meta,
                pro: Some(&self.telemetry),
                stack_info: slot.stack_info,
                quote_index: quote_idx,
            };
            let view = desktop_cards::render_allegro_card(model, text, &ctx);
            scene.append(&view.scene, Some(Affine::translate((draw_r.x, draw_r.y))));
            for hit in view.hits {
                next_hits.push(HitEntry {
                    rect: rect(
                        hit.rect.x0 + draw_r.x,
                        hit.rect.y0 + draw_r.y,
                        hit.rect.width(),
                        hit.rect.height(),
                    ),
                    action: hit.action,
                    label: hit.label,
                    enabled: hit.enabled,
                    widget_id: placement.id.clone(),
                });
            }

            // 编辑模式下绘制卡片聚焦外框及 Apple 红色「−」删除角标
            if self.edit_mode && !placement.hidden {
                use vello::kurbo::{Circle, Stroke};
                use vello::peniko::Fill;
                let border_r = rect(draw_r.x, draw_r.y, draw_r.w, draw_r.h);
                scene.stroke(
                    &Stroke::new(1.5),
                    Affine::IDENTITY,
                    allegro_theme::rgba(0x0A84FF, 0.72),
                    None,
                    &allegro_theme::squircle_path(border_r, 24.0),
                );
                let badge_c = (draw_r.x + 2.0, draw_r.y + 2.0);
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    allegro_theme::rgba(0xFF3B30, 0.96),
                    None,
                    &Circle::new(badge_c, 11.0),
                );
                scene.stroke(
                    &Stroke::new(1.2),
                    Affine::IDENTITY,
                    allegro_theme::rgba(0xFFFFFF, 0.90),
                    None,
                    &Circle::new(badge_c, 11.0),
                );
                scene.fill(
                    Fill::NonZero,
                    Affine::IDENTITY,
                    allegro_theme::rgba(0xFFFFFF, 0.98),
                    None,
                    &rect(badge_c.0 - 4.5, badge_c.1 - 1.1, 9.0, 2.2),
                );
                next_hits.push(HitEntry {
                    rect: rect(badge_c.0 - 12.0, badge_c.1 - 12.0, 24.0, 24.0),
                    action: Action::WidgetRemove(placement.id.clone()),
                    label: "删除小组件".into(),
                    enabled: true,
                    widget_id: placement.id.clone(),
                });
            }
        }

        // 编辑模式顶部悬浮 Apple 玻璃工具胶囊（一键自动对齐 / 完成）
        if self.edit_mode {
            use vello::kurbo::Stroke;
            use vello::peniko::Fill;
            let bar_w = 438.0_f64.min((screen_log.0 - 32.0).max(240.0));
            let bar_h = 38.0;
            let bar_x = (screen_log.0 - bar_w) * 0.5;
            let bar_y = 14.0;
            let bar_r = rect(bar_x, bar_y, bar_w, bar_h);
            let bar_shape = allegro_theme::squircle_path(bar_r, 19.0);
            scene.fill(
                Fill::NonZero,
                Affine::translate((0., 4.)),
                allegro_theme::rgba(0x090D16, 0.22),
                None,
                &bar_shape,
            );
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                allegro_theme::rgba(if model.prefs.dark { 0x1C1E26 } else { 0xFCFCFE }, 0.92),
                None,
                &bar_shape,
            );
            scene.stroke(
                &Stroke::new(0.9),
                Affine::IDENTITY,
                allegro_theme::rgba(if model.prefs.dark { 0xFFFFFF } else { 0x0F172A }, 0.14),
                None,
                &bar_shape,
            );
            text.draw_face_fixed(
                &mut scene,
                "Noto Sans SC",
                "网格编辑 · 拖动对齐格子（Alt+E / Esc 退出）",
                bar_x + 16.,
                bar_y + 11.,
                11.5,
                if model.prefs.dark { 0xF5F7FA } else { 0x1D1D1F },
                600,
                (bar_w - 176.).max(80.),
            );
            let align_r = rect(bar_r.x1 - 156., bar_y + 6., 78., 26.);
            let done_r = rect(bar_r.x1 - 72., bar_y + 6., 64., 26.);
            let align_hov = align_r.contains(pointer);
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                allegro_theme::rgba(0x0A84FF, if align_hov { 0.22 } else { 0.12 }),
                None,
                &allegro_theme::squircle_path(align_r, 13.0),
            );
            text.draw_face_fixed(
                &mut scene,
                "Noto Sans SC",
                "自动对齐",
                align_r.x0 + 14.,
                align_r.y0 + 5.5,
                11.5,
                0x0A84FF,
                600,
                60.,
            );
            let done_hov = done_r.contains(pointer);
            scene.fill(
                Fill::NonZero,
                Affine::IDENTITY,
                allegro_theme::rgba(0x0A84FF, if done_hov { 1.0 } else { 0.92 }),
                None,
                &allegro_theme::squircle_path(done_r, 13.0),
            );
            text.draw_face_fixed(
                &mut scene,
                "Noto Sans SC",
                "完成",
                done_r.x0 + 19.,
                done_r.y0 + 5.5,
                11.5,
                0xFFFFFF,
                600,
                40.,
            );
            next_hits.push(HitEntry {
                rect: align_r,
                action: Action::EditModeAutoAlign,
                label: "自动对齐网格".into(),
                enabled: true,
                widget_id: String::new(),
            });
            next_hits.push(HitEntry {
                rect: done_r,
                action: Action::EditModeToggle,
                label: "完成网格编辑".into(),
                enabled: true,
                widget_id: String::new(),
            });
        }

        // 在全屏场景最顶层绘制跨组件磁吸辅助线和智能叠放融合边框。
        if let Some(drag) = &self.drag {
            if drag.canvas_id == canvas_id {
                let target_rect = drag.stack_target.as_ref().and_then(|tid| {
                    slots.iter().find(|s| &s.id == tid).map(|s| s.rect)
                });
                paint_guides(&mut scene, &drag.guides, target_rect);
            }
        } else if let Some(rs) = &self.resize {
            if rs.canvas_id == canvas_id {
                paint_guides(&mut scene, &rs.guides, None);
            }
        }

        // 在画布最顶层弹出 Apple 风格玻璃右键菜单（可自由跨越卡片边缘且不挤占内容）
        if let Some((wid, anchor)) = &self.menu_for {
            if let Some(slot) = slots.iter().find(|s| &s.id == wid) {
                if let Some(placement) = model.desktop_widgets.items.get(slot.placement_index) {
                    let mr = Self::clamped_menu_rect(screen_log.0, screen_log.1, *anchor);
                    let scheme = dotisle_domain::monet::scheme(0x0A84FF, model.prefs.dark);
                    let mut menu_ui = crate::widgets::Ui::new(
                        &mut scene,
                        text,
                        scheme,
                        pointer,
                        rect(0., 0., screen_log.0, screen_log.1),
                    );
                    let _ = desktop_cards::paint_apple_context_menu(
                        &mut menu_ui,
                        placement,
                        (mr.x, mr.y),
                        model.prefs.dark,
                    );
                    for hit in menu_ui.hits {
                        next_hits.push(HitEntry {
                            rect: hit.rect,
                            action: hit.action,
                            label: hit.label,
                            enabled: hit.enabled,
                            widget_id: placement.id.clone(),
                        });
                    }
                }
            }
        }

        let c = &mut self.canvases[idx];
        c.hits = next_hits;
        if let Err(err) = gpu.draw(&mut c.surface, &scene, scale) {
            eprintln!("Allegro gpu.draw failed: {err}");
            self.error = Some(err);
        }
    }

    fn collect_peers(slots: &[CanvasSlot]) -> Vec<SnapPeer> {
        slots
            .iter()
            .map(|s| SnapPeer {
                id: s.id.clone(),
                rect: s.rect,
                hidden: s.hidden,
            })
            .collect()
    }

    fn handle_cursor_move(&mut self, idx: usize, pt: (f64, f64), _model: &Model) {
        let canvas_id = self.canvases[idx].window.id();
        let scale = self.canvases[idx].desc.scale.max(0.5);
        let work_area = Rect2D::new(
            0.0,
            0.0,
            self.canvases[idx].desc.phys_w as f64 / scale,
            self.canvases[idx].desc.phys_h as f64 / scale,
        );
        let peers = Self::collect_peers(&self.canvases[idx].slots);

        if let Some(drag) = &mut self.drag {
            if drag.canvas_id == canvas_id {
                let dx = pt.0 - drag.start_cursor.0;
                let dy = pt.1 - drag.start_cursor.1;
                if dx.abs() > 3.0 || dy.abs() > 3.0 {
                    drag.moved = true;
                }
                let raw = Rect2D::new(
                    drag.orig_rect.x + dx,
                    drag.orig_rect.y + dy,
                    drag.orig_rect.w,
                    drag.orig_rect.h,
                );
                if self.edit_mode {
                    let grid = compute_grid(work_area.w, work_area.h);
                    drag.preview_rect = snap_rect_to_grid(raw, &grid);
                    drag.guides.clear();
                    drag.stack_target = None;
                } else {
                    let snapped = snap_drag(
                        &drag.widget_id,
                        raw,
                        work_area,
                        &peers,
                        SnapConfig::default(),
                    );
                    drag.preview_rect = snapped.rect;
                    drag.guides = snapped.guides;
                    drag.stack_target = snapped.stack_target;
                }
                self.canvases[idx].window.request_redraw();
                return;
            }
        }

        if let Some(rs) = &mut self.resize {
            if rs.canvas_id == canvas_id {
                let dx = pt.0 - rs.start_cursor.0;
                let dy = pt.1 - rs.start_cursor.1;
                let mut x = rs.orig_rect.x;
                let mut y = rs.orig_rect.y;
                let mut w = rs.orig_rect.w;
                let mut h = rs.orig_rect.h;
                if rs.edges.right {
                    w = (rs.orig_rect.w + dx).clamp(MIN_W, MAX_W);
                } else if rs.edges.left {
                    let nw = (rs.orig_rect.w - dx).clamp(MIN_W, MAX_W);
                    x = rs.orig_rect.right() - nw;
                    w = nw;
                }
                if rs.edges.bottom {
                    h = (rs.orig_rect.h + dy).clamp(MIN_H, MAX_H);
                } else if rs.edges.top {
                    let nh = (rs.orig_rect.h - dy).clamp(MIN_H, MAX_H);
                    y = rs.orig_rect.bottom() - nh;
                    h = nh;
                }
                let raw = Rect2D::new(x, y, w, h);
                let (snapped_rect, guides) = snap_resize(
                    &rs.widget_id,
                    rs.edges.left,
                    rs.edges.right,
                    rs.edges.top,
                    rs.edges.bottom,
                    raw,
                    &peers,
                    SnapConfig::default(),
                );
                rs.preview_rect = snapped_rect;
                rs.guides = guides;
                self.canvases[idx].window.request_redraw();
                return;
            }
        }

        let hover_slot = self.canvases[idx]
            .slots
            .iter()
            .rposition(|s| !s.hidden && s.rect.contains(pt.0, pt.1))
            .map(|i| self.canvases[idx].slots[i].rect);
        let cursor = hover_slot
            .map(|r| detect_edges(r, pt).icon())
            .unwrap_or(CursorIcon::Default);
        let next_hit = self.canvases[idx]
            .hits
            .iter()
            .rposition(|h| h.enabled && h.rect.contains(pt));
        let c = &mut self.canvases[idx];
        let hit_changed = c.hovered_hit != next_hit;
        let cursor_changed = c.cursor != cursor;
        c.hovered_hit = next_hit;
        if cursor_changed {
            c.cursor = cursor;
            c.window.set_cursor(cursor);
        }
        // 非拖拽时仅在右键菜单打开、网格编辑模式或悬停切换时重绘，消除无意义重绘开销。
        if self.menu_for.is_some()
            || self.edit_mode
            || hit_changed
            || cursor_changed
        {
            c.window.request_redraw();
        }
    }

    fn handle_mouse(
        &mut self,
        idx: usize,
        canvas_id: WindowId,
        state: ElementState,
        button: MouseButton,
        model: &mut Model,
    ) -> Option<Action> {
        let pt = self.canvases[idx].pointer;

        if button == MouseButton::Right {
            if state == ElementState::Released {
                if let Some(si) = hit_test(&self.canvases[idx].slots, pt) {
                    let wid = self.canvases[idx].slots[si].id.clone();
                    let hidden = self.canvases[idx].slots[si].hidden;
                    if !hidden {
                        self.menu_for = Some((wid, pt));
                        self.refresh_input_regions();
                        self.canvases[idx].window.request_redraw();
                    }
                } else if self.menu_for.take().is_some() {
                    self.refresh_input_regions();
                    self.canvases[idx].window.request_redraw();
                }
            }
            return None;
        }

        if button != MouseButton::Left {
            return None;
        }

        if state == ElementState::Pressed {
            // 若当前有右键菜单打开，点击菜单区域外直接收起菜单并拦截透传。
            if let Some((ref m_wid, m_anchor)) = self.menu_for {
                if self.canvases[idx].slots.iter().any(|s| &s.id == m_wid) {
                    let scale = self.canvases[idx].desc.scale.max(0.5);
                    let mr = Self::clamped_menu_rect(
                        self.canvases[idx].desc.phys_w as f64 / scale,
                        self.canvases[idx].desc.phys_h as f64 / scale,
                        m_anchor,
                    );
                    if !mr.contains(pt.0, pt.1) {
                        self.menu_for = None;
                        self.refresh_input_regions();
                        self.canvases[idx].window.request_redraw();
                        return None;
                    }
                }
            }

            platform::capture_pointer(&self.canvases[idx].window, true);

            // 1. 优先命中逆序交互元素（按钮 / 复选框 / 输入框 / 叠放页码箭头）。
            let hit_action = self.canvases[idx]
                .hits
                .iter()
                .rev()
                .find(|h| h.enabled && h.rect.contains(pt))
                .map(|h| (h.widget_id.clone(), h.action.clone()));

            // 2. 检测卡片边缘调整尺寸（如果不在明确的子按钮上）。
            if let Some(si) = self.canvases[idx]
                .slots
                .iter()
                .rposition(|s| !s.hidden && s.rect.contains(pt.0, pt.1))
            {
                let slot = self.canvases[idx].slots[si].clone();
                let edges = detect_edges(slot.rect, pt);
                let is_card_body_action = matches!(
                    hit_action.as_ref().map(|(_, a)| a),
                    None | Some(Action::WidgetConfigure(_))
                );
                if edges.any() && is_card_body_action && self.menu_for.is_none() {
                    self.resize = Some(ResizeState {
                        widget_id: slot.id,
                        canvas_id,
                        edges,
                        orig_rect: slot.rect,
                        start_cursor: pt,
                        preview_rect: slot.rect,
                        guides: Vec::new(),
                    });
                    self.refresh_input_regions();
                    return None;
                }

                // 3. 记录可能转为拖拽或触发点击。
                self.drag = Some(DragState {
                    widget_id: slot.id.clone(),
                    canvas_id,
                    orig_rect: slot.rect,
                    start_cursor: pt,
                    preview_rect: slot.rect,
                    guides: Vec::new(),
                    stack_target: None,
                    moved: false,
                });
                if let Some((wid, act)) = hit_action {
                    self.pressed = Some((canvas_id, wid, act));
                }
                self.refresh_input_regions();
                return None;
            }

            if let Some((wid, act)) = hit_action {
                self.pressed = Some((canvas_id, wid, act));
            }
            return None;
        }

        // Left mouse button Released
        platform::capture_pointer(&self.canvases[idx].window, false);
        let pressed = self.pressed.take();
        let drag = self.drag.take();
        let resize = self.resize.take();

        if let Some(rs) = resize {
            let scale = self.canvases[idx].desc.scale.max(0.5);
            let mon_x = self.canvases[idx].desc.phys_x;
            let mon_y = self.canvases[idx].desc.phys_y;
            let nx = mon_x + (rs.preview_rect.x * scale).round() as i32;
            let ny = mon_y + (rs.preview_rect.y * scale).round() as i32;
            model.native_command(
                "widgetUpdate",
                json!({
                    "id": rs.widget_id,
                    "x": nx,
                    "y": ny,
                    "width": rs.preview_rect.w.round(),
                    "height": rs.preview_rect.h.round(),
                }),
            );
            self.refresh_input_regions();
            self.canvases[idx].window.request_redraw();
            return None;
        }

        if let Some(d) = drag {
            if d.moved {
                if let Some(target_id) = d.stack_target {
                    if let Some(base) = model
                        .desktop_widgets
                        .items
                        .iter()
                        .find(|p| p.id == target_id)
                        .cloned()
                    {
                        let (_sid, updates) = self.stacks.merge_pair(
                            &base.id,
                            &d.widget_id,
                            &base.stack_id,
                        );
                        for (mid, stack_id, order) in updates {
                            model.native_command(
                                "widgetUpdate",
                                json!({
                                    "id": mid,
                                    "x": base.x,
                                    "y": base.y,
                                    "width": base.width,
                                    "height": base.height,
                                    "size": base.size,
                                    "stackId": stack_id,
                                    "stackOrder": order,
                                }),
                            );
                        }
                    }
                } else {
                    let scale = self.canvases[idx].desc.scale.max(0.5);
                    let nx = self.canvases[idx].desc.phys_x
                        + (d.preview_rect.x * scale).round() as i32;
                    let ny = self.canvases[idx].desc.phys_y
                        + (d.preview_rect.y * scale).round() as i32;
                    let stack_group = model
                        .desktop_widgets
                        .items
                        .iter()
                        .find(|p| p.id == d.widget_id)
                        .map(|p| p.stack_id.clone())
                        .unwrap_or_default();
                    if stack_group.is_empty() {
                        if self.edit_mode {
                            model.native_command(
                                "widgetUpdate",
                                json!({
                                    "id": d.widget_id,
                                    "x": nx,
                                    "y": ny,
                                    "width": d.preview_rect.w.round(),
                                    "height": d.preview_rect.h.round(),
                                }),
                            );
                        } else {
                            model.native_command(
                                "widgetUpdate",
                                json!({"id": d.widget_id, "x": nx, "y": ny}),
                            );
                        }
                    } else {
                        let members: Vec<String> = model
                            .desktop_widgets
                            .items
                            .iter()
                            .filter(|p| p.stack_id == stack_group)
                            .map(|p| p.id.clone())
                            .collect();
                        for mid in members {
                            model.native_command(
                                "widgetUpdate",
                                json!({"id": mid, "x": nx, "y": ny}),
                            );
                        }
                    }
                }
                self.refresh_input_regions();
                self.canvases[idx].window.request_redraw();
                return None;
            }
        }

        self.refresh_input_regions();

        if let Some((cid, _wid, action)) = pressed {
            if cid == canvas_id {
                return self.execute_local_or_forward(idx, action, model);
            }
        }

        None
    }

    fn execute_local_or_forward(
        &mut self,
        idx: usize,
        action: Action,
        model: &mut Model,
    ) -> Option<Action> {
        let win = self.canvases[idx].window.clone();
        match action {
            Action::StackNext(id) => {
                self.stacks.step_by_member(&id, 1, self.now_sec);
                self.canvases[idx].dirty = true;
                win.request_redraw();
                None
            }
            Action::StackPrev(id) => {
                self.stacks.step_by_member(&id, -1, self.now_sec);
                self.canvases[idx].dirty = true;
                win.request_redraw();
                None
            }
            Action::StackUnpack(id) => {
                self.menu_for = None;
                let base = model.desktop_widgets.items.iter().find(|p| p.id == id);
                let offset_x = base.map(|p| p.x + 36).unwrap_or(100);
                let offset_y = base.map(|p| p.y + 36).unwrap_or(100);
                let updates = self.stacks.unpack_member(&id);
                for (mid, sid, ord) in updates {
                    if mid == id {
                        model.native_command(
                            "widgetUpdate",
                            json!({"id": mid, "x": offset_x, "y": offset_y, "stackId": sid, "stackOrder": ord}),
                        );
                    } else {
                        model.native_command(
                            "widgetUpdate",
                            json!({"id": mid, "stackId": sid, "stackOrder": ord}),
                        );
                    }
                }
                None
            }
            Action::NoteInlineEdit(id) => {
                if let Some(p) = model.desktop_widgets.items.iter().find(|p| p.id == id) {
                    self.note_edit = Some(NoteEdit::new(&p.id, &p.config.title, &p.config.text));
                    platform::activate(&win);
                    win.set_ime_allowed(true);
                    let px = (self.canvases[idx].pointer.0.max(20.0) as i32).max(0);
                    let py = (self.canvases[idx].pointer.1.max(20.0) as i32).max(0);
                    win.set_ime_cursor_area(
                        LogicalPosition::new(px, py),
                        PhysicalSize::new(120, 24),
                    );
                    win.request_redraw();
                }
                None
            }
            Action::NoteCancel => {
                self.note_edit = None;
                win.set_ime_allowed(false);
                platform::set_dashboard_layer(&win, self.edit_mode);
                win.request_redraw();
                None
            }
            Action::NoteSave => {
                if let Some(edit) = self.note_edit.take() {
                    if let Some(p) = model
                        .desktop_widgets
                        .items
                        .iter()
                        .find(|p| p.id == edit.widget_id)
                    {
                        let mut cfg = p.config.clone();
                        cfg.text = edit.text;
                        model.native_command(
                            "widgetUpdate",
                            json!({"id": edit.widget_id, "config": cfg}),
                        );
                    }
                }
                win.set_ime_allowed(false);
                platform::set_dashboard_layer(&win, self.edit_mode);
                win.request_redraw();
                None
            }
            Action::NoteToggleCheck(id, line_idx) => {
                if let Some(p) = model.desktop_widgets.items.iter().find(|p| p.id == id) {
                    if let Some(next_text) = toggle_note_checkbox(&p.config.text, line_idx) {
                        let mut cfg = p.config.clone();
                        cfg.text = next_text;
                        model.native_command("widgetUpdate", json!({"id": id, "config": cfg}));
                    }
                }
                None
            }
            Action::DropzoneOpen(id, item_idx) => {
                if let Some(it) = self.dropzone.items(&id).get(item_idx) {
                    platform::open_shell_path(&it.path);
                } else if let Some(raw) = model
                    .desktop_widgets
                    .items
                    .iter()
                    .find(|p| p.id == id)
                    .and_then(|p| p.config.items.get(item_idx))
                {
                    platform::open_shell_path(raw);
                }
                None
            }
            Action::DropzoneClear(id) => {
                self.dropzone.clear(&id);
                if let Some(p) = model.desktop_widgets.items.iter().find(|p| p.id == id) {
                    let mut cfg = p.config.clone();
                    cfg.items.clear();
                    model.native_command("widgetUpdate", json!({"id": id, "config": cfg}));
                }
                win.request_redraw();
                None
            }
            Action::PhotoNext(id) => {
                self.photos.next_photo(&id);
                win.request_redraw();
                None
            }
            Action::QuoteNext(id) => {
                let entry = self.quote_indices.entry(id).or_insert(0);
                *entry = entry.wrapping_add(1);
                win.request_redraw();
                None
            }
            Action::AppFolderLaunch(id, item_idx) => {
                if let Some(p) = model.desktop_widgets.items.iter().find(|p| p.id == id) {
                    let entries = crate::apple_desktop::app_folder_entries(&p.config);
                    if let Some(entry) = entries.get(item_idx) {
                        platform::open_shell_path(&entry.target);
                    }
                }
                None
            }
            Action::AppFolderToggleGrid(id) => {
                self.menu_for = None;
                self.refresh_input_regions();
                if let Some(p) = model.desktop_widgets.items.iter().find(|p| p.id == id) {
                    let mut cfg = p.config.clone();
                    cfg.caption = if cfg.caption.trim() == "9" {
                        "4".into()
                    } else {
                        "9".into()
                    };
                    model.native_command("widgetUpdate", json!({"id": id, "config": cfg}));
                }
                win.request_redraw();
                None
            }
            Action::EditModeToggle => {
                self.edit_mode = !self.edit_mode;
                self.menu_for = None;
                for c in &self.canvases {
                    platform::set_dashboard_layer(&c.window, self.edit_mode);
                    c.window.request_redraw();
                }
                self.refresh_input_regions();
                None
            }
            Action::EditModeAutoAlign => {
                let scale = self.canvases[idx].desc.scale.max(0.5);
                let mon_x = self.canvases[idx].desc.phys_x;
                let mon_y = self.canvases[idx].desc.phys_y;
                let grid = compute_grid(
                    self.canvases[idx].desc.phys_w as f64 / scale,
                    self.canvases[idx].desc.phys_h as f64 / scale,
                );
                let items: Vec<(String, Rect2D)> = self.canvases[idx]
                    .slots
                    .iter()
                    .filter(|s| !s.hidden)
                    .map(|s| (s.id.clone(), s.rect))
                    .collect();
                for (wid, aligned) in auto_pack_grid(&items, &grid) {
                    let nx = mon_x + (aligned.x * scale).round() as i32;
                    let ny = mon_y + (aligned.y * scale).round() as i32;
                    model.native_command(
                        "widgetUpdate",
                        json!({
                            "id": wid,
                            "x": nx,
                            "y": ny,
                            "width": aligned.w.round(),
                            "height": aligned.h.round(),
                        }),
                    );
                }
                win.request_redraw();
                None
            }
            Action::TodoInlineEdit(todo_id) => {
                let target_wid = self.canvases[idx]
                    .hits
                    .iter()
                    .find(|h| matches!(h.action, Action::TodoInlineEdit(_)))
                    .map(|h| h.widget_id.clone())
                    .or_else(|| {
                        model
                            .desktop_widgets
                            .items
                            .iter()
                            .find(|p| p.kind == Kind::Todos)
                            .map(|p| p.id.clone())
                    });
                if let Some(wid) = target_wid {
                    let ed = if let Some(tid) = todo_id {
                        model
                            .todos
                            .items
                            .iter()
                            .find(|t| t.id == tid)
                            .map(|t| TodoEdit {
                                id: Some(t.id.clone()),
                                title: t.title.clone(),
                                date: t.date.clone(),
                                time: t.time.clone(),
                                content: t.content.clone(),
                                daily: t.daily,
                                color: t.color.clone(),
                                ..Default::default()
                            })
                            .unwrap_or_default()
                    } else {
                        let d = crate::desktop_cards::date();
                        TodoEdit {
                            date: format!("{:04}-{:02}-{:02}", d.year, d.month, d.day),
                            ..Default::default()
                        }
                    };
                    self.todo_edit = Some((wid, ed));
                    platform::activate(&win);
                    win.set_ime_allowed(true);
                    win.request_redraw();
                }
                None
            }
            Action::TodoField(f) => {
                if let Some((_, ed)) = &mut self.todo_edit {
                    ed.field = f;
                    ed.preedit.clear();
                    ed.error = None;
                    win.request_redraw();
                }
                None
            }
            Action::TodoCancel => {
                self.todo_edit = None;
                win.set_ime_allowed(false);
                platform::set_dashboard_layer(&win, self.edit_mode);
                win.request_redraw();
                None
            }
            Action::TodoSave => {
                self.save_todo_edit(model);
                win.set_ime_allowed(false);
                platform::set_dashboard_layer(&win, self.edit_mode);
                win.request_redraw();
                None
            }
            other => {
                if matches!(
                    other,
                    Action::WidgetStyle(..)
                        | Action::WidgetMaterial(..)
                        | Action::WidgetResize(..)
                        | Action::WidgetHide(..)
                        | Action::WidgetRemove(..)
                        | Action::WidgetConfigure(..)
                ) {
                    // 仅在隐藏/移除时收起菜单，方便连续切换主题和材质手感。
                    if matches!(
                        other,
                        Action::WidgetHide(_) | Action::WidgetRemove(_) | Action::WidgetConfigure(_)
                    ) {
                        self.menu_for = None;
                        self.refresh_input_regions();
                    }
                    win.request_redraw();
                }
                Some(other)
            }
        }
    }

    fn save_todo_edit(&mut self, model: &mut Model) {
        let Some((wid, mut ed)) = self.todo_edit.take() else {
            return;
        };
        let title = ed.title.trim();
        if title.is_empty() {
            ed.error = Some("请填写标题".into());
            self.todo_edit = Some((wid, ed));
            return;
        }
        if !ed.date.is_empty() && !crate::desktop_cards::valid_date(&ed.date) {
            ed.error = Some("日期格式 yyyy-MM-dd".into());
            self.todo_edit = Some((wid, ed));
            return;
        }
        if !ed.time.is_empty() && !crate::desktop_cards::valid_time(&ed.time) {
            ed.error = Some("时间格式 HH:mm".into());
            self.todo_edit = Some((wid, ed));
            return;
        }
        if let Some(tid) = ed.id {
            model.native_command(
                "todoEdit",
                json!({"id":tid,"title":title,"date":ed.date,"time":ed.time,"content":ed.content,"daily":ed.daily,"color":ed.color}),
            );
        } else {
            model.native_command(
                "todoCreate",
                json!({"title":title,"date":ed.date,"time":ed.time,"content":ed.content,"daily":ed.daily,"color":ed.color}),
            );
        }
    }

    fn handle_wheel(&mut self, idx: usize, delta: MouseScrollDelta, model: &Model) {
        let dy = match delta {
            MouseScrollDelta::LineDelta(_, y) => y as f64 * 32.0,
            MouseScrollDelta::PixelDelta(p) => p.y,
        };
        if dy.abs() < 0.1 {
            return;
        }
        let pt = self.canvases[idx].pointer;
        let Some(si) = hit_test(&self.canvases[idx].slots, pt) else {
            return;
        };
        let slot = self.canvases[idx].slots[si].clone();
        let Some(p) = model.desktop_widgets.items.get(slot.placement_index) else {
            return;
        };

        let in_header = pt.1 - slot.rect.y <= 34.0;
        let stacked = slot.stack_info.is_some_and(|(_, tot)| tot >= 2);
        if stacked && (p.kind != Kind::Todos || in_header) {
            let step = if dy < 0.0 { 1 } else { -1 };
            self.stacks.step_by_member(&p.id, step, self.now_sec);
            self.canvases[idx].dirty = true;
            self.canvases[idx].window.request_redraw();
            return;
        }

        if p.kind == Kind::Todos {
            let (_, max) = crate::desktop_cards::todo_metrics(
                slot.rect.w,
                slot.rect.h,
                p.size,
                model.todos.items.len(),
            );
            let cur = self.scroll_for.entry(p.id.clone()).or_insert(0.0);
            *cur = (*cur - dy).clamp(0.0, max);
            self.canvases[idx].window.request_redraw();
        }
    }

    fn handle_drop(&mut self, idx: usize, path: &std::path::Path, model: &Model) -> Option<Action> {
        let pt = self.canvases[idx].pointer;
        let target_slot = hit_test(&self.canvases[idx].slots, pt)
            .or_else(|| {
                self.canvases[idx].slots.iter().position(|s| {
                    model
                        .desktop_widgets
                        .items
                        .get(s.placement_index)
                        .is_some_and(|p| p.kind == Kind::Dropzone)
                })
            })?;
        let pid = self.canvases[idx].slots[target_slot].id.clone();
        let p = model
            .desktop_widgets
            .items
            .iter()
            .find(|p| p.id == pid)?
            .clone();

        match p.kind {
            Kind::Dropzone => {
                if let Some(paths) = self.dropzone.push_path(&p.id, path) {
                    let mut cfg = p.config.clone();
                    cfg.items = paths;
                    model.native_command("widgetUpdate", json!({"id": p.id, "config": cfg}));
                    self.canvases[idx].window.request_redraw();
                }
            }
            Kind::Launcher => {
                if let Some((title, target)) = resolve_dropped_launcher(path) {
                    let mut cfg = p.config.clone();
                    if cfg.title.is_empty() {
                        cfg.title = title;
                    }
                    cfg.target = target;
                    model.native_command("widgetUpdate", json!({"id": p.id, "config": cfg}));
                }
            }
            Kind::PhotoFrame => {
                let folder = if path.is_dir() {
                    Some(path.to_path_buf())
                } else {
                    path.parent().map(|d| d.to_path_buf())
                };
                if let Some(dir) = folder {
                    let mut cfg = p.config.clone();
                    cfg.folder = dir.to_string_lossy().to_string();
                    model.native_command("widgetUpdate", json!({"id": p.id, "config": cfg}));
                    self.canvases[idx].window.request_redraw();
                }
            }
            Kind::AppFolder => {
                if let Some((title, target)) = resolve_dropped_launcher(path) {
                    let mut cfg = p.config.clone();
                    let entry = format!("{title}|{target}");
                    if !cfg.items.iter().any(|existing| existing == &entry || existing == &target) {
                        cfg.items.insert(0, entry);
                        cfg.items.truncate(9);
                        model.native_command("widgetUpdate", json!({"id": p.id, "config": cfg}));
                        self.canvases[idx].window.request_redraw();
                    }
                }
            }
            _ => {}
        }
        None
    }

    fn handle_key(&mut self, key: Key, model: &mut Model) -> Option<Action> {
        if key == Key::Named(NamedKey::Escape) {
            if self.note_edit.take().is_some() {
                for c in &self.canvases {
                    c.window.set_ime_allowed(false);
                    platform::set_dashboard_layer(&c.window, self.edit_mode);
                    c.window.request_redraw();
                }
                return None;
            }
            if self.todo_edit.take().is_some() {
                for c in &self.canvases {
                    c.window.set_ime_allowed(false);
                    platform::set_dashboard_layer(&c.window, self.edit_mode);
                    c.window.request_redraw();
                }
                return None;
            }
            if self.menu_for.take().is_some() {
                self.refresh_input_regions();
                for c in &self.canvases {
                    c.window.request_redraw();
                }
                return None;
            }
            if self.edit_mode {
                self.edit_mode = false;
                for c in &self.canvases {
                    platform::set_dashboard_layer(&c.window, false);
                    c.window.request_redraw();
                }
                self.refresh_input_regions();
                return None;
            }
        }

        if let Some(note) = &mut self.note_edit {
            match key {
                Key::Named(NamedKey::Backspace) => note.backspace(),
                Key::Named(NamedKey::Enter) => note.newline(),
                Key::Named(NamedKey::Space) => note.push_str(" "),
                Key::Character(s) => note.push_str(&s),
                _ => return None,
            }
            for c in &self.canvases {
                c.window.request_redraw();
            }
            return None;
        }

        if let Some((wid, ed)) = &mut self.todo_edit {
            match key {
                Key::Named(NamedKey::Tab) => {
                    ed.field = ed.field.next();
                    ed.preedit.clear();
                }
                Key::Named(NamedKey::Backspace) => {
                    ed.preedit.clear();
                    match ed.field {
                        TodoField::Title => {
                            ed.title.pop();
                        }
                        TodoField::Date => {
                            ed.date.pop();
                        }
                        TodoField::Time => {
                            ed.time.pop();
                        }
                    }
                }
                Key::Named(NamedKey::Enter) => {
                    let _ = wid.clone();
                    self.save_todo_edit(model);
                    for c in &self.canvases {
                        c.window.set_ime_allowed(false);
                        platform::set_dashboard_layer(&c.window, self.edit_mode);
                        c.window.request_redraw();
                    }
                    return None;
                }
                Key::Named(NamedKey::Space) => push_todo_field(ed, " "),
                Key::Character(s) => push_todo_field(ed, &s),
                _ => return None,
            }
            for c in &self.canvases {
                c.window.request_redraw();
            }
        }
        None
    }

    fn handle_ime(&mut self, ime: Ime, _model: &mut Model) {
        if let Some(note) = &mut self.note_edit {
            match ime {
                Ime::Preedit(s, _) => note.preedit = s,
                Ime::Commit(s) => note.push_str(&s),
                Ime::Disabled => note.preedit.clear(),
                Ime::Enabled => {}
            }
            for c in &self.canvases {
                c.window.request_redraw();
            }
            return;
        }
        if let Some((_, ed)) = &mut self.todo_edit {
            match ime {
                Ime::Preedit(s, _) => ed.preedit = s,
                Ime::Commit(s) => {
                    ed.preedit.clear();
                    push_todo_field(ed, &s);
                }
                Ime::Disabled => ed.preedit.clear(),
                Ime::Enabled => {}
            }
            for c in &self.canvases {
                c.window.request_redraw();
            }
        }
    }

}

fn push_todo_field(ed: &mut TodoEdit, text: &str) {
    ed.preedit.clear();
    match ed.field {
        TodoField::Title if ed.title.chars().count() < 80 => ed.title.push_str(text),
        TodoField::Date if ed.date.len() < 10 => ed.date.push_str(text),
        TodoField::Time if ed.time.len() < 5 => ed.time.push_str(text),
        _ => {}
    }
}

fn detect_edges(r: Rect2D, pt: (f64, f64)) -> Edges {
    Edges {
        left: (pt.0 - r.x).abs() <= EDGE,
        right: (pt.0 - r.right()).abs() <= EDGE,
        top: (pt.1 - r.y).abs() <= EDGE,
        bottom: (pt.1 - r.bottom()).abs() <= EDGE,
    }
}

fn collect_monitors(event: &ActiveEventLoop) -> Vec<MonitorDesc> {
    let mut out = Vec::new();
    for m in event.available_monitors() {
        let s = m.size();
        if s.width == 0 || s.height == 0 {
            continue;
        }
        let p = m.position();
        out.push(MonitorDesc {
            key: format!("{}:{}:{}x{}", p.x, p.y, s.width, s.height),
            phys_x: p.x,
            phys_y: p.y,
            phys_w: s.width,
            phys_h: s.height,
            scale: m.scale_factor(),
        });
    }
    if out.is_empty() {
        if let Some(m) = event.primary_monitor() {
            let s = m.size();
            let p = m.position();
            if s.width > 0 && s.height > 0 {
                out.push(MonitorDesc {
                    key: format!("{}:{}:{}x{}", p.x, p.y, s.width, s.height),
                    phys_x: p.x,
                    phys_y: p.y,
                    phys_w: s.width,
                    phys_h: s.height,
                    scale: m.scale_factor(),
                });
            }
        }
    }
    out
}

fn create_canvas(
    event: &ActiveEventLoop,
    gpu: &mut Gpu,
    desc: MonitorDesc,
    slots: Vec<CanvasSlot>,
    dashboard_open: bool,
) -> Option<MonitorCanvas> {
    let attrs = Window::default_attributes()
        .with_title("DotIsle Allegro Canvas")
        .with_decorations(false)
        .with_transparent(true)
        .with_no_redirection_bitmap(true)
        .with_undecorated_shadow(false)
        .with_drag_and_drop(true)
        .with_resizable(false)
        .with_skip_taskbar(true)
        .with_visible(true)
        .with_inner_size(PhysicalSize::new(desc.phys_w, desc.phys_h))
        .with_position(PhysicalPosition::new(desc.phys_x, desc.phys_y));
    let window = match event.create_window(attrs) {
        Ok(w) => Arc::new(w),
        Err(err) => {
            eprintln!("Allegro create_window failed: {err}");
            return None;
        }
    };
    platform::desktop_owner(&window);
    platform::remove_dwm_frame(&window);
    let surface = match gpu.surface(window.clone()) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("Allegro gpu.surface failed: {err}");
            return None;
        }
    };
    window.set_visible(true);
    platform::show_inactive(&window);
    platform::set_dashboard_layer(&window, dashboard_open);
    Some(MonitorCanvas {
        desc,
        window,
        surface,
        slots,
        hits: Vec::new(),
        pointer: (-1000.0, -1000.0),
        hovered_hit: None,
        cursor: CursorIcon::Default,
        last_input_rects: Vec::new(),
        last_allow_full: false,
        dirty: true,
    })
}
