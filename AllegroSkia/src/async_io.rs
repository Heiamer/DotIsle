use crate::{
    platform::{self, Wake},
    state::{Application, Board},
};
use skia_safe::{
    AlphaType, ColorType, Data, FilterMode, Image, ImageInfo, MipmapMode, Paint, Rect,
    SamplingOptions, TileMode, image_filters, surfaces,
};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, mpsc},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use windows::Win32::{
    Foundation::FILETIME,
    NetworkManagement::IpHelper::{FreeMibTable, GetIfTable2},
    Storage::FileSystem::GetDiskFreeSpaceExW,
    System::{
        Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS},
        SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX},
        Threading::GetSystemTimes,
    },
};

#[derive(Clone, Debug, Default)]
pub struct Metrics {
    pub cpu: Option<f32>,
    pub memory: Option<f32>,
    pub battery: Option<u8>,
    pub disk: Option<f32>,
    pub download: f64,
    pub upload: f64,
}
pub struct Snapshot {
    pub wallpaper: Option<Image>,
    /// 后台预下采样并预先完成 4 挡高斯模糊的壁纸缓存，消除主线程每帧重复模糊导致的卡顿。
    pub wallpaper_blurred: [Option<Image>; 4],
    pub wallpaper_status: String,
    pub apps: Vec<Application>,
    pub indexed: bool,
    pub metrics: Metrics,
    pub clipboard: Vec<String>,
    pub ai_quota: crate::ai_quota::QuotaSnapshot,
    pub errors: Vec<String>,
    pub pipe_status: String,
    pub live: serde_json::Value,
    pub revision: u64,
}
impl Default for Snapshot {
    fn default() -> Self {
        Self {
            wallpaper: None,
            wallpaper_blurred: [None, None, None, None],
            wallpaper_status: String::new(),
            apps: Vec::new(),
            indexed: false,
            metrics: Metrics::default(),
            clipboard: Vec::new(),
            ai_quota: crate::ai_quota::demo_snapshot(),
            errors: Vec::new(),
            pipe_status: String::new(),
            live: serde_json::Value::Null,
            revision: 0,
        }
    }
}
impl Snapshot {
    /// 获取对应亚克力挡位（0..=3）的已预模糊壁纸，若尚未就绪则回退原图。
    pub fn blurred_wallpaper(&self, level: u8) -> Option<&Image> {
        let idx = level.min(3) as usize;
        self.wallpaper_blurred[idx]
            .as_ref()
            .or(self.wallpaper.as_ref())
    }
}

