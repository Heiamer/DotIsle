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

#[test]
fn allegro_new_kinds_photo_dropzone_and_system_pro_render_and_stay_inside() {
    let mut m = model();
    m.native_state = json!({
        "systemMetrics": { "cpuPercent": 32.5, "memoryPercent": 58.0, "gpuPercent": 24.0 }
    });
    let mut text = Text::new_lazy_japanese();

    let placements: Vec<dotisle_domain::desktop_widgets::Placement> = serde_json::from_value(json!([
        { "id": "pf1", "kind": "photoFrame", "size": "small", "style": "original", "material": "lumen", "x": 40, "y": 40, "config": { "caption": "2026 桌搭" } },
        { "id": "dz1", "kind": "dropzone", "size": "medium", "style": "original", "material": "liquidGlass", "x": 240, "y": 40 },
        { "id": "sp1", "kind": "systemPro", "size": "medium", "style": "original", "material": "acrylic", "x": 40, "y": 240 }
    ]))
    .unwrap();
    m.desktop_widgets.items = placements.clone();

    let drop_items = vec![allegro::efficiency::DropItem {
        path: "C:\\temp\\notes.md".into(),
        name: "notes.md".into(),
        tag: "DOC",
        size_label: "4.2 KB".into(),
    }];
    let mut pro = allegro::telemetry::ProTelemetry::new();
    pro.cpu_history.push(28.0);
    pro.cpu_history.push(32.5);
    pro.gpu_history.push(20.0);
    pro.gpu_history.push(24.0);
    pro.net_rx_kbps = Some(512.0);
    pro.net_tx_kbps = Some(48.0);

    for (p, dz) in placements
        .iter()
        .zip([&[][..], &drop_items[..], &[][..]])
    {
        let (w, h) = p.dimensions();
        let ctx = desktop_cards::AllegroCardContext {
            placement: p,
            width: w,
            height: h,
            pointer: (-1., -1.),
            menu: false,
            scroll: 0.,
            todo_edit: None,
            note_edit: None,
            time: 0.,
            media_position: None,
            wallpaper_crop: None,
            wallpaper_luma: None,
            dropzone_items: dz,
            photo_image: None,
            photo_meta: (0, 0),
            pro: Some(&pro),
            stack_info: None,
            quote_index: 0,
        };
        let view = desktop_cards::render_allegro_card(&m, &mut text, &ctx);
        assert!(!view.scene.is_empty());
        assert_inside(&view, w, h);
        if p.kind == Kind::Dropzone {
            assert!(view.hits.iter().any(|h| matches!(h.action, Action::DropzoneClear(_))));
            assert!(view.hits.iter().any(|h| matches!(h.action, Action::DropzoneOpen(_, 0))));
        }
    }
}

#[test]
fn allegro_three_materials_and_extended_menu() {
    let m = model();
    let mut text = Text::new_lazy_japanese();
    for mat in allegro::theme::WidgetMaterial::ALL {
        let p = dotisle_domain::desktop_widgets::Placement {
            id: "w-menu".into(),
            kind: Kind::Clock,
            size: Size::Medium,
            style: dotisle_domain::desktop_widgets::Style::Original,
            config: Default::default(),
            x: 0,
            y: 0,
            width: 0.,
            height: 0.,
            hidden: false,
            revision: 1,
            material: mat.as_str().into(),
            opacity: 0.9,
            stack_id: "stack-1".into(),
            stack_order: 0,
        };
        let (w, h) = p.dimensions();
        let ctx = desktop_cards::AllegroCardContext {
            placement: &p,
            width: w,
            height: h,
            pointer: (-1., -1.),
            menu: true,
            scroll: 0.,
            todo_edit: None,
            note_edit: None,
            time: 0.,
            media_position: None,
            wallpaper_crop: None,
            wallpaper_luma: Some(0.85),
            dropzone_items: &[],
            photo_image: None,
            photo_meta: (0, 0),
            pro: None,
            stack_info: Some((0, 2)),
            quote_index: 0,
        };
        let view = desktop_cards::render_allegro_card(&m, &mut text, &ctx);
        assert_inside(&view, w, h);
        assert!(!view.hits.iter().any(|h| matches!(h.action, Action::WidgetStyle(..))));
        assert!(view.hits.iter().any(|h| matches!(h.action, Action::WidgetMaterial(..))));
        assert!(view.hits.iter().any(|h| matches!(h.action, Action::StackUnpack(_))));
        assert!(view.hits.iter().any(|h| matches!(h.action, Action::StackNext(_))));
        assert!(view.hits.iter().any(|h| matches!(h.action, Action::StackPrev(_))));
    }
}

