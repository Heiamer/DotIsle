use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// AI 额度与中转站查询渠道类型（严格不包含任何 Gemini / Google 相关渠道）。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    /// 通用中转站：支持 NewAPI / OneAPI / Veloera / RixAPI（`/api/user/self` 与 `/v1/dashboard/billing/*` 双路径）。
    OneApi,
    /// DeepSeek 官方接口（`/user/balance`）。
    DeepSeek,
    /// OpenRouter 聚合平台（`/api/v1/credits` + `/api/v1/key` 双端点回退）。
    OpenRouter,
    /// 硅基流动 SiliconFlow（`/v1/user/info`）。
    SiliconFlow,
    /// 月之暗面 Kimi / Moonshot（`/v1/users/me/balance`）。
    Moonshot,
    /// 本地 Claude Code OAuth 用量探测（读取 `~/.claude/.credentials.json`，访问 `/api/oauth/usage`）。
    ClaudeCode,
    /// 本地 OpenAI Codex 用量探测（读取 `~/.codex/auth.json`，访问 `chatgpt.com/backend-api/wham/usage`）。
    Codex,
    /// 手动追踪套餐或包月额度。
    Manual,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub name: String,
    pub kind: ProviderKind,
    #[serde(default)]
    pub base_url: String,
    #[serde(default)]
    pub api_key: String,
    /// 站点用户 ID：NewAPI/Veloera/RixAPI `New-Api-User` 请求头，或 Codex `ChatGPT-Account-Id`。
    #[serde(default)]
    pub user_id: String,
    /// 中转站 `/api/user/self` 额度换算单位（默认自动请求 `/api/status` 读取，回退 `500000.0`）。
    #[serde(default)]
    pub quota_per_unit: Option<f64>,
    #[serde(default = "default_currency")]
    pub currency: String,
    #[serde(default)]
    pub total: Option<f64>,
    #[serde(default)]
    pub used: Option<f64>,
    #[serde(default)]
    pub reset_note: String,
    #[serde(default)]
    pub tag: String,
    #[serde(default)]
    pub color: String,
}

