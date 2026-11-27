//! Allegro Sidebar 右侧独立效率侧边栏与区块排布核心：
//! 强制 2 列 Apple 区块网格（1×1 小 / 2×1 大 / 2×2 特大，与桌面小组件完全独立、互不同步），
//! 同时集成 Spotlight 搜索与四则算术、斜杠指令、低开销 Win32 剪贴板历史及本机应用快速索引。
use crate::{efficiency::toggle_note_checkbox, snap::Rect2D};
use dotisle_domain::{
    desktop_widgets::{Kind, Placement, Size, Style},
    widget_config::Config,
};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

/// 剪贴板历史条目。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClipboardItem {
    pub text: String,
    pub preview: String,
}

/// 低开销 Win32 系统剪贴板历史记录器（仅在序列号变更时读取 Unicode 文本，上限 8 条）。
#[derive(Default)]
pub struct ClipboardHistory {
    pub items: Vec<ClipboardItem>,
    last_seq: u32,
}

#[link(name = "user32")]
extern "system" {
    fn GetClipboardSequenceNumber() -> u32;
    fn OpenClipboard(hwnd_new_owner: isize) -> i32;
    fn CloseClipboard() -> i32;
    fn EmptyClipboard() -> i32;
    fn GetClipboardData(format: u32) -> isize;
    fn SetClipboardData(format: u32, mem: isize) -> isize;
    fn CreateWindowExW(ex: u32, class: *const u16, title: *const u16, style: u32,
        x: i32, y: i32, width: i32, height: i32, parent: isize, menu: isize,
        instance: isize, param: *const std::ffi::c_void) -> isize;
    fn DestroyWindow(hwnd: isize) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn GlobalAlloc(flags: u32, bytes: usize) -> isize;
    fn GlobalLock(mem: isize) -> *mut u16;
    fn GlobalUnlock(mem: isize) -> i32;
    fn GlobalFree(mem: isize) -> isize;
    fn GlobalSize(mem: isize) -> usize;
}

const CF_UNICODETEXT: u32 = 13;
const GMEM_MOVEABLE: u32 = 0x0002;

impl ClipboardHistory {
    /// 轻检测剪贴板序列号是否变化；有新文本内容时推入历史队列。
    pub fn poll(&mut self) {
        let seq = unsafe { GetClipboardSequenceNumber() };
        if seq == self.last_seq && seq != 0 {
            return;
        }
        self.last_seq = seq;
        if let Some(txt) = read_system_clipboard_text() {
            self.push_text(&txt);
        }
    }

    /// 将纯文本去重后插入剪贴板历史队首（最多保留 8 条）。
    pub fn push_text(&mut self, raw: &str) {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            return;
        }
        let preview = make_preview(trimmed, 40);
        self.items.retain(|it| it.text != trimmed);
        self.items.insert(
            0,
            ClipboardItem {
                text: trimmed.to_string(),
                preview,
            },
        );
        self.items.truncate(8);
    }

    /// 复制历史记录中第 `idx` 条到系统剪贴板并返回原文。
    pub fn copy_index(&mut self, idx: usize) -> Option<String> {
        let text = self.items.get(idx)?.text.clone();
        if self.write_text(&text) {
            self.push_text(&text);
            Some(text)
        } else {
            None
        }
    }

    /// 将指定文本写入系统剪贴板并同步序列号，避免自身触发重复采集。
    pub fn write_text(&mut self, text: &str) -> bool {
        let ok = write_system_clipboard_text(text);
        if ok {
            self.last_seq = unsafe { GetClipboardSequenceNumber() };
        }
        ok
    }
}

fn make_preview(s: &str, max_chars: usize) -> String {
    let single: String = s
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut out = String::new();
    for (i, ch) in single.chars().enumerate() {
        if i >= max_chars {
            out.push('…');
            break;
        }
        out.push(ch);
    }
    out
}

fn read_system_clipboard_text() -> Option<String> {
    unsafe {
        if OpenClipboard(0) == 0 {
            return None;
        }
        let handle = GetClipboardData(CF_UNICODETEXT);
        let result = if handle != 0 {
            let ptr = GlobalLock(handle);
            if !ptr.is_null() {
                let mut len = 0usize;
                let limit = (GlobalSize(handle) / std::mem::size_of::<u16>()).min(16_384);
                while len < limit && *ptr.add(len) != 0 {
                    len += 1;
                }
                let slice = std::slice::from_raw_parts(ptr, len);
                let s = String::from_utf16_lossy(slice);
                let _ = GlobalUnlock(handle);
                Some(s)
            } else {
                None
            }
        } else {
            None
        };
        let _ = CloseClipboard();
        result
    }
}

