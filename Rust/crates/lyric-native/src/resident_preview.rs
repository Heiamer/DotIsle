//! Isolated production-renderer coverage of idle residency and task hand-offs.
use super::*;

pub unsafe fn run(app: &mut App, directory: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(directory)?;
    app.renderer = None;
    app.demo = false;
    app.opacity = 1.;
    app.expanded = false;
    app.held = false;
    app.settings["IslandKeepVisibleEnabled"] = json!(true);
    app.settings["Animation"] = json!(false);
    app.snapshot = json!({"track":null,"isPlaying":false});
    app.lyric = "等待播放".into();
    app.compact_secondary.clear();
    app.previous_lyric.clear();
    let mut reports = vec![];
    for mini in [false, true] {
        app.settings["IslandSizeMode"] = json!(if mini { "Mini" } else { "Classic" });
        app.layout.capsule = Rect::new(150., 24., if mini { 160. } else { 260. }, if mini { 18. } else { 40. });
        app.frame = Frame::hidden(app.options());
        for material in ["Acrylic", "Glass"] {
            app.settings["MiniMaterial"] = json!(material);
            app.settings["ClassicCapsuleMaterial"] = json!(material);
            for dpi in [96., 144., 192.] {
                let scale = dpi / 96.;
                let mut renderer = Renderer::new(app.visual_hwnd, (app.layout.host.w * scale) as u32,
                    (app.layout.host.h * scale) as u32, dpi as f32, &app.base.join("Assets/Fonts/NotoSansSC.ttf"))?;
                for task in ["idle", "paused", "music", "notification", "returned", "system", "device", "expanded", "expanded-empty"] {
                    app.banner = Default::default();
                    app.snapshot = if task == "music" || task == "paused" {
                        json!({"track":{"id":"resident-fixture","title":"示例音乐","artist":"DotIsle"},"isPlaying":task=="music"})
                    } else { json!({"track":null,"isPlaying":false}) };
                    if task == "notification" {
                        let r = app.layout.capsule;
                        app.banner.show(notifications::Notice::default(), r, r.h / 2., app.layout.host.w, 80., 0., false);
                    }
                    use dotisle_domain::desktop_widgets::{Metrics, WirelessDevice};
                    app.resident = Default::default();
                    app.expanded = matches!(task, "expanded" | "expanded-empty");
                    app.frame = if app.expanded { Frame::expanded() } else { Frame::hidden(app.options()) };
                    app.refresh_components();
                    if app.selected == resident::ID {
                        app.resident = Default::default();
                        let metrics = Metrics { cpu_percent: Some(37.), memory_percent: Some(62.),
                            wireless_devices: if matches!(task, "expanded" | "device") { vec![
                                WirelessDevice { connected: true, name: "蓝牙耳机 · 测试样本".into(), battery_percent: Some(82), ..Default::default() },
                                WirelessDevice { connected: true, name: "电量未知的无线鼠标".into(), ..Default::default() },
                                WirelessDevice { connected: false, name: "不该出现的设备".into(), battery_percent: Some(100), ..Default::default() },
                            ] } else { vec![] }, ..Default::default() };
                        app.resident.update(Instant::now(), resident::Choices {time: !matches!(task, "system" | "device"),
                            system: task == "system", devices: task == "device"}, &metrics, false, app.expanded, || (9, 41));
                    }
                    app.layout.card.h = app.card_height();
                    let mut scene = app.scene();
                    scene.layout_scale = scale;
                    if (scene.background_opacity - 1.).abs() > 0.001 { return Err("resident background state mismatch".into()); }
                    renderer.render(&scene, false)?;
                    let file = format!("resident-{}-{}-{task}-{dpi:.0}.png", if mini { "mini" } else { "classic" }, material.to_lowercase());
                    renderer.snapshot(&directory.join(&file))?;
                    reports.push(json!({"file":file,"backgroundOpacity":scene.background_opacity,"task":task,"synthetic":true,"deviceRows":app.resident.view.devices.len(),"activeComponent":app.selected}));
                }
            }
        }
    }
    std::fs::write(directory.join("report.json"), serde_json::to_vec_pretty(&json!({"success":true,"cases":reports}))?)?;
    Ok(())
}
