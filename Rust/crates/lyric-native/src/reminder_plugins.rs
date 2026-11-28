use serde_json::{json, Value};

pub const AWAY_MS: u64 = 5 * 60_000;
const MAX_MINUTES: u64 = 1_440;
const MAX_LOGGED_ML: u64 = 100_000;

#[derive(Clone, Copy, Debug)]
pub struct Hydration {
    pub goal_ml: u64,
    pub serving_ml: u64,
    pub drank_ml: u64,
}

impl Hydration {
    pub fn log(self, date: &str) -> Result<Value, &'static str> {
        let total = self.drank_ml + self.serving_ml;
        if total > MAX_LOGGED_ML { return Err("今天的记录已达到上限"); }
        Ok(json!({"date":date,"drankMl":total}))
    }
}

fn amount(value: &Value, min: u64, max: u64) -> Option<u64> {
    value.as_u64().filter(|v| (min..=max).contains(v))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Stand,
    Water,
}

impl Kind {
    pub const ALL: [Self; 2] = [Self::Stand, Self::Water];

    pub fn id(self) -> &'static str {
        match self { Self::Stand => "sedentary", Self::Water => "hydration" }
    }

    pub fn alert_key(self) -> &'static str {
        match self { Self::Stand => "reminder:sedentary", Self::Water => "reminder:hydration" }
    }
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Preference {
    minutes: Option<u64>,
    enabled: bool,
    goal_ml: u64,
    serving_ml: u64,
}

fn minutes(value: &Value) -> Option<u64> {
    value.as_u64().filter(|v| (1..=MAX_MINUTES).contains(v))
}

impl Preference {
    fn read(value: &Value) -> Self {
        let minutes = minutes(&value["minutes"]);
        let goal_ml = amount(&value["goalMl"], 100, 10_000).unwrap_or(2000);
        let serving_ml = amount(&value["servingMl"], 10, 3000).filter(|v| *v <= goal_ml).unwrap_or(250.min(goal_ml));
        Self { minutes, enabled: minutes.is_some() && value["enabled"] == true, goal_ml, serving_ml }
    }

    fn value(self, kind: Kind) -> Value {
        let mut result = json!({"minutes":self.minutes,"enabled":self.enabled});
        if kind == Kind::Water {
            result["goalMl"] = json!(self.goal_ml);
            result["servingMl"] = json!(self.serving_ml);
        }
        result
    }
}

#[derive(Default)]
pub struct Scheduler {
    preferences: [Preference; 2],
    progress: [u64; 2],
}

impl Scheduler {
    pub fn new(value: &Value) -> Self {
        Self { preferences: Kind::ALL.map(|k| Preference::read(&value[k.id()])), ..Self::default() }
    }

    pub fn state(&self) -> Value {
        Value::Object(Kind::ALL.into_iter().map(|k| (k.id().into(), self.preferences[k as usize].value(k))).collect())
    }

    pub fn hydration(&self, stored: &Value, date: &str) -> Hydration {
        let p = self.preferences[Kind::Water as usize];
        let drank_ml = if stored["date"].as_str() == Some(date) {
            amount(&stored["drankMl"], 0, MAX_LOGGED_ML).unwrap_or(0)
        } else { 0 };
        Hydration { goal_ml: p.goal_ml, serving_ml: p.serving_ml, drank_ml }
    }

    pub fn enabled(&self) -> bool { self.preferences.iter().any(|p| p.enabled) }

