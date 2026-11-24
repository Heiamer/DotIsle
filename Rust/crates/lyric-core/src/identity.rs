//! 高准确度曲目身份识别与防跨版本误配评分引擎（借鉴开源歌词匹配最佳实践）。
use crate::models::*;
use regex::Regex;
use std::collections::BTreeSet;
use std::sync::LazyLock;
use unicode_normalization::UnicodeNormalization;

/// 常见曲名高频繁简与异体字补充映射（保障跨系统/跨平台一致归一化）。
fn fold_trad_char(c: char) -> char {
    match c {
        '愛' => '爱', '夢' => '梦', '風' => '风', '聽' => '听', '說' => '说',
        '雲' => '云', '聲' => '声', '畫' => '画', '國' => '国', '時' => '时',
        '淚' => '泪', '歲' => '岁', '飛' => '飞', '來' => '来', '後' => '后',
        '開' => '开', '關' => '关', '見' => '见', '裡' => '里', '為' => '为',
        '無' => '无', '與' => '与', '這' => '这', '還' => '还', '錯' => '错',
        '戀' => '恋', '願' => '愿', '離' => '离', '別' => '别', '誰' => '谁',
        '點' => '点', '燈' => '灯', '櫻' => '樱', '藍' => '蓝', '樂' => '乐',
        '寫' => '写', '葉' => '叶', '氣' => '气', '華' => '华', '語' => '语',
        '書' => '书', '東' => '东', '車' => '车', '長' => '长', '門' => '门',
        '問' => '问', '間' => '间', '銀' => '银', '鐵' => '铁', '鎮' => '镇',
        '鏡' => '镜', '響' => '响', '飲' => '饮', '體' => '体', '龍' => '龙',
        '鳳' => '凤', '鳥' => '鸟', '鳴' => '鸣', '麗' => '丽', '齊' => '齐',
        '萬' => '万', '雙' => '双', '隻' => '只', '黃' => '黄', '邊' => '边',
        '達' => '达', '過' => '过', '選' => '选', '轉' => '转', '輕' => '轻',
        '輝' => '辉', '輪' => '轮', '動' => '动', '勝' => '胜', '實' => '实',
        '寬' => '宽', '寶' => '宝', '導' => '导', '專' => '专', '將' => '将',
        '應' => '应', '懷' => '怀', '戰' => '战', '戲' => '戏', '擁' => '拥',
        '擇' => '择', '斷' => '断', '舊' => '旧', '極' => '极', '條' => '条',
        '榮' => '荣', '標' => '标', '樣' => '样', '歡' => '欢', '歸' => '归',
        '殘' => '残', '溫' => '温', '滅' => '灭', '滿' => '满', '獨' => '独',
        '現' => '现', '環' => '环', '產' => '产', '異' => '异', '發' => '发',
        '盡' => '尽', '眾' => '众', '確' => '确', '禮' => '礼', '稱' => '称',
        '種' => '种', '積' => '积', '穩' => '稳', '簡' => '简', '紅' => '红',
        '約' => '约', '純' => '纯', '紙' => '纸', '終' => '终', '結' => '结',
        '絕' => '绝', '維' => '维', '網' => '网', '綠' => '绿', '線' => '线',
        '總' => '总', '織' => '织', '緣' => '缘', '續' => '续', '羅' => '罗',
        '義' => '义', '習' => '习', '聯' => '联', '舉' => '举', '落' => '落',
        '藥' => '药', '虛' => '虚', '補' => '补', '裝' => '装', '親' => '亲',
        '覺' => '觉', '觀' => '观', '記' => '记', '設' => '设', '許' => '许',
        '訴' => '诉', '詞' => '词', '詩' => '诗', '試' => '试', '話' => '话',
        '認' => '认', '誤' => '误', '請' => '请', '諾' => '诺', '讀' => '读',
        '變' => '变', '讓' => '讓', '貝' => '贝', '貴' => '贵', '費' => '费',
        '買' => '买', '賣' => '卖', '賞' => '赏', '賽' => '赛', '贈' => '赠',
        '贊' => '赞', '贏' => '赢', '連' => '连', '進' => '进', '遠' => '远',
        '遲' => '迟', '遺' => '遗', '郵' => '邮', '鄉' => '乡', '醒' => '醒',
        '隨' => '随', '隱' => '隐', '難' => '难', '電' => '电', '霧' => '雾',
        '靜' => '静', '頁' => '页', '頂' => '顶', '順' => '顺', '須' => '须',
        '領' => '领', '題' => '题', '類' => '类', '飄' => '飘', '館' => '馆',
        '馬' => '马', '駕' => '驾', '驅' => '驱', '驚' => '惊', '髮' => '发',
        '魚' => '鱼', '麥' => '麦', '齒' => '齿', '桜' => '樱', '涙' => '泪',
        '駅' => '驿', '伝' => '传', '広' => '广', '声' => '声', '恋' => '恋',
        '嵐' => '岚', '星' => '星',
        other => other,
    }
}

