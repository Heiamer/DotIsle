//! Headless control-centre captures and hit-tree dumps for theme verification.
//! Read-only with respect to layout: it calls the production scene builder.
use std::path::Path;
use std::time::Instant;
use serde_json::{json,Value};
use vello::kurbo::{Affine,Shape};
use vello::Scene;
use crate::{model::{Action,Model,Page},draw::{self,Icon,Text},views::{self,ViewState},gpu::Gpu};

pub fn run(out:&Path)->Result<(),String>{
    std::fs::create_dir_all(out).map_err(|e|e.to_string())?;
    let data=out.join(format!("runtime-{}",std::process::id()));
    let mut m=Model::new(data.clone(),true);
    m.tutorial=None;m.tutorial_checked=true;m.toast=None;
    let mut text=Text::new();let mut gpu=Gpu::new();
    for theme in ["Original","Nothing"]{
        m.prefs.control_center_theme=theme.into();
        for dark in [false,true]{
            m.prefs.dark=dark;
            for tab in 0..8u8{
                m.page=Page::Settings;m.settings_tab=tab;m.scroll=0.;
                let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
                let mode=if dark{"dark"}else{"light"};
                gpu.capture(&view.scene,1200,800,&out.join(format!("settings-{mode}-{tab}-{theme}.png")))?;
                let dump=out.join(format!("hits-{mode}-{tab}-{theme}.json"));
                std::fs::write(&dump,serde_json::to_vec_pretty(&hit_json(&m,&view)).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
            }
        }
    }
    // Bottom-of-page captures: theme slider (tab 0) and font scope (tab 4), both themes.
    for theme in ["Original","Nothing"]{
        m.prefs.control_center_theme=theme.into();m.prefs.dark=false;
        for tab in [0u8,4]{
            m.page=Page::Settings;m.settings_tab=tab;m.scroll=0.;
            let first=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
            m.scroll=first.max_scroll;
            let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
            gpu.capture(&view.scene,1200,800,&out.join(format!("settings-bottom-{tab}-{theme}.png")))?;
            std::fs::write(out.join(format!("hits-bottom-{tab}-{theme}.json")),serde_json::to_vec_pretty(&hit_json(&m,&view)).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        }
    }
    m.prefs.control_center_theme="Nothing".into();m.scroll=0.;
    // Control-centre / island dynamic-colour cross matrix: separate keys, separate gates.
    for(cc,island)in [(false,true),(true,false),(true,true),(false,false)]{
        m.prefs.nothing_dynamic=cc;m.prefs.control_center_theme="Nothing".into();
        m.native_state["settings"]["ControlCenterMonetEnabled"]=json!(cc);
        m.native_state["settings"]["ExpandedMonetEnabled"]=json!(island);
        let roles=m.dynamic_color();
        std::fs::write(out.join(format!("isolation-cc{}-island{}.json",cc as u8,island as u8)),serde_json::to_vec_pretty(&json!({
            "control_center":{"key":"ControlCenterMonetEnabled","theme_gate":"prefs.nothing_dynamic","on":cc,"uses_static_palette":!cc,"resolved_primary":format!("#{:06X}",roles.roles.primary)},
            "island":{"key":"ExpandedMonetEnabled","on":island,"independent":true},
            "lyric_color":{"key":"LyricColorFromCover","untouched":true},
        })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    }
    m.prefs.nothing_dynamic=false;m.prefs.control_center_theme="Nothing".into();

    // CPU scene-build cost: same page, 60 frames, Original vs Nothing.
    let mut perf=vec![];
    for theme in ["Original","Nothing"]{
        m.prefs.control_center_theme=theme.into();m.prefs.dark=true;m.page=Page::Settings;m.settings_tab=0;m.scroll=0.;
        let start=std::time::Instant::now();for _ in 0..60{let _=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});}
        perf.push(json!({"theme":theme,"frames":60,"scene_build_ms_per_frame":(start.elapsed().as_secs_f64()*1000./60.*1000.).round()/1000.}));
    }
    std::fs::write(out.join("perf.json"),serde_json::to_vec_pretty(&json!({"scene_build":perf})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    m.prefs.control_center_theme="Nothing".into();
    // Persistence: write through the existing preferences file, then construct a new model.
    m.dispatch(Action::ControlTheme(0));
    let reread=Model::new(out.join(format!("runtime-{}",std::process::id())),true);
    m.dispatch(Action::FontScope(0));
    let font_reread=Model::new(out.join(format!("runtime-{}",std::process::id())),true);
    std::fs::write(out.join("persisted.json"),serde_json::to_vec_pretty(&json!({
        "theme_after_restart":reread.prefs.control_center_theme,
        "font_scope_after_restart":font_reread.prefs.font_scope,
        "dynamic_default":reread.prefs.nothing_dynamic,
        "font_family_key":"IslandFontFamily",
        "font_scope_key":"FontScope",
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;

    // [1] Font scope matrix. Every capture is the production scene; the face used
    // at runtime is read back through the same resolver the UI calls.
    m.prefs.control_center_theme="Nothing".into();m.prefs.dark=true;m.page=Page::Settings;m.settings_tab=0;m.scroll=0.;
    m.native_state["settings"]["IslandFontFamily"]=json!("Microsoft YaHei");
    let island_face=Text::system_family("Microsoft YaHei").unwrap_or_else(||"Microsoft YaHei".into());
    let sample="点屿 DotIsle 字体样张 Face 0123";
    let mut rows=vec![];let mut sets=Vec::new();
    for scope in ["Island","Center","Both"]{
        m.prefs.font_scope=scope.into();
        let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
        let center=views::center_face(&m).unwrap_or_else(||"Google Sans Flex".into());
        let island=if scope=="Center"{"Google Sans Flex".to_string()}else{island_face.clone()};
        let pinned=text.face_width("Google Sans Flex",sample,16.,400);
        let shaped=text.face_width(&center,sample,16.,400);
        gpu.capture(&view.scene,1200,800,&out.join(format!("font-scope-{scope}-center.png")))?;
        let hits=hit_json(&m,&view);std::fs::write(out.join(format!("hits-font-{scope}.json")),serde_json::to_vec_pretty(&hits).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
        sets.push((scope.to_string(),hits));
        rows.push(json!({"scope":scope,"center_face_runtime":center,"center_width_dp":(shaped*100.).round()/100.,"pinned_width_dp":(pinned*100.).round()/100.,"glyph_face_differs":(shaped-pinned).abs()>0.01,"island_face_runtime":island}));
    }
    let mut comparisons=vec![];
    for(a,b)in[("Island","Center"),("Center","Both")]{
        let find=|set:&Value,action:&str,label:&str|set["hits"].as_array().and_then(|h|h.iter().find(|x|x["action"]==action&&x["label"]==label)).map(|x|x["rect"].clone());
        let(ba,bb)=(sets.iter().find(|(s,_)|s.as_str()==a).map(|(_,v)|v).unwrap(),sets.iter().find(|(s,_)|s.as_str()==b).map(|(_,v)|v).unwrap());
        let mut max=0f64;let mut pairs=0;let mut missing=vec![];
        for h in ba["hits"].as_array().unwrap_or(&vec![]){let(action,label)=(h["action"].as_str().unwrap_or(""),h["label"].as_str().unwrap_or(""));let Some(other)=find(bb,action,label)else{missing.push(format!("{action}|{label}"));continue};pairs+=1;
            for i in 0..4{max=max.max((h["rect"][i].as_f64().unwrap_or(0.)-other[i].as_f64().unwrap_or(0.)).abs());}}
        comparisons.push(json!({"a":a,"b":b,"rects_compared":pairs,"max_delta":max,"missing":missing}));
    }
    std::fs::write(out.join("font-scope.json"),serde_json::to_vec_pretty(&json!({"family":"Microsoft YaHei","rows":rows,"comparisons":comparisons,"overflow":crate::draw::Text::face_overflow_log().iter().map(|(t,s,e)|json!({"text":t,"size":s,"excess_dp":(e*100.).round()/100.})).collect::<Vec<_>>()})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;

    // Font samples: same mixed CN/EN line at three sizes, pinned face vs an explicit
    // face, 1x and 2x. `preview_line` never touches the pinned layout cache.
    let families=[("default","Google Sans Flex"),("selected","Microsoft YaHei")];
    for(name,family)in families{
        for size in [12u32,16,24]{
            let mut sample_scene=Scene::new();
            draw::fill(&mut sample_scene,draw::rect(0.,0.,720.,320.),0.,0x101010);
            text.preview_line(&mut sample_scene,family,"点屿 DotIsle · 专注与聆听 Focus & Listen 0123",draw::rect(24.,90.,660.,40.),size as f32,0xE8E8E8,400);
            text.preview_line(&mut sample_scene,family,"中文回退：轻舟已过万重山 Lyric Island",draw::rect(24.,180.,660.,40.),size as f32,0xE8E8E8,400);
            text.preview_line(&mut sample_scene,family,"小字号细节 Aa 12.5px 未 hint 子像素定位",draw::rect(24.,260.,660.,40.),size as f32,0x999999,400);
            gpu.capture(&sample_scene,720,320,&out.join(format!("font-{name}-{size}-1x.png")))?;
            let mut scaled=Scene::new();scaled.append(&sample_scene,Some(Affine::scale(2.)));
            gpu.capture(&scaled,1440,640,&out.join(format!("font-{name}-{size}-2x.png")))?;
        }
    }
    // Icon sheet: 4× samples over the shared 16×16 cell grid, then a 1× annotated row.
    let pad=28f64;let cell=96f64;let cols=8usize;let icons=crate::nothing::ALL;
    let rows_n=(icons.len()+cols-1)/cols;
    let width=pad*2.+cols as f64*(cell+pad);
    let height=pad*2.+rows_n as f64*(cell+34.+pad)+230.;
    let mut sheet=Scene::new();draw::fill(&mut sheet,draw::rect(0.,0.,width,height),0.,0x080808);
    for(i,icon)in icons.iter().enumerate(){
        let cx=pad+(i%cols)as f64*(cell+pad);let cy=pad+(i/cols)as f64*(cell+34.+pad);
        draw::fill(&mut sheet,draw::rect(cx-2.,cy-2.,cell+4.,cell+4.),0.,0x101010);
        for g in 0..=16{let x0=cx+cell*g as f64/16.;draw::fill(&mut sheet,draw::rect(x0,cy,0.6,cell),0.,0x1E1E1E);let y0=cy+cell*g as f64/16.;draw::fill(&mut sheet,draw::rect(cx,y0,cell,0.6),0.,0x1E1E1E);}
        let mut glyph=Scene::new();crate::nothing::icon(&mut glyph,*icon,0.,0.,24.,0xFFFFFF);
        sheet.append(&glyph,Some(Affine::translate((cx,cy))*Affine::scale(4.)));
        text.draw(&mut sheet,crate::nothing::icon_name(*icon),cx,cy+cell+18.,11.,0xB0B0B0,false,cell);
    }
    // The 1× row uses the same 8 columns as the sheet; 16 columns overflowed the canvas
    // and cut the last names off, so names now always get their full cell width.
    for(i,icon)in icons.iter().enumerate(){
        let step=cell+pad;let(cx,cy)=(pad+(i%cols)as f64*step,pad+rows_n as f64*(cell+34.+pad)+36.+(i/cols)as f64*100.);
        crate::nothing::icon(&mut sheet,*icon,cx,cy,24.,0xFFFFFF);
        text.draw(&mut sheet,&format!("{}.{}",i+1,crate::nothing::icon_name(*icon)),cx-2.,cy+34.,10.,0x9A9A9A,false,step-10.);
    }
    gpu.capture(&sheet,width as u32,height as u32,&out.join("icon-sheet-4x.png"))?;
    // Rail sheet: the six navigation glyphs on their own, 4× over the shared 16×16 lattice,
    // each labelled with the three numbers the brief asks for (ink box, dots, coverage).
    let rail_icons:[Icon;6]=[Icon::Home,Icon::Island,Icon::Music,Icon::Focus,Icon::Todo,Icon::Settings];
    let rcell=96f64;let rcols=3usize;let rrow=rcell+48.+pad;
    let rw=pad*2.+rcols as f64*(rcell+pad);
    let rh=pad*2.+2.*rrow+52.;
    let mut rsheet=Scene::new();draw::fill(&mut rsheet,draw::rect(0.,0.,rw,rh),0.,0x080808);
    let mut rail_rows=vec![];
    for(i,icon)in rail_icons.iter().enumerate(){
        let cx=pad+(i%rcols)as f64*(rcell+pad);let cy=pad+(i/rcols)as f64*rrow;
        draw::fill(&mut rsheet,draw::rect(cx-2.,cy-2.,rcell+4.,rcell+4.),0.,0x101010);
        for g in 0..=16{
            let x0=cx+rcell*g as f64/16.;draw::fill(&mut rsheet,draw::rect(x0,cy,0.7,rcell),0.,0x272727);
            let y0=cy+rcell*g as f64/16.;draw::fill(&mut rsheet,draw::rect(cx,y0,rcell,0.7),0.,0x272727);
        }
        let mut glyph=Scene::new();crate::nothing::icon(&mut glyph,*icon,0.,0.,24.,0xFFFFFF);
        rsheet.append(&glyph,Some(Affine::translate((cx,cy))*Affine::scale(4.)));
        let(w,h)=crate::nothing::ink_box_measured(*icon);let n=crate::nothing::dot_count(*icon);
        let(bw,bh)=crate::nothing::ink_box(*icon);
        let ext=crate::nothing::dot_extent(*icon).map(|(a,b,c,d)|vec![a,b,c,d]).unwrap_or_default();
        let cov=crate::nothing::coverage(*icon)*100.;
        let(sw,sh)=crate::nothing::ink_box_small(*icon);let sn=crate::nothing::dot_count_small(*icon);
        text.draw(&mut rsheet,crate::nothing::icon_name(*icon),cx,cy+rcell+17.,12.,0xE8E8E8,false,rcell);
        text.draw(&mut rsheet,&format!("ink {:.*}×{:.*}c · {} dots · {:.1}%",0, w,0, h,n,cov),cx,cy+rcell+33.,10.,0x9A9A9A,false,rcell);
        text.draw(&mut rsheet,&format!("16dp: {:.*}×{:.*}c · {} dots",0, sw,0, sh, sn),cx,cy+rcell+45.,9.,0x707070,false,rcell);
        let box_ok=(w-14.).abs()<=0.5&&(h-14.).abs()<=0.5;
        let min_ok=w>=10.&&h>=10.;
        let small_ok=(sw-6.).abs()<=0.5&&(sh-6.).abs()<=0.5;
        rail_rows.push(json!({"icon":crate::nothing::icon_name(*icon),"ink":[w,h],"dots":n,
            "coverage_pct":(cov*100.).round()/100.,"box_ok":box_ok,"min_ink_ok":min_ok,
            "legacy_ink_field":[bw,bh],"extent":ext,
            "small_ink":[sw,sh],"small_dots":sn,"small_ok":small_ok}));
    }
    let strip_y=pad*2.+2.*rrow+6.;
    for(i,icon)in rail_icons.iter().enumerate(){
        let cx=pad+i as f64*(rcell+pad);
        crate::nothing::icon(&mut rsheet,*icon,cx,strip_y,24.,0xFFFFFF);
        text.draw(&mut rsheet,"24dp 1×",cx,strip_y+30.,9.,0x707070,false,rcell);
    }
    gpu.capture(&rsheet,rw as u32,rh as u32,&out.join("rail-sheet-4x.png"))?;
    let rail_pass=rail_rows.iter().all(|r|r["box_ok"].as_bool()==Some(true)&&r["min_ink_ok"].as_bool()==Some(true)&&r["small_ok"].as_bool()==Some(true));
    std::fs::write(out.join("rail-icons.json"),serde_json::to_vec_pretty(&json!({
        "grid":16,"target_box_cells":14,"min_ink_cells":10,"small_grid":8,"small_target_cells":6,
        "dot_ratio":crate::nothing::DOT_RATIO,"rows":rail_rows,"all_pass":rail_pass,
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;

    // Self-check sheets: every icon at the two sizes it is actually used at.
    for(size,scale)in[(24f64,1f64),(32.,1.)]{
        let cols=8usize;let cell=size+26.;let(w,h)=(pad*2.+cols as f64*(cell+pad),pad*2.+rows_n as f64*(cell+26.+pad));
        let mut check=Scene::new();draw::fill(&mut check,draw::rect(0.,0.,w,h),0.,0x080808);
        for(i,icon)in icons.iter().enumerate(){
            let cx=pad+(i%cols)as f64*(cell+pad);let cy=pad+(i/cols)as f64*(cell+26.+pad);
            crate::nothing::icon(&mut check,*icon,cx,cy,size,0xFFFFFF);
            text.draw(&mut check,&format!("{}.{}",i+1,crate::nothing::icon_name(*icon)),cx,cy+size+18.,11.,0xB0B0B0,false,cell);
        }
        let _=scale;gpu.capture(&check,w as u32,h as u32,&out.join(format!("icon-check-{size}.png")))?;
    }

    // Bounding-box audit: legacy Material ink box vs the new dot extents at 24dp.
    let cell=24./crate::nothing::GRID;let r=cell*crate::nothing::DOT_RATIO;
    let mut bounds=vec![];let mut over=0;
    for icon in icons{
        let Some((x0,y0,x1,y1))=crate::nothing::ink_bounds(icon)else{continue};
        let Some(paths)=dotisle_render::material_symbols::paths_for(icon)else{continue};
        let tr=Affine::translate((0.,24.))*Affine::scale(1./40.);
        let mut legacy:Option<vello::kurbo::Rect>=None;
        for path in paths{let b=tr.transform_rect_bbox(path.bounding_box());legacy=Some(match legacy{None=>b,Some(a)=>a.union(b)});}
        let new=[x0*cell-r,y0*cell-r,x1*cell+r,y1*cell+r];
        if let Some(old)=legacy{
            let d=[(new[0]-old.x0).abs(),(new[1]-old.y0).abs(),(new[2]-old.x1).abs(),(new[3]-old.y1).abs()];
            if d.iter().cloned().fold(0.,f64::max)>1.{over+=1;}
            bounds.push(json!({"icon":crate::nothing::icon_name(icon),"legacy":[old.x0,old.y0,old.x1,old.y1],"dots":new,"delta_dp":d}));
        }
    }
    std::fs::write(out.join("icon-bounds.json"),serde_json::to_vec_pretty(&json!({"over_1dp":over,"rows":bounds})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    m.prefs.dark=true;m.page=Page::Settings;m.settings_tab=0;m.preview_values.clear();
    m.prefs.control_center_theme="Original".into();
    let first=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});m.scroll=first.max_scroll;
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("slider-left.png"))?;
    m.prefs.control_center_theme="Nothing".into();
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("slider-right.png"))?;
    m.preview_values.insert("ControlCenterThemeSlot".into(),json!(0.5));
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("slider-drag.png"))?;
    let slider=view.hits.iter().find(|h|matches!(h.action,Action::ThemeHandle(_)));
    std::fs::write(out.join("slider-hits.json"),serde_json::to_vec_pretty(&json!({
        "hit":slider.map(|h|json!({"label":h.label,"rect":[h.rect.x0,h.rect.y0,h.rect.x1,h.rect.y1],"action":format!("{:?}",h.action)}))}))
        .map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    m.preview_values.clear();
    std::fs::write(out.join("slider-labels.json"),serde_json::to_vec_pretty(&json!({
        "drag":{"fraction":0.5,"target_label":crate::nothing::label(crate::nothing::slot_from_fraction(0.5)),"target_alpha":0.72,"committed":false},
        "release":{"target_label":"Nothing","target_alpha":1.0,"committed":true,"theme":m.prefs.control_center_theme},
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;

    // 16dp uses the compact 8×8 lattice: Home (navigation slot) and Check (light chip).
    for(icon,tag,file)in[(Icon::Home,"Home","icon-home-16-4x.png"),(Icon::Check,"Check","icon-check-16-4x.png")]{
        let s=16f64;let scale=4.;let pad=36f64;let square=s*scale;let total=square+pad*2.;
        let mut zoom=Scene::new();draw::fill(&mut zoom,draw::rect(0.,0.,total,total+30.),0.,0x080808);
        for g in 0..=8{let q=pad+square*g as f64/8.;draw::fill(&mut zoom,draw::rect(q,pad,0.6,square),0.,0x1E1E1E);draw::fill(&mut zoom,draw::rect(pad,q,square,0.6),0.,0x1E1E1E);}
        let mut glyph=Scene::new();crate::nothing::icon(&mut glyph,icon,0.,0.,s,0xFFFFFF);
        zoom.append(&glyph,Some(Affine::translate((pad,pad))*Affine::scale(scale)));
        text.draw(&mut zoom,&format!("{tag} · 16dp · 8×8 · 4×"),8.,pad+square+20.,10.,0xB0B0B0,false,total-16.);
        gpu.capture(&zoom,total as u32,(total+30.)as u32,&out.join(file))?;
    }
    // Light card 1px border at 4×: page #EBEBEB, card #FFFFFF, border #C4C4C4.
    {
        let light=crate::nothing::scheme(false);
        let mut swatch=Scene::new();
        draw::fill(&mut swatch,draw::rect(0.,0.,120.,78.),0.,light.surface);
        draw::fill(&mut swatch,draw::rect(14.,14.,92.,50.),8.,light.container);
        draw::outline(&mut swatch,draw::rect(14.,14.,92.,50.),8.,crate::nothing::border(false),1.);
        let mut zoom=Scene::new();draw::fill(&mut zoom,draw::rect(0.,0.,500.,350.),0.,0x080808);
        zoom.append(&swatch,Some(Affine::scale(4.)));
        text.draw(&mut zoom,"page #EBEBEB · card #FFFFFF · border #C4C4C4 · 4×",10.,332.,12.,0xB0B0B0,false,480.);
        gpu.capture(&zoom,500,350,&out.join("light-border-4x.png"))?;
    }
    // Compact-grid bounding-box audit at 16dp against the legacy ink box scaled to 16dp.
    let cell_s=16./crate::nothing::SMALL_GRID;let r_s=cell_s*crate::nothing::DOT_RATIO;
    let mut small=vec![];let mut small_over=0;
    for icon in icons{
        let Some((x0,y0,x1,y1))=crate::nothing::ink_bounds_small(icon)else{continue};
        let Some(paths)=dotisle_render::material_symbols::paths_for(icon)else{continue};
        let tr=Affine::translate((0.,16.))*Affine::scale(1./60.);
        let mut legacy:Option<vello::kurbo::Rect>=None;
        for path in paths{let b=tr.transform_rect_bbox(path.bounding_box());legacy=Some(match legacy{None=>b,Some(a)=>a.union(b)});}
        let new=[x0*cell_s-r_s,y0*cell_s-r_s,x1*cell_s+r_s,y1*cell_s+r_s];
        if let Some(old)=legacy{
            let d=[(new[0]-old.x0).abs(),(new[1]-old.y0).abs(),(new[2]-old.x1).abs(),(new[3]-old.y1).abs()];
            if d.iter().cloned().fold(0.,f64::max)>1.{small_over+=1;}
            small.push(json!({"icon":crate::nothing::icon_name(icon),"dots":crate::nothing::dot_count_small(icon),"delta_dp":d}));
        }
    }
    let grid_by_size:Vec<_>=[16.,20.,24.,32.].iter().map(|s|{
        let g=crate::nothing::grid_for(*s);let cell=s/g;let r=(cell*crate::nothing::DOT_RATIO).clamp(0.5,1.6);
        json!({"size_dp":s,"grid":g,"lattice":if g==crate::nothing::SMALL_GRID{"8x8"}else{"16x16"},
            "cell_dp":cell,"dot_radius_dp":r,"home_dots":if g==crate::nothing::SMALL_GRID{crate::nothing::dot_count_small(Icon::Home)}else{crate::nothing::dot_count(Icon::Home)},
            "check_dots":if g==crate::nothing::SMALL_GRID{crate::nothing::dot_count_small(Icon::Check)}else{crate::nothing::dot_count(Icon::Check)}})
    }).collect();
    std::fs::write(out.join("icon-small-bounds.json"),serde_json::to_vec_pretty(&json!({
        "grid":8,"size_dp":16,"cell":cell_s,"dot_radius":r_s,"over_1dp":small_over,"rows":small,
        "grid_by_size":grid_by_size,"small_max_dp":crate::nothing::SMALL_MAX,"dot_ratio":crate::nothing::DOT_RATIO,
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;

    // Theme transition: pin at 45% so a real mid-transition frame can be inspected.
    m.prefs.control_center_theme="Original".into();let from=m.scheme();let dark=m.prefs.dark;
    m.prefs.control_center_theme="Nothing".into();
    crate::nothing::begin_transition(0,from,dark);
    crate::nothing::set_transition_progress(Some(0.45));
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("theme-transition-mid.png"))?;
    crate::nothing::end_transition();
    // Measured, not just declared: start the engine and tick it until it settles.
    crate::nothing::begin_transition(0,from,dark);
    let t0=Instant::now();let mut ticks=0;
    while crate::nothing::finish_transition(){ticks+=1;std::thread::sleep(std::time::Duration::from_millis(1));}
    let measured_ms=t0.elapsed().as_secs_f64()*1000.;
    std::fs::write(out.join("theme-transition.json"),serde_json::to_vec_pretty(&json!({
        "duration_ms":crate::nothing::TRANSITION_MS,"measured_ms":(measured_ms*1000.).round()/1000.,"ticks":ticks,
        "pinned_progress":0.45,"interpolated":["primary","surface","container","outline","on_surface","on_surface_variant"],
        "mid_frame":"theme-transition-mid.png",
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    crate::nothing::end_transition();

    // Slider hover / pressed frames. The hit index is stable between renders.
    let probe=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
    let idx=probe.hits.iter().position(|h|matches!(h.action,Action::ThemeHandle(_)));
    let handle=probe.hits.iter().find_map(|h|if let Action::ThemeHandle(t)=h.action{Some((t[0]+t[2],t[1]+t[3]/2.))}else{None}).unwrap_or((1000.,711.));
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,pointer:handle,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("slider-hover.png"))?;
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,pointer:handle,pressed:idx,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("slider-pressed.png"))?;
    m.preview_values.clear();
    let view=views::render(&m,&mut text,1200.,800.,&ViewState{rail_y:522.,..Default::default()});
    gpu.capture(&view.scene,1200,800,&out.join("slider-release.png"))?;
    std::fs::write(out.join("slider-feedback.json"),serde_json::to_vec_pretty(&json!({
        "hover":{"pointer":handle,"ring":"1.5dp #D71921","handle_r":11.0,"dot_r":3.0},
        "pressed":{"hit_index":idx,"handle_r":13.0,"dot_r":4.5,"delta_r":2.0,"delta_dot":1.5},
        "release":{"theme":m.prefs.control_center_theme,"label_alpha":1.0},
    })).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;

    // Contrast table for the two requested pairs plus the off/on ordering guard.
    let ratios=json!({
        "dark":{"track_off":"#3A3A3A","thumb_off":"#888888","container":"#111111",
            "thumb_off_vs_track":contrast(0x888888,0x3A3A3A),
            "thumb_off_vs_container":contrast(0x888888,0x111111),
            "thumb_off_vs_on_track":contrast(0x888888,0xFFFFFF),
            "track_off_vs_container":contrast(0x3A3A3A,0x111111)},
        "light":{"border":"#C4C4C4","card":"#FFFFFF","page":"#EBEBEB",
            "border_vs_card":contrast(0xC4C4C4,0xFFFFFF),
            "card_vs_page":contrast(0xFFFFFF,0xEBEBEB),
            "border_vs_page":contrast(0xC4C4C4,0xEBEBEB)},
    });
    std::fs::write(out.join("contrast.json"),serde_json::to_vec_pretty(&ratios).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    Ok(())
}
fn lum(rgb:u32)->f64{let c=|v:u32|{let x=v as f64/255.;if x<=0.03928{x/12.92}else{((x+0.055)/1.055).powf(2.4)}};0.2126*c((rgb>>16)&255)+0.7152*c((rgb>>8)&255)+0.0722*c(rgb&255)}
fn contrast(a:u32,b:u32)->f64{let(la,lb)=(lum(a),lum(b));let hi=la.max(lb)+0.05;let lo=la.min(lb)+0.05;(hi/lo*100.).round()/100.}
fn hit_json(m:&Model,view:&views::View)->Value{
    let mut hits=view.hits.iter().map(|h|json!({
        "label":h.label,"action":format!("{:?}",h.action),
        "rect":[h.rect.x0,h.rect.y0,h.rect.x1,h.rect.y1],"enabled":h.enabled,
    })).collect::<Vec<_>>();
    hits.sort_by(|a,b|a["action"].as_str().unwrap_or("").cmp(b["action"].as_str().unwrap_or(""))
        .then(a["rect"][0].as_f64().unwrap_or(0.).partial_cmp(&b["rect"][0].as_f64().unwrap_or(0.)).unwrap_or(std::cmp::Ordering::Equal)));
    json!({"width":1200.,"height":800.,"dark":m.prefs.dark,"tab":m.settings_tab,"theme":crate::nothing::name(m),"max_scroll":view.max_scroll,"hits":hits})
}
