//! Integration checks for the 3.5 navigation, shared controls and widget scenes.
use crate::{
    draw::Text,
    gpu::Gpu,
    model::{Action, Model, Page},
    views::{self, ViewState},
};
use dotisle_domain::desktop_widgets::{Kind, Size, Style};
pub fn run(out: &std::path::Path, text: &mut Text, gpu: &mut Gpu) -> Result<Vec<String>, String> {
    if std::env::var_os("DOTISLE_LAYOUT_AUDIT").is_some() {
        crate::layout_checks::run(&out.join("layout-audit"), text, gpu)?;
    }
    crate::music_card::verify(&out.join("lyrics-motion"))?;
    crate::music_card::capture(&out.join("lyrics-motion"), text, gpu)?;
    crate::shell::verify_settings_sliders(&out.join("settings-sliders"))?;
    crate::shell::verify_overview_inputs(&out.join("deck-inputs"))?;
    crate::shell::verify_settings_navigation(&out.join("settings-inputs"))?;
    crate::native_sync::verify(&out.join("native-sync"))?;
    let mut prefs = Model::new(out.join("settings-routing"), true);
    prefs.toast = None;
    for dark in [false, true] {
        prefs.prefs.dark = dark;
        for (w, h) in [(900., 680.), (1200., 800.)] {
            prefs.dispatch(Action::SettingsTab(255));
            let root = views::render(&prefs, text, w, h, &ViewState::default());
            gpu.capture(
                &root.scene,
                w as u32,
                h as u32,
                &out.join(format!("settings-root-{w:.0}-{dark}.png")),
            )?;
            for (group, leaf, label) in [
                (243, 8, "计时与待办"),
                (244, 4, "通用与窗口"),
                (245, 3, "关于与更新"),
            ] {
                if !root.hits.iter().any(|hit| {
                    hit.label == label && matches!(hit.action,Action::SettingsTab(tab)if tab==leaf)
                }) {
                    return Err(format!("{label} retained a redundant category"));
                }
                prefs.dispatch(Action::SettingsTab(group));
                if prefs.settings_tab != leaf {
                    return Err("single settings destination was not normalized".into());
                }
                let v = views::render(&prefs, text, w, h, &ViewState::default());
                if !v
                    .hits
                    .iter()
                    .any(|h| h.label == "全部设置" && matches!(h.action, Action::SettingsTab(255)))
                {
                    return Err("direct settings leaf gained a redundant return step".into());
                }
                gpu.capture(
                    &v.scene,
                    w as u32,
                    h as u32,
                    &out.join(format!("settings-direct-{leaf}-{w:.0}-{dark}.png")),
                )?;
                let interrupted = views::render(
                    &prefs,
                    text,
                    w,
                    h,
                    &ViewState {
                        page_enter: Some(0.),
                        ..Default::default()
                    },
                );
                let name = out.join(format!("settings-no-lift-{leaf}-{w:.0}-{dark}.png"));
                gpu.capture(&interrupted.scene, w as u32, h as u32, &name)?;
                if std::fs::read(&name).map_err(|e| e.to_string())?
                    != std::fs::read(out.join(format!("settings-direct-{leaf}-{w:.0}-{dark}.png")))
                        .map_err(|e| e.to_string())?
                {
                    return Err("settings moved or faded during a destination change".into());
                }
            }
        }
    }
    verify_acrylic_material(text, gpu, out)?;
    let mut m = Model::new(out.join("widgets-model"), true);
    m.prefs.control_center_theme = "Original".into();
    let mut captures = crate::widget_extras::verify(out, text, gpu)?;
    if Page::ALL.contains(&Page::Island) || Page::ALL.contains(&Page::Music) {
        return Err("legacy settings/music destination leaked into navigation".into());
    }
    m.mode = 2;
    m.dispatch(Action::QuickFocus);
    if m.timers.items.len() != 1 || m.timers.items[0].mode != "pomodoro" {
        return Err("overview quick focus inherited stopwatch mode".into());
    }
    let id = m.timers.items[0].id.clone();
    let revision = m
        .timer_action_value(&id, "pause")
        .ok_or("focus command missing")?["revision"]
        .clone();
    if revision.is_null() {
        return Err("widget timer command lacks revision".into());
    }
    m.dispatch(Action::Timer(id, "pause"));
    if m.timers.items[0].running {
        return Err("overview focus did not pause".into());
    }
    for kind in Kind::ALL {
        m.dispatch(Action::WidgetAdd(kind, kind.default_size()));
    }
    if m.desktop_widgets.items.len() != Kind::ALL.len() {
        return Err("widget commands did not add every kind".into());
    }
    for dark in [false, true] {
        m.prefs.dark = dark;
        for kind in Kind::ALL {
            for size in Size::ALL {
                let view = crate::desktop_cards::render(
                    &m,
                    text,
                    kind,
                    size,
                    "capture",
                    (-1., -1.),
                    true,
                    0.,
                );
                let (w, h) = size.dimensions();
                if view
                    .hits
                    .iter()
                    .any(|h| h.rect.x0 < 0. || h.rect.x1 > w || h.rect.y1 > h64(size))
                {
                    return Err("widget hit outside window".into());
                }
                let name = format!("widget-{kind:?}-{size:?}-{dark}.png");
                gpu.capture(&view.scene, w as u32, h as u32, &out.join(&name))?;
                captures.push(name);
            }
        }
    }
    m.toast = None;
    m.prefs.dark = false;
    for (w, h) in [(1200., 800.), (900., 680.)] {
        for page in [
            Page::Home,
            Page::Widgets,
            Page::Notifications,
            Page::Settings,
        ] {
            m.page = page;
            m.scroll = 0.;
            m.settings_tab = 255;
            let state = ViewState {
                rail_y: 112. + Page::ALL.iter().position(|p| *p == page).unwrap() as f64 * 82.,
                ..Default::default()
            };
            let first = views::render(&m, text, w, h, &state);
            let nav = first
                .hits
                .iter()
                .filter(|hit| matches!(hit.action, Action::Navigate(_)) && hit.rect.y0 > h - 110.)
                .count();
            if nav < 6 {
                return Err("capsule dock omitted destinations".into());
            }
            for scroll in [0., first.max_scroll] {
                m.scroll = scroll;
                let view = views::render(&m, text, w, h, &state);
                if view
                    .hits
                    .iter()
                    .any(|hit| hit.rect.y1 > h || hit.rect.x1 > w)
                {
                    return Err("3.5 view hit outside window".into());
                }
                let name = format!("350-{page:?}-{}-{}.png", w as u32, scroll as u32);
                gpu.capture(&view.scene, w as u32, h as u32, &out.join(&name))?;
                captures.push(name);
            }
        }
    }

    m.prefs.control_center_theme = "Original".into();
    m.toast = None;
    for dark in [false, true] {
        m.prefs.dark = dark;
        for (w, h) in [(1200., 800.), (900., 680.)] {
            m.page = Page::Home;
            for i in 0..5 {
                m.scroll = i as f64 * crate::overview::STEP;
                let v = views::render(&m, text, w, h, &ViewState::default());
                if v.max_scroll != 0. {
                    return Err("infinite deck gained a finite document extent".into());
                }
                let relevant = v
                    .hits
                    .iter()
                    .filter(|hit| match i {
                        0 => matches!(
                            hit.action,
                            Action::Media(_) | Action::Seek | Action::LayoutOpen
                        ),
                        1 => matches!(hit.action, Action::Timer(..) | Action::QuickFocus),
                        2 => matches!(hit.action, Action::EditTodo(_)),
                        3 => matches!(hit.action, Action::Navigate(Page::Notifications)),
                        _ => matches!(hit.action, Action::WidgetAdd(..)),
                    })
                    .count();
                if relevant == 0 {
                    return Err(format!("deck card {i} lost its inline actions"));
                }
                if i == 0 && v.hits.iter().any(|h| matches!(h.action, Action::Offset(_))) {
                    return Err("removed lyric offset toolbar returned to the overview".into());
                }
                let name = format!("deck-{i}-{w:.0}-{dark}.png");
                gpu.capture(&v.scene, w as u32, h as u32, &out.join(&name))?;
                captures.push(name);
            }
            m.scroll = crate::overview::STEP * 0.45;
            let v = views::render(
                &m,
                text,
                w,
                h,
                &ViewState {
                    scroll_blur: 0.65,
                    ..Default::default()
                },
            );
            let name = format!("deck-rolling-{w:.0}-{dark}.png");
            gpu.capture(&v.scene, w as u32, h as u32, &out.join(&name))?;
            captures.push(name);
        }
    }
    m.page = Page::Home;
    m.prefs.control_center_theme = "Original".into();
    m.prefs.dark = false;
    for position in [
        -crate::overview::STEP,
        0.,
        crate::overview::ring::CYCLE,
        crate::overview::ring::CYCLE * 100. + crate::overview::STEP * 4.,
    ] {
        m.scroll = position;
        let v = views::render(&m, text, 900., 680., &ViewState::default());
        let active = crate::overview::ring::index(position);
        let relevant = v.hits.iter().any(|hit| match active {
            0 => matches!(hit.action, Action::Seek),
            4 => matches!(hit.action, Action::WidgetAdd(..)),
            _ => false,
        });
        if !relevant {
            return Err("looped card lost its live controls".into());
        }
        let mut wrapped = m.scroll;
        wrapped += crate::overview::ring::CYCLE;
        m.scroll = wrapped;
        let next = views::render(&m, text, 900., 680., &ViewState::default());
        if v.hits.len() != next.hits.len()
            || v.hits
                .iter()
                .zip(&next.hits)
                .any(|(a, b)| a.label != b.label || a.rect != b.rect)
        {
            return Err("loop seam changed card hit geometry".into());
        }
    }
    for (label, position) in [
        (
            "before",
            crate::overview::ring::CYCLE - crate::overview::STEP * 0.25,
        ),
        ("seam", crate::overview::ring::CYCLE),
        (
            "after",
            crate::overview::ring::CYCLE + crate::overview::STEP * 0.25,
        ),
        ("reverse", -crate::overview::STEP * 0.25),
    ] {
        m.scroll = position;
        let v = views::render(
            &m,
            text,
            1200.,
            800.,
            &ViewState {
                scroll_blur: 0.25,
                ..Default::default()
            },
        );
        let name = format!("deck-loop-{label}.png");
        gpu.capture(&v.scene, 1200, 800, &out.join(&name))?;
        captures.push(name);
    }
    if std::env::var_os("DOTISLE_CAPTURE_DECK_MOTION").is_some() {
        let frames = out.join("motion");
        std::fs::create_dir_all(&frames).map_err(|e| e.to_string())?;
        for frame in 0..96 {
            let t = frame as f64 / 32.;
            let whole = t.floor();
            let f = ((t - whole - 0.25) / 0.6).clamp(0., 1.);
            let eased = f * f * (3. - 2. * f);
            m.scroll = (4. + whole + eased) * crate::overview::STEP;
            let v = views::render(
                &m,
                text,
                900.,
                680.,
                &ViewState {
                    scroll_blur: if f > 0. && f < 1. { 0.45 } else { 0. },
                    ..Default::default()
                },
            );
            gpu.capture(&v.scene, 900, 680, &frames.join(format!("{frame:03}.png")))?;
        }
    }
    m.dispatch(Action::LayoutOpen);
    let baseline = m.layout_editor.as_ref().unwrap().draft.clone();
    m.dispatch(Action::LayoutMove(0, 1));
    if m.setting("MusicCustomLayout").is_object() {
        return Err("draft leaked into saved settings".into());
    }
    m.dispatch(Action::LayoutDiscard);
    if m.layout_editor.as_ref().unwrap().draft != baseline {
        return Err("discard failed to restore the baseline".into());
    }
    m.dispatch(Action::LayoutToggle(2));
    m.dispatch(Action::LayoutStyle(1, "Segmented".into()));
    m.dispatch(Action::LayoutApply);
    let applied = lyric_core::custom_layout::Layout::from_value(&m.setting("MusicCustomLayout"))?;
    if !applied.thumb_enabled || applied.bar_style != "Segmented" {
        return Err("atomic layout apply dropped style settings".into());
    }
    for theme in ["Original", "Gallery"] {
        m.prefs.control_center_theme = theme.into();
        for dark in [false, true] {
            m.prefs.dark = dark;
            for (w, h) in [(1200., 800.), (900., 680.)] {
                for tab in 0..3 {
                    m.dispatch(Action::LayoutTab(tab));
                    m.toast = None;
                    let first = views::render(&m, text, w, h, &ViewState::default());
                    for scroll in [0., first.max_scroll] {
                        m.scroll = scroll;
                        let v = views::render(&m, text, w, h, &ViewState::default());
                        if !v
                            .hits
                            .iter()
                            .any(|hit| matches!(hit.action, Action::LayoutApply))
                        {
                            return Err("editor apply button left the viewport".into());
                        }
                        let name = format!("layout-{theme}-{tab}-{w:.0}-{dark}-{scroll:.0}.png");
                        gpu.capture(&v.scene, w as u32, h as u32, &out.join(&name))?;
                        captures.push(name);
                    }
                }
            }
        }
    }
    verify_canvas(&mut m, text)?;
    verify_independent_styles(&mut m, text, gpu, out, &mut captures)?;
    verify_cover_variants(&mut m, text, gpu, out, &mut captures)?;
    m.prefs.control_center_theme = "Original".into();
    m.page = Page::Settings;
    m.settings_tab = 3;
    m.scroll = 0.;
    m.layout_editor = None;
    m.toast = None;
    for dark in [false, true] {
        m.prefs.dark = dark;
        for (w, h) in [(900., 680.), (1200., 800.)] {
            let v = views::render(&m, text, w, h, &ViewState::default());
            if v.hits.iter().any(|h| matches!(h.action, Action::Import)) {
                return Err("removed migration card is still reachable".into());
            }
            let name = format!("about-{w:.0}-{dark}.png");
            gpu.capture(&v.scene, w as u32, h as u32, &out.join(&name))?;
            captures.push(name);
        }
    }
    m.page = Page::Home;
    m.settings_tab = 255;
    m.prefs.dark = false;
    let frames = out.join("signature");
    std::fs::create_dir_all(&frames).map_err(|e| e.to_string())?;
    for frame in 0..49 {
        let age = frame as f64 / 30.;
        let v = views::render(
            &m,
            text,
            900.,
            680.,
            &ViewState {
                signature_age: if age < dotisle_render::brand_signature::duration(false) {
                    Some(age)
                } else {
                    None
                },
                ..Default::default()
            },
        );
        if frame < 44 && v.hits.iter().any(|h| h.rect.y0 >= views::TITLEBAR_H) {
            return Err("signature exposed covered controls".into());
        }
        gpu.capture(&v.scene, 900, 680, &frames.join(format!("{frame:03}.png")))?;
    }
    m.dispatch(Action::IslandSettings(2));
    if m.page != Page::Settings || m.settings_tab != 130 {
        return Err("island preferences did not merge into settings".into());
    }
    Ok(captures)
}
fn h64(size: Size) -> f64 {
    size.dimensions().1
}

