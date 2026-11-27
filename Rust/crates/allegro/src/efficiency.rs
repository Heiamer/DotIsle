//! 效率交互内核：热键浮层仪表盘、文件中转站 (Dropzone)、原地便签编辑器、本地相册轮播与拖放启动解析。
use dotisle_domain::desktop_widgets::Placement;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::Arc,
};
use vello::peniko::{Blob, ImageAlphaType, ImageData, ImageFormat};

/// 热键呼出的桌面仪表盘浮层状态。
#[derive(Clone, Copy, Debug, Default)]
pub struct DashboardState {
    pub open: bool,
    pub opacity: f64,
}

impl DashboardState {
    pub fn toggle(&mut self) -> bool {
        self.open = !self.open;
        self.open
    }
    pub fn close(&mut self) {
        self.open = false;
    }
    /// 每帧平滑更新微暗背幕不透明度，返回是否仍在动画中。
    pub fn tick(&mut self, dt: f64) -> bool {
        let target = if self.open { 1.0 } else { 0.0 };
        let diff = target - self.opacity;
        if diff.abs() <= 0.005 {
            self.opacity = target;
            return false;
        }
        let rate = 1.0 - (-18.0 * dt.clamp(0.0, 0.05)).exp();
        self.opacity += diff * rate;
        true
    }
}

/// 单个暂存进文件中转站的条目。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DropItem {
    pub path: String,
    pub name: String,
    pub tag: &'static str,
    pub size_label: String,
}

/// 文件中转站状态管理器（支持按 widget_id 隔离或从配置恢复）。
#[derive(Clone, Debug, Default)]
pub struct DropzoneStore {
    pub buckets: HashMap<String, Vec<DropItem>>,
}

impl DropzoneStore {
    pub fn items(&self, widget_id: &str) -> &[DropItem] {
        self.buckets.get(widget_id).map(|v| v.as_slice()).unwrap_or(&[])
    }

    pub fn total_count(&self) -> usize {
        self.buckets.values().map(|v| v.len()).sum()
    }

    /// 同步 `Placement.config.items` 中的持久化路径（若桶为空时初始化）。
    pub fn sync_from_placements(&mut self, placements: &[Placement]) {
        for p in placements {
            if !p.config.items.is_empty() && !self.buckets.contains_key(&p.id) {
                for raw in &p.config.items {
                    let _ = self.push_path(&p.id, Path::new(raw));
                }
            }
        }
    }

    /// 拖入一个文件或文件夹进入指定中转站（自动去重并限最多 12 项）。
    pub fn push_path(&mut self, widget_id: &str, path: &Path) -> Option<Vec<String>> {
        let full = path.to_string_lossy().to_string();
        if full.is_empty() || full.contains('\0') {
            return None;
        }
        let list = self.buckets.entry(widget_id.to_string()).or_default();
        if list.iter().any(|it| it.path.eq_ignore_ascii_case(&full)) {
            return None;
        }
        if list.len() >= 12 {
            list.remove(0);
        }
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
            .unwrap_or(&full)
            .to_string();
        let meta = std::fs::metadata(path).ok();
        let is_dir = meta.as_ref().is_some_and(|m| m.is_dir()) || path.is_dir();
        let size_label = if is_dir {
            "文件夹".into()
        } else if let Some(m) = meta {
            format_bytes(m.len())
        } else {
            "文件".into()
        };
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let tag = if is_dir {
            "DIR"
        } else if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "webp" | "bmp" | "gif") {
            "IMG"
        } else if matches!(ext.as_str(), "zip" | "7z" | "rar" | "tar" | "gz") {
            "ZIP"
        } else if matches!(ext.as_str(), "pdf" | "doc" | "docx" | "md" | "txt" | "xlsx") {
            "DOC"
        } else if matches!(ext.as_str(), "exe" | "lnk" | "bat" | "ps1") {
            "APP"
        } else {
            "FILE"
        };
        list.push(DropItem {
            path: full,
            name,
            tag,
            size_label,
        });
        Some(list.iter().map(|i| i.path.clone()).collect())
    }

    /// 清空指定中转站。
    pub fn clear(&mut self, widget_id: &str) {
        self.buckets.remove(widget_id);
    }
}

pub fn format_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

