use serde_json::{json, Value};

pub const AWAY_MS: u64 = 5 * 60_000;
const MAX_MINUTES: u64 = 1_440;

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
}

fn minutes(value: &Value) -> Option<u64> {
    value.as_u64().filter(|v| (1..=MAX_MINUTES).contains(v))
}

impl Preference {
    fn read(value: &Value) -> Self {
        let minutes = minutes(&value["minutes"]);
        Self { minutes, enabled: minutes.is_some() && value["enabled"] == true }
    }

    fn value(self) -> Value { json!({"minutes":self.minutes,"enabled":self.enabled}) }
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
        Value::Object(Kind::ALL.into_iter().map(|k| (k.id().into(), self.preferences[k as usize].value())).collect())
    }

    pub fn enabled(&self) -> bool { self.preferences.iter().any(|p| p.enabled) }

    /// Return the new persisted snapshot without changing the live schedule.
    pub fn proposed(&self, request: &Value) -> Result<(Kind, Value), &'static str> {
        let kind = Kind::ALL.into_iter().find(|k| request["id"] == k.id()).ok_or("内置插件不存在")?;
        let mut p = self.preferences[kind as usize];
        match (request.get("minutes"), request.get("enabled")) {
            (Some(value), None) => p.minutes = Some(minutes(value).ok_or("请输入 1–1440 之间的整数分钟")?),
            (None, Some(value)) => {
                p.enabled = value.as_bool().ok_or("启用状态无效")?;
                if p.enabled && p.minutes.is_none() { return Err("请先设置提醒间隔"); }
            }
            _ => return Err("缺少有效的插件设置"),
        }
        let mut next = self.state();
        next[kind.id()] = p.value();
        Ok((kind, next))
    }

    pub fn apply(&mut self, kind: Kind, next: &Value) -> bool {
        let i = kind as usize;
        let preference = Preference::read(&next[kind.id()]);
        if self.preferences[i] == preference { return false; }
        self.preferences[i] = preference;
        self.progress[i] = 0;
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
