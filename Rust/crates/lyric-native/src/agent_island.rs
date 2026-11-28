//! Native activity state and shared geometry for the AI quota island.
use crate::{ai_quota::Reading, layout::Rect};
use std::{
    collections::BTreeMap,
    sync::mpsc,
    time::{Duration, Instant},
};

pub const ID: &str = "agent-quota";
pub const KIND: u8 = 6;
pub const COLLAPSE: i32 = 1700;
pub const PREV: i32 = 1701;
pub const NEXT: i32 = 1702;
pub const CUSTOM: i32 = 1703;
pub const SEND: i32 = 1704;
pub const OPTION: i32 = 1720;
pub const OPTIONS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Agent {
    Codex,
    Claude,
    ZCode,
    Cursor,
    Harness,
}
const AGENT_COUNT: usize = Agent::ALL.len();
impl Agent {
    pub const ALL: [Self; 5] = [Self::Codex, Self::Claude, Self::ZCode, Self::Cursor, Self::Harness];
    fn index(self) -> usize {
        self as usize
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::Claude => "Claude",
            Self::ZCode => "ZCode",
            Self::Cursor => "Cursor",
            Self::Harness => "DeepSeek Harness",
        }
    }
    fn provider(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::ZCode => "zcode",
            Self::Cursor => "cursor",
            Self::Harness => "dsh",
        }
    }
    pub fn process(name: &str) -> Option<Self> {
        match name.to_ascii_lowercase().as_str() {
            "codex.exe"
            | "codex"
            | "codex-cli.exe"
            | "codex-x86_64-pc-windows-msvc.exe"
            | "codex-aarch64-pc-windows-msvc.exe" => Some(Self::Codex),
            "claude.exe" | "claude" | "claude-code.exe" => Some(Self::Claude),
            "zcode.exe" | "zcode" => Some(Self::ZCode),
            "cursor.exe" => Some(Self::Cursor),
            "deepseek harness.exe" | "deepseek-harness.exe" => Some(Self::Harness),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq)]
pub struct Tile {
    pub agent: Agent,
    pub hour: Option<f64>,
    pub week: Option<f64>,
    pub wallet: Vec<String>,
    pub token_quota: Option<crate::ai_quota::TokenQuota>,
}
impl Tile {
    pub fn fallback_tokens(&self) -> Option<crate::ai_quota::TokenQuota> {
        if self.agent == Agent::ZCode && self.hour.is_none() && self.week.is_none() {
            self.token_quota
        } else {
            None
        }
    }