pub fn normalized(s: &str) -> String {
    let s = simplified(s);
    s.nfkd()
        .map(fold_trad_char)
        .flat_map(char::to_lowercase)
        .filter(|c| c.is_alphanumeric())
        .collect()
}

#[cfg(windows)]
fn simplified(s: &str) -> String {
    #[link(name = "kernel32")]
    extern "system" {
        fn LCMapStringW(
            locale: u32,
            flags: u32,
            src: *const u16,
            n: i32,
            dst: *mut u16,
            m: i32,
        ) -> i32;
    }
    let src = s.encode_utf16().collect::<Vec<_>>();
    if src.is_empty() {
        return String::new();
    }
    let mut dst = vec![0; src.len() * 3];
    let n = unsafe {
        LCMapStringW(
            0x0804,
            0x02000000,
            src.as_ptr(),
            src.len() as i32,
            dst.as_mut_ptr(),
            dst.len() as i32,
        )
    };
    if n > 0 {
        String::from_utf16_lossy(&dst[..n as usize])
    } else {
        s.into()
    }
}
#[cfg(not(windows))]
fn simplified(s: &str) -> String {
    s.chars().map(fold_trad_char).collect()
}

pub fn search_text(s: &str) -> String {
    simplified(s)
        .nfkc()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn artists(s: &str) -> Vec<String> {
    static COLLAB: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\s+(?:feat\.?|ft\.?|featuring|with|vs\.?|x)\s+").unwrap()
    });
    let s = COLLAB.replace_all(s, "/");
    s.split(['/', '&', '、', ',', '，', ';', '；', '／'])
        .map(str::trim)
        .filter(|s| {
            !s.is_empty()
                && !matches!(
                    s.to_lowercase().as_str(),
                    "unknown artist" | "unknown" | "未知歌手" | "未知艺术家" | "群星" | "various artists"
                )
        })
        .map(str::to_owned)
        .collect()
}

fn title_suffix(s: &str) -> Option<(&str, &str)> {
    let s = s.trim();
    for (left, right) in [('(', ')'), ('（', '）'), ('[', ']'), ('【', '】')] {
        if let Some((base, suffix)) = s.strip_suffix(right).and_then(|s| s.rsplit_once(left)) {
            return Some((base.trim(), suffix.trim()));
        }
    }
    s.rsplit_once(" - ")
        .or_else(|| s.rsplit_once(" – "))
        .or_else(|| s.rsplit_once(" — "))
}

/// 识别并抽离标题尾部自带的 `(feat. xxx)` / `ft. xxx` 合作歌手，避免污染歌名并补全歌手集合。
fn extract_title_features(raw: &str) -> (String, Vec<String>) {
    static BRACKET_FEAT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\s*[\(\[（【]\s*(?:feat\.?|ft\.?|featuring|with)\s+([^\)\]）】]+)[\)\]）】]\s*$").unwrap()
    });
    static INLINE_FEAT: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?i)\s+(?:feat\.?|ft\.?|featuring)\s+([^\(\[\-–—]+)$").unwrap()
    });
    let mut title = raw.trim().to_owned();
    let mut feats = Vec::new();
    for _ in 0..2 {
        if let Some(cap) = BRACKET_FEAT.captures(&title) {
            let extra = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            feats.extend(artists(extra));
            title = title[..cap.get(0).unwrap().start()].trim().to_owned();
            continue;
        }
        if let Some(cap) = INLINE_FEAT.captures(&title) {
            let extra = cap.get(1).map(|m| m.as_str()).unwrap_or("");
            feats.extend(artists(extra));
            title = title[..cap.get(0).unwrap().start()].trim().to_owned();
            continue;
        }
        break;
    }
    (title, feats)
}

