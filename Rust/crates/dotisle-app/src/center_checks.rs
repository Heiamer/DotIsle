//! Production-scene checks for headers, deck edges and overlay interruption.
use crate::{
    draw::Text,
    gpu::Gpu,
    model::{Action, Model, Page},
    overlay_motion::OverlayMotion,
    views::{self, ViewState},
};
use serde_json::json;
use std::path::Path;

pub fn run(out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    let mut model = Model::new(out.join("data"), true);
    model.tutorial_checked = true;
    if crate::themes::SLOTS.contains(&"Nothing") || crate::themes::SLOTS.len() != 5 {
        return Err("removed theme is still selectable".into());
    }
    let mut text = Text::new();
    let mut gpu = Gpu::new();
    let mut captures = Vec::new();
    for theme in crate::themes::SLOTS {
        model.prefs.control_center_theme = theme.into();
        for dark in [false, true] {
            model.prefs.dark = dark;
            for page in Page::ALL {
                model.page = page;
                model.scroll = 0.;
                model.settings_tab = 255;
                let view = views::render(&model, &mut text, 1200., 800., &ViewState::default());
                for hit in &view.hits {
                    if hit.rect.y0 < 0.
                        || hit.rect.y1 > 800.
                        || hit.rect.x0 < 0.
                        || hit.rect.x1 > 1200.
                    {
                        return Err(format!(
                            "{theme}/{page:?}: hit outside window: {}",
                            hit.label
                        ));
                    }
                }
                if page == Page::Todos {
                    let add = view
                        .hits
                        .iter()
                        .find(|h| matches!(h.action, Action::EditTodo(None)))
                        .ok_or("todo header action missing")?;
                    if add.rect.y0 != crate::center_layout::TODO_ACTION_Y {
                        return Err("todo action lost the shared header reference".into());
                    }
                }
                let name = format!("{theme}-{dark}-{page:?}.png");
                gpu.capture(&view.scene, 1200, 800, &out.join(&name))?;
                captures.push(name);
            }
            model.page = Page::Home;
            model.scroll = crate::overview::STEP * 0.45;
            let view = views::render(
                &model,
                &mut text,
                1200.,
                800.,
                &ViewState {
                    scroll_blur: 1.2,
                    ..Default::default()
                },
            );
            let name = format!("{theme}-{dark}-deck-mid.png");
            gpu.capture(&view.scene, 1200, 800, &out.join(&name))?;
            captures.push(name);
        }
    }
    model.prefs.control_center_theme = "Original".into();
    model.page = Page::Home;
    model.scroll = 0.;
    let mut motion = OverlayMotion::default();
    model.dispatch(Action::EditTodo(None));
    for _ in 0..10 {
        motion.tick(&model, 1. / 120.);
    }
    let opened = motion.value();
    if !(0.05..0.95).contains(&opened) {
        return Err("overlay did not animate into view".into());
    }
    let view = views::render(
        &model,
        &mut text,
        1200.,
        800.,
        &ViewState {
            overlay_progress: Some(opened),
            ..Default::default()
        },
    );
    motion.closing_scene = view.overlay_scene;
    gpu.capture(&view.scene, 1200, 800, &out.join("dialog-enter.png"))?;
    model.dispatch(Action::CancelModal);
    let closing = views::render(
        &model,
        &mut text,
        1200.,
        800.,
        &ViewState {
            overlay_progress: Some(motion.value()),
            closing_overlay: motion.closing_scene.clone(),
            ..Default::default()
        },
    );
    if !closing.hits.is_empty() {
        return Err("retiring dialog exposes controls underneath".into());
    }
    motion.tick(&model, 1. / 120.);
    model.dispatch(Action::EditTodo(None));
    let before = motion.value();
    motion.tick(&model, 0.);
    if motion.value() != before {
        return Err("overlay jumps on reversal".into());
    }
    model.prefs.reduced_motion = true;
    motion.tick(&model, 1. / 120.);
    if motion.value() != 1. {
        return Err("reduced-motion overlay did not snap".into());
    }
    model.dispatch(Action::CancelModal);
    motion.tick(&model, 1. / 120.);
    if motion.value() != 0. || motion.closing_scene.is_some() {
        return Err("closed overlay retains its scene".into());
    }
    model.prefs.reduced_motion = false;
    for age in [0.15, 0.6, 1.36] {
        let view = views::render(
            &model,
            &mut text,
            1200.,
            800.,
            &ViewState {
                signature_age: Some(age),
                ..Default::default()
            },
        );
        if view.hits.iter().any(|h| h.rect.y1 > views::TITLEBAR_H) {
            return Err("opening signature exposes hidden content".into());
        }
        gpu.capture(
            &view.scene,
            1200,
            800,
            &out.join(format!("opening-{age}.png")),
        )?;
    }
    crate::shell::verify_settings_navigation(&out.join("settings-input"))?;
    let report = json!({"success":true,"captures":captures,"overlay_interruption":true,
        "reduced_motion":true,"opening_hides_content_hits":true,"settings_entry_stays_cancelled":true});
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .map_err(|e| e.to_string())
}
