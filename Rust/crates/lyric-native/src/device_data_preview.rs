//! Isolated rendering regression with fixtures and optional live telemetry.
use super::*;

pub unsafe fn run(app:&mut App,directory:&Path)->std::result::Result<(),Box<dyn std::error::Error>>{
    std::fs::create_dir_all(directory)?;
    app.opacity=1.;app.renderer=None;
    let mut reports=Vec::new();
    for dpi in [96.,144.,192.]{
        for mini in [false,true]{
            app.settings["IslandSizeMode"]=json!(if mini{"Mini"}else{"Classic"});
            app.alerts=Default::default();app.device_watch=Default::default();
            app.device_watch.update(&[],1,true);
            let event=devices::Event::new("{fixture-container}".into(),devices::Kind::Headphones,true,"示例耳机".into());
            app.enqueue_device(&event);
            let base=Rect::new(150.,24.,if mini{100.}else{260.},if mini{18.}else{40.});
            app.alerts.begin(base,base.h/2.,app.layout.host,24.,0.,false);
            let scale=dpi/96.;
            let mut renderer=Renderer::new(app.visual_hwnd,(app.layout.host.w*scale)as u32,(app.layout.host.h*scale)as u32,dpi as f32,&app.base.join("Assets/Fonts/NotoSansSC.ttf"))?;
            for(level,stamp,name)in [(None,2,"unknown"),(Some(60),3,"known"),(Some(59),4,"updated")]{
                let device=dotisle_domain::desktop_widgets::WirelessDevice{id:"fixture-headphones".into(),container_id:Some("fixture-container".into()),name:"示例耳机".into(),kind:"headphones".into(),connected:true,battery_percent:level};
                let arrivals=app.device_watch.update(&[device],stamp,true);
                app.alerts.refresh_devices(&app.device_watch);
                for alert in arrivals{app.alerts.enqueue(alert);}
                if app.alerts.pending(){return Err("duplicate device popup after telemetry update".into())}
                let alert=app.alerts.active.as_ref().ok_or("device alert missing")?;
                let expected=level.map(|n|format!("{n}%")).unwrap_or_default();
                if alert.trailing!=expected{return Err("device battery display mismatch".into())}
                let mut scene=app.scene();scene.layout_scale=scale;
                scene.alert_rect=Rect::new((app.layout.host.w-400.)/2.,24.,400.,48.);
                scene.alert_radius=24.;scene.alert_mix=1.;
                renderer.render(&scene,false)?;
                let file=format!("device-{}-{name}-{dpi:.0}.png",if mini{"mini"}else{"classic"});
                renderer.snapshot(&directory.join(&file))?;
                reports.push(json!({"file":file,"battery":expected,"synthetic":true,"duplicateQueued":false}));
            }
            app.alerts.active=Some(capsule_alert::Alert::device("fixture-headphones","Headphones","示例耳机",false,Some(59),false));
            if !app.alerts.active.as_ref().unwrap().trailing.is_empty(){return Err("disconnected device retained battery placeholder".into())}
            let mut scene=app.scene();scene.layout_scale=scale;
            scene.alert_rect=Rect::new((app.layout.host.w-400.)/2.,24.,400.,48.);scene.alert_radius=24.;scene.alert_mix=1.;
            renderer.render(&scene,false)?;
            let file=format!("device-{}-disconnected-{dpi:.0}.png",if mini{"mini"}else{"classic"});
            renderer.snapshot(&directory.join(&file))?;
            reports.push(json!({"file":file,"battery":"","connected":false,"synthetic":true}));
        }
    }
    if let Ok(id)=std::env::var("DOTISLE_PREVIEW_LIVE_DEVICE"){
        let deadline=Instant::now()+Duration::from_secs(25);
        let device=loop{
            app.widget_metrics.poll();
            if let Some(device)=app.widget_metrics.value.wireless_devices.iter().find(|d|d.id==id&&d.connected&&d.battery_percent.is_some()){break device.clone()}
            if Instant::now()>=deadline{return Err("live device battery unavailable".into())}
            std::thread::sleep(Duration::from_millis(100));
        };
        for mini in [false,true]{
            app.settings["IslandSizeMode"]=json!(if mini{"Mini"}else{"Classic"});
            app.alerts=Default::default();app.device_watch=Default::default();
            app.device_watch.update(std::slice::from_ref(&device),1,true);
            // Replay only the connection event; the level and identity are live.
            let event=devices::Event::new(device.container_id.clone().unwrap_or(device.id.clone()),devices::Kind::Headphones,true,device.name.clone());
            app.enqueue_device(&event);
            let base=Rect::new(150.,24.,if mini{100.}else{260.},if mini{18.}else{40.});
            app.alerts.begin(base,base.h/2.,app.layout.host,24.,0.,false);
            let alert=app.alerts.active.as_ref().ok_or("live alert missing")?;
            let expected=format!("{}%",device.battery_percent.unwrap());
            if alert.trailing!=expected||!alert.title.contains(&device.name){return Err("live device association mismatch".into())}
            let mut renderer=Renderer::new(app.visual_hwnd,(app.layout.host.w*1.5)as u32,(app.layout.host.h*1.5)as u32,144.,&app.base.join("Assets/Fonts/NotoSansSC.ttf"))?;
            let mut scene=app.scene();scene.layout_scale=1.5;
            scene.alert_rect=Rect::new((app.layout.host.w-400.)/2.,24.,400.,48.);scene.alert_radius=24.;scene.alert_mix=1.;
            renderer.render(&scene,false)?;
            let file=format!("device-live-{}-144.png",if mini{"mini"}else{"classic"});
            renderer.snapshot(&directory.join(&file))?;
            reports.push(json!({"file":file,"device":device,"battery":expected,"synthetic":false,"connectionEventReplayed":true}));
        }
    }
    std::fs::write(directory.join("report.json"),serde_json::to_vec_pretty(&json!({"success":true,"cases":reports}))?)?;
    Ok(())
}
