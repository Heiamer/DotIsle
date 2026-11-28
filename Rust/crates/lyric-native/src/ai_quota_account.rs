// SPDX-License-Identifier: Apache-2.0
//! Port of BrimDeck's CommandCodeUsage, OpenCodeUsage, KimiUsage, MiMoUsage
//! and their ScriptProvider quota flows. Fixed HTTPS issuers, bounded local reads.
use super::{metric, money, num, quota, read_limit, relay_balance_metric, text, time, valid_token, Metric, Value};
use chrono::{DateTime, NaiveDateTime};
use std::path::{Path, PathBuf};

type ResultSet = Result<(Vec<Metric>, String), String>;
type Request<'a> = dyn Fn(&str, &[(&str, &str)]) -> Result<Value, String> + 'a;

const CODE: &str = "https://api.commandcode.ai";
const OPEN: &str = "https://opencode.ai/zen/go/v1/usage";
const MIMO: &str = "https://api.xiaomimimo.com";
const MIMO_PLANS: &[&str] = &[
    "https://token-plan-cn.xiaomimimo.com/v1/platform/token_plan",
    "https://token-plan-sgp.xiaomimimo.com/v1/platform/token_plan",
    "https://token-plan-ams.xiaomimimo.com/v1/platform/token_plan",
];

fn xdg_data() -> Option<PathBuf> {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            Some(
                PathBuf::from(std::env::var_os("USERPROFILE").filter(|s| !s.is_empty())?)
                    .join(".local/share"),
            )
        })
}

fn mimo_linked_keys(doc: &Value) -> Vec<(&str, Option<&'static str>)> {
    let variants = [
        "xiaomi-token-plan-cn",
        "xiaomi-token-plan-sgp",
        "xiaomi-token-plan-ams",
    ];
    variants
        .into_iter()
        .zip(MIMO_PLANS.iter().copied().map(Some))
        .chain([("xiaomi", None)])
        .filter_map(|(name, plan)| {
            let entry = &doc[name];
            let key = entry["key"].as_str()?.trim();
            let issuer = plan
                .and_then(|url| url.strip_suffix("/v1/platform/token_plan"))
                .unwrap_or(MIMO);
            let base = entry["metadata"]["base_url"]
                .as_str()?
                .trim_end_matches('/');
            if entry["type"] != "api"
                || !valid_token(key)
                || (base != issuer && base.strip_suffix("/v1") != Some(issuer))
            {
                return None;
            }
            Some((key, plan))
        })
        .collect()
}

pub(super) fn mimo_linked(now: i64, get: &Request<'_>) -> ResultSet {
    let root = xdg_data()
        .and_then(|dir| read_limit(&dir.join("mimocode/auth.json"), 128 * 1024))
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or("未找到本机额度登录")?;
    read_mimo_linked(&root, now, get)
}

fn read_mimo_linked(root: &Value, now: i64, get: &Request<'_>) -> ResultSet {
    let keys = mimo_linked_keys(root);
    if keys.is_empty() {
        return Err("未找到本机额度登录".into());
    }
    let mut metrics = Vec::new();
    let mut plans = Vec::new();
    let mut last_error = None;
    for (key, endpoint) in &keys {
        let auth = format!("Bearer {key}");
        let call = |url: &str| {
            get(
                url,
                &[("Authorization", &auth), ("Accept", "application/json")],
            )
        };
        let result = match endpoint {
            Some(url) => call(url).map(|body| mimo_plan_body(&body)),
            None => mimo_api(now, &call),
        };
        match result {
            Ok((items, plan)) => {
                metrics.extend(items);
                if !plan.is_empty() && !plans.contains(&plan) {
                    plans.push(plan);
                }
            }
            Err(e) => last_error = Some(e),
        }
    }
    if metrics.is_empty() {
        if let Some(e) = last_error {
            return Err(e);
        }
    }
    let mut plan = plans.join(" · ");
    for (key, _) in keys {
        plan = plan.replace(key, "••••");
        for item in &mut metrics {
            item.value = item.value.replace(key, "••••");
        }
    }
    Ok((metrics, plan))
}