/// 在后台线程将高分辨率壁纸下采样至长边 <= 480px，并一次性预生成 4 挡模糊缓存图。
pub fn preblur_wallpaper(source: &Image) -> [Option<Image>; 4] {
    let w = source.width().max(1) as f32;
    let h = source.height().max(1) as f32;
    let scale = (480.0 / w.max(h)).min(1.0);
    let tw = (w * scale).round().clamp(32.0, 480.0) as i32;
    let th = (h * scale).round().clamp(32.0, 480.0) as i32;
    let sigmas = [5.5_f32, 8.0, 10.5, 13.5];
    let dst = Rect::from_xywh(0., 0., tw as f32, th as f32);
    let mut out = [None, None, None, None];
    for (idx, sigma) in sigmas.into_iter().enumerate() {
        if let Some(mut s) = surfaces::raster_n32_premul((tw, th)) {
            let mut p = Paint::default();
            p.set_anti_alias(true);
            p.set_image_filter(image_filters::blur(
                (sigma, sigma),
                TileMode::Clamp,
                None,
                None,
            ));
            s.canvas().draw_image_rect_with_sampling_options(
                source,
                None,
                dst,
                SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                &p,
            );
            out[idx] = Some(s.image_snapshot());
        }
    }
    out
}
pub enum Request {
    HostStatus(PathBuf, serde_json::Value),
    Save(PathBuf, Board),
    Copy(String),
    Launch(PathBuf),
    LoadBoards(PathBuf),
    RefreshAiQuota(PathBuf),
    OpenAiQuotaConfig(PathBuf),
    SaveFileShelf(PathBuf, crate::file_shelf::FileShelf),
    SaveSettings(PathBuf, crate::state::SidebarSettings),
    SaveTodos(PathBuf, crate::state::TodoList),
    SaveReminders(PathBuf, crate::reminders::ReminderState),
    Stop,
}
pub struct Background {
    pub shared: Arc<Mutex<Snapshot>>,
    pub loaded: mpsc::Receiver<(Board, Board)>,
    pub settings: mpsc::Receiver<crate::state::SidebarSettings>,
    tx: mpsc::Sender<Request>,
    join: Option<JoinHandle<()>>,
}
impl Background {
    pub fn start(wake: Wake, pipe: Option<String>) -> Result<Self, String> {
        let shared = Arc::new(Mutex::new(Snapshot::default()));
        let state = shared.clone();
        let (tx, rx) = mpsc::channel();
        let (loaded_tx, loaded) = mpsc::channel();
        let (settings_tx, settings) = mpsc::channel();
        let join = thread::Builder::new()
            .name("allegro-skia-io".into())
            .spawn(move || {
                // 启动扫描、解码和采样全部留在后台；前端只尝试获取快照锁。
                let wallpaper = wallpaper_path().and_then(|p| decode_wallpaper(&p));
                let apps = index_applications(&start_menu_roots());
                {
                    let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                    match wallpaper {
                        Ok(i) => {
                            s.wallpaper_blurred = preblur_wallpaper(&i);
                            s.wallpaper = Some(i);
                            s.wallpaper_status = "已缓存".into()
                        }
                        Err(e) => s.wallpaper_status = e,
                    };
                    s.apps = apps;
                    s.indexed = true;
                    s.pipe_status = pipe.map_or("离线独立模式".into(), |p| {
                        format!("预留通道：{p}（离线）")
                    });
                    s.revision += 1;
                }
                wake();
                let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
                let clipboard_owner = unsafe {
                    platform::CreateWindowExW(
                        0,
                        class.as_ptr(),
                        class.as_ptr(),
                        0,
                        0,
                        0,
                        0,
                        0,
                        -3isize as platform::Hwnd,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    )
                };
                let mut sample = Sampler::default();
                let mut next = Instant::now();
                let mut next_ai = Instant::now() + Duration::from_secs(300);
                let mut ai_dir: Option<PathBuf> = None;
                let mut sequence = 0;
                let mut watched = [Vec::new(),Vec::new(),Vec::new()];
                let mut live_bytes=Vec::new();
                loop {
                    match rx.recv_timeout(
                        next.saturating_duration_since(Instant::now())
                            .min(Duration::from_millis(250)),
                    ) {
                        Ok(Request::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
                        Ok(Request::HostStatus(root, status)) => {
                            let path=root.join("allegro-host-status.json");
                            if let Ok(bytes)=serde_json::to_vec(&status) {let temp=path.with_extension("status.tmp");if std::fs::write(&temp,bytes).is_ok(){let _=std::fs::rename(&temp,&path);}}
                        }
                        Ok(Request::Save(path, board)) => {
                            if let Err(e) = save_board(&path, &board) {
                                record_error(&state, e)
                            }
                        }
                        Ok(Request::Copy(text)) => {
                            if let Err(e) = write_clipboard(clipboard_owner, &text) {
                                record_error(&state, e)
                            }
                        }
                        Ok(Request::Launch(path)) => {
                            if let Err(e) = launch(&path) {
                                record_error(&state, e)
                            }
                        }
                        Ok(Request::LoadBoards(root)) => {
                            ai_dir = Some(root.clone());
                            let read = |name| {
                                let p = root.join(name);
                                match load_board(&p) {
                                    Ok(b) => b,
                                    Err(e) => {
                                        record_error(&state, e);
                                        Board::demo()
                                    }
                                }
                            };
                            let _ = loaded_tx
                                .send((read("allegro-sidebar.json"), read("allegro-desktop.json")));
                            if crate::ai_quota::has_configured_live_provider(&root) {
                                let q = crate::ai_quota::fetch_all(&root);
                                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                                s.ai_quota = q;
                                s.revision += 1;
                            }
                            wake();
                        }
                        Ok(Request::RefreshAiQuota(dir)) => {
                            ai_dir = Some(dir.clone());
                            let q = crate::ai_quota::fetch_all(&dir);
                            {
                                let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                                s.ai_quota = q;
                                s.revision += 1;
                            }
                            wake();
                        }
                        Ok(Request::OpenAiQuotaConfig(dir)) => {
                            match crate::ai_quota::ensure_config_file(&dir) {
                                Ok(p) => {
                                    if let Err(e) = launch(&p) {
                                        record_error(&state, e)
                                    }
                                }
                                Err(e) => record_error(&state, e),
                            }
                        }
                        Ok(Request::SaveFileShelf(dir, shelf)) => {
                            if let Err(e) = shelf.save(&dir) {
                                record_error(&state, e);
                            }
                        }
                        Ok(Request::SaveSettings(dir, settings)) => {
                            if let Err(e) = settings.save(&dir) {
                                record_error(&state, e);
                            }
                        }
                        Ok(Request::SaveTodos(dir, todos)) => {
                            if let Err(e) = todos.save(&dir) {
                                record_error(&state, e);
                            }
                        }
                        Ok(Request::SaveReminders(dir, rem)) => {
                            if let Err(e) = rem.save(&dir) {
                                record_error(&state, e);
                            }
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => {}
                    }
                    if let Some(root) = &ai_dir {
                        if let Ok(bytes)=std::fs::read(root.join("allegro-live.json")) {
                            if bytes!=live_bytes {
                                if let Ok(live)=serde_json::from_slice(&bytes) {
                                    let mut s=state.lock().unwrap_or_else(|e|e.into_inner());s.live=live;s.revision+=1;
                                }
                                live_bytes=bytes;wake();
                            }
                        }
                        let names=["allegro-sidebar.json","allegro-desktop.json","allegro-settings.json"];
                        let mut boards_changed=false;
                        let mut changed=false;
                        for (i,name) in names.iter().enumerate() {
                            if let Ok(bytes)=std::fs::read(root.join(name)) {
                                if bytes != watched[i] {
                                    changed=true;
                                    if i==2 {
                                        if let Ok(s)=serde_json::from_slice(&bytes) { let _=settings_tx.send(s); }
                                    } else { boards_changed=true; }
                                    watched[i]=bytes;
                                }
                            }
                        }
                        if boards_changed {
                            if let (Ok(side),Ok(desk))=(load_board(&root.join(names[0])),load_board(&root.join(names[1]))) { let _=loaded_tx.send((side,desk)); }
                        }
                        if changed { wake(); }
                    }
                    if Instant::now() >= next_ai {
                        if let Some(dir) = &ai_dir
                            && crate::ai_quota::has_configured_live_provider(dir)
                        {
                            let q = crate::ai_quota::fetch_all(dir);
                            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                            s.ai_quota = q;
                            s.revision += 1;
                            wake();
                        }
                        next_ai = Instant::now() + Duration::from_secs(300);
                    }
                    if Instant::now() >= next {
                        let metrics = sample.sample();
                        let clip = read_clipboard(&mut sequence);
                        {
                            let mut s = state.lock().unwrap_or_else(|e| e.into_inner());
                            s.metrics = metrics;
                            if let Some(text) = clip {
                                s.clipboard.retain(|v| v != &text);
                                s.clipboard.insert(0, text);
                                s.clipboard.truncate(20);
                            }
                            s.revision += 1;
                        }
                        wake();
                        next = Instant::now() + Duration::from_millis(2000);
                    }
                }
                if !clipboard_owner.is_null() {
                    unsafe {
                        platform::DestroyWindow(clipboard_owner);
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self {
            shared,
            loaded,
            settings,
            tx,
            join: Some(join),
        })
    }
    pub fn send(&self, r: Request) {
        let _ = self.tx.send(r);
    }
}
impl Drop for Background {
    fn drop(&mut self) {
        let _ = self.tx.send(Request::Stop);
        if let Some(j) = self.join.take() {
            let _ = j.join();
        }
    }
}
fn record_error(s: &Mutex<Snapshot>, e: String) {
    let mut s = s.lock().unwrap_or_else(|e| e.into_inner());
    s.errors.push(e);
    if s.errors.len() > 16 {
        s.errors.remove(0);
    }
}
fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;
    s.encode_wide().chain(Some(0)).collect()
}
pub fn decode_wallpaper(path: &Path) -> Result<Image, String> {
    let reader = image::ImageReader::open(path)
        .map_err(|e| e.to_string())?
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    let image = reader.decode().map_err(|e| e.to_string())?;
    let image = if image.width() > 7680 || image.height() > 4320 {
        image.thumbnail(7680, 4320)
    } else {
        image
    };
    let rgba = image.to_rgba8();
    let info = ImageInfo::new(
        (rgba.width() as i32, rgba.height() as i32),
        ColorType::RGBA8888,
        AlphaType::Unpremul,
        None,
    );
    skia_safe::images::raster_from_data(
        &info,
        Data::new_copy(rgba.as_raw()),
        rgba.width() as usize * 4,
    )
    .ok_or("壁纸转为 Skia 图像失败".into())
}
fn wallpaper_path() -> Result<PathBuf, String> {
    let mut path = [0u16; 32768];
    unsafe {
        if platform::SystemParametersInfoW(0x73, path.len() as u32, path.as_mut_ptr().cast(), 0)
            == 0
        {
            return Err("系统未提供壁纸".into());
        }
    }
    let n = path.iter().position(|v| *v == 0).unwrap_or(path.len());
    if n == 0 {
        return Err("未设置图片壁纸".into());
    }
    Ok(PathBuf::from(String::from_utf16_lossy(&path[..n])))
}
fn start_menu_roots() -> Vec<PathBuf> {
    ["APPDATA", "PROGRAMDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(|p| PathBuf::from(p).join("Microsoft/Windows/Start Menu/Programs"))
        .collect()
}
pub fn index_applications(roots: &[PathBuf]) -> Vec<Application> {
    fn visit(p: &Path, out: &mut Vec<Application>, depth: usize) {
        if depth > 24 || out.len() > 20000 {
            return;
        }
        if let Ok(entries) = std::fs::read_dir(p) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Ok(ty) = entry.file_type() {
                    if ty.is_dir() && !ty.is_symlink() {
                        visit(&path, out, depth + 1)
                    } else if ty.is_file()
                        && path
                            .extension()
                            .is_some_and(|s| s.eq_ignore_ascii_case("lnk"))
                    {
                        out.push(Application {
                            name: path
                                .file_stem()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned(),
                            path,
                        });
                    }
                }
            }
        }
    }
    let mut apps = vec![];
    for root in roots {
        visit(root, &mut apps, 0)
    }
    apps.sort_by_key(|a| a.name.to_lowercase());
    apps.dedup_by(|a, b| a.path == b.path);
    apps
}
fn save_board(path: &Path, board: &Board) -> Result<(), String> {
    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(|e| e.to_string())?
    }
    let bytes = serde_json::to_vec_pretty(board).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}
fn load_board(path: &Path) -> Result<Board, String> {
    match std::fs::read(path) {
        Ok(bytes) => {
            let mut b: Board =
                serde_json::from_slice(&bytes).map_err(|e| format!("{}: {e}", path.display()))?;
            b.sanitize();
            Ok(b)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Board::demo()),
        Err(e) => Err(e.to_string()),
    }
}
fn launch(path: &Path) -> Result<(), String> {
    use windows::{
        Win32::UI::{Shell::ShellExecuteW, WindowsAndMessaging::SW_SHOWNORMAL},
        core::PCWSTR,
    };
    let p = wide(path.as_os_str());
    let r = unsafe {
        ShellExecuteW(
            None,
            PCWSTR::null(),
            PCWSTR(p.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    if r.0 as isize <= 32 {
        Err("启动应用失败".into())
    } else {
        Ok(())
    }
}
fn read_clipboard(sequence: &mut u32) -> Option<String> {
    unsafe {
        let n = platform::GetClipboardSequenceNumber();
        if n == *sequence || platform::OpenClipboard(std::ptr::null_mut()) == 0 {
            return None;
        }
        let handle = platform::GetClipboardData(13);
        let mut result = None;
        if !handle.is_null() {
            let p = platform::GlobalLock(handle).cast::<u16>();
            if !p.is_null() {
                let mut len = 0;
                while len < (platform::GlobalSize(handle) / 2).min(65536) && *p.add(len) != 0 {
                    len += 1
                }
                if len > 0 {
                    result = Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)))
                }
                platform::GlobalUnlock(handle);
            }
        }
        platform::CloseClipboard();
        *sequence = n;
        result
    }
}
fn write_clipboard(owner: platform::Hwnd, text: &str) -> Result<(), String> {
    unsafe {
        let chars: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
        if owner.is_null() || platform::OpenClipboard(owner) == 0 {
            return Err("剪贴板暂时被占用".into());
        }
        let r = (|| {
            let h = platform::GlobalAlloc(2, chars.len() * 2);
            if h.is_null() {
                return Err("剪贴板内存不足".into());
            }
            let p = platform::GlobalLock(h);
            if p.is_null() {
                platform::GlobalFree(h);
                return Err("剪贴板内存锁定失败".into());
            }
            std::ptr::copy_nonoverlapping(chars.as_ptr(), p.cast(), chars.len());
            platform::GlobalUnlock(h);
            platform::EmptyClipboard();
            if platform::SetClipboardData(13, h).is_null() {
                platform::GlobalFree(h);
                Err("剪贴板写入失败".into())
            } else {
                Ok(())
            }
        })();
        platform::CloseClipboard();
        r
    }
}
#[derive(Default)]
struct Sampler {
    cpu: Option<(u64, u64, u64)>,
    net: Option<(u64, u64, Instant)>,
}
impl Sampler {
    fn sample(&mut self) -> Metrics {
        let mut m = Metrics::default();
        unsafe {
            let (mut idle, mut kernel, mut user) = (
                FILETIME::default(),
                FILETIME::default(),
                FILETIME::default(),
            );
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                let cv = |v: FILETIME| ((v.dwHighDateTime as u64) << 32) | v.dwLowDateTime as u64;
                let current = (cv(idle), cv(kernel), cv(user));
                if let Some(old) = self.cpu {
                    let total = current.1.saturating_sub(old.1) + current.2.saturating_sub(old.2);
                    if total > 0 {
                        m.cpu = Some(
                            (1. - current.0.saturating_sub(old.0) as f32 / total as f32)
                                .clamp(0., 1.)
                                * 100.,
                        )
                    }
                }
                self.cpu = Some(current);
            }
            let mut mem = MEMORYSTATUSEX {
                dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
                ..Default::default()
            };
            if GlobalMemoryStatusEx(&mut mem).is_ok() {
                m.memory = Some(mem.dwMemoryLoad as f32)
            }
            let mut power = SYSTEM_POWER_STATUS::default();
            if GetSystemPowerStatus(&mut power).is_ok()
                && power.BatteryFlag != 128
                && power.BatteryLifePercent <= 100
            {
                m.battery = Some(power.BatteryLifePercent)
            }
            let root = wide(std::ffi::OsStr::new("C:\\"));
            let (mut free, mut total) = (0u64, 0u64);
            if GetDiskFreeSpaceExW(
                windows::core::PCWSTR(root.as_ptr()),
                None,
                Some(&mut total),
                Some(&mut free),
            )
            .is_ok()
                && total > 0
            {
                m.disk = Some((1. - free as f32 / total as f32) * 100.)
            }
            let mut table = std::ptr::null_mut();
            if GetIfTable2(&mut table).0 == 0 && !table.is_null() {
                let rows = std::slice::from_raw_parts(
                    (*table).Table.as_ptr(),
                    (*table).NumEntries as usize,
                );
                let (mut down, mut up) = (0u64, 0u64);
                for row in rows {
                    if row.Type != 24 && row.OperStatus.0 == 1 {
                        down = down.saturating_add(row.InOctets);
                        up = up.saturating_add(row.OutOctets);
                    }
                }
                FreeMibTable(table.cast());
                let now = Instant::now();
                if let Some((d, u, t)) = self.net {
                    let seconds = now.duration_since(t).as_secs_f64().max(0.001);
                    m.download = down.saturating_sub(d) as f64 / seconds;
                    m.upload = up.saturating_sub(u) as f64 / seconds;
                }
                self.net = Some((down, up, now));
            }
        }
        m
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("allegro-{name}-{}", std::process::id()))
    }
    #[test]
    fn async_index() {
        let dir = temp("index");
        std::fs::create_dir_all(dir.join("Nested")).unwrap();
        std::fs::write(dir.join("Nested/记事本.lnk"), []).unwrap();
        let d = dir.clone();
        let apps = thread::spawn(move || index_applications(&[d]))
            .join()
            .unwrap();
        assert_eq!(apps.len(), 1);
        assert_eq!(crate::state::search("记事", &apps).len(), 1);
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn wallpaper_decode_cache() {
        let path = temp("wall.png");
        let bytes = crate::surface::render_offscreen_png(32, 24, 1., |c| {
            c.clear(skia_safe::Color::BLUE);
        })
        .unwrap();
        std::fs::write(&path, bytes).unwrap();
        let p = path.clone();
        let image = thread::spawn(move || decode_wallpaper(&p))
            .join()
            .unwrap()
            .unwrap();
        let blurred = preblur_wallpaper(&image);
        assert!(blurred[1].is_some());
        let s = Snapshot {
            wallpaper: Some(image),
            wallpaper_blurred: blurred,
            wallpaper_status: "已缓存".into(),
            ..Default::default()
        };
        assert_eq!(s.wallpaper.as_ref().unwrap().dimensions(), (32, 24).into());
        assert!(s.blurred_wallpaper(1).is_some());
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn separate_persistence() {
        let dir = temp("boards");
        save_board(&dir.join("allegro-sidebar.json"), &Board::demo()).unwrap();
        save_board(&dir.join("allegro-desktop.json"), &Board::default()).unwrap();
        assert_eq!(
            load_board(&dir.join("allegro-sidebar.json"))
                .unwrap()
                .cards
                .len(),
            6
        );
        assert!(
            load_board(&dir.join("allegro-desktop.json"))
                .unwrap()
                .cards
                .is_empty()
        );
        std::fs::remove_dir_all(dir).unwrap();
    }
}
