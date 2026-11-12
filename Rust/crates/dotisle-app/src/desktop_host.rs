//! Tool windows owned by the desktop, kept below application windows without SetParent.
use crate::{
    draw::Text,
    gpu::Gpu,
    model::{Action, Model, Page},
    widgets::Hit,
};
use dotisle_domain::desktop_widgets::Placement;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    dpi::{LogicalSize, PhysicalPosition},
    event::{ElementState, MouseButton, WindowEvent},
    event_loop::ActiveEventLoop,
    platform::windows::WindowAttributesExtWindows,
    window::{Window, WindowId},
};
pub struct Host {
    windows: Vec<Card>,
    last_second: Instant,
    pub error: Option<String>,
}
struct Card {
    window: Arc<Window>,
    surface: dotisle_render::surface::Surface,
    placement: Placement,
    hits: Vec<Hit>,
    pointer: (f64, f64),
    pressed: Option<Action>,
    drag: Option<((i32, i32), (f64, f64))>,
    menu: bool,
    pending: Option<((i32, i32), Instant)>,
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
impl Host {
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }
    pub fn contains(&self, id: WindowId) -> bool {
        self.windows.iter().any(|c| c.window.id() == id)
    }
    pub fn sync(&mut self, event: &ActiveEventLoop, m: &mut Model, gpu: &mut Gpu, changed: bool) {
        self.windows.retain(|c| {
            m.desktop_widgets
                .items
                .iter()
                .any(|p| p.id == c.placement.id)
        });
        for p in m.desktop_widgets.items.clone() {
            if let Some(card) = self.windows.iter_mut().find(|c| c.placement.id == p.id) {
                if card.placement.size != p.size {
                    let (w, h) = p.size.dimensions();
                    let _ = card.window.request_inner_size(LogicalSize::new(w, h));
                    card.placement.size = p.size;
                    card.window.request_redraw();
                }
                if card.drag.is_none() {
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
                card.placement = p;
                continue;
            }
            let (w, h) = p.size.dimensions();
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
                    let position = clamp_position(&window, p.x, p.y);
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
                        menu: false,
                        pending: None,
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
            if second {
                crate::platform::desktop_floor(&c.window);
            }
            if changed || second {
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
    ) -> Option<Action> {
        let c = self.windows.iter_mut().find(|c| c.window.id() == id)?;
        let mut action = None;
        match event {
            WindowEvent::RedrawRequested => {
                let view = crate::desktop_cards::render(
                    m,
                    text,
                    c.placement.kind,
                    c.placement.size,
                    &c.placement.id,
                    c.pointer,
                    c.menu,
                    time,
                );
                c.hits = view.hits;
                if let Err(e) = gpu.draw(&mut c.surface, &view.scene, c.window.scale_factor()) {
                    self.error = Some(e);
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    c.surface.resize(size.width, size.height);
                    c.window.request_redraw();
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                let (w, h) = c.placement.size.dimensions();
                let _ = c.window.request_inner_size(LogicalSize::new(w, h));
                c.window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let scale = c.window.scale_factor();
                c.pointer = (position.x / scale, position.y / scale);
                if let Some((origin, grab)) = c.drag {
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
                c.window.request_redraw();
            }
            WindowEvent::CursorLeft { .. } => {
                if c.drag.is_none() {
                    c.pointer = (-1., -1.);
                    c.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                c.menu = !c.menu;
                c.window.request_redraw();
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
                    if c.pressed.is_none() {
                        if let Ok(origin) = c.window.outer_position() {
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
                    if c.drag.take().is_some() {
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
                            if matches!(pressed, Action::WidgetFocus) {
                                c.menu = !c.menu;
                            } else {
                                action = Some(pressed);
                                c.menu = false;
                            }
                        }
                    }
                    c.window.request_redraw();
                }
            }
            WindowEvent::Focused(false) => {
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
        match &action {
            Some(Action::EditTodo(_))=>{m.page=Page::Todos;m.show_center=true;}
            Some(Action::WidgetConfigure(_))=>{m.page=Page::Widgets;m.show_center=true;}
            Some(Action::Navigate(page))=>{m.page=*page;m.show_center=true;}
            _=>{}
        }
        action
    }
}
