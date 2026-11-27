use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActiveAlert {
    Water,
    Stand,
}

const INTERVAL_PRESETS: [u32; 5] = [15, 30, 45, 60, 90];

fn default_true() -> bool {
    true
}
fn default_water_interval() -> u32 {
    45
}
fn default_cup_ml() -> u32 {
    250
}
fn default_goal_ml() -> u32 {
    2000
}
fn default_stand_interval() -> u32 {
    45
}

/// 从旧版插件提取并升级的「喝水提醒 + 久坐提醒」独立引擎状态与持久化模型。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ReminderState {
    #[serde(default = "default_true")]
    pub water_enabled: bool,
    #[serde(default = "default_water_interval")]
    pub water_interval_min: u32,
    #[serde(default = "default_cup_ml")]
    pub cup_ml: u32,
    #[serde(default = "default_goal_ml")]
    pub goal_ml: u32,
    #[serde(default)]
    pub drank_ml: u32,
    #[serde(default)]
    pub cups_today: u32,
    #[serde(default = "default_true")]
    pub stand_enabled: bool,
    #[serde(default = "default_stand_interval")]
    pub stand_interval_min: u32,
    #[serde(default)]
    pub stand_count: u32,
    #[serde(default)]
    pub day_key: String,
    #[serde(skip)]
    pub water_elapsed_sec: f32,
    #[serde(skip)]
    pub stand_elapsed_sec: f32,
    #[serde(skip)]
    pub active_alert: Option<ActiveAlert>,
    #[serde(skip)]
    pub island_progress: f32,
}

impl Default for ReminderState {
    fn default() -> Self {
        Self {
            water_enabled: true,
            water_interval_min: 45,
            cup_ml: 250,
            goal_ml: 2000,
            drank_ml: 0,
            cups_today: 0,
            stand_enabled: true,
            stand_interval_min: 45,
            stand_count: 0,
            day_key: today_key(),
            water_elapsed_sec: 0.,
            stand_elapsed_sec: 0.,
            active_alert: None,
            island_progress: 0.,
        }
    }
}

/// 返回本机当地日期标识 `YYYY-MM-DD`。
pub fn today_key() -> String {
    let st = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}", st.wYear, st.wMonth, st.wDay)
}

impl ReminderState {
    /// 演示态默认数据（已喝 3 杯共 750ml，已活动 4 次）。
    pub fn demo() -> Self {
        Self {
            water_enabled: true,
            water_interval_min: 45,
            cup_ml: 250,
            goal_ml: 2000,
            drank_ml: 750,
            cups_today: 3,
            stand_enabled: true,
            stand_interval_min: 45,
            stand_count: 4,
            day_key: today_key(),
            water_elapsed_sec: 16. * 60.,
            stand_elapsed_sec: 22. * 60.,
            active_alert: None,
            island_progress: 0.,
        }
    }

    pub fn load(data_dir: &Path) -> Self {
        let p = data_dir.join("allegro-reminders.json");
        let mut s: Self = std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default();
        s.cup_ml = s.cup_ml.clamp(50, 1000);
        s.goal_ml = s.goal_ml.clamp(500, 5000);
        if !INTERVAL_PRESETS.contains(&s.water_interval_min) {
            s.water_interval_min = 45;
        }
        if !INTERVAL_PRESETS.contains(&s.stand_interval_min) {
            s.stand_interval_min = 45;
        }
        s.check_day_rollover(&today_key());
        s
    }

    pub fn save(&self, data_dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
        let p = data_dir.join("allegro-reminders.json");
        let tmp = p.with_extension("json.tmp");
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
        std::fs::rename(&tmp, &p).map_err(|e| e.to_string())
    }

    /// 跨天时自动清零今日饮水与起身计数，并重置当日计时。
    pub fn check_day_rollover(&mut self, today: &str) -> bool {
        if self.day_key == today {
            return false;
        }
        self.day_key = today.to_string();
        self.drank_ml = 0;
        self.cups_today = 0;
        self.stand_count = 0;
        self.water_elapsed_sec = 0.;
        self.stand_elapsed_sec = 0.;
        true
    }

    /// 推进定时周期；无弹窗时累计秒数，到点则触发对应灵动岛提醒并返回 `true`。
    pub fn tick(&mut self, dt_sec: f32) -> bool {
        if dt_sec <= 0. || self.active_alert.is_some() {
            return false;
        }
        let mut fired = false;
        if self.water_enabled {
            self.water_elapsed_sec += dt_sec;
            if self.water_elapsed_sec >= (self.water_interval_min * 60) as f32 {
                self.water_elapsed_sec = 0.;
                self.active_alert = Some(ActiveAlert::Water);
                fired = true;
            }
        }
        if !fired && self.stand_enabled {
            self.stand_elapsed_sec += dt_sec;
            if self.stand_elapsed_sec >= (self.stand_interval_min * 60) as f32 {
                self.stand_elapsed_sec = 0.;
                self.active_alert = Some(ActiveAlert::Stand);
                fired = true;
            }
        }
        fired
    }

    /// 点击饮水或提醒灵动岛打卡：计入本杯毫升数、重算下轮倒计时并关闭喝水灵动岛。
    pub fn acknowledge_water(&mut self) {
        self.drank_ml = self.drank_ml.saturating_add(self.cup_ml);
        self.cups_today = self.cups_today.saturating_add(1);
        self.water_elapsed_sec = 0.;
        if self.active_alert == Some(ActiveAlert::Water) {
            self.active_alert = None;
        }
    }

