// SPDX-License-Identifier: Apache-2.0
//! Port of BrimDeck's KimiUsage, CodeAgentUsage, GrokBotUsage, UsageService,
//! Cursor support from BrimDeck's DesktopSources, LogReader, UsageService and ChromiumSecrets.
//! Uses only current-user local data.
use super::{
    account, metric, num, quota, read_limit, text, time, unprotect, valid_token, zcode, Metric,
    Value,
};
use base64::{engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD}, Engine};
use chrono::{FixedOffset, NaiveDateTime};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

#[path = "ai_quota_desktop_credits.rs"]
mod credits;

type ResultSet = Result<(Vec<Metric>, String), String>;
type Get<'a> = dyn Fn(&str, &[(&str, &str)]) -> Result<Value, String> + 'a;
type Post<'a> = dyn Fn(&str, &[u8], &[(&str, &str)]) -> Result<Value, String> + 'a;
const KIMI_CN: &str = "https://api.kimi.com/coding/v1";
const KIMI_GLOBAL: &str = "https://api.kimi.ai/coding/v1";
const GROK: &str = "https://api2.cursor.sh/aiserver.v1.DashboardService/GetSandUsageStatus";
const CAC: &str =
    "https://snapengine.cida.cce.prod-szv-g.dragon.tools.huawei.com/api/v1/quota/usage";

fn app_home(variable: &str, fallback: &str) -> Option<PathBuf> {
    std::env::var_os(variable)
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            Some(
                PathBuf::from(std::env::var_os("USERPROFILE").filter(|v| !v.is_empty())?)
                    .join(fallback),
            )
        })
}
fn json_file(path: &Path) -> Option<Value> {
    serde_json::from_slice(&read_limit(path, 256 * 1024)?).ok()
}
fn file_text(path: &Path, size: u64) -> Option<String> {
    String::from_utf8(read_limit(path, size)?).ok()
}
fn redact(items: &mut [Metric], plan: &mut String, token: &str) {
    *plan = plan.replace(token, "••••");
    for item in items {
        item.label = item.label.replace(token, "••••");
        item.value = item.value.replace(token, "••••");
    }
}

pub(super) fn read(kind: &str, now: i64, get: &Get<'_>, post: &Post<'_>) -> Option<ResultSet> {
    Some(match kind {
        "kimi" => kimi(now, get),
        "codeagent" => code_agent(now, get),
        "cursor" => std::env::var_os("APPDATA").filter(|s| !s.is_empty()).map(PathBuf::from)
            .ok_or_else(|| "未找到本机额度登录".to_owned())
            .and_then(|p| cursor(&p.join("Cursor"), get, post)),
        "grokbot" => std::env::var_os("APPDATA")
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .ok_or_else(|| "未找到本机额度登录".to_owned())
            .and_then(|p| grok(&p.join("Grok Bot"), now, post)),
        _ => return credits::read(kind, now, get, post),
    })
}

