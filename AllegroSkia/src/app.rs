use crate::{
    async_io::{Background, Request, Snapshot},
    file_shelf::{self, FileShelf},
    platform::{self, HotkeyListener, Hwnd},
    reminders::{self, ActiveAlert, ReminderState},
    state::{
        self, Board, CURRENT_TUTORIAL_VERSION, Cli, ExtraWidgetState, SearchResult, SidebarEdge,
        SidebarSettings, TodoList,
    },
    surface::SkiaSurface,
    ui::{self, Action, HitItem, View},
};
use skia_safe::Contains;
use std::{
    path::PathBuf,
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    application::ApplicationHandler,
    dpi::{LogicalPosition, LogicalSize, PhysicalPosition},
    event::{ElementState, Ime, MouseButton, MouseScrollDelta, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    keyboard::{Key, NamedKey},
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::{Window, WindowId},
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum WinRole {
    Sidebar,
    Desktop,
    Island,
}

struct WindowState {
    window: Window,
    surface: SkiaSurface,
    hits: Vec<HitItem>,
    cursor: (f32, f32),
    scroll: f32,
    menu: Option<(u64, (f32, f32))>,
    drawer: bool,
    hovered_interactive: Option<usize>,
    cursor_pointer: bool,
}
impl WindowState {
    fn hwnd(&self) -> Hwnd {
        match self.window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => h.hwnd.get() as Hwnd,
            _ => std::ptr::null_mut(),
        }
    }
    fn size(&self) -> (f32, f32) {
        let s = self
            .window
            .inner_size()
            .to_logical::<f32>(self.window.scale_factor());
        (s.width, s.height)
    }
}
struct Drag {
    id: u64,
    start: (f32, f32),
    origin: (f32, f32),
    moved: bool,
}
struct DrawerDrag {
    kind: String,
    start: (f32, f32),
    cursor: (f32, f32),
    moved: bool,
    insert_index: Option<usize>,
}

struct App {
    cli: Cli,
    wake: platform::Wake,
    background: Background,
    hotkeys: HotkeyListener,
    sidebar: Option<WindowState>,
    desktop: Option<WindowState>,
    island: Option<WindowState>,
    sidebar_board: Board,
    desktop_board: Board,
    data: Snapshot,
    settings: SidebarSettings,
    settings_open: bool,
    tutorial_open: bool,
    tutorial_step: u8,
    file_shelf: FileShelf,
    shelf_is_demo: bool,
    file_hovering: bool,
    shelf_drag: Option<(Vec<PathBuf>, (f32, f32))>,
    todos: TodoList,
    reminders: ReminderState,
    extra: ExtraWidgetState,
    quote_index: usize,
    last_click: Option<(bool, u64, Instant)>,
    editing_note: Option<(bool, u64)>,
    note_preedit: String,
    monitor_origin: (i32, i32),
    monitor_physical: (u32, u32),
    shown: bool,
    progress: f32,
    drawer_progress: f32,
    drawer_scroll: f32,
    drawer_drag: Option<DrawerDrag>,
    started: Instant,
    last: Instant,
    last_day_check: Instant,
    last_clock_tick: Instant,
    edit: bool,
    dark: bool,
    query: String,
    preedit: String,
    searching: bool,
    previous: Hwnd,
    drag: Option<Drag>,
    error: Option<String>,
    focus_running: bool,
    focus_total: u32,
    focus_left: f32,
}

pub fn run(cli: Cli) -> Result<(), String> {
    let event_loop = EventLoop::<()>::with_user_event()
        .build()
        .map_err(|e| e.to_string())?;
    let proxy = event_loop.create_proxy();
    let wake: platform::Wake = Arc::new(move || {
        let _ = proxy.send_event(());
    });

    let settings = if cli.demo {
        SidebarSettings {
            tutorial_version: CURRENT_TUTORIAL_VERSION,
            ..Default::default()
        }
    } else {
        SidebarSettings::load(&cli.data_dir)
    };
    let (mods, vk) = settings.effective_hotkey();
    let (edit_mods,edit_vk) = settings.effective_edit_hotkey();
    let hotkeys = HotkeyListener::start_with(wake.clone(), mods, if settings.enabled {vk} else {0}, edit_mods, edit_vk)?;

    let todos = if cli.demo {
        TodoList::demo()
    } else {
        TodoList::load(&cli.data_dir)
    };
    let reminders = if cli.demo {
        ReminderState::demo()
    } else {
        ReminderState::load(&cli.data_dir)
    };
    let (file_shelf, shelf_is_demo) = if cli.demo {
        (FileShelf::demo(), true)
    } else {
        let loaded = FileShelf::load(&cli.data_dir);
        if loaded.items.is_empty() {
            (FileShelf::demo(), true)
        } else {
            (loaded, false)
        }
    };

    let background = Background::start(wake.clone(), cli.pipe.clone())?;
    if !cli.demo {
        background.send(Request::LoadBoards(cli.data_dir.clone()));
    }
    let tutorial_open = !cli.demo && !cli.integrated && settings.tutorial_version < CURRENT_TUTORIAL_VERSION;
    let now = Instant::now();
    let mut app = App {
        cli: cli.clone(),
        wake,
        background,
        hotkeys,
        sidebar: None,
        desktop: None,
        island: None,
        sidebar_board: if cli.integrated { Board::default() } else { Board::sidebar_demo() },
        desktop_board: if cli.integrated { Board::default() } else { Board::demo() },
        data: Snapshot::default(),
        settings,
        settings_open: false,
        tutorial_open,
        tutorial_step: 0,
        file_shelf,
        shelf_is_demo,
        file_hovering: false,
        shelf_drag: None,
        todos,
        reminders,
        extra: ExtraWidgetState::default(),
        quote_index: 0,
        last_click: None,
        editing_note: None,
        note_preedit: String::new(),
        monitor_origin: (0, 0),
        monitor_physical: (1920, 1080),
        shown: false,
        progress: 0.,
        drawer_progress: 0.,
        drawer_scroll: 0.,
        drawer_drag: None,
        started: now,
        last: now,
        last_day_check: now,
        last_clock_tick: now,
        edit: false,
        dark: false,
        query: String::new(),
        preedit: String::new(),
        searching: false,
        previous: std::ptr::null_mut(),
        drag: None,
        error: None,
        focus_running: false,
        focus_total: 25 * 60,
        focus_left: (25 * 60) as f32,
    };
    event_loop.run_app(&mut app).map_err(|e| e.to_string())?;
    if let Some(e) = app.error {
        Err(e)
    } else {
        Ok(())
    }
}

impl App {
    fn create(&mut self, el: &ActiveEventLoop) -> Result<(), String> {
        let monitor = el
            .primary_monitor()
            .or_else(|| el.available_monitors().next())
            .ok_or("找不到显示器")?;
        let scale = monitor.scale_factor();
        let physical = monitor.size();
        let position = monitor.position();
        self.monitor_origin = (position.x, position.y);
        self.monitor_physical = (physical.width, physical.height);

        for role in [WinRole::Desktop, WinRole::Sidebar, WinRole::Island] {
            let logical = match role {
                WinRole::Sidebar => LogicalSize::new(
                    ui::sidebar::WINDOW_WIDTH as f64,
                    physical.height as f64 / scale,
                ),
                WinRole::Desktop => LogicalSize::new(
                    physical.width as f64 / scale,
                    physical.height as f64 / scale,
                ),
                WinRole::Island => LogicalSize::new(
                    ui::island::ISLAND_WIN_W as f64,
                    ui::island::ISLAND_WIN_H as f64,
                ),
            };
            let title = match role {
                WinRole::Sidebar => "点屿 · 侧边栏",
                WinRole::Desktop => "点屿 · 桌面组件",
                WinRole::Island => "点屿 · 提醒",
            };
            let window = el
                .create_window(
                    Window::default_attributes()
                        .with_title(title)
                        .with_decorations(false)
                        .with_transparent(true)
                        .with_resizable(false)
                        .with_visible(false)
                        .with_active(false)
                        .with_inner_size(logical),
                )
                .map_err(|e| e.to_string())?;
            let x = match role {
                WinRole::Sidebar => match self.settings.edge {
                    SidebarEdge::Left => position.x,
                    SidebarEdge::Right => {
                        position.x + physical.width as i32 - window.inner_size().width as i32
                    }
                },
                WinRole::Desktop => position.x,
                WinRole::Island => {
                    position.x + (physical.width as i32 - window.inner_size().width as i32) / 2
                }
            };
            window.set_outer_position(PhysicalPosition::new(x, position.y));
            let s = window.inner_size();
            let surface = SkiaSurface::new(s.width, s.height, window.scale_factor() as f32)?;
            let ws = WindowState {
                window,
                surface,
                hits: vec![],
                cursor: (-1., -1.),
                scroll: 0.,
                menu: None,
                drawer: false,
                hovered_interactive: None,
                cursor_pointer: false,
            };
            let hwnd = ws.hwnd();
            platform::layered(hwnd);
            match role {
                WinRole::Sidebar => {
                    self.sidebar = Some(ws);
                    self.sync_sidebar_window_and_region();
                }
                WinRole::Desktop => {
                    // 桌面绝不使用 WS_EX_TRANSPARENT 死锁鼠标，改为按每张卡片与浮层合成精确 GDI 多矩形输入区
                    platform::z_order(hwnd, self.edit || self.tutorial_open);
                    unsafe {
                        platform::ShowWindow(hwnd, 4);
                    }
                    self.desktop = Some(ws);
                    self.sync_desktop_input_region();
                }
                WinRole::Island => {
                    platform::z_order(hwnd, true);
                    self.island = Some(ws);
                }
            }
        }
        if self.cli.demo && self.settings.enabled {
            self.toggle_sidebar();
        }
        self.redraw_all();
        Ok(())
    }

    fn redraw_sidebar(&self) {
        if let Some(w) = &self.sidebar
            && (self.progress > 0. || self.shown)
        {
            w.window.request_redraw();
        }
    }
    fn redraw_desktop(&self) {
        if let Some(w) = &self.desktop {
            w.window.request_redraw();
        }
    }
    fn redraw_island(&self) {
        if let Some(w) = &self.island
            && (self.reminders.island_progress > 0. || self.reminders.active_alert.is_some())
        {
            w.window.request_redraw();
        }
    }
    fn redraw_role(&self, role: WinRole) {
        match role {
            WinRole::Sidebar => self.redraw_sidebar(),
            WinRole::Desktop => self.redraw_desktop(),
            WinRole::Island => self.redraw_island(),
        }
    }
    fn redraw_all(&self) {
        self.redraw_desktop();
        self.redraw_sidebar();
        self.redraw_island();
    }

    fn sync_sidebar_window_and_region(&self) {
        let Some(w) = &self.sidebar else { return };
        let phys_w = w.window.inner_size().width as i32;
        let x = match self.settings.edge {
            SidebarEdge::Left => self.monitor_origin.0,
            SidebarEdge::Right => self.monitor_origin.0 + self.monitor_physical.0 as i32 - phys_w,
        };
        w.window
            .set_outer_position(PhysicalPosition::new(x, self.monitor_origin.1));

        let (wi, he) = w.size();
        let dpi = w.window.scale_factor() as f32;
        let expanded = w.drawer || self.drawer_progress > 0.01;
        let (left_l, right_l) = match self.settings.edge {
            SidebarEdge::Right => {
                let l = if expanded {
                    (wi - ui::sidebar::PANEL_WIDTH
                        - ui::sidebar::DRAWER_WIDTH
                        - ui::sidebar::DRAWER_GAP
                        + 8.)
                        .max(0.)
                } else {
                    (wi - ui::sidebar::PANEL_WIDTH + 8.).max(0.)
                };
                (l, (wi - 8.).max(l))
            }
            SidebarEdge::Left => {
                let r = if expanded {
                    (ui::sidebar::PANEL_WIDTH
                        + ui::sidebar::DRAWER_WIDTH
                        + ui::sidebar::DRAWER_GAP
                        - 8.)
                        .min(wi)
                } else {
                    (ui::sidebar::PANEL_WIDTH - 8.).min(wi)
                };
                (8., r.max(8.))
            }
        };
        let _ = platform::input_region(
            w.hwnd(),
            (left_l * dpi) as i32,
            (16. * dpi) as i32,
            (right_l * dpi) as i32,
            ((he - 16.) * dpi) as i32,
        );
    }

    /// 精确合成桌面卡片、右键菜单、添加面板与编辑顶栏的可交互矩形，保证空白区 100% 穿透桌面。
    fn sync_desktop_input_region(&self) {
        let Some(w) = &self.desktop else { return };
        let (wi, he) = w.size();
        let dpi = w.window.scale_factor() as f32;
        if self.tutorial_open || self.drag.is_some() || (self.edit && w.drawer) {
            let _ = platform::input_region(w.hwnd(), 0, 0, (wi * dpi) as i32, (he * dpi) as i32);
            return;
        }
        let mut rects: Vec<(i32, i32, i32, i32)> =
            Vec::with_capacity(self.desktop_board.cards.len() + 3);
        let to_phys = |l: f32, t: f32, r: f32, b: f32| {
            (
                (l.max(0.) * dpi).floor() as i32,
                (t.max(0.) * dpi).floor() as i32,
                ((r.min(wi)) * dpi).ceil() as i32,
                ((b.min(he)) * dpi).ceil() as i32,
            )
        };
        for c in &self.desktop_board.cards {
            let (cw, ch) = c.size.dimensions();
            rects.push(to_phys(c.x - 6., c.y - 6., c.x + cw + 6., c.y + ch + 6.));
        }
        if self.edit {
            rects.push(to_phys((wi - 398.) * 0.5 - 6., 12., (wi + 398.) * 0.5 + 6., 68.));
        }
        if let Some((_, (mx, my))) = w.menu {
            let x = mx.clamp(12., (wi - 236.).max(12.));
            let y = my.clamp(12., (he - 326.).max(12.));
            rects.push(to_phys(x - 4., y - 4., x + 228., y + 318.));
        }
        if w.drawer {
            let panel_w = 668.0_f32;
            let rows = ui::sidebar::GALLERY_ITEMS.len().div_ceil(4);
            let panel_h = 62. + rows as f32 * 40. + 14.;
            let x = ((wi - panel_w) * 0.5).clamp(12., (wi - panel_w - 12.).max(12.));
            let y = 74.0_f32.min((he - panel_h - 12.).max(16.));
            rects.push(to_phys(x - 4., y - 4., x + panel_w + 4., y + panel_h + 4.));
        }
        let _ = platform::input_regions(w.hwnd(), &rects);
    }

    fn toggle_sidebar(&mut self) {
        if !self.shown && !self.settings.enabled {
            return;
        }
        self.shown = !self.shown;
        self.last = Instant::now();
        if self.shown {
            self.previous = unsafe { platform::GetForegroundWindow() };
            self.sync_sidebar_window_and_region();
            if let Some(w) = &self.sidebar {
                unsafe {
                    platform::SetWindowPos(w.hwnd(), -1isize as Hwnd, 0, 0, 0, 0, 0x53);
                }
                w.window.request_redraw();
            }
        } else {
            self.drawer_drag = None;
            self.settings_open = false;
            if self.editing_note.is_some_and(|(sb, _)| sb) {
                self.finish_note_edit();
            }
            if let Some(w) = &mut self.sidebar {
                w.drawer = false;
                w.menu = None;
            }
            self.finish_search();
        }
    }

    fn finish_search(&mut self) {
        if let Some(w) = &self.sidebar {
            if !self.editing_note.is_some_and(|(sb, _)| sb) {
                w.window.set_ime_allowed(false);
                platform::restore_focus(w.hwnd(), self.previous);
            }
        }
        self.searching = false;
        self.preedit.clear();
    }

    fn start_note_edit(&mut self, sidebar: bool, card_id: u64) {
        if self.editing_note.is_some() && self.editing_note != Some((sidebar, card_id)) {
            self.finish_note_edit();
        }
        self.searching = false;
        self.preedit.clear();
        self.drag = None;
        self.note_preedit.clear();

        // 若是剪贴板卡片尚未手写正文，把系统最新剪贴板内容预填入方便直接改写
        let board = if sidebar {
            &mut self.sidebar_board
        } else {
            &mut self.desktop_board
        };
        let card_pos = if let Some(c) = board.cards.iter_mut().find(|c| c.id == card_id) {
            if c.note_text.is_empty() && c.active() == "clipboard" {
                if let Some(first) = self.data.clipboard.first() {
                    c.note_text = first.clone();
                }
            }
            (c.x, c.y)
        } else {
            return;
        };

        self.editing_note = Some((sidebar, card_id));
        if sidebar {
            if let Some(w) = &self.sidebar {
                platform::allow_focus(w.hwnd(), true);
                w.window.set_ime_allowed(true);
                w.window
                    .set_ime_cursor_area(LogicalPosition::new(80., 220.), LogicalSize::new(260., 64.));
            }
        } else if let Some(w) = &self.desktop {
            self.previous = unsafe { platform::GetForegroundWindow() };
            platform::allow_focus(w.hwnd(), true);
            w.window.set_ime_allowed(true);
            w.window.set_ime_cursor_area(
                LogicalPosition::new(card_pos.0 as f64 + 16., card_pos.1 as f64 + 44.),
                LogicalSize::new(160., 64.),
            );
        }
        self.redraw_role(if sidebar { WinRole::Sidebar } else { WinRole::Desktop });
    }

    fn finish_note_edit(&mut self) {
        let Some((sidebar, _)) = self.editing_note.take() else {
            return;
        };
        self.note_preedit.clear();
        if let Some(w) = if sidebar {
            self.sidebar.as_ref()
        } else {
            self.desktop.as_ref()
        } {
            w.window.set_ime_allowed(false);
            platform::restore_focus(w.hwnd(), self.previous);
        }
        self.persist(sidebar);
        self.redraw_role(if sidebar { WinRole::Sidebar } else { WinRole::Desktop });
    }

    fn toggle_edit(&mut self) {
        self.edit = !self.edit;
        if let Some(w) = &self.desktop {
            platform::z_order(w.hwnd(), self.edit || self.tutorial_open);
        }
        self.sync_desktop_input_region();
        self.redraw_all();
    }

    fn persist(&self, sidebar: bool) {
        if !self.cli.demo {
            self.background.send(Request::Save(
                self.cli.data_dir.join(if sidebar {
                    "allegro-sidebar.json"
                } else {
                    "allegro-desktop.json"
                }),
                if sidebar {
                    self.sidebar_board.clone()
                } else {
                    self.desktop_board.clone()
                },
            ));
        }
    }
    fn persist_settings(&self) {
        if !self.cli.demo {
            self.background
                .send(Request::SaveSettings(self.cli.data_dir.clone(), self.settings));
        }
    }
    fn persist_reminders(&self) {
        if !self.cli.demo {
            self.background.send(Request::SaveReminders(
                self.cli.data_dir.clone(),
                self.reminders.clone(),
            ));
        }
    }

    fn action(&mut self, a: Action, role: WinRole, el: &ActiveEventLoop) {
        let sidebar = role == WinRole::Sidebar;
        if self.cli.integrated && matches!(a, Action::ToggleSettings | Action::OpenTutorial) {
            let request=if matches!(a,Action::OpenTutorial){"tutorial"}else{"settings"};
            let _=std::fs::write(self.cli.data_dir.join("allegro-center-request.json"),request);
            return;
        }
        let mut sync_all = false;
        let mut sync_desk_rgn = false;

        match a {
            Action::Close => {
                if self.shown {
                    self.toggle_sidebar();
                }
            }
            Action::Theme => {
                self.dark = !self.dark;
                sync_all = true;
            }
            Action::Edit => {
                self.toggle_edit();
                return;
            }
            Action::ToggleSettings => {
                if sidebar {
                    self.settings_open = !self.settings_open;
                    if let Some(w) = &mut self.sidebar {
                        w.menu = None;
                    }
                }
            }
            Action::ToggleSidebarEnabled => {
                self.settings.enabled = !self.settings.enabled;
                self.persist_settings();
                if !self.settings.enabled && self.shown {
                    self.toggle_sidebar();
                }
                sync_all = true;
            }
            Action::ToggleHotkeyHint => {
                self.settings.show_hotkey_hint = !self.settings.show_hotkey_hint;
                self.persist_settings();
                sync_all = true;
            }
            Action::SetSidebarEdge(edge) => {
                self.settings.edge = edge;
                self.persist_settings();
                self.sync_sidebar_window_and_region();
                sync_all = true;
            }
            Action::SetSidebarHotkey(preset) => {
                self.settings.hotkey = preset;
                self.settings.custom_key=0;self.settings.custom_modifiers=0;
                let (mods, vk) = preset.win32_mods_vk();
                let (em,ev)=self.settings.effective_edit_hotkey();
                let _ = self.hotkeys.rebind(self.wake.clone(), mods, if self.settings.enabled {vk} else {0}, em, ev);
                self.persist_settings();
                sync_all = true;
            }
            Action::OpenTutorial => {
                self.tutorial_open = true;
                self.tutorial_step = if sidebar { 0 } else { 1 };
                self.settings_open = false;
                if let Some(w) = &self.desktop {
                    platform::z_order(w.hwnd(), true);
                }
                sync_desk_rgn = true;
                sync_all = true;
            }
            Action::TutorialNext => {
                self.tutorial_step = 1;
                sync_all = true;
            }
            Action::TutorialPrev => {
                self.tutorial_step = 0;
                sync_all = true;
            }
            Action::TutorialDismiss | Action::TutorialComplete => {
                let complete = matches!(a, Action::TutorialComplete);
                self.tutorial_open = false;
                self.settings.tutorial_version = CURRENT_TUTORIAL_VERSION;
                self.persist_settings();
                if let Some(w) = &self.desktop {
                    platform::z_order(w.hwnd(), self.edit);
                }
                sync_desk_rgn = true;
                if complete && self.settings.enabled && !self.shown {
                    self.toggle_sidebar();
                }
                sync_all = true;
            }
            Action::Search => {
                if sidebar {
                    self.finish_note_edit();
                    self.searching = true;
                    if let Some(w) = &self.sidebar {
                        platform::allow_focus(w.hwnd(), true);
                        w.window.set_ime_allowed(true);
                        w.window.set_ime_cursor_area(
                            LogicalPosition::new(48., 154.),
                            LogicalSize::new(340., 32.),
                        );
                    }
                }
            }
            Action::AddDrawer => {
                if let Some(w) = if sidebar {
                    self.sidebar.as_mut()
                } else {
                    self.desktop.as_mut()
                } {
                    w.drawer = !w.drawer;
                    w.menu = None;
                    if sidebar && !w.drawer {
                        self.drawer_drag = None;
                    }
                }
                if sidebar {
                    self.last = Instant::now();
                    self.sync_sidebar_window_and_region();
                } else {
                    sync_desk_rgn = true;
                }
            }
            Action::Copy(s) => self.background.send(Request::Copy(s)),
            Action::Result(i) => {
                let results = state::search(&self.query, &self.data.apps);
                if let Some(result) = results.get(i).cloned() {
                    match result {
                        SearchResult::Math(s) => self.background.send(Request::Copy(s)),
                        SearchResult::Application(a) => {
                            self.background.send(Request::Launch(a.path))
                        }
                        SearchResult::Command(s) => match s.as_str() {
                            "/dark" => {
                                self.dark = true;
                                sync_all = true;
                            }
                            "/light" => {
                                self.dark = false;
                                sync_all = true;
                            }
                            "/edit" => {
                                self.toggle_edit();
                                return;
                            }
                            "/add" => {
                                if let Some(w) = &mut self.sidebar {
                                    w.drawer = true;
                                }
                                self.sync_sidebar_window_and_region();
                            }
                            "/quit" => el.exit(),
                            _ => {}
                        },
                    }
                    self.query.clear();
                    self.finish_search();
                }
            }
            Action::FocusToggle => {
                self.focus_running = !self.focus_running;
                if self.focus_left <= 0.5 {
                    self.focus_left = self.focus_total as f32;
                }
                sync_all = true;
            }
            Action::FocusPreset(mins) => {
                self.focus_running = false;
                self.focus_total = mins.saturating_mul(60).max(60);
                self.focus_left = self.focus_total as f32;
                sync_all = true;
            }
            Action::TodoToggle(idx) => {
                self.todos.toggle(idx);
                if !self.cli.demo {
                    self.background
                        .send(Request::SaveTodos(self.cli.data_dir.clone(), self.todos.clone()));
                }
                sync_all = true;
            }
            Action::QuoteNext => {
                self.quote_index = self.quote_index.wrapping_add(1);
                sync_all = true;
            }
            Action::NoteFinish => {
                self.finish_note_edit();
                return;
            }
            Action::NoteClear(id) => {
                let board = if sidebar {
                    &mut self.sidebar_board
                } else {
                    &mut self.desktop_board
                };
                if let Some(c) = board.cards.iter_mut().find(|c| c.id == id) {
                    c.note_text.clear();
                }
                self.note_preedit.clear();
                self.persist(sidebar);
                sync_all = true;
            }
            Action::RefreshAiQuota => {
                if !self.cli.demo {
                    self.data.ai_quota.refreshing = true;
                    self.background
                        .send(Request::RefreshAiQuota(self.cli.data_dir.clone()));
                    sync_all = true;
                }
            }
            Action::OpenAiQuotaConfig => {
                self.background
                    .send(Request::OpenAiQuotaConfig(self.cli.data_dir.clone()));
            }
            Action::ShelfRemove(i) => {
                if self.shelf_is_demo {
                    self.shelf_is_demo = false;
                }
                self.file_shelf.remove_index(i);
                if !self.cli.demo {
                    self.background.send(Request::SaveFileShelf(
                        self.cli.data_dir.clone(),
                        self.file_shelf.clone(),
                    ));
                }
                sync_all = true;
            }
            Action::ShelfClear => {
                self.shelf_is_demo = false;
                self.file_shelf.clear();
                if !self.cli.demo {
                    self.background.send(Request::SaveFileShelf(
                        self.cli.data_dir.clone(),
                        self.file_shelf.clone(),
                    ));
                }
                sync_all = true;
            }
            Action::ShelfDragAll | Action::ShelfDragItem(_) => {}
            Action::WaterDrink | Action::IslandAckWater => {
                self.reminders.acknowledge_water();
                self.last = Instant::now();
                self.persist_reminders();
                sync_all = true;
            }
            Action::WaterCupDelta(d) => {
                self.reminders.adjust_cup(d);
                self.persist_reminders();
                sync_all = true;
            }
            Action::WaterGoalDelta(d) => {
                self.reminders.adjust_goal(d);
                self.persist_reminders();
                sync_all = true;
            }
            Action::WaterCycleInterval => {
                self.reminders.cycle_water_interval();
                self.persist_reminders();
                sync_all = true;
            }
            Action::WaterToggle => {
                self.reminders.toggle_water();
                self.persist_reminders();
                sync_all = true;
            }
            Action::WaterPreviewIsland => {
                self.reminders.trigger_preview(ActiveAlert::Water);
                self.show_island_window();
                sync_all = true;
            }
            Action::StandAck | Action::IslandAckStand => {
                self.reminders.acknowledge_stand();
                self.last = Instant::now();
                self.persist_reminders();
                sync_all = true;
            }
            Action::StandCycleInterval => {
                self.reminders.cycle_stand_interval();
                self.persist_reminders();
                sync_all = true;
            }
            Action::StandToggle => {
                self.reminders.toggle_stand();
                self.persist_reminders();
                sync_all = true;
            }
            Action::StandPreviewIsland => {
                self.reminders.trigger_preview(ActiveAlert::Stand);
                self.show_island_window();
                sync_all = true;
            }
            Action::HabitToggle(i) => {
                if let Some(h) = self.extra.habits.get_mut(i) {
                    *h = !*h;
                }
                sync_all = true;
            }
            Action::AmbientToggle => {
                self.extra.ambient_playing = !self.extra.ambient_playing;
                sync_all = true;
            }
            Action::AmbientNext => {
                self.extra.ambient_idx = self.extra.ambient_idx.wrapping_add(1);
                sync_all = true;
            }
            Action::PaletteNext => {
                self.extra.palette_idx = self.extra.palette_idx.wrapping_add(1);
                sync_all = true;
            }
            Action::WoodFishTap => {
                self.extra.merit_count = self.extra.merit_count.saturating_add(1);
                sync_all = true;
            }
            Action::BreathToggle => {
                self.extra.breath_active = !self.extra.breath_active;
                sync_all = true;
            }
            Action::CalcKey(k) => {
                self.extra.calc_press(&k);
                sync_all = true;
            }
            Action::Context(id) => {
                if let Some(w) = if sidebar {
                    self.sidebar.as_mut()
                } else {
                    self.desktop.as_mut()
                } {
                    let pos = w.cursor;
                    w.menu = Some((id, pos));
                    w.drawer = false;
                }
                if !sidebar {
                    sync_desk_rgn = true;
                }
            }
            other => {
                let is_turn = matches!(other, Action::Turn(_, _));
                let board = if sidebar {
                    &mut self.sidebar_board
                } else {
                    &mut self.desktop_board
                };
                match other {
                    Action::Add(kind) => {
                        let offset = board.cards.len() as f32;
                        board.add(&kind);
                        if !sidebar {
                            if let Some(last) = board.cards.last_mut() {
                                last.x = 64. + (offset % 4.) * 48.;
                                last.y = 80. + (offset % 3.) * 48.;
                            }
                        }
                    }
                    Action::Remove(id) => board.cards.retain(|c| c.id != id),
                    Action::Resize(id) => {
                        if let Some(c) = board.cards.iter_mut().find(|c| c.id == id) {
                            c.size = c.size.next();
                        }
                    }
                    Action::Acrylic(id) => {
                        if let Some(c) = board.cards.iter_mut().find(|c| c.id == id) {
                            c.acrylic = (c.acrylic + 1) % 4;
                        }
                    }
                    Action::Move(id, delta) => {
                        if let Some(i) = board.cards.iter().position(|c| c.id == id) {
                            let j =
                                (i as i32 + delta).clamp(0, board.cards.len() as i32 - 1) as usize;
                            board.cards.swap(i, j);
                        }
                    }
                    Action::Turn(id, d) => {
                        if let Some(c) = board.cards.iter_mut().find(|c| c.id == id) {
                            c.turn(d);
                        }
                    }
                    Action::Split(id) => {
                        board.split(id);
                    }
                    Action::Merge(id) => {
                        if let Some(c) = board.cards.iter().find(|c| c.id == id) {
                            let size = c.size;
                            if let Some(other) = board
                                .cards
                                .iter()
                                .find(|c| c.id != id && c.size == size)
                                .map(|c| c.id)
                            {
                                board.merge(id, other);
                            }
                        }
                    }
                    Action::Card(_) | Action::DrawerDrag(_) => {}
                    _ => {}
                }
                if !is_turn {
                    if let Some(w) = if sidebar {
                        self.sidebar.as_mut()
                    } else {
                        self.desktop.as_mut()
                    } {
                        w.menu = None;
                        w.drawer = false;
                    }
                }
                if !sidebar {
                    sync_desk_rgn = true;
                }
                self.persist(sidebar);
            }
        }
        if sync_desk_rgn {
            self.sync_desktop_input_region();
        }
        if sync_all {
            self.redraw_all();
        } else {
            self.redraw_role(role);
        }
    }

    fn show_island_window(&mut self) {
        self.last = Instant::now();
        if let Some(w) = &self.island {
            unsafe {
                platform::SetWindowPos(w.hwnd(), -1isize as Hwnd, 0, 0, 0, 0, 0x53);
            }
            w.window.request_redraw();
        }
    }

    fn draw(&mut self, role: WinRole) -> Result<(), String> {
        let ws = match role {
            WinRole::Sidebar => self.sidebar.as_mut(),
            WinRole::Desktop => self.desktop.as_mut(),
            WinRole::Island => self.island.as_mut(),
        };
        let Some(w) = ws else { return Ok(()) };
        let size = w.window.inner_size();
        if size.width == 0 || size.height == 0 {
            return Ok(());
        }
        let dpi = w.window.scale_factor() as f32;
        w.surface.resize(size.width, size.height, dpi)?;
        let (width, height) = w.size();
        let mut hits = vec![];

        if role == WinRole::Island {
            let alert = self.reminders.active_alert.unwrap_or(ActiveAlert::Water);
            w.surface.draw(|c| {
                hits = ui::island::draw(
                    c,
                    width,
                    height,
                    self.reminders.island_progress,
                    alert,
                    &self.reminders,
                    Some(w.cursor),
                );
            })?;
        } else {
            let sidebar = role == WinRole::Sidebar;
            let results = state::search(&self.query, &self.data.apps);
            let editing_this_win = self
                .editing_note
                .and_then(|(sb, id)| (sb == sidebar).then_some(id));
            let v = View {
                board: if sidebar {
                    &self.sidebar_board
                } else {
                    &self.desktop_board
                },
                data: &self.data,
                dark: self.dark,
                edit: self.edit,
                query: &self.query,
                preedit: &self.preedit,
                results: &results,
                scroll: w.scroll,
                hover: Some(w.cursor),
                menu: w.menu,
                drawer: w.drawer,
                drawer_progress: if sidebar { self.drawer_progress } else { 0. },
                drawer_scroll: if sidebar { self.drawer_scroll } else { 0. },
                dragging_card: if sidebar {
                    self.drawer_drag
                        .as_ref()
                        .filter(|d| d.moved)
                        .map(|d| (d.kind.as_str(), d.cursor, d.insert_index))
                } else {
                    None
                },
                settings: self.settings,
                settings_open: if sidebar { self.settings_open } else { false },
                hotkey_native_ok: self.hotkeys.sidebar_registered(),
                file_shelf: &self.file_shelf,
                file_hovering: self.file_hovering,
                todos: &self.todos,
                quote_index: self.quote_index,
                editing_note: editing_this_win,
                note_preedit: if editing_this_win.is_some() {
                    &self.note_preedit
                } else {
                    ""
                },
                reminders: &self.reminders,
                extra: &self.extra,
                tutorial_open: if sidebar { false } else { self.tutorial_open },
                tutorial_step: self.tutorial_step,
                time_sec: self.started.elapsed().as_secs_f32(),
                focus_running: self.focus_running,
                focus_left: self.focus_left.ceil() as u32,
                focus_total: self.focus_total,
            };
            w.surface.draw(|c| {
                hits = if sidebar {
                    ui::sidebar::draw(c, width, height, self.progress, &v)
                } else {
                    ui::widgets::desktop(c, width, height, &v)
                }
            })?;
        }

        let pt = skia_safe::Point::from(w.cursor);
        w.hovered_interactive = hits
            .iter()
            .rposition(|h| h.rect.contains(pt) && !h.label.is_empty() && !matches!(h.action, Action::Card(_)));
        w.hits = hits;
        w.surface.present(w.hwnd())?;
        Ok(())
    }

    fn report_host_status(&self) {
        if self.cli.integrated { self.background.send(Request::HostStatus(self.cli.data_dir.clone(),serde_json::json!({"settings":self.settings,"registered":self.hotkeys.sidebar_registered(),"edit_registered":self.hotkeys.edit_registered(),"desktop_count":self.desktop_board.cards.len(),"sidebar_count":self.sidebar_board.cards.len()}))); }
    }
    fn tick(&mut self, el: &ActiveEventLoop) {
        while let Ok(settings)=self.background.settings.try_recv() {
            if settings != self.settings {
                if settings.effective_hotkey() != self.settings.effective_hotkey() || settings.enabled != self.settings.enabled || settings.effective_edit_hotkey() != self.settings.effective_edit_hotkey() {
                    let (mods,vk)=settings.effective_hotkey();
                    let (em,ev)=settings.effective_edit_hotkey();
                    if let Err(err)=self.hotkeys.rebind(self.wake.clone(),mods,if settings.enabled {vk} else {0},em,ev){self.error=Some(err);}
                }
                self.settings=settings;
                self.report_host_status();
                if !settings.enabled && self.shown { self.toggle_sidebar(); }
                self.sync_sidebar_window_and_region();
                self.redraw_all();
            }
        }
        let (side, edit) = self.hotkeys.take();
        if side % 2 == 1 && self.settings.enabled {
            self.toggle_sidebar();
        }
        if edit % 2 == 1 {
            self.toggle_edit();
        }
        if let Ok((sidebar, desktop)) = self.background.loaded.try_recv() {
            self.sidebar_board = sidebar;
            self.desktop_board = desktop;
            self.report_host_status();
            self.sync_desktop_input_region();
            self.redraw_all();
        }

        let mut need_all = false;
        let mut metrics_changed = false;
        if let Ok(s) = self.background.shared.try_lock() {
            if s.revision != self.data.revision {
                let wall_changed = s.wallpaper.is_some() != self.data.wallpaper.is_some();
                let clip_changed = s.clipboard != self.data.clipboard;
                let ai_changed = s.ai_quota != self.data.ai_quota;
                let round_eq = |a: Option<f32>, b: Option<f32>| {
                    a.map(|x| x.round() as i32) == b.map(|x| x.round() as i32)
                };
                metrics_changed = !round_eq(s.metrics.cpu, self.data.metrics.cpu)
                    || !round_eq(s.metrics.memory, self.data.metrics.memory)
                    || !round_eq(s.metrics.disk, self.data.metrics.disk)
                    || s.metrics.battery != self.data.metrics.battery
                    || (s.metrics.download / 1024.).round() as u64
                        != (self.data.metrics.download / 1024.).round() as u64
                    || (s.metrics.upload / 1024.).round() as u64
                        != (self.data.metrics.upload / 1024.).round() as u64;
                need_all = wall_changed || clip_changed || ai_changed || s.live != self.data.live;
                self.data = Snapshot {
                    wallpaper: s.wallpaper.clone(),
                    wallpaper_blurred: s.wallpaper_blurred.clone(),
                    wallpaper_status: s.wallpaper_status.clone(),
                    apps: s.apps.clone(),
                    indexed: s.indexed,
                    metrics: s.metrics.clone(),
                    clipboard: s.clipboard.clone(),
                    ai_quota: s.ai_quota.clone(),
                    errors: s.errors.clone(),
                    pipe_status: s.pipe_status.clone(),
                    live: if self.cli.integrated && s.live.is_null() {serde_json::json!({})} else {s.live.clone()},
                    revision: s.revision,
                };
            }
        }
        if need_all {
            self.redraw_all();
        } else if metrics_changed {
            let has_sys = |b: &Board| {
                b.cards.iter().any(|c| {
                    matches!(
                        c.active(),
                        "system" | "net_speed" | "wireless" | "storage"
                    )
                })
            };
            if (self.progress > 0. || self.shown) && has_sys(&self.sidebar_board) {
                self.redraw_sidebar();
            }
            if has_sys(&self.desktop_board) {
                self.redraw_desktop();
            }
        }

        let now = Instant::now();
        let dt_raw = now.duration_since(self.last).as_secs_f32();
        let dt = dt_raw.min(0.05);
        self.last = now;

        // 跨天检测（每 30 秒检查一次日期变更，自动清零饮水与久坐计数）
        if now.duration_since(self.last_day_check) >= Duration::from_secs(30) {
            self.last_day_check = now;
            if self.reminders.check_day_rollover(&reminders::today_key()) {
                self.persist_reminders();
                self.redraw_all();
            }
        }

        // 喝水与久坐提醒周期推进；到点弹出顶部独立黑色提醒灵动岛
        if !self.cli.demo && self.reminders.tick(dt_raw.min(5.0)) {
            self.show_island_window();
            self.redraw_all();
        }

        if self.focus_running {
            let prev_sec = self.focus_left.ceil() as u32;
            self.focus_left = (self.focus_left - dt).max(0.);
            if self.focus_left <= 0.01 {
                self.focus_running = false;
            }
            if self.focus_left.ceil() as u32 != prev_sec {
                if (self.progress > 0. || self.shown)
                    && self.sidebar_board.cards.iter().any(|c| c.active() == "focus")
                {
                    self.redraw_sidebar();
                }
                if self.desktop_board.cards.iter().any(|c| c.active() == "focus") {
                    self.redraw_desktop();
                }
            }
        }

        let mut animating = false;
        let target = if self.shown { 1. } else { 0. };
        if (target - self.progress).abs() > 0.001 {
            self.progress += (target - self.progress) * (1. - (-dt * 20.).exp());
            if (target - self.progress).abs() < 0.002 {
                self.progress = target;
            }
            animating = true;
            if let Some(w) = &self.sidebar {
                w.window.request_redraw();
                if self.progress == 0. {
                    unsafe {
                        platform::ShowWindow(w.hwnd(), 0);
                    }
                    if !self.cli.keep_alive && !self.cli.demo && self.reminders.active_alert.is_none()
                    {
                        el.exit();
                    }
                }
            }
        }

        let drawer_open = self.shown && self.sidebar.as_ref().is_some_and(|w| w.drawer);
        let drawer_target = if drawer_open { 1. } else { 0. };
        if (drawer_target - self.drawer_progress).abs() > 0.001 {
            self.drawer_progress +=
                (drawer_target - self.drawer_progress) * (1. - (-dt * 22.).exp());
            if (drawer_target - self.drawer_progress).abs() < 0.002 {
                self.drawer_progress = drawer_target;
                if self.drawer_progress == 0. {
                    self.sync_sidebar_window_and_region();
                }
            }
            animating = true;
            self.redraw_sidebar();
        }

        // 提醒灵动岛平滑展开/收缩
        let island_target = if self.reminders.active_alert.is_some() {
            1.
        } else {
            0.
        };
        if (island_target - self.reminders.island_progress).abs() > 0.001 {
            self.reminders.island_progress +=
                (island_target - self.reminders.island_progress) * (1. - (-dt * 18.).exp());
            if (island_target - self.reminders.island_progress).abs() < 0.002 {
                self.reminders.island_progress = island_target;
            }
            animating = true;
            if let Some(w) = &self.island {
                w.window.request_redraw();
                if self.reminders.island_progress == 0. {
                    unsafe {
                        platform::ShowWindow(w.hwnd(), 0);
                    }
                }
            }
        }

        let desktop_sweep = self
            .desktop_board
            .cards
            .iter()
            .any(|c| c.active() == "analog_clock");
        let sidebar_sweep = (self.shown || self.progress > 0.01)
            && self
                .sidebar_board
                .cards
                .iter()
                .any(|c| c.active() == "analog_clock");
        if (desktop_sweep || sidebar_sweep)
            && now.duration_since(self.last_clock_tick) >= Duration::from_millis(25)
        {
            self.last_clock_tick = now;
            if desktop_sweep {
                self.redraw_desktop();
            }
            if sidebar_sweep {
                self.redraw_sidebar();
            }
        }

        let live_anim = (self.extra.ambient_playing || self.extra.breath_active)
            && (self.shown || self.progress > 0.);
        if animating || live_anim {
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(16)));
        } else if desktop_sweep || sidebar_sweep {
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(25)));
        } else if self.focus_running {
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_millis(250)));
        } else if self.reminders.water_enabled || self.reminders.stand_enabled {
            el.set_control_flow(ControlFlow::WaitUntil(now + Duration::from_secs(1)));
        } else {
            el.set_control_flow(ControlFlow::Wait);
        }
    }
}

