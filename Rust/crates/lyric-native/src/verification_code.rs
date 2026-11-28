//! Conservative local OTP extraction. Codes are never logged or persisted.
use crate::layout::Rect;
pub const WIDTH:f64=384.;
pub const HEIGHT:f64=84.;
#[derive(Clone,Copy,Debug)]
pub struct Layout{pub body:Rect,pub icon:Rect,pub source:Rect,pub digits:Rect,pub action:Rect,pub compact_action:bool}
/// Content keeps its final geometry while the live shell reveals and clips it.
pub fn layout(shell:Rect,host_width:f64)->Layout{
    let width=WIDTH.min((host_width-48.).max(140.));
    let body=Rect::new((host_width-width)/2.,shell.y,width,HEIGHT);
    let compact_action=width<336.;let action_width=if compact_action{28.}else{84.};
    let x=body.x+60.;let action=Rect::new(body.x+width-20.-action_width,body.y+38.,action_width,28.);
    Layout{body,icon:Rect::new(body.x+18.,body.y+27.,30.,30.),
        source:Rect::new(x,body.y+13.,width-80.,18.),
        digits:Rect::new(x,body.y+34.,(action.x-x-14.).max(1.),36.),action,compact_action}
}
pub fn content_alpha(shell_height:f64,mix:f64)->f32{
    (mix.clamp(0.,1.)*((shell_height-(HEIGHT-8.))/8.).clamp(0.,1.)) as f32
}

#[derive(Clone)]
struct Candidate {
    score: i32,
    code: String,
    start: usize,
    structured: bool,
}

/// 统一清洗通知文本：移除零宽隐形字符，规整全角数字/字母、特殊空格与连字符变体。
fn normalize_text(title: &str, body: &str) -> Vec<char> {
    let raw = format!("{title}\n{body}");
    raw.chars()
        .filter(|c| !matches!(c, '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{2060}' | '\u{FEFF}' | '\u{00AD}'))
        .map(|c| {
            if ('０'..='９').contains(&c) {
                char::from_u32(c as u32 - '０' as u32 + '0' as u32).unwrap()
            } else if ('Ａ'..='Ｚ').contains(&c) {
                char::from_u32(c as u32 - 'Ａ' as u32 + 'A' as u32).unwrap()
            } else if ('ａ'..='ｚ').contains(&c) {
                char::from_u32(c as u32 - 'ａ' as u32 + 'a' as u32).unwrap()
            } else if matches!(c, '\u{00A0}' | '\u{3000}' | '\u{2000}'..='\u{200A}') {
                ' '
            } else if matches!(c, '\u{2010}'..='\u{2014}' | '\u{FF0D}') {
                '-'
            } else {
                c
            }
        })
        .collect()
}

/// 判断字符区间是否匹配独立英文单词边界。
fn has_ascii_word_boundaries(text: &[char], start: usize, end: usize) -> bool {
    let prev_ok = start == 0 || !text[start - 1].is_ascii_alphanumeric();
    let next_ok = !text.get(end).is_some_and(char::is_ascii_alphanumeric);
    prev_ok && next_ok
}

