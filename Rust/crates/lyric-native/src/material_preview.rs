//! Deterministic screenshots of the shared painter, including every system-hint state.
use std::path::Path;
use serde_json::{json,Value};
use crate::{App,renderer::{Renderer,Material},layout::Rect,motion::Frame,osd::{Hint,Kind},surface::Surface};

unsafe fn save(r:&Renderer,dir:&Path,name:String,shape:Rect,rows:&mut Vec<Value>)->Result<(),Box<dyn std::error::Error>>{
    r.snapshot(&dir.join(&name))?;
    let scale=r.dpi as f64/96.;
    rows.push(json!({"file":name,"sample":[((shape.x+10.)*scale).round()as u32,((shape.y+shape.h/2.)*scale).round()as u32],"bounds":shape}));Ok(())
}
pub unsafe fn surfaces(a:&mut App,dir:&Path)->Result<(),Box<dyn std::error::Error>>{
    std::fs::create_dir_all(dir)?;a.opacity=1.;a.lyric="晚风轻轻，音乐刚好".into();
    let mut r=a.renderer.take().unwrap();r.set_cover(Some(&std::fs::read(a.base.join("Assets/DemoCover.png"))?))?;
    let palette=r.preview_palette();let mut rows=vec![];
    for mini in [false,true]{
        a.settings["IslandSizeMode"]=json!(if mini{"Mini"}else{"Classic"});
        a.settings["MiniMaterial"]=json!("Acrylic");a.settings["ClassicCapsuleMaterial"]=json!("Acrylic");
        a.layout.card.w=if mini{420.}else{400.};a.layout.card.h=if mini{120.}else{220.};a.layout.card.x=(a.layout.host.w-a.layout.card.w)/2.;
        for (label,material) in [("Acrylic",Material::Acrylic),("LightField",Material::LightField)]{
            a.settings["IslandMaterial"]=json!(label);a.settings["MiniExpandedMaterial"]=json!(label);a.frame=Frame::expanded();
            let prefix=format!("{}-{label}",if mini{"mini"}else{"classic"});
            let mut scene=a.scene();scene.lyric_age_ms=1000.;r.render(&scene,false)?;save(&r,dir,format!("{prefix}-card.png"),a.layout.card,&mut rows)?;
            a.frame=Frame::hidden(a.options());
            let notice=crate::notifications::Notice::example(&a.base);r.set_notification_icon(notice.icon.as_deref())?;
            let notification=Rect::new((a.layout.host.w-440.)/2.,42.,440.,90.);
            let mut scene=a.scene();scene.notification=Some(&notice);scene.notification_rect=notification;scene.notification_radius=24.;scene.notification_mix=1.;
            r.render(&scene,false)?;save(&r,dir,format!("{prefix}-notification.png"),notification,&mut rows)?;
            for kind in [Kind::Caps,Kind::Num,Kind::Mic,Kind::Touchpad]{for on in [false,true]{
                let notice=Hint{kind,on}.notice();let hint=Rect::new((a.layout.host.w-240.)/2.,42.,240.,44.);
                let mut scene=a.scene();scene.osd=Some(&notice);scene.osd_rect=hint;scene.osd_radius=22.;scene.osd_mix=1.;
                r.render(&scene,false)?;save(&r,dir,format!("{prefix}-island-{}-{on}.png",kind.key()),hint,&mut rows)?;
                r.render_bottom_osd(&notice,1.,0.,Surface::expanded(material,true),false)?;
                save(&r,dir,format!("{prefix}-bottom-{}-{on}.png",kind.key()),Rect::new(16.,12.,240.,44.),&mut rows)?;
            }}
        }
    }
    std::fs::write(dir.join("surfaces.json"),serde_json::to_vec_pretty(&json!({"background":format!("#{:06X}",palette.0),"images":rows}))?)?;Ok(())
}

pub unsafe fn cover_radii(a:&mut App,dir:&Path)->Result<(),Box<dyn std::error::Error>>{
    std::fs::create_dir_all(dir)?;a.opacity=1.;a.frame=Frame::expanded();a.lyric="晚风轻轻，音乐刚好".into();
    let mut r=a.renderer.take().unwrap();let cover=std::fs::read(a.base.join("Assets/DemoCover.png"))?;
    r.set_cover(Some(&cover))?;std::thread::sleep(std::time::Duration::from_millis(300));r.set_cover(Some(&cover))?;let mut rows=vec![];
    for mini in [false,true]{for radius in [8.,16.,30.,48.]{
        a.settings["IslandSizeMode"]=json!(if mini{"Mini"}else{"Classic"});a.settings["IslandMaterial"]=json!("Glass");a.settings["MiniExpandedMaterial"]=json!("Glass");a.settings["Motion"]["Radius"]=json!(radius);
        a.layout.card.w=if mini{420.}else{400.};a.layout.card.x=(a.layout.host.w-a.layout.card.w)/2.;
        for layout in 0..if mini{1}else{4}{
            a.music_mix=layout as f64;a.layout.card.h=if mini{120.}else{crate::layout::music_height(layout as f64,12.5)};
            let cover=if mini{Rect::new(20.,8.,40.,40.)}else{crate::layout::music_geometry(400.,crate::layout::MusicLayout::SideBySide,layout as f64,r.dpi as f64/96.,12.5).cover};
            for age in [0.,70.,280.]{
                let file=format!("{}-layout-{layout}-radius-{radius:.0}-age-{age:.0}.png",if mini{"mini"}else{"classic"});
                let mut scene=a.scene();scene.cover_age_ms=age;scene.lyric_age_ms=1000.;r.render(&scene,false)?;r.snapshot(&dir.join(&file))?;
                let scale=r.dpi as f64/96.;rows.push(json!({"file":file,"mini":mini,"layout":layout,"radius":radius,"coverRadius":crate::layout::cover_corner_radius(radius,cover.w,cover.h),"age":age,"cover":[(a.layout.card.x+cover.x)*scale,(a.layout.card.y+cover.y)*scale,cover.w*scale,cover.h*scale],"scale":scale}));
            }
        }
    }}std::fs::write(dir.join("cover-radii.json"),serde_json::to_vec_pretty(&json!({"images":rows}))?)?;Ok(())
}

