//! A temporary lesson on the production island; only native gesture outcomes advance it.
use super::*;

pub const KIND: u8 = 7;
const FIRST: &str = "practice:first";
const SECOND: &str = "practice:second";
const IDLE_LIMIT: Duration = Duration::from_secs(180);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Step { ClickExpand, HoverExpand, Move, ClickCollapse, HoverCollapse, Swipe, Hide, Recover }

impl Step {
    fn code(self) -> &'static str {
        match self {
            Self::ClickExpand => "clickExpand", Self::HoverExpand => "hoverExpand",
            Self::Move => "move", Self::ClickCollapse => "clickCollapse", Self::HoverCollapse => "hoverCollapse",
            Self::Swipe => "swipe", Self::Hide => "hide", Self::Recover => "recover",
        }
    }
}

fn steps(click: bool, hover: bool, floating: bool, swipe: bool, hide: bool) -> Vec<Step> {
    let mut steps = Vec::with_capacity(7);
    if hover || click {
        steps.push(if hover { Step::HoverExpand } else { Step::ClickExpand });
        if floating { steps.push(Step::Move); }
        steps.push(if hover { Step::HoverCollapse } else { Step::ClickCollapse });
    }
    if swipe { steps.push(Step::Swipe); }
    if hide { steps.extend([Step::Hide, Step::Recover]); }
    steps
}

pub struct Practice {
    owner: String,
    steps: Vec<Step>,
    index: usize,
    until: Instant,
    previous_hidden: bool,
    previous_held: bool,
    previous_expanded: bool,
    previous_selection: String,
    previous_bounds: Option<RECT>,
    previous_manual_until: Option<Instant>,
}

impl Practice {
    pub fn step(&self) -> Option<Step> { self.steps.get(self.index).copied() }
    fn advance(&mut self, event: Step) -> bool {
        if self.step() != Some(event) { return false; }
        self.index += 1;
        self.until = Instant::now() + IDLE_LIMIT;
        true
    }
    pub fn state(&self) -> Value {
        json!({"id":self.owner,"index":self.index,"total":self.steps.len(),
            "step":self.step().map_or("done", Step::code)})
    }
}

pub fn panel(id: &str) -> Option<activities::Panel> {
    let title = match id { FIRST => "练习卡片 A", SECOND => "练习卡片 B", _ => return None };
    Some(activities::Panel { kind: KIND, title: title.into(), value: "手势实操".into(), ..Default::default() })
}

pub fn move_button(height: f64) -> Rect { Rect::new(20., height - 58., 132., 38.) }

impl App {
    pub fn practice_step(&self) -> Option<Step> { self.practice.as_ref().and_then(Practice::step) }

    pub fn practice_event(&mut self, event: Step) {
        if self.practice.as_mut().is_some_and(|practice| practice.advance(event)) {
            self.dirty = true;
            self.wake_practice_center();
        }
    }

    fn wake_practice_center(&self) {
        if let Some(wake) = &self.settings_wake { wake.signal(); }
    }

    pub fn practice_components(&mut self) {
        if !matches!(self.selected.as_str(), FIRST | SECOND) {
            self.selected = FIRST.into();
            self.components.clear();
            self.components.extend([FIRST.into(), SECOND.into()]);
        }
        self.panel = panel(&self.selected);
    }

    pub fn practice_expiry(&mut self) {
        if self.practice.as_ref().is_some_and(|p| self.session_locked || Instant::now() >= p.until) {
            self.stop_practice();
        }
    }

    fn stop_practice(&mut self) {
        let Some(p) = self.practice.take() else { return; };
        self.reset_practice_input();
        self.selected = p.previous_selection;
        self.user_hidden = p.previous_hidden;
        self.hidden_mini = false;
        self.edge_revealed = false;
        self.edge_hover_since = None;
        self.edge_leave_since = None;
        self.held = p.previous_held;
        self.manual_until = p.previous_manual_until.filter(|until| *until > Instant::now());
        self.refresh_components();
        let wanted = !self.session_locked && self.base_needed();
        self.retarget(wanted && p.previous_expanded && !p.previous_hidden);
        self.display_wanted = wanted;
        self.fade(if wanted && !p.previous_hidden { 1. } else { 0. });
        self.compact_key.clear();
        if !self.hwnd.0.is_null() {
            let _ = unsafe {
                if let Some(bounds) = p.previous_bounds {
                    self.position_on(MonitorFromRect(&bounds, MONITOR_DEFAULTTONEAREST))
                } else { self.position() }
            };
            let _ = unsafe { window_level::apply(self.visual_hwnd, self.hwnd, self.effective_topmost(), false) };
        }
        self.dirty = true;
        self.wake_practice_center();
    }