/// 检索全部验证码强/中/弱关键词位置及基础权重。
fn find_markers(text: &[char], matching: &[char]) -> Vec<(usize, usize, i32)> {
    let mut markers = Vec::new();
    let strong_words = [
        "验证码", "验证代码", "登录代码", "安全代码", "登录码", "安全码", "校验码",
        "动态码", "动态密码", "动态口令", "短信码", "短信口令", "确认码", "一次性密码", "一次性代码",
        "认证码", "认证口令", "鉴权码", "随机码", "随机密码", "临时密码", "通行码", "验证密码",
        "登录验证", "账号验证", "安全验证", "手机验证", "身份验证", "二次验证", "两步验证",
        "二次校验", "两步校验", "双因素", "短信验证", "邮箱验证", "密保", "绑定码", "注册码", "激活码",
        "verification code", "security code", "one-time code", "one time code",
        "one-time password", "one time password", "one-time passcode", "one time passcode",
        "single-use code", "single use code", "temporary code", "temp code",
        "reset code", "recovery code", "email code", "sms code", "text code",
        "authorization code", "challenge code", "secret code",
        "sign-in code", "signin code", "login code", "log-in code",
        "auth code", "authentication code", "confirmation code", "confirm code",
        "passcode", "access code", "activation code", "verify code", "validation code",
        "2fa code", "mfa code", "pin code", "otp", "totp", "2fa", "mfa",
        "two-factor", "two factor", "2-step", "verify your",
    ];
    let medium_words = ["重置密码", "找回密码", "修改密码"];

    let scan_list = |markers: &mut Vec<(usize, usize, i32)>, words: &[&str], weight: i32, word_bound_ascii: &[&str]| {
        for word in words {
            let key: Vec<char> = word.chars().collect();
            for i in 0..text.len().saturating_sub(key.len() - 1) {
                if matching.get(i..i + key.len()) == Some(key.as_slice()) {
                    if word_bound_ascii.contains(word) && !has_ascii_word_boundaries(text, i, i + key.len()) {
                        continue;
                    }
                    markers.push((i, i + key.len(), weight));
                }
            }
        }
    };

    scan_list(&mut markers, &strong_words, 200, &["otp", "totp", "2fa", "mfa"]);
    if markers.is_empty() {
        scan_list(&mut markers, &medium_words, 170, &[]);
    }

    // 无强/中关键词时，采用严格语境约束的弱短语（code / pin / 口令）
    if markers.is_empty() {
        let negative_code_prefix = [
            "error", "status", "exit", "zip", "postal", "area",
            "promo", "coupon", "invite", "referral", "http", "qr", "bar", "source",
        ];
        let negative_pin_prefix = ["zip", "postal", "bank", "atm", "sim", "card", "puk", "hair", "bowling", "map"];
        let negative_kouling_prefix = ["红包", "淘", "复制", "分享", "福利", "优惠", "暗号"];

        for (key, negs, weight) in [
            (&['c', 'o', 'd', 'e'][..], &negative_code_prefix[..], 175),
            (&['p', 'i', 'n'][..], &negative_pin_prefix[..], 170),
        ] {
            for i in 0..text.len().saturating_sub(key.len() - 1) {
                if matching.get(i..i + key.len()) == Some(key)
                    && has_ascii_word_boundaries(text, i, i + key.len())
                {
                    let before: String = matching[i.saturating_sub(16)..i].iter().collect();
                    if negs.iter().any(|w| before.contains(w)) {
                        continue;
                    }
                    let after: String = matching[i + key.len()..(i + key.len() + 12).min(matching.len())]
                        .iter()
                        .collect();
                    let trimmed_after = after.trim_start();
                    let guided_after = trimmed_after.starts_with(':')
                        || trimmed_after.starts_with('：')
                        || after.starts_with(" is ")
                        || after.starts_with(" is:")
                        || after.starts_with(" is：")
                        || trimmed_after.starts_with("for ");
                    let guided_before = before.ends_with("use ")
                        || before.ends_with("enter ")
                        || before.ends_with("your ");
                    if guided_after || guided_before {
                        markers.push((i, i + key.len(), weight));
                    }
                }
            }
        }

        if markers.is_empty() {
            let kouling = ['口', '令'];
            for i in 0..text.len().saturating_sub(1) {
                if matching.get(i..i + 2) == Some(kouling.as_slice()) {
                    let before: String = matching[i.saturating_sub(8)..i].iter().collect();
                    if !negative_kouling_prefix.iter().any(|w| before.contains(w)) {
                        markers.push((i, i + 2, 170));
                    }
                }
            }
        }
    }

    markers
}

fn is_url_or_email(text: &[char], matching: &[char], start: usize, end: usize) -> bool {
    let prev = start.checked_sub(1).map(|k| text[k]);
    let next = text.get(end).copied();
    if matches!(prev, Some('@' | '/')) || matches!(next, Some('@' | '/')) {
        return true;
    }
    if prev == Some('.') && start >= 2 && text[start - 2].is_ascii_alphanumeric() {
        return true;
    }
    if next == Some('.') && text.get(end + 1).is_some_and(char::is_ascii_alphanumeric) {
        return true;
    }
    let ctx: String = matching[start.saturating_sub(16)..start].iter().collect();
    ctx.contains("://") || ctx.contains("http")
}