/// 导出 9 张 曜光视效（Lumen Glass）+ 3 套重制桌面展开布局 144 DPI GPU 真机截图
pub unsafe fn lightfield(a:&mut App,dir:&Path)->Result<(),Box<dyn std::error::Error>>{
    std::fs::create_dir_all(dir)?;
    a.opacity=1.;
    a.lyric="晚风轻轻掠过玻璃海面".into();
    a.settings["IslandMaterial"]=json!("LightField");
    a.settings["ClassicCapsuleMaterial"]=json!("LightField");
    a.settings["MiniMaterial"]=json!("LightField");
    a.settings["MiniExpandedMaterial"]=json!("LightField");
    a.settings["MusicExpandedLayout"]=json!("SideBySide");
    a.settings["ExpandedCoverOverlay"]=json!(false);
    a.settings["ExpandedMonetEnabled"]=json!(false);
    a.settings["CoverBackground"]=json!(false);
    a.settings["LyricColorFromCover"]=json!(false);
    a.settings["ExpandedTextColor"]=json!("#FFFFFF");
    a.settings["ExpandedComponentColor"]=json!("#FFFFFF");
    a.settings["LyricProgressColor"]=json!("#DDF6FF");
    a.music_mix=0.0;
    a.music_controls.snap(1.);
    a.snapshot=json!({
        "isPlaying":true,
        "track":{
            "title":"光屿漫步 · Prism",
            "artist":"DotIsle Acoustic Lab",
            "durationSeconds":248.0,
            "sourceApp":"Netease"
        }
    });
    let mut r=a.renderer.take().unwrap();
    let cover=std::fs::read(a.base.join("Assets/DemoCover.png"))?;
    r.set_cover(Some(&cover))?;
    let mut files=vec![];

    fn configure_scene(a:&App,layout:crate::layout::MusicLayout,mix:f64,time_ms:f64,cover_age:f64,progress:f64)->crate::renderer::Scene<'_>{
        let mut s=a.scene();
        s.has_track=true;
        s.playing=true;
        s.favorite_enabled=true;
        s.title="光屿漫步 · Prism";
        s.artist="DotIsle Acoustic Lab";
        s.lyric="晚风轻轻掠过玻璃海面";
        s.secondary="Where glass meets fluid aurora light";
        s.highlight=0.46;
        s.progress=progress;
        s.elapsed=248.0*progress;
        s.duration=248.0;
        s.time_ms=time_ms;
        s.cover_age_ms=cover_age;
        s.lyric_age_ms=1000.;
        s.animate=true;
        s.music_layout=layout;
        s.music_mix=mix;
        s.activity_scales=[0.62,1.0,0.78];
        s.activity_opacity=1.0;
        s.controls_opacity=1.0;
        s
    }

    // 1. 经典收起态黑曜通透胶囊 + 4 柱纯净冰白声波
    a.settings["IslandSizeMode"]=json!("Classic");
    a.frame=Frame::hidden(a.options());
    let s1=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,600.,1000.,0.42);
    r.render(&s1,false)?;
    r.snapshot(&dir.join("01-capsule-lightfield.png"))?;
    files.push("01-capsule-lightfield.png");

    a.layout.card.w=420.;
    a.layout.card.x=(a.layout.host.w-a.layout.card.w)/2.;
    a.frame=Frame::expanded();

    // 2. 重制布局 1：「光屿画廊 (SideBySide)」+ 常态「追光」锥形彗尾进度条
    a.layout.card.h=crate::layout::music_height(0.0,12.5);
    let s2=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,1200.,1000.,0.42);
    r.render(&s2,false)?;
    r.snapshot(&dir.join("02-layout1-prism-stage.png"))?;
    files.push("02-layout1-prism-stage.png");

    // 3. 重制布局 2：「流光分栏 (Separated)」电影顶眉 + 全宽歌词剧场
    a.layout.card.h=crate::layout::music_height(1.0,12.5);
    let s3=configure_scene(a,crate::layout::MusicLayout::Separated,1.0,1200.,1000.,0.42);
    r.render(&s3,false)?;
    r.snapshot(&dir.join("03-layout2-cinema-split.png"))?;
    files.push("03-layout2-cinema-split.png");

    // 4. 重制布局 3：「星幕歌词 (LyricFocus)」极简中轴居中大歌词舞台
    a.layout.card.h=crate::layout::music_height(2.0,12.5);
    let s4=configure_scene(a,crate::layout::MusicLayout::LyricFocus,2.0,1200.,1000.,0.42);
    r.render(&s4,false)?;
    r.snapshot(&dir.join("04-layout3-lyric-stage.png"))?;
    files.push("04-layout3-lyric-stage.png");

    // 5. 实拍图二同款：滑动进度条「凝光液态玻璃圆镜 (Condensing Lens)」
    a.layout.card.h=crate::layout::music_height(0.0,12.5);
    let mut s5=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,2200.,1000.,0.54);
    s5.hover=10;
    s5.pressed=10;
    r.render(&s5,false)?;
    r.snapshot(&dir.join("05-anim-scrub-lens.png"))?;
    files.push("05-anim-scrub-lens.png");

    // 6. 切歌/切换封面瞬间：封面外缘纯白凝光脉冲 + 向全卡扩散的「凝光环」波纹
    let s6=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,1800.,240.,0.42);
    r.render(&s6,false)?;
    r.snapshot(&dir.join("06-anim-cover-ripple.png"))?;
    files.push("06-anim-cover-ripple.png");

    // 7. 中央通透聚光圆钮按压态凝光聚点 + 扩散光环
    let mut s7=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,2600.,1000.,0.50);
    s7.hover=4;
    s7.pressed=4;
    s7.feedback[4]=0.14;
    r.render(&s7,false)?;
    r.snapshot(&dir.join("07-anim-button-condense.png"))?;
    files.push("07-anim-button-condense.png");

    // 8. 迷你模式收起态细条
    a.settings["IslandSizeMode"]=json!("Mini");
    a.frame=Frame::hidden(a.options());
    let s8=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,1400.,1000.,0.35);
    r.render(&s8,false)?;
    r.snapshot(&dir.join("08-mini-strip-lightfield.png"))?;
    files.push("08-mini-strip-lightfield.png");

    // 9. 通知弹窗态凝光玻璃
    a.settings["IslandSizeMode"]=json!("Classic");
    a.frame=Frame::hidden(a.options());
    let notice=crate::notifications::Notice::example(&a.base);
    r.set_notification_icon(notice.icon.as_deref())?;
    let notification=Rect::new((a.layout.host.w-440.)/2.,42.,440.,90.);
    let mut s9=configure_scene(a,crate::layout::MusicLayout::SideBySide,0.0,1500.,1000.,0.42);
    s9.notification=Some(&notice);
    s9.notification_rect=notification;
    s9.notification_radius=24.;
    s9.notification_mix=1.;
    r.render(&s9,false)?;
    r.snapshot(&dir.join("09-notification-lightfield.png"))?;
    files.push("09-notification-lightfield.png");

    a.renderer=Some(r);
    std::fs::write(dir.join("lightfield.json"),serde_json::to_vec_pretty(&json!({"images":files}))?)?;
    Ok(())
}

