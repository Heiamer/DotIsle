#![windows_subsystem = "windows"]
// The settings defaults are one large json! literal; the default macro recursion
// budget is not enough for it once the 1.3 keys are included.
#![recursion_limit = "256"]
mod activation;
mod backdrop_capture;
mod bottom_osd;
mod top_popup;
mod cover_color;
mod displays;
mod frame_pacing;
mod taskbar;
mod frame_stat;
mod material_preview;
mod motion_preview;

mod startup_preview;
mod surface;
mod update_activity;
mod window_level;
use std::os::windows::process::CommandExt;
pub const DISPLAY_VERSION: &str = env!("CARGO_PKG_VERSION");
mod activities;
#[cfg(test)]
mod activity_delete_tests;
mod app_icon;
mod app_rules;
mod banner;
mod capsule_alert;
mod device_data_preview;
mod resident;
mod gesture_practice;
use gesture_practice::Step as PracticeStep;
mod resident_preview;
mod verification_code;
mod volume;
mod screen_time;
mod reminder_plugins;
mod ai_quota;
mod agent_island;
mod agent_desktop;
mod compact;
mod config;
mod devices;
mod diagnostics;
mod foreground;
mod glow_preview;
mod history;
mod immersive_preview;
mod island_glow;
mod module_canvas;
mod module_updates;
mod layout;
mod liquid_preview;
mod liquid_timer;
mod m3e_preview;
mod motion;
mod morph_transition;
mod notifications;
mod osd;
mod progress;
mod quick_reminder;
mod renderer;
mod sound;
mod theme_accent;
mod timers;
mod todos;
mod vello_surface;

mod overview_widgets;
mod widget_data;
mod widget_metrics;
use base64::Engine;
use layout::{Layout, Rect};
use lyric_backend::{io, ipc::Server, runtime::Backend, shell};
use motion::{Frame, Options, Transition};
use renderer::{Renderer, Scene};
use serde_json::{json, Value};
use std::{
    cell::RefCell,
    ffi::{c_char, CStr},
    path::{Path, PathBuf},
    sync::{mpsc, Arc, Mutex, OnceLock},
    time::{Duration, Instant},
};
use windows::{
    core::{w, Result, PCWSTR},
    Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::{
            Com::*,
            LibraryLoader::GetModuleHandleW,
            RemoteDesktop::{
                ProcessIdToSessionId, WTSRegisterSessionNotification,
                WTSUnRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION,
            },
            Threading::*,
            WindowsProgramming::QueryUnbiasedInterruptTime,
        },
        UI::{HiDpi::*, Input::KeyboardAndMouse::*, Shell::*, WindowsAndMessaging::*},
    },
};
const WM_TRAY: u32 = WM_APP + 1;
const WM_SETTINGS: u32 = WM_APP + 2;
const WM_APPLY_WINDOW_LEVEL: u32 = WM_APP + 4;
static TASKBAR_CREATED_MSG: OnceLock<u32> = OnceLock::new();
fn taskbar_created_msg() -> u32 {
    *TASKBAR_CREATED_MSG.get_or_init(|| unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) })
}
/// Synthetic AUMID for reminder banners, so a click opens the todo page instead
/// of trying to activate an external application.
const TODO_APP_ID: &str = "LyricIsland.Todo";
type Pending = (Value, mpsc::Sender<Value>);
/// One queued banner. Reminder banners are tracked by todo id so a completion or
/// deletion can cancel a banner that has not been presented yet.
struct Queued {
    pub notice: notifications::Notice,
    pub todo: Option<String>,
    pub chime: bool,
}
/// A parsed reminder awaiting an explicit commit. The token, not the original
/// text, is what commit accepts, so "明天"/"N分钟后" is never reinterpreted.
struct QuickPreview {
    pub token: String,
    pub title: String,
    pub date: String,
    pub time: String,
    pub expires: Instant,
}
/// Focus timers parked by the current lock. Kept after restart only as ordinary
/// paused state; nothing resumes automatically without a matching generation.
struct FocusResume {
    pub generation: u64,
    pub ids: Vec<String>,
    pub prompted: bool,
}
fn hotkey_code(name: &str) -> Option<(HOT_KEY_MODIFIERS, u32)> {
    // One parser for the recorder and the registration, so a binding the settings window accepts
    // is never one this side quietly drops.
    let hotkey = lyric_core::hotkey::Hotkey::parse(name)?;
    let mut mods = HOT_KEY_MODIFIERS(0);
    if hotkey.ctrl {
        mods |= MOD_CONTROL
    }
    if hotkey.alt {
        mods |= MOD_ALT
    }
    if hotkey.shift {
        mods |= MOD_SHIFT
    }
    if hotkey.win {
        mods |= MOD_WIN
    }
    Some((mods, hotkey.vk))
}
fn awake_ms() -> u64 {
    let mut value = 0u64;
    unsafe {
        let _ = QueryUnbiasedInterruptTime(&mut value);
    }
    (value / 10_000) as u64
}
static REQUESTS: OnceLock<Mutex<Vec<Pending>>> = OnceLock::new();
static WINDOW: OnceLock<isize> = OnceLock::new();
fn inside_round_rect(r: Rect, radius: f64, x: f64, y: f64) -> bool {
    if ![r.x, r.y, r.w, r.h, radius, x, y]
        .iter()
        .all(|v| v.is_finite())
        || !r.contains(x, y)
    {
        return false;
    }
    let radius = radius.min(r.w / 2.).min(r.h / 2.).max(0.);
    // Work in local coordinates. At half-height, separately rounded absolute
    // clamp endpoints can reverse by one ULP during a morph and panic in f64::clamp.
    let dx = (((x - r.x) - r.w / 2.).abs() - (r.w / 2. - radius).max(0.)).max(0.);
    let dy = (((y - r.y) - r.h / 2.).abs() - (r.h / 2. - radius).max(0.)).max(0.);
    dx * dx + dy * dy <= radius * radius
}
/// "Appear when needed, retract once handled": decide whether the island must be
/// present from what is asking for attention right now. A feature merely being
/// switched on, or merely being selectable, is not a reason to stay on screen.
fn island_needed(
    valid: bool,
    playing: bool,
    hide_when_paused: bool,
    running_timer: bool,
    banner: bool,
    queue: bool,
    held: bool,
    manual: bool,
) -> bool {
    valid && (playing || !hide_when_paused) || running_timer || banner || queue || held || manual
}
unsafe extern "C" fn settings_callback(request: *const c_char) -> *mut c_char {
    let response = std::panic::catch_unwind(|| {
        let request = serde_json::from_slice::<Value>(CStr::from_ptr(request).to_bytes())
            .unwrap_or(Value::Null);
        let (tx, rx) = mpsc::channel();
        REQUESTS
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .push((request, tx));
        if let Some(h) = WINDOW.get() {
            let _ = PostMessageW(Some(HWND(*h as _)), WM_SETTINGS, WPARAM(0), LPARAM(0));
        }
        rx.recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|_| json!({"success":false,"message":"主窗口未响应"}))
    })
    .unwrap_or_else(|_| json!({"success":false,"message":"设置请求失败"}));
    let bytes = response.to_string();
    let p = CoTaskMemAlloc(bytes.len() + 1) as *mut u8;
    if !p.is_null() {
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), p, bytes.len());
        *p.add(bytes.len()) = 0;
    }
    p as _
}
struct App {
    banner: banner::Banner,
    osd_banner: banner::Banner,
    alerts: capsule_alert::Hub,
    device_watch: capsule_alert::DeviceWatch,
    weather_watch: capsule_alert::WeatherWatch,
    screen_time: screen_time::Tracker,
    reminder_plugins: reminder_plugins::Scheduler,
    ai_quota: ai_quota::Monitor,
    agent: agent_island::Monitor,
    agent_previous: Option<String>,
    agent_focus_return: HWND,
    agent_auto_open: bool,
    agent_pressed_token: Option<u64>,
    plugins: dotisle_domain::plugins::Registry,
    last_context_poll: Instant,
    osd: osd::Hub,
    pending_osd: u32,
    notification_preview: bool,
    banner_todo: Option<String>,
    notification_listener: Option<notifications::Listener>,
    notification_state: Value,
    hwnd: HWND,
    visual_hwnd: HWND,
    input_region: Option<[i32; 10]>,
    dragging: Option<(POINT, RECT)>,
    float_drag_arm: Option<(POINT, POINT, RECT)>,
    edge_hover_since: Option<Instant>,
    edge_revealed: bool,
    edge_leave_since: Option<Instant>,
    last_taskbar_probe: Option<taskbar::TaskbarProbe>,
    last_island_visibility_check: Instant,
    last_wellness_guard_check: Instant,
    taskbar_suppressed: bool,
    fullscreen_suppressed: bool,
    taskbar_light: bool,
    todo_scroll: f64,
    todo_scroll_target: f64,
    renderer: Option<Renderer>,
    base: PathBuf,
    data: PathBuf,
    settings: Value,
    wechat_warning: Option<(String, Instant)>,
    backend: Option<Arc<Backend>>,
    _server: Option<Server>,
    snapshot: Value,
    cover_rx: Option<
        mpsc::Receiver<(
            u64,
            std::result::Result<Option<renderer::PreparedCover>, String>,
        )>,
    >,
    pending_cover: Option<Option<renderer::PreparedCover>>,
    cover_version: u64,
    demo: bool,
    layout: Layout,
    frame: Frame,
    transition: Option<Transition>,
    transition_start: Instant,
    sampled_ms: f64,
    music_controls: lyric_core::shared_motion::Spring,
    play_shape: lyric_core::shared_motion::Spring,
    feedback: [lyric_core::shared_motion::Spring; 64],
    feedback_moving: bool,
    last_dynamic_system: Option<u32>,
    expanded: bool,
    held: bool,
    suppress_hover_expand: bool,
    hover_since: Instant,
    tooltip_visible: bool,
    volume: volume::Volume,
    hover: i32,
    pressed: i32,
    seeking: bool,
    seek_fraction: f64,
    seek_track: String,
    seek_confirm: bool,
    command_busy: Arc<std::sync::atomic::AtomicBool>,
    start: Instant,
    last_poll: Instant,
    last_visible: Instant,
    lyric: String,
    previous_lyric: String,
    compact_secondary: String,
    previous_compact_secondary: String,
    lyric_start: Instant,
    compact_size: compact::Resize,
    compact_key: String,
    last_compact: Instant,
    todos: todos::Store,
    todo_now: todos::Now,
    last_schedule: Instant,
    history: history::Store,
    last_history_prune: Instant,
    queue: std::collections::VecDeque<Queued>,
    pending_page: Option<String>,
    pending_request: u64,
    pending_center_action: Value,
    practice: Option<gesture_practice::Practice>,
    updates: update_activity::Hub,
    update_return: String,
    update_until: Option<Instant>,
    update_card_deleted: bool,
    devices: devices::Hub,
    timers: timers::Store,
    selected: String,
    components: Vec<String>,
    panel: Option<activities::Panel>,
    previous_panel: Option<activities::Panel>,
    drag_panel: Option<activities::Panel>,
    drag_target: String,
    slide_outgoing: bool,
    switching: bool,
    slide: activities::Spring,
    slide_direction: f64,
    swipe: Option<activities::Swipe>,
    press_origin: Option<(f64, f64)>,
    last_timer: Instant,
    last_tick: Instant,
    timer_fraction: u128,
    last_checkpoint: Instant,
    cursor_inside: bool,
    leave_at: Option<Instant>,
    last_left: bool,
    user_hidden: bool,
    hidden_mini: bool,
    island_morph: morph_transition::IslandMorph,
    // A verification code surfaced while the island was manually hidden; the
    // hidden state returns once its alert is gone.
    restore_hidden: bool,
    display_wanted: bool,
    missing_since: Option<Instant>,
    manual_until: Option<Instant>,
    opacity: f64,
    fade_from: f64,
    fade_to: f64,
    fade_start: Instant,
    hide_at: Option<Instant>,
    dirty: bool,
    /// When the steady playback loop last drew. See `frame_pacing::CONTINUOUS_FPS`.
    continuous_last: Instant,
    glow: island_glow::State,
    module_updates:module_updates::Manager,
    widget_data: widget_data::Hub,
    desktop_widgets: dotisle_domain::desktop_widgets::Store,
    overview_widgets: overview_widgets::Store,
    widget_metrics: widget_metrics::Hub,
    resident: resident::Rotation,
    last_widget_host: Instant,
    greeting_pending: bool,
    center_process: Option<std::process::Child>,

    center_started: bool,
    settings_wake: Option<io::SettingsWake>,
    renderer_generation: u64,
    render_frames: u64,
    error: Option<String>,
    transparent: bool,
    activity: motion::Activity,
    cover_start: Instant,
    progress: progress::PlaybackProgress,
    outgoing_opacity: f64,
    outgoing_y: f64,
    quick: Vec<QuickPreview>,
    quick_commits: Vec<(String, String)>,
    pending_action: Option<String>,
    templates: timers::Templates,
    rules: app_rules::Store,
    foreground: foreground::Watcher,
    usage_date: String,
    last_awake: u64,
    last_rules_checkpoint: Instant,
    lock_generation: u64,
    session_locked: bool,
    focus_resume: Option<FocusResume>,
    /// One in-flight silent-control enable run; the result message is picked up by tick().
    bottom_osd: bottom_osd::Overlay,
    top_popup: top_popup::Overlay,
    music_mix: f64,
    music_from: f64,
    music_start: Instant,
    music_custom: lyric_core::custom_layout::Layout,
    custom_from: Option<lyric_core::music_layout::MusicGeometry>,
    custom_start: Instant,
    /// Last app notification that was queued, so a store that reports the same
    /// message twice cannot make the banner open, close and open again.
    last_notice: Option<(String, f64)>,
    /// When the collapsed lyric last changed. Width retargets wait for it to settle.
    compact_stability: compact::Stability,
    acrylic_preview: Value,
    backdrop_capturer: backdrop_capture::BackdropCapturer,
}
impl App {
    fn new(base: PathBuf, data: PathBuf, demo: bool) -> Self {
        lyric_backend::updates::set_runtime_version(DISPLAY_VERSION);
        // Demo builds paint a synthetic track, but a font probe must still resolve the real
        // settings.json + preferences.json so the island honours fontScope exactly as shipping does.
        let settings = if demo && std::env::var_os("LYRICISLAND_FONT_PROBE").is_none() {
            config::defaults()
        } else {
            config::load(&data.join("settings.json"))
        };
        let options =
            serde_json::from_value::<Options>(settings["Motion"].clone()).unwrap_or_default();
        let backend = if demo {
            None
        } else {
            Some(Backend::new(
                settings.clone(),
                data.clone(),
                PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default()),
                data.join("Logs"),
            ))
        };
        let module_updates=module_updates::Manager::new(&base,&data,demo);
        let now = Instant::now();
        let snapshot = if demo {
            json!({"track":{"id":"demo","title":"晚风电台","artist":"DotIsle","sourceApp":"演示","durationSeconds":236.},"isPlaying":true,"positionSeconds":83.,"canSeek":true,"frame":{"currentLine":"景色は移ろい瞬く間に変わる","currentTranslation":"景色转瞬即逝","nextLine":"変わらないものはない","wordScanProgress":0.4}})
        } else {
            Value::Null
        };
        let notification_listener = if demo {
            None
        } else {
            Some(notifications::Listener::new(
                notifications::Policy::from_settings(&settings),
            ))
        };
        // Demo runs must never touch the user's real todo file.
        let todos = todos::Store::new(if demo {
            PathBuf::new()
        } else {
            data.join("todos.json")
        });
        let todo_now = todos::Now::local();
        let timers = timers::Store::new(if demo {
            PathBuf::new()
        } else {
            data.join("timers.json")
        });
        let widget_data = widget_data::Hub::new(if demo {
            PathBuf::new()
        } else {
            data.join("focus-stats.json")
        });
        let desktop_widgets = dotisle_domain::desktop_widgets::Store::new(if demo {
            PathBuf::new()
        } else {
            data.join("desktop-widgets.json")
        });
        let overview_widgets = overview_widgets::Store::new(if demo {
            PathBuf::new()
        } else {
            data.join("overview-widgets.json")
        });
        let templates = timers::Templates::new(if demo {
            PathBuf::new()
        } else {
            data.join("workflows.json")
        });
        let foreground = foreground::Watcher::new();
        let mut rules = app_rules::Store::new(
            if demo {
                PathBuf::new()
            } else {
                data.join("rules.json")
            },
            if demo {
                PathBuf::new()
            } else {
                data.join("usage.json")
            },
            &todo_now.date,
        );
        rules.baseline_now(foreground.path.as_deref(), foreground.segment);
        let usage_date = todo_now.date.clone();
        let plugins=dotisle_domain::plugins::Registry::new(data.join("plugins.json"));
        let screen_time = screen_time::Tracker::new(if demo {PathBuf::new()}else{data.join("screen-time.json")},awake_ms());
        let reminder_plugins = reminder_plugins::Scheduler::new(&settings["ReminderPlugins"]);
        let mut history = history::Store::new(if demo {
            PathBuf::new()
        } else {
            data.join("history.json")
        });
        let history_days = || {
            settings["NotificationHistoryDays"]
                .as_u64()
                .unwrap_or(7)
                .clamp(1, 30) as u32
        };
        let epoch = || {
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs() as i64
        };
        history.prune(history_days(), epoch());
        let compact_size =
            compact::Resize::new(settings["CompactWidth"].as_f64().unwrap_or(260.), 40.);
        let music_custom =
            lyric_core::custom_layout::Layout::from_value(&settings["MusicCustomLayout"])
                .unwrap_or_default();
        let music_mix = layout::MusicLayout::from_setting(
            settings["MusicExpandedLayout"].as_str().unwrap_or(""),
        )
        .position();
        let mut layout = Layout::new(settings["CompactWidth"].as_f64().unwrap_or(260.));
        layout.card.h =
            layout::music_height(music_mix, settings["FontSize"].as_f64().unwrap_or(12.5));
        let updates = update_activity::Hub::new(&data);
        Self {
            bottom_osd: bottom_osd::Overlay::new(),
            top_popup: top_popup::Overlay::new(),
            updates,
            update_return: "music".into(),
            update_until: None,
            update_card_deleted: false,
            banner: banner::Banner::default(),
            osd_banner: banner::Banner::default(),
            osd: osd::Hub::default(),
            pending_osd: 0,
            notification_preview: false,
            banner_todo: None,
            notification_listener,
            notification_state: json!({"access":"disabled","message":"通知提醒已关闭","apps":[]}),
            hwnd: HWND::default(),
            visual_hwnd: HWND::default(),
            input_region: None,
            dragging: None,
            float_drag_arm: None,
            edge_hover_since: None,
            edge_revealed: false,
            edge_leave_since: None,
            last_taskbar_probe: None,
            last_island_visibility_check: now - Duration::from_secs(1),
            last_wellness_guard_check: now - Duration::from_secs(1),
            taskbar_suppressed: false,
            fullscreen_suppressed: false,
            taskbar_light: taskbar::is_system_light_taskbar(),
            todo_scroll: 0.,
            todo_scroll_target: 0.,
            renderer: None,
            base,
            data,
            layout,
            settings,
            wechat_warning: None,
            backend,
            _server: None,
            snapshot,
            cover_rx: None,
            pending_cover: None,
            cover_version: 0,
            demo,
            frame: Frame::hidden(options),
            transition: None,
            transition_start: now,
            sampled_ms: 0.,
            music_controls: lyric_core::shared_motion::Spring::new(0.),
            play_shape: lyric_core::shared_motion::Spring::new(0.),
            feedback: [lyric_core::shared_motion::Spring::new(0.); 64],
            feedback_moving: false,
            last_dynamic_system: None,
            expanded: false,
            held: false,
            suppress_hover_expand: false,
            hover_since: now,
            tooltip_visible: false,
            volume: volume::Volume::default(),
            hover: -1,
            pressed: 0,
            seeking: false,
            seek_fraction: 0.,
            seek_track: String::new(),
            seek_confirm: false,
            command_busy: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            start: now,
            last_poll: now - Duration::from_secs(1),
            last_visible: now,
            lyric: "打开播放器，开始一首音乐".into(),
            previous_lyric: String::new(),
            compact_secondary: String::new(),
            previous_compact_secondary: String::new(),
            lyric_start: now - Duration::from_secs(2),
            compact_size,
            compact_key: String::new(),
            last_compact: now,
            todos,
            todo_now,
            last_schedule: now,
            history,
            last_history_prune: now,
            queue: std::collections::VecDeque::new(),
            pending_page: None,
            pending_request: 0,
            pending_center_action: Value::Null,
            practice: None,
            devices: devices::Hub::default(),
            timers,
            selected: "music".into(),
            components: vec!["music".into()],
            panel: None,
            previous_panel: None,
            drag_panel: None,
            drag_target: String::new(),
            slide_outgoing: false,
            switching: false,
            slide: activities::Spring::default(),
            slide_direction: 1.,
            swipe: None,
            press_origin: None,
            last_timer: now,
            last_tick: now,
            timer_fraction: 0,
            last_checkpoint: now,
            cursor_inside: false,
            leave_at: None,
            last_left: false,
            user_hidden: false,
            hidden_mini: false,
            island_morph: morph_transition::IslandMorph::new(),
            restore_hidden: false,
            display_wanted: demo,
            missing_since: None,
            manual_until: None,
            opacity: if demo { 1. } else { 0. },
            fade_from: 0.,
            fade_to: if demo { 1. } else { 0. },
            fade_start: now,
            hide_at: None,
            dirty: true,
            continuous_last: now - Duration::from_secs(1),
            glow: Default::default(),
            module_updates,
            widget_data,
            desktop_widgets,
            overview_widgets,
            widget_metrics: Default::default(),
            resident: Default::default(),
            last_widget_host: now - Duration::from_secs(6),
            greeting_pending: false,
            alerts: Default::default(),
            device_watch: Default::default(),
            weather_watch: Default::default(),
            screen_time,
            reminder_plugins,
            ai_quota: ai_quota::Monitor::default(),
            agent: Default::default(),
            agent_previous: None,
            agent_focus_return: HWND::default(),
            agent_auto_open: false,
            agent_pressed_token: None,
            plugins,
            last_context_poll: now-Duration::from_secs(2),
            center_process: None,

            center_started: false,
            settings_wake: io::SettingsWake::new(),
            renderer_generation: 0,
            render_frames: 0,
            error: None,
            transparent: false,
            activity: motion::Activity::new(),
            cover_start: now - Duration::from_secs(1),
            progress: progress::PlaybackProgress::default(),
            outgoing_opacity: 1.,
            outgoing_y: 0.,
            music_mix,
            music_from: music_mix,
            music_start: now,
            music_custom,
            custom_from: None,
            custom_start: now,
            last_notice: None,
            acrylic_preview: json!({}),
            compact_stability: compact::Stability::default(),
            backdrop_capturer: backdrop_capture::BackdropCapturer::default(),
            quick: vec![],
            quick_commits: vec![],
            pending_action: None,
            templates,
            rules,
            foreground,
            usage_date,
            last_awake: awake_ms(),
            last_rules_checkpoint: now,
            lock_generation: 0,
            session_locked: false,
            focus_resume: None,
        }
    }
    fn glow_target(&self) -> island_glow::Kind {
        use island_glow::Kind;
        if self.mini() || self.opacity <= 0.001 {
            return Kind::Off;
        }
        if self.flag("UpdateGlowEnabled", false)
            && matches!(
                self.updates.phase(),
                "checking" | "downloading" | "verifying" | "applying" | "restarting"
            )
        {
            return Kind::Update;
        }
        if self.flag("TimerGlowEnabled", false)
            && self.panel.as_ref().is_some_and(|p| {
                p.kind == 1 && p.running && p.timer.as_ref().is_some_and(|t| !t.indefinite)
            })
        {
            return Kind::Timer;
        }
        if self.flag("MusicGlowEnabled", false)
            && self.selected == "music"
            && self.snapshot["isPlaying"] == true
        {
            return if self.settings["MusicGlowMode"] == "Rhythm" {
                Kind::MusicRhythm
            } else {
                Kind::MusicBreath
            };
        }
        Kind::Off
    }
    fn liquid_timer_visible(&self) -> bool {
        self.opacity > 0.001
            && !self.expanded
            && self.flag("Animation", true)
            && self.panel.as_ref().is_some_and(|p| {
                p.kind == 1 && p.running && p.timer.as_ref().is_some_and(|timer| !timer.indefinite)
            })
    }
    fn taskbar_mode(&self) -> bool {
        self.flag("TaskbarModeEnabled", false) && self.practice.is_none()
    }
    fn island_suppressed(&self) -> bool {
        self.taskbar_suppressed || self.fullscreen_suppressed
    }
    fn taskbar_always_show(&self) -> bool {
        self.taskbar_mode() && self.flag("TaskbarAlwaysShow", true)
    }
    fn taskbar_width(&self) -> f64 {
        self.settings["TaskbarWidth"]
            .as_f64()
            .unwrap_or(340.)
            .clamp(220., 640.)
    }
    fn effective_topmost(&self) -> bool {
        self.flag("AlwaysOnTop", true) || self.taskbar_mode()
    }
    fn refresh_components(&mut self) {
        if self.practice.is_some() {
            self.practice_components();
            return;
        }
        if (self.flag("IslandKeepVisibleEnabled", false) || self.taskbar_always_show())
            && !self.demo && self.snapshot["isPlaying"] != true && !self.active_tasks()
            && !(self.updates.selectable() && !self.update_card_deleted)
        {
            if self.selected != resident::ID {
                self.selected = resident::ID.into();
                self.components.clear();
                self.components.push(resident::ID.into());
                self.panel = Some(activities::Panel { kind: resident::KIND, ..Default::default() });
                self.compact_key.clear();
                self.dirty = true;
            }
            if !self.user_hidden || self.restore_hidden {
                self.update_resident();
            } else {
                self.resident.suspend();
            }
            return;
        }
        let leaving_resident = self.selected == resident::ID;
        if leaving_resident { self.resident.suspend(); }
        let mut components = vec![];
        if self.updates.selectable() && !self.update_card_deleted {
            components.push("update".into());
        }
        if self.demo || self.snapshot["track"].is_object() {
            components.push("music".to_string());
        }
        if self.flag("TodoIslandEnabled", true)
            && self.todos.items.iter().any(|t| !t.done(&self.todo_now))
        {
            components.push("todos".into());
        }
        for t in &self.timers.items {
            components.push(format!("timer:{}", t.id));
        }
        if self.agent_visible() { components.push(agent_island::ID.into()); }
        components.extend(self.plugins.island_ids());
        if components.is_empty() {
            components.push("music".into());
        }
        if !components.contains(&self.selected) {
            let running = self.timers.items.iter().find(|t| t.running && !t.finished);
            self.selected = if leaving_resident && components[0] != "update" && self.snapshot["isPlaying"] != true {
                running.map_or_else(|| components[0].clone(), |t| format!("timer:{}", t.id))
            } else {
                components[0].clone()
            };
        }
        self.components = components;
        self.panel = self.component_panel(&self.selected);
    }
    fn update_resident(&mut self) {
        let expanded = self.expanded || self.frame.card_reveal > 0.01;
        let paused = expanded || self.cursor_inside || self.banner.visible()
            || self.osd_banner.visible() || self.bottom_osd.visible()
            || self.alerts.visible() || self.alerts.pending() || !self.queue.is_empty();
        let choices = resident::Choices {
            time: self.flag(resident::TIME, true),
            system: self.flag(resident::SYSTEM, false),
            devices: self.flag(resident::DEVICES, false),
        };
        if self.resident.update(Instant::now(), choices, &self.widget_metrics.value, paused, expanded, || {
            let local = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
            (local.wHour, local.wMinute)
        }) {
            self.dirty = true;
        }
    }
    fn agent_visible(&self) -> bool { !self.agent.view.tiles.is_empty() || self.agent.view.prompt.is_some() }
    fn component_panel(&self, id: &str) -> Option<activities::Panel> {
        if id == agent_island::ID {
            return Some(activities::Panel { kind: agent_island::KIND, ..Default::default() });
        }
        if self.practice.is_some() { return gesture_practice::panel(id); }
        if id == "update" {
            Some(self.updates.panel())
        } else if id == "todos" {
            let tasks = self
                .todos
                .items
                .iter()
                .filter(|t| !t.done(&self.todo_now))
                .take(120)
                .map(|t| activities::Task {
                    id: t.id.clone(),
                    title: t.title.clone(),
                    detail: format!(
                        "{} {}{}",
                        t.date,
                        t.time,
                        if t.daily { " · 每天" } else { "" }
                    ),
                    color: 0xff000000
                        | u32::from_str_radix(t.color.trim_start_matches('#'), 16)
                            .unwrap_or(0xf3d79b),
                })
                .collect();
            Some(activities::Panel {
                kind: 2,
                title: "待办清单".into(),
                value: format!(
                    "{} 项待办",
                    self.todos
                        .items
                        .iter()
                        .filter(|t| !t.done(&self.todo_now))
                        .count()
                ),
                subtitle: "记住重要的小事".into(),
                accent: 0xfff3d79b,
                tasks,
                ..Default::default()
            })
        } else if let Some(id) = id.strip_prefix("plugin:") {
            let p=self.plugins.get(id)?;let blocks:Vec<dotisle_domain::plugins::ResolvedBlock>=serde_json::from_value(p["views"]["island"].clone()).ok()?;
            let value=blocks.iter().find(|b|b.kind=="metric"||b.kind=="text").map(|b|b.text.clone()).unwrap_or_default();
            Some(activities::Panel{kind:4,title:p["name"].as_str().unwrap_or(id).into(),value,blocks,..Default::default()})
        } else if let Some(id) = id.strip_prefix("timer:") {
            self.timers
                .items
                .iter()
                .find(|t| t.id == id)
                .map(|t| t.panel())
        } else {
            None
        }
    }
    fn select_component(&mut self, id: &str, direction: i32) {
        if !self.components.iter().any(|c| c == id) || self.selected == id {
            return;
        }
        if self.selected==agent_island::ID {
            self.agent_auto_open=false;
            if self.agent.view.editing {unsafe{self.end_agent_edit(false,true);}}
        }
        self.hide_at = None;
        self.manual_until = Some(Instant::now() + Duration::from_secs(6));
        self.slide_outgoing = true;
        self.previous_panel = self.panel.clone();
        self.selected = id.into();
        self.refresh_components();
        self.compact_key.clear();
        self.slide_direction = if direction < 0 { -1. } else { 1. };
        self.switching = self.flag("Animation", true);
        self.slide.set(if self.switching {
            self.slide_direction
                * if self.expanded {
                    self.layout.card.w
                } else {
                    self.layout.capsule.w
                }
        } else {
            0.
        });
        self.dirty = true;
    }
    fn cycle_component(&mut self, direction: i32) {
        self.refresh_components();
        let n = self.components.len();
        if n < 2 {
            return;
        }
        let i = self
            .components
            .iter()
            .position(|c| c == &self.selected)
            .unwrap_or(0);
        let next = (i as i32 + direction).rem_euclid(n as i32) as usize;
        let id = self.components[next].clone();
        self.select_component(&id, direction);
    }
    fn hidden_display(&self) -> &str {
        self.settings["HiddenDisplay"].as_str().unwrap_or("Strip")
    }
    fn practice_preview(&self) -> bool {
        self.practice.is_none() || self.practice_step() == Some(PracticeStep::Recover)
    }
    fn hidden_mini_mode(&self) -> bool {
        self.hidden_mini
            && self.user_hidden
            && !self.edge_revealed
            && !self.taskbar_mode()
            && self.practice_preview()
    }
    fn hidden_edge(&self) -> bool {
        self.user_hidden
            && !self.edge_revealed
            && !self.hidden_mini
            && !self.taskbar_mode()
            && self.practice_preview()
            && self.hidden_display() == "Strip"
    }
    fn hidden_edge_rect(&self) -> Rect {
        let c = self.layout.capsule;
        let width = 56.0_f64.min(c.w.max(1.));
        Rect::new(c.x + (c.w - width) / 2., c.y, width, 5.)
    }
    /// 蓝条视觉保持 56×5，点击热区垂直方向扩展至 14dp、水平各扩 6dp，避免过细难触发。
    fn hidden_edge_hit_rect(&self) -> Rect {
        let bar = self.hidden_edge_rect();
        let x = (bar.x - 6.).max(0.);
        let y = (bar.y - 3.).max(0.);
        let w = (bar.w + 12.).min((self.layout.host.w - x).max(1.));
        let h = 14.0_f64.min((self.layout.host.h - y).max(1.));
        Rect::new(x, y, w, h)
    }
    fn capsule_corner_radius(&self, h: f64) -> f64 {
        if self.layout.anchor_bottom {
            8.0
        } else {
            self.settings["CapsuleCornerRadius"]
                .as_f64()
                .unwrap_or(h / 2.)
                .clamp(0., h / 2.)
        }
    }
    /// 捕获当前壳体实时视觉几何与混合权重，用于无缝启动或中途打断四向形变。
    fn live_morph_origin(&self, now: f64) -> (Rect, f64, f64, f64, f64) {
        if self.island_morph.active() {
            let p = self.island_morph.presentation(now, self.layout.host);
            return (p.rect, p.radius, p.strip_mix, p.mini_mix, p.card_mix);
        }
        if self.hidden_edge() {
            return (self.hidden_edge_rect(), 2.5, 1., 0., 0.);
        }
        let (rect, rad) = self.shape();
        let mini_mix = if self.mini() { 1. } else { 0. };
        let card_mix = if self.expanded || self.frame.card_reveal > 0.01 {
            self.frame.card_reveal.max(if self.expanded { 1. } else { 0. })
        } else {
            0.
        };
        (rect, rad, 0., mini_mix, card_mix)
    }
    fn enter_hidden_edge(&mut self) {
        let now0 = self.start.elapsed().as_secs_f64();
        let (mut from_rect, mut from_radius, from_strip, from_mini, from_card) =
            self.live_morph_origin(now0);
        if from_rect.w < 10. || from_rect.h < 4. {
            from_rect = self.layout.capsule;
            from_radius = self.capsule_corner_radius(from_rect.h);
        }
        let was_visible = !self.taskbar_mode()
            && (self.opacity > 0.01 || self.display_wanted || self.island_morph.active());
        let collapse_ms = self.options().collapse_duration;

        // 若从展开态直接隐藏，复位卡片展开状态并沿用当前大轮廓直接缩成蓝条/Mini
        if self.expanded || self.frame.card_reveal > 0.001 {
            self.expanded = false;
            self.held = false;
            self.transition = None;
            self.frame = Frame::hidden(self.options());
        }

        self.user_hidden = true;
        self.restore_hidden = false;
        self.edge_revealed = false;
        self.edge_hover_since = None;
        self.edge_leave_since = None;
        self.hide_at = None;
        self.missing_since = None;
        self.suppress_hover_expand = false;
        let display = self.hidden_display().to_owned();
        self.hidden_mini = display == "Mini";
        if display == "None" {
            self.island_morph.cancel();
            self.display_wanted = false;
            self.fade(0.);
        } else {
            self.display_wanted = true;
            self.opacity = 1.;
            self.fade(1.);
        }
        self.dirty = true;
        unsafe {
            let _ = self.position();
            let now = self.start.elapsed().as_secs_f64();
            if display != "None" {
                let _ = window_level::apply(self.visual_hwnd, self.hwnd, true, false);
            }
            if display == "Strip" && !self.taskbar_mode() && self.hidden_edge() {
                let target_rect = self.hidden_edge_rect();
                self.island_morph.transition_with_card(
                    from_rect,
                    from_radius,
                    from_strip,
                    from_mini,
                    from_card,
                    morph_transition::IslandMode::Strip,
                    target_rect,
                    2.5,
                    now,
                    self.flag("Animation", true),
                    collapse_ms,
                );
            } else if display == "Mini" && was_visible && self.hidden_mini_mode() {
                self.poll();
                let (tw, th) = self
                    .measure_compact_layout()
                    .unwrap_or((self.compact_base(), 14.));
                self.compact_size = compact::Resize::new(tw, th);
                self.layout.set_compact(tw, th);
                let target_rect = self.layout.capsule;
                let target_rad = self.capsule_corner_radius(th);
                self.island_morph.transition_with_card(
                    from_rect,
                    from_radius,
                    from_strip,
                    from_mini,
                    from_card,
                    morph_transition::IslandMode::Mini,
                    target_rect,
                    target_rad,
                    now,
                    self.flag("Animation", true),
                    collapse_ms,
                );
            }
            let _ = self.sync_input_region();
        }
    }
    fn reveal_hidden_edge(&mut self) {
        let now0 = self.start.elapsed().as_secs_f64();
        let was_strip = self.hidden_edge()
            || (self.island_morph.active()
                && self.island_morph.mode() == morph_transition::IslandMode::Strip);
        let was_morphable = !self.taskbar_mode()
            && (self.hidden_edge() || self.hidden_mini_mode() || self.island_morph.active());
        let (from_rect, from_radius, from_strip, from_mini, from_card) =
            self.live_morph_origin(now0);
        let expand_ms = self.options().duration;

        // 还原回紧凑灵动岛：强制维持未展开状态并阻断悬停误展开
        self.expanded = false;
        self.held = false;
        self.transition = None;
        self.frame = Frame::hidden(self.options());
        self.suppress_hover_expand = true;
        self.leave_at = None;

        self.user_hidden = false;
        self.hidden_mini = false;
        self.edge_revealed = false;
        self.edge_leave_since = None;
        self.restore_hidden = false;
        self.display_wanted = true;
        self.opacity = 1.;
        self.manual_until = Some(Instant::now() + Duration::from_secs(6));
        self.fade(1.);
        self.dirty = true;
        if self.practice_step() == Some(PracticeStep::Recover) {
            self.practice_event(PracticeStep::Recover);
        }

        // 内容实时接力：先立即同步当前时间线歌词/译文/封面（未唱完保留该句，已唱完切至下一句）
        self.poll();
        if was_strip {
            self.previous_lyric.clear();
            self.previous_compact_secondary.clear();
            self.lyric_start = Instant::now() - Duration::from_millis(300);
        }

        unsafe {
            let _ = self.position();
            let now = self.start.elapsed().as_secs_f64();
            let _ = window_level::apply(self.visual_hwnd, self.hwnd, self.effective_topmost(), false);
            if was_morphable {
                let default_h = if self.mini() { 14. } else { 40. };
                let (tw, th) = self
                    .measure_compact_layout()
                    .unwrap_or((self.compact_base(), default_h));
                self.compact_size = compact::Resize::new(tw, th);
                self.layout.set_compact(tw, th);
                let to_mode = if self.mini() {
                    morph_transition::IslandMode::Mini
                } else {
                    morph_transition::IslandMode::Normal
                };
                let target_rect = self.layout.capsule;
                let target_rad = self.capsule_corner_radius(th);
                self.island_morph.transition_with_card(
                    from_rect,
                    from_radius,
                    from_strip,
                    from_mini,
                    from_card,
                    to_mode,
                    target_rect,
                    target_rad,
                    now,
                    self.flag("Animation", true),
                    expand_ms,
                );
            }
            let _ = self.sync_input_region();
        }
    }
    /// 左键点击顶部隐藏蓝条：从 56×5 蓝条一气呵成直接连续形变为完整展开卡片，绝不先缩再弹两段跳变。
    fn reveal_hidden_edge_to_card(&mut self) {
        let now0 = self.start.elapsed().as_secs_f64();
        let (from_rect, from_radius, from_strip, from_mini, from_card) =
            self.live_morph_origin(now0);
        let expand_ms = self.options().duration;

        self.expanded = true;
        self.held = true;
        self.transition = None;
        self.frame = Frame::expanded();
        self.suppress_hover_expand = false;
        self.leave_at = None;

        self.user_hidden = false;
        self.hidden_mini = false;
        self.edge_revealed = false;
        self.edge_leave_since = None;
        self.restore_hidden = false;
        self.display_wanted = true;
        self.opacity = 1.;
        self.manual_until = Some(Instant::now() + Duration::from_secs(6));
        self.fade(1.);
        self.dirty = true;
        if self.practice_step() == Some(PracticeStep::Recover) {
            self.practice_event(PracticeStep::Recover);
        }

        self.poll();
        unsafe {
            let _ = self.position();
            let now = self.start.elapsed().as_secs_f64();
            let _ = window_level::apply(self.visual_hwnd, self.hwnd, self.effective_topmost(), false);
            if !self.taskbar_mode() {
                let target_rect = self.layout.card;
                let target_rad = self.options().radius;
                self.island_morph.transition_with_card(
                    from_rect,
                    from_radius,
                    from_strip,
                    from_mini,
                    from_card,
                    morph_transition::IslandMode::Card,
                    target_rect,
                    target_rad,
                    now,
                    self.flag("Animation", true),
                    expand_ms,
                );
            }
            let _ = self.sync_input_region();
        }
    }
    fn hide_by_gesture(&mut self) {
        self.practice_event(PracticeStep::Hide);
        self.held = false;
        self.leave_at = None;
        self.hide_at = None;
        self.seeking = false;
        self.pressed = 0;
        self.drag_target.clear();
        self.drag_panel = None;
        self.swipe = None;
        if self.practice.is_none() || self.practice_step() == Some(PracticeStep::Recover) {
            // 先保留隐藏前状态调用 enter_hidden_edge()，确保 live_morph_origin() 捕获到真实起点坐标
            self.enter_hidden_edge();
        } else {
            self.user_hidden = true;
            self.restore_hidden = false;
            self.edge_revealed = false;
            self.edge_hover_since = None;
            self.edge_leave_since = None;
            self.island_morph.cancel();
            self.fade(0.);
        }
        self.dirty = true;
    }
    /// 手动隐藏期间收到需要展示的通知或提醒时，临时唤出灵动岛并在弹窗结束后自动恢复隐藏。
    fn wake_for_transient_notice(&mut self) -> bool {
        if self.island_suppressed() { return false; }
        if !self.user_hidden {
            return true;
        }
        if self.practice.is_some()
            || self.session_locked
            || self.flag("PauseNotifications", false)
        {
            return false;
        }
        self.island_morph.cancel();
        self.restore_hidden = true;
        self.user_hidden = false;
        self.hidden_mini = false;
        self.edge_revealed = false;
        self.display_wanted = true;
        unsafe {
            let _ = self.position();
        }
        self.fade(1.);
        self.dirty = true;
        true
    }
    fn can_swipe(&self, hit: i32) -> bool {
        self.flag("SwipeActivities", true)
            && (self.practice.is_none() || self.practice_step() == Some(PracticeStep::Swipe))
            && !self.banner.visible()
            && !self.alerts.visible()
            && self.components.len() > 1
            && (matches!(hit, 0 | 1 | 13 | 40..=42) || hit >= 1000)
    }
    fn finish_swipe(&mut self, swipe: activities::Swipe) -> bool {
        if !swipe.active {
            return false;
        }
        // A flick is a decision even when the finger only travelled twenty pixels. Without
        // this a quick throw springs back to where it started, which is the first gesture
        // everyone tries and the one that reads as broken.
        let direction = {
            let flick = swipe.flick();
            if flick != 0 {
                flick
            } else {
                swipe.direction()
            }
        };
        let width = if self.expanded {
            self.layout.card.w
        } else {
            self.layout.capsule.w
        };
        let offset = motion::rubber(swipe.dx, width);
        if direction != 0 {
            let before = self.selected.clone();
            self.cycle_component(direction);
            if self.selected != before { self.practice_event(PracticeStep::Swipe); }
            if self.flag("Animation", true) {
                self.slide
                    .release(self.slide_direction * width + offset, swipe.velocity);
            }
        } else {
            self.slide.release(
                if self.flag("Animation", true) {
                    offset
                } else {
                    0.
                },
                swipe.velocity,
            );
            self.slide_outgoing = false;
            self.switching = self.flag("Animation", true);
        }
        self.drag_target.clear();
        self.drag_panel = None;
        self.pressed = 0;
        self.dirty = true;
        true
    }
    fn active_timer_action(&mut self, action: &str) {
        let Some(id) = self.selected.strip_prefix("timer:").map(str::to_owned) else {
            return;
        };
        let Some(t) = self.timers.items.iter().find(|t| t.id == id) else {
            return;
        };
        let revision = t.revision;
        let action = if action == "toggle" {
            t.primary_action()
        } else {
            action
        };
        self.advance_timers(Instant::now());
        if let Err(e) = self.timers.act(&id, action, Some(revision)) {
            self.error = Some(e);
        }
        self.refresh_components();
        self.dirty = true;
    }
    fn flag(&self, k: &str, default: bool) -> bool {
        self.settings[k].as_bool().unwrap_or(default)
    }
    /// Prefer the single-file launcher when one extracted this copy: a versioned runtime folder
    /// changes on every update, so registering it would break login startup after the next release.
    fn startup_target(&self) -> PathBuf {
        if self.base.join("Installed.json").exists() || self.base.join("Portable.json").is_file() {
            return self.base.join("LyricIsland.exe");
        }
        if let Ok(text) = std::fs::read_to_string(self.base.join("launcher.txt")) {
            let path = PathBuf::from(text.trim());
            if path.is_file() {
                return path;
            }
        }
        std::env::current_exe().unwrap_or_else(|_| self.base.join("LyricIsland.exe"))
    }
    fn apply_startup(&mut self) -> std::result::Result<(), String> {
        shell::set_startup(
            self.flag("AutoStart", true),
            &self.startup_target().to_string_lossy(),
            &self.data.join("settings.json"),
            &self.settings,
            None,
        )
    }
    fn mini(&self) -> bool {
        self.hidden_mini || (self.selected != agent_island::ID && self.settings["IslandSizeMode"].as_str() == Some("Mini"))
    }
    fn music_geometry(&self) -> lyric_core::music_layout::MusicGeometry {
        let w = self.layout.card.w;
        let layout = layout::MusicLayout::from_setting(
            self.settings["MusicExpandedLayout"].as_str().unwrap_or(""),
        );
        let scale = unsafe { GetDpiForWindow(self.hwnd).max(96) as f64 / 96. };
        let font = self.settings["FontSize"].as_f64().unwrap_or(12.5);
        let target = if self.music_custom.enabled {
            self.music_custom.geometry(w, scale, font, self.music_mix)
        } else {
            layout::music_geometry(w, layout, self.music_mix, scale, font)
        };
        if let Some(from) = self.custom_from {
            let t = lyric_core::music_layout::music_layout_mix(
                0.,
                1.,
                self.custom_start.elapsed().as_secs_f64(),
            );
            lyric_core::music_layout::MusicGeometry::between(&from, &target, t)
        } else {
            target
        }
    }
    fn card_height(&self) -> f64 {
        if self.practice.is_some() { return 164.; }
        if self.selected == resident::ID { return self.resident.view.height(); }
        if self.selected == agent_island::ID { return self.agent.view.height(); }
        if let Some(p)=self.panel.as_ref().filter(|p|p.kind==4){return 70.+p.blocks.iter().map(|b|match b.kind.as_str(){"metric"=>56.,"progress"=>40.,"divider"=>12.,_=>28.}).sum::<f64>()}
        self.music_geometry().height
    }
    fn music_target(&self) -> f64 {
        layout::MusicLayout::from_setting(
            self.settings["MusicExpandedLayout"].as_str().unwrap_or(""),
        )
        .position()
    }
    /// Height is part of the shared geometry, so the window keeps the taller host
    /// and only the card itself grows while the two layouts cross-fade.
    fn start_music_shift(&mut self) {
        let target = self.music_target();
        if (target - self.music_mix).abs() < 0.001 {
            self.music_mix = target;
            return;
        }
        if self.expanded && self.flag("Animation", true) {
            self.music_from = self.music_mix;
            self.music_start = Instant::now();
        } else {
            self.music_mix = target;
            self.music_from = target;
        }
        self.dirty = true;
    }
    fn shape(&self) -> (Rect, f64) {
        let closed_r = if self.layout.anchor_bottom {
            6.0
        } else {
            self.settings["CapsuleCornerRadius"]
                .as_f64()
                .unwrap_or(self.layout.capsule.h / 2.)
        };
        self.layout.morph_shape_with_radii(
            self.frame.card_reveal,
            closed_r,
            self.options().radius,
        )
    }
    fn options(&self) -> Options {
        serde_json::from_value(self.settings["Motion"].clone()).unwrap_or_default()
    }
    fn rebuild_layout(&mut self) {
        let taskbar_mode = self.taskbar_mode();
        let tb_w = self.taskbar_width();
        let base = if taskbar_mode { tb_w } else { self.compact_base() };
        self.layout = Layout::new(base);
        self.layout.card.h = self.card_height();
        self.layout.host.h = self.layout.host.h.max(self.layout.card.h + 72.);
        if taskbar_mode {
            if (self.compact_size.width_target - tb_w).abs() > 1. || (self.compact_size.height_target - 38.).abs() > 1. {
                self.compact_size = compact::Resize::new(tb_w, 38.);
            }
        } else if (self.compact_size.height_target < 30.) != self.mini() || (self.compact_size.height_target - 38.).abs() < 0.5 && !self.mini() {
            self.compact_size = compact::Resize::new(base, if self.mini() { 14. } else { 40. });
        }
        self.layout
            .set_compact(self.compact_size.width, self.compact_size.height);
        self.layout.reserve_compact(
            tb_w.max(
                self.settings["AutoCompactMaxWidth"]
                    .as_f64()
                    .unwrap_or(420.)
                    .max(base),
            ).max(agent_island::MAX_COMPACT_WIDTH) + 64.,
        );
        self.compact_key = String::new();
    }
    fn target_monitor(&self) -> HMONITOR {
        displays::resolve_monitor(
            self.settings["Display"].as_str().unwrap_or(""),
            self.hwnd,
        )
    }
    unsafe fn position(&mut self) -> Result<()> {
        self.position_on(self.target_monitor())
    }
    unsafe fn position_on(&mut self, monitor: HMONITOR) -> Result<()> {
        self.rebuild_layout();
        // Headless demo fixtures exercise the same layout/request path without a Win32 host.
        #[cfg(test)]
        if self.demo && self.hwnd == HWND::default() && self.visual_hwnd == HWND::default() {
            return Ok(());
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let dpi = GetDpiForWindow(self.hwnd).max(96) as f64;
        let scale = dpi / 96.;
        let work = info.rcWork;
        let taskbar_mode = self.taskbar_mode();
        self.taskbar_light = taskbar::is_system_light_taskbar();
        let (x, y, width, height) = if taskbar_mode {
            if let Some(probe) = taskbar::probe_for_monitor(monitor) {
                self.last_taskbar_probe = Some(probe);
                let align = self.settings["TaskbarDockAlign"].as_str().unwrap_or("Free");
                let pos_pct = self.settings["TaskbarPosition"].as_f64().unwrap_or(18.);
                let dock = taskbar::compute_dock_placement(
                    probe,
                    align,
                    pos_pct,
                    self.layout.host.w,
                    self.layout.capsule.w,
                    self.layout.card.w,
                    scale,
                );
                let top = taskbar::top_dock(probe.taskbar, info.rcMonitor);
                self.layout.configure_anchor(!top, dock.taskbar_height_dp);
                self.layout.align_dock(dock.capsule_center_in_host_dp, dock.card_center_in_host_dp);
                let w_px = (self.layout.host.w * scale).ceil() as i32;
                let h_px = (self.layout.host.h * scale).ceil() as i32;
                let y = if top {
                    (probe.taskbar.top + probe.taskbar.bottom) / 2 - ((self.layout.capsule.y + self.layout.capsule.h / 2.) * scale).round() as i32
                } else { dock.win_bottom - h_px };
                (dock.win_x, y, w_px, h_px)
            } else {
                self.layout.configure_anchor(true, 48.);
                let w_px = (self.layout.host.w * scale).ceil() as i32;
                let h_px = (self.layout.host.h * scale).ceil() as i32;
                let m = info.rcMonitor;
                let mx = (m.left + (m.right - m.left - w_px) / 2).clamp(m.left, (m.right - w_px).max(m.left));
                (mx, m.bottom - h_px, w_px, h_px)
            }
        } else {
            self.last_taskbar_probe = None;
            self.layout.configure_anchor(false, 48.);
            let width = (self.layout.host.w * scale).ceil() as i32;
            let height = (self.layout.host.h * scale).ceil() as i32;
            let floating = self.flag("Floating", false);
            let center = if floating {
                self.settings["CenterX"].as_f64().unwrap_or(0.5)
            } else {
                0.5
            };
            let x = (work.left as f64 + (work.right - work.left) as f64 * center - width as f64 / 2.)
                .round() as i32;
            let x = x.clamp(work.left, (work.right - width).max(work.left));
            let y = if floating {
                (work.top as f64
                    + ((work.bottom - work.top) as f64 - 40. * scale)
                        * self.settings["TopFraction"].as_f64().unwrap_or(0.08)
                    - 24. * scale)
                    .round() as i32
            } else {
                work.top - (24. * scale).round() as i32
            };
            (x, y, width, height)
        };
        SetWindowPos(
            self.hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
        SetWindowPos(
            self.visual_hwnd,
            None,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )?;
        let rebuild = !self.renderer.as_ref().is_some_and(|r| {
            r.width == width as u32
                && r.height == height as u32
                && (r.dpi - dpi as f32).abs() < 0.01
        });
        if rebuild {
            // An existing renderer only needs a resize; a missing one is created on
            // demand by the first visible frame (`ensure_renderer`), so a hidden
            // island never pays for a wgpu + Vello context it is not painting with.
            if let Some(renderer) = &mut self.renderer {
                renderer.resize(width as u32, height as u32, dpi as f32);
            }
        }
        window_level::apply(
            self.visual_hwnd,
            self.hwnd,
            self.effective_topmost(),
            false,
        )?;
        self.input_region = None;
        self.sync_input_region()?;
        self.dirty = true;
        if rebuild {
            self.draw()?;
        }
        Ok(())
    }
    fn delete_activity(&mut self) -> std::result::Result<(), String> {
        if self.selected == "music" || self.panel.is_none() || !self.expanded {
            return Ok(());
        }
        let selected = self.selected.clone();
        if let Some(id) = selected.strip_prefix("timer:") {
            let revision = self.timers.items.iter().find(|t|t.id==id).map(|t|t.revision);
            self.timers.act(id, "delete", revision)?;
        } else if let Some(id) = selected.strip_prefix("plugin:") {
            self.plugins.change(id, None)?;
        } else if selected == "todos" {
            let before = self.settings["TodoIslandEnabled"].clone();
            self.settings["TodoIslandEnabled"] = json!(false);
            if let Err(error) = self.save() {
                self.settings["TodoIslandEnabled"] = before;
                return Err(error);
            }
        } else if selected == "update" {
            // Delete the presentation without cancelling work owned by the updater.
            self.update_card_deleted = true;
            self.updates.reminder = false;
            self.update_until = None;
        } else {
            return Ok(());
        }
        self.held = false;
        self.manual_until = None;
        self.leave_at = None;
        self.suppress_hover_expand = true;
        self.cursor_inside = true;
        self.retarget(false);
        self.refresh_components();
        self.compact_key.clear();
        self.dirty = true;
        if let Some(wake) = &self.settings_wake { wake.signal(); }
        Ok(())
    }
    fn track_pointer_presence(&mut self, inside: bool) {
        // A closing or morphing surface can cross the pointer; wait for the morph to settle.
        if !inside && self.transition.is_none() && !self.island_morph.active() {
            self.suppress_hover_expand = false;
        }
        if !self.banner.visible() && inside && !self.cursor_inside {
            self.leave_at = None;
            let on_tb_ctrl = self.layout.anchor_bottom && !self.expanded && [3, 4, 5].contains(&self.hover);
            if !on_tb_ctrl
                && !self.user_hidden
                && !self.island_morph.active()
                && !self.suppress_hover_expand
                && !self.mini()
                && self.flag("HoverExpand", false)
                && (self.practice.is_none() || self.practice_step() == Some(PracticeStep::HoverExpand))
            {
                let was_expanded = self.expanded;
                self.retarget(true);
                if !was_expanded && self.expanded { self.practice_event(PracticeStep::HoverExpand); }
            }
        }
        if !inside && self.cursor_inside && !self.held {
            self.leave_at = Some(Instant::now());
        }
        if self.cursor_inside != inside && self.taskbar_mode() {
            self.dirty = true;
        }
        self.cursor_inside = inside;
    }
    fn retarget(&mut self, expanded: bool) {
        if !expanded {
            self.agent_auto_open=false;
            if self.agent.view.editing { unsafe { self.end_agent_edit(false, true); } }
        }
        if self.expanded && !expanded {
            self.manual_until = None;
        }
        // Retire an idle manual expansion in the same gesture, while its content is alive.
        if !expanded
            && !self.base_needed()
            && !self.banner.visible()
            && !self.osd_banner.visible()
            && !self.alerts.visible()
            && !self.alerts.pending()
            && self.queue.is_empty()
        {
            self.display_wanted = false;
            self.hide_at = None;
            self.fade(0.);
        }
        if self.expanded == expanded {
            return;
        }
        let velocity = self
            .transition
            .as_ref()
            .map(|t| t.velocity(self.sampled_ms))
            .unwrap_or_default();
        self.expanded = expanded;
        let o = self.options();
        if self.flag("Animation", true) {
            self.transition = Some(Transition::new(expanded, self.frame, o, velocity, false));
            self.transition_start = Instant::now();
            self.sampled_ms = 0.;
        } else {
            self.transition = None;
            self.frame = if expanded {
                Frame::expanded()
            } else {
                Frame::hidden(o)
            };
        }
        self.dirty = true;
    }
    /// Drag resistance stays continuous beyond the surface width.
    fn swipe_offset(&self) -> f64 {
        let Some(swipe) = self.swipe.filter(|s| s.active) else {
            return 0.;
        };
        let width = if self.expanded {
            self.layout.card.w
        } else {
            self.layout.capsule.w
        };
        motion::rubber(swipe.dx, width)
    }
    fn fade(&mut self, target: f64) {
        if self.fade_to != target {
            if target > self.fade_to {
                if let Err(e) = unsafe {
                    window_level::apply(
                        self.visual_hwnd,
                        self.hwnd,
                        self.effective_topmost(),
                        false,
                    )
                } {
                    self.error = Some(e.to_string());
                }
            }
            self.fade_from = self.opacity;
            self.fade_to = target;
            self.fade_start = Instant::now();
            self.dirty = true;
        }
    }
    fn command(&mut self, action: &str, seconds: f64) {
        if self.demo {
            if action == "seek" {
                self.snapshot["positionSeconds"] = json!(seconds);
                self.dirty = true;
            }
            return;
        }
        if let Some(backend) = &self.backend {
            if let Err(error) = backend.enqueue(action, seconds) {
                self.show_notice(
                    notifications::Notice {
                        app_id: "LyricIsland.System.Seek".into(),
                        app_name: "点屿".into(),
                        title: "播放操作未完成".into(),
                        body: error,
                        icon: None,
                    },
                    true,
                    None,
                    false,
                );
            }
        }
    }
    fn show_settings(&mut self) {
        self.show_settings_page("")
    }
    /// Open the settings window, optionally asking it to land on a named page.
    ///
    /// A window that is already running cannot be re-launched, so the request is
    /// also published as `pendingPage` in the state reply; the settings window
    /// reads it on its next refresh and switches itself.
    /// The settings process is started only on demand: an always-resident warm
    /// copy kept a second full GPU context (wgpu adapter + Vello pipelines +
    /// fonts) alive for the whole session, which dominated the app's memory
    /// footprint even though the window was never opened.
    fn show_settings_page(&mut self, page: &str) {
        self.pending_request=self.pending_request.wrapping_add(1);
        self.pending_center_action=Value::Null;
        self.pending_page = Some(if page.is_empty() {
            self.settings["ControlCenterStartPage"]
                .as_str()
                .unwrap_or("overview")
                .to_owned()
        } else {
            page.to_owned()
        });
        // A running MyGo center picks up pendingPage from its next state refresh,
        // so waking it is enough. Its embedded DLL owns the desktop widgets.
        if self
            .center_process
            .as_mut()
            .is_some_and(|p| p.try_wait().ok().flatten().is_none())
        {
            if let Some(wake) = io::SettingsWake::new() {
                wake.signal();
            }
            return;
        }
        self.launch_center(self.center_started);
    }
    fn launch_center(&mut self, warm: bool) {
        let exe = self.base.join("LyricIsland.Center.exe");
        if !exe.is_file() {
            self.error = Some("设置组件缺失，请完整解压运行包".into());
            return;
        }
        let mut command = std::process::Command::new(exe);
        if warm { command.arg("--warm"); }
        // Open the GUI executable directly, detached from the caller's console.
        // A hidden/minimized startup show state would also affect the actual UI.
        self.center_process = command
            .creation_flags(windows::Win32::System::Threading::DETACHED_PROCESS.0)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .env("LYRICISLAND_TEST_PIPE", std::env::var("LYRICISLAND_TEST_PIPE")
                .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into()))
            .env("LYRICISLAND_NATIVE_DATA", &self.data)
            .env("DOTISLE_CENTER_IDLE_EXIT", "1")
            .current_dir(&self.base).spawn()
            .map_err(|e| { self.error = Some(format!("无法启动控制中心：{e}")); e }).ok();
        self.center_started |= self.center_process.is_some();
    }
    fn reap_center(&mut self) {

        // Replay an open request that raced the clean idle exit, not a crash.
        const IDLE_EXIT: i32 = 35;
        if self.pending_page.is_none() {
            return;
        }
        let Some(status) = self.center_process.as_mut().and_then(|p| p.try_wait().ok().flatten()) else {
            return;
        };
        self.center_process = None;
        if status.code() == Some(IDLE_EXIT) {
            self.launch_center(true);
        }
    }
    fn terminate_child(slot: &mut Option<std::process::Child>) {
        if let Some(mut child) = slot.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
    fn ensure_widget_host(&mut self) {
        if self.demo { return; }

        let mail = self.data.join("mail-account.dpapi").is_file();
        if !mail && (self.desktop_widgets.items.is_empty() || widget_metrics::host_running(&self.data)) { return; }
        if self.center_process.as_mut().is_some_and(|p|p.try_wait().ok().flatten().is_none()) { return; }
        if self.last_widget_host.elapsed() < Duration::from_secs(5) { return; }
        self.last_widget_host = Instant::now();
        self.launch_center(true);
    }
    /// Register the configured quick-reminder hotkey. The previous binding is
    /// removed first; a conflict is reported so the caller can roll the setting back.
    unsafe fn register_hotkey(&self) -> std::result::Result<(), String> {
        let _ = UnregisterHotKey(Some(self.hwnd), 1);
        let name = self.settings["QuickReminderHotkey"].as_str().unwrap_or("");
        let Some((mods, vk)) = hotkey_code(name) else {
            return Ok(());
        };
        RegisterHotKey(Some(self.hwnd), 1, mods | MOD_NOREPEAT, vk)
            .map_err(|e| format!("快捷键注册失败，可能已被其他程序占用：{e}"))
    }
    fn quick_preview(&mut self, text: &str) -> Value {
        if self.demo {
            return json!({"success":true,"previewToken":"demo","date":self.todo_now.date,"time":self.todo_now.time,"title":"演示","expiresAt":0,"minutePrecision":true});
        }
        let now = todos::Now::local();
        match quick_reminder::parse(text, &now) {
            Ok(draft) => {
                self.quick.retain(|p| p.expires > Instant::now());
                if self.quick.len() >= 8 {
                    self.quick.remove(0);
                }
                let epoch = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();
                let token = format!(
                    "qr-{}-{}",
                    std::process::id(),
                    epoch * 1000 + self.quick.len() as u64
                );
                self.quick.push(QuickPreview {
                    token: token.clone(),
                    title: draft.title.clone(),
                    date: draft.date.clone(),
                    time: draft.time.clone(),
                    expires: Instant::now() + Duration::from_secs(120),
                });
                json!({"success":true,"previewToken":token,"date":draft.date,"time":draft.time,"title":draft.title,"expiresAt":epoch+120,"minutePrecision":true,"message":format!("将在 {} {} 提醒",draft.date,draft.time)})
            }
            Err(e) => json!({"success":false,"code":e.code,"message":e.message}),
        }
    }
    fn quick_commit(&mut self, token: &str, request: &str) -> Value {
        let request = request.trim();
        if request.is_empty() || request.len() > 120 {
            return json!({"success":false,"message":"缺少提交标识"});
        }
        if let Some((_, id)) = self.quick_commits.iter().find(|(r, _)| r == request) {
            return json!({"success":true,"id":id,"todos":self.todos.list(&self.todo_now),"message":"已创建提醒"});
        }
        if self.demo {
            return json!({"success":true,"id":"demo"});
        }
        let now = todos::Now::local();
        let Some(preview) = self.quick.iter().find(|p| p.token == token) else {
            return json!({"success":false,"message":"预览已过期，请重新解析"});
        };
        if preview.expires <= Instant::now() {
            return json!({"success":false,"message":"预览已过期，请重新解析"});
        }
        if format!("{} {}", preview.date, preview.time) <= format!("{} {}", now.date, now.time) {
            return json!({"success":false,"message":"提醒时间已到，请重新解析"});
        }
        let (title, date, time) = (
            preview.title.clone(),
            preview.date.clone(),
            preview.time.clone(),
        );
        match self
            .todos
            .save(&json!({"title":title,"content":"","date":date,"time":time,"daily":false}))
        {
            Ok(id) => {
                self.quick.retain(|p| p.token != token);
                self.quick_commits.push((request.into(), id.clone()));
                if self.quick_commits.len() > 32 {
                    self.quick_commits.remove(0);
                }
                json!({"success":true,"id":id,"todos":self.todos.list(&self.todo_now),"message":"已创建提醒"})
            }
            Err(e) => json!({"success":false,"message":e}),
        }
    }
    fn on_lock(&mut self) {
        if self.session_locked {
            return;
        }
        self.session_locked = true;
        self.reminder_plugins.break_session();
        for kind in reminder_plugins::Kind::ALL { self.alerts.remove_pending(kind.alert_key()); }
        if !self.flag("FocusPauseOnLock", false) { return; }
        self.advance_timers(Instant::now());
        self.lock_generation = self.lock_generation.wrapping_add(1);
        match self.timers.pause_for_lock(self.lock_generation) {
            Ok(ids) if !ids.is_empty() => {
                self.focus_resume = Some(FocusResume {
                    generation: self.lock_generation,
                    ids,
                    prompted: false,
                })
            }
            Ok(_) => {}
            Err(e) => self.error = Some(e),
        }
        self.last_awake = awake_ms();
    }
    fn on_unlock(&mut self) {
        if !self.session_locked {
            return;
        }
        self.session_locked = false;
        self.last_awake = awake_ms();
        if !self.flag("FocusPauseOnLock", false) {
            self.focus_resume = None;
            return;
        }
        let Some(mut resume) = self.focus_resume.take() else {
            return;
        };
        let reason = format!("lock:{}", resume.generation);
        resume.ids.retain(|id| {
            self.timers.items.iter().any(|t| {
                &t.id == id
                    && !t.running
                    && !t.finished
                    && t.pause_reason.as_deref() == Some(reason.as_str())
            })
        });
        if resume.ids.is_empty() {
            return;
        }
        let notice = notifications::Notice {
            app_id: "LyricIsland.Focus".into(),
            app_name: "专注".into(),
            title: format!("继续刚才的 {} 项专注？", resume.ids.len()),
            body: "点击打开计时页，可逐项继续或暂不。".into(),
            icon: None,
        };
        self.enqueue(Queued {
            notice,
            todo: Some(format!("focus:{}", resume.generation)),
            chime: false,
        });
        resume.prompted = true;
        self.focus_resume = Some(resume);
    }
    fn fire_rule_hits(&mut self, hits: &[app_rules::Hit]) {
        let (title, body) = if hits.len() == 1 {
            (hits[0].1.clone(), hits[0].2.clone())
        } else {
            (
                format!("{} 条应用提醒", hits.len()),
                hits.iter()
                    .map(|(_, name, message)| format!("{name}：{message}"))
                    .collect::<Vec<_>>()
                    .join("\n"),
            )
        };
        let notice = notifications::Notice {
            app_id: "LyricIsland.AppRule".into(),
            app_name: "应用提醒".into(),
            title,
            body: body.clone(),
            icon: None,
        };
        self.enqueue(Queued {
            notice: notice.clone(),
            todo: Some(format!("rule:{}", hits[0].0)),
            chime: true,
        });
        if self.flag("NotificationHistoryEnabled", true) {
            for (id, name, message) in hits {
                self.history.record(
                    "automation",
                    "LyricIsland.AppRule",
                    name,
                    name,
                    message,
                    Some(id.clone()),
                );
            }
        }
    }
    /// One shared foreground listener for F05/F06. Enter events are consumed even
    /// while notifications are paused so they are never replayed later.
    unsafe fn poll_app_rules(&mut self) {
        let entry = self.flag("AppEntryRemindersEnabled", false);
        let usage = self.flag("AppUsageRemindersEnabled", false);
        if !entry && !usage {
            return;
        }
        let now = todos::Now::local();
        if now.date != self.usage_date {
            self.rules.roll_day(&now.date);
            self.usage_date = now.date.clone();
        }
        let allowed = !self.flag("PauseNotifications", false);
        let mut hits: Vec<app_rules::Hit> = vec![];
        if let Some((path, segment)) = self.foreground.poll() {
            if entry {
                hits.extend(
                    self.rules
                        .enter_hits(&path, segment, &now.date, &now, allowed),
                );
            }
        }
        let awake = awake_ms();
        if usage {
            if let Some(path) = self.foreground.path.clone() {
                let delta = if self.session_locked || !allowed {
                    0
                } else {
                    awake.saturating_sub(self.last_awake).min(2000)
                };
                if delta > 0 {
                    self.rules.accumulate(&path, delta, &now.date);
                }
                if allowed && !self.session_locked {
                    hits.extend(self.rules.due_usage(&path, &now.date));
                }
            }
        }
        self.last_awake = awake;
        if !hits.is_empty() {
            self.fire_rule_hits(&hits);
        }
        if self.last_rules_checkpoint.elapsed() >= Duration::from_secs(60) {
            self.last_rules_checkpoint = Instant::now();
            if usage {
                if let Err(e) = self.rules.persist_usage() {
                    self.error = Some(e);
                }
            }
        }
    }
    fn save(&mut self) -> std::result::Result<(), String> {
        if self.demo {
            return Ok(());
        }
        io::save_settings(&self.data.join("settings.json"), &self.settings)
    }
    /// The collapsed second line: the translation when one exists, otherwise the
    /// next line for Chinese lyrics. Never invents an empty second row.
    fn compact_secondary_for(&self) -> String {
        if self.mini() {
            return String::new();
        }
        compact::normalize_line(compact::secondary(
            &self.snapshot,
            self.flag("CompactTranslation", true),
            self.flag("ChineseTwoLine", false),
        ))
    }
    fn compact_base(&self) -> f64 {
        self.settings["CompactWidth"]
            .as_f64()
            .unwrap_or(260.)
            .clamp(140., 480.)
    }
    /// Size the collapsed capsule for the current lyric. The host surface stays
    /// reserved at the widest allowed width, so the window and the expanded card
    /// never move when the lyric changes; only the capsule inside it resizes.
    unsafe fn measure_compact_layout(&mut self) -> Result<(f64, f64)> {
        if self.selected == agent_island::ID && !self.taskbar_mode() {
            return Ok((self.agent.view.compact_width(), 40.));
        }
        if self.taskbar_mode() {
            return Ok((self.taskbar_width(), 38.));
        }
        let base = self.compact_base();
        let font = self.settings["FontSize"].as_f64().unwrap_or(12.5);
        let font = if self.mini() {
            compact::mini_font_size(font)
        } else {
            font
        };
        let double = self.selected == "music" && !self.compact_secondary.is_empty();
        let scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
        let height = if self.mini() {
            14.
        } else {
            self.renderer
                .as_ref()
                .map(|r| r.compact_height(font, double, scale))
                .unwrap_or(40.)
        };
        let auto = self.flag("AutoCompactWidth", false);
        let target = if self.selected == resident::ID {
            base.max(if self.mini() { 160. } else { 190. })
        } else if self.selected != "music" {
            260.
        } else if auto {
            let secondary_size = (font * 0.78).clamp(9., 11.5);
            let mut widest = 0.;
            if let Some(r) = self.renderer.as_mut() {
                r.set_font_family(
                    self.settings["IslandFontFamilyRender"]
                        .as_str()
                        .unwrap_or("Noto Sans SC"),
                );
                // The compact lyric paints at weight 500; measuring its default
                // 400 face under-sized the capsule and left an ellipsis.
                widest = r.measure_line_weighted(&self.lyric, font, if font >= 20. { 600 } else { 500 })?;
                if double {
                    widest = widest.max(r.measure_line(&self.compact_secondary, secondary_size)?);
                }
            }
            // 预留 80 布局单位给左侧封面、右侧声波与安全间隙，避免任何字宽误差吞掉胶囊末尾歌词
            (widest + if self.mini() { 52. } else { 80. }).clamp(
                140.,
                self.settings["AutoCompactMaxWidth"]
                    .as_f64()
                    .unwrap_or(420.)
                    .clamp(140., 480.),
            )
        } else {
            base
        };
        // Land the edges on the physical pixel grid so the final capsule is crisp.
        // Round up, never down: a hair narrower than the measured line would make
        // text layout add an ellipsis. Keep the current target when a measurement
        // differs by less than two physical pixels in either direction.
        let round_up = |v: f64| (v * scale).ceil() / scale;
        let mut target = round_up(target);
        if auto && (target - self.compact_size.width_target).abs() < 2. / scale {
            target = self.compact_size.width_target;
        }
        Ok((target, round_up(height)))
    }
    fn apply_compact_target(&mut self, target: f64, height: f64) {
        if self.mini() {
            self.compact_size
                .target_mini(target, height, self.flag("Animation", true));
        } else {
            self.compact_size
                .target(target, height, self.flag("Animation", true));
        }
        self.layout
            .set_compact(self.compact_size.width, self.compact_size.height);
        self.dirty = true;
    }
    #[cfg(test)]
    unsafe fn update_compact_layout(&mut self) -> Result<()> {
        let (target, height) = self.measure_compact_layout()?;
        self.apply_compact_target(target, height);
        self.layout
            .set_compact(self.compact_size.width, self.compact_size.height);
        Ok(())
    }
    /// Queue a banner. Reminders are never dropped silently: when the queue is
    /// full an older notification gives way, and a notification is refused
    /// rather than evicting a pending reminder.
    fn enqueue(&mut self, q: Queued) {
        if q.todo.is_none() {
            let signature = format!("{}|{}|{}", q.notice.app_id, q.notice.title, q.notice.body);
            let now = self.start.elapsed().as_secs_f64();
            let showing = self
                .banner
                .notice
                .as_ref()
                .map(|n| format!("{}|{}|{}", n.app_id, n.title, n.body));
            if showing.as_deref() == Some(signature.as_str()) {
                return;
            }
            if self
                .last_notice
                .as_ref()
                .is_some_and(|(seen, at)| *seen == signature && now - *at < 6.)
            {
                return;
            }
            self.last_notice = Some((signature, now));
        }
        let limit = if q.todo.is_some() { 32 } else { 8 };
        while self.queue.len() >= limit {
            match self.queue.iter().position(|e| e.todo.is_none()) {
                Some(i) => {
                    self.queue.remove(i);
                }
                None => {
                    if q.todo.is_none() {
                        return;
                    }
                    break;
                }
            }
        }
        self.queue.push_back(q);
    }
    fn enqueue_todo(&mut self, t: &todos::Todo) {
        let body = if t.content.trim().is_empty() {
            if t.daily {
                "每日待办提醒".into()
            } else {
                "待办提醒".into()
            }
        } else {
            t.content.clone()
        };
        self.enqueue(Queued {
            notice: notifications::Notice {
                app_id: TODO_APP_ID.into(),
                app_name: "待办".into(),
                title: t.title.clone(),
                body: body.clone(),
                icon: None,
            },
            todo: Some(t.id.clone()),
            chime: true,
        });
        if self.flag("NotificationHistoryEnabled", true) {
            self.history.record(
                "todo",
                TODO_APP_ID,
                "待办",
                &t.title,
                &body,
                Some(t.id.clone()),
            );
        }
    }
    /// Only events that actually reached the user are kept: previews, paused
    /// notifications and whitelist rejects never get here.
    fn record_history(
        &mut self,
        kind: &str,
        notice: &notifications::Notice,
        todo_id: Option<String>,
    ) {
        if notice.app_id=="WeChat"{return}
        if self.flag("NotificationHistoryEnabled", true) {
            self.history.record(
                kind,
                &notice.app_id,
                &notice.app_name,
                &notice.title,
                &notice.body,
                todo_id,
            );
        }
    }
    /// Drop a reminder banner that has not been presented yet, and close it if it
    /// is on screen. Called after a todo is completed or deleted.
    fn cancel_todo(&mut self, id: &str) {
        self.queue.retain(|e| e.todo.as_deref() != Some(id));
        if self.banner_todo.as_deref() == Some(id) {
            self.banner_todo = None;
            let (r, rad) = self.shape();
            self.banner.dismiss(
                r,
                rad,
                self.start.elapsed().as_secs_f64(),
                self.flag("Animation", true),
                self.base_needed(),
            );
        }
    }
    fn poll(&mut self) {
        if self.plugins.refresh(false){self.refresh_components();self.dirty=true;if let Some(wake)=&self.settings_wake{wake.signal();}}
        let previous = self.snapshot.clone();
        self.last_poll = Instant::now();
        if let Some(b) = &self.backend {
            self.snapshot = b.snapshot(self.cover_version);
        }
        if self.seeking
            && (previous["track"]["id"] != self.snapshot["track"]["id"]
                || previous["track"]["sourceApp"] != self.snapshot["track"]["sourceApp"])
        {
            self.seeking = false;
            self.pressed = 0;
        }
        let failed = &self.snapshot["mediaCommand"];
        if failed["state"] == "failed" && failed != &previous["mediaCommand"] {
            if let Some(error) = failed["error"].as_str() {
                self.show_notice(
                    notifications::Notice {
                        app_id: "LyricIsland.System.Seek".into(),
                        app_name: "点屿".into(),
                        title: "播放操作未完成".into(),
                        body: error.to_string(),
                        icon: None,
                    },
                    true,
                    None,
                    false,
                );
            }
        }
        if previous["track"]["id"] != self.snapshot["track"]["id"]
            || previous["sourceId"] != self.snapshot["sourceId"]
        {
            self.cover_rx = None;
            self.pending_cover = None;
            if let Some(r) = &mut self.renderer {
                r.clear_track_cover();
            }
            self.sync_bottom_material();
            self.dirty = true;
        }
        // Desktop widget lyrics must follow the island frame: wake the settings process so
        // its poll loop refreshes the second-hand snapshot right away instead of waiting out
        // its cadence (widget lyrics used to trail the island by hundreds of milliseconds).
        if self.snapshot["frame"] != previous["frame"] {
            if let Some(wake) = &self.settings_wake {
                wake.signal();
            }
        }
        let version = self.snapshot["coverVersion"].as_u64().unwrap_or(0);
        if version != self.cover_version {
            self.cover_version = version;
            let cover = self.snapshot["cover"].as_str().map(str::to_owned);
            let (tx, rx) = mpsc::channel();
            self.cover_rx = Some(rx);
            let cache = self.data.join("Cache/MusicPreview");
            std::thread::spawn(move || {
                let bytes =
                    cover.and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok());
                let result = Renderer::prepare_cover(bytes.as_deref());
                if let Ok(Some(p)) = &result {
                    let _ = std::fs::create_dir_all(&cache);
                    let _ = p.save_preview(&cache.join(format!("cover-{version}.png")));
                    if let Ok(entries) = std::fs::read_dir(&cache) {
                        for e in entries.flatten() {
                            let path = e.path();
                            if path
                                .file_stem()
                                .and_then(|s| s.to_str())
                                .and_then(|s| s.strip_prefix("cover-"))
                                .and_then(|v| v.parse::<u64>().ok())
                                .is_some_and(|v| v.saturating_add(2) < version)
                                && path.extension().is_some_and(|v| v == "png")
                            {
                                let _ = std::fs::remove_file(path);
                            }
                        }
                    }
                }
                let _ = tx.send((version, result));
            });
        }
        let lyric = compact::normalize_line(compact::primary(&self.snapshot));
        if lyric != self.lyric {
            let t = (self.lyric_start.elapsed().as_secs_f64() / 0.24).min(1.);
            self.outgoing_opacity = 1. - (1. - t).powi(4);
            self.outgoing_y = 5. * (1. - t).powi(4);
            self.previous_lyric = std::mem::replace(&mut self.lyric, lyric);
            self.lyric_start = Instant::now();
        }
        // A translation can arrive a beat after its line, so the second row is
        // tracked independently of the first.
        let compact_secondary = self.compact_secondary_for();
        if compact_secondary != self.compact_secondary {
            self.previous_compact_secondary =
                std::mem::replace(&mut self.compact_secondary, compact_secondary);
            self.dirty = true;
        }
        let playing = self.snapshot["isPlaying"] == true;
        if playing != self.activity.playing {
            self.activity.retarget(
                playing,
                self.start.elapsed().as_secs_f64() * 1000.,
                self.flag("Animation", true),
            );
        }
        let duration = self.snapshot["track"]["durationSeconds"]
            .as_f64()
            .unwrap_or(0.);
        self.progress.update(
            self.start.elapsed().as_secs_f64(),
            self.snapshot["track"]["id"].as_str().unwrap_or(""),
            self.snapshot["positionSeconds"].as_f64().unwrap_or(0.),
            duration,
            playing,
            self.flag("Animation", true),
        );
        if self.seeking && previous["track"]["id"] != self.snapshot["track"]["id"] {
            self.seeking = false;
        }
        if !self.demo {
            let valid = self.snapshot["track"]["title"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty())
                && !self.snapshot["track"]["id"]
                    .as_str()
                    .unwrap_or("")
                    .ends_with("|ProcessFallback");
            let flag = self.practice.is_some()
                || self.flag("IslandKeepVisibleEnabled", false)
                || self.taskbar_always_show()
                || island_needed(
                valid,
                self.snapshot["isPlaying"] == true,
                self.flag("HideWhenPaused", true),
                self.active_tasks(),
                self.banner.visible() || self.osd_banner.visible() || self.alerts.visible() || self.alerts.pending() || self.updates.busy(),
                !self.queue.is_empty(),
                self.held,
                self.manual_until.is_some_and(|t| Instant::now() < t),
            );
            let mut wanted = flag;
            if valid {
                self.missing_since = None;
            } else if !flag {
                let since = self.missing_since.get_or_insert(Instant::now());
                wanted = self.display_wanted && since.elapsed() < Duration::from_millis(600);
            }
            if wanted != self.display_wanted {
                self.display_wanted = wanted;
                if wanted {
                    self.hide_at = None;
                    if !self.user_hidden && !self.banner.visible() {
                        self.fade(1.);
                    }
                } else {
                    self.held = false;
                    self.retarget(false);
                    if !self.banner.visible() {
                        self.hide_at = Some(
                            Instant::now()
                                + Duration::from_millis(if self.flag("Animation", true) {
                                    220
                                } else {
                                    0
                                }),
                        );
                    }
                }
            }
        }
        // A paused instrumental track keeps the same lyric frame after a seek.
        self.dirty |= previous["track"] != self.snapshot["track"]
            || previous["isPlaying"] != self.snapshot["isPlaying"]
            || previous["frame"] != self.snapshot["frame"]
            || previous["coverVersion"] != self.snapshot["coverVersion"]
            || (self.snapshot["isPlaying"] != true
                && previous["positionSeconds"] != self.snapshot["positionSeconds"]);
    }
    fn active_tasks(&self) -> bool {
        self.timers.items.iter().any(|t| t.running && !t.finished)
            || (self.updates.busy() && !self.update_card_deleted)
            || self.plugins.has_island_plugins()
            || self.agent_visible()
    }
    fn resident_idle(&self) -> bool {
        // Visible but idle; only pacing uses this state.
        (self.flag("IslandKeepVisibleEnabled", false) || self.taskbar_always_show())
            && self.snapshot["isPlaying"] != true
            && !self.active_tasks()
            && !self.updates.busy()
            && !self.banner.visible() && !self.osd_banner.visible()
            && !self.alerts.visible() && !self.alerts.pending() && self.queue.is_empty()
    }
    fn base_needed(&self) -> bool {
        let valid = self.snapshot["track"]["title"]
            .as_str()
            .is_some_and(|s| !s.trim().is_empty())
            && !self.snapshot["track"]["id"]
                .as_str()
                .unwrap_or("")
                .ends_with("|ProcessFallback");
        self.practice.is_some()
            || self.flag("IslandKeepVisibleEnabled", false)
            || self.taskbar_always_show()
            || island_needed(
            valid,
            self.snapshot["isPlaying"] == true,
            self.flag("HideWhenPaused", true),
            self.active_tasks(),
            false,
            false,
            self.held,
            self.manual_until.is_some_and(|t| Instant::now() < t),
        )
    }
    fn finish_transient(&mut self) {
        if !self.banner.visible()
            && !self.osd_banner.visible()
            && !self.alerts.visible()
            && !self.alerts.pending()
            && self.queue.is_empty()
            && (self.restore_hidden || !self.base_needed())
        {
            self.display_wanted = false;
            self.missing_since = None;
            self.hide_at = None;
            if self.restore_hidden {
                self.restore_hidden = false;
                self.enter_hidden_edge();
            } else {
                self.fade(0.);
                self.opacity = 0.;
            }
            self.dirty = true;
        }
    }
    fn weather_city(&self)->String {
        let city=self.settings["WeatherCity"].as_str().unwrap_or("").trim();
        if !city.is_empty(){city.into()}else{self.overview_widgets.items.iter().chain(self.desktop_widgets.items.iter())
            .find(|p|p.kind==dotisle_domain::desktop_widgets::Kind::Weather&&!p.config.city.trim().is_empty())
            .map(|p|p.config.city.trim().to_owned()).unwrap_or_default()}
    }
    fn show_greeting(&mut self) {
        if self.user_hidden||self.flag("PauseNotifications",false){return}
        let local=unsafe{windows::Win32::System::SystemInformation::GetLocalTime()};
        let title=match local.wHour{5..=10=>"早上好",11..=13=>"中午好",14..=17=>"下午好",18..=22=>"晚上好",_=>"夜深了"};
        let usage=self.screen_time.snapshot();
        let yesterday=usage["yesterdayMs"].as_u64().map(|v|format!("昨日用机 {}",screen_time::duration(v))).unwrap_or("昨日暂无用机记录".into());
        let weather=&self.widget_data.value["island-weather"];
        let detail=if let Some(t)=weather["temperature"].as_f64().filter(|_|weather["stale"]!=true){
            let city=weather["city"].as_str().unwrap_or("");let condition=weather_description(weather["code"].as_u64().unwrap_or(255));
            let range=match(weather["high"].as_f64(),weather["low"].as_f64()){(Some(h),Some(l))=>format!(" · {l:.0}°–{h:.0}°"),_=>String::new()};
            format!("{city} · {condition} {t:.0}°{range}")
        }else if self.weather_city().is_empty(){"设置城市，开启今日天气".into()}else{"今日天气暂不可用".into()};
        let mut alert=capsule_alert::Alert::new("greeting".into(),"Sun",title.into(),format!("{yesterday}\n{detail}"));
        alert.width=432.;alert.height=104.;alert.hold=8.;self.alerts.enqueue(alert);
    }
    fn enqueue_device(&mut self,event:&devices::Event){
        self.widget_metrics.refresh_wireless();
        let telemetry=self.device_watch.lookup(&event.key);
        let id=telemetry.map(|d|d.id.as_str()).unwrap_or(&event.key);
        let name=telemetry.map(|d|d.name.as_str()).unwrap_or(&event.label);
        let level=telemetry.and_then(|d|d.battery_percent);
        self.alerts.enqueue(capsule_alert::Alert::device(id,event.kind.key(),name,event.connected,level,false));
    }
    fn music_controls_visible(&self) -> bool {
        let ml = layout::MusicLayout::from_setting(
            self.settings["MusicExpandedLayout"]
                .as_str()
                .unwrap_or("SideBySide"),
        );
        self.music_custom.enabled
            || lyric_core::music_layout::immersion_amount(ml, self.music_mix, self.music_custom.enabled) < 0.999
            || ((self.cursor_inside || self.seeking || self.pressed > 0)
                && self.music_controls.value > 0.08)
    }
    fn hit(&self, x: f64, y: f64) -> i32 {
        if self.island_suppressed() || self.opacity < 0.01 {
            return -1;
        }
        if self.alerts.visible(){
            let(r,rad,_)=self.alerts.banner.presentation(self.start.elapsed().as_secs_f64());
            if !inside_round_rect(r,rad,x,y) { return -1; }
            if self.alerts.active.as_ref().is_some_and(|a| a.hydration.is_some()) {
                if self.alerts.banner.open {
                    if let Some(g) = capsule_alert::WaterLayout::in_card(r) {
                        if g.record.contains(x,y) { return 61; }
                        if g.later.contains(x,y) { return 60; }
                    }
                }
                return -1;
            }
            return 60;
        }
        if self.banner.visible() {
            let (r, radius, _) = self.banner.presentation(self.start.elapsed().as_secs_f64());
            if !inside_round_rect(r, radius, x, y) {
                return -1;
            }
            return if Rect::new(r.x + r.w - 32., r.y + 6., 26., 26.).contains(x, y) {
                11
            } else {
                12
            };
        }
        if !self.mini()
            && !self.expanded
            && self.components.len() > 1
            && self.layout.satellite().contains(x, y)
        {
            return 20;
        }
        let (shape, radius) = self.shape();
        if !inside_round_rect(shape, radius, x, y) {
            return -1;
        }
        if self.mini() && !self.expanded {
            return 0;
        }
        if self.selected != "music" {
            if self.expanded && self.frame.card_reveal > 0.7 {
                let scale = (shape.w / self.layout.card.w).min(shape.h / self.layout.card.h);
                let x = (x - shape.x) / scale;
                let y = (y - shape.y) / scale - self.frame.content_offset_y;
                let w = self.layout.card.w;
                if self.practice.is_some() {
                    return if self.practice_step() == Some(PracticeStep::Move)
                        && gesture_practice::move_button(self.layout.card.h).contains(x, y) { 2 } else { 0 };
                }
                if self.selected == agent_island::ID {
                    return agent_island::hit(&self.agent.view,self.layout.card,x,y);
                }
                if self.selected == resident::ID {
                    if layout::activity_close(w).contains(x, y) { return resident::COLLAPSE; }
                    return resident::buttons(w).into_iter().find(|(_, rect, _)| rect.contains(x, y))
                        .map_or(0, |(id, _, _)| id);
                }
                if self.selected != "update" && layout::activity_close(w).contains(x, y) {
                    return layout::ACTIVITY_CLOSE;
                }
                if self.selected == "update" {
                    if self.updates.phase() == "available" && layout::update_skip(w).contains(x, y)
                    {
                        return 52;
                    }
                    if self.panel.as_ref().is_some_and(|p| !p.tasks.is_empty())
                        && layout::update_primary(w, self.layout.card.h).contains(x, y)
                    {
                        return 50;
                    }
                    if layout::update_close(w).contains(x, y) {
                        return 51;
                    }
                } else if self.selected == "todos" {
                    let count = self.panel.as_ref().map(|p| p.tasks.len()).unwrap_or(0);
                    if let Some(i) = layout::todo_hit(w, self.layout.card.h, count, self.todo_scroll, x, y) {
                        return 1000 + i as i32;
                    }
                    if layout::todo_footer(w, self.layout.card.h).contains(x, y) {
                        return 7;
                    }
                } else if !self.selected.starts_with("plugin:") {
                    for (id, rect) in layout::timer_buttons(w, self.layout.card.h) {
                        if rect.contains(x, y) {
                            return id;
                        }
                    }
                }
            }
            return 0;
        }
        if self.layout.anchor_bottom
            && !self.expanded
            && self.selected == "music"
            && self.snapshot["track"].is_object()
            && self.hover >= 0
            && motion::content_weights(self.frame).1 > 0.1
        {
            for (id, r) in layout::taskbar_widget_buttons(self.layout.capsule) {
                if r.contains(x, y) {
                    return id;
                }
            }
            return 0;
        }
        if !self.mini()
            && motion::content_weights(self.frame).1 > 0.1
            && self
                .layout
                .compact_cover(unsafe { GetDpiForWindow(self.hwnd).max(96) as f64 / 96. })
                .contains(x, y)
        {
            return 1;
        }
        if self.expanded && motion::content_weights(self.frame).0 > 0.01 {
            let scale = (shape.w / self.layout.card.w).min(shape.h / self.layout.card.h);
            let tx = shape.x + (shape.w - self.layout.card.w * scale) / 2.;
            let ty = shape.y + (shape.h - self.layout.card.h * scale) / 2.;
            let x = (x - tx) / scale;
            let y = (y - ty) / scale - self.frame.content_offset_y;
            let g = self.music_geometry();
            if self.flag("MusicClickOpenApp", false)
                && if self.music_custom.enabled {
                    g.title.contains(x, y) || g.artist.contains(x, y)
                } else {
                    g.text_hit.contains(x, y)
                }
            {
                return 13;
            }
            if self.music_controls_visible() {
                // A blocked progress bar still answers: tapping it opens the enable flow instead
                // of pretending to drag, and the chip names the action in words.
                if g.progress_hit.w > 0. && self.seek_offer() {
                    if self.seek_confirm {
                        let (cancel, confirm) =
                            lyric_core::music_layout::seek_confirm_pair(g.seek_chip);
                        if cancel.contains(x, y) {
                            return 14;
                        }
                        if confirm.contains(x, y) {
                            return 15;
                        }
                    } else if g.seek_chip.contains(x, y) || g.progress_hit.contains(x, y) {
                        return 14;
                    }
                }
                if g.progress_hit.contains(x, y) {
                    return 10;
                }
                for (id, r) in g.buttons {
                    if id == 6 && !self.flag("TranslationButton", true) {
                        continue;
                    }
                    if r.contains(x, y) {
                        return id;
                    }
                }
            }
            if g.cover.contains(x, y) {
                return 1;
            }
        }
        0
    }
    unsafe fn sync_input_region(&mut self) -> Result<()> {
        // The click-through visual HWND retains the complete GPU surface, including
        // shadows. This separate, undrawn HWND receives input only inside the
        // animated contour. No alpha readback or cursor polling is required.
        let now = self.start.elapsed().as_secs_f64();
        let morph = self.island_morph.presentation(now, self.layout.host);
        let hidden = self.hidden_edge() && !morph.active;
        let (r, radius) = if morph.active && !self.alerts.visible() && !self.banner.visible() {
            (morph.rect, morph.radius)
        } else if hidden {
            (self.hidden_edge_hit_rect(), 2.5)
        } else if self.alerts.visible(){
            let(r,rad,_)=self.alerts.banner.presentation(now);(r,rad)
        } else if self.banner.visible() {
            let (r, radius, _) = self.banner.presentation(now);
            (r, radius)
        } else {
            self.shape()
        };
        let scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
        let satellite = if hidden || morph.active {
            Rect::default()
        } else if !self.mini()
            && !self.expanded
            && !self.banner.visible()
            && !self.alerts.visible()
            && self.components.len() > 1
        {
            self.layout.satellite()
        } else {
            Rect::default()
        };
        let key = if !hidden && self.opacity < 0.01 {
            [0; 10]
        } else {
            [
                (r.x * scale).floor() as i32,
                (r.y * scale).floor() as i32,
                ((r.x + r.w) * scale).ceil() as i32,
                ((r.y + r.h) * scale).ceil() as i32,
                (radius * 2. * scale).round() as i32,
                (radius * 2. * scale).round() as i32,
                (satellite.x * scale) as i32,
                (satellite.y * scale) as i32,
                ((satellite.x + satellite.w) * scale) as i32,
                ((satellite.y + satellite.h) * scale) as i32,
            ]
        };
        if self.input_region == Some(key) {
            return Ok(());
        }
        let region = if key[4] > 0 {
            CreateRoundRectRgn(key[0], key[1], key[2], key[3], key[4], key[5])
        } else {
            CreateRectRgn(key[0], key[1], key[2], key[3])
        };
        if key[8] > key[6] {
            let other = CreateRoundRectRgn(
                key[6],
                key[7],
                key[8],
                key[9],
                (satellite.w * scale) as i32,
                (satellite.h * scale) as i32,
            );
            let _ = CombineRgn(Some(region), Some(region), Some(other), RGN_OR);
            let _ = DeleteObject(other.into());
        }
        if region.is_invalid() {
            return Err(windows::core::Error::from_thread());
        }
        if SetWindowRgn(self.hwnd, Some(region), false) == 0 {
            let _ = DeleteObject(region.into());
            return Err(windows::core::Error::from_thread());
        }
        self.input_region = Some(key);
        Ok(())
    }
    unsafe fn remember_position(&mut self) {
        let mut bounds = RECT::default();
        let _ = GetWindowRect(self.hwnd, &mut bounds);
        let monitor = MonitorFromWindow(self.hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let _ = GetMonitorInfoW(monitor, &mut info);
        let w = info.rcWork;
        let scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
        let cx = ((bounds.left + bounds.right) as f64 / 2. - w.left as f64)
            / (w.right - w.left).max(1) as f64;
        let tf = ((bounds.top - w.top) as f64 + 24. * scale)
            / ((w.bottom - w.top) as f64 - 40. * scale).max(1.);
        self.settings["CenterX"] = json!(cx.clamp(0.05, 0.95));
        self.settings["TopFraction"] = json!(tf.clamp(0.0, 0.95));
        let mon_id = displays::monitor_id(monitor);
        if !mon_id.is_empty() {
            self.settings["Display"] = json!(mon_id);
        }
        let _ = self.save();
    }
    fn step_edge_reveal(&mut self, in_zone: bool, inside: bool, now: Instant) {
        if self.hidden_edge() || self.hidden_mini_mode() {
            return;
        }
        if self.user_hidden && !self.edge_revealed {
            if in_zone {
                let since = *self.edge_hover_since.get_or_insert(now);
                if now.duration_since(since) >= Duration::from_millis(160) {
                    self.edge_revealed = true;
                    self.edge_hover_since = None;
                    self.edge_leave_since = None;
                    self.display_wanted = true;
                    unsafe {
                        let _ = self.position();
                    }
                    self.fade(1.);
                    self.dirty = true;
                }
            } else {
                self.edge_hover_since = None;
            }
        } else if self.edge_revealed {
            if inside || in_zone || self.expanded || self.banner.visible() || self.alerts.visible() {
                self.edge_leave_since = None;
            } else {
                let since = *self.edge_leave_since.get_or_insert(now);
                if now.duration_since(since) >= Duration::from_millis(900) {
                    self.edge_leave_since = None;
                    self.enter_hidden_edge();
                    self.dirty = true;
                }
            }
        }
    }
    unsafe fn close_update(&mut self) {
        let target = self.update_return.clone();
        self.updates.dismiss();
        self.select_component(&target, -1);
        self.refresh_components();
        self.held = false;
        self.retarget(false);
        self.manual_until = None;
        self.update_until = None;
    }
    unsafe fn toggle_island_visibility(&mut self, from_tray: bool) {
        if self.practice.is_some() && !(from_tray && self.user_hidden && self.practice_step() == Some(PracticeStep::Recover)) { return; }
        self.restore_hidden = false;
        if self.user_hidden {
            self.reveal_hidden_edge();
            if from_tray { self.practice_event(PracticeStep::Recover); }
        } else {
            self.enter_hidden_edge();
        }
        let _ = self.position();
    }
    unsafe fn activate(&mut self, id: i32) {
        if self.practice.is_some() && id != 2 {
            if id == 0 {
                match (self.practice_step(), self.expanded) {
                    (Some(PracticeStep::ClickExpand), false) => {
                        self.held = true;
                        self.retarget(true);
                        self.practice_event(PracticeStep::ClickExpand);
                    }
                    (Some(PracticeStep::ClickCollapse), true) => {
                        self.held = false;
                        self.retarget(false);
                        self.practice_event(PracticeStep::ClickCollapse);
                    }
                    _ => {}
                }
            }
            return;
        }
        if self.selected == agent_island::ID && (agent_island::COLLAPSE..agent_island::OPTION+agent_island::OPTIONS as i32).contains(&id) {
            use agent_island::*;
            match id {
                COLLAPSE => { self.held=false; self.retarget(false); }
                CUSTOM => {
                    if self.agent.view.editing { self.end_agent_edit(true,true); }
                    else if self.agent.view.start_edit() {
                        self.agent_focus_return=GetForegroundWindow();
                        let _=SetForegroundWindow(self.hwnd);
                        let _=SetFocus(Some(self.hwnd));
                        if GetFocus()!=self.hwnd || GetForegroundWindow()!=self.hwnd {
                            self.end_agent_edit(false,false);
                            self.agent.view.feedback="无法取得输入焦点，请在 Codex 中回答";
                        }
                    }
                }
                PREV | NEXT => {
                    self.end_agent_edit(false,true);
                    if id==PREV {self.agent.view.page=self.agent.view.page.saturating_sub(1);}
                    else if self.agent.view.prompt.as_ref().is_some_and(|p|self.agent.view.page+1<p.questions.len()) {self.agent.view.page+=1;}
                }
                SEND => {
                    if let Some(request)=(!self.session_locked&&!self.island_suppressed()).then(||self.agent.view.submission()).flatten() {
                        if !self.agent.bridge.send(request) { self.agent.view.feedback="连接已断开，请在 Codex 中回答"; }
                    }
                }
                i if i >= OPTION => { self.end_agent_edit(false,true); self.agent.view.choose((i-OPTION) as usize); }
                _=>{}
            }
            self.dirty=true;
            return;
        }
        if id == 61 {
            if self.alerts.banner.open && self.alerts.active.as_ref().is_some_and(|a| a.hydration.is_some()) {
                match self.record_hydration() {
                    Ok(_) => {
                        let (r, rad) = self.shape();
                        self.alerts.dismiss(r, rad, self.start.elapsed().as_secs_f64(), self.flag("Animation",true));
                    }
                    Err(message) => { self.error = Some(message); }
                }
            }
            self.dirty = true;
            return;
        }
        if id==60{
            let code=self.alerts.active.as_ref().and_then(|a|a.code.clone());
            if let Some(code)=code{
                let result=verification_code::copy(&code,self.hwnd);
                if let Some(a)=self.alerts.active.as_mut(){a.detail=match result{Ok(())=>"已复制验证码".into(),Err(e)=>e};}
                self.alerts.banner.hold_until(self.start.elapsed().as_secs_f64()+1.8);
            }else{
                let(r,rad)=self.shape();self.alerts.dismiss(r,rad,self.start.elapsed().as_secs_f64(),self.flag("Animation",true));
            }
            self.dirty=true;return
        }
        match id {
            volume::BUTTON => {self.volume.refresh(true);self.tooltip_visible=true;self.dirty=true;},
            50 => match self.updates.phase() {
                "available" => {
                    self.show_settings_page("update");
                    self.pending_center_action = json!({"action":"updateReview"});
                }
                "ready" => {
                    self.updates.ask_apply();
                }
                "downloading" | "switching" | "verifying" => {
                    let _ = self.updates.cancel();
                }
                "completed" => self.close_update(),
                "failed" => {
                    self.request(json!({"version":1,"command":"checkUpdate"}));
                }
                _ => {}
            },
            51 | layout::ACTIVITY_CLOSE => {
                if let Err(error) = self.delete_activity() {
                    self.error = Some(error.clone());
                    self.show_notice(notifications::Notice{app_id:"LyricIsland.System.Delete".into(),app_name:"点屿".into(),title:"删除失败".into(),body:error,icon:None},true,None,false);
                }
            },
            52 => {
                if self.updates.skip() {
                    self.close_update();
                }
            }
            resident::FOCUS => {
                let result = self.request(json!({"version":1,"command":"timerCreate",
                    "value":{"mode":"pomodoro","focus":1500,"rest":300,"rounds":4}}));
                if result["success"] != true {
                    self.error = result["message"].as_str().map(str::to_owned);
                }
            }
            resident::TODO => {
                self.show_settings_page("todos");
                self.pending_center_action = json!({"action":"todoEdit","id":""});
            }
            resident::CENTER => self.show_settings(),
            resident::COLLAPSE => {
                self.held = false;
                self.retarget(false);
            }
            20 => self.cycle_component(1),
            21 => self.cycle_component(-1),
            30 => self.active_timer_action("toggle"),
            31 => self.active_timer_action("reset"),
            id if (40..=42).contains(&id) || id >= 1000 => {
                let idx = if id >= 1000 { (id - 1000) as usize } else { (id - 40) as usize };
                let task = self
                    .panel
                    .as_ref()
                    .and_then(|p| p.tasks.get(idx))
                    .cloned();
                if let Some(task) = task {
                    let _ = self.todos.complete(&task.id, true, &self.todo_now);
                    self.cancel_todo(&task.id);
                    self.refresh_components();
                }
            }
            11 | 12 => {
                let todo = self.banner_todo.clone();
                if id == 12 && !self.notification_preview {
                    if let Some(n) = self.banner.notice.clone() {
                        self.activate_notice(&n, &todo);
                    }
                }
                let (base, radius) = self.shape();
                self.banner.dismiss(
                    base,
                    radius,
                    self.start.elapsed().as_secs_f64(),
                    self.flag("Animation", true),
                    self.base_needed(),
                );
            }
            0 => {
                if self.expanded {
                    self.held = false;
                    self.retarget(false)
                } else if self.layout.anchor_bottom || self.selected == resident::ID || self.selected == agent_island::ID || self.mini() || self.flag("ClickAnywhereExpand", false) {
                    self.held = true;
                    self.retarget(true)
                }
            }
            1 | 13 => {
                if self.flag("MusicClickOpenApp", false) {
                    if let Err(e) =
                        activation::open(self.snapshot["sourceId"].as_str().unwrap_or(""))
                    {
                        self.error = Some(e);
                    }
                } else if id == 1 {
                    self.held = !self.held;
                    self.retarget(self.held)
                }
            }
            2 => {
                if self.flag("Floating", false) {
                    let mut cursor = POINT::default();
                    let mut bounds = RECT::default();
                    let _ = GetCursorPos(&mut cursor);
                    let _ = GetWindowRect(self.hwnd, &mut bounds);
                    self.dragging = Some((cursor, bounds));
                    SetCapture(self.hwnd);
                }
            }
            3 => self.command("previous", 0.),
            4 => {
                if self.demo {
                    self.snapshot["isPlaying"] = json!(self.snapshot["isPlaying"] != true)
                } else {
                    self.command("toggle", 0.)
                }
            }
            5 => self.command("next", 0.),
            14 => {
                self.enable_netease_silent();
            }
            15 => {
                self.seek_confirm = false;
                self.enable_netease_silent();
            }
            6 => {
                // Hidden 译 buttons have no click target at all, so the setting is read here too.
                if self.flag("TranslationButton", true) {
                    // The 译 button is the island's language switch, not a card-only switch.
                    // The collapsed capsule reads its second row from CompactTranslation, so
                    // both move together and the island can never be left in the other language.
                    let translated = !self.flag("Translation", true);
                    self.settings["Translation"] = json!(translated);
                    self.settings["CompactTranslation"] = json!(translated);
                    let next = self.compact_secondary_for();
                    if next != self.compact_secondary {
                        self.previous_compact_secondary =
                            std::mem::replace(&mut self.compact_secondary, next);
                    }
                    let _ = self.save();
                }
            }
            7 => {
                if self.selected == "todos" {
                    self.show_settings_page("todos")
                } else if self.selected.starts_with("timer:") {
                    self.show_settings_page("timers")
                } else {
                    self.show_settings()
                }
            }
            9 => {
                let title = self.snapshot["track"]["title"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                let source = self.snapshot["track"]["sourceApp"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                std::thread::spawn(move || {
                    shell::favorite(&title, &source, true);
                });
            }
            _ => {}
        }
        self.dirty = true;
    }
    fn update_seek(&mut self, x: f64) {
        let (shape, _) = self.shape();
        let scale = (shape.w / self.layout.card.w).min(shape.h / self.layout.card.h);
        let tx = shape.x + (shape.w - self.layout.card.w * scale) / 2.;
        let g = self.music_geometry();
        if g.track.w <= 0. {
            return;
        }
        self.seek_fraction = ((x - tx - g.track.x * scale) / (g.track.w * scale)).clamp(0., 1.);
        self.dirty = true;
    }
    /// NetEase reports a live timeline but the silent seek channel is not connected.
    fn seek_offer(&self) -> bool {
        !self.demo
            && self.snapshot["track"].is_object()
            && self.snapshot["track"]["songId"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
            && self.snapshot["canSeek"] == false
            && self.snapshot["sourceId"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("cloudmusic.exe"))
    }
    /// Explain automatic connection without changing the running music player.
    fn enable_netease_silent(&mut self) {
        self.seek_confirm = false;
        self.last_poll = Instant::now() - Duration::from_secs(1);
        self.dirty = true;
        if !self.demo {
            std::thread::spawn(|| unsafe {
                let body="正常启动网易云并播放歌曲，点屿会自动连接进度。\r\n\r\n无需脚本、特殊启动参数或重启。后台、最小化和托盘状态均可操作。\r\n\r\n尚未连接时请稍候，并保持两款应用的运行权限一致；未适配的客户端版本继续使用系统媒体会话。".encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                MessageBoxW(
                    None,
                    PCWSTR(body.as_ptr()),
                    windows::core::w!("网易云自动连接状态"),
                    MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND,
                );
            });
        }
    }
    fn progress_value(&self) -> f64 {
        self.progress.fraction(self.start.elapsed().as_secs_f64())
    }
    /// The banner surface as it is being drawn, so a probe can prove the card does not
    /// grow and shrink on its own while a message is on screen.
    fn notification_geometry(&self) -> Value {
        let (r, radius, mix) = self.banner.presentation(self.start.elapsed().as_secs_f64());
        json!({"open":self.banner.open,"x":r.x,"y":r.y,"w":r.w,"h":r.h,"radius":radius,"mix":mix})
    }
    fn music_preview(&self) -> Value {
        let s = self.scene();
        let (background, accent) = self
            .renderer
            .as_ref()
            .map(|r| r.preview_palette())
            .unwrap_or((0x1b1f25, 0xa9e6cd));
        let cover = self
            .data
            .join("Cache/MusicPreview")
            .join(format!("cover-{}.png", self.cover_version));
        json!({"lyricFrame":self.snapshot["frame"],"lyric":self.snapshot["frame"]["currentLine"],"isPureMusic":self.snapshot["frame"]["isPureMusic"],"secondary":s.secondary,"elapsed":s.elapsed,"duration":s.duration,"progress":s.progress,"highlight":s.highlight,"background":format!("#{background:06X}"),"accent":format!("#{accent:06X}"),"coverPath":if cover.is_file(){cover.to_string_lossy().into_owned()}else{String::new()},"width":s.layout.card.w})
    }
    fn auxiliary_timer_progress(&self) -> Option<(f64, f64)> {
        let index = self.components.iter().position(|c| c == &self.selected)?;
        let component = self.components.get((index + 1) % self.components.len())?;
        let id = component.strip_prefix("timer:")?;
        let timer = self
            .timers
            .items
            .iter()
            .find(|timer| timer.id == id && timer.mode != "stopwatch")?;
        Some((timer.progress(), timer.elapsed_ms as f64))
    }
    fn auxiliary_state(&self) -> (bool, f64) {
        let next = self
            .components
            .iter()
            .position(|c| c == &self.selected)
            .and_then(|i| self.components.get((i + 1) % self.components.len()));
        match next.map(String::as_str) {
            Some("update") => (self.updates.active(), 0.),
            Some("music") => (
                self.snapshot["isPlaying"] == true,
                self.start.elapsed().as_secs_f64() * 1000.,
            ),
            Some(id) if id.starts_with("timer:") => self
                .timers
                .items
                .iter()
                .find(|t| id == format!("timer:{}", t.id))
                .map(|t| (t.running, t.elapsed_ms as f64))
                .unwrap_or_default(),
            _ => (false, 0.),
        }
    }
    /// Hands the takeover window the island's own material, geometry and roles before exit.
    fn export_update_appearance(&mut self) {
        self.export_update_appearance_to(self.data.join("Cache/UpdateAppearance"));
    }
    fn export_update_appearance_to(&mut self, directory: PathBuf) {
        let host = self.layout.host;
        let card = self.layout.card;
        let scale = self
            .renderer
            .as_ref()
            .map(|r| (r.dpi as f64 / 96.).clamp(1., 3.))
            .unwrap_or(1.);
        let mut rect = RECT::default();
        let placed = unsafe { GetWindowRect(self.hwnd, &mut rect).is_ok() };
        let (x, y) = if placed {
            (rect.left, rect.top)
        } else {
            (0, 0)
        };
        let width = self
            .renderer
            .as_ref()
            .map(|r| r.width as i32)
            .unwrap_or_else(|| (host.w * scale).ceil().max(1.) as i32);
        let height = self
            .renderer
            .as_ref()
            .map(|r| r.height as i32)
            .unwrap_or_else(|| (host.h * scale).ceil().max(1.) as i32);
        // Export from the live renderer. Creating a second composition swapchain on
        // this HWND both stalled the UI and detached the island's existing visual.
        let Some(mut renderer) = self.renderer.take() else {
            self.error = Some("更新外观导出失败：渲染器未就绪".into());
            return;
        };
        let job = self.updates.job.clone();
        let result = {
            let mut scene = self.scene();
            scene.frame = Frame::expanded();
            scene.layout.host = Rect::new(0., 0., host.w, host.h);
            scene.layout.card = Rect::new(card.x, card.y, card.w, card.h);
            scene.layout_scale = scale;
            scene.notification = None;
            scene.notification_mix = 0.;
            scene.osd = None;
            scene.osd_mix = 0.;
            unsafe {
                renderer.capture_update_appearance(&scene, &directory, (x, y, width, height), &job)
            }
        };
        let live = self.scene();
        let _ = unsafe { renderer.render(&live, true) };
        self.renderer = Some(renderer);
        if let Err(error) = result {
            self.error = Some(format!("更新外观导出失败：{error}"));
        }
    }
    fn opacity_fraction(&self) -> f32 {
        (self.acrylic_preview["IslandOpacity"]
            .as_f64()
            .or_else(|| self.settings["IslandOpacity"].as_f64())
            .unwrap_or(100.)
            .clamp(25., 100.)
            / 100.) as f32
    }
    fn scene(&self) -> Scene<'_> {
        let elapsed = self.progress.seconds(self.start.elapsed().as_secs_f64());
        let duration = self.snapshot["track"]["durationSeconds"]
            .as_f64()
            .unwrap_or(0.);
        let track = &self.snapshot["track"];
        let f = &self.snapshot["frame"];
        // Sampled once: two calls per frame meant the bars and their opacity were read at
        // two different instants, so the analyser could disagree with itself mid-fade.
        let activity = self.activity.sample(
            self.start.elapsed().as_secs_f64() * 1000.,
            self.flag("Animation", true),
        );
        let col = |k: &str, default: u32| {
            self.settings[k]
                .as_str()
                .and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok())
                .map(|v| 0xff000000 | v)
                .unwrap_or(default)
        };
        let (notice_rect, notice_radius, notice_mix) =
            self.banner.presentation(self.start.elapsed().as_secs_f64());
        let (osd_rect, osd_radius, osd_mix) = self
            .osd_banner
            .presentation(self.start.elapsed().as_secs_f64());
        let (alert_rect,alert_radius,alert_mix)=self.alerts.banner.presentation(self.start.elapsed().as_secs_f64());
        Scene {
            agent: &self.agent.view,
            background_opacity: self.opacity_fraction(),
            alert: self.alerts.active.as_ref().filter(|_|self.alerts.visible()),
            alert_rect, alert_radius, alert_mix,
            glow: self.glow.frame(
                self.start.elapsed().as_secs_f64() * 1000.,
                self.flag("Animation", true),
            ),
            transient_only: (self.banner.visible() || self.osd_banner.visible() || self.alerts.visible())
                && !self.base_needed(),
            custom_geometry: if self.music_custom.enabled || self.custom_from.is_some() {
                Some(self.music_geometry())
            } else {
                None
            },
            previous_context: f["previousLine"].as_str().unwrap_or(""),
            next_context: f["nextLine"].as_str().unwrap_or(""),
            controls_opacity: if self.music_custom.enabled {
                1.
            } else {
                let ml = layout::MusicLayout::from_setting(
                    self.settings["MusicExpandedLayout"]
                        .as_str()
                        .unwrap_or("SideBySide"),
                );
                1. - lyric_core::music_layout::immersion_amount(ml, self.music_mix, self.music_custom.enabled)
                    * (1. - self.music_controls.value.clamp(0., 1.))
            },
            play_shape: self.play_shape.value.clamp(0., 1.),
            capsule_radius: if self.layout.anchor_bottom {
                8.0
            } else {
                self.settings["CapsuleCornerRadius"]
                    .as_f64()
                    .unwrap_or(self.layout.capsule.h / 2.)
            },
            taskbar_transparent: self.settings["TaskbarStyle"].as_str().unwrap_or("Transparent") != "Pill",
            taskbar_light: self.taskbar_light,
            taskbar_hovered: self.cursor_inside || self.pressed > 0,
            update_phase: self.updates.phase(),
            feedback: self.feedback.map(|v| v.value.clamp(0., 0.16) as f32),
            dynamic_source: self.settings["IslandColorSource"]
                .as_str()
                .unwrap_or("Content"),
            dynamic_system_seed: theme_accent::system_accent(),
            dynamic_custom_seed: self.settings["IslandDynamicSeedColor"]
                .as_str()
                .and_then(|s| u32::from_str_radix(s.trim_start_matches('#'), 16).ok())
                .unwrap_or(0xb1e8d1),
            dynamic_high_contrast: self.flag("DynamicColorHighContrast", false),
            font_family: self.settings["IslandFontFamilyRender"]
                .as_str()
                .unwrap_or("Noto Sans SC"),
            timer_mono: self.flag("TimerMonoDigits", false),
            hidden_edge: self.hidden_edge(),
            morph: self
                .island_morph
                .presentation(self.start.elapsed().as_secs_f64(), self.layout.host),
            expressive_dark: true,
            mini: self.mini(),
            mini_light_field: self.settings["MiniMaterial"] == "LightField",
            classic_capsule: crate::renderer::Material::from_setting(
                self.settings["ClassicCapsuleMaterial"]
                    .as_str()
                    .unwrap_or("LightField"),
            ),
            mini_capsule: crate::renderer::Material::from_setting(
                self.settings["MiniMaterial"].as_str().unwrap_or("LightField"),
            ),
            mini_ink: 0xfff4f6f8,
            mini_outline: 0xe0000000,
            translation_button: self.flag("TranslationButton", true),
            translation_enabled: self.flag("Translation", true),
            translation_tooltip: self.tooltip_visible,
            volume_level: self.volume.level,
            volume_muted: self.volume.muted,
            tool_theme: theme_accent::resolve(&self.settings),
            material: crate::renderer::Material::expanded(&self.settings, self.mini()),
            popup_material: crate::renderer::Material::from_setting(
                self.settings["PopupMaterial"].as_str().unwrap_or("Acrylic"),
            ),
            drag_panel: self.drag_panel.as_ref(),
            drag_active: self.swipe.is_some_and(|s| s.active),
            switch_outgoing: self.slide_outgoing,
            switch_direction: self.slide_direction,
            auxiliary_timer: self.auxiliary_timer_progress(),
            auxiliary_running: self.auxiliary_state().0,
            auxiliary_elapsed: self.auxiliary_state().1,
            auxiliary_update_progress: self.updates.panel().progress,
            auxiliary_update_completed: matches!(self.updates.phase(), "ready" | "completed"),
            auxiliary_kind: self
                .components
                .iter()
                .position(|c| c == &self.selected)
                .and_then(|i| self.components.get((i + 1) % self.components.len()))
                .map(|s| {
                    if s == "todos" {
                        2
                    } else if s == "update" {
                        3
                    } else if s.starts_with("timer:") {
                        1
                    } else {
                        0
                    }
                })
                .unwrap_or(0),
            panel: self.panel.as_ref(),
            previous_panel: self.previous_panel.as_ref(),
            resident: &self.resident.view,
            component_count: self.components.len(),
            switching: self.switching,
            switch_offset: self.slide.value,
            swipe_offset: self.swipe_offset(),
            todo_scroll: self.todo_scroll,
            lyric_cover_color: self.flag("LyricColorFromCover", true),
            cover_background: self.flag("CoverBackground", false),
            expanded_cover_background: self.flag("ExpandedMonetEnabled", true),
            cover_overlay: self.flag("ExpandedCoverOverlay", true),
            acrylic_card_frost: self.acrylic_preview["AcrylicCardFrost"]
                .as_f64()
                .or_else(|| self.settings["AcrylicCardFrost"].as_f64())
                .unwrap_or(40.),
            acrylic_card_opacity: self.acrylic_preview["AcrylicCardOpacity"]
                .as_f64()
                .or_else(|| self.settings["AcrylicCardOpacity"].as_f64())
                .unwrap_or(60.),
            acrylic_capsule_frost: self.acrylic_preview["AcrylicCapsuleFrost"]
                .as_f64()
                .or_else(|| self.settings["AcrylicCapsuleFrost"].as_f64())
                .unwrap_or(40.),
            acrylic_capsule_opacity: self.acrylic_preview["AcrylicCapsuleOpacity"]
                .as_f64()
                .or_else(|| self.settings["AcrylicCapsuleOpacity"].as_f64())
                .unwrap_or(0.),
            liquid_glass_refraction: self.acrylic_preview["LiquidGlassRefraction"]
                .as_f64()
                .or_else(|| self.settings["LiquidGlassRefraction"].as_f64())
                .unwrap_or(68.),
            liquid_glass_dispersion: self.acrylic_preview["LiquidGlassDispersion"]
                .as_f64()
                .or_else(|| self.settings["LiquidGlassDispersion"].as_f64())
                .unwrap_or(38.),
            liquid_glass_blur: self.acrylic_preview["LiquidGlassBlur"]
                .as_f64()
                .or_else(|| self.settings["LiquidGlassBlur"].as_f64())
                .unwrap_or(32.),
            liquid_glass_tint: self.acrylic_preview["LiquidGlassTint"]
                .as_f64()
                .or_else(|| self.settings["LiquidGlassTint"].as_f64())
                .unwrap_or(42.),
            demo: self.demo,
            animate: self.flag("Animation", true),
            notification: self.banner.notice.as_ref(),
            notification_rect: notice_rect,
            notification_radius: notice_radius,
            notification_mix: notice_mix,
            osd: if self.banner.visible() {
                None
            } else {
                self.osd_banner.notice.as_ref()
            },
            osd_rect,
            osd_radius,
            osd_mix,
            layout: self.layout,
            frame: self.frame,
            radius: self.options().radius,
            resizing: self.transition.is_some(),
            time_ms: self.start.elapsed().as_secs_f64() * 1000.,
            lyric: &self.lyric,
            previous_lyric: &self.previous_lyric,
            compact_secondary: &self.compact_secondary,
            previous_compact_secondary: &self.previous_compact_secondary,
            lyric_age_ms: if self.flag("Animation", true) {
                self.lyric_start.elapsed().as_secs_f64() * 1000.
            } else {
                1000.
            },
            secondary: if self.flag("Translation", true) {
                f["currentTranslation"]
                    .as_str()
                    .filter(|s| !s.is_empty())
                    .or_else(|| f["nextLine"].as_str())
                    .unwrap_or("等待歌词")
            } else {
                f["nextLine"].as_str().unwrap_or("等待歌词")
            },
            title: track["title"].as_str().unwrap_or("暂无播放"),
            artist: track["artist"]
                .as_str()
                .unwrap_or("打开播放器，开始一首音乐"),
            playing: self.snapshot["isPlaying"] == true,
            seek_offer: self.seek_offer(),
            seek_confirm: self.seek_confirm,
            highlight: if self.flag("WordHighlight", true) {
                f["wordScanProgress"].as_f64().unwrap_or(0.).clamp(0., 1.)
            } else {
                0.
            },
            progress: if self.seeking {
                self.seek_fraction
            } else {
                self.progress_value()
            },
            elapsed: if self.seeking {
                self.seek_fraction * duration
            } else {
                elapsed
            },
            duration,
            font_size: self.settings["FontSize"].as_f64().unwrap_or(12.5),
            music_layout: layout::MusicLayout::from_setting(
                self.settings["MusicExpandedLayout"].as_str().unwrap_or("SideBySide"),
            ),
            music_mix: self.music_mix,
            text_color: col("ExpandedTextColor", 0xffffffff),
            component_color: col("ExpandedComponentColor", 0xffffffff),
            progress_color: col("LyricProgressColor", 0xffddf6ff),
            progress_bar: dotisle_render::progress::BarStyle::from_setting(
                self.settings["ProgressBarStyle"].as_str().unwrap_or("Normal"),
            ),
            progress_thumb: if self.flag("ProgressBarThumbEnabled", false) {
                dotisle_render::progress::ThumbStyle::from_setting(
                    self.settings["ProgressBarThumb"].as_str().unwrap_or("Pill"),
                )
            } else {
                dotisle_render::progress::ThumbStyle::None
            },
            progress_thumb_spin: self.flag("ProgressBarThumbSpin", false),
            progress_glow: self.flag("ProgressBarGlow", true),
            lyric_glow: self.flag("MusicLyricGlow", true),
            transport_theme: dotisle_render::transport::TransportTheme::from_setting(
                self.settings["TransportTheme"].as_str().unwrap_or("Reef"),
            ),
            hover: self.hover,
            pressed: self.pressed,
            opacity: self.opacity as f32,
            transparent: self.transparent,
            floating: !self.mini() && self.flag("Floating", false),
            practice_move: self.practice_step() == Some(PracticeStep::Move),
            favorite_enabled: track["sourceApp"]
                .as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case("Netease")),
            has_track: track.is_object(),
            layout_scale: unsafe { GetDpiForWindow(self.hwnd).max(96) as f64 / 96. },
            activity_scales: activity.0,
            activity_opacity: activity.1,
            cover_opacity: 1.,
            cover_age_ms: self.cover_start.elapsed().as_secs_f64() * 1000.,
            outgoing_opacity: self.outgoing_opacity,
            outgoing_y: self.outgoing_y,
        }
    }
    unsafe fn music_layout_preview(&mut self, value: &Value) -> std::result::Result<Value, String> {
        use lyric_core::custom_layout::{element_label, elements, resizable, Layout};

        let draft = Layout::from_value(value)?;
        self.ensure_renderer().map_err(|error| error.to_string())?;
        let mut renderer = self.renderer.take().unwrap();
        let result = (|| -> std::result::Result<Value, String> {
            let mut s = self.scene();
            s.music_mix = s.music_layout.position();
            let width = s.layout.card.w;
            let geometry = if draft.enabled {
                let geometry = draft.geometry(width, s.layout_scale, s.font_size, s.music_mix);
                s.custom_geometry = Some(geometry);
                s.transport_theme = dotisle_render::transport::TransportTheme::from_setting(&draft.transport_theme);
                s.progress_bar = dotisle_render::progress::BarStyle::from_setting(&draft.bar_style);
                s.progress_thumb = if draft.thumb_enabled {
                    dotisle_render::progress::ThumbStyle::from_setting(&draft.thumb_style)
                } else {
                    dotisle_render::progress::ThumbStyle::None
                };
                s.progress_thumb_spin = draft.thumb_spin;
                geometry
            } else {
                s.custom_geometry = None;
                layout::music_geometry(width, s.music_layout, s.music_mix, s.layout_scale, s.font_size)
            };
            // Render the settled expanded music scene, without presenting a second island.
            let pad = 20.;
            let height = geometry.height;
            s.layout.card = Rect::new(pad, pad, width, height);
            s.layout.host = Rect::new(0., 0., width + 2. * pad, height + 2. * pad);
            s.frame = Frame::expanded();
            s.opacity = 1.;
            s.controls_opacity = 1.;
            s.component_count = 1;
            s.panel = None;
            s.notification = None;
            s.osd = None;
            s.alert = None;
            s.transient_only = false;
            s.drag_active = false;
            s.switching = false;
            s.switch_offset = 0.;
            s.swipe_offset = 0.;
            s.resizing = false;
            s.hover = 0;
            s.pressed = 0;
            s.feedback = [0.; 64];
            s.translation_tooltip = false;
            s.seek_offer = false;
            s.lyric_age_ms = 1000.;
            s.cover_age_ms = 1000.;
            let png = renderer.capture_layout_frame(&s).map_err(|error| error.to_string())?;
            let hitboxes = elements(&geometry).into_iter().map(|(id, rect)| {
                json!({"id":id,"label":element_label(id),"resizable":resizable(id),
                    "x":rect.x,"y":rect.y,"w":rect.w,"h":rect.h})
            }).collect::<Vec<_>>();
            Ok(json!({"success":true,"width":width,"height":height,"padding":pad,
                "elements":hitboxes,"hasTrack":s.has_track,
                "png":base64::engine::general_purpose::STANDARD.encode(png)}))
        })();
        self.renderer = Some(renderer);
        result
    }
    fn uses_liquid_glass_material(&self) -> bool {
        [
            "IslandMaterial",
            "ClassicCapsuleMaterial",
            "MiniMaterial",
            "MiniExpandedMaterial",
            "PopupMaterial",
        ]
        .iter()
        .any(|k| self.settings[k].as_str() == Some("LiquidGlass"))
    }
    fn liquid_glass_source(&self) -> backdrop_capture::BackdropSource {
        backdrop_capture::BackdropSource::from_setting(
            self.settings["LiquidGlassSource"]
                .as_str()
                .unwrap_or("Live"),
        )
    }
    unsafe fn draw(&mut self) -> Result<()> {
        if self.hidden_edge() || self.hidden_mini_mode() || self.island_morph.active() {
            self.opacity = 1.;
            self.display_wanted = true;
        }
        // Take the renderer to split immutable scene data from mutable graphics state.
        if self.island_suppressed() {
            self.dirty = false;
            return Ok(());
        }
        let showing = self.opacity > 0.001;
        // A fully transparent swapchain can leave DWM presenting the last visible
        // frame (a pale ghost of the capsule). Hide the visual window instead of
        // trusting the compositor to discard it.
        if !showing {
            self.backdrop_capturer.deactivate(self.visual_hwnd);
            if let Some(r) = &mut self.renderer {
                r.release_liquid_glass();
            }
            if IsWindowVisible(self.visual_hwnd).as_bool() {
                let _ = ShowWindow(self.visual_hwnd, SW_HIDE);
            }
            self.dirty = false;
            return Ok(());
        }
        self.ensure_renderer()?;
        let active_lg = self.uses_liquid_glass_material() && !(self.hidden_edge() && !self.island_morph.active());
        if active_lg {
            let animating = self.island_morph.active() || self.frame.card_reveal > 0.01 && self.frame.card_reveal < 0.99;
            if let Some(cap) = self.backdrop_capturer.capture_if_due(
                self.visual_hwnd,
                self.liquid_glass_source(),
                animating,
            ) {
                if let Some(r) = &mut self.renderer {
                    r.update_liquid_glass_backdrop(&cap.rgba, cap.width, cap.height);
                }
            }
        } else {
            self.backdrop_capturer.deactivate(self.visual_hwnd);
            if let Some(r) = &mut self.renderer {
                r.release_liquid_glass();
            }
        }
        if let Some(mut r) = self.renderer.take() {
            let result = r.render(&self.scene(), true);
            self.renderer = Some(r);
            result?;
            self.render_frames += 1;
            if self.render_frames == 1 {
                if let Some(dir) = std::env::var_os("DOTISLE_PROFILE_DIR") {
                    let dir = std::path::PathBuf::from(dir);
                    let _ = std::fs::create_dir_all(&dir);
                    let _=std::fs::write(dir.join("native-first-frame.json"),json!({"firstAppFrameMs":self.start.elapsed().as_secs_f64()*1000.,"unixMs":std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis()}).to_string());
                }
            }
        }
        if !IsWindowVisible(self.visual_hwnd).as_bool() {
            let _ = ShowWindow(self.visual_hwnd, SW_SHOWNA);
            if self.taskbar_mode() {
                let _ = window_level::apply(self.visual_hwnd, self.hwnd, true, false);
            }
        }
        self.continuous_last = Instant::now();
        self.dirty = false;
        Ok(())
    }
    /// Build the GPU renderer on demand: a hidden or idle island pays nothing for
    /// a GPU context it is not painting with. The
    /// first visible frame after a long idle period pays the creation cost instead.
    unsafe fn ensure_renderer(&mut self) -> Result<()> {
        if self.renderer.is_some() {
            return Ok(());
        }
        let dpi = GetDpiForWindow(self.hwnd).max(96) as f32;
        let scale = dpi as f64 / 96.;
        let width = (self.layout.host.w * scale).ceil() as u32;
        let height = (self.layout.host.h * scale).ceil() as u32;
        let font = self.base.join("Assets/Fonts/NotoSansSC.ttf");
        self.renderer = Some(Renderer::new(self.visual_hwnd, width, height, dpi, &font)?);
        if let Some(prepared) = self.pending_cover.take() {
            if let Some(r) = &mut self.renderer {
                r.apply_cover(prepared);
            }
        }
        self.renderer_generation += 1;
        self.cover_version = u64::MAX;
        self.last_poll = Instant::now() - Duration::from_secs(1);
        Ok(())
    }
    fn activate_notice(&mut self, notice: &notifications::Notice, todo: &Option<String>) {
        if todo.as_ref().is_some_and(|id| id.starts_with("device:")) {
        } else if todo.as_ref().is_some_and(|id| id.starts_with("timer:")) {
            self.show_settings_page("timers");
        } else if todo.as_ref().is_some_and(|id| id.starts_with("focus:")) {
            self.show_settings_page("timers");
        } else if todo.as_ref().is_some_and(|id| id.starts_with("rule:")) {
            self.show_settings_page("notifications");
        } else if todo.is_some() {
            self.show_settings_page("todos");
        } else if self.flag("NotificationClickOpenApp", true) {
            if let Err(e) = activation::open(&notice.app_id) {
                self.error = Some(e);
            }
        }
    }
    fn show_notice(
        &mut self,
        notice: notifications::Notice,
        preview: bool,
        todo: Option<String>,
        chime: bool,
    ) {
        if !preview && todo.is_none() {
            let policy = notifications::Policy::from_settings(&self.settings);
            if !policy.should_show() || !policy.permits(&notice.app_id, &notice.app_name) {
                return;
            }
        }
        if self.fullscreen_suppressed { return; }
        if self.practice.is_some() {
            if !preview { self.enqueue(Queued{notice,todo,chime}); }
            return;
        }
        let device = notice.app_id.starts_with("LyricIsland.Device");
        // 任务栏模式下通知依然照常在屏幕正上方独立弹窗展示，绝不从底部任务栏冒出
        if self.flag("TaskbarModeEnabled", false) {
            self.notification_preview = preview;
            let font = self.base.join("Assets/Fonts/NotoSansSC.ttf");
            let animate = self.flag("Animation", true);
            let theme = self.hint_theme();
            let monitor = self.target_monitor();
            if let Err(e) = unsafe {
                self.top_popup
                    .show_notice(notice, todo.clone(), monitor, &font, animate, theme)
            } {
                self.error = Some(e.to_string());
            }
            if !preview && chime {
                if let Err(e) = if device {
                    sound::play_device(&self.settings, false)
                } else {
                    sound::play(&self.settings, todo.is_some(), false)
                } {
                    self.error = Some(e);
                }
            }
            return;
        }
        if self.alerts.visible(){
            if preview{self.alerts=Default::default();}
            else{self.enqueue(Queued{notice,todo,chime});return;}
        }
        if !preview && !self.wake_for_transient_notice() {
            return;
        }
        // The banner height below is measured once, so the renderer must exist here
        // even when the island was fully retired a moment ago.
        unsafe {
            let _ = self.ensure_renderer();
        }
        self.notification_preview = preview;
        self.banner_todo = todo.clone();
        let now = self.start.elapsed().as_secs_f64();
        if self.osd_banner.visible() {
            let (r, rad) = self.shape();
            self.osd_banner
                .dismiss(r, rad, now, false, self.base_needed());
        }
        let (base, radius) = self.shape();
        if let Some(r) = &mut self.renderer {
            let _ = unsafe { r.set_notification_icon(notice.icon.as_deref()) };
        }
        let width = 440_f64.min(self.layout.host.w - 48.);
        let height = self
            .renderer
            .as_mut()
            .and_then(|r| unsafe { r.notification_height(&notice, width) }.ok())
            .unwrap_or(80.);
        self.banner.show(
            notice,
            base,
            radius,
            self.layout.host.w,
            height,
            now,
            self.flag("Animation", true),
        );
        // The chime is bound to a banner actually appearing. Previews, paused
        // notifications, whitelist rejects and repeats never reach this point.
        if !preview && chime {
            if let Err(e) = if device {
                sound::play_device(&self.settings, false)
            } else {
                sound::play(&self.settings, todo.is_some(), false)
            } {
                self.error = Some(e);
            }
        }
        self.hide_at = None;
        self.fade(1.);
        self.dirty = true;
    }
    /// System hints share the island surface but never the notification queue: they are
    /// ephemeral, so a newer state replaces the previous one instead of stacking.
    fn hint_theme(&self) -> renderer::MaterialTheme {
        let material = renderer::Material::from_setting(
            self.settings["PopupMaterial"].as_str().unwrap_or("Acrylic"),
        );
        let cover = self.flag("ExpandedMonetEnabled", true);
        self.renderer
            .as_ref()
            .map(|r| r.material_theme(material, cover))
            .unwrap_or_else(|| renderer::MaterialTheme::new(material, cover))
    }
    fn sync_bottom_material(&mut self) {
        if self.bottom_osd.visible() {
            let theme = self.hint_theme();
            if let Err(e) = unsafe { self.bottom_osd.set_theme(theme) } {
                self.error = Some(e.to_string());
            }
        }
        if self.top_popup.visible() {
            let theme = self.hint_theme();
            if let Err(e) = unsafe { self.top_popup.set_theme(theme) } {
                self.error = Some(e.to_string());
            }
        }
    }
    fn show_osd(&mut self, hint: osd::Hint, preview: bool) {
        if !preview
            && (!self.flag("SystemStateHintsEnabled", true)
                || self.flag("PauseNotifications", false))
        {
            return;
        }
        if self.demo && !preview {
            return;
        }
        if self.settings["SystemStateHintPlacement"].as_str() == Some("Bottom") {
            let font = self.base.join("Assets/Fonts/NotoSansSC.ttf");
            let animate = self.flag("Animation", true);
            let theme = self.hint_theme();
            if let Err(e) = unsafe {
                self.bottom_osd
                    .show(hint.notice(), self.hwnd, &font, animate, theme)
            } {
                self.error = Some(e.to_string());
            }
            return;
        }
        if self.fullscreen_suppressed { return; }
        if self.flag("TaskbarModeEnabled", false) {
            if !preview && (self.top_popup.notice_visible() || self.top_popup.alert_visible()) {
                return;
            }
            let font = self.base.join("Assets/Fonts/NotoSansSC.ttf");
            let animate = self.flag("Animation", true);
            let theme = self.hint_theme();
            let monitor = self.target_monitor();
            if let Err(e) = unsafe {
                self.top_popup
                    .show_osd(hint.notice(), monitor, &font, animate, theme)
            } {
                self.error = Some(e.to_string());
            }
            return;
        }
        if self.practice.is_some() || (self.user_hidden && !preview) {
            return;
        }
        let now = self.start.elapsed().as_secs_f64();
        // A message the user is reading always wins; the hint is transient and is dropped
        // instead of cutting the notification short.
        if self.banner.visible() || self.alerts.visible() {
            if !preview {
                return;
            }
            let (r, rad) = self.shape();
            self.banner.dismiss(r, rad, now, false, self.base_needed());
        }
        let (base, radius) = self.shape();
        self.osd_banner.show_compact(
            hint.notice(),
            base,
            radius,
            self.layout.host.w,
            now,
            self.flag("Animation", true),
        );
        self.hide_at = None;
        self.fade(1.);
        self.dirty = true;
    }
    fn flush_osd(&mut self) {
        if !self.flag("SystemStateHintsEnabled", true) {
            self.pending_osd = 0;
            return;
        }
        let pending = std::mem::take(&mut self.pending_osd);
        if pending == 0 {
            return;
        }
        for hint in self.osd.flush(pending, true, true, true) {
            self.show_osd(hint, false);
        }
    }
    fn apply_osd(&mut self) {
        if self.demo {
            return;
        }
        if self.flag("SystemStateHintsEnabled", true) {
            unsafe {
                self.osd.install(self.hwnd);
            }
        } else {
            self.osd.uninstall();
            self.pending_osd = 0;
            unsafe {
                self.bottom_osd.clear();
            self.top_popup.clear();
            }
            if self.osd_banner.visible() {
                let now = self.start.elapsed().as_secs_f64();
                let (r, rad) = self.shape();
                self.osd_banner
                    .dismiss(r, rad, now, false, self.base_needed());
            }
        }
    }
    // Settle the old state before any clock mutation. The message queue runs
    // before tick(), so applying its whole gap after resume/next would charge
    // paused time to the new phase (or time before creation to a new timer).
    fn advance_timers(&mut self, now: Instant) {
        let dt = now.saturating_duration_since(self.last_timer);
        self.last_timer = now;
        if !self.timers.items.iter().any(|t| t.running && !t.finished) {
            self.timer_fraction = 0;
            return;
        }
        let nanos = dt.as_nanos() + self.timer_fraction;
        self.timer_fraction = nanos % 1_000_000;
        let elapsed_ms = (nanos / 1_000_000).min(u64::MAX as u128) as u64;
        if elapsed_ms == 0 { return; }
        if self.timers.items.iter().any(|t| t.running && !t.finished &&
            (t.mode == "workflow" || (t.mode == "pomodoro" && t.phase == "focus"))) {
            self.widget_data.advance_focus(&self.timers.items, elapsed_ms);
        }
        let events = self.timers.advance(elapsed_ms);
        if !events.is_empty() {
            for t in events {
                let notice = notifications::Notice {
                    app_id: "LyricIsland.Timer".into(),
                    app_name: "计时器".into(),
                    title: t.name.clone(),
                    body: if t.finished {
                        "计时已完成".into()
                    } else {
                        t.subtitle()
                    },
                    icon: None,
                };
                self.enqueue(Queued {
                    notice: notice.clone(),
                    todo: Some(format!("timer:{}", t.id)),
                    chime: true,
                });
                self.record_history("timer", &notice, None);
            }
            if let Err(e) = self.timers.persist() {
                self.error = Some(e);
            }
        }
    }
    fn record_hydration(&mut self) -> std::result::Result<reminder_plugins::Hydration, String> {
        let date = todos::Now::local().date;
        let next = self.reminder_plugins.hydration(&self.settings["HydrationToday"], &date).log(&date)?;
        let before = std::mem::replace(&mut self.settings["HydrationToday"], next);
        if let Err(error) = self.save() {
            self.settings["HydrationToday"] = before;
            return Err(error);
        }
        let progress = self.reminder_plugins.hydration(&self.settings["HydrationToday"], &date);
        if let Some(alert) = self.alerts.active.as_mut().filter(|a| a.hydration.is_some()) {
            alert.hydration = Some(progress);
            self.dirty = true;
        }
        if progress.drank_ml >= progress.goal_ml {
            self.alerts.remove_pending(reminder_plugins::Kind::Water.alert_key());
        }
        Ok(progress)
    }
    fn tick_reminder_plugins(&mut self, sample: screen_time::Sample, foreground_protected: bool) {
        if !self.reminder_plugins.enabled() { return; }
        let can_show = !foreground_protected && !self.session_locked && self.practice.is_none() && !self.island_suppressed() && !self.flag("PauseNotifications", false);
        let due = self.reminder_plugins.sample(sample.active_ms, sample.continuous, can_show);
        for kind in reminder_plugins::Kind::ALL {
            if !due[kind as usize] { continue; }
            let mut alert = capsule_alert::Alert::wellness(kind);
            if kind == reminder_plugins::Kind::Water {
                let progress = self.reminder_plugins.hydration(&self.settings["HydrationToday"], &todos::Now::local().date);
                if progress.drank_ml >= progress.goal_ml { continue; }
                alert = alert.with_hydration(progress);
            }
            if !self.wake_for_transient_notice() { continue; }
            self.alerts.enqueue(alert);
        }
    }
    fn suppress_wellness_interrupt(&mut self) {
        self.dirty |= self.alerts.cancel_wellness();
        self.finish_transient();
    }
    unsafe fn hide_island_windows(&mut self) {
        let _ = ShowWindow(self.hwnd, SW_HIDE);
        let _ = ShowWindow(self.visual_hwnd, SW_HIDE);
        self.backdrop_capturer.deactivate(self.visual_hwnd);
    }
    unsafe fn poll_island_visibility(&mut self, force: bool) -> Result<()> {
        let docked = self.taskbar_mode() && !self.demo;
        let hide_in_fullscreen = !self.demo && self.flag("HideIslandWhenFullscreen", false);
        let was_suppressed = self.island_suppressed();
        if !docked && !hide_in_fullscreen && !was_suppressed { return Ok(()); }
        if !force && self.last_island_visibility_check.elapsed() < Duration::from_millis(200) { return Ok(()); }
        self.last_island_visibility_check = Instant::now();
        let monitor = self.target_monitor();
        let probe = if docked { taskbar::probe_for_monitor(monitor) } else { None };
        let was_fullscreen = self.fullscreen_suppressed;
        self.fullscreen_suppressed = hide_in_fullscreen && taskbar::foreground_fullscreen_on(monitor);
        self.taskbar_suppressed = docked && taskbar::should_suppress(monitor, probe, self.hwnd, self.visual_hwnd);
        if self.island_suppressed() {
            if !was_suppressed {
                self.hide_island_windows();
                let (shape, radius) = self.shape();
                self.alerts.clear(shape, radius, self.start.elapsed().as_secs_f64(), false);
                self.finish_transient();
            }
            if !was_suppressed || (self.fullscreen_suppressed && !was_fullscreen) {
                self.top_popup.clear();
            }
            return Ok(());
        }
        if was_suppressed {
            self.position_on(monitor)?;
            // Restore the input window without activating it or changing manual visibility.
            let _ = ShowWindow(self.hwnd, SW_SHOWNA);
            self.dirty = true;
        }
        if !docked { return Ok(()); }
        if probe != self.last_taskbar_probe { self.position()?; }
        let light = taskbar::is_system_light_taskbar();
        if light != self.taskbar_light { self.taskbar_light = light; self.dirty = true; }
        if self.opacity > 0.001 && probe.is_some_and(|p| taskbar::is_behind_taskbar(self.visual_hwnd, p.hwnd)
            || taskbar::is_behind_taskbar(self.hwnd, p.hwnd)) {
            window_level::apply(self.visual_hwnd, self.hwnd, true, false)?;
        }
        Ok(())
    }
    unsafe fn end_agent_edit(&mut self, save: bool, restore: bool) {
        self.agent.view.finish_edit(save);
        let prev=std::mem::take(&mut self.agent_focus_return);
        unsafe {
            if restore && !prev.0.is_null() && prev!=self.hwnd && GetForegroundWindow()==self.hwnd && IsWindow(Some(prev)).as_bool() {
                let _=SetForegroundWindow(prev);
            }
        }
        self.dirty=true;
    }
    unsafe fn tick_agents(&mut self, enabled: bool, can_interrupt: bool) -> Result<()> { unsafe {
        let (old_visible, old_height, old_editing)=(self.agent_visible(), self.agent.view.height(), self.agent.view.editing);
        let (changed, appeared, question)=self.agent.poll(enabled,self.ai_quota.readings());
        if old_editing && !self.agent.view.editing {self.end_agent_edit(false,true);}
        if !changed {return Ok(());}
        if appeared {self.ai_quota.refresh_soon();}
        let surface_ended=old_visible&&!self.agent_visible();
        if surface_ended && self.selected==agent_island::ID {
            if let Some(previous)=self.agent_previous.take(){self.selected=previous;}else{self.selected="music".into();}
        }
        if !self.agent_visible() {self.agent_previous=None;}
        self.refresh_components();
        if (appeared||question) && can_interrupt && !self.user_hidden && self.components.iter().any(|id|id==agent_island::ID) {
            if self.selected!=agent_island::ID {
                self.agent_previous=Some(self.selected.clone());
                self.select_component(agent_island::ID,1);
            }
            if question && !self.expanded { self.agent_auto_open=true; self.held=true; self.retarget(true); }
        }
        if self.agent_auto_open && self.agent.view.prompt.is_none() {
            self.held=false;
            self.retarget(false);
        }
        if surface_ended || (self.selected==agent_island::ID && (appeared||old_height!=self.agent.view.height())) {
            self.position()?;
        }
        self.dirty=true;
        Ok(())
    }}
    unsafe fn tick(&mut self) -> Result<()> {
        self.poll_island_visibility(false)?;
        if !self.demo {
            let settings = ai_quota::Settings::from_value(&self.settings["AiQuotaMonitor"]);
            let can_show = !self.session_locked && !self.island_suppressed() && self.practice.is_none() && !self.flag("PauseNotifications", false);
            for alert in self.ai_quota.poll(settings, can_show, &self.settings["AiQuotaSources"], &self.data) {
                if self.wake_for_transient_notice() { self.alerts.enqueue(alert); }
            }
            self.tick_agents(settings.enabled && self.practice.is_none() && !self.session_locked, can_show)?;
        }
        self.practice_expiry();
        if self.module_updates.tick(){self.dirty=true;}
        self.reap_center();
        self.ensure_widget_host();
        if self.opacity > 0.001 {
            self.last_visible = Instant::now();
        }
        // Keep a short reuse window for stacked notices, but release the device
        // while hidden instead of retaining surface/atlas memory for 20 seconds.
        if self.renderer.is_some()
            && self.opacity <= 0.001 && self.fade_to <= 0.001
            && self.last_visible.elapsed() >= Duration::from_secs(8)
            && !self.banner.visible()
            && !self.osd_banner.visible()
            && !self.bottom_osd.visible()
            && self.queue.is_empty()
            && self.cover_rx.is_none()
            && !self.updates.busy()
        {
            self.renderer = None;
            self.renderer_generation += 1;
        }
        self.bottom_osd.tick()?;
        match self.top_popup.tick()? {
            Some(top_popup::Event::ClickNotice(notice, todo)) => {
                if !self.notification_preview {
                    self.activate_notice(&notice, &todo);
                }
            }
            Some(top_popup::Event::CopyCode(code)) => {
                let res = verification_code::copy(&code, self.hwnd);
                self.top_popup
                    .set_copy_feedback(res.is_ok(), res.err().unwrap_or_default());
            }
            None => {}
        }

        if self
            .updates
            .tick(self.flag("AutoCheckUpdatesEnabled", true), self.demo)
        {
            self.dirty = true;
        }
        if self.updates.reminder && self.practice.is_none() {
            self.update_card_deleted = false;
            self.updates.reminder = false;
            if !self.flag("PauseNotifications", false) {
                if self.selected != "update" {
                    self.update_return = self.selected.clone();
                }
                self.refresh_components();
                self.select_component("update", 1);
                self.update_until = Some(Instant::now() + Duration::from_secs(8));
                self.user_hidden = false;
                self.hidden_mini = false;
                self.fade(1.);
            }
        }
        if self
            .update_until
            .is_some_and(|until| Instant::now() > until)
            && !self.updates.busy()
            && !self.held
        {
            self.update_until = None;
            let previous = self.update_return.clone();
            self.select_component(&previous, -1);
            self.manual_until = None;
        }
        let gates = devices::Gates {
            master: self.flag("DeviceRemindersEnabled", true)
                && !self.flag("PauseNotifications", false),
            disconnect: self.flag("DeviceDisconnectRemindersEnabled", true),
            headphones: self.flag("HeadphoneConnectionRemindersEnabled", true),
            speaker: self.flag("SpeakerConnectionRemindersEnabled", true),
            gamepad: self.flag("GamepadConnectionRemindersEnabled", true),
            mouse: self.flag("MouseConnectionRemindersEnabled", true),
            keyboard: self.flag("KeyboardConnectionRemindersEnabled", true),
        };
        for event in self.devices.poll(&gates) {
            if !self.flag("PauseNotifications", false) {
                self.enqueue_device(&event);
            }
            self.record_history("device", &event.notice(), None);
        }
        if self.flag("SystemStateHintsEnabled", true) && !self.flag("PauseNotifications", false) {
            for hint in self.osd.poll(true, true) {
                self.show_osd(hint, false);
            }
        } else {
            self.osd.poll(false, false);
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last_tick);
        self.last_tick = now;
        self.advance_timers(Instant::now());
        if self.last_context_poll.elapsed()>=Duration::from_secs(1){
            self.last_context_poll=Instant::now();
            if !self.demo {
                let sample = self.screen_time.poll(self.last_context_poll,awake_ms(),self.session_locked);
                let protected = self.reminder_plugins.enabled() && taskbar::blocks_wellness_interrupt();
                self.tick_reminder_plugins(sample, protected);
            }
            self.widget_metrics.poll();
            let mut placements=self.desktop_widgets.items.clone();
            placements.extend(self.overview_widgets.items.iter().cloned());
            let city=self.weather_city();
            if !city.is_empty(){
                placements.push(dotisle_domain::desktop_widgets::Placement{id:"island-weather".into(),kind:dotisle_domain::desktop_widgets::Kind::Weather,
                    size:Default::default(),style:Default::default(),config:dotisle_domain::widget_config::Config{city,..Default::default()},x:0,y:0,width:0.,height:0.,hidden:false,revision:1,material:String::new(),opacity:1.0,stack_id:String::new(),stack_order:0});
            }
            self.widget_data.poll(&placements);
            let m=&self.widget_metrics.value;
            let device_alerts=self.device_watch.update(&m.wireless_devices,m.wireless_updated_at,m.wireless_status=="ready");
            if self.alerts.refresh_devices(&self.device_watch){self.dirty=true;}
            if gates.master&&!self.flag("PauseNotifications",false){
                for alert in device_alerts{
                    let allowed=match alert.icon.as_str(){"Headphones"=>gates.headphones,"Speaker"=>gates.speaker,"Mouse"=>gates.mouse,"Keyboard"=>gates.keyboard,"Gamepad"=>gates.gamepad,_=>true};
                    if allowed && self.wake_for_transient_notice(){self.alerts.enqueue(alert);}
                }
            }
            let epoch=std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() as i64;
            if self.flag("WeatherAlertsEnabled",true)&&!self.flag("PauseNotifications",false){
                for alert in self.weather_watch.poll(&self.widget_data.value["island-weather"],epoch){if self.wake_for_transient_notice(){self.alerts.enqueue(alert);}}
            }
        }
        if self.greeting_pending{
            let waiting=!self.weather_city().is_empty()&&self.widget_data.value["island-weather"]["temperature"].is_null()
                &&self.start.elapsed()<Duration::from_secs(8);
            if !waiting{self.greeting_pending=false;self.show_greeting();}
        }
        if self.last_checkpoint.elapsed() >= Duration::from_secs(15) {
            if self.timers.items.iter().any(|t| t.running) {
                let _ = self.timers.persist();
            }
            self.last_checkpoint = Instant::now();
        }
        if self.last_history_prune.elapsed() >= Duration::from_secs(3600) {
            self.last_history_prune = Instant::now();
            self.history.prune(
                self.settings["NotificationHistoryDays"]
                    .as_u64()
                    .unwrap_or(7)
                    .clamp(1, 30) as u32,
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs() as i64,
            );
        }
        if let Some((version, result)) = self.cover_rx.as_ref().and_then(|rx| rx.try_recv().ok()) {
            self.cover_rx = None;
            if version == self.cover_version {
                match result {
                    Ok(prepared) => {
                        if let Some(r) = &mut self.renderer {
                            r.apply_cover(prepared);
                        } else {
                            self.pending_cover = Some(prepared);
                        }
                        self.cover_start = Instant::now();
                        self.sync_bottom_material();
                        self.dirty = true;
                    }
                    Err(e) => self.error = Some(e),
                }
            }
        }
        let old_panel = self.panel.clone();
        self.refresh_components();
        if self.panel != old_panel {
            self.dirty = true;
        }
        // 待办滚轮平滑阻尼追随与边界约束
        if self.selected == "todos" {
            let count = self.panel.as_ref().map(|p| p.tasks.len()).unwrap_or(0);
            let max = layout::todo_max_scroll(self.layout.card.h, count);
            self.todo_scroll_target = self.todo_scroll_target.clamp(0., max);
            if (self.todo_scroll_target - self.todo_scroll).abs() > 0.15 {
                let alpha = 1. - (-18. * dt.as_secs_f64().clamp(0., 0.1)).exp();
                self.todo_scroll += (self.todo_scroll_target - self.todo_scroll) * alpha;
                self.dirty = true;
            } else if self.todo_scroll != self.todo_scroll_target {
                self.todo_scroll = self.todo_scroll_target;
                self.dirty = true;
            }
        } else if self.todo_scroll != 0. || self.todo_scroll_target != 0. {
            self.todo_scroll = 0.;
            self.todo_scroll_target = 0.;
        }
        let glow_target = self.glow_target();
        let animate = self.flag("Animation", true);
        if self.glow.tick(dt.as_secs_f64(), glow_target, animate) {
            self.dirty = true;
        }
        if self.switching && self.slide.tick(dt.as_secs_f64()) {
            self.dirty = true;
        }
        if self.slide.value == 0. {
            self.switching = false;
        }

        let events = self
            .notification_listener
            .as_ref()
            .map(|l| l.events.try_iter().collect::<Vec<_>>())
            .unwrap_or_default();
        for event in events {
            match event {
                notifications::Event::MessageHint(hint) => {
                    let policy=notifications::Policy::from_settings(&self.settings);
                    // 消息提示独立于通知总开关，仅在用户暂停提醒时静音。
                    if !policy.paused&&policy.permits(&hint.app_id,&hint.app_name)&&!self.session_locked&&self.wake_for_transient_notice(){
                        self.alerts.enqueue(capsule_alert::Alert::message(&hint.app_id,&hint.app_name,hint.icon.as_deref()));
                    }
                }
                notifications::Event::State(state) => {
                    self.notification_state = state;
                }
                // Banners are queued rather than shown directly: showing each one on
                // arrival would make a burst of messages overwrite the previous one.
                notifications::Event::Notice(notice) => {
                    // Ignore content from a stale pre-5.2.1 notification module.
                    if notice.app_id.eq_ignore_ascii_case("WeChat") { continue }
                    let policy = notifications::Policy::from_settings(&self.settings);
                    if policy.should_show() && policy.permits(&notice.app_id, &notice.app_name) {
                        let code = if self.flag("VerificationCodeEnabled", true) {
                            verification_code::extract_from_notice(
                                &notice.app_name,
                                &notice.title,
                                &notice.body,
                            )
                        } else {
                            None
                        };
                        if let Some(code)=code{
                            if !self.session_locked{
                                let alert=capsule_alert::Alert::verification(&notice.app_id,&notice.app_name,code);
                                if !self.taskbar_mode()&&self.alerts.visible(){
                                    let (base,rad)=self.shape();
                                    let now=self.start.elapsed().as_secs_f64();
                                    if self.alerts.preempt_or_enqueue(alert,base,rad,self.layout.host.w,self.layout.capsule.y,now,self.flag("Animation",true)){
                                        self.hide_at=None;self.fade(1.);self.dirty=true;
                                    }
                                }else if self.wake_for_transient_notice(){
                                    self.alerts.enqueue(alert);
                                }
                            }
                        }else{
                            // 手动隐藏期间也正常入队新通知，弹窗展示后再自动退回隐藏态
                            if !self.flag("PauseNotifications", false) {
                                self.enqueue(Queued{notice:notice.clone(),todo:None,chime:true});
                            }
                            self.record_history("notice", &notice, None);
                        }
                    }
                }
            }
        }
        if self.flag("PauseNotifications", false) {
            if self.banner.visible() {
                let (r, rad) = self.shape();
                self.banner.dismiss(
                    r,
                    rad,
                    self.start.elapsed().as_secs_f64(),
                    self.flag("Animation", true),
                    self.base_needed(),
                );
            }
            if self.selected == "update"
                && self.opacity > 0.001
                && self
                    .panel
                    .as_ref()
                    .is_some_and(|p| p.running && p.progress < 0.)
                && self.flag("Animation", true)
            {
                self.dirty = true;
            }
            if self.osd_banner.visible() {
                let (r, rad) = self.shape();
                self.osd_banner.dismiss(
                    r,
                    rad,
                    self.start.elapsed().as_secs_f64(),
                    self.flag("Animation", true),
                    self.base_needed(),
                );
            }
            self.bottom_osd.clear();
            self.top_popup.clear();
            // Nothing may be presented or chimed while paused. Reminders stay
            // unmarked, so they are re-checked once notifications resume.
            self.queue.retain(|e| e.todo.is_some());
        }
        // Snapshot cadence follows activity: a playing track needs the 90 ms refresh
        // for smooth lyric hand-offs; a paused one only waits for control changes, and
        // an idle player only waits for a track to appear at all.
        let poll_interval = if self.snapshot["isPlaying"] == true {
            Duration::from_millis(90)
        } else if self.snapshot["track"].is_object() {
            Duration::from_millis(250)
        } else {
            Duration::from_millis(400)
        };
        if self.last_poll.elapsed() >= poll_interval {
            self.poll()
        }
        if let Some(t) = self.transition {
            self.sampled_ms = self.transition_start.elapsed().as_secs_f64() * 1000.;
            self.frame = t.sample(self.sampled_ms);
            self.dirty = true;
            if self.sampled_ms >= t.duration_ms {
                self.transition = None;
                self.frame = t.end;
            }
        }
        if self.custom_from.is_some() {
            self.dirty = true;
            if self.custom_start.elapsed().as_secs_f64() >= 0.23 {
                self.custom_from = None;
            }
        }
        let target = self.music_target();
        if (self.music_mix - target).abs() > 0.0001 {
            self.music_mix = if self.flag("Animation", true) {
                lyric_core::music_layout::music_layout_mix(
                    self.music_from,
                    target,
                    self.music_start.elapsed().as_secs_f64(),
                )
            } else {
                target
            };
            self.dirty = true;
        }
        let target_height = self.card_height();
        if target_height+72.>self.layout.host.h {self.layout.host.h=target_height+72.;unsafe{let _=self.position();}}
        if (self.layout.card.h - target_height).abs() > 0.01 {
            let weight = if self.selected == "music" && self.panel.is_none() {
                1.
            } else if self.flag("Animation", true) {
                1. - (-dt.as_secs_f64() * 26.).exp()
            } else {
                1.
            };
            self.layout.card.h += (target_height - self.layout.card.h) * weight;
            if (self.layout.card.h - target_height).abs() < 0.1 {
                self.layout.card.h = target_height;
            }
            self.dirty = true;
        }
        if !self.banner.visible() && self.hide_at.is_some_and(|t| Instant::now() >= t) {
            self.hide_at = None;
            if !self.base_needed() { self.fade(0.); }
        }
        if (self.opacity - self.fade_to).abs() > 0.00001 {
            let appearing = self.fade_to > self.fade_from;
            let dur = if appearing { 0.22 } else { 0.17 };
            let t = if self.flag("Animation", true) {
                (self.fade_start.elapsed().as_secs_f64() / dur).min(1.)
            } else {
                1.
            };
            // Apple 非对称显隐曲线：淡入使用 Quintic 四次缓出迅速聚焦，淡出平稳收束
            let eased = if appearing {
                1. - (1. - t).powi(4)
            } else {
                motion::cubic_out(t)
            };
            self.opacity = self.fade_from + (self.fade_to - self.fade_from) * eased;
            self.dirty = true;
        }
        let mut pt = POINT::default();
        let interactive_surface = self.opacity >= 0.01;
        let edge_watch = !self.flag("TaskbarModeEnabled", false)
            && self.flag("EdgeHoverRevealEnabled", false)
            && (self.user_hidden || self.edge_revealed);
        if interactive_surface || self.dragging.is_some() || edge_watch {
            let _ = GetCursorPos(&mut pt);
        }
        let hit = if interactive_surface {
            let mut local = pt;
            let _ = ScreenToClient(self.hwnd, &mut local);
            let scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
            self.hit(local.x as f64 / scale,local.y as f64 / scale)
        } else { -1 };
        let inside = hit >= 0;
        if edge_watch && !self.demo && self.practice.is_none() {
            let monitor = self.target_monitor();
            let mut info = MONITORINFO {
                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                ..Default::default()
            };
            let _ = GetMonitorInfoW(monitor, &mut info);
            let scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
            let cx_frac = if self.flag("Floating", false) {
                self.settings["CenterX"].as_f64().unwrap_or(0.5)
            } else {
                0.5
            };
            let in_zone = edge_hit_test(pt, info.rcMonitor, self.layout.capsule.w, cx_frac, scale);
            self.step_edge_reveal(in_zone, inside, Instant::now());
        } else {
            self.edge_hover_since = None;
            self.edge_leave_since = None;
        }
        if inside {
            if let Some(t) = self.manual_until.as_mut() {
                *t = Instant::now() + Duration::from_secs(6);
            }
        }
        self.track_pointer_presence(inside);
        if self.swipe.is_none()
            && !self.banner.visible()
            && self
                .leave_at
                .is_some_and(|t| t.elapsed() >= Duration::from_millis(1200))
        {
            self.leave_at = None;
            if !self.held && (self.practice.is_none() || self.practice_step() == Some(PracticeStep::HoverCollapse)) {
                let was_expanded = self.expanded;
                self.retarget(false);
                if was_expanded && !self.expanded { self.practice_event(PracticeStep::HoverCollapse); }
            }
        }
        let reduce = !self.flag("Animation", true);
        self.music_controls.target = if self.expanded
            && self.selected == "music"
            && (inside || self.seeking || self.pressed > 0)
        {
            1.
        } else {
            0.
        };
        if self.music_controls.tick(dt.as_secs_f64(), reduce) {
            self.dirty = true;
        }
        self.feedback_moving = false;
        for (i, state) in self.feedback.iter_mut().enumerate() {
            state.target = if self.pressed > 0 && self.pressed == i as i32 {
                0.12
            } else if self.hover > 0 && self.hover == i as i32 {
                0.08
            } else {
                0.
            };
            self.feedback_moving |= state.tick(dt.as_secs_f64(), reduce);
        }
        self.play_shape.target = if self.snapshot["isPlaying"] == true {
            1.
        } else {
            0.
        };
        self.feedback_moving |= self.play_shape.tick(dt.as_secs_f64(), reduce);
        if self.feedback_moving {
            self.dirty = true;
        }
        if self.settings["IslandColorSource"] == "System" {
            let accent = theme_accent::system_accent();
            if self.last_dynamic_system != accent {
                self.last_dynamic_system = accent;
                self.dirty = true;
            }
        }
        if self.hover != hit {
            self.hover = hit;
            self.hover_since = Instant::now();
            self.dirty = true;
        }
        if hit == volume::BUTTON {let old=(self.volume.level,self.volume.muted);self.volume.refresh(false);if old!=(self.volume.level,self.volume.muted){self.dirty=true}} else {self.volume.leave();}
        let tooltip = (hit == 6 || hit == volume::BUTTON || hit == 51 || hit == layout::ACTIVITY_CLOSE) && self.hover_since.elapsed() >= Duration::from_millis(450);
        if self.tooltip_visible != tooltip {
            self.tooltip_visible = tooltip;
            self.dirty = true;
        }
        let left = GetAsyncKeyState(VK_LBUTTON.0 as i32) < 0;
        if self.practice.is_none() && !self.banner.visible() && left && !self.last_left && !inside && self.expanded {
            self.held = false;
            self.retarget(false)
        }
        self.last_left = left;
        // A new lyric or a cover flip deserves the panel rate. Steady playback only moves
        // the progress bar and the activity bars, so it is capped — a 144 Hz panel used to
        // buy 144 full Vello scene rebuilds a second for motion nobody can resolve.
        let arriving = self.lyric_start.elapsed() < Duration::from_millis(220)
            || (self.expanded
                && self.flag("Animation", true)
                && self.cover_start.elapsed() < Duration::from_millis(300));
        if self.opacity > 0.001 && arriving {
            self.dirty = true;
        } else if self.opacity > 0.001
            && (self.snapshot["isPlaying"] == true
                || self.auxiliary_state().0
                || self.panel.as_ref().is_some_and(|p| p.running)
                || self.cover_start.elapsed() < Duration::from_millis(850)
                || self
                    .renderer
                    .as_ref()
                    .is_some_and(|r| r.colors_moving(self.start.elapsed().as_secs_f64())))
            && self.flag("Animation", true)
            && self.continuous_last.elapsed() >= frame_pacing::continuous_interval()
        {
            self.dirty = true;
            self.continuous_last = Instant::now();
        }
        // A message waiting in the queue takes over the surface it already has. Closing
        // first would morph the card back to the capsule and out again, which reads as the
        // island growing and shrinking on its own.
        if !self.fullscreen_suppressed && self.banner.visible()
            && !self.queue.is_empty()
            && (!self.user_hidden || self.restore_hidden)
            && !self.flag("PauseNotifications", false)
            && self.banner.due(self.start.elapsed().as_secs_f64())
        {
            if let Some(q) = self.queue.pop_front() {
                self.show_notice(q.notice, false, q.todo, q.chime);
            }
        }
        if self.banner.visible() {
            let (r, rad) = self.shape();
            self.banner.tick(
                r,
                rad,
                self.start.elapsed().as_secs_f64(),
                inside,
                self.flag("Animation", true),
                self.base_needed(),
            );
            self.dirty = true;
            if !self.banner.visible() {
                self.banner_todo = None;
                self.last_poll = Instant::now() - Duration::from_secs(1);
                self.finish_transient();
            }
        }
        if self.practice.is_none() && self.alerts.pending() && (self.user_hidden || self.flag("PauseNotifications", false)) {
            for kind in reminder_plugins::Kind::ALL { self.alerts.remove_pending(kind.alert_key()); }
        }
        let about_to_present_wellness = !self.alerts.visible() && !self.banner.visible() && !self.osd_banner.visible()
            && !self.top_popup.visible() && !self.island_suppressed() && self.alerts.next_wellness();
        if !self.demo && self.alerts.has_wellness() && (about_to_present_wellness || self.last_wellness_guard_check.elapsed() >= Duration::from_millis(150)) {
            self.last_wellness_guard_check = Instant::now();
            if taskbar::blocks_wellness_interrupt() {
                self.suppress_wellness_interrupt();
            }
        }
        if self.alerts.visible(){
            let(r,rad)=self.shape();
            if self.flag("PauseNotifications",false)||self.user_hidden||self.session_locked{
                self.alerts.clear(r,rad,self.start.elapsed().as_secs_f64(),self.flag("Animation",true));
            }
            self.alerts.tick(r,rad,self.start.elapsed().as_secs_f64(),self.flag("Animation",true));self.dirty=true;
            if !self.alerts.visible(){self.finish_transient();}
        }
        if self.taskbar_mode()
            && !self.island_suppressed()
            && !self.alerts.visible()
            && !self.alerts.next_island_only()
            && !self.top_popup.visible()
            && !self.user_hidden
            && !self.session_locked
            && !self.flag("PauseNotifications", false)
        {
            if let Some(alert) = self.alerts.pop_next() {
                let font = self.base.join("Assets/Fonts/NotoSansSC.ttf");
                let animate = self.flag("Animation", true);
                let theme = self.hint_theme();
                let monitor = self.target_monitor();
                if let Err(e) = unsafe {
                    self.top_popup.show_alert(alert, monitor, &font, animate, theme)
                } {
                    self.error = Some(e.to_string());
                }
            }
        } else if self.practice.is_none() && !self.island_suppressed() && !self.top_popup.visible()
            && (!self.taskbar_mode() || self.alerts.next_island_only())
            && !self.banner.visible()&&!self.osd_banner.visible()&&!self.alerts.visible()
            &&!self.user_hidden&&!self.session_locked&&!self.flag("PauseNotifications",false){
            let(r,rad)=self.shape();
            if self.alerts.pending(){self.ensure_renderer()?;}
            if self.alerts.begin_at(r,rad,self.layout.host,self.layout.capsule.y,self.start.elapsed().as_secs_f64(),self.flag("Animation",true),self.layout.anchor_bottom){
                self.hide_at=None;self.fade(1.);self.dirty=true;
            }
        }
        if self.osd_banner.visible() {
            let (r, rad) = self.shape();
            self.osd_banner.tick(
                r,
                rad,
                self.start.elapsed().as_secs_f64(),
                false,
                self.flag("Animation", true),
                self.base_needed(),
            );
            self.dirty = true;
            if !self.osd_banner.visible() {
                self.finish_transient();
            }
        }
        // Collapsed capsule: retarget only when an input actually changed, then
        // advance the spring. Measurement is cached inside the renderer.
        let compact_key = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
            self.selected,
            format!("{}|{}", self.lyric, self.settings["IslandFontFamilyRender"]),
            self.compact_secondary,
            self.settings["FontSize"],
            self.settings["CompactWidth"],
            self.flag("AutoCompactWidth", false),
            self.settings["AutoCompactMaxWidth"],
            self.flag("Animation", true),
            self.mini(),
            self.renderer.as_ref().map(|r| r.dpi).unwrap_or(96.)
        );
        let auto = self.flag("AutoCompactWidth", false);
        let has_lyric = self.snapshot["track"].is_object()
            && self.snapshot["frame"]["currentLine"]
                .as_str()
                .is_some_and(|s| !s.trim().is_empty());
        // Keep the last measured geometry through a transient empty player/lyric frame.
        // Content itself is still updated immediately by poll().
        if compact_key == self.compact_key || auto && self.selected == "music" && !has_lyric {
            self.compact_stability.cancel();
        } else {
            match self.measure_compact_layout() {
                Ok(size) => {
                    // Never pump. Growth needs a short confirmation so one bad snapshot
                    // (a seek flashing a long line) cannot widen the capsule; a foreign line
                    // whose translation has not arrived yet gets a much longer contraction
                    // window, because shrinking now means growing back a beat later.
                    let translated = self.snapshot["frame"]["currentTranslation"]
                        .as_str()
                        .is_some_and(|t| !t.trim().is_empty());
                    let line = self.snapshot["frame"]["currentLine"].as_str().unwrap_or("");
                    let expects = self.flag("CompactTranslation", true)
                        && !self.mini()
                        && !translated
                        && line.chars().any(
                            |c| matches!(c as u32,0x3040..=0x30ff|0xac00..=0xd7af|0x0000..=0x024f),
                        );
                    let shrink = if expects { 0.18 } else { 0.10 };
                    let force = self.compact_key.is_empty() || !auto || self.selected != "music";
                    if self.compact_stability.ready_within(
                        &compact_key,
                        size,
                        (
                            self.compact_size.width_target,
                            self.compact_size.height_target,
                        ),
                        self.start.elapsed().as_secs_f64(),
                        force,
                        shrink,
                        0.04,
                    ) {
                        self.compact_key = compact_key;
                        self.apply_compact_target(size.0, size.1);
                    }
                }
                Err(e) => self.error = Some(e.to_string()),
            }
        }
        let dt = self.last_compact.elapsed().as_secs_f64();
        self.last_compact = Instant::now();
        if self.compact_size.moving() && self.compact_size.tick(dt, self.options().duration) {
            self.layout
                .set_compact(self.compact_size.width, self.compact_size.height);
            self.dirty = true;
        }
        if self.island_morph.active() {
            let now = self.start.elapsed().as_secs_f64();
            match self.island_morph.mode() {
                morph_transition::IslandMode::Strip => {
                    self.island_morph.update_target(self.hidden_edge_rect(), 2.5, now);
                }
                morph_transition::IslandMode::Normal | morph_transition::IslandMode::Mini => {
                    let target_rect = self.layout.capsule;
                    let target_rad = self.capsule_corner_radius(target_rect.h);
                    self.island_morph.update_target(target_rect, target_rad, now);
                }
                morph_transition::IslandMode::Card => {
                    self.island_morph
                        .update_target(self.layout.card, self.options().radius, now);
                }
            }
            let _ = self.island_morph.tick(now);
            let _ = unsafe { self.sync_input_region() };
            self.dirty = true;
        }
        // Reminders are checked once a second, not per rendered frame.
        if !self.demo
            && self.flag("TodoRemindersEnabled", true)
            && !self.flag("PauseNotifications", false)
            && self.last_schedule.elapsed() >= Duration::from_secs(1)
        {
            self.last_schedule = Instant::now();
            self.todo_now = todos::Now::local();
            match self.todos.poll(&self.todo_now) {
                Ok(due) => {
                    for t in due {
                        self.enqueue_todo(&t);
                    }
                }
                Err(e) => {
                    self.error = Some(e);
                }
            }
        }
        if !self.demo {
            self.poll_app_rules();
        }
        // One banner at a time; the next one starts once the previous is gone.
        if !self.fullscreen_suppressed && self.practice.is_none() && !self.banner.visible() && !self.osd_banner.visible() && !self.alerts.visible() && !self.alerts.pending() && !self.top_popup.visible() && !self.flag("PauseNotifications", false) {
            if !self.queue.is_empty() && (self.flag("TaskbarModeEnabled", false) || self.wake_for_transient_notice()) {
                if let Some(q) = self.queue.pop_front() {
                    self.show_notice(q.notice, false, q.todo, q.chime);
                }
            }
        }
        if let Some((start, bounds)) = self.dragging {
            if left {
                let dx = pt.x - start.x;
                let dy = pt.y - start.y;
                for hwnd in [self.visual_hwnd, self.hwnd] {
                    SetWindowPos(
                        hwnd,
                        None,
                        bounds.left + dx,
                        bounds.top + dy,
                        0,
                        0,
                        SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
                    )?;
                }
            } else {
                self.dragging = None;
                let _ = ReleaseCapture();
                if self.practice.is_some() {
                    let mut end = RECT::default();
                    if GetWindowRect(self.hwnd, &mut end).is_ok()
                        && (end.left.abs_diff(bounds.left) >= 12 || end.top.abs_diff(bounds.top) >= 12) {
                        if self.practice_step() == Some(PracticeStep::Move) && !inside { self.leave_at = Some(Instant::now()); }
                        self.practice_event(PracticeStep::Move);
                    }
                } else {
                    self.remember_position();
                }
            }
        }
        self.sync_input_region()?;
        // 仅当选用液态玻璃 Beta + 实时画面源且灵动岛可见非蓝条时，保持 30fps 轻量捕获刷新
        if !self.dirty
            && self.opacity > 0.001
            && self.uses_liquid_glass_material()
            && self.liquid_glass_source() == backdrop_capture::BackdropSource::Live
            && !(self.hidden_edge() && !self.island_morph.active())
        {
            self.dirty = true;
        }
        if self.dirty {
            if self.opacity > 0.001 || self.fade_from > 0.001 {
                self.draw()?;
                if self.opacity <= 0.001 {
                    self.fade_from = 0.;
                }
            } else {
                self.dirty = false;
            }
        }
        Ok(())
    }
    unsafe fn request(&mut self, r: Value) -> Value {
        if r["version"] != 1 {
            return json!({"success":false,"message":"请求版本无效"});
        }
        let command = r["command"].as_str().unwrap_or("");
        if matches!(
            command,
            "state" | "timerCreate" | "timerAction" | "focusResume"
        ) {
            self.advance_timers(Instant::now());
        }
        match command {
            "moduleRefresh"=>{self.module_updates.refresh();json!({"success":true})},
            "moduleStage"=>match self.module_updates.stage(r["value"]["id"].as_str().unwrap_or(""),r["value"]["path"].as_str().unwrap_or("")){Ok(())=>json!({"success":true,"message":"正在校验模块"}),Err(e)=>json!({"success":false,"message":e})},
            "moduleRollback"=>match self.module_updates.rollback(r["value"]["id"].as_str().unwrap_or("")){Ok(())=>json!({"success":true,"message":"正在校验备份"}),Err(e)=>json!({"success":false,"message":e})},
            "moduleCancel"=>match self.module_updates.cancel(){Ok(())=>json!({"success":true}),Err(e)=>json!({"success":false,"message":e})},
            "moduleApply"=>match self.module_updates.apply(r["value"]["callerSettingsPid"].as_u64().unwrap_or(0)as u32){Ok(())=>json!({"success":true}),Err(e)=>json!({"success":false,"message":e})},
            "media" => {
                let name = r["value"]["action"].as_str().unwrap_or("");
                if !["toggle", "previous", "next", "seek"].contains(&name) {
                    return json!({"success":false,"message":"无效的播放操作"});
                }
                let seconds = r["value"]["seconds"].as_f64().unwrap_or(if name == "seek" {
                    f64::NAN
                } else {
                    0.
                });
                if !seconds.is_finite() || seconds < 0. {
                    return json!({"success":false,"message":"播放进度无效"});
                }
                if self.demo {
                    if name == "seek" {
                        self.snapshot["positionSeconds"] = json!(seconds);
                    }
                    return json!({"success":true});
                }
                match self
                    .backend
                    .as_ref()
                    .ok_or_else(|| "播放服务尚未就绪".to_string())
                    .and_then(|backend| backend.enqueue(name, seconds))
                {
                    Ok(id) => json!({"success":true,"commandId":id,"pending":true}),
                    Err(message) => json!({"success":false,"message":message}),
                }
            }
            "todoRestore" => match self.todos.restore(&r["value"]) {
                Ok(()) => json!({"success":true,"message":"已恢复待办"}),
                Err(e) => json!({"success":false,"message":e}),
            },
            "testPointerSeek"
                if std::env::var("LYRICISLAND_TEST_PIPE")
                    .is_ok_and(|p| p.starts_with("LyricIsland.Settings.test.")) =>
            {
                let fraction = r["value"]["fraction"].as_f64().unwrap_or(0.5).clamp(0., 1.);
                let (shape, _) = self.shape();
                let scale = (shape.w / self.layout.card.w).min(shape.h / self.layout.card.h);
                let g = self.music_geometry();
                if g.progress_hit.w <= 0. {
                    return json!({"success":false,"message":"当前布局未显示播放进度"});
                }
                let dpi = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
                let x = ((shape.x
                    + (shape.w - self.layout.card.w * scale) / 2.
                    + (g.track.x + g.track.w * fraction) * scale)
                    * dpi)
                    .round() as u16;
                let y = ((shape.y
                    + (shape.h - self.layout.card.h * scale) / 2.
                    + (g.progress_hit.y + g.progress_hit.h / 2.) * scale)
                    * dpi)
                    .round() as u16;
                let pos = LPARAM(((y as u32) << 16 | x as u32) as isize);
                let down = PostMessageW(Some(self.hwnd), WM_LBUTTONDOWN, WPARAM(1), pos);
                let up = PostMessageW(Some(self.hwnd), WM_LBUTTONUP, WPARAM(0), pos);
                json!({"success":down.is_ok()&&up.is_ok()})
            }
            "testPointerClick"
                if std::env::var("LYRICISLAND_TEST_PIPE")
                    .is_ok_and(|p| p.starts_with("LyricIsland.Settings.test.")) =>
            {
                let (shape, _) = self.shape();
                let scale = GetDpiForWindow(self.hwnd).max(96) as f64 / 96.;
                let x_fraction = r["value"]["x"].as_f64().unwrap_or(0.5).clamp(0., 1.);
                let y_fraction = r["value"]["y"].as_f64().unwrap_or(0.25).clamp(0., 1.);
                let x = ((shape.x + shape.w * x_fraction) * scale).round() as u16;
                let y = ((shape.y + shape.h * y_fraction) * scale).round() as u16;
                let pos = LPARAM(((y as u32) << 16 | x as u32) as isize);
                let down = PostMessageW(Some(self.hwnd), WM_LBUTTONDOWN, WPARAM(1), pos);
                let up = PostMessageW(Some(self.hwnd), WM_LBUTTONUP, WPARAM(0), pos);
                json!({"success":down.is_ok()&&up.is_ok()})
            }
            "testCompactFrame"
                if self.demo
                    && std::env::var("LYRICISLAND_TEST_PIPE")
                        .is_ok_and(|p| p.starts_with("LyricIsland.Settings.test.")) =>
            {
                let line = r["value"]["line"].as_str().unwrap_or("");
                let translation = r["value"]["translation"].as_str().unwrap_or("");
                if line.chars().count() > 512 || translation.chars().count() > 512 {
                    return json!({"success":false,"message":"测试歌词过长"});
                }
                self.snapshot["track"] = json!({"id":"compact-regression","title":"几何回归","artist":"DotIsle","durationSeconds":180.});
                self.snapshot["isPlaying"] = json!(true);
                self.snapshot["positionSeconds"] =
                    json!(r["value"]["position"].as_f64().unwrap_or(10.));
                self.snapshot["frame"] = json!({"currentLine":line,"currentTranslation":translation,"nextLine":"","currentLineIndex":r["value"]["index"].as_i64().unwrap_or(1),"wordScanProgress":0.5});
                self.poll();
                self.dirty = true;
                json!({"success":true})
            }
            "state" => {
                // Refresh the music snapshot so the desktop widget receives the lyric frame
                // of this instant instead of the GUI loop's stale one (same-thread as poll).
                self.poll();

                self.todo_now = todos::Now::local();
                let t = &self.snapshot["track"];
                let mut module_versions=dotisle_module_api::versions();module_versions["core"]=json!(DISPLAY_VERSION);
                let mut state = json!({"success":true,"version":DISPLAY_VERSION,"bundleVersion":lyric_backend::updates::installed_version(),"settings":self.settings,"moduleManagement":self.module_updates.state,"moduleVersions":module_versions,"moduleErrors":dotisle_module_api::initialization_errors(),"moduleReady":dotisle_module_api::initialization_errors().as_object().is_some_and(|m|m.is_empty()),"softwareUpdate":self.updates.state,"updateChannel":self.updates.channel(),"updateSource":self.updates.source(),"settingsDark":match self.settings["SettingsTheme"].as_str().unwrap_or("System"){"Dark"=>true,"Light"=>false,_=>theme_accent::system_dark()},"dataDirectory":self.data.to_string_lossy(),"musicPreview":self.music_preview(),"activeSource":self.snapshot["activeSource"],"nativeStatus":self.snapshot["nativeStatus"],"title":t["title"],"artist":t["artist"],"songId":t["songId"],"player":t["sourceApp"],"position":self.snapshot["positionSeconds"],"duration":t["durationSeconds"],"timeline":self.snapshot["clockStatus"],"lyricSource":self.snapshot["lyricSource"],"isPlaying":self.snapshot["isPlaying"],"canSeek":self.snapshot["canSeek"],"seekStatus":self.snapshot["seekStatus"],"neteaseAttention":lyric_backend::netease_control::attention_diagnostics(),"mediaCommand":self.snapshot["mediaCommand"],"expanded":self.expanded,"islandEnabled":!self.user_hidden,"islandVisible":self.opacity>0.001&&!self.island_suppressed(),"processId":std::process::id(),"windowLevel":unsafe{window_level::snapshot(self.visual_hwnd,self.hwnd)},"rendererGeneration":self.renderer_generation,"renderFrames":self.render_frames,"renderer":"Vello Hybrid 0.2 / wgpu 29","lastError":self.error,"notificationState":self.notification_state,"notificationVisible":self.banner.visible()||self.top_popup.notice_visible(),"notificationGeometry":self.notification_geometry(),"osdVisible":self.osd_banner.visible()||self.bottom_osd.visible()||self.top_popup.osd_visible(),"bottomOsdVisible":self.bottom_osd.visible(),"systemState":self.osd.status(),"todos":self.todos.list(&self.todo_now),"todoError":self.todos.error,"today":self.todo_now.date,"now":self.todo_now.time,"pendingPage":self.pending_page,"pendingRequestId":self.pending_request,"pendingCenterAction":self.pending_center_action,"centerFrontend":if self.base.join("LyricIsland.Center.exe").is_file(){"go"}else{"missing"},"islandGeometry":{"mini":self.mini(),"capsule":self.layout.capsule,"card":self.layout.card,"satellite":!self.mini()&&self.components.len()>1,"horizontalGestures":self.flag("SwipeActivities",true)},"compactTargetWidth":self.compact_size.width_target,"compactWidth":self.compact_size.width,"compactHeight":self.compact_size.height,"compactSecondary":self.compact_secondary,"timers":self.timers.list(),"timerError":self.timers.error,"screenTime":self.screen_time.snapshot(),"weather":self.widget_data.value["island-weather"],"capsuleAlertVisible":self.alerts.visible()||self.top_popup.alert_visible(),"widgetData":self.widget_data.value,"desktopWidgets":self.desktop_widgets.list(),"overviewWidgets":self.overview_widgets.list(),"desktopWidgetError":self.desktop_widgets.error,"systemMetrics":self.widget_metrics.value,"activeComponent":self.selected,"components":self.components,"notificationHistory":self.history.list(),"historyError":self.history.error,"quickHotkey":self.settings["QuickReminderHotkey"],"pendingAction":self.pending_action,"sessionLocked":self.session_locked,"focusResume":self.focus_resume.as_ref().map(|r|json!({"generation":r.generation,"ids":r.ids,"prompted":r.prompted})),"templates":self.templates.list(),"workflowTemplateError":self.templates.error,"appRules":self.rules.list(&self.todo_now.date),"appRuleError":self.rules.error,"foreground":{"path":self.foreground.path,"name":self.foreground.name,"unreadable":self.foreground.unreadable},"lockWatch":true});
                state["notificationContent"]=self.banner.notice.as_ref().map(|n|json!({"appId":n.app_id,"titleCharacters":n.title.chars().count(),"bodyCharacters":n.body.chars().count(),"iconBytes":n.icon.as_ref().map_or(0,Vec::len)})).unwrap_or(Value::Null);
                if std::env::var("LYRICISLAND_TEST_PIPE").is_ok_and(|p| p.starts_with("LyricIsland.Settings.test.")) {
                    state["testMessageAlert"]=json!(self.alerts.active.as_ref().filter(|a|a.key.starts_with("message:")).map(|a|json!({"title":a.title,"detail":a.detail,"trailing":a.trailing})));
                }
                state["islandWindowPresent"] = json!(self.opacity > 0.001);
                state["idleClickThrough"] = json!(self.input_region == Some([0; 10]));
                state["verificationAlertVisible"] = json!(self.alerts.visible() && self.alerts.active.as_ref().is_some_and(|a|a.code.is_some()));
                state["verificationCodeLength"] = json!(self.alerts.active.as_ref().and_then(|a|a.code.as_ref()).map(String::len));
                state["residentIdle"] = json!(self.resident_idle());
                if let Some(practice) = &self.practice { state["gesturePractice"] = practice.state(); }
                if self.selected == resident::ID {
                    let view = &self.resident.view;
                    state["residentContent"] = json!({"page":view.page.name(),"text":view.label,"battery":view.battery,
                        "expandedDeviceRows":view.devices.len(),"extraDevices":view.more_devices});
                }
                state["backgroundOpacity"] = json!(self.opacity_fraction());
                state["displays"] = displays::list_json();
                state["plugins"] = self.plugins.state.clone();
                state["taskbarSuppressed"] = json!(self.taskbar_suppressed);
                state["fullscreenSuppressed"] = json!(self.fullscreen_suppressed);
                state["reminderPlugins"] = self.reminder_plugins.state();
                let hydration = self.reminder_plugins.hydration(&self.settings["HydrationToday"], &self.todo_now.date);
                state["hydrationToday"] = json!({"drankMl":hydration.drank_ml});
                state["aiQuota"] = self.ai_quota.state(ai_quota::Settings::from_value(&self.settings["AiQuotaMonitor"]), &self.settings["AiQuotaSources"]);
                state["aiQuota"]["activeAgents"] = json!(self.agent.view.tiles.iter().map(|tile|tile.agent.name()).collect::<Vec<_>>());
                state["aiQuota"]["replyPending"] = json!(self.agent.view.prompt.is_some());
                state["reminderPluginVisible"] = json!(self.alerts.active.as_ref().and_then(|alert| alert.key.strip_prefix("reminder:")));
                if std::env::var("LYRICISLAND_TEST_PIPE").is_ok_and(|p| p.starts_with("LyricIsland.Settings.test."))
                    && self.alerts.banner.open {
                    let (rect, _, _) = self.alerts.banner.presentation(self.start.elapsed().as_secs_f64());
                    state["testAlertRect"] = json!(rect);
                    if let Some(alert) = &self.alerts.active {
                        state["testAlertKey"] = json!(alert.key);
                    }
                    if self.alerts.active.as_ref().is_some_and(|a| a.hydration.is_some()) {
                        if let Some(g) = capsule_alert::WaterLayout::in_card(rect) {
                            state["testWaterLayout"] = json!({"track":g.track,"record":g.record,"later":g.later});
                        }
                    }
                }
                state["rendererLoaded"] = json!(self.renderer.is_some());
                state["pluginError"] = json!(self.plugins.error);
                state["systemAccent"] = json!(theme_accent::system_accent());
                state
            }
            "updateTestPrepare"
                if std::env::var("LYRICISLAND_TEST_PIPE")
                    .is_ok_and(|p| p.starts_with("LyricIsland.Settings.test.")) =>
            {
                self.updates.nonce = "integration-test-nonce".into();
                self.updates.job = "integration-test-job".into();
                self.updates.action.clear();
                self.updates.release = json!({"newer":true,"version":DISPLAY_VERSION});
                self.updates.state = json!({"phase":"downloading","seq":0});
                self.refresh_components();
                json!({"success":true,"nonce":self.updates.nonce,"jobId":self.updates.job})
            }
            "showUpdate" => {
                self.update_card_deleted = false;
                self.refresh_components();
                if self.updates.selectable() {
                    self.select_component("update", 1);
                    self.held = true;
                    self.retarget(true);
                    self.user_hidden = false;
                    self.hidden_mini = false;
                    self.fade(1.);
                    json!({"success":true})
                } else {
                    json!({"success":false,"message":"当前没有更新任务，请先检查更新。"})
                }
            }
            "openInstaller" => {
                match std::process::Command::new(self.base.join("DotIsle.Setup.exe"))
                    .arg(format!("/MIGRATEDATA={}", self.data.display()))
                    .spawn()
                {
                    Ok(_) => json!({"success":true}),
                    Err(e) => json!({"success":false,"message":e.to_string()}),
                }
            }
            "gesturePractice" => self.practice_request(&r["value"]),
            "updateSource" => {
                match self.updates.set_source(r["value"].as_str().unwrap_or("")) {
                    Ok(()) => json!({"success":true,"message":"已切换更新源"}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "updateChannel" => {
                match self.updates.set_channel(r["value"].as_str().unwrap_or("")) {
                    Ok(()) => json!({"success":true,"message":"已切换更新渠道"}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "checkUpdate" => {
                self.updates.check(true);
                json!({"success":true,"message":"正在检查更新…"})
            }
            "playbackProgressPreview" => {
                let value = &r["value"];
                let width = value["width"].as_u64().unwrap_or(360).clamp(80, 1200) as u32;
                let bar = dotisle_render::progress::BarStyle::from_setting(value["bar"].as_str().unwrap_or("Normal"));
                let thumb = if value["thumbOn"].as_bool() == Some(true) {
                    dotisle_render::progress::ThumbStyle::from_setting(value["thumb"].as_str().unwrap_or("Pill"))
                } else { dotisle_render::progress::ThumbStyle::None };
                let ink = value["ink"].as_u64().unwrap_or(0x28486a) as u32 & 0xffffff;
                let rail = value["rail"].as_u64().unwrap_or(0xc5ceda) as u32 & 0xffffff;
                match self.ensure_renderer().and_then(|_| self.renderer.as_ref().unwrap().capture_progress_frame(
                    width, bar, thumb, value["glow"].as_bool() == Some(true), value["spin"].as_bool() == Some(true),
                    value["animate"].as_bool() != Some(false), ink, rail)) {
                    Ok(png) => json!({"success":true,"png":base64::engine::general_purpose::STANDARD.encode(png)}),
                    Err(error) => json!({"success":false,"message":error.to_string()}),
                }
            },
            "musicLayoutPreview" => match self.music_layout_preview(&r["value"]) {
                Ok(preview) => preview,
                Err(message) => json!({"success":false,"message":message}),
            },
            "updateReview" => match self.updates.review() {
                Ok(review) => review,
                Err(message) => json!({"success":false,"message":message}),
            },
            "neteaseEnableSilent" => {
                self.enable_netease_silent();
                json!({"success":true,"message":"已显示网易云自动连接状态。"})
            }
            "updateDownload" | "installUpdate" => {
                let pipe = std::env::var("LYRICISLAND_TEST_PIPE")
                    .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into());
                match self.updates.begin(&self.base, &self.data, &pipe, &r["value"]) {
                    Ok(()) => {
                        self.update_card_deleted = false;
                        if self.selected != "update" {
                            self.update_return = self.selected.clone();
                        }
                        self.refresh_components();
                        self.select_component("update", 1);
                        self.held = true;
                        self.retarget(true);
                        self.user_hidden = false;
                        self.hidden_mini = false;
                        self.fade(1.);
                        json!({"success":true,"message":"开始下载；校验后等待你确认安装。"})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "updateApply" => {
                if !self.updates.ask_apply() {
                    json!({"success":false,"message":"更新尚未就绪"})
                } else {
                    self.export_update_appearance();
                    json!({"success":true})
                }
            }
            "updateCancel" => match self.updates.cancel() {
                Ok(()) => json!({"success":true}),
                Err(e) => json!({"success":false,"message":e}),
            },
            "updateUnskip" => {
                self.updates.unskip();
                self.dirty = true;
                json!({"success":true,"message":"已恢复此次更新提醒"})
            }
            "updateProgress" => match self.updates.progress(&r["value"]) {
                Ok(reply) => {
                    if matches!(self.updates.phase(), "cancelled" | "failed" | "completed") {
                        self.held = false;
                        self.update_until = Some(Instant::now() + Duration::from_secs(5));
                    }
                    self.dirty = true;
                    reply
                }
                Err(e) => json!({"success":false,"message":e}),
            },
            "deviceSoundPreview" => {
                match sound::preview(&self.settings, "Device", r["value"]["kind"].as_str()) {
                    Ok(()) => json!({"success":true}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "devicePreview" => {
                let kind = match r["value"]["kind"].as_str().unwrap_or("usb") {
                    "phone" => devices::Kind::Phone,
                    "volume" => devices::Kind::Volume,
                    "headphones" => devices::Kind::Headphones,
                    "speaker" => devices::Kind::Speaker,
                    "gamepad" => devices::Kind::Gamepad,
                    "mouse" => devices::Kind::Mouse,
                    "keyboard" => devices::Kind::Keyboard,
                    _ => devices::Kind::Usb,
                };
                let connected = r["value"]["connected"].as_bool().unwrap_or(true);
                let label = match kind {
                    devices::Kind::Phone => "手机 · 示例设备",
                    devices::Kind::Volume => "移动磁盘 · 示例设备",
                    devices::Kind::Headphones => "耳机 · 示例设备",
                    devices::Kind::Speaker => "音响 · 示例设备",
                    devices::Kind::Gamepad => "手柄 · 示例设备",
                    devices::Kind::Mouse => "鼠标 · 示例设备",
                    devices::Kind::Keyboard => "键盘 · 示例设备",
                    devices::Kind::Usb => "U 盘 · 示例设备",
                };
                let e = devices::Event::new("preview".into(), kind, connected, label.into());
                self.enqueue_device(&e);
                json!({"success":true})
            }
            "osdPreview" => {
                let kind = match r["value"]["kind"].as_str().unwrap_or("caps") {
                    "num" => osd::Kind::Num,
                    "mic" => osd::Kind::Mic,
                    "touchpad" => osd::Kind::Touchpad,
                    _ => osd::Kind::Caps,
                };
                let on = r["value"]["on"].as_bool().unwrap_or(true);
                self.show_osd(osd::Hint { kind, on }, true);
                json!({"success":true})
            }
            "mailOtp" => {
                if self.demo || self.session_locked || self.practice.is_some() || self.island_suppressed()
                    || !self.flag("VerificationCodeEnabled", true) || self.flag("PauseNotifications", false) {
                    return json!({"success":true,"shown":false});
                }
                let subject:String = r["value"]["subject"].as_str().unwrap_or("").chars().take(240).collect();
                let body:String = r["value"]["body"].as_str().unwrap_or("").chars().take(1600).collect();
                let Some(code) = verification_code::extract_from_notice("邮箱", &subject, &body) else {
                    return json!({"success":true,"shown":false});
                };
                if !self.wake_for_transient_notice() { return json!({"success":true,"shown":false}); }
                let alert = capsule_alert::Alert::verification("imap", "邮箱验证码", code);
                if !self.taskbar_mode() && self.alerts.visible() {
                    let (r, rad) = self.shape();
                    let now=self.start.elapsed().as_secs_f64();
                    if self.alerts.preempt_or_enqueue(alert,r,rad,self.layout.host.w,self.layout.capsule.y,now,self.flag("Animation",true)) {
                        self.hide_at=None;self.fade(1.);self.dirty=true;
                    }
                } else { self.alerts.enqueue(alert); }
                json!({"success":true,"shown":true})
            }
            "aiQuotaSourceAdd" => {
                let Ok(row) = serde_json::from_value::<ai_quota::sources::Profile>(r["value"].clone()) else {
                    return json!({"success":false,"message":"额度来源配置无效"});
                };
                let mut next=ai_quota::profiles(&self.settings["AiQuotaSources"]);
                if !row.valid() || next.len()>=16 || next.iter().any(|p|p.id==row.id) {
                    return json!({"success":false,"message":"渠道未接入、站点地址无效，或已达到 16 个配置上限"});
                }
                next.push(row);
                let old=std::mem::replace(&mut self.settings["AiQuotaSources"],json!(next));
                if let Err(message)=self.save(){self.settings["AiQuotaSources"]=old;return json!({"success":false,"message":message});}
                self.ai_quota.reset();
                json!({"success":true,"message":"额度来源已添加"})
            }
            "aiQuotaSourceRemove" => {
                let mut next=ai_quota::profiles(&self.settings["AiQuotaSources"]);
                let id=r["value"]["id"].as_str().unwrap_or("");
                let Some(index)=next.iter().position(|row|row.id==id) else {return json!({"success":false,"message":"找不到额度来源"});};
                let row=next.remove(index);
                let old=std::mem::replace(&mut self.settings["AiQuotaSources"],json!(next));
                if let Err(message)=self.save(){self.settings["AiQuotaSources"]=old;return json!({"success":false,"message":message});}
                let _=std::fs::remove_file(self.data.join("ai-quota").join(format!("{}.dpapi",row.id)));
                self.ai_quota.reset();
                self.alerts.remove_pending(&format!("quota:{}",row.id));
                self.dirty |= self.alerts.cancel_prefix(&format!("quota:balance:{}:",row.id));
                json!({"success":true,"message":"额度来源已移除"})
            }
            "aiQuotaRefresh" => {
                self.ai_quota.refresh_soon();
                json!({"success":true})
            }
            "aiQuotaConfigure" => {
                let previous = ai_quota::Settings::from_value(&self.settings["AiQuotaMonitor"]);
                let Ok(mut next) = serde_json::from_value::<ai_quota::Settings>(r["value"].clone()) else {
                    return json!({"success":false,"message":"额度提醒设置无效"});
                };
                if !next.valid() { return json!({"success":false,"message":"阈值应在 0–100% 之间"}); }
                // Clients built before the currency setting must not change an existing preference.
                if r["value"].get("relayCurrency").is_none() { next.relay_unit = previous.relay_unit; }
                if previous == next { return json!({"success":true,"message":"额度提醒设置已保存"}); }
                let old = std::mem::replace(&mut self.settings["AiQuotaMonitor"], json!(next));
                if let Err(message) = self.save() {
                    self.settings["AiQuotaMonitor"] = old;
                    return json!({"success":false,"message":message});
                }
                if (previous.enabled, previous.five_hour, previous.weekly) != (next.enabled, next.five_hour, next.weekly) {
                    self.ai_quota.reset();
                    self.dirty |= self.alerts.cancel_prefix("quota:");
                } else if previous.relay_unit != next.relay_unit {
                    self.dirty |= self.alerts.cancel_prefix("quota:balance:");
                }
                json!({"success":true,"message":"额度提醒设置已保存"})
            }
            "hydrationRecord" => match self.record_hydration() {
                Ok(p) => json!({"success":true,"hydrationToday":{"drankMl":p.drank_ml},"message":format!("已记录 {} mL",p.serving_ml)}),
                Err(message) => json!({"success":false,"message":message}),
            },
            "reminderPluginConfigure" => {
                let (kind, next) = match self.reminder_plugins.proposed(&r["value"]) {
                    Ok(v) => v,
                    Err(message) => return json!({"success":false,"message":message}),
                };
                let old = std::mem::replace(&mut self.settings["ReminderPlugins"], next.clone());
                if let Err(message) = self.save() {
                    self.settings["ReminderPlugins"] = old;
                    return json!({"success":false,"message":message});
                }
                if self.reminder_plugins.apply(kind, &next) {
                    self.alerts.remove_pending(kind.alert_key());
                    if self.alerts.active.as_ref().is_some_and(|a| a.key == kind.alert_key()) {
                        let (r,rad) = self.shape();
                        self.alerts.dismiss(r,rad,self.start.elapsed().as_secs_f64(),self.flag("Animation",true));
                        self.dirty = true;
                    }
                }
                json!({"success":true,"reminderPlugins":next,"message":"已保存"})
            }
            "pluginInstall" | "pluginEnable" | "pluginRemove" => {
                let result=match r["command"].as_str().unwrap_or(""){
                    "pluginInstall"=>self.plugins.install(Path::new(r["value"]["path"].as_str().unwrap_or(""))),
                    "pluginEnable"=>r["value"]["enabled"].as_bool().ok_or("缺少启用状态".to_string()).and_then(|enabled|self.plugins.change(r["value"]["id"].as_str().unwrap_or(""),Some(enabled))),
                    _=>self.plugins.change(r["value"]["id"].as_str().unwrap_or(""),None),
                };self.refresh_components();self.dirty=true;match result{Ok(())=>json!({"success":true,"plugins":self.plugins.state}),Err(e)=>json!({"success":false,"message":e})}
            }
            "widgetLaunch" => {
                let id = r["value"]["id"].as_str().unwrap_or("");
                let placements = if id.starts_with("overview-") {
                    &self.overview_widgets.items
                } else {
                    &self.desktop_widgets.items
                };
                widget_data::launch(placements, id)
            }
            "widgetAdd" => match self.desktop_widgets.add(&r["value"]) {
                Ok(id) => {
                    self.ensure_widget_host();
                    json!({"success":true,"id":id,"desktopWidgets":self.desktop_widgets.list()})
                }
                Err(e) => json!({"success":false,"message":e}),
            },
            "widgetUpdate" => match self.desktop_widgets.update(&r["value"]) {
                Ok(()) => json!({"success":true,"desktopWidgets":self.desktop_widgets.list()}),
                Err(e) => json!({"success":false,"message":e}),
            },
            "overviewWidgetSet" => match self.overview_widgets.set(&r["value"]) {
                Ok(()) => json!({"success":true,"overviewWidgets":self.overview_widgets.list()}),
                Err(e) => json!({"success":false,"message":e}),
            },
            "overviewWidgetRemove" => {
                match serde_json::from_value::<dotisle_domain::desktop_widgets::Kind>(r["value"]["kind"].clone()) {
                    Ok(kind) => match self.overview_widgets.remove(kind) {
                        Ok(()) => json!({"success":true,"overviewWidgets":self.overview_widgets.list()}),
                        Err(e) => json!({"success":false,"message":e}),
                    },
                    Err(_) => json!({"success":false,"message":"组件类型无效"}),
                }
            }
            "widgetRemove" => {
                match self
                    .desktop_widgets
                    .remove(r["value"]["id"].as_str().unwrap_or(""))
                {
                    Ok(()) => json!({"success":true,"desktopWidgets":self.desktop_widgets.list()}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "timerCreate" => {
                let mut value = r["value"].clone();
                if value["mode"] == "pomodoro" && value.get("countUp").is_none() {
                    value["countUp"] = json!(self.flag("FocusCountUp", false));
                }
                if value["name"].as_str().is_none_or(|s| s.trim().is_empty()) {
                    if let Some(title) = value["todoId"]
                        .as_str()
                        .and_then(|id| self.todos.items.iter().find(|t| t.id == id))
                        .map(|t| t.title.clone())
                    {
                        value["name"] = json!(title);
                    }
                }
                match self.timers.create(&value) {
                    Ok(id) => {
                        self.refresh_components();
                        self.select_component(&format!("timer:{id}"), 1);
                        self.user_hidden = false;
                        self.hidden_mini = false;
                        self.hide_at = None;
                        self.fade(1.);
                        self.held = true;
                        self.retarget(true);
                        json!({"success":true,"id":id})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "timerAction" => {
                let id = r["value"]["id"].as_str().unwrap_or("");
                let action = r["value"]["action"].as_str().unwrap_or("");
                let result = if action == "direction" {
                    r["value"]["countUp"]
                        .as_bool()
                        .ok_or_else(|| "计时方向无效".to_owned())
                        .and_then(|up| {
                            self.timers
                                .set_direction(id, up, r["value"]["revision"].as_u64())
                        })
                } else {
                    self.timers.act(id, action, r["value"]["revision"].as_u64())
                };
                match result {
                    Ok(()) => {
                        self.refresh_components();
                        self.compact_key.clear();
                        json!({"success":true,"timers":self.timers.list()})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "quickReminderPreview" => {
                let text = r["value"]["text"].as_str().unwrap_or("").to_owned();
                self.quick_preview(&text)
            }
            "quickReminderCommit" => {
                let token = r["value"]["previewToken"].as_str().unwrap_or("");
                let request = r["value"]["clientRequestId"].as_str().unwrap_or("");
                self.quick_commit(token, request)
            }
            "pendingActionSeen" => {
                self.pending_action = None;
                json!({"success":true})
            }
            "focusResume" => {
                let ids = r["value"]["ids"]
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| v.as_str().map(str::to_owned))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default();
                let generation = r["value"]["generation"].as_u64().unwrap_or(0);
                if !self.flag("FocusPauseOnLock", false) {
                    return json!({"success":false,"message":"锁屏暂停专注已关闭"});
                }
                match self.timers.resume_focus(&ids, generation) {
                    Ok(resumed) => {
                        if let Some(f) = &mut self.focus_resume {
                            f.ids.retain(|id| !resumed.contains(id));
                        }
                        if self.focus_resume.as_ref().is_some_and(|f| f.ids.is_empty()) {
                            self.focus_resume = None;
                        }
                        self.refresh_components();
                        json!({"success":true,"timers":self.timers.list(),"resumed":resumed})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "focusResumeDismiss" => {
                self.focus_resume = None;
                json!({"success":true})
            }
            "workflowTemplateSave" => match self.templates.save(&r["value"]) {
                Ok(id) => json!({"success":true,"id":id,"templates":self.templates.list()}),
                Err(e) => json!({"success":false,"message":e}),
            },
            "workflowTemplateDelete" => {
                match self.templates.delete(r["value"].as_str().unwrap_or("")) {
                    Ok(()) => json!({"success":true,"templates":self.templates.list()}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "appRuleSave" => {
                self.todo_now = todos::Now::local();
                let today = self.todo_now.date.clone();
                match self.rules.save(
                    &r["value"],
                    &today,
                    self.foreground.path.as_deref(),
                    self.foreground.segment,
                ) {
                    Ok(id) => json!({"success":true,"id":id,"rules":self.rules.list(&today)}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "appRuleAction" => {
                self.todo_now = todos::Now::local();
                let today = self.todo_now.date.clone();
                let id = r["value"]["id"].as_str().unwrap_or("").to_owned();
                let action = r["value"]["action"].as_str().unwrap_or("").to_owned();
                match self
                    .rules
                    .action(&id, &action, r["value"]["revision"].as_u64(), &today)
                {
                    Ok(()) => {
                        if action == "delete" {
                            let key = format!("rule:{id}");
                            self.queue
                                .retain(|e| e.todo.as_deref() != Some(key.as_str()));
                        }
                        json!({"success":true,"rules":self.rules.list(&today)})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "appRuleClearUsage" => match self.rules.clear_usage() {
                Ok(()) => json!({"success":true,"rules":self.rules.list(&self.todo_now.date)}),
                Err(e) => json!({"success":false,"message":e}),
            },
            "foregroundInfo" => {
                json!({"success":true,"path":self.foreground.path,"name":self.foreground.name,"unreadable":self.foreground.unreadable,"segment":self.foreground.segment})
            }
            "islandSelect" => {
                self.refresh_components();
                let id = r["value"].as_str().unwrap_or("");
                if !self.components.iter().any(|c| c == id) {
                    json!({"success":false,"message":"组件不存在"})
                } else {
                    self.select_component(id, 1);
                    self.user_hidden = false;
                    self.hidden_mini = false;
                    self.fade(1.);
                    json!({"success":true})
                }
            }
            "islandCycle" => {
                self.cycle_component(if r["value"] == json!(-1) { -1 } else { 1 });
                json!({"success":true})
            }
            "todos" => {
                json!({"success":true,"todos":self.todos.list(&self.todo_now),"today":self.todo_now.date,"message":self.todos.error})
            }
            "todoSave" => {
                if self.demo {
                    return json!({"success":true,"id":"demo"});
                }
                self.todo_now = todos::Now::local();
                match self.todos.save(&r["value"]) {
                    Ok(id) => {
                        json!({"success":true,"id":id,"todos":self.todos.list(&self.todo_now),"message":"已保存待办"})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "todoComplete" => {
                if self.demo {
                    return json!({"success":true});
                }
                let id = r["value"]["id"].as_str().unwrap_or("").to_owned();
                let done = r["value"]["completed"].as_bool().unwrap_or(true);
                self.todo_now = todos::Now::local();
                match self.todos.complete(&id, done, &self.todo_now) {
                    Ok(()) => {
                        if done {
                            self.cancel_todo(&id);
                        }
                        json!({"success":true,"todos":self.todos.list(&self.todo_now),"message":if done{"已完成"}else{"已恢复待办"}})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "todoSnooze" => {
                if self.demo {
                    return json!({"success":true});
                }
                let id = r["value"]["id"].as_str().unwrap_or("").to_owned();
                let minutes = r["value"]["minutes"].as_u64().unwrap_or(0) as u32;
                self.todo_now = todos::Now::local();
                match self.todos.snooze(&id, minutes, &self.todo_now) {
                    Ok(()) => {
                        self.cancel_todo(&id);
                        json!({"success":true,"todos":self.todos.list(&self.todo_now),"message":if minutes>=60{"已推迟 1 小时后提醒"}else{"已推迟 10 分钟后提醒"}})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "todoDelete" => {
                if self.demo {
                    return json!({"success":true});
                }
                let id = r["value"]["id"].as_str().unwrap_or("").to_owned();
                match self.todos.delete(&id) {
                    Ok(()) => {
                        self.cancel_todo(&id);
                        json!({"success":true,"todos":self.todos.list(&self.todo_now),"message":"已删除"})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "historyClear" => {
                let kind = r["value"]["kind"].as_str().filter(|k| !k.is_empty());
                if let Some(kind) = kind {
                    if !history::kinds().contains(&kind) {
                        return json!({"success":false,"message":"未知的通知分类"});
                    }
                }
                self.history.clear(kind);
                json!({"success":true,"history":self.history.list(),"message":"已清除通知历史"})
            }
            "historyOpen" => {
                let id = r["value"].as_str().unwrap_or("");
                match self.history.items.iter().find(|e| e.id == id) {
                    Some(event) if event.kind == "notice" && !event.app_id.is_empty() => {
                        match activation::open(&event.app_id) {
                            Ok(()) => json!({"success":true,"message":"已打开来源应用"}),
                            Err(e) => json!({"success":false,"message":e}),
                        }
                    }
                    Some(_) => json!({"success":false,"message":"该记录没有可打开的来源应用"}),
                    None => json!({"success":false,"message":"通知记录不存在"}),
                }
            }
            "pageSeen" => {
                // A delayed acknowledgement must not consume a newer request.
                if r["value"]["requestId"].as_u64().is_none_or(|id|id==self.pending_request){
                    self.pending_page = None;
                    self.pending_center_action=Value::Null;
                }
                json!({"success":true})
            }
            "collectLogs" => {
                match diagnostics::collect(&self.data, self.error.as_deref(), DISPLAY_VERSION) {
                    Ok(path) => {
                        if std::env::var_os("LYRICISLAND_TEST_PIPE").is_none() {
                            diagnostics::reveal(&path);
                        }
                        json!({"success":true,"message":format!("诊断日志已生成：{}（已在资源管理器中选中）",path.file_name().map(|n|n.to_string_lossy().to_string()).unwrap_or_default()),"path":path.to_string_lossy()})
                    }
                    Err(e) => json!({"success":false,"message":format!("生成诊断日志失败：{e}")}),
                }
            }
            "soundPreview" => {
                let todo = r["value"]["todo"].as_bool().unwrap_or(false);
                match sound::preview(
                    &self.settings,
                    if todo { "Todo" } else { "Notification" },
                    r["value"]["kind"].as_str(),
                ) {
                    Ok(()) => json!({"success":true,"message":"已播放试听"}),
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "debugSnapshot"
                if std::env::var("LYRICISLAND_TEST_PIPE")
                    .is_ok_and(|p| p.starts_with("LyricIsland.Settings.test.")) =>
            {
                if let (Some(renderer), Some(path)) = (&self.renderer, r["value"].as_str()) {
                    match renderer.snapshot(Path::new(path)) {
                        Ok(()) => json!({"success":true}),
                        Err(e) => json!({"success":false,"message":e.to_string()}),
                    }
                } else {
                    json!({"success":false})
                }
            }
            "expand" => {
                if self.practice.is_some() { return json!({"success":false,"message":"请按实操教程提示操作。"}); }
                if self.user_hidden && self.hidden_edge() {
                    self.reveal_hidden_edge_to_card();
                    return json!({"success":true});
                }
                if self.user_hidden {
                    self.reveal_hidden_edge();
                }
                self.user_hidden = false;
                self.hidden_mini = false;
                self.held = true;
                self.hide_at = None;
                self.missing_since = None;
                self.display_wanted = true;
                let _ = self.position();
                self.retarget(true);
                self.fade(1.);
                json!({"success":true})
            }
            "toggleVisibility" => {
                if self.practice.is_some() { return json!({"success":false,"message":"请按实操教程提示操作。"}); }
                unsafe { self.toggle_island_visibility(false); }
                json!({"success":true})
            }
            "notificationPreview" => {
                if self.flag("PauseNotifications", false) {
                    return json!({"success":false,"message":"通知已暂停，请先关闭“暂停通知”后预览"});
                }
                self.show_notice(notifications::Notice::example(&self.base), true, None, true);
                json!({"success":true,"message":"通知横幅预览"})
            }
            "toggleNotifications" => {
                if !self.flag("NotificationsEnabled", true) {
                    return self.request(json!({"version":1,"command":"update","value":{"key":"NotificationsEnabled","value":true}}));
                }
                let paused = !self.flag("PauseNotifications", false);
                self.request(json!({"version":1,"command":"update","value":{"key":"PauseNotifications","value":paused}}))
            }
            "prepareUpdateExit" => {
                if let Err(e) = self.save().and_then(|_| self.timers.persist()) {
                    return json!({"success":false,"message":e});
                }

                Self::terminate_child(&mut self.center_process);
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                json!({"success":true})
            }
            "restart" => {
                let result = (|| -> std::io::Result<()> {
                    let exe = std::env::current_exe()?;
                    let mut command = std::process::Command::new(exe);
                    command
                        .args([
                            "--restart-after",
                            &std::process::id().to_string(),
                            "--data-dir",
                        ])
                        .arg(&self.data);
                    if self.demo {
                        command.arg("--demo");
                    }
                    command.current_dir(&self.base).spawn()?;
                    Ok(())
                })();
                match result {
                    Ok(()) => {
                        let _ = self.save();

                        Self::terminate_child(&mut self.center_process);
                        let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                        json!({"success":true,"message":"正在重启点屿"})
                    }
                    Err(e) => json!({"success":false,"message":format!("重启失败：{e}")}),
                }
            }
            "settings" => {
                let action=r["value"]["action"].as_str().unwrap_or("");
                let id=r["value"]["id"].as_str().unwrap_or("");
                let page=match action{
                    "todoEdit" if id.is_empty()||self.todos.items.iter().any(|t|t.id==id)=>"todos",
                    "widgetConfigure" if self.desktop_widgets.items.iter().any(|p|p.id==id)=>"widgets",
                    ""=>r["value"]["page"].as_str().unwrap_or(""),
                    _=>return json!({"success":false,"message":"目标已移除或请求无效"}),
                };
                self.show_settings_page(page);
                if !action.is_empty(){self.pending_center_action=json!({"action":action,"id":id});}
                json!({"success":true})
            }
            "exit" => {
                let _ = PostMessageW(Some(self.hwnd), WM_CLOSE, WPARAM(0), LPARAM(0));
                json!({"success":true})
            }
            "synchronize" => {
                if let Some(n) = r["value"]
                    .as_f64()
                    .filter(|n| n.is_finite() && (0.0..=86400.).contains(n))
                {
                    self.command("synchronize", n);
                    json!({"success":true})
                } else {
                    json!({"success":false,"message":"请输入有效的播放秒数"})
                }
            }
            "previewAppearance" => {
                let key = r["value"]["key"].as_str().unwrap_or("");
                if !config::is_acrylic_value(key) {
                    return json!({"success":false,"message":"只支持亚克力外观预览"});
                }
                match config::validate_setting(
                    json!({"key":key,"value":r["value"]["value"],"settings":self.settings}),
                ) {
                    Ok(next) => {
                        self.acrylic_preview[key] = next[key].clone();
                        self.dirty = true;
                        json!({"success":true})
                    }
                    Err(e) => json!({"success":false,"message":e}),
                }
            }
            "wechatWarningOpen" => {
                let Some(id) = r["value"].as_str().filter(|id| !id.is_empty() && id.len() <= 64) else {
                    return json!({"success":false,"message":"警告确认标识无效"});
                };
                self.wechat_warning = Some((id.to_owned(), Instant::now()));
                json!({"success":true})
            }
            "wechatWarningCancel" => {
                if self.wechat_warning.as_ref().is_some_and(|(id, _)| r["value"].as_str() == Some(id.as_str())) {
                    self.wechat_warning = None;
                }
                json!({"success":true})
            }
            "update"
            | "wechatWarningConfirm"
            | "resetAppearance"
            | "resetMotion"
            | "resetSettingsColors"
            | "soundSelect"
            | "resetBuiltinSounds" => {
                let previous_geometry = self.music_geometry();
                let old = self.settings.clone();
                let next = if command == "wechatWarningConfirm" {
                    let eligible = self.wechat_warning.as_ref().is_some_and(|(id, opened)| {
                        r["value"].as_str() == Some(id.as_str())
                            && (Duration::from_secs(10)..=Duration::from_secs(600)).contains(&opened.elapsed())
                    });
                    if !eligible {
                        return json!({"success":false,"message":"请重新打开警告，等待至少 10 秒并点击确认"});
                    }
                    self.wechat_warning = None;
                    let mut s = old.clone();
                    s["WeChatTaskbarWarningAccepted"] = json!(true);
                    s["WeChatMessageHintsEnabled"] = json!(true);
                    Ok(s)
                } else if command == "update" && r["value"]["key"] == "WeChatMessageHintsEnabled" && r["value"]["value"] == true {
                    Err("请在控制中心阅读警告并确认后再开启微信提醒".into())
                } else if command == "soundSelect" {
                    sound::select(
                        &old,
                        r["value"]["category"].as_str().unwrap_or(""),
                        r["value"]["source"].as_str().unwrap_or("Builtin"),
                        r["value"]["kind"].as_str().unwrap_or(""),
                    )
                } else if command == "resetBuiltinSounds" {
                    Ok(sound::reset_builtin(&old))
                } else if command == "resetMotion" {
                    let mut s = old.clone();
                    s["Motion"] = serde_json::to_value(motion::Options::default()).unwrap();
                    s["Animation"] = json!(true);
                    Ok(s)
                } else if command == "resetSettingsColors" {
                    let mut s = old.clone();
                    s["SettingsLightAccentColor"] = json!("#2563EB");
                    s["SettingsDarkAccentColor"] = json!("#6EA8FF");
                    Ok(s)
                } else if command == "resetAppearance" {
                    let mut s = old.clone();
                    s["CompactWidth"] = json!(260.);
                    s["FontSize"] = json!(14.);
                    s["Floating"] = json!(false);
                    Ok(s)
                } else {
                    config::validate_setting(
                        json!({"key":r["value"]["key"],"value":r["value"]["value"],"settings":old}),
                    )
                };
                let next = match next {
                    Ok(v) => v,
                    Err(e) => return json!({"success":false,"message":e}),
                };
                let size_mode_origin = if r["value"]["key"] == "IslandSizeMode"
                    && !self.taskbar_mode()
                    && !self.user_hidden
                    && self.opacity > 0.01
                {
                    let now = self.start.elapsed().as_secs_f64();
                    let (rect, rad, sm, mm, _cm) = self.live_morph_origin(now);
                    Some((now, rect, rad, sm, mm))
                } else {
                    None
                };
                self.settings = next;
                config::resolve_render_font(&mut self.settings, &self.data.join("settings.json"));
                let saved = if !self.demo && r["value"]["key"] == "AutoStart" {
                    self.apply_startup()
                } else {
                    self.save()
                };
                if let Err(e) = saved {
                    self.settings = old;
                    return json!({"success":false,"message":e});
                }
                if r["value"]["key"] == "MusicCustomLayout"
                    || r["value"]["key"] == "MusicExpandedLayout"
                {
                    self.music_custom = lyric_core::custom_layout::Layout::from_value(
                        &self.settings["MusicCustomLayout"],
                    )
                    .unwrap_or_default();
                    self.custom_from = None;
                    let final_height = self.card_height();
                    if self.flag("Animation", true) && self.expanded {
                        self.custom_from = Some(previous_geometry);
                        self.custom_start = Instant::now();
                    }
                    let _ = unsafe { self.position() };
                    self.layout.host.h = self.layout.host.h.max(final_height + 72.);
                    self.dirty = true;
                }
                let key = r["value"]["key"].as_str().unwrap_or("");
                if key == "TaskbarPosition" && self.settings["TaskbarDockAlign"].as_str() != Some("Free") {
                    self.settings["TaskbarDockAlign"] = json!("Free");
                    let _ = self.save();
                }
                if matches!(
                    key,
                    "Display"
                        | "Floating"
                        | "CenterX"
                        | "TopFraction"
                        | "TaskbarModeEnabled"
                        | "TaskbarAlwaysShow"
                        | "TaskbarStyle"
                        | "TaskbarDockAlign"
                        | "TaskbarPosition"
                        | "TaskbarWidth"
                ) {
                    if matches!(key, "TaskbarModeEnabled" | "TaskbarAlwaysShow") {
                        if self.taskbar_mode() {
                            self.island_morph.cancel();
                            self.user_hidden = false;
                            self.hidden_mini = false;
                            self.edge_revealed = false;
                            self.restore_hidden = false;
                            if self.taskbar_always_show() {
                                self.hide_at = None;
                                self.missing_since = None;
                                self.display_wanted = true;
                            }
                            self.fade(1.);
                            if key == "TaskbarModeEnabled" {
                                let (r, rad) = self.shape();
                                let now = self.start.elapsed().as_secs_f64();
                                self.banner.dismiss(r, rad, now, false, self.base_needed());
                                self.osd_banner.dismiss(r, rad, now, false, self.base_needed());
                            }
                        } else if key == "TaskbarModeEnabled" {
                            unsafe { self.top_popup.clear(); }
                        }
                    }
                    let _ = unsafe { self.position() };
                    self.refresh_components();
                    self.poll();
                    self.dirty = true;
                }
                if matches!(key, "HideIslandWhenFullscreen" | "Display" | "TaskbarModeEnabled") {
                    if let Err(error) = unsafe { self.poll_island_visibility(true) } {
                        self.error = Some(error.to_string());
                    }
                }
                if key == "EdgeHoverRevealEnabled" && !self.flag("EdgeHoverRevealEnabled", false) {
                    self.edge_hover_since = None;
                    if self.edge_revealed {
                        self.edge_revealed = false;
                        if self.user_hidden && !self.banner.visible() {
                            self.fade(0.);
                        }
                    }
                }
                if key == "AppUsageRemindersEnabled" { self.last_awake = awake_ms(); }
                if config::is_paint_setting(key) {
                    if let Some(values) = self.acrylic_preview.as_object_mut() {
                        values.remove(key);
                    }
                    self.dirty = true;
                    self.sync_bottom_material();
                    return json!({"success":true,"message":"已保存"});
                }
                if r["value"]["key"] == "NotificationHistoryDays" {
                    self.history.prune(
                        self.settings["NotificationHistoryDays"]
                            .as_u64()
                            .unwrap_or(7)
                            .clamp(1, 30) as u32,
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs() as i64,
                    );
                }
                if let Some(listener) = &self.notification_listener {
                    listener.configure(notifications::Policy::from_settings(&self.settings));
                }
                if r["value"]["key"] == "QuickReminderHotkey" && !self.demo {
                    if let Err(e) = unsafe { self.register_hotkey() } {
                        self.settings = old.clone();
                        let _ = self.save();
                        let _ = unsafe { self.register_hotkey() };
                        return json!({"success":false,"message":e});
                    }
                }
                if r["value"]["key"] == "FocusPauseOnLock" && !self.flag("FocusPauseOnLock", false)
                {
                    self.focus_resume = None;
                    if let Err(e) = self.timers.clear_lock_pauses() {
                        self.error = Some(e);
                    }
                }
                if !self.notification_preview
                    && self.banner.visible()
                    && (self.flag("PauseNotifications", false)
                        || (self.banner_todo.is_none()
                            && (!self.flag("NotificationsEnabled", true)
                                || self.banner.notice.as_ref().is_some_and(|n| {
                                    !notifications::Policy::from_settings(&self.settings)
                                        .permits(&n.app_id, &n.app_name)
                                }))))
                {
                    let (r, rad) = self.shape();
                    self.banner.dismiss(
                        r,
                        rad,
                        self.start.elapsed().as_secs_f64(),
                        self.flag("Animation", true),
                        self.base_needed(),
                    );
                }
                if let Some(b) = &self.backend {
                    b.apply(self.settings.clone());
                }
                if let Err(e) = self.position() {
                    self.error = Some(e.to_string());
                    return json!({"success":false,"message":e.to_string()});
                }
                if let Some((now, from_rect, from_rad, from_strip, from_mini)) = size_mode_origin {
                    self.poll();
                    let default_h = if self.mini() { 14. } else { 40. };
                    let (tw, th) = unsafe { self.measure_compact_layout() }
                        .unwrap_or((self.compact_base(), default_h));
                    self.compact_size = compact::Resize::new(tw, th);
                    self.layout.set_compact(tw, th);
                    let to_mode = if self.mini() {
                        morph_transition::IslandMode::Mini
                    } else {
                        morph_transition::IslandMode::Normal
                    };
                    let target_rect = self.layout.capsule;
                    let target_rad = self.capsule_corner_radius(th);
                    let duration_ms = if to_mode == morph_transition::IslandMode::Mini {
                        self.options().collapse_duration
                    } else {
                        self.options().duration
                    };
                    self.island_morph.transition(
                        from_rect,
                        from_rad,
                        from_strip,
                        from_mini,
                        to_mode,
                        target_rect,
                        target_rad,
                        now,
                        self.flag("Animation", true),
                        duration_ms,
                    );
                }
                if r["value"]["key"] == "SystemStateHintPlacement" {
                    unsafe {
                        self.bottom_osd.clear();
            self.top_popup.clear();
                    }
                    let now = self.start.elapsed().as_secs_f64();
                    let (r, rad) = self.shape();
                    self.osd_banner
                        .dismiss(r, rad, now, false, self.base_needed());
                }
                if r["value"]["key"] == "MusicExpandedLayout" {
                    self.start_music_shift();
                }
                if matches!(
                    key,
                    "ProgressBarStyle"
                        | "ProgressBarThumb"
                        | "ProgressBarThumbEnabled"
                        | "ProgressBarThumbSpin"
                        | "TransportTheme"
                ) {
                    self.dirty = true;
                }
                if r["value"]["key"] == "IslandSizeMode" {
                    self.swipe = None;
                    self.press_origin = None;
                    self.dragging = None;
                    self.seeking = false;
                    self.hover = 0;
                    self.pressed = 0;
                    self.compact_secondary = self.compact_secondary_for();
                    self.previous_compact_secondary.clear();
                    self.input_region = None;
                }
                if [
                    "SystemStateHintsEnabled",
                    "CapsLockHintEnabled",
                    "NumLockHintEnabled",
                    "MicrophoneMuteHintEnabled",
                    "TouchpadHintEnabled",
                ]
                .contains(&r["value"]["key"].as_str().unwrap_or(""))
                {
                    self.apply_osd();
                }
                if key == "IslandKeepVisibleEnabled" {
                    if self.flag("IslandKeepVisibleEnabled", false) {
                        self.hide_at = None;
                        self.missing_since = None;
                        self.display_wanted = true;
                        if !self.user_hidden { self.fade(1.); }
                    }
                    self.poll();
                    self.dirty = true;
                }
                if [resident::TIME, resident::SYSTEM, resident::DEVICES].contains(&key) {
                    self.refresh_components();
                }
                self.sync_bottom_material();
                self.transition = None;
                self.frame = if self.expanded {
                    Frame::expanded()
                } else {
                    Frame::hidden(self.options())
                };
                json!({"success":true,"message":"已保存"})
            }
            _ => json!({"success":false,"message":"不支持的操作"}),
        }
    }
}
unsafe extern "system" fn visual_wndproc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_NCHITTEST => LRESULT(HTTRANSPARENT as isize),
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
        WM_PAINT => {
            let mut paint = PAINTSTRUCT::default();
            let _ = BeginPaint(hwnd, &mut paint);
            let _ = EndPaint(hwnd, &paint);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}
fn within_click_slop(origin: Option<(f64, f64)>, x: f64, y: f64) -> bool {
    origin.is_some_and(|(down_x, down_y)| (x - down_x).hypot(y - down_y) <= 8.)
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_NCCREATE {
        let create = &*(lparam.0 as *const CREATESTRUCTW);
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        return LRESULT(1);
    }
    if msg == WM_DESTROY {
        let mut icon = NOTIFYICONDATAW {
            cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: hwnd,
            uID: 1,
            ..Default::default()
        };
        let _ = Shell_NotifyIconW(NIM_DELETE, &mut icon);
        PostQuitMessage(0);
        return LRESULT(0);
    }
    if msg == WM_NCDESTROY {
        SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
        return DefWindowProcW(hwnd, msg, wparam, lparam);
    }
    if msg == taskbar_created_msg() {
        let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<App>;
        if !ptr.is_null() {
            if let Ok(mut app) = (*ptr).try_borrow_mut() {
                if app.taskbar_mode() {
                    app.last_taskbar_probe = None;
                    let _ = app.position();
                    let _ = window_level::apply(app.visual_hwnd, app.hwnd, true, !app.island_suppressed() && app.opacity > 0.001);
                    app.dirty = true;
                }
            }
        }
        return LRESULT(0);
    }
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const RefCell<App>;
    if !ptr.is_null() {
        if let Ok(mut app) = (*ptr).try_borrow_mut() {
            let a = &mut *app;
            match msg {
                WM_NCHITTEST => {
                    let mut pt = POINT {
                        x: (lparam.0 as u16 as i16) as i32,
                        y: ((lparam.0 >> 16) as u16 as i16) as i32,
                    };
                    let _ = ScreenToClient(hwnd, &mut pt);
                    let s = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
                    let (px, py) = (pt.x as f64 / s, pt.y as f64 / s);
                    if a.island_morph.active() && !a.alerts.visible() && !a.banner.visible() {
                        let p = a.island_morph.presentation(a.start.elapsed().as_secs_f64(), a.layout.host);
                        return LRESULT(if inside_round_rect(p.rect, p.radius, px, py) {
                            HTCLIENT as isize
                        } else {
                            HTTRANSPARENT as isize
                        });
                    }
                    if a.hidden_edge() {
                        return LRESULT(if a.hidden_edge_hit_rect().contains(px, py) {
                            HTCLIENT as isize
                        } else {
                            HTTRANSPARENT as isize
                        });
                    }
                    return LRESULT(if a.hit(px, py) < 0 {
                        HTTRANSPARENT as isize
                    } else {
                        HTCLIENT as isize
                    });
                }
                WM_DEVICECHANGE => {
                    if a.flag("DeviceRemindersEnabled", true) {
                        a.devices.message(wparam.0, lparam.0);
                    }
                    return LRESULT(0);
                }
                WM_INPUT_DEVICE_CHANGE => {
                    if a.flag("DeviceRemindersEnabled", true) {
                        a.devices.input_change(wparam.0 as u32, lparam.0);
                    }
                    return LRESULT(0);
                }
                osd::WM_OSD => {
                    a.pending_osd |= wparam.0 as u32;
                    let _ = SetTimer(Some(hwnd), osd::OSD_TIMER, 45, None);
                    return LRESULT(0);
                }
                WM_TIMER => {
                    if wparam.0 == osd::OSD_TIMER {
                        let _ = KillTimer(Some(hwnd), osd::OSD_TIMER);
                        a.flush_osd();
                        return LRESULT(0);
                    }
                }
                WM_KILLFOCUS if a.agent.view.editing => { a.end_agent_edit(false,false); }
                WM_CHAR if a.agent.view.editing => {
                    match wparam.0 as u16 {
                        8 => {
                            if a.agent.view.draft.pop().is_some_and(|c|(0xdc00..=0xdfff).contains(&c)) && a.agent.view.draft.last().is_some_and(|c|(0xd800..=0xdbff).contains(c)) {a.agent.view.draft.pop();}
                        }
                        13 => a.end_agent_edit(true,true),
                        27 => a.end_agent_edit(false,true),
                        c if c >= 32 && c != 127 && a.agent.view.draft.len() < 240 => a.agent.view.draft.push(c),
                        _ => {}
                    }
                    a.dirty=true;
                    return LRESULT(0);
                }
                WM_MOUSEACTIVATE => return LRESULT(MA_NOACTIVATE as isize),
                WM_ERASEBKGND => return LRESULT(1),
                WM_PAINT => {
                    let mut p = PAINTSTRUCT::default();
                    let _ = BeginPaint(hwnd, &mut p);
                    let _ = EndPaint(hwnd, &p);
                    a.dirty = true;
                    return LRESULT(0);
                }
                WM_MOUSEWHEEL => {
                    let mut point=POINT{x:lparam.0 as u16 as i16 as i32,y:(lparam.0>>16) as u16 as i16 as i32};
                    let _=ScreenToClient(hwnd,&mut point);
                    let scale=GetDpiForWindow(hwnd).max(96) as f64/96.;
                    let (px, py) = (point.x as f64 / scale, point.y as f64 / scale);
                    if a.expanded && a.selected == "todos" {
                        let count = a.panel.as_ref().map(|p| p.tasks.len()).unwrap_or(0);
                        let max = layout::todo_max_scroll(a.layout.card.h, count);
                        if max > 0. {
                            let delta = ((wparam.0 >> 16) as u16 as i16) as f64;
                            a.todo_scroll_target = (a.todo_scroll_target - (delta / 120.) * 46.).clamp(0., max);
                            a.dirty = true;
                            return LRESULT(0);
                        }
                    }
                    if a.hit(px, py)==volume::BUTTON {
                        a.volume.wheel((wparam.0>>16) as u16 as i16);a.hover=volume::BUTTON;a.hover_since=Instant::now()-Duration::from_millis(450);a.tooltip_visible=true;a.dirty=true;return LRESULT(0);
                    }
                }
                WM_RBUTTONDOWN => {
                    if a.user_hidden {
                        a.reveal_hidden_edge();
                        a.pressed = -1;
                    } else if (a.practice.is_none() || a.practice_step() == Some(PracticeStep::Hide))
                        && !a.taskbar_mode()
                        && !a.banner.visible()
                        && !a.alerts.visible()
                    {
                        a.hide_by_gesture();
                    }
                    return LRESULT(0);
                }
                WM_LBUTTONDOWN => {
                    a.press_origin = None;
                    if a.user_hidden {
                        if a.hidden_edge() {
                            a.reveal_hidden_edge_to_card();
                        } else {
                            a.reveal_hidden_edge();
                        }
                        a.pressed = -1;
                        return LRESULT(0);
                    }
                    let s = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
                    let x = (lparam.0 as u16 as i16) as f64 / s;
                    let y = ((lparam.0 >> 16) as u16 as i16) as f64 / s;
                    a.pressed = a.hit(x, y);
                    a.press_origin = (a.pressed >= 0).then_some((x, y));
                    a.agent_pressed_token=a.agent.view.prompt.as_ref().map(|p|p.token);
                    // 用户主动交互时若正处于边缘临时浮现状态，转为常态显示
                    if a.edge_revealed && a.pressed >= 0 {
                        a.user_hidden = false;
                        a.hidden_mini = false;
                        a.edge_revealed = false;
                        a.edge_leave_since = None;
                        a.manual_until = Some(Instant::now() + Duration::from_secs(6));
                    }
                    a.float_drag_arm = None;
                    if a.flag("Floating", false)
                        && !a.flag("TaskbarModeEnabled", false)
                        && !a.banner.visible()
                        && matches!(a.pressed, 0 | 1 | 13)
                    {
                        let mut cursor = POINT::default();
                        let mut bounds = RECT::default();
                        let _ = GetCursorPos(&mut cursor);
                        let _ = GetWindowRect(hwnd, &mut bounds);
                        a.float_drag_arm = Some((
                            POINT {
                                x: (lparam.0 as u16 as i16) as i32,
                                y: ((lparam.0 >> 16) as u16 as i16) as i32,
                            },
                            cursor,
                            bounds,
                        ));
                    }
                    if a.can_swipe(a.pressed) {
                        a.swipe = Some(activities::Swipe::new(x, y));
                    }
                    a.dirty = true;
                    if a.pressed == 2 {
                        a.activate(2);
                        a.pressed = 0;
                    } else {
                        SetCapture(hwnd);
                        if a.pressed == 10 && a.snapshot["canSeek"] == true {
                            a.seeking = true;
                            a.seek_track =
                                a.snapshot["track"]["id"].as_str().unwrap_or("").to_string();
                            a.update_seek(x);
                        }
                    }
                    return LRESULT(0);
                }
                WM_MOUSEMOVE => {

                    let scale = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
                    if let Some((down_pt, start_cursor, bounds)) = a.float_drag_arm {
                        let cx = (lparam.0 as u16 as i16) as i32;
                        let cy = ((lparam.0 >> 16) as u16 as i16) as i32;
                        let dist_sq = (cx - down_pt.x).pow(2) + (cy - down_pt.y).pow(2);
                        if dist_sq >= (6. * scale).round().powi(2) as i32 {
                            a.float_drag_arm = None;
                            a.dragging = Some((start_cursor, bounds));
                            a.swipe = None;
                            a.pressed = -1;
                            SetCapture(hwnd);
                        }
                    }
                    if let Some((start, bounds)) = a.dragging {
                        let mut cur = POINT::default();
                        let _ = GetCursorPos(&mut cur);
                        let dx = cur.x - start.x;
                        let dy = cur.y - start.y;
                        for h in [a.visual_hwnd, a.hwnd] {
                            let _ = SetWindowPos(
                                h,
                                None,
                                bounds.left + dx,
                                bounds.top + dy,
                                0,
                                0,
                                SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE,
                            );
                        }
                        a.dirty = true;
                        return LRESULT(0);
                    }
                    let x = (lparam.0 as u16 as i16) as f64 / scale;
                    let y = ((lparam.0 >> 16) as u16 as i16) as f64 / scale;
                    if !within_click_slop(a.press_origin, x, y) {
                        a.press_origin = None;
                    }
                    if let Some(swipe) = &mut a.swipe {
                        swipe.update(x, y);
                        if swipe.active {
                            a.pressed = -1;
                            a.dirty = true;
                        }
                    }
                    if let Some(swipe) = a.swipe.filter(|s| s.active) {
                        let direction = if swipe.dx < 0. { 1 } else { -1 };
                        let n = a.components.len();
                        if n > 1 {
                            let i = a
                                .components
                                .iter()
                                .position(|c| c == &a.selected)
                                .unwrap_or(0);
                            let target = a.components
                                [(i as i32 + direction).rem_euclid(n as i32) as usize]
                                .clone();
                            if target != a.drag_target {
                                a.drag_panel = a.component_panel(&target);
                                a.drag_target = target;
                            }
                        }
                    }
                    if a.seeking {
                        let s = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
                        a.update_seek((lparam.0 as u16 as i16) as f64 / s);
                    }
                    return LRESULT(0);
                }
                WM_LBUTTONUP => {
                    a.float_drag_arm = None;
                    let dragged = a.dragging.take();
                    let press_origin = a.press_origin.take();
                    let swipe = a.swipe.take();
                    let seeking = std::mem::take(&mut a.seeking);
                    let pressed = std::mem::replace(&mut a.pressed, 0);
                    let same_agent_prompt=a.agent_pressed_token.take()==a.agent.view.prompt.as_ref().map(|p|p.token);
                    let seek_track = std::mem::take(&mut a.seek_track);
                    let _ = ReleaseCapture();
                    if let Some((_, bounds)) = dragged {
                        if a.practice.is_some() {
                            let mut end = RECT::default();
                            if GetWindowRect(a.hwnd, &mut end).is_ok()
                                && (end.left.abs_diff(bounds.left) >= 12 || end.top.abs_diff(bounds.top) >= 12)
                            {
                                a.practice_event(PracticeStep::Move);
                            }
                        } else {
                            a.remember_position();
                        }
                        a.dirty = true;
                        return LRESULT(0);
                    }
                    let s = GetDpiForWindow(hwnd).max(96) as f64 / 96.;
                    let x = (lparam.0 as u16 as i16) as f64 / s;
                    let y = ((lparam.0 >> 16) as u16 as i16) as f64 / s;
                    let hit = a.hit(x, y);
                    if let Some(swipe) = swipe {
                        if a.finish_swipe(swipe) {
                            return LRESULT(0);
                        }
                    }
                    if seeking {
                        if !seek_track.is_empty()
                            && a.snapshot["track"]["id"].as_str() == Some(seek_track.as_str())
                        {
                            a.command(
                                "seek",
                                a.seek_fraction
                                    * a.snapshot["track"]["durationSeconds"]
                                        .as_f64()
                                        .unwrap_or(0.),
                            );
                        }
                    } else if hit == pressed && hit >= 0 && within_click_slop(press_origin, x, y)
                        && (hit < agent_island::COLLAPSE || same_agent_prompt) {
                        a.activate(hit)
                    }
                    a.pressed = 0;
                    a.dirty = true;
                    return LRESULT(0);
                }
                WM_CAPTURECHANGED | WM_CANCELMODE => {
                    a.float_drag_arm = None;
                    a.press_origin = None;
                    if let Some(swipe) = a.swipe.take() {
                        if swipe.active {
                            a.slide.set(swipe.dx);
                            a.slide_outgoing = false;
                            a.switching = a.flag("Animation", true);
                        }
                    }
                    a.drag_target.clear();
                    a.drag_panel = None;
                    a.seeking = false;
                    a.pressed = 0;
                    a.dirty = true;
                    return LRESULT(0);
                }
                WM_APPLY_WINDOW_LEVEL => {
                    if let Err(e) = window_level::apply(
                        a.visual_hwnd,
                        a.hwnd,
                        a.effective_topmost(),
                        false,
                    ) {
                        a.error = Some(e.to_string());
                    }
                    return LRESULT(0);
                }
                WM_DPICHANGED | WM_DISPLAYCHANGE => {
                    if let Err(e) = a.position() {
                        a.error = Some(e.to_string());
                    }
                    return LRESULT(0);
                }
                WM_WTSSESSION_CHANGE => {
                    let mut session = 0u32;
                    let _ = ProcessIdToSessionId(GetCurrentProcessId(), &mut session);
                    if lparam.0 as u32 == session {
                        if wparam.0 as u32 == WTS_SESSION_LOCK {
                            a.pending_osd = 0;
                            if a.osd_banner.visible() {
                                let now = a.start.elapsed().as_secs_f64();
                                let (r, rad) = a.shape();
                                a.osd_banner.dismiss(r, rad, now, false, a.base_needed());
                            }
                            a.on_lock();
                        } else if wparam.0 as u32 == WTS_SESSION_UNLOCK {
                            a.osd.resync();
                            a.on_unlock();
                        }
                    }
                    return LRESULT(0);
                }
                WM_HOTKEY => {
                    if wparam.0 == 1 {
                        a.pending_action = Some("quickReminder".into());
                        a.show_settings_page("overview");
                    }
                    return LRESULT(0);
                }
                WM_SETTINGS => {
                    let requests = REQUESTS
                        .get_or_init(Default::default)
                        .lock()
                        .map(|mut q| std::mem::take(&mut *q))
                        .unwrap_or_default();
                    for (r, tx) in requests {
                        let response = a.request(r);
                        let _ = tx.send(response);
                    }
                    return LRESULT(0);
                }
                WM_TRAY => {
                    if lparam.0 as u32 == WM_RBUTTONUP {
                        let mut p = POINT::default();
                        let _ = GetCursorPos(&mut p);
                        let menu = CreatePopupMenu();
                        if let Ok(menu) = menu {
                            for (id, text) in [
                                (1, w!("展开灵动岛")),
                                (2, w!("显示 / 隐藏灵动岛")),
                                (3, w!("设置")),
                                (5, w!("重启点屿")),
                                (4, w!("退出")),
                            ] {
                                let _ = AppendMenuW(menu, MF_STRING, id, text);
                            }
                            let active = a.flag("NotificationsEnabled", true)
                                && !a.flag("PauseNotifications", false);
                            let label = if !a.flag("NotificationsEnabled", true) {
                                w!("开启通知")
                            } else if a.flag("PauseNotifications", false) {
                                w!("恢复通知（已暂停）")
                            } else {
                                w!("暂停通知（已开启）")
                            };
                            let flags = if active {
                                MF_STRING | MF_CHECKED
                            } else {
                                MF_STRING | MF_UNCHECKED
                            };
                            let _ = InsertMenuW(menu, 3, flags | MF_BYPOSITION, 6, label);
                            let startup_flags = MF_STRING
                                | MF_BYPOSITION
                                | if a.flag("AutoStart", true) {
                                    MF_CHECKED
                                } else {
                                    MF_UNCHECKED
                                };
                            let _ = InsertMenuW(menu, 3, startup_flags, 7, w!("开机启动"));
                            let _ = SetForegroundWindow(hwnd);
                            let selected = TrackPopupMenuEx(
                                menu,
                                (TPM_RETURNCMD | TPM_RIGHTBUTTON).0,
                                p.x,
                                p.y,
                                hwnd,
                                None,
                            )
                            .0;
                            let _ = DestroyMenu(menu);
                            let command = match selected {
                                1 => "expand",
                                2 => "",
                                3 => "settings",
                                5 => "restart",
                                6 => "toggleNotifications",
                                4 => "exit",
                                _ => "",
                            };
                            if selected == 7 {
                                let enabled = !a.flag("AutoStart", true);
                                let result=a.request(json!({"version":1,"command":"update","value":{"key":"AutoStart","value":enabled}}));
                                if result["success"] != true {
                                    a.error = Some(
                                        result["message"]
                                            .as_str()
                                            .unwrap_or("开机启动设置未保存")
                                            .to_owned(),
                                    );
                                }
                            } else if selected == 2 {
                                a.toggle_island_visibility(true);
                            } else if !command.is_empty() {
                                a.request(json!({"version":1,"command":command}));
                            }
                        }
                    } else if lparam.0 as u32 == WM_LBUTTONDBLCLK {
                        a.request(json!({"version":1,"command":"expand"}));
                    }
                    return LRESULT(0);
                }
                WM_CLOSE => {
                    a.widget_data.flush();

                    App::terminate_child(&mut a.center_process);
                    a.bottom_osd.clear();
                    a.osd.uninstall();
                    let _ = a.timers.persist();
                    let _ = a.rules.persist_usage();
                    let _ = WTSUnRegisterSessionNotification(hwnd);
                    let _ = UnregisterHotKey(Some(hwnd), 1);
                    let _ = DestroyWindow(hwnd);
                    return LRESULT(0);
                }
                WM_DESTROY => {
                    let mut icon = NOTIFYICONDATAW {
                        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
                        hWnd: hwnd,
                        uID: 1,
                        ..Default::default()
                    };
                    let _ = Shell_NotifyIconW(NIM_DELETE, &mut icon);
                    if let Some(b) = &a.backend {
                        b.close();
                    }
                    PostQuitMessage(0);
                    return LRESULT(0);
                }
                _ => {}
            }
        }
    }
    DefWindowProcW(hwnd, msg, wparam, lparam)
}
fn font_base() -> PathBuf {
    let exe = std::env::current_exe().unwrap();
    let base = exe.parent().unwrap();
    if base.join("Assets/Fonts/NotoSansSC.ttf").exists() {
        return base.into();
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
unsafe fn create(a: &mut App, owner: *const RefCell<App>, hidden: bool) -> Result<()> {
    shell::ensure_per_monitor_dpi_awareness();
    CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
    a.transparent = windows::UI::ViewManagement::UISettings::new()
        .and_then(|s| s.AdvancedEffectsEnabled())
        .unwrap_or(false);
    let instance = GetModuleHandleW(None)?;
    let class = w!("LyricIsland.Native.DirectComposition.v1");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(wndproc),
        hInstance: instance.into(),
        lpszClassName: class,
        hCursor: LoadCursorW(None, IDC_ARROW)?,
        ..Default::default()
    };
    RegisterClassW(&wc);
    let visual_class = w!("LyricIsland.Native.Visual.v1");
    let visual_wc = WNDCLASSW {
        lpfnWndProc: Some(visual_wndproc),
        hInstance: instance.into(),
        lpszClassName: visual_class,
        ..Default::default()
    };
    RegisterClassW(&visual_wc);
    let level = window_level::creation_style(a.effective_topmost());
    let visual = CreateWindowExW(
        WS_EX_TOOLWINDOW
            | WS_EX_NOACTIVATE
            | WS_EX_LAYERED
            | WS_EX_TRANSPARENT
            | WS_EX_NOREDIRECTIONBITMAP
            | level,
        visual_class,
        w!("DotIsle · Visual"),
        WS_POPUP,
        0,
        0,
        388,
        264,
        None,
        None,
        Some(instance.into()),
        None,
    )?;
    a.visual_hwnd = visual;
    SetLayeredWindowAttributes(visual, COLORREF(0), 255, LWA_ALPHA)?;
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_NOREDIRECTIONBITMAP | level,
        class,
        w!("DotIsle · Native"),
        WS_POPUP,
        0,
        0,
        388,
        264,
        Some(visual),
        None,
        Some(instance.into()),
        Some(owner as _),
    )?;
    a.hwnd = hwnd;
    a.devices.register(hwnd);
    a.apply_osd();
    let _ = WINDOW.set(hwnd.0 as isize);
    let _ = WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION);
    if !a.demo {
        if let Err(e) = a.register_hotkey() {
            a.error = Some(e);
        }
    }
    a.rules
        .baseline_now(a.foreground.path.clone().as_deref(), a.foreground.segment);
    a.poll_island_visibility(true)?;
    a.position()?;
    // Repair a stale Run entry: the launcher may have moved or a previous version registered
    // its extracted runtime path, which dies on the next update.
    if !a.demo
        && !std::env::var("LYRICISLAND_TEST_PIPE")
            .is_ok_and(|p| p.starts_with("LyricIsland.Settings.test."))
        && a.flag("AutoStart", true)
    {
        let _ = a.apply_startup();
    }
    if a.island_suppressed() {
        return Ok(());
    }
    if !hidden {
        window_level::apply(visual, hwnd, a.effective_topmost(), true)?;
        // Reapply after initialization yields to the first message-pump turn, too.
        PostMessageW(Some(hwnd), WM_APPLY_WINDOW_LEVEL, WPARAM(0), LPARAM(0))?;
    } else if a.taskbar_always_show() {
        a.display_wanted = true;
        a.opacity = 1.;
        a.fade_to = 1.;
        window_level::apply(visual, hwnd, true, true)?;
        PostMessageW(Some(hwnd), WM_APPLY_WINDOW_LEVEL, WPARAM(0), LPARAM(0))?;
    }
    Ok(())
}
unsafe fn tray(a: &App) -> Result<()> {
    let path = a
        .base
        .join("Assets/LyricIsland-v3.ico")
        .to_string_lossy()
        .encode_utf16()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let icon = LoadImageW(
        None,
        PCWSTR(path.as_ptr()),
        IMAGE_ICON,
        0,
        0,
        LR_LOADFROMFILE | LR_DEFAULTSIZE,
    )
    .ok()
    .map(|i| HICON(i.0))
    .unwrap_or(LoadIconW(None, IDI_APPLICATION)?);
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: a.hwnd,
        uID: 1,
        uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
        uCallbackMessage: WM_TRAY,
        hIcon: icon,
        ..Default::default()
    };
    let tip = format!("DotIsle · 点屿 {}", DISPLAY_VERSION)
        .encode_utf16()
        .collect::<Vec<_>>();
    let tip = &tip[..tip.len().min(127)];
    nid.szTip[..tip.len()].copy_from_slice(&tip);
    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    Ok(())
}
fn traces(directory: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    let mut cases = vec![];
    let tag = "Fluid";
    for damping in [10., 30., 2. * (420_f64 * 0.8).sqrt(), 80.] {
        let mut o = Options::default();
        o.spring.damping = damping;
        for expanded in [true, false] {
            let t = Transition::new(
                expanded,
                if expanded {
                    Frame::hidden(o)
                } else {
                    Frame::expanded()
                },
                o,
                Frame::default(),
                false,
            );
            for ms in (0..=1400).step_by(5) {
                cases.push(json!({"style":tag,"damping":damping,"expanded":expanded,"time":ms,"sample":t.sample(ms as f64),"velocity":t.velocity(ms as f64)}));
            }
            for ms in [40., 90., 180., 320.] {
                let rev = Transition::new(!expanded, t.sample(ms), o, t.velocity(ms), false);
                for after in [0., 1., 20., 80., 180., 400.] {
                    cases.push(json!({"style":tag,"damping":damping,"expanded":!expanded,"reverseAt":ms,"time":after,"sample":rev.sample(after),"velocity":rev.velocity(after)}));
                }
            }
        }
    }
    std::fs::write(
        directory.join("native-motion.json"),
        serde_json::to_vec(&cases)?,
    )?;
    Ok(())
}
unsafe fn preview(
    a: &mut App,
    directory: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    a.renderer = None;
    a.renderer = Some(Renderer::new(
        a.visual_hwnd,
        a.layout.host.w as u32,
        a.layout.host.h as u32,
        96.,
        &a.base.join("Assets/Fonts/NotoSansSC.ttf"),
    )?);
    a.poll();
    a.opacity = 1.;
    let o = a.options();
    for (name, expanded, count) in [("opening", true, 48), ("closing", false, 44)] {
        let t = Transition::new(
            expanded,
            if expanded {
                Frame::hidden(o)
            } else {
                Frame::expanded()
            },
            o,
            Frame::default(),
            false,
        );
        a.transition = Some(t);
        a.expanded = expanded;
        for i in 0..count {
            let ms = i as f64 * 20.;
            if ms > t.duration_ms + 20. {
                break;
            }
            a.frame = t.sample(ms);
            let mut renderer = a.renderer.take().unwrap();
            let mut scene = a.scene();
            scene.time_ms = 0.;
            scene.lyric_age_ms = 1000.;
            scene.cover_opacity = 1.;
            scene.progress = 83. / 236.;
            let mut activity = motion::Activity::new();
            activity.retarget(true, 0., true);
            scene.activity_scales = activity.sample(330. + ms, true).0;
            scene.activity_opacity = 0.9;
            renderer.render(&scene, false)?;
            let file = format!("{name}-fluid-{:03}ms.png", i * 20);
            renderer.snapshot(&directory.join(file))?;
            a.renderer = Some(renderer);
        }
    }
    traces(directory)?;
    Ok(())
}
unsafe fn wellness_preview(a: &mut App, directory: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    use reminder_plugins::Kind::{Stand, Water};
    std::fs::create_dir_all(directory)?;
    a.opacity = 1.;
    a.layout = Layout::new(304.);
    a.layout.set_compact(304., 40.);
    a.layout.host.h = 220.;
    let dpi = 144.;
    let scale = dpi / 96.;
    let mut renderer = a.renderer.take().ok_or("missing preview renderer")?;
    renderer.resize((a.layout.host.w * scale) as u32, (a.layout.host.h * scale) as u32, dpi as f32);
    use capsule_alert::Alert;
    let previews = [
        (Alert::wellness(Water).with_hydration(reminder_plugins::Hydration{goal_ml:2000,serving_ml:250,drank_ml:750}), "hydration"),
        (Alert::wellness(Stand), "sedentary"),
        (Alert::quota("preview", "Command Code", [Some(12.), None], [20, 10]), "quota-low"),
        (Alert::relay_balance("relay-preview", "New API 中转站", ai_quota::RelayBalance {amount:0.72,currency:ai_quota::RelayUnit::Usd,key:true}), "relay-balance"),
        (Alert::relay_balance("relay-preview-cny", "Sub2API 中转站", ai_quota::RelayBalance {amount:0.68,currency:ai_quota::RelayUnit::Cny,key:false}), "relay-balance-cny"),
    ];
    for (alert, name) in previews {
        a.alerts = capsule_alert::Hub::default();
        a.alerts.enqueue(alert);
        let (base, radius) = a.shape();
        let now = a.start.elapsed().as_secs_f64();
        let _ = a.alerts.begin_at(base, radius, a.layout.host, base.y, now - 0.75, false, false);
        let mut scene = a.scene();
        scene.layout_scale = scale;
        scene.animate = false;
        renderer.render(&scene, false)?;
        renderer.snapshot(&directory.join(format!("{name}.png")))?;
    }
    Ok(())
}

unsafe fn agent_preview(a: &mut App, directory: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> { unsafe {
    use agent_island::{Agent, Question, Prompt, Tile};
    std::fs::create_dir_all(directory)?;
    a.renderer=None;
    a.opacity=1.;
    a.selected=agent_island::ID.into();
    a.panel=Some(activities::Panel{kind:agent_island::KIND,..Default::default()});
    a.agent.view.tiles=vec![
        Tile{agent:Agent::Codex,hour:Some(18.),week:Some(59.),wallet:vec![],token_quota:None},
        Tile{agent:Agent::Claude,hour:Some(77.),week:None,wallet:vec![],token_quota:None},
        Tile{agent:Agent::ZCode,hour:None,week:None,wallet:vec![],token_quota:Some(ai_quota::TokenQuota { remaining: 1_200_000, total: 2_000_000 })},
        Tile{agent:Agent::Cursor,hour:None,week:Some(61.),wallet:vec![],token_quota:None},
        Tile{agent:Agent::Harness,hour:None,week:None,wallet:vec!["¥32.80".into(),"$4.20".into()],token_quota:None},
    ];
    a.layout=Layout::new(400.);
    a.layout.reserve_compact(a.agent.view.compact_width());
    a.layout.set_compact(a.agent.view.compact_width(),40.);
    a.layout.host.h=400.;
    let dpi=144.; let scale=dpi/96.;
    let mut renderer=Renderer::new(a.visual_hwnd,(a.layout.host.w*scale) as u32,(a.layout.host.h*scale) as u32,dpi as f32,&a.base.join("Assets/Fonts/NotoSansSC.ttf"))?;
    for (name, compact, zcode_hour) in [
        ("agent-compact", true, None),
        ("agent-quota", false, None),
        ("zcode-window-compact", true, Some(0.)),
        ("zcode-window-quota", false, Some(0.)),
        ("agent-reply", false, None),
    ] {
        if let Some(tile) = a.agent.view.tiles.iter_mut().find(|tile| tile.agent == Agent::ZCode) {
            tile.hour = zcode_hour;
        }
        if name=="agent-reply" {
            a.agent.view.set_prompt(Some(Prompt{token:0,questions:vec![Question{id:"qa".into(),text:"你希望先检查额度数据，还是先验证灵动岛布局？".into(),labels:vec!["先检查额度数据".into(),"先验证灵动岛布局".into()],custom:true,multi:false}]}));
            a.agent.view.choose(1);
        }
        a.layout.card.h=a.agent.view.height();
        let mut scene=a.scene();
        scene.frame = if compact { Frame::hidden(a.options()) } else { Frame::expanded() };
        scene.layout_scale=scale;
        scene.opacity=1.;
        scene.animate=false;
        renderer.render(&scene,false)?;
        renderer.snapshot(&directory.join(format!("{name}.png")))?;
    }
    Ok(())
}}
unsafe fn tools_preview(
    a: &mut App,
    directory: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    a.opacity = 1.;
    a.frame = Frame::expanded();
    a.lyric = "于是我拼命奔跑".into();
    let todo = activities::Panel {
        kind: 2,
        title: "待办清单".into(),
        value: "3 项待办".into(),
        accent: 0xfff3d79b,
        tasks: [
            ("复盘今天的工作", "2026-10-17 08:30", 0xfff3d79b),
            ("整理桌面与备份文件", "2026-10-17 09:30", 0xffb7dbcb),
            ("背单词 20 分钟", "2026-10-17 10:30 · 每天", 0xffc9bce5),
        ]
        .iter()
        .map(|(t, d, c)| activities::Task {
            title: (*t).into(),
            detail: (*d).into(),
            color: *c,
            ..Default::default()
        })
        .collect(),
        ..Default::default()
    };
    let timer = activities::Panel {
        kind: 1,
        title: "番茄钟".into(),
        value: "24:38".into(),
        subtitle: "专注 · 第 1 / 4 轮".into(),
        progress: 0.16,
        running: true,
        accent: 0xffe7a185,
        ..Default::default()
    };
    for dpi in [96., 144., 192.] {
        let scale = dpi / 96.;
        a.renderer = None;
        let mut renderer = Renderer::new(
            a.visual_hwnd,
            (a.layout.host.w * scale) as u32,
            (a.layout.host.h * scale) as u32,
            dpi as f32,
            &a.base.join("Assets/Fonts/NotoSansSC.ttf"),
        )?;
        for (name, panel) in [
            ("music", None),
            ("todos", Some(&todo)),
            ("timer", Some(&timer)),
        ] {
            let mut scene = a.scene();
            scene.panel = panel;
            scene.layout.card.h = a.card_height();
            scene.component_count = 3;
            scene.previous_panel = None;
            scene.switching = false;
            scene.notification = None;
            scene.layout_scale = scale;
            scene.lyric_age_ms = 1000.;
            scene.progress = 0.37;
            scene.secondary = "等待着夜晚的再次来临";
            scene.time_ms = 0.;
            renderer.render(&scene, false)?;
            renderer.snapshot(&directory.join(format!("{name}-{dpi:.0}.png")))?;
        }
        {
            let mut scene = a.scene();
            scene.panel = Some(&timer);
            scene.frame = Frame::hidden(a.options());
            scene.component_count = 3;
            scene.layout_scale = scale;
            scene.opacity = 1.;
            renderer.render(&scene, false)?;
            renderer.snapshot(&directory.join(format!("capsule-{dpi:.0}.png")))?;
        }
        if dpi == 144. {
            let mut spring = activities::Spring::default();
            spring.release(a.layout.card.w - 70., -650.);
            for i in 0..30 {
                let mut scene = a.scene();
                scene.panel = Some(&timer);
                scene.previous_panel = Some(&todo);
                scene.switching = true;
                scene.switch_outgoing = true;
                scene.switch_direction = 1.;
                scene.switch_offset = spring.value;
                scene.layout_scale = scale;
                scene.component_count = 3;
                renderer.render(&scene, false)?;
                renderer.snapshot(&directory.join(format!("switch-{i:02}.png")))?;
                spring.tick(1. / 60.);
            }
        }
        a.renderer = Some(renderer);
    }
    Ok(())
}
unsafe fn notification_preview(
    a: &mut App,
    directory: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    a.frame = Frame::hidden(a.options());
    a.opacity = 1.;
    a.lyric = "景色は移ろい瞬く間に変わる".into();
    for dpi in [96., 120., 144., 192.] {
        a.renderer = None;
        let scale = dpi / 96.;
        let mut renderer = Renderer::new(
            a.visual_hwnd,
            (a.layout.host.w * scale) as u32,
            (a.layout.host.h * scale) as u32,
            dpi as f32,
            &a.base.join("Assets/Fonts/NotoSansSC.ttf"),
        )?;
        for amount in [0., 0.5, 1.] {
            let mut s = a.scene();
            s.notification = None;
            s.notification_mix = 0.;
            s.layout_scale = scale;
            s.lyric_age_ms = 1000.;
            s.highlight = amount;
            renderer.render(&s, false)?;
            renderer.snapshot(
                &directory.join(format!("font-{dpi:.0}-{}.png", (amount * 100.) as i32)),
            )?;
        }
        {
            let mut s = a.scene();
            s.notification = None;
            s.notification_mix = 0.;
            s.frame = Frame::expanded();
            s.layout_scale = scale;
            s.lyric_age_ms = 1000.;
            renderer.render(&s, false)?;
            renderer.snapshot(&directory.join(format!("toolbar-{dpi:.0}.png")))?;
        }
        let notice = notifications::Notice::example(&a.base);
        renderer.set_notification_icon(notice.icon.as_deref())?;
        a.banner = banner::Banner::default();
        let (base, radius) = a.shape();
        let height = renderer.notification_height(&notice, 440_f64.min(a.layout.host.w - 48.))?;
        a.banner
            .show(notice, base, radius, a.layout.host.w, height, 0., true);
        for ms in [0., 40., 80., 120., 200., 320., 640.] {
            let (r, rad, mix) = a.banner.presentation(ms / 1000.);
            let mut s = a.scene();
            s.notification_rect = r;
            s.notification_radius = rad;
            s.notification_mix = mix;
            s.layout_scale = scale;
            s.lyric_age_ms = 1000.;
            renderer.render(&s, false)?;
            renderer.snapshot(&directory.join(format!("notification-{dpi:.0}-{ms:03.0}.png")))?;
        }
        a.banner.dismiss(base, radius, 0.64, true, a.base_needed());
        for ms in [640., 680., 720., 800., 960., 1280.] {
            let (r, rad, mix) = a.banner.presentation(ms / 1000.);
            let mut s = a.scene();
            s.notification_rect = r;
            s.notification_radius = rad;
            s.notification_mix = mix;
            s.layout_scale = scale;
            s.lyric_age_ms = 1000.;
            renderer.render(&s, false)?;
            renderer.snapshot(&directory.join(format!("dismiss-{dpi:.0}-{ms:04.0}.png")))?;
        }
        for (name,body) in [("short","这是一条简短消息。"),("empty",""),("long","这是一条用于检查多行通知排版的长消息。收到新消息后，点屿会根据正文内容调整横幅高度，保留清晰的文字间距，并在显示结束后平滑返回音乐界面。")]{
            let mut notice=notifications::Notice::example(&a.base);notice.body=body.into();
            if let Some(path)=std::env::var_os("LYRICISLAND_PREVIEW_ICON"){notice.icon=std::fs::read(path).ok();notice.app_name="QQ".into();}
            renderer.set_notification_icon(notice.icon.as_deref())?;
            let height=renderer.notification_height(&notice,440.)?;
            a.banner.show(notice,base,radius,a.layout.host.w,height,0.,false);
            let (r,rad,mix)=a.banner.presentation(0.);let mut scene=a.scene();scene.notification_rect=r;scene.notification_radius=rad;scene.notification_mix=mix;scene.layout_scale=scale;
            renderer.render(&scene,false)?;renderer.snapshot(&directory.join(format!("notification-{name}-{dpi:.0}.png")))?;
            std::fs::write(directory.join(format!("height-{name}-{dpi:.0}.json")),serde_json::to_vec(&json!({"height":height,"dpi":dpi}))?)?;
        }
        a.renderer = Some(renderer);
    }
    Ok(())
}
unsafe fn icon_preview(
    a: &mut App,
    directory: &Path,
) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    a.poll();
    a.opacity = 1.;
    a.frame = Frame::expanded();
    a.expanded = true;
    a.renderer = None;
    for dpi in [96., 120., 144., 192.] {
        let scale = dpi / 96.;
        let mut renderer = Renderer::new(
            a.visual_hwnd,
            (a.layout.host.w * scale) as u32,
            (a.layout.host.h * scale) as u32,
            dpi as f32,
            &a.base.join("Assets/Fonts/NotoSansSC.ttf"),
        )?;
        let mut scene = a.scene();
        scene.layout_scale = scale;
        scene.lyric_age_ms = 1000.;
        renderer.render(&scene, false)?;
        renderer.snapshot(&directory.join(format!("icons-{dpi:.0}.png")))?;
        std::fs::write(
            directory.join(format!("icons-{dpi:.0}.json")),
            serde_json::to_vec(&renderer.icon_bounds()?)?,
        )?;
    }
    Ok(())
}
mod consent;
fn run() -> std::result::Result<(), Box<dyn std::error::Error>> {
    unsafe {
        let args = std::env::args().collect::<Vec<_>>();
        if let Some(i) = args.iter().position(|s| s == "--test-pipe") {
            if let Some(pipe) = args.get(i + 1) {
                std::env::set_var("LYRICISLAND_TEST_PIPE", pipe);
            }
        }

        if let Some(i) = args.iter().position(|s| s == "--restart-after") {
            let pid = args
                .get(i + 1)
                .and_then(|s| s.parse::<u32>().ok())
                .ok_or("invalid restart PID")?;
            if let Ok(parent) = OpenProcess(PROCESS_ACCESS_RIGHTS(0x00100000), false, pid) {
                let result = WaitForSingleObject(parent, 15000);
                let _ = CloseHandle(parent);
                if result != WAIT_OBJECT_0 {
                    return Err("等待旧实例退出超时".into());
                }
            }
        }
        if args.get(1).is_some_and(|s| s == "--check-update") {
            let report = lyric_backend::updates::check_channel(args.get(3).map(String::as_str).unwrap_or("stable"))
                .unwrap_or_else(|e| json!({"success":false,"message":e}));
            std::fs::write(
                args.get(2).ok_or("missing report")?,
                serde_json::to_vec_pretty(&report)?,
            )?;
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--app-icon") {
            let id = args.get(2).ok_or("missing app id")?;
            let path = args.get(3).ok_or("missing output path")?;
            std::thread::spawn({
                let id = id.clone();
                let path = path.clone();
                move || -> std::result::Result<(), String> {
                    let _ = windows::Win32::System::WinRT::RoInitialize(
                        windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                    );
                    let bytes =
                        app_icon::shell_icon(&id).ok_or("Windows application icon unavailable")?;
                    std::fs::write(path, bytes).map_err(|e| e.to_string())
                }
            })
            .join()
            .map_err(|_| "icon worker failed")??;
            return Ok(());
        }

        if args.get(1).is_some_and(|s| s == "--activation-check") {
            let title = w!("LyricIsland activation test");
            let h = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("STATIC"),
                title,
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                0,
                0,
                200,
                100,
                None,
                None,
                None,
                None,
            )?;
            let _ = ShowWindow(h, SW_MINIMIZE);
            let result = activation::open(&std::env::current_exe()?.to_string_lossy());
            let success = result.is_ok() && !IsIconic(h).as_bool() && GetForegroundWindow() == h;
            std::fs::write(
                args.get(2).ok_or("missing report")?,
                serde_json::to_vec_pretty(
                    &json!({"success":success,"restoredMinimizedWindow":!IsIconic(h).as_bool(),"error":result.err()}),
                )?,
            )?;
            let _ = DestroyWindow(h);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--notification-info") {
            let report = std::thread::spawn(|| {
                let _ = windows::Win32::System::WinRT::RoInitialize(
                    windows::Win32::System::WinRT::RO_INIT_MULTITHREADED,
                );
                notifications::diagnostics()
            })
            .join()
            .map_err(|_| "notification diagnostic failed")?;
            std::fs::write(
                args.get(2).ok_or("missing report path")?,
                serde_json::to_vec_pretty(&report)?,
            )?;
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--motion-trace") {
            return traces(Path::new(args.get(2).ok_or("missing directory")?));
        }
        if args.get(1).is_some_and(|s| s == "--media-info") {
            let data = std::thread::spawn(|| {
                lyric_backend::media::control_diagnostics().map_err(|e| e.to_string())
            })
            .join()
            .map_err(|_| "media diagnostics worker failed")??;
            std::fs::write(
                args.get(2).ok_or("missing output path")?,
                serde_json::to_vec_pretty(&data)?,
            )?;
            return Ok(());
        }
        let icon_mode = args.get(1).is_some_and(|s| s == "--icon-preview");
        let font_mode = args.get(1).is_some_and(|s| s == "--font-check");
        let notice_mode = args.get(1).is_some_and(|s| s == "--notification-preview");
        let cover_mode = args.get(1).is_some_and(|s| s == "--cover-preview");
        let tool_mode = args.get(1).is_some_and(|s| s == "--tools-preview");
        let preview_mode = args.get(1).is_some_and(|s| {
            s == "--glow-preview"
                || s == "--liquid-preview"
                || s == "--resident-preview"
                || s == "--startup-ui-preview"
                || s == "--motion-capture"
                || s == "--performance-check"
                || s == "--immersive-preview"
                || s == "--m3e-preview"
                || s == "--cover-radius-preview"
                || s == "--material-surfaces-preview"
                || s == "--expanded-material-preview"
                || s == "--lightfield-preview"
                || s == "--acrylic-cover-preview"
                || s == "--liquid-glass-preview"
                || s == "--upgrade-preview"
                || s == "--mini-preview"
                || s == "--integration-preview"
                || s == "--agent-preview"
                || s == "--wellness-preview"
        }) || tool_mode
            || cover_mode
            || icon_mode
            || font_mode
            || notice_mode
            || args.get(1).is_some_and(|s| s == "--animation-preview");
        let demo = preview_mode || args.iter().any(|s| s == "--demo");
        // One island per desktop, so a second copy must not start. It used to exit without a word,
        // which made a freshly built portable copy look broken: double-clicking it did nothing at all
        // while the installed version answered every click. The mutex is taken before any window is
        // created so the refused copy never flashes an island on its way out.
        let test_pipe = std::env::var("LYRICISLAND_TEST_PIPE").unwrap_or_default();
        let mutex_name = if test_pipe.starts_with("LyricIsland.Settings.test.") {
            format!("Local.LyricIsland.Native.Test.{test_pipe}")
        } else {
            "Local.LyricIsland.Native.Vello.v3".into()
        };
        let mutex_name = mutex_name.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
        let mutex = CreateMutexW(None, true, PCWSTR(mutex_name.as_ptr()))?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(mutex);
            // Not while logging in: a dialog nobody asked for, before the desktop has settled, is
            // worse than staying quiet.
            if !preview_mode && !args.iter().any(|s| s == "--startup") {
                let current = io::pipe_request(
                    &std::env::var("LYRICISLAND_TEST_PIPE")
                        .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into()),
                    &json!({"version":1,"command":"state"}),
                )
                .unwrap_or_default();
                let message=format!("已有点屿 {} 正在运行。\r\n你正在启动的是此目录的 {}，不会跳转或改用安装版。\r\n\r\n请先从托盘退出正在运行的点屿，再启动这一份。",current["version"].as_str().unwrap_or("（版本未知）"),DISPLAY_VERSION);
                let text = message.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
                let title = format!("点屿 · DotIsle {}", DISPLAY_VERSION)
                    .encode_utf16()
                    .chain(Some(0))
                    .collect::<Vec<_>>();
                MessageBoxW(
                    None,
                    PCWSTR(text.as_ptr()),
                    PCWSTR(title.as_ptr()),
                    MB_OK | MB_ICONINFORMATION | MB_SETFOREGROUND,
                );
            }
            return Ok(());
        }
        let base = font_base();
        let data = if let Some(i) = args.iter().position(|s| s == "--data-dir") {
            PathBuf::from(args.get(i + 1).ok_or("missing data directory")?)
        } else if !base.join("Portable.json").is_file() && base.join("Installed.json").exists() {
            let marker = io::read_settings(&base.join("Installed.json"));
            marker["dataDirectory"]
                .as_str()
                .map(PathBuf::from)
                .unwrap_or_else(|| io::data_directory())
        } else {
            base.join("Data")
        };
        if !demo && !consent::ensure(&data)? {return Ok(())}
        if !demo
            && std::env::var_os("LYRICISLAND_TEST_PIPE").is_none()
            && !base.join("Portable.json").is_file()
            && base.join("Installed.json").is_file()
            && base.join("Uninstall.exe").is_file()
        {
            let _ = std::process::Command::new(base.join("Uninstall.exe"))
                .creation_flags(0x08000000)
                .arg("--repair-if-needed")
                .arg(&base)
                .spawn();
        }
        let owner = Box::new(RefCell::new(App::new(base, data, demo)));
        let owner_ptr = &*owner as *const RefCell<App>;
        let mut app = owner.borrow_mut();
        create(&mut app, owner_ptr, preview_mode)?;
        if args.get(1).is_some_and(|s| s == "--glow-preview") {
            glow_preview::run(
                &mut app,
                Path::new(args.get(2).ok_or("missing glow preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--liquid-preview") {
            liquid_preview::run(
                &mut app,
                Path::new(args.get(2).ok_or("missing liquid preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--wellness-preview") {
            wellness_preview(&mut app, Path::new(args.get(2).ok_or("missing wellness capture directory")?))?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--agent-preview") {
            agent_preview(&mut app,Path::new(args.get(2).ok_or("missing agent capture directory")?))?;
            let _=DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--startup-ui-preview") {
            startup_preview::run(
                &mut app,
                Path::new(args.get(2).ok_or("missing startup capture directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--immersive-preview") {
            immersive_preview::run(
                &mut app,
                Path::new(args.get(2).ok_or("missing immersive directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--motion-capture") {
            motion_preview::run(
                &mut app,
                Path::new(args.get(2).ok_or("missing motion directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--performance-check") {
            m3e_preview::benchmark(
                &mut app,
                Path::new(args.get(2).ok_or("missing performance directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--cover-radius-preview") {
            material_preview::cover_radii(
                &mut app,
                Path::new(args.get(2).ok_or("missing radius preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--lightfield-preview") {
            material_preview::lightfield(
                &mut app,
                Path::new(args.get(2).ok_or("missing lightfield preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--acrylic-cover-preview") {
            material_preview::acrylic_cover(
                &mut app,
                Path::new(args.get(2).ok_or("missing acrylic cover preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--liquid-glass-preview") {
            material_preview::liquid_glass_beta(
                &mut app,
                Path::new(args.get(2).ok_or("missing liquid glass preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--upgrade-preview") {
            material_preview::upgrade(
                &mut app,
                Path::new(args.get(2).ok_or("missing upgrade preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args
            .get(1)
            .is_some_and(|s| s == "--material-surfaces-preview")
        {
            material_preview::surfaces(
                &mut app,
                Path::new(args.get(2).ok_or("missing preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--expanded-material-preview") {
            let dir = Path::new(args.get(2).ok_or("missing preview directory")?);
            std::fs::create_dir_all(dir)?;
            app.opacity = 1.;
            app.frame = Frame::expanded();
            app.lyric = "晚风轻轻，音乐刚好".into();
            let mut r = app.renderer.take().unwrap();
            r.set_cover(Some(&std::fs::read(app.base.join("Assets/DemoCover.png"))?))?;
            for mini in [false, true] {
                app.settings["IslandSizeMode"] = json!(if mini { "Mini" } else { "Classic" });
                app.layout.card.w = if mini { 420. } else { 400. };
                app.layout.card.x = (app.layout.host.w - app.layout.card.w) / 2.;
                for material in ["LightField", "Acrylic"] {
                    for tool in [false, true] {
                        app.settings["IslandMaterial"] = json!(material);
                        app.settings["MiniExpandedMaterial"] = json!(material);
                        app.layout.card.h = if mini {
                            if tool {
                                110.
                            } else {
                                120.
                            }
                        } else {
                            220.
                        };
                        let mut store = timers::Store::new(PathBuf::new());
                        let id = store
                            .create(&json!({"mode":"countdown","duration":300}))
                            .unwrap();
                        let panel = store.items.iter().find(|t| t.id == id).unwrap().panel();
                        let mut scene = app.scene();
                        scene.panel = if tool { Some(&panel) } else { None };
                        scene.title = "晚风电台";
                        scene.artist = "DotIsle";
                        scene.elapsed = 83.;
                        scene.duration = 236.;
                        scene.progress = 0.35;
                        scene.lyric_age_ms = 1000.;
                        r.render(&scene, false)?;
                        r.snapshot(&dir.join(format!(
                            "{}-{}-{}.png",
                            if mini { "mini" } else { "classic" },
                            material,
                            if tool { "timer" } else { "music" }
                        )))?;
                    }
                }
            }
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--mini-preview") {
            let dir = Path::new(args.get(2).ok_or("missing preview directory")?);
            std::fs::create_dir_all(dir)?;
            app.opacity = 1.;
            app.lyric = "晚风轻轻，音乐刚好".into();
            app.settings["IslandSizeMode"] = json!("Mini");
            app.settings["SettingsTheme"] = json!("Light");
            app.renderer = None;
            for scale in [1., 1.5, 2.] {
                let mut r = Renderer::new(
                    app.visual_hwnd,
                    (app.layout.host.w * scale) as u32,
                    (app.layout.host.h * scale) as u32,
                    (96. * scale) as f32,
                    &app.base.join("Assets/Fonts/NotoSansSC.ttf"),
                )?;
                r.set_cover(Some(&std::fs::read(app.base.join("Assets/DemoCover.png"))?))?;
                app.layout.set_compact(276., 14.);
                app.layout.card.w = 400.;
                app.layout.card.x = (app.layout.host.w - app.layout.card.w) / 2.;
                app.layout.card.h = app.card_height();
                app.frame = Frame::hidden(app.options());
                let mut s = app.scene();
                s.layout_scale = scale;
                s.time_ms = 900.;
                s.lyric_age_ms = 1000.;
                r.render(&s, false)?;
                r.snapshot(&dir.join(format!("mini-capsule-{scale}.png")))?;
                app.frame = Frame::expanded();
                let mut s = app.scene();
                s.layout_scale = scale;
                s.title = "晚风电台";
                s.artist = "DotIsle";
                s.elapsed = 83.;
                s.duration = 236.;
                s.progress = 0.35;
                s.component_count = 3;
                r.render(&s, false)?;
                r.snapshot(&dir.join(format!("mini-music-{scale}.png")))?;
                let mut store = timers::Store::new(PathBuf::new());
                for (name, v) in [
                    ("timer", json!({"mode":"countdown","duration":300})),
                    ("stopwatch", json!({"mode":"stopwatch"})),
                    ("pomodoro", json!({"mode":"pomodoro","focus":1500})),
                    (
                        "workflow",
                        json!({"mode":"workflow","stages":[{"name":"专注","seconds":1500},{"name":"休息","seconds":300}]}),
                    ),
                ] {
                    let id = store.create(&v).unwrap();
                    let p = store.items.iter().find(|t| t.id == id).unwrap().panel();
                    app.layout.card.h = app.card_height();
                    let mut s = app.scene();
                    s.panel = Some(&p);
                    s.layout_scale = scale;
                    s.component_count = 3;
                    r.render(&s, false)?;
                    r.snapshot(&dir.join(format!("mini-{name}-{scale}.png")))?;
                }
                let p = activities::Panel {
                    kind: 2,
                    title: "今天的待办".into(),
                    tasks: vec![
                        activities::Task {
                            id: "a".into(),
                            title: "整理今天的想法".into(),
                            color: 0xff6ea8ff,
                            ..Default::default()
                        },
                        activities::Task {
                            id: "b".into(),
                            title: "让工作告一段落".into(),
                            color: 0xff9b8bd7,
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                };
                let mut s = app.scene();
                s.panel = Some(&p);
                s.component_count = 3;
                r.render(&s, false)?;
                r.snapshot(&dir.join(format!("mini-todos-{scale}.png")))?;
                app.layout.card.h = app.card_height();
                app.updates.state = json!({"phase":"downloading","version":"演示","percent":43,"receivedBytes":4300000,"totalBytes":10000000});
                let p = app.updates.panel();
                let mut s = app.scene();
                s.panel = Some(&p);
                s.component_count = 3;
                r.render(&s, false)?;
                r.snapshot(&dir.join(format!("mini-update-{scale}.png")))?;
            }
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if cover_mode {
            // Capture the actual text layers with both surface tint switches disabled.
            let dir = Path::new(args.get(2).ok_or("missing directory")?);
            std::fs::create_dir_all(dir)?;
            app.opacity = 1.;
            app.lyric = "晚风轻轻，音乐刚好".into();
            let mut r = app.renderer.take().unwrap();
            for cover in ["red", "blue", "missing"] {
                let bytes = if cover == "missing" {
                    None
                } else {
                    Some(std::fs::read(dir.join(format!("{cover}.png")))?)
                };
                r.set_cover(bytes.as_deref())?;
                for mini in [false, true] {
                    for expanded in [false, true] {
                        for enabled in [false, true] {
                            let mut scene = app.scene();
                            scene.mini = mini;
                            scene.frame = if expanded {
                                Frame::expanded()
                            } else {
                                Frame::hidden(app.options())
                            };
                            scene.material = renderer::Material::Acrylic;
                            scene.classic_capsule = renderer::Material::Acrylic;
                            scene.mini_capsule = renderer::Material::Acrylic;
                            scene.cover_background = false;
                            scene.expanded_cover_background = false;
                            scene.cover_overlay = false;
                            scene.lyric_cover_color = enabled;
                            scene.layout.set_compact(276., if mini { 14. } else { 40. });
                            if mini {
                                scene.layout.card.h = scene.layout.card.h * 0.5 + 10.;
                            }
                            scene.highlight = 0.65;
                            scene.progress_color = 0xffa9e6cd;
                            scene.lyric_age_ms = 1000.;
                            scene.cover_age_ms = 1000.;
                            scene.animate = false;
                            scene.time_ms = 0.;
                            r.render(&scene, false)?;
                            r.snapshot(
                                &dir.join(format!("lyric-{cover}-{mini}-{expanded}-{enabled}.png")),
                            )?;
                        }
                    }
                }
            }
            app.renderer = Some(r);
            let dir = Path::new(args.get(2).ok_or("missing directory")?);
            std::fs::create_dir_all(dir)?;
            app.poll();
            app.opacity = 1.;
            app.frame = Frame::expanded();
            let mut r = app.renderer.take().unwrap();
            for name in ["red", "blue", "white"] {
                r.set_cover(Some(&std::fs::read(dir.join(format!("{name}.png")))?))?;
                for enabled in [false, true] {
                    for (label, material) in [
                        ("classic", renderer::Material::Acrylic),
                        ("acrylic", renderer::Material::Acrylic),
                    ] {
                        let mut scene = app.scene();
                        scene.material = material;
                        scene.cover_background = enabled;
                        scene.expanded_cover_background = enabled;
                        scene.lyric_age_ms = 1000.;
                        scene.cover_age_ms = 1000.;
                        r.render(&scene, false)?;
                        let file = format!("{label}-{name}-{enabled}.png");
                        r.snapshot(&dir.join(file))?;
                    }
                }
            }
            r.set_cover(Some(&std::fs::read(dir.join("artwork.png"))?))?;
            for mini in [false, true] {
                for expanded in [false, true] {
                    for island in [false, true] {
                        for card in [false, true] {
                            for overlay in [false, true] {
                                let mut scene = app.scene();
                                scene.mini = mini;
                                scene.frame = if expanded {
                                    Frame::expanded()
                                } else {
                                    Frame::hidden(app.options())
                                };
                                scene.classic_capsule = renderer::Material::Acrylic;
                                scene.mini_capsule = renderer::Material::Acrylic;
                                scene.material = renderer::Material::Acrylic;
                                scene.cover_background = island;
                                scene.expanded_cover_background = card;
                                scene.cover_overlay = overlay;
                                scene.acrylic_capsule_opacity = 80.;
                                scene.lyric_age_ms = 1000.;
                                scene.cover_age_ms = 1000.;
                                r.render(&scene, false)?;
                                r.snapshot(&dir.join(format!(
                                    "independent-{mini}-{expanded}-{island}-{card}-{overlay}.png"
                                )))?;
                            }
                        }
                    }
                }
            }
            for frost in [0., 40., 100.] {
                for opacity in [0., 60., 100.] {
                    let mut scene = app.scene();
                    scene.cover_background = false;
                    scene.expanded_cover_background = true;
                    scene.cover_overlay = true;
                    scene.acrylic_card_frost = frost;
                    scene.acrylic_card_opacity = opacity;
                    scene.lyric_age_ms = 1000.;
                    scene.cover_age_ms = 1000.;
                    r.render(&scene, false)?;
                    r.snapshot(&dir.join(format!("acrylic-{frost}-{opacity}.png")))?;
                }
            }
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--m3e-preview") {
            m3e_preview::run(
                &mut app,
                Path::new(args.get(2).ok_or("missing preview directory")?),
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if args.get(1).is_some_and(|s|s=="--resident-preview") {
            resident_preview::run(&mut app,Path::new(args.get(2).ok_or("missing preview directory")?))?;
            let _=DestroyWindow(app.hwnd);return Ok(());
        }
        if args.get(1).is_some_and(|s|s=="--device-data-preview") {
            device_data_preview::run(&mut app,Path::new(args.get(2).ok_or("missing preview directory")?))?;
            let _=DestroyWindow(app.hwnd);return Ok(());
        }
        if args.get(1).is_some_and(|s| s == "--integration-preview") {
            let directory = Path::new(args.get(2).ok_or("missing directory")?);
            std::fs::create_dir_all(directory)?;
            app.opacity = 1.;
            app.frame = Frame::expanded();
            app.lyric = "最後の最後に答え合わせをしよう".into();
            let base = app.base.clone();
            app.renderer = None;
            for dpi in [96., 144., 192.] {
                let scale = dpi / 96.;
                let mut renderer = Renderer::new(
                    app.visual_hwnd,
                    (app.layout.host.w * scale) as u32,
                    (app.layout.host.h * scale) as u32,
                    dpi as f32,
                    &base.join("Assets/Fonts/NotoSansSC.ttf"),
                )?;
                renderer.set_cover(Some(&std::fs::read(base.join("Assets/DemoCover.png"))?))?;
                for (name, mix) in [("side", 0.), ("separated", 1.), ("focus", 2.)] {
                    app.music_mix = mix;
                    app.layout.card.h = layout::music_height(mix, 12.5);
                    let mut scene = app.scene();
                    scene.music_layout = if mix > 1.5 {
                        layout::MusicLayout::LyricFocus
                    } else if mix > 0.5 {
                        layout::MusicLayout::Separated
                    } else {
                        layout::MusicLayout::SideBySide
                    };
                    scene.layout_scale = scale;
                    scene.lyric_age_ms = 1000.;
                    scene.title = "ハウル";
                    scene.artist = "LiSA";
                    scene.secondary = "在最后的最后来核对答案吧";
                    scene.highlight = 0.55;
                    scene.component_count = 1;
                    renderer.render(&scene, false)?;
                    renderer.snapshot(&directory.join(format!("music-{name}-{dpi:.0}.png")))?;
                }
                renderer.set_cover(Some(&std::fs::read(
                    base.join("Assets/DeviceIcons/headphones-128.png"),
                )?))?;
                app.music_mix = 1.;
                app.layout.card.h = layout::music_height(1., 12.5);
                for ms in [0., 60., 120., 140., 180., 240., 280.] {
                    let mut scene = app.scene();
                    scene.cover_age_ms = ms;
                    scene.layout_scale = scale;
                    scene.lyric_age_ms = 1000.;
                    renderer.render(&scene, false)?;
                    renderer
                        .snapshot(&directory.join(format!("cover-flip-{dpi:.0}-{ms:03.0}.png")))?;
                }
                app.layout.card.h = app.card_height();
                let mut clocks = timers::Store::new(PathBuf::new());
                for (name, value) in [
                    ("timer", json!({"mode":"countdown","duration":300})),
                    ("stopwatch", json!({"mode":"stopwatch"})),
                    ("pomodoro", json!({"mode":"pomodoro","focus":1500})),
                    (
                        "workflow",
                        json!({"mode":"workflow","stages":[{"name":"专注","seconds":1500},{"name":"休息","seconds":300}]}),
                    ),
                ] {
                    let id = clocks.create(&value).unwrap();
                    let t = clocks.items.iter_mut().find(|t| t.id == id).unwrap();
                    t.elapsed_ms = if name == "stopwatch" { 3830 } else { 5000 };
                    let panel = t.panel();
                    let mut scene = app.scene();
                    scene.layout_scale = scale;
                    scene.panel = Some(&panel);
                    scene.component_count = 1;
                    renderer.render(&scene, false)?;
                    renderer.snapshot(&directory.join(format!("island-{name}-{dpi:.0}.png")))?;
                }
                app.layout.card.h = app.card_height();
                app.updates.state = json!({"phase":"downloading","version":"演示","percent":43,"receivedBytes":4300000,"totalBytes":10000000});
                let panel = app.updates.panel();
                let mut scene = app.scene();
                scene.layout_scale = scale;
                scene.panel = Some(&panel);
                scene.component_count = 1;
                renderer.render(&scene, false)?;
                renderer.snapshot(&directory.join(format!("update-{dpi:.0}.png")))?;
                app.updates.state = json!({"phase":"available","version":"演示","message":"下载后由你确认安装；未确认不会重启，现有数据与设置会完整保留。"});
                let panel = app.updates.panel();
                let mut scene = app.scene();
                scene.layout_scale = scale;
                scene.panel = Some(&panel);
                scene.component_count = 1;
                renderer.render(&scene, false)?;
                renderer.snapshot(&directory.join(format!("update-available-{dpi:.0}.png")))?;
                for kind in [
                    devices::Kind::Usb,
                    devices::Kind::Volume,
                    devices::Kind::Phone,
                    devices::Kind::Headphones,
                    devices::Kind::Speaker,
                    devices::Kind::Gamepad,
                    devices::Kind::Mouse,
                    devices::Kind::Keyboard,
                ] {
                    let notice = devices::Event::new(
                        "preview".into(),
                        kind,
                        true,
                        format!("{} · 示例设备", kind.label()),
                    )
                    .notice();
                    let mut scene = app.scene();
                    scene.layout_scale = scale;
                    scene.notification = Some(&notice);
                    scene.notification_rect = Rect::new(24., 24., 440., 110.);
                    scene.notification_radius = 26.;
                    scene.notification_mix = 1.;
                    renderer.render(&scene, false)?;
                    renderer
                        .snapshot(&directory.join(format!("device-{}-{dpi:.0}.png", kind.key())))?;
                }
            }
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if icon_mode {
            icon_preview(&mut app, Path::new(args.get(2).ok_or("missing directory")?))?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if font_mode {
            let report = app.renderer.as_mut().unwrap().verify_font()?;
            std::fs::write(
                args.get(2).ok_or("missing font report path")?,
                serde_json::to_vec_pretty(&report)?,
            )?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if tool_mode {
            tools_preview(&mut app, Path::new(args.get(2).ok_or("missing directory")?))?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if notice_mode {
            notification_preview(&mut app, Path::new(args.get(2).ok_or("missing directory")?))?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if preview_mode {
            preview(&mut app, Path::new(args.get(2).ok_or("missing directory")?))?;
            let _ = DestroyWindow(app.hwnd);
            return Ok(());
        }
        if !app.demo
            && !args.iter().any(|a| a == "--startup")
            && std::env::var_os("LYRICISLAND_TEST_PIPE").is_none()
            && !app.base.join("Portable.json").is_file()
            && !app.base.join("Installed.json").exists()
            && app.base.join("OtaInstall.json").exists()
            && app.base.join("DotIsle.Setup.exe").exists()
        {
            let offered = app
                .data
                .join(format!("install-offer-{}.txt", DISPLAY_VERSION));
            if !offered.exists() {
                let _ = std::fs::create_dir_all(&app.data);
                if std::process::Command::new(app.base.join("DotIsle.Setup.exe"))
                    .arg(format!("/MIGRATEDATA={}", app.data.display()))
                    .spawn()
                    .is_ok()
                {
                    let _ = std::fs::write(offered, b"offered");
                }
            }
        }
        tray(&app)?;
        let pipe = std::env::var("LYRICISLAND_TEST_PIPE")
            .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into());
        app._server = Some(Server::new(&pipe, settings_callback));
        if should_open_tutorial(
            app.demo,
            std::env::var_os("LYRICISLAND_TEST_PIPE").is_some(),
            app.flag("GesturePracticeSeen", false),
        ) {
            app.show_settings_page("gestureTutorial");
        }
        app.greeting_pending = app.flag("StartupGreetingEnabled", true)
            && !app.demo
            && std::env::var_os("LYRICISLAND_TEST_PIPE").is_none()
            && !args.iter().any(|s| s == "--expanded");
        if args.iter().any(|s| s == "--expanded") {
            app.held = true;
            app.retarget(true)
        }
        let until = args
            .iter()
            .position(|s| s == "--run-ms")
            .and_then(|i| args.get(i + 1))
            .and_then(|s| s.parse::<u64>().ok())
            .map(|ms| Instant::now() + Duration::from_millis(ms));
        drop(app);
        let pacing_hwnd = owner.borrow().visual_hwnd;
        let mut pacer = frame_pacing::FramePacer::new(pacing_hwnd);
        let mut msg = MSG::default();
        'events: loop {
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'events;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let mut app = owner.borrow_mut();
            if until.is_some_and(|t| Instant::now() >= t) {
                let _ = DestroyWindow(app.hwnd);
                break;
            }
            let animating = (app.glow.active() && app.flag("Animation", true))
                || app.feedback_moving
                || app
                    .renderer
                    .as_ref()
                    .is_some_and(|r| r.colors_moving(app.start.elapsed().as_secs_f64()))
                || app.switching
                || app.swipe.is_some_and(|s| s.active)
                || app.panel.as_ref().is_some_and(|p| p.running)
                || app.banner.visible()
                || app.osd_banner.visible()
                || app.alerts.visible()
                || app.top_popup.visible()
                || app.bottom_osd.visible()
                || app.island_morph.active()
                || app.compact_size.moving()
                || app.opacity > 0.001
                    && (app.snapshot["isPlaying"] == true || app.auxiliary_state().0)
                    && app.flag("Animation", true)
                || app.transition.is_some()
                || (app.opacity - app.fade_to).abs() > 0.00001
                || app.lyric_start.elapsed() < Duration::from_millis(250);
            // Pace before drawing: Present(1) already waits for the compositor, so
            // this only spends what is left of the frame budget.
            // Only an interaction, a transition, a fade or an arriving lyric needs the panel
            // rate. Steady playback waits on the capped interval, so the loop itself stops
            // waking a 144 Hz panel 144 times a second.
            let interactive = (app.glow.active() && app.flag("Animation", true))
                || app.liquid_timer_visible()
                || app.feedback_moving
                || app.transition.is_some()
                || app.island_morph.active()
                || app.compact_size.moving()
                || app.swipe.is_some_and(|s| s.active)
                || app.dragging.is_some()
                || (app.opacity - app.fade_to).abs() > 0.00001
                || app.banner.visible()
                || app.osd_banner.visible()
                || app.alerts.visible()
                || app.top_popup.visible()
                || app.bottom_osd.visible()
                || app.switching
                || app.lyric_start.elapsed() < Duration::from_millis(250);
            if animating {
                if interactive {
                    pacer.wait();
                } else {
                    pacer.wait_interval(frame_pacing::continuous_interval());
                }
            }
            if let Err(e) = app.tick() {
                app.error = Some(e.to_string());
                io::log(&app.data.join("Logs"), "NATIVE_RENDER", &e.to_string());
                if let Err(recovery) = app.position() {
                    return Err(recovery.into());
                }
            }
            // Present(1) already synchronizes rendering to the display. Waiting for
            // DwmFlush here as well would introduce a second refresh interval.
            if !animating {
                // The 16 ms poll serves interaction and playback polling. A fully hidden
                // island with nothing scheduled has neither, so it settles into a slow
                // heartbeat instead of waking the loop sixty times a second forever.
                let idle = (app.opacity <= 0.001 || (app.resident_idle() && !app.cursor_inside && app.pressed == 0))
                    && !app.expanded && !app.held && app.dragging.is_none()
                    && !app.banner.visible()
                    && !app.osd_banner.visible()
                    && !app.alerts.visible()
                    && !app.alerts.pending()
                    && !app.bottom_osd.visible()
                    && app.queue.is_empty()
                    && !app.timers.items.iter().any(|t| t.running);
                let _ = MsgWaitForMultipleObjectsEx(
                    None,
                    if idle { 125 } else { 16 },
                    QS_ALLINPUT,
                    MWMO_INPUTAVAILABLE,
                );
            }
        }
        let app = owner.borrow();
        if let Some(b) = &app.backend {
            b.close();
        }
        if let Some(server) = &app._server {
            server.close();
        }
        let _ = CloseHandle(mutex);
        CoUninitialize();
        Ok(())
    }
}
fn should_open_tutorial(demo: bool, test: bool, seen: bool) -> bool {
    !demo && !test && !seen
}
#[cfg(test)]
mod login_tests {
    #[test]
    fn practice_is_required_once_in_normal_launches() {
        assert!(super::should_open_tutorial(false, false, false));
        assert!(!super::should_open_tutorial(false, false, true));
        assert!(!super::should_open_tutorial(true, false, false));
        assert!(!super::should_open_tutorial(false, true, false));
    }
}
fn main() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let report = format!("{}\n{}", info, std::backtrace::Backtrace::force_capture());
        if std::fs::write(base.join("native-panic.txt"), &report).is_err() {
            let path = io::data_directory().join("Logs");
            let _ = std::fs::create_dir_all(&path);
            let _ = std::fs::write(path.join("native-panic.txt"), report);
        }
        previous(info);
    }));
    if let Err(e) = run() {
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_default();
        let _ = std::fs::write(base.join("native-error.txt"), e.to_string());
        std::process::exit(1)
    }
}

#[cfg(test)]
mod interaction_tests {
    use super::*;
    #[test]
    fn rounded_hit_testing_survives_degenerate_morph_endpoints() {
        let r = Rect::new(100., 24., 300., 12.55065091416845);
        assert!(inside_round_rect(r, 30., 250., 30.275325457084225));
        for height in [14., 20., 24., 110., 120., 220.] {
            for i in 0..12000 {
                let p = i as f64 / 11999.;
                let mut layout = Layout::new(260.);
                layout.capsule.h = height;
                layout.card.h = 120.;
                let (r, rad) = layout.morph_shape(p, 30.);
                assert!(inside_round_rect(r, rad, r.x + r.w / 2., r.y + r.h / 2.));
                assert!(!inside_round_rect(r, rad, r.x - 1., r.y));
            }
        }
        assert!(!inside_round_rect(r, f64::NAN, 250., 30.));
    }
    fn app() -> App {
        App::new(PathBuf::new(), PathBuf::new(), true)
    }
    #[test]
    fn swipes_work_on_both_views_but_leave_controls_alone() {
        let mut a = app();
        a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
        a.refresh_components();
        a.settings["SwipeActivities"] = json!(false);
        assert!(!a.can_swipe(0));
        a.settings["SwipeActivities"] = json!(true);
        for expanded in [false, true] {
            a.expanded = expanded;
            for hit in [0, 1, 13, 40, 41, 42] {
                assert!(a.can_swipe(hit));
            }
            for hit in [-1, 2, 3, 4, 5, 6, 7, 10, 30, 31] {
                assert!(!a.can_swipe(hit));
            }
        }
    }
    #[test]
    fn releasing_a_swipe_keeps_the_rendered_position_continuous() {
        for expanded in [false, true] {
            let mut a = app();
            let id = a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
            a.refresh_components();
            a.expanded = expanded;
            let width = if expanded {
                a.layout.card.w
            } else {
                a.layout.capsule.w
            };
            let mut swipe = activities::Swipe::new(100., 80.);
            swipe.update(30., 80.);
            let offset = swipe.dx;
            assert!(a.finish_swipe(swipe));
            assert_eq!(a.selected, format!("timer:{id}"));
            assert!((a.slide.value - width - offset).abs() < 0.001);
            assert!(a.slide_outgoing);
        }
    }
    #[test]
    fn short_swipe_returns_without_changing_the_component() {
        let mut a = app();
        a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
        a.refresh_components();
        let mut swipe = activities::Swipe::new(100., 80.);
        swipe.update(120., 80.);
        assert!(a.finish_swipe(swipe));
        assert_eq!(a.selected, "music");
        assert_eq!(a.slide.value, 20.);
        assert!(!a.slide_outgoing);
    }
    #[test]
    fn disabled_body_click_does_not_expand_but_cover_does() {
        unsafe {
            let mut a = app();
            a.settings["ClickAnywhereExpand"] = json!(false);
            let c = a.layout.capsule;
            assert_eq!(a.hit(c.x + 22., c.y + 20.), 1);
            assert_eq!(a.hit(c.x + 75., c.y + 20.), 0);
            a.activate(0);
            assert!(!a.expanded);
            a.activate(1);
            assert!(a.expanded && a.held);
        }
    }
    #[test]
    fn enabled_body_click_expands_without_changing_playback() {
        unsafe {
            let mut a = app();
            a.settings["ClickAnywhereExpand"] = json!(true);
            let before = a.snapshot["isPlaying"].clone();
            a.activate(0);
            assert!(a.expanded && a.held);
            assert_eq!(a.snapshot["isPlaying"], before);
            a.settings["MusicExpandedLayout"] = json!("SideBySide");
            a.music_mix = 0.;
            a.frame = Frame::expanded();
            a.transition = None;
            let c = a.layout.card;
            let g = a.music_geometry();
            let button = g.buttons[3].1;
            assert_eq!(
                a.hit(
                    c.x + button.x + button.w / 2.,
                    c.y + button.y + button.h / 2.
                ),
                4
            );
            a.activate(4);
            assert!(a.expanded);
            assert_eq!(a.snapshot["isPlaying"], false);
        }
    }
    #[test]
    fn body_click_option_is_boolean_and_preserves_other_settings() {
        let original = config::defaults();
        assert_eq!(original["PauseNotifications"], false);
        assert!(original.get("FollowWindowsDnd").is_none());
        assert_eq!(original["ClickAnywhereExpand"], true);
        assert_eq!(original["CoverBackground"], false);
        assert_eq!(original["OnlineSongInfo"], true);
        let updated = config::validate_setting(
            json!({"key":"ClickAnywhereExpand","value":true,"settings":original}),
        )
        .unwrap();
        assert_eq!(updated["ClickAnywhereExpand"], true);
        assert_eq!(updated["Motion"], original["Motion"]);
        assert!(config::validate_setting(
            json!({"key":"ClickAnywhereExpand","value":"true","settings":original})
        )
        .is_err());
    }
    /// Mouse and keyboard reconnect several times a day on their own — a wireless mouse waking up,
    /// a KVM switching — so they are the two device reminders that turn into noise. Off by default,
    /// carried over once for installs that inherited 2.0.0's bundle default.
    #[test]
    fn mouse_and_keyboard_reminders_default_off() {
        let s = config::defaults();
        for key in [
            "MouseConnectionRemindersEnabled",
            "KeyboardConnectionRemindersEnabled",
        ] {
            assert_eq!(s[key], json!(false), "{key}");
            // Still a real switch: turning it on must validate and stick.
            assert_eq!(
                config::validate_setting(json!({"key":key,"value":true,"settings":s.clone()}))
                    .unwrap()[key],
                json!(true),
                "{key}"
            );
            assert!(
                config::validate_setting(json!({"key":key,"value":1,"settings":s.clone()}))
                    .is_err(),
                "{key}"
            );
        }
        // Everything else in the device bundle keeps its 2.0.0 default.
        for key in [
            "HeadphoneConnectionRemindersEnabled",
            "SpeakerConnectionRemindersEnabled",
            "GamepadConnectionRemindersEnabled",
            "DeviceDisconnectRemindersEnabled",
        ] {
            assert_eq!(s[key], json!(true), "{key}");
        }
        // A 2.0.0 install persisted `true` for both without anyone choosing it.
        let legacy = std::env::temp_dir().join(format!(
            "dotisle-device-defaults-{}.json",
            std::process::id()
        ));
        std::fs::write(&legacy,r#"{"DefaultsOnboardingVersion":1,"MouseConnectionRemindersEnabled":true,"KeyboardConnectionRemindersEnabled":true}"#).unwrap();
        let migrated = config::load(&legacy);
        assert_eq!(migrated["DeviceDefaultsVersion"], json!(1));
        assert_eq!(migrated["MouseConnectionRemindersEnabled"], json!(false));
        assert_eq!(migrated["KeyboardConnectionRemindersEnabled"], json!(false));
        // Once the marker is stamped, a user who turns them back on keeps that choice.
        std::fs::write(&legacy,r#"{"DefaultsOnboardingVersion":1,"DeviceDefaultsVersion":1,"MouseConnectionRemindersEnabled":true,"KeyboardConnectionRemindersEnabled":true}"#).unwrap();
        let chosen = config::load(&legacy);
        assert_eq!(chosen["MouseConnectionRemindersEnabled"], json!(true));
        assert_eq!(chosen["KeyboardConnectionRemindersEnabled"], json!(true));
        let _ = std::fs::remove_file(&legacy);
    }
    #[test]
    fn swipe_notifications_and_devices_default_on() {
        let s = config::defaults();
        for key in [
            "SwipeActivities",
            "NotificationsEnabled",
            "NotificationClickOpenApp",
            "DeviceRemindersEnabled",
        ] {
            assert_eq!(s[key], json!(true), "{key}");
            assert!(
                config::validate_setting(json!({"key":key,"value":"yes","settings":s.clone()}))
                    .is_err(),
                "{key}"
            );
            assert_eq!(
                config::validate_setting(json!({"key":key,"value":false,"settings":s.clone()}))
                    .unwrap()[key],
                json!(false),
                "{key}"
            );
        }
        // A settings.json written by an older build has none of these keys, so the shipped
        // defaults — not the JSON — decide what a first run does.
        let loaded = config::load(std::path::Path::new(
            "no-such-settings-from-an-older-build.json",
        ));
        assert_eq!(loaded["SwipeActivities"], json!(true));
        assert_eq!(loaded["NotificationsEnabled"], json!(true));
        assert_eq!(loaded["DeviceRemindersEnabled"], json!(true));
        // A 1.4 install persisted `false` for all three and knows nothing about the marker, so the
        // one-time migration switches them on and records that it ran.
        let legacy = std::env::temp_dir().join(format!(
            "dotisle-defaults-migration-{}.json",
            std::process::id()
        ));
        std::fs::write(&legacy,r#"{"SwipeActivities":false,"NotificationsEnabled":false,"DeviceRemindersEnabled":false}"#).unwrap();
        let migrated = config::load(&legacy);
        assert_eq!(migrated["DefaultsOnboardingVersion"], json!(1));
        for key in [
            "SwipeActivities",
            "NotificationsEnabled",
            "DeviceRemindersEnabled",
        ] {
            assert_eq!(migrated[key], json!(true), "{key}");
        }
        // Once the marker is stamped those same false values are a real opt-out and must survive.
        std::fs::write(&legacy,r#"{"DefaultsOnboardingVersion":1,"SwipeActivities":false,"NotificationsEnabled":false,"DeviceRemindersEnabled":false}"#).unwrap();
        let opted_out = config::load(&legacy);
        for key in [
            "SwipeActivities",
            "NotificationsEnabled",
            "DeviceRemindersEnabled",
        ] {
            assert_eq!(opted_out[key], json!(false), "{key}");
        }
        let _ = std::fs::remove_file(&legacy);
        assert!(notifications::Policy::from_settings(&json!({})).enabled);
        // Nothing opted in, so a first-run island already swipes between components.
        let mut a = app();
        a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
        a.refresh_components();
        assert_eq!(a.components.len(), 2);
        assert!(a.can_swipe(0));
        assert!(a.can_swipe(41));
        assert!(!a.can_swipe(2));
    }
}

#[cfg(test)]
mod v121_tests {
    use super::*;
    #[test]
    fn independent_click_defaults_and_retired_options() {
        let s = config::defaults();
        assert_eq!(s["NotificationClickOpenApp"], true);
        assert_eq!(s["MusicClickOpenApp"], false);
        for key in ["NotificationCompatibility", "CardMaterial"] {
            assert!(s.get(key).is_none());
            assert!(
                config::validate_setting(json!({"key":key,"value":true,"settings":s})).is_err()
            );
        }
        let s =
            config::validate_setting(json!({"key":"MusicClickOpenApp","value":true,"settings":s}))
                .unwrap();
        assert_eq!(s["NotificationClickOpenApp"], true);
        assert_eq!(s["MusicClickOpenApp"], true);
    }
    #[test]
    fn preview_and_close_do_not_launch_app() {
        let mut app = App::new(PathBuf::new(), PathBuf::new(), true);
        app.notification_preview = true;
        app.banner.notice = Some(notifications::Notice {
            app_id: "".into(),
            ..Default::default()
        });
        unsafe { app.activate(12) }
        assert!(app.error.is_none());
        app.notification_preview = false;
        unsafe { app.activate(11) }
        assert!(app.error.is_none());
        unsafe { app.activate(12) }
        assert!(app.error.is_some());
        app.error = None;
        app.settings["NotificationClickOpenApp"] = json!(false);
        unsafe { app.activate(12) }
        assert!(app.error.is_none());
    }
}

#[cfg(test)]
mod music_jump_tests {
    use super::*;
    #[test]
    fn music_title_and_cover_follow_independent_switch() {
        unsafe {
            let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
            a.opacity = 1.;
            a.expanded = true;
            a.frame = Frame::expanded();
            a.transition = None;
            let c = a.layout.card;
            assert_eq!(a.hit(c.x + 150., c.y + 40.), 0);
            a.settings["MusicClickOpenApp"] = json!(true);
            assert_eq!(a.hit(c.x + 150., c.y + 40.), 13);
            assert_eq!(a.hit(c.x + 30., c.y + 30.), 1);
            let expanded = a.expanded;
            a.activate(13);
            assert_eq!(a.expanded, expanded);
            assert!(a.error.is_some());
            let before = a.snapshot["isPlaying"].clone();
            a.activate(4);
            assert_ne!(a.snapshot["isPlaying"], before);
        }
    }
}

#[cfg(test)]
mod v13_tests {
    use super::*;
    fn app() -> App {
        App::new(PathBuf::new(), PathBuf::new(), true)
    }
    fn reminder(id: &str) -> todos::Todo {
        todos::Todo {
            color: "#F3D79B".into(),
            id: id.into(),
            title: format!("待办 {id}"),
            content: String::new(),
            date: "2026-10-16".into(),
            time: "09:00".into(),
            daily: false,
            completed: false,
            completed_date: None,
            last_reminded_date: None,
            snooze_until: None,
        }
    }
    fn notice(title: &str) -> Queued {
        Queued {
            notice: notifications::Notice {
                title: title.into(),
                ..Default::default()
            },
            todo: None,
            chime: true,
        }
    }
    /// The capsule may shrink for a short lyric, but the window and the expanded
    /// card must stay exactly where they were.
    #[test]
    fn collapsed_capsule_resizes_without_moving_host_or_card() {
        let mut a = app();
        let base = a.compact_base();
        a.layout.reserve_compact(base);
        let (host, card, card_x) = (a.layout.host.w, a.layout.card.w, a.layout.card.x);
        a.layout.set_compact(140., 40.);
        assert_eq!(a.layout.capsule.w, 140.);
        assert_eq!(a.layout.host.w, host);
        assert_eq!(a.layout.card.w, card);
        a.layout.set_compact(base, 48.);
        assert_eq!(a.layout.capsule.w, base);
        assert_eq!(a.layout.host.w, host);
        assert_eq!(a.layout.card.w, card);
        assert_eq!(a.layout.card.x, card_x);
    }
    /// Auto width off is the 1.2 behaviour: the capsule is exactly the stored width.
    #[test]
    fn auto_width_off_keeps_the_stored_capsule_width() {
        let mut a = app();
        unsafe {
            a.settings["CompactWidth"] = json!(300.);
            a.settings["AutoCompactWidth"] = json!(false);
            a.compact_size = compact::Resize::new(300., 40.);
            a.update_compact_layout().unwrap();
            assert_eq!(a.layout.capsule.w, 300.);
            assert_eq!(a.compact_size.width_target, 300.);
            a.settings["AutoCompactWidth"] = json!(true);
            a.update_compact_layout().unwrap();
            assert!(a.compact_size.width_target < 300. && a.compact_size.width_target >= 140.);
        }
    }
    #[test]
    fn lyric_cover_color_is_independent_of_both_surface_switches() {
        let a = app();
        let palette = crate::cover_color::CoverPalette {
            background: 0x382020,
            accent: 0xf9b5a2,
        };
        for mini in [false, true] {
            for island in [false, true] {
                for card in [false, true] {
                    for lyric in [false, true] {
                        for cover in [None, Some(palette)] {
                            let mut s = a.scene();
                            s.material = renderer::Material::Acrylic;
                            s.classic_capsule = renderer::Material::Acrylic;
                            s.mini_capsule = renderer::Material::Acrylic;
                            s.mini = mini;
                            s.cover_background = island;
                            s.expanded_cover_background = card;
                            s.lyric_cover_color = lyric;
                            s.progress_color = 0xffa9e6cd;
                            s.component_color = 0xffb1e8d1;
                            let (expanded, compact) = renderer::cover_scenes(&s, cover);
                            let expected = if lyric && cover.is_some() {
                                0xfff9b5a2
                            } else {
                                s.progress_color
                            };
                            assert_eq!(expanded.progress_color, expected);
                            assert_eq!(compact.progress_color, expected);
                            assert_eq!(
                                expanded.component_color,
                                if card && cover.is_some() {
                                    0xfff9b5a2
                                } else {
                                    s.component_color
                                }
                            );
                            assert_eq!(
                                compact.component_color,
                                if island && cover.is_some() {
                                    0xfff9b5a2
                                } else {
                                    s.component_color
                                }
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn album_color_source_produces_distinct_cover_accents_across_materials() {
        let mut a = app();
        a.settings["IslandColorSource"] = json!("Content");
        a.settings["ExpandedMonetEnabled"] = json!(true);
        a.settings["LyricColorFromCover"] = json!(true);
        let make_png = |rgb: [u8; 3]| -> Vec<u8> {
            let mut img = image::RgbaImage::new(24, 24);
            for px in img.pixels_mut() {
                *px = image::Rgba([rgb[0], rgb[1], rgb[2], 255]);
            }
            let mut buf = Vec::new();
            img.write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).unwrap();
            buf
        };
        let red_seed = Renderer::prepare_cover(Some(&make_png([220, 42, 42]))).unwrap().unwrap().seed.unwrap();
        let blue_seed = Renderer::prepare_cover(Some(&make_png([36, 98, 224]))).unwrap().unwrap().seed.unwrap();
        let red_roles = dotisle_domain::dynamic_color::roles(red_seed, true, true, false);
        let blue_roles = dotisle_domain::dynamic_color::roles(blue_seed, true, true, false);
        for mat in [renderer::Material::LightField, renderer::Material::Acrylic] {
            let mut s = a.scene();
            s.material = mat;
            let (exp_r, _) = renderer::cover_scenes(&s, Some(crate::cover_color::CoverPalette { background: red_roles.surface_container_high, accent: red_roles.primary }));
            let (exp_b, _) = renderer::cover_scenes(&s, Some(crate::cover_color::CoverPalette { background: blue_roles.surface_container_high, accent: blue_roles.primary }));
            assert_ne!(exp_r.progress_color, exp_b.progress_color);
            assert_ne!(exp_r.component_color, exp_b.component_color);
            assert!(((exp_r.progress_color >> 16) & 255) > (exp_r.progress_color & 255));
            assert!((exp_b.progress_color & 255) > ((exp_b.progress_color >> 16) & 255));
        }
        assert_eq!(a.scene().popup_material, renderer::Material::Acrylic);
    }
    #[test]
    fn compact_second_line_follows_both_switches() {
        let mut a = app();
        a.snapshot["frame"] = json!({"currentLine":"You are my sunshine","currentTranslation":"你是我的阳光","nextLine":"下一句"});
        a.settings["CompactTranslation"] = json!(true);
        a.settings["ChineseTwoLine"] = json!(false);
        assert_eq!(a.compact_secondary_for(), "你是我的阳光");
        a.settings["CompactTranslation"] = json!(false);
        assert_eq!(a.compact_secondary_for(), "");
        a.settings["ChineseTwoLine"] = json!(true);
        assert_eq!(a.compact_secondary_for(), "");
        a.snapshot["frame"] =
            json!({"currentLine":"晚风轻轻","currentTranslation":"","nextLine":"音乐刚好"});
        assert_eq!(a.compact_secondary_for(), "音乐刚好");
        a.settings["CompactTranslation"] = json!(true);
        assert_eq!(a.compact_secondary_for(), "音乐刚好");
    }
    /// A burst of notifications must not overwrite each other, and a pending
    /// reminder must never be evicted to make room for one.
    #[test]
    fn notifications_queue_and_reminders_are_never_evicted() {
        let mut a = app();
        for i in 0..9 {
            a.enqueue(notice(&format!("n{i}")));
        }
        assert_eq!(a.queue.len(), 8);
        assert_eq!(a.queue.front().unwrap().notice.title, "n1");
        a.enqueue_todo(&reminder("t1"));
        assert_eq!(a.queue.len(), 9);
        assert!(a.queue.iter().any(|e| e.todo.as_deref() == Some("t1")));
        for i in 0..40 {
            a.enqueue(notice(&format!("m{i}")));
        }
        assert!(a.queue.iter().any(|e| e.todo.as_deref() == Some("t1")));
    }
    #[test]
    fn completing_or_deleting_a_todo_cancels_its_pending_banner() {
        let mut a = app();
        a.enqueue_todo(&reminder("t1"));
        a.enqueue_todo(&reminder("t2"));
        a.cancel_todo("t1");
        assert_eq!(a.queue.len(), 1);
        assert_eq!(a.queue[0].todo.as_deref(), Some("t2"));
    }
    #[test]
    fn state_exposes_todos_version_and_pending_page() {
        let mut a = app();
        a.pending_page = Some("todos".into());
        a.pending_request=2;
        a.pending_center_action=json!({"action":"todoEdit","id":"t1"});
        unsafe {
            a.request(json!({"version":1,"command":"pageSeen","value":{"requestId":1}}));
            assert_eq!(a.pending_page.as_deref(),Some("todos"));
            assert_eq!(a.pending_center_action["id"],"t1");
            let state = a.request(json!({"version":1,"command":"state"}));
            assert_eq!(state["version"], DISPLAY_VERSION);
            assert!(state["todos"].is_array());
            assert!(state["today"]
                .as_str()
                .is_some_and(|d| todos::valid_date(d)));
            assert_eq!(state["pendingPage"], "todos");
            assert_eq!(
                a.request(json!({"version":1,"command":"pageSeen"}))["success"],
                true
            );
            assert!(a.pending_page.is_none());
        }
    }
    /// Demo runs are self-checks: they must never create or edit a todo file.
    #[test]
    fn demo_mode_leaves_the_todo_store_untouched() {
        let mut a = app();
        unsafe {
            let saved=a.request(json!({"version":1,"command":"todoSave","value":{"title":"x","date":"2026-10-16","time":"09:00"}}));
            assert_eq!(saved["success"], true);
            assert!(a.todos.items.is_empty());
            assert_eq!(a.request(json!({"version":1,"command":"todoComplete","value":{"id":"x","completed":true}}))["success"],true);
            assert_eq!(
                a.request(json!({"version":1,"command":"todoDelete","value":{"id":"x"}}))
                    ["success"],
                true
            );
            assert!(a.todos.items.is_empty());
        }
    }
    /// Un-completing a reminder must not replay it: the delivery marker for that
    /// date survives, so a finished item never chimes twice.
    #[test]
    fn uncompleting_a_reminder_does_not_replay_it() {
        let p = std::env::temp_dir().join(format!(
            "LyricIsland.UncompleteTest.{}.{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut store = todos::Store::new(p.clone());
        let now = todos::Now {
            date: "2026-10-16".into(),
            time: "09:00".into(),
        };
        let id = store
            .save(&json!({"title":"喝水","date":"2026-10-16","time":"09:00"}))
            .unwrap();
        assert_eq!(store.poll(&now).unwrap().len(), 1);
        store.complete(&id, false, &now).unwrap();
        assert!(store.poll(&now).unwrap().is_empty());
        let _ = std::fs::remove_file(p);
    }
    #[test]
    fn delivered_reminders_land_in_history_and_the_switch_silences_it() {
        let mut a = app();
        a.enqueue_todo(&reminder("t1"));
        assert_eq!(a.history.items.len(), 1);
        assert_eq!(a.history.items[0].kind, "todo");
        assert_eq!(a.history.items[0].todo_id.as_deref(), Some("t1"));
        assert_eq!(a.history.list()[0]["title"], "待办 t1");
        a.settings["NotificationHistoryEnabled"] = json!(false);
        a.enqueue_todo(&reminder("t2"));
        assert_eq!(a.history.items.len(), 1);
    }
    #[test]
    fn history_clear_rejects_unknown_kinds_and_can_target_one() {
        let mut a = app();
        a.enqueue_todo(&reminder("t1"));
        unsafe {
            assert_eq!(
                a.request(json!({"version":1,"command":"historyClear","value":{"kind":"bogus"}}))
                    ["success"],
                false
            );
            assert_eq!(
                a.request(json!({"version":1,"command":"historyClear"}))["success"],
                true
            );
        }
        assert!(a.history.items.is_empty());
    }
    #[test]
    fn island_presence_follows_need_not_switches() {
        assert!(island_needed(
            true, true, true, false, false, false, false, false
        ));
        assert!(island_needed(
            true, false, false, false, false, false, false, false
        ));
        assert!(!island_needed(
            true, false, true, false, false, false, false, false
        ));
        assert!(island_needed(
            false, false, true, true, false, false, false, false
        ));
        assert!(island_needed(
            false, false, true, false, false, true, false, false
        ));
        assert!(island_needed(
            false, false, true, false, true, false, false, false
        ));
        assert!(island_needed(
            false, false, true, false, false, false, true, false
        ));
        assert!(island_needed(
            false, false, true, false, false, false, false, true
        ));
        assert!(!island_needed(
            false, false, true, false, false, false, false, false
        ));
    }
}

#[cfg(test)]
mod hide_gesture_tests {
    use super::*;
    #[test]
    fn an_upward_drag_is_neither_a_click_nor_a_horizontal_swipe() {
        let start = (100., 80.);
        assert!(within_click_slop(Some(start), 102., 78.));
        assert!(!within_click_slop(Some(start), 102., 46.));
        assert!(!within_click_slop(None, 100., 80.));
        let mut swipe = activities::Swipe::new(start.0, start.1);
        swipe.update(102., 46.);
        assert!(!swipe.active);
        swipe.update(64., 79.);
        assert!(swipe.active);
    }
    #[test]
    fn hide_is_available_with_one_component_and_preserves_timers() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        assert_eq!(a.settings["HiddenDisplay"], "Strip");
        let id = a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
        a.hide_by_gesture();
        assert!(a.user_hidden);
        assert!(!a.held);
        assert!(a.timers.items.iter().any(|t| t.id == id && t.running));
    }
    #[test]
    fn hidden_display_switches_between_none_strip_and_mini() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.settings["HiddenDisplay"] = json!("None");
        a.hide_by_gesture();
        assert!(a.user_hidden && !a.hidden_edge() && !a.hidden_mini_mode() && !a.mini());
        a.settings["HiddenDisplay"] = json!("Strip");
        a.hide_by_gesture();
        assert!(a.user_hidden && a.hidden_edge() && !a.hidden_mini_mode() && !a.mini());
        a.settings["HiddenDisplay"] = json!("Mini");
        a.hide_by_gesture();
        assert!(a.user_hidden && !a.hidden_edge() && a.hidden_mini_mode() && a.mini());
        a.reveal_hidden_edge();
        assert!(!a.user_hidden && !a.hidden_mini_mode() && !a.mini());
    }
    #[test]
    fn practice_recover_step_shows_the_strip_and_finishes_on_click() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.settings["HiddenDisplay"] = json!("Strip");
        a.practice_request(&json!({"id":"practice-test","action":"start"}));
        while let Some(step) = a.practice_step() {
            if step == PracticeStep::Hide { break; }
            a.practice_event(step);
        }
        assert_eq!(a.practice_step(), Some(PracticeStep::Hide));
        assert!(!a.hidden_edge());
        a.hide_by_gesture();
        assert_eq!(a.practice_step(), Some(PracticeStep::Recover));
        assert!(a.user_hidden && a.hidden_edge());
        a.reveal_hidden_edge();
        assert!(!a.user_hidden && !a.hidden_edge());
        assert_eq!(a.practice_step(), None);
        a.practice_request(&json!({"id":"practice-test","action":"finish"}));
        assert!(a.practice.is_none());
    }
    #[test]
    fn practice_skips_swipe_when_only_one_component_exists() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.components.truncate(1);
        a.settings["SwipeActivities"] = json!(true);
        a.practice_request(&json!({"id":"practice-swipe","action":"start"}));
        let mut seen = Vec::new();
        while let Some(step) = a.practice_step() {
            seen.push(step);
            a.practice_event(step);
        }
        assert!(!seen.contains(&PracticeStep::Swipe), "{seen:?}");
        assert!(seen.contains(&PracticeStep::Hide) && seen.contains(&PracticeStep::Recover));
    }
    #[test]
    fn four_way_island_strip_and_mini_morphs_start_smoothly_and_interrupt_cleanly() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.settings["Animation"] = json!(true);

        // 1. 灵动岛 -> 蓝条：起跳几何必须为真实正常胶囊，绝不能提前退化为蓝条尺寸
        a.settings["HiddenDisplay"] = json!("Strip");
        a.hide_by_gesture();
        assert!(a.user_hidden && a.hidden_edge() && a.island_morph.active());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Strip);
        let p_strip_start = a.island_morph.presentation(0., a.layout.host);
        assert!(p_strip_start.rect.w > 150. && p_strip_start.rect.h >= 38.);

        // 2. 中途立即点按蓝条 -> 还原灵动岛（连续不跳变，且绝不误展开卡片）
        let t_mid = a.start.elapsed().as_secs_f64();
        let before = a.island_morph.presentation(t_mid, a.layout.host);
        a.settings["ClickAnywhereExpand"] = json!(true);
        a.settings["HoverExpand"] = json!(true);
        a.reveal_hidden_edge();
        assert!(!a.user_hidden && !a.hidden_edge() && a.island_morph.active());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Normal);
        assert!(!a.expanded && !a.held && a.transition.is_none() && a.suppress_hover_expand);
        a.track_pointer_presence(true);
        assert!(!a.expanded, "restoring from strip must never open expanded card");
        let after = a.island_morph.presentation(t_mid, a.layout.host);
        assert!((before.rect.w - after.rect.w).abs() < 2.0);

        // 3. 灵动岛展开态右键直缩 -> Mini 岛：起点高且未提前置为 Mini
        a.expanded = true;
        a.held = true;
        a.frame = Frame::expanded();
        a.settings["HiddenDisplay"] = json!("Mini");
        a.hide_by_gesture();
        assert!(!a.expanded && !a.held && a.hidden_mini_mode() && a.island_morph.active());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Mini);

        // 4. Mini 岛还原 -> 灵动岛（不展开卡片，且内容全过程不归零）
        a.reveal_hidden_edge();
        assert!(!a.user_hidden && !a.hidden_mini_mode() && a.island_morph.active());
        assert!(!a.expanded && !a.held && a.transition.is_none());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Normal);
    }

    #[test]
    fn size_mode_toggle_starts_from_previous_geometry_instead_of_target() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.settings["Animation"] = json!(true);
        a.settings["IslandSizeMode"] = json!("Classic");
        let result = unsafe {
            a.request(json!({"version":1,"command":"update","value":{"key":"IslandSizeMode","value":"Mini"}}))
        };
        assert_eq!(result["success"], true, "{result}");
        assert!(a.island_morph.active());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Mini);
        let p0 = a.island_morph.presentation(0., a.layout.host);
        assert!(p0.rect.h >= 38. && p0.mini_mix < 0.05, "expected start from classic capsule, got {:?}", p0);
    }

    #[test]
    fn fullscreen_retracts_only_wellness_even_if_it_was_already_queued_or_visible() {
        use reminder_plugins::Kind::{Stand, Water};
        let mut app = App::new(PathBuf::new(), PathBuf::new(), true);
        app.alerts.enqueue(capsule_alert::Alert::wellness(Stand));
        app.alerts.enqueue(capsule_alert::Alert::wellness(Water));
        let (base, radius) = app.shape();
        assert!(app.alerts.begin_at(base, radius, app.layout.host, base.y, 0., false, false));
        assert_eq!(app.alerts.active.as_ref().unwrap().key, Stand.alert_key());
        app.alerts.enqueue(capsule_alert::Alert::device("fixture", "Usb", "Device", true, None, false));
        app.suppress_wellness_interrupt();
        assert!(!app.alerts.has_wellness());
        assert!(!app.alerts.visible());
        assert!(app.alerts.pending(), "must not remove non-wellness alerts");
    }

    #[test]
    fn wellness_settings_and_suppression_gate_real_island_alerts() {
        use reminder_plugins::Kind::{Stand, Water};
        let mut app = App::new(PathBuf::new(), PathBuf::new(), true);
        app.settings["Animation"] = json!(false);
        let sample = |app: &mut App, active_ms| app.tick_reminder_plugins(screen_time::Sample {
            active_ms, continuous: true,
        }, false);
        let configure = |app: &mut App, value| {
            let result = unsafe { app.request(json!({"version":1,"command":"reminderPluginConfigure","value":value})) };
            assert_eq!(result["success"], true, "{result}");
        };
        for kind in [Stand, Water] { configure(&mut app, json!({"id":kind.id(),"minutes":1})); }
        sample(&mut app, 60_000);
        assert!(!app.alerts.pending(), "setting an interval alone must not enable reminders");
        for kind in [Stand, Water] { configure(&mut app, json!({"id":kind.id(),"enabled":true})); }
        for scenario in 0..4 {
            app.session_locked = scenario == 0;
            app.taskbar_suppressed = scenario == 1;
            app.fullscreen_suppressed = scenario == 3;
            app.settings["PauseNotifications"] = json!(scenario == 2);
            sample(&mut app, 60_000);
            assert!(!app.alerts.pending(), "suppressed scenario {scenario}");
        }
        app.fullscreen_suppressed = false;
        app.settings["PauseNotifications"] = json!(false);
        app.tick_reminder_plugins(screen_time::Sample { active_ms: 60_000, continuous: true }, true);
        assert!(!app.alerts.pending(), "a fullscreen app must not wake the island");
        sample(&mut app, 1);
        assert!(!app.alerts.pending(), "no catch-up after suppression");
        sample(&mut app, 59_999);
        assert!(app.alerts.next_island_only());
        assert!(app.alerts.pop_next().is_none(), "never open the independent taskbar popup");
        configure(&mut app, json!({"id":Water.id(),"enabled":false}));
        let (base, radius) = app.shape();
        assert!(app.alerts.begin_at(base, radius, app.layout.host, base.y, 0., false, true));
        let alert = app.alerts.active.as_ref().unwrap();
        assert_eq!(alert.key, Stand.alert_key());
        assert!(alert.height > 48., "dedicated expanded card");
        assert!(!app.alerts.pending(), "turning water off retracts its pending reminder");
        configure(&mut app, json!({"id":Stand.id(),"enabled":false}));
        app.alerts.tick(base, radius, 1., false);
        assert!(!app.alerts.visible(), "turning stand off retracts its visible card");
        sample(&mut app, 60_000);
        assert!(!app.alerts.pending());
    }

    #[test]
    fn reveal_from_strip_or_mini_relays_live_lyric_immediately_without_blank_frame() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.settings["Animation"] = json!(true);
        a.settings["HiddenDisplay"] = json!("Strip");
        a.snapshot["track"] = json!({"id":"relay-1","title":"Song","artist":"Singer","durationSeconds":180.});
        a.snapshot["isPlaying"] = json!(true);
        a.snapshot["frame"] = json!({"currentLine":"第一句歌词还没唱完","currentTranslation":"","nextLine":"第二句歌词"});
        a.poll();
        assert_eq!(a.lyric, "第一句歌词还没唱完");

        // 场景 A：缩成蓝条期间同一句未唱完 -> 还原依旧承接该句，且首帧完整清晰可见
        a.hide_by_gesture();
        a.lyric_start = Instant::now() - Duration::from_millis(260);
        a.reveal_hidden_edge();
        assert_eq!(a.lyric, "第一句歌词还没唱完");
        assert!(a.previous_lyric.is_empty());
        assert!(a.lyric_start.elapsed() >= Duration::from_millis(240));
        assert!(!a.expanded);

        // 场景 B：缩成蓝条期间切到下一句 -> 还原立即显示下一句，绝不先闪黑空框或滞留上一句
        a.hide_by_gesture();
        a.snapshot["frame"] = json!({"currentLine":"已经切到下一句歌词","currentTranslation":"","nextLine":""});
        a.reveal_hidden_edge();
        assert_eq!(a.lyric, "已经切到下一句歌词");
        assert!(a.previous_lyric.is_empty());
        assert!(a.lyric_start.elapsed() >= Duration::from_millis(240));
    }

    #[test]
    fn right_click_collapse_always_animates_to_strip_even_if_fading() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.settings["Animation"] = json!(true);
        a.settings["HiddenDisplay"] = json!("Strip");
        a.opacity = 0.0; // 模拟刚醒来或处于低透明度状态
        a.enter_hidden_edge();
        assert!(a.user_hidden && a.hidden_edge());
        assert!(a.island_morph.active());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Strip);
        let p0 = a.island_morph.presentation(0., a.layout.host);
        assert!(p0.rect.w > 120.);
    }

    #[test]
    fn left_click_strip_morphs_directly_into_card() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.0;
        a.settings["Animation"] = json!(true);
        a.settings["HiddenDisplay"] = json!("Strip");
        a.hide_by_gesture();
        assert!(a.hidden_edge());
        a.reveal_hidden_edge_to_card();
        assert!(!a.user_hidden && a.expanded && a.held);
        assert!(a.island_morph.active());
        assert_eq!(a.island_morph.mode(), morph_transition::IslandMode::Card);
        let p_mid = a.island_morph.presentation(0.25, a.layout.host);
        assert!(p_mid.rect.h > 40. && p_mid.card_mix > 0.1);
    }
}

#[cfg(test)]
mod mini_behavior_tests {
    use super::*;
    #[test]
    fn mini_defaults_and_validation_survive_serialization() {
        let s = config::defaults();
        assert_eq!(s["IslandSizeMode"], "Classic");
        let mini =
            config::validate_setting(json!({"key":"IslandSizeMode","value":"Mini","settings":s}))
                .unwrap();
        let restored: Value = serde_json::from_slice(&serde_json::to_vec(&mini).unwrap()).unwrap();
        assert_eq!(restored["IslandSizeMode"], "Mini");
        assert!(config::validate_setting(
            json!({"key":"IslandSizeMode","value":true,"settings":restored})
        )
        .is_err());
    }
    #[test]
    fn mini_hit_region_has_no_satellite_or_invisible_cover() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.settings["IslandSizeMode"] = json!("Mini");
        a.layout.set_compact(260., 20.);
        a.opacity = 1.;
        a.frame = Frame::hidden(a.options());
        a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
        a.refresh_components();
        let c = a.layout.capsule;
        assert_eq!(a.hit(c.x + 12., c.y + 10.), 0);
        assert_eq!(a.hit(c.x + 100., c.y + 21.), -1);
        let sat = a.layout.satellite();
        assert_eq!(a.hit(sat.x + sat.w / 2., sat.y + sat.h / 2.), -1);
        assert!(a.can_swipe(0));
        assert_eq!(a.compact_secondary_for(), "");
    }
    #[test]
    fn classic_and_mini_share_the_expanded_frame_and_music_targets() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.expanded = true;
        a.frame = Frame::expanded();
        for width in [140., 260., 480.] {
            a.settings["CompactWidth"] = json!(width);
            for name in [
                "SideBySide",
                "Separated",
                "LyricFocus",
                "LyricRight",
                "Immersive",
            ] {
                a.settings["MusicExpandedLayout"] = json!(name);
                a.music_mix = layout::MusicLayout::from_setting(name).position();
                a.cursor_inside = true;
                a.music_controls.snap(1.);
                let mut dimensions = None;
                for mode in ["Classic", "Mini"] {
                    a.settings["IslandSizeMode"] = json!(mode);
                    a.rebuild_layout();
                    let g = a.music_geometry();
                    let d = (a.layout.card.w, a.layout.card.h);
                    if let Some(expected) = dimensions {
                        assert_eq!(d, expected, "{name}/{mode}");
                    } else {
                        dimensions = Some(d);
                    }
                    for (id, r) in g.buttons {
                        let x = a.layout.card.x + r.x + r.w / 2.;
                        let y = a.layout.card.y + r.y + r.h / 2.;
                        assert_eq!(a.hit(x, y), id, "{name}/{mode}/{id}");
                    }
                    let r = g.progress_hit;
                    assert_eq!(
                        a.hit(
                            a.layout.card.x + r.x + r.w / 2.,
                            a.layout.card.y + r.y + r.h / 2.
                        ),
                        10
                    );
                }
            }
        }
    }
    #[test]
    fn every_activity_uses_the_music_frame_in_both_size_modes() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        for mode in ["Classic", "Mini"] {
            a.settings["IslandSizeMode"] = json!(mode);
            let expected = a.music_geometry().height;
            for selected in ["music", "update", "timer:test", "todos"] {
                a.selected = selected.into();
                a.panel = if selected == "music" {
                    None
                } else {
                    Some(Default::default())
                };
                assert_eq!(a.card_height(), expected, "{mode}/{selected}");
            }
        }
    }
}

#[cfg(test)]
mod stable_layout_tests {
    use super::*;
    #[test]
    fn disabling_animation_applies_the_new_bounds_in_the_same_frame() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.settings["Animation"] = json!(false);
        a.dirty = false;
        a.apply_compact_target(300., 44.);
        assert_eq!((a.layout.capsule.w, a.layout.capsule.h), (300., 44.));
        assert!(a.dirty);
        a.settings["IslandSizeMode"] = json!("Mini");
        a.apply_compact_target(240., 14.);
        assert_eq!((a.layout.capsule.w, a.layout.capsule.h), (240., 14.));
    }
    #[test]
    fn rebuilding_for_a_setting_preserves_the_live_capsule_width() {
        for mini in [false, true] {
            let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
            a.settings["IslandSizeMode"] = json!(if mini { "Mini" } else { "Classic" });
            a.compact_size = compact::Resize::new(390., if mini { 14. } else { 40. });
            a.rebuild_layout();
            assert_eq!(a.layout.capsule.w, 390.);
            assert_eq!(a.layout.capsule.h, if mini { 14. } else { 40. });
        }
    }
}

#[cfg(test)]
mod appearance_update_tests {
    use super::*;
    #[test]
    fn dragging_previews_without_saving_or_mutating_other_preferences() {
        unsafe {
            let mut app = App::new(PathBuf::new(), PathBuf::new(), true);
            let original = app.settings.clone();
            let reply=app.request(json!({"version":1,"command":"previewAppearance","value":{"key":"AcrylicCardFrost","value":90.}}));
            assert_eq!(reply["success"], true);
            assert_eq!(app.settings, original);
            assert_eq!(app.scene().acrylic_card_frost, 90.);
            assert!(app.dirty);
            assert_eq!(app.request(json!({"version":1,"command":"previewAppearance","value":{"key":"CoverBackground","value":true}}))["success"],false);
            assert_eq!(app.request(json!({"version":1,"command":"previewAppearance","value":{"key":"AcrylicCardFrost","value":101.}}))["success"],false);
            assert_eq!(app.scene().acrylic_card_frost, 90.);
            let generation = app.renderer_generation;
            let reply=app.request(json!({"version":1,"command":"update","value":{"key":"AcrylicCardFrost","value":25.}}));
            assert_eq!(reply["success"], true);
            assert_eq!(app.settings["AcrylicCardFrost"], 25.);
            assert!(app.acrylic_preview.get("AcrylicCardFrost").is_none());
            assert_eq!(app.scene().acrylic_card_frost, 25.);
            assert_eq!(app.renderer_generation, generation);
            app.request(json!({"version":1,"command":"update","value":{"key":"ExpandedMonetEnabled","value":true}}));
            app.request(json!({"version":1,"command":"update","value":{"key":"ExpandedCoverOverlay","value":true}}));
            for enabled in [false, true] {
                app.request(json!({"version":1,"command":"update","value":{"key":"CoverBackground","value":enabled}}));
                assert_eq!(app.scene().cover_background, enabled);
                assert!(app.scene().expanded_cover_background);
                assert!(app.scene().cover_overlay);
            }
        }
    }
}

#[cfg(test)]
mod progress_rail_tests {
    use super::*;
    #[test]
    fn every_layout_and_both_card_sizes_share_the_four_pixel_rail() {
        unsafe {
            let mut app = App::new(PathBuf::new(), PathBuf::new(), true);
            for size in ["Classic", "Mini"] {
                app.settings["IslandSizeMode"] = json!(size);
                for layout in ["SideBySide", "Separated", "LyricFocus", "LyricRight"] {
                    app.settings["MusicExpandedLayout"] = json!(layout);
                    let g = app.music_geometry();
                    assert_eq!(g.track.h, 4., "{size} {layout}");
                    assert!(g.elapsed.x <= g.track.x + 1. || size == "Mini");
                    assert!(g.total.x + g.total.w >= g.track.x + g.track.w - 1. || size == "Mini");
                }
            }
        }
    }
}

#[cfg(test)]
mod translation_button_tests {
    use super::*;
    /// Hiding the 译 button must remove the paint and the pointer target together; a
    /// transparent hot spot would toggle the language from a place with no visible button.
    #[test]
    fn hiding_the_translation_button_removes_paint_and_hit_target() {
        unsafe {
            let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
            a.opacity = 1.;
            a.expanded = true;
            a.settings["MusicExpandedLayout"] = json!("SideBySide");
            a.music_mix = 0.;
            a.frame = Frame::expanded();
            a.transition = None;
            let c = a.layout.card;
            let g = a.music_geometry();
            let button = g
                .buttons
                .iter()
                .find(|(id, _)| *id == 6)
                .expect("expanded card keeps a 译 slot")
                .1;
            let (x, y) = (
                c.x + button.x + button.w / 2.,
                c.y + button.y + button.h / 2.,
            );
            a.settings["TranslationButton"] = json!(true);
            assert_eq!(a.hit(x, y), 6);
            assert!(a.scene().translation_button);
            a.settings["TranslationButton"] = json!(false);
            assert_eq!(a.hit(x, y), 0);
            assert!(!a.scene().translation_button);
            let before = (
                a.settings["Translation"].clone(),
                a.settings["CompactTranslation"].clone(),
            );
            a.activate(6);
            assert_eq!(
                (
                    a.settings["Translation"].clone(),
                    a.settings["CompactTranslation"].clone()
                ),
                before
            );
            a.settings["TranslationButton"] = json!(true);
            assert_eq!(a.hit(x, y), 6);
        }
    }
}

#[cfg(test)]
mod immersive_hover_tests {
    use super::*;
    fn app() -> App {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.expanded = true;
        a.frame = Frame::expanded();
        a.settings["MusicExpandedLayout"] = json!("Immersive");
        a.music_mix = 4.;
        a.rebuild_layout();
        a
    }
    #[test]
    fn hidden_music_controls_have_no_progress_or_button_hotspots() {
        let mut a = app();
        let c = a.layout.card;
        let g = a.music_geometry();
        for (_, r) in g.buttons {
            assert_eq!(a.hit(c.x + r.x + r.w / 2., c.y + r.y + r.h / 2.), 0);
        }
        let r = g.progress_hit;
        assert_eq!(a.hit(c.x + r.x + 10., c.y + r.y + 5.), 0);
        assert_eq!(a.scene().controls_opacity, 0.);
        a.cursor_inside = true;
        a.music_controls.snap(1.);
        for (id, r) in g.buttons {
            assert_eq!(a.hit(c.x + r.x + r.w / 2., c.y + r.y + r.h / 2.), id);
        }
        assert_eq!(a.hit(c.x + r.x + 10., c.y + r.y + 5.), 10);
        a.cursor_inside = false;
        assert_eq!(a.hit(c.x + r.x + 10., c.y + r.y + 5.), 0);
    }
    #[test]
    fn timeline_drag_keeps_controls_until_release_and_reduced_motion_settles() {
        let mut a = app();
        a.music_controls.snap(1.);
        a.seeking = true;
        assert!(a.music_controls_visible());
        a.seeking = false;
        assert!(!a.music_controls_visible());
        a.music_controls.target = 0.;
        for _ in 0..180 {
            a.music_controls.tick(1. / 60., false);
        }
        assert!(a.music_controls.settled());
        assert_eq!(a.music_controls.value, 0.);
        a.music_controls.target = 1.;
        a.music_controls.tick(1. / 60., true);
        assert_eq!(a.music_controls.value, 1.);
        assert!(a.music_controls.settled());
    }
    #[test]
    fn header_artwork_is_removed_but_title_open_and_lyric_context_remain() {
        let mut a = app();
        let c = a.layout.card;
        assert_eq!(a.hit(c.x + 44., c.y + 40.), 0);
        a.settings["MusicClickOpenApp"] = json!(true);
        assert_eq!(a.hit(c.x + 44., c.y + 40.), 13);
        a.snapshot["frame"] =
            json!({"previousLine":"previous","nextLine":"next","currentTranslation":"translation"});
        let s = a.scene();
        assert_eq!(
            (s.previous_context, s.next_context, s.secondary),
            ("previous", "next", "translation")
        );
        let height = a.layout.card.h;
        a.cursor_inside = true;
        a.music_controls.snap(1.);
        assert_eq!(a.layout.card.h, height);
        assert_eq!(a.scene().controls_opacity, 1.);
    }
}

#[cfg(test)]
mod netease_seek_request_tests {
    use super::*;
    #[test]
    fn ipc_seek_requires_numeric_seconds_and_accepts_valid_demo_position() {
        unsafe {
            let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
            for value in [
                json!({"action":"seek"}),
                json!({"action":"seek","seconds":"12"}),
                json!({"action":"seek","seconds":null}),
                json!({"action":"seek","seconds":-1.}),
            ] {
                assert_eq!(
                    a.request(json!({"version":1,"command":"media","value":value}))["success"],
                    false
                );
            }
            let reply = a.request(
                json!({"version":1,"command":"media","value":{"action":"seek","seconds":72.5}}),
            );
            assert_eq!(reply["success"], true);
            assert_eq!(a.snapshot["positionSeconds"], 72.5);
            a.demo = false;
            assert_eq!(
                a.request(
                    json!({"version":1,"command":"media","value":{"action":"seek","seconds":12.}})
                )["success"],
                false
            );
        }
    }
}

#[cfg(test)]
mod timer_boundary_tests {
    use super::*;
    fn app() -> App {
        App::new(PathBuf::new(), PathBuf::new(), true)
    }
    fn flow() -> Value {
        json!({"mode":"workflow","stages":[{"name":"学习","seconds":60},{"name":"休息","seconds":60},{"name":"学习","seconds":60}]})
    }
    #[test]
    fn resume_settles_paused_time_without_resetting_other_clocks() {
        let mut a = app();
        let id = a.timers.create(&flow()).unwrap();
        a.timers.action(&id, "pause").unwrap();
        a.timers.create(&json!({"mode":"stopwatch"})).unwrap();
        a.last_timer = Instant::now() - Duration::from_secs(300);
        unsafe {
            assert_eq!(a.request(json!({"version":1,"command":"timerAction","value":{"id":id,"action":"resume","revision":2}}))["success"],true);
        }
        assert_eq!(a.timers.items[0].elapsed_ms, 0);
        assert!(a.timers.items[1].elapsed_ms >= 300_000);
        let now = a.last_timer + Duration::from_secs(1);
        a.advance_timers(now);
        assert!((1000..=1001).contains(&a.timers.items[0].elapsed_ms));
        assert_eq!(a.timers.items[0].workflow.as_ref().unwrap().stage_index, 0);
    }
    #[test]
    fn next_does_not_inherit_time_spent_waiting() {
        let mut a = app();
        let id = a.timers.create(&flow()).unwrap();
        a.timers.advance(60_000);
        let revision = a.timers.items[0].revision;
        a.last_timer = Instant::now() - Duration::from_secs(600);
        unsafe {
            assert_eq!(a.request(json!({"version":1,"command":"timerAction","value":{"id":id,"action":"next","revision":revision}}))["success"],true);
        }
        assert_eq!(a.timers.items[0].workflow.as_ref().unwrap().stage_index, 1);
        assert_eq!(a.timers.items[0].elapsed_ms, 0);
        let now = a.last_timer + Duration::from_secs(1);
        a.advance_timers(now);
        assert!(a.timers.items[0].running);
        assert!(!a.timers.items[0].workflow.as_ref().unwrap().waiting);
        assert!((1000..=1001).contains(&a.timers.items[0].elapsed_ms));
    }
    #[test]
    fn new_timer_does_not_inherit_the_previous_idle_gap() {
        let mut a = app();
        a.last_timer = Instant::now() - Duration::from_secs(600);
        unsafe {
            assert_eq!(a.request(json!({"version":1,"command":"timerCreate","value":{"mode":"pomodoro","focus":2,"rest":1,"rounds":3}}))["success"],true);
        }
        let now = a.last_timer + Duration::from_millis(500);
        a.advance_timers(now);
        assert_eq!(a.timers.items[0].phase, "focus");
        assert!((500..=501).contains(&a.timers.items[0].elapsed_ms));
        assert_eq!(a.timers.items[0].round, 1);
    }
    #[test]
    fn native_duplicate_skip_is_rejected() {
        let mut a = app();
        let id = a.timers.create(&flow()).unwrap();
        let command = json!({"version":1,"command":"timerAction","value":{"id":id,"action":"skip","revision":1}});
        unsafe {
            assert_eq!(a.request(command.clone())["success"], true);
            assert_eq!(a.request(command)["success"], false);
        }
        assert_eq!(a.timers.items[0].workflow.as_ref().unwrap().stage_index, 1);
        assert_eq!(a.timers.items[0].clock(), "01:00");
    }
    #[test]
    fn primary_button_cannot_turn_a_stale_pause_into_next() {
        let mut a = app();
        let id = a.timers.create(&flow()).unwrap();
        a.selected = format!("timer:{id}");
        a.last_timer = Instant::now() - Duration::from_secs(120);
        a.active_timer_action("toggle");
        let t = &a.timers.items[0];
        assert_eq!(t.workflow.as_ref().unwrap().stage_index, 0);
        assert!(t.workflow.as_ref().unwrap().waiting);
        assert!(!t.running);
        assert!(a.error.is_some());
    }
}

#[cfg(test)]
mod custom_composition_tests {
    use super::*;
    #[test]
    fn reordered_native_targets_follow_paint_in_both_size_modes() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.expanded = true;
        a.frame = Frame::expanded();
        a.settings["Animation"] = json!(false);
        a.settings["TranslationButton"] = json!(true);
        a.music_custom.enabled = true;
        a.music_custom.move_module(5, 0);
        for mode in ["Classic", "Mini"] {
            a.settings["IslandSizeMode"] = json!(mode);
            a.rebuild_layout();
            let g = a.music_geometry();
            assert!(a.layout.host.h >= g.height + 48.);
            assert_eq!(a.scene().custom_geometry.unwrap().buttons, g.buttons);
            for (id, r) in g.buttons {
                assert_eq!(
                    a.hit(
                        a.layout.card.x + r.x + r.w / 2.,
                        a.layout.card.y + r.y + r.h / 2.
                    ),
                    id
                );
            }
            let r = g.progress_hit;
            assert_eq!(
                a.hit(
                    a.layout.card.x + r.x + r.w / 2.,
                    a.layout.card.y + r.y + r.h / 2.
                ),
                10
            );
        }
    }
    #[test]
    fn hidden_controls_and_progress_leave_no_stale_hotspots() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.opacity = 1.;
        a.expanded = true;
        a.frame = Frame::expanded();
        a.music_custom.enabled = true;
        a.music_custom.controls = vec![4];
        for s in &mut a.music_custom.modules {
            if ![
                lyric_core::custom_layout::Module::Header,
                lyric_core::custom_layout::Module::Controls,
            ]
            .contains(&s.kind)
            {
                s.visible = false;
            }
        }
        a.rebuild_layout();
        let g = a.music_geometry();
        assert_eq!(g.progress_hit.w, 0.);
        assert_eq!(g.buttons.iter().filter(|(_, r)| r.w > 0.).count(), 1);
        for x in (0..a.layout.card.w as usize).step_by(10) {
            for y in (0..a.layout.card.h as usize).step_by(10) {
                assert!(![2, 3, 5, 6, 9, 10, 14, 15]
                    .contains(&a.hit(a.layout.card.x + x as f64, a.layout.card.y + y as f64)));
            }
        }
    }
    #[test]
    fn enabling_custom_layout_keeps_the_music_card_frame() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        for mix in [0., 1., 2., 3., 4.] {
            for font in [11., 14., 20.] {
                a.music_mix = mix;
                a.settings["FontSize"] = json!(font);
                a.music_custom.enabled = false;
                a.rebuild_layout();
                let original = (a.layout.card.w, a.layout.card.h, a.layout.host.h);
                a.music_custom.enabled = true;
                a.music_custom.set_element(36, 220., 600., None);
                a.rebuild_layout();
                assert_eq!(original, (a.layout.card.w, a.layout.card.h, a.layout.host.h));
            }
        }
    }
    #[test]
    fn custom_retarget_starts_at_the_presented_geometry_and_reduced_motion_is_immediate() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        let from = a.music_geometry();
        a.music_custom.enabled = true;
        a.music_custom.move_module(5, 0);
        a.custom_from = Some(from);
        a.custom_start = Instant::now();
        let first = a.music_geometry();
        assert!((first.buttons[3].1.y - from.buttons[3].1.y).abs() < 0.5);
        a.custom_from = None;
        a.settings["Animation"] = json!(false);
        assert_eq!(
            a.music_geometry().buttons,
            a.music_custom.geometry(a.layout.card.w, 1., 14., a.music_mix).buttons
        );
    }
}

#[cfg(test)]
mod transient_exit_tests {
    use super::*;
    fn idle() -> App {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.snapshot = json!({"track":null,"isPlaying":false});
        a.opacity = 1.;
        a.fade_to = 1.;
        a.display_wanted = true;
        a
    }
    #[test]
    fn idle_notice_never_hands_over_to_an_empty_music_capsule() {
        let mut a = idle();
        let (r, rad) = a.shape();
        a.banner.show(
            notifications::Notice::default(),
            r,
            rad,
            a.layout.host.w,
            80.,
            0.,
            true,
        );
        a.banner.dismiss(r, rad, 0.4, true, a.base_needed());
        for i in 0..50 {
            let t = 0.4 + i as f64 / 60.;
            assert!(a.scene().transient_only);
            a.banner.tick(r, rad, t, false, true, a.base_needed());
            if !a.banner.visible() {
                a.finish_transient();
                assert_eq!(a.opacity, 0.);
                assert!(!a.display_wanted);
                assert!(a.hide_at.is_none());
                return;
            }
        }
        panic!("notification exit did not settle");
    }
    #[test]
    fn playback_arriving_during_exit_restores_the_live_capsule() {
        let mut a = idle();
        a.snapshot = json!({"track":{"id":"song","title":"音乐"},"isPlaying":true});
        assert!(a.base_needed());
        assert!(!a.scene().transient_only);
        a.finish_transient();
        assert_eq!(a.opacity, 1.);
    }
    #[test]
    fn paused_track_follows_visibility_preference_and_running_timer_wins() {
        let mut a = idle();
        a.snapshot = json!({"track":{"id":"song","title":"音乐"},"isPlaying":false});
        assert!(!a.base_needed());
        a.settings["HideWhenPaused"] = json!(false);
        assert!(a.base_needed());
        a.settings["HideWhenPaused"] = json!(true);
        a.held = true;
        assert!(a.base_needed());
    }
    #[test]
    fn queued_notice_and_active_hint_prevent_early_surface_retirement() {
        let mut a = idle();
        a.queue.push_back(Queued {
            notice: Default::default(),
            todo: None,
            chime: false,
        });
        a.finish_transient();
        assert_eq!(a.opacity, 1.);
        a.queue.clear();
        let (r, rad) = a.shape();
        a.osd_banner
            .show_compact(Default::default(), r, rad, a.layout.host.w, 0., false);
        a.finish_transient();
        assert_eq!(a.opacity, 1.);
        a.osd_banner.dismiss(r, rad, 0., false, a.base_needed());
        a.osd_banner.tick(r, rad, 1., false, false, a.base_needed());
        a.finish_transient();
        assert_eq!(a.opacity, 0.);
    }
    #[test]
    fn notice_wakes_hidden_island_and_restores_hidden_after_dismiss() {
        let mut a = idle();
        a.hide_by_gesture();
        assert!(a.user_hidden);
        assert!(a.wake_for_transient_notice());
        assert!(!a.user_hidden && a.restore_hidden);
        a.show_notice(
            notifications::Notice {
                app_id: "QQNT".into(),
                app_name: "QQ".into(),
                title: "测试消息".into(),
                body: "手动隐藏期间也能正常提醒".into(),
                ..Default::default()
            },
            false,
            None,
            false,
        );
        assert!(a.banner.visible());
        let (r, rad) = a.shape();
        a.banner.dismiss(r, rad, 0., false, a.base_needed());
        a.banner.tick(r, rad, 1., false, false, a.base_needed());
        a.finish_transient();
        assert!(a.user_hidden && !a.restore_hidden);
        assert!(a.hidden_edge());
        assert_eq!(a.opacity, 1.);
    }
    #[test]
    fn edge_hover_reveal_defaults_off_and_respects_dwell_and_auto_hide() {
        let mut a = idle();
        assert_eq!(a.flag("EdgeHoverRevealEnabled", false), false);
        assert_eq!(a.scene().background_opacity, 1.);
        a.settings["IslandOpacity"] = json!(65.);
        assert!((a.scene().background_opacity - 0.65).abs() < 0.01);
        let mon = RECT { left: 0, top: 0, right: 1920, bottom: 1080 };
        assert!(edge_hit_test(POINT { x: 960, y: 2 }, mon, 260., 0.5, 1.));
        assert!(!edge_hit_test(POINT { x: 960, y: 40 }, mon, 260., 0.5, 1.));
        a.settings["HiddenDisplay"] = json!("None");
        a.hide_by_gesture();
        let t0 = Instant::now();
        a.step_edge_reveal(true, false, t0);
        assert!(!a.edge_revealed);
        a.step_edge_reveal(true, false, t0 + Duration::from_millis(200));
        assert!(a.edge_revealed && a.fade_to == 1.);
        a.step_edge_reveal(false, false, t0 + Duration::from_millis(300));
        assert!(a.edge_revealed);
        a.step_edge_reveal(false, false, t0 + Duration::from_millis(1300));
        assert!(!a.edge_revealed && a.fade_to == 0. && a.user_hidden);
    }
}

#[cfg(test)]
mod manual_retirement_tests {
    use super::*;
    #[test]
    fn closing_a_tray_expansion_keeps_its_content_and_starts_retirement_immediately() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.snapshot = json!({"track":null,"isPlaying":false});
        a.held = true;
        a.expanded = true;
        a.frame = Frame::expanded();
        a.opacity = 1.;
        a.fade_to = 1.;
        a.display_wanted = true;
        assert!(!a.scene().transient_only);
        a.held = false;
        a.retarget(false);
        assert_eq!(a.fade_to, 0.);
        assert!(!a.display_wanted);
        assert!(a.hide_at.is_none());
        assert!(!a.scene().transient_only);
        assert!(!a.scene().lyric.trim().is_empty());
    }
    #[test]
    fn closing_playing_music_preserves_the_capsule() {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.snapshot = json!({"track":{"id":"song","title":"海边来信"},"isPlaying":true});
        a.expanded = true;
        a.frame = Frame::expanded();
        a.opacity = 1.;
        a.fade_to = 1.;
        a.retarget(false);
        assert_eq!(a.fade_to, 1.);
        assert!(a.base_needed());
        assert!(!a.scene().transient_only);
    }
}

// Stable C ABI; no Rust values or allocator ownership cross the DLL boundary.
#[no_mangle]
pub extern "C" fn dotisle_module_abi() -> u32 { 1 }
#[no_mangle]
pub extern "C" fn dotisle_module_run() -> i32 {
    match std::panic::catch_unwind(main) { Ok(()) => 0, Err(_) => 1 }
}

#[no_mangle]
pub extern "C" fn dotisle_module_kind() -> u32 { 1 }

fn describe_only(_:serde_json::Value)->std::result::Result<serde_json::Value,String>{Err("bootstrap module has no service commands".into())}
dotisle_module_api::export_service!("core",describe_only);

fn edge_hit_test(cursor: POINT, monitor: RECT, capsule_w: f64, center_x: f64, scale: f64) -> bool {
    let top_band = (6. * scale).round().max(4.) as i32;
    if cursor.y < monitor.top || cursor.y > monitor.top + top_band {
        return false;
    }
    let mw = (monitor.right - monitor.left).max(1) as f64;
    let cx = monitor.left as f64 + mw * center_x.clamp(0.05, 0.95);
    let half = ((capsule_w * 0.65).max(110.) * scale).round();
    (cursor.x as f64 - cx).abs() <= half
}

fn weather_description(code:u64)->&'static str{match code{0=>"晴",1..=3=>"多云",45|48=>"有雾",51..=67|80..=82=>"有雨",71..=77|85|86=>"有雪",95..=99=>"雷雨",_=>"天气"}}

#[cfg(test)]
mod resident_tests {
    use super::*;
    fn idle() -> App {
        let mut a = App::new(PathBuf::new(), PathBuf::new(), true);
        a.snapshot = json!({"track":null,"isPlaying":false});
        a.expanded = false;
        a.held = false;
        a.settings["IslandKeepVisibleEnabled"] = json!(true);
        a
    }
    #[test]
    fn residency_keeps_idle_surface_after_transients_and_cancels_pending_hide() {
        let mut a = idle();
        a.demo = false;
        a.display_wanted = false;
        a.hide_at = Some(Instant::now());
        a.poll();
        assert!(a.display_wanted && a.base_needed());
        assert!(a.hide_at.is_none());
        a.finish_transient();
        assert_eq!(a.fade_to, 1.);
        assert!(a.resident_idle());
        a.settings["IslandKeepVisibleEnabled"] = json!(false);
        a.finish_transient();
        assert_eq!(a.opacity, 0.);
    }
    #[test]
    fn idle_island_is_opaque_and_interactive_without_music() {
        let mut a = idle();
        a.opacity = 1.;
        assert_eq!(a.scene().background_opacity, 1.);
        assert_eq!(a.scene().opacity, 1.);
        assert_ne!(a.hit(a.layout.capsule.x + a.layout.capsule.w / 2., a.layout.capsule.y + a.layout.capsule.h / 2.), -1);
        a.snapshot = json!({"track":{"id":"song","title":"音乐"},"isPlaying":true});
        assert!(!a.resident_idle());
        a.snapshot["isPlaying"] = json!(false);
        assert!(a.resident_idle());
        let (r, radius) = a.shape();
        a.banner.show(Default::default(), r, radius, a.layout.host.w, 80., 0., false);
        assert!(!a.resident_idle());
    }
    #[test]
    fn verification_restores_manual_hide_even_when_residency_is_enabled() {
        let mut a=idle();a.restore_hidden=true;a.user_hidden=false;a.opacity=1.;
        a.finish_transient();
        assert!(a.user_hidden && !a.restore_hidden);assert!(a.hidden_edge());assert_eq!(a.opacity,1.);
    }
    #[test]
    fn residency_does_not_override_explicit_hide() {
        let mut a = idle();
        a.demo = false;
        a.user_hidden = true;
        a.display_wanted = false;
        a.opacity = 0.; a.fade_to = 0.;
        a.poll();
        assert!(a.display_wanted);
        assert_eq!(a.fade_to, 0.);
    }
}
