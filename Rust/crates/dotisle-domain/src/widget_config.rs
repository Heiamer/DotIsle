//! Per-placement content. Empty values represent an unconfigured widget.
use crate::desktop_widgets::{Date, Kind};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Config {
    pub title: String,
    pub plugin_id: String,
    pub text: String,
    pub date: String,
    pub deadline: String,
    pub city: String,
    pub target: String,
    pub drive: String,
    /// 相框小组件的本地图片目录或单张图片路径。
    pub folder: String,
    /// 拍立得 / 相框底部手写题字。
    pub caption: String,
    /// 可选自定义主题强调色（`#RRGGBB`）。
    pub accent: String,
    /// 暂存区 / 快捷启动列表条目。
    pub items: Vec<String>,
}
impl Config {
    pub fn validate(&self, kind: Kind) -> Result<(), String> {
        if self.plugin_id.len()>64 || (kind==Kind::Plugin && (self.plugin_id.is_empty() || !self.plugin_id.bytes().all(|c|c.is_ascii_lowercase()||c.is_ascii_digit()||c==b'-'||c==b'.'))) {return Err("插件标识无效".into())}
        if self.title.chars().count() > 60
            || self.text.chars().count() > 500
            || self.city.chars().count() > 100
            || self.target.len() > 2048
            || self.folder.len() > 2048
            || self.caption.chars().count() > 120
            || self.accent.len() > 16
            || self.items.len() > 16
            || self.items.iter().any(|item| {
                item.len() > 1024 || item.chars().any(|c| c == '\0' || c.is_control())
            })
            || [
                &self.title,
                &self.text,
                &self.date,
                &self.deadline,
                &self.city,
                &self.target,
                &self.drive,
                &self.folder,
                &self.caption,
                &self.accent,
            ]
            .iter()
            .any(|v| {
                v.chars()
                    .any(|c| c == '\0' || (c.is_control() && c != '\n'))
            })
        {
            return Err("组件内容过长或包含无效字符".into());
        }
        if !self.accent.is_empty() {
            let hex = self.accent.trim().trim_start_matches('#');
            if hex.len() != 6 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err("自定义强调色格式无效，请使用 #RRGGBB".into());
            }
        }
        if kind == Kind::Anniversary && !self.date.is_empty() && date_ordinal(&self.date).is_none()
        {
            return Err("请填写有效日期，如 2027-01-01".into());
        }
        if kind == Kind::Schedule && !self.deadline.is_empty() && deadline_minutes(&self.deadline).is_none() {
            return Err("请填写有效日期与时间，如 2027-01-01 09:00".into());
        }
        if kind == Kind::WorldClock && !self.city.is_empty() && timezone(&self.city).is_none() {
            return Err("请选择北京、东京、伦敦、纽约、悉尼或 UTC".into());
        }
        if kind == Kind::Storage && !self.drive.is_empty() && drive_root(&self.drive).is_none() {
            return Err("请填写盘符，如 C: 或 D:".into());
        }
        if kind == Kind::Launcher && !self.target.is_empty() && !launch_target(&self.target) {
            return Err("请选择程序、文件夹或 HTTPS 网页，不支持命令参数".into());
        }
        Ok(())
    }
}
pub fn launch_target(value: &str) -> bool {
    if value.contains(['\0', '\r', '\n', '"']) {
        return false;
    }
    if let Some(host) = value.strip_prefix("https://") {
        return !host.is_empty()
            && !host.starts_with('/')
            && !host.contains('@')
            && !host.chars().any(char::is_whitespace);
    }
    let path = std::path::Path::new(value);
    path.is_absolute()
        && (path.is_dir()
            || path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("lnk")))
}
pub fn drive_root(value: &str) -> Option<String> {
    let bytes = value.as_bytes();
    (bytes.len() >= 2
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && value[2..].chars().all(|c| c == '\\' || c == '/'))
    .then(|| format!("{}:\\", (bytes[0] as char).to_ascii_uppercase()))
}
pub fn timezone(city: &str) -> Option<&'static str> {
    match city.trim() {
        "北京" | "上海" => Some("China Standard Time"),
        "东京" => Some("Tokyo Standard Time"),
        "伦敦" => Some("GMT Standard Time"),
        "纽约" => Some("Eastern Standard Time"),
        "悉尼" => Some("AUS Eastern Standard Time"),
        "UTC" => Some("UTC"),
        _ => None,
    }
}
pub fn ordinal(date: Date) -> i64 {
    let y = date.year as i64 - 1;
    let mut days = y * 365 + y / 4 - y / 100 + y / 400;
    for month in 1..date.month {
        days += Date { month, ..date }.days() as i64;
    }
    days + date.day as i64 - 1
}
pub fn date_ordinal(value: &str) -> Option<i64> {
    let parts = value
        .split('-')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if parts.len() != 3 || !(1900..=9999).contains(&parts[0]) || !(1..=12).contains(&parts[1]) {
        return None;
    }
    let date = Date {
        year: parts[0] as i32,
        month: parts[1],
        day: parts[2],
        hour: 0,
        minute: 0,
        second: 0,
    };
    (date.day >= 1 && date.day <= date.days()).then(|| ordinal(date))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_validation_respects_leap_days() {
        assert!(date_ordinal("2028-02-29").is_some());
        assert!(date_ordinal("2027-02-29").is_none());
        assert_eq!(
            date_ordinal("2028-03-01").unwrap() - date_ordinal("2028-02-28").unwrap(),
            2
        );
    }
    #[test]
    fn unsafe_launch_strings_and_partial_drives_are_rejected() {
        for target in [
            "javascript:alert(1)",
            "https://",
            "https://user@example.com",
            "cmd.exe /c test",
            "https://x\nheader",
        ] {
            assert!(!launch_target(target));
        }
        assert!(launch_target("https://example.com/path"));
        assert_eq!(drive_root("d:"), Some("D:\\".into()));
        assert!(drive_root("C:\\folder").is_none());
    }
}

/// Local wall-clock deadline. Calendar arithmetic also handles DST day boundaries.
pub fn deadline_minutes(raw: &str) -> Option<i64> {
    if raw.len()!=16 || !raw.is_ascii() || raw.as_bytes()[10]!=b' ' || raw.as_bytes()[13]!=b':' { return None }
    let day=date_ordinal(&raw[..10])?;
    let h=raw[11..13].parse::<i64>().ok()?; let m=raw[14..16].parse::<i64>().ok()?;
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(day*1440+h*60+m)
}
