//! DotIsle-Allegro 独立小组件引擎进程入口：纯原生无控制台窗口运行，与主进程隔离便于独立能效与显存优化。
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

fn main() {
    let mut data_dir = default_data_dir();
    let mut pipe = std::env::var("LYRICISLAND_TEST_PIPE")
        .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into());
    let mut keep_alive = false;
    let mut demo = false;

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--data-dir" => {
                if let Some(val) = args.next() {
                    if !val.trim().is_empty() {
                        data_dir = PathBuf::from(val);
                    }
                }
            }
            "--pipe" => {
                if let Some(val) = args.next() {
                    if !val.trim().is_empty() {
                        pipe = val;
                    }
                }
            }
            "--keep-alive" => keep_alive = true,
            "--demo" => {
                demo = true;
                keep_alive = true;
            }
            _ => {}
        }
    }

    if let Err(err) = dotisle_desktop::run_standalone(data_dir.clone(), pipe, keep_alive, demo) {
        let _ = std::fs::create_dir_all(&data_dir);
        let log_path = data_dir.join("allegro-startup.log");
        let _ = std::fs::write(&log_path, format!("DotIsle-Allegro 退出: {err}\n"));
        eprintln!("DotIsle-Allegro 退出: {err}");
        std::process::exit(1);
    }
}

fn default_data_dir() -> PathBuf {
    if let Ok(v) = std::env::var("LYRICISLAND_NATIVE_DATA") {
        if !v.trim().is_empty() {
            return PathBuf::from(v);
        }
    }
    if let Ok(local) = std::env::var("LOCALAPPDATA") {
        return PathBuf::from(local).join("DotIsle");
    }
    std::env::temp_dir().join("DotIsle")
}