fn presentation_suffix(s: &str) -> bool {
    static HARMLESS_SUFFIX: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(concat!(
            r"(?i)^(?:",
            r"(?:official\s+)?(?:music\s+)?(?:video|audio|mv|lyric\s+video)|",
            r"(?:\d{2,4}\s+)?remaster(?:ed)?(?:\s+\d{2,4})?|",
            r"(?:super\s+)?deluxe(?:\s+edition)?|(?:expanded|anniversary|collector'?s|special)\s+edition|",
            r"explicit|clean|mono|stereo|album\s+version|single\s+version|original\s+(?:version|mix)|",
            r"radio\s+edit|bonus\s+track|prod\.?\s+by\s+.+|hd|hq|mv|",
            r"from\s+.+|feat\.?.+|ft\.?.+|",
            r"官方(?:mv|\s*mv|歌词版|版|音频|完整版)?|歌词版|完整版|高清版|无损(?:音质)?|",
            r"(?:电影|电视剧|网剧|动画|动漫|游戏|影视)?《?[^》]*》?(?:主题曲|片尾曲|片头曲|插曲|推广曲|宣传曲|概念曲|原声带|原声|ost)",
            r")$"
        ))
        .unwrap()
    });
    let clean = search_text(s).to_lowercase();
    HARMLESS_SUFFIX.is_match(clean.trim())
}

fn strip_presentation_tail(mut s: &str) -> &str {
    while let Some((base, suffix)) = title_suffix(s) {
        if base.is_empty() || !presentation_suffix(suffix) {
            break;
        }
        s = base;
    }
    s
}

/// 抽取曲目的纯净标题及全部分身别名（如括号副标题、斜杠双语标题）。
fn clean_title_variants(raw: &str) -> (String, Vec<String>, Vec<String>) {
    let (without_feat, feats) = extract_title_features(raw);
    let clean = strip_presentation_tail(&without_feat).trim().to_owned();
    let mut variants = Vec::new();
    let mut push_var = |v: &str| {
        let t = strip_presentation_tail(v).trim();
        if !t.is_empty() && !variants.iter().any(|x: &String| x.eq_ignore_ascii_case(t)) {
            variants.push(t.to_owned());
        }
    };
    push_var(&clean);
    if let Some((base, tail)) = title_suffix(&clean) {
        if !base.is_empty() && !tail.is_empty() && versions(tail).is_empty() {
            push_var(base);
            push_var(tail);
        }
    }
    for sep in [" / ", " ／ ", " | "] {
        if clean.contains(sep) {
            for part in clean.split(sep) {
                push_var(part);
            }
        }
    }
    (clean, variants, feats)
}

pub fn search_title(s: &str) -> &str {
    let title = strip_presentation_tail(s);
    let stripped = title_suffix(title)
        .filter(|(base, suffix)| {
            !base.is_empty()
                && (!versions(suffix).is_empty()
                    || suffix.to_lowercase().starts_with("feat")
                    || suffix.to_lowercase().starts_with("ft."))
        })
        .map(|(base, _)| base)
        .unwrap_or(title);
    stripped.trim()
}

fn similarity(a: &str, b: &str) -> f64 {
    let a = normalized(a).chars().take(256).collect::<Vec<_>>();
    let b = normalized(b).chars().take(256).collect::<Vec<_>>();
    if a.is_empty() || b.is_empty() {
        return 0.;
    }
    let mut row = (0..=b.len()).collect::<Vec<_>>();
    for (i, x) in a.iter().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for (j, y) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (prev + usize::from(x != y)).min(row[j] + 1).min(old + 1);
            prev = old;
        }
    }
    1. - row[b.len()] as f64 / a.len().max(b.len()) as f64
}