/// 原地便签编辑器（支持复选框列表原地勾选与 IME 输入）。
#[derive(Clone, Debug, Default)]
pub struct NoteEdit {
    pub widget_id: String,
    pub title: String,
    pub text: String,
    pub preedit: String,
}

impl NoteEdit {
    pub fn new(widget_id: &str, title: &str, text: &str) -> Self {
        Self {
            widget_id: widget_id.to_string(),
            title: title.to_string(),
            text: text.to_string(),
            preedit: String::new(),
        }
    }

    pub fn push_str(&mut self, s: &str) {
        self.preedit.clear();
        if self.text.chars().count() + s.chars().count() <= 500 {
            self.text.push_str(s);
        }
    }

    pub fn backspace(&mut self) {
        self.preedit.clear();
        self.text.pop();
    }

    pub fn newline(&mut self) {
        self.preedit.clear();
        if self.text.chars().count() < 500 {
            self.text.push('\n');
        }
    }
}

/// 切换便签文本中指定行的 `- [ ] ` / `- [x] ` 任务复选框状态，返回变更后的全文。
pub fn toggle_note_checkbox(text: &str, target_line: usize) -> Option<String> {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let line = lines.get_mut(target_line)?;
    let trimmed = line.trim_start();
    let indent_len = line.len() - trimmed.len();
    let indent = &line[..indent_len];
    if let Some(rest) = trimmed.strip_prefix("- [ ] ") {
        *line = format!("{indent}- [x] {rest}");
    } else if let Some(rest) = trimmed
        .strip_prefix("- [x] ")
        .or_else(|| trimmed.strip_prefix("- [X] "))
    {
        *line = format!("{indent}- [ ] {rest}");
    } else {
        return None;
    }
    Some(lines.join("\n"))
}

/// 本地相册文件夹扫描与懒加载缩略缓存。
#[derive(Default)]
struct PhotoDecoder {
    folder_files: HashMap<String, Vec<PathBuf>>,
    folders: HashMap<String, String>,
    images: HashMap<PathBuf, ImageData>,
}

impl PhotoDecoder {
    pub fn sync_folder(&mut self, widget_id: &str, folder: &str) {
        if self.folders.get(widget_id).is_some_and(|old| old == folder) { return; }
        if self.folders.len() >= 64 {
            self.folders.clear();
            self.folder_files.clear();
        }
        let dir = Path::new(folder);
        self.folders.insert(widget_id.into(), folder.into());
        let mut files = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_file()
                    && p.extension()
                        .and_then(|s| s.to_str())
                        .is_some_and(|ext| {
                            matches!(
                                ext.to_ascii_lowercase().as_str(),
                                "png" | "jpg" | "jpeg" | "bmp"
                            )
                        })
                {
                    files.push(p);
                }
                if files.len() >= 64 {
                    break;
                }
            }
        }
        files.sort();
        self.folder_files.insert(widget_id.to_string(), files);
    }

    fn image_at(&mut self, widget_id: &str, index: usize) -> Option<&ImageData> {
        let files = self.folder_files.get(widget_id)?;
        if files.is_empty() {
            return None;
        }
        let idx = index % files.len();
        let path = files[idx].clone();
        if !self.images.contains_key(&path) {
            if let Ok(decoded) = image::open(&path) {
                let rgba = decoded.thumbnail(720, 720).to_rgba8();
                let (w, h) = rgba.dimensions();
                let img = ImageData {
                    data: Blob::new(Arc::new(rgba.into_raw())),
                    format: ImageFormat::Rgba8,
                    alpha_type: ImageAlphaType::Alpha,
                    width: w.max(1),
                    height: h.max(1),
                };
                if self.images.len() > 24 {
                    self.images.clear();
                }
                self.images.insert(path.clone(), img);
            }
        }
        self.images.get(&path)
    }

}

use std::{sync::mpsc, thread};

struct PhotoRequest { id: String, folder: String, index: usize }
struct PhotoResult { request: PhotoRequest, total: usize, image: Option<ImageData> }

#[derive(Default)]
struct PhotoSlot {
    folder: String,
    index: usize,
    total: usize,
    image: Option<ImageData>,
    ready_index: Option<usize>,
    pending: bool,
}