/// 导出 5.2.0 全面升级实机 144 DPI GPU 验收截图（通知居中、氛围色场亚克力、专辑取色、重制番茄钟、滚动待办、任务栏模式）
pub unsafe fn upgrade(a:&mut App,dir:&Path)->Result<(),Box<dyn std::error::Error>>{
    std::fs::create_dir_all(dir)?;
    a.opacity=1.;
    a.lyric="晚风轻轻掠过玻璃海面".into();
    a.settings["IslandSizeMode"]=json!("Classic");
    a.settings["MusicExpandedLayout"]=json!("SideBySide");
    a.settings["ExpandedMonetEnabled"]=json!(true);
    a.settings["LyricColorFromCover"]=json!(true);
    a.settings["IslandColorSource"]=json!("Content");
    a.settings["IslandOpacity"]=json!(100.);
    a.settings["PopupMaterial"]=json!("Acrylic");
    a.music_mix=0.0;
    a.music_controls.snap(1.);
    a.snapshot=json!({"isPlaying":true,"track":{"title":"光屿漫步 · Prism","artist":"DotIsle Acoustic Lab","durationSeconds":248.0,"sourceApp":"Netease"}});
    let mut r=a.renderer.take().unwrap();
    let cover=std::fs::read(a.base.join("Assets/DemoCover.png"))?;
    r.set_cover(Some(&cover))?;
    let mut files=vec![];

    let notice=crate::notifications::Notice::example(&a.base);
    r.set_notification_icon(notice.icon.as_deref())?;
    let make_color_cover=|r_c:u8,g_c:u8,b_c:u8|->Vec<u8>{
        let mut img=image::RgbaImage::new(64,64);
        for(x,y,px)in img.enumerate_pixels_mut(){
            let f=(x+y)as f32/128.;
            *px=image::Rgba([
                ((r_c as f32)*(0.75+0.25*f)).min(255.) as u8,
                ((g_c as f32)*(0.75+0.25*(1.-f))).min(255.) as u8,
                b_c,
                255,
            ]);
        }
        let mut buf=Vec::new();
        let _=img.write_to(&mut std::io::Cursor::new(&mut buf),image::ImageFormat::Png);
        buf
    };

    // 1. 默认亚克力弹窗 + 垂直居中通知
    a.frame=Frame::hidden(a.options());
    a.settings["PopupMaterial"]=json!("Acrylic");
    let n_rect=Rect::new((a.layout.host.w-440.)/2.,42.,440.,84.);
    let mut s1=a.scene();
    s1.notification=Some(&notice);s1.notification_rect=n_rect;s1.notification_radius=24.;s1.notification_mix=1.;
    r.render(&s1,false)?;r.snapshot(&dir.join("01-notice-acrylic-centered.png"))?;files.push("01-notice-acrylic-centered.png");

    // 2. 曜光玻璃弹窗 + 垂直居中通知
    a.settings["PopupMaterial"]=json!("LightField");
    let mut s2=a.scene();
    s2.notification=Some(&notice);s2.notification_rect=n_rect;s2.notification_radius=24.;s2.notification_mix=1.;
    r.render(&s2,false)?;r.snapshot(&dir.join("02-notice-lumen-centered.png"))?;files.push("02-notice-lumen-centered.png");

    // 3. 亚克力系统状态 OSD 垂直居中
    a.settings["PopupMaterial"]=json!("Acrylic");
    let osd_notice=crate::notifications::Notice{app_id:"LyricIsland.System.Caps".into(),app_name:"系统".into(),title:"大写锁定".into(),body:"已开启".into(),icon:None};
    let mut s3=a.scene();
    s3.osd=Some(&osd_notice);s3.osd_rect=Rect::new((a.layout.host.w-220.)/2.,24.,220.,44.);s3.osd_radius=22.;s3.osd_mix=1.;
    r.render(&s3,false)?;r.snapshot(&dir.join("03-osd-acrylic-centered.png"))?;files.push("03-osd-acrylic-centered.png");

    // 先无动画落定动态取色状态，再开启光效重绘，避免连拍切歌瞬间采样停留在上一首渐变途中
    let render_settled=|r:&mut Renderer,s:&mut crate::renderer::Scene<'_>|->Result<(),Box<dyn std::error::Error>>{
        let was=s.animate;
        s.animate=false;
        r.render(s,false)?;
        if was{
            s.animate=true;
            r.render(s,false)?;
        }
        Ok(())
    };

    // 4 & 5. 高级氛围色场亚克力 + 专辑取色对比（炽烈红 vs 蔚海蓝）
    a.layout.card.w=420.;a.layout.card.x=(a.layout.host.w-a.layout.card.w)/2.;
    a.layout.card.h=crate::layout::music_height(0.0,12.5);
    a.frame=Frame::expanded();
    a.settings["IslandMaterial"]=json!("Acrylic");
    a.settings["ExpandedCoverOverlay"]=json!(true);
    a.settings["AcrylicCardFrost"]=json!(50.);
    a.settings["AcrylicCardOpacity"]=json!(75.);
    let red_art=make_color_cover(226,48,58);
    r.set_cover(Some(&red_art))?;
    let mut s4=a.scene();
    s4.has_track=true;s4.playing=true;s4.favorite_enabled=true;
    s4.title="绯红落日 · Sunset";s4.artist="DotIsle Lab";s4.lyric="暮色把天空染成温柔珊瑚";s4.secondary="Sunset glows beneath frosted glass";
    s4.highlight=0.55;s4.progress=0.45;s4.elapsed=111.;s4.duration=248.;s4.controls_opacity=1.;s4.activity_scales=[0.6,0.95,0.7];s4.activity_opacity=1.;
    render_settled(&mut r,&mut s4)?;r.snapshot(&dir.join("04-acrylic-cover-red.png"))?;files.push("04-acrylic-cover-red.png");

    let blue_art=make_color_cover(34,116,236);
    r.set_cover(Some(&blue_art))?;
    let mut s5=a.scene();
    s5.has_track=true;s5.playing=true;s5.favorite_enabled=true;
    s5.title="深海星夜 · Ocean";s5.artist="DotIsle Lab";s5.lyric="静谧海风吹亮午夜星光";s5.secondary="Starlight drifting across deep blue";
    s5.highlight=0.62;s5.progress=0.52;s5.elapsed=128.;s5.duration=248.;s5.controls_opacity=1.;s5.activity_scales=[0.7,0.9,0.55];s5.activity_opacity=1.;
    render_settled(&mut r,&mut s5)?;r.snapshot(&dir.join("05-acrylic-cover-blue.png"))?;files.push("05-acrylic-cover-blue.png");

    // 6. 曜光玻璃开启专辑取色与封面磨砂叠层：兼容通透曜光纹理、封面氛围色场与晶莹底光
    a.settings["IslandMaterial"]=json!("LightField");
    a.settings["ExpandedCoverOverlay"]=json!(true);
    r.set_cover(Some(&red_art))?;
    let mut s6=a.scene();
    s6.has_track=true;s6.playing=true;s6.favorite_enabled=true;
    s6.title="绯红曜光 · Lumen";s6.artist="DotIsle Lab";s6.lyric="曜光之下点亮专辑流光";s6.secondary="Clean dark velvet glass with album accent";
    s6.highlight=0.58;s6.progress=0.48;s6.elapsed=119.;s6.duration=248.;s6.controls_opacity=1.;s6.activity_scales=[0.8,0.6,0.95];s6.activity_opacity=1.;
    render_settled(&mut r,&mut s6)?;r.snapshot(&dir.join("06-lumen-album-accent.png"))?;files.push("06-lumen-album-accent.png");
    a.settings["ExpandedCoverOverlay"]=json!(false);

    // 7. 灵动岛半透明设置效果（65%）
    r.set_cover(Some(&cover))?;
    a.settings["IslandOpacity"]=json!(65.);
    let mut s7=a.scene();
    s7.has_track=true;s7.playing=true;s7.favorite_enabled=true;
    s7.title="轻透微醺 · 65%";s7.artist="DotIsle Lab";s7.lyric="通透度独立随心调节";s7.secondary="Custom background translucency";
    s7.highlight=0.45;s7.progress=0.40;s7.elapsed=98.;s7.duration=248.;s7.controls_opacity=1.;s7.activity_scales=[0.6,1.0,0.75];s7.activity_opacity=1.;
    render_settled(&mut r,&mut s7)?;r.snapshot(&dir.join("07-island-opacity-65.png"))?;files.push("07-island-opacity-65.png");
    a.settings["IslandOpacity"]=json!(100.);

    // 8. 重制番茄钟极简曜光布局
    a.layout.card.h=220.;
    let timer_panel=crate::activities::Panel{
        kind:1,title:"深度专注 · 番茄钟".into(),value:"24:38".into(),subtitle:"第 2 / 4 轮 · 休息 5 分钟".into(),progress:0.42,running:true,
        timer:Some(crate::activities::TimerPresentation{label:"专注".into(),elapsed:"00:22".into(),footnote:"".into(),primary:"暂停".into(),expanded_value:"24:38".into(),indefinite:false,elapsed_ms:22_000}),
        ..Default::default()
    };
    let mut s8=a.scene();s8.panel=Some(&timer_panel);s8.controls_opacity=1.;
    render_settled(&mut r,&mut s8)?;r.snapshot(&dir.join("08-timer-pomodoro-redesign.png"))?;files.push("08-timer-pomodoro-redesign.png");

    // 9. 重制待办组件（去盒化无框行 + 滚至中段 + 悬浮滚动条）
    let todo_panel=crate::activities::Panel{
        kind:2,title:"待办清单".into(),value:"6 项待办".into(),subtitle:"支持滚轮滚动查看全部".into(),
        tasks:(0..6).map(|i|crate::activities::Task{
            id:format!("t-{i}"),title:format!("任务 {} · 打磨点屿 5.2.0 光学界面",i+1),detail:"今天 18:30 · 每天".into(),
            color:[0xFF57BFFF,0xFF9A8BEB,0xFFFF9EAD,0xFFEAB36A,0xFF6DD5B0,0xFFA7B8FF][i],
        }).collect(),
        ..Default::default()
    };
    a.todo_scroll=64.;
    let mut s9=a.scene();s9.panel=Some(&todo_panel);s9.controls_opacity=1.;s9.hover=1002;
    render_settled(&mut r,&mut s9)?;r.snapshot(&dir.join("09-todo-scrollable-redesign.png"))?;files.push("09-todo-scrollable-redesign.png");
    a.todo_scroll=0.;

    // 10, 10b, 10c & 11. 高颜值任务栏歌词条（透明融入实时逐字歌词 / 悬停切换控制钮 / 常驻时钟系统状态）+ 向上弹出复用大卡片
    a.layout.card.h=crate::layout::music_height(0.0,12.5);
    a.layout.set_compact(340.,38.);
    a.layout.configure_anchor(true,48.);
    a.layout.align_dock(190.,230.);
    r.resize((a.layout.host.w*a.scene().layout_scale).ceil() as u32,(a.layout.host.h*a.scene().layout_scale).ceil() as u32,r.dpi);

    a.frame=Frame::hidden(a.options());
    let mut s10=a.scene();
    s10.capsule_radius=8.0;
    s10.taskbar_transparent=true;s10.taskbar_light=false;s10.taskbar_hovered=false;
    s10.hover=-1;s10.pressed=0;
    s10.has_track=true;s10.playing=true;s10.title="光屿漫步 · Prism";s10.artist="DotIsle Acoustic Lab";
    s10.lyric="晚风轻轻掠过玻璃海面";s10.compact_secondary="Gentle breeze across the glass sea";
    s10.highlight=0.52;s10.progress=0.42;s10.activity_scales=[0.6,1.0,0.8];s10.activity_opacity=1.;
    render_settled(&mut r,&mut s10)?;r.snapshot(&dir.join("10-taskbar-widget-collapsed.png"))?;files.push("10-taskbar-widget-collapsed.png");

    let mut s10b=s10;
    s10b.taskbar_hovered=true;s10b.hover=4;
    render_settled(&mut r,&mut s10b)?;r.snapshot(&dir.join("10b-taskbar-hover-controls.png"))?;files.push("10b-taskbar-hover-controls.png");

    let idle_res=crate::resident::View{
        page:crate::resident::Page::Time,
        label:"16:08".into(),
        clock:"16:08".into(),
        cpu:"12%".into(),
        memory:"42%".into(),
        ..Default::default()
    };
    let idle_panel=crate::activities::Panel{kind:crate::resident::KIND,..Default::default()};
    let mut s10c=a.scene();
    s10c.capsule_radius=8.0;
    s10c.taskbar_transparent=false;s10c.taskbar_light=false;s10c.taskbar_hovered=false;
    s10c.hover=-1;s10c.pressed=0;
    s10c.has_track=false;s10c.panel=Some(&idle_panel);s10c.resident=&idle_res;
    render_settled(&mut r,&mut s10c)?;r.snapshot(&dir.join("10c-taskbar-resident-idle.png"))?;files.push("10c-taskbar-resident-idle.png");

    a.frame=Frame::expanded();
    let mut s11=a.scene();
    s11.capsule_radius=8.0;
    s11.has_track=true;s11.playing=true;s11.favorite_enabled=true;s11.title="光屿漫步 · Prism";s11.artist="DotIsle Acoustic Lab";
    s11.lyric="晚风轻轻掠过玻璃海面";s11.secondary="自任务栏歌词条向上弹出并复用完整大卡片";
    s11.highlight=0.48;s11.progress=0.42;s11.controls_opacity=1.;s11.activity_scales=[0.6,1.0,0.8];s11.activity_opacity=1.;
    render_settled(&mut r,&mut s11)?;r.snapshot(&dir.join("11-taskbar-flyout-expanded.png"))?;files.push("11-taskbar-flyout-expanded.png");

    // 12. 任务栏模式下通知依然照常在屏幕正上方居中弹出（默认亚克力）
    let scale=r.dpi as f64/96.;
    r.resize((488.*scale).ceil() as u32,(146.*scale).ceil() as u32,r.dpi);
    r.render_top_popup(
        Some(&notice),
        None,
        None,
        Rect::new(24.,24.,440.,84.),
        24.,
        1.,
        crate::surface::Surface::expanded(crate::renderer::Material::Acrylic,false),
        false,
    )?;
    r.snapshot(&dir.join("12-taskbar-mode-top-notice.png"))?;files.push("12-taskbar-mode-top-notice.png");

    // 13 & 14. 全新优化的岛周围三大光效（音乐极光呼吸、更新棱镜流光；设置中默认关闭）
    a.layout=crate::layout::Layout::new(260.);
    a.layout.configure_anchor(false,48.);
    let g_scale=r.dpi as f64/96.;
    r.resize((a.layout.host.w*g_scale).ceil() as u32,(a.layout.host.h*g_scale).ceil() as u32,r.dpi);
    a.frame=Frame::hidden(a.options());
    a.settings["IslandMaterial"]=json!("LightField");
    r.set_cover(Some(&cover))?;
    let mut s13=a.scene();
    s13.has_track=true;s13.playing=true;s13.lyric="晚风轻轻掠过玻璃海面";s13.activity_scales=[0.7,1.0,0.6];s13.activity_opacity=1.;
    s13.glow=crate::island_glow::Frame{kind:crate::island_glow::Kind::MusicBreath,strength:0.92,phase:1.1};
    render_settled(&mut r,&mut s13)?;r.snapshot(&dir.join("13-glow-music-aurora.png"))?;files.push("13-glow-music-aurora.png");

    let mut s14=s13;
    s14.glow=crate::island_glow::Frame{kind:crate::island_glow::Kind::Update,strength:0.95,phase:2.4};
    render_settled(&mut r,&mut s14)?;r.snapshot(&dir.join("14-glow-update-prism.png"))?;files.push("14-glow-update-prism.png");

    a.renderer=Some(r);
    std::fs::write(dir.join("upgrade.json"),serde_json::to_vec_pretty(&json!({"images":files}))?)?;
    Ok(())
}