fn verify_canvas(m: &mut Model, text: &mut Text) -> Result<(), String> {
    m.dispatch(Action::LayoutOpen);
    m.dispatch(Action::LayoutTab(0));
    let font = m.setting("FontSize").as_f64().unwrap_or(14.);
    let e = m.layout_editor.as_mut().unwrap();
    let original = e.draft.clone();
    let r = lyric_core::custom_layout::elements(&e.draft.geometry(400., 1., font))
        .into_iter()
        .find(|(id, _)| *id == 1)
        .ok_or("title missing")?
        .1;
    e.begin_drag(Some(1));
    e.drag_to(
        1,
        [20., 40., 1.],
        (20. + r.x + 5., 40. + r.y + 5.),
        false,
        font,
    );
    e.drag_to(
        1,
        [20., 40., 1.],
        (20. + r.x + 21., 40. + r.y + 29.),
        false,
        font,
    );
    e.end_drag(false);
    let moved = e.draft.clone();
    if moved == original || !moved.enabled {
        return Err("canvas drag did not save a placement".into());
    }
    m.dispatch(Action::LayoutUndo);
    if m.layout_editor.as_ref().unwrap().draft != original {
        return Err("canvas undo did not restore layout".into());
    }
    m.dispatch(Action::LayoutRedo);
    if m.layout_editor.as_ref().unwrap().draft != moved {
        return Err("canvas redo lost placement".into());
    }
    let e = m.layout_editor.as_mut().unwrap();
    e.begin_drag(Some(1));
    e.drag_to(1, [0., 0., 1.], (r.x, r.y), false, font);
    e.drag_to(1, [0., 0., 1.], (r.x + 72., r.y + 72.), false, font);
    e.end_drag(true);
    if e.draft != moved {
        return Err("cancelled canvas drag was applied".into());
    }
    let e = m.layout_editor.as_mut().unwrap();
    let r = lyric_core::custom_layout::elements(&e.draft.geometry(400., 1., font))
        .into_iter()
        .find(|(id, _)| *id == 6)
        .ok_or("progress missing")?
        .1;
    e.begin_drag(Some(6));
    e.drag_to(6, [0., 0., 1.], (r.x + r.w - 48., r.y), true, font);
    e.end_drag(false);
    let g = e.draft.geometry(400., 1., font);
    if g.track.w >= r.w {
        return Err("canvas resize did not change progress width".into());
    }
    if (g.track.x - g.progress_hit.x).abs() > 8. || g.seek_chip.x < g.track.x - 40. {
        return Err("progress resize separated its seek geometry".into());
    }
    m.dispatch(Action::LayoutNudge(1, 0));
    m.dispatch(Action::LayoutApply);
    let stored = lyric_core::custom_layout::Layout::from_value(&m.setting("MusicCustomLayout"))?;
    if stored.elements.len() < 2 {
        return Err("canvas apply omitted placements".into());
    }
    for tab in 0..3 {
        m.dispatch(Action::LayoutTab(tab));
        for (w, h) in [(900., 680.), (1200., 800.)] {
            m.scroll = 0.;
            let v = views::render(m, text, w, h, &ViewState::default());
            m.scroll = v.max_scroll;
            let v = views::render(m, text, w, h, &ViewState::default());
            for t in 0..3 {
                if !v
                    .hits
                    .iter()
                    .any(|h| matches!(h.action,Action::LayoutTab(n) if n==t))
                {
                    return Err("fixed theme tab disappeared when inspector scrolled".into());
                }
            }
        }
    }
    Ok(())
}