/// 严格的跨版本污染检测标签（Live/Remix/DJ/伴奏/翻唱/变速/纯音乐等绝不能互相误配）。
fn versions(s: &str) -> BTreeSet<&'static str> {
    let s = search_text(s).to_lowercase();
    const TAGS: [(&str, &str); 36] = [
        ("live", "live"),
        ("现场", "live"),
        ("演唱会", "live"),
        ("concert", "live"),
        ("unplugged", "acoustic"),
        ("不插电", "acoustic"),
        ("acoustic", "acoustic"),
        ("remix", "remix"),
        ("混音", "remix"),
        ("mashup", "remix"),
        ("bootleg", "remix"),
        ("dj", "dj"),
        ("电音", "dj"),
        ("慢摇", "dj"),
        ("串烧", "dj"),
        ("伴奏", "instrumental"),
        ("instrumental", "instrumental"),
        ("off vocal", "instrumental"),
        ("karaoke", "instrumental"),
        ("ktv", "instrumental"),
        ("inst", "instrumental"),
        ("纯音乐", "instrumental"),
        ("钢琴版", "piano"),
        ("piano ver", "piano"),
        ("吉他版", "guitar"),
        ("翻唱", "cover"),
        ("cover", "cover"),
        ("demo", "demo"),
        ("试听", "demo"),
        ("sped up", "speed"),
        ("speed up", "speed"),
        ("nightcore", "speed"),
        ("变速", "speed"),
        ("slowed", "slow"),
        ("降调", "slow"),
        ("acapella", "acapella"),
    ];
    TAGS.into_iter()
        .filter_map(|(word, tag)| {
            let matched = if word.is_ascii() {
                s.match_indices(word).any(|(i, _)| {
                    !s[..i].ends_with(|c: char| c.is_ascii_alphanumeric())
                        && !s[i + word.len()..].starts_with(|c: char| c.is_ascii_alphanumeric())
                })
            } else {
                s.contains(word)
            };
            matched.then_some(tag)
        })
        .collect()
}

pub fn official(source: &str) -> Option<Provider> {
    let source = source.to_lowercase();
    [
        ("netease", "Netease"),
        ("cloudmusic", "Netease"),
        ("qqmusic", "QQMusic"),
        ("kugou", "Kugou"),
        ("kuwo", "Kuwo"),
        ("migu", "Migu"),
        ("sodamusic", "SodaMusic"),
        ("汽水音乐", "SodaMusic"),
    ]
    .into_iter()
    .find_map(|(s, p)| source.contains(s).then(|| Provider::new(p)))
}

pub fn track_key(t: &Track) -> String {
    serde_json::to_string(&(
        normalized(&t.source_app),
        &t.song_id,
        normalized(&t.title),
        normalized(&t.artist),
        normalized(&t.album),
    ))
    .unwrap_or_default()
}