pub(super) fn local_key(kind: &str) -> Option<String> {
    let home = std::env::var_os("USERPROFILE")
        .filter(|s| !s.is_empty())
        .map(PathBuf::from);
    let path = match kind {
        "commandcode" => home?.join(".commandcode/auth.json"),
        "opencode" => xdg_data()?.join("opencode/auth.json"),
        _ => return None,
    };
    let root = serde_json::from_slice(&read_limit(Path::new(&path), 128 * 1024)?).ok()?;
    key_from_file(kind, &root).map(str::to_owned)
}

fn key_from_file<'a>(kind: &str, root: &'a Value) -> Option<&'a str> {
    let pick = |v: &'a Value| v.as_str().map(str::trim).filter(|v| valid_token(v));
    match kind {
        "commandcode" => pick(&root["apiKey"]),
        "opencode" => ["opencode-go", "opencode"].iter().find_map(|name| {
            let item = &root[*name];
            (item["type"] == "api")
                .then(|| pick(&item["key"]))
                .flatten()
        }),
        _ => None,
    }
}

pub(super) fn read(kind: &str, token: &str, now: i64, get: &Request<'_>) -> Option<ResultSet> {
    let auth = format!("Bearer {token}");
    let call = |url: &str| {
        get(
            url,
            &[("Authorization", &auth), ("Accept", "application/json")],
        )
    };
    let reading = match kind {
        "commandcode" | "commandcode-api" => command_code(&call),
        "opencode" | "opencode-go" => call(OPEN).map(|body| (open_code(&body), String::new())),
        "kimi-api" => kimi_code(&call),
        "kimi-platform" => kimi_platform(&call),
        "mimo-api" | "mimo-token-plan" => {
            let plan_key = token.starts_with("tp-") || token.starts_with("ttp-");
            match (kind == "mimo-token-plan", plan_key) {
                (true, true) => mimo_plan(&call),
                (false, false) => mimo_api(now, &call),
                (true, false) => Err("请选择 Xiaomi MiMo API".into()),
                (false, true) => Err("请选择 Xiaomi MiMo Token Plan".into()),
            }
        }
        _ => return None,
    };
    Some(reading)
}

fn first_region<'a>(
    urls: &'a [&str],
    try_forbidden: bool,
    get: &impl Fn(&str) -> Result<Value, String>,
) -> Result<(&'a str, Value), String> {
    let mut last = "HTTP 401".to_owned();
    for &url in urls {
        match get(url) {
            Ok(doc) => return Ok((url, doc)),
            Err(e) if e == "HTTP 401" || (try_forbidden && e == "HTTP 403") => last = e,
            Err(e) => return Err(e),
        }
    }
    Err(last)
}

fn windows(
    root: &Value,
    keys: [(&str, &str, Option<u64>); 3],
    field: &str,
    reset: &str,
    scale: f64,
) -> Vec<Metric> {
    keys.into_iter()
        .filter_map(|(key, label, minutes)| {
            let cell = &root[key];
            Some(quota(
                label,
                minutes,
                num(&cell[field])? * scale,
                time(&cell[reset]),
            ))
        })
        .collect()
}

fn open_code(root: &Value) -> Vec<Metric> {
    windows(
        &root["usage"],
        [
            ("rolling", "5 小时额度", Some(300)),
            ("weekly", "每周额度", Some(10080)),
            ("monthly", "每月额度", None),
        ],
        "percent",
        "resetsAt",
        1.,
    )
}

