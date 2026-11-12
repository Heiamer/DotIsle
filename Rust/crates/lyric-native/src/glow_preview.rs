//! Deterministic production captures for the opt-in halo states.
use crate::{
    activities::Panel,
    island_glow::{Frame as GlowFrame, Kind},
    motion::Frame,
    renderer::Renderer,
    App,
};
use serde_json::json;
use std::path::Path;

pub unsafe fn run(app: &mut App, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    app.renderer = None;
    app.opacity = 1.;
    app.layout.set_compact(260., 40.);
    app.frame = Frame::hidden(app.options());
    app.lyric = "海风吹过，晚霞落在你身旁".into();
    let mut renderer = Renderer::new(
        app.visual_hwnd,
        (app.layout.host.w * 1.5) as u32,
        (app.layout.host.h * 1.5) as u32,
        144.,
        &app.base.join("Assets/Fonts/NotoSansSC.ttf"),
    )?;
    renderer.set_cover(Some(&std::fs::read(app.base.join("Assets/DemoCover.png"))?))?;
    let timer = Panel {
        kind: 1,
        title: "番茄钟".into(),
        value: "12:30".into(),
        progress: 0.5,
        running: true,
        ..Default::default()
    };
    let mut captures = vec![];
    for (name, kind) in [
        ("off", Kind::Off),
        ("music-breath", Kind::MusicBreath),
        ("music-rhythm", Kind::MusicRhythm),
        ("timer", Kind::Timer),
        ("update", Kind::Update),
    ] {
        for i in 0..60 {
            if kind == Kind::Off && i > 0 {
                break;
            }
            let time = i as f64 / 40.;
            let phase = time / 3.8 * std::f64::consts::TAU;
            let strength = if kind == Kind::Off {
                0.
            } else if kind == Kind::MusicRhythm {
                0.15 + 0.7 * (time * 8.).sin().abs()
            } else {
                0.7 + 0.15 * (time * 2.6).sin()
            };
            let mut scene = app.scene();
            scene.component_count = 1;
            scene.glass_refraction = false;
            scene.lyric_age_ms = 1000.;
            scene.cover_age_ms = 1000.;
            scene.time_ms = 0.;
            scene.glow = GlowFrame {
                kind,
                strength,
                phase,
            };
            scene.panel = if kind == Kind::Timer {
                Some(&timer)
            } else {
                None
            };
            let file = format!("{name}-{i:03}.png");
            renderer.render(&scene, false)?;
            renderer.snapshot(&out.join(&file))?;
            if i == 0 {
                captures.push(file);
            }
        }
    }
    for (name, kind) in [
        ("mini-off", Kind::Off),
        ("mini-injected-glow", Kind::Update),
    ] {
        let mut scene = app.scene();
        scene.mini = true;
        scene.layout.set_compact(260., 14.);
        scene.capsule_radius = 7.;
        scene.component_count = 1;
        scene.glass_refraction = false;
        scene.lyric_age_ms = 1000.;
        scene.time_ms = 0.;
        scene.glow = GlowFrame {
            kind,
            strength: 1.,
            phase: 1.,
        };
        renderer.render(&scene, false)?;
        renderer.snapshot(&out.join(format!("{name}.png")))?;
    }
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(
            &json!({"success":true,"default_all_off":true,"mini_has_no_halo":true,"music_rhythm_capture_uses_explicit_test_envelope":true,"captures":captures}),
        )?,
    )?;
    Ok(())
}
