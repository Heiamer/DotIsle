//! One Vello settings window. The original island is owned by the native process.
use crate::{
    draw, gpu,
    model::{Action, Model, Page},
    motion::Spring,
    platform, views,
    widgets::Hit,
};
use dotisle_render::surface::Surface as RenderSurface;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalSize},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, ModifiersState, NamedKey},
    platform::windows::IconExtWindows,
    window::{ResizeDirection, Window, WindowId},
};
static WAKE: std::sync::OnceLock<winit::event_loop::EventLoopProxy<()>> =
    std::sync::OnceLock::new();
pub fn wake() {
    if let Some(proxy) = WAKE.get() {
        let _ = proxy.send_event(());
    }
}
static SWITCH_START: std::sync::OnceLock<std::sync::Mutex<Option<Instant>>> =
    std::sync::OnceLock::new();
static FRAME_TICK: std::sync::OnceLock<std::sync::Mutex<(Instant, usize)>> =
    std::sync::OnceLock::new();
fn switch_path() -> Option<&'static std::path::Path> {
    static P: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    P.get_or_init(|| std::env::var_os("DOTISLE_SWITCH_LOG").map(std::path::PathBuf::from))
        .as_deref()
}
fn fps_path() -> Option<&'static std::path::Path> {
    static P: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    P.get_or_init(|| std::env::var_os("DOTISLE_FPS_LOG").map(std::path::PathBuf::from))
        .as_deref()
}
fn hits_path() -> Option<&'static std::path::Path> {
    static P: std::sync::OnceLock<Option<std::path::PathBuf>> = std::sync::OnceLock::new();
    P.get_or_init(|| std::env::var_os("DOTISLE_HITS_PATH").map(std::path::PathBuf::from))
        .as_deref()
}
fn append_line(path: &std::path::Path, line: &str) {
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
    {
        let _ = writeln!(f, "{line}");
    }
}
struct Surface {
    surface: RenderSurface,
    window: Arc<Window>,
    hits: Vec<Hit>,
    pointer: (f64, f64),
    focus: Option<usize>,
    pressed: Option<usize>,
    visible: bool,
    occluded: bool,
    taskbar: bool,
    open_profile: bool,
    hidden_since: Option<Instant>,
}
struct App {
    frame_trace: Option<crate::frame_trace::Trace>,
    overlay_motion: crate::overlay_motion::OverlayMotion,
    signature_at: Option<Instant>,
    desktop: crate::desktop_host::Host,
    scroll_motion: Spring,
    scroll_target: f64,
    scroll_page: Page,
    scroll_blur: f64,
    scroll_input: Instant,
    deck_velocity: f64,
    deck_drag: Option<(f64, f64, Instant, f64)>,
    native_pid: Option<u32>,
    warm: bool,
    switches: std::collections::HashMap<String, Spring>,
    color_motion: dotisle_domain::dynamic_color::RoleMotion,
    interactions: Vec<Spring>,
    presses: Vec<Spring>,
    selections: std::collections::HashMap<String, Spring>,
    motion_targets: Vec<(String, f64)>,
    page_enter: Spring,
    seek_preview: Option<f64>,
    seek_track: Option<serde_json::Value>,
    timeline: crate::timeline::Timeline,
    lyrics: crate::music_card::LyricsMotion,
    model: Model,
    gpu: gpu::Gpu,
    text: draw::Text,
    main: Option<Surface>,
    rail: Spring,
    mods: ModifiersState,
    started: Instant,
    last: Instant,
    max_scroll: f64,
    touch: Option<u64>,
    drag: Option<Action>,
    last_title_click: Option<Instant>,
    title_press: Option<(Instant, (f64, f64))>,
}
impl App {
    fn new(model: Model) -> Self {
        let mut color_motion = dotisle_domain::dynamic_color::RoleMotion::default();
        color_motion.update(model.dynamic_color().roles, 0., false);
        Self {
            frame_trace: crate::frame_trace::Trace::from_args(model.demo),
            overlay_motion: Default::default(),
            signature_at: None,
            desktop: Default::default(),
            scroll_motion: Spring::new(0.),
            scroll_target: 0.,
            scroll_page: model.page,
            scroll_blur: 0.,
            scroll_input: Instant::now(),
            deck_velocity: 0.,
            deck_drag: None,
            native_pid: None,
            warm: false,
            switches: Default::default(),
            color_motion,
            interactions: vec![],
            presses: vec![],
            selections: Default::default(),
            motion_targets: vec![],
            page_enter: Spring::new(1.),
            seek_preview: None,
            seek_track: None,
            timeline: Default::default(),
            lyrics: Default::default(),
            model,
            gpu: gpu::Gpu::new(),
            text: draw::Text::new_lazy_japanese(),
            main: None,
            rail: Spring::new(522.),
            mods: ModifiersState::empty(),
            started: Instant::now(),
            last: Instant::now(),
            max_scroll: 0.,
            touch: None,
            drag: None,
            last_title_click: None,
            title_press: None,
        }
    }
    fn surface(&mut self, event: &ActiveEventLoop) -> Result<Surface, String> {
        self.startup_phase("creating-window");
        let icon = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.join("Assets/LyricIsland-v3.ico")))
            .and_then(|p| winit::window::Icon::from_path(p, Some(PhysicalSize::new(64, 64))).ok());
        let attrs = Window::default_attributes()
            .with_title("点屿 V3 · M3 Expressive")
            .with_window_icon(icon)
            .with_inner_size(LogicalSize::new(1200., 800.))
            .with_min_inner_size(LogicalSize::new(900., 680.))
            .with_decorations(false)
            .with_resizable(crate::themes::slot(&self.model) == 0)
            .with_transparent(true)
            .with_visible(false);
        let window = Arc::new(event.create_window(attrs).map_err(|e| e.to_string())?);
        window.set_ime_allowed(true);
        platform::backdrop(&window);
        platform::set_theme_icon(&window, crate::themes::slot(&self.model));
        self.startup_phase("creating-surface");
        let mut surface = self.gpu.surface(window.clone())?;
        self.startup_phase("building-first-frame");
        // Hidden Windows HWNDs do not receive WM_PAINT. Render synchronously once,
        // then expose the completed composition surface and enter the normal loop.
        self.model.tick();
        self.color_motion.update(
            self.model.dynamic_color().roles,
            self.started.elapsed().as_secs_f64(),
            false,
        );
        let size = window.inner_size();
        let scale = window.scale_factor();
        let initial = views::ViewState {
            rail_y: 112.
                + Page::ALL
                    .iter()
                    .position(|p| *p == self.model.page)
                    .unwrap_or(0) as f64
                    * 82.,
            color_scheme: Some(self.model.scheme()),
            ..Default::default()
        };
        self.rail.snap(initial.rail_y);
        let mut initial = initial;
        if !self.warm
            && self
                .frame_trace
                .as_ref()
                .is_none_or(|trace| trace.is_startup())
            && self.model.setting("StartupSignatureEnabled") != false
        {
            self.signature_at = Some(Instant::now());
            initial.signature_age = Some(0.);
        }
        let view = views::render(
            &self.model,
            &mut self.text,
            size.width as f64 / scale,
            size.height as f64 / scale,
            &initial,
        );
        self.max_scroll = view.max_scroll;
        self.motion_targets = view.motion_targets;
        for (key, target) in &self.motion_targets {
            self.selections
                .entry(key.clone())
                .or_insert_with(|| Spring::new(*target));
        }
        self.startup_phase("drawing-first-frame");
        self.gpu.draw(&mut surface, &view.scene, scale)?;
        self.startup_phase("showing-window");
        // The Japanese fallback (about 9.5 MB) is read after this first completed
        // frame, so its disk read never sits on the window's cold-start path.
        // Fallback fonts are registered by Text only when a glyph needs them.
        if let Some(dir) = std::env::var_os("DOTISLE_PROFILE_DIR") {
            let dir = std::path::PathBuf::from(dir);
            let _ = std::fs::create_dir_all(&dir);
            let _=std::fs::write(dir.join("settings-startup.json"),serde_json::json!({"firstCompleteFrameMs":crate::STARTED.get().map(|s|s.elapsed().as_secs_f64()*1000.),"mode":if self.model.demo{"preview"}else{"connected"},"whiteFrameExposed":false}).to_string());
        }
        if !self.warm {
            if self.signature_at.is_some() {
                self.signature_at = Some(Instant::now());
            }
            window.set_visible(true);
            window.request_redraw();
        }
        Ok(Surface {
            surface,
            window,
            hits: view.hits,
            pointer: (-1., -1.),
            focus: None,
            pressed: None,
            visible: !self.warm,
            occluded: false,
            taskbar: false,
            open_profile: false,
            hidden_since: if self.warm {
                Some(Instant::now())
            } else {
                None
            },
        })
    }
    fn startup_phase(&self, phase: &str) {
        if let Some(dir) = std::env::var_os("DOTISLE_PROFILE_DIR") {
            let path = std::path::PathBuf::from(dir);
            let _ = std::fs::create_dir_all(&path);
            let _ = std::fs::write(path.join("settings-phase.txt"), phase);
        }
    }
    fn error(&mut self, e: String) {
        lyric_backend::io::log(&self.model.data.join("Logs"), "V3_SETTINGS", &e);
        self.model.message(format!("绘制失败：{e}"));
    }
    fn action(&mut self, a: Action) {
        if let Action::OverviewCard(i) = &a {
            self.deck_to(
                crate::overview::ring::nearest(self.scroll_motion.value, *i),
                false,
            );
            return;
        }
        if matches!(a, Action::ControlTheme(_)) {
            let lock = SWITCH_START.get_or_init(|| std::sync::Mutex::new(None));
            if let Ok(mut g) = lock.lock() {
                *g = Some(Instant::now());
            }
        }
        // Capture the palette already on screen so the switch interpolates colour by colour
        // instead of repainting the whole window in one step.
        let theme_from = if matches!(a, Action::ControlTheme(_)) {
            Some((
                crate::themes::slot(&self.model),
                crate::themes::resolved(&self.model),
                self.model.prefs.dark,
            ))
        } else {
            None
        };
        if let Action::ReplaceModule(id)=a {
            if let Some(s)=&self.main{match platform::choose_module(&s.window){Ok(Some(path))=>self.model.native_command("moduleStage",serde_json::json!({"id":id,"path":path})),Ok(None)=>{},Err(e)=>self.model.message(&e)}}self.redraw();return;
        }
        if matches!(a, Action::BackgroundPick) {
            if let Some(s) = &self.main {
                match platform::choose_background(&s.window) {
                    Ok(Some(path)) => {
                        if let Err(e) = self.model.import_background(&path) {
                            self.model.background.error = Some(e);
                        }
                    }
                    Ok(None) => {}
                    Err(e) => self.model.background.error = Some(e),
                }
            }
            self.redraw();
            return;
        }
        match &a {
            Action::WindowMinimize => {
                if let Some(s) = &self.main {
                    s.window.set_minimized(true);
                }
                return;
            }
            Action::WindowMaximize => {
                if crate::themes::slot(&self.model) == 0 {
                    if let Some(s) = &self.main {
                        s.window.set_maximized(!s.window.is_maximized());
                    }
                    self.redraw();
                }
                return;
            }
            Action::WindowClose => {
                if let Some(s) = &mut self.main {
                    s.window.set_visible(false);
                    s.visible = false;
                    s.hidden_since = Some(Instant::now());
                }
                return;
            }
            _ => {}
        }
        let location = (
            self.model.page,
            self.model.settings_tab,
            self.model.island_tab,
        );
        let settings_navigation = matches!(
            a,
            Action::SettingsTab(_)
                | Action::IslandTab(_)
                | Action::IslandSettings(_)
                | Action::LayoutOpen
        );
        let reset = crate::tutorial::allowed(&a)
            || matches!(
                a,
                Action::Navigate(_)
                    | Action::SettingsTab(_)
                    | Action::SaveModal
                    | Action::CancelModal
                    | Action::EditTodo(_)
                    | Action::Paths
                    | Action::Palette
            );
        self.model.dispatch(a);
        if self.model.page == Page::Settings
            && (settings_navigation
                || location
                    != (
                        self.model.page,
                        self.model.settings_tab,
                        self.model.island_tab,
                    ))
        {
            self.page_enter.snap(1.);
            self.scroll_motion.snap(self.model.scroll);
            self.scroll_target = self.model.scroll;
            self.scroll_page = Page::Settings;
            self.scroll_blur = 0.;
        }
        if let Some((slot, from, dark)) = theme_from {
            if self.model.prefs.reduced_motion {
                crate::themes::end_transition();
            } else {
                crate::themes::begin_transition(slot, from, dark);
            }
            if let Some(s) = &self.main {
                s.window
                    .set_resizable(crate::themes::slot(&self.model) == 0);
                platform::set_theme_icon(&s.window, crate::themes::slot(&self.model));
            }
        }
        if reset {
            self.interactions.clear();
            self.presses.clear();
            if let Some(s) = &mut self.main {
                s.focus = None;
                s.pressed = None;
            }
        }
        self.redraw();
    }
    fn deck_to(&mut self, target: f64, immediate: bool) {
        self.scroll_target = target;
        self.scroll_motion.target = target;
        self.scroll_input = Instant::now();
        if immediate || self.model.prefs.reduced_motion {
            self.scroll_motion.snap(target);
            self.model.scroll = target;
        }
        if let Some(s) = &mut self.main {
            s.focus = None;
            s.pressed = None;
        }
        self.interactions.clear();
        self.presses.clear();
        self.redraw();
    }
    fn scroll_by(&mut self, dy: f64) {
        self.signature_at = None;
        if self.scroll_page != self.model.page {
            self.scroll_motion.snap(self.model.scroll);
            self.scroll_target = self.model.scroll;
            self.scroll_page = self.model.page;
        }
        let target = self.scroll_target - dy;
        if self.model.page == Page::Home {
            self.deck_to(target, false);
        } else {
            self.scroll_input = Instant::now();
            self.scroll_target = target.clamp(0., self.max_scroll);
            self.scroll_motion.target = self.scroll_target;
            if self.model.prefs.reduced_motion {
                self.scroll_motion.snap(self.scroll_target);
                self.model.scroll = self.scroll_target;
            }
        }
    }
    fn rebase_deck(&mut self) {
        let shift = crate::overview::ring::rebase_offset(self.scroll_motion.value);
        if shift == 0. {
            return;
        }
        self.scroll_motion.value -= shift;
        self.scroll_motion.target -= shift;
        self.scroll_target -= shift;
        self.model.scroll -= shift;
        if let Some((_, origin, _, last)) = &mut self.deck_drag {
            *origin -= shift;
            *last -= shift;
        }
    }
    fn redraw(&self) {
        if let Some(s) = &self.main {
            if s.visible && !s.occluded {
                s.window.request_redraw();
            }
        }
    }
    fn dump_hits(&self) {
        let Some(path) = hits_path() else { return };
        let Some(s) = &self.main else { return };
        let mut hits=s.hits.iter().map(|h|serde_json::json!({"label":h.label,"action":format!("{:?}",h.action),"rect":[h.rect.x0,h.rect.y0,h.rect.x1,h.rect.y1],"enabled":h.enabled})).collect::<Vec<_>>();
        hits.sort_by(|a, b| {
            a["action"]
                .as_str()
                .unwrap_or("")
                .cmp(b["action"].as_str().unwrap_or(""))
                .then(
                    a["rect"][0]
                        .as_f64()
                        .unwrap_or(0.)
                        .partial_cmp(&b["rect"][0].as_f64().unwrap_or(0.))
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
        });
        let dump = serde_json::json!({"scroll":self.model.scroll,"max_scroll":self.max_scroll,"tab":self.model.settings_tab,"theme":self.model.prefs.control_center_theme,"font_scope":self.model.prefs.font_scope,"hits":hits});
        let _ = std::fs::write(path, serde_json::to_vec_pretty(&dump).unwrap_or_default());
    }
    fn get(&mut self, _cloud: bool) -> Option<&mut Surface> {
        self.main.as_mut()
    }
    fn cancel_drag(&mut self) {
        if let Some(e) = &mut self.model.layout_editor {
            e.end_drag(true);
        }
        self.deck_drag = None;
        if let Some(s) = &self.main {
            platform::capture_pointer(&s.window, false);
        }
        self.drag = None;
        self.seek_preview = None;
        self.seek_track = None;
        self.model.preview_curve = None;
        self.model.preview_radius = None;
        self.model.preview_values.clear();
    }
    fn drag_to(&mut self, p: (f64, f64)) {
        if let Some((start, origin, last, last_position)) = self.deck_drag {
            let position = origin + (start - p.1) * 1.45;
            let velocity = (position - last_position) / last.elapsed().as_secs_f64().max(0.001);
            self.deck_drag = Some((start, origin, Instant::now(), position));
            self.scroll_motion.snap(position);
            self.deck_velocity = velocity;
            self.model.scroll = position;
            self.scroll_target = position;
            self.scroll_input = Instant::now();
            self.redraw();
            return;
        }
        if let Some(Action::LayoutElement(id, b) | Action::LayoutResize(id, b)) = self.drag.clone()
        {
            let resize = matches!(self.drag, Some(Action::LayoutResize(..)));
            let font = self.model.setting("FontSize").as_f64().unwrap_or(14.);
            if let Some(e) = &mut self.model.layout_editor {
                e.drag_to(id, b, p, resize, font);
            }
            self.redraw();
            return;
        }
        if let Some(Action::LayoutDrag(index)) = self.drag.clone() {
            if let Some(s) = &self.main {
                let target = s
                    .hits
                    .iter()
                    .filter_map(|h| {
                        if let Action::LayoutDrag(i) = h.action {
                            Some((i, h.rect.center().y))
                        } else {
                            None
                        }
                    })
                    .min_by(|a, b| (a.1 - p.1).abs().total_cmp(&(b.1 - p.1).abs()));
                if let Some((to, _)) = target {
                    if to != index {
                        if let Some(e) = &mut self.model.layout_editor {
                            e.draft.move_module(index, to);
                            e.draft.enabled = true;
                        }
                        self.drag = Some(Action::LayoutDrag(to));
                    }
                }
            }
            self.redraw();
            return;
        }
        if let Some(action) = &self.drag {
            match action {
                Action::Seek => {
                    if let Some(s) = &self.main {
                        if let Some(hit) = s.pressed.and_then(|i| s.hits.get(i)) {
                            let duration = self.model.snapshot["track"]["durationSeconds"]
                                .as_f64()
                                .unwrap_or(0.);
                            self.seek_preview = Some(
                                ((p.0 - hit.rect.x0) / hit.rect.width()).clamp(0., 1.) * duration,
                            );
                        }
                    }
                }
                Action::CurveHandle(i, b) => {
                    let mut c = self.model.preview_curve.unwrap_or_else(|| {
                        self.model
                            .curve()
                            .unwrap_or(lyric_core::motion_curve::EMPHASIZED)
                    });
                    let n = *i as usize * 2;
                    c[n] = ((p.0 - b[0]) / b[2]).clamp(0., 1.);
                    c[n + 1] = (1. - (p.1 - b[1]) / b[3]).clamp(0., 1.);
                    self.model.preview_curve = Some(c);
                }
                Action::RadiusHandle(b) => {
                    self.model.preview_radius = Some(
                        (((b[0] - p.0) + (p.1 - b[1])) / (2. * b[2]))
                            .clamp(
                                if self.model.radius_target == 0 {
                                    0.
                                } else {
                                    8.
                                },
                                48.,
                            )
                            .round(),
                    )
                }
                Action::ColorPad(b) => {
                    let s = ((p.0 - b[0]) / b[2]).clamp(0., 1.);
                    let v = (1. - (p.1 - b[1]) / b[3]).clamp(0., 1.);
                    self.model.color_pad(s, v);
                }
                Action::ColorHue(b) => self.model.color_hue((p.0 - b[0]) / b[2]),
                Action::RangeHandle(key, min, max, b) => {
                    let v = min + (max - min) * ((p.0 - b[0]) / b[2]).clamp(0., 1.);
                    let v = if *key == "FontSize" {
                        (v * 2.).round() / 2.
                    } else if *key == "SpringMass" {
                        (v * 100.).round() / 100.
                    } else {
                        v.round()
                    };
                    self.model.preview_values.insert(key, serde_json::json!(v));
                }
                Action::ThemeHandle(b) => {
                    let f = ((p.0 - b[0]) / b[2]).clamp(0., 1.);
                    self.model
                        .preview_values
                        .insert("ControlCenterThemeSlot", serde_json::json!(f));
                }
                _ => {}
            }
        }
        self.redraw();
    }
    fn resize_from_edge(&mut self) -> bool {
        if crate::themes::slot(&self.model) != 0 || self.touch.is_some() {
            return false;
        }
        let Some(s) = &self.main else { return false };
        if s.window.is_maximized() {
            return false;
        }
        let size = s.window.inner_size();
        let scale = s.window.scale_factor();
        let (w, h) = (size.width as f64 / scale, size.height as f64 / scale);
        let (x, y) = s.pointer;
        if x < 0. || y < 0. {
            return false;
        }
        let (left, right, top, bottom) = (x < 6., x > w - 6., y < 6., y > h - 6.);
        let direction = if top && left {
            Some(ResizeDirection::NorthWest)
        } else if top && right {
            Some(ResizeDirection::NorthEast)
        } else if bottom && left {
            Some(ResizeDirection::SouthWest)
        } else if bottom && right {
            Some(ResizeDirection::SouthEast)
        } else if left {
            Some(ResizeDirection::West)
        } else if right {
            Some(ResizeDirection::East)
        } else if top {
            Some(ResizeDirection::North)
        } else if bottom {
            Some(ResizeDirection::South)
        } else {
            None
        };
        if let Some(direction) = direction {
            let _ = s.window.drag_resize_window(direction);
            return true;
        }
        false
    }
    fn down(&mut self) {
        if self.signature_at.take().is_some() {
            self.redraw();
            return;
        }
        if self.resize_from_edge() {
            return;
        }

        let mut action = None;
        let mut p = (0., 0.);
        if let Some(s) = &mut self.main {
            s.focus = None;
            s.pressed = s
                .hits
                .iter()
                .rposition(|h| h.enabled && h.rect.contains(s.pointer));
            action = s
                .pressed
                .and_then(|i| s.hits.get(i))
                .map(|h| h.action.clone());
            p = s.pointer;
        }
        if self.model.tutorial.is_some() && action.is_none() {
            self.redraw();
            return;
        }
        if action.is_none() && p.1 >= 0. && p.1 < views::TITLEBAR_H {
            let now = Instant::now();
            let double = self
                .last_title_click
                .is_some_and(|t| now.duration_since(t) < Duration::from_millis(420));
            if double {
                self.last_title_click = None;
                if crate::themes::slot(&self.model) == 0 {
                    if let Some(s) = &self.main {
                        s.window.set_maximized(!s.window.is_maximized());
                    }
                }
                self.redraw();
                return;
            }
            self.last_title_click = Some(now);
            self.title_press = Some((now, p));
            if let Some(s) = &self.main {
                s.window.focus_window();
            }
            return;
        }
        if let Some(a) = action {
            if matches!(
                a,
                Action::LayoutElement(..) | Action::LayoutResize(..) | Action::LayoutDrag(_)
            ) {
                if let Some(e) = &mut self.model.layout_editor {
                    if e.pending.is_some() {
                        return;
                    }
                    let id = match a {
                        Action::LayoutElement(id, _) | Action::LayoutResize(id, _) => Some(id),
                        _ => None,
                    };
                    e.begin_drag(id);
                }
            }
            if matches!(a, Action::OverviewDrag) {
                self.deck_drag = Some((p.1, self.model.scroll, Instant::now(), self.model.scroll));
                if let Some(s) = &self.main {
                    platform::capture_pointer(&s.window, true);
                }
                self.redraw();
                return;
            }
            if matches!(
                a,
                Action::Seek
                    | Action::CurveHandle(_, _)
                    | Action::RadiusHandle(_)
                    | Action::RangeHandle(_, _, _, _)
                    | Action::ColorPad(_)
                    | Action::ColorHue(_)
                    | Action::ThemeHandle(_)
                    | Action::LayoutDrag(_)
                    | Action::LayoutElement(..)
                    | Action::LayoutResize(..)
            ) {
                if let Some(s) = &self.main {
                    platform::capture_pointer(&s.window, true);
                }
                if matches!(a, Action::Seek) {
                    self.seek_track = Some(self.model.snapshot["track"].clone());
                }
                self.drag = Some(a);
                self.drag_to(p);
            }
        }
        self.redraw();
    }
    fn up(&mut self) {
        if let Some(s) = &self.main {
            platform::capture_pointer(&s.window, false);
        }
        if self.deck_drag.take().is_some() {
            self.scroll_motion.set_velocity(self.deck_velocity);
            let projected = self.model.scroll + self.deck_velocity * 0.12;
            self.scroll_target = crate::overview::ring::snap(projected);
            self.scroll_motion.target = self.scroll_target;
            if let Some(s) = &mut self.main {
                s.pressed = None;
            }
            self.redraw();
            return;
        }
        if self.title_press.take().is_some() {
            self.redraw();
            return;
        }
        if let Some(drag) = self.drag.take() {
            if let Some(e) = &mut self.model.layout_editor {
                e.end_drag(false);
            }
            if matches!(drag, Action::ThemeHandle(_)) {
                if let Some(v) = self.model.preview_values.remove("ControlCenterThemeSlot") {
                    let slot = crate::themes::slot_from_fraction(v.as_f64().unwrap_or(1.));
                    self.action(Action::ControlTheme(slot));
                }
                if let Some(s) = &mut self.main {
                    s.pressed = None;
                }
                self.redraw();
                return;
            }
            if matches!(drag, Action::Seek) {
                if let Some(position) = self.seek_preview.take() {
                    let same = self.seek_track.take().is_some_and(|track| {
                        track["id"] == self.model.snapshot["track"]["id"]
                            && track["sourceApp"] == self.model.snapshot["track"]["sourceApp"]
                    });
                    if !same {
                        self.cancel_drag();
                        if let Some(s) = &mut self.main {
                            s.pressed = None;
                        }
                        self.model.message("歌曲已切换，已取消这次进度拖动");
                        self.redraw();
                        return;
                    }
                    let duration = self.model.snapshot["track"]["durationSeconds"]
                        .as_f64()
                        .unwrap_or(0.);
                    if duration > 0. {
                        self.timeline.seek(position);
                        self.model.seek(position / duration);
                    }
                }
            }
            let update = match drag {
                Action::CurveHandle(_, _) => self
                    .model
                    .preview_curve
                    .take()
                    .map(|c| ("MotionCurve", serde_json::json!(c))),
                Action::RadiusHandle(_) => self.model.preview_radius.take().map(|r| {
                    (
                        if self.model.radius_target == 0 {
                            "CapsuleCornerRadius"
                        } else {
                            "MotionRadius"
                        },
                        serde_json::json!(r),
                    )
                }),
                Action::RangeHandle(key, _, _, _) => {
                    self.model.preview_values.remove(key).map(|v| (key, v))
                }
                _ => None,
            };
            if let Some((key, value)) = update {
                if key.starts_with("Background") {
                    self.action(Action::BackgroundValue(key, value.as_f64().unwrap_or(0.)));
                } else {
                    self.action(Action::NativeEdit(key, value));
                }
            }
            if let Some(s) = &mut self.main {
                s.pressed = None;
            }
            self.redraw();
            return;
        }
        let action = self.main.as_mut().and_then(|s| {
            let p = s.pointer;
            s.pressed
                .take()
                .and_then(|i| s.hits.get(i))
                .filter(|h| h.enabled && h.rect.contains(p))
                .map(|h| (h.action.clone(), h.rect, p))
        });
        if let Some((a, r, p)) = action {
            if let Action::ThemeHandle(b) = &a {
                let slot = crate::themes::slot_from_fraction(((p.0 - b[0]) / b[2]).clamp(0., 1.));
                self.action(Action::ControlTheme(slot));
            } else if matches!(a, Action::Seek) {
                self.model.seek((p.0 - r.x0) / r.width());
            } else {
                if matches!(a, Action::Navigate(_)) && !self.model.prefs.reduced_motion {
                    if self.page_enter.settled() {
                        self.page_enter.snap(0.);
                    }
                    self.page_enter.target = 1.;
                    self.interactions.clear();
                    self.presses.clear();
                }
                self.action(a);
            }
        }
        self.redraw();
    }
    fn key(&mut self, cloud: bool, key: Key, text: Option<String>) {
        self.key_impl(cloud, key, text);
        self.overlay_motion.snap(&self.model);
    }
    fn key_impl(&mut self, cloud: bool, key: Key, text: Option<String>) {
        self.signature_at = None;
        if key == Key::Named(NamedKey::F5) {
            self.dump_hits();
            return;
        }
        if self.model.tutorial.is_some() {
            if key == Key::Named(NamedKey::Tab) {
                let reverse = self.mods.shift_key();
                if let Some(s) = &mut self.main {
                    let enabled = s
                        .hits
                        .iter()
                        .enumerate()
                        .filter(|(_, h)| h.enabled && crate::tutorial::allowed(&h.action))
                        .map(|(i, _)| i)
                        .collect::<Vec<_>>();
                    if !enabled.is_empty() {
                        let pos = s.focus.and_then(|i| enabled.iter().position(|n| *n == i));
                        let next = if reverse {
                            pos.map(|p| (p + enabled.len() - 1) % enabled.len())
                                .unwrap_or(enabled.len() - 1)
                        } else {
                            pos.map(|p| (p + 1) % enabled.len()).unwrap_or(0)
                        };
                        s.focus = Some(enabled[next]);
                    }
                }
                self.redraw();
                return;
            }
            let action = match key {
                Key::Named(NamedKey::Escape) => Some(Action::TutorialFinish),
                Key::Named(NamedKey::ArrowLeft) => Some(Action::TutorialBack),
                Key::Named(NamedKey::ArrowRight) => Some(Action::TutorialNext),
                Key::Named(NamedKey::Enter) | Key::Named(NamedKey::Space) => Some(
                    self.main
                        .as_ref()
                        .and_then(|s| s.focus.and_then(|i| s.hits.get(i)))
                        .filter(|h| h.enabled && crate::tutorial::allowed(&h.action))
                        .map(|h| h.action.clone())
                        .unwrap_or(Action::TutorialNext),
                ),
                _ => None,
            };
            if let Some(action) = action {
                self.action(action);
            }
            return;
        }
        if self.model.picker.is_some() {
            if key == Key::Named(NamedKey::Escape) {
                self.cancel_drag();
                self.model.dispatch(Action::ColorClose);
                self.redraw();
                return;
            }
            if key == Key::Named(NamedKey::Enter) {
                self.cancel_drag();
                self.model.dispatch(Action::ColorSave);
                self.redraw();
                return;
            }
            if self.mods.control_key() {
                if let Key::Character(c) = &key {
                    if c.to_lowercase().as_str() == "v" {
                        if let Some(text) = platform::clipboard() {
                            self.model.picker_input(&text);
                        }
                        self.redraw();
                        return;
                    }
                }
            }
            if key == Key::Named(NamedKey::Backspace) {
                self.model.picker_backspace();
                self.redraw();
                return;
            }
            if let Key::Named(n) = &key {
                let name = match n {
                    NamedKey::ArrowLeft => Some("Left"),
                    NamedKey::ArrowRight => Some("Right"),
                    NamedKey::Home => Some("Home"),
                    NamedKey::End => Some("End"),
                    NamedKey::Delete => Some("Delete"),
                    _ => None,
                };
                if let Some(name) = name {
                    self.model.picker_edit_key(name);
                    self.redraw();
                    return;
                }
            }
            if let Some(text) = text {
                self.model.picker_input(&text);
                self.redraw();
            }
            return;
        }
        if self.model.font_picker.is_some() {
            if key == Key::Named(NamedKey::Escape) {
                self.model.dispatch(Action::FontClose);
                self.redraw();
                return;
            }
            if key == Key::Named(NamedKey::Enter) {
                self.model.dispatch(Action::FontConfirm);
                self.redraw();
                return;
            }
            if self.mods.control_key() {
                if let Key::Character(c) = &key {
                    if c.to_lowercase().as_str() == "v" {
                        if let Some(text) = platform::clipboard() {
                            self.model.font_input(&text);
                        }
                        self.redraw();
                        return;
                    }
                }
            }
            if key == Key::Named(NamedKey::Backspace) {
                self.model.font_backspace();
                self.redraw();
                return;
            }
            if let Key::Named(n) = &key {
                let delta = match n {
                    NamedKey::ArrowUp => -1,
                    NamedKey::ArrowDown => 1,
                    _ => 0,
                };
                if delta != 0 {
                    self.model.font_move(delta);
                    self.redraw();
                    return;
                }
            }
            if let Some(text) = text {
                self.model.font_input(&text);
                self.redraw();
            }
            return;
        }
        if key == Key::Named(NamedKey::Escape) && self.drag.is_some() {
            self.cancel_drag();
            self.redraw();
            return;
        }
        if key == Key::Named(NamedKey::Escape) {
            if self.model.dialog.is_some() {
                self.action(Action::CancelModal);
            } else if self.model.palette {
                self.action(Action::Palette);
            } else if cloud {
                self.model.cloud_expanded = false;
            }
            return;
        }
        if self.model.page == Page::Settings
            && self.model.settings_tab == 135
            && self.model.dialog.is_none()
        {
            if self.mods.control_key() {
                if let Key::Character(c) = &key {
                    let a = match c.to_lowercase().as_str() {
                        "z" => Some(if self.mods.shift_key() {
                            Action::LayoutRedo
                        } else {
                            Action::LayoutUndo
                        }),
                        "y" => Some(Action::LayoutRedo),
                        _ => None,
                    };
                    if let Some(a) = a {
                        self.action(a);
                        return;
                    }
                }
            }
            if self
                .model
                .layout_editor
                .as_ref()
                .is_some_and(|e| e.selected.is_some())
            {
                let step = if self.mods.shift_key() { 10 } else { 1 };
                let delta = match &key {
                    Key::Named(NamedKey::ArrowLeft) => (-step, 0),
                    Key::Named(NamedKey::ArrowRight) => (step, 0),
                    Key::Named(NamedKey::ArrowUp) => (0, -step),
                    Key::Named(NamedKey::ArrowDown) => (0, step),
                    _ => (0, 0),
                };
                if delta != (0, 0) {
                    self.action(Action::LayoutNudge(delta.0, delta.1));
                    return;
                }
            }
        }
        if self.mods.control_key() {
            if let Key::Character(c) = &key {
                match c.to_lowercase().as_str() {
                    "k" => {
                        self.action(Action::Palette);
                        return;
                    }
                    "v" => {
                        if let Some(text) = platform::clipboard() {
                            self.model.input(&text);
                        }
                        self.redraw();
                        return;
                    }
                    "a" => {
                        self.model.select_all();
                        self.redraw();
                        return;
                    }
                    "1" | "2" | "3" | "4" | "5" | "6" => {
                        let i = c.parse::<usize>().unwrap_or(1) - 1;
                        self.rail.snap(112. + i as f64 * 82.);
                        self.action(Action::Navigate(Page::ALL[i]));
                        return;
                    }
                    _ => {}
                }
            }
        }
        if self.model.page == Page::Home && self.model.dialog.is_none() && !self.model.palette {
            let delta = match key {
                Key::Named(NamedKey::ArrowDown | NamedKey::PageDown) => 1.,
                Key::Named(NamedKey::ArrowUp | NamedKey::PageUp) => -1.,
                _ => 0.,
            };
            if delta != 0. {
                self.deck_to(
                    crate::overview::ring::snap(self.scroll_target) + delta * crate::overview::STEP,
                    true,
                );
                return;
            }
        }
        if !cloud
            && self.model.page == Page::Settings
            && self.model.settings_tab == 0
            && self.model.dialog.is_none()
        {
            if let Key::Named(NamedKey::ArrowLeft | NamedKey::ArrowRight) = key {
                let cur = crate::themes::slot(&self.model);
                let next = if matches!(key, Key::Named(NamedKey::ArrowRight)) {
                    (cur + 1).min((crate::themes::SLOTS.len() - 1) as u8)
                } else {
                    cur.saturating_sub(1)
                };
                self.action(Action::ControlTheme(next));
                return;
            }
        }
        if key == Key::Named(NamedKey::Tab) {
            if let Some(d) = &mut self.model.dialog {
                if !d.fields.is_empty() {
                    let n = d.fields.len();
                    d.active = (d.active + if self.mods.shift_key() { n - 1 } else { 1 }) % n;
                }
                self.redraw();
                return;
            }
            let reverse = self.mods.shift_key();
            if let Some(s) = self.get(cloud) {
                let enabled = s
                    .hits
                    .iter()
                    .enumerate()
                    .filter(|(_, h)| h.enabled)
                    .map(|(i, _)| i)
                    .collect::<Vec<_>>();
                if !enabled.is_empty() {
                    let pos = s.focus.and_then(|i| enabled.iter().position(|n| *n == i));
                    let next = if reverse {
                        pos.map(|p| (p + enabled.len() - 1) % enabled.len())
                            .unwrap_or(enabled.len() - 1)
                    } else {
                        pos.map(|p| (p + 1) % enabled.len()).unwrap_or(0)
                    };
                    s.focus = Some(enabled[next]);
                }
            }
            self.redraw();
            return;
        }
        if let Key::Named(n) = &key {
            let name = match n {
                NamedKey::ArrowLeft => Some("Left"),
                NamedKey::ArrowRight => Some("Right"),
                NamedKey::Home => Some("Home"),
                NamedKey::End => Some("End"),
                NamedKey::Delete => Some("Delete"),
                _ => None,
            };
            if let Some(name) = name {
                if self.model.dialog.is_some() {
                    self.model.edit_key(name);
                    self.redraw();
                    return;
                }
            }
        }
        if key == Key::Named(NamedKey::Backspace) {
            self.model.backspace();
            self.redraw();
            return;
        }
        if key == Key::Named(NamedKey::Enter) || key == Key::Named(NamedKey::Space) {
            if self.model.dialog.is_some() {
                if key == Key::Named(NamedKey::Enter) {
                    self.action(Action::SaveModal);
                } else {
                    self.model.input(" ");
                }
                return;
            }
            if self.model.palette {
                if let Some(page) = Page::ALL
                    .into_iter()
                    .find(|p| self.model.query.is_empty() || p.label().contains(&self.model.query))
                {
                    self.action(Action::Navigate(page));
                }
                return;
            }
            let action = self
                .get(cloud)
                .and_then(|s| s.focus.and_then(|i| s.hits.get(i)))
                .filter(|h| h.enabled)
                .map(|h| h.action.clone());
            if let Some(a) = action {
                if let Action::ThemeHandle(b) = a {
                    let x = self
                        .main
                        .as_ref()
                        .and_then(|s| s.focus.and_then(|i| s.hits.get(i)))
                        .map(|h| h.rect.center().x)
                        .unwrap_or(b[0]);
                    self.action(Action::ControlTheme(crate::themes::slot_from_fraction(
                        ((x - b[0]) / b[2]).clamp(0., 1.),
                    )));
                } else {
                    self.action(a);
                }
            } else if key == Key::Named(NamedKey::Space) && self.model.page == Page::Music {
                self.action(Action::Media("toggle"));
            }
            return;
        }
        if !self.mods.control_key() && !self.mods.alt_key() {
            if let Some(text) = text {
                self.model.input(&text);
                self.redraw();
            }
        }
    }
}
impl ApplicationHandler for App {
    fn resumed(&mut self, e: &ActiveEventLoop) {
        if self.main.is_none() {
            match self.surface(e) {
                Ok(s) => self.main = Some(s),
                Err(err) => {
                    self.error(err);
                    e.exit();
                }
            }
        }
    }
    fn window_event(&mut self, e: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        if self.desktop.contains(id) {
            if let Some(action) = self.desktop.event(
                id,
                event,
                &mut self.model,
                &mut self.gpu,
                &mut self.text,
                self.started.elapsed().as_secs_f64(),
            ) {
                self.action(action);
            }
            return;
        }
        if !self.main.as_ref().is_some_and(|s| s.window.id() == id) {
            return;
        }
        match event {
            WindowEvent::CloseRequested => {
                if let Some(s) = &mut self.main {
                    s.window.set_visible(false);
                    s.visible = false;
                    s.hidden_since = Some(Instant::now());
                }
            }
            WindowEvent::Resized(size) => {
                if size.width > 0 && size.height > 0 {
                    if let Some(s) = &mut self.main {
                        s.surface.resize(size.width, size.height);
                        s.window.request_redraw();
                    }
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => self.redraw(),
            WindowEvent::Occluded(value) => {
                if let Some(s) = &mut self.main {
                    s.occluded = value;
                }
                if !value {
                    self.redraw();
                }
            }
            WindowEvent::Focused(false) => {
                self.cancel_drag();
                if let Some(s) = &mut self.main {
                    s.pressed = None;
                }
                self.touch = None;
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let Some(s) = &mut self.main {
                    let scale = s.window.scale_factor();
                    s.pointer = (position.x / scale, position.y / scale);
                    s.window.set_ime_cursor_area(
                        LogicalPosition::new(s.pointer.0, s.pointer.1),
                        LogicalSize::new(1., 24.),
                    );
                    s.window.set_cursor(
                        if s.hits
                            .iter()
                            .any(|h| h.enabled && h.rect.contains(s.pointer))
                        {
                            winit::window::CursorIcon::Pointer
                        } else {
                            winit::window::CursorIcon::Default
                        },
                    );
                    s.window.request_redraw();
                }
                if let Some((_, origin)) = self.title_press {
                    let p = self.main.as_ref().map(|s| s.pointer).unwrap_or_default();
                    if (p.0 - origin.0).abs().max((p.1 - origin.1).abs()) > 4. {
                        self.title_press = None;
                        self.last_title_click = None;
                        if let Some(s) = &self.main {
                            if s.window.is_maximized() {
                                s.window.set_maximized(false);
                            }
                            let _ = s.window.drag_window();
                        }
                        return;
                    }
                }
                if let Some(s) = &self.main {
                    let p = s.pointer;
                    if self.drag.is_some() || self.deck_drag.is_some() {
                        self.drag_to(p);
                    }
                }
            }
            WindowEvent::CursorLeft { .. } => {
                if self.drag.is_none() && self.deck_drag.is_none() {
                    if let Some(s) = &mut self.main {
                        s.pointer = (-1., -1.);
                        s.window.request_redraw();
                    }
                }
            }
            WindowEvent::MouseInput {
                state: ElementState::Pressed,
                button: MouseButton::Right,
                ..
            } => {
                if self.model.tutorial.is_some() {
                    let action = self
                        .main
                        .as_ref()
                        .and_then(|s| {
                            s.hits.iter().rfind(|h| {
                                h.enabled
                                    && h.rect.contains(s.pointer)
                                    && matches!(h.action, Action::TutorialDemo(1))
                            })
                        })
                        .map(|h| h.action.clone());
                    if let Some(action) = action {
                        self.action(action);
                    }
                }
            }
            WindowEvent::MouseInput {
                state,
                button: MouseButton::Left,
                ..
            } => {
                if state == ElementState::Pressed {
                    self.down()
                } else {
                    self.up()
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.model.tutorial.is_none()
                    && self.drag.is_none()
                    && self.deck_drag.is_none()
                    && self.model.dialog.is_none()
                    && !self.model.palette
                {
                    let home = self.model.page == Page::Home && self.model.font_picker.is_none();
                    let dy = match delta {
                        MouseScrollDelta::LineDelta(_, y) => {
                            y as f64 * if home { crate::overview::STEP } else { 48. }
                        }
                        MouseScrollDelta::PixelDelta(p) => {
                            p.y / self
                                .main
                                .as_ref()
                                .map(|s| s.window.scale_factor())
                                .unwrap_or(1.)
                                * if home { 1.45 } else { 1. }
                        }
                    };
                    if self.model.font_picker.is_some() {
                        self.model.font_scroll(-dy / 34.);
                    } else {
                        self.scroll_by(dy);
                    }
                    self.redraw();
                }
            }
            WindowEvent::ModifiersChanged(mods) => self.mods = mods.state(),
            WindowEvent::KeyboardInput { event: key, .. } => {
                if key.state == ElementState::Pressed {
                    self.key(false, key.logical_key, key.text.map(|s| s.to_string()));
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                if self.model.font_picker.is_some() {
                    self.model.font_input(&text);
                } else {
                    self.model.input(&text);
                }
                self.redraw();
            }
            WindowEvent::Ime(Ime::Preedit(text, _)) => {
                if let Some(d) = &mut self.model.dialog {
                    d.preedit = text;
                }
                self.redraw();
            }
            WindowEvent::Touch(t) => {
                if let Some(s) = &mut self.main {
                    let scale = s.window.scale_factor();
                    s.pointer = (t.location.x / scale, t.location.y / scale);
                }
                match t.phase {
                    TouchPhase::Started => {
                        if self.touch.is_none() {
                            self.touch = Some(t.id);
                            self.down();
                        }
                    }
                    TouchPhase::Ended => {
                        if self.touch == Some(t.id) {
                            self.touch = None;
                            self.up();
                        }
                    }
                    TouchPhase::Moved => {
                        if self.touch == Some(t.id) {
                            if let Some(s) = &self.main {
                                self.drag_to(s.pointer);
                            }
                        }
                    }
                    TouchPhase::Cancelled => {
                        self.cancel_drag();
                        self.touch = None;
                        if let Some(s) = &mut self.main {
                            s.pressed = None;
                        }
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let frame_at = Instant::now();
                let state = {
                    let Some(s) = &self.main else { return };
                    let palette = self
                        .color_motion
                        .sample(self.started.elapsed().as_secs_f64())
                        .ui();
                    views::ViewState {
                        signature_age: self.signature_at.map(|at| at.elapsed().as_secs_f64()),
                        overlay_progress: Some(self.overlay_motion.value()),
                        closing_overlay: self.overlay_motion.closing_scene.clone(),
                        scroll_blur: self.scroll_blur,
                        switches: self
                            .switches
                            .iter()
                            .map(|(k, v)| (k.clone(), v.value))
                            .collect(),
                        color_scheme: Some(palette),
                        state_layers: self.interactions.iter().map(|v| v.value as f32).collect(),
                        page_enter: Some(self.page_enter.value),
                        selection_values: self
                            .selections
                            .iter()
                            .map(|(k, v)| (k.clone(), v.value))
                            .collect(),
                        press_layers: self.presses.iter().map(|v| v.value).collect(),
                        rail_target: Some(self.rail.target),
                        pointer: s.pointer,
                        focus: s.focus,
                        pressed: s.pressed,
                        media_position: self.seek_preview.or(Some(self.timeline.position)),
                        lyric_lines: Some(self.lyrics.presentation()),
                        rail_y: self.rail.value,
                        time: self.started.elapsed().as_secs_f64(),
                        maximized: s.window.is_maximized(),
                        ..Default::default()
                    }
                };
                let s = self.main.as_ref().unwrap();
                let size = s.window.inner_size();
                let scale = s.window.scale_factor();
                let view = views::render(
                    &self.model,
                    &mut self.text,
                    size.width as f64 / scale,
                    size.height as f64 / scale,
                    &state,
                );
                let s = self.main.as_mut().unwrap();
                if let Some(overlay) = view.overlay_scene {
                    self.overlay_motion.closing_scene = Some(overlay);
                }
                s.hits = view.hits;
                self.motion_targets = view.motion_targets;
                self.max_scroll = view.max_scroll;
                if self.model.page != Page::Home {
                    self.model.scroll = self.model.scroll.min(self.max_scroll);
                }
                let built_at = Instant::now();
                let drawn = self.gpu.draw(&mut s.surface, &view.scene, scale);
                if let Some(trace) = &mut self.frame_trace {
                    trace.record(
                        frame_at,
                        built_at.duration_since(frame_at).as_secs_f64() * 1000.,
                        built_at.elapsed().as_secs_f64() * 1000.,
                        self.model.scroll,
                        (size.width, size.height),
                        scale,
                        drawn.is_ok(),
                        s.surface.draw_timings(),
                        state.signature_age,
                    );
                }
                if drawn.is_ok() {
                    if let (Some(path), Some(lock)) = (switch_path(), SWITCH_START.get()) {
                        if let Ok(mut g) = lock.lock() {
                            if let Some(t) = g.take() {
                                append_line(
                                    path,
                                    &format!("{:.3}", t.elapsed().as_secs_f64() * 1000.),
                                );
                            }
                        }
                    }
                    if let Some(path) = fps_path() {
                        let lock =
                            FRAME_TICK.get_or_init(|| std::sync::Mutex::new((Instant::now(), 0)));
                        if let Ok(mut g) = lock.lock() {
                            g.1 += 1;
                            let ms = g.0.elapsed().as_secs_f64() * 1000.;
                            if ms >= 1000. {
                                append_line(path,&serde_json::json!({"frames":g.1,"elapsed_ms":(ms*10.).round()/10.}).to_string());
                                *g = (Instant::now(), 0);
                            }
                        }
                    }
                }
                if s.open_profile && drawn.is_ok() {
                    s.open_profile = false;
                    if let Some(dir) = std::env::var_os("DOTISLE_PROFILE_DIR") {
                        let dir = std::path::PathBuf::from(dir);
                        let _=std::fs::write(dir.join("settings-visible.json"),serde_json::json!({"processId":std::process::id(),"processElapsedMs":crate::STARTED.get().map(|v|v.elapsed().as_secs_f64()*1000.),"unixMs":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()}).to_string());
                    }
                }
                let refresh = !s.taskbar && s.visible;
                if refresh {
                    s.taskbar = true;
                }
                let window = if refresh {
                    Some(s.window.clone())
                } else {
                    None
                };
                if let Some(window) = window {
                    window.set_visible(true);
                    platform::refresh_taskbar(&window);
                }
                if let Err(err) = drawn {
                    self.error(err);
                }
            }
            _ => {}
        }
        if self.model.exit {
            self.model.shutdown();
            e.exit();
        }
    }
    fn about_to_wait(&mut self, e: &ActiveEventLoop) {
        let now = Instant::now();
        // A closed settings window used to keep this process — and its whole wgpu +
        // Vello context — resident until the island exited. After a short hidden grace period
        // the process retires instead; the island spawns a fresh one on demand.
        if self.desktop.is_empty()
            && self.model.desktop_widgets.items.is_empty()
            && self.main.as_ref().is_some_and(|s| {
                !s.visible
                    && s.hidden_since
                        .is_some_and(|at| now.duration_since(at) >= Duration::from_secs(10))
            })
        {
            self.model.shutdown();
            e.exit();
            return;
        }
        let dt = now.duration_since(self.last).as_secs_f64();
        self.last = now;
        if let Some(trace) = &self.frame_trace {
            if trace.elapsed() >= trace.duration() {
                if let Err(error) = trace.finish() {
                    self.error(error);
                }
                self.model.shutdown();
                e.exit();
                return;
            }
            if !trace.is_startup() {
                if trace.is_interactive() {
                    let index = (trace.elapsed() / 0.45) as usize % Page::ALL.len();
                    self.model.page = Page::ALL[index];
                    self.model.settings_tab = 255;
                }
                self.scroll_target = if self.model.page == Page::Home {
                    trace.target()
                } else {
                    0.
                };
                self.scroll_input = now;
            }
        }
        let changed = self.model.tick();
        let signature_moving = if let Some(at) = self.signature_at {
            if at.elapsed().as_secs_f64()
                >= dotisle_render::brand_signature::duration(self.model.prefs.reduced_motion)
            {
                self.signature_at = None;
            }
            true
        } else {
            false
        };
        if let Some(pid) = self.model.native_state["processId"].as_u64() {
            self.native_pid = Some(pid as u32);
        }
        if !self.model.demo
            && self
                .native_pid
                .is_some_and(|pid| !platform::process_alive(pid))
        {
            self.model.shutdown();
            e.exit();
            return;
        }
        self.desktop
            .sync(e, &mut self.model, &mut self.gpu, changed);
        if self.scroll_page != self.model.page
            || (self.model.scroll - self.scroll_motion.value).abs() > 0.5
        {
            self.scroll_motion.snap(self.model.scroll);
            self.scroll_target = self.model.scroll;
            self.scroll_page = self.model.page;
        }
        if self.model.page == Page::Home
            && self.deck_drag.is_none()
            && self.scroll_input.elapsed() > Duration::from_millis(160)
        {
            self.scroll_target = crate::overview::ring::snap(self.scroll_target);
        }
        if self.model.page == Page::Home {
            self.rebase_deck();
        } else {
            self.scroll_target = self.scroll_target.clamp(0., self.max_scroll);
        }
        self.scroll_motion.target = self.scroll_target;
        let before = self.scroll_motion.value;
        let scroll_moving = self.scroll_motion.tick(dt, self.model.prefs.reduced_motion);
        self.model.scroll = if self.model.page == Page::Home {
            self.scroll_motion.value
        } else {
            self.scroll_motion.value.max(0.)
        };
        let velocity = (self.scroll_motion.value - before).abs() / dt.max(0.001);
        self.scroll_blur = if self.model.prefs.reduced_motion || !scroll_moving {
            0.
        } else {
            (velocity / 1800.).min(1.5)
        };
        let visible = self.main.as_ref().is_some_and(|s| s.visible && !s.occluded);
        let resolved = self.model.dynamic_color();
        let colors_moving = self.color_motion.update(
            resolved.roles,
            self.started.elapsed().as_secs_f64(),
            !self.model.prefs.reduced_motion,
        );
        let mut interaction_moving = false;
        if visible {
            if let Some(surface) = &self.main {
                self.interactions
                    .resize_with(surface.hits.len(), || Spring::new(0.));
                self.presses
                    .resize_with(surface.hits.len(), || Spring::new(0.));
                for (i, hit) in surface.hits.iter().enumerate() {
                    let target = if !hit.enabled {
                        0.
                    } else if surface.pressed == Some(i) {
                        0.12
                    } else if surface.focus == Some(i) {
                        0.10
                    } else if hit.rect.contains(surface.pointer) {
                        0.08
                    } else {
                        0.
                    };
                    self.interactions[i].target = target;
                    interaction_moving |=
                        self.interactions[i].tick(dt, self.model.prefs.reduced_motion);
                    self.presses[i].target = if hit.enabled && surface.pressed == Some(i) {
                        1.
                    } else {
                        0.
                    };
                    interaction_moving |= self.presses[i].tick(dt, self.model.prefs.reduced_motion);
                }
            }
        }
        let lyric_moving = self.lyrics.tick(
            &self.model,
            dt,
            visible
                && self.model.page == Page::Home
                && crate::overview::ring::index(self.model.scroll) == 0,
        );
        let entering = self.page_enter.tick(dt, self.model.prefs.reduced_motion);
        interaction_moving |= crate::motion::advance_selections(
            &mut self.selections,
            &self.motion_targets,
            dt,
            self.model.prefs.reduced_motion,
        );
        let media_moving = if self.seek_preview.is_some() {
            false
        } else {
            self.timeline.tick(&self.model, dt)
        };
        self.rail.target = 112.
            + Page::ALL
                .iter()
                .position(|p| *p == self.model.page)
                .unwrap_or(0) as f64
                * 82.;
        let transition_moving = crate::themes::finish_transition();
        let rail_moving = self.rail.tick(dt, self.model.prefs.reduced_motion);
        let overlay_moving = self.overlay_motion.tick(&self.model, dt);
        let moving = overlay_moving
            || lyric_moving
            || signature_moving
            || scroll_moving
            || rail_moving
            || colors_moving
            || interaction_moving
            || entering
            || transition_moving;
        if self.model.show_center {
            self.model.show_center = false;
            let opening = self.main.as_ref().is_some_and(|s| !s.visible);
            if opening && self.model.setting("StartupSignatureEnabled") != false {
                self.signature_at = Some(Instant::now());
            }
            if let Some(s) = &mut self.main {
                s.window.set_visible(true);
                s.window.set_minimized(false);
                s.window.focus_window();
                s.open_profile = true;
                s.visible = true;
                s.hidden_since = None;
                s.window.request_redraw();
            }
        }
        let animated = visible
            && ((self.model.tutorial.as_ref().is_some_and(|t| t.demo == 0)
                && !self.model.prefs.reduced_motion)
                || (media_moving && matches!(self.model.page, Page::Home | Page::Music))
                || (self.model.page == Page::Island
                    && self.model.island_tab == 3
                    && !self.model.prefs.reduced_motion
                    && self.main.as_ref().is_some_and(|s| s.visible && !s.occluded)));
        if changed || moving || animated {
            self.redraw();
        }
        if self.model.exit {
            self.model.shutdown();
            e.exit();
            return;
        }
        // Let the vsynced swapchain pace active animation. An additional 16ms
        // timer can miss the next refresh after rendering and halve the cadence.
        let flow = if visible && (moving || animated) {
            ControlFlow::Poll
        } else {
            ControlFlow::WaitUntil(now + Duration::from_millis(120))
        };
        e.set_control_flow(flow);
    }
    fn exiting(&mut self, _: &ActiveEventLoop) {
        self.model.shutdown();
    }
}
pub fn run(model: Model) {
    platform::set_app_id();
    let event = EventLoop::new().expect("Windows event loop");
    let _ = WAKE.set(event.create_proxy());
    let mut app = App::new(model);
    app.warm = std::env::args().any(|a| a == "--warm");
    if let Err(e) = event.run_app(&mut app) {
        app.error(e.to_string());
    }
}

pub fn verify_tutorial_inputs(data: &std::path::Path) -> Result<(), String> {
    let mut model = Model::new(data.to_path_buf(), true);
    model.dispatch(Action::TutorialStart);
    let snapshot = model.snapshot.clone();
    let mut app = App::new(model);
    app.key(false, Key::Named(NamedKey::ArrowRight), None);
    if app.model.tutorial.as_ref().map(|t| t.index) != Some(1) {
        return Err("tutorial right key failed".into());
    }
    app.key(false, Key::Named(NamedKey::ArrowLeft), None);
    if app.model.tutorial.as_ref().map(|t| t.index) != Some(0) {
        return Err("tutorial left key failed".into());
    }
    app.mods = ModifiersState::CONTROL;
    app.key(false, Key::Character("k".into()), Some("k".into()));
    if app.model.palette {
        return Err("tutorial leaked a background shortcut".into());
    }
    app.mods = ModifiersState::empty();
    app.key(false, Key::Named(NamedKey::Space), None);
    if app.model.snapshot != snapshot {
        return Err("tutorial space operated the player".into());
    }
    app.key(false, Key::Named(NamedKey::Escape), None);
    if app.model.tutorial.is_some() || app.model.setting("GestureTutorialSeen") != true {
        return Err("tutorial escape did not save completion".into());
    }
    Ok(())
}
pub fn verify_motion_editor(data: &std::path::Path) -> Result<(), String> {
    let mut model = Model::new(data.to_path_buf(), true);
    model.page = Page::Island;
    model.island_tab = 3;
    let mut app = App::new(model);
    let view = views::render(
        &app.model,
        &mut app.text,
        1200.,
        800.,
        &views::ViewState::default(),
    );
    let curve = view
        .hits
        .iter()
        .find(|h| matches!(h.action, Action::CurveHandle(0, _)))
        .ok_or("curve handle is missing")?
        .action
        .clone();
    let Action::CurveHandle(_, b) = curve else {
        unreachable!()
    };
    app.drag = Some(curve);
    app.drag_to((b[0] + b[2] * 0.4, b[1] + b[3] * 0.8));
    app.up();
    let c = app.model.curve().ok_or("curve drag did not commit")?;
    if (c[0] - 0.4).abs() > 1e-6 || (c[1] - 0.2).abs() > 1e-6 {
        return Err("curve drag uses the wrong coordinate space".into());
    }
    let radius = view
        .hits
        .iter()
        .find(|h| matches!(h.action, Action::RadiusHandle(_)))
        .ok_or("radius handle is missing")?
        .action
        .clone();
    let Action::RadiusHandle(b) = radius else {
        unreachable!()
    };
    app.drag = Some(radius);
    app.drag_to((b[0] - 40. * b[2], b[1] + 40. * b[2]));
    app.up();
    if (app.model.radius() - 40.).abs() > 0.1 {
        return Err("radius drag did not reach native MotionRadius".into());
    }
    app.model.preview_curve = Some([0., 0., 1., 1.]);
    app.cancel_drag();
    if app.model.curve() != Some(c) {
        return Err("cancelled drag changed the saved curve".into());
    }
    Ok(())
}

/// Exercise settings slider endpoints through the production drag/commit path.
pub fn verify_settings_sliders(data: &std::path::Path) -> Result<(), String> {
    let mut app = App::new(Model::new(data.to_path_buf(), true));
    app.model.dispatch(Action::SettingsTab(128));
    app.model.tutorial_checked = true;
    app.model.tutorial = None;
    for (width, height) in [(900., 680.), (1200., 800.)] {
        let first = views::render(
            &app.model,
            &mut app.text,
            width,
            height,
            &Default::default(),
        );
        app.model.scroll = first.max_scroll * 0.5;
        let view = views::render(
            &app.model,
            &mut app.text,
            width,
            height,
            &Default::default(),
        );
        let action = view
            .hits
            .iter()
            .find_map(|h| match h.action {
                Action::RangeHandle("FontSize", ..) => Some(h.action.clone()),
                _ => None,
            })
            .ok_or("settings font slider missing")?;
        let Action::RangeHandle(key, min, max, bounds) = action.clone() else {
            unreachable!()
        };
        for (fraction, expected) in [(0., min), (0.5, ((min + max) * 1.).round() / 2.), (1., max)] {
            app.drag = Some(action.clone());
            app.drag_to((bounds[0] + bounds[2] * fraction, bounds[1] + bounds[3] / 2.));
            app.up();
            let value = app
                .model
                .setting(key)
                .as_f64()
                .ok_or("slider did not save")?;
            if (value - expected).abs() > 0.01 {
                return Err("settings slider thumb and pointer projection disagree".into());
            }
        }
        app.drag = Some(action.clone());
        app.drag_to((bounds[0] + bounds[2] * 0.2, bounds[1]));
        app.cancel_drag();
        if app.model.setting(key).as_f64() != Some(max) {
            return Err("cancelled slider drag changed saved setting".into());
        }
        app.model.scroll = 0.;
    }
    Ok(())
}

pub fn verify_seek_inputs(data: &std::path::Path) -> Result<(), String> {
    let mut model = Model::new(data.to_path_buf(), true);
    model.snapshot = serde_json::json!({"track":{"id":"netease-seek-a","title":"网易云进度适配","artist":"隔离演示","sourceApp":"Netease","durationSeconds":240.},"positionSeconds":48.,"isPlaying":false,"canSeek":true,"frame":{"currentLine":"播放进度由真实播放器确认"}});
    let mut app = App::new(model);
    for page in [Page::Home, Page::Music] {
        app.model.page = page;
        for width in [900., 1200.] {
            let view = views::render(
                &app.model,
                &mut app.text,
                width,
                800.,
                &views::ViewState::default(),
            );
            let hit = view
                .hits
                .iter()
                .find(|h| matches!(h.action, Action::Seek))
                .ok_or("NetEase seek slider missing")?;
            if !hit.enabled {
                return Err("NetEase native capability did not enable slider".into());
            }
            let p = hit.rect.x0 + hit.rect.width() * 0.73;
            let position = ((p - hit.rect.x0) / hit.rect.width()).clamp(0., 1.) * 240.;
            app.seek_track = Some(app.model.snapshot["track"].clone());
            app.seek_preview = Some(position);
            app.drag = Some(Action::Seek);
            app.up();
            if (app.model.snapshot["positionSeconds"].as_f64().unwrap_or(0.) - 175.2).abs() > 0.001
            {
                return Err("seek release did not use production Model::seek".into());
            }
        }
    }
    app.model.snapshot["positionSeconds"] = serde_json::json!(48.);
    app.seek_track = Some(app.model.snapshot["track"].clone());
    app.seek_preview = Some(180.);
    app.drag = Some(Action::Seek);
    app.model.snapshot["track"]["id"] = serde_json::json!("netease-seek-b");
    app.up();
    if app.model.snapshot["positionSeconds"] != 48. {
        return Err("old drag sought the new track".into());
    }
    app.model.snapshot["canSeek"] = serde_json::json!(false);
    app.model.seek(0.5);
    if app.model.snapshot["positionSeconds"] != 48. {
        return Err("unavailable seek changed position".into());
    }
    app.model.snapshot["canSeek"] = serde_json::json!(true);
    app.model.seek(f64::NAN);
    if app.model.snapshot["positionSeconds"] != 48. {
        return Err("invalid seek changed position".into());
    }
    Ok(())
}

/// Exercise the same drag/release/cancel handlers used by the native window.
pub fn verify_theme_inputs(out: &std::path::Path) -> Result<(), String> {
    let mut app = App::new(Model::new(out.to_path_buf(), true));
    app.model.page = Page::Settings;
    app.model.settings_tab = 0;
    for target in 0..crate::themes::SLOTS.len() as u8 {
        app.drag = Some(Action::ThemeHandle([100., 100., 600., 60.]));
        app.drag_to((
            100. + 600. * target as f64 / (crate::themes::SLOTS.len() - 1) as f64,
            100.,
        ));
        app.up();
        if crate::themes::slot(&app.model) != target {
            return Err(format!("theme drag/release failed for {target}"));
        }
    }
    app.drag = Some(Action::ThemeHandle([100., 100., 600., 60.]));
    app.drag_to((100., 100.));
    app.cancel_drag();
    app.up();
    if crate::themes::slot(&app.model) != (crate::themes::SLOTS.len() - 1) as u8 {
        return Err("cancelled theme drag committed".into());
    }
    app.key(false, Key::Named(NamedKey::ArrowLeft), None);
    if crate::themes::slot(&app.model) != (crate::themes::SLOTS.len() - 2) as u8 {
        return Err("left arrow failed".into());
    }
    app.key(false, Key::Named(NamedKey::ArrowRight), None);
    if crate::themes::slot(&app.model) != (crate::themes::SLOTS.len() - 1) as u8 {
        return Err("right arrow failed".into());
    }
    crate::themes::end_transition();
    Ok(())
}

pub fn verify_overview_inputs(data: &std::path::Path) -> Result<(), String> {
    use crate::overview::ring;
    let mut app = App::new(Model::new(data.to_path_buf(), true));
    app.model.prefs.reduced_motion = true;
    for _ in 0..17 {
        app.scroll_by(-ring::STEP);
    }
    if app.model.scroll != 17. * ring::STEP || ring::index(app.model.scroll) != 2 {
        return Err("wheel stopped at a deck endpoint".into());
    }
    for _ in 0..35 {
        app.scroll_by(ring::STEP);
    }
    if app.model.scroll != -18. * ring::STEP || ring::index(app.model.scroll) != 2 {
        return Err("reverse wheel stopped at a deck endpoint".into());
    }
    app.deck_to(0., true);
    app.key(false, Key::Named(NamedKey::ArrowUp), None);
    if app.model.scroll != -ring::STEP {
        return Err("keyboard did not wrap backwards".into());
    }
    app.action(Action::OverviewCard(0));
    if app.model.scroll != 0. {
        return Err("card selection did not choose the closest occurrence".into());
    }
    app.deck_to(4. * ring::STEP, true);
    app.key(false, Key::Named(NamedKey::ArrowDown), None);
    if app.model.scroll != ring::CYCLE {
        return Err("keyboard did not wrap forwards".into());
    }
    app.deck_drag = Some((200., app.model.scroll, Instant::now(), app.model.scroll));
    app.drag_to((20., -600.));
    if app.model.scroll <= ring::CYCLE {
        return Err("drag stopped at the seam".into());
    }
    app.up();
    if app.scroll_target <= ring::CYCLE {
        return Err("drag release snapped back to the last card".into());
    }
    app.deck_to(100. * ring::CYCLE + ring::STEP * 0.25, true);
    app.scroll_target += ring::STEP;
    app.scroll_motion.target = app.scroll_target;
    app.scroll_motion.set_velocity(620.);
    let mut expected = app.scroll_motion;
    let base = app.model.scroll;
    app.deck_drag = Some((200., base, Instant::now(), base));
    app.rebase_deck();
    let shift = base - app.model.scroll;
    if ring::index(base) != ring::index(app.model.scroll)
        || app
            .deck_drag
            .is_none_or(|(_, origin, _, last)| origin != base - shift || last != base - shift)
    {
        return Err("rebasing changed the card or drag origin".into());
    }
    app.scroll_motion.tick(1. / 60., false);
    expected.tick(1. / 60., false);
    if (expected.value - shift - app.scroll_motion.value).abs() > 1e-6 {
        return Err("rebasing dropped spring velocity".into());
    }
    app.model.page = Page::Settings;
    app.scroll_page = Page::Settings;
    app.model.scroll = 0.;
    app.scroll_motion.snap(0.);
    app.scroll_target = 0.;
    app.max_scroll = 480.;
    app.scroll_by(-2000.);
    if app.model.scroll != 480. {
        return Err("ordinary settings lost finite scrolling".into());
    }
    Ok(())
}

/// Settings destinations start at their own resting position, including interrupted scrolls.
pub fn verify_settings_navigation(data: &std::path::Path) -> Result<(), String> {
    let mut app = App::new(Model::new(data.to_path_buf(), true));
    app.model.prefs.reduced_motion = false;
    for tab in [
        255, 240, 128, 129, 130, 131, 241, 1, 132, 242, 2, 5, 6, 7, 243, 244, 245, 135, 255,
    ] {
        app.model.scroll = 0.;
        app.scroll_motion.snap(0.);
        app.scroll_motion.target = 240.;
        app.scroll_target = 240.;
        app.scroll_motion.set_velocity(700.);
        app.scroll_blur = 1.;
        app.page_enter.snap(0.25);
        app.page_enter.target = 1.;
        app.action(Action::SettingsTab(tab));
        if app.model.settings_tab != crate::settings_hub::destination(tab)
            || app.model.scroll != 0.
            || app.scroll_motion.value != 0.
            || app.scroll_motion.target != 0.
            || app.scroll_target != 0.
            || app.scroll_blur != 0.
            || app.page_enter.value != 1.
            || !app.page_enter.settled()
        {
            return Err(format!(
                "settings {tab} inherited navigation or scrolling motion"
            ));
        }
        app.scroll_motion.tick(1. / 60., false);
        if app.scroll_motion.value != 0. {
            return Err("settings retained previous scroll velocity".into());
        }
    }
    app.action(Action::SettingsTab(4));
    app.max_scroll = 500.;
    app.scroll_by(-180.);
    if app.scroll_target != 180. || app.scroll_motion.target != 180. {
        return Err("settings lost normal wheel scrolling".into());
    }
    Ok(())
}