/// 导出 10 张 液态玻璃（Liquid Glass）+ 动态取色系统 144 DPI GPU 实拍验收截图
pub unsafe fn acrylic_cover(a:&mut App,dir:&Path)->Result<(),Box<dyn std::error::Error>>{
    std::fs::create_dir_all(dir)?;
    a.opacity=1.;
    a.lyric="晚风轻轻掠过亚克力封面".into();
    a.settings["IslandSizeMode"]=json!("Classic");
    a.settings["MusicExpandedLayout"]=json!("SideBySide");
    a.settings["ClassicCapsuleMaterial"]=json!("Acrylic");
    a.settings["IslandMaterial"]=json!("Acrylic");
    a.settings["ExpandedCoverOverlay"]=json!(true);
    a.settings["ExpandedMonetEnabled"]=json!(true);
    a.settings["CoverBackground"]=json!(false);
    a.settings["LyricColorFromCover"]=json!(true);
    a.settings["IslandOpacity"]=json!(100.);
    a.music_mix=0.0;
    a.music_controls.snap(1.);
    a.snapshot=json!({"isPlaying":true,"track":{"title":"落日潮汐 · Acrylic Cover","artist":"DotIsle Acoustic Lab","durationSeconds":248.0,"sourceApp":"Netease"}});
    let mut r=a.renderer.take().unwrap();
    let cover=std::fs::read(a.base.join("Assets/DemoCover.png"))?;
    r.set_cover(Some(&cover))?;
    let mut files=vec![];

    let render_settled=|r:&mut Renderer,s:&mut crate::renderer::Scene<'_>|->Result<(),Box<dyn std::error::Error>>{
        let was=s.animate;
        s.animate=false;
        r.render(s,false)?;
        if was{
            s.animate=true;
            r.render(s,false)?;
        }
        Ok(())
    };
    fn make_scene(app:&App)->crate::renderer::Scene<'_>{
        let mut s=app.scene();
        s.has_track=true;
        s.playing=true;
        s.favorite_enabled=true;
        s.title="落日潮汐 · Acrylic Cover";
        s.artist="DotIsle Acoustic Lab";
        s.lyric="晚风轻轻掠过亚克力封面";
        s.secondary="Crisp to frosted album art under Acrylic";
        s.highlight=0.52;
        s.progress=0.44;
        s.elapsed=109.;
        s.duration=248.;
        s.time_ms=1200.;
        s.cover_age_ms=1000.;
        s.lyric_age_ms=1000.;
        s.animate=true;
        s.controls_opacity=1.;
        s.activity_scales=[0.65,1.0,0.78];
        s.activity_opacity=1.;
        s
    }

    a.layout.card.w=420.;
    a.layout.card.x=(a.layout.host.w-a.layout.card.w)/2.;
    a.layout.card.h=crate::layout::music_height(0.0,12.5);
    a.frame=Frame::expanded();

    for (file, frost, opacity, label) in [
        ("01-acrylic-card-frost-0.png", 0., 70., "0% 磨砂 · 清晰封面背景"),
        ("02-acrylic-card-frost-40.png", 40., 65., "40% 磨砂 · 柔化质感"),
        ("03-acrylic-card-frost-80.png", 80., 75., "80% 磨砂 · 浓醇氛围色场"),
    ] {
        a.settings["AcrylicCardFrost"]=json!(frost);
        a.settings["AcrylicCardOpacity"]=json!(opacity);
        let mut s=make_scene(a);
        s.title=label;
        render_settled(&mut r,&mut s)?;
        r.snapshot(&dir.join(file))?;
        files.push(file);
    }

    a.settings["CoverBackground"]=json!(true);
    a.settings["AcrylicCardFrost"]=json!(95.);
    a.settings["AcrylicCapsuleFrost"]=json!(25.);
    a.settings["AcrylicCapsuleOpacity"]=json!(70.);
    a.frame=Frame::hidden(a.options());
    let mut s4=make_scene(a);
    render_settled(&mut r,&mut s4)?;
    r.snapshot(&dir.join("04-acrylic-capsule-cover.png"))?;
    files.push("04-acrylic-capsule-cover.png");

    a.renderer=Some(r);
    std::fs::write(dir.join("acrylic-cover.json"),serde_json::to_vec_pretty(&json!({"images":files}))?)?;
    Ok(())
}