/// 过滤日期、时间、单位、版本号、普通词汇、金额以及业务单号/电话号码等误报。
fn is_negative_context(
    text: &[char],
    matching: &[char],
    start: usize,
    end: usize,
    code: &str,
    markers: &[(usize, usize, i32)],
) -> bool {
    // 1. 紧跟中文计量、时间与货币单位
    if text[end..]
        .iter()
        .find(|c| !c.is_whitespace())
        .is_some_and(|c| "年月日号时分秒天周元块折条次位个%℃°".contains(*c))
    {
        return true;
    }

    // 2. 紧跟英文时间、容量、货币与 AM/PM 单位（要求完整单词边界）
    let mut after_ws = end;
    while after_ws < text.len() && text[after_ws] == ' ' {
        after_ws += 1;
    }
    if after_ws < text.len() && matching[after_ws].is_ascii_alphabetic() {
        let mut word_end = after_ws;
        while word_end < matching.len() && matching[word_end].is_ascii_alphabetic() {
            word_end += 1;
        }
        let unit: String = matching[after_ws..word_end].iter().collect();
        if matches!(
            unit.as_str(),
            "sec" | "secs" | "second" | "seconds"
                | "min" | "mins" | "minute" | "minutes"
                | "hr" | "hrs" | "hour" | "hours"
                | "day" | "days" | "week" | "weeks" | "month" | "months" | "year" | "years"
                | "ms" | "kb" | "mb" | "gb" | "tb" | "px" | "hz"
                | "usd" | "cny" | "rmb" | "eur" | "gbp" | "jpy"
                | "am" | "pm"
        ) {
            return true;
        }
    }

    // 3. 货币符号前缀 & 冒号时间段（HH:MM / :MM）
    let mut before_ws = start;
    while before_ws > 0 && text[before_ws - 1] == ' ' {
        before_ws -= 1;
    }
    if before_ws > 0 && matches!(text[before_ws - 1], '$' | '¥' | '￥' | '€' | '£') {
        return true;
    }
    let prev = start.checked_sub(1).map(|k| text[k]);
    let next = text.get(end).copied();
    if prev == Some(':') && start >= 2 && text[start - 2].is_ascii_digit() {
        return true;
    }
    if next == Some(':') && text.get(end + 1).is_some_and(char::is_ascii_digit) {
        return true;
    }

    // 4. 日期分段与版本号小数（YYYY-MM-DD / 2026-11 / 1.2026）
    if text[start..end].iter().all(char::is_ascii_digit) {
        if matches!(next, Some('-' | '/' | '.')) {
            let mut k = end + 1;
            while k < text.len() && text[k].is_ascii_digit() {
                k += 1;
            }
            let n_after = k - (end + 1);
            if ((1..=2).contains(&n_after) && !text.get(k).is_some_and(char::is_ascii_alphanumeric))
                || (next == Some('.') && n_after >= 1)
            {
                return true;
            }
        }
        if matches!(prev, Some('-' | '/' | '.')) && start >= 2 {
            let mut k = start - 1;
            while k > 0 && text[k - 1].is_ascii_digit() {
                k -= 1;
            }
            let n_before = (start - 1) - k;
            if ((1..=2).contains(&n_before) && (k == 0 || !text[k - 1].is_ascii_alphanumeric()))
                || (prev == Some('.') && n_before >= 1)
            {
                return true;
            }
        }
    }

    // 5. 子句隔离后的近距负向业务上下文（避免跨句被“订单/尾号/客服”或前方已出现的关键词互相污染）
    let mut ctx_start = start.saturating_sub(18);
    for &(_, mb, _) in markers {
        if mb <= start {
            ctx_start = ctx_start.max(mb);
        }
    }
    for idx in (ctx_start..start).rev() {
        if matches!(text[idx], '\n' | '。' | '！' | '!' | '？' | '?' | '；' | ';' | '，' | ',') {
            ctx_start = idx + 1;
            break;
        }
    }
    let ctx: String = matching[ctx_start..start].iter().collect();
    let neg_words = [
        "订单", "运单", "快递", "单号", "取件", "提货", "核销", "取餐", "餐号",
        "排队", "票号", "航班", "车次", "座位", "房间号", "尾号", "卡号", "账号",
        "工号", "学号", "编号", "电话", "手机", "座机", "客服", "热线", "致电", "拨打",
        "金额", "余额", "消费", "支付", "转账", "扣款", "账单", "工单", "版本", "端口",
        "错误码", "状态码", "优惠", "折扣", "满减", "红包", "淘口令",
        "order", "tracking", "invoice", "ticket", "phone", "tel", "call",
        "hotline", "support", "error", "status", "exit", "errno", "port",
        "version", "build", "commit", "zip", "postal", "promo", "coupon",
        "invite", "referral", "amount", "balance", "total", "price",
        "usd", "cny", "rmb", "￥", "¥", "$",
    ];
    if neg_words.iter().any(|w| ctx.contains(w)) {
        return true;
    }

    // 6. 常见 5 位银行/运营商/政务特服客服号（如 95588、10086、12306）：无紧邻强引导时拒绝
    if code.len() == 5
        && code.bytes().all(|b| b.is_ascii_digit())
        && (code.starts_with("95") || code.starts_with("100") || code.starts_with("123"))
    {
        let tightly_guided = markers.iter().any(|&(_, mb, weight)| {
            weight >= 190
                && start >= mb
                && start - mb <= 6
                && matching[mb..start]
                    .iter()
                    .all(|c| matches!(c, ':' | '：' | '为' | '是' | ' ' | '【' | '[' | '「'))
        });
        if !tightly_guided {
            return true;
        }
    }

    false
}