fn kimi_code(get: &impl Fn(&str) -> Result<Value, String>) -> ResultSet {
    const CN: &str = "https://api.kimi.com/coding/v1/usages";
    const GLOBAL: &str = "https://api.kimi.ai/coding/v1/usages";
    let (url, doc) = first_region(&[CN, GLOBAL], false, get)?;
    let (base, _) = url.rsplit_once('/').expect("fixed endpoint");
    let me = get(&format!("{base}/me")).unwrap_or_default();
    let global = me["region"]
        .as_str()
        .filter(|s| s.starts_with("REGION_"))
        .map_or(url == GLOBAL, |s| s != "REGION_CN");
    let metrics = kimi_usage(&doc, if global { "USD" } else { "CNY" });
    Ok((
        metrics,
        text(&me["user_level_name"]).chars().take(40).collect(),
    ))
}

fn share(detail: &Value) -> Option<f64> {
    let limit = num(&detail["limit"]).filter(|v| *v > 0.)?;
    let used = num(&detail["used"]).or_else(|| Some(limit - num(&detail["remaining"])?))?;
    Some(used / limit * 100.)
}

fn reset(detail: &Value) -> Option<i64> {
    ["resetTime", "reset_time", "resetAt", "reset_at"]
        .into_iter()
        .find_map(|k| time(&detail[k]))
}

pub(super) fn kimi_usage(root: &Value, currency: &str) -> Vec<Metric> {
    let mut out = if root["usages"].is_object() {
        windows(
            &root["usages"],
            [
                ("limit_5h", "5 小时额度", Some(300)),
                ("limit_7d", "每周额度", Some(10080)),
                ("limit_month_total", "每月额度", Some(43200)),
            ],
            "used_ratio",
            "reset_time",
            100.,
        )
    } else {
        let mut out: Vec<Metric> = Vec::new();
        for row in root["limits"].as_array().into_iter().flatten().take(64) {
            let window = &row["window"];
            let multiplier = match window["timeUnit"].as_str() {
                Some("TIME_UNIT_SECOND") => 1. / 60.,
                Some("TIME_UNIT_MINUTE") => 1.,
                Some("TIME_UNIT_HOUR") => 60.,
                Some("TIME_UNIT_DAY") => 1440.,
                _ => continue,
            };
            let Some(minutes) = num(&window["duration"])
                .map(|n| n * multiplier)
                .filter(|n| (1. ..1_000_000.).contains(n))
            else {
                continue;
            };
            // Do not round an approximate duration into a named 5h/week alert window.
            let whole = (minutes.fract() == 0.).then_some(minutes as u64);
            let label = match whole {
                Some(300) => "5 小时额度".to_owned(),
                Some(10080) => "每周额度".to_owned(),
                Some(43200 | 44640) => "每月额度".to_owned(),
                _ => format!("{minutes:.1} 分钟额度"),
            };
            if let Some(used) =
                share(&row["detail"]).filter(|_| !out.iter().any(|m| m.label == label))
            {
                out.push(quota(&label, whole, used, reset(&row["detail"])));
            }
        }
        if !out.iter().any(|m| m.window == Some(10080)) {
            if let Some(used) = share(&root["usage"]) {
                out.push(quota("每周额度", Some(10080), used, reset(&root["usage"])));
            }
        }
        out
    };
    out.sort_by_key(|m| m.window.unwrap_or(u64::MAX));
    let wallet = &root["boosterWallet"];
    let limit = &wallet["monthlyChargeLimit"];
    let used = &wallet["monthlyUsed"];
    let balance = &wallet["balance"];
    if !out.is_empty()
        && balance["type"] == "BOOSTER"
        && num(&balance["amount"]).is_some_and(|n| n > 0.)
    {
        let currency = [limit, used]
            .into_iter()
            .find_map(|m| currency_code(&m["currency"]))
            .unwrap_or(currency);
        if let Some(left) = num(&balance["amountLeft"]) {
            out.push(metric(
                "加油包",
                money((left.max(0.) / 1e6).trunc() / 100., currency),
            ));
        }
        if wallet["monthlyChargeLimitEnabled"] == true {
            if let (Some(total), Some(spent)) = (
                num(&limit["priceInCents"]).filter(|n| *n > 0.),
                num(&used["priceInCents"]),
            ) {
                out.push(metric(
                    "本月加油包",
                    format!(
                        "{} / {}",
                        money(spent.max(0.) / 100., currency),
                        money(total / 100., currency)
                    ),
                ));
            }
        }
    }
    out
}

