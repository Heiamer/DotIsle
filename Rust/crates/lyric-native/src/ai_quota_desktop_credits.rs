// SPDX-License-Identifier: Apache-2.0
//! Native quota-only translations of BrimDeck's QoderUsage, QwenWorkUsage and TraeUsage.
use super::{
    chromium_folder_plain, metric, num, read_limit, redact, text, time, valid_token, Get, Metric,
    Post, ResultSet, Value,
};
use std::path::{Path, PathBuf};

struct Edition {
    folder: &'static str,
    domain: &'static str,
    default: &'static str,
}
const QODER: Edition = Edition {
    folder: "com.qoder.app.stable",
    domain: "qoder.sh",
    default: "https://openapi.qoder.sh",
};
const QODER_CN: Edition = Edition {
    folder: "com.qodercn.app.stable",
    domain: "qoder.com.cn",
    default: "https://openapi.qoder.com.cn",
};
const QWEN: Edition = Edition {
    folder: "QwenWork",
    domain: "qwenwork.ai",
    default: "https://gateway.qwenwork.ai",
};
const QWEN_CN: Edition = Edition {
    folder: "QwenWorkCN",
    domain: "qwenwork.cn",
    default: "https://gateway.qwenwork.cn",
};
const TRAE: Edition = Edition {
    folder: "Trae",
    domain: "trae.ai",
    default: "https://api-sg-central.trae.ai",
};
const TRAE_CN: Edition = Edition {
    folder: "Trae CN",
    domain: "trae.cn",
    default: "https://api.trae.cn",
};
const LOGIN: &str = "未找到本机额度登录";
const EMPTY: &str = "没有可识别的额度数据";

pub(super) fn read(kind: &str, now: i64, get: &Get<'_>, post: &Post<'_>) -> Option<ResultSet> {
    let e = match kind {
        "qoder" => &QODER,
        "qoder-cn" => &QODER_CN,
        "qwenwork" => &QWEN,
        "qwenwork-cn" => &QWEN_CN,
        "trae" => &TRAE,
        "trae-cn" => &TRAE_CN,
        _ => return None,
    };
    let work = || {
        let roaming = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .ok_or(LOGIN)?;
        if kind.starts_with("trae") {
            return trae(&roaming, e, kind == "trae-cn", now, post);
        }
        let qoder = kind.starts_with("qoder");
        let folder = roaming.join(e.folder);
        let name = if qoder { "auth.v1.dat" } else { "auth-v2.dat" };
        let mut plain = chromium_folder_plain(
            &folder,
            read_limit(&folder.join(name), 32 * 1024).ok_or(LOGIN)?,
        )
        .ok_or(LOGIN)?;
        let doc = serde_json::from_slice::<Value>(&plain);
        plain.fill(0);
        let doc = doc.map_err(|_| LOGIN)?;
        if qoder {
            read_qoder(&doc, e, now, get)
        } else {
            read_qwen(&doc, e, now, get)
        }
    };
    Some(work())
}