#[test]
fn allegro_note_checkboxes_and_inline_edit_hits() {
    let mut m = model();
    let p: dotisle_domain::desktop_widgets::Placement =
        serde_json::from_value(json!({
            "id": "note1",
            "kind": "note",
            "size": "large",
            "x": 0,
            "y": 0,
            "config": { "text": "- [ ] 买咖啡\n- [x] 写代码" }
        }))
        .unwrap();
    m.desktop_widgets.items = vec![p.clone()];
    let mut text = Text::new_lazy_japanese();
    let (w, h) = p.dimensions();
    let ctx = desktop_cards::AllegroCardContext {
        placement: &p,
        width: w,
        height: h,
        pointer: (-1., -1.),
        menu: false,
        scroll: 0.,
        todo_edit: None,
        note_edit: None,
        time: 0.,
        media_position: None,
        wallpaper_crop: None,
        wallpaper_luma: None,
        dropzone_items: &[],
        photo_image: None,
        photo_meta: (0, 0),
        pro: None,
        stack_info: None,
        quote_index: 0,
    };
    let view = desktop_cards::render_allegro_card(&m, &mut text, &ctx);
    assert_inside(&view, w, h);
    assert!(view.hits.iter().any(|h| matches!(h.action, Action::NoteToggleCheck(ref id, 0) if id == "note1")));
    assert!(view.hits.iter().any(|h| matches!(h.action, Action::NoteToggleCheck(ref id, 1) if id == "note1")));
    assert!(view.hits.iter().any(|h| matches!(h.action, Action::NoteInlineEdit(ref id) if id == "note1")));

    let edit = allegro::efficiency::NoteEdit::new("note1", "", "编辑中");
    let ctx_edit = desktop_cards::AllegroCardContext {
        note_edit: Some(&edit),
        ..ctx
    };
    let view_edit = desktop_cards::render_allegro_card(&m, &mut text, &ctx_edit);
    assert_inside(&view_edit, w, h);
    assert!(view_edit.hits.iter().any(|h| matches!(h.action, Action::NoteSave)));
    assert!(view_edit.hits.iter().any(|h| matches!(h.action, Action::NoteCancel)));
}

#[test]
fn allegro_all_26_kinds_render_cleanly_in_all_three_materials() {
    let mut m = model();
    m.native_state = json!({
        "systemMetrics": {
            "cpuPercent": 36.0,
            "memoryPercent": 62.0,
            "gpuPercent": 28.0,
            "batteryPercent": 84,
            "externalPower": true,
            "wirelessDevices": [
                { "kind": "headphones", "name": "AirPods Pro", "batteryPercent": 88, "connected": true }
            ]
        },
        "screenTime": { "todayMs": 5400000, "yesterdayMs": 4200000, "week": [1000, 2000, 3000, 4000, 5000, 6000, 5400000] },
        "widgetData": {
            "focusStats": { "todayMs": 2700000, "week": [600000, 1200000, 900000, 1800000, 1500000, 2100000, 2700000] },
            "weather1": { "temperature": 22.0, "city": "上海", "high": 25.0, "low": 16.0, "stale": false },
            "storage1": { "free": 128000000000.0, "total": 512000000000.0 },
            "world1": { "clock": "09:41", "city": "东京", "date": "2026-11-27 星期五" }
        }
    });
    let mut text = Text::new_lazy_japanese();
    for kind in Kind::ALL {
        for size in Size::ALL {
            for mat in allegro::theme::WidgetMaterial::ALL {
                let captions: &[&str] = if kind == Kind::AppFolder { &["4", "9"] } else { &["2026 Studio"] };
                for cap in captions {
                    let id = match kind {
                        Kind::Weather => "weather1",
                        Kind::Storage => "storage1",
                        Kind::WorldClock => "world1",
                        _ => "w-all",
                    };
                    let p = dotisle_domain::desktop_widgets::Placement {
                        id: id.into(),
                        kind,
                        size,
                        style: dotisle_domain::desktop_widgets::Style::Original,
                        config: serde_json::from_value(json!({
                            "date": "2027-01-01",
                            "deadline": "2027-01-01 09:00",
                            "city": "上海",
                            "text": "- [ ] 晨间梳理\n- [x] 完成重构",
                            "target": "https://apple.com",
                            "title": "捷径",
                            "caption": cap
                        }))
                        .unwrap(),
                        x: 0,
                        y: 0,
                        width: 0.,
                        height: 0.,
                        hidden: false,
                        revision: 1,
                        material: mat.as_str().into(),
                        opacity: 0.92,
                        stack_id: "".into(),
                        stack_order: 0,
                    };
                    m.desktop_widgets.items = vec![p.clone()];
                    let (w, h) = p.dimensions();
                    let ctx = desktop_cards::AllegroCardContext {
                        placement: &p,
                        width: w,
                        height: h,
                        pointer: (-1., -1.),
                        menu: false,
                        scroll: 0.,
                        todo_edit: None,
                        note_edit: None,
                        time: 1.0,
                        media_position: Some(0.4),
                        wallpaper_crop: None,
                        wallpaper_luma: Some(0.82),
                        dropzone_items: &[],
                        photo_image: None,
                        photo_meta: (0, 0),
                        pro: None,
                        stack_info: None,
                        quote_index: 2,
                    };
                    let view = desktop_cards::render_allegro_card(&m, &mut text, &ctx);
                    assert!(!view.scene.is_empty(), "{:?} {:?} {:?} empty scene", kind, size, mat);
                    assert_inside(&view, w, h);
                    if kind == Kind::AppFolder {
                        let launches = view.hits.iter().filter(|h| matches!(h.action, Action::AppFolderLaunch(..))).count();
                        let expected = if *cap == "9" { 9 } else { 4 };
                        assert_eq!(launches, expected, "appFolder {:?} cap={cap} launches", size);
                    }
                }
            }
        }
    }
}