    /// 点击起身打卡或久坐灵动岛：计入活动次数、重算下轮倒计时并关闭久坐灵动岛。
    pub fn acknowledge_stand(&mut self) {
        self.stand_count = self.stand_count.saturating_add(1);
        self.stand_elapsed_sec = 0.;
        if self.active_alert == Some(ActiveAlert::Stand) {
            self.active_alert = None;
        }
    }

    /// 供组件内点击「试弹灵动岛」随时预览或触发对应提醒。
    pub fn trigger_preview(&mut self, alert: ActiveAlert) {
        self.active_alert = Some(alert);
    }

    pub fn adjust_cup(&mut self, delta: i32) {
        let next = self.cup_ml as i32 + delta;
        self.cup_ml = next.clamp(50, 1000) as u32;
    }

    pub fn adjust_goal(&mut self, delta: i32) {
        let next = self.goal_ml as i32 + delta;
        self.goal_ml = next.clamp(500, 5000) as u32;
    }

    pub fn cycle_water_interval(&mut self) {
        let idx = INTERVAL_PRESETS
            .iter()
            .position(|&m| m == self.water_interval_min)
            .unwrap_or(2);
        self.water_interval_min = INTERVAL_PRESETS[(idx + 1) % INTERVAL_PRESETS.len()];
        self.water_elapsed_sec = 0.;
    }

    pub fn cycle_stand_interval(&mut self) {
        let idx = INTERVAL_PRESETS
            .iter()
            .position(|&m| m == self.stand_interval_min)
            .unwrap_or(2);
        self.stand_interval_min = INTERVAL_PRESETS[(idx + 1) % INTERVAL_PRESETS.len()];
        self.stand_elapsed_sec = 0.;
    }

    pub fn toggle_water(&mut self) {
        self.water_enabled = !self.water_enabled;
        if !self.water_enabled && self.active_alert == Some(ActiveAlert::Water) {
            self.active_alert = None;
        }
    }

    pub fn toggle_stand(&mut self) {
        self.stand_enabled = !self.stand_enabled;
        if !self.stand_enabled && self.active_alert == Some(ActiveAlert::Stand) {
            self.active_alert = None;
        }
    }

    pub fn water_remaining_sec(&self) -> u32 {
        let total = (self.water_interval_min * 60) as f32;
        (total - self.water_elapsed_sec).max(0.).ceil() as u32
    }

    pub fn stand_remaining_sec(&self) -> u32 {
        let total = (self.stand_interval_min * 60) as f32;
        (total - self.stand_elapsed_sec).max(0.).ceil() as u32
    }

    pub fn water_progress(&self) -> f32 {
        if self.goal_ml == 0 {
            0.
        } else {
            (self.drank_ml as f32 / self.goal_ml as f32).clamp(0., 1.)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn water_cycle_trigger_and_dismiss() {
        let mut r = ReminderState {
            water_interval_min: 15,
            stand_enabled: false,
            cup_ml: 300,
            goal_ml: 1800,
            ..Default::default()
        };
        assert!(!r.tick(10. * 60.));
        assert_eq!(r.active_alert, None);
        assert!(r.tick(5. * 60. + 0.5));
        assert_eq!(r.active_alert, Some(ActiveAlert::Water));

        // 弹窗期间不会重复触发；点击灵动岛后计入 300ml 并立即消失
        assert!(!r.tick(60.));
        r.acknowledge_water();
        assert_eq!(r.active_alert, None);
        assert_eq!(r.drank_ml, 300);
        assert_eq!(r.cups_today, 1);
        assert_eq!(r.water_elapsed_sec, 0.);

        // 下个周期到点再弹出
        assert!(r.tick(15. * 60.));
        assert_eq!(r.active_alert, Some(ActiveAlert::Water));
    }

    #[test]
    fn stand_cycle_and_reset() {
        let mut r = ReminderState {
            water_enabled: false,
            stand_interval_min: 30,
            ..Default::default()
        };
        assert!(r.tick(30. * 60.));
        assert_eq!(r.active_alert, Some(ActiveAlert::Stand));
        r.acknowledge_stand();
        assert_eq!(r.active_alert, None);
        assert_eq!(r.stand_count, 1);
        assert_eq!(r.stand_elapsed_sec, 0.);
    }

    #[test]
    fn cup_and_goal_bounds() {
        let mut r = ReminderState::default();
        r.adjust_cup(-1000);
        assert_eq!(r.cup_ml, 50);
        r.adjust_cup(5000);
        assert_eq!(r.cup_ml, 1000);
        r.adjust_goal(-9999);
        assert_eq!(r.goal_ml, 500);
        r.adjust_goal(99999);
        assert_eq!(r.goal_ml, 5000);
    }

    #[test]
    fn reminders_persist_roundtrip() {
        let dir = std::env::temp_dir().join(format!("allegro-rem-{}", std::process::id()));
        let mut r = ReminderState::default();
        r.cup_ml = 350;
        r.goal_ml = 2500;
        r.acknowledge_water();
        r.save(&dir).unwrap();
        let loaded = ReminderState::load(&dir);
        assert_eq!(loaded.cup_ml, 350);
        assert_eq!(loaded.goal_ml, 2500);
        assert_eq!(loaded.drank_ml, 350);
        assert_eq!(loaded.cups_today, 1);

        let mut rolled = loaded;
        assert!(rolled.check_day_rollover("2099-01-01"));
        assert_eq!(rolled.drank_ml, 0);
        assert_eq!(rolled.cups_today, 0);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