fn origin<'a>(raw: &'a str, e: &Edition) -> Option<&'a str> {
    let raw = raw.strip_suffix('/').unwrap_or(raw);
    let host = raw.strip_prefix("https://")?;
    if host.len() > 253
        || !host.split('.').all(|part| {
            (1..=63).contains(&part.len())
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return None;
    }
    let host = host.to_ascii_lowercase();
    if host == e.domain
        || host
            .strip_suffix(e.domain)
            .is_some_and(|s| s.ends_with('.'))
    {
        Some(raw)
    } else {
        None
    }
}
fn token<'a>(doc: &'a Value, expiry: &str, now: i64) -> Result<&'a str, String> {
    let token = doc["token"]
        .as_str()
        .filter(|s| valid_token(s))
        .ok_or(LOGIN)?;
    if !doc[expiry].is_null() && time(&doc[expiry]).is_none_or(|t| t <= now.saturating_add(60)) {
        return Err("本机登录已失效，请重新打开对应应用登录".into());
    }
    Ok(token)
}
fn finish(mut rows: Vec<Metric>, mut plan: String, token: &str) -> ResultSet {
    if rows.is_empty() {
        return Err(EMPTY.into());
    }
    // Do not serialize credentials even if a service mistakenly echoes them in a label.
    redact(&mut rows, &mut plan, token);
    Ok((rows, plan))
}
fn credit(
    label: &str,
    unit: &str,
    total: Option<f64>,
    used: Option<f64>,
    left: Option<f64>,
) -> Option<Metric> {
    let n = |x: Option<f64>| x.filter(|v| v.is_finite() && *v >= 0.);
    let (total, used, left) = (n(total).filter(|n| *n > 0.), n(used), n(left));
    let val = match (total, used, left) {
        (Some(t), Some(u), _) => format!("剩余 {:.2} / {t:.2} {unit}", (t - u).max(0.)),
        (Some(t), _, Some(l)) => format!("剩余 {l:.2} / {t:.2} {unit}"),
        (_, _, Some(l)) => format!("剩余 {l:.2} {unit}"),
        (None, Some(u), _) => format!("已用 {u:.2} {unit}"),
        (Some(t), None, None) => format!("总计 {t:.2} {unit}"),
        _ => return None,
    };
    Some(metric(label, val))
}
fn bucket(label: &str, doc: &Value, total: &str) -> Option<Metric> {
    credit(
        label,
        "积分",
        num(&doc[total]),
        num(&doc["used"]),
        num(&doc["remaining"]),
    )
}
fn read_qoder(doc: &Value, e: &Edition, now: i64, get: &Get<'_>) -> ResultSet {
    let token = token(doc, "expiresAt", now)?;
    let context = &doc["authenticationContext"];
    let parsed = context
        .as_str()
        .and_then(|s| serde_json::from_str::<Value>(s).ok());
    let base = parsed
        .as_ref()
        .unwrap_or(context)
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .find_map(|s| origin(s, e))
        .unwrap_or(e.default);
    let auth = format!("Bearer {token}");
    let get = |path: &str| get(&format!("{base}{path}"), &[("Authorization", &auth)]);
    let usage = get("/sash/api/v2/me/usage")?;
    if usage["displayMode"].as_str() != Some("qoder") {
        return Err("此 Qoder 账户的额度格式暂不支持".into());
    }
    let plan = get("/api/v2/user/plan")
        .ok()
        .map(|d| text(&d["plan_tier_name"]))
        .unwrap_or_default();
    finish(qoder_metrics(&usage["qoderUsage"]), plan, token)
}
fn qoder_metrics(usage: &Value) -> Vec<Metric> {
    let mut rows = Vec::new();
    if let Some(mut m) = bucket("套餐积分", &usage["userQuota"], "total") {
        m.expires_at = time(&usage["expiresAt"]);
        rows.push(m);
    }
    rows.extend(bucket("加购积分", &usage["addOnQuota"], "total"));
    rows.extend(bucket("组织资源包", &usage["orgResourcePackage"], "cap"));
    for p in usage["dedicatedResourcePackages"]
        .as_array()
        .into_iter()
        .flatten()
        .take(8)
    {
        if p["available"].as_bool() == Some(false) {
            continue;
        }
        let mut name = p["displayLabels"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|l| l["dimension"] == "title")
            .map(|l| {
                ["zh-CN", "en-US"]
                    .into_iter()
                    .map(|key| text(&l["valueI18n"][key]))
                    .find(|s| !s.is_empty())
                    .unwrap_or_default()
            })
            .unwrap_or_default();
        if name.is_empty() {
            name = text(&p["name"]);
        }
        if let Some(mut m) = bucket(
            if name.is_empty() {
                "专属积分"
            } else {
                &name
            },
            p,
            "total",
        ) {
            m.expires_at = time(&p["expiresAt"]);
            rows.push(m);
        }
    }
    rows
}
fn qwen_data(doc: &Value) -> Result<&Value, String> {
    if !doc.is_object() || doc.get("code").is_some_and(|v| v != "ok") {
        return Err(EMPTY.into());
    }
    Ok(doc.get("data").filter(|v| v.is_object()).unwrap_or(doc))
}
fn read_qwen(doc: &Value, e: &Edition, now: i64, get: &Get<'_>) -> ResultSet {
    if doc["authRealm"] == "premiumIam" {
        return Err("暂不支持企业专属部署".into());
    }
    let token = token(doc, "expiresAt", now)?;
    let base = doc["endpoints"]["apiOrigin"]
        .as_str()
        .and_then(|s| origin(s, e))
        .unwrap_or(e.default);
    let auth = format!("Bearer {token}");
    let get = |path: &str| get(&format!("{base}{path}"), &[("Authorization", &auth)]);
    let context = get("/api/v1/adapter/user/account-context?include=user,plan,quota")?;
    let context = qwen_data(&context)?;
    let plan = &context["plan"];
    let mut rows = Vec::new();
    if plan["is_personal_version"].as_bool() == Some(false)
        || context["user"]["is_biz"].as_bool() == Some(true)
    {
        let usage = get("/api/v2/quota/usage")?;
        let usage = qwen_data(&usage)?;
        let member = usage
            .get("user_quota")
            .filter(|v| v.is_object())
            .unwrap_or(&usage["userQuota"]);
        if let Some(mut m) = bucket("成员积分", member, "total") {
            m.reset_at = get("/api/v1/adapter/biz/settings/enterprise/quota")
                .ok()
                .and_then(|d| time(&qwen_data(&d).ok()?["next_used_reset_at"]));
            rows.push(m);
        }
        rows.extend(credit(
            "企业积分",
            "积分",
            None,
            None,
            num(&usage["org"]["remaining"]),
        ));
    } else {
        rows.push(bucket("全部积分", &context["quota"], "total").ok_or(EMPTY)?);
        if let Ok(doc) = get("/api/v1/adapter/user/wallets") {
            if let Ok(wallet) = qwen_data(&doc) {
                let kinds: Vec<_> = [
                    ("daily_credits", "每日积分"),
                    ("monthly_credits", "每月积分"),
                    ("longterm_credits", "长期积分"),
                ]
                .into_iter()
                .filter_map(|(key, label)| {
                    Some((
                        key,
                        credit(
                            label,
                            "积分",
                            None,
                            None,
                            num(&wallet[key]["total_balance"]).filter(|n| *n > 0.),
                        )?,
                    ))
                })
                .collect();
                if kinds.len() > 1 {
                    rows.extend(kinds.into_iter().map(|(key, mut m)| {
                        if key == "monthly_credits" && plan["is_subscribed"].as_bool() == Some(true)
                        {
                            m.reset_at = time(&plan["next_refresh_date"]);
                        }
                        m
                    }));
                }
                let expiring = &wallet["expiring_soon"];
                if let Some(mut m) = credit(
                    "即将到期积分",
                    "积分",
                    None,
                    None,
                    num(&expiring["total_balance"]).filter(|n| *n > 0.),
                ) {
                    m.expires_at = expiring["wallets"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(|x| time(&x["valid_to"]))
                        .min();
                    rows.push(m);
                }
            }
        }
    }
    finish(rows, text(&plan["name"]), token)
}

const STANDARD_MASK: [u8; 64] = [
    77, 212, 194, 230, 184, 49, 98, 9, 14, 82, 179, 199, 166, 115, 59, 164, 28, 178, 70, 43, 130,
    154, 181, 138, 25, 107, 57, 219, 87, 23, 117, 36, 244, 155, 175, 127, 8, 232, 214, 141, 38,
    167, 46, 55, 193, 169, 90, 47, 31, 5, 165, 24, 146, 174, 242, 148, 151, 50, 182, 42, 56, 170,
    221, 88,
];
const PRIVATE_MASK: [u8; 64] = [
    73, 12, 194, 18, 146, 176, 93, 12, 192, 108, 203, 233, 31, 185, 46, 33, 181, 167, 197, 56, 129,
    29, 200, 94, 39, 169, 110, 40, 16, 78, 28, 246, 156, 113, 102, 239, 171, 35, 40, 7, 57, 13,
    167, 138, 139, 145, 228, 156, 68, 180, 154, 67, 254, 185, 65, 161, 228, 111, 38, 86, 230, 19,
    228, 123,
];

// Use only the IDE's documented storage locations for this edition. Older IDEs
// wrote plain JSON. Modern records are verified before parsing; never rewrite them.
fn trae_sign_in(value: &str) -> Option<Value> {
    use base64::{engine::general_purpose::STANDARD, Engine};
    use sha2::{Digest, Sha512};
    let value = value.trim();
    if value.starts_with('{') {
        return serde_json::from_str(value).ok();
    }
    let raw = STANDARD.decode(value).ok()?;
    if !(54..=32 * 1024).contains(&raw.len()) || (raw.len() - 38) % 16 != 0 {
        return None;
    }
    let mask = match &raw[..6] {
        b"tc\x05\x10\0\0" => STANDARD_MASK,
        [18, 57, 32, 32, 2, 3] => PRIVATE_MASK,
        _ => return None,
    };
    let mut key = Sha512::new()
        .chain_update(Sha512::digest(&raw[6..38]))
        .chain_update(mask)
        .finalize();
    let mut iv: [u8; 16] = key[16..32].try_into().ok()?;
    let body = super::zcode::decrypt_cbc(&key[..16], &mut iv, &raw[38..]);
    key.fill(0);
    iv.fill(0);
    let mut body = body?;
    let doc = (|| {
        let (sum, payload) = body.split_at_checked(64)?;
        let digest = Sha512::digest(payload);
        if digest.iter().zip(sum).fold(0, |acc, (l, r)| acc | (l ^ r)) != 0 {
            return None;
        }
        serde_json::from_slice(payload).ok()
    })();
    body.fill(0);
    doc
}
fn trae(roaming: &Path, e: &Edition, cn: bool, now: i64, post: &Post<'_>) -> ResultSet {
    let solo = if cn { "TRAE SOLO CN" } else { "TRAE SOLO" };
    let mut paths =
        [e.folder, solo].map(|folder| roaming.join(folder).join("User/globalStorage/storage.json"));
    paths.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
    let doc = paths
        .iter()
        .find_map(|p| {
            let data = read_limit(p, 2 * 1024 * 1024)?;
            let storage: Value = serde_json::from_slice(&data).ok()?;
            let sign_in = trae_sign_in(storage["iCubeAuthInfo://icube.cloudide"].as_str()?)?;
            sign_in["token"].as_str().filter(|s| valid_token(s))?;
            Some(sign_in)
        })
        .ok_or(LOGIN)?;
    read_trae(&doc, e, cn, now, post)
}
fn read_trae(doc: &Value, e: &Edition, cn: bool, now: i64, post: &Post<'_>) -> ResultSet {
    let token = token(doc, "expiredAt", now)?;
    if doc["account"]["scope"]
        .as_str()
        .is_some_and(|s| s.eq_ignore_ascii_case("saas"))
    {
        return Err("暂不支持 TRAE 团队或企业账户额度".into());
    }
    let base = doc["host"]
        .as_str()
        .and_then(|s| origin(s, e))
        .unwrap_or(e.default);
    let auth = format!("Cloud-IDE-JWT {token}");
    let (version, body): (_, &[u8]) = if cn {
        (2, br#"{"require_usage":true,"req_source":1}"#)
    } else {
        (1, br#"{"require_usage":true}"#)
    };
    let doc = post(
        &format!("{base}/trae/api/v{version}/pay/ide_user_ent_usage"),
        body,
        &[
            ("Authorization", &auth),
            ("Content-Type", "application/json"),
            ("Accept", "application/json"),
        ],
    )?;
    let (rows, plan) = trae_metrics(&doc, cn)?;
    finish(rows, plan, token)
}
fn trae_type(p: &Value) -> i32 {
    num(&p["entitlement_base_info"]["product_type"])
        .map(|v| v as i32)
        .unwrap_or(-1)
}
fn trae_quota(p: &Value) -> &Value {
    &p["entitlement_base_info"]["quota"]
}
fn trae_reset(p: &Value) -> Option<i64> {
    time(&p["entitlement_base_info"]["end_time"])
        .or_else(|| time(&p["expire_time"]))
        .map(|t| t.saturating_add(1))
}
fn trae_expiry(p: &Value) -> Option<i64> {
    time(&p["expire_time"]).or_else(|| time(&p["entitlement_base_info"]["end_time"]))
}
fn trae_metrics(doc: &Value, cn: bool) -> Result<(Vec<Metric>, String), String> {
    if !doc.is_object() || doc.get("code").is_some_and(|v| num(v) != Some(0.)) {
        return Err(EMPTY.into());
    }
    let packs: Vec<_> = doc["user_entitlement_pack_list"]
        .as_array()
        .ok_or(EMPTY)?
        .iter()
        .filter(|p| p.is_object() && p["is_hide"].as_bool() != Some(true) && trae_type(p) != 3)
        .take(64)
        .collect();
    let primary = [6, 4, 1, 8, 100, 0]
        .into_iter()
        .find_map(|kind| packs.iter().copied().find(|p| trae_type(p) == kind));
    let plan = primary
        .map(|p| match trae_type(p) {
            6 => "Ultra",
            4 => "Pro+",
            1 => "Pro",
            8 => "Lite",
            100 => "Express",
            _ => "Free",
        })
        .unwrap_or_default()
        .to_owned();
    let credits = doc["is_credits_billing"] == true;
    let dollar = doc["is_dollar_usage_billing"] == true
        || !cn
            && !credits
            && packs.iter().any(|p| {
                num(&trae_quota(p)["basic_usage_limit"]).is_some_and(|v| v > 0.)
                    || trae_type(p) == 7
            });
    let mut rows = Vec::new();
    if dollar {
        if let Some(p) = primary {
            let q = trae_quota(p);
            for (label, limit, spent) in [
                ("套餐额度", "basic_usage_limit", "basic_usage_amount"),
                ("赠送额度", "bonus_usage_limit", "bonus_usage_amount"),
            ] {
                if limit.starts_with("bonus") && q["no_bonus_quota"] == true {
                    continue;
                }
                if let Some(mut m) = credit(
                    label,
                    "USD",
                    num(&q[limit]).filter(|v| *v > 0.),
                    num(&p["usage"][spent]),
                    None,
                )
                .filter(|_| num(&q[limit]).is_some_and(|v| v > 0.))
                {
                    m.reset_at = trae_reset(p);
                    rows.push(m);
                }
            }
        }
        let mut top_up = Bucket::default();
        for p in packs.iter().copied().filter(|p| trae_type(p) == 2) {
            if let Some(limit) = num(&trae_quota(p)["basic_usage_limit"]).filter(|v| *v > 0.) {
                top_up.add(
                    limit,
                    num(&p["usage"]["basic_usage_amount"]),
                    None,
                    trae_expiry(p),
                );
            }
        }
        rows.extend(top_up.finish("加油包", "USD"));
        if let Some(used) = packs
            .iter()
            .copied()
            .find(|p| trae_type(p) == 7)
            .and_then(|p| num(&p["usage"]["pay_go_amount"]))
        {
            rows.push(metric("按量计费", format!("已用 {:.2} USD", used.max(0.))));
        }
    } else {
        let credits = credits
            || packs
                .iter()
                .any(|p| num(&trae_quota(p)["credits_limit"]).is_some_and(|v| v != 0.));
        let (limit, spent, unit, total_label) = if credits {
            ("credits_limit", "credits_amount", "积分", "全部积分")
        } else {
            (
                "premium_model_fast_request_limit",
                "premium_model_fast_amount",
                "次",
                "快速请求",
            )
        };
        let mut all = Bucket::default();
        let mut groups: Vec<(String, Bucket)> = Vec::new();
        for p in packs {
            let Some(cap) = num(&trae_quota(p)[limit]).filter(|v| *v != 0.) else {
                continue;
            };
            let used = num(&p["usage"][spent]);
            all.add(cap, used, None, None);
            if credits {
                let sub = matches!(trae_type(p), 6 | 4 | 1 | 8 | 100);
                let name = if sub {
                    "套餐积分".to_owned()
                } else {
                    [&p["group_name"], &p["display_desc"]]
                        .into_iter()
                        .map(text)
                        .find(|s| !s.trim().is_empty())
                        .unwrap_or_else(|| "额外积分".to_owned())
                };
                let grp = match groups.iter_mut().find(|(label, _)| *label == name) {
                    Some((_, bucket)) => bucket,
                    None => {
                        groups.push((name, Bucket::default()));
                        &mut groups.last_mut().expect("inserted").1
                    }
                };
                grp.add(
                    cap,
                    used,
                    sub.then(|| trae_reset(p)).flatten(),
                    (!sub).then(|| trae_expiry(p)).flatten(),
                );
            }
        }
        if !credits {
            all.reset = primary.and_then(trae_reset);
        }
        rows.extend(all.finish(total_label, unit));
        if groups.len() > 1 {
            rows.extend(
                groups
                    .into_iter()
                    .take(8)
                    .filter_map(|(label, b)| b.finish(&label, unit)),
            );
        }
    }
    Ok((rows, plan))
}
#[derive(Default)]
struct Bucket {
    total: f64,
    used: f64,
    unlimited: bool,
    has_usage: bool,
    reset: Option<i64>,
    expires: Option<i64>,
}
impl Bucket {
    fn add(&mut self, limit: f64, used: Option<f64>, reset: Option<i64>, expires: Option<i64>) {
        self.unlimited |= limit < 0.;
        self.total += limit.max(0.);
        self.has_usage |= used.is_some();
        self.used += used.unwrap_or(0.).max(0.);
        self.reset = self.reset.into_iter().chain(reset).min();
        self.expires = self.expires.into_iter().chain(expires).min();
    }
    fn finish(self, label: &str, unit: &str) -> Option<Metric> {
        let mut m = if self.unlimited {
            metric(
                label,
                if self.has_usage {
                    format!("已用 {:.2} {unit} / 不限量", self.used)
                } else {
                    "不限量".into()
                },
            )
        } else if self.total > 0. {
            credit(
                label,
                unit,
                Some(self.total),
                self.has_usage.then_some(self.used),
                None,
            )?
        } else {
            return None;
        };
        m.reset_at = self.reset;
        m.expires_at = self.expires;
        Some(m)
    }
}