fn verify_independent_styles(
    m: &mut Model,
    text: &mut Text,
    gpu: &mut Gpu,
    out: &std::path::Path,
    captures: &mut Vec<String>,
) -> Result<(), String> {
    for style in Style::ALL {
        m.prefs.control_center_theme = "Original".into();
        m.prefs.dark = false;
        let v = crate::desktop_cards::render_style(
            m,
            text,
            Kind::Music,
            Size::Large,
            style,
            "independent",
            (-1., -1.),
            false,
            0.,
        );
        let name = format!("independent-{style:?}.png");
        let reference = out.join(&name);
        gpu.capture(&v.scene, 352, 352, &reference)?;
        let bytes = std::fs::read(&reference).map_err(|e| e.to_string())?;
        captures.push(name);
        for theme in crate::themes::SLOTS {
            m.prefs.control_center_theme = theme.into();
            m.prefs.dark = true;
            let v = crate::desktop_cards::render_style(
                m,
                text,
                Kind::Music,
                Size::Large,
                style,
                "independent",
                (-1., -1.),
                false,
                0.,
            );
            let path = out.join("independent-comparison.png");
            gpu.capture(&v.scene, 352, 352, &path)?;
            if std::fs::read(path).map_err(|e| e.to_string())? != bytes {
                return Err(format!("{style:?} widget changed with shell theme {theme}"));
            }
        }
    }
    m.prefs.control_center_theme = "Original".into();
    m.prefs.dark = false;
    m.dispatch(Action::LayoutOpen);
    m.dispatch(Action::LayoutTab(0));
    m.scroll = 0.;
    for (w, h) in [(900., 680.), (1200., 800.)] {
        let v = views::render(m, text, w, h, &ViewState::default());
        let toolbar = v
            .hits
            .iter()
            .filter(|h| {
                matches!(
                    h.action,
                    Action::LayoutUndo
                        | Action::LayoutRedo
                        | Action::LayoutReset
                        | Action::LayoutDiscard
                        | Action::LayoutApply
                        | Action::SettingsTab(255)
                )
            })
            .collect::<Vec<_>>();
        if toolbar.len() != 6 {
            return Err("editor toolbar is incomplete".into());
        }
        for (i, a) in toolbar.iter().enumerate() {
            if a.rect.width() < 70. || a.rect.height() < 34. {
                return Err("editor toolbar clipped a label".into());
            }
            for b in &toolbar[i + 1..] {
                let overlap = a.rect.intersect(b.rect);
                if overlap.width() > 0. && overlap.height() > 0. {
                    return Err("editor toolbar buttons overlap".into());
                }
            }
        }
        let canvas = v
            .hits
            .iter()
            .filter(|h| matches!(h.action, Action::LayoutElement(..)))
            .collect::<Vec<_>>();
        if canvas.is_empty()
            || canvas.iter().any(|a| {
                toolbar.iter().any(|b| {
                    let r = a.rect.intersect(b.rect);
                    r.width() > 0. && r.height() > 0.
                })
            })
        {
            return Err("canvas intersects toolbar".into());
        }
    }
    Ok(())
}