    fn new(agent: Agent, quota: Option<&Reading>) -> Self {
        Self {
            agent,
            hour: quota.and_then(|q| q.five_hour),
            week: quota.and_then(|q| q.weekly),
            wallet: quota.map(Reading::harness_balances).unwrap_or_default(),
            token_quota: quota.and_then(|q| q.zcode_tokens(crate::ai_quota::now())),
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Question {
    pub id: String,
    pub text: String,
    pub labels: Vec<String>,
    pub custom: bool,
    pub multi: bool,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Prompt {
    // The token is generated locally. It never grants access through settings IPC.
    pub token: u64,
    pub questions: Vec<Question>,
}
#[derive(Clone, Debug)]
pub struct Submission {
    pub token: u64,
    pub answers: BTreeMap<String, Vec<String>>,
}

#[derive(Default)]
pub struct View {
    pub tiles: Vec<Tile>,
    pub prompt: Option<Prompt>,
    pub page: usize,
    pub editing: bool,
    pub draft: Vec<u16>,
    pub sending: bool,
    pub feedback: &'static str,
    answers: BTreeMap<String, Vec<String>>,
}
pub const MAX_COMPACT_WIDTH: f64 = 584.;
pub fn tile_rect(card: Rect, i: usize, count: usize) -> Rect {
    let columns = if count == 1 { 1 } else { 2 };
    let gap = 8.;
    let width = (card.w - 32. - (columns - 1) as f64 * gap) / columns as f64;
    Rect::new(card.x + 16. + (i % columns) as f64 * (width + gap),
        card.y + 48. + (i / columns) as f64 * (80. + gap), width, 80.)
}

impl View {
    pub fn compact_width(&self) -> f64 {
        (136. * self.tiles.len().max(1) as f64 + 16.).min(MAX_COMPACT_WIDTH)
    }
    pub fn height(&self) -> f64 {
        let quota_height = 148. + self.tiles.len().max(1).div_ceil(2).saturating_sub(1) as f64 * 88.;
        self.prompt.as_ref().map_or(quota_height, |p| {
            let rows = p
                .questions
                .iter()
                .map(|q| q.labels.len())
                .max()
                .unwrap_or(0);
            let any_custom = p.questions.iter().any(|q| q.custom);
            150. + rows as f64 * 30. + if any_custom { 42. } else { 0. }
        })
    }
    pub fn question(&self) -> Option<&Question> {
        self.prompt.as_ref()?.questions.get(self.page)
    }
    pub fn answer(&self) -> &[String] {
        self.question()
            .and_then(|q| self.answers.get(&q.id))
            .map_or(&[], Vec::as_slice)
    }
    pub fn set_prompt(&mut self, value: Option<Prompt>) -> bool {
        if self.prompt == value {
            return false;
        }
        self.prompt = value;
        self.page = 0;
        self.editing = false;
        self.draft.clear();
        self.sending = false;
        self.feedback = "";
        self.answers.clear();
        true
    }
    pub fn choose(&mut self, index: usize) {
        if self.sending {
            return;
        }
        let Some(q) = self.question() else {
            return;
        };
        let Some(value) = q.labels.get(index).cloned() else {
            return;
        };
        let (id, multi, labels) = (q.id.clone(), q.multi, q.labels.clone());
        let entry = self.answers.entry(id).or_default();
        if multi {
            entry.retain(|v| labels.contains(v));
            if let Some(i) = entry.iter().position(|v| *v == value) {
                entry.remove(i);
            } else {
                entry.push(value);
            }
        } else {
            *entry = vec![value];
        }
        self.feedback = "";
    }
    pub fn start_edit(&mut self) -> bool {
        let Some(q) = self.question() else {
            return false;
        };
        if self.sending || !q.custom {
            return false;
        }
        self.draft = self
            .answer()
            .iter()
            .find(|s| !q.labels.contains(s))
            .map_or_else(Vec::new, |s| s.encode_utf16().collect());
        self.editing = true;
        self.feedback = "输入后按 Enter 保存，Esc 取消";
        true
    }
    pub fn finish_edit(&mut self, save: bool) {
        if !self.editing {
            return;
        }
        self.editing = false;
        let text = String::from_utf16_lossy(&std::mem::take(&mut self.draft));
        if save && !text.trim().is_empty() {
            if let Some(q) = self.question() {
                self.answers.insert(q.id.clone(), vec![text.trim().into()]);
            }
        }
        self.feedback = "";
    }
    pub fn can_send(&self) -> bool {
        !self.sending
            && !self.editing
            && self.prompt.as_ref().is_some_and(|p| {
                p.questions
                    .iter()
                    .all(|q| self.answers.get(&q.id).is_some_and(|a| !a.is_empty()))
            })
    }
    pub fn submission(&mut self) -> Option<Submission> {
        if !self.can_send() {
            return None;
        }
        let token = self.prompt.as_ref()?.token;
        self.sending = true;
        self.feedback = "正在向原会话发送…";
        Some(Submission {
            token,
            answers: self.answers.clone(),
        })
    }
}
pub fn close(r: Rect) -> Rect {
    Rect::new(r.w - 42., 12., 30., 28.)
}
pub fn option(r: Rect, i: usize) -> Rect {
    Rect::new(16., 104. + i as f64 * 30., r.w - 32., 27.)
}
pub fn custom(r: Rect) -> Rect {
    Rect::new(16., r.h - 84., r.w - 32., 32.)
}
pub fn prev(r: Rect) -> Rect {
    Rect::new(16., r.h - 44., 42., 31.)
}
pub fn next(r: Rect) -> Rect {
    Rect::new(64., r.h - 44., 42., 31.)
}
pub fn send(r: Rect) -> Rect {
    Rect::new(r.w - 116., r.h - 44., 100., 31.)
}
pub fn hit(v: &View, r: Rect, x: f64, y: f64) -> i32 {
    if close(r).contains(x, y) {
        return COLLAPSE;
    }
    if v.sending {
        return 0;
    }
    if let Some(q) = v.question() {
        for i in 0..q.labels.len() {
            if option(r, i).contains(x, y) {
                return OPTION + i as i32;
            }
        }
        if q.custom && custom(r).contains(x, y) {
            return CUSTOM;
        }
        if v.page > 0 && prev(r).contains(x, y) {
            return PREV;
        }
        if v.prompt
            .as_ref()
            .is_some_and(|p| v.page + 1 < p.questions.len())
            && next(r).contains(x, y)
        {
            return NEXT;
        }
        if v.can_send() && send(r).contains(x, y) {
            return SEND;
        }
    }
    0
}

#[derive(Default)]
pub struct Monitor {
    pub view: View,
    process_rx: Option<mpsc::Receiver<[bool; AGENT_COUNT]>>,
    last: Option<Instant>,
    active: [bool; AGENT_COUNT],
    pub bridge: crate::agent_desktop::Bridge,
}
impl Monitor {
    pub fn poll(&mut self, enabled: bool, readings: &[Reading]) -> (bool, bool, bool) {
        if !enabled {
            let changed = !self.view.tiles.is_empty() || self.view.prompt.is_some();
            *self = Self::default();
            return (changed, false, false);
        }
        if let Some(rx) = self.process_rx.as_ref() {
            match rx.try_recv() {
                Ok(active) => {
                    self.active = active;
                    self.process_rx = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.active = [false; AGENT_COUNT];
                    self.process_rx = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.process_rx.is_none()
            && self
                .last
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(2))
        {
            self.last = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            if std::thread::Builder::new()
                .name("agent-processes".into())
                .spawn(move || {
                    let _ = tx.send(processes());
                })
                .is_ok()
            {
                self.process_rx = Some(rx);
            }
        }
        let (prompt, feedback) = self.bridge.poll(self.active[0]);
        let tiles: Vec<_> = Agent::ALL
            .into_iter()
            .enumerate()
            .filter(|(i, _)| self.active[*i])
            .map(|(_, a)| {
                Tile::new(
                    a,
                    readings
                        .iter()
                        .filter(|r| r.provider == a.provider())
                        .max_by_key(|r| {
                            usize::from(r.five_hour.is_some()) + usize::from(r.weekly.is_some()) + r.harness_balances().len() + usize::from(r.zcode_tokens(crate::ai_quota::now()).is_some())
                        }),
                )
            })
            .collect();
        let appeared = tiles
            .iter()
            .any(|tile| !self.view.tiles.iter().any(|old| old.agent == tile.agent));
        let changed = tiles != self.view.tiles;
        self.view.tiles = tiles;
        let question_changed = self.view.set_prompt(prompt);
        if let Some(feedback) = feedback {
            self.view.feedback = feedback;
        }
        (
            changed || question_changed || feedback.is_some(),
            appeared,
            question_changed && self.view.prompt.is_some(),
        )
    }
}

fn processes() -> [bool; AGENT_COUNT] {
    use windows::Win32::{
        Foundation::CloseHandle,
        System::{
            Diagnostics::ToolHelp::*,
            RemoteDesktop::ProcessIdToSessionId,
            Threading::{GetCurrentProcessId, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION},
        },
    };
    let mut result = [false; AGENT_COUNT];
    unsafe {
        let mut own = 0;
        if ProcessIdToSessionId(GetCurrentProcessId(), &mut own).is_err() {
            return result;
        }
        let Ok(snapshot) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return result;
        };
        let mut p = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        if Process32FirstW(snapshot, &mut p).is_ok() {
            loop {
                let n = p
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(p.szExeFile.len());
                let name = String::from_utf16_lossy(&p.szExeFile[..n]);
                let agent = Agent::process(&name).or_else(|| {
                    if !name.eq_ignore_ascii_case("ChatGPT.exe") {
                        return None;
                    }
                    let p = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, p.th32ProcessID)
                        .ok()?;
                    let is_codex = crate::agent_desktop::verified_package(p);
                    let _ = CloseHandle(p);
                    is_codex.then_some(Agent::Codex)
                });
                if let Some(agent) = agent {
                    let mut process_session = 0;
                    if ProcessIdToSessionId(p.th32ProcessID, &mut process_session).is_ok()
                        && process_session == own
                    {
                        result[agent.index()] = true;
                    }
                }
                if Process32NextW(snapshot, &mut p).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn agent_grid_fits_all_five_without_clipping_and_harness_does_not_match_web_hosts() {
        let v = View { tiles: Agent::ALL.into_iter().map(|a| Tile::new(a, None)).collect(), ..Default::default() };
        let r = Rect::new(0.,0.,400.,v.height());
        for i in 0..v.tiles.len() {
            let a = tile_rect(r,i,v.tiles.len());
            assert!(a.x >= 16. && a.x+a.w <= r.w-16. && a.y+a.h <= r.h-20.);
            for j in 0..i {
                let b = tile_rect(r,j,v.tiles.len());
                assert!(b.x+b.w+8. <= a.x || a.x+a.w+8. <= b.x || b.y+b.h+8. <= a.y);
            }
        }
        assert!(v.compact_width() <= MAX_COMPACT_WIDTH);
        assert_eq!(Agent::process("DeepSeek Harness.exe"), Some(Agent::Harness));
        assert_eq!(Agent::process("Cursor.exe"), Some(Agent::Cursor));
        assert_eq!(Agent::process("node.exe"), None);
    }
    #[test]
    fn zcode_only_falls_back_to_tokens_without_either_window() {
        let mut tile = Tile::new(Agent::ZCode, None);
        assert_eq!(tile.fallback_tokens(), None);

        let token = crate::ai_quota::TokenQuota { remaining: 0, total: 1_000_000 };
        tile.token_quota = Some(token);
        assert_eq!(tile.fallback_tokens(), Some(token));
        assert_eq!(tile.fallback_tokens().unwrap().compact_label(), "0 TOKEN");

        for (hour, week) in [(Some(0.), None), (None, Some(0.)), (Some(20.), Some(10.))] {
            tile.hour = hour;
            tile.week = week;
            assert_eq!(tile.fallback_tokens(), None);
        }
    }
    #[test]
    fn requires_each_answer_and_sends_only_once() {
        let mut v = View::default();
        let q = |id: &str| Question {
            id: id.into(),
            text: "选择".into(),
            labels: vec!["是".into()],
            custom: true,
            multi: false,
        };
        v.set_prompt(Some(Prompt {
            token: 5,
            questions: vec![q("a"), q("b")],
        }));
        v.choose(0);
        assert!(v.submission().is_none());
        v.page = 1;
        assert!(v.start_edit());
        v.draft = "自定义".encode_utf16().collect();
        v.finish_edit(true);
        let sent = v.submission().unwrap();
        assert_eq!(sent.token, 5);
        assert_eq!(sent.answers["b"], vec!["自定义"]);
        assert!(v.submission().is_none());
    }
}
