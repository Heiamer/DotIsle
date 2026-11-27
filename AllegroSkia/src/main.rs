#![windows_subsystem = "windows"]
mod ai_quota;
mod app;
mod async_io;
mod file_shelf;
mod platform;
mod reminders;
mod state;
mod surface;
mod ui;

fn log_error(message: &str) {
    let root =
        std::path::PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
            .join("DotIsle");
    let _ = std::fs::create_dir_all(&root);
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("allegro-skia-startup.log"))
    {
        let _ = writeln!(f, "{:?} {message}", std::time::SystemTime::now());
    }
}
fn main() {
    std::panic::set_hook(Box::new(|info| log_error(&info.to_string())));
    if let Err(e) = run() {
        log_error(&e);
        eprintln!("{e}");
        std::process::exit(1);
    }
}
fn run() -> Result<(), String> {
    let cli = state::Cli::parse(std::env::args().skip(1))?;
    if let Some(dir) = &cli.preview {
        return ui::render_previews(dir);
    }
    app::run(cli)
}
