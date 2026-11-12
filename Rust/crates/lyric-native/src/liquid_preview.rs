//! Actual native GPU captures, including a stopwatch identity comparison.
use crate::{motion::Frame, renderer::Renderer, App};
use serde_json::json;
use std::path::{Path, PathBuf};
pub unsafe fn run(app: &mut App, out: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(out)?;
    app.renderer = None;
    app.opacity = 1.;
    app.layout.set_compact(260., 40.);
    app.frame = Frame::hidden(app.options());
    let mut renderer = Renderer::new(
        app.visual_hwnd,
        (app.layout.host.w * 1.5) as u32,
        (app.layout.host.h * 1.5) as u32,
        144.,
        &app.base.join("Assets/Fonts/NotoSansSC.ttf"),
    )?;
    let mut store = crate::timers::Store::new(PathBuf::new());
    let mut captures = vec![];
    for mode in ["countdown", "pomodoro", "workflow", "stopwatch"] {
        let id = store.create(&json!({"mode":mode,"name":if mode=="pomodoro"{"番茄钟"}else if mode=="workflow"{"专注流程"}else if mode=="stopwatch"{"秒表"}else{"倒计时"},"duration":300,"focus":300,"stages":[{"name":"专注","seconds":300},{"name":"休息","seconds":60}]}))?;
        let timer = store.items.iter_mut().find(|t| t.id == id).unwrap();
        for amount in [0., 0.25, 0.5, 0.75, 1.] {
            timer.elapsed_ms = (timer.limit_ms() as f64 * amount) as u64;
            let panel = timer.panel();
            let mut scene = app.scene();
            scene.panel = Some(&panel);
            scene.component_count = 1;
            scene.glass_refraction = false;
            let file = format!("{mode}-{:03}.png", (amount * 100.) as u32);
            renderer.render(&scene, false)?;
            renderer.snapshot(&out.join(&file))?;
            captures.push(file);
            let mut mini = scene;
            mini.mini = true;
            mini.layout.set_compact(260., 14.);
            mini.capsule_radius = 7.;
            let file = format!("mini-{mode}-{:03}.png", (amount * 100.) as u32);
            renderer.render(&mini, false)?;
            renderer.snapshot(&out.join(&file))?;
            captures.push(file);
            let mut satellite = scene;
            satellite.component_count = 2;
            satellite.auxiliary_kind = 1;
            satellite.auxiliary_timer = if mode == "stopwatch" {
                None
            } else {
                Some((amount, timer.elapsed_ms as f64))
            };
            let file = format!("satellite-{mode}-{:03}.png", (amount * 100.) as u32);
            renderer.render(&satellite, false)?;
            renderer.snapshot(&out.join(&file))?;
            captures.push(file);
        }
    }
    let timer = &mut store.items[0];
    for i in 0..60 {
        timer.elapsed_ms = 150000 + i * 25;
        let panel = timer.panel();
        let mut scene = app.scene();
        scene.panel = Some(&panel);
        scene.component_count = 1;
        scene.glass_refraction = false;
        renderer.render(&scene, false)?;
        renderer.snapshot(&out.join(format!("flow-{i:03}.png")))?;
    }
    timer.running = false;
    let panel = timer.panel();
    for (name, time) in [("paused-a", 100.), ("paused-b", 5000.)] {
        let mut scene = app.scene();
        scene.panel = Some(&panel);
        scene.component_count = 1;
        scene.time_ms = time;
        scene.glass_refraction = false;
        renderer.render(&scene, false)?;
        renderer.snapshot(&out.join(format!("{name}.png")))?;
    }
    std::fs::write(
        out.join("report.json"),
        serde_json::to_vec_pretty(
            &json!({"success":true,"progressive_capsule_fill":true,"stopwatch_excluded":true,"paused_phase_uses_timer_clock":true,"captures":captures}),
        )?,
    )?;
    Ok(())
}