/// 识别技术词、数字带单位词、版本号及“纯小写英文单词+数字”等非验证码词元。
fn is_non_otp_token(code: &str) -> bool {
    let lower = code.to_ascii_lowercase();
    if matches!(
        lower.as_str(),
        "win10" | "win11" | "utf8" | "utf16" | "sha1" | "sha256" | "md5"
            | "ipv4" | "ipv6" | "http2" | "http3" | "html5" | "css3"
            | "mp3" | "mp4" | "h264" | "h265" | "tls1" | "x64" | "x86" | "arm64"
            | "macos14" | "macos15" | "ios17" | "ios18" | "ios26"
    ) {
        return true;
    }
    if code.bytes().all(|b| b.is_ascii_digit()) {
        return false;
    }

    let bytes = lower.as_bytes();
    // 数字 + 单位字母拼合（如 10min, 300s, 24hr, 500mb, 100px）
    let d_end = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
    if d_end > 0 && bytes[d_end..].iter().all(u8::is_ascii_alphabetic) {
        let suffix = &lower[d_end..];
        if matches!(
            suffix,
            "s" | "sec" | "secs" | "m" | "min" | "mins" | "h" | "hr" | "hrs"
                | "d" | "day" | "days" | "w" | "wk" | "wks" | "y" | "yr" | "yrs"
                | "ms" | "kb" | "mb" | "gb" | "tb" | "px" | "hz" | "mhz" | "ghz"
                | "q1" | "q2" | "q3" | "q4" | "am" | "pm" | "st" | "nd" | "rd" | "th"
        ) {
            return true;
        }
    }

    // 英文前缀 + 纯数字后缀（版本号、技术代号、月份日期或普通单词+年份，如 pass1234 / user2026 / v2025）
    let a_end = bytes.iter().take_while(|b| b.is_ascii_alphabetic()).count();
    if a_end > 0 && bytes[a_end..].iter().all(u8::is_ascii_digit) {
        let prefix_lower = &lower[..a_end];
        if matches!(
            prefix_lower,
            "v" | "ver" | "r" | "win" | "ios" | "ipad" | "mac" | "macos" | "iphone" | "android"
                | "chrome" | "edge" | "firefox" | "safari" | "gpt" | "utf" | "sha" | "md"
                | "ipv" | "http" | "https" | "html" | "css" | "tls" | "ssl" | "usb"
                | "hdmi" | "pcie" | "cpu" | "gpu" | "ddr" | "wifi"
                | "jan" | "feb" | "mar" | "apr" | "may" | "jun" | "jul" | "aug" | "sep" | "sept" | "oct" | "nov" | "dec"
                | "beta" | "alpha" | "rc"
        ) {
            return true;
        }
        if a_end >= 4 && code[..a_end].bytes().all(|b| b.is_ascii_lowercase()) {
            return true;
        }
    }

    false
}

/// 综合关键词距离、跨句惩罚、引导特征、包裹括号、独立成行与字符形态打分。
fn score_candidate(
    text: &[char],
    matching: &[char],
    code: &str,
    start: usize,
    end: usize,
    structured: bool,
    markers: &[(usize, usize, i32)],
) -> Option<i32> {
    let has_caution_tone = {
        let full: String = matching.iter().collect();
        [
            "切勿泄露", "请勿告知", "请勿告诉", "切勿告知", "分钟内有效",
            "do not share", "never share", "expires in", "valid for", "one-time",
        ]
        .iter()
        .any(|w| full.contains(w))
    };

    let mut best_score = None;
    for &(ma, mb, marker_weight) in markers {
        let dist = if start >= mb {
            start - mb
        } else if end <= ma {
            ma - end
        } else {
            0
        };
        if dist > 160 {
            continue;
        }

        // 近距快速衰减，长邮件正文超过 64 字符后采用平缓衰减保留唯一高置信验证码
        let dist_penalty = if dist <= 64 {
            (dist as i32) * 2
        } else {
            128 + (dist - 64) as i32
        };

        // 统计关键词与候选之间跨越的强句末标点（保留换行以兼容“标题+下一行验证码”）
        let span = if start >= mb {
            &matching[mb..start]
        } else if end <= ma {
            &matching[end..ma]
        } else {
            &[]
        };
        let sentence_breaks = span
            .iter()
            .filter(|c| matches!(c, '。' | '!' | '！' | '?' | '？' | ';' | '；'))
            .count() as i32;

        let mut score = marker_weight - dist_penalty - sentence_breaks * 28;
        if start >= mb {
            score += 30;
        }

        // 紧邻引导符判断
        let between: String = if start >= mb {
            matching[mb..start].iter().collect()
        } else {
            String::new()
        };
        if between
            .chars()
            .any(|c| matches!(c, ':' | '：' | '为' | '是' | '【' | '[' | '「' | '#'))
            || between.contains(" is ")
            || between.contains(" is:")
            || between.contains(" is：")
            || between.contains("enter ")
            || between.contains("use ")
            || between.contains("输入")
        {
            score += 45;
        }

        // 配对括号、引号或星号包裹加分
        let prev = start.checked_sub(1).map(|k| text[k]);
        let next = text.get(end).copied();
        if matches!(
            (prev, next),
            (Some('【'), Some('】'))
                | (Some('「'), Some('」'))
                | (Some('『'), Some('』'))
                | (Some('['), Some(']'))
                | (Some('('), Some(')'))
                | (Some('"'), Some('"'))
                | (Some('\''), Some('\''))
                | (Some('*'), Some('*'))
        ) {
            score += 35;
        }

        // 独立成行加分
        let left_line_clear = text[..start]
            .iter()
            .rev()
            .take_while(|c| **c != '\n')
            .all(|c| *c == ' ');
        let right_line_clear = text[end..]
            .iter()
            .take_while(|c| **c != '\n')
            .all(|c| *c == ' ');
        if left_line_clear && right_line_clear {
            score += 25;
        }

        if structured {
            score += 25;
        }
        if has_caution_tone {
            score += 18;
        }

        let digits_only = code.bytes().all(|b| b.is_ascii_digit());
        let has_upper = code.bytes().any(|b| b.is_ascii_uppercase());
        score += match (digits_only, code.len(), has_upper) {
            (true, 6, _) => 35,
            (true, 4, _) => 24,
            (true, _, _) => 18,
            (false, _, true) => 14,
            (false, _, false) => 4,
        };

        if best_score.is_none_or(|b| score > b) {
            best_score = Some(score);
        }
    }
    best_score
}

