//! Isolated production-renderer coverage of idle residency and task hand-offs.
use super::*;

pub unsafe fn run(app: &mut App, directory: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    app.renderer = None;
    app.opacity = 1.;
    app.expanded = false;
    app.held = false;
    app.settings["IslandResidentEnabled"] = json!(true);
    app.settings["Animation"] = json!(false);
    app.snapshot = json!({"track":null,"isPlaying":false});
    app.lyric = "等待播放".into();
    app.compact_secondary.clear();
    app.previous_lyric.clear();
    let mut reports = vec![];
    for mini in [false, true] {
        app.settings["IslandSizeMode"] = json!(if mini { "Mini" } else { "Classic" });
        app.layout.capsule = Rect::new(150., 24., if mini { 100. } else { 260. }, if mini { 18. } else { 40. });
        app.frame = Frame::hidden(app.options());
        for material in ["Acrylic", "Glass"] {
            app.settings["MiniMaterial"] = json!(material);
            app.settings["ClassicCapsuleMaterial"] = json!(material);
            for dpi in [96., 144., 192.] {
                let scale = dpi / 96.;
                let mut renderer = Renderer::new(app.visual_hwnd, (app.layout.host.w * scale) as u32,
                    (app.layout.host.h * scale) as u32, dpi as f32, &app.base.join("Assets/Fonts/NotoSansSC.ttf"))?;
                for task in ["idle", "paused", "music", "notification", "returned"] {
                    app.banner = Default::default();
                    app.snapshot = if task == "music" || task == "paused" {
                        json!({"track":{"id":"resident-fixture","title":"示例音乐","artist":"DotIsle"},"isPlaying":task=="music"})
                    } else { json!({"track":null,"isPlaying":false}) };
                    if task == "notification" {
                        let r = app.layout.capsule;
                        app.banner.show(notifications::Notice::default(), r, r.h / 2., app.layout.host.w, 80., 0., false);
                    }
                    let mut scene = app.scene();
                    scene.layout_scale = scale;
                    let expected = if task == "music" || task == "notification" { 1. } else { 0. };
                    if (scene.background_opacity - expected).abs() > 0.001 { return Err("resident background state mismatch".into()); }
                    renderer.render(&scene, false)?;
                    let file = format!("resident-{}-{}-{task}-{dpi:.0}.png", if mini { "mini" } else { "classic" }, material.to_lowercase());
                    renderer.snapshot(&directory.join(&file))?;
                    reports.push(json!({"file":file,"backgroundOpacity":scene.background_opacity,"task":task,"synthetic":true}));
                }
            }
        }
    }
    std::fs::write(directory.join("report.json"), serde_json::to_vec_pretty(&json!({"success":true,"cases":reports}))?)?;
    Ok(())
}