/// 生成模拟 WinWidget 同款发光点阵背景下的灵动岛「液态玻璃 Beta」GPU 渲染验收图（胶囊态 + 展开音乐大卡片态）。
pub unsafe fn liquid_glass_beta(a: &mut App, dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    a.opacity = 1.;
    a.lyric = "晚风穿过玻璃边缘，折射出细腻霓虹".into();
    a.settings["IslandSizeMode"] = json!("Classic");
    a.settings["MusicExpandedLayout"] = json!("SideBySide");
    a.settings["ClassicCapsuleMaterial"] = json!("LiquidGlass");
    a.settings["IslandMaterial"] = json!("LiquidGlass");
    a.settings["LiquidGlassRefraction"] = json!(76.);
    a.settings["LiquidGlassDispersion"] = json!(46.);
    a.settings["LiquidGlassBlur"] = json!(8.);
    a.settings["LiquidGlassTint"] = json!(6.);
    a.settings["ExpandedMonetEnabled"] = json!(false);
    a.settings["ExpandedCoverOverlay"] = json!(false);
    a.settings["CoverBackground"] = json!(false);
    a.settings["LyricColorFromCover"] = json!(false);
    a.settings["ExpandedTextColor"] = json!("#FFFFFF");
    a.settings["ExpandedComponentColor"] = json!("#FFFFFF");
    a.settings["LyricProgressColor"] = json!("#DDF6FF");
    a.music_mix = 0.0;
    a.music_controls.snap(1.);
    a.snapshot = json!({
        "isPlaying": true,
        "track": {
            "title": "DIA VERAO · Liquid Glass Beta",
            "artist": "DotIsle Optical Lab",
            "durationSeconds": 248.0,
            "sourceApp": "Netease"
        }
    });

    let mut r = a.renderer.take().unwrap();
    let cover = std::fs::read(a.base.join("Assets/DemoCover.png"))?;
    r.set_cover(Some(&cover))?;

    // 优先读取真实 Windows 桌面壁纸并保留局部色彩线条，若未找到则生成高彩渐变极光桌面底图
    let bw = r.width.max(1);
    let bh = r.height.max(1);
    let mut bg = vec![0u8; (bw as usize) * (bh as usize) * 4];
    let loaded_wp = std::env::var_os("APPDATA")
        .map(|a| {
            std::path::PathBuf::from(a)
                .join("Microsoft")
                .join("Windows")
                .join("Themes")
                .join("TranscodedWallpaper")
        })
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|img| {
            let sw = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows::Win32::UI::WindowsAndMessaging::SM_CXSCREEN).max(bw as i32) as u32 };
            let sh = unsafe { windows::Win32::UI::WindowsAndMessaging::GetSystemMetrics(windows::Win32::UI::WindowsAndMessaging::SM_CYSCREEN).max(bh as i32) as u32 };
            let resized = img.resize_to_fill(sw, sh, image::imageops::FilterType::Triangle).to_rgba8();
            let x0 = ((sw - bw) / 2).min(sw.saturating_sub(bw));
            let y0 = 0u32;
            image::imageops::crop_imm(&resized, x0, y0, bw, bh).to_image().into_raw()
        });
    if let Some(wp) = loaded_wp {
        bg = wp;
    } else {
        for y in 0..bh {
            for x in 0..bw {
                let u = x as f32 / bw as f32;
                let v = y as f32 / bh as f32;
                let wave = ((u * 9.0 + v * 5.0).sin() * 0.5 + 0.5) * 0.45;
                let idx = ((y * bw + x) * 4) as usize;
                bg[idx] = (40.0 + 185.0 * u * (1.0 - 0.3 * v) + 40.0 * wave).clamp(0.0, 255.0) as u8;
                bg[idx + 1] = (65.0 + 110.0 * (1.0 - u) + 85.0 * wave).clamp(0.0, 255.0) as u8;
                bg[idx + 2] = (120.0 + 125.0 * v + 30.0 * (1.0 - wave)).clamp(0.0, 255.0) as u8;
                bg[idx + 3] = 255;
            }
        }
    }
    r.update_liquid_glass_backdrop(&bg, bw, bh);

    // 辅助函数：把前景透明窗口直接与真实桌面背景叠加，输出完整同框对比图
    let save_desktop_composite = |fg_path: &Path, dst_path: &Path| -> Result<(), Box<dyn std::error::Error>> {
        let fg_img = image::open(fg_path)?.to_rgba8();
        let mut comp = image::RgbaImage::from_raw(bw, bh, bg.clone()).unwrap();
        for (x, y, px) in comp.enumerate_pixels_mut() {
            if x < fg_img.width() && y < fg_img.height() {
                let f = fg_img.get_pixel(x, y);
                let a = f[3] as f32 / 255.0;
                if a > 0.002 {
                    px[0] = (f[0] as f32 + px[0] as f32 * (1.0 - a)).clamp(0.0, 255.0) as u8;
                    px[1] = (f[1] as f32 + px[1] as f32 * (1.0 - a)).clamp(0.0, 255.0) as u8;
                    px[2] = (f[2] as f32 + px[2] as f32 * (1.0 - a)).clamp(0.0, 255.0) as u8;
                    px[3] = 255;
                }
            }
        }
        comp.save(dst_path)?;
        Ok(())
    };

    // 1. 经典收起态液态玻璃胶囊
    a.frame = Frame::hidden(a.options());
    let mut cap = a.scene();
    cap.has_track = true;
    cap.playing = true;
    cap.lyric = "Liquid Glass Beta · 曲面折射";
    cap.animate = true;
    cap.lyric_age_ms = 1000.;
    cap.cover_age_ms = 1000.;
    cap.activity_scales = [0.65, 1.0, 0.78];
    cap.activity_opacity = 1.0;
    r.render(&cap, false)?;
    let cap_path = dir.join("liquid-glass-capsule.png");
    r.snapshot(&cap_path)?;
    save_desktop_composite(&cap_path, &dir.join("liquid-glass-capsule-desktop.png"))?;

    // 2. 展开态液态玻璃音乐大卡片
    a.layout.card.w = 420.;
    a.layout.card.x = (a.layout.host.w - a.layout.card.w) / 2.;
    a.layout.card.h = crate::layout::music_height(0.0, 12.5);
    a.frame = Frame::expanded();
    let mut card = a.scene();
    card.has_track = true;
    card.playing = true;
    card.favorite_enabled = true;
    card.title = "DIA VERAO · Liquid Glass Beta";
    card.artist = "DotIsle Optical Lab";
    card.lyric = "晚风穿过玻璃边缘，折射出细腻霓虹";
    card.secondary = "Refraction · Dispersion · Fresnel Specular Rim";
    card.highlight = 0.56;
    card.progress = 0.44;
    card.elapsed = 109.;
    card.duration = 248.;
    card.time_ms = 1200.;
    card.cover_age_ms = 1000.;
    card.lyric_age_ms = 1000.;
    card.animate = true;
    card.controls_opacity = 1.;
    card.activity_scales = [0.65, 1.0, 0.78];
    card.activity_opacity = 1.;
    r.render(&card, false)?;
    let card_path = dir.join("liquid-glass-card.png");
    r.snapshot(&card_path)?;
    save_desktop_composite(&card_path, &dir.join("liquid-glass-card-desktop.png"))?;

    a.renderer = Some(r);
    Ok(())
}