pub fn rate(t: &Track, h: &Hit) -> Option<u8> {
    if versions(&t.title) != versions(&h.title) {
        return None;
    }
    // 额外校验劣质专辑标签污染（原唱歌曲禁止落入 DJ热歌/伴奏合集 等专辑）
    if versions(&t.album).is_empty() {
        let hit_album_vers = versions(&h.album);
        if hit_album_vers.contains("dj")
            || hit_album_vers.contains("instrumental")
            || hit_album_vers.contains("cover")
            || hit_album_vers.contains("speed")
        {
            return None;
        }
    }

    let (_, mut t_variants, mut t_feats) = clean_title_variants(&t.title);
    let (_, mut h_variants, mut h_feats) = clean_title_variants(&h.title);
    if let Some(alias) = h.extra.get("alias").filter(|s| !s.trim().is_empty()) {
        for part in alias.split(['/', ';', '；']) {
            let cleaned = strip_presentation_tail(part).trim();
            if !cleaned.is_empty() {
                h_variants.push(cleaned.to_owned());
            }
        }
    }
    if t_variants.is_empty() {
        t_variants.push(t.title.clone());
    }
    if h_variants.is_empty() {
        h_variants.push(h.title.clone());
    }

    // 主标题与别名之间求最大相似度，防止无害后缀/翻译别名被错误淘汰
    let mut title = similarity(strip_presentation_tail(&t.title), strip_presentation_tail(&h.title));
    for tv in &t_variants {
        for hv in &h_variants {
            let sim = similarity(tv, hv);
            if sim > title {
                title = sim;
            }
        }
    }
    // 至少要求标题较高一致（允许双语/标点轻微差异，不再在 0.83 一刀切）
    if title < 0.72 {
        return None;
    }

    let delta = (!t.duration.is_zero() && !h.duration.is_zero())
        .then(|| t.duration.as_secs_f64().abs_diff(h.duration.as_secs_f64()));
    if delta.is_some_and(|d| d > 20.) {
        return None;
    }

    let mut wanted = artists(&t.artist);
    wanted.append(&mut t_feats);
    let mut available = h
        .artists
        .iter()
        .flat_map(|a| artists(a))
        .collect::<Vec<_>>();
    available.append(&mut h_feats);

    let compare = |a: &str, b: &str| {
        fn alias(s: &str) -> Option<(&str, &str)> {
            title_suffix(s)
                .filter(|(base, tail)| !base.is_empty() && !tail.is_empty() && !presentation_suffix(tail))
        }
        let a_parts = alias(a).unwrap_or((a, a));
        let b_parts = alias(b).unwrap_or((b, b));
        let na = normalized(a);
        let nb = normalized(b);
        let sub_bonus = if !na.is_empty()
            && !nb.is_empty()
            && na.chars().count() >= 2
            && nb.chars().count() >= 2
            && (na == nb)
        {
            1.0
        } else {
            0.
        };
        [
            sub_bonus,
            similarity(a, b),
            similarity(a_parts.0, b_parts.0),
            similarity(a_parts.0, b_parts.1),
            similarity(a_parts.1, b_parts.0),
        ]
        .into_iter()
        .fold(0., f64::max)
    };

    let artist = if wanted.is_empty() || available.is_empty() {
        // 缺少歌手信息时严控安全底线：要求标题极高一致且时长差 <= 3s，杜绝同名异人误配
        if title < 0.96 || !delta.is_some_and(|d| d <= 3.) {
            return None;
        }
        0.78
    } else {
        let best_in = |a: &str, group: &[String]| {
            group.iter().map(|b| compare(a, b)).fold(0., f64::max)
        };
        let recall = wanted.iter().map(|a| best_in(a, &available)).sum::<f64>() / wanted.len() as f64;
        let precision =
            available.iter().map(|b| best_in(b, &wanted)).sum::<f64>() / available.len() as f64;
        let max_pair = wanted
            .iter()
            .map(|a| best_in(a, &available))
            .fold(0., f64::max);
        let both = if recall >= 0.78 && precision >= 0.75 {
            (recall + precision) * 0.5
        } else if title >= 0.96
            && delta.is_some_and(|d| d <= 4.)
            && max_pair >= 0.95
            && available.iter().all(|b| best_in(b, &wanted) >= 0.85)
        {
            0.84
        } else if title >= 0.96
            && delta.is_some_and(|d| d <= 3.)
            && max_pair >= 0.96
            && wanted.len() == 1
            && available.len() > 1
            && best_in(&wanted[0], &available) >= 0.96
        {
            // 仅缺失一个合唱后缀但第一顺位主歌手全等且歌名时长精吻合
            0.80
        } else {
            return None;
        };
        both
    };

    // 时长分段精确评估曲线（参考 BetterLyrics / LRCLIB：2s 内极高可信，>12s 大幅递减）
    let duration = match delta {
        None => 0.62,
        Some(d) if d <= 2.5 => 1.0,
        Some(d) if d <= 5.5 => 0.85,
        Some(d) if d <= 10.0 => 0.55,
        Some(d) if d <= 15.0 => 0.25,
        Some(_) => {
            // 超过 15s 仅当歌手与歌名近乎完美一致才放行，否则淘汰
            if title < 0.96 || artist < 0.92 {
                return None;
            }
            0.05
        }
    };

    let album = if t.album.is_empty() || h.album.is_empty() {
        0.5
    } else {
        let a = strip_presentation_tail(&t.album);
        let b = strip_presentation_tail(&h.album);
        similarity(a, b).max(similarity(&t.album, &h.album))
    };

    Some(
        (title * 54. + artist * 26. + duration * 13. + album * 7.)
            .round()
            .clamp(0., 100.) as u8,
    )
}

