#![windows_subsystem = "windows"]
mod acrylic;
mod appearance;
mod background;
mod center_checks;
mod center_layout;
mod cover_art;
mod cover_checks;
mod desktop_cards;
mod desktop_host;
mod draw;
mod expressive_checks;
mod font_picker;
mod frame_trace;
mod gestures;
mod gpu;
mod hybrid_checks;
mod island_settings;
mod layout_checks;
mod layout_editor;
mod miuix;
mod model;
mod motion;
mod music_card;
mod native_sync;
mod notification_center;
mod overlay_motion;
mod overview;
mod picker;
mod pixel;
mod pixel_glyphs;
mod platform;
mod polish_checks;
mod preferences_ui;
mod remote;
mod settings_hub;
mod settings_slider;
mod shell;
mod software_update;
mod swiss;
mod terminal;
mod themes;
mod widget_extras;

mod theme_icons;
mod theme_regression;
mod timeline;
mod timer_surface;
mod tool_preferences;
mod tutorial;
mod tutorial_checks;
mod verify;
mod views;
mod widget_checks;
mod widgets;
mod workflow;
use std::path::PathBuf;
pub static STARTED: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
fn main() {
    STARTED.get_or_init(std::time::Instant::now);
    let args = std::env::args().collect::<Vec<_>>();
    if let Some(i) = args.iter().position(|a| a == "--center-capture") {
        let out = PathBuf::from(args.get(i + 1).expect("capture directory"));
        if let Err(error) = center_checks::run(&out) {
            let _ = std::fs::create_dir_all(&out);
            let _ = std::fs::write(out.join("failure.txt"), error);
            std::process::exit(1);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--350-capture") {
        let out = PathBuf::from(args.get(i + 1).expect("capture directory"));
        let _ = std::fs::create_dir_all(&out);
        let result = widget_checks::run(&out, &mut draw::Text::new(), &mut gpu::Gpu::new());
        let success = result.is_ok();
        let report = match result {
            Ok(captures) => serde_json::json!({"success":true,"captures":captures}),
            Err(error) => serde_json::json!({"success":false,"error":error}),
        };
        let _ = std::fs::write(out.join("350-report.json"), report.to_string());
        if !success {
            std::process::exit(1)
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--theme-capture") {
        let path = args
            .get(i + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("theme-capture"));
        let _ = std::fs::create_dir_all(&path);
        let panic_path = path.join("panic.txt");
        std::panic::set_hook(Box::new(move |info| {
            let _ = std::fs::write(
                &panic_path,
                format!("{info}\n{}", std::backtrace::Backtrace::force_capture()),
            );
        }));
        if let Err(e) = theme_regression::run(&path) {
            let _ = std::fs::create_dir_all(&path);
            let _ = std::fs::write(path.join("failure.txt"), e);
            std::process::exit(1);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--verify") {
        let path = args
            .get(i + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("verification"));
        if let Err(e) = verify::run(&path) {
            let _ = std::fs::create_dir_all(&path);
            let _ = std::fs::write(path.join("failure.txt"), e);
            std::process::exit(1);
        }
        return;
    }
    if let Some(i) = args.iter().position(|a| a == "--cover-capture") {
        let out = args
            .get(i + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("cover-capture"));
        if let Err(e) = cover_checks::run(&out) {
            let _ = std::fs::create_dir_all(&out);
            let _ = std::fs::write(out.join("failure.txt"), e);
            std::process::exit(1);
        }
        return;
    }
    let data = args
        .iter()
        .position(|a| a == "--data-dir")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("LYRICISLAND_NATIVE_DATA").map(PathBuf::from))
        .unwrap_or_else(default_data_dir);
    let demo = args.iter().any(|a| a == "--preview");
    let _instance = if demo {
        None
    } else {
        match platform::center_instance(&data) {
            Ok(Some(guard)) => Some(guard),
            Ok(None) => {
                if !args.iter().any(|a| a == "--warm") {
                    let _ = remote::request("settings", serde_json::json!({"page":"overview"}));
                }
                return;
            }
            Err(_) => None,
        }
    };
    let mut model = model::Model::new(data, demo);
    if demo
        && args.iter().any(|a| {
            matches!(
                a.as_str(),
                "--perf-run" | "--perf-startup" | "--perf-interactions"
            )
        })
    {
        model.tutorial_checked = true;
        model.page = model::Page::Home;
    }
    if let Some(i) = args.iter().position(|a| a == "--page") {
        if args.get(i + 1).is_some_and(|s| s == "island") {
            model.page = model::Page::Settings;
            model.settings_tab = 128;
        }
    }
    if let Some(i) = args.iter().position(|a| a == "--island-tab") {
        model.island_tab = args
            .get(i + 1)
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
            .min(6);
        model.settings_tab = 128 + model.island_tab;
    }
    if args.iter().any(|a| a == "--tutorial") {
        if demo {
            model.dispatch(model::Action::TutorialStart);
        } else {
            model.tutorial_requested = true;
        }
    }
    shell::run(model);
}

fn default_data_dir() -> PathBuf {
    if let Some(base) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(std::path::Path::to_path_buf))
    {
        if base.join("Portable.json").is_file() {
            return base.join("Data");
        }
        if let Ok(bytes) = std::fs::read(base.join("Installed.json")) {
            if let Ok(value) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                if let Some(path) = value["dataDirectory"].as_str() {
                    return PathBuf::from(path);
                }
            }
        }
    }
    dotisle_domain::preferences::data_dir()
}