/// 扫描全部合法候选区间（#数字、官方字母前缀码、分组数字码与连续字母数字串）。
fn collect_candidates(
    text: &[char],
    matching: &[char],
    markers: &[(usize, usize, i32)],
) -> Vec<Candidate> {
    let mut candidates: Vec<Candidate> = Vec::new();
    let mut push = |code: String, start: usize, end: usize, structured: bool| {
        if !valid_code(&code)
            || is_url_or_email(text, matching, start, end)
            || is_negative_context(text, matching, start, end, &code, markers)
            || is_non_otp_token(&code)
        {
            return;
        }
        let Some(score) = score_candidate(text, matching, &code, start, end, structured, markers) else {
            return;
        };
        if score < 115 {
            return;
        }
        if let Some(existing) = candidates.iter_mut().find(|c| c.code == code) {
            if score > existing.score {
                existing.score = score;
                existing.start = start;
                existing.structured |= structured;
            }
        } else {
            candidates.push(Candidate {
                score,
                code,
                start,
                structured,
            });
        }
    };

    // 1) # + 4..=8 位纯数字（如 #739201，排除十六进制色值与订单/工单编号）
    let mut idx = 0;
    while idx < text.len() {
        if text[idx] == '#' && !idx.checked_sub(1).is_some_and(|p| text[p].is_ascii_alphanumeric()) {
            let mut k = idx + 1;
            while k < text.len() && text[k].is_ascii_digit() {
                k += 1;
            }
            let len = k - (idx + 1);
            if (4..=8).contains(&len) && !text.get(k).is_some_and(char::is_ascii_alphanumeric) {
                let before: String = matching[idx.saturating_sub(12)..idx].iter().collect();
                if !["pr", "issue", "ticket", "order", "ref", "订单", "工单", "编号", "排号"]
                    .iter()
                    .any(|w| before.contains(w))
                {
                    let digits: String = text[idx + 1..k].iter().collect();
                    push(digits, idx, k, true);
                    idx = k;
                    continue;
                }
            }
        }
        idx += 1;
    }

    // 2) 连续 4-8 位字母数字串，及 1~2 位字母前缀 OTP（如 G-482910 / FB-618204）
    let mut i = 0;
    while i < text.len() {
        if !text[i].is_ascii_alphanumeric() {
            i += 1;
            continue;
        }
        let start = i;
        while i < text.len() && text[i].is_ascii_alphanumeric() {
            i += 1;
        }
        let raw: String = text[start..i].iter().collect();
        let prefix_upper = raw.to_ascii_uppercase();
        if (1..=2).contains(&raw.len())
            && raw.bytes().all(|b| b.is_ascii_alphabetic())
            && !matches!(
                prefix_upper.as_str(),
                "V" | "PR" | "NO" | "ID" | "QQ" | "IP" | "OS" | "EX" | "PM" | "AM" | "KB" | "MB" | "GB" | "TB" | "CN" | "US" | "UK"
            )
            && text.get(i).is_some_and(|c| *c == '-' || *c == ' ')
        {
            let mut k = i + 1;
            while k < text.len() && text[k].is_ascii_digit() {
                k += 1;
            }
            let dlen = k - (i + 1);
            if (6..=8).contains(&dlen) && !text.get(k).is_some_and(char::is_ascii_alphanumeric) {
                let digits: String = text[i + 1..k].iter().collect();
                push(digits, start, k, true);
                i = k;
                continue;
            }
        }
        push(raw, start, i, false);
    }

    // 3) 分组数字码：3-3 / 4-4（如 123-456、482 910）、2-2-2（如 12 34 56）及单空格 4~6 位（4 8 2 9 1 0）
    let mut j = 0;
    while j < text.len() {
        let boundary_ok = j == 0 || (!text[j - 1].is_ascii_alphanumeric() && !matches!(text[j - 1], '-' | '/' | '.'));
        if !text[j].is_ascii_digit() || !boundary_ok {
            j += 1;
            continue;
        }
        let s1 = j;
        while j < text.len() && text[j].is_ascii_digit() {
            j += 1;
        }
        let len1 = j - s1;

        // 3a) 对称 3+3 或 4+4
        if (len1 == 3 || len1 == 4) && text.get(j).is_some_and(|c| *c == '-' || *c == ' ') {
            let s2 = j + 1;
            let mut k = s2;
            while k < text.len() && text[k].is_ascii_digit() {
                k += 1;
            }
            if k - s2 == len1 && !text.get(k).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | '.')) {
                let combined: String = text[s1..j].iter().chain(&text[s2..k]).collect();
                push(combined, s1, k, true);
            }
        }

        // 3b) 2+2+2 分组（如 12 34 56；若用 '-' 连接且落在 YY-MM-DD 日期范围则避开）
        if len1 == 2 && text.get(j).is_some_and(|c| *c == ' ' || *c == '-') {
            let sep = text[j];
            let s2 = j + 1;
            if text.get(s2..s2 + 2).is_some_and(|d| d.iter().all(char::is_ascii_digit))
                && text.get(s2 + 2) == Some(&sep)
                && text.get(s2 + 3..s2 + 5).is_some_and(|d| d.iter().all(char::is_ascii_digit))
                && !text.get(s2 + 5).is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | '.'))
            {
                let p1: u32 = text[s1..s1 + 2].iter().collect::<String>().parse().unwrap_or(99);
                let p2: u32 = text[s2..s2 + 2].iter().collect::<String>().parse().unwrap_or(99);
                let p3: u32 = text[s2 + 3..s2 + 5].iter().collect::<String>().parse().unwrap_or(99);
                let looks_like_date = sep == '-' && (20..=39).contains(&p1) && (1..=12).contains(&p2) && (1..=31).contains(&p3);
                if !looks_like_date {
                    let combined: String = text[s1..s1 + 2]
                        .iter()
                        .chain(&text[s2..s2 + 2])
                        .chain(&text[s2 + 3..s2 + 5])
                        .collect();
                    push(combined, s1, s2 + 5, true);
                }
            }
        }

        // 3c) 单数字空格隔开 4~6 位（如 "4 8 2 9 1 0"）
        if len1 == 1 && text.get(j) == Some(&' ') {
            let mut digits = vec![text[s1]];
            let mut cur = j;
            while text.get(cur) == Some(&' ') && text.get(cur + 1).is_some_and(char::is_ascii_digit) {
                let next_after = text.get(cur + 2).copied();
                if next_after.is_some_and(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '/' | '.')) {
                    digits.clear();
                    break;
                }
                digits.push(text[cur + 1]);
                cur += 2;
                if digits.len() > 6 {
                    break;
                }
            }
            if (4..=6).contains(&digits.len()) && !text.get(cur).is_some_and(char::is_ascii_alphanumeric) {
                let combined: String = digits.into_iter().collect();
                push(combined, s1, cur, true);
            }
        }
    }

    candidates
}