#[test]
fn apple_context_menu_exposes_sizes_materials_and_actions() {
    let mut text = Text::new_lazy_japanese();
    let s = dotisle_domain::monet::scheme(0x0088FF, false);
    let mut scene = vello::Scene::new();
    let mut ui = crate::widgets::Ui::new(
        &mut scene,
        &mut text,
        s,
        (-1., -1.),
        crate::draw::rect(0., 0., 400., 400.),
    );
    let p = dotisle_domain::desktop_widgets::Placement {
        id: "ctx-1".into(),
        kind: Kind::AnalogClock,
        size: Size::Medium,
        style: dotisle_domain::desktop_widgets::Style::Original,
        config: Default::default(),
        x: 0,
        y: 0,
        width: 0.,
        height: 0.,
        hidden: false,
        revision: 1,
        material: "liquidGlass".into(),
        opacity: 0.9,
        stack_id: "".into(),
        stack_order: 0,
    };
    let panel = desktop_cards::paint_apple_context_menu(&mut ui, &p, (20., 30.), false);
    assert!((panel.width() - desktop_cards::CONTEXT_MENU_W).abs() < 0.01);
    assert!((panel.height() - desktop_cards::CONTEXT_MENU_H).abs() < 0.01);
    for sz in Size::ALL {
        assert!(ui.hits.iter().any(|h| matches!(h.action, Action::WidgetResize(ref id, s2) if id == "ctx-1" && s2 == sz)));
    }
    for mat in allegro::theme::WidgetMaterial::ALL {
        assert!(ui.hits.iter().any(|h| matches!(h.action, Action::WidgetMaterial(ref id, ref m2) if id == "ctx-1" && m2 == mat.as_str())));
    }
    assert!(ui.hits.iter().any(|h| matches!(h.action, Action::WidgetConfigure(ref id) if id == "ctx-1")));
    assert!(ui.hits.iter().any(|h| matches!(h.action, Action::EditModeToggle)));
    assert!(ui.hits.iter().any(|h| matches!(h.action, Action::WidgetHide(ref id) if id == "ctx-1")));
    assert!(ui.hits.iter().any(|h| matches!(h.action, Action::WidgetRemove(ref id) if id == "ctx-1")));

    // 应用文件夹右键应露出 4/9 宫格直达切换操作
    let mut ui2 = crate::widgets::Ui::new(
        &mut scene,
        &mut text,
        s,
        (-1., -1.),
        crate::draw::rect(0., 0., 400., 400.),
    );
    let mut p_folder = p;
    p_folder.id = "folder-1".into();
    p_folder.kind = Kind::AppFolder;
    let _ = desktop_cards::paint_apple_context_menu(&mut ui2, &p_folder, (20., 30.), false);
    assert!(ui2.hits.iter().any(|h| matches!(h.action, Action::AppFolderToggleGrid(ref id) if id == "folder-1")));
}