// Only these two managed TOML tables are read. Unsupported shapes fail closed with
// a fixed issuer/default, never with a URL supplied by the local file.
fn kimi_config(config: &str) -> (Option<String>, Option<String>, Option<String>) {
    let mut table = 0;
    let (mut url, mut oauth, mut host) = (None, None, None);
    for row in config.trim_start_matches('\u{feff}').lines() {
        let line = row.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            let normalized = line.replace([' ', '\t', '"', '\''], "");
            table = match normalized.as_str() {
                "[providers.managed:kimi-code]" => 1,
                "[providers.managed:kimi-code.oauth]" => 2,
                _ => 0,
            };
        } else if let Some((k, raw)) = line.split_once('=') {
            let raw = raw.trim();
            let value = raw
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| raw.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')));
            let Some(value) = value.filter(|v| !v.is_empty()) else {
                continue;
            };
            match (table, k.trim()) {
                (1, "base_url") => url = Some(value.into()),
                (2, "key") => oauth = Some(value.into()),
                (2, "oauth_host" | "oauthHost") => host = Some(value.into()),
                _ => {}
            }
        }
    }
    (url, oauth, host)
}
fn kimi_pair(base: &str, oauth: &str) -> Option<(&'static str, &'static str)> {
    let base = match base.trim().trim_end_matches('/') {
        KIMI_CN => KIMI_CN,
        KIMI_GLOBAL => KIMI_GLOBAL,
        _ => return None,
    };
    let oauth = match oauth.trim().trim_end_matches('/') {
        "https://auth.kimi.com" => "https://auth.kimi.com",
        "https://auth.kimi.ai" => "https://auth.kimi.ai",
        _ => return None,
    };
    Some((base, oauth))
}
fn kimi_filename(base: &str, host: &str, configured: Option<&str>) -> Option<String> {
    if let Some(value) = configured {
        let name = value.strip_prefix("oauth/").unwrap_or(value);
        if name.is_empty()
            || name.len() > 120
            || name.starts_with('.')
            || !name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return None;
        }
        return Some(name.into());
    }
    if base == KIMI_CN && host == "https://auth.kimi.com" {
        return Some("kimi-code".into());
    }
    let key = format!(r#"{{"oauthHost":"{host}","baseUrl":"{base}"}}"#);
    let hash = Sha256::digest(key.as_bytes());
    Some(format!(
        "kimi-code-env-{}",
        hash[..8]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    ))
}
fn kimi(now: i64, get: &Get<'_>) -> ResultSet {
    let home = app_home("KIMI_CODE_HOME", ".kimi-code").ok_or("未找到本机额度登录")?;
    let (config_base, key, config_host) =
        kimi_config(&file_text(&home.join("config.toml"), 64 * 1024).unwrap_or_default());
    let env = |k| {
        std::env::var(k)
            .ok()
            .map(|s| s.trim().to_owned())
            .filter(|s| !s.is_empty())
    };
    let env_base = env("KIMI_CODE_BASE_URL");
    let env_host = env("KIMI_CODE_OAUTH_HOST").or_else(|| env("KIMI_OAUTH_HOST"));
    let host = env_host.as_deref().or(config_host.as_deref());
    let marker_allowed = env("KIMI_CODE_REGION_MARKER").as_deref() != Some("off");
    let global = host.is_some_and(|h| h.trim_end_matches('/') == "https://auth.kimi.ai")
        || (host.is_none()
            && !matches!(key.as_deref(), Some("oauth/kimi-code" | "kimi-code"))
            && marker_allowed
            && file_text(&home.join("region"), 32).is_some_and(|s| s.trim() == "global"));
    let base = env_base
        .as_deref()
        .or(config_base.as_deref())
        .unwrap_or(if global { KIMI_GLOBAL } else { KIMI_CN });
    let (base, host) =
        kimi_pair(base, host.unwrap_or("https://auth.kimi.com")).ok_or("非官方额度网关")?;
    let configured = if env_base.is_none() && env_host.is_none() {
        key.as_deref()
    } else {
        None
    };
    let name = kimi_filename(base, host, configured).ok_or("未找到本机额度登录")?;
    let val = json_file(&home.join("credentials").join(format!("{name}.json")))
        .ok_or("未找到本机额度登录")?;
    let token = val["access_token"]
        .as_str()
        .map(str::trim)
        .filter(|s| valid_token(s))
        .ok_or("未找到本机额度登录")?;
    if time(&val["expires_at"]).is_some_and(|t| t <= now) {
        return Err("登录已失效".into());
    }
    let auth = format!("Bearer {token}");
    let headers = [("Authorization", auth.as_str())];
    let doc = get(&format!("{base}/usages"), &headers)?;
    let mut metrics = account::kimi_usage(&doc, if base == KIMI_GLOBAL { "USD" } else { "CNY" });
    let mut plan = if metrics.is_empty() {
        String::new()
    } else {
        let me = get(&format!("{base}/me"), &headers).unwrap_or_default();
        text(&me["user_level_name"]).chars().take(40).collect()
    };
    redact(&mut metrics, &mut plan, token);
    Ok((metrics, plan))
}