impl ApplicationHandler<()> for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.desktop.is_none() {
            if let Err(e) = self.create(el) {
                self.error = Some(e);
                el.exit();
            }
        }
    }
    fn user_event(&mut self, el: &ActiveEventLoop, _: ()) {
        self.tick(el)
    }
    fn about_to_wait(&mut self, el: &ActiveEventLoop) {
        self.tick(el)
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        let role = if self.sidebar.as_ref().is_some_and(|w| w.window.id() == id) {
            WinRole::Sidebar
        } else if self.island.as_ref().is_some_and(|w| w.window.id() == id) {
            WinRole::Island
        } else if self.desktop.as_ref().is_some_and(|w| w.window.id() == id) {
            WinRole::Desktop
        } else {
            return;
        };
        let sidebar = role == WinRole::Sidebar;

        let ws = match role {
            WinRole::Sidebar => self.sidebar.as_mut(),
            WinRole::Desktop => self.desktop.as_mut(),
            WinRole::Island => self.island.as_mut(),
        };
        let Some(w) = ws else { return };

        match event {
            WindowEvent::CloseRequested => {
                if self.cli.keep_alive {
                    if sidebar && self.shown {
                        self.toggle_sidebar();
                    }
                } else {
                    el.exit();
                }
            }
            WindowEvent::RedrawRequested => {
                if let Err(e) = self.draw(role) {
                    self.error = Some(e);
                    el.exit();
                }
            }
            WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => match role {
                WinRole::Sidebar => {
                    self.sync_sidebar_window_and_region();
                    self.redraw_sidebar();
                }
                WinRole::Desktop => {
                    self.sync_desktop_input_region();
                    self.redraw_desktop();
                }
                WinRole::Island => self.redraw_island(),
            },
            WindowEvent::HoveredFile(_) => {
                if !self.file_hovering {
                    self.file_hovering = true;
                    self.redraw_all();
                }
            }
            WindowEvent::HoveredFileCancelled => {
                if self.file_hovering {
                    self.file_hovering = false;
                    self.redraw_all();
                }
            }
            WindowEvent::DroppedFile(path) => {
                self.file_hovering = false;
                if self.shelf_is_demo {
                    self.file_shelf.clear();
                    self.shelf_is_demo = false;
                }
                self.file_shelf.add_path(path);
                if !self.cli.demo {
                    self.background.send(Request::SaveFileShelf(
                        self.cli.data_dir.clone(),
                        self.file_shelf.clone(),
                    ));
                }
                self.redraw_all();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = position.to_logical::<f32>(w.window.scale_factor());
                w.cursor = (p.x, p.y);
                let (win_w, win_h) = w.size();
                let pt = skia_safe::Point::from(w.cursor);
                let new_hover = w.hits.iter().rposition(|h| {
                    h.rect.contains(pt) && !h.label.is_empty() && !matches!(h.action, Action::Card(_))
                });
                let interactive = self.drawer_drag.is_some()
                    || self.drag.is_some()
                    || w.hits
                        .iter()
                        .any(|h| h.rect.contains(pt) && !h.label.is_empty());
                if interactive != w.cursor_pointer {
                    w.cursor_pointer = interactive;
                    w.window.set_cursor(if interactive {
                        winit::window::CursorIcon::Pointer
                    } else {
                        winit::window::CursorIcon::Default
                    });
                }

                // 若按住了文件中转站文件条目并拖动超过 5px，立即唤起 Win32 OLE CF_HDROP 跨进程拖出
                if let Some((paths, start)) = &self.shelf_drag {
                    if (w.cursor.0 - start.0).abs() + (w.cursor.1 - start.1).abs() > 5. {
                        let owned = paths.clone();
                        self.shelf_drag = None;
                        let _ = file_shelf::start_ole_file_drag(&owned);
                        return;
                    }
                }

                let mut dragging = false;
                if sidebar {
                    if let Some(d) = &mut self.drawer_drag {
                        d.cursor = w.cursor;
                        if (d.cursor.0 - d.start.0).abs() + (d.cursor.1 - d.start.1).abs() > 5. {
                            d.moved = true;
                        }
                        if d.moved {
                            let (in_panel, panel_origin_x) = match self.settings.edge {
                                SidebarEdge::Right => {
                                    let pl = win_w - ui::sidebar::PANEL_WIDTH + 8.;
                                    (d.cursor.0 >= pl, win_w - ui::sidebar::PANEL_WIDTH + 24.)
                                }
                                SidebarEdge::Left => {
                                    let pr = ui::sidebar::PANEL_WIDTH - 8.;
                                    (d.cursor.0 <= pr, 24.)
                                }
                            };
                            if in_panel && self.query.is_empty() {
                                let content_h = state::pack_sidebar_blocks(&self.sidebar_board.cards)
                                    .iter()
                                    .map(|blk| blk.y + blk.h)
                                    .fold(0., f32::max);
                                let max_s = (content_h + 140. - (win_h - 134.)).max(0.);
                                if d.cursor.1 > win_h - 68. {
                                    w.scroll = (w.scroll + 14.).clamp(0., max_s);
                                } else if d.cursor.1 < 144. && d.cursor.1 > 90. {
                                    w.scroll = (w.scroll - 14.).clamp(0., max_s);
                                }
                                let rel_x = d.cursor.0 - panel_origin_x;
                                let rel_y = d.cursor.1 - 106. + w.scroll;
                                d.insert_index = Some(state::sidebar_insert_index(
                                    &self.sidebar_board.cards,
                                    &d.kind,
                                    rel_x,
                                    rel_y,
                                ));
                            } else {
                                d.insert_index = None;
                            }
                            dragging = true;
                        }
                    }
                } else if role == WinRole::Desktop {
                    if let Some(d) = &mut self.drag {
                        let dx = p.x - d.start.0;
                        let dy = p.y - d.start.1;
                        if dx.abs() + dy.abs() > 4. {
                            d.moved = true;
                        }
                        if d.moved {
                            if let Some(c) =
                                self.desktop_board.cards.iter_mut().find(|c| c.id == d.id)
                            {
                                c.x = (d.origin.0 + dx).max(8.);
                                c.y = (d.origin.1 + dy).max(8.);
                            }
                            dragging = true;
                        }
                    }
                }
                if new_hover != w.hovered_interactive || dragging {
                    w.hovered_interactive = new_hover;
                    w.window.request_redraw();
                }
            }
            WindowEvent::CursorLeft { .. } => {
                w.cursor = (-1., -1.);
                if w.cursor_pointer {
                    w.cursor_pointer = false;
                    w.window.set_cursor(winit::window::CursorIcon::Default);
                }
                if w.hovered_interactive.take().is_some() {
                    w.window.request_redraw();
                }
            }
            WindowEvent::MouseInput {
                state: button_state,
                button,
                ..
            } => {
                let cursor = w.cursor;
                let pt = skia_safe::Point::from(cursor);
                let hit = w
                    .hits
                    .iter()
                    .rev()
                    .find(|h| h.rect.contains(pt))
                    .cloned();

                if button == MouseButton::Right && button_state == ElementState::Pressed {
                    let card = w
                        .hits
                        .iter()
                        .rev()
                        .find(|h| h.rect.contains(pt) && matches!(h.action, Action::Card(id) if id != 0))
                        .and_then(|h| match h.action {
                            Action::Card(id) => Some(id),
                            _ => None,
                        });
                    let cleared_menu = if card.is_none() {
                        w.menu.take().is_some()
                    } else {
                        false
                    };
                    if let Some(id) = card {
                        self.action(Action::Context(id), role, el);
                    } else if cleared_menu {
                        if role == WinRole::Desktop {
                            self.sync_desktop_input_region();
                        }
                        self.redraw_role(role);
                    }
                    return;
                }

                if button != MouseButton::Left {
                    return;
                }

                if button_state == ElementState::Pressed {
                    // 独立提醒灵动岛窗口：任意点击立即执行打卡（关闭弹窗并重新计时）
                    if role == WinRole::Island {
                        if let Some(h) = hit {
                            self.action(h.action, WinRole::Island, el);
                        }
                        return;
                    }

                    // 优先检查文件拖出热区（按住拖动即开启 OLE 拖出）
                    match hit.as_ref().map(|h| &h.action) {
                        Some(Action::ShelfDragAll) => {
                            let paths = self.file_shelf.existing_paths();
                            if !paths.is_empty() {
                                self.shelf_drag = Some((paths, cursor));
                            }
                            return;
                        }
                        Some(Action::ShelfDragItem(idx)) => {
                            if let Some(item) = self.file_shelf.items.get(*idx) {
                                if item.path.exists() {
                                    self.shelf_drag = Some((vec![item.path.clone()], cursor));
                                }
                            }
                            return;
                        }
                        _ => {}
                    }

                    // 检查是否双击便签/剪贴板卡片进入原地中文输入
                    if let Some(HitItem {
                        action: Action::Card(id),
                        ..
                    }) = &hit
                    {
                        if *id != 0 {
                            let is_note_kind = {
                                let board = if sidebar {
                                    &self.sidebar_board
                                } else {
                                    &self.desktop_board
                                };
                                board
                                    .cards
                                    .iter()
                                    .find(|c| c.id == *id)
                                    .is_some_and(|c| matches!(c.active(), "clipboard" | "note"))
                            };
                            let is_double = is_note_kind
                                && self.last_click.is_some_and(|(sb, pid, t)| {
                                    sb == sidebar
                                        && pid == *id
                                        && t.elapsed() <= Duration::from_millis(420)
                                });
                            if is_double {
                                self.last_click = None;
                                self.start_note_edit(sidebar, *id);
                                return;
                            }
                            self.last_click = Some((sidebar, *id, Instant::now()));
                        }
                    }

                    // 点在外部时收起笔记编辑态（保留点清空/完成按钮由对应 Action 自己处理）
                    if let Some((esb, eid)) = self.editing_note {
                        let keep_editing = matches!(
                            hit.as_ref().map(|h| &h.action),
                            Some(Action::NoteFinish) | Some(Action::NoteClear(_))
                        ) || (esb == sidebar
                            && matches!(hit.as_ref().map(|h| &h.action), Some(Action::Card(id)) if *id == eid));
                        if !keep_editing {
                            self.finish_note_edit();
                        }
                    }

                    if sidebar {
                        if let Some(HitItem {
                            action: Action::DrawerDrag(kind),
                            ..
                        }) = hit
                        {
                            self.drawer_drag = Some(DrawerDrag {
                                kind,
                                start: cursor,
                                cursor,
                                moved: false,
                                insert_index: None,
                            });
                        } else if let Some(h) = hit {
                            if !matches!(h.action, Action::Card(_)) {
                                self.action(h.action, WinRole::Sidebar, el);
                            }
                        }
                    } else {
                        // 桌面窗口：点到红圆删除、按钮、待办勾选、菜单项等非卡片主体控件立即执行；
                        // 点到卡片主体（无论是否开启 edit）直接开始拖动！
                        if let Some(h) = hit {
                            if matches!(h.action, Action::Card(0)) {
                                return;
                            }
                            if let Action::Card(cid) = h.action {
                                if let Some(ws) = &mut self.desktop {
                                    if ws.menu.take().is_some() {
                                        ws.window.request_redraw();
                                    }
                                }
                                if let Some(c) =
                                    self.desktop_board.cards.iter().find(|c| c.id == cid)
                                {
                                    self.drag = Some(Drag {
                                        id: cid,
                                        start: cursor,
                                        origin: (c.x, c.y),
                                        moved: false,
                                    });
                                    self.sync_desktop_input_region();
                                }
                            } else {
                                self.action(h.action, WinRole::Desktop, el);
                            }
                        } else {
                            let closed = if let Some(ws) = &mut self.desktop {
                                if ws.menu.take().is_some() {
                                    ws.window.request_redraw();
                                    true
                                } else {
                                    false
                                }
                            } else {
                                false
                            };
                            if closed {
                                self.sync_desktop_input_region();
                            }
                        }
                    }
                } else {
                    // Left Released
                    self.shelf_drag = None;
                    if sidebar {
                        if let Some(d) = self.drawer_drag.take() {
                            if !d.moved {
                                self.sidebar_board.add(&d.kind);
                                self.persist(true);
                            } else if let Some(idx) = d.insert_index {
                                self.sidebar_board.insert_at(idx, &d.kind);
                                self.persist(true);
                            }
                            self.redraw_sidebar();
                        }
                    } else if let Some(d) = self.drag.take() {
                        if d.moved {
                            if let Some(c) =
                                self.desktop_board.cards.iter_mut().find(|c| c.id == d.id)
                            {
                                c.x = ((c.x / 16.).round() * 16.).max(8.);
                                c.y = ((c.y / 16.).round() * 16.).max(8.);
                            }
                            self.persist(false);
                        }
                        self.sync_desktop_input_region();
                        self.redraw_all();
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if role == WinRole::Island {
                    return;
                }
                let dy = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y * 48.,
                    MouseScrollDelta::PixelDelta(p) => p.y as f32 / w.window.scale_factor() as f32,
                };
                let (win_w, win_h) = w.size();
                if sidebar && w.drawer {
                    let in_drawer = match self.settings.edge {
                        SidebarEdge::Right => w.cursor.0 < win_w - ui::sidebar::PANEL_WIDTH,
                        SidebarEdge::Left => w.cursor.0 > ui::sidebar::PANEL_WIDTH,
                    };
                    if in_drawer {
                        let max_drawer =
                            (ui::sidebar::drawer_content_height() - (win_h - 116.)).max(0.);
                        self.drawer_scroll = (self.drawer_scroll - dy).clamp(0., max_drawer);
                        w.window.request_redraw();
                        return;
                    }
                }
                let pt = skia_safe::Point::from(w.cursor);
                let card = w
                    .hits
                    .iter()
                    .rev()
                    .find(|h| h.rect.contains(pt) && matches!(h.action, Action::Card(id) if id != 0))
                    .and_then(|h| match h.action {
                        Action::Card(id) => Some(id),
                        _ => None,
                    });
                let board = if sidebar {
                    &self.sidebar_board
                } else {
                    &self.desktop_board
                };
                if let Some(id) =
                    card.filter(|id| board.cards.iter().any(|c| c.id == *id && c.stack.len() > 1))
                {
                    self.action(Action::Turn(id, if dy > 0. { -1 } else { 1 }), role, el);
                } else if sidebar {
                    let content = if self.query.is_empty() {
                        state::pack_sidebar_blocks(&board.cards)
                            .iter()
                            .map(|p| p.y + p.h)
                            .fold(0., f32::max)
                    } else {
                        state::search(&self.query, &self.data.apps).len() as f32 * 58.
                    };
                    w.scroll = (w.scroll - dy).clamp(0., (content - (win_h - 270.)).max(0.));
                    w.window.request_redraw();
                }
            }
            WindowEvent::Ime(Ime::Commit(s)) => {
                if let Some((esb, eid)) = self.editing_note {
                    if esb == sidebar {
                        let board = if sidebar {
                            &mut self.sidebar_board
                        } else {
                            &mut self.desktop_board
                        };
                        if let Some(c) = board.cards.iter_mut().find(|c| c.id == eid) {
                            if c.note_text.len() + s.len() <= 2000 {
                                c.note_text.push_str(&s);
                            }
                        }
                        self.note_preedit.clear();
                        self.persist(sidebar);
                        self.redraw_role(role);
                        return;
                    }
                }
                if self.searching && sidebar {
                    if self.query.len() + s.len() < 1024 {
                        self.query.push_str(&s);
                    }
                    self.preedit.clear();
                    if let Some(ws) = &mut self.sidebar {
                        ws.scroll = 0.;
                        ws.window.request_redraw();
                    }
                }
            }
            WindowEvent::Ime(Ime::Preedit(s, _)) => {
                if self.editing_note.is_some_and(|(sb, _)| sb == sidebar) {
                    self.note_preedit = s;
                    w.window.request_redraw();
                } else if self.searching && sidebar {
                    self.preedit = s;
                    w.window.request_redraw();
                }
            }
            WindowEvent::Focused(false) => {
                if self.editing_note.is_some_and(|(sb, _)| sb == sidebar) {
                    self.finish_note_edit();
                } else if sidebar && self.searching {
                    self.searching = false;
                    w.window.set_ime_allowed(false);
                    platform::allow_focus(w.hwnd(), false);
                }
            }
            WindowEvent::KeyboardInput { event, .. } if event.state == ElementState::Pressed => {
                if let Some((esb, eid)) = self.editing_note {
                    if esb == sidebar {
                        match event.logical_key {
                            Key::Named(NamedKey::Escape) => {
                                self.finish_note_edit();
                            }
                            Key::Named(NamedKey::Enter) => {
                                let board = if sidebar {
                                    &mut self.sidebar_board
                                } else {
                                    &mut self.desktop_board
                                };
                                if let Some(c) = board.cards.iter_mut().find(|c| c.id == eid) {
                                    if c.note_text.len() < 2000 {
                                        c.note_text.push('\n');
                                    }
                                }
                                self.persist(sidebar);
                                self.redraw_role(role);
                            }
                            Key::Named(NamedKey::Backspace) => {
                                let board = if sidebar {
                                    &mut self.sidebar_board
                                } else {
                                    &mut self.desktop_board
                                };
                                if let Some(c) = board.cards.iter_mut().find(|c| c.id == eid) {
                                    c.note_text.pop();
                                }
                                self.persist(sidebar);
                                self.redraw_role(role);
                            }
                            _ if self.note_preedit.is_empty() => {
                                if let Some(t) = event.text {
                                    if !t.chars().any(char::is_control) {
                                        let board = if sidebar {
                                            &mut self.sidebar_board
                                        } else {
                                            &mut self.desktop_board
                                        };
                                        if let Some(c) = board.cards.iter_mut().find(|c| c.id == eid)
                                        {
                                            if c.note_text.len() + t.len() <= 2000 {
                                                c.note_text.push_str(&t);
                                            }
                                        }
                                        self.persist(sidebar);
                                        self.redraw_role(role);
                                    }
                                }
                            }
                            _ => {}
                        }
                        return;
                    }
                }

                match event.logical_key {
                    Key::Named(NamedKey::Escape) => {
                        if self.tutorial_open {
                            self.action(Action::TutorialDismiss, WinRole::Desktop, el);
                        } else if self.drawer_drag.take().is_some() {
                            self.redraw_sidebar();
                        } else if sidebar && self.settings_open {
                            self.settings_open = false;
                            self.redraw_sidebar();
                        } else if sidebar && w.drawer {
                            w.drawer = false;
                            self.last = Instant::now();
                            self.sync_sidebar_window_and_region();
                            self.redraw_sidebar();
                        } else if self.searching {
                            self.query.clear();
                            self.finish_search();
                        } else if sidebar && self.shown {
                            self.toggle_sidebar();
                        }
                    }
                    Key::Named(NamedKey::Enter) if self.searching && sidebar => {
                        self.action(Action::Result(0), WinRole::Sidebar, el);
                    }
                    Key::Named(NamedKey::Backspace) if self.searching && sidebar => {
                        self.query.pop();
                        w.window.request_redraw();
                    }
                    _ if self.searching && sidebar && self.preedit.is_empty() => {
                        if let Some(t) = event.text {
                            if !t.chars().any(char::is_control) && self.query.len() + t.len() < 1024
                            {
                                self.query.push_str(&t);
                                w.scroll = 0.;
                                w.window.request_redraw();
                            }
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
}