fn verify_cover_variants(
    m: &mut Model,
    text: &mut Text,
    gpu: &mut Gpu,
    out: &std::path::Path,
    captures: &mut Vec<String>,
) -> Result<(), String> {
    let original = m.artwork.take();
    m.page = Page::Home;
    m.scroll = 0.;
    m.prefs.control_center_theme = "Original".into();
    m.toast = None;
    for variant in ["none", "light", "dark"] {
        m.artwork = match variant {
            "none" => None,
            _ => {
                let source =
                    image::load_from_memory(include_bytes!("../../../../Assets/DemoCover.png"))
                        .map_err(|e| e.to_string())?;
                Some(crate::model::Artwork::new(
                    source.brighten(if variant == "light" { 90 } else { -90 }),
                ))
            }
        };
        for dark in [false, true] {
            m.prefs.dark = dark;
            let v = views::render(m, text, 900., 680., &ViewState::default());
            let name = format!("deck-cover-{variant}-{dark}.png");
            gpu.capture(&v.scene, 900, 680, &out.join(&name))?;
            captures.push(name);
        }
    }
    let wallpaper = out.join("material-wallpaper.png");
    std::fs::write(
        &wallpaper,
        include_bytes!("../../../../Assets/DemoCover.png"),
    )
    .map_err(|e| e.to_string())?;
    m.prefs.backgrounds[0].path = wallpaper.to_string_lossy().into_owned();
    m.prefs.backgrounds[0].enabled = true;
    m.prefs.backgrounds[0].opacity = 0.8;
    m.refresh_background();
    m.artwork = None;
    for dark in [false, true] {
        m.prefs.dark = dark;
        let v = views::render(m, text, 900., 680., &ViewState::default());
        let name = format!("deck-wallpaper-{dark}.png");
        gpu.capture(&v.scene, 900, 680, &out.join(&name))?;
        captures.push(name);
    }
    m.prefs.backgrounds[0] = Default::default();
    m.refresh_background();
    m.artwork = original;
    m.prefs.dark = false;
    Ok(())
}