fn code_agent(now: i64, get: &Get<'_>) -> ResultSet {
    let home = app_home("CODEAGENT3_CONFIG_DIR", ".cac").ok_or("未找到本机额度登录")?;
    let root = json_file(&home.join(".credentials.json")).ok_or("未找到本机额度登录")?;
    let record = &root["idaasOAuth"];
    if time(&record["xAuthExpires"]).is_some_and(|t| t <= now) {
        return Err("登录已失效".into());
    }
    let mut blob = record["xAuthToken"]
        .as_str()
        .and_then(|s| STANDARD.decode(s).ok())
        .ok_or("未找到本机额度登录")?;
    let token = unprotect(&mut blob, |clear| {
        std::str::from_utf8(clear)
            .ok()
            .map(str::trim)
            .filter(|s| valid_token(s))
            .map(str::to_owned)
    })
    .ok_or("未找到本机额度登录")?;
    let doc = get(CAC, &[("X-Auth-Token", &token)])?;
    let mut out = code_agent_metrics(&doc);
    let mut plan = String::new();
    redact(&mut out, &mut plan, &token);
    Ok((out, plan))
}
fn code_agent_metrics(doc: &Value) -> Vec<Metric> {
    let mut out = Vec::new();
    for (field, prefix) in [
        ("quotas", ""),
        ("external_quotas", "外部模型 · "),
        ("internal_quotas", "内部模型 · "),
    ] {
        for row in doc[field].as_array().into_iter().flatten().take(24) {
            let Some(total) = num(&row["total_quota"]).filter(|n| *n > 0.) else {
                continue;
            };
            let Some(used) = num(&row["used_quota"]) else {
                continue;
            };
            let (name, window) = match row["type"].as_str() {
                Some("CPM") => ("每月额度", None),
                Some("CPD_Peak") => ("忙时每日额度", Some(1440)),
                Some("CPD_Valley") => ("闲时每日额度", Some(1440)),
                _ => continue,
            };
            let mut item = metric(
                &format!("{prefix}{name}"),
                format!("{:.2} / {total:.2} 积分", (total - used.max(0.)).max(0.)),
            );
            item.window = window;
            item.reset_at = row["reset_at"].as_str().and_then(|s| {
                let wall = NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S").ok()?;
                Some(
                    wall.and_local_timezone(FixedOffset::east_opt(8 * 3600)?)
                        .single()?
                        .timestamp(),
                )
            });
            out.push(item);
        }
    }
    out
}