fn kimi_platform(get: &impl Fn(&str) -> Result<Value, String>) -> ResultSet {
    const CN: &str = "https://api.moonshot.cn/v1/users/me/balance";
    const GLOBAL: &str = "https://api.moonshot.ai/v1/users/me/balance";
    let (url, body) = first_region(&[CN, GLOBAL], false, get)?;
    if num(&body["code"]) != Some(0.) || num(&body["data"]["available_balance"]).is_none() {
        return Ok((Vec::new(), String::new()));
    }
    let currency = if url == CN { "CNY" } else { "USD" };
    Ok((
        balances(
            &body["data"],
            &[
                ("available_balance", "账户余额"),
                ("cash_balance", "现金余额"),
                ("voucher_balance", "代金券余额"),
            ],
            currency,
        ),
        String::new(),
    ))
}

fn currency_code(v: &Value) -> Option<&str> {
    v.as_str()
        .filter(|s| s.len() == 3 && s.bytes().all(|b| b.is_ascii_alphabetic()))
}

fn balances(root: &Value, fields: &[(&str, &str)], currency: &str) -> Vec<Metric> {
    fields
        .iter()
        .filter_map(|&(key, label)| {
            num(&root[key]).map(|amount| relay_balance_metric(label, money(amount, currency), amount,
                (label == "账户余额").then(|| super::RelayUnit::from_unit(currency)).flatten(), false))
        })
        .collect()
}

fn mimo_api(now: i64, get: &impl Fn(&str) -> Result<Value, String>) -> ResultSet {
    let doc = get(&format!("{MIMO}/v1/platform/balance"))?;
    let info = &doc["balance_infos"][0];
    let Some(currency) = currency_code(&info["currency"]).map(str::to_ascii_uppercase) else {
        return Ok((Vec::new(), String::new()));
    };
    if num(&info["total_available"]).is_none() {
        return Ok((Vec::new(), String::new()));
    }
    let mut out = balances(
        info,
        &[
            ("total_available", "账户余额"),
            ("cash_balance", "充值余额"),
            ("gift_balance", "赠送余额"),
        ],
        &currency,
    );
    if let Some(frozen) = num(&info["frozen_balance"]).filter(|n| *n > 0.) {
        out.push(metric("冻结金额", money(frozen, &currency)));
    }
    if let Some(date) = DateTime::from_timestamp(now, 0) {
        let query = date.format("start_date=%Y-%m-01&end_date=%Y-%m-%d");
        if let Ok(usage) = get(&format!("{MIMO}/v1/platform/usage/daily?{query}")) {
            if let Some(rows) = usage["data"].as_array() {
                let mut amounts = rows
                    .iter()
                    .filter(|row| {
                        row["currency"]
                            .as_str()
                            .is_some_and(|c| c.eq_ignore_ascii_case(&currency))
                    })
                    .filter_map(|row| num(&row["consumed_amount"]))
                    .peekable();
                if rows.is_empty() || amounts.peek().is_some() {
                    out.push(metric("本月扣费", money(amounts.sum(), &currency)));
                }
            }
        }
    }
    Ok((out, String::new()))
}

