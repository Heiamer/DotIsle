//! Interruptible overlay fades reuse the application's existing spring clock.
use crate::{model::Model, motion::Spring};
use vello::Scene;

pub fn active(model: &Model) -> bool {
    model.dialog.is_some() || model.picker.is_some() || model.font_picker.is_some() || model.palette
}

pub struct OverlayMotion {
    spring: Spring,
    pub closing_scene: Option<Scene>,
}
impl Default for OverlayMotion {
    fn default() -> Self {
        Self {
            spring: Spring::new(0.),
            closing_scene: None,
        }
    }
}
impl OverlayMotion {
    pub fn value(&self) -> f64 {
        self.spring.value.clamp(0., 1.)
    }
    pub fn tick(&mut self, model: &Model, dt: f64) -> bool {
        self.spring.target = if active(model) { 1. } else { 0. };
        let moving = self.spring.tick(dt, model.prefs.reduced_motion);
        if !active(model) && self.spring.settled() {
            self.closing_scene = None;
        }
        moving
    }
    pub fn snap(&mut self, model: &Model) {
        self.spring.snap(if active(model) { 1. } else { 0. });
        if !active(model) {
            self.closing_scene = None;
        }
    }
}