fn cursor_token(folder: &Path) -> Option<String> {
    let raw = lyric_backend::storage::readonly_state_text(
        &folder.join("User/globalStorage/state.vscdb"),
        c"SELECT CASE WHEN length(value)<=16384 THEN CAST(value AS TEXT) END FROM ItemTable WHERE key='cursorAuth/accessToken' LIMIT 1",
    )?;
    if let Some(cipher) = raw.strip_prefix("cursor-safe-storage:v1:") {
        chromium_folder_token(folder, cipher)
    } else {
        let raw = raw.trim();
        valid_token(raw).then(|| raw.to_owned())
    }
}
fn cursor_identity(token: &str) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let data = URL_SAFE_NO_PAD.decode(payload).or_else(|_| URL_SAFE.decode(payload)).ok()?;
    let parsed: Value = serde_json::from_slice(&data).ok()?;
    let subject = parsed["sub"].as_str()?;
    (!subject.is_empty() && subject.len() <= 160 && subject.bytes().all(|c| c.is_ascii_alphanumeric() || b"._-|:@".contains(&c)))
        .then(|| subject.to_owned())
}
fn cursor_metrics(doc: &Value, unified: bool) -> Vec<Metric> {
    let plan = if doc["planUsage"].is_object() { &doc["planUsage"] } else { &doc["individualUsage"]["plan"] };
    let reset = time(&doc["billingCycleEnd"]);
    if unified {
        let weekly = time(&doc["billingCycleStart"]).zip(reset).is_some_and(|(begin,end)| end-begin == 604800);
        return num(&plan["totalPercentUsed"]).map(|used|
            quota(if weekly {"每周额度"} else {"账期额度"}, weekly.then_some(10080), used, reset)).into_iter().collect();
    }
    let mut result: Vec<_> = [("autoPercentUsed","Cursor 模型"),("apiPercentUsed","其他模型")]
        .into_iter().filter_map(|(key,label)| Some(quota(label,None,num(&plan[key])?,reset))).collect();
    if result.is_empty() {
        result.extend(num(&plan["totalPercentUsed"]).map(|used| quota("账期额度",None,used,reset)));
    }
    result
}
fn cursor(folder: &Path, get: &Get<'_>, post: &Post<'_>) -> ResultSet {
    let token = cursor_token(folder).ok_or("未找到本机额度登录")?;
    let auth = format!("Bearer {token}");
    let headers = [("Authorization",auth.as_str())];
    let sand = post(GROK,b"{}",&headers).unwrap_or_default();
    let unified = sand["isUnified"] == true;
    let period = post("https://api2.cursor.sh/aiserver.v1.DashboardService/GetCurrentPeriodUsage",b"{}",&headers);
    let mut metrics = period.as_ref().map(|doc| cursor_metrics(doc,unified)).unwrap_or_default();
    let mut plan = if unified {text(&sand["cursorPlanName"])} else {period.as_ref().map(|d| text(&d["membershipType"])).unwrap_or_default()};
    let sand_quota = num(&sand["usagePercent"]).map(|used| {
        quota(if unified {"每周额度"} else {"Grok Bot"}, unified.then_some(10080),used,time(&sand["nextResetTimestampUtc"]))
    });
    if unified && metrics.is_empty() { metrics.extend(sand_quota.clone()); }
    if metrics.is_empty() || plan.is_empty() {
        if let Some(subject) = cursor_identity(&token) {
            let cookie = format!("WorkosCursorSessionToken={}",lyric_backend::http::encode(&format!("{subject}::{token}")));
            if let Ok(doc) = get("https://cursor.com/api/usage-summary",&[("Cookie",&cookie)]) {
                if metrics.is_empty() && !unified { metrics = cursor_metrics(&doc,false); }
                if plan.is_empty() { plan = text(&doc["membershipType"]); }
            }
        }
    }
    if !unified { metrics.extend(sand_quota); }
    if metrics.is_empty() { if let Err(err) = period { return Err(err); } }
    redact(&mut metrics,&mut plan,&token);
    Ok((metrics,plan))
}