/// 图片路径可能位于慢盘或网络盘，目录扫描与缩略图解码均不在渲染线程进行。
pub struct PhotoFrameState {
    slots: HashMap<String, PhotoSlot>,
    requests: Option<mpsc::SyncSender<PhotoRequest>>,
    ready: mpsc::Receiver<PhotoResult>,
    worker: Option<thread::JoinHandle<()>>,
}

impl Default for PhotoFrameState {
    fn default() -> Self { Self::with_wake(|| {}) }
}

impl PhotoFrameState {
    pub fn with_wake(wake: fn()) -> Self {
        let (send, pending) = mpsc::sync_channel::<PhotoRequest>(8);
        let (results, ready) = mpsc::channel();
        let worker = thread::Builder::new().name("allegro-photos".into()).spawn(move || {
            let mut decoder = PhotoDecoder::default();
            while let Ok(request) = pending.recv() {
                decoder.sync_folder(&request.id, &request.folder);
                let total = decoder.folder_files.get(&request.id).map_or(0, Vec::len);
                let image = decoder.image_at(&request.id, request.index).cloned();
                if results.send(PhotoResult { request, total, image }).is_err() { break; }
                wake();
            }
        }).map_err(|err| eprintln!("Allegro photo thread: {err}")).ok();
        Self { slots: HashMap::new(), requests: Some(send), ready, worker }
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        while let Ok(result) = self.ready.try_recv() {
            if let Some(slot) = self.slots.get_mut(&result.request.id) {
                slot.pending = false;
                if slot.folder == result.request.folder {
                    slot.total = result.total;
                    slot.ready_index = Some(result.request.index);
                    slot.image = result.image;
                }
                changed = true;
            }
        }
        changed
    }

    pub fn sync_folder(&mut self, widget_id: &str, folder: &str) {
        let folder = folder.trim();
        let slot = self.slots.entry(widget_id.into()).or_default();
        if slot.folder != folder {
            // 等待旧请求退出，但重设文件夹与显示游标，旧结果不覆写新文件夹。
            *slot = PhotoSlot { folder: folder.into(), pending: slot.pending, ..Default::default() };
        }
        self.submit(widget_id);
    }

    pub fn next_photo(&mut self, widget_id: &str) {
        if let Some(slot) = self.slots.get_mut(widget_id) {
            if slot.total > 1 { slot.index = (slot.index + 1) % slot.total; }
        }
        self.submit(widget_id);
    }

    fn submit(&mut self, widget_id: &str) {
        let Some(slot) = self.slots.get_mut(widget_id) else { return; };
        if !slot.folder.is_empty() && !slot.pending && slot.ready_index != Some(slot.index) {
            let request = PhotoRequest { id: widget_id.into(), folder: slot.folder.clone(), index: slot.index };
            slot.pending = self.requests.as_ref().is_some_and(|tx| tx.try_send(request).is_ok());
        }
    }

    pub fn current_image(&mut self, widget_id: &str) -> Option<&ImageData> {
        self.slots.get(widget_id)?.image.as_ref()
    }

    pub fn photo_count(&self, widget_id: &str) -> (usize, usize) {
        self.slots.get(widget_id).map_or((0, 0), |slot| (slot.ready_index.unwrap_or(0), slot.total))
    }

    pub fn retain(&mut self, keep: impl Fn(&str) -> bool) {
        self.slots.retain(|id, slot| slot.pending || keep(id));
    }
}

impl Drop for PhotoFrameState {
    fn drop(&mut self) {
        self.requests.take();
        // 先断开回传队列，当前操作结束后工作线程不再处理过期任务。
        self.ready = mpsc::channel().1;
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

/// 从拖入的 `.exe`、`.lnk` 或文件夹解析出安全的快捷启动标题与目标路径。
pub fn resolve_dropped_launcher(path: &Path) -> Option<(String, String)> {
    let target = path.to_string_lossy().to_string();
    if !dotisle_domain::widget_config::launch_target(&target) {
        return None;
    }
    let stem = path
        .file_stem()
        .or_else(|| path.file_name())
        .and_then(|s| s.to_str())
        .unwrap_or("快捷启动")
        .trim();
    let title: String = if stem.is_empty() {
        "快捷启动".into()
    } else {
        stem.chars().take(24).collect()
    };
    Some((title, target))
}