pub fn extract(title:&str,body:&str)->Option<String>{
    let text=normalize_text(title,body);
    let matching=text.iter().map(char::to_ascii_lowercase).collect::<Vec<_>>();
    let markers=find_markers(&text,&matching);
    if markers.is_empty(){return None}

    let mut candidates=collect_candidates(&text,&matching,&markers);
    candidates.sort_by(|a,b|b.score.cmp(&a.score));

    if let Some(second)=candidates.get(1){
        if second.code!=candidates[0].code
            && (second.score==candidates[0].score
                || (candidates[0].score<150 && candidates[0].score-second.score<15))
        {
            return None;
        }
    }
    candidates.first().map(|c|c.code.clone())
}

/// 结合来源应用名称辅助识别：普通消息直接复用 `extract`，仅当发自验证器或短信应用且语义明确时补充极简提示。
pub fn extract_from_notice(app_name: &str, title: &str, body: &str) -> Option<String> {
    if let Some(code) = extract(title, body) {
        return Some(code);
    }
    let app_lower = app_name.to_ascii_lowercase();
    let trusted_auth_or_sms = [
        "authenticator", "authy", "验证器", "手机连接", "phone link", "your phone", "短信", "messages",
    ]
    .iter()
    .any(|k| app_lower.contains(k));
    if !trusted_auth_or_sms {
        return None;
    }

    let text = normalize_text(title, body);
    let matching: Vec<char> = text.iter().map(char::to_ascii_lowercase).collect();
    let full: String = matching.iter().collect();
    let has_action_hint = [
        "登录", "确认", "安全", "验证", "一次性", "sign in", "signin", "login", "verify", "security", "one-time", "passcode",
    ]
    .iter()
    .any(|w| full.contains(w));
    if !has_action_hint {
        return None;
    }

    let synthetic_body = format!("{title}\n{body}");
    let synth_text = normalize_text("验证码", &synthetic_body);
    let synth_matching: Vec<char> = synth_text.iter().map(char::to_ascii_lowercase).collect();
    let synth_markers = find_markers(&synth_text, &synth_matching);
    let numeric_candidates: Vec<Candidate> = collect_candidates(&synth_text, &synth_matching, &synth_markers)
        .into_iter()
        .filter(|c| c.code.bytes().all(|b| b.is_ascii_digit()))
        .collect();
    if numeric_candidates.len() == 1 {
        Some(numeric_candidates[0].code.clone())
    } else {
        None
    }
}

