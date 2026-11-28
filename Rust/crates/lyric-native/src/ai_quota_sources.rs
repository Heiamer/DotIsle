// SPDX-License-Identifier: Apache-2.0
//! Native quota-source translations of BrimDeck ProviderScripts.cs and DshUsage.cs.
//! Modified for DotIsle: Rust, DPAPI, no redirect, no request or secret in settings IPC.
use super::{Metric, Reading, RelayBalance, RelayUnit};
use lyric_backend::http::{quota_get, quota_post, RequestContext};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

#[path = "ai_quota_zcode.rs"]
mod zcode;

#[path = "ai_quota_account.rs"]
mod account;

#[path = "ai_quota_desktop.rs"]
mod desktop;

#[path = "ai_quota_ark.rs"]
mod ark;

#[path = "ai_quota_browser.rs"]
mod browser;

#[path = "ai_quota_custom.rs"]
mod custom;

struct SourceType {
    id: &'static str,
    name: &'static str,
    implementation: &'static str,
}
const REGISTRY: &[SourceType] = &[
    ty("codex", "Codex", "local"),
    ty("claude", "Claude", "local"),
    ty("cursor", "Cursor", "profile"),
    ty("grokbot", "Grok Bot", "profile"),
    ty("zcode", "ZCode", "profile"),
    ty("dsh", "DeepSeek Harness", "profile"),
    ty("kimi", "Kimi Code", "profile"),
    ty("opencode", "OpenCode", "profile"),
    ty("codeagent", "CodeAgent", "profile"),
    ty("commandcode", "Command Code", "profile"),
    ty("workbuddy", "WorkBuddy", "browser"),
    ty("workbuddy-ai", "WorkBuddy AI", "browser"),
    ty("mimo", "Xiaomi MiMo 桌面订阅", "browser"),
    ty("mimo-linked", "Xiaomi MiMo 已关联套餐", "profile"),
    ty("qoder", "Qoder", "profile"),
    ty("qoder-cn", "Qoder CN", "profile"),
    ty("qwenwork-cn", "千问办公", "profile"),
    ty("qwenwork", "QwenWork", "profile"),
    ty("trae", "TRAE", "profile"),
    ty("trae-cn", "TRAE CN", "profile"),
    ty("qianwen", "千问", "browser"),
    ty("qwencloud", "QwenCloud", "browser"),
    ty("doubao", "豆包", "browser"),
    ty("dola", "Dola", "browser"),
    ty("glm-cn", "智谱 GLM", "key"),
    ty("glm-global", "Z.ai GLM", "key"),
    ty("mimo-api", "Xiaomi MiMo API", "key"),
    ty("mimo-token-plan", "Xiaomi MiMo Token Plan", "key"),
    ty("deepseek", "DeepSeek", "key"),
    ty("kimi-api", "Kimi Code API", "key"),
    ty("kimi-platform", "Kimi 开放平台", "key"),
    ty("opencode-go", "OpenCode Go", "key"),
    ty("commandcode-api", "Command Code API", "key"),
    ty("openrouter", "OpenRouter", "key"),
    ty("newapi", "New API 中转站", "relay"),
    ty("sub2api", "Sub2API 中转站", "relay"),
    ty("volcengine-ark", "火山方舟", "cloud-ak"),
    ty("byteplus-ark", "BytePlus ModelArk", "cloud-ak"),
    ty("custom", "自定义 JavaScript", "custom"),
];
const fn ty(id: &'static str, name: &'static str, implementation: &'static str) -> SourceType {
    SourceType {
        id,
        name,
        implementation,
    }
}
fn provider(id: &str) -> Option<&'static SourceType> {
    REGISTRY.iter().find(|row| row.id == id)
}
pub fn catalog() -> Value {
    Value::Array(
        REGISTRY
            .iter()
            .map(|row| serde_json::json!({"id":row.id,"name":row.name,"mode":row.implementation}))
            .collect(),
    )
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    pub id: String,
    pub provider: String,
    #[serde(default)]
    pub site: String,
    #[serde(default)]
    pub name: String,
}
impl Profile {
    pub fn valid(&self) -> bool {
        self.id.len() == 32
            && self
                .id
                .bytes()
                .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
            && self.name.chars().count() <= 50
            && !self.name.chars().any(char::is_control)
            && provider(&self.provider).is_some_and(|row| match row.implementation {
                "relay" => relay_base(&self.site).is_some(),
                "custom" => custom::accepts_site(&self.site),
                "profile" | "key" | "cloud-ak" | "browser" => self.site.is_empty(),
                _ => false,
            })
    }
}
fn relay_base(site: &str) -> Option<String> {
    if site.len() > 512
        || !site
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-_:/".contains(&b))
    {
        return None;
    }
    let (scheme, rest) = site.split_once("://")?;
    let mut path = rest.trim_end_matches('/').split('/');
    let authority = path.next()?;
    let host = if let Some((host, port)) = authority.split_once(':') {
        if port.parse::<u16>().ok()? == 0 {
            return None;
        }
        host
    } else {
        authority
    };
    if host.is_empty()
        || !host
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".-".contains(&b))
        || host.starts_with('.')
        || host.ends_with('.')
    {
        return None;
    }
    if scheme != "https"
        && !(scheme == "http" && (host.eq_ignore_ascii_case("localhost") || host == "127.0.0.1"))
    {
        return None;
    }
    let segments = path
        .take_while(|p| {
            !["console", "dashboard", "panel", "login", "pricing", "keys"]
                .iter()
                .any(|v| v.eq_ignore_ascii_case(p))
        })
        .collect::<Vec<_>>();
    if segments.iter().any(|s| matches!(*s, "" | "." | "..")) {
        return None;
    }
    let mut base = format!("{scheme}://{authority}");
    let tail = segments.join("/");
    let tail = if tail == "v1" {
        ""
    } else {
        tail.strip_suffix("/v1").unwrap_or(&tail)
    };
    if !tail.is_empty() {
        base.push('/');
        base.push_str(tail);
    }
    Some(base)
}
fn read_limit(path: &Path, limit: u64) -> Option<Vec<u8>> {
    let file = fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= limit).then_some(bytes)
}
#[derive(Deserialize)]
struct Credential {
    provider: String,
    site: String,
    token: String,
}
#[repr(C)]
struct Blob {
    size: u32,
    content: *mut u8,
}
#[link(name = "crypt32")]
extern "system" {
    fn CryptUnprotectData(
        input: *const Blob,
        desc: *mut *mut u16,
        entropy: *const Blob,
        reserved: *mut std::ffi::c_void,
        prompt: *mut std::ffi::c_void,
        flags: u32,
        output: *mut Blob,
    ) -> i32;
}
#[link(name = "kernel32")]
extern "system" {
    fn LocalFree(memory: *mut std::ffi::c_void) -> *mut std::ffi::c_void;
}
fn credential(data: &Path, row: &Profile) -> Option<String> {
    let mut encrypted = read_limit(
        &data.join("ai-quota").join(format!("{}.dpapi", row.id)),
        48 * 1024,
    )?;
    let opened = unprotect(&mut encrypted, |memory| serde_json::from_slice::<Credential>(memory).ok())?;
    (opened.provider == row.provider && opened.site == row.site && valid_token(&opened.token))
        .then_some(opened.token)
}
fn unprotect<T>(encrypted: &mut [u8], parse: impl FnOnce(&[u8]) -> Option<T>) -> Option<T> {
    if encrypted.is_empty() || encrypted.len() > 48 * 1024 { return None; }
    let input = Blob {
        size: encrypted.len() as u32,
        content: encrypted.as_mut_ptr(),
    };
    let mut output = Blob {
        size: 0,
        content: std::ptr::null_mut(),
    };
    let ok = unsafe {
        CryptUnprotectData(
            &input,
            std::ptr::null_mut(),
            std::ptr::null(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            1,
            &mut output,
        )
    };
    if ok == 0 || output.content.is_null() {
        return None;
    }
    unsafe {
        let memory = std::slice::from_raw_parts_mut(output.content, output.size as usize);
        let parsed = parse(memory);
        memory.fill(0);
        LocalFree(output.content.cast());
        parsed
    }
}
fn valid_token(token: &str) -> bool {
    !token.is_empty() && token.len() <= 8192 && token.bytes().all(|b| (0x21..=0x7e).contains(&b))
}
fn num(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str()?.parse::<f64>().ok())
        .filter(|v| v.is_finite() && v.abs() <= 1e18)
}
fn time(v: &Value) -> Option<i64> {
    super::stamp(v)
        .or_else(|| v.as_str()?.parse().ok())
        .filter(|n| *n > 0)
        .map(|n| if n >= 100_000_000_000 { n / 1000 } else { n })
}
fn text(v: &Value) -> String {
    v.as_str()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .take(72)
        .collect()
}
fn money(v: f64, currency: &str) -> String {
    match currency {
        "USD" => format!("${v:.2}"),
        "CNY" => format!("¥{v:.2}"),
        u if u.eq_ignore_ascii_case("TOKENS") => {
            if v.fract() == 0. { format!("{v:.0} Tokens") } else { format!("{v} Tokens") }
        }
        c => format!(
            "{v:.2} {}",
            c.chars()
                .filter(|ch| !ch.is_control())
                .take(10)
                .collect::<String>()
        ),
    }
}
fn quota(label: &str, minutes: Option<u64>, used: f64, reset_at: Option<i64>) -> Metric {
    let remaining = 100. - used.clamp(0., 100.);
    Metric {
        label: label.into(),
        value: format!("剩余 {remaining:.1}%"),
        reset_at,
        window: minutes,
        remaining: Some(remaining),
        ..Default::default()
    }
}
fn metric(label: &str, value: String) -> Metric {
    Metric {
        label: label.into(),
        value,
        ..Default::default()
    }
}
fn relay_balance_metric(label: &str, value: String, amount: f64, currency: Option<RelayUnit>, key: bool) -> Metric {
    Metric {
        relay_money: currency.filter(|_| amount.is_finite()).map(|currency| RelayBalance { amount, currency, key }),
        ..metric(label, value)
    }
}
fn deepseek(value: &Value) -> Vec<Metric> {
    value["balance_infos"]
        .as_array()
        .into_iter()
        .flatten()
        .take(6)
        .flat_map(|info| {
            let currency = match info["currency"].as_str() {
                Some(c @ ("USD" | "CNY")) => c,
                _ => return Vec::new(),
            };
            [
                ("账户余额", "total_balance"),
                ("赠送余额", "granted_balance"),
                ("充值余额", "topped_up_balance"),
            ]
            .into_iter()
            .filter_map(|(label, field)| {
                let amount = num(&info[field])?;
                let unit = (field == "total_balance").then(|| RelayUnit::from_unit(currency)).flatten();
                Some(relay_balance_metric(label, money(amount, currency), amount, unit, false))
            })
            .collect()
        })
        .collect()
}
fn dsh_summary(root: &Value) -> Vec<Metric> {
    if num(&root["code"]) != Some(0.) || num(&root["data"]["biz_code"]) != Some(0.) {
        return Vec::new();
    }
    let data = &root["data"]["biz_data"];
    let sum = |field: &str, amount: &str, currency: &str| {
        let mut total = None;
        for item in data[field]
            .as_array()
            .into_iter()
            .flatten()
            .take(256)
            .filter(|item| item["currency"] == currency)
        {
            total = Some(total.unwrap_or(0.) + num(&item[amount])?);
        }
        total
    };
    let mut values = Vec::new();
    for c in ["CNY", "USD"] {
        let paid = sum("normal_wallets", "balance", c);
        let bonus = sum("bonus_wallets", "balance", c);
        if paid.is_none() && bonus.is_none() {
            continue;
        }
        let amount = paid.unwrap_or(0.) + bonus.unwrap_or(0.);
        let unit = paid.and_then(|_| RelayUnit::from_unit(c));
        values.push(relay_balance_metric("账户余额", money(amount, c), amount, unit, false));
        if let Some(bonus) = bonus {
            values.push(metric("赠送余额", money(bonus, c)));
        }
        if let Some(cost) = sum("total_costs", "amount", c) {
            values.push(metric("累计消费", money(cost.max(0.), c)));
        }
    }
    values
}
fn yaml_scalar(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.starts_with('"') {
        return serde_json::from_str::<String>(raw).ok();
    }
    if let Some(quoted) = raw.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')) {
        return Some(quoted.replace("''", "'"));
    }
    let raw = raw.split(" #").next()?.trim();
    (!raw.is_empty() && !raw.starts_with(['{', '[', '&', '*', '!', '|', '>', '\'', '"']))
        .then(|| raw.into())
}
fn dsh_credentials(raw: &str) -> (Option<String>, Option<String>) {
    let mut stack = Vec::<(usize, String)>::new();
    let mut fields = std::collections::HashMap::new();
    for line in raw.trim_start_matches('\u{feff}').lines().take(4096) {
        let row = line.trim_start_matches(' ');
        let col = line.len() - row.len();
        if row.starts_with(['#', '\t', '-']) || col >= 64 {
            continue;
        }
        let Some((key, value)) = row.split_once(':') else {
            continue;
        };
        let Some(key) = yaml_scalar(key) else {
            continue;
        };
        if key.contains('\0') || stack.len() > 12 {
            continue;
        }
        while stack.last().is_some_and(|(n, _)| *n >= col) {
            stack.pop();
        }
        stack.push((col, key));
        if let Some(value) = yaml_scalar(value) {
            let path = stack
                .iter()
                .map(|(_, key)| key.as_str())
                .collect::<Vec<_>>()
                .join("\0");
            if fields.insert(path, value).is_some() {
                return (None, None);
            }
        }
    }
    if !fields.get("version").is_some_and(|v| v == "1") {
        return (None, None);
    }
    let prefix = "records\0deepseek-account-platform/default\0";
    let grant = if fields
        .get(&format!("{prefix}kind"))
        .is_some_and(|v| v == "grant")
        && fields
            .get(&format!("{prefix}payload\0issuer"))
            .is_some_and(|v| v.trim_end_matches('/') == "https://platform.deepseek.com")
    {
        fields
            .remove(&format!("{prefix}payload\0token"))
            .filter(|s| valid_token(s))
    } else {
        None
    };
    (
        grant,
        fields
            .remove("refs\0DEEPSEEK_API_KEY")
            .filter(|s| valid_token(s)),
    )
}
fn glm(root: &Value, now: i64) -> Vec<Metric> {
    if root["success"] == false || num(&root["code"]).is_some_and(|n| n != 0. && n != 200.) {
        return Vec::new();
    }
    root["data"]["limits"]
        .as_array()
        .into_iter()
        .flatten()
        .take(24)
        .filter_map(|r| {
            let reset = time(&r["nextResetTime"]);
            let kind = r["type"].as_str()?;
            if kind == "TIME_LIMIT" {
                let total = num(&r["usage"]).filter(|n| *n > 0.)?;
                return Some(Metric {
                    label: "工具调用".into(),
                    value: format!(
                        "{:.0} / {total:.0} 次",
                        num(&r["currentValue"]).unwrap_or(0.)
                    ),
                    reset_at: reset,
                    ..Default::default()
                });
            }
            if !matches!(kind, "TOKENS_LIMIT" | "CREDIT_LIMIT") {
                return None;
            }
            let n = r["number"].as_u64();
            let (label, window) = match r["unit"].as_u64() {
                Some(3) if n.is_some_and(|n| n > 0 && n <= 24) => {
                    (format!("{} 小时", n.unwrap()), Some(n.unwrap() * 60))
                }
                Some(6) => ("每周".into(), Some(10080)),
                _ => ("套餐额度".into(), None),
            };
            let used = if kind == "CREDIT_LIMIT" {
                num(&r["currentValue"])
                    .zip(num(&r["usage"]).filter(|n| *n > 0.))
                    .map(|(used, limit)| used / limit * 100.)
            } else {
                None
            };
            let used = used.or_else(|| num(&r["percentage"]))?;
            Some(quota(
                &label,
                window,
                used,
                reset.filter(|r| window.is_none_or(|m| *r <= now + m as i64 * 60)),
            ))
        })
        .collect()
}
fn sub2api(root: &Value) -> Vec<Metric> {
    if root["isValid"] != true {
        return Vec::new();
    }
    let mut out = Vec::new();
    let window = |label: &str, m, used: &Value, limit: &Value, reset| {
        Some(quota(
            label,
            Some(m),
            num(used)? / num(limit).filter(|n| *n > 0.)? * 100.,
            reset,
        ))
    };
    if root["mode"] == "quota_limited" {
        for r in root["rate_limits"]
            .as_array()
            .into_iter()
            .flatten()
            .take(16)
        {
            let (label, m) = match r["window"].as_str() {
                Some("5h") => ("5 小时", 300),
                Some("1d") => ("每日", 1440),
                Some("7d") => ("每周", 10080),
                _ => continue,
            };
            out.extend(window(
                label,
                m,
                &r["used"],
                &r["limit"],
                time(&r["reset_at"]),
            ));
        }
        if let Some(amount) = num(&root["quota"]["remaining"]) {
            let unit = root["quota"]["unit"].as_str();
            let currency = unit.and_then(RelayUnit::from_unit);
            out.push(relay_balance_metric("密钥余额", relay_display(amount, unit, currency), amount, currency, true));
        }
        let ends = time(&root["expires_at"]);
        for m in &mut out {
            m.expires_at = ends;
        }
    } else if root["mode"] == "unrestricted" {
        let s = &root["subscription"];
        if s.is_object() {
            for (key, label, m) in [
                ("daily", "每日", 1440),
                ("weekly", "每周", 10080),
                ("monthly", "每月", 43200),
            ] {
                let reset = (m == 10080)
                    .then(|| time(&s["weekly_window_start"]).map(|at| at + 604800))
                    .flatten();
                out.extend(window(
                    label,
                    m,
                    &s[format!("{key}_usage_usd")],
                    &s[format!("{key}_limit_usd")],
                    reset,
                ));
            }
            for m in &mut out {
                m.expires_at = time(&s["expires_at"]);
            }
        } else if let Some(amount) = num(&root["balance"]) {
            let unit = root["unit"].as_str();
            let currency = unit.and_then(RelayUnit::from_unit);
            out.push(relay_balance_metric("账户余额", relay_display(amount, unit, currency), amount, currency, false));
            if let Some(n) = num(&root["usage"]["today"]["actual_cost"]) {
                out.push(metric("今日扣费", relay_display(n, unit, currency)));
            }
        }
    }
    out
}
fn relay_display(amount: f64, unit: Option<&str>, currency: Option<RelayUnit>) -> String {
    let unit = currency.filter(|u| *u != RelayUnit::Points).map(RelayUnit::code).or(unit).filter(|unit| !unit.is_empty());
    unit.map_or_else(|| format!("{amount:.2}（单位未知）"), |unit| money(amount, unit))
}
fn new_api(body: &Value, config: &Value, billing: &Value, usage: &Value) -> Vec<Metric> {
    let data = &body["data"];
    if data["object"] != "token_usage" || body["success"] == false {
        return Vec::new();
    }
    let config = if config["success"] != false { &config["data"] } else { &Value::Null };
    let per = num(&config["quota_per_unit"]).filter(|n| *n > 0.);
    // Respect the relay's configured money denomination. CNY needs the
    // relay's own display factor; never assume 1:1 or fetch an FX rate.
    let (unit, factor) = match config["quota_display_type"].as_str() {
        Some("TOKENS") => ("TOKENS", Some(1.)),
        Some("CNY") => (
            "CNY",
            per.zip(num(&config["usd_exchange_rate"]).filter(|rate| *rate > 0.)).map(|(p, rate)| rate / p),
        ),
        Some("USD") => ("USD", per.map(|p| 1. / p)),
        Some("CUSTOM") => (
            config["custom_currency_symbol"].as_str().unwrap_or("¤"),
            per.map(|p| num(&config["custom_currency_exchange_rate"]).unwrap_or(1.) / p),
        ),
        _ => match config["display_in_currency"].as_bool() {
            Some(false) => ("TOKENS", Some(1.)),
            Some(true) => ("USD", per.map(|p| 1. / p)),
            _ => ("", None),
        },
    };
    // If the relay has not confirmed the units, show the raw quota instead of guessing money.
    let factor = factor.filter(|f| f.is_finite() && *f > 0.);
    // A CUSTOM symbol like "USD" or "CNY" is not currency. Explicit points are supported.
    let currency = factor.and(match config["quota_display_type"].as_str() {
        Some("USD") => Some(RelayUnit::Usd),
        Some("CNY") => Some(RelayUnit::Cny),
        Some("CUSTOM") if num(&config["custom_currency_exchange_rate"]).is_some()
            && RelayUnit::from_unit(unit) == Some(RelayUnit::Points) => Some(RelayUnit::Points),
        None if config["display_in_currency"] == true => Some(RelayUnit::Usd),
        _ => None,
    });
    let cost = |amount: f64| {
        factor.map_or_else(
            || format!("{amount} 原始额度"),
            |f| {
                if config["quota_display_type"] == "CUSTOM" {
                    let label: String = unit.chars().filter(|ch| !ch.is_control()).take(10).collect();
                    format!("{:.2} {label}", amount * f)
                } else { money(amount * f, unit) }
            },
        )
    };
    let mut out = Vec::new();
    if data["unlimited_quota"] == true {
        out.push(metric("密钥额度", "无上限".into()));
    } else if let Some(amount) = num(&data["total_available"]) {
        out.push(Metric {
            expires_at: time(&data["expires_at"]),
            ..relay_balance_metric("密钥余额", cost(amount), match (per, currency) {
                (Some(per), Some(RelayUnit::Usd)) => amount / per,
                (Some(per), Some(RelayUnit::Cny)) => amount / per * num(&config["usd_exchange_rate"]).unwrap_or_default(),
                (_, Some(RelayUnit::Points)) => amount * factor.unwrap_or_default(),
                _ => amount,
            }, currency, true)
        });
    }
    if let Some(amount) = num(&data["total_used"]) {
        out.push(metric("密钥累计消费", cost(amount)));
    }
    if currency == Some(RelayUnit::Usd) {
        if let (Some(limit), Some(used), Some(f)) = (
            num(&billing["hard_limit_usd"]),
            num(&usage["total_usage"]),
            factor,
        ) {
            let independent = if data["unlimited_quota"] == true {
                (limit - 100_000_000.).abs() > 0.5
            } else {
                num(&data["total_granted"]).is_some_and(|total| (limit - total * f).abs() > 0.01)
            };
            if independent {
                let amount = limit - used / 100.;
                out.insert(0, relay_balance_metric("账户余额", money(amount, "USD"), amount, Some(RelayUnit::Usd), false));
            }
        }
    }
    out
}
fn openrouter(key: &Value, credits: &Value) -> Vec<Metric> {
    let data = &key["data"];
    let mut out = Vec::new();
    if let (Some(total), Some(used)) = (
        num(&credits["data"]["total_credits"]),
        num(&credits["data"]["total_usage"]),
    ) {
        let amount = total - used;
        out.push(relay_balance_metric("账户余额", money(amount, "USD"), amount, Some(RelayUnit::Usd), false));
    }
    if let Some(n) =
        num(&data["effective_budget"]["remaining_usd"]).or_else(|| num(&data["limit_remaining"]))
    {
        out.push(Metric {
            reset_at: time(&data["effective_budget"]["resets_at"]),
            expires_at: time(&data["expires_at"]),
            ..metric("预算余额", money(n, "USD"))
        });
    }
    for (label, field) in [("今日扣费", "usage_daily"), ("本月扣费", "usage_monthly")] {
        if let Some(n) = num(&data[field]) {
            out.push(metric(label, money(n.max(0.), "USD")));
        }
    }
    out
}

