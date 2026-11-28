// SPDX-License-Identifier: MIT
// Adapted from DesktopToast watcher.py (467b66ab6a31aa6db26e86b97f638512eb4e5604).
// Copyright (c) 2026 DesktopToast contributors. See licenses/DesktopToast-MIT.txt.
use crate::{
    message_hints::{self, App, Hint},
    notifications::{Event, Notice, Policy},
};
use std::collections::{hash_map::DefaultHasher, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use windows::{
    core::HSTRING,
    UI::Notifications::{
        Management::{UserNotificationListener, UserNotificationListenerAccessStatus},
        NotificationKinds, UserNotification,
    },
};

const INTERVAL: Duration = Duration::from_secs(1);
const BACKOFF: Duration = Duration::from_secs(4);
const DEDUP: Duration = Duration::from_secs(4);
const MAX_TOASTS: u32 = 800;

#[derive(serde::Deserialize)]
struct Toast {
    id: u32,
    time: i64,
    app: App,
    aumid: String,
    texts: Vec<String>,
}

fn can_read(policy: &Policy, aumid: &str, name: &str) -> bool {
    !policy.paused && policy.permits(aumid, name)
}

fn extract(notification: &UserNotification, policy: &Policy) -> Option<Toast> {
    let info = notification.AppInfo().ok()?;
    let aumid = info
        .AppUserModelId()
        .ok()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let name = info
        .DisplayInfo()
        .and_then(|d| d.DisplayName())
        .ok()
        .map(|s| s.to_string())
        .unwrap_or_default();
    let app = message_hints::identify(&aumid, &name).filter(|app| app.content_only())?;
    if !can_read(policy, aumid.trim(), name.trim()) {
        return None;
    }
    let id = notification.Id().ok()?;
    let time = notification
        .CreationTime()
        .map(|t| t.UniversalTime)
        .unwrap_or(0);
    let mut texts = Vec::new();
    if policy.enabled {
        let visual = notification.Notification().ok()?.Visual().ok()?;
        let binding = visual
            .GetBinding(&HSTRING::from("ToastGeneric"))
            .or_else(|_| visual.Bindings()?.GetAt(0))
            .ok()?;
        let elements = binding.GetTextElements().ok()?;
        for i in 0..elements.Size().ok()?.min(20) {
            if let Ok(text) = elements.GetAt(i).and_then(|t| t.Text()) {
                let line: String = text.to_string().trim().chars().take(500).collect();
                if !line.is_empty() {
                    texts.push(line);
                }
            }
        }
        if texts.is_empty() {
            return None;
        }
    }
    Some(Toast {
        id,
        time,
        app,
        aumid,
        texts,
    })
}

fn signature(app: App, parts: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    app.hash(&mut hasher);
    parts.hash(&mut hasher);
    hasher.finish()
}

#[derive(Default)]
struct History {
    seen: HashMap<u32, (i64, u64)>,
    recent: VecDeque<(u64, Instant)>,
    ready: bool,
}
impl History {
    fn fresh(&mut self, mut snapshot: Vec<Toast>, now: Instant) -> Vec<Toast> {
        snapshot.sort_by_key(|toast| toast.time);
        while self
            .recent
            .front()
            .is_some_and(|(_, at)| now.saturating_duration_since(*at) >= DEDUP)
        {
            self.recent.pop_front();
        }
        let mut seen = HashMap::new();
        let mut out = Vec::new();
        for toast in snapshot {
            let digest = signature(toast.app, &toast.texts);
            let key = (toast.time, digest);
            seen.insert(toast.id, key);
            if !self.ready || self.seen.get(&toast.id) == Some(&key) {
                continue;
            }
            // In metadata-only mode there is no body to deduplicate; keep each real ID.
            if !toast.texts.is_empty() {
                if self.recent.iter().any(|(sig, _)| *sig == digest) {
                    continue;
                }
                if self.recent.len() == 128 {
                    self.recent.pop_front();
                }
                self.recent.push_back((digest, now));
            }
            out.push(toast);
        }
        self.seen = seen;
        self.ready = true;
        out
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Disabled,
    Connecting,
    Listening,
    AccessNeeded,
    Unavailable,
}
impl Status {
    pub fn state(self) -> serde_json::Value {
        use serde_json::json;
        let (access, message) = match self {
            Self::Disabled => ("disabled", "飞书、钉钉的系统通知监听已关闭或暂停"),
            Self::Connecting => ("connecting", "正在连接 Windows 系统通知"),
            Self::Listening => (
                "listening",
                "已连接 Windows 系统通知；需在飞书或钉钉中开启系统通知",
            ),
            Self::AccessNeeded => (
                "permission",
                "未获 Windows 通知访问权限，请检查 Windows 隐私设置中的通知访问",
            ),
            Self::Unavailable => ("unavailable", "Windows 系统通知暂不可读，点屿会自动重试"),
        };
        json!({"access":access,"message":message,"source":"winrt"})
    }
}

pub struct Watcher {
    history: History,
    status: Status,
    next: Instant,
    config: Option<Policy>,
    fixture: Option<std::path::PathBuf>,
    icons: HashMap<App, Option<Vec<u8>>>,
}
impl Watcher {
    pub fn new() -> Self {
        Self {
            history: History::default(),
            status: Status::Disabled,
            next: Instant::now(),
            config: None,
            // Test snapshots and live client notifications must never mix.
            fixture: std::env::var("LYRICISLAND_TEST_PIPE")
                .ok()
                .filter(|pipe| pipe.starts_with("LyricIsland.Settings.test."))
                .and_then(|_| std::env::var_os("LYRICISLAND_DESKTOP_TOAST_FIXTURE"))
                .map(std::path::PathBuf::from),
            icons: HashMap::new(),
        }
    }

    pub fn state(&self) -> serde_json::Value {
        self.status.state()
    }

    pub fn poll(&mut self, policy: &Policy, stopping: &AtomicBool) -> Vec<Event> {
        let now = Instant::now();
        if self.config.as_ref() != Some(policy) {
            self.config = Some(policy.clone());
            self.history = History::default();
            self.next = now;
        }
        if policy.paused
            || ![App::Feishu, App::DingTalk]
                .iter()
                .any(|app| policy.message_apps.iter().any(|id| id == app.id()))
        {
            self.status = Status::Disabled;
            return vec![];
        }
        if now < self.next {
            return vec![];
        }
        self.status = Status::Connecting;
        let data = if let Some(path) = &self.fixture {
            use std::io::Read;
            std::fs::File::open(path)
                .and_then(|f| {
                    let mut bytes = Vec::new();
                    f.take(256 * 1024).read_to_end(&mut bytes)?;
                    Ok(bytes)
                })
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Vec<Toast>>(&bytes).ok())
                .map(|rows| {
                    rows.into_iter()
                        .take(MAX_TOASTS as usize)
                        .filter_map(|mut toast| {
                            if !toast.app.content_only()
                                || !can_read(policy, &toast.aumid, toast.app.name())
                            {
                                return None;
                            }
                            if !policy.enabled {
                                toast.texts.clear();
                            }
                            Some(toast)
                        })
                        .collect()
                })
                .ok_or(Status::Unavailable)
        } else {
            snapshot(policy, stopping)
        };
        match data {
            Ok(rows) => {
                self.status = Status::Listening;
                self.next = Instant::now() + INTERVAL;
                self.history
                    .fresh(rows, now)
                    .into_iter()
                    .map(|toast| {
                        let icon = self
                            .icons
                            .entry(toast.app)
                            .or_insert_with(|| crate::app_icon::shell_icon(&toast.aumid))
                            .clone();
                        to_event(toast, icon)
                    })
                    .collect()
            }
            Err(status) => {
                self.status = status;
                self.next = Instant::now() + BACKOFF;
                // On permission loss, do not replay messages from the inaccessible period.
                self.history = History::default();
                vec![]
            }
        }
    }
}

fn snapshot(policy: &Policy, stopping: &AtomicBool) -> Result<Vec<Toast>, Status> {
    let unavailable = |_| Status::Unavailable;
    let listener = UserNotificationListener::Current().map_err(unavailable)?;
    if listener.GetAccessStatus().map_err(unavailable)?
        != UserNotificationListenerAccessStatus::Allowed
    {
        // Never ask for permission from the worker or change permission silently.
        return Err(Status::AccessNeeded);
    }
    let op = listener
        .GetNotificationsAsync(NotificationKinds::Toast)
        .map_err(unavailable)?;
    let start = Instant::now();
    while op.Status().map_err(unavailable)?.0 == 0 {
        if stopping.load(Ordering::Acquire) || start.elapsed() >= Duration::from_millis(400) {
            let _ = op.Cancel();
            return Err(Status::Unavailable);
        }
        std::thread::sleep(Duration::from_millis(12));
    }
    let notifications = op.GetResults().map_err(unavailable)?;
    let count = notifications.Size().map_err(unavailable)?;
    Ok((0..count.min(MAX_TOASTS))
        .filter_map(|i| notifications.GetAt(i).ok())
        .filter_map(|notice| extract(&notice, policy))
        .collect())
}

fn to_event(toast: Toast, icon: Option<Vec<u8>>) -> Event {
    let app = toast.app;
    let first = toast.texts.first().cloned().unwrap_or_default();
    let body = toast.texts.get(1..).unwrap_or_default().join("\n");
    let placeholder = [
        "有新消息",
        "你有新消息",
        "您有新消息",
        "收到新消息",
        "你收到一条新消息",
        "您收到一条新消息",
        "你收到了一条新消息",
        "您收到了一条新消息",
    ];
    if first.is_empty()
        || (body.is_empty()
            && (first == app.name()
                || placeholder.contains(&first.trim_end_matches(['。', '！', '!']))))
        || placeholder.contains(&body.trim_end_matches(['。', '！', '!']))
    {
        let mut hint: Hint = app.into();
        hint.icon = icon;
        return Event::MessageHint(hint);
    }
    Event::Notice(Notice {
        app_id: app.id().into(),
        app_name: app.name().into(),
        title: first,
        body,
        icon,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toast(app: App, id: u32, body: &str) -> Toast {
        Toast {
            id,
            app,
            time: i64::from(id),
            aumid: app.id().into(),
            texts: vec!["发件人".into(), body.into()],
        }
    }

    #[test]
    fn baseline_id_updates_cross_id_dedup_and_both_apps() {
        let now = Instant::now();
        let mut history = History::default();
        assert!(history
            .fresh(vec![toast(App::Feishu, 1, "旧消息")], now)
            .is_empty());
        let out = history.fresh(
            vec![
                toast(App::Feishu, 1, "旧消息"),
                toast(App::Feishu, 2, "新消息"),
                toast(App::Feishu, 3, "新消息"),
                toast(App::DingTalk, 4, "新消息"),
            ],
            now,
        );
        assert_eq!(out.iter().map(|n| n.id).collect::<Vec<_>>(), [2, 4]);
        let out = history.fresh(
            vec![
                toast(App::Feishu, 2, "修订消息"),
                toast(App::DingTalk, 4, "新消息"),
            ],
            now + Duration::from_secs(1),
        );
        assert_eq!(out.iter().map(|n| n.id).collect::<Vec<_>>(), [2]);
        let out = history.fresh(vec![toast(App::DingTalk, 5, "新消息")], now + DEDUP);
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn respects_pause_and_app_filters_and_preserves_real_lines() {
        let mut policy =
            Policy::from_settings(&serde_json::json!({"DingTalkMessageHintsEnabled":false}));
        assert!(can_read(&policy, "Feishu", "飞书"));
        assert!(!can_read(&policy, "DingTalk", "钉钉"));
        policy.paused = true;
        assert!(!can_read(&policy, "Feishu", "飞书"));
        let mut notice = toast(App::Feishu, 1, "正文");
        notice.texts.push("第二行".into());
        let Event::Notice(notice) = to_event(notice, None) else {
            panic!("content missing")
        };
        assert_eq!(
            (
                notice.app_id.as_str(),
                notice.title.as_str(),
                notice.body.as_str()
            ),
            ("Feishu", "发件人", "正文\n第二行")
        );
        let Event::MessageHint(hint) =
            to_event(toast(App::DingTalk, 2, "您收到了一条新消息。"), None)
        else {
            panic!("fabricated body")
        };
        assert_eq!(hint.app_id, "DingTalk");
    }
}