fn valid_code(code:&str)->bool {
    (4..=8).contains(&code.len()) && code.bytes().all(|c|c.is_ascii_alphanumeric())
        && code.bytes().any(|c|c.is_ascii_digit())
}

pub fn copy(code:&str,hwnd:windows::Win32::Foundation::HWND)->Result<(),String>{
    use std::ffi::c_void;
    #[link(name="user32")]unsafe extern "system"{
        fn OpenClipboard(hwnd:*mut c_void)->i32;fn CloseClipboard()->i32;
        fn EmptyClipboard()->i32;fn SetClipboardData(format:u32,memory:*mut c_void)->*mut c_void;
    }
    #[link(name="kernel32")]unsafe extern "system"{
        fn GlobalAlloc(flags:u32,size:usize)->*mut c_void;fn GlobalLock(memory:*mut c_void)->*mut c_void;
        fn GlobalUnlock(memory:*mut c_void)->i32;fn GlobalFree(memory:*mut c_void)->*mut c_void;
    }
    if !valid_code(code){return Err("验证码无效".into())}
    let data=code.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    unsafe{
        let memory=GlobalAlloc(0x42,data.len()*2);
        if memory.is_null(){return Err("复制失败，请重试".into())}
        let ptr=GlobalLock(memory);
        if ptr.is_null(){GlobalFree(memory);return Err("复制失败，请重试".into())}
        std::ptr::copy_nonoverlapping(data.as_ptr(),ptr.cast::<u16>(),data.len());GlobalUnlock(memory);
        // 剪贴板可能被系统或第三方剪切板工具瞬时占用，进行轻量退避重试以提升一次成功率
        let mut opened=false;
        for attempt in 0..6u64{
            if OpenClipboard(hwnd.0)!=0{opened=true;break;}
            std::thread::sleep(std::time::Duration::from_millis(8+attempt*4));
        }
        if !opened{GlobalFree(memory);return Err("剪贴板正忙，请再点一次".into())}
        let ok=EmptyClipboard()!=0&&!SetClipboardData(13,memory).is_null();CloseClipboard();
        if !ok{GlobalFree(memory);return Err("复制失败，请重试".into())}
    }
    Ok(())
}
#[cfg(test)] mod tests{
    use super::*;
    #[test]fn recognizes_nearby_codes_and_full_width_digits(){
        assert_eq!(extract("登录提醒","验证码：１２３４５６，5 分钟内有效"),Some("123456".into()));
        assert_eq!(extract("Your OTP","Your verification code is 472913"),Some("472913".into()));
        assert_eq!(extract("8421 是您的动态码","请勿告诉他人"),Some("8421".into()));
        assert_eq!(extract("","您的动态密码：739251，请勿泄露"),Some("739251".into()));
        assert_eq!(extract("","确认码 4821"),Some("4821".into()));
    }
    #[test]fn mail_codes_preserve_case_and_copy_validation_accepts_the_same_payload(){
        for code in ["a7B2c9","0042","Z9x8Y7w6"] {
            let body=format!("QQ邮箱提醒：服务登录，您的验证码是 {code}，5 分钟内有效。");
            assert_eq!(extract("新邮件提醒",&body).as_deref(),Some(code));
            assert!(valid_code(code));
        }
        assert_eq!(extract("Sign-in code","Your login code is a7B2c9"),Some("a7B2c9".into()));
        for invalid in ["security","minutes","AB123456CD","123456789","a7 B2c9","https://"] {assert!(!valid_code(invalid));}
    }
    #[test]fn recognizes_grouped_prefixed_fullwidth_alpha_and_long_context(){
        assert_eq!(extract("Google","G-482910 是您的 Google 验证码。"),Some("482910".into()));
        assert_eq!(extract("验证代码","动态口令：123-456，请勿泄露"),Some("123456".into()));
        assert_eq!(extract("安全验证","您的验证码为 482 910"),Some("482910".into()));
        assert_eq!(extract("登录验证","验证码：Ａ７b２C９"),Some("A7b2C9".into()));
        assert_eq!(extract("Verification code","Hello Alice, thank you for signing in today. Please enter 619042 to continue."),Some("619042".into()));
        assert_eq!(extract("银行通知","您尾号 8821 的银行卡本次交易验证码：482910，切勿告知他人"),Some("482910".into()));
        assert_eq!(extract("Security","Your code is: 904182"),Some("904182".into()));
    }
    #[test]fn rejects_order_phone_expiry_and_unrelated_numbers(){
        for body in [
            "订单 123456 已发货",
            "验证码有效期为 2026 年，订单号 12345678",
            "验证码为 12345678901",
            "电话 13800138000",
            "photoOTP 123456",
            "error code is 4040",
            "status code: 2001",
            "验证码请看 https://example.com/123456",
            "验证码功能已适配 win11 和 utf8",
            "验证码生成于 2026-11-26",
        ]{
            assert_eq!(extract("",body),None,"{body}");
        }
        assert_eq!(extract("验证码","AB123456CD"),None);
    }
    #[test]fn improves_accuracy_across_zero_width_spacing_prefixes_and_noisy_distractors(){
        // 零宽字符拆字、En-dash 与单数字空格间距
        assert_eq!(extract("安全中心","您的验证码是 482\u{200B}910，请勿转发"),Some("482910".into()));
        assert_eq!(extract("Facebook","Use FB\u{2013}618204 to verify your account"),Some("618204".into()));
        assert_eq!(extract("Apple","您的验证码为：4 8 2 9 1 0"),Some("482910".into()));
        assert_eq!(extract("Steam","您的令牌安全码：#739201，请在 5 分钟内使用"),Some("739201".into()));
        assert_eq!(extract("Cloud","动态口令：12 34 56"),Some("123456".into()));

        // 多干扰数字竞争时精准选出真验证码，排除有效期/热线/版本号/金额/端口
        assert_eq!(
            extract("【某银行】","您正在登录手机银行，验证码为【654321】，请在 10 分钟内完成验证。如有疑问请致电 95588。"),
            Some("654321".into())
        );
        assert_eq!(
            extract("Account Security","Use temporary code 791304 to verify your sign-in. Expires in 15 mins. Version v5230 on port 8080."),
            Some("791304".into())
        );
        assert_eq!(extract("支付通知","修改密码成功，如有疑问请拨打 95588"),None);
        assert_eq!(extract("促销短信","优惠码：pass1234，消费满 $1999 享折扣，倒计时 300s"),None);

        // 验证器与手机连接短信极简通知辅助识别
        assert_eq!(extract_from_notice("Phone Link","安全登录","请确认本次操作：918273"),Some("918273".into()));
        assert_eq!(extract_from_notice("微信","好友消息","明天晚上 1930 见"),None);
    }
}