pub fn read(row: &Profile, data: &Path, now: i64, cancel: Arc<AtomicBool>) -> Reading {
    let mut result = Reading::blank(
        &row.id,
        &row.provider,
        if row.name.is_empty() {
            provider(&row.provider).map_or("渠道", |t| t.name)
        } else {
            &row.name
        },
        now,
    );
    if !row.valid() {
        return result;
    }
    let mut context = RequestContext::new(Duration::from_secs(12));
    context.cancelled = cancel;
    let get = |url: &str, headers: &[(&str, &str)]| quota_get(url, headers, &context);
    let post = |url: &str, body: &[u8], headers: &[(&str, &str)]| quota_post(url, body, headers, &context);
    let work = || -> Result<(Vec<Metric>, String), String> {
        if let Some(reading) = desktop::read(&row.provider, now, &get, &post) {
            return reading;
        }
        if row.provider == "zcode" {
            return zcode::read(now, &get);
        }
        if row.provider == "mimo-linked" {
            return account::mimo_linked(now, &get);
        }
        if row.provider == "dsh" {
            let home = std::env::var_os("DSH_HOME")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .or_else(|| std::env::var_os("USERPROFILE").map(|p| PathBuf::from(p).join(".dsh")));
            let raw = home
                .and_then(|home| read_limit(&home.join(".credentials.yaml"), 128 * 1024))
                .ok_or("未连接")?;
            let text = std::str::from_utf8(&raw).map_err(|_| "无法读取额度")?;
            let (grant, key) = dsh_credentials(text);
            if let Some(grant) = grant {
                let doc = get(
                    "https://platform.deepseek.com/api/v0/users/get_user_summary",
                    &[("x-dsh-auth-token", &grant)],
                )?;
                if num(&doc["code"]) == Some(40003.) {
                    return Err("登录已失效".into());
                }
                return Ok((dsh_summary(&doc), String::new()));
            }
            let key = key.ok_or("未连接")?;
            let auth = format!("Bearer {key}");
            return Ok((
                deepseek(&get(
                    "https://api.deepseek.com/user/balance",
                    &[("Authorization", &auth)],
                )?),
                String::new(),
            ));
        }
        let token = if matches!(row.provider.as_str(), "commandcode" | "opencode") {
            account::local_key(&row.provider).ok_or("未找到本机额度登录")?
        } else {
            credential(data, row).ok_or("未配置凭据")?
        };
        if row.provider == "custom" {
            return custom::read(&row.site, &token, &get);
        }
        if matches!(row.provider.as_str(), "volcengine-ark" | "byteplus-ark") {
            return ark::read(&row.provider, &token, now, &post);
        }
        if matches!(row.provider.as_str(), "workbuddy" | "workbuddy-ai" | "qianwen" | "qwencloud" | "mimo" | "doubao" | "dola") {
            return browser::read(&row.provider, &token, now, &get, &post);
        }
        let auth = format!("Bearer {token}");
        let headers = [("Authorization", auth.as_str())];
        let (mut metrics, mut plan) = match row.provider.as_str() {
            "deepseek" => (
                deepseek(&get("https://api.deepseek.com/user/balance", &headers)?),
                String::new(),
            ),
            "glm-cn" | "glm-global" => {
                let host = if row.provider == "glm-cn" {
                    "https://open.bigmodel.cn"
                } else {
                    "https://api.z.ai"
                };
                let doc = get(
                    &format!("{host}/api/monitor/usage/quota/limit"),
                    &[("Authorization", &token)],
                )?;
                (glm(&doc, now), text(&doc["data"]["level"]))
            }
            "sub2api" => {
                let base = relay_base(&row.site).ok_or("站点地址无效")?;
                let doc = get(&format!("{base}/v1/usage"), &headers)?;
                if doc["isValid"] == false {
                    return Err("密钥无效".into());
                }
                (sub2api(&doc), text(&doc["planName"]))
            }
            "newapi" => {
                let base = relay_base(&row.site).ok_or("站点地址无效")?;
                let doc = get(&format!("{base}/api/usage/token/"), &headers).or_else(|err| {
                    if err == "HTTP 404" || err == "HTTP 301" || err == "HTTP 308" {
                        get(&format!("{base}/api/usage/token"), &headers)
                    } else {
                        Err(err)
                    }
                })?;
                let config = get(&format!("{base}/api/status"), &[]).unwrap_or_default();
                let bill = get(
                    &format!("{base}/v1/dashboard/billing/subscription"),
                    &headers,
                )
                .unwrap_or_default();
                let usage = get(&format!("{base}/v1/dashboard/billing/usage"), &headers)
                    .unwrap_or_default();
                (new_api(&doc, &config, &bill, &usage), String::new())
            }
            "openrouter" => {
                let key = get("https://openrouter.ai/api/v1/key", &headers)?;
                let credits =
                    get("https://openrouter.ai/api/v1/credits", &headers).unwrap_or_default();
                (openrouter(&key, &credits), String::new())
            }
            other => account::read(other, &token, now, &get).ok_or("等待原生转译")??,
        };
        // Provider-controlled strings are bounded and cannot accidentally mirror the supplied key.
        for item in &mut metrics {
            item.value = item.value.replace(&token, "••••");
        }
        plan = plan.replace(&token, "••••");
        Ok((metrics, plan))
    };
    match work() {
        Ok((m, p)) => {
            result.status = if m.is_empty() {
                "未返回可识别的额度"
            } else {
                "已连接"
            }
            .into();
            result.metrics = m;
            result.plan = p;
            result.pick_windows(now);
        }
        Err(e) => {
            result.status = match e.as_str() {
                "HTTP 401" | "HTTP 403" | "密钥无效" => "凭据无效或访问受限",
                "HTTP 429" => "请求频繁，稍后重试",
                "未连接" => "请先在 DeepSeek Harness 登录",
                "ZCode 未连接" => "请先在 ZCode 登录",
                "ZCode 无法读取凭据" => "ZCode 凭据不可用",
                "未配置凭据" => "未配置凭据",
                "未找到本机额度登录" => "未找到本机额度登录",
                "请选择 Xiaomi MiMo API" => "请选择 Xiaomi MiMo API",
                "请选择 Xiaomi MiMo Token Plan" => "请选择 Xiaomi MiMo Token Plan",
                "登录已失效" => "登录已失效",
                "本机登录已失效，请重新打开对应应用登录" => "本机登录已失效，请重新打开对应应用登录",
                "此 Qoder 账户的额度格式暂不支持" => "此 Qoder 账户的额度格式暂不支持",
                "暂不支持企业专属部署" => "暂不支持企业专属部署",
                "暂不支持 TRAE 团队或企业账户额度" => "暂不支持 TRAE 团队或企业账户额度",
                "没有可识别的额度数据" | "未返回可识别的额度" => "未返回可识别的额度",
                "官网授权已失效，请重新授权" => "官网授权已失效，请重新授权",
                "自定义脚本无效、超时或系统脚本引擎不可用" => "自定义脚本无效、超时或系统脚本引擎不可用",
                "方舟 AK/SK 格式错误" => "方舟 AK/SK 格式错误",
                "方舟接口拒绝请求" => "方舟接口拒绝请求，请检查云账号权限",
                _ => "读取失败",
            }
            .into()
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn relay_balance_preserves_raw_units_and_only_uses_confirmed_money() {
        use RelayUnit::{Cny, Usd};
        let balance = |metrics: Vec<Metric>, at, currency| {
            let mut r = Reading::blank("id", "fixture", "demo", 100);
            r.metrics = metrics;
            r.relay_balance(at, currency)
        };
        let expected = |amount, currency, key| Some(RelayBalance { amount, currency, key });
        for (unit, usd, cny) in [(json!("USD"), expected(0.44, Usd, false), None),
            (json!("CNY"), None, expected(0.44, Cny, false)),
            (json!("RMB"), None, expected(0.44, Cny, false)),
            (json!("积分"), None, None), (json!("JPY"), None, None), (Value::Null, None, None)] {
            let metrics = sub2api(&json!({"isValid":true,"mode":"unrestricted","balance":0.44,"unit":unit}));
            assert_eq!(balance(metrics.clone(), 101, Usd), usd);
            assert_eq!(balance(metrics, 101, Cny), cny);
        }
        let raw = sub2api(&json!({"isValid":true,"mode":"unrestricted","balance":0.44,"unit":"积分"}));
        assert_eq!(raw[0].value, "0.44 积分");
        assert_eq!(raw[0].relay_money.unwrap().currency, RelayUnit::Points);
        assert_eq!(raw[0].relay_money.unwrap().amount, 0.44);
        let tokens = sub2api(&json!({"isValid":true,"mode":"unrestricted","balance":0.44,"unit":"TOKENS"}));
        assert_eq!(tokens[0].value, "0.44 Tokens");
        assert_eq!(balance(tokens, 101, RelayUnit::Points), None);
        let unknown = sub2api(&json!({"isValid":true,"mode":"unrestricted","balance":0.44}));
        assert!(unknown[0].value.contains("单位未知"));
        let expired = sub2api(&json!({"isValid":true,"mode":"quota_limited","quota":{"remaining":0,"unit":"CNY"},"expires_at":102}));
        assert_eq!(balance(expired.clone(), 101, Cny), expected(0., Cny, true));
        assert_eq!(balance(expired, 102, Cny), None);
        let body = json!({"success":true,"data":{"object":"token_usage","total_available":50000,"total_granted":500000}});
        let config = |kind, rate| json!({"success":true,"data":{"quota_per_unit":500000,"quota_display_type":kind,"usd_exchange_rate":rate,"custom_currency_symbol":"CNY"}});
        let sample = |config| new_api(&body,&config,&Value::Null,&Value::Null);
        let cny = sample(config("CNY", json!(7)));
        assert!((balance(cny.clone(), 101, Cny).unwrap().amount - 0.7).abs() < 1e-12);
        assert_eq!(balance(cny, 101, Usd), None);
        let custom = sample(config("CUSTOM", Value::Null));
        assert_eq!(custom[0].value, "0.10 CNY", "a custom label is not reinterpreted as a currency");
        let mut points = config("CUSTOM", Value::Null);
        points["data"]["custom_currency_symbol"] = json!("积分");
        assert_eq!(balance(sample(points.clone()), 101, RelayUnit::Points), None);
        points["data"]["custom_currency_exchange_rate"] = json!(5.);
        assert_eq!(balance(sample(points), 101, RelayUnit::Points), expected(0.5, RelayUnit::Points, true));
        let exact = new_api(&body,&config("CNY", json!(10)),&Value::Null,&Value::Null);
        assert_eq!(balance(exact, 101, Cny), expected(1., Cny, true));
        let usd = sample(config("USD", Value::Null));
        assert!((balance(usd.clone(), 101, Usd).unwrap().amount - 0.1).abs() < 1e-12);
        assert_eq!(balance(usd, 401, Usd), None);
        for cfg in [config("CNY", Value::Null), config("TOKENS", json!(7)), config("CUSTOM", json!(7)), Value::Null] {
            let rows = sample(cfg);
            assert_eq!(balance(rows.clone(), 101, Usd), None);
            assert_eq!(balance(rows, 101, Cny), None);
        }
        assert_eq!(balance(openrouter(&json!({"data":{"limit_remaining":0}}), &Value::Null), 101, Usd), None);
        assert_eq!(balance(openrouter(&Value::Null, &json!({"data":{"total_credits":2,"total_usage":1.5}})), 101, Usd), expected(0.5, Usd, false));
        let dsh = dsh_summary(&json!({"code":0,"data":{"biz_code":0,"biz_data":{
            "normal_wallets":[{"currency":"USD","balance":"0.625"}],
            "bonus_wallets":[{"currency":"USD","balance":"0.375"},{"currency":"CNY","balance":"0.03"}]
        }}}));
        assert_eq!(balance(dsh.clone(), 101, Usd), expected(1., Usd, false));
        assert_eq!(balance(dsh, 101, Cny), None, "bonus-only balances must not trigger");
        let deepseek = deepseek(&json!({"balance_infos":[{"currency":"CNY","total_balance":"1.22","granted_balance":"0.23"}]}));
        assert_eq!(balance(deepseek, 101, Cny), expected(1.22, Cny, false));
    }
    #[test]
    fn quota_catalog_excludes_antigravity_and_usage_only_pi() {
        assert_eq!(REGISTRY.len(), 39);
        assert!(REGISTRY.iter().all(|row| row.implementation != "pending"));
        assert_eq!(
            REGISTRY
                .iter()
                .map(|s| s.id)
                .collect::<std::collections::HashSet<_>>()
                .len(),
            39
        );
        assert!(provider("antigravity").is_none() && provider("pi").is_none());
    }
    #[test]
    fn relay_pins_origin_and_never_allows_external_plain_http() {
        for site in [
            "https://safe.test@evil.test",
            "https://safe.test/%2e%2e",
            "https://safe.test/path/../v1",
            "http://safe.test",
            "https://safe.test?secret=abc",
        ] {
            assert!(relay_base(site).is_none(), "{site}");
        }
        assert_eq!(
            relay_base("https://relay.test/team/dashboard").as_deref(),
            Some("https://relay.test/team")
        );
        assert_eq!(
            relay_base("http://127.0.0.1:9000/v1").as_deref(),
            Some("http://127.0.0.1:9000")
        );
    }
    #[test]
    fn dsh_grant_is_restricted_to_its_real_issuer() {
        let doc="version: 1\nrefs:\n  DEEPSEEK_API_KEY: test-key\nrecords:\n  deepseek-account-platform/default:\n    kind: grant\n    payload:\n      token: 'grant-123'\n      issuer: https://platform.deepseek.com\n";
        assert_eq!(
            dsh_credentials(doc),
            (Some("grant-123".into()), Some("test-key".into()))
        );
        assert_eq!(
            dsh_credentials(&doc.replace("platform.deepseek.com", "example.org")).0,
            None
        );
    }
    #[test]
    fn native_parsers_keep_units_and_windows() {
        let d = deepseek(&json!({"balance_infos":[{"currency":"USD","total_balance":"8.42"}]}));
        assert_eq!(d[0].value, "$8.42");
        assert!(d[0].window.is_none());
        let s = sub2api(
            &json!({"mode":"quota_limited","isValid":true,"rate_limits":[{"window":"5h","used":9.0,"limit":10.0,"reset_at":777777},{"window":"1d","limit":10.0}]}),
        );
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].remaining, Some(10.));
        let n = new_api(
            &json!({"data":{"object":"token_usage","total_available":30}}),
            &Value::Null,
            &Value::Null,
            &Value::Null,
        );
        assert_eq!(n[0].value, "30 原始额度");
        assert!(n[0].remaining.is_none());
        let g = glm(
            &json!({"data":{"limits":[{"type":"TOKENS_LIMIT","unit":3,"number":5,"percentage":20},{"type":"TOKENS_LIMIT","unit":6,"percentage":95}]}}),
            0,
        );
        assert_eq!((g[0].window, g[1].window), (Some(300), Some(10080)));
    }
}