fn title(code: &str) -> String {
    code.split(['-', '_', ' '])
        .filter(|s| !s.is_empty())
        .map(|word| {
            let mut chars = word.chars();
            chars
                .next()
                .unwrap()
                .to_uppercase()
                .chain(chars)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn mimo_plan(get: &impl Fn(&str) -> Result<Value, String>) -> ResultSet {
    let (_, doc) = first_region(MIMO_PLANS, true, get)?;
    Ok(mimo_plan_body(&doc))
}

fn mimo_plan_body(doc: &Value) -> (Vec<Metric>, String) {
    let plan = title(&text(&doc["plan_code"]));
    let total = num(&doc["quota"]["total_credits"]).filter(|n| *n > 0.);
    let used = num(&doc["quota"]["used_credits"]).filter(|n| *n >= 0.);
    let (Some(total), Some(used)) = (total, used) else {
        return (Vec::new(), plan);
    };
    if plan.is_empty() {
        return (Vec::new(), plan);
    }
    let end = &doc["period"]["end_at"];
    let mut item = metric(
        "套餐积分",
        format!("{:.2} / {total:.2} Credits", (total - used).max(0.)),
    );
    item.reset_at = time(end).or_else(|| {
        Some(
            NaiveDateTime::parse_from_str(end.as_str()?, "%Y-%m-%d %H:%M:%S")
                .ok()?
                .and_utc()
                .timestamp(),
        )
    });
    (vec![item], plan)
}

fn query(pairs: &[(&str, &str)]) -> String {
    let mut out = String::new();
    for &(key, value) in pairs.iter().filter(|(_, v)| !v.is_empty()) {
        out.push(if out.is_empty() { '?' } else { '&' });
        out.push_str(key);
        out.push('=');
        for b in value.bytes() {
            if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
                out.push(b as char);
            } else {
                use std::fmt::Write;
                let _ = write!(out, "%{b:02X}");
            }
        }
    }
    out
}

fn command_code(get: &impl Fn(&str) -> Result<Value, String>) -> ResultSet {
    let who = get(&format!("{CODE}/alpha/whoami"))?;
    let org = who["org"]["id"].as_str().unwrap_or("");
    let org_query = query(&[("orgId", org)]);
    let credits = get(&format!("{CODE}/alpha/billing/credits{org_query}"))?;
    let subscription =
        get(&format!("{CODE}/alpha/billing/subscriptions{org_query}")).unwrap_or_default();
    let since = subscription["data"]["currentPeriodStart"]
        .as_str()
        .unwrap_or("");
    let summary_query = query(&[("orgId", org), ("since", since)]);
    let summary = get(&format!("{CODE}/alpha/usage/summary{summary_query}")).unwrap_or_default();
    Ok(command_metrics(&credits, &subscription, &summary))
}

fn command_metrics(
    credits: &Value,
    subscription: &Value,
    summary: &Value,
) -> (Vec<Metric>, String) {
    let mut out = Vec::new();
    if !credits["credits"].is_object() {
        return (out, String::new());
    }
    for (field, label, minutes) in [
        ("fiveHour", "5 小时额度", 300),
        ("weekly", "每周额度", 10080),
    ] {
        let window = &credits["windowLimits"][field];
        if let (Some(cap), Some(used)) = (
            num(&window["cap"]).filter(|n| *n > 0.),
            num(&window["used"]),
        ) {
            out.push(quota(
                label,
                Some(minutes),
                used / cap * 100.,
                time(&window["resetAt"]),
            ));
        }
    }
    let balance = &credits["credits"];
    let period = &subscription["data"];
    let active = matches!(
        period["status"].as_str(),
        Some("active" | "trialing" | "past_due")
    );
    let end = active.then(|| time(&period["currentPeriodEnd"])).flatten();
    if let Some(left) = num(&balance["monthlyCredits"]).filter(|n| *n >= 0.) {
        if let Some(spent) =
            num(&summary["totalMonthlyCredits"]).filter(|n| *n >= 0. && *n + left > 0.)
        {
            out.push(quota("每月额度", None, spent / (spent + left) * 100., end));
        } else {
            let mut item = metric("每月剩余积分", money(left, "USD"));
            item.reset_at = end;
            out.push(item);
        }
    }
    for (key, label) in [
        ("purchasedCredits", "购买积分"),
        ("freeCredits", "赠送积分"),
    ] {
        if let Some(n) = num(&balance[key]).filter(|n| *n >= 0.) {
            out.push(metric(label, money(n, "USD")));
        }
    }
    let plan = if active {
        let id = text(&period["planId"])
            .to_ascii_lowercase()
            .replace('_', "-");
        let id = id
            .rsplit_once("-v")
            .filter(|(_, v)| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()))
            .map_or(id.as_str(), |(s, _)| s);
        if id == "individual-goat" {
            "GOAT".into()
        } else {
            title(id.strip_prefix("individual-").unwrap_or(id))
        }
    } else {
        String::new()
    };
    (out, plan)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::cell::RefCell;

    #[test]
    fn quota_account_opencode_known_zero_and_missing_week_are_distinct() {
        let out = open_code(
            &json!({"usage":{"rolling":{"percent":100,"resetsAt":"2026-11-28T20:00:00Z"},"weekly":{"status":"ok"},"monthly":{"percent":0}}}),
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].window, Some(300));
        assert_eq!(out[0].remaining, Some(0.));
        assert_eq!(out[1].window, None);
        assert_eq!(out[1].remaining, Some(100.));
        assert_eq!(
            key_from_file(
                "opencode",
                &json!({"opencode-go":{"type":"oauth","key":"wrong"},"opencode":{"type":"api","key":"oc_sk_demo"}})
            ),
            Some("oc_sk_demo")
        );
    }

    #[test]
    fn quota_account_kimi_handles_both_shapes_without_guessing_missing_values() {
        let modern = kimi_usage(
            &json!({"usages":{"limit_5h":{"used_ratio":0.875},"limit_7d":{},"limit_month_total":{"used_ratio":0.5}}}),
            "CNY",
        );
        assert_eq!(modern[0].remaining, Some(12.5));
        assert_eq!(modern[1].window, Some(43200));
        let old = kimi_usage(
            &json!({"limits":[{"window":{"duration":5,"timeUnit":"TIME_UNIT_HOUR"},"detail":{"limit":100,"remaining":15}}],"usage":{"limit":100,"used":95},"boosterWallet":{"balance":{"type":"BOOSTER","amount":"200000000","amountLeft":"150000000"}}}),
            "USD",
        );
        assert_eq!(
            old.iter().map(|m| m.window).collect::<Vec<_>>(),
            [Some(300), Some(10080), None]
        );
        assert_eq!(old[2].value, "$1.50");
    }

    #[test]
    fn quota_account_platform_fallback_only_on_rejected_key() {
        let calls = RefCell::new(Vec::new());
        let read = |url: &str, headers: &[(&str, &str)]| {
            assert_eq!(headers[0], ("Authorization", "Bearer example"));
            calls.borrow_mut().push(url.to_owned());
            if url.contains("moonshot.cn") {
                Err("HTTP 401".into())
            } else {
                Ok(json!({"code":0,"data":{"available_balance":0,"cash_balance":-1}}))
            }
        };
        let (metrics, _) = super::read("kimi-platform", "example", 0, &read)
            .unwrap()
            .unwrap();
        assert_eq!(calls.borrow().len(), 2);
        assert_eq!(metrics[0].value, "$0.00");
        assert_eq!(metrics[0].remaining, None);
        let rate_limited = |url: &str| {
            assert!(url.contains("moonshot.cn"));
            Err("HTTP 429".into())
        };
        assert!(matches!(kimi_platform(&rate_limited), Err(e) if e == "HTTP 429"));
    }

    #[test]
    fn quota_account_command_code_uses_org_query_without_reclassifying_month() {
        let read = |url: &str| {
            assert!(url.starts_with(CODE));
            if url.ends_with("/whoami") {
                return Ok(json!({"org":{"id":"org/a&b"}}));
            }
            assert!(url.contains("?orgId=org%2Fa%26b"));
            if url.contains("/credits") {
                Ok(
                    json!({"credits":{"monthlyCredits":40},"windowLimits":{"fiveHour":{"cap":10,"used":9,"resetAt":1795896000000_i64},"weekly":{"cap":30}}}),
                )
            } else if url.contains("/subscriptions") {
                Ok(
                    json!({"data":{"status":"active","planId":"individual-goat-v2","currentPeriodStart":"2026-11-01T00:00:00Z"}}),
                )
            } else {
                assert!(url.ends_with("&since=2026-11-01T00%3A00%3A00Z"));
                Ok(json!({"totalMonthlyCredits":60}))
            }
        };
        let (out, plan) = command_code(&read).unwrap();
        assert_eq!(plan, "GOAT");
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].remaining, Some(10.));
        assert_eq!(out[0].reset_at, Some(1795896000));
        assert_eq!(out[1].window, None);
        assert_eq!(out[1].remaining, Some(40.));
    }

    #[test]
    fn quota_account_mimo_pins_token_plan_to_issuer_and_reports_credits_not_windows() {
        let calls = RefCell::new(Vec::new());
        let get = |url: &str, _: &[(&str, &str)]| {
            calls.borrow_mut().push(url.to_owned());
            if url == MIMO_PLANS[0] {
                Err("HTTP 403".into())
            } else {
                Ok(
                    json!({"plan_code":"lite","period":{"end_at":"2026-12-13 23:59:59"},"quota":{"total_credits":"12.0","used_credits":"9.5"}}),
                )
            }
        };
        assert!(matches!(read("mimo-api", "tp-test", 0, &get), Some(Err(_))));
        assert!(calls.borrow().is_empty());
        let (out, plan) = read("mimo-token-plan", "tp-test", 0, &get)
            .unwrap()
            .unwrap();
        assert_eq!(calls.borrow().as_slice(), &MIMO_PLANS[..2]);
        assert_eq!(plan, "Lite");
        assert_eq!(out[0].value, "2.50 / 12.00 Credits");
        assert!(out[0].reset_at.is_some());
        assert_eq!(out[0].window, None);
        assert_eq!(out[0].remaining, None);
    }

    #[test]
    fn quota_account_mimo_linked_never_probes_wrong_cluster_or_untrusted_host() {
        let root = json!({
            "xiaomi-token-plan-cn": {"type":"api","key":"tp-rejected","metadata":{"base_url":"https://token-plan-cn.xiaomimimo.com.evil.test/v1"}},
            "xiaomi-token-plan-sgp": {"type":"api","key":"tp-bound","metadata":{"base_url":"https://token-plan-sgp.xiaomimimo.com/v1"}},
            "xiaomi-token-plan-ams": {"type":"api","key":"tp-wrong","metadata":{"base_url":"https://token-plan-sgp.xiaomimimo.com/v1"}}
        });
        let (metrics, plan) = read_mimo_linked(&root, 0, &|url, headers| {
            assert_eq!(url, MIMO_PLANS[1]);
            assert_eq!(headers[0], ("Authorization", "Bearer tp-bound"));
            Ok(json!({"plan_code":"lite","quota":{"total_credits":5,"used_credits":3}}))
        })
        .unwrap();
        assert_eq!(plan, "Lite");
        assert_eq!(metrics[0].value, "2.00 / 5.00 Credits");
        assert!(
            matches!(read_mimo_linked(&json!({}), 0, &|_, _| panic!("unexpected request")), Err(e) if e == "未找到本机额度登录")
        );
    }

    #[test]
    fn quota_account_mimo_api_sums_only_response_currency() {
        let (out, _) = mimo_api(1_795_824_000, &|url| {
            if url.ends_with("/balance") { return Ok(json!({"balance_infos":[{"currency":"cny","total_available":"2.0"}]})) }
            assert!(url.contains("/usage/daily?start_date=2026-11-01&end_date=2026-11-"));
            Ok(json!({"data":[{"currency":"CNY","consumed_amount":"1.5"},{"currency":"USD","consumed_amount":"4.0"}]}))
        }).unwrap();
        assert_eq!(out[0].value, "¥2.00");
        assert_eq!(out[1].value, "¥1.50");
    }
}