fn write_system_clipboard_text(text: &str) -> bool {
    struct Owner(isize);
    impl Drop for Owner {
        fn drop(&mut self) { unsafe { DestroyWindow(self.0); } }
    }
    let utf16: Vec<u16> = text.encode_utf16().chain(Some(0)).collect();
    let byte_len = utf16.len() * std::mem::size_of::<u16>();
    unsafe {
        // EmptyClipboard 要求有效的剪贴板所有者；用不可见消息窗口，不影响焦点或任务栏。
        let hwnd = CreateWindowExW(0, windows::core::w!("STATIC").0, std::ptr::null(), 0,
            0, 0, 0, 0, -3, 0, 0, std::ptr::null()); // HWND_MESSAGE
        if hwnd == 0 { return false; }
        let owner = Owner(hwnd);
        let mem = GlobalAlloc(GMEM_MOVEABLE, byte_len);
        if mem == 0 { return false; }
        let dst = GlobalLock(mem);
        if dst.is_null() {
            let _ = GlobalFree(mem);
            return false;
        }
        std::ptr::copy_nonoverlapping(utf16.as_ptr(), dst, utf16.len());
        let _ = GlobalUnlock(mem);
        if OpenClipboard(owner.0) == 0 {
            let _ = GlobalFree(mem);
            return false;
        }
        let success = EmptyClipboard() != 0 && SetClipboardData(CF_UNICODETEXT, mem) != 0;
        let _ = CloseClipboard();
        if !success { let _ = GlobalFree(mem); }
        success
    }
}

/// Spotlight 可启动应用条目。
#[derive(Clone, Debug)]
pub struct IndexedApp {
    pub name: String,
    pub aliases: &'static str,
    pub target: String,
    pub category: &'static str,
}