fn verify_acrylic_material(
    text: &mut Text,
    gpu: &mut Gpu,
    out: &std::path::Path,
) -> Result<(), String> {
    use crate::{
        draw::{self, rect},
        widgets::Ui,
    };
    use vello::{kurbo::Affine, Scene};
    let mut scene = Scene::new();
    let s = dotisle_domain::monet::scheme(0x5264a8, false);
    let window = rect(0., 0., 480., 280.);
    draw::fill(&mut scene, window, 0., 0xf4f0ee);
    for x in (0..480).step_by(8) {
        draw::fill(&mut scene, rect(x as f64, 0., 4., 280.), 0., 0x384c68);
    }
    let backdrop = scene.snapshot();
    let area = rect(80., 60., 320., 160.);
    scene.push_clip_layer(Affine::IDENTITY, &area.inflate(4., 4.));
    let mut ui = Ui {
        scene: &mut scene,
        text,
        s,
        selection_values: Default::default(),
        motion_targets: vec![],
        press_layers: vec![],
        switches: Default::default(),
        state_layers: vec![],
        hits: vec![],
        pointer: (-1., -1.),
        focus: None,
        pressed: None,
        clip: window,
        miuix: false,
        terminal: false,
        pixel: false,
        swiss: false,
        face: None,
    };
    crate::acrylic::surface(&mut ui, area, 28., &backdrop, Affine::IDENTITY, false);
    let surface_path = out.join("material-surface.png");
    gpu.capture(&ui.scene.snapshot(), 480, 280, &surface_path)?;
    let art = crate::model::Artwork::new(image::DynamicImage::ImageRgba8(
        image::RgbaImage::from_fn(64, 64, |x, y| {
            image::Rgba([
                if (x / 2 + y / 2) % 2 == 0 { 240 } else { 30 },
                96,
                180,
                255,
            ])
        }),
    ));
    crate::acrylic::album(&mut ui, area, 28., &art.image);
    let album_path = out.join("material-album-clipping.png");
    gpu.capture(&ui.scene.snapshot(), 480, 280, &album_path)?;
    let before = image::open(surface_path)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    let after = image::open(album_path)
        .map_err(|e| e.to_string())?
        .to_rgba8();
    for y in 0..280 {
        for x in 0..480 {
            if !(78..402).contains(&x) || !(58..222).contains(&y) {
                if before.get_pixel(x, y) != after.get_pixel(x, y) {
                    return Err(format!("album frost escaped card clipping at {x},{y}"));
                }
            }
        }
    }
    // Eight-pixel stripes must disappear under the material, while the source stays crisp.
    for x in [90, 202, 378] {
        let contrast =
            (before.get_pixel(x, 140)[0] as i32 - before.get_pixel(x + 4, 140)[0] as i32).abs();
        if contrast > 8 {
            return Err(format!(
                "acrylic backdrop lost its frost near a parent clip at {x}: {contrast}"
            ));
        }
    }
    Ok(())
}
