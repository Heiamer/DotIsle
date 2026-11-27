//! 侧栏配置与剪贴板后台 I/O：绘图只消费已完成的快照。
use allegro::stage::{load_sidebar_widgets, save_sidebar_widgets, ClipboardHistory, ClipboardItem};
use dotisle_domain::desktop_widgets::Placement;
use std::{path::PathBuf, sync::mpsc, thread::{self, JoinHandle}};

pub enum Result {
    Loaded(Vec<Placement>),
    Clipboard(Vec<ClipboardItem>),
    Copied(Option<String>),
}

enum Request {
    Save(Vec<Placement>),
    PollClipboard,
    Copy { text: String, from_history: bool },
}

pub struct SidebarIo {
    requests: Option<mpsc::Sender<Request>>,
    results: mpsc::Receiver<Result>,
    worker: Option<JoinHandle<()>>,
    clipboard_pending: bool,
}

impl SidebarIo {
    pub fn start(path: Option<PathBuf>) -> Self {
        let (requests, pending) = mpsc::channel();
        let (results, ready) = mpsc::channel();
        let worker = thread::Builder::new().name("allegro-sidebar-io".into()).spawn(move || {
            let publish = |result| {
                if results.send(result).is_ok() { crate::wake(); }
            };
            if let Some(path) = &path {
                publish(Result::Loaded(load_sidebar_widgets(path)));
            }
            let mut clipboard = ClipboardHistory::default();
            let mut last_items = Vec::new();
            while let Ok(request) = pending.recv() {
                match request {
                    Request::Save(widgets) => {
                        if let Some(path) = &path { save_sidebar_widgets(path, &widgets); }
                    }
                    Request::PollClipboard => {
                        clipboard.poll();
                        // 空结果也结束在途请求；无变化时不做历史快照拷贝。
                        let changed = last_items != clipboard.items;
                        if changed { last_items.clone_from(&clipboard.items); }
                        publish(Result::Clipboard(if changed { last_items.clone() } else { Vec::new() }));
                    }
                    Request::Copy { text, from_history } => {
                        if clipboard.write_text(&text) {
                            if from_history { clipboard.push_text(&text); }
                            publish(Result::Copied(Some(if from_history { String::new() } else { text })));
                        } else {
                            publish(Result::Copied(None));
                        }
                    }
                }
            }
        }).map_err(|err| eprintln!("Allegro sidebar I/O thread: {err}")).ok();
        Self { requests: Some(requests), results: ready, worker, clipboard_pending: false }
    }

    pub fn available(&self) -> bool { self.worker.is_some() }

    pub fn poll_clipboard(&mut self) {
        if !self.clipboard_pending {
            self.clipboard_pending = self.send(Request::PollClipboard);
        }
    }

    pub fn save(&self, widgets: Vec<Placement>) { self.send(Request::Save(widgets)); }

    pub fn copy(&self, text: String, from_history: bool) {
        self.send(Request::Copy { text, from_history });
    }

    pub fn take(&mut self) -> Option<Result> {
        let result = self.results.try_recv().ok()?;
        if matches!(result, Result::Clipboard(_)) { self.clipboard_pending = false; }
        Some(result)
    }

    fn send(&self, request: Request) -> bool {
        self.requests.as_ref().is_some_and(|tx| tx.send(request).is_ok())
    }
}

impl Drop for SidebarIo {
    fn drop(&mut self) {
        // 事件循环退出后排空未落盘的编辑；运行中从不等待 worker。
        self.requests.take();
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}