    fn reset_practice_input(&mut self) {
        self.restore_hidden = false;
        self.hide_at = None;
        self.leave_at = None;
        self.manual_until = None;
        self.dragging = None;
        self.hide_drag = None;
        self.swipe = None;
        self.seeking = false;
        self.pressed = 0;
        self.drag_target.clear();
        self.drag_panel = None;
        self.previous_panel = None;
        self.slide.set(0.);
        self.switching = false;
        self.dismiss_offset.set(0.);
        if !self.hwnd.0.is_null() { let _ = unsafe { ReleaseCapture() }; }
    }

    pub fn practice_request(&mut self, value: &Value) -> Value {
        let Some(id) = value["id"].as_str().filter(|id| !id.is_empty() && id.len() <= 64) else {
            return json!({"success":false,"message":"教程会话无效。"});
        };
        let action = value["action"].as_str().unwrap_or("");
        let owns = self.practice.as_ref().is_some_and(|p| p.owner == id);
        match action {
            "start" => {
                if owns { return json!({"success":true,"practice":self.practice.as_ref().map(Practice::state)}); }
                if self.practice.is_some() || self.session_locked || self.dragging.is_some() || self.seeking || self.pressed != 0
                    || self.banner.visible() || self.alerts.visible() || self.osd_banner.visible() {
                    return json!({"success":false,"message":"灵动岛正在处理其他操作，请稍后再试。"});
                }
                let mini = self.mini();
                let steps = steps(mini || self.flag("ClickAnywhereExpand", true), !mini && self.flag("HoverExpand", false),
                    !mini && self.flag("Floating", false), self.flag("SwipeActivities", true) && self.components.len() > 1, true);
                if steps.is_empty() { return json!({"success":false,"message":"当前手势均已关闭。可在「交互与提示」中开启手势后再练习。"}); }
                let mut bounds = RECT::default();
                let previous_bounds = (!self.hwnd.0.is_null() && unsafe { GetWindowRect(self.hwnd, &mut bounds).is_ok() }).then_some(bounds);
                let previous_manual_until = self.manual_until;
                self.reset_practice_input();
                self.practice = Some(Practice { owner: id.into(), steps, index: 0, until: Instant::now() + IDLE_LIMIT,
                    previous_hidden: self.user_hidden, previous_held: self.held, previous_expanded: self.expanded,
                    previous_selection: self.selected.clone(), previous_bounds, previous_manual_until });
                self.user_hidden = false;
                self.hidden_mini = false;
                self.held = false;
                self.cursor_inside = false;
                self.display_wanted = true;
                self.refresh_components();
                self.retarget(false);
                self.fade(1.);
                self.compact_key.clear();
                self.dirty = true;
                self.wake_practice_center();
                json!({"success":true,"practice":self.practice.as_ref().map(Practice::state)})
            }
            "finish" | "skip" => {
                if (self.practice.is_some() && !owns) || (action == "finish" && (!owns || self.practice_step().is_some())) {
                    return json!({"success":false,"message":"请先完成当前实操步骤。"});
                }
                let old = self.settings["GesturePracticeSeen"].clone();
                self.settings["GesturePracticeSeen"] = json!(true);
                if let Err(message) = self.save() {
                    self.settings["GesturePracticeSeen"] = old;
                    return json!({"success":false,"message":message});
                }
                if owns { self.stop_practice(); }
                json!({"success":true})
            }
            "cancel" => {
                if owns { self.stop_practice(); }
                json!({"success":true})
            }
            _ => json!({"success":false,"message":"教程操作无效。"}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_modes_determine_the_exact_lesson_sequence() {
        use Step::*;
        assert_eq!(steps(true, false, false, true, true), [ClickExpand, ClickCollapse, Swipe, Hide, Recover]);
        assert_eq!(steps(true, true, true, false, false), [HoverExpand, Move, HoverCollapse]);
        assert!(steps(false, false, true, false, false).is_empty());
    }

    #[test]
    fn stale_or_wrong_gestures_never_finish_a_lesson() {
        let mut p = Practice {
            owner: "session".into(), steps: vec![Step::ClickExpand, Step::ClickCollapse, Step::Hide, Step::Recover],
            index: 0, until: Instant::now(), previous_hidden: false, previous_held: false,
            previous_expanded: false, previous_selection: "music".into(), previous_bounds: None,
            previous_manual_until: None,
        };
        assert!(!p.advance(Step::Recover));
        assert!(p.advance(Step::ClickExpand));
        assert!(!p.advance(Step::ClickExpand));
        assert!(p.advance(Step::ClickCollapse));
        assert!(!p.advance(Step::Swipe));
        assert!(p.advance(Step::Hide));
        assert!(p.advance(Step::Recover));
        assert!(!p.advance(Step::Recover));
        assert_eq!(p.state()["step"], "done");
    }
}
