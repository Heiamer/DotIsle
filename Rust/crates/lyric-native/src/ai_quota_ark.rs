// SPDX-License-Identifier: Apache-2.0
//! Quota/account-balance APIs from BrimDeck VolcengineArkUsage.cs, using a
//! user-provided read-only cloud access key pair. No browser or broad OAuth scope.
use super::{metric, money, num, quota, text, time, valid_token, Metric, Value};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};

type Post<'a> = dyn Fn(&str, &[u8], &[(&str, &str)]) -> Result<Value, String> + 'a;
struct Edition {
    ark: &'static str,
    billing: &'static str,
    region: &'static str,
    currency: &'static str,
}
const CN: Edition = Edition {
    ark: "ark.cn-beijing.volcengineapi.com",
    billing: "open.volcengineapi.com",
    region: "cn-beijing",
    currency: "CNY",
};
const INTL: Edition = Edition {
    ark: "ark.ap-southeast-1.byteplusapi.com",
    billing: "billing.byteplusapi.com",
    region: "ap-southeast-1",
    currency: "USD",
};

fn hmac(key: &[u8], bytes: &[u8]) -> [u8; 32] {
    let mut block = [0; 64];
    if key.len() > 64 {
        block[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        block[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new()
        .chain_update(block.map(|b| b ^ 0x36))
        .chain_update(bytes)
        .finalize();
    let outer = Sha256::new()
        .chain_update(block.map(|b| b ^ 0x5c))
        .chain_update(inner)
        .finalize()
        .into();
    block.fill(0);
    inner.fill(0);
    outer
}
fn credentials(value: &str) -> Option<(&str, &str)> {
    let (ak, sk) = value.split_once(':')?;
    if ak.len() > 256
        || ak.is_empty()
        || !ak
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        || sk.len() > 512
        || !valid_token(sk)
        || sk.contains(':')
    {
        return None;
    }
    Some((ak, sk))
}
fn call(
    e: &Edition,
    ak: &str,
    sk: &str,
    action: &str,
    bill: bool,
    at: DateTime<Utc>,
    post: &Post<'_>,
) -> Result<Value, String> {
    let (host, service, version) = if bill {
        (e.billing, "billing", "2022-01-01")
    } else {
        (e.ark, "ark", "2024-01-01")
    };
    let query = format!("Action={action}&Version={version}");
    let date = at.format("%Y%m%dT%H%M%SZ").to_string();
    let short = at.format("%Y%m%d").to_string();
    let body = b"{}";
    let hash = format!("{:x}", Sha256::digest(body));
    let signed = "content-type;host;x-content-sha256;x-date";
    let canonical = format!("POST\n/\n{query}\ncontent-type:application/json\nhost:{host}\nx-content-sha256:{hash}\nx-date:{date}\n\n{signed}\n{hash}");
    let scope = format!("{short}/{}/{service}/request", e.region);
    let to_sign = format!(
        "HMAC-SHA256\n{date}\n{scope}\n{:x}",
        Sha256::digest(canonical)
    );
    let mut key = hmac(sk.as_bytes(), short.as_bytes());
    for part in [e.region, service, "request"] {
        let next = hmac(&key, part.as_bytes());
        key.fill(0);
        key = next;
    }
    let signature = hmac(&key, to_sign.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    key.fill(0);
    let auth = format!(
        "HMAC-SHA256 Credential={ak}/{scope}, SignedHeaders={signed}, Signature={signature}"
    );
    let doc = post(
        &format!("https://{host}/?{query}"),
        body,
        &[
            ("Content-Type", "application/json"),
            ("X-Date", &date),
            ("X-Content-Sha256", &hash),
            ("Authorization", &auth),
        ],
    )?;
    if !doc["ResponseMetadata"]["Error"].is_null() {
        return Err("方舟接口拒绝请求".into());
    }
    doc.get("Result")
        .filter(|v| v.is_object())
        .cloned()
        .ok_or_else(|| "未返回可识别的额度".into())
}

pub(super) fn read(
    kind: &str,
    token: &str,
    now: i64,
    post: &Post<'_>,
) -> Result<(Vec<Metric>, String), String> {
    let (ak, sk) = credentials(token).ok_or("方舟 AK/SK 格式错误")?;
    let at = DateTime::<Utc>::from_timestamp(now, 0).ok_or("本机时间不可用")?;
    let e = if kind == "volcengine-ark" { &CN } else { &INTL };
    let mut rows = Vec::new();
    let mut plans = Vec::new();
    let mut errors = Vec::new();
    let mut request = |action, billing| {
        call(e, ak, sk, action, billing, at, post)
            .map_err(|e| errors.push(e))
            .ok()
    };
    if let Some(doc) = request("GetAFPUsage", false) {
        let start = rows.len();
        for (field, label, window) in [
            ("AFPFiveHour", "Agent Plan · 5 小时", Some(300)),
            ("AFPWeekly", "Agent Plan · 每周", Some(10080)),
            ("AFPMonthly", "Agent Plan · 每月", None),
        ] {
            let b = &doc[field];
            if let (Some(total), Some(used)) = (
                num(&b["Quota"])
                    .or_else(|| num(&b["Total"]))
                    .filter(|n| *n > 0.),
                num(&b["Used"]),
            ) {
                rows.push(quota(
                    label,
                    window,
                    100. * used / total,
                    time(&b["ResetTime"]),
                ));
            }
        }
        if rows.len() > start {
            let mut plan = text(&doc["PlanType"]);
            if plan.is_empty() {
                plan = text(&doc["Tier"]);
            }
            plans.push(if plan.is_empty() {
                "Agent Plan".into()
            } else {
                format!("Agent Plan {plan}")
            });
        }
    }
    if let Some(doc) = request("GetCodingPlanUsage", false) {
        let start = rows.len();
        for item in doc["QuotaUsage"].as_array().into_iter().flatten().take(12) {
            let Some(used) = num(&item["Percent"]) else {
                continue;
            };
            let (label, window) = match item["Level"]
                .as_str()
                .map(str::to_ascii_lowercase)
                .as_deref()
            {
                Some("session") => ("Coding Plan · 5 小时", Some(300)),
                Some("weekly") => ("Coding Plan · 每周", Some(10080)),
                Some("monthly") => ("Coding Plan · 每月", None),
                _ => continue,
            };
            rows.push(quota(label, window, used, time(&item["ResetTimestamp"])));
        }
        if rows.len() > start {
            plans.push("Coding Plan".into());
        }
    }
    if let Some(doc) = request("QueryBalanceAcct", true) {
        if let Some(amount) = ["AvailableBalance", "CashBalance", "TotalBalance"]
            .into_iter()
            .find_map(|k| num(&doc[k]))
        {
            // Cloud billing is not a configured relay wallet.
            rows.push(metric("云账户余额", money(amount, e.currency)));
        }
    }
    if rows.is_empty() {
        return Err(errors
            .into_iter()
            .find(|s| matches!(s.as_str(), "HTTP 401" | "HTTP 403" | "HTTP 429"))
            .unwrap_or_else(|| "未返回可识别的额度".into()));
    }
    let mut plan = plans.join(" / ");
    for secret in [token, ak, sk] {
        plan = plan.replace(secret, "••••");
    }
    Ok((rows, plan))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hmac_reference_vector() {
        let out = hmac(&[0x0b; 20], b"Hi There");
        assert_eq!(
            out.iter().map(|b| format!("{b:02x}")).collect::<String>(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
    #[test]
    fn queries_use_edition_domains_and_real_windows() {
        let post = |url: &str, _: &[u8], headers: &[(&str, &str)]| {
            let auth = headers
                .iter()
                .find(|(k, _)| *k == "Authorization")
                .unwrap()
                .1;
            assert!(auth.starts_with("HMAC-SHA256 Credential=AK_01/"));
            assert!(!auth.contains("testsecret"));
            assert!(auth.contains("/ap-southeast-1/"));
            if url.contains("QueryBalanceAcct") {
                assert!(url.starts_with("https://billing.byteplusapi.com/?"));
                Ok(serde_json::json!({"Result":{"AvailableBalance":"0.50"}}))
            } else {
                assert!(url.starts_with("https://ark.ap-southeast-1.byteplusapi.com/?"));
                Ok(if url.contains("GetAFPUsage") {
                    serde_json::json!({"Result":{"AFPFiveHour":{"Quota":100,"Used":60}}})
                } else {
                    serde_json::json!({"Result":{"QuotaUsage":[{"Level":"Weekly","Percent":10}]}})
                })
            }
        };
        let (rows, _) = read("byteplus-ark", "AK_01:testsecret", 1, &post).unwrap();
        assert_eq!(rows[0].remaining, Some(40.));
        assert_eq!(rows[1].window, Some(10080));
        assert_eq!(rows[2].value, "$0.50");
        assert!(rows.iter().all(|m| m.relay_money.is_none()));
        assert!(credentials("ak/escape:secret").is_none());
    }
}