trait Difference {
    fn abs_diff(self, other: Self) -> Self;
}
impl Difference for f64 {
    fn abs_diff(self, other: Self) -> Self {
        (self - other).abs()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn identity_rejects_variants_and_performers() {
        let t = Track::new("a", "轨道", "甲", "", "QQMusic", Duration::from_secs(204));
        let mut h = Hit {
            provider: Provider::new("QQMusic"),
            id: "b".into(),
            title: "轨道".into(),
            artists: vec!["甲".into()],
            album: String::new(),
            duration: t.duration,
            extra: Default::default(),
        };
        assert!(rate(&t, &h).unwrap() >= 95);
        h.title = "轨道 (Live)".into();
        assert!(rate(&t, &h).is_none());
        h.title = "轨道 (DJ版)".into();
        assert!(rate(&t, &h).is_none());
        h.title = "轨道 (伴奏)".into();
        assert!(rate(&t, &h).is_none());
        h.title = "轨道 (Sped Up)".into();
        assert!(rate(&t, &h).is_none());
        h.title = t.title.clone();
        h.artists = vec!["乙".into()];
        assert!(rate(&t, &h).is_none());
    }

    #[test]
    fn punctuation_diacritics_and_collaborations() {
        assert_eq!(normalized("Café：夜！"), normalized("cafe 夜"));
        assert_eq!(normalized("愛情轉移"), normalized("爱情转移"));
        assert_eq!(artists("甲 / 乙、丙"), ["甲", "乙", "丙"]);
    }

    #[test]
    fn decorations_and_partial_collaboration_match_without_crossing_versions() {
        let mut t = Track::new(
            "a",
            "夜航（Official Video）",
            "甲 feat. 乙",
            "",
            "Browser",
            Duration::from_secs(201),
        );
        let mut h = Hit {
            provider: Provider::new("SodaMusic"),
            id: "b".into(),
            title: "夜航".into(),
            artists: vec!["甲".into()],
            album: String::new(),
            duration: Duration::from_secs(200),
            extra: Default::default(),
        };
        assert!(rate(&t, &h).is_some());
        h.artists.push("丙".into());
        assert!(rate(&t, &h).is_none());
        h.artists = vec!["甲".into(), "乙".into()];
        h.title = "夜航 - Live".into();
        assert!(rate(&t, &h).is_none());
        h.title = "夜航".into();
        t.artist = "未知歌手".into();
        assert!(rate(&t, &h).is_some());
        h.duration = Duration::from_secs(207);
        assert!(rate(&t, &h).is_none());
    }

    #[test]
    fn title_feat_and_remaster_ost_and_bilingual_aliases_match_accurately() {
        // 1. 歌名携带 (feat. xxx) + (2023 Remastered) 与纯净结果精准匹配
        let t = Track::new(
            "1",
            "Stay (feat. Justin Bieber) [2023 Remaster]",
            "The Kid LAROI",
            "F*CK LOVE 3",
            "Netease",
            Duration::from_secs(141),
        );
        let h = Hit {
            provider: Provider::new("LRCLIB"),
            id: "2".into(),
            title: "Stay".into(),
            artists: vec!["The Kid LAROI".into(), "Justin Bieber".into()],
            album: "F*CK LOVE 3".into(),
            duration: Duration::from_secs(142),
            extra: Default::default(),
        };
        assert!(rate(&t, &h).unwrap() >= 93);

        // 2. 影视原声带后缀剥离与匹配
        let ost = Track::new(
            "3",
            "如愿（电影《我和我的父辈》推广曲）",
            "王菲",
            "",
            "QQMusic",
            Duration::from_secs(265),
        );
        let ost_clean = Hit {
            provider: Provider::new("Netease"),
            id: "4".into(),
            title: "如愿".into(),
            artists: vec!["王菲".into()],
            album: String::new(),
            duration: Duration::from_secs(265),
            extra: Default::default(),
        };
        assert!(rate(&ost, &ost_clean).unwrap() >= 95);

        // 3. 中日双语副标题 / extra.alias 交叉匹配
        let jp = Track::new(
            "5",
            "夜に駆ける",
            "YOASOBI",
            "THE BOOK",
            "Netease",
            Duration::from_secs(261),
        );
        let mut extra = std::collections::BTreeMap::new();
        extra.insert("alias".into(), "向夜晚奔去 / 夜に駆ける".into());
        let cn_hit = Hit {
            provider: Provider::new("QQMusic"),
            id: "6".into(),
            title: "向夜晚奔去 (Racing into the Night)".into(),
            artists: vec!["YOASOBI".into()],
            album: "THE BOOK".into(),
            duration: Duration::from_secs(261),
            extra,
        };
        assert!(rate(&jp, &cn_hit).unwrap() >= 90);
    }

    #[test]
    fn dj_album_or_wrong_duration_never_pollutes_clean_track() {
        let t = Track::new("7", "晴天", "周杰伦", "叶惠美", "QQMusic", Duration::from_secs(269));
        let mut dj_hit = Hit {
            provider: Provider::new("Kugou"),
            id: "8".into(),
            title: "晴天".into(),
            artists: vec!["周杰伦".into()],
            album: "车载DJ慢摇串烧精选".into(),
            duration: Duration::from_secs(269),
            extra: Default::default(),
        };
        assert!(rate(&t, &dj_hit).is_none());
        dj_hit.album = "叶惠美".into();
        dj_hit.duration = Duration::from_secs(295);
        assert!(rate(&t, &dj_hit).is_none());
    }

    #[test]
    fn identity_changes_with_song_and_artist() {
        let mut t = Track::new("a", "同名", "甲", "", "Netease", Duration::ZERO);
        let a = track_key(&t);
        t.artist = "乙".into();
        assert_ne!(a, track_key(&t));
    }
}