    /// Return the new persisted snapshot without changing the live schedule.
    pub fn proposed(&self, request: &Value) -> Result<(Kind, Value), &'static str> {
        let kind = Kind::ALL.into_iter().find(|k| request["id"] == k.id()).ok_or("内置插件不存在")?;
        let mut p = self.preferences[kind as usize];
        if request.get("goalMl").is_some() || request.get("servingMl").is_some() {
            if kind != Kind::Water || request.get("minutes").is_some() || request.get("enabled").is_some() {
                return Err("饮水计划设置无效");
            }
            p.goal_ml = amount(&request["goalMl"], 100, 10_000).ok_or("每日目标须为 100–10000 毫升的整数")?;
            p.serving_ml = amount(&request["servingMl"], 10, 3000).filter(|v| *v <= p.goal_ml)
                .ok_or("每次须为 10–3000 毫升的整数，且不能超过每日目标")?;
            let mut next = self.state();
            next[kind.id()] = p.value(kind);
            return Ok((kind, next));
        }
        match (request.get("minutes"), request.get("enabled")) {
            (Some(value), None) => p.minutes = Some(minutes(value).ok_or("请输入 1–1440 之间的整数分钟")?),
            (None, Some(value)) => {
                p.enabled = value.as_bool().ok_or("启用状态无效")?;
                if p.enabled && p.minutes.is_none() { return Err("请先设置提醒间隔"); }
            }
            _ => return Err("缺少有效的插件设置"),
        }
        let mut next = self.state();
        next[kind.id()] = p.value(kind);
        Ok((kind, next))
    }

    pub fn apply(&mut self, kind: Kind, next: &Value) -> bool {
        let i = kind as usize;
        let preference = Preference::read(&next[kind.id()]);
        if self.preferences[i] == preference { return false; }
        let previous = std::mem::replace(&mut self.preferences[i], preference);
        if previous.minutes != preference.minutes || previous.enabled != preference.enabled {
            self.progress[i] = 0;
        }
        true
    }

    pub fn break_session(&mut self) { self.progress[Kind::Stand as usize] = 0; }

    /// Count awake, active PC time only. Being away resets the continuous-use
    /// estimate; the water reminder resumes from its accumulated active time.
    pub fn sample(&mut self, active_ms: u64, continuous: bool, can_show: bool) -> [bool; 2] {
        if !continuous { self.break_session(); }
        let mut due = [false; 2];
        for kind in Kind::ALL {
            let i = kind as usize;
            let p = self.preferences[i];
            let Some(interval) = p.minutes.filter(|_| p.enabled).map(|m| m * 60_000) else { continue };
            if active_ms == 0 || (kind == Kind::Stand && !continuous) { continue; }
            self.progress[i] = self.progress[i].saturating_add(active_ms);
            if self.progress[i] >= interval {
                // Do not replay missed reminders after notifications resume.
                self.progress[i] = 0;
                due[i] = can_show;
            }
        }
        due
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hydration_targets_validate_and_logging_is_local_day_scoped() {
        let mut s = Scheduler::new(&json!({}));
        assert!(!s.enabled());
        for (goal, serving) in [(99, 15), (150, 151), (3000, 3001), (1600, 0)] {
            assert!(s.proposed(&json!({"id":"hydration","goalMl":goal,"servingMl":serving})).is_err());
        }
        let (k,next) = s.proposed(&json!({"id":"hydration","goalMl":1500,"servingMl":180})).unwrap();
        assert!(s.apply(k, &next));
        assert!(!s.enabled());
        let empty = s.hydration(&Value::Null, "2026-11-28");
        let log = empty.log("2026-11-28").unwrap();
        assert_eq!(empty.goal_ml, 1500);
        assert_eq!(s.hydration(&log, "2026-11-28").drank_ml, 180);
        assert_eq!(s.hydration(&log, "2026-11-29").drank_ml, 0);
    }

    fn setup() -> Scheduler {
        Scheduler::new(&json!({"sedentary":{"enabled":true,"minutes":1},"hydration":{"enabled":true,"minutes":1}}))
    }

    #[test]
    fn manual_integer_is_required_and_bad_persisted_values_are_inert() {
        let s = Scheduler::new(&json!({"sedentary":{"enabled":true,"minutes":-1}}));
        assert!(!s.enabled());
        assert!(s.proposed(&json!({"id":"sedentary","enabled":true})).is_err());
        for value in [json!(0),json!(1441),json!(1.5),json!("15"),json!(null)] {
            assert!(s.proposed(&json!({"id":"sedentary","minutes":value})).is_err());
        }
        let (_, next) = s.proposed(&json!({"id":"sedentary","minutes":1440})).unwrap();
        assert!(!Scheduler::new(&next).enabled());
    }

    #[test]
    fn away_resets_stand_but_pauses_water_and_changes_are_independent() {
        let mut s = setup();
        assert_eq!(s.sample(59_000, true, true), [false, false]);
        assert_eq!(s.sample(0, false, true), [false, false]);
        assert_eq!(s.sample(1_000, true, true), [false, true]);
        let (kind, next) = s.proposed(&json!({"id":"hydration","enabled":false})).unwrap();
        assert!(s.apply(kind, &next));
        assert_eq!(s.sample(59_000, true, true), [true, false]);
        assert_eq!(s.state()["hydration"]["minutes"], 1);
    }

    #[test]
    fn suppressed_deadlines_do_not_replay_and_edit_starts_new_interval() {
        let mut s = setup();
        assert_eq!(s.sample(60_000, true, false), [false, false]);
        assert_eq!(s.sample(1_000, true, true), [false, false]);
        let (kind, next) = s.proposed(&json!({"id":"hydration","minutes":2})).unwrap();
        assert!(s.apply(kind, &next));
        assert_eq!(s.sample(59_000, true, true), [true, false]);
        assert_eq!(s.sample(61_000, true, true), [true, true]);
    }
}