#[cfg(test)]mod layout_tests{
    use super::*;
    #[test]fn source_digits_and_copy_action_have_separate_safe_lanes(){
        for host in [352.,400.,448.,488.,640.]{let g=layout(Rect::new(host/2.-130.,24.,260.,40.),host);
            for r in [g.icon,g.source,g.digits,g.action]{assert!(r.w>0.&&r.h>0.);assert!(r.x>=g.body.x+16.&&r.x+r.w<=g.body.x+g.body.w-16.);assert!(r.y>=g.body.y+12.&&r.y+r.h<=g.body.y+g.body.h-12.);}
            assert!(g.icon.x+g.icon.w+12.<=g.source.x);
            assert!(g.source.y+g.source.h+3.<=g.digits.y);
            assert!(g.digits.x+g.digits.w+14.<=g.action.x);
        }
    }
    #[test]fn mini_and_classic_reveal_fixed_content_without_squeezing_text(){
        for base in [Rect::new(114.,24.,260.,40.),Rect::new(194.,24.,100.,18.)]{
            let mut hub=crate::capsule_alert::Hub::default();hub.enqueue(crate::capsule_alert::Alert::verification("test","测试应用","12345678".into()));
            assert!(hub.begin(base,base.h/2.,Rect::new(0.,0.,488.,332.),24.,0.,true));
            let expected=layout(Rect::new(52.,24.,WIDTH,HEIGHT),488.);
            for i in 0..120{let now=i as f64/120.;let(shell,_,mix)=hub.banner.presentation(now);let g=layout(shell,488.);
                assert!((g.source.x-expected.source.x).abs()<0.001);assert_eq!(g.source.y,expected.source.y);assert_eq!(g.digits,expected.digits);assert_eq!(g.action,expected.action);
                assert!((0.0..=1.0).contains(&content_alpha(shell.h,mix)));
            }
            hub.dismiss(base,base.h/2.,0.5,true);hub.tick(base,base.h/2.,2.,true);assert!(!hub.visible());
            let(shell,_,mix)=hub.banner.presentation(2.);assert!(content_alpha(shell.h,mix)<0.001);
        }
        assert_eq!(content_alpha(18.,1.),0.);assert_eq!(content_alpha(HEIGHT,1.),1.);
    }
    #[test]fn supported_code_lengths_keep_leading_zeroes_and_use_one_payload(){
        for code in ["0042","002468","00135790"]{let a=crate::capsule_alert::Alert::verification("test","验证测试",code.into());
            assert_eq!(a.code.as_deref(),Some(code));assert!(a.trailing.is_empty());assert_eq!((a.width,a.height),(WIDTH,HEIGHT));
        }
    }
}