fn grok_token(folder: &Path) -> Result<Option<String>, ()> {
    if !folder.join("sand-secrets.json").is_file() {
        return Ok(None);
    }
    let file = json_file(&folder.join("sand-secrets.json")).ok_or(())?;
    let Some(raw) = file["cursor-accounts"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let data: Value = serde_json::from_str(raw).map_err(|_| ())?;
    let Some(active) = data["active"].as_str().filter(|s| !s.is_empty()) else {
        return Ok(None);
    };
    let Some(cipher) = data["accounts"][active]["cursor-access-token"]
        .as_str()
        .filter(|s| !s.is_empty())
    else {
        return Ok(None);
    };
    chromium_folder_token(folder, cipher).map(Some).ok_or(())
}
fn chromium_folder_token(folder: &Path, cipher: &str) -> Option<String> {
    let mut plain = chromium_folder_plain(folder, STANDARD.decode(cipher).ok()?)?;
    let token = std::str::from_utf8(&plain)
        .ok()
        .map(str::trim)
        .filter(|s| valid_token(s))
        .map(str::to_owned);
    plain.fill(0);
    token
}
fn chromium_folder_plain(folder: &Path, mut data: Vec<u8>) -> Option<Vec<u8>> {
    if data.len() < 32 || !data.starts_with(b"v10") {
        return None;
    }
    let local = json_file(&folder.join("Local State"))?;
    let mut wrapped = STANDARD.decode(local["os_crypt"]["encrypted_key"].as_str()?).ok()?;
    if !wrapped.starts_with(b"DPAPI") { return None; }
    let mut key: [u8; 32] = unprotect(&mut wrapped[5..], |plain| plain.try_into().ok())?;
    let tag_at = data.len() - 16;
    let (msg, tag) = data.split_at_mut(tag_at);
    let (nonce, msg) = msg[3..].split_at_mut(12);
    let result = zcode::decrypt(&key, nonce, tag, msg);
    key.fill(0);
    result
}
fn grok(folder: &Path, now: i64, post: &Post<'_>) -> ResultSet {
    let token = grok_token(folder);
    let result = match &token {
        Ok(Some(token)) => {
            let auth = format!("Bearer {token}");
            post(GROK, b"{}", &[("Authorization", &auth)]).map(|doc| {
                let (mut items, mut plan) =
                    grok_metrics(&doc, "usagePercent", "nextResetTimestampUtc");
                redact(&mut items, &mut plan, token);
                (items, plan)
            })
        }
        Ok(None) => return Err("未找到本机额度登录".into()),
        Err(()) => Err("读取失败".into()),
    };
    // Rejected credentials must not display another account's cache. For a
    // transient error only use a current cache from this application's folder.
    if result
        .as_ref()
        .err()
        .is_some_and(|e| e != "HTTP 401" && e != "HTTP 403")
    {
        if let Some(cache) = grok_cache(folder, now) {
            return Ok(cache);
        }
    }
    result
}
fn grok_metrics(doc: &Value, count: &str, next: &str) -> (Vec<Metric>, String) {
    let plan = text(&doc["grokPlanLabel"]);
    let items = if doc["usesPooledEnterpriseAllowance"] == true {
        vec![metric("企业共享额度", "未提供独立额度".into())]
    } else {
        num(&doc[count])
            .map(|used| quota("每周额度", Some(10080), used, time(&doc[next])))
            .into_iter()
            .collect()
    };
    (items, plan)
}
fn grok_cache(folder: &Path, now: i64) -> Option<(Vec<Metric>, String)> {
    std::fs::read_dir(folder.join("sand-client-persistence"))
        .ok()?
        .take(1024)
        .filter_map(|e| {
            let e = e.ok()?;
            if e.file_type().ok()?.is_symlink() {
                return None;
            }
            let file = e.path();
            if file.extension()?.to_str()? != "blob" {
                return None;
            }
            let name = slice_name(file.file_stem()?.to_str()?)?;
            if !name.starts_with("sand.client.slice.account.")
                || !name.ends_with(".weekly-usage.cache")
            {
                return None;
            }
            let doc = json_file(&file)?;
            if doc["value"]["kind"] != "present" {
                return None;
            }
            let reading = &doc["value"]["reading"];
            let stamp = time(&reading["readAtMs"]).or_else(|| {
                Some(
                    e.metadata()
                        .ok()?
                        .modified()
                        .ok()?
                        .duration_since(UNIX_EPOCH)
                        .ok()?
                        .as_secs() as i64,
                )
            })?;
            if !(now - 3600..=now + 300).contains(&stamp) {
                return None;
            }
            let (mut metrics, plan) = grok_metrics(&reading["usage"], "percentUsed", "nextResetMs");
            if metrics.is_empty() {
                return None;
            }
            for item in &mut metrics {
                item.expires_at = Some(stamp + 3600);
            }
            Some((stamp, metrics, plan))
        })
        .max_by_key(|r| r.0)
        .map(|(_, metrics, plan)| (metrics, plan))
}
fn slice_name(encoded: &str) -> Option<String> {
    if encoded.len() > 512 {
        return None;
    }
    let mut out = Vec::new();
    let mut buffer = 0u16;
    let mut bits = 0;
    for ch in encoded.bytes() {
        let n = match ch {
            b'a'..=b'z' => ch - b'a',
            b'2'..=b'7' => ch - b'2' + 26,
            _ => return None,
        };
        buffer = (buffer << 5) | u16::from(n);
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8);
            buffer &= (1 << bits) - 1;
        }
    }
    (buffer == 0).then(|| String::from_utf8(out).ok()).flatten()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn quota_desktop_kimi_configuration_pins_issuer_and_path() {
        let (base, key, oauth) = kimi_config("[providers.\"managed:kimi-code\"]\nbase_url = 'https://api.kimi.ai/coding/v1/'\n[providers.\"managed:kimi-code\".oauth]\nkey='oauth/kimi-code-global'\noauth_host='https://auth.kimi.ai'\n[elsewhere]\nkey='bad'");
        assert_eq!(
            kimi_pair(&base.unwrap(), &oauth.unwrap()),
            Some((KIMI_GLOBAL, "https://auth.kimi.ai"))
        );
        assert_eq!(
            kimi_filename(KIMI_GLOBAL, "https://auth.kimi.ai", key.as_deref()).as_deref(),
            Some("kimi-code-global")
        );
        assert!(kimi_pair(
            "https://api.kimi.ai.example.com/coding/v1",
            "https://auth.kimi.ai"
        )
        .is_none());
        assert!(kimi_filename(KIMI_CN, "https://auth.kimi.com", Some("oauth/../other")).is_none());
        assert_eq!(
            kimi_filename(KIMI_CN, "https://auth.kimi.com", None).as_deref(),
            Some("kimi-code")
        );
    }
    #[test]
    fn cursor_keeps_billing_cycles_distinct_from_weekly_and_never_invents_5h() {
        let body = serde_json::json!({"planUsage":{"totalPercentUsed":65.,"autoPercentUsed":10.,"apiPercentUsed":50.},
            "billingCycleStart":"2026-11-01T00:00:00Z","billingCycleEnd":"2026-11-08T00:00:00Z"});
        let unified = cursor_metrics(&body,true);
        assert_eq!(unified.len(),1);
        assert_eq!((unified[0].window,unified[0].remaining),(Some(10080),Some(35.)));
        let classic = cursor_metrics(&body,false);
        assert_eq!(classic.len(),2);
        assert!(classic.iter().all(|m|m.window.is_none()));
        let fake = URL_SAFE_NO_PAD.encode(br#"{"sub":"auth0|fixture"}"#);
        assert_eq!(cursor_identity(&format!("header.{fake}.sig")).as_deref(),Some("auth0|fixture"));
        assert!(cursor_identity("abc.invalid.signature").is_none());
    }
    #[test]
    fn quota_desktop_grok_is_weekly_only_and_pooled_is_not_zero() {
        let (personal, _) = grok_metrics(
            &json!({"usagePercent":94}),
            "usagePercent",
            "nextResetTimestampUtc",
        );
        assert_eq!(personal[0].window, Some(10080));
        assert_eq!(personal[0].remaining, Some(6.));
        let (pooled, _) = grok_metrics(
            &json!({"usagePercent":100,"usesPooledEnterpriseAllowance":true}),
            "usagePercent",
            "nextResetTimestampUtc",
        );
        assert_eq!(pooled[0].remaining, None);
        assert_eq!(slice_name("mzxw6ytboi").as_deref(), Some("foobar"));
        assert!(slice_name("mzxw6ytboi=").is_none());
    }
    #[test]
    fn quota_desktop_codeagent_credits_use_cst_and_never_weekly_or_5h() {
        let out = code_agent_metrics(
            &json!({"quotas":[{"type":"CPD_Peak","used_quota":2,"total_quota":5,"reset_at":"2026-11-29 00:00:00"},{"type":"CPM","total_quota":10}]}),
        );
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].window, Some(1440));
        assert_eq!(out[0].value, "3.00 / 5.00 积分");
        assert_eq!(out[0].reset_at, time(&json!("2026-11-28T16:00:00Z")));
        assert_eq!(out[0].remaining, None);
    }
}
