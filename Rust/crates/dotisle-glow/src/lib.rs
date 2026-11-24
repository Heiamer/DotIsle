use serde_json::{json, Value};
use std::{cell::RefCell, collections::BTreeMap};
mod engine;

thread_local! {
    static STATES: RefCell<BTreeMap<u64, engine::State>> = RefCell::new(BTreeMap::new());
    static NEXT: std::cell::Cell<u64> = const { std::cell::Cell::new(1) };
}

fn mix_rgb(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0., 1.);
    let ch = |shift: u32| -> u32 {
        let ca = ((a >> shift) & 0xff) as f64;
        let cb = ((b >> shift) & 0xff) as f64;
        (ca + (cb - ca) * t).round().clamp(0., 255.) as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

fn build_paint_commands(f: engine::Frame, accent: u32, bounds: &Value, radius: &Value) -> Vec<Value> {
    let base = accent & 0xffffff;
    let hi = mix_rgb(base, 0xFFFFFF, 0.56);
    let soft = mix_rgb(base, 0xA8DCFF, 0.42);
    let (colors, halo_rgb, expand, stroke_scale) = match f.kind {
        engine::Kind::Update => (
            // 高雅光学棱镜流彩光谱
            vec![0x60A5FA, 0xA78BFA, 0xF472B6, 0xFBBF24, 0x34D399, 0x38BDF8, 0x60A5FA],
            0x8AB4F8,
            1.6,
            1.05,
        ),
        engine::Kind::Timer => (
            // 专注暖金与柔和象牙白双色冥想流光
            vec![0xFFF3E0, 0xFFB870, 0xF69D7E, 0xFFD8A8, 0xFFF3E0],
            0xFFB870,
            1.2,
            0.92,
        ),
        engine::Kind::MusicRhythm => (
            // 随音频峰值动态调和的专辑极光三色，外晕微幅舒张
            vec![0xFFFFFF, hi, base, soft, 0xFFFFFF],
            mix_rgb(base, 0xE2F4FF, 0.30),
            1.2 + f.strength * 2.4,
            1.15,
        ),
        engine::Kind::MusicBreath => (
            // 基于当前强调/专辑主色生成温润呼吸极光
            vec![hi, base, soft, mix_rgb(soft, 0xFFFFFF, 0.45), hi],
            base,
            1.0 + f.strength * 1.1,
            0.95,
        ),
        engine::Kind::Off => return vec![],
    };

    let mut out = vec![
        // 1. 底层原生高斯漫反射外光晕（安全边距内柔和包裹灵动岛）
        json!({
            "op": "blurredHalo",
            "bounds": bounds,
            "radius": radius,
            "inflate": expand + 3.0,
            "blur": 9.5,
            "alpha": (0.20 * f.strength).clamp(0., 0.35),
            "color": halo_rgb,
        }),
    ];
    // 2. 分层光学扫光边缘丝线，外散宽晕收至细腻贴边亮边
    for (width, opacity, inf) in [
        (8.5, 0.055, expand + 0.8),
        (4.0, 0.125, expand + 0.4),
        (1.35, 0.34, 0.6),
    ] {
        out.push(json!({
            "op": "roundedStroke",
            "bounds": bounds,
            "radius": radius,
            "inflate": inf,
            "width": width,
            "alpha": (opacity * f.strength * stroke_scale).clamp(0., 0.65),
            "colors": colors,
            "phase": f.phase,
        }));
    }
    out
}

fn dispatch(v: Value) -> Result<Value, String> {
    let a = &v["args"];
    let method = v["method"].as_str().ok_or("method")?;
    match method {
        "state.new" => {
            let id = NEXT.with(|n| {
                let id = n.get();
                n.set(id + 1);
                id
            });
            STATES.with(|m| m.borrow_mut().insert(id, engine::State::default()));
            Ok(json!({"handle": id}))
        }
        "state.drop" => {
            STATES.with(|m| m.borrow_mut().remove(&a["handle"].as_u64().unwrap_or(0)));
            Ok(Value::Null)
        }
        "paint" => {
            let f: engine::Frame =
                serde_json::from_value(a["frame"].clone()).map_err(|e| e.to_string())?;
            if f.kind == engine::Kind::Off || f.strength <= 0.001 {
                return Ok(json!([]));
            }
            let accent = a["accent"].as_u64().ok_or("accent")? as u32;
            Ok(json!(build_paint_commands(f, accent, &a["bounds"], &a["radius"])))
        }
        _ => STATES.with(|m| {
            let mut m = m.borrow_mut();
            let s = m
                .get_mut(&a["handle"].as_u64().ok_or("handle")?)
                .ok_or("glow session missing")?;
            match method {
                "state.tick" => Ok(json!(s.tick(
                    a["dt"].as_f64().ok_or("dt")?,
                    serde_json::from_value(a["target"].clone()).map_err(|e| e.to_string())?,
                    a["animate"] == true
                ))),
                "state.active" => Ok(json!(s.active())),
                "state.frame" => Ok(json!(s.frame(
                    a["time_ms"].as_f64().ok_or("time_ms")?,
                    a["animate"] == true
                ))),
                _ => Err("unknown glow method".into()),
            }
        }),
    }
}

dotisle_module_api::export_service!("glow", dispatch);
