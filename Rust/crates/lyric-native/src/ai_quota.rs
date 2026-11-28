// SPDX-License-Identifier: Apache-2.0
//! Derived from BrimDeck LogReader.cs (Codex), ClaudeUsageHistory.cs and DataLocations.cs.
//! Apache-2.0, see licenses/BrimDeck-Apache-2.0.txt and BrimDeck-NOTICE.txt.
//! Modified: Rust port; read-only local usage only, freshness/file bounds and threshold alerts.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
#[path = "ai_quota_sources.rs"]
pub mod sources;
const MAX_READ: u64 = 2 * 1024 * 1024;
const FRESH: i64 = 6 * 3600;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub enabled: bool,
    pub five_hour: u8,
    pub weekly: u8,
    #[serde(rename = "relayCurrency")]
    pub relay_unit: RelayUnit,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: false,
            five_hour: 20,
            weekly: 10,
            relay_unit: RelayUnit::Usd,
        }
    }
}
impl Settings {
    pub fn from_value(v: &Value) -> Self {
        serde_json::from_value::<Self>(v.clone())
            .ok()
            .filter(Self::valid)
            .unwrap_or_default()
    }
    pub fn valid(&self) -> bool {
        self.five_hour <= 100 && self.weekly <= 100
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Reading {
    id: String,
    pub provider: String,
    name: String,
    status: String,
    plan: String,
    pub five_hour: Option<f64>,
    pub weekly: Option<f64>,
    sampled_at: i64,
    metrics: Vec<Metric>,
}
#[derive(Clone, Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Metric {
    label: String,
    value: String,
    reset_at: Option<i64>,
    expires_at: Option<i64>,
    window: Option<u64>,
    remaining: Option<f64>,
    #[serde(skip)]
    relay_money: Option<RelayBalance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    token_quota: Option<TokenQuota>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
pub struct TokenQuota {
    pub remaining: u64,
    pub total: u64,
}
impl TokenQuota {
    pub fn compact_label(self) -> String {
        for (size, suffix) in [(1_000_000_000_000_000_000_u64, "E"), (1_000_000_000_000_000, "P"), (1_000_000_000_000, "T"), (1_000_000_000, "B"), (1_000_000, "M"), (1_000, "K")] {
            if self.remaining >= size {
                let whole = self.remaining / size;
                let fraction = (self.remaining % size) / (size / 10);
                return if fraction == 0 { format!("{whole}{suffix} TOKEN") } else { format!("{whole}.{fraction}{suffix} TOKEN") };
            }
        }
        self.remaining_label()
    }
    pub fn remaining_label(self) -> String {
        format!("{} TOKEN", self.remaining)
    }
}

pub const RELAY_WARNING: f64 = 1.;
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum RelayUnit {
    #[default]
    Usd,
    Cny,
    Points,
}
impl RelayUnit {
    pub const ALL: [Self; 3] = [Self::Usd, Self::Cny, Self::Points];
    pub fn from_unit(unit: &str) -> Option<Self> {
        if unit.eq_ignore_ascii_case("USD") { Some(Self::Usd) }
        else if unit.eq_ignore_ascii_case("CNY") || unit.eq_ignore_ascii_case("RMB") { Some(Self::Cny) }
        else if unit == "积分" || unit.eq_ignore_ascii_case("POINTS") || unit.eq_ignore_ascii_case("POINT") { Some(Self::Points) }
        else { None }
    }
    pub fn code(self) -> &'static str {
        match self { Self::Usd => "USD", Self::Cny => "CNY", Self::Points => "POINTS" }
    }
    pub fn symbol(self) -> &'static str {
        match self { Self::Usd => "US$", Self::Cny => "¥", Self::Points => "" }
    }
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelayBalance {
    pub amount: f64,
    pub currency: RelayUnit,
    pub key: bool,
}
impl RelayBalance {
    pub fn amount_label(self) -> String {
        let suffix = if self.currency == RelayUnit::Points { " 积分" } else { "" };
        let symbol = self.currency.symbol();
        // Avoid displaying the exact threshold after rounding a below-boundary value.
        if (RELAY_WARNING - 0.005..RELAY_WARNING).contains(&self.amount) {
            format!("< {symbol}{RELAY_WARNING:.2}{suffix}")
        } else {
            format!("{symbol}{:.2}{suffix}", self.amount)
        }
    }
}

impl Reading {
    fn relay_balance(&self, now: i64, currency: RelayUnit) -> Option<RelayBalance> {
        if !(0..=300).contains(&(now - self.sampled_at)) {
            return None;
        }
        self.metrics.iter()
            .filter(|m| m.expires_at.is_none_or(|t| t > now))
            .filter_map(|m| m.relay_money)
            .filter(|m| m.currency == currency)
            .min_by(|a,b| a.amount.total_cmp(&b.amount).then(a.key.cmp(&b.key)))
    }
    pub fn harness_balances(&self) -> Vec<String> {
        if self.provider != "dsh" { return Vec::new(); }
        self.metrics.iter().filter(|m| m.label == "账户余额").take(2).map(|m| m.value.clone()).collect()
    }
    pub fn zcode_tokens(&self, now: i64) -> Option<TokenQuota> {
        if self.provider != "zcode" { return None; }
        self.metrics.iter()
            .filter(|m| m.reset_at.is_none_or(|at| at > now) && m.expires_at.is_none_or(|at| at > now))
            .find_map(|m| m.token_quota)
    }
    fn blank(id: &str, provider: &str, name: &str, sampled_at: i64) -> Self {
        Self {
            id: id.into(),
            provider: provider.into(),
            name: name.into(),
            sampled_at,
            status: "暂无额度记录".into(),
            plan: String::new(),
            metrics: Vec::new(),
            five_hour: None,
            weekly: None,
        }
    }
    fn pick_windows(&mut self, now: i64) {
        let min_left = |n| {
            self.metrics
                .iter()
                .filter(|m| {
                    m.window == Some(n)
                        && m.reset_at.is_none_or(|r| r > now)
                        && m.expires_at.is_none_or(|e| e > now)
                })
                .filter_map(|m| m.remaining)
                .min_by(f64::total_cmp)
        };
        self.five_hour = min_left(300);
        self.weekly = min_left(10080);
    }
    fn add_local_windows(&mut self) {
        if self.metrics.is_empty() {
            self.metrics = [
                ("5 小时", 300, self.five_hour),
                ("每周", 10080, self.weekly),
            ]
            .into_iter()
            .filter_map(|(label, window, value)| {
                Some(Metric {
                    label: label.into(),
                    value: format!("剩余 {:.1}%", value?),
                    window: Some(window),
                    remaining: value,
                    ..Default::default()
                })
            })
            .collect();
        }
        if !self.metrics.is_empty() {
            self.status = "本机记录".into();
        }
    }
}
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}
fn stamp(v: &Value) -> Option<i64> {
    v.as_i64().or_else(|| {
        chrono::DateTime::parse_from_rfc3339(v.as_str()?)
            .ok()
            .map(|t| t.timestamp())
    })
}
fn fresh(at: i64, now: i64) -> bool {
    (now - FRESH..=now + 300).contains(&at)
}
fn remaining(v: &Value) -> Option<f64> {
    let n = v.as_f64()?;
    n.is_finite().then(|| 100. - n.clamp(0., 100.))
}
fn codex_record(event: &Value, now: i64) -> Option<Reading> {
    let at = stamp(&event["timestamp"])?;
    let p = &event["payload"];
    if !fresh(at, now) || event["type"] != "event_msg" || p["type"] != "token_count" {
        return None;
    }
    let limits = &p["rate_limits"];
    if !matches!(limits["limit_id"].as_str(), None | Some("") | Some("codex")) {
        return None;
    }
    let mut result = Reading::blank("codex-local", "codex", "Codex", at);
    for w in [&limits["primary"], &limits["secondary"]] {
        let reset = stamp(&w["resets_at"]).or_else(|| {
            w["reset_after_seconds"]
                .as_i64()
                .filter(|n| (0..31_536_000).contains(n))
                .map(|n| at + n)
        });
        if reset.is_some_and(|n| n <= now) {
            continue;
        }
        if let (Some(minutes), Some(left)) =
            (w["window_minutes"].as_u64(), remaining(&w["used_percent"]))
        {
            let label = match minutes {
                300 => "5 小时".into(),
                10080 => "每周".into(),
                m => format!("{m} 分钟"),
            };
            result.metrics.push(Metric {
                label,
                value: format!("剩余 {left:.1}%"),
                reset_at: reset,
                window: Some(minutes),
                remaining: Some(left),
                ..Default::default()
            });
        }
    }
    result.pick_windows(now);
    // Retain the newest sample even after its reset, so an older record cannot
    // resurrect an expired window. Unknown windows are never treated as zero.
    limits.is_object().then_some(result)
}
fn claude_history(doc: &Value, now: i64) -> Option<Reading> {
    doc["samples"]
        .as_array()?
        .iter()
        .filter_map(|v| {
            let at = stamp(&v["t"])?;
            if !fresh(at, now) || now - at > 1800 {
                return None;
            } // no reset timestamp in this cache
            let u = &v["u"];
            let (five_hour, weekly) = (remaining(&u["fh"]), remaining(&u["sd"]));
            (five_hour.is_some() || weekly.is_some()).then_some(Reading {
                five_hour,
                weekly,
                ..Reading::blank("claude-local", "claude", "Claude", at)
            })
        })
        .max_by_key(|r| r.sampled_at)
}
fn read_codex_tail(path: &Path, now: i64) -> Option<Reading> {
    let mut f = fs::File::open(path).ok()?;
    let start = f.metadata().ok()?.len().saturating_sub(MAX_READ);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    f.take(MAX_READ).read_to_end(&mut bytes).ok()?;
    let mut lines = bytes.split(|b| *b == b'\n');
    if start != 0 {
        lines.next();
    }
    lines
        .filter(|line| {
            line.windows(b"rate_limits".len())
                .any(|x| x == b"rate_limits")
        })
        .filter_map(|line| {
            serde_json::from_slice::<Value>(line)
                .ok()
                .and_then(|v| codex_record(&v, now))
        })
        .max_by_key(|r| r.sampled_at)
}
fn subdirs(path: &Path) -> impl Iterator<Item = PathBuf> {
    fs::read_dir(path)
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .map(|e| e.path())
        .take(256)
}
pub(crate) fn codex_home() -> Option<PathBuf> {
    std::env::var_os("CODEX_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".codex")))
}
fn samples() -> Vec<Reading> {
    let now = now();
    let mut out = Vec::new();
    if let Some(home) = codex_home() {
        let mut candidates = Vec::new();
        if let Some(date) = chrono::DateTime::from_timestamp(now, 0) {
            // Include adjacent local-calendar days (session directories are local date).
            for day in -2..=1 {
                let date = date + chrono::TimeDelta::days(day);
                let dir = home
                    .join("sessions")
                    .join(date.format("%Y/%m/%d").to_string());
                for entry in fs::read_dir(dir).into_iter().flatten().flatten().take(4096) {
                    let name = entry.file_name();
                    let name = name.to_string_lossy();
                    if name.starts_with("rollout-")
                        && name.ends_with(".jsonl")
                        && entry.file_type().is_ok_and(|t| t.is_file())
                    {
                        if let Ok(modified) = entry.metadata().and_then(|m| m.modified()) {
                            candidates.push((modified, entry.path()));
                        }
                    }
                }
            }
        }
        candidates.sort_unstable_by(|a, b| b.0.cmp(&a.0));
        if let Some(reading) = candidates
            .iter()
            .take(24)
            .filter_map(|(_, p)| read_codex_tail(p, now))
            .max_by_key(|r| r.sampled_at)
        {
            out.push(reading);
        }
    }
    let mut profiles = Vec::new();
    if let Some(dir) = std::env::var_os("APPDATA") {
        profiles.push(PathBuf::from(dir).join("Claude"));
    }
    if let Some(dir) = std::env::var_os("LOCALAPPDATA").map(PathBuf::from) {
        profiles
            .extend(subdirs(&dir.join("Claude-Data")).filter(|p| p.join("config.json").is_file()));
        profiles.extend(
            subdirs(&dir.join("Packages"))
                .filter(|p| {
                    p.file_name().is_some_and(|n| {
                        n.to_string_lossy().starts_with("Claude_")
                            || n.to_string_lossy().starts_with("AnthropicPBC.Claude_")
                    })
                })
                .map(|p| p.join("LocalCache/Roaming/Claude")),
        );
    }
    if let Some(r) = profiles
        .iter()
        .filter_map(|profile| {
            if !profile.join("config.json").is_file()
                && fs::read_dir(profile)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|e| {
                        e.file_name()
                            .to_string_lossy()
                            .starts_with(".user-data-left.")
                    })
            {
                return None;
            }
            let p = profile.join("plan-usage-history.json");
            let f = fs::File::open(p).ok()?;
            if f.metadata().ok()?.len() > MAX_READ {
                return None;
            }
            let doc = serde_json::from_reader(f.take(MAX_READ)).ok()?;
            claude_history(&doc, now)
        })
        .max_by_key(|r| r.sampled_at)
    {
        out.push(r);
    }
    for record in &mut out {
        record.add_local_windows();
    }
    for (code, name) in [("codex", "Codex"), ("claude", "Claude")] {
        if !out.iter().any(|r| r.provider == code) {
            out.push(Reading::blank(&format!("{code}-local"), code, name, now));
        }
    }
    out
}
#[derive(Default)]
pub struct Monitor {
    rx: Option<mpsc::Receiver<Vec<Reading>>>,
    last: Option<Instant>,
    readings: Vec<Reading>,
    cancel: Option<Arc<AtomicBool>>,
    announced: std::collections::HashMap<String, [bool; 2]>,
    relay_announced: std::collections::HashMap<String, [bool; 3]>,
}
impl Drop for Monitor {
    fn drop(&mut self) -> () {
        if let Some(signal) = &self.cancel {
            signal.store(true, Ordering::Release);
        }
    }
}
impl Monitor {
    pub fn readings(&self) -> &[Reading] {
        &self.readings
    }
    pub fn refresh_soon(&mut self) {
        if self.rx.is_none() {
            self.last = None;
        }
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn state(&self, s: Settings, registered: &Value) -> Value {
        json!({"settings":s,"readings":self.readings,"pending":self.rx.is_some(),"nativeSources":true,"catalog":sources::catalog(),"profiles":profiles(registered)})
    }
    pub fn poll(
        &mut self,
        settings: Settings,
        can_show: bool,
        registered: &Value,
        data: &Path,
    ) -> Vec<crate::capsule_alert::Alert> {
        if !settings.enabled {
            return Vec::new();
        }
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(values) => {
                    self.rx = None;
                    self.cancel = None;
                    self.readings = values;
                }
                Err(mpsc::TryRecvError::Empty) => {}
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                    self.readings.clear();
                }
            }
        }
        if self.rx.is_none()
            && self
                .last
                .is_none_or(|t| t.elapsed() >= Duration::from_secs(60))
        {
            self.last = Some(Instant::now());
            let (tx, rx) = mpsc::channel();
            let rows = profiles(registered);
            let data = data.to_path_buf();
            let cancel = Arc::new(AtomicBool::new(false));
            self.cancel = Some(cancel.clone());
            if std::thread::Builder::new()
                .name("quota-native".into())
                .spawn(move || {
                    let mut values = samples();
                    std::thread::scope(|scope| {
                        let workers = rows
                            .iter()
                            .map(|row| {
                                let data = &data;
                                let cancel = cancel.clone();
                                scope.spawn(move || sources::read(row, data, now(), cancel))
                            })
                            .collect::<Vec<_>>();
                        values.extend(workers.into_iter().filter_map(|worker| worker.join().ok()));
                    });
                    let _ = tx.send(values);
                })
                .is_ok()
            {
                self.rx = Some(rx);
            }
        }
        let mut out = Vec::new();
        let at = now();
        for reading in &mut self.readings {
            if !reading.metrics.is_empty() {
                reading.metrics.retain(|m| m.token_quota.is_none() || (m.reset_at.is_none_or(|reset| reset > at) && m.expires_at.is_none_or(|expires| expires > at)));
                reading.pick_windows(at);
            }
            let warned = self
                .announced
                .entry(reading.id.clone())
                .or_insert([false; 2]);
            let changed = threshold_edges(
                warned,
                [reading.five_hour, reading.weekly],
                [settings.five_hour, settings.weekly],
                can_show,
            );
            for (index, currency) in RelayUnit::ALL.into_iter().enumerate() {
                let Some(balance) = reading.relay_balance(at, currency) else { continue; };
                let warned = &mut self.relay_announced.entry(reading.id.clone()).or_default()[index];
                if relay_warning_edge(warned, Some(balance.amount), can_show && settings.relay_unit == currency) {
                    out.push(crate::capsule_alert::Alert::relay_balance(&reading.id, &reading.name, balance));
                }
            }
            if changed {
                out.push(crate::capsule_alert::Alert::quota(
                    &reading.id,
                    &reading.name,
                    [reading.five_hour, reading.weekly],
                    [settings.five_hour, settings.weekly],
                ));
            }
        }
        out
    }
}
pub fn profiles(v: &Value) -> Vec<sources::Profile> {
    let mut seen = std::collections::HashSet::new();
    v.as_array()
        .into_iter()
        .flatten()
        .take(16)
        .filter_map(|v| serde_json::from_value::<sources::Profile>(v.clone()).ok())
        .filter(|r| r.valid() && seen.insert(r.id.clone()))
        .collect()
}
fn relay_warning_edge(warned: &mut bool, balance: Option<f64>, can_show: bool) -> bool {
    let Some(amount) = balance.filter(|n| n.is_finite()) else { return false; };
    let low = amount < RELAY_WARNING;
    let trigger = low && !*warned && can_show;
    if !low || can_show { *warned = low; }
    trigger
}
fn threshold_edges(
    warned: &mut [bool; 2],
    remaining: [Option<f64>; 2],
    thresholds: [u8; 2],
    show: bool,
) -> bool {
    let mut changed = false;
    for i in 0..2 {
        if let Some(value) = remaining[i] {
            let low = value < f64::from(thresholds[i]);
            changed |= low && !warned[i] && show;
            if !low || show {
                warned[i] = low;
            }
        }
    }
    changed
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn relay_unit_defaults_to_usd_and_formats_the_selected_unit() {
        assert_eq!(Settings::from_value(&json!({"enabled":true,"weekly":12})).relay_unit, RelayUnit::Usd);
        let cny = Settings::from_value(&json!({"relayCurrency":"CNY"}));
        assert_eq!(cny.relay_unit, RelayUnit::Cny);
        assert_eq!(json!(cny)["relayCurrency"], "CNY");
        assert!(serde_json::from_value::<Settings>(json!({"relayCurrency":"EUR"})).is_err());
        let balance = |amount, currency| RelayBalance { amount, currency, key: false };
        assert_eq!(balance(0.72, RelayUnit::Usd).amount_label(), "US$0.72");
        assert_eq!(balance(0.68, RelayUnit::Cny).amount_label(), "¥0.68");
        assert_eq!(balance(0.999, RelayUnit::Cny).amount_label(), "< ¥1.00");
        assert_eq!(balance(0.77, RelayUnit::Points).amount_label(), "0.77 积分");
        assert_eq!(Settings::from_value(&json!({"relayCurrency":"POINTS"})).relay_unit, RelayUnit::Points);
        assert!(RelayUnit::from_unit("TOKEN").is_none());
        assert!(RelayUnit::from_unit("credits").is_none());
    }
    #[test]
    fn relay_warning_edges_ignore_missing_and_rearm_after_recharge() {
        let mut warned = false;
        for amount in [None, Some(f64::NAN), Some(1.), Some(1.23)] {
            assert!(!relay_warning_edge(&mut warned, amount, true));
        }
        assert!(!relay_warning_edge(&mut warned, Some(0.999), false));
        assert!(relay_warning_edge(&mut warned, Some(0.999), true));
        for amount in [Some(0.), None, Some(-0.01)] {
            assert!(!relay_warning_edge(&mut warned, amount, true));
            assert!(warned);
        }
        assert!(!relay_warning_edge(&mut warned, Some(1.), false));
        assert!(relay_warning_edge(&mut warned, Some(0.47), true));
    }
    #[test]
    fn window_duration_controls_classification_and_expired_is_not_zero() {
        let event = json!({"timestamp":1200,"type":"event_msg","payload":{"type":"token_count","rate_limits":{
            "primary":{"window_minutes":10080,"used_percent":92,"resets_at":2000},
            "secondary":{"window_minutes":300,"used_percent":99,"resets_at":1201}}}});
        let r = codex_record(&event, 1500).unwrap();
        assert_eq!(r.weekly, Some(8.));
        assert_eq!(r.five_hour, None);
    }
    #[test]
    fn strictly_below_and_only_one_per_crossing() {
        let mut warned = [false; 2];
        let limits = [20, 10];
        assert!(!threshold_edges(
            &mut warned,
            [Some(20.), Some(10.)],
            limits,
            true
        ));
        assert!(threshold_edges(
            &mut warned,
            [Some(19.), Some(10.)],
            limits,
            true
        ));
        assert!(!threshold_edges(&mut warned, [None, None], limits, true));
        assert!(!threshold_edges(
            &mut warned,
            [Some(19.), Some(10.)],
            limits,
            true
        ));
        assert!(threshold_edges(
            &mut warned,
            [Some(19.), Some(9.)],
            limits,
            true
        ));
    }
}