fn default_currency() -> String {
    "USD".into()
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct AiQuotaConfig {
    #[serde(default)]
    pub providers: Vec<ProviderConfig>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QuotaEntry {
    pub name: String,
    pub tag: String,
    pub remaining_text: String,
    pub detail_text: String,
    pub ratio: f32,
    pub color_rgb: (u8, u8, u8),
    pub live: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct QuotaSnapshot {
    pub entries: Vec<QuotaEntry>,
    pub updated_label: String,
    pub is_demo: bool,
    pub refreshing: bool,
}

pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("ai-tokens.json")
}

fn resolve_color(name: &str, kind: ProviderKind) -> (u8, u8, u8) {
    match name.trim().to_ascii_lowercase().as_str() {
        "blue" => (0, 122, 255),
        "indigo" => (88, 86, 214),
        "green" => (52, 199, 89),
        "orange" => (255, 149, 0),
        "purple" => (175, 82, 222),
        "teal" => (48, 176, 199),
        "red" | "pink" => (255, 59, 48),
        _ => match kind {
            ProviderKind::OneApi => (0, 122, 255),
            ProviderKind::DeepSeek => (88, 86, 214),
            ProviderKind::OpenRouter => (48, 176, 199),
            ProviderKind::SiliconFlow => (175, 82, 222),
            ProviderKind::Moonshot => (52, 199, 89),
            ProviderKind::ClaudeCode => (255, 149, 0),
            ProviderKind::Codex => (16, 163, 127),
            ProviderKind::Manual => (255, 149, 0),
        },
    }
}

fn default_tag(cfg: &ProviderConfig) -> String {
    if !cfg.tag.trim().is_empty() {
        return cfg.tag.trim().to_string();
    }
    match cfg.kind {
        ProviderKind::OneApi => "中转站".into(),
        ProviderKind::ClaudeCode | ProviderKind::Codex => "OAuth".into(),
        ProviderKind::Manual => "套餐".into(),
        _ => "官方".into(),
    }
}

fn format_money(currency: &str, val: f64) -> String {
    match currency.trim().to_ascii_uppercase().as_str() {
        "CNY" | "RMB" | "¥" => format!("¥{val:.2}"),
        "%" => format!("{val:.0}%"),
        _ => format!("${val:.2}"),
    }
}

/// 演示套餐快照（严格屏蔽任何 Gemini 相关条目）。
pub fn demo_snapshot() -> QuotaSnapshot {
    QuotaSnapshot {
        entries: vec![
            QuotaEntry {
                name: "NewAPI 中转".into(),
                tag: "中转站".into(),
                remaining_text: "$38.40".into(),
                detail_text: "共 $50.00".into(),
                ratio: 0.768,
                color_rgb: (0, 122, 255),
                live: false,
                error: None,
            },
            QuotaEntry {
                name: "DeepSeek".into(),
                tag: "官方".into(),
                remaining_text: "¥84.20".into(),
                detail_text: "共 ¥100.00".into(),
                ratio: 0.842,
                color_rgb: (88, 86, 214),
                live: false,
                error: None,
            },
            QuotaEntry {
                name: "OpenRouter".into(),
                tag: "中转站".into(),
                remaining_text: "$14.60".into(),
                detail_text: "共 $20.00".into(),
                ratio: 0.73,
                color_rgb: (48, 176, 199),
                live: false,
                error: None,
            },
            QuotaEntry {
                name: "Claude Code".into(),
                tag: "OAuth".into(),
                remaining_text: "67%".into(),
                detail_text: "5h 剩 67% · 7d 剩 87%".into(),
                ratio: 0.67,
                color_rgb: (255, 149, 0),
                live: false,
                error: None,
            },
            QuotaEntry {
                name: "硅基流动".into(),
                tag: "官方".into(),
                remaining_text: "¥42.50".into(),
                detail_text: "按量余额".into(),
                ratio: 0.85,
                color_rgb: (175, 82, 222),
                live: false,
                error: None,
            },
        ],
        updated_label: "演示 · 点⚙配中转站".into(),
        is_demo: true,
        refreshing: false,
    }
}

/// 默认配置模板（支持 NewAPI `user_id`、OpenRouter、DeepSeek、硅基流动、Kimi 与本地 OAuth 探针；严禁包含任何 Gemini / Google 条目）。
pub fn default_config_template() -> AiQuotaConfig {
    AiQuotaConfig {
        providers: vec![
            ProviderConfig {
                name: "NewAPI 中转站".into(),
                kind: ProviderKind::OneApi,
                base_url: "https://api.example-relay.com".into(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: Some(500_000.0),
                currency: "USD".into(),
                total: Some(50.0),
                used: None,
                reset_note: "填 AccessToken+user_id 或 sk-".into(),
                tag: "中转站".into(),
                color: "blue".into(),
            },
            ProviderConfig {
                name: "DeepSeek".into(),
                kind: ProviderKind::DeepSeek,
                base_url: "https://api.deepseek.com".into(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: None,
                currency: "CNY".into(),
                total: Some(100.0),
                used: None,
                reset_note: "官方余额".into(),
                tag: "官方".into(),
                color: "indigo".into(),
            },
            ProviderConfig {
                name: "OpenRouter".into(),
                kind: ProviderKind::OpenRouter,
                base_url: "https://openrouter.ai".into(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: None,
                currency: "USD".into(),
                total: None,
                used: None,
                reset_note: "支持管理Key与普通sk-or".into(),
                tag: "聚合".into(),
                color: "teal".into(),
            },
            ProviderConfig {
                name: "硅基流动".into(),
                kind: ProviderKind::SiliconFlow,
                base_url: "https://api.siliconflow.cn".into(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: None,
                currency: "CNY".into(),
                total: Some(50.0),
                used: None,
                reset_note: "官方余额".into(),
                tag: "官方".into(),
                color: "purple".into(),
            },
            ProviderConfig {
                name: "Kimi / Moonshot".into(),
                kind: ProviderKind::Moonshot,
                base_url: "https://api.moonshot.cn".into(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: None,
                currency: "CNY".into(),
                total: Some(50.0),
                used: None,
                reset_note: "官方余额".into(),
                tag: "官方".into(),
                color: "green".into(),
            },
            ProviderConfig {
                name: "Claude Code".into(),
                kind: ProviderKind::ClaudeCode,
                base_url: String::new(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: None,
                currency: "%".into(),
                total: None,
                used: None,
                reset_note: "自动读取本机 .claude 凭据".into(),
                tag: "OAuth".into(),
                color: "orange".into(),
            },
            ProviderConfig {
                name: "Codex".into(),
                kind: ProviderKind::Codex,
                base_url: String::new(),
                api_key: String::new(),
                user_id: String::new(),
                quota_per_unit: None,
                currency: "%".into(),
                total: None,
                used: None,
                reset_note: "自动读取本机 .codex 凭据".into(),
                tag: "OAuth".into(),
                color: "green".into(),
            },
        ],
    }
}

/// 确保 `{data_dir}/ai-tokens.json` 存在，不存在时写入默认模板。
pub fn ensure_config_file(data_dir: &Path) -> Result<PathBuf, String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = config_path(data_dir);
    if !path.exists() {
        let bytes =
            serde_json::to_vec_pretty(&default_config_template()).map_err(|e| e.to_string())?;
        std::fs::write(&path, bytes).map_err(|e| e.to_string())?;
    }
    Ok(path)
}

pub fn load_config(data_dir: &Path) -> Option<AiQuotaConfig> {
    let path = config_path(data_dir);
    let bytes = std::fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// 参考 CodexBar：从指定用户目录读取 `.claude/.credentials.json` 中的 OAuth AccessToken。
pub fn extract_claude_oauth_token_from_home(home: &Path) -> Option<String> {
    let raw = std::fs::read_to_string(home.join(".claude").join(".credentials.json")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    v["claudeAiOauth"]["accessToken"]
        .as_str()
        .or_else(|| v["accessToken"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned)
}

/// 参考 CodexBar：从指定用户目录读取 `.codex/auth.json` 中的 `access_token` 与可选 `account_id`。
pub fn extract_codex_auth_from_home(home: &Path) -> Option<(String, Option<String>)> {
    let raw = std::fs::read_to_string(home.join(".codex").join("auth.json")).ok()?;
    let v: Value = serde_json::from_str(&raw).ok()?;
    let token = v["tokens"]["access_token"]
        .as_str()
        .or_else(|| v["access_token"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())?
        .to_string();
    let account_id = v["tokens"]["account_id"]
        .as_str()
        .or_else(|| v["account_id"].as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToOwned::to_owned);
    Some((token, account_id))
}

pub fn has_configured_live_provider(data_dir: &Path) -> bool {
    let Some(cfg) = load_config(data_dir) else {
        return false;
    };
    let home = user_home_dir();
    cfg.providers.iter().any(|p| match p.kind {
        ProviderKind::Manual => false,
        ProviderKind::ClaudeCode => {
            !p.api_key.trim().is_empty()
                || home
                    .as_deref()
                    .and_then(extract_claude_oauth_token_from_home)
                    .is_some()
        }
        ProviderKind::Codex => {
            !p.api_key.trim().is_empty()
                || home
                    .as_deref()
                    .and_then(extract_codex_auth_from_home)
                    .is_some()
        }
        _ => !p.api_key.trim().is_empty(),
    })
}

pub fn build_manual(cfg: &ProviderConfig) -> QuotaEntry {
    let total = cfg.total.unwrap_or(100.0).max(0.01);
    let used = cfg.used.unwrap_or(0.0).clamp(0.0, total);
    let rem = (total - used).max(0.0);
    let ratio = (rem / total) as f32;
    let remaining_text = if cfg.currency.trim() == "%" {
        format!("{:.0}%", ratio * 100.)
    } else {
        format_money(&cfg.currency, rem)
    };
    let detail_text = if !cfg.reset_note.trim().is_empty() {
        cfg.reset_note.trim().to_string()
    } else if cfg.currency.trim() == "%" {
        "手动追踪".into()
    } else {
        format!("共 {}", format_money(&cfg.currency, total))
    };
    QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text,
        detail_text,
        ratio: ratio.clamp(0.0, 1.0),
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: false,
        error: None,
    }
}

fn json_f64(v: &Value) -> Option<f64> {
    v.as_f64()
        .or_else(|| v.as_str().and_then(|s| s.trim().parse::<f64>().ok()))
}

/// 解析 DeepSeek `/user/balance` JSON。
pub fn parse_deepseek(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value = serde_json::from_str(json).map_err(|_| "DeepSeek JSON 格式错误".to_string())?;
    let infos = v["balance_infos"]
        .as_array()
        .ok_or_else(|| "缺少 balance_infos".to_string())?;
    let item = infos
        .iter()
        .find(|x| x["currency"].as_str() == Some("CNY"))
        .or_else(|| infos.first())
        .ok_or_else(|| "余额信息为空".to_string())?;
    let rem = json_f64(&item["total_balance"]).ok_or_else(|| "缺少 total_balance".to_string())?;
    let currency = item["currency"].as_str().unwrap_or(&cfg.currency);
    let total = cfg.total.filter(|t| *t > 0.0).unwrap_or(rem.max(10.0));
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format_money(currency, rem),
        detail_text: if cfg.total.is_some() {
            format!("共 {}", format_money(currency, total))
        } else {
            "按量计费".into()
        },
        ratio: (rem / total).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 解析 OpenRouter `/api/v1/credits` 管理端点 JSON。
pub fn parse_openrouter(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| "OpenRouter JSON 格式错误".to_string())?;
    let data = &v["data"];
    let total = json_f64(&data["total_credits"]).ok_or_else(|| "缺少 total_credits".to_string())?;
    let used = json_f64(&data["total_usage"]).unwrap_or(0.0);
    let rem = (total - used).max(0.0);
    let denom = cfg.total.filter(|t| *t > 0.0).unwrap_or(total.max(1.0));
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format_money("USD", rem),
        detail_text: format!("共 {}", format_money("USD", total)),
        ratio: (rem / denom).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 解析 OpenRouter `/api/v1/key` 普通 `sk-or-v1-*` Key 元数据与消费限额。
pub fn parse_openrouter_key(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| "OpenRouter Key JSON 格式错误".to_string())?;
    let data = v.get("data").filter(|d| d.is_object()).unwrap_or(&v);
    let limit = json_f64(&data["limit"]);
    let limit_remaining = json_f64(&data["limit_remaining"]);
    let usage = json_f64(&data["usage"])
        .or_else(|| json_f64(&data["usage_monthly"]))
        .unwrap_or(0.0);
    let reset = data["limit_reset"].as_str().unwrap_or("").trim();
    let reset_cn = match reset {
        "daily" => " · 日重置",
        "weekly" => " · 周重置",
        "monthly" => " · 月重置",
        _ => "",
    };
    let (remaining_text, detail_text, ratio) = if let Some(lim) = limit.filter(|l| *l > 0.0) {
        let rem = limit_remaining.unwrap_or((lim - usage).max(0.0)).max(0.0);
        (
            format_money("USD", rem),
            format!("限额 {}{reset_cn}", format_money("USD", lim)),
            (rem / lim).clamp(0.0, 1.0) as f32,
        )
    } else if let Some(rem) = limit_remaining {
        (
            format_money("USD", rem.max(0.0)),
            format!("已用 {}{reset_cn}", format_money("USD", usage)),
            0.85_f32,
        )
    } else {
        (
            format!("已用 {}", format_money("USD", usage)),
            format!("无限额 Key{reset_cn}"),
            0.88_f32,
        )
    };
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text,
        detail_text,
        ratio,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 解析 SiliconFlow `/v1/user/info` JSON。
pub fn parse_siliconflow(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| "硅基流动 JSON 格式错误".to_string())?;
    let data = &v["data"];
    let rem = json_f64(&data["totalBalance"])
        .or_else(|| json_f64(&data["balance"]))
        .ok_or_else(|| "缺少 totalBalance".to_string())?;
    let total = cfg.total.filter(|t| *t > 0.0).unwrap_or(rem.max(10.0));
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format_money("CNY", rem),
        detail_text: if cfg.total.is_some() {
            format!("共 {}", format_money("CNY", total))
        } else {
            "官方余额".into()
        },
        ratio: (rem / total).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 解析 Moonshot (Kimi) `/v1/users/me/balance` JSON。
pub fn parse_moonshot(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value = serde_json::from_str(json).map_err(|_| "Kimi JSON 格式错误".to_string())?;
    let data = &v["data"];
    let rem = json_f64(&data["available_balance"])
        .or_else(|| json_f64(&data["cash_balance"]))
        .ok_or_else(|| "缺少 available_balance".to_string())?;
    let total = cfg.total.filter(|t| *t > 0.0).unwrap_or(rem.max(10.0));
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format_money("CNY", rem),
        detail_text: if cfg.total.is_some() {
            format!("共 {}", format_money("CNY", total))
        } else {
            "官方余额".into()
        },
        ratio: (rem / total).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 参考 qixing-jk/all-api-hub 与 QuantumNous/new-api：解析 `/api/user/self` 站点余额与已用额度。
pub fn parse_newapi_user_self(
    json: &str,
    quota_per_unit: f64,
    cfg: &ProviderConfig,
) -> Result<QuotaEntry, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| "站点 /api/user/self JSON 格式错误".to_string())?;
    if v.get("success").and_then(Value::as_bool) == Some(false) {
        let msg = v["message"].as_str().unwrap_or("站点鉴权失败");
        return Err(msg.to_string());
    }
    let data = &v["data"];
    let raw_quota = json_f64(&data["quota"]).ok_or_else(|| "缺少 data.quota".to_string())?;
    let raw_used = json_f64(&data["used_quota"]).unwrap_or(0.0).max(0.0);
    let per_unit = cfg
        .quota_per_unit
        .filter(|u| *u > 0.0)
        .unwrap_or(quota_per_unit)
        .max(1.0);
    let rem = (raw_quota / per_unit).max(0.0);
    let used = raw_used / per_unit;
    let denom = cfg
        .total
        .filter(|t| *t > 0.0)
        .unwrap_or((rem + used).max(0.01));
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format_money(&cfg.currency, rem),
        detail_text: format!("已用 {}", format_money(&cfg.currency, used)),
        ratio: (rem / denom).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 解析通用 OneAPI / NewAPI 中转站 `/v1/dashboard/billing/subscription` 与 `/usage` JSON。
pub fn parse_oneapi(
    sub_json: &str,
    usage_json: Option<&str>,
    cfg: &ProviderConfig,
) -> Result<QuotaEntry, String> {
    let sub: Value =
        serde_json::from_str(sub_json).map_err(|_| "中转站 JSON 格式错误".to_string())?;
    let unlimited = sub["has_payment_method"].as_bool() == Some(false)
        && json_f64(&sub["hard_limit_usd"]).is_some_and(|h| h >= 100_000_000.0);
    let hard_limit = json_f64(&sub["hard_limit_usd"])
        .or_else(|| json_f64(&sub["system_hard_limit_usd"]))
        .or(cfg.total)
        .unwrap_or(0.0);
    let used_usd = usage_json
        .and_then(|u| serde_json::from_str::<Value>(u).ok())
        .and_then(|u| json_f64(&u["total_usage"]))
        .map(|cents| cents / 100.0)
        .unwrap_or(0.0);

    let (remaining_text, detail_text, ratio) = if unlimited {
        (
            format!("已用 {}", format_money(&cfg.currency, used_usd)),
            "无限额中转".into(),
            0.88_f32,
        )
    } else {
        let rem = (hard_limit - used_usd).max(0.0);
        let denom = hard_limit.max(0.01);
        (
            format_money(&cfg.currency, rem),
            format!("共 {}", format_money(&cfg.currency, hard_limit)),
            (rem / denom).clamp(0.0, 1.0) as f32,
        )
    };
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text,
        detail_text,
        ratio,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 参考 CodexBar：解析 Anthropic `/api/oauth/usage` 的 5h 与 7d 窗口利用率。
pub fn parse_claude_oauth_usage(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| "Claude OAuth JSON 格式错误".to_string())?;
    let five_used = json_f64(&v["five_hour"]["utilization"]);
    let seven_used = json_f64(&v["seven_day"]["utilization"]);
    if five_used.is_none() && seven_used.is_none() {
        return Err("缺少 five_hour/seven_day 字段".into());
    }
    let rem_5h = five_used.map(|u| (100.0 - u).clamp(0.0, 100.0));
    let rem_7d = seven_used.map(|u| (100.0 - u).clamp(0.0, 100.0));
    let primary_rem = rem_5h.or(rem_7d).unwrap_or(0.0);
    let detail = match (rem_5h, rem_7d) {
        (Some(h5), Some(d7)) => format!("5h 剩 {h5:.0}% · 7d 剩 {d7:.0}%"),
        (Some(h5), None) => format!("5h 窗口剩 {h5:.0}%"),
        (None, Some(d7)) => format!("7d 周窗剩 {d7:.0}%"),
        (None, None) => unreachable!(),
    };
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format!("{primary_rem:.0}%"),
        detail_text: detail,
        ratio: (primary_rem / 100.0).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

/// 参考 CodexBar：解析 `https://chatgpt.com/backend-api/wham/usage` 的主/副限额窗口。
pub fn parse_codex_wham_usage(json: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    let v: Value =
        serde_json::from_str(json).map_err(|_| "Codex wham JSON 格式错误".to_string())?;
    let rl = v.get("rate_limit").filter(|x| x.is_object()).unwrap_or(&v);
    let prim_used = json_f64(&rl["primary_window"]["used_percent"])
        .or_else(|| json_f64(&rl["used_percent"]));
    let sec_used = json_f64(&rl["secondary_window"]["used_percent"]);
    if prim_used.is_none() && sec_used.is_none() {
        return Err("缺少 primary_window.used_percent".into());
    }
    let rem_p = prim_used.map(|u| (100.0 - u).clamp(0.0, 100.0));
    let rem_s = sec_used.map(|u| (100.0 - u).clamp(0.0, 100.0));
    let primary_rem = rem_p.or(rem_s).unwrap_or(0.0);
    let detail = match (rem_p, rem_s) {
        (Some(p), Some(s)) => format!("5h 剩 {p:.0}% · 周剩 {s:.0}%"),
        (Some(p), None) => format!("当前窗口剩 {p:.0}%"),
        (None, Some(s)) => format!("周窗口剩 {s:.0}%"),
        (None, None) => unreachable!(),
    };
    Ok(QuotaEntry {
        name: cfg.name.clone(),
        tag: default_tag(cfg),
        remaining_text: format!("{primary_rem:.0}%"),
        detail_text: detail,
        ratio: (primary_rem / 100.0).clamp(0.0, 1.0) as f32,
        color_rgb: resolve_color(&cfg.color, cfg.kind),
        live: true,
        error: None,
    })
}

struct WinHttpHandle(*mut std::ffi::c_void);
impl Drop for WinHttpHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                let _ = windows::Win32::Networking::WinHttp::WinHttpCloseHandle(self.0);
            }
        }
    }
}

/// 扩展版原生 Windows WinHTTP HTTPS GET：支持任意自定义请求头、自定义 User-Agent 与细分状态码报错。
fn http_get_json_ext(
    url: &str,
    bearer: Option<&str>,
    extra_headers: &[(&str, &str)],
    user_agent: Option<&str>,
) -> Result<String, String> {
    use windows::{
        Win32::Networking::WinHttp::{
            URL_COMPONENTS, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, WINHTTP_FLAG_SECURE,
            WINHTTP_QUERY_FLAG_NUMBER, WINHTTP_QUERY_STATUS_CODE, WinHttpConnect, WinHttpCrackUrl,
            WinHttpOpen, WinHttpOpenRequest, WinHttpQueryDataAvailable, WinHttpQueryHeaders,
            WinHttpReadData, WinHttpReceiveResponse, WinHttpSendRequest, WinHttpSetTimeouts,
        },
        core::PCWSTR,
    };

    let wide_url: Vec<u16> = url.encode_utf16().chain(Some(0)).collect();
    let mut host_buf = [0u16; 256];
    let mut path_buf = [0u16; 2048];
    let mut uc = URL_COMPONENTS {
        dwStructSize: std::mem::size_of::<URL_COMPONENTS>() as u32,
        lpszHostName: windows::core::PWSTR(host_buf.as_mut_ptr()),
        dwHostNameLength: host_buf.len() as u32,
        lpszUrlPath: windows::core::PWSTR(path_buf.as_mut_ptr()),
        dwUrlPathLength: path_buf.len() as u32,
        ..Default::default()
    };
    unsafe {
        WinHttpCrackUrl(&wide_url[..wide_url.len() - 1], 0, &mut uc)
            .map_err(|_| "中转站 URL 无效".to_string())?;
    }
    let host: Vec<u16> = host_buf[..uc.dwHostNameLength as usize]
        .iter()
        .copied()
        .chain(Some(0))
        .collect();
    let path: Vec<u16> = if uc.dwUrlPathLength == 0 {
        "/\0".encode_utf16().collect()
    } else {
        path_buf[..uc.dwUrlPathLength as usize]
            .iter()
            .copied()
            .chain(Some(0))
            .collect()
    };
    let is_https = uc.nScheme.0 == 2; // INTERNET_SCHEME_HTTPS

    let ua = user_agent.unwrap_or("AllegroSkia/1.1");
    let agent: Vec<u16> = format!("{ua}\0").encode_utf16().collect();
    let session = WinHttpHandle(unsafe {
        WinHttpOpen(
            PCWSTR(agent.as_ptr()),
            WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY,
            PCWSTR::null(),
            PCWSTR::null(),
            0,
        )
    });
    if session.0.is_null() {
        return Err("WinHTTP 初始化失败".into());
    }
    unsafe {
        let _ = WinHttpSetTimeouts(session.0, 5000, 5000, 5000, 5000);
    }
    let conn = WinHttpHandle(unsafe { WinHttpConnect(session.0, PCWSTR(host.as_ptr()), uc.nPort, 0) });
    if conn.0.is_null() {
        return Err("连接中转站失败".into());
    }
    let method: Vec<u16> = "GET\0".encode_utf16().collect();
    let req = WinHttpHandle(unsafe {
        WinHttpOpenRequest(
            conn.0,
            PCWSTR(method.as_ptr()),
            PCWSTR(path.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            std::ptr::null_mut(),
            if is_https {
                WINHTTP_FLAG_SECURE
            } else {
                windows::Win32::Networking::WinHttp::WINHTTP_OPEN_REQUEST_FLAGS(0)
            },
        )
    });
    if req.0.is_null() {
        return Err("创建请求失败".into());
    }

    let mut hdrs_str = String::from("Accept: application/json\r\n");
    if let Some(tok) = bearer.filter(|s| !s.trim().is_empty()) {
        hdrs_str.push_str(&format!("Authorization: Bearer {}\r\n", tok.trim()));
    }
    for (k, v) in extra_headers {
        if !k.trim().is_empty() && !v.trim().is_empty() {
            hdrs_str.push_str(&format!("{}: {}\r\n", k.trim(), v.trim()));
        }
    }
    let hdrs: Vec<u16> = hdrs_str.encode_utf16().collect();
    unsafe {
        WinHttpSendRequest(req.0, Some(&hdrs), None, 0, 0, 0)
            .map_err(|_| "请求超时或网络不可达".to_string())?;
        WinHttpReceiveResponse(req.0, std::ptr::null_mut())
            .map_err(|_| "接收响应失败".to_string())?;
    }

    let mut status: u32 = 0;
    let mut status_len: u32 = std::mem::size_of::<u32>() as u32;
    unsafe {
        let _ = WinHttpQueryHeaders(
            req.0,
            WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
            PCWSTR::null(),
            Some((&mut status as *mut u32).cast()),
            &mut status_len,
            std::ptr::null_mut(),
        );
    }
    if status == 401 || status == 403 {
        return Err("Key 或凭据无效(401)".into());
    }
    if status == 429 {
        return Err("触发接口限流(429)".into());
    }
    if status == 404 {
        return Err("端点不支持(404)".into());
    }
    if status >= 400 {
        return Err(format!("HTTP {status}"));
    }

    let mut body = Vec::with_capacity(2048);
    loop {
        let mut avail: u32 = 0;
        unsafe {
            WinHttpQueryDataAvailable(req.0, &mut avail)
                .map_err(|_| "读取响应失败".to_string())?;
        }
        if avail == 0 {
            break;
        }
        let chunk = (avail as usize).min(64 * 1024);
        if body.len() + chunk > 1024 * 1024 {
            return Err("响应过大".into());
        }
        let old_len = body.len();
        body.resize(old_len + chunk, 0);
        let mut read: u32 = 0;
        unsafe {
            WinHttpReadData(req.0, body[old_len..].as_mut_ptr().cast(), chunk as u32, &mut read)
                .map_err(|_| "读取数据流失败".to_string())?;
        }
        body.truncate(old_len + read as usize);
        if read == 0 {
            break;
        }
    }
    String::from_utf8(body).map_err(|_| "响应非 UTF-8".into())
}

fn http_get_json(url: &str, bearer: &str) -> Result<String, String> {
    http_get_json_ext(url, Some(bearer), &[], None)
}

static OAUTH_CACHE: OnceLock<Mutex<HashMap<String, (Instant, QuotaEntry)>>> = OnceLock::new();
const OAUTH_COOLDOWN: Duration = Duration::from_secs(300);

fn oauth_cache_get(cache_key: &str) -> Option<QuotaEntry> {
    let map = OAUTH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let guard = map.lock().ok()?;
    guard
        .get(cache_key)
        .filter(|(t, _)| t.elapsed() < OAUTH_COOLDOWN)
        .map(|(_, e)| e.clone())
}

fn oauth_cache_put(cache_key: &str, entry: &QuotaEntry) {
    let map = OAUTH_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mut guard) = map.lock() {
        guard.insert(cache_key.to_string(), (Instant::now(), entry.clone()));
    }
}

fn ninety_days_ago_ymd() -> (String, String) {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    let end = format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay);
    // 粗略向前推 3 个月做 OneAPI usage 窗口统计
    let mut y = st.wYear as i32;
    let mut m = st.wMonth as i32 - 3;
    if m <= 0 {
        m += 12;
        y -= 1;
    }
    let start = format!("{y:04}-{m:02}-01");
    (start, end)
}

fn query_oneapi_site(base: &str, token: &str, cfg: &ProviderConfig) -> Result<QuotaEntry, String> {
    // 1. 尝试从 /api/status 获取站点的 quota_per_unit（失败按 NewAPI 默认 500000）
    let per_unit = cfg.quota_per_unit.unwrap_or_else(|| {
        http_get_json_ext(&format!("{base}/api/status"), None, &[], None)
            .ok()
            .and_then(|s| serde_json::from_str::<Value>(&s).ok())
            .and_then(|v| json_f64(&v["data"]["quota_per_unit"]))
            .filter(|u| *u > 0.0)
            .unwrap_or(500_000.0)
    });
    // 2. 携带 NewAPI / Veloera / RixAPI 全兼容用户 ID 请求头访问 /api/user/self
    let uid = cfg.user_id.trim();
    let headers: Vec<(&str, &str)> = if uid.is_empty() {
        vec![]
    } else {
        vec![
            ("New-Api-User", uid),
            ("Veloera-User", uid),
            ("Rix-Api-User", uid),
            ("User-Id", uid),
        ]
    };
    let body = http_get_json_ext(&format!("{base}/api/user/self"), Some(token), &headers, None)?;
    parse_newapi_user_self(&body, per_unit, cfg)
}

fn query_provider(cfg: &ProviderConfig) -> QuotaEntry {
    if cfg.kind == ProviderKind::Manual {
        return build_manual(cfg);
    }

    let key = cfg.api_key.trim();
    let base = cfg.base_url.trim().trim_end_matches('/');

    // 针对本地 OAuth 探针做防限流缓存（CodexBar 实践：5 分钟冷却避免高频 429）
    if matches!(cfg.kind, ProviderKind::ClaudeCode | ProviderKind::Codex) {
        let cache_key = format!("{:?}:{}", cfg.kind, cfg.name);
        if let Some(cached) = oauth_cache_get(&cache_key) {
            return cached;
        }
        let home = user_home_dir();
        let res = match cfg.kind {
            ProviderKind::ClaudeCode => {
                let tok = if !key.is_empty() {
                    Some(key.to_string())
                } else {
                    home.as_deref().and_then(extract_claude_oauth_token_from_home)
                };
                match tok {
                    Some(t) => http_get_json_ext(
                        "https://api.anthropic.com/api/oauth/usage",
                        Some(&t),
                        &[("anthropic-beta", "oauth-2025-04-20")],
                        Some("claude-code/2.0.0"),
                    )
                    .and_then(|s| parse_claude_oauth_usage(&s, cfg)),
                    None => Err("未找到本机凭据".into()),
                }
            }
            ProviderKind::Codex => {
                let auth = if !key.is_empty() {
                    Some((
                        key.to_string(),
                        (!cfg.user_id.trim().is_empty()).then(|| cfg.user_id.trim().to_string()),
                    ))
                } else {
                    home.as_deref().and_then(extract_codex_auth_from_home)
                };
                match auth {
                    Some((t, acct)) => {
                        let hdrs: Vec<(&str, &str)> = acct
                            .as_deref()
                            .map(|id| vec![("ChatGPT-Account-Id", id)])
                            .unwrap_or_default();
                        http_get_json_ext(
                            "https://chatgpt.com/backend-api/wham/usage",
                            Some(&t),
                            &hdrs,
                            Some("codex-cli/1.0"),
                        )
                        .and_then(|s| parse_codex_wham_usage(&s, cfg))
                    }
                    None => Err("未找到本机凭据".into()),
                }
            }
            _ => unreachable!(),
        };
        return match res {
            Ok(entry) => {
                oauth_cache_put(&cache_key, &entry);
                entry
            }
            Err(err) => QuotaEntry {
                name: cfg.name.clone(),
                tag: default_tag(cfg),
                remaining_text: if err.contains("未找到") {
                    "未登录".into()
                } else {
                    err.clone()
                },
                detail_text: if cfg.kind == ProviderKind::ClaudeCode {
                    "需本机 .claude 凭据".into()
                } else {
                    "需本机 .codex 凭据".into()
                },
                ratio: 0.0,
                color_rgb: if err.contains("未找到") {
                    resolve_color(&cfg.color, cfg.kind)
                } else {
                    (255, 59, 48)
                },
                live: false,
                error: Some(err),
            },
        };
    }

    if key.is_empty() {
        return QuotaEntry {
            name: cfg.name.clone(),
            tag: default_tag(cfg),
            remaining_text: "未配置".into(),
            detail_text: "点 ⚙ 填入 Key".into(),
            ratio: 0.0,
            color_rgb: resolve_color(&cfg.color, cfg.kind),
            live: false,
            error: Some("未填 Key".into()),
        };
    }

    let res = match cfg.kind {
        ProviderKind::DeepSeek => {
            let url = if base.is_empty() {
                "https://api.deepseek.com/user/balance".to_string()
            } else {
                format!("{base}/user/balance")
            };
            http_get_json(&url, key).and_then(|s| parse_deepseek(&s, cfg))
        }
        ProviderKind::OpenRouter => {
            let root = if base.is_empty() {
                "https://openrouter.ai"
            } else {
                base
            };
            // 参考开源实践：优先管理端 /credits，失败自动回退普通 sk-or Key 专用 /api/v1/key
            http_get_json(&format!("{root}/api/v1/credits"), key)
                .and_then(|s| parse_openrouter(&s, cfg))
                .or_else(|_| {
                    http_get_json(&format!("{root}/api/v1/key"), key)
                        .and_then(|s| parse_openrouter_key(&s, cfg))
                })
        }
        ProviderKind::SiliconFlow => {
            let url = if base.is_empty() {
                "https://api.siliconflow.cn/v1/user/info".to_string()
            } else {
                format!("{base}/v1/user/info")
            };
            http_get_json(&url, key).and_then(|s| parse_siliconflow(&s, cfg))
        }
        ProviderKind::Moonshot => {
            let url = if base.is_empty() {
                "https://api.moonshot.cn/v1/users/me/balance".to_string()
            } else {
                format!("{base}/v1/users/me/balance")
            };
            http_get_json(&url, key).and_then(|s| parse_moonshot(&s, cfg))
        }
        ProviderKind::OneApi => {
            if base.is_empty() {
                Err("请填写中转站地址".into())
            } else {
                let prefer_site = !cfg.user_id.trim().is_empty() || !key.starts_with("sk-");
                let billing_fallback = || {
                    let (start, end) = ninety_days_ago_ymd();
                    let sub_url = format!("{base}/v1/dashboard/billing/subscription");
                    let usage_url = format!(
                        "{base}/v1/dashboard/billing/usage?start_date={start}&end_date={end}"
                    );
                    http_get_json(&sub_url, key).and_then(|sub_body| {
                        let usage_body = http_get_json(&usage_url, key).ok();
                        parse_oneapi(&sub_body, usage_body.as_deref(), cfg)
                    })
                };
                if prefer_site {
                    query_oneapi_site(base, key, cfg).or_else(|_| billing_fallback())
                } else {
                    billing_fallback().or_else(|_| query_oneapi_site(base, key, cfg))
                }
            }
        }
        ProviderKind::ClaudeCode | ProviderKind::Codex | ProviderKind::Manual => unreachable!(),
    };

    match res {
        Ok(entry) => entry,
        Err(err) => QuotaEntry {
            name: cfg.name.clone(),
            tag: default_tag(cfg),
            remaining_text: err.clone(),
            detail_text: "请检查网络/Key".into(),
            ratio: 0.0,
            color_rgb: (255, 59, 48),
            live: false,
            error: Some(err),
        },
    }
}

/// 读取 `{data_dir}/ai-tokens.json` 并查询所有已配置的 AI 渠道/中转站余额。
pub fn fetch_all(data_dir: &Path) -> QuotaSnapshot {
    let Some(cfg) = load_config(data_dir) else {
        return demo_snapshot();
    };
    if !has_configured_live_provider(data_dir)
        && !cfg.providers.iter().any(|p| p.kind == ProviderKind::Manual)
    {
        return demo_snapshot();
    }

    let entries: Vec<QuotaEntry> = cfg.providers.iter().map(query_provider).collect();
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    QuotaSnapshot {
        entries,
        updated_label: format!("{:02}:{:02} 已刷新", st.wHour, st.wMinute),
        is_demo: false,
        refreshing: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_cfg(name: &str, kind: ProviderKind, total: Option<f64>) -> ProviderConfig {
        ProviderConfig {
            name: name.into(),
            kind,
            base_url: "https://api.example.com".into(),
            api_key: "sk-test".into(),
            user_id: String::new(),
            quota_per_unit: None,
            currency: "USD".into(),
            total,
            used: None,
            reset_note: String::new(),
            tag: String::new(),
            color: String::new(),
        }
    }

    #[test]
    fn parse_deepseek_balance() {
        let json = r#"{"is_available":true,"balance_infos":[{"currency":"CNY","total_balance":"84.25","granted_balance":"0.00","topped_up_balance":"84.25"}]}"#;
        let e = parse_deepseek(json, &sample_cfg("DeepSeek", ProviderKind::DeepSeek, Some(100.)))
            .unwrap();
        assert_eq!(e.remaining_text, "¥84.25");
        assert!(e.ratio > 0.84 && e.ratio < 0.85);
        assert!(e.live);
    }

    #[test]
    fn parse_openrouter_credits() {
        let json = r#"{"data":{"total_credits":20.0,"total_usage":5.4}}"#;
        let e = parse_openrouter(
            json,
            &sample_cfg("OpenRouter", ProviderKind::OpenRouter, None),
        )
        .unwrap();
        assert_eq!(e.remaining_text, "$14.60");
        assert_eq!(e.detail_text, "共 $20.00");
        assert!(e.ratio > 0.72 && e.ratio < 0.74);
    }

    #[test]
    fn parse_openrouter_key_fallback() {
        let json = r#"{"data":{"label":"DevKey","limit":10.0,"limit_remaining":6.42,"limit_reset":"monthly","usage":3.58}}"#;
        let e = parse_openrouter_key(
            json,
            &sample_cfg("OpenRouter", ProviderKind::OpenRouter, None),
        )
        .unwrap();
        assert_eq!(e.remaining_text, "$6.42");
        assert_eq!(e.detail_text, "限额 $10.00 · 月重置");
        assert!(e.ratio > 0.64 && e.ratio < 0.65);

        let unlimited = r#"{"data":{"limit":null,"limit_remaining":null,"usage":2.15}}"#;
        let e2 = parse_openrouter_key(
            unlimited,
            &sample_cfg("OpenRouter", ProviderKind::OpenRouter, None),
        )
        .unwrap();
        assert_eq!(e2.remaining_text, "已用 $2.15");
    }

    #[test]
    fn parse_siliconflow_and_moonshot() {
        let sf = r#"{"code":20000,"data":{"totalBalance":"42.50"}}"#;
        let ms = r#"{"data":{"available_balance":19.8}}"#;
        let e1 = parse_siliconflow(
            sf,
            &sample_cfg("硅基流动", ProviderKind::SiliconFlow, Some(50.)),
        )
        .unwrap();
        let e2 = parse_moonshot(ms, &sample_cfg("Kimi", ProviderKind::Moonshot, Some(50.)))
            .unwrap();
        assert_eq!(e1.remaining_text, "¥42.50");
        assert_eq!(e2.remaining_text, "¥19.80");
    }

    #[test]
    fn parse_oneapi_subscription_and_usage() {
        let sub = r#"{"has_payment_method":true,"hard_limit_usd":50.0}"#;
        let usage = r#"{"total_usage":1160.0}"#; // 11.60 USD
        let e = parse_oneapi(
            sub,
            Some(usage),
            &sample_cfg("NewAPI 中转", ProviderKind::OneApi, None),
        )
        .unwrap();
        assert_eq!(e.remaining_text, "$38.40");
        assert_eq!(e.detail_text, "共 $50.00");
        assert_eq!(e.tag, "中转站");
    }

    #[test]
    fn parse_newapi_user_self_quota() {
        let body = r#"{"success":true,"data":{"quota":19200000,"used_quota":5800000}}"#;
        let cfg = sample_cfg("NewAPI 中转", ProviderKind::OneApi, None);
        let e = parse_newapi_user_self(body, 500_000.0, &cfg).unwrap();
        assert_eq!(e.remaining_text, "$38.40");
        assert_eq!(e.detail_text, "已用 $11.60");
        assert!(e.ratio > 0.76 && e.ratio < 0.77);
    }

    #[test]
    fn parse_claude_code_oauth_and_codex_wham() {
        let claude = r#"{"five_hour":{"utilization":33.0,"resets_at":"2026-04-11T07:00:00Z"},"seven_day":{"utilization":13.0}}"#;
        let ce = parse_claude_oauth_usage(
            claude,
            &sample_cfg("Claude Code", ProviderKind::ClaudeCode, None),
        )
        .unwrap();
        assert_eq!(ce.remaining_text, "67%");
        assert_eq!(ce.detail_text, "5h 剩 67% · 7d 剩 87%");
        assert_eq!(ce.tag, "OAuth");

        let codex = r#"{"rate_limit":{"primary_window":{"used_percent":22.0},"secondary_window":{"used_percent":40.0}}}"#;
        let xe = parse_codex_wham_usage(codex, &sample_cfg("Codex", ProviderKind::Codex, None))
            .unwrap();
        assert_eq!(xe.remaining_text, "78%");
        assert_eq!(xe.detail_text, "5h 剩 78% · 周剩 60%");
    }

    #[test]
    fn extract_local_credentials_json() {
        let tmp = std::env::temp_dir().join(format!("allegro_creds_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join(".claude")).unwrap();
        std::fs::create_dir_all(tmp.join(".codex")).unwrap();
        std::fs::write(
            tmp.join(".claude").join(".credentials.json"),
            r#"{"claudeAiOauth":{"accessToken":"oat01-test-token"}}"#,
        )
        .unwrap();
        std::fs::write(
            tmp.join(".codex").join("auth.json"),
            r#"{"tokens":{"access_token":"codex-jwt-123","account_id":"acct-999"}}"#,
        )
        .unwrap();

        assert_eq!(
            extract_claude_oauth_token_from_home(&tmp).as_deref(),
            Some("oat01-test-token")
        );
        assert_eq!(
            extract_codex_auth_from_home(&tmp),
            Some(("codex-jwt-123".into(), Some("acct-999".into())))
        );
        let _ = std::fs::remove_dir_all(&tmp);
    }

    #[test]
    fn default_config_excludes_gemini() {
        let tpl = serde_json::to_string(&default_config_template())
            .unwrap()
            .to_lowercase();
        assert!(!tpl.contains("gemini"));
        assert!(!tpl.contains("google"));
        let demo = demo_snapshot();
        assert!(!demo.updated_label.to_lowercase().contains("gemini"));
        for e in &demo.entries {
            assert!(!e.name.to_lowercase().contains("gemini"));
            assert!(!e.tag.to_lowercase().contains("gemini"));
            assert!(!e.detail_text.to_lowercase().contains("gemini"));
        }
    }
}
