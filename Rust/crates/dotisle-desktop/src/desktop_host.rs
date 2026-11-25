//! Tool windows owned by the desktop, kept below application windows without SetParent.
use crate::{
    draw::Text,
    gpu::Gpu,
    model::{Action, Model},
    views::{TodoEdit, TodoField},
    widgets::Hit,
};
use dotisle_domain::desktop_widgets::{Kind, Placement};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::ActiveEventLoop,
    keyboard::{Key, NamedKey},
    platform::windows::WindowAttributesExtWindows,
    window::{CursorIcon, Window, WindowId},
};

const HIDDEN_BAR: f64 = 12.;
const MIN_W: f64 = 160.;
const MIN_H: f64 = 120.;
const MAX_W: f64 = 1200.;
const MAX_H: f64 = 1200.;
const EDGE: f64 = 8.;

pub struct Host {
    windows: Vec<Card>,
    last_second: Instant,
    pub error: Option<String>,
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
#[derive(Clone, Copy)]
struct Resize {
    edges: Edges,
    origin: (i32, i32),
    size: (u32, u32),
    cursor: (f64, f64),
}
struct Card {
    window: Arc<Window>,
    surface: dotisle_render::surface::Surface,
    placement: Placement,
    hits: Vec<Hit>,
    pointer: (f64, f64),
    pressed: Option<Action>,
    drag: Option<((i32, i32), (f64, f64))>,
    resize: Option<Resize>,
    menu: bool,
    pending: Option<((i32, i32), Instant)>,
    hidden: bool,
    scroll: f64,
    edit: Option<TodoEdit>,
    last_dims: (f64, f64),
    cursor: CursorIcon,
}
impl Default for Host {
    fn default() -> Self {
        Self {
            windows: vec![],
            last_second: Instant::now() - Duration::from_secs(2),
            error: None,
        }
    }
}
fn clamp_position(window: &Window, x: i32, y: i32) -> (i32, i32) {
    let size = window.outer_size();
    let monitors = window.available_monitors().collect::<Vec<_>>();
    if monitors.iter().any(|m| {
        let p = m.position();
        let s = m.size();
        x >= p.x
            && y >= p.y
            && x + size.width as i32 <= p.x + s.width as i32
            && y + size.height as i32 <= p.y + s.height as i32
    }) {
        return (x, y);
    }
    let selected = monitors.iter().min_by_key(|m| {
        let p = m.position();
        (x as i64 - p.x as i64).abs() + (y as i64 - p.y as i64).abs()
    });
    if let Some(m) = selected {
        let p = m.position();
        let s = m.size();
        (
            x.clamp(p.x, (p.x + s.width as i32 - size.width as i32).max(p.x)),
            y.clamp(
                p.y,
                (p.y + s.height as i32 - size.height as i32 - 48).max(p.y),
            ),
        )
    } else {
        (x, y)
    }
}
fn monitor_near(window: &Window, x: i32) -> Option<(i32, i32, u32, u32)> {
    window
        .available_monitors()
        .min_by_key(|m| (x as i64 - m.position().x as i64).abs())
        .map(|m| {
            let p = m.position();
            let s = m.size();
            (p.x, p.y, s.width, s.height)
        })
}
fn hidden_position(window: &Window, x: i32, width: f64, bar_width: f64) -> (i32, i32) {
    let scale = window.scale_factor();
    let Some((mx, my, mw, _)) = monitor_near(window, x) else {
        return (x, 0);
    };
    let bar = (bar_width * scale).round() as i32;
    let original = (width * scale).round() as i32;
    let center = x + (original - bar) / 2;
    (center.clamp(mx, (mx + mw as i32 - bar).max(mx)), my)
}
fn edges_at(pointer: (f64, f64), w: f64, h: f64) -> Edges {
    Edges {
        left: pointer.0 <= EDGE,
        right: pointer.0 >= w - EDGE,
        top: pointer.1 <= EDGE,
        bottom: pointer.1 >= h - EDGE,
    }
}
fn logical_size(window: &Window) -> (f64, f64) {
    let scale = window.scale_factor();
    let inner = window.inner_size();
    (
        (inner.width as f64 / scale).max(1.),
        (inner.height as f64 / scale).max(1.),
    )
}
fn enter_hidden(card: &mut Card) {
    let scale = card.window.scale_factor();
    let inner = card.window.inner_size();
    let current = inner.width as f64 / scale;
    let bar = current.clamp(MIN_W, 240.);
    let position = card
        .window
        .outer_position()
        .unwrap_or(PhysicalPosition::new(0, 0));
    let (x, y) = hidden_position(&card.window, position.x, current, bar);
    let _ = card
        .window
        .request_inner_size(LogicalSize::new(bar, HIDDEN_BAR));
    card.window
        .set_outer_position(PhysicalPosition::new(x, y));
    card.hidden = true;
    card.menu = false;
    card.edit = None;
    card.scroll = 0.;
    card.window.set_ime_allowed(false);
    card.window.request_redraw();
}
fn leave_hidden(card: &mut Card, placement: &Placement) {
    let (w, h) = placement.dimensions();
    let _ = card.window.request_inner_size(LogicalSize::new(w, h));
    let (x, y) = clamp_position(&card.window, placement.x, placement.y);
    card.window
        .set_outer_position(PhysicalPosition::new(x, y));
    card.hidden = false;
    card.last_dims = (w, h);
    card.window.request_redraw();
}
fn position_ime(card: &Card) {
    let Some(edit) = card.edit.as_ref() else {
        return;
    };
    let (w, _) = logical_size(&card.window);
    let field = crate::desktop_cards::editor_field_rect(w, edit.field);
    card.window.set_ime_cursor_area(
        LogicalPosition::new(field.x0, field.y0 + field.height() / 2.),
        LogicalSize::new(field.width(), field.height()),
    );
}
fn days_in_month(year: i32, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 400 == 0 || (year % 4 == 0 && year % 100 != 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}
/// One hour after now, carrying across midnight so a late-night draft is still in the future.
fn next_default_time() -> (String, String) {
    let d = crate::desktop_cards::date();
    let total = d.hour * 60 + d.minute + 60;
    let (mut year, mut month, mut day, minute) = if total >= 1440 {
        (d.year, d.month, d.day, total - 1440)
    } else {
        (d.year, d.month, d.day, total)
    };
    if total >= 1440 {
        day += 1;
        if day > days_in_month(year, month) {
            day = 1;
            month += 1;
            if month > 12 {
                month = 1;
                year += 1;
            }
        }
    }
    (
        format!("{:04}-{:02}-{:02}", year, month, day),
        format!("{:02}:{:02}", minute / 60, minute % 60),
    )
}
fn begin_edit(card: &mut Card, m: &Model, id: Option<String>) {
    let mut edit = TodoEdit::default();
    if let Some(id) = id {
        if let Some(todo) = m.todos.items.iter().find(|t| t.id == id) {
            edit.id = Some(todo.id.clone());
            edit.title = todo.title.clone();
            edit.date = todo.date.clone();
            edit.time = todo.time.clone();
            edit.content = todo.content.clone();
            edit.daily = todo.daily;
            edit.color = todo.color.clone();
        }
    } else {
        let (date, time) = next_default_time();
        edit.date = date;
        edit.time = time;
    }
    card.edit = Some(edit);
    card.scroll = 0.;
    card.menu = false;
    card.window.focus_window();
    crate::platform::activate(&card.window);
    card.window.set_ime_allowed(true);
    position_ime(card);
    card.window.request_redraw();
}
fn save_edit(card: &mut Card, m: &Model) {
    let Some(edit) = card.edit.as_mut() else {
        return;
    };
    let title = edit.title.trim();
    if title.is_empty() {
        edit.error = Some("请填写待办标题".into());
        card.window.request_redraw();
        return;
    }
    if title.chars().count() > 120 {
        edit.error = Some("标题最多 120 个字符".into());
        card.window.request_redraw();
        return;
    }
    if !crate::desktop_cards::valid_date(&edit.date) {
        edit.error = Some("日期需为 YYYY-MM-DD".into());
        card.window.request_redraw();
        return;
    }
    if !crate::desktop_cards::valid_time(&edit.time) {
        edit.error = Some("时间需为 HH:MM".into());
        card.window.request_redraw();
        return;
    }
    let mut value = serde_json::json!({
        "title": title,
        "content": edit.content,
        "date": edit.date,
        "time": edit.time,
        "daily": edit.daily,
        "color": edit.color,
    });
    if let Some(id) = &edit.id {
        value["id"] = serde_json::json!(id);
    }
    m.native_command("todoSave", value);
    card.edit = None;
    card.window.set_ime_allowed(false);
    crate::platform::sink(&card.window);
    card.window.request_redraw();
}
fn cancel_edit(card: &mut Card) {
    card.edit = None;
    card.window.set_ime_allowed(false);
    crate::platform::sink(&card.window);
    card.window.request_redraw();
}
fn edit_text(card: &mut Card, text: &str) {
    let Some(edit) = card.edit.as_mut() else {
        return;
    };
    edit.error = None;
    edit.preedit.clear();
    match edit.field {
        TodoField::Title => edit.title.push_str(text),
        TodoField::Date => edit.date.push_str(text),
        TodoField::Time => edit.time.push_str(text),
    }
    card.window.request_redraw();
}
fn edit_backspace(card: &mut Card) {
    let Some(edit) = card.edit.as_mut() else {
        return;
    };
    edit.error = None;
    edit.preedit.clear();
    match edit.field {
        TodoField::Title => {
            edit.title.pop();
        }
        TodoField::Date => {
            edit.date.pop();
        }
        TodoField::Time => {
            edit.time.pop();
        }
    }
    card.window.request_redraw();
}
impl Host {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
    pub fn contains(&self, id: WindowId) -> bool {
        self.windows.iter().any(|c| c.window.id() == id)
    }
    pub fn sync(&mut self, event: &ActiveEventLoop, m: &mut Model, gpu: &mut Gpu, changed: bool, moving: bool) {
        self.windows.retain(|c| {
            m.desktop_widgets
                .items
                .iter()
                .any(|p| p.id == c.placement.id)
        });
        for p in m.desktop_widgets.items.clone() {
            if let Some(card) = self.windows.iter_mut().find(|c| c.placement.id == p.id) {
                if card.hidden != p.hidden {
                    if p.hidden {
                        enter_hidden(card);
                    } else {
                        leave_hidden(card, &p);
                    }
                }
                if !p.hidden {
                    let dims = p.dimensions();
                    if card.resize.is_none()
                        && card.drag.is_none()
                        && ((card.last_dims.0 - dims.0).abs() > 0.5
                            || (card.last_dims.1 - dims.1).abs() > 0.5)
                    {
                        let _ = card
                            .window
                            .request_inner_size(LogicalSize::new(dims.0, dims.1));
                        card.last_dims = dims;
                        card.window.request_redraw();
                    }
                    if card.drag.is_none() && card.resize.is_none() {
                        if let Some((position, at)) = card.pending {
                            if (p.x, p.y) == position {
                                card.pending = None;
                            } else if at.elapsed() > Duration::from_secs(4) {
                                card.pending = None;
                            }
                        }
                        if card.pending.is_none() {
                            let position = clamp_position(&card.window, p.x, p.y);
                            if card.window.outer_position().ok().map(|v| (v.x, v.y)) != Some(position) {
                                card.window
                                    .set_outer_position(PhysicalPosition::new(position.0, position.1));
                            }
                            if position != (p.x, p.y) {
                                m.native_command(
                                    "widgetUpdate",
                                    serde_json::json!({"id":p.id,"x":position.0,"y":position.1}),
                                );
                                card.pending = Some((position, Instant::now()));
                            }
                        }
                    }
                }
                card.placement = p;
                continue;
            }
            let dims = p.dimensions();
            let (w, h) = if p.hidden {
                (dims.0.clamp(MIN_W, 240.), HIDDEN_BAR)
            } else {
                dims
            };
            let attributes = Window::default_attributes()
                .with_title(format!("点屿桌面组件 · {}", p.kind.label()))
                .with_inner_size(LogicalSize::new(w, h))
                .with_decorations(false)
                .with_resizable(false)
                .with_transparent(true)
                .with_visible(false)
                .with_active(false)
                .with_skip_taskbar(true)
                .with_no_redirection_bitmap(true);
            match event
                .create_window(attributes)
                .map_err(|e| e.to_string())
                .and_then(|window| {
                    let window = Arc::new(window);
                    crate::platform::desktop_owner(&window);
                    crate::platform::remove_dwm_frame(&window);
                    let position = if p.hidden {
                        hidden_position(&window, p.x, dims.0, w)
                    } else {
                        clamp_position(&window, p.x, p.y)
                    };
                    window.set_outer_position(PhysicalPosition::new(position.0, position.1));
                    let surface = gpu.surface(window.clone())?;
                    Ok(Card {
                        window,
                        surface,
                        placement: p.clone(),
                        hits: vec![],
                        pointer: (-1., -1.),
                        pressed: None,
                        drag: None,
                        resize: None,
                        menu: false,
                        pending: None,
                        hidden: p.hidden,
                        scroll: 0.,
                        edit: None,
                        last_dims: dims,
                        cursor: CursorIcon::Default,
                    })
                }) {
                Ok(card) => {
                    self.windows.push(card);
                    if let Some(c) = self.windows.last() {
                        c.window.set_visible(true);
                        crate::platform::desktop_floor(&c.window);
                        c.window.request_redraw();
                    }
                }
                Err(error) => {
                    self.error = Some(error.clone());
                    m.message(format!("桌面组件未能显示：{error}"));
                    break;
                }
            }
        }
        let second = self.last_second.elapsed() >= Duration::from_secs(1);
        if second {
            self.last_second = Instant::now();
        }
        for c in &self.windows {
            if second && c.edit.is_none() {
                crate::platform::desktop_floor(&c.window);
            }
            if changed || second || (moving && c.placement.kind == Kind::Music) {
                c.window.request_redraw();
            }
        }
    }
    pub fn event(
        &mut self,
        id: WindowId,
        event: WindowEvent,
        m: &mut Model,
        gpu: &mut Gpu,
        text: &mut Text,
        time: f64,
        media_position: Option<f64>,
    ) -> Option<Action> {
        let c = self.windows.iter_mut().find(|c| c.window.id() == id)?;
        let mut action = None;
        match event {
            WindowEvent::RedrawRequested => {
                let (w, h) = logical_size(&c.window);
                let view = crate::desktop_cards::render(
                    m,
                    text,
                    c.placement.kind,
                    c.placement.size,
                    &c.placement.id,
                    w,
                    h,
                    c.pointer,
                    c.menu,
                    c.hidden,
                    c.scroll,
                    c.edit.as_ref(),
                    time,
                    media_position,
                );
                c.hits = view.hits;
                if let Err(e) = gpu.draw(&mut c.surface, &view.scene, c.window.scale_factor()) {
                    self.error = Some(e);
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    c.surface.resize(size.width, size.height);
                    let scale = c.window.scale_factor();
                    c.last_dims = (
                        size.width as f64 / scale,
                        size.height as f64 / scale,
                    );
                    c.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                if c.hidden {
                    let (_, _) = logical_size(&c.window);
                    let bar = (c.placement.dimensions().0).clamp(MIN_W, 240.);
                    let _ = c.window.request_inner_size(LogicalSize::new(bar, HIDDEN_BAR));
                } else {
                    let (w, h) = c.placement.dimensions();
                    let _ = c.window.request_inner_size(LogicalSize::new(w, h));
                }
                c.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = c.window.scale_factor();
                c.pointer = (position.x / scale, position.y / scale);
                if let Some(resize) = c.resize {
                    let dx = position.x - resize.cursor.0;
                    let dy = position.y - resize.cursor.1;
                    let scale = c.window.scale_factor();
                    let mut w = resize.size.0 as f64;
                    let mut h = resize.size.1 as f64;
                    if resize.edges.right {
                        w += dx;
                    }
                    if resize.edges.left {
                        w -= dx;
                    }
                    if resize.edges.bottom {
                        h += dy;
                    }
                    if resize.edges.top {
                        h -= dy;
                    }
                    w = w.clamp(MIN_W * scale, MAX_W * scale);
                    h = h.clamp(MIN_H * scale, MAX_H * scale);
                    let _ = c
                        .window
                        .request_inner_size(LogicalSize::new(w / scale, h / scale));
                    let mut x = resize.origin.0;
                    let mut y = resize.origin.1;
                    if resize.edges.left {
                        x += resize.size.0 as i32 - w.round() as i32;
                    }
                    if resize.edges.top {
                        y += resize.size.1 as i32 - h.round() as i32;
                    }
                    c.window
                        .set_outer_position(PhysicalPosition::new(x, y));
                } else if let Some((origin, grab)) = c.drag {
                    let current = c
                        .window
                        .outer_position()
                        .unwrap_or(PhysicalPosition::new(origin.0, origin.1));
                    let target = (
                        origin.0 + (current.x as f64 + position.x - grab.0).round() as i32,
                        origin.1 + (current.y as f64 + position.y - grab.1).round() as i32,
                    );
                    c.window
                        .set_outer_position(PhysicalPosition::new(target.0, target.1));
                }
                let (w, h) = logical_size(&c.window);
                let cursor = if c.hidden {
                    CursorIcon::Pointer
                } else if c.edit.is_some() {
                    CursorIcon::Text
                } else {
                    edges_at(c.pointer, w, h).icon()
                };
                if c.cursor != cursor {
                    c.cursor = cursor;
                    c.window.set_cursor_icon(cursor);
                }
                c.window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                if c.drag.is_none() && c.resize.is_none() {
                    c.pointer = (-1., -1.);
                    c.window.request_redraw();
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if !c.hidden && c.edit.is_none() && c.placement.kind == Kind::Todos {
                    let amount = match delta {
                        MouseScrollDelta::LineDelta(_, y) => y as f64 * 46.,
                        MouseScrollDelta::PixelDelta(p) => p.y,
                    };
                    let (w, h) = logical_size(&c.window);
                    let count = m
                        .todos
                        .items
                        .iter()
                        .filter(|t| !t.done(&dotisle_domain::todos::Now::local()))
                        .count();
                    let (_, max) = crate::desktop_cards::todo_metrics(w, h, c.placement.size, count);
                    c.scroll = (c.scroll - amount).clamp(0., max);
                    c.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if c.hidden {
                    action = Some(Action::WidgetRestore(c.placement.id.clone()));
                } else if c.edit.is_none() {
                    c.menu = !c.menu;
                    c.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if state == ElementState::Pressed {
                    c.pressed = c
                        .hits
                        .iter()
                        .rfind(|h| h.enabled && h.rect.contains(c.pointer))
                        .map(|h| h.action.clone());
                    if c.pressed.is_none() && !c.hidden && c.edit.is_none() {
                        let (w, h) = logical_size(&c.window);
                        let edges = edges_at(c.pointer, w, h);
                        if edges.any() {
                            if let Ok(origin) = c.window.outer_position() {
                                let scale = c.window.scale_factor();
                                let inner = c.window.inner_size();
                                c.resize = Some(Resize {
                                    edges,
                                    origin: (origin.x, origin.y),
                                    size: (inner.width, inner.height),
                                    cursor: (c.pointer.0 * scale, c.pointer.1 * scale),
                                });
                                crate::platform::capture_pointer(&c.window, true);
                            }
                        } else if let Ok(origin) = c.window.outer_position() {
                            let scale = c.window.scale_factor();
                            c.drag = Some((
                                (origin.x, origin.y),
                                (
                                    origin.x as f64 + c.pointer.0 * scale,
                                    origin.y as f64 + c.pointer.1 * scale,
                                ),
                            ));
                            crate::platform::capture_pointer(&c.window, true);
                        }
                    }
                } else {
                    if let Some(_resize) = c.resize.take() {
                        crate::platform::capture_pointer(&c.window, false);
                        let (w, h) = logical_size(&c.window);
                        m.native_command(
                            "widgetUpdate",
                            serde_json::json!({"id":c.placement.id,"width":w,"height":h}),
                        );
                        if m.demo {
                            let _ = m.desktop_widgets.update(
                                &serde_json::json!({"id":c.placement.id,"width":w,"height":h}),
                            );
                        }
                        c.last_dims = (w, h);
                        c.pending = None;
                    } else if c.drag.take().is_some() {
                        crate::platform::capture_pointer(&c.window, false);
                        if let Ok(position) = c.window.outer_position() {
                            let p = clamp_position(&c.window, position.x, position.y);
                            c.window.set_outer_position(PhysicalPosition::new(p.0, p.1));
                            m.native_command(
                                "widgetUpdate",
                                serde_json::json!({"id":c.placement.id,"x":p.0,"y":p.1}),
                            );
                            if m.demo {
                                let _ = m.desktop_widgets.update(
                                    &serde_json::json!({"id":c.placement.id,"x":p.0,"y":p.1}),
                                );
                            }
                            c.pending = Some((p, Instant::now()));
                        }
                    } else if let Some(pressed) = c.pressed.take() {
                        if c.hits.iter().any(|h| {
                            h.enabled
                                && h.rect.contains(c.pointer)
                                && format!("{:?}", h.action) == format!("{pressed:?}")
                        }) {
                            match pressed {
                                Action::WidgetFocus => c.menu = !c.menu,
                                Action::TodoInlineEdit(todo) => begin_edit(c, m, todo),
                                Action::TodoField(field) => {
                                    if let Some(edit) = c.edit.as_mut() {
                                        edit.field = field;
                                        edit.preedit.clear();
                                        edit.error = None;
                                    }
                                    c.window.focus_window();
                                    crate::platform::activate(&c.window);
                                    c.window.set_ime_allowed(true);
                                    position_ime(c);
                                    c.window.request_redraw();
                                }
                                Action::TodoSave => save_edit(c, m),
                                Action::TodoCancel => cancel_edit(c),
                                pressed => {
                                    action = Some(pressed);
                                    c.menu = false;
                                }
                            }
                        }
                    }
                    c.window.request_redraw();
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if c.edit.is_some() && event.state == ElementState::Pressed {
                    match &event.logical_key {
                        Key::Named(NamedKey::Escape) => cancel_edit(c),
                        Key::Named(NamedKey::Enter) => save_edit(c, m),
                        Key::Named(NamedKey::Tab) => {
                            if let Some(edit) = c.edit.as_mut() {
                                edit.field = edit.field.next();
                                edit.preedit.clear();
                            }
                            position_ime(c);
                            c.window.request_redraw();
                        }
                        Key::Named(NamedKey::Backspace) => edit_backspace(c),
                        Key::Character(value) if !value.chars().any(char::is_control) => {
                            edit_text(c, value)
                        }
                        _ => {}
                    }
                }
            }
            WindowEvent::Ime(Ime::Preedit(value, _)) => {
                if let Some(edit) = c.edit.as_mut() {
                    edit.preedit = value;
                }
                c.window.request_redraw();
            }
            WindowEvent::Ime(Ime::Commit(value)) => {
                if let Some(edit) = c.edit.as_mut() {
                    edit.preedit.clear();
                }
                if c.edit.is_some() {
                    edit_text(c, &value);
                }
                c.window.request_redraw();
            }
            WindowEvent::Ime(Ime::Disabled) => {
                if let Some(edit) = c.edit.as_mut() {
                    edit.preedit.clear();
                }
                c.window.request_redraw();
            }
            WindowEvent::Focused(false) => {
                if let Some(_resize) = c.resize.take() {
                    crate::platform::capture_pointer(&c.window, false);
                    let (w, h) = logical_size(&c.window);
                    m.native_command(
                        "widgetUpdate",
                        serde_json::json!({"id":c.placement.id,"width":w,"height":h}),
                    );
                    c.last_dims = (w, h);
                }
                if c.drag.take().is_some() {
                    crate::platform::capture_pointer(&c.window, false);
                    if let Ok(p) = c.window.outer_position() {
                        m.native_command(
                            "widgetUpdate",
                            serde_json::json!({"id":c.placement.id,"x":p.x,"y":p.y}),
                        );
                        c.pending = Some(((p.x, p.y), Instant::now()));
                    }
                }
                c.pressed = None;
            }
            WindowEvent::CloseRequested => {
                action = Some(Action::WidgetRemove(c.placement.id.clone()))
            }
            _ => {}
        }
        action
    }
}