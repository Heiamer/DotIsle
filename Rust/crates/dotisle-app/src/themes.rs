//! Control-centre theme selection and shared colour transitions.
use crate::model::Model;
use dotisle_domain::monet::Scheme;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

pub const SLOTS: [&str; 5] = ["Original", "Miuix", "Terminal", "Pixel", "Gallery"];

pub fn slot(model: &Model) -> u8 {
    match model.prefs.control_center_theme.as_str() {
        "Miuix" => 1,
        "Terminal" => 2,
        "Pixel" => 3,
        "Gallery" | "Swiss" => 4,
        _ => 0,
    }
}
/// Persisted background indices stay stable when a theme is removed.
pub fn background_slot(model: &Model) -> usize {
    [0, 2, 3, 4, 5][slot(model) as usize]
}
pub fn slot_from_fraction(f: f64) -> u8 {
    (f.clamp(0., 1.) * (SLOTS.len() - 1) as f64).round() as u8
}
pub fn label(slot: u8) -> &'static str {
    ["默认", "MIUIX", "终端", "像素", "留白"][(slot as usize).min(SLOTS.len() - 1)]
}
pub const TRANSITION_MS: f64 = 200.;

// ---- theme transition --------------------------------------------------------------------
struct Transition {
    from: u8,
    from_scheme: Scheme,
    dark: bool,
    start: Instant,
    progress: Option<f64>,
}
static TRANS: OnceLock<Mutex<Option<Transition>>> = OnceLock::new();
fn trans() -> &'static Mutex<Option<Transition>> {
    TRANS.get_or_init(|| Mutex::new(None))
}
/// Start a colour/outline/text-colour transition from the palette in force before the
/// switch. Nothing global is repainted in one step.
pub fn begin_transition(from: u8, from_scheme: Scheme, dark: bool) {
    if let Ok(mut g) = trans().lock() {
        *g = Some(Transition {
            from,
            from_scheme,
            dark,
            start: Instant::now(),
            progress: None,
        });
    }
}
pub fn mix(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0., 1.);
    let c = |n: u32| {
        let x = ((a >> n) & 255) as f64;
        let y = ((b >> n) & 255) as f64;
        (x + (y - x) * t).round() as u32
    };
    c(16) << 16 | c(8) << 8 | c(0)
}
fn mix_scheme(a: Scheme, b: Scheme, t: f64) -> Scheme {
    Scheme {
        primary: mix(a.primary, b.primary, t),
        on_primary: mix(a.on_primary, b.on_primary, t),
        primary_container: mix(a.primary_container, b.primary_container, t),
        on_primary_container: mix(a.on_primary_container, b.on_primary_container, t),
        secondary_container: mix(a.secondary_container, b.secondary_container, t),
        on_secondary_container: mix(a.on_secondary_container, b.on_secondary_container, t),
        tertiary_container: mix(a.tertiary_container, b.tertiary_container, t),
        on_tertiary_container: mix(a.on_tertiary_container, b.on_tertiary_container, t),
        surface: mix(a.surface, b.surface, t),
        container: mix(a.container, b.container, t),
        high: mix(a.high, b.high, t),
        on_surface: mix(a.on_surface, b.on_surface, t),
        on_surface_variant: mix(a.on_surface_variant, b.on_surface_variant, t),
        outline: mix(a.outline, b.outline, t),
        outline_variant: mix(a.outline_variant, b.outline_variant, t),
        error: mix(a.error, b.error, t),
    }
}
fn progress() -> Option<f64> {
    let Ok(mut g) = trans().lock() else {
        return None;
    };
    let Some(t) = g.as_mut() else { return None };
    if let Some(p) = t.progress {
        return Some(p.clamp(0., 1.));
    }
    let p = (t.start.elapsed().as_secs_f64() * 1000. / TRANSITION_MS).clamp(0., 1.);
    if p >= 1. {
        g.take();
        return None;
    }
    Some(p)
}
/// Advance/clamp the transition. Returns true while a frame is still needed.
pub fn finish_transition() -> bool {
    progress().is_some()
}
/// Snapshot of the palette in force before the switch while the transition is running.
/// Progress is computed before taking the lock again so this can never self-deadlock.
fn active_from() -> Option<(Scheme, f64)> {
    let p = progress()?;
    if p >= 1. {
        return None;
    }
    let g = trans().lock().ok()?;
    g.as_ref().map(|t| (t.from_scheme, p))
}
/// Blend the target palette with the palette that was on screen before the switch.
pub fn position(target: u8) -> f64 {
    let Some(p) = progress() else {
        return target as f64;
    };
    let from = trans()
        .lock()
        .ok()
        .and_then(|g| g.as_ref().map(|t| t.from))
        .unwrap_or(target);
    let e = 1. - (1. - p).powi(3);
    from as f64 + (target as f64 - from as f64) * e
}
pub fn resolved(model: &Model) -> Scheme {
    if crate::miuix::active(model) {
        crate::miuix::scheme(model.prefs.dark)
    } else if crate::terminal::active(model) {
        crate::terminal::scheme(model.prefs.dark)
    } else if crate::pixel::active(model) {
        crate::pixel::scheme(model.prefs.dark)
    } else if crate::swiss::active(model) {
        crate::swiss::scheme(model.prefs.dark)
    } else {
        model.scheme()
    }
}
pub fn blend_to(target: Scheme) -> Scheme {
    match active_from() {
        Some((from, p)) => mix_scheme(from, target, p),
        None => target,
    }
}
/// Headless verification hook: pin the transition at a given fraction.
pub fn set_transition_progress(p: Option<f64>) {
    if let Ok(mut g) = trans().lock() {
        if let Some(t) = &mut *g {
            t.progress = p;
        }
    }
}
pub fn end_transition() {
    if let Ok(mut g) = trans().lock() {
        *g = None;
    }
}
