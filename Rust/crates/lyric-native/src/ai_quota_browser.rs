// SPDX-License-Identifier: Apache-2.0
//! BrimDeck WorkBuddyUsage/QianwenUsage translated to the DotIsle read-only sampler.
//! The control center acquires a session through explicit browser authorization.
use super::{metric, money, num, quota, text, time, valid_token, Metric, Value};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde::Deserialize;
use serde_json::json;

type Post<'a> = dyn Fn(&str, &[u8], &[(&str, &str)]) -> Result<Value, String> + 'a;
type Get<'a> = dyn Fn(&str, &[(&str, &str)]) -> Result<Value, String> + 'a;
type Reading = (Vec<Metric>, String);
const EMPTY: &str = "未返回可识别的额度";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Session {
    token: String,
    #[serde(default)]
    user: String,
    #[serde(default)]
    domain: String,
    #[serde(default)]
    expires: String,
}

pub(super) fn read(
    kind: &str,
    blob: &str,
    now: i64,
    get: &Get<'_>,
    post: &Post<'_>,
) -> Result<Reading, String> {
    let mut raw = URL_SAFE_NO_PAD.decode(blob).map_err(|_| "未配置凭据")?;
    let s = serde_json::from_slice::<Session>(&raw);
    raw.fill(0);
    let s = s.map_err(|_| "未配置凭据")?;
    if !valid_token(&s.token)
        || (!s.domain.is_empty() && !valid_token(&s.domain))
        || (!s.user.is_empty() && !valid_token(&s.user))
    {
        return Err("未配置凭据".into());
    }
    if !s.expires.is_empty() && time(&json!(s.expires)).is_none_or(|v| v <= now) {
        return Err("官网授权已失效，请重新授权".into());
    }
    let result = match kind {
        "workbuddy" => workbuddy("https://www.workbuddy.cn", &s, post),
        "workbuddy-ai" => workbuddy("https://www.workbuddy.ai", &s, post),
        "qianwen" => qianwen(true, &s, now, post),
        "qwencloud" => qianwen(false, &s, now, post),
        "mimo" | "doubao" | "dola" => cookie_provider(kind, &s, get, post),
        _ => return Err(EMPTY.into()),
    };
    let (mut rows, mut plan) = result.map_err(|e| {
        if e == "HTTP 401" {
            "官网授权已失效，请重新授权".into()
        } else {
            e
        }
    })?;
    if rows.is_empty() {
        return Err(EMPTY.into());
    }
    for secret in [s.token.as_str(), blob] {
        plan = plan.replace(secret, "••••");
        for row in &mut rows {
            row.label = row.label.replace(secret, "••••");
            row.value = row.value.replace(secret, "••••");
        }
    }
    Ok((rows, plan))
}
fn points(label: &str, total: Option<f64>, left: Option<f64>) -> Option<Metric> {
    let total = total.filter(|v| *v > 0.);
    let left = left.filter(|v| *v >= 0.);
    let val = match (total, left) {
        (Some(all), Some(n)) => format!("剩余 {n:.2} / {:.2} 积分", all.max(n)),
        (_, Some(n)) => format!("剩余 {n:.2} 积分"),
        (Some(n), None) => format!("总计 {n:.2} 积分"),
        _ => return None,
    };
    Some(metric(label, val))
}
fn sum(values: impl Iterator<Item = f64>) -> Option<f64> {
    values
        .filter(|v| v.is_finite())
        .map(Some)
        .reduce(|a, b| Some(a.unwrap_or(0.) + b.unwrap_or(0.)))
        .flatten()
}
fn workbuddy(base: &str, s: &Session, post: &Post<'_>) -> Result<Reading, String> {
    if s.user.is_empty() {
        return Err("未配置凭据".into());
    }
    let bearer = format!("Bearer {}", s.token);
    let mut headers = vec![
        ("Authorization", bearer.as_str()),
        ("X-User-Id", s.user.as_str()),
        ("Content-Type", "application/json"),
        ("Accept-Language", "zh"),
    ];
    if !s.domain.is_empty() {
        headers.push(("X-Domain", &s.domain));
    }
    let doc = post(
        &format!("{base}/billing/meter/get-user-resource-summary"),
        b"{}",
        &headers,
    )?;
    if num(&doc["code"]) != Some(0.) {
        return Err(EMPTY.into());
    }
    let data = &doc["data"];
    let field = |label: &str, row: &Value| {
        points(
            label,
            num(&row["CycleTotalCapacity"]),
            num(&row["CycleRemainCapacity"]),
        )
    };
    let rows = if let Some(groups) = data["CreditGroups"].as_array() {
        groups
            .iter()
            .take(16)
            .filter_map(|r| {
                let label = match r["Kind"].as_str() {
                    Some("plan_base") => "套餐积分",
                    Some("plan_bonus") => "套餐赠送积分",
                    Some("addon") => "购买积分",
                    Some("benefit") => "奖励积分",
                    _ => "",
                };
                let other = text(&r["Name"]);
                let label = if label.is_empty() {
                    other.trim()
                } else {
                    label
                };
                if label.is_empty() {
                    return None;
                };
                field(label, r)
            })
            .collect()
    } else {
        let packages = data["Packages"].as_array().ok_or(EMPTY)?;
        points(
            "积分",
            sum(packages
                .iter()
                .filter_map(|r| num(&r["CycleTotalCapacity"]))),
            sum(packages
                .iter()
                .filter_map(|r| num(&r["CycleRemainCapacity"]))),
        )
        .into_iter()
        .collect()
    };
    Ok((rows, text(&data["SubscriptionPackageName"])))
}
fn form(v: &str) -> String {
    let mut out = String::new();
    for b in v.bytes() {
        if b.is_ascii_alphanumeric() || b"-._~".contains(&b) {
            out.push(b as char)
        } else {
            use std::fmt::Write;
            let _ = write!(out, "%{b:02X}");
        }
    }
    out
}
fn qianwen(cn: bool, s: &Session, now: i64, post: &Post<'_>) -> Result<Reading, String> {
    let (api, region, suffix) = if cn {
        (
            "https://cli.qianwenai.com/data/v2/api.json",
            "cn-beijing",
            "cn",
        )
    } else {
        (
            "https://cli.qwencloud.com/data/v2/api.json",
            "ap-southeast-1",
            "intl",
        )
    };
    let bearer = format!("Bearer {}", s.token);
    let call = |action: &str, params: Value, console: bool| -> Result<Value, String> {
        let mut request = json!({"product":if console {"ModelStudioRegional"}else{"BssOpenAPI-V3"}, "action":action,"region":region,"params":params});
        if console {
            request["version"] = json!("2026-02-10")
        };
        let doc = post(
            api,
            request.to_string().as_bytes(),
            &[
                ("Authorization", &bearer),
                ("Content-Type", "application/json"),
            ],
        )?;
        let code = &doc["code"];
        if matches!(
            code.as_str(),
            Some("ConsoleNeedLogin" | "NotLogin" | "TokenExpired")
        ) {
            return Err("HTTP 401".into());
        }
        if num(code) != Some(200.) {
            return Err(EMPTY.into());
        }
        Ok(doc["data"].clone())
    };
    let instances = |name: &str| {
        call(
            "DescribeFrInstances",
            json!({"Group":"tokenPlan","CommodityCode":format!("sfm_tokenplan{name}_dp_{suffix}"),"PageNum":"1","PageSize":"10"}),
            false,
        )
    };
    let mut rows = Vec::new();
    let mut plans = Vec::new();
    let mut rejected = false;
    let mut optional = |r: Result<Value, String>| match r {
        Ok(v) => Some(v),
        Err(e) => {
            rejected |= e == "HTTP 401";
            None
        }
    };
    if cn {
        if let Some(key) = optional(call("GenerateCLIAccessToken", json!({}), true)).and_then(|v| {
            v["cliAccessToken"]
                .as_str()
                .filter(|k| valid_token(k) && *k != s.token)
                .map(ToOwned::to_owned)
        }) {
            let cn_bearer = format!("Bearer {key}");
            let regional = |path: &str, mut params: Value| -> Result<Value, String> {
                let api = format!("zeldaHttp.apikeyMgr./tokenplan/personal/api/v2/{path}");
                params["cornerstoneParam"] = json!({"protocol":"V2","console":"ONE_CONSOLE","productCode":"p_efm","switchUserType":3,"consoleSite":"QIANWENAI"});
                let body = format!(
                    "params={}&region=cn-beijing",
                    form(&json!({"Api":api,"V":"1.0","Data":params}).to_string())
                );
                let doc=post(&format!("https://cs-data.qianwenai.com/cli/api.json?action=BroadScopeAspnGateway&product=sfm_bailian&api={}",form(&api)),body.as_bytes(),&[("Authorization",&cn_bearer),("Content-Type","application/x-www-form-urlencoded")])?;
                let env = &doc["data"];
                let result = &env["DataV2"];
                let business = &result["data"];
                if num(&doc["code"]) != Some(200.)
                    || doc["successResponse"] != true
                    || env["success"] != true
                    || !result["ret"][0]
                        .as_str()
                        .is_some_and(|s| s.starts_with("SUCCESS::"))
                    || business["success"] != true
                {
                    return Err(EMPTY.into());
                }
                Ok(business["data"].clone())
            };
            if let Some(sub) = optional(regional(
                "subscription",
                json!({"commodityCode":"sfm_tokenplansolo_public_cn"}),
            )) {
                let expires = time(&sub["endTime"]);
                if sub.is_object()
                    && expires.is_none_or(|e| e > now)
                    && sub["status"]
                        .as_str()
                        .is_none_or(|v| v.eq_ignore_ascii_case("valid"))
                {
                    if let Some(usage) = optional(regional("usage", json!({}))) {
                        let mut valid = false;
                        for (label, used, reset, minutes) in [
                            (
                                "个人版 · 每周额度",
                                "per1WeekPercentage",
                                "per1WeekResetTime",
                                Some(10080),
                            ),
                            (
                                "个人版 · 每月额度",
                                "per1MonthPercentage",
                                "per1MonthResetTime",
                                None,
                            ),
                        ] {
                            if let Some(part) = num(&usage[used]).filter(|p| (0. ..=1.).contains(p))
                            {
                                let mut row =
                                    quota(label, minutes, part * 100., time(&usage[reset]));
                                row.expires_at = expires;
                                rows.push(row);
                                valid = true;
                            }
                        }
                        if valid {
                            let name = text(&sub["specCode"]);
                            plans.push(if name.is_empty() {
                                "个人版".to_owned()
                            } else {
                                name.replace(&key, "••••")
                            });
                        }
                    }
                }
            }
        }
    }
    if let Some(d) = optional(instances("personal")) {
        if let Some((m, name)) = qwen_instance(&d, "个人版 · 套餐额度", now) {
            rows.push(m);
            plans.push(name);
        }
    }
    let seats = if cn {
        true
    } else {
        optional(call("QuerySubscriptionGray", json!({}), false))
            .is_some_and(|d| d["IsGray"] == true)
    };
    let team = if seats {
        optional(call(
            "GetSeatSubscriptionSummary",
            json!({"productCode":format!("sfm_tokenplanteams_dp_{suffix}")}),
            false,
        ))
        .and_then(|d| qwen_seats(&d, now))
    } else {
        None
    };
    if let Some((m, name)) = team.or_else(|| {
        optional(instances("teams")).and_then(|d| qwen_instance(&d, "团队版 · 套餐额度", now))
    }) {
        rows.push(m);
        plans.push(name);
    }
    if let Some(d) = optional(instances("teamsaddon")) {
        let items = qwen_active(&d, now);
        let mut m = points(
            "团队版 · 加油包",
            sum(qwen_active(&d, now).filter_map(|v| capacity(v).0)),
            sum(items.filter_map(|v| capacity(v).1)),
        );
        if let Some(row) = m.as_mut() {
            row.expires_at = qwen_active(&d, now)
                .filter_map(|v| time(&v["EndTime"]))
                .min();
        }
        rows.extend(m);
    }
    if cn {
        if let Some(d) = optional(call("GetFundAccountAvailableAmount", json!({}), false)) {
            if let Some(amount) = num(&d["AvailableAmount"]) {
                let given = d["Currency"].as_str().unwrap_or("CNY").to_ascii_uppercase();
                rows.push(metric("账户余额", money(amount, &given)));
            }
        }
    }
    if rows.is_empty() && rejected {
        return Err("HTTP 401".into());
    }
    plans.dedup();
    Ok((rows, plans.join(" · ")))
}
fn qwen_active(d: &Value, now: i64) -> impl Iterator<Item = &Value> {
    d["Data"]
        .as_array()
        .into_iter()
        .flatten()
        .take(128)
        .filter(move |v| {
            let status = &v["Status"];
            status
                .as_str()
                .or_else(|| status["Code"].as_str())
                .is_some_and(|s| s.eq_ignore_ascii_case("valid"))
                && time(&v["EndTime"]).is_none_or(|e| e > now)
        })
}
fn capacity(item: &Value) -> (Option<f64>, Option<f64>) {
    let cur = num(&item["CurrCapacityBaseValue"]);
    let left = if item["CapacityTypeCode"] == "periodMonthlyShift" {
        num(&item["periodCapacityBaseValue"]).or(cur)
    } else {
        cur
    };
    (num(&item["InitCapacityBaseValue"]), left)
}
fn qwen_instance(d: &Value, label: &str, now: i64) -> Option<(Metric, String)> {
    let item = qwen_active(d, now).next()?;
    let (total, left) = capacity(item);
    let mut m = points(label, total, left)?;
    m.expires_at = time(&item["EndTime"]);
    let name = text(&item["TemplateName"]);
    Some((
        m,
        if name.is_empty() {
            text(&item["CommodityName"])
        } else {
            name
        },
    ))
}
fn qwen_seats(d: &Value, now: i64) -> Option<(Metric, String)> {
    let d = d.get("Data").filter(|v| v.is_object()).unwrap_or(d);
    let expiry = time(&d["PeriodEnd"]).or_else(|| time(&d["EndTime"]));
    if expiry.is_some_and(|v| v <= now) {
        return None;
    }
    let groups = d["SubscriptionGroupList"].as_array()?;
    let load = |field| {
        sum(groups
            .iter()
            .take(128)
            .filter_map(|g| num(&g["EquityList"][0][field]).or_else(|| num(&g[field]))))
    };
    let mut m = points(
        "团队版 · 每月额度",
        load("TotalValue"),
        load("SurplusValue"),
    )?;
    m.expires_at = expiry;
    m.reset_at = groups
        .iter()
        .filter_map(|g| time(&g["NextCycleFlushTime"]))
        .filter(|r| expiry.is_none_or(|e| *r <= e))
        .min();
    let name = text(&d["PlanName"]);
    Some((
        m,
        if name.is_empty() {
            "团队版".into()
        } else {
            name
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn workbuddy_uses_personal_account_and_does_not_echo_credential() {
        let blob = URL_SAFE_NO_PAD.encode(br#"{"token":"fake-test-value","user":"case001"}"#);
        let (m,p)=read("workbuddy-ai",&blob,1,&|_,_|unreachable!(),&|u,_,h|{
            assert_eq!(u,"https://www.workbuddy.ai/billing/meter/get-user-resource-summary");
            assert!(h.contains(&("X-User-Id","case001")));
            Ok(json!({"code":0,"data":{"CreditGroups":[{"Kind":"plan_base","CycleTotalCapacity":23,"CycleRemainCapacity":5}],"SubscriptionPackageName":"fake-test-value"}}))
        }).unwrap();
        assert_eq!(p, "••••");
        assert_eq!(m[0].value, "剩余 5.00 / 23.00 积分");
        assert!(m[0].window.is_none());
    }
    #[test]
    fn qwen_plan_reads_active_capacity_and_pins_edition() {
        let blob = URL_SAFE_NO_PAD.encode(br#"{"token":"fake-test-value"}"#);
        let (m,p)=read("qwencloud",&blob,11,&|_,_|unreachable!(),&|u,b,h|{
            assert_eq!(u,"https://cli.qwencloud.com/data/v2/api.json");
            assert!(h.contains(&("Authorization","Bearer fake-test-value")));
            let call:Value=serde_json::from_slice(b).unwrap();
            if call["params"]["CommodityCode"]=="sfm_tokenplanpersonal_dp_intl" {
                return Ok(json!({"code":"200","data":{"Data":[{"Status":"valid","InitCapacityBaseValue":100,"CurrCapacityBaseValue":80,"EndTime":10}, {"Status":"valid","TemplateName":"test","InitCapacityBaseValue":1000,"CurrCapacityBaseValue":650}]}}));
            }
            Ok(json!({"code":"200","data":{}}))
        }).unwrap();
        assert_eq!(p, "test");
        assert_eq!(m[0].value, "剩余 650.00 / 1000.00 积分");
        assert!(m[0].window.is_none());
    }
}

fn cookie_provider(
    kind: &str,
    s: &Session,
    get: &Get<'_>,
    post: &Post<'_>,
) -> Result<Reading, String> {
    let cookie = s
        .token
        .split(';')
        .filter(|p| {
            p.split_once('=').is_some_and(|(k, v)| {
                !k.is_empty() && !v.is_empty() && valid_token(k) && valid_token(v)
            })
        })
        .collect::<Vec<_>>()
        .join("; ");
    if cookie.is_empty() || cookie.len() > 8192 {
        return Err("未配置凭据".into());
    }
    if kind == "mimo" {
        let doc = get(
            "https://mimo-server-cn.xiaomimimo.com/api/user/xiaomi/subscription/self",
            &[("Cookie", &cookie)],
        )?;
        if num(&doc["code"]) != Some(0.) || !doc["data"].is_object() {
            return Err("HTTP 401".into());
        }
        let cur = &doc["data"]["current"];
        let plan = match num(&cur["planTier"]) {
            Some(1.) => "入门".into(),
            Some(2.) => "进阶".into(),
            Some(3.) => "高阶".into(),
            Some(4.) => "尊享".into(),
            _ => text(&cur["title"]),
        };
        let rows = num(&cur["percent"])
            .filter(|p| (0. ..=100.).contains(p))
            .map(|left| {
                quota(
                    "每周额度",
                    Some(10080),
                    100. - left,
                    mimo_reset(&cur["nextResetTime"]),
                )
            })
            .into_iter()
            .collect();
        return Ok((rows, plan));
    }
    let (base, path) = if kind == "dola" {
        (
            "https://www.dola.com",
            "/alice/commerce/sale/subscription/overview/",
        )
    } else {
        (
            "https://www.doubao.com",
            "/alice/commerce/sale/subscription/quota/summary/",
        )
    };
    let csrf = ["passport_csrf_token", "passport_csrf_token_default"]
        .into_iter()
        .find_map(|name| {
            s.token
                .split(';')
                .filter_map(|p| p.split_once('='))
                .find(|(k, v)| *k == name && !v.is_empty())
                .map(|(_, v)| v)
        });
    let referer = format!("{base}/chat/");
    let mut headers = vec![
        ("Cookie", cookie.as_str()),
        ("Origin", base),
        ("Referer", referer.as_str()),
        ("Content-Type", "application/json"),
        ("Accept", "application/json"),
    ];
    if let Some(csrf) = csrf {
        headers.push(("x-csrf-token", csrf));
    }
    let doc = post(
        &format!("{base}{path}"),
        br#"{"product_line":"membership"}"#,
        &headers,
    )?;
    let (mut rows, mut plan) = doubao_metrics(&doc)?;
    // The provider may echo a single cookie value rather than the entire header.
    for (_, value) in s
        .token
        .split(';')
        .filter_map(|p| p.split_once('='))
        .filter(|(_, v)| v.len() > 7)
    {
        plan = plan.replace(value, "••••");
        for row in &mut rows {
            row.label = row.label.replace(value, "••••");
        }
    }
    Ok((rows, plan))
}
fn mimo_reset(v: &Value) -> Option<i64> {
    use chrono::{FixedOffset, NaiveDateTime};
    time(v).or_else(|| {
        ["%Y-%m-%d %H:%M:%S", "%Y-%m-%dT%H:%M:%S%.f"]
            .iter()
            .find_map(|fmt| {
                NaiveDateTime::parse_from_str(v.as_str()?, fmt)
                    .ok()?
                    .and_local_timezone(FixedOffset::east_opt(8 * 3600)?)
                    .single()
                    .map(|t| t.timestamp())
            })
    })
}
fn doubao_metrics(root: &Value) -> Result<Reading, String> {
    if !root.is_object() {
        return Err(EMPTY.into());
    }
    if !root["code"].is_null() && num(&root["code"]) != Some(0.) {
        if matches!(
            num(&root["code"]),
            Some(1000. | 10001. | 10004. | 401. | 403. | 70001. | 710012001.)
        ) {
            return Err("HTTP 401".into());
        }
        return Err(EMPTY.into());
    }
    let d = root.get("data").filter(|v| v.is_object()).unwrap_or(root);
    let mut rows = Vec::new();
    let mut plans = Vec::new();
    for (section, label) in [
        ("window_limit_section", "个人版"),
        ("enterprise_window_limit_section", "团队／企业"),
        ("quota_package_section", "创作额度包"),
        ("incentive_quota_section", "活动奖励"),
    ] {
        let data = &d[section];
        let prev = rows.len();
        for group in data["window_limit_groups"]
            .as_array()
            .into_iter()
            .flatten()
            .take(24)
        {
            let title = text(&group["feature_group_name"]);
            for item in group["window_limits"]
                .as_array()
                .into_iter()
                .flatten()
                .take(12)
            {
                let (period, window) = match num(&item["window_type"]) {
                    Some(1.) => ("5 小时额度", Some(300)),
                    Some(2.) => ("每周额度", Some(10080)),
                    Some(3.) => ("合计额度", None),
                    Some(4.) => ("每月额度", None),
                    _ => continue,
                };
                let is_subscription = section == "window_limit_section"
                    || section == "enterprise_window_limit_section";
                if !is_subscription && period != "合计额度" {
                    continue;
                }
                let prefix = if title.is_empty() {
                    label
                } else {
                    title.as_str()
                };
                let name = format!("{prefix} · {period}");
                let exemption = window == Some(300) && item["exemption"]["active"] == true;
                let reset = time(if exemption {
                    &item["exemption"]["end_time"]
                } else {
                    &item["end_time"]
                });
                if exemption || item["unlimited"] == true {
                    let mut m = metric(&name, "不限量".into());
                    m.reset_at = reset;
                    rows.push(m);
                    continue;
                }
                let exhausted = data["usage_exhausted"] == true || item["exhausted"] == true;
                let amount = num(&item["total_amount"])
                    .filter(|n| *n > 0.)
                    .zip(num(&item["used_amount"]))
                    .map(|(all, used)| 100. * used / all);
                let used = if exhausted {
                    Some(100.)
                } else {
                    amount.or_else(|| {
                        num(&item["used_percent"]).map(|pct| {
                            if pct <= 0. && item["less_than_one_percent"] == true {
                                0.5
                            } else {
                                pct
                            }
                        })
                    })
                };
                if let Some(used) = used {
                    rows.push(quota(&name, window, used, reset));
                }
            }
        }
        if rows.len() > prev
            && (section == "window_limit_section" || section == "enterprise_window_limit_section")
        {
            let plan = [
                "plan_name",
                "planName",
                "subscription_name",
                "subscriptionName",
                "tier_name",
                "tierName",
            ]
            .into_iter()
            .find_map(|k| {
                let v = text(&data[k]);
                if !v.is_empty() {
                    Some(v)
                } else {
                    None
                }
            })
            .unwrap_or_else(|| label.into());
            plans.push(plan);
        }
    }
    if rows.is_empty()
        && d["member_info"]["hasActiveSubscription"] == false
        && d["member_info"]["hasEnterpriseSubscription"] != true
    {
        rows.push(metric("套餐", "未订阅或无可用额度窗口".into()));
    }
    rows.truncate(24);
    Ok((rows, plans.join(" · ")))
}

#[cfg(test)]
mod cookie_tests {
    use super::*;
    #[test]
    fn doubao_real_windows_and_unknown_are_distinct() {
        let doc = json!({"code":0,"data":{"member_info":{"hasActiveSubscription":true},"window_limit_section":{"window_limit_groups":[{"window_limits":[
            {"window_type":1,"used_percent":92,"end_time":1700000000},
            {"window_type":2,"total_amount":100,"used_amount":92},
            {"window_type":4},
            {"window_type":1,"exemption":{"active":true}}
        ]}]}}});
        let (rows, _) = doubao_metrics(&doc).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(
            (rows[0].window, rows[0].remaining, rows[0].reset_at),
            (Some(300), Some(8.), Some(1700000000))
        );
        assert_eq!((rows[1].window, rows[1].remaining), (Some(10080), Some(8.)));
        assert!(rows[2].remaining.is_none());
        assert_eq!(
            doubao_metrics(&json!({"code":710012001})).unwrap_err(),
            "HTTP 401"
        );
    }
    #[test]
    fn dola_uses_overview_and_passes_only_its_cookie_and_csrf() {
        let sample = URL_SAFE_NO_PAD
            .encode(br#"{"token":"sessionid=fixture-session;passport_csrf_token=fixture-csrf"}"#);
        let (rows,_)=read("dola",&sample,1,&|_,_|unreachable!(),&|url,_,headers|{
            assert_eq!(url,"https://www.dola.com/alice/commerce/sale/subscription/overview/");
            assert!(headers.contains(&("x-csrf-token","fixture-csrf")));
            Ok(json!({"data":{"member_info":{"hasActiveSubscription":false,"hasEnterpriseSubscription":false}}}))
        }).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].remaining.is_none());
    }
}
