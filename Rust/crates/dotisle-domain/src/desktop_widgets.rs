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
pub struct Store{session:dotisle_module_api::Session,pub items:Vec<Placement>,pub error:Option<String>}
impl Store{
pub fn new(path:PathBuf)->Self{let(session,result)=dotisle_module_api::Session::new("widgets","placements",json!({"path":path}));let mut s=Self{session,items:vec![],error:None};match result{Ok(v)=>s.sync(&v),Err(e)=>s.error=Some(e)}s}
fn sync(&mut self,v:&Value){if let Ok(items)=serde_json::from_value(v["items"].clone()){self.items=items;}self.error=v["error"].as_str().map(str::to_owned);}
fn invoke(&mut self,method:&str,mut a:Value)->Result<Value,String>{a["items"]=json!(self.items);let v=self.session.invoke(method,a)?;self.sync(&v);if let Some(e)=v["operationError"].as_str(){return Err(e.into())}Ok(v["result"].clone())}
pub fn add(&mut self,value:&Value)->Result<String,String>{serde_json::from_value(self.invoke("add",json!({"value":value}))?).map_err(|e|e.to_string())}
pub fn update(&mut self,value:&Value)->Result<(),String>{self.invoke("update",json!({"value":value})).map(|_|())}
pub fn remove(&mut self,id:&str)->Result<(),String>{self.invoke("remove",json!({"id":id})).map(|_|())}
pub fn list(&self)->Value{self.session.invoke("list",json!({"items":self.items})).ok().map(|v|v["result"].clone()).unwrap_or(Value::Null)}
pub fn persist(&self)->Result<(),String>{let v=self.session.invoke("persist",json!({"items":self.items}))?;if let Some(e)=v["operationError"].as_str(){return Err(e.into())}Ok(())}
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



pub fn host_key(data: &std::path::Path) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    data.to_string_lossy().to_lowercase().hash(&mut h);
    std::env::var("LYRICISLAND_TEST_PIPE")
        .unwrap_or_else(|_| "LyricIsland.Native.Settings.v1".into())
        .hash(&mut h);
    format!("Local\\DotIsle.DesktopHost.{:016x}", h.finish())
}
