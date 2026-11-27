use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

pub const UNIT: f32 = 176.0;
pub const GAP: f32 = 16.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidebarEdge {
    Left,
    #[default]
    Right,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HotkeyPreset {
    #[default]
    AltGrave,
    AltSpace,
    CtrlSpace,
    AltQ,
    CtrlShiftD,
}
impl HotkeyPreset {
    pub fn all() -> [Self; 5] {
        [
            Self::AltGrave,
            Self::AltSpace,
            Self::CtrlSpace,
            Self::AltQ,
            Self::CtrlShiftD,
        ]
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::AltGrave => "Alt + `",
            Self::AltSpace => "Alt + Space",
            Self::CtrlSpace => "Ctrl + Space",
            Self::AltQ => "Alt + Q",
            Self::CtrlShiftD => "Ctrl + Shift + D",
        }
    }
    /// 返回 `(Win32 RegisterHotKey MOD_* | MOD_NOREPEAT, VK_*)`。
    pub fn win32_mods_vk(self) -> (u32, u32) {
        match self {
            Self::AltGrave => (0x4001, 0xC0),   // MOD_ALT | MOD_NOREPEAT, VK_OEM_3
            Self::AltSpace => (0x4001, 0x20),   // MOD_ALT | MOD_NOREPEAT, VK_SPACE
            Self::CtrlSpace => (0x4002, 0x20),  // MOD_CONTROL | MOD_NOREPEAT, VK_SPACE
            Self::AltQ => (0x4001, 0x51),       // MOD_ALT | MOD_NOREPEAT, 'Q'
            Self::CtrlShiftD => (0x4006, 0x44), // MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT, 'D'
        }
    }
}

pub const CURRENT_TUTORIAL_VERSION: u32 = 1;

fn default_true() -> bool {
    true
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SidebarSettings {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub onboarding_done: bool,
    #[serde(default)]
    pub custom_modifiers: u32,
    #[serde(default)]
    pub custom_key: u32,
    #[serde(default)]
    pub edit_modifiers: u32,
    #[serde(default)]
    pub edit_key: u32,
    #[serde(default)]
    pub edge: SidebarEdge,
    #[serde(default)]
    pub hotkey: HotkeyPreset,
    #[serde(default = "default_true")]
    pub show_hotkey_hint: bool,
    #[serde(default)]
    pub tutorial_version: u32,
}
impl Default for SidebarSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            onboarding_done: false,
            custom_modifiers: 0, custom_key: 0,
            edit_modifiers: 0, edit_key: 0,
            edge: SidebarEdge::Right,
            hotkey: HotkeyPreset::AltGrave,
            show_hotkey_hint: true,
            tutorial_version: 0,
        }
    }
}
impl SidebarSettings {
    pub fn effective_hotkey(&self)->(u32,u32) {
        if self.custom_key>0 && self.custom_key<=0xfe && self.custom_modifiers>0 && self.custom_modifiers<=15 {
            (0x4000|self.custom_modifiers,self.custom_key)
        } else {self.hotkey.win32_mods_vk()}
    }
    pub fn effective_edit_hotkey(&self)->(u32,u32) {
        if self.edit_key>0 && self.edit_key<=0xfe && self.edit_modifiers>0 && self.edit_modifiers<=15 {
            (0x4000|self.edit_modifiers,self.edit_key)
        } else {(0x4001,0x45)}
    }
    pub fn hotkey_label(&self)->String {
        if self.custom_key==0 { return self.hotkey.label().into(); }
        let mut parts=Vec::new();
        for (bit,name) in [(2,"Ctrl"),(1,"Alt"),(4,"Shift"),(8,"Win")] {if self.custom_modifiers&bit!=0{parts.push(name.to_string());}}
        let vk=self.custom_key;
        parts.push(match vk {0x30..=0x39|0x41..=0x5a=>char::from_u32(vk).unwrap().to_string(),0x70..=0x7b=>format!("F{}",vk-0x6f),0x20=>"Space".into(),0xc0=>"`".into(),_=>format!("按键 {vk:X}")});
        parts.join(" + ")
    }