/// 本机应用及常用系统入口索引器（首次打开浮层时懒扫描一次开始菜单）。
#[derive(Default)]
pub struct AppIndex {
    loaded: bool,
    entries: Vec<IndexedApp>,
    scanned: std::sync::Arc<std::sync::OnceLock<Vec<IndexedApp>>>,
    wake: Option<fn()>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl AppIndex {
    pub fn with_wake(wake: fn()) -> Self {
        let mut index = Self::default();
        index.wake = Some(wake);
        index
    }

    /// 懒加载内置系统常用应用与开始菜单 `.lnk` 快捷方式（最多 120 项）。
    pub fn ensure_loaded(&mut self) {
        self.ensure_loaded_with(scan_start_menu);
    }

    fn ensure_loaded_with(&mut self, scan: impl FnOnce(&[IndexedApp]) -> Vec<IndexedApp> + Send + 'static) {
        if self.loaded {
            return;
        }
        self.loaded = true;
        let builtins = [
            ("文件资源管理器", "explorer file 资源管理器 文件夹", "explorer.exe", "系统"),
            ("Windows 终端", "terminal wt cmd powershell 命令行", "wt.exe", "开发"),
            ("Microsoft Edge 浏览器", "edge browser 网页 浏览器", "msedge.exe", "网络"),
            ("Windows 系统设置", "settings 设置 控制面板", "ms-settings:", "系统"),
            ("记事本 Notepad", "notepad 记事本 文本", "notepad.exe", "效率"),
            ("计算器 Calculator", "calc calculator 计算器", "calc.exe", "工具"),
            ("屏幕截图工具", "snipping screenshot 截图", "ms-screenclip:", "工具"),
            ("任务管理器", "taskmgr 任务管理器 性能 进程", "taskmgr.exe", "系统"),
            ("系统时钟与闹钟", "clock alarm timer 时钟 闹钟", "ms-clock:", "效率"),
            ("画图 Paint", "mspaint paint 画图", "mspaint.exe", "创作"),
        ];
        for (name, aliases, target, category) in builtins {
            self.entries.push(IndexedApp {
                name: name.into(),
                aliases,
                target: target.into(),
                category,
            });
        }

        let builtins = self.entries.clone();
        let scanned = self.scanned.clone();
        let wake = self.wake;
        self.worker = std::thread::Builder::new().name("allegro-app-index".into()).spawn(move || {
            let entries = scan(&builtins);
            if scanned.set(entries).is_ok() {
                if let Some(wake) = wake { wake(); }
            }
        }).map_err(|err| eprintln!("Allegro app index thread: {err}")).ok();
    }

    /// 无需加锁或取走结果；首次完成时用于触发搜索重绘。
    pub fn is_ready(&self) -> bool {
        self.scanned.get().is_some()
    }

    /// 按关键词搜索应用（前缀匹配优先，其次子串匹配）。
    pub fn search(&self, query: &str, limit: usize) -> Vec<IndexedApp> {
        let q = query.trim().to_ascii_lowercase();
        if q.is_empty() {
            return Vec::new();
        }
        let mut scored: Vec<(i32, usize)> = Vec::new();
        let entries = || self.entries.iter().chain(self.scanned.get().into_iter().flatten());
        let visible: Vec<_> = entries().collect();
        for (idx, item) in visible.iter().enumerate() {
            let name_l = item.name.to_ascii_lowercase();
            let alias_l = item.aliases.to_ascii_lowercase();
            let score = if name_l.starts_with(&q) {
                100
            } else if name_l.contains(&q) {
                75
            } else if alias_l.split_whitespace().any(|a| a.starts_with(&q)) {
                60
            } else if alias_l.contains(&q) {
                40
            } else {
                0
            };
            if score > 0 {
                scored.push((score, idx));
            }
        }
        scored.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
        scored
            .into_iter()
            .take(limit)
            .map(|(_, idx)| visible[idx].clone())
            .collect()
    }
}

impl Drop for AppIndex {
    fn drop(&mut self) {
        // 宿主退出后才回收扫描线程，防止嵌入式 DLL 已卸载而线程仍在执行。
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

#[cfg(test)]
mod async_index_tests {
    use super::*;
    use std::{sync::mpsc::channel, time::{Duration, Instant}};

    #[test]
    fn builtins_are_searchable_before_one_shot_index_is_ready() {
        let (release, blocked) = channel();
        let (started, begin) = channel();
        let mut index = AppIndex::default();
        index.ensure_loaded_with(move |_| {
            started.send(()).unwrap();
            blocked.recv_timeout(Duration::from_secs(3)).unwrap();
            vec![IndexedApp { name: "Test async app".into(), aliases: "", target: "test.lnk".into(), category: "test" }]
        });
        begin.recv_timeout(Duration::from_secs(3)).unwrap();
        assert!(!index.is_ready());
        assert_eq!(index.search("notepad", 1)[0].target, "notepad.exe");
        assert!(index.search("Test async app", 1).is_empty());
        index.ensure_loaded_with(|_| panic!("must not spawn a second scan"));
        release.send(()).unwrap();
        let until = Instant::now() + Duration::from_secs(3);
        while !index.is_ready() && Instant::now() < until {
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(index.is_ready());
        assert_eq!(index.search("Test async app", 1)[0].target, "test.lnk");
    }
}

// 所有文件系统访问均在索引线程中执行。
fn scan_start_menu(builtins: &[IndexedApp]) -> Vec<IndexedApp> {
    let mut entries = Vec::new();
    let limit = 120usize.saturating_sub(builtins.len());
    let mut seen: HashSet<String> = builtins
        .iter()
        .map(|e| e.name.to_ascii_lowercase())
        .collect();
    let mut roots = Vec::new();
    if let Some(pd) = std::env::var_os("ProgramData") {
        roots.push(PathBuf::from(pd).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    if let Some(ad) = std::env::var_os("APPDATA") {
        roots.push(PathBuf::from(ad).join("Microsoft\\Windows\\Start Menu\\Programs"));
    }
    for root in roots {
        scan_shortcuts(&root, 0, limit, &mut seen, &mut entries);
        if entries.len() >= limit {
            break;
        }
    }
    entries
}

fn scan_shortcuts(
    dir: &Path,
    depth: usize,
    limit: usize,
    seen: &mut HashSet<String>,
    out: &mut Vec<IndexedApp>,
) {
    if depth > 3 || out.len() >= limit {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for ent in rd.flatten() {
        if out.len() >= limit {
            break;
        }
        let path = ent.path();
        if path.is_dir() {
            scan_shortcuts(&path, depth + 1, limit, seen, out);
            continue;
        }
        let is_lnk = path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"));
        if !is_lnk {
            continue;
        }
        let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        let lower = stem.to_ascii_lowercase();
        if lower.contains("uninstall") || lower.contains("卸载") || lower.contains("readme") {
            continue;
        }
        if seen.insert(lower) {
            out.push(IndexedApp {
                name: stem.to_string(),
                aliases: "",
                target: path.to_string_lossy().to_string(),
                category: "应用",
            });
        }
    }
}

/// Spotlight 斜杠命令行动作。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageSlashCmd {
    StartFocus(u32),
    AddTodo(String),
    EnterGridEdit,
    QuickAddSidebarWidget(Kind, Size, String),
}

/// Spotlight 搜索条单条结果类型。
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StageResultKind {
    MathCopy(String),
    RunCommand(StageSlashCmd),
    LaunchApp { name: String, target: String },
}

/// Spotlight 单条展示结果。
#[derive(Clone, Debug)]
pub struct StageResult {
    pub title: String,
    pub subtitle: String,
    pub badge: &'static str,
    pub kind: StageResultKind,
}

/// 递归下降四则算术求值器：支持 `+ - * / ( )` 与一元正负号（要求至少含有一个运算符才触发）。
pub fn eval_quick_math(raw: &str) -> Option<f64> {
    let trimmed = raw.trim().trim_start_matches('=').trim();
    if trimmed.is_empty()
        || !trimmed.chars().any(|c| matches!(c, '+' | '-' | '*' | '/'))
    {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut pos = 0usize;
    let val = parse_expr(bytes, &mut pos)?;
    skip_ws(bytes, &mut pos);
    if pos != bytes.len() || !val.is_finite() {
        return None;
    }
    Some(val)
}

pub fn format_math_value(v: f64) -> String {
    if (v - v.round()).abs() < 1e-9 && v.abs() < 1e13 {
        format!("{}", v.round() as i64)
    } else {
        let s = format!("{v:.4}");
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn skip_ws(b: &[u8], p: &mut usize) {
    while *p < b.len() && b[*p].is_ascii_whitespace() {
        *p += 1;
    }
}

fn parse_expr(b: &[u8], p: &mut usize) -> Option<f64> {
    let mut lhs = parse_term(b, p)?;
    loop {
        skip_ws(b, p);
        if *p >= b.len() {
            break;
        }
        match b[*p] {
            b'+' => {
                *p += 1;
                lhs += parse_term(b, p)?;
            }
            b'-' => {
                *p += 1;
                lhs -= parse_term(b, p)?;
            }
            _ => break,
        }
    }
    Some(lhs)
}

fn parse_term(b: &[u8], p: &mut usize) -> Option<f64> {
    let mut lhs = parse_factor(b, p)?;
    loop {
        skip_ws(b, p);
        if *p >= b.len() {
            break;
        }
        match b[*p] {
            b'*' => {
                *p += 1;
                lhs *= parse_factor(b, p)?;
            }
            b'/' => {
                *p += 1;
                let rhs = parse_factor(b, p)?;
                if rhs.abs() < 1e-12 {
                    return None;
                }
                lhs /= rhs;
            }
            _ => break,
        }
    }
    Some(lhs)
}

fn parse_factor(b: &[u8], p: &mut usize) -> Option<f64> {
    skip_ws(b, p);
    if *p >= b.len() {
        return None;
    }
    match b[*p] {
        b'+' => {
            *p += 1;
            parse_factor(b, p)
        }
        b'-' => {
            *p += 1;
            parse_factor(b, p).map(|v| -v)
        }
        b'(' => {
            *p += 1;
            let v = parse_expr(b, p)?;
            skip_ws(b, p);
            if *p >= b.len() || b[*p] != b')' {
                return None;
            }
            *p += 1;
            Some(v)
        }
        _ => parse_number(b, p),
    }
}

fn parse_number(b: &[u8], p: &mut usize) -> Option<f64> {
    let start = *p;
    let mut has_digit = false;
    let mut has_dot = false;
    while *p < b.len() {
        let ch = b[*p];
        if ch.is_ascii_digit() {
            has_digit = true;
            *p += 1;
        } else if ch == b'.' && !has_dot {
            has_dot = true;
            *p += 1;
        } else {
            break;
        }
    }
    if !has_digit {
        return None;
    }
    std::str::from_utf8(&b[start..*p]).ok()?.parse::<f64>().ok()
}

/// 根据用户输入生成最多 6 条 Spotlight 搜索与指令结果。
pub fn resolve_stage_results(query: &str, apps: &AppIndex) -> Vec<StageResult> {
    let q = query.trim();
    if q.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();

    if let Some(rest) = q.strip_prefix('/') {
        let low = rest.trim().to_ascii_lowercase();
        let (head, arg) = match rest.trim().split_once(char::is_whitespace) {
            Some((h, a)) => (h.to_ascii_lowercase(), a.trim()),
            None => (low.clone(), ""),
        };
        if head.is_empty() || "focus".starts_with(&head) || "专注".starts_with(&head) {
            let mins = arg.parse::<u32>().ok().filter(|&m| m > 0 && m <= 180).unwrap_or(25);
            out.push(StageResult {
                title: format!("开启 {mins} 分钟番茄专注"),
                subtitle: "回车启动专注计时器并保持沉浸节奏 · /focus [分钟]".into(),
                badge: "命令",
                kind: StageResultKind::RunCommand(StageSlashCmd::StartFocus(mins)),
            });
        }
        if head.is_empty() || "todo".starts_with(&head) || "待办".starts_with(&head) {
            let todo_title = if arg.is_empty() { "整理今天最重要的一件事" } else { arg };
            out.push(StageResult {
                title: format!("新建待办：{todo_title}"),
                subtitle: "立即添加到今日待办清单 · /todo <标题>".into(),
                badge: "命令",
                kind: StageResultKind::RunCommand(StageSlashCmd::AddTodo(todo_title.into())),
            });
        }
        if head.is_empty() || "grid".starts_with(&head) || "网格".starts_with(&head) {
            out.push(StageResult {
                title: "进入桌面全屏网格排布模式".into(),
                subtitle: "铺满桌面网格，支持拖动吸附与一键对齐 · /grid".into(),
                badge: "命令",
                kind: StageResultKind::RunCommand(StageSlashCmd::EnterGridEdit),
            });
        }
        if head.is_empty() || "add".starts_with(&head) || "添加".starts_with(&head) {
            for (kind, size, cap, lbl) in [
                (Kind::AppFolder, Size::Small, "4", "应用文件夹（1×1）"),
                (Kind::AppFolder, Size::Small, "9", "9格应用库（1×1）"),
                (Kind::Weather, Size::Small, "", "晴空天气（1×1）"),
                (Kind::AnalogClock, Size::Small, "", "Apple 模拟表盘（1×1）"),
                (Kind::ActivityRings, Size::Small, "", "效率三环（1×1）"),
                (Kind::SystemPro, Size::Medium, "", "性能探针 Pro（2×1）"),
                (Kind::Note, Size::Medium, "", "备忘录便签（2×1）"),
                (Kind::Todos, Size::Large, "", "待办清单（2×2 特大）"),
            ] {
                if arg.is_empty() || lbl.contains(arg) {
                    out.push(StageResult {
                        title: format!("向侧边栏添加：{lbl}"),
                        subtitle: "添加到右侧独立区块流，不与桌面同步".into(),
                        badge: "区块",
                        kind: StageResultKind::RunCommand(StageSlashCmd::QuickAddSidebarWidget(
                            kind,
                            size,
                            cap.into(),
                        )),
                    });
                }
            }
        }
        out.truncate(6);
        return out;
    }

    if let Some(val) = eval_quick_math(q) {
        let formatted = format_math_value(val);
        out.push(StageResult {
            title: format!("= {formatted}"),
            subtitle: format!("表达式 {q} 计算结果 · 回车复制到剪贴板"),
            badge: "计算",
            kind: StageResultKind::MathCopy(formatted),
        });
    }

    for app in apps.search(q, 6 - out.len()) {
        out.push(StageResult {
            title: app.name,
            subtitle: format!("打开 {}", app.target),
            badge: app.category,
            kind: StageResultKind::LaunchApp {
                name: app.target.clone(),
                target: app.target,
            },
        });
    }
    out.truncate(6);
    out
}

/// 侧边栏强制区块列数（固定 2 列：小=1×1，大=2×1，特大=2×2）。
pub const SIDEBAR_COLS: usize = 2;

/// 侧边栏中单个独立组件的区块排布结果。
#[derive(Clone, Debug, PartialEq)]
pub struct SidebarBlockSlot {
    pub index: usize,
    pub id: String,
    pub rect: Rect2D,
    pub col: usize,
    pub row: usize,
    pub span_x: usize,
    pub span_y: usize,
}

/// 将侧边栏独立小组件按强制 2 列 Apple 区块规则紧密排布：
/// - `Size::Small` (1×1)：`cell × cell`
/// - `Size::Medium` (2×1)：`(2*cell + gap) × cell`（严格占 2 个小组件面积）
/// - `Size::Large` (2×2)：`(2*cell + gap) × (2*cell + gap)`（严格占 4 个小组件面积）
/// 返回各组件精确矩形以及内容总高度。
pub fn pack_sidebar_blocks(
    widgets: &[Placement],
    origin_x: f64,
    origin_y: f64,
    cell: f64,
    gap: f64,
) -> (Vec<SidebarBlockSlot>, f64) {
    let mut occ: Vec<[bool; SIDEBAR_COLS]> = Vec::new();
    let mut slots = Vec::with_capacity(widgets.len());
    let mut max_row_end = 0usize;
    let step = cell + gap;

    let ensure_rows = |occ: &mut Vec<[bool; SIDEBAR_COLS]>, need: usize| {
        while occ.len() < need {
            occ.push([false; SIDEBAR_COLS]);
        }
    };

    for (idx, p) in widgets.iter().enumerate() {
        let (sx, sy) = p.size.grid_span();
        let sx = sx.min(SIDEBAR_COLS).max(1);
        let sy = sy.max(1);

        let placed;
        let mut r = 0usize;
        'search: loop {
            ensure_rows(&mut occ, r + sy);
            for c in 0..=(SIDEBAR_COLS - sx) {
                let mut free = true;
                for rr in r..(r + sy) {
                    for cc in c..(c + sx) {
                        if occ[rr][cc] {
                            free = false;
                            break;
                        }
                    }
                    if !free {
                        break;
                    }
                }
                if free {
                    for rr in r..(r + sy) {
                        for cc in c..(c + sx) {
                            occ[rr][cc] = true;
                        }
                    }
                    placed = (c, r);
                    break 'search;
                }
            }
            r += 1;
        }

        let (col, row) = placed;
        max_row_end = max_row_end.max(row + sy);
        let w = sx as f64 * cell + (sx - 1) as f64 * gap;
        let h = sy as f64 * cell + (sy - 1) as f64 * gap;
        slots.push(SidebarBlockSlot {
            index: idx,
            id: p.id.clone(),
            rect: Rect2D::new(origin_x + col as f64 * step, origin_y + row as f64 * step, w, h),
            col,
            row,
            span_x: sx,
            span_y: sy,
        });
    }

    let total_h = if max_row_end == 0 {
        0.0
    } else {
        max_row_end as f64 * cell + (max_row_end - 1) as f64 * gap
    };
    (slots, total_h)
}

/// 生成侧边栏独立的默认小组件清单（完全独立于桌面，覆盖 1×1 小 / 2×1 大 / 2×2 特大区块规范）。
pub fn default_sidebar_widgets() -> Vec<Placement> {
    let make = |id: &str, kind: Kind, size: Size, material: &str, config: Config| -> Placement {
        Placement {
            id: id.into(),
            kind,
            size,
            style: Style::Original,
            config,
            x: 0,
            y: 0,
            width: 0.0,
            height: 0.0,
            hidden: false,
            revision: 1,
            material: material.into(),
            opacity: 0.96,
            stack_id: String::new(),
            stack_order: 0,
        }
    };

    // Deskset × Smart Stack 旗舰默认侧边栏组合：全透明亚克力 + Apple 1×1 / 2×1 / 2×2 标准区块比例
    vec![
        make("sb-clock", Kind::AnalogClock, Size::Small, "acrylic", Config::default()),
        make(
            "sb-weather",
            Kind::Weather,
            Size::Small,
            "acrylic",
            Config {
                city: "上海".into(),
                ..Default::default()
            },
        ),
        make("sb-pro", Kind::SystemPro, Size::Medium, "acrylic", Config::default()),
        make("sb-music", Kind::Music, Size::Medium, "acrylic", Config::default()),
        make("sb-focus", Kind::Focus, Size::Small, "acrylic", Config::default()),
        make(
            "sb-folder",
            Kind::AppFolder,
            Size::Small,
            "acrylic",
            Config {
                title: "常用应用".into(),
                caption: "4".into(),
                ..Default::default()
            },
        ),
        make("sb-cal", Kind::Calendar, Size::Medium, "acrylic", Config::default()),
        make("sb-todos", Kind::Todos, Size::Large, "lumen", Config::default()),
    ]
}

/// 从独立配置文件读取侧边栏小组件（不存在或损坏时回退默认高颜值区块组合）。
pub fn load_sidebar_widgets(path: &Path) -> Vec<Placement> {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return default_sidebar_widgets();
    };
    match serde_json::from_str::<Vec<Placement>>(&raw) {
        Ok(list) if !list.is_empty() => list,
        _ => default_sidebar_widgets(),
    }
}

/// 将侧边栏独立小组件持久化到专属 JSON 文件（绝不触碰桌面小组件存储）。
pub fn save_sidebar_widgets(path: &Path, widgets: &[Placement]) {
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(widgets) {
        let _ = std::fs::write(path, json);
    }
}

/// 右侧独立侧边栏动态交互状态（不含任何全屏浮层状态）。
#[derive(Default)]
pub struct StageState {
    pub query: String,
    pub preedit: String,
    pub selected: usize,
    pub search_focused: bool,
    pub sidebar_edit: bool,
    pub sidebar_add_open: bool,
    pub sidebar_scroll: f64,
    pub sidebar_widgets: Vec<Placement>,
    pub sidebar_loaded: bool,
    pub sidebar_path: Option<PathBuf>,
    pub toast: Option<(String, f64)>,
}

impl StageState {
    /// 懒加载侧边栏专属组件列表（非 demo 模式读写 `allegro-sidebar.json`，demo 模式仅内存隔离）。
    pub fn ensure_sidebar_loaded(&mut self, data_dir: Option<&Path>, demo: bool) {
        if self.sidebar_loaded {
            return;
        }
        self.sidebar_loaded = true;
        if demo {
            self.sidebar_path = None;
            self.sidebar_widgets = default_sidebar_widgets();
        } else if let Some(dir) = data_dir {
            let path = dir.join("allegro-sidebar.json");
            self.sidebar_widgets = load_sidebar_widgets(&path);
            self.sidebar_path = Some(path);
        } else {
            self.sidebar_widgets = default_sidebar_widgets();
        }
    }

    /// 将当前侧边栏独立组件列表持久化。
    pub fn save_sidebar(&self) {
        if let Some(path) = &self.sidebar_path {
            save_sidebar_widgets(path, &self.sidebar_widgets);
        }
    }

    /// 向侧边栏独立列表添加指定类型与区块尺寸的小组件。
    pub fn add_sidebar_widget(&mut self, kind: Kind, size: Size, caption: &str) {
        let seq = self.sidebar_widgets.len() + 1;
        let mut id = format!("sb-{}-{seq}", kind.index());
        let mut salt = 1usize;
        while self.sidebar_widgets.iter().any(|w| w.id == id) {
            id = format!("sb-{}-{seq}-{salt}", kind.index());
            salt += 1;
        }
        let material = "acrylic";
        let mut cfg = Config::default();
        if !caption.is_empty() {
            cfg.caption = caption.into();
        }
        if kind == Kind::Weather {
            cfg.city = "上海".into();
        }
        self.sidebar_widgets.push(Placement {
            id,
            kind,
            size,
            style: Style::Original,
            config: cfg,
            x: 0,
            y: 0,
            width: 0.0,
            height: 0.0,
            hidden: false,
            revision: 1,
            material: material.into(),
            opacity: 0.96,
            stack_id: String::new(),
            stack_order: 0,
        });
        self.save_sidebar();
    }

    /// 从侧边栏独立列表中移除指定 ID 的小组件。
    pub fn remove_sidebar_widget(&mut self, id: &str) -> bool {
        let before = self.sidebar_widgets.len();
        self.sidebar_widgets.retain(|w| w.id != id);
        if self.sidebar_widgets.len() != before {
            self.save_sidebar();
            true
        } else {
            false
        }
    }

    /// 修改侧边栏单个独立小组件的区块尺寸（小 1×1 / 大 2×1 / 特大 2×2）。
    pub fn set_sidebar_size(&mut self, id: &str, size: Size) -> bool {
        if let Some(w) = self.sidebar_widgets.iter_mut().find(|w| w.id == id) {
            w.size = size;
            w.width = 0.0;
            w.height = 0.0;
            self.save_sidebar();
            true
        } else {
            false
        }
    }

    /// 轮切侧边栏组件尺寸：小 (1×1) -> 大 (2×1) -> 特大 (2×2)。
    pub fn cycle_sidebar_size(&mut self, id: &str) -> Option<Size> {
        let next = {
            let w = self.sidebar_widgets.iter_mut().find(|w| w.id == id)?;
            w.size = match w.size {
                Size::Small => Size::Medium,
                Size::Medium => Size::Large,
                Size::Large => Size::Small,
            };
            w.width = 0.0;
            w.height = 0.0;
            w.size
        };
        self.save_sidebar();
        Some(next)
    }

    /// 修改侧边栏单个独立小组件的材质（耀光 / 亚克力）。
    pub fn set_sidebar_material(&mut self, id: &str, material: &str) -> bool {
        if let Some(w) = self.sidebar_widgets.iter_mut().find(|w| w.id == id) {
            w.material = material.into();
            self.save_sidebar();
            true
        } else {
            false
        }
    }

    /// 调整侧边栏独立小组件在区块流中的前后顺序。
    pub fn move_sidebar_widget(&mut self, id: &str, delta: i32) -> bool {
        let Some(pos) = self.sidebar_widgets.iter().position(|w| w.id == id) else {
            return false;
        };
        let target = (pos as i32 + delta).clamp(0, self.sidebar_widgets.len().saturating_sub(1) as i32) as usize;
        if target == pos {
            return false;
        }
        let item = self.sidebar_widgets.remove(pos);
        self.sidebar_widgets.insert(target, item);
        self.save_sidebar();
        true
    }

    /// 切换侧边栏内应用文件夹的 4 / 9 宫格布局。
    pub fn toggle_sidebar_app_folder_grid(&mut self, id: &str) -> bool {
        if let Some(w) = self.sidebar_widgets.iter_mut().find(|w| w.id == id) {
            w.config.caption = if w.config.caption.trim() == "9" {
                "4".into()
            } else {
                "9".into()
            };
            self.save_sidebar();
            true
        } else {
            false
        }
    }

    /// 切换侧边栏内便签组件的勾选框状态。
    pub fn toggle_sidebar_note_check(&mut self, id: &str, line_idx: usize) -> bool {
        if let Some(w) = self.sidebar_widgets.iter_mut().find(|w| w.id == id) {
            if let Some(next_text) = toggle_note_checkbox(&w.config.text, line_idx) {
                w.config.text = next_text;
                self.save_sidebar();
                return true;
            }
        }
        false
    }

    /// 推进轻提示计时器，返回是否仍在显示提示（用于驱动渲染）。
    pub fn tick(&mut self, dt: f64) -> bool {
        if let Some((_, left)) = &mut self.toast {
            *left -= dt.max(0.0);
            if *left <= 0.0 {
                self.toast = None;
                return false;
            }
            return true;
        }
        false
    }

    pub fn set_toast(&mut self, msg: impl Into<String>) {
        self.toast = Some((msg.into(), 2.4));
    }

    pub fn clear_query(&mut self) {
        self.query.clear();
        self.preedit.clear();
        self.selected = 0;
    }

    pub fn backspace(&mut self) {
        self.preedit.clear();
        self.query.pop();
        self.selected = 0;
    }

    pub fn push_str(&mut self, s: &str) {
        self.preedit.clear();
        if self.query.chars().count() < 64 {
            self.query.push_str(s);
            self.selected = 0;
        }
    }

    pub fn step_selection(&mut self, delta: isize, total: usize) {
        if total == 0 {
            self.selected = 0;
            return;
        }
        let n = total as isize;
        self.selected = ((self.selected as isize + delta).rem_euclid(n)) as usize;
    }
}
