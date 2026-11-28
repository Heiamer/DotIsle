// SPDX-License-Identifier: Apache-2.0
//! Native port of BrimDeck ZCodeUsage.cs and UsageService.ReadZCodeAsync.
//! Modified: authenticated CNG decryption, bounded reads, no credential renewal or IPC.
use super::{glm, num, read_limit, text, time, valid_token, Metric};
use crate::ai_quota::TokenQuota;
use base64::{
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
    Engine,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};
use windows::{
    core::PCWSTR,
    Win32::Security::{
        Credentials::{CredFree, CredReadW, CRED_TYPE_GENERIC},
        Cryptography::*,
    },
};

fn decode(value: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(value)
        .or_else(|_| URL_SAFE.decode(value))
        .ok()
}
fn digest(value: impl AsRef<[u8]>) -> [u8; 32] {
    Sha256::digest(value).into()
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
struct Alg(BCRYPT_ALG_HANDLE);
impl Drop for Alg {
    fn drop(&mut self) {
        unsafe {
            let _ = BCryptCloseAlgorithmProvider(self.0, 0);
        }
    }
}
struct Key(BCRYPT_KEY_HANDLE);
impl Drop for Key {
    fn drop(&mut self) {
        unsafe {
            let _ = BCryptDestroyKey(self.0);
        }
    }
}
pub(super) fn decrypt(key: &[u8], nonce: &mut [u8], tag: &mut [u8], cipher: &[u8]) -> Option<Vec<u8>> {
    if key.len() != 32 || nonce.len() != 12 || tag.len() != 16 || cipher.len() > 32768 {
        return None;
    }
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        BCryptOpenAlgorithmProvider(
            &mut alg,
            BCRYPT_AES_ALGORITHM,
            PCWSTR::null(),
            BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0),
        )
        .ok()
        .ok()?;
        let alg = Alg(alg);
        let gcm: Vec<u8> = "ChainingModeGCM\0"
            .encode_utf16()
            .flat_map(u16::to_le_bytes)
            .collect();
        BCryptSetProperty(BCRYPT_HANDLE(alg.0 .0), BCRYPT_CHAINING_MODE, &gcm, 0)
            .ok()
            .ok()?;
        let mut handle = BCRYPT_KEY_HANDLE::default();
        BCryptGenerateSymmetricKey(alg.0, &mut handle, None, key, 0)
            .ok()
            .ok()?;
        let handle = Key(handle);
        let info = BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO {
            cbSize: size_of::<BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO>() as u32,
            dwInfoVersion: BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO_VERSION,
            pbNonce: nonce.as_mut_ptr(),
            cbNonce: nonce.len() as u32,
            pbTag: tag.as_mut_ptr(),
            cbTag: tag.len() as u32,
            ..Default::default()
        };
        let mut output = vec![0u8; cipher.len()];
        let mut count = 0;
        if BCryptDecrypt(
            handle.0,
            Some(cipher),
            Some((&info as *const BCRYPT_AUTHENTICATED_CIPHER_MODE_INFO).cast()),
            None,
            Some(&mut output),
            &mut count,
            BCRYPT_FLAGS(0),
        )
        .is_err()
        {
            output.fill(0);
            return None;
        }
        output.truncate(count as usize);
        Some(output)
    }
}
fn os_key(path: &Path) -> Option<[u8; 32]> {
    let path = std::path::absolute(path).ok()?;
    let target = format!(
        "{}.ZCode Credential Encryption",
        hex(&digest(path.to_str()?))
    );
    let wide: Vec<u16> = target.encode_utf16().chain(Some(0)).collect();
    unsafe {
        let mut cred = std::ptr::null_mut();
        CredReadW(PCWSTR(wide.as_ptr()), CRED_TYPE_GENERIC, None, &mut cred).ok()?;
        if cred.is_null() {
            return None;
        }
        let cred_data = &*cred;
        let key = if !cred_data.CredentialBlob.is_null() && cred_data.CredentialBlobSize == 86 {
            let raw = std::slice::from_raw_parts(cred_data.CredentialBlob, 86);
            let mut string = String::from_utf16(
                &raw.chunks_exact(2)
                    .map(|u| u16::from_le_bytes([u[0], u[1]]))
                    .collect::<Vec<_>>(),
            )
            .ok();
            let r = string
                .as_ref()
                .and_then(|s| decode(s))
                .and_then(|b| b.try_into().ok());
            if let Some(text) = string.as_mut() {
                text.clear();
            }
            r
        } else {
            None
        };
        CredFree(cred.cast());
        key
    }
}
fn secret_env(path: &Path) -> Option<[u8; 32]> {
    let mut key = std::env::var("ZCODE_CREDENTIAL_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
        .map(String::into_bytes)
        .or_else(|| {
            let file = std::env::var_os("ZCODE_CREDENTIAL_SECRET_FILE")
                .filter(|p| !p.is_empty())
                .map(PathBuf::from)
                .or_else(|| Some(path.parent()?.join("credential-secret")))?;
            read_limit(&file, 8192)
        })?;
    let result = digest(&key);
    key.fill(0);
    Some(result)
}
fn decrypt_field(value: &str, path: &Path, legacy: &[u8; 32]) -> Option<String> {
    if !value.starts_with("enc:") {
        return Some(value.to_owned());
    }
    let (encoded, mut key) = if let Some(s) = value.strip_prefix("enc:v1:") {
        (s, *legacy)
    } else {
        let mut p = value.strip_prefix("enc:v2:")?.split(':');
        let mut key = match p.next()? {
            "os" => os_key(path)?,
            "env" => secret_env(path)?,
            _ => return None,
        };
        let fingerprint = p.next()?;
        let encoded = p.next()?;
        if hex(&digest(key)).get(..24) != Some(fingerprint) || p.next().is_some() {
            key.fill(0);
            return None;
        }
        (encoded, key)
    };
    let plain = (|| {
        let mut p = encoded.split('.');
        let (mut nonce, mut tag, cipher) =
            (decode(p.next()?)?, decode(p.next()?)?, decode(p.next()?)?);
        if p.next().is_some() {
            return None;
        }
        decrypt(&key, &mut nonce, &mut tag, &cipher)
    })();
    key.fill(0);
    String::from_utf8(plain?).ok()
}
fn token_count(value: &Value) -> Option<u64> {
    value.as_u64().or_else(|| value.as_str()?.parse().ok()).or_else(|| {
        // Preserve integers exactly; only accept a floating representation within the safe integer range.
        let v = num(value)?;
        (v >= 0. && v <= 9_007_199_254_740_991. && v.fract() == 0.).then_some(v as u64)
    })
}

fn start_plan(body: &Value, now: i64) -> (Vec<Metric>, String) {
    if num(&body["code"]) != Some(0.) {
        return (Vec::new(), String::new());
    }
    let doc = &body["data"];
    let mut active = HashMap::new();
    let mut periods = HashMap::new();
    let mut title = String::new();
    for p in doc["plans"].as_array().into_iter().flatten().take(32) {
        if !p["status"]
            .as_str()
            .is_some_and(|s| s.eq_ignore_ascii_case("active"))
            || time(&p["ends_at"]).is_some_and(|e| e <= now)
        {
            continue;
        }
        let Some(id) = p["user_plan_id"].as_str().filter(|s| !s.is_empty()) else {
            continue;
        };
        active.insert(id, ());
        if title.is_empty() {
            title = text(&p["name"]);
        }
        for e in p["entitlements"].as_array().into_iter().flatten().take(64) {
            if let (Some(id), Some(period)) = (e["entitlement_id"].as_str(), e["period"].as_str()) {
                periods.entry(id).or_insert(period);
            }
        }
    }
    let items = doc["balances"]
        .as_array()
        .into_iter()
        .flatten()
        .take(24)
        .filter_map(|r| {
            if !active.contains_key(r["user_plan_id"].as_str()?) {
                return None;
            }
            let end = time(&r["period_end"]);
            let expires = time(&r["expires_at"]).or(end);
            if expires.is_some_and(|e| e <= now) {
                return None;
            }
            let label = if let Some(label) = r["show_name"].as_str().filter(|v| !v.is_empty()) {
                label
            } else {
                r["entitlement_id"].as_str()?
            };
            let recurring = periods
                .get(r["entitlement_id"].as_str().unwrap_or(""))
                .is_some_and(|p| !p.is_empty() && *p != "one_time");
            let unit = text(&r["unit_type"]);
            let is_token = unit.eq_ignore_ascii_case("token") || unit.eq_ignore_ascii_case("tokens");
            let (value, token_quota) = if is_token {
                let total = token_count(&r["total_units"]).filter(|n| *n > 0)?;
                let remaining = token_count(&r["remaining_units"])
                    .filter(|n| *n <= total)
                    .or_else(|| total.checked_sub(token_count(&r["used_units"])?))?;
                (format!("剩余 {remaining} / {total} TOKEN"), Some(TokenQuota { remaining, total }))
            } else {
                let total = num(&r["total_units"]).filter(|v| *v > 0.)?;
                let used = num(&r["used_units"])
                    .or_else(|| Some(total - num(&r["remaining_units"])?))?
                    .clamp(0., total);
                (format!("{used:.0} / {total:.0} {unit}"), None)
            };
            Some(Metric {
                label: if is_token { label.replace("令牌", "TOKEN") } else { label.to_owned() }.chars().filter(|c| !c.is_control()).take(70).collect(),
                value,
                token_quota,
                reset_at: end.filter(|end| recurring && expires.is_some_and(|e| *end < e)),
                expires_at: expires,
                ..Default::default()
            })
        })
        .collect();
    (items, title)
}
fn mcp(body: &Value) -> Option<Metric> {
    if num(&body["code"]) != Some(0.) {
        return None;
    }
    let r = &body["data"];
    let usage = &r["total_usage"];
    let (used, total) = (
        num(&usage["used"])?.max(0.),
        num(&usage["limit"]).filter(|v| *v > 0.)?,
    );
    Some(Metric {
        label: "ZCode MCP".into(),
        value: format!("{used:.0} / {total:.0} 次"),
        reset_at: time(&r["next_refresh_at"]),
        ..Default::default()
    })
}

pub fn read(
    now: i64,
    get: &(impl Fn(&str, &[(&str, &str)]) -> Result<Value, String> + Sync),
) -> Result<(Vec<Metric>, String), String> {
    let home = std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .ok_or("ZCode 未连接")?;
    let tool = std::env::var_os("ZCODE_HOME")
        .filter(|p| !p.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| home.join(".zcode"));
    let path = tool.join("v2/credentials.json");
    let raw = read_limit(&path, 512 * 1024).ok_or("ZCode 未连接")?;
    let store: Value = serde_json::from_slice(&raw).map_err(|_| "ZCode 无法读取凭据")?;
    let username = std::env::var("USERNAME").unwrap_or_default();
    let mut legacy = std::env::var("ZCODE_CREDENTIAL_SECRET")
        .ok()
        .filter(|s| !s.is_empty())
        .map(digest)
        .unwrap_or_else(|| {
            digest(format!(
                "zcode-credential-fallback:win32:{}:{username}",
                home.to_string_lossy()
            ))
        });
    let keys = (|| {
        let field = |name: &str| -> Result<String, String> {
            let v = store[name].as_str().unwrap_or("");
            decrypt_field(v, &path, &legacy)
                .map(|s| s.trim().to_owned())
                .ok_or("ZCode 无法读取凭据".into())
        };
        let hints = store.as_object().ok_or("ZCode 无法读取凭据")?;
        let key_name = |region: &str| {
            hints.keys().find(|key| {
                key.starts_with("account-provider:coding-plan:account:")
                    && key.ends_with(":api-key")
                    && key.contains(&format!(":{region}-individual-coding-plan:"))
            })
        };
        let family = field("oauth:active_provider")?;
        let global = family == "zai"
            || (family != "bigmodel"
                && key_name("bigmodel").is_none()
                && key_name("zai").is_some());
        let family = if global { "zai" } else { "bigmodel" };
        let api = key_name(family)
            .map(|name| field(name))
            .transpose()?
            .unwrap_or_default();
        Ok::<_, String>((
            global,
            api,
            field("zcodejwttoken")?,
            field(&format!("oauth:{family}:access_token"))?,
        ))
    })();
    legacy.fill(0);
    let (global, api, jwt, access) = keys?;
    let jwt = valid_token(&jwt).then(|| format!("Bearer {jwt}"));
    let bearer = valid_token(&access).then(|| format!("Bearer {access}"));
    let coding = if valid_token(&api) {
        Some(api.as_str())
    } else if global {
        bearer.as_deref()
    } else {
        valid_token(&access).then_some(access.as_str())
    };
    let device = read_limit(&tool.join("v2/telemetry-state.json"), 64 * 1024)
        .and_then(|v| serde_json::from_slice::<Value>(&v).ok())
        .map(|v| text(&v["deviceMid"]))
        .filter(|v| v.len() == 36 && v.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-'));
    if coding.is_none() && jwt.is_none() {
        return Err("ZCode 未连接".into());
    }
    let host = if global {
        "https://api.z.ai"
    } else {
        "https://open.bigmodel.cn"
    };
    let (coding, start, mcp) = std::thread::scope(|scope| {
        let one = scope.spawn(|| {
            coding.map(|auth| {
                get(
                    &format!("{host}/api/monitor/usage/quota/limit"),
                    &[("Authorization", auth)],
                )
            })
        });
        let two = scope.spawn(|| {
            jwt.as_deref().zip(device.as_deref()).map(|(jwt, device)| {
                get(
                    "https://zcode.z.ai/api/v1/zcode-plan/billing/balance",
                    &[("Authorization", jwt), ("X-Device-Mid", device)],
                )
            })
        });
        let three = scope.spawn(|| {
            jwt.as_deref().zip(bearer.as_deref()).map(|(jwt, auth)| {
                get(
                    "https://zcode.z.ai/api/v1/mcp/usage",
                    &[
                        ("Authorization", jwt),
                        ("X-Bigmodel-Authorization", auth),
                        ("Bigmodel-Target-Type", "PERSONAL"),
                    ],
                )
            })
        });
        (
            one.join().ok().flatten(),
            two.join().ok().flatten(),
            three.join().ok().flatten(),
        )
    });
    let failure = [&coding, &start, &mcp]
        .into_iter()
        .find_map(|v| v.as_ref().and_then(|v| v.as_ref().err()))
        .cloned();
    let mut plan = String::new();
    let mut rows = Vec::new();
    if let Some(Ok(coding)) = coding {
        rows.extend(glm(&coding, now));
        plan = text(&coding["data"]["level"]);
    }
    if let Some(Ok(start)) = start {
        let (values, title) = start_plan(&start, now);
        rows.extend(values);
        if plan.is_empty() {
            plan = title;
        }
    }
    if let Some(Ok(record)) = mcp {
        rows.extend(self::mcp(&record));
    }
    order_metrics(&mut rows, now);
    if rows.is_empty() {
        if let Some(e) = failure {
            return Err(e);
        }
    }
    for token in [api.as_str(), access.as_str()]
        .into_iter()
        .chain(jwt.as_deref().and_then(|t| t.strip_prefix("Bearer ")))
    {
        if !token.is_empty() {
            plan = plan.replace(token, "••••");
            for row in &mut rows {
                row.label = row.label.replace(token, "••••");
                row.value = row.value.replace(token, "••••");
            }
        }
    }
    Ok((rows, plan))
}

fn order_metrics(rows: &mut [Metric], now: i64) {
    rows.sort_by_key(|metric| {
        let active = metric.reset_at.is_none_or(|at| at > now)
            && metric.expires_at.is_none_or(|at| at > now);
        match (metric.window, metric.remaining, metric.token_quota) {
            (Some(300 | 10080), Some(_), _) if active => 0,
            (_, _, Some(_)) if active => 1,
            _ => 2,
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn start_plan_displays_remaining_token_not_used_or_credential() {
        use serde_json::json;
        let mut input = json!({"code":0,"data":{"plans":[{"user_plan_id":"p","status":"active"}],"balances":[
            {"user_plan_id":"p","entitlement_id":"glm","unit_type":"token","total_units":"9007199254740993","remaining_units":"1200000","used_units":22},
            {"user_plan_id":"p","entitlement_id":"fallback","unit_type":"TOKEN","total_units":9000,"used_units":9000},
            {"user_plan_id":"p","entitlement_id":"points","unit_type":"points","total_units":50,"remaining_units":2},
            {"user_plan_id":"p","entitlement_id":"expired","unit_type":"token","total_units":5,"remaining_units":2,"expires_at":5},
            {"user_plan_id":"p","entitlement_id":"bad","unit_type":"token","total_units":5,"remaining_units":-1}
        ]}});
        let (rows, _) = start_plan(&input, 10);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].value, "剩余 1200000 / 9007199254740993 TOKEN");
        assert_eq!(rows[0].token_quota.unwrap().compact_label(), "1.2M TOKEN");
        assert_eq!(rows[1].value, "剩余 0 / 9000 TOKEN");
        assert!(rows[2].token_quota.is_none());
        assert!(rows.iter().all(|m| m.window.is_none() && m.relay_money.is_none()));
        input["data"]["balances"][0]["remaining_units"] = json!("NaN");
        assert_eq!(start_plan(&input, 10).0[0].token_quota.unwrap().remaining, 9007199254740971);
    }
    #[test]
    fn display_order_prefers_active_windows_including_zero_then_tokens() {
        let mut rows = vec![
            Metric { label: "MCP".into(), ..Default::default() },
            Metric { label: "Token".into(), token_quota: Some(TokenQuota { remaining: 0, total: 10 }), ..Default::default() },
            Metric { label: "Expired".into(), window: Some(300), remaining: Some(50.), reset_at: Some(10), ..Default::default() },
            Metric { label: "Week".into(), window: Some(10080), remaining: Some(0.), ..Default::default() },
        ];
        order_metrics(&mut rows, 10);
        assert_eq!(rows.iter().map(|row| row.label.as_str()).collect::<Vec<_>>(), ["Week", "Token", "MCP", "Expired"]);
        rows.remove(0);
        order_metrics(&mut rows, 10);
        assert_eq!(rows[0].label, "Token");
    }
    fn bytes(s: &str) -> Vec<u8> {
        s.as_bytes()
            .chunks_exact(2)
            .map(|c| u8::from_str_radix(std::str::from_utf8(c).unwrap(), 16).unwrap())
            .collect()
    }
    #[test]
    fn windows_cng_authenticates_before_returning_plaintext() {
        let cipher = bytes("cea7403d4d606b6e074ec5d3baf39d18");
        let mut tag = bytes("d0d1c8a799996bf0265b98b5d48ab9191");
        assert_eq!(
            decrypt(&[0; 32], &mut [0; 12], &mut tag, &cipher),
            Some(vec![0; 16])
        );
        tag[0] ^= 1;
        assert_eq!(decrypt(&[0; 32], &mut [0; 12], &mut tag, &cipher), None);
    }
}

// Compatibility for TRAE's local auth container; callers must verify its digest.
pub(super) fn decrypt_cbc(key: &[u8], iv: &mut [u8;16], cipher: &[u8]) -> Option<Vec<u8>> {
    if key.len() != 16 || cipher.is_empty() || cipher.len() > 32768 { return None; }
    let mut output = vec![0u8; cipher.len()];
    unsafe {
        let mut alg = BCRYPT_ALG_HANDLE::default();
        BCryptOpenAlgorithmProvider(&mut alg, BCRYPT_AES_ALGORITHM, PCWSTR::null(), BCRYPT_OPEN_ALGORITHM_PROVIDER_FLAGS(0)).ok().ok()?;
        let alg = Alg(alg);
        let cbc: Vec<u8> = "ChainingModeCBC\0".encode_utf16().flat_map(u16::to_le_bytes).collect();
        BCryptSetProperty(BCRYPT_HANDLE(alg.0.0), BCRYPT_CHAINING_MODE, &cbc, 0).ok().ok()?;
        let mut handle = BCRYPT_KEY_HANDLE::default();
        BCryptGenerateSymmetricKey(alg.0, &mut handle, None, key, 0).ok().ok()?;
        let handle = Key(handle);
        let mut count = 0;
        if BCryptDecrypt(handle.0,Some(cipher),None,Some(iv),Some(&mut output),&mut count,BCRYPT_BLOCK_PADDING).is_err() {
            output.fill(0);
            return None;
        }
        output.truncate(count as usize);
    }
    Some(output)
}
