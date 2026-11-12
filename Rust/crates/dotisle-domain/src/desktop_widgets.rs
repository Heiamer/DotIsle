//! Persisted desktop placements. The native process is the sole writer.
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Kind {
    Clock,
    Calendar,
    Todos,
    Focus,
    Music,
    System,
    Note,
    Anniversary,
    Weather,
    Today,
    Battery,
    Storage,
    WorldClock,
    Launcher,
    FocusStats,
}
impl Kind {
    pub const ALL: [Self; 15] = [
        Self::Clock,
        Self::Calendar,
        Self::Todos,
        Self::Focus,
        Self::Music,
        Self::System,
        Self::Note,
        Self::Anniversary,
        Self::Weather,
        Self::Today,
        Self::Battery,
        Self::Storage,
        Self::WorldClock,
        Self::Launcher,
        Self::FocusStats,
    ];
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|k| *k == self).unwrap()
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Clock => "时钟",
            Self::Calendar => "日历",
            Self::Todos => "待办",
            Self::Focus => "专注计时",
            Self::Music => "音乐",
            Self::System => "系统状态",
            Self::Note => "便签",
            Self::Anniversary => "纪念日倒数",
            Self::Weather => "天气",
            Self::Today => "今日概览",
            Self::Battery => "电量",
            Self::Storage => "存储空间",
            Self::WorldClock => "世界时钟",
            Self::Launcher => "快捷启动",
            Self::FocusStats => "专注统计",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Clock => "让时间，慢慢走",
            Self::Calendar => "今天，也值得期待",
            Self::Todos => "给重要的小事一个位置",
            Self::Focus => "留一段时间给自己",
            Self::Music => "让喜欢的歌近一点",
            Self::System => "电脑的状态，一眼知晓",
            Self::Note => "随手记下，不必打开另一个窗口",
            Self::Anniversary => "让重要的日子一直在眼前",
            Self::Weather => "抬眼看看城市的天气",
            Self::Today => "今天的安排与正在进行的事",
            Self::Battery => "电量与供电状态，清楚可见",
            Self::Storage => "为文件留出一点余地",
            Self::WorldClock => "把远方的时间放在身边",
            Self::Launcher => "常用程序、文件夹和网页，一点即达",
            Self::FocusStats => "把投入的时间，慢慢积累起来",
        }
    }
    pub fn default_size(self) -> Size {
        if matches!(self, Self::Clock | Self::Calendar) {
            Size::Small
        } else {
            Size::Medium
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Size {
    #[default]
    Small,
    Medium,
    Large,
}
impl Size {
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];
    pub fn label(self) -> &'static str {
        match self {
            Self::Small => "小",
            Self::Medium => "中",
            Self::Large => "大",
        }
    }
    pub fn dimensions(self) -> (f64, f64) {
        match self {
            Self::Small => (176., 176.),
            Self::Medium => (352., 176.),
            Self::Large => (352., 352.),
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum Style {
    #[default]
    #[serde(alias = "nothing", alias = "Nothing")]
    Original,
    Miuix,
    Terminal,
    Pixel,
    Gallery,
}
impl Style {
    pub const ALL: [Self; 5] = [
        Self::Original,
        Self::Miuix,
        Self::Terminal,
        Self::Pixel,
        Self::Gallery,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::Original => "默认",
            Self::Miuix => "MIUIX",
            Self::Terminal => "终端",
            Self::Pixel => "像素",
            Self::Gallery => "留白",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Placement {
    pub id: String,
    pub kind: Kind,
    #[serde(default)]
    pub size: Size,
    #[serde(default)]
    pub style: Style,
    #[serde(default)]
    pub config: crate::widget_config::Config,
    pub x: i32,
    pub y: i32,
    #[serde(default = "initial_revision")]
    pub revision: u64,
}
fn initial_revision() -> u64 {
    1
}
fn coordinate(v: &Value, key: &str, default: i32) -> Result<i32, String> {
    match v.get(key) {
        None => Ok(default),
        Some(v) => v
            .as_i64()
            .filter(|n| (-131072..=131072).contains(n))
            .map(|n| n as i32)
            .ok_or_else(|| "组件位置无效".into()),
    }
}
fn valid(p: &Placement) -> bool {
    !p.id.is_empty()
        && p.id.len() <= 96
        && p.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        && p.config.validate(p.kind).is_ok()
        && (-131072..=131072).contains(&p.x)
        && (-131072..=131072).contains(&p.y)
}
pub struct Store {
    path: PathBuf,
    pub items: Vec<Placement>,
    pub error: Option<String>,
    serial: u64,
}
impl Store {
    pub const LIMIT: usize = 16;
    pub fn new(path: PathBuf) -> Self {
        let mut s = Self {
            path,
            items: vec![],
            error: None,
            serial: 0,
        };
        if s.path.is_file() {
            match std::fs::read(&s.path)
                .map_err(|e| e.to_string())
                .and_then(|b| {
                    serde_json::from_slice::<Vec<Placement>>(&b).map_err(|e| e.to_string())
                }) {
                Ok(items)
                    if items.len() <= Self::LIMIT
                        && items.iter().all(valid)
                        && items
                            .iter()
                            .enumerate()
                            .all(|(i, p)| !items[..i].iter().any(|a| a.id == p.id)) =>
                {
                    s.items = items
                }
                _ => s.error = Some("桌面组件数据无效，原文件已保留".into()),
            }
        }
        s
    }
    pub fn persist(&self) -> Result<(), String> {
        if let Some(e) = &self.error {
            return Err(e.clone());
        }
        if self.path.as_os_str().is_empty() {
            return Ok(());
        }
        lyric_backend::io::save_settings(&self.path, &json!(self.items))
    }
    pub fn list(&self) -> Value {
        json!(self.items)
    }
    pub fn add(&mut self, value: &Value) -> Result<String, String> {
        if self.items.len() >= Self::LIMIT {
            return Err("最多同时放置 16 个桌面组件，请先移除不用的组件".into());
        }
        let kind = serde_json::from_value(value["kind"].clone()).map_err(|_| "组件类型无效")?;
        let size = match value.get("size") {
            None => Kind::default_size(kind),
            Some(v) => serde_json::from_value(v.clone()).map_err(|_| "组件尺寸无效")?,
        };
        let style = match value.get("style") {
            None => Style::default(),
            Some(v) => serde_json::from_value(v.clone()).map_err(|_| "组件风格无效")?,
        };
        let config = value
            .get("config")
            .map(|v| serde_json::from_value::<crate::widget_config::Config>(v.clone()))
            .transpose()
            .map_err(|_| "组件配置无效")?
            .unwrap_or_default();
        config.validate(kind)?;
        let x = coordinate(value, "x", 64 + self.items.len() as i32 * 24)?;
        let y = coordinate(value, "y", 80 + self.items.len() as i32 * 24)?;
        self.serial = self.serial.wrapping_add(1);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let id = format!("widget-{stamp}-{}", self.serial);
        self.items.push(Placement {
            id: id.clone(),
            kind,
            size,
            style,
            config,
            x,
            y,
            revision: 1,
        });
        if let Err(e) = self.persist() {
            self.items.pop();
            return Err(e);
        }
        Ok(id)
    }
    pub fn update(&mut self, value: &Value) -> Result<(), String> {
        let id = value["id"].as_str().ok_or("组件不存在")?;
        let index = self
            .items
            .iter()
            .position(|p| p.id == id)
            .ok_or("组件不存在")?;
        let old = self.items[index].clone();
        let x = coordinate(value, "x", old.x)?;
        let y = coordinate(value, "y", old.y)?;
        let size = match value.get("size") {
            None => old.size,
            Some(v) => serde_json::from_value(v.clone()).map_err(|_| "组件尺寸无效")?,
        };
        let style = match value.get("style") {
            None => old.style,
            Some(v) => serde_json::from_value(v.clone()).map_err(|_| "组件风格无效")?,
        };
        let config = value
            .get("config")
            .map(|v| serde_json::from_value::<crate::widget_config::Config>(v.clone()))
            .transpose()
            .map_err(|_| "组件配置无效")?
            .unwrap_or_else(|| old.config.clone());
        config.validate(old.kind)?;
        if old.x == x
            && old.y == y
            && old.size == size
            && old.style == style
            && old.config == config
        {
            return Ok(());
        }
        let p = &mut self.items[index];
        p.x = x;
        p.y = y;
        p.size = size;
        p.style = style;
        p.config = config;
        p.revision = p.revision.saturating_add(1);
        if let Err(e) = self.persist() {
            self.items[index] = old;
            return Err(e);
        }
        Ok(())
    }
    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        let index = self
            .items
            .iter()
            .position(|p| p.id == id)
            .ok_or("组件不存在")?;
        let old = self.items.remove(index);
        if let Err(e) = self.persist() {
            self.items.insert(index, old);
            return Err(e);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Metrics {
    pub cpu_percent: Option<f64>,
    pub memory_percent: Option<f64>,
    pub memory_used_gib: Option<f64>,
    pub memory_total_gib: Option<f64>,
    pub battery_percent: Option<u8>,
    pub external_power: Option<bool>,
}
pub fn cpu_usage(previous: (u64, u64, u64), current: (u64, u64, u64)) -> Option<f64> {
    let idle = current.0.checked_sub(previous.0)?;
    let total = current
        .1
        .checked_sub(previous.1)?
        .checked_add(current.2.checked_sub(previous.2)?)?;
    if total == 0 || idle > total {
        return None;
    }
    Some((total - idle) as f64 * 100. / total as f64)
}

#[cfg(test)]
mod content_tests {
    use super::*;
    #[test]
    fn separate_notes_and_invalid_date_updates_survive_restart() {
        let path = std::env::temp_dir().join(format!(
            "dotisle-widget-content-{}.json",
            std::process::id()
        ));
        let mut store = Store::new(path.clone());
        let a = store
            .add(&json!({"kind":"note","config":{"text":"第一张便签"}}))
            .unwrap();
        let b = store
            .add(&json!({"kind":"note","config":{"text":"第二张便签"}}))
            .unwrap();
        store
            .update(&json!({"id":a,"config":{"text":"单独修改第一张"}}))
            .unwrap();
        let anniversary = store
            .add(&json!({"kind":"anniversary","config":{"date":"2028-02-29"}}))
            .unwrap();
        assert!(store
            .update(&json!({"id":anniversary,"config":{"date":"2027-02-29"}}))
            .is_err());
        let restored = Store::new(path.clone());
        assert_eq!(
            restored
                .items
                .iter()
                .find(|p| p.id == b)
                .unwrap()
                .config
                .text,
            "第二张便签"
        );
        assert_eq!(
            restored
                .items
                .iter()
                .find(|p| p.id == anniversary)
                .unwrap()
                .config
                .date,
            "2028-02-29"
        );
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn old_placements_load_without_requiring_new_content() {
        let placement: Placement = serde_json::from_value(
            json!({"id":"old-clock","kind":"clock","size":"small","style":"original","x":0,"y":0}),
        )
        .unwrap();
        assert_eq!(placement.config, crate::widget_config::Config::default());
    }
}
#[derive(Clone, Copy, Debug)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
}
impl Date {
    pub fn days(self) -> u32 {
        match self.month {
            4 | 6 | 9 | 11 => 30,
            2 => {
                if self.year % 4 == 0 && (self.year % 100 != 0 || self.year % 400 == 0) {
                    29
                } else {
                    28
                }
            }
            _ => 31,
        }
    }
    pub fn first_weekday(self) -> usize {
        // Gregorian calendar, Monday first. 0001-01-01 was a Monday.
        let y = self.year as i64 - 1;
        let mut days = 365 * y + y / 4 - y / 100 + y / 400;
        for month in 1..self.month {
            days += Self { month, ..self }.days() as i64
        }
        days.rem_euclid(7) as usize
    }
    pub fn weekday(self) -> usize {
        (self.first_weekday() + self.day.saturating_sub(1) as usize) % 7
    }
    pub fn weekday_label(self) -> &'static str {
        [
            "星期一",
            "星期二",
            "星期三",
            "星期四",
            "星期五",
            "星期六",
            "星期日",
        ][self.weekday()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn temporary(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "DotIsle.Widgets.{name}.{}.{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }
    #[test]
    fn all_widget_kinds_and_sizes_roundtrip_at_negative_monitor_positions() {
        let path = temporary("roundtrip");
        let mut s = Store::new(path.clone());
        for kind in Kind::ALL {
            let id = s
                .add(&json!({"kind":kind,"size":Size::Medium,"x":-1600,"y":120}))
                .unwrap();
            s.update(&json!({"id":id,"size":"large","x":-1200}))
                .unwrap();
        }
        let restored = Store::new(path.clone());
        assert_eq!(s.items, restored.items);
        assert!(restored.error.is_none());
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn malformed_updates_never_mutate_a_placement() {
        let mut s = Store::new(PathBuf::new());
        let id = s.add(&json!({"kind":"clock"})).unwrap();
        let old = s.items.clone();
        for value in [
            json!({"id":id,"x":1.5}),
            json!({"id":id,"y":999999999}),
            json!({"id":id,"size":"enormous"}),
            json!({"id":id,"style":"unknown"}),
        ] {
            assert!(s.update(&value).is_err());
            assert_eq!(s.items, old);
        }
        assert!(s.add(&json!({"kind":"unknownWidget"})).is_err());
    }
    #[test]
    fn failed_writes_roll_back_add_move_resize_and_remove() {
        let parent = temporary("blocked");
        std::fs::write(&parent, b"blocked").unwrap();
        let mut s = Store::new(PathBuf::new());
        let id = s.add(&json!({"kind":"todos"})).unwrap();
        s.path = parent.join("widgets.json");
        let old = s.items.clone();
        assert!(s.add(&json!({"kind":"clock"})).is_err());
        assert!(s.update(&json!({"id":id,"x":500,"size":"large"})).is_err());
        assert!(s.remove(&id).is_err());
        assert_eq!(s.items, old);
        let _ = std::fs::remove_file(parent);
    }
    #[test]
    fn damaged_data_is_retained_and_cannot_be_overwritten() {
        let path = temporary("damaged");
        std::fs::write(&path, b"invalid").unwrap();
        let mut s = Store::new(path.clone());
        assert!(s.error.is_some());
        assert!(s.add(&json!({"kind":"clock"})).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"invalid");
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn capacity_and_duplicate_ids_are_checked() {
        let mut s = Store::new(PathBuf::new());
        for _ in 0..Store::LIMIT {
            s.add(&json!({"kind":"clock"})).unwrap();
        }
        assert!(s.add(&json!({"kind":"clock"})).is_err());
        let path = temporary("duplicate");
        std::fs::write(
            &path,
            serde_json::to_vec(&vec![s.items[0].clone(), s.items[0].clone()]).unwrap(),
        )
        .unwrap();
        assert!(Store::new(path.clone()).error.is_some());
        let _ = std::fs::remove_file(path);
    }
    #[test]
    fn calendar_handles_leap_years_and_sunday_boundaries() {
        let d = Date {
            year: 2026,
            month: 11,
            day: 10,
            hour: 0,
            minute: 0,
            second: 0,
        };
        assert_eq!(d.first_weekday(), 6);
        assert_eq!(d.weekday_label(), "星期二");
        assert_eq!(
            Date {
                year: 2000,
                month: 2,
                ..d
            }
            .days(),
            29
        );
        assert_eq!(
            Date {
                year: 2100,
                month: 2,
                ..d
            }
            .days(),
            28
        );
        assert_eq!(
            Date {
                year: 2024,
                month: 2,
                ..d
            }
            .days(),
            29
        );
    }
    #[test]
    fn cpu_uses_deltas_and_never_invents_samples() {
        assert_eq!(cpu_usage((100, 200, 100), (120, 250, 150)), Some(80.));
        assert_eq!(cpu_usage((100, 200, 100), (100, 200, 100)), None);
        assert_eq!(cpu_usage((100, 200, 100), (90, 250, 150)), None);
        assert_eq!(cpu_usage((0, 0, 0), (110, 50, 50)), None);
    }
}

pub fn host_key(data: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.to_string_lossy().to_lowercase().hash(&mut h);
    std::env::var("LYRICISLAND_TEST_PIPE")
        .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into())
        .hash(&mut h);
    format!("Local\\DotIsle.DesktopHost.{:016x}", h.finish())
}

#[cfg(test)]
mod style_tests {
    use super::*;
    #[test]
    fn independent_styles_roundtrip_and_invalid_style_is_atomic() {
        let path =
            std::env::temp_dir().join(format!("dotisle-widget-styles-{}.json", std::process::id()));
        let mut store = Store::new(path.clone());
        for style in Style::ALL {
            store.add(&json!({"kind":"music","style":style})).unwrap();
        }
        assert_eq!(Store::new(path.clone()).items, store.items);
        let before = store.items.clone();
        assert!(store
            .add(&json!({"kind":"clock","style":"unknown"}))
            .is_err());
        assert!(store
            .update(&json!({"id":before[0].id,"x":800,"style":"unknown"}))
            .is_err());
        assert_eq!(store.items, before);
        store
            .update(&json!({"id":before[0].id,"size":"large"}))
            .unwrap();
        assert_eq!(store.items[0].style, Style::Original);
        std::fs::remove_file(path).unwrap();
    }
    #[test]
    fn existing_placements_get_a_fixed_default() {
        let p: Placement =
            serde_json::from_value(json!({"id":"old","kind":"clock","x":30,"y":40})).unwrap();
        assert_eq!(p.style, Style::Original);
        assert_eq!(p.revision, 1);
    }
}