    pub fn load(data_dir: &Path) -> Self {
        let p = data_dir.join("allegro-settings.json");
        std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }
    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let p = data_dir.join("allegro-settings.json");
        let tmp = p.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    }
}

/// 拓展交互小组件的轻量化状态（习惯打卡、白噪音氛围、灵感色板、解压木鱼、呼吸引导、迷你计算器）。
#[derive(Clone, Debug)]
pub struct ExtraWidgetState {
    pub habits: [bool; 4],
    pub ambient_idx: usize,
    pub ambient_playing: bool,
    pub palette_idx: usize,
    pub merit_count: u64,
    pub breath_active: bool,
    pub calc_display: String,
    pub calc_acc: Option<f64>,
    pub calc_op: Option<char>,
    pub calc_fresh: bool,
}
impl Default for ExtraWidgetState {
    fn default() -> Self {
        Self {
            habits: [true, true, false, false],
            ambient_idx: 0,
            ambient_playing: false,
            palette_idx: 0,
            merit_count: 108,
            breath_active: true,
            calc_display: "0".into(),
            calc_acc: None,
            calc_op: None,
            calc_fresh: true,
        }
    }
}
impl ExtraWidgetState {
    pub fn calc_press(&mut self, key: &str) {
        let format_num = |v: f64| -> String {
            if !v.is_finite() {
                return "错误".into();
            }
            let r = (v * 1e6).round() / 1e6;
            if (r - r.round()).abs() < 1e-9 {
                format!("{:.0}", r)
            } else {
                let s = format!("{r:.6}");
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }
        };
        match key {
            "C" => {
                self.calc_display = "0".into();
                self.calc_acc = None;
                self.calc_op = None;
                self.calc_fresh = true;
            }
            "+" | "-" | "*" | "/" => {
                let cur: f64 = self.calc_display.parse().unwrap_or(0.);
                let next = match (self.calc_acc, self.calc_op) {
                    (Some(a), Some(op)) if !self.calc_fresh => match op {
                        '+' => a + cur,
                        '-' => a - cur,
                        '*' => a * cur,
                        '/' => if cur.abs() < 1e-12 { f64::NAN } else { a / cur },
                        _ => cur,
                    },
                    _ => cur,
                };
                self.calc_acc = Some(next);
                self.calc_display = format_num(next);
                self.calc_op = key.chars().next();
                self.calc_fresh = true;
            }
            "=" => {
                if let (Some(a), Some(op)) = (self.calc_acc, self.calc_op) {
                    let cur: f64 = self.calc_display.parse().unwrap_or(0.);
                    let res = match op {
                        '+' => a + cur,
                        '-' => a - cur,
                        '*' => a * cur,
                        '/' => if cur.abs() < 1e-12 { f64::NAN } else { a / cur },
                        _ => cur,
                    };
                    self.calc_display = format_num(res);
                    self.calc_acc = None;
                    self.calc_op = None;
                    self.calc_fresh = true;
                }
            }
            "." => {
                if self.calc_fresh {
                    self.calc_display = "0.".into();
                    self.calc_fresh = false;
                } else if !self.calc_display.contains('.') && self.calc_display.len() < 12 {
                    self.calc_display.push('.');
                }
            }
            d if d.len() == 1 && d.chars().all(|c| c.is_ascii_digit()) => {
                if self.calc_fresh || self.calc_display == "0" || self.calc_display == "错误" {
                    self.calc_display = d.to_string();
                    self.calc_fresh = false;
                } else if self.calc_display.len() < 12 {
                    self.calc_display.push_str(d);
                }
            }
            _ => {}
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub enum BlockSize {
    #[default]
    Small,
    Medium,
    Large,
}
impl BlockSize {
    pub fn cells(self) -> (usize, usize) {
        match self {
            Self::Small => (1, 1),
            Self::Medium => (2, 1),
            Self::Large => (2, 2),
        }
    }
    pub fn dimensions(self) -> (f32, f32) {
        let (w, h) = self.cells();
        (w as f32 * (UNIT + GAP) - GAP, h as f32 * (UNIT + GAP) - GAP)
    }
    pub fn next(self) -> Self {
        match self {
            Self::Small => Self::Medium,
            Self::Medium => Self::Large,
            Self::Large => Self::Small,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoItem {
    pub title: String,
    pub time: String,
    pub done: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TodoList {
    pub items: Vec<TodoItem>,
}
impl TodoList {
    /// 内置贴近日常使用的 Apple 提醒事项示例清单。
    pub fn demo() -> Self {
        Self {
            items: vec![
                TodoItem {
                    title: "确认侧边栏组件抽屉体验".into(),
                    time: "14:30".into(),
                    done: false,
                },
                TodoItem {
                    title: "核对中转站 API 额度配置".into(),
                    time: "16:00".into(),
                    done: false,
                },
                TodoItem {
                    title: "整理项目交付压缩包".into(),
                    time: "17:30".into(),
                    done: false,
                },
                TodoItem {
                    title: "审阅界面视觉验收清单".into(),
                    time: "11:00".into(),
                    done: true,
                },
                TodoItem {
                    title: "晚间慢跑 30 分钟".into(),
                    time: "20:00".into(),
                    done: false,
                },
            ],
        }
    }

    /// 从数据目录读取持久化待办列表，缺失或为空时回退示例数据。
    pub fn load(data_dir: &Path) -> Self {
        let p = data_dir.join("allegro-todos.json");
        let loaded: Option<Self> = std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        match loaded {
            Some(list) if !list.items.is_empty() => list,
            _ => Self::demo(),
        }
    }

    /// 原子持久化待办列表到本地配置目录。
    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let p = data_dir.join("allegro-todos.json");
        let tmp = p.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    }

    /// 原地切换指定待办项的完成状态。
    pub fn toggle(&mut self, idx: usize) {
        if let Some(item) = self.items.get_mut(idx) {
            item.done = !item.done;
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Card {
    pub id: u64,
    pub title: String,
    pub kind: String,
    pub size: BlockSize,
    pub acrylic: u8,
    pub stack: Vec<String>,
    pub page: usize,
    pub x: f32,
    pub y: f32,
    #[serde(default)]
    pub note_text: String,
}
impl Card {
    pub fn active(&self) -> &str {
        self.stack
            .get(self.page)
            .map(String::as_str)
            .unwrap_or(&self.kind)
    }
    pub fn turn(&mut self, delta: i32) {
        if !self.stack.is_empty() {
            self.page = (self.page as i32 + delta).rem_euclid(self.stack.len() as i32) as usize;
        }
    }
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Board {
    pub cards: Vec<Card>,
}
impl Board {
    // 不信任外部配置中的坐标、页码及重复编号，加载后统一规范化。
    pub fn sanitize(&mut self) {
        self.cards.truncate(256);
        let mut used = std::collections::HashSet::new();
        let mut next = 1u64;
        for c in self.cards.iter_mut() {
            if c.id == 0 || !used.insert(c.id) { while used.contains(&next) { next += 1; } c.id=next; used.insert(next); next += 1; }
            c.acrylic = c.acrylic.min(3);
            c.stack.truncate(32);
            c.page = if c.stack.is_empty() {
                0
            } else {
                c.page % c.stack.len()
            };
            if !c.x.is_finite() {
                c.x = 32.
            }
            if !c.y.is_finite() {
                c.y = 32.
            }
            c.x = c.x.clamp(0., 16000.);
            c.y = c.y.clamp(0., 16000.);
        }
    }
    pub fn demo() -> Self {
        Self {
            cards: vec![
                Card {
                    id: 1,
                    title: "电池与性能".into(),
                    kind: "system".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 32.,
                    y: 32.,
                    note_text: String::new(),
                },
                Card {
                    id: 2,
                    title: "计时器".into(),
                    kind: "focus".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 416.,
                    y: 32.,
                    note_text: String::new(),
                },
                Card {
                    id: 3,
                    title: "天气".into(),
                    kind: "weather".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 608.,
                    y: 32.,
                    note_text: String::new(),
                },
                Card {
                    id: 4,
                    title: "日历与日程".into(),
                    kind: "calendar".into(),
                    size: BlockSize::Large,
                    acrylic: 1,
                    stack: vec!["calendar".into(), "music".into()],
                    page: 0,
                    x: 32.,
                    y: 224.,
                    note_text: String::new(),
                },
                Card {
                    id: 5,
                    title: "待办事项".into(),
                    kind: "todo".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 416.,
                    y: 224.,
                    note_text: String::new(),
                },
                Card {
                    id: 6,
                    title: "便签".into(),
                    kind: "note".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 416.,
                    y: 416.,
                    note_text: "双击直接写入便签\n支持中文输入与换行".into(),
                },
            ],
        }
    }
    /// Deskset × Smart Stack 独立侧边栏旗舰示范排列（严守 1×1 / 2×1 / 2×2 Apple 规范，与桌面互不同步）。
    pub fn sidebar_demo() -> Self {
        Self {
            cards: vec![
                Card {
                    id: 1,
                    title: "时钟".into(),
                    kind: "analog_clock".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 2,
                    title: "天气".into(),
                    kind: "weather".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 3,
                    title: "喝水提醒".into(),
                    kind: "hydration".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 4,
                    title: "久坐提醒".into(),
                    kind: "sedentary".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 5,
                    title: "待办事项".into(),
                    kind: "todo".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 6,
                    title: "AI 额度".into(),
                    kind: "ai_quota".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 7,
                    title: "文件中转站".into(),
                    kind: "file_shelf".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 8,
                    title: "电池与性能".into(),
                    kind: "system".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 9,
                    title: "音乐".into(),
                    kind: "music".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 10,
                    title: "计时器".into(),
                    kind: "focus".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 11,
                    title: "快捷指令".into(),
                    kind: "apps".into(),
                    size: BlockSize::Small,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 12,
                    title: "日历".into(),
                    kind: "calendar".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec![],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
                Card {
                    id: 13,
                    title: "日常秩序".into(),
                    kind: "activity".into(),
                    size: BlockSize::Medium,
                    acrylic: 1,
                    stack: vec!["activity".into(), "clock".into(), "clipboard".into()],
                    page: 0,
                    x: 0.,
                    y: 0.,
                    note_text: String::new(),
                },
            ],
        }
    }
    pub fn merge(&mut self, a: u64, b: u64) -> bool {
        let Some(i) = self.cards.iter().position(|c| c.id == a) else {
            return false;
        };
        let Some(j) = self.cards.iter().position(|c| c.id == b) else {
            return false;
        };
        if i == j || self.cards[i].size != self.cards[j].size {
            return false;
        }
        let mut donor = self.cards.remove(j);
        let target = self.cards.iter_mut().find(|c| c.id == a).unwrap();
        if target.stack.is_empty() {
            target.stack.push(target.kind.clone())
        }
        if donor.stack.is_empty() {
            donor.stack.push(donor.kind)
        }
        target.stack.extend(donor.stack);
        true
    }
    pub fn split(&mut self, id: u64) -> bool {
        let Some(i) = self.cards.iter().position(|c| c.id == id) else {
            return false;
        };
        if self.cards[i].stack.len() < 2 {
            return false;
        }
        let kinds = std::mem::take(&mut self.cards[i].stack);
        self.cards[i].kind = kinds[0].clone();
        self.cards[i].page = 0;
        let mut next = self.cards.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        for (offset, kind) in kinds.into_iter().skip(1).enumerate() {
            let mut c = self.cards[i].clone();
            c.id = next;
            next += 1;
            c.kind = kind;
            c.note_text.clear();
            c.x += 192. * (offset + 1) as f32;
            self.cards.push(c)
        }
        true
    }
    /// 根据组件类型构造默认卡片模板（`id` 由插入方法统一分配，涵盖 22 类 Apple 原生组件）。
    pub fn make_card(&self, kind: &str) -> Card {
        let id = self.cards.iter().map(|c| c.id).max().unwrap_or(0) + 1;
        let (title, size) = match kind {
            "analog_clock" => ("时钟", BlockSize::Small),
            "weather" => ("天气", BlockSize::Small),
            "todo" => ("待办事项", BlockSize::Medium),
            "ai_quota" => ("AI 额度", BlockSize::Medium),
            "file_shelf" => ("文件中转站", BlockSize::Medium),
            "system" => ("电池与性能", BlockSize::Medium),
            "music" => ("音乐", BlockSize::Medium),
            "calendar" => ("日历", BlockSize::Medium),
            "focus" => ("计时器", BlockSize::Small),
            "note" => ("便签", BlockSize::Small),
            "anniversary" => ("纪念日倒数", BlockSize::Small),
            "today" => ("今日概览", BlockSize::Medium),
            "wireless" => ("无线设备电量", BlockSize::Medium),
            "storage" => ("存储空间", BlockSize::Medium),
            "clock" => ("世界时钟", BlockSize::Medium),
            "apps" => ("快捷指令", BlockSize::Small),
            "focus_stats" => ("专注统计", BlockSize::Medium),
            "schedule" => ("日程倒计时", BlockSize::Small),
            "screen_time" => ("用机时长", BlockSize::Medium),
            "activity" => ("健身圆环", BlockSize::Medium),
            "quote" => ("杂志日签", BlockSize::Small),
            "clipboard" => ("备忘录 · 剪贴板", BlockSize::Small),
            "hydration" => ("喝水提醒", BlockSize::Medium),
            "sedentary" => ("久坐提醒", BlockSize::Medium),
            "habit" => ("习惯打卡", BlockSize::Medium),
            "countdown_ring" => ("下班倒计时", BlockSize::Small),
            "pomodoro_wave" => ("白噪音氛围", BlockSize::Medium),
            "color_picker" => ("灵感色板", BlockSize::Small),
            "net_speed" => ("实时网速", BlockSize::Medium),
            "converter" => ("汇率快查", BlockSize::Medium),
            "github_grid" => ("贡献热力图", BlockSize::Medium),
            "wooden_fish" => ("解压木鱼", BlockSize::Small),
            "breathing" => ("呼吸引导", BlockSize::Small),
            "quick_calc" => ("迷你计算器", BlockSize::Small),
            _ => (kind, BlockSize::Small),
        };
        Card {
            id,
            title: title.into(),
            kind: kind.into(),
            size,
            acrylic: 1,
            stack: vec![],
            page: 0,
            x: 32.,
            y: 32.,
            note_text: String::new(),
        }
    }
    /// 在指定顺序下标插入新组件并返回其分配到的 `id`。
    pub fn insert_at(&mut self, index: usize, kind: &str) -> u64 {
        let card = self.make_card(kind);
        let id = card.id;
        let pos = index.min(self.cards.len());
        self.cards.insert(pos, card);
        id
    }
    pub fn add(&mut self, kind: &str) {
        self.insert_at(self.cards.len(), kind);
    }
}

/// 根据指针在侧边栏内容区相对坐标 `(rel_x, rel_y)`，模拟各插入位置排版并选出最贴近的落点下标。
pub fn sidebar_insert_index(cards: &[Card], kind: &str, rel_x: f32, rel_y: f32) -> usize {
    let temp_board = Board {
        cards: cards.to_vec(),
    };
    let mut ghost = temp_board.make_card(kind);
    let ghost_id = u64::MAX;
    ghost.id = ghost_id;

    let mut best_idx = cards.len();
    let mut best_score = f32::INFINITY;
    for idx in 0..=cards.len() {
        let mut trial = Vec::with_capacity(cards.len() + 1);
        trial.extend_from_slice(&cards[..idx]);
        trial.push(ghost.clone());
        trial.extend_from_slice(&cards[idx..]);
        let placements = pack_sidebar_blocks(&trial);
        if let Some(p) = placements.iter().find(|p| p.id == ghost_id) {
            let cx = p.x + p.w * 0.5;
            let cy = p.y + p.h * 0.5;
            let inside = rel_x >= p.x && rel_x <= p.x + p.w && rel_y >= p.y && rel_y <= p.y + p.h;
            let dx = (rel_x - cx) * 0.72;
            let dy = rel_y - cy;
            let mut score = dx * dx + dy * dy;
            if inside {
                score *= 0.25;
            }
            if score < best_score {
                best_score = score;
                best_idx = idx;
            }
        }
    }
    best_idx
}
#[derive(Clone, Copy, Debug)]
pub struct Placement {
    pub id: u64,
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}
// 两列网格逐格寻找首个空位，大卡片占用四个单元，后续小卡片可回填。
pub fn pack_sidebar_blocks(cards: &[Card]) -> Vec<Placement> {
    let mut grid: Vec<[bool; 2]> = vec![];
    let mut out = vec![];
    for c in cards {
        let (w, h) = c.size.cells();
        let mut row = 0;
        'search: loop {
            while grid.len() < row + h {
                grid.push([false; 2])
            }
            for col in 0..=2 - w {
                if (row..row + h).all(|r| (col..col + w).all(|x| !grid[r][x])) {
                    for line in grid.iter_mut().skip(row).take(h) {
                        for cell in line.iter_mut().skip(col).take(w) {
                            *cell = true
                        }
                    }
                    let (pw, ph) = c.size.dimensions();
                    out.push(Placement {
                        id: c.id,
                        x: col as f32 * 192.,
                        y: row as f32 * 192.,
                        w: pw,
                        h: ph,
                    });
                    break 'search;
                }
            }
            row += 1;
        }
    }
    out
}
#[derive(Clone, Debug, Default)]
pub struct Cli {
    pub integrated: bool,
    pub demo: bool,
    pub keep_alive: bool,
    pub data_dir: PathBuf,
    pub pipe: Option<String>,
    pub preview: Option<PathBuf>,
}
impl Cli {
    pub fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut c = Self {
            data_dir: PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| ".".into()))
                .join("DotIsle/AllegroSkia"),
            ..Default::default()
        };
        let mut it = args.into_iter();
        while let Some(a) = it.next() {
            match a.as_str() {
                "--demo" => c.demo = true,
                "--integrated" => c.integrated = true,
                "--keep-alive" => c.keep_alive = true,
                "--data-dir" => c.data_dir = it.next().ok_or("--data-dir 缺少目录")?.into(),
                "--pipe" => c.pipe = Some(it.next().ok_or("--pipe 缺少名称")?),
                "--render-preview" => {
                    c.preview = Some(it.next().ok_or("--render-preview 缺少目录")?.into())
                }
                _ => return Err(format!("未知参数：{a}")),
            }
        }
        Ok(c)
    }
}
#[derive(Clone, Debug)]
pub struct Application {
    pub name: String,
    pub path: PathBuf,
}
#[derive(Clone, Debug)]
pub enum SearchResult {
    Math(String),
    Command(String),
    Application(Application),
}
pub fn search(query: &str, apps: &[Application]) -> Vec<SearchResult> {
    let q = query.trim();
    if q.is_empty() {
        return vec![];
    }
    if let Ok(n) = evaluate(q) {
        return vec![SearchResult::Math(n.to_string())];
    }
    if q.starts_with('/') {
        return ["/light", "/dark", "/edit", "/add", "/quit"]
            .into_iter()
            .filter(|s| s.starts_with(q))
            .map(|s| SearchResult::Command(s.into()))
            .collect();
    }
    let needle = q.to_lowercase();
    apps.iter()
        .filter(|a| a.name.to_lowercase().contains(&needle))
        .take(8)
        .cloned()
        .map(SearchResult::Application)
        .collect()
}
// 递归下降求值器限制表达式长度和括号深度，拒绝无穷值及残留字符。
pub fn evaluate(input: &str) -> Result<f64, String> {
    if input.len() > 1024 {
        return Err("表达式过长".into());
    }
    struct Parser {
        s: Vec<char>,
        p: usize,
    }
    impl Parser {
        fn space(&mut self) {
            while self.s.get(self.p).is_some_and(|c| c.is_whitespace()) {
                self.p += 1
            }
        }
        fn expr(&mut self, d: usize) -> Result<f64, String> {
            let mut v = self.term(d)?;
            loop {
                self.space();
                match self.s.get(self.p) {
                    Some('+') => {
                        self.p += 1;
                        v += self.term(d)?
                    }
                    Some('-') => {
                        self.p += 1;
                        v -= self.term(d)?
                    }
                    _ => break,
                }
            }
            Ok(v)
        }
        fn term(&mut self, d: usize) -> Result<f64, String> {
            let mut v = self.atom(d)?;
            loop {
                self.space();
                match self.s.get(self.p) {
                    Some('*') => {
                        self.p += 1;
                        v *= self.atom(d)?
                    }
                    Some('/') => {
                        self.p += 1;
                        v /= self.atom(d)?
                    }
                    _ => break,
                }
            }
            Ok(v)
        }
        fn atom(&mut self, d: usize) -> Result<f64, String> {
            if d > 32 {
                return Err("括号过深".into());
            }
            self.space();
            match self.s.get(self.p) {
                Some('+') => {
                    self.p += 1;
                    self.atom(d + 1)
                }
                Some('-') => {
                    self.p += 1;
                    Ok(-self.atom(d + 1)?)
                }
                Some('(') => {
                    self.p += 1;
                    let v = self.expr(d + 1)?;
                    self.space();
                    if self.s.get(self.p) != Some(&')') {
                        return Err("缺少右括号".into());
                    }
                    self.p += 1;
                    Ok(v)
                }
                _ => {
                    let start = self.p;
                    while self
                        .s
                        .get(self.p)
                        .is_some_and(|c| c.is_ascii_digit() || *c == '.')
                    {
                        self.p += 1
                    }
                    self.s[start..self.p]
                        .iter()
                        .collect::<String>()
                        .parse()
                        .map_err(|_| "无效数字".into())
                }
            }
        }
    }
    let mut p = Parser {
        s: input.chars().collect(),
        p: 0,
    };
    let v = p.expr(0)?;
    p.space();
    if p.p != p.s.len() || !v.is_finite() {
        Err("无效表达式".into())
    } else {
        Ok(v)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn shared_board_preserves_ids_and_repairs_duplicates() {
        let mut b=Board::demo();
        b.cards[0].id=u64::MAX; b.cards[1].id=u64::MAX; b.cards[2].id=0;
        b.sanitize();
        assert_eq!(b.cards[0].id,u64::MAX);
        let ids:std::collections::HashSet<_>=b.cards.iter().map(|c|c.id).collect();
        assert_eq!(ids.len(),b.cards.len()); assert!(!ids.contains(&0));
        let before:Vec<_>=b.cards.iter().map(|c|c.id).collect();
        b.sanitize(); assert_eq!(before,b.cards.iter().map(|c|c.id).collect::<Vec<_>>());
        let mut loaded:Board=serde_json::from_slice(&serde_json::to_vec(&b).unwrap()).unwrap();
        loaded.sanitize(); assert_eq!(before,loaded.cards.iter().map(|c|c.id).collect::<Vec<_>>());
    }
    #[test]
    fn apple_packing() {
        let mut b = Board::demo();
        b.cards[0].size = BlockSize::Small;
        b.cards[1].size = BlockSize::Large;
        b.cards[2].size = BlockSize::Small;
        let p = pack_sidebar_blocks(&b.cards);
        assert_eq!(BlockSize::Medium.dimensions(), (368., 176.));
        assert_eq!(BlockSize::Large.dimensions(), (368., 368.));
        assert_eq!(p[2].y, 0.);
        for (i, a) in p.iter().enumerate() {
            for b in &p[i + 1..] {
                assert!(
                    a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y
                )
            }
        }
    }
    #[test]
    fn all_size_sequences_fill_without_overlap() {
        for seed in 0..81 {
            let mut b = Board::demo();
            let mut n = seed;
            for c in &mut b.cards {
                c.size = match n % 3 {
                    0 => BlockSize::Small,
                    1 => BlockSize::Medium,
                    _ => BlockSize::Large,
                };
                n /= 3;
            }
            let places = pack_sidebar_blocks(&b.cards);
            assert_eq!(places.len(), b.cards.len());
            for (i, a) in places.iter().enumerate() {
                assert!(a.x >= 0. && a.x + a.w <= 368.);
                for z in &places[i + 1..] {
                    assert!(
                        a.x + a.w <= z.x
                            || z.x + z.w <= a.x
                            || a.y + a.h <= z.y
                            || z.y + z.h <= a.y
                    )
                }
            }
        }
    }
    #[test]
    fn stack_roundtrip() {
        let mut b = Board::demo();
        assert!(b.merge(2, 3));
        let c = b.cards.iter_mut().find(|c| c.id == 2).unwrap();
        c.turn(-1);
        assert_eq!(c.page, 1);
        c.turn(1);
        assert_eq!(c.page, 0);
        assert!(b.split(2));
        assert_eq!(b.cards.len(), 6);
        assert!(!b.merge(1, 2));
    }
    #[test]
    fn math_and_cli() {
        assert_eq!(evaluate("128*36").unwrap(), 4608.);
        assert_eq!(evaluate("-(2+3)*4/2").unwrap(), -10.);
        assert!(evaluate("1/0").is_err());
        assert!(evaluate("2abc").is_err());
        assert!(Cli::parse(vec!["--pipe".into()]).is_err());
        assert!(matches!(&search("128*36",&[])[0],SearchResult::Math(s) if s=="4608"));
    }
    #[test]
    fn insert_at_and_drag_target() {
        let mut b = Board::sidebar_demo();
        let initial_len = b.cards.len();
        let idx_top = sidebar_insert_index(&b.cards, "music", 180., 220.);
        assert!(idx_top <= initial_len);
        let id_first = b.insert_at(0, "clipboard");
        assert_eq!(b.cards.len(), initial_len + 1);
        assert_eq!(b.cards[0].id, id_first);
        let id_end = b.insert_at(999, "weather");
        assert_eq!(b.cards.last().unwrap().id, id_end);
        assert_ne!(id_first, id_end);
    }
    #[test]
    fn settings_roundtrip() {
        let dir = std::env::temp_dir().join(format!("allegro-settings-{}", std::process::id()));
        let s = SidebarSettings {
            enabled: false,
            edge: SidebarEdge::Left,
            hotkey: HotkeyPreset::CtrlShiftD,
            show_hotkey_hint: false,
            tutorial_version: CURRENT_TUTORIAL_VERSION,
            onboarding_done: true,
            custom_modifiers: 6, custom_key: 0x58,
            edit_modifiers: 7, edit_key: 0x59,
        };
        s.save(&dir).unwrap();
        let loaded = SidebarSettings::load(&dir);
        assert_eq!(loaded, s);
        assert_eq!(loaded.effective_edit_hotkey(),(0x4007,0x59));
        let compat: SidebarSettings = serde_json::from_str(r#"{"edge":"left"}"#).unwrap();
        assert!(compat.enabled && compat.show_hotkey_hint && compat.tutorial_version == 0);
        assert_eq!(compat.effective_edit_hotkey(),(0x4001,0x45));
        for p in HotkeyPreset::all() {
            let (mods, vk) = p.win32_mods_vk();
            assert!(mods != 0 && vk != 0 && !p.label().is_empty());
        }
        std::fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn mini_calc_works() {
        let mut ex = ExtraWidgetState::default();
        for k in ["1", "2", "+", "3", "0", "="] {
            ex.calc_press(k);
        }
        assert_eq!(ex.calc_display, "42");
        ex.calc_press("*");
        ex.calc_press("2");
        ex.calc_press("=");
        assert_eq!(ex.calc_display, "84");
        ex.calc_press("C");
        assert_eq!(ex.calc_display, "0");
    }
    #[test]
    fn todo_toggle_and_roundtrip() {
        let dir = std::env::temp_dir().join(format!("allegro-todos-{}", std::process::id()));
        let mut list = TodoList::demo();
        let before = list.items[0].done;
        list.toggle(0);
        assert_ne!(list.items[0].done, before);
        list.save(&dir).unwrap();
        let loaded = TodoList::load(&dir);
        assert_eq!(loaded, list);

        // 旧卡片 JSON 缺少 note_text 时仍可反序列化
        let raw_old = r#"{"id":1,"title":"便签","kind":"note","size":"Small","acrylic":1,"stack":[],"page":0,"x":0.0,"y":0.0}"#;
        let card: Card = serde_json::from_str(raw_old).unwrap();
        assert_eq!(card.note_text, "");

        std::fs::remove_dir_all(dir).unwrap();
    }
}
