use crate::{desktop_cards, draw::Text, model::{Action, Model}, views::{TodoEdit, TodoField}};
use dotisle_domain::desktop_widgets::{Kind, Size};
use serde_json::json;

fn model() -> Model {
    let mut m = Model::new();
    m.todos.items = serde_json::from_value(json!([
        {"id":"t1","title":"第一件事","content":"","date":"2026-11-25","time":"09:00","daily":false,"color":"#F3D79B","completed":false},
        {"id":"t2","title":"第二件事","content":"","date":"2026-11-25","time":"10:00","daily":false,"color":"#57BFFF","completed":false},
        {"id":"t3","title":"第三件事","content":"","date":"2026-11-25","time":"11:00","daily":false,"color":"#9A8BEB","completed":false},
        {"id":"t4","title":"第四件事","content":"","date":"2026-11-25","time":"12:00","daily":false,"color":"#34C77B","completed":false},
        {"id":"t5","title":"第五件事","content":"","date":"2026-11-25","time":"13:00","daily":false,"color":"#FF9EAD","completed":false}
    ]))
    .unwrap();
    m
}
fn assert_inside(view: &crate::views::View, w: f64, h: f64) {
    for hit in &view.hits {
        assert!(
            hit.rect.x0 >= 0. && hit.rect.y0 >= 0. && hit.rect.x1 <= w + 0.01 && hit.rect.y1 <= h + 0.01,
            "hit {} outside {}x{}: {:?}",
            hit.label,
            w,
            h,
            hit.rect
        );
    }
}
#[test]
fn large_todo_editor_exposes_fields_and_buttons_inside_the_card() {
    let m = model();
    let mut text = Text::new_lazy_japanese();
    let editor = TodoEdit {
        title: "写周报".into(),
        date: "2026-11-25".into(),
        time: "18:30".into(),
        ..Default::default()
    };
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Large, "w1", 352., 352., (-1., -1.), false, false, 0.,
        Some(&editor), 0., None,
    );
    assert_inside(&view, 352., 352.);
    assert!(view.hits.iter().any(|h| matches!(h.action, Action::TodoSave)));
    assert!(view.hits.iter().any(|h| matches!(h.action, Action::TodoCancel)));
    assert!(view
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::TodoField(TodoField::Title))));
    assert!(view
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::TodoField(TodoField::Date))));
    assert!(view
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::TodoField(TodoField::Time))));
}
#[test]
fn hidden_card_is_a_full_width_restore_target() {
    let m = model();
    let mut text = Text::new_lazy_japanese();
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Large, "w1", 220., desktop_cards::HIDDEN_BAR, (-1., -1.),
        false, true, 0., None, 0., None,
    );
    assert_inside(&view, 220., desktop_cards::HIDDEN_BAR);
    assert_eq!(view.hits.len(), 1);
    assert!(matches!(view.hits[0].action, Action::WidgetRestore(_)));
}
#[test]
fn scrolled_todo_rows_never_draw_hits_outside_the_window() {
    let m = model();
    let mut text = Text::new_lazy_japanese();
    for scroll in [0., 40., 9999.] {
        let view = desktop_cards::render(
            &m, &mut text, Kind::Todos, Size::Large, "w1", 352., 260., (-1., -1.), false, false,
            scroll, None, 0., None,
        );
        assert_inside(&view, 352., 260.);
    }
}
#[test]
fn todo_metrics_scales_with_card_height() {
    let (rows, max) = desktop_cards::todo_metrics(352., 352., Size::Large, 8);
    assert_eq!(rows, 4);
    assert!(max > 0.);
    let (rows, max) = desktop_cards::todo_metrics(352., 176., Size::Medium, 4);
    assert_eq!(rows, 2);
    assert!(max > 0.);
    let (rows, _) = desktop_cards::todo_metrics(176., 176., Size::Small, 3);
    assert_eq!(rows, 1);
}
#[test]
fn editor_field_rectangles_stay_inside_a_large_card() {
    for field in [TodoField::Title, TodoField::Date, TodoField::Time] {
        let r = desktop_cards::editor_field_rect(352., field);
        assert!(r.x0 >= 0. && r.y0 >= 0. && r.x1 <= 352. && r.y1 <= 352.);
    }
}#[test]
#[ignore = "requires a working D3D11 adapter"]
fn gpu_captures_the_editor_and_the_hidden_bar() {
    let m = model();
    let mut text = Text::new_lazy_japanese();
    let mut gpu = crate::gpu::Gpu::new();
    let dir = std::env::temp_dir().join(format!("dotisle-widget-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let editor = TodoEdit {
        title: "写周报".into(),
        date: "2026-11-25".into(),
        time: "18:30".into(),
        ..Default::default()
    };
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Large, "w1", 352., 352., (-1., -1.), false, false, 0.,
        Some(&editor), 0., None,
    );
    gpu.capture(&view.scene, 352, 352, &dir.join("todos-editor.png"))
        .unwrap();
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Large, "w1", 220., desktop_cards::HIDDEN_BAR,
        (-1., -1.), false, true, 0., None, 0., None,
    );
    gpu.capture(
        &view.scene,
        220,
        desktop_cards::HIDDEN_BAR as u32,
        &dir.join("todos-hidden.png"),
    )
    .unwrap();
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Large, "w1", 352., 352., (-1., -1.), false, false, 0.,
        None, 0., None,
    );
    gpu.capture(&view.scene, 352, 352, &dir.join("todos-list.png"))
        .unwrap();
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Medium, "w1", 500., 420., (-1., -1.), false, false, 0.,
        None, 0., None,
    );
    gpu.capture(&view.scene, 500, 420, &dir.join("todos-wide.png"))
        .unwrap();
    for name in ["todos-editor.png", "todos-hidden.png", "todos-list.png", "todos-wide.png"] {
        let image = image::open(dir.join(name)).unwrap();
        assert!(image.width() > 0 && image.height() > 0);
    }
    eprintln!("captures: {}", dir.display());
}#[test]
#[ignore = "requires a working D3D11 adapter"]
fn gpu_captures_mono_focus_clock() {
    let mut m = Model::new();
    m.native_state = serde_json::json!({"settings":{"TimerMonoDigits":true}});
    let mut text = Text::new_lazy_japanese();
    let mut gpu = crate::gpu::Gpu::new();
    let dir = std::env::temp_dir().join(format!("dotisle-mono-render-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    for (size, name) in [
        (Size::Large, "focus-mono-large.png"),
        (Size::Medium, "focus-mono-medium.png"),
    ] {
        let (w, h) = size.dimensions();
        let view = desktop_cards::render(
            &m, &mut text, Kind::Focus, size, "w1", w, h, (-1., -1.), false, false, 0., None, 0.,
            Some(0.),
        );
        gpu.capture(&view.scene, w as u32, h as u32, &dir.join(name))
            .unwrap();
    }
    eprintln!("captures: {}", dir.display());
}#[test]
#[ignore = "requires a working D3D11 adapter"]
fn gpu_captures_empty_large_todo_actions() {
    let mut m = model();
    m.todos.items.clear();
    let mut text = Text::new_lazy_japanese();
    let mut gpu = crate::gpu::Gpu::new();
    let dir = std::env::temp_dir().join(format!("dotisle-empty-large-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let view = desktop_cards::render(
        &m, &mut text, Kind::Todos, Size::Large, "w1", 352., 352., (-1., -1.), false, false, 0.,
        None, 0., None,
    );
    assert!(view
        .hits
        .iter()
        .any(|h| matches!(h.action, Action::TodoInlineEdit(None))));
    gpu.capture(&view.scene, 352, 352, &dir.join("todos-empty-large.png"))
        .unwrap();
    eprintln!("captures: {}", dir.display());
}