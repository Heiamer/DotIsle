use super::*;
use crate::state::Card;
use skia_safe::{Image, Path, Point, TileMode, gradient_shader, paint::Cap, paint::Style};
use std::f32::consts::TAU;

fn draw_line(c: &Canvas, x0: f32, y0: f32, x1: f32, y1: f32, width: f32, col: Color) {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_style(Style::Stroke);
    p.set_stroke_width(width);
    p.set_stroke_cap(Cap::Round);
    p.set_color(col);
    c.draw_line((x0, y0), (x1, y1), &p);
}

/// Apple 风格圆环进度（Batteries / Timer / Fitness 通用）。
fn draw_apple_ring(
    c: &Canvas,
    cx: f32,
    cy: f32,
    radius: f32,
    thick: f32,
    frac: f32,
    col: Color,
    track_col: Color,
) {
    let oval = Rect::from_xywh(cx - radius, cy - radius, radius * 2., radius * 2.);
    let mut track = Paint::default();
    track.set_anti_alias(true);
    track.set_style(Style::Stroke);
    track.set_stroke_width(thick);
    track.set_color(track_col);
    c.draw_circle((cx, cy), radius, &track);

    let mut arc = Paint::default();
    arc.set_anti_alias(true);
    arc.set_style(Style::Stroke);
    arc.set_stroke_width(thick);
    arc.set_stroke_cap(Cap::Round);
    arc.set_color(col);
    c.draw_arc(oval, -90., frac.clamp(0.03, 1.0) * 360.0, false, &arc);
}

/// 绘制白色实心 Apple 定位小箭头（Weather 用）。
fn draw_location_arrow(c: &Canvas, x: f32, y: f32, col: Color) {
    let mut p = Path::new();
    p.move_to((x + 8.5, y + 0.5));
    p.line_to((x, y + 4.2));
    p.line_to((x + 4.0, y + 4.8));
    p.line_to((x + 4.6, y + 8.8));
    p.close();
    let mut paint = Paint::default();
    paint.set_anti_alias(true);
    paint.set_color(col);
    c.draw_path(&p, &paint);
}

/// 绘制精致 Apple 音乐双音符图标。
fn draw_music_note(c: &Canvas, x: f32, y: f32, scale: f32, col: Color) {
    let mut p = Paint::default();
    p.set_anti_alias(true);
    p.set_color(col);
    c.draw_oval(
        Rect::from_xywh(x, y + 7.5 * scale, 4.8 * scale, 3.6 * scale),
        &p,
    );
    c.draw_oval(
        Rect::from_xywh(x + 7. * scale, y + 6. * scale, 4.8 * scale, 3.6 * scale),
        &p,
    );
    let mut beam = Paint::default();
    beam.set_anti_alias(true);
    beam.set_style(Style::Stroke);
    beam.set_stroke_width(1.6 * scale);
    beam.set_stroke_cap(Cap::Round);
    beam.set_color(col);
    c.draw_line(
        (x + 4.2 * scale, y + 8.8 * scale),
        (x + 4.2 * scale, y + 1.5 * scale),
        &beam,
    );
    c.draw_line(
        (x + 11.2 * scale, y + 7.3 * scale),
        (x + 11.2 * scale, y),
        &beam,
    );
    beam.set_stroke_width(2.4 * scale);
    c.draw_line(
        (x + 4.2 * scale, y + 1.5 * scale),
        (x + 11.2 * scale, y),
        &beam,
    );
}

/// 绘制 Apple Weather 柔和暖阳（金黄渐变圆核）或太阳带云朵。
fn draw_apple_weather_icon(c: &Canvas, cx: f32, cy: f32, scale: f32, cloud: bool) {
    let sun_cx = if cloud { cx + 5. * scale } else { cx };
    let sun_cy = if cloud { cy - 4. * scale } else { cy };
    let sun_r = (if cloud { 6.5 } else { 8.5 }) * scale;
    let sun_cols = [Color::from_rgb(255, 224, 66), Color::from_rgb(255, 149, 0)];
    if let Some(sh) = gradient_shader::linear(
        (Point::new(sun_cx, sun_cy - sun_r), Point::new(sun_cx, sun_cy + sun_r)),
        &sun_cols[..],
        None,
        TileMode::Clamp,
        None,
        None,
    ) {
        let mut sp = Paint::default();
        sp.set_anti_alias(true);
        sp.set_shader(sh);
        c.draw_circle((sun_cx, sun_cy), sun_r, &sp);
    }
    if cloud {
        let ccols = [
            Color::from_argb(248, 255, 255, 255),
            Color::from_argb(215, 232, 240, 250),
        ];
        if let Some(sh) = gradient_shader::linear(
            (Point::new(cx, cy - 6. * scale), Point::new(cx, cy + 7. * scale)),
            &ccols[..],
            None,
            TileMode::Clamp,
            None,
            None,
        ) {
            let mut cp = Paint::default();
            cp.set_anti_alias(true);
            cp.set_shader(sh);
            c.draw_round_rect(
                Rect::from_xywh(cx - 10. * scale, cy - 0.5 * scale, 18. * scale, 7.8 * scale),
                3.9 * scale,
                3.9 * scale,
                &cp,
            );
            c.draw_circle((cx - 3. * scale, cy - 1.5 * scale), 5.0 * scale, &cp);
            c.draw_circle((cx + 3. * scale, cy + 0.5 * scale), 3.6 * scale, &cp);
        }
    }
}

/// 绘制 Apple Music 精绘专辑封面（暮光靛紫天空 + 暖金圆月 + 渐变微高光）。
fn draw_album_cover(c: &Canvas, r: Rect, rad: f32, seed: u8) {
    c.save();
    c.clip_rrect(skia_safe::RRect::new_rect_xy(r, rad, rad), None, true);
    let (top, bot, orb) = match seed {
        0 => (
            Color::from_rgb(28, 38, 86),
            Color::from_rgb(104, 52, 138),
            Color::from_rgb(255, 198, 92),
        ),
        1 => (
            Color::from_rgb(14, 72, 108),
            Color::from_rgb(46, 152, 168),
            Color::from_rgb(167, 243, 208),
        ),
        _ => (
            Color::from_rgb(84, 32, 52),
            Color::from_rgb(214, 92, 68),
            Color::from_rgb(254, 215, 170),
        ),
    };
    let cols = [top, bot];
    if let Some(sh) = gradient_shader::linear(
        (Point::new(r.left, r.top), Point::new(r.right, r.bottom)),
        &cols[..],
        None,
        TileMode::Clamp,
        None,
        None,
    ) {
        let mut p = Paint::default();
        p.set_shader(sh);
        c.draw_rect(r, &p);
    }
    let mut op = Paint::default();
    op.set_anti_alias(true);
    op.set_color(orb);
    c.draw_circle(
        (r.left + r.width() * 0.62, r.top + r.height() * 0.36),
        r.width() * 0.22,
        &op,
    );
    // 柔和山丘剪影弧
    let mut wave = Paint::default();
    wave.set_anti_alias(true);
    wave.set_color(Color::from_argb(95, 10, 14, 28));
    c.draw_circle(
        (r.left + r.width() * 0.35, r.bottom + r.height() * 0.12),
        r.width() * 0.68,
        &wave,
    );
    c.restore();

    let mut rim = Paint::default();
    rim.set_anti_alias(true);
    rim.set_style(Style::Stroke);
    rim.set_stroke_width(0.8);
    rim.set_color(Color::from_argb(85, 255, 255, 255));
    c.draw_round_rect(r, rad, rad, &rim);
}

/// 根据卡片类型返回 Apple 原生底壳双色（保留约 88%–93% 高对比饱和度与微妙亚克力呼吸感）。
fn apple_palette(active: &str, dark: bool) -> (Color, Color) {
    match active {
        "weather" => {
            if dark {
                (Color::from_argb(238, 16, 36, 72), Color::from_argb(238, 32, 72, 128))
            } else {
                (Color::from_argb(236, 30, 120, 224), Color::from_argb(236, 86, 166, 248))
            }
        }
        "music" => {
            if dark {
                (Color::from_argb(238, 152, 22, 44), Color::from_argb(238, 206, 36, 66))
            } else {
                (Color::from_argb(238, 250, 35, 59), Color::from_argb(238, 255, 79, 107))
            }
        }
        "activity" => (
            Color::from_argb(242, 24, 24, 27),
            Color::from_argb(242, 18, 18, 20),
        ),
        "analog_clock" => {
            if dark {
                (Color::from_argb(238, 28, 28, 30), Color::from_argb(238, 24, 24, 26))
            } else {
                (Color::from_argb(238, 34, 34, 38), Color::from_argb(238, 28, 28, 30))
            }
        }
        "clock" => {
            if dark {
                (Color::from_argb(238, 28, 28, 30), Color::from_argb(238, 24, 24, 26))
            } else {
                (Color::from_argb(236, 255, 255, 255), Color::from_argb(236, 247, 247, 250))
            }
        }
        "quote" => {
            if dark {
                (Color::from_argb(238, 32, 31, 29), Color::from_argb(238, 26, 25, 24))
            } else {
                (Color::from_argb(238, 252, 249, 242), Color::from_argb(238, 246, 242, 234))
            }
        }
        _ => {
            if dark {
                (Color::from_argb(236, 28, 28, 30), Color::from_argb(236, 24, 24, 26))
            } else {
                (Color::from_argb(234, 255, 255, 255), Color::from_argb(234, 248, 249, 252))
            }
        }
    }
}

/// 共享绘制：Apple Notes 风格「备忘录 · 剪贴板」与「便签」，支持双击原地编辑及中文 IME。
#[allow(clippy::too_many_arguments)]
fn draw_note_card(
    c: &Canvas,
    r: Rect,
    card: &Card,
    title: &str,
    default_hint: &str,
    fallback_text: Option<&str>,
    x: f32,
    w: f32,
    large: bool,
    fg: Color,
    sub: Color,
    v: &View<'_>,
    hits: &mut Vec<HitItem>,
) {
    let is_editing = v.editing_note == Some(card.id);
    let banner = Rect::from_xywh(r.left, r.top, r.width(), 34.);
    let mut bp = Paint::default();
    bp.set_anti_alias(true);
    bp.set_color(Color::from_rgb(255, 204, 0));
    c.draw_rect(banner, &bp);
    text_styled(
        c,
        title,
        x,
        r.top + 9.,
        (w - 94.).max(60.),
        12.,
        Color::from_rgb(28, 28, 30),
        Weight::BOLD,
        TextAlign::Left,
        1,
    );

    let base_text = if !card.note_text.is_empty() || is_editing {
        card.note_text.as_str()
    } else {
        fallback_text.unwrap_or("")
    };

    if is_editing {
        let clr_r = Rect::from_xywh(r.right - 98., r.top + 6., 40., 22.);
        let done_r = Rect::from_xywh(r.right - 54., r.top + 6., 42., 22.);
        let mut pill = Paint::default();
        pill.set_anti_alias(true);
        pill.set_color(Color::from_argb(38, 28, 28, 30));
        c.draw_round_rect(clr_r, 8., 8., &pill);
        pill.set_color(Color::from_rgb(28, 28, 30));
        c.draw_round_rect(done_r, 8., 8., &pill);
        text_styled(
            c,
            "清空",
            clr_r.left,
            clr_r.top + 3.5,
            clr_r.width(),
            10.,
            Color::from_rgb(28, 28, 30),
            Weight::BOLD,
            TextAlign::Center,
            1,
        );
        text_styled(
            c,
            "完成",
            done_r.left,
            done_r.top + 3.5,
            done_r.width(),
            10.,
            Color::WHITE,
            Weight::BOLD,
            TextAlign::Center,
            1,
        );
        hits.push(HitItem::new(clr_r, "清空笔记", Action::NoteClear(card.id)));
        hits.push(HitItem::new(done_r, "完成编辑", Action::NoteFinish));
    } else if !base_text.is_empty() {
        let copy_r = Rect::from_xywh(r.right - 66., r.top + 5., 56., 24.);
        let copy_hov = v.hover.is_some_and(|p| copy_r.contains(Point::from(p)));
        let mut cp = Paint::default();
        cp.set_anti_alias(true);
        cp.set_color(Color::from_argb(if copy_hov { 54 } else { 28 }, 28, 28, 30));
        c.draw_round_rect(copy_r, 8., 8., &cp);
        text_styled(
            c,
            "复制",
            copy_r.left,
            copy_r.top + 4.5,
            copy_r.width(),
            10.5,
            Color::from_rgb(48, 38, 6),
            Weight::BOLD,
            TextAlign::Center,
            1,
        );
        hits.push(HitItem::new(copy_r, "复制全文", Action::Copy(base_text.to_string())));
    }

    // 笔记本横格细线
    let mut rule = Paint::default();
    rule.set_color(Color::from_argb(if v.dark { 24 } else { 18 }, 120, 120, 128));
    let rule_count = ((r.height() - 68.) / 24.).floor().max(1.) as usize;
    for k in 0..rule_count {
        let ly = r.top + 66. + k as f32 * 24.;
        if ly < r.bottom - 20. {
            c.draw_line((x, ly), (x + w, ly), &rule);
        }
    }

    let max_lines = if large { 11 } else { 4 };
    if is_editing {
        let composed = format!("{base_text}{}|", v.note_preedit);
        text_styled(c, &composed, x, r.top + 44., w, 12.5, fg, Weight::MEDIUM, TextAlign::Left, max_lines);
        text_styled(
            c,
            "直接键盘/中文输入 · Enter 换行 · Esc 保存",
            x,
            r.bottom - 18.,
            w,
            9.,
            Color::from_rgb(217, 119, 6),
            Weight::SEMI_BOLD,
            TextAlign::Right,
            1,
        );
        let mut focus_rim = Paint::default();
        focus_rim.set_anti_alias(true);
        focus_rim.set_style(Style::Stroke);
        focus_rim.set_stroke_width(2.0);
        focus_rim.set_color(Color::from_rgb(255, 149, 0));
        c.draw_round_rect(r.with_outset((-1., -1.)), 21., 21., &focus_rim);
    } else if !base_text.is_empty() {
        text_styled(c, base_text, x, r.top + 44., w, 12.5, fg, Weight::MEDIUM, TextAlign::Left, max_lines);
        text_styled(c, "双击写入", x, r.bottom - 18., w, 9., sub, Weight::NORMAL, TextAlign::Right, 1);
    } else {
        text_styled(c, default_hint, x, r.top + 46., w, 12., sub, Weight::NORMAL, TextAlign::Left, 3);
        text_styled(c, "双击写入", x, r.bottom - 18., w, 9., sub, Weight::NORMAL, TextAlign::Right, 1);
    }
}

/// 统一绘制列表项圆圈复选框（待办组件与 2×2 日历底部共用）。
fn draw_todo_row(
    c: &Canvas,
    row_rect: Rect,
    title: &str,
    time: &str,
    done: bool,
    idx: usize,
    fg: Color,
    sub: Color,
    v: &View<'_>,
    hits: &mut Vec<HitItem>,
) {
    let hovered = v.hover.is_some_and(|p| row_rect.contains(Point::from(p)));
    if hovered {
        glass::liquid_pill(c, row_rect, 9., v.dark, true);
    }
    let cx = row_rect.left + 12.;
    let cy = row_rect.center_y();
    let rad = 7.0;
    if done {
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        fill.set_color(accent_blue(v.dark));
        c.draw_circle((cx, cy), rad, &fill);
        let mut ck = Paint::default();
        ck.set_anti_alias(true);
        ck.set_style(Style::Stroke);
        ck.set_stroke_width(1.6);
        ck.set_stroke_cap(Cap::Round);
        ck.set_color(Color::WHITE);
        c.draw_line((cx - 3.0, cy + 0.2), (cx - 0.8, cy + 2.6), &ck);
        c.draw_line((cx - 0.8, cy + 2.6), (cx + 3.4, cy - 2.2), &ck);
    } else {
        let mut ring = Paint::default();
        ring.set_anti_alias(true);
        ring.set_style(Style::Stroke);
        ring.set_stroke_width(1.5);
        ring.set_color(Color::from_argb(150, sub.r(), sub.g(), sub.b()));
        c.draw_circle((cx, cy), rad, &ring);
    }

    let time_w = if time.is_empty() { 0. } else { 42. };
    let text_col = if done { sub } else { fg };
    text_styled(
        c,
        title,
        row_rect.left + 26.,
        cy - 7.5,
        (row_rect.width() - 30. - time_w).max(40.),
        11.5,
        text_col,
        if done { Weight::NORMAL } else { Weight::MEDIUM },
        TextAlign::Left,
        1,
    );
    if !time.is_empty() {
        text_styled(
            c,
            time,
            row_rect.right - time_w,
            cy - 6.5,
            time_w - 4.,
            9.5,
            sub,
            Weight::MEDIUM,
            TextAlign::Right,
            1,
        );
    }
    hits.push(HitItem::new(
        row_rect,
        if done { "标为未完成" } else { "标记已完成" },
        Action::TodoToggle(idx),
    ));
}

pub fn card(
    c: &Canvas,
    r: Rect,
    card: &Card,
    v: &View<'_>,
    hits: &mut Vec<HitItem>,
    backdrop: Option<(&Image, (f32, f32))>,
) {
    if card.stack.len() >= 2 {
        glass::stack_depth(c, r, card.stack.len(), v.dark);
    }

    let active = card.active();
    let (fill_top, fill_bot) = apple_palette(active, v.dark);
    glass::apple_card_shell(c, r, 22., v.dark, fill_top, fill_bot, backdrop);
    hits.push(HitItem::new(r, &card.title, Action::Card(card.id)));

    c.save();
    c.clip_rrect(skia_safe::RRect::new_rect_xy(r, 22., 22.), None, true);

    let fg = ink(v.dark);
    let sub = muted(v.dark);
    let small = r.width() < 240. && r.height() < 240.;
    let large = r.height() > 240.;
    let stack_pad = if card.stack.len() >= 2 { 14.0 } else { 0.0 };
    let x = r.left + 16.;
    let y = r.top + 14.;
    let w = (r.width() - 32. - stack_pad).max(40.);

    if !v.data.live.is_null() && matches!(active,"weather"|"music"|"todo"|"focus"|"screen_time"|"focus_stats"|"today"|"wireless"|"system"|"storage"|"schedule"|"anniversary"|"activity"|"converter"|"github_grid") {
        live_card(c,r,card,v,x,y,w,fg,sub,hits);
        c.restore();return;
    }
    match active {
        "analog_clock" => {
            // 1:1 复刻 Apple Clock 原生圆表盘（深色外框 + 白/深灰表盘 + 12 个清晰数字 + Apple 亮橙连续扫秒针）
            let (_, mon, day, dow, hr, min, s_f) = super::sidebar::local_datetime_precise();
            let sec = s_f.floor() as u32;
            let rad = if large {
                (w.min(r.height() - 28.) * 0.45).clamp(54., 128.)
            } else {
                ((r.height() - 20.) * 0.48).clamp(50., 76.)
            };
            let cx = if small || large { r.center_x() } else { x + rad + 6. };
            let cy = r.center_y();

            let mut dial = Paint::default();
            dial.set_anti_alias(true);
            dial.set_color(if v.dark {
                Color::from_rgb(44, 44, 46)
            } else {
                Color::WHITE
            });
            c.draw_circle((cx, cy), rad, &dial);

            // 60 道精致刻度
            let dial_ink = if v.dark { Color::WHITE } else { Color::from_rgb(28, 28, 30) };
            for i in 0..60 {
                let ang = i as f32 * TAU / 60.0 - std::f32::consts::FRAC_PI_2;
                let major = i % 5 == 0;
                let r_out = rad - 3.0;
                let r_in = rad - if major { 7.2 } else { 4.8 };
                let col = Color::from_argb(
                    if major { 215 } else { 55 },
                    dial_ink.r(),
                    dial_ink.g(),
                    dial_ink.b(),
                );
                draw_line(
                    c,
                    cx + ang.cos() * r_in,
                    cy + ang.sin() * r_in,
                    cx + ang.cos() * r_out,
                    cy + ang.sin() * r_out,
                    if major { 1.4 } else { 0.7 },
                    col,
                );
            }

            // 12 个清晰苹果数字 12..11（适当内缩避开外圈刻度）
            let num_r = rad - if large { 24. } else { 18.0 };
            let num_size = if large { 18. } else { 11.8 };
            for n in 1..=12 {
                let ang = n as f32 * TAU / 12.0 - std::f32::consts::FRAC_PI_2;
                let nx = cx + ang.cos() * num_r;
                let ny = cy + ang.sin() * num_r;
                text_styled(
                    c,
                    &n.to_string(),
                    nx - 14.,
                    ny - num_size * 0.62,
                    28.,
                    num_size,
                    dial_ink,
                    Weight::SEMI_BOLD,
                    TextAlign::Center,
                    1,
                );
            }

            let m_f = min as f32 + s_f / 60.;
            let h_f = (hr % 12) as f32 + m_f / 60.;
            let ah = h_f * TAU / 12. - std::f32::consts::FRAC_PI_2;
            let am = m_f * TAU / 60. - std::f32::consts::FRAC_PI_2;
            let a_s = super::sidebar::sweep_second_angle(s_f);

            // Apple 经典指针：细颈底座 + 加粗圆头棒
            draw_line(c, cx, cy, cx + ah.cos() * rad * 0.30, cy + ah.sin() * rad * 0.30, 1.8, dial_ink);
            draw_line(c, cx + ah.cos() * rad * 0.14, cy + ah.sin() * rad * 0.14, cx + ah.cos() * rad * 0.46, cy + ah.sin() * rad * 0.46, 3.8, dial_ink);

            draw_line(c, cx, cy, cx + am.cos() * rad * 0.34, cy + am.sin() * rad * 0.34, 1.8, dial_ink);
            draw_line(c, cx + am.cos() * rad * 0.14, cy + am.sin() * rad * 0.14, cx + am.cos() * rad * 0.72, cy + am.sin() * rad * 0.72, 3.0, dial_ink);

            let orange = Color::from_rgb(255, 149, 0);
            draw_line(
                c,
                cx - a_s.cos() * rad * 0.18,
                cy - a_s.sin() * rad * 0.18,
                cx + a_s.cos() * rad * 0.84,
                cy + a_s.sin() * rad * 0.84,
                1.25,
                orange,
            );
            let mut pin = Paint::default();
            pin.set_anti_alias(true);
            pin.set_color(orange);
            c.draw_circle((cx, cy), 3.4, &pin);
            pin.set_color(if v.dark { Color::from_rgb(44, 44, 46) } else { Color::WHITE });
            c.draw_circle((cx, cy), 1.3, &pin);

            if !small && !large {
                let wk = ["日", "一", "二", "三", "四", "五", "六"][dow.min(6) as usize];
                let rx = cx + rad + 20.;
                let rw = (x + w - rx).max(60.);
                text_styled(c, "北京", rx, y + 26., rw, 13., orange, Weight::SEMI_BOLD, TextAlign::Left, 1);
                text_styled(c, &format!("{hr:02}:{min:02}:{sec:02}"), rx, y + 48., rw, 26., Color::WHITE, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &format!("今天 · {mon}月{day}日 周{wk}"), rx, y + 84., rw, 11., Color::from_rgb(152, 152, 157), Weight::NORMAL, TextAlign::Left, 1);
            }
        }
        "weather" => {
            // 1:1 复刻 Apple Weather（蓝天渐变 + 实心定位小箭头 + 大号轻盈温度 + 暖阳 + 6 时段预报）
            glass::glow_blob(c, r, 22., (255, 220, 120), 0.88, 0.12, 0.55, if v.dark { 22 } else { 42 });

            let white = Color::WHITE;
            let white_sub = Color::from_argb(215, 255, 255, 255);
            if small {
                text_styled(c, "上海", x, y, 42., 13.5, white, Weight::SEMI_BOLD, TextAlign::Left, 1);
                draw_location_arrow(c, x + 31., y + 4., white);
                text_styled(c, "22°", x - 2., y + 18., w, 42., white, Weight::NORMAL, TextAlign::Left, 1);
                draw_apple_weather_icon(c, x + 10., r.bottom - 56., 1.0, false);
                text_styled(c, "晴朗", x, r.bottom - 44., w, 12.5, white, Weight::SEMI_BOLD, TextAlign::Left, 1);
                text_styled(c, "最高26° 最低16°", x, r.bottom - 27., w, 11., white_sub, Weight::MEDIUM, TextAlign::Left, 1);
            } else {
                text_styled(c, "上海", x, y, 42., 14., white, Weight::SEMI_BOLD, TextAlign::Left, 1);
                draw_location_arrow(c, x + 33., y + 4.5, white);
                text_styled(c, "22°", x - 2., y + 18., 120., 40., white, Weight::NORMAL, TextAlign::Left, 1);

                draw_apple_weather_icon(c, x + w - 12., y + 12., 1.15, false);
                text_styled(c, "晴朗", x + 120., y + 26., w - 120., 12.5, white, Weight::SEMI_BOLD, TextAlign::Right, 1);
                text_styled(c, "最高26° 最低16°", x + 120., y + 43., w - 120., 11., white_sub, Weight::MEDIUM, TextAlign::Right, 1);

                // Apple Weather 底部 6 列每小时预报
                let hours = [
                    ("14时", "22°", false),
                    ("15时", "24°", false),
                    ("16时", "26°", false),
                    ("17时", "23°", true),
                    ("18时", "20°", true),
                    ("19时", "18°", true),
                ];
                let hy = if large { y + 104. } else { r.bottom - 62. };
                let cw = w / hours.len() as f32;
                for (i, (hl, ht, cloud)) in hours.into_iter().enumerate() {
                    let cx = x + cw * (i as f32 + 0.5);
                    text_styled(c, hl, cx - cw * 0.5, hy, cw, 10., white_sub, Weight::MEDIUM, TextAlign::Center, 1);
                    draw_apple_weather_icon(c, cx, hy + 24., 0.72, cloud);
                    text_styled(c, ht, cx - cw * 0.5, hy + 38., cw, 11.5, white, Weight::SEMI_BOLD, TextAlign::Center, 1);
                }
                if large {
                    let days = [("明天", "15°", "25°", false), ("周日", "14°", "23°", true), ("周一", "12°", "20°", true)];
                    let mut div = Paint::default();
                    div.set_color(Color::from_argb(48, 255, 255, 255));
                    for (i, (dl, lo, hi, cl)) in days.into_iter().enumerate() {
                        let ry = hy + 72. + i as f32 * 52.;
                        c.draw_line((x, ry), (x + w, ry), &div);
                        text_styled(c, dl, x, ry + 16., 64., 12.5, white, Weight::SEMI_BOLD, TextAlign::Left, 1);
                        draw_apple_weather_icon(c, x + 98., ry + 24., 0.85, cl);
                        text_styled(c, lo, x + 150., ry + 16., 40., 12., white_sub, Weight::MEDIUM, TextAlign::Right, 1);
                        draw_line(c, x + 202., ry + 24., x + w - 46., ry + 24., 4., Color::from_argb(60, 255, 255, 255));
                        draw_line(c, x + 218., ry + 24., x + w - 58., ry + 24., 4., Color::from_rgb(255, 204, 0));
                        text_styled(c, hi, x + w - 40., ry + 16., 40., 12.5, white, Weight::SEMI_BOLD, TextAlign::Right, 1);
                    }
                }
            }
        }
        "system" => {
            // 1:1 复刻 Apple Batteries 四圆环小组件（纯净白/炭黑底 + 厚实系统色圆环 + 中心图标 + 百分比）
            let m = &v.data.metrics;
            let cpu = m.cpu.unwrap_or(27.);
            let mem = m.memory.unwrap_or(54.);
            let disk = m.disk.unwrap_or(55.);
            let bat = m.battery.unwrap_or(86) as f32;
            let track_col = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            let items = [
                ("CPU", cpu, Color::from_rgb(0, 122, 255)),
                ("内存", mem, Color::from_rgb(52, 199, 89)),
                ("磁盘", disk, Color::from_rgb(255, 149, 0)),
                ("电量", bat, Color::from_rgb(48, 209, 88)),
            ];

            if small {
                let cw = w / 2.;
                let rh = (r.height() - 28.) / 2.;
                let rad = (rh * 0.30).clamp(15., 18.);
                for (i, (lbl, val, col)) in items.into_iter().enumerate() {
                    let cx = x + (i % 2) as f32 * cw + cw * 0.5;
                    let cy = y + 4. + (i / 2) as f32 * rh + rad;
                    draw_apple_ring(c, cx, cy, rad, 4.4, val / 100., col, track_col);
                    text_styled(c, &format!("{val:.0}%"), cx - cw * 0.5, cy - 7.5, cw, 11., fg, Weight::BOLD, TextAlign::Center, 1);
                    text_styled(c, lbl, cx - cw * 0.5, cy + rad + 4., cw, 9.5, sub, Weight::MEDIUM, TextAlign::Center, 1);
                }
            } else {
                let net = format!("↓{:.0}K · ↑{:.0}K", m.download / 1024., m.upload / 1024.);
                text_styled(c, "电池与性能", x, y, 120., 11., sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
                text_styled(c, &net, x + 120., y, (w - 120.).max(60.), 10., sub, Weight::NORMAL, TextAlign::Right, 1);

                let cw = w / 4.;
                let rad = 26.0;
                let cy = y + 32. + rad;
                for (i, (lbl, val, col)) in items.into_iter().enumerate() {
                    let cx = x + i as f32 * cw + cw * 0.5;
                    draw_apple_ring(c, cx, cy, rad, 5.8, val / 100., col, track_col);
                    let mut gp = Paint::default();
                    gp.set_anti_alias(true);
                    gp.set_style(Style::Stroke);
                    gp.set_stroke_width(1.6);
                    gp.set_stroke_cap(Cap::Round);
                    gp.set_color(fg);
                    match i {
                        0 => {
                            c.draw_round_rect(Rect::from_xywh(cx - 5.5, cy - 5.5, 11., 11.), 2.2, 2.2, &gp);
                        }
                        1 => {
                            c.draw_round_rect(Rect::from_xywh(cx - 7., cy - 4., 14., 8.), 2., 2., &gp);
                            c.draw_line((cx - 2.5, cy - 1.5), (cx - 2.5, cy + 1.5), &gp);
                            c.draw_line((cx + 2.5, cy - 1.5), (cx + 2.5, cy + 1.5), &gp);
                        }
                        2 => {
                            c.draw_round_rect(Rect::from_xywh(cx - 6., cy - 5., 12., 10.), 2.5, 2.5, &gp);
                            c.draw_line((cx - 3., cy + 2.), (cx + 3., cy + 2.), &gp);
                        }
                        _ => {
                            c.draw_round_rect(Rect::from_xywh(cx - 7.5, cy - 4.2, 13.5, 8.4), 2., 2., &gp);
                            gp.set_style(Style::Fill);
                            gp.set_color(col);
                            c.draw_round_rect(Rect::from_xywh(cx - 5.5, cy - 2.4, 9.5 * (val / 100.).clamp(0.2, 1.), 4.8), 1., 1., &gp);
                        }
                    }
                    text_styled(c, &format!("{val:.0}%"), cx - cw * 0.5, cy + rad + 10., cw, 16., fg, Weight::SEMI_BOLD, TextAlign::Center, 1);
                    text_styled(c, lbl, cx - cw * 0.5, cy + rad + 30., cw, 9.5, sub, Weight::MEDIUM, TextAlign::Center, 1);
                }
                if large {
                    let list_y = cy + rad + 60.;
                    for (i, (lbl, val, col)) in items.into_iter().enumerate() {
                        let ry = list_y + i as f32 * 40.;
                        text_styled(c, lbl, x, ry, 52., 12., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                        draw_line(c, x + 56., ry + 8., x + w - 48., ry + 8., 6., track_col);
                        draw_line(c, x + 56., ry + 8., x + 56. + (w - 104.) * (val / 100.).clamp(0.05, 1.), ry + 8., 6., col);
                        text_styled(c, &format!("{val:.0}%"), x + w - 42., ry, 42., 12., fg, Weight::BOLD, TextAlign::Right, 1);
                    }
                }
            }
        }
        "music" => {
            // 1:1 复刻 Apple Music（玫瑰绯红渐变底 + 现代封面艺术 + 白色双音符 + 2×2 即将播放列表）
            let white = Color::WHITE;
            let white_sub = Color::from_argb(205, 255, 255, 255);
            draw_music_note(c, x + w - 16., y + 2., 1.15, white_sub);

            if small {
                let csz = 64.0;
                draw_album_cover(c, Rect::from_xywh(x, y, csz, csz), 12., 0);
                text_styled(c, "夜色钢琴曲", x, r.bottom - 48., w, 14., white, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, "轻音乐 · 空间音频", x, r.bottom - 29., w, 11., white_sub, Weight::MEDIUM, TextAlign::Left, 1);
            } else {
                let csz = if large { 116. } else { (r.height() - 32.).clamp(90., 128.) };
                draw_album_cover(c, Rect::from_xywh(x, y + 2., csz, csz), 14., 0);
                let tx = x + csz + 16.;
                let tw = (x + w - tx - 18.).max(80.);
                text_styled(c, "夜色钢琴曲", tx, y + 10., tw, if large { 18. } else { 15.5 }, white, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, "林屿 · 空间音频", tx, y + if large { 36. } else { 32. }, tw, 12., white_sub, Weight::MEDIUM, TextAlign::Left, 1);

                // Apple Music 经典极简白色细进度条 + 三键播放控制
                let prog_y = y + csz - 36.;
                draw_line(c, tx, prog_y, x + w - 6., prog_y, 3.0, Color::from_argb(72, 255, 255, 255));
                draw_line(c, tx, prog_y, tx + (x + w - 6. - tx) * 0.42, prog_y, 3.0, white);
                let cy = y + csz - 12.;
                let pcx = tx + (x + w - tx) * 0.46;
                let mut pp = Paint::default();
                pp.set_anti_alias(true);
                pp.set_color(white);
                c.draw_round_rect(Rect::from_xywh(pcx - 5., cy - 7., 3.5, 14.), 1.2, 1.2, &pp);
                c.draw_round_rect(Rect::from_xywh(pcx + 1.5, cy - 7., 3.5, 14.), 1.2, 1.2, &pp);
                let mut np = Paint::default();
                np.set_anti_alias(true);
                np.set_style(Style::Stroke);
                np.set_stroke_width(2.0);
                np.set_stroke_cap(Cap::Round);
                np.set_color(white_sub);
                c.draw_line((pcx - 26., cy - 5.), (pcx - 34., cy), &np);
                c.draw_line((pcx - 34., cy), (pcx - 26., cy + 5.), &np);
                c.draw_line((pcx + 26., cy - 5.), (pcx + 34., cy), &np);
                c.draw_line((pcx + 34., cy), (pcx + 26., cy + 5.), &np);

                if large {
                    text_styled(c, "即将播放", x, y + csz + 24., w, 12.5, white_sub, Weight::BOLD, TextAlign::Left, 1);
                    let tracks = [
                        ("风过林梢", "晨光合奏团", "03:42", 1),
                        ("海风与星辰", "静谧弦乐", "04:18", 2),
                        ("雨落窗檐", "纯粹人声", "02:56", 0),
                    ];
                    let mut div = Paint::default();
                    div.set_color(Color::from_argb(46, 255, 255, 255));
                    for (i, (title, artist, dur, seed)) in tracks.into_iter().enumerate() {
                        let ry = y + csz + 48. + i as f32 * 56.;
                        c.draw_line((x, ry), (x + w, ry), &div);
                        draw_album_cover(c, Rect::from_xywh(x, ry + 10., 38., 38.), 8., seed);
                        text_styled(c, title, x + 50., ry + 11., w - 102., 13.5, white, Weight::SEMI_BOLD, TextAlign::Left, 1);
                        text_styled(c, artist, x + 50., ry + 30., w - 102., 10.5, white_sub, Weight::NORMAL, TextAlign::Left, 1);
                        text_styled(c, dur, x + w - 48., ry + 20., 48., 11., white_sub, Weight::MEDIUM, TextAlign::Right, 1);
                    }
                }
            }
        }
        "focus" => {
            // 1:1 复刻 Apple Timer 原生计时器（纯净白/炭黑底 + 亮橙环 + 环心大号倒计时 + 底部分段胶囊）
            let orange = Color::from_rgb(255, 149, 0);
            let track_col = Color::from_argb(if v.dark { 42 } else { 24 }, 120, 120, 128);
            let frac = if v.focus_total > 0 {
                (1.0 - (v.focus_left as f32 / v.focus_total as f32).clamp(0., 1.)).max(0.08)
            } else {
                0.25
            };
            let mins = v.focus_left / 60;
            let secs = v.focus_left % 60;
            let (_, _, _, _, hr, min_now, _) = super::sidebar::local_datetime();
            let end_total = hr * 60 + min_now + mins.max(1);
            let end_str = format!("{:02}:{:02}", (end_total / 60) % 24, end_total % 60);

            text_styled(c, "计时器", x, y, 60., 11., sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
            text_styled(c, &end_str, x + 60., y, (w - 60.).max(40.), 11., orange, Weight::SEMI_BOLD, TextAlign::Right, 1);

            let ring_r = if small { 38. } else if large { 82. } else { 42. };
            let cx = if small || large { r.center_x() } else { x + ring_r + 12. };
            let cy = if large { y + 122. } else { y + 60. };
            draw_apple_ring(c, cx, cy, ring_r, if large { 8. } else { 5.2 }, frac, orange, track_col);

            let clock_str = format!("{mins:02}:{secs:02}");
            text_styled(
                c,
                &clock_str,
                cx - ring_r,
                cy - if large { 20. } else { 11.5 },
                ring_r * 2.,
                if large { 32. } else { 18.5 },
                fg,
                Weight::SEMI_BOLD,
                TextAlign::Center,
                1,
            );
            text_styled(
                c,
                if v.focus_running { "暂停" } else { "开始" },
                cx - ring_r,
                cy + if large { 18. } else { 10. },
                ring_r * 2.,
                9.5,
                orange,
                Weight::BOLD,
                TextAlign::Center,
                1,
            );
            hits.push(HitItem::new(
                Rect::from_xywh(cx - ring_r, cy - ring_r, ring_r * 2., ring_r * 2.),
                if v.focus_running { "暂停专注" } else { "开始专注" },
                Action::FocusToggle,
            ));

            let bw = if small || large { w } else { (x + w - (cx + ring_r + 18.)).max(120.) };
            let bx = if small || large { x } else { cx + ring_r + 18. };
            let by = r.bottom - 34.;
            glass::liquid_pill(c, Rect::from_xywh(bx, by, bw, 22.), 11., v.dark, false);
            let presets = [5_u32, 15, 25, 45];
            let pw = (bw - 4.) / 4.;
            for (idx, pm) in presets.into_iter().enumerate() {
                let pr = Rect::from_xywh(bx + 2. + idx as f32 * pw, by + 2., pw, 18.);
                let active_preset = v.focus_total == pm * 60;
                if active_preset {
                    glass::liquid_pill(c, pr, 9., v.dark, true);
                }
                text_styled(
                    c,
                    &pm.to_string(),
                    pr.left,
                    pr.top + 2.,
                    pw,
                    9.8,
                    if active_preset { fg } else { sub },
                    if active_preset { Weight::BOLD } else { Weight::MEDIUM },
                    TextAlign::Center,
                    1,
                );
                hits.push(HitItem::new(pr, &format!("设为 {pm} 分钟"), Action::FocusPreset(pm)));
            }
        }
        "calendar" => {
            // 1:1 复刻 Apple Calendar 原生日历（Apple 红月份 + 7 列规整小月历 + 右侧彩色左竖条日程）
            let (yr, mon, day, dow, _, _, _) = super::sidebar::local_datetime();
            let red = Color::from_rgb(255, 59, 48);
            let wk_name = ["星期日", "星期一", "星期二", "星期三", "星期四", "星期五", "星期六"][dow.min(6) as usize];

            if small {
                text_styled(c, wk_name, x, y, w, 12.5, red, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &day.to_string(), x - 2., y + 18., w, 46., fg, Weight::LIGHT, TextAlign::Left, 1);
                let mut bar = Paint::default();
                bar.set_anti_alias(true);
                bar.set_color(Color::from_argb(if v.dark { 40 } else { 22 }, 0, 122, 255));
                c.draw_round_rect(Rect::from_xywh(x, r.bottom - 52., w, 36.), 8., 8., &bar);
                bar.set_color(Color::from_rgb(0, 122, 255));
                c.draw_round_rect(Rect::from_xywh(x + 4., r.bottom - 47., 3., 26.), 1.5, 1.5, &bar);
                text_styled(c, "界面评审", x + 13., r.bottom - 47., w - 18., 11., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                text_styled(c, "15:00 – 16:00", x + 13., r.bottom - 31., w - 18., 9.5, sub, Weight::NORMAL, TextAlign::Left, 1);
            } else {
                let lw = (w * 0.48).max(146.);
                text_styled(c, &format!("{mon}月"), x, y, lw, 13., red, Weight::BOLD, TextAlign::Left, 1);
                let cell = lw / 7.;
                let step = if large { 20. } else { 18. };
                let gy = y + 20.;
                for (i, n) in ["日", "一", "二", "三", "四", "五", "六"].iter().enumerate() {
                    text_styled(c, n, x + i as f32 * cell, gy, cell, 8.2, sub, Weight::SEMI_BOLD, TextAlign::Center, 1);
                }
                let days_in_mon = match mon {
                    1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
                    4 | 6 | 9 | 11 => 30,
                    2 => if (yr % 400 == 0) || (yr % 4 == 0 && yr % 100 != 0) { 29 } else { 28 },
                    _ => 30,
                };
                let first_dow = (dow as i32 - ((day as i32 - 1) % 7)).rem_euclid(7) as u32;
                for d in 1..=days_in_mon {
                    let slot = first_dow + d - 1;
                    let col = slot % 7;
                    let row = slot / 7;
                    let xx = x + col as f32 * cell;
                    let yy = gy + 15. + row as f32 * step;
                    let today = d == day;
                    if today {
                        let mut cp = Paint::default();
                        cp.set_anti_alias(true);
                        cp.set_color(red);
                        c.draw_circle((xx + cell * 0.5, yy + step * 0.5 - 1.), step * 0.42, &cp);
                    }
                    text_styled(
                        c,
                        &d.to_string(),
                        xx,
                        yy + 1.,
                        cell,
                        9.2,
                        if today { Color::WHITE } else { fg },
                        if today { Weight::BOLD } else { Weight::MEDIUM },
                        TextAlign::Center,
                        1,
                    );
                }

                // 右半栏：Apple Calendar 彩色左竖条日程列表
                let rx = x + lw + 14.;
                let rw = (x + w - rx).max(110.);
                text_styled(c, "今日日程", rx, y, rw, 11., sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
                let events = [
                    ("界面评审", "15:00", Color::from_rgb(0, 122, 255)),
                    ("版本验收", "17:30", Color::from_rgb(255, 149, 0)),
                    ("晚间运动", "20:00", Color::from_rgb(52, 199, 89)),
                ];
                for (i, (title, tm, col)) in events.into_iter().enumerate() {
                    let ey = y + 24. + i as f32 * 40.;
                    let mut bg = Paint::default();
                    bg.set_anti_alias(true);
                    bg.set_color(Color::from_argb(if v.dark { 40 } else { 22 }, col.r(), col.g(), col.b()));
                    c.draw_round_rect(Rect::from_xywh(rx, ey, rw, 34.), 8., 8., &bg);
                    bg.set_color(col);
                    c.draw_round_rect(Rect::from_xywh(rx + 4., ey + 5., 3., 24.), 1.5, 1.5, &bg);
                    text_styled(c, title, rx + 13., ey + 4., rw - 16., 11., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, tm, rx + 13., ey + 18., rw - 16., 9.5, sub, Weight::NORMAL, TextAlign::Left, 1);
                }

                // 2×2 特大号下半区：明日日程 + 待办提醒联动（彻底消除底部留白）
                if large {
                    let split_y = y + 168.;
                    let mut div = Paint::default();
                    div.set_color(Color::from_argb(if v.dark { 34 } else { 22 }, 120, 120, 128));
                    c.draw_line((x, split_y), (x + w, split_y), &div);

                    let bot_y = split_y + 12.;
                    // 左下栏：明日日程 & 周次状态胶囊
                    text_styled(c, "明日日程", x, bot_y, lw, 10.5, sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    let tomorrow = [
                        ("产品设计同步", "10:00", Color::from_rgb(88, 86, 214)),
                        ("版本发布走查", "14:30", Color::from_rgb(175, 82, 222)),
                    ];
                    for (i, (title, tm, col)) in tomorrow.into_iter().enumerate() {
                        let ey = bot_y + 22. + i as f32 * 42.;
                        let mut bg = Paint::default();
                        bg.set_anti_alias(true);
                        bg.set_color(Color::from_argb(if v.dark { 40 } else { 22 }, col.r(), col.g(), col.b()));
                        c.draw_round_rect(Rect::from_xywh(x, ey, lw, 35.), 8., 8., &bg);
                        bg.set_color(col);
                        c.draw_round_rect(Rect::from_xywh(x + 4., ey + 5., 3., 25.), 1.5, 1.5, &bg);
                        text_styled(c, title, x + 13., ey + 4.5, lw - 16., 11., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                        text_styled(c, tm, x + 13., ey + 18.5, lw - 16., 9.5, sub, Weight::NORMAL, TextAlign::Left, 1);
                    }
                    let week_of_month = (day.saturating_sub(1) / 7) + 1;
                    let pill_r = Rect::from_xywh(x, (r.bottom - 38.).max(bot_y + 110.), lw, 24.);
                    glass::liquid_pill(c, pill_r, 12., v.dark, false);
                    text_styled(
                        c,
                        &format!("{mon}月第 {week_of_month} 周 · 今日 3 项安排"),
                        pill_r.left + 8.,
                        pill_r.top + 5.5,
                        pill_r.width() - 16.,
                        9.5,
                        sub,
                        Weight::MEDIUM,
                        TextAlign::Center,
                        1,
                    );

                    // 右下栏：待办提醒联动列表（点击可直接勾选）
                    let pending_cnt = v.todos.items.iter().filter(|t| !t.done).count();
                    text_styled(c, "待办提醒", rx, bot_y, rw - 46., 10.5, sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    let badge_r = Rect::from_xywh(rx + rw - 44., bot_y - 1., 44., 17.);
                    let mut bp = Paint::default();
                    bp.set_anti_alias(true);
                    bp.set_color(Color::from_argb(if v.dark { 48 } else { 26 }, 255, 59, 48));
                    c.draw_round_rect(badge_r, 8.5, 8.5, &bp);
                    text_styled(
                        c,
                        &format!("{pending_cnt} 待办"),
                        badge_r.left,
                        badge_r.top + 2.,
                        badge_r.width(),
                        9.,
                        red,
                        Weight::BOLD,
                        TextAlign::Center,
                        1,
                    );

                    for (idx, item) in v.todos.items.iter().take(3).enumerate() {
                        let ry = bot_y + 22. + idx as f32 * 42.;
                        if ry + 36. > r.bottom - 6. {
                            break;
                        }
                        draw_todo_row(
                            c,
                            Rect::from_xywh(rx, ry, rw, 36.),
                            &item.title,
                            &item.time,
                            item.done,
                            idx,
                            fg,
                            sub,
                            v,
                            hits,
                        );
                    }
                }
            }
        }
        "activity" => {
            // 1:1 复刻 Apple Fitness 原生健身三环（标志性炭黑底 + 霓虹红/绿/青饱满同心三环）
            let ring_r = if large { 68. } else { 44. };
            let thick = if large { 12. } else { 8.8 };
            let gap = thick + 2.8;
            let cx = if small || large { r.center_x() } else { x + ring_r + 12. };
            let cy = if large { y + 96. } else { r.center_y() };

            let specs = [
                (ring_r, 0.77_f32, Color::from_rgb(250, 17, 79), "活动", "460/600 千卡"),
                (ring_r - gap, 0.75, Color::from_rgb(166, 255, 0), "锻炼", "34/45 分钟"),
                (ring_r - gap * 2., 0.83, Color::from_rgb(0, 245, 234), "站立", "10/12 小时"),
            ];
            for (rad_i, frac, col, _, _) in &specs {
                let track = Color::from_argb(46, col.r(), col.g(), col.b());
                draw_apple_ring(c, cx, cy, *rad_i, thick, *frac, *col, track);
            }
            if !small {
                let lx = if large { x } else { cx + ring_r + 22. };
                let lw = if large { w } else { (x + w - lx).max(100.) };
                let ly = if large { cy + ring_r + 22. } else { y + 14. };
                for (i, (_, _, col, name, val)) in specs.into_iter().enumerate() {
                    let row_y = ly + i as f32 * 40.;
                    text_styled(c, name, lx, row_y, lw, 10.5, Color::from_rgb(174, 174, 178), Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, val, lx, row_y + 15., lw, 14., col, Weight::BOLD, TextAlign::Left, 1);
                }
            }
        }
        "clock" => {
            // 1:1 复刻 Apple World Clock 四城小圆表盘（昼白夜黑严格区分）
            let (_, _, _, _, hr, min, _) = super::sidebar::local_datetime();
            let cities = [
                ("北京", "今天 +0", hr as i32, false),
                ("东京", "今天 +1", (hr as i32 + 1).rem_euclid(24), false),
                ("伦敦", "今天 -8", (hr as i32 - 8).rem_euclid(24), true),
                ("纽约", "今天 -13", (hr as i32 - 13).rem_euclid(24), true),
            ];
            let count = if small { 2 } else { 4 };
            let cw = w / count as f32;
            let rad = if small { 28. } else { 25. };
            let cy = y + 20. + rad;
            for (i, (name, diff, ch, night)) in cities.into_iter().take(count).enumerate() {
                let cx = x + cw * (i as f32 + 0.5);
                let mut dp = Paint::default();
                dp.set_anti_alias(true);
                dp.set_color(if night { Color::from_rgb(44, 44, 46) } else { Color::from_rgb(242, 242, 247) });
                c.draw_circle((cx, cy), rad, &dp);

                let hand_col = if night { Color::WHITE } else { Color::from_rgb(28, 28, 30) };
                let m_f = min as f32;
                let h_f = (ch % 12) as f32 + m_f / 60.;
                let ah = h_f * TAU / 12. - std::f32::consts::FRAC_PI_2;
                let am = m_f * TAU / 60. - std::f32::consts::FRAC_PI_2;
                draw_line(c, cx, cy, cx + ah.cos() * rad * 0.48, cy + ah.sin() * rad * 0.48, 2.2, hand_col);
                draw_line(c, cx, cy, cx + am.cos() * rad * 0.74, cy + am.sin() * rad * 0.74, 1.5, hand_col);
                dp.set_color(Color::from_rgb(255, 149, 0));
                c.draw_circle((cx, cy), 2.2, &dp);

                text_styled(c, name, cx - cw * 0.5, cy + rad + 10., cw, 11.5, fg, Weight::BOLD, TextAlign::Center, 1);
                text_styled(c, &format!("{ch:02}:{min:02}"), cx - cw * 0.5, cy + rad + 26., cw, 10.5, sub, Weight::MEDIUM, TextAlign::Center, 1);
                text_styled(c, diff, cx - cw * 0.5, cy + rad + 41., cw, 9., sub, Weight::NORMAL, TextAlign::Center, 1);
            }
            if large {
                let row_start = cy + rad + 72.;
                let mut div = Paint::default();
                div.set_color(Color::from_argb(if v.dark { 30 } else { 20 }, 120, 120, 128));
                for (i, (name, diff, ch, night)) in cities.into_iter().enumerate() {
                    let ry = row_start + i as f32 * 44.;
                    c.draw_line((x, ry), (x + w, ry), &div);
                    text_styled(c, name, x, ry + 12., 80., 13., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, if night { "夜间 · 次日" } else { "白天 · 本地" }, x + 84., ry + 13., 120., 10.5, sub, Weight::NORMAL, TextAlign::Left, 1);
                    text_styled(c, diff, x + w - 130., ry + 13., 64., 10.5, sub, Weight::MEDIUM, TextAlign::Right, 1);
                    text_styled(c, &format!("{ch:02}:{min:02}"), x + w - 58., ry + 11., 58., 14., fg, Weight::BOLD, TextAlign::Right, 1);
                }
            }
        }
        "clipboard" => {
            draw_note_card(
                c,
                r,
                card,
                "备忘录 · 剪贴板",
                "最近复制的文本会自动保存在这里，也可双击写入笔记",
                v.data.clipboard.first().map(String::as_str),
                x,
                w,
                large,
                fg,
                sub,
                v,
                hits,
            );
        }
        "note" => {
            draw_note_card(
                c,
                r,
                card,
                "便签",
                "双击卡片直接输入备忘内容，支持中文与换行",
                None,
                x,
                w,
                large,
                fg,
                sub,
                v,
                hits,
            );
        }
        "todo" => {
            // 1:1 复刻 Apple Reminders 提醒事项组件
            let total = v.todos.items.len();
            let pending = v.todos.items.iter().filter(|t| !t.done).count();
            let done_cnt = total.saturating_sub(pending);
            text_styled(c, "待办事项", x, y, 100., 12.5, accent_blue(v.dark), Weight::BOLD, TextAlign::Left, 1);
            text_styled(
                c,
                &pending.to_string(),
                x + w - 64.,
                y - 4.,
                40.,
                20.,
                fg,
                Weight::BOLD,
                TextAlign::Right,
                1,
            );
            text_styled(
                c,
                &format!("/{total}"),
                x + w - 24.,
                y + 2.,
                24.,
                10.5,
                sub,
                Weight::MEDIUM,
                TextAlign::Right,
                1,
            );

            if small {
                for (i, item) in v.todos.items.iter().take(3).enumerate() {
                    draw_todo_row(
                        c,
                        Rect::from_xywh(x - 4., y + 28. + i as f32 * 38., w + 8., 34.),
                        &item.title,
                        "",
                        item.done,
                        i,
                        fg,
                        sub,
                        v,
                        hits,
                    );
                }
            } else if large {
                for (i, item) in v.todos.items.iter().take(6).enumerate() {
                    let ry = y + 32. + i as f32 * 44.;
                    if ry + 38. > r.bottom - 42. {
                        break;
                    }
                    draw_todo_row(
                        c,
                        Rect::from_xywh(x - 4., ry, w + 8., 38.),
                        &item.title,
                        &item.time,
                        item.done,
                        i,
                        fg,
                        sub,
                        v,
                        hits,
                    );
                }
                let bar_y = r.bottom - 26.;
                let frac = if total > 0 { done_cnt as f32 / total as f32 } else { 0. };
                let track = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
                draw_line(c, x, bar_y + 6., x + w - 92., bar_y + 6., 6., track);
                if frac > 0.01 {
                    draw_line(
                        c,
                        x,
                        bar_y + 6.,
                        x + (w - 92.) * frac.clamp(0.04, 1.),
                        bar_y + 6.,
                        6.,
                        accent_blue(v.dark),
                    );
                }
                text_styled(
                    c,
                    &format!("已完成 {done_cnt}/{total}"),
                    x + w - 84.,
                    bar_y - 1.,
                    84.,
                    10.,
                    sub,
                    Weight::SEMI_BOLD,
                    TextAlign::Right,
                    1,
                );
            } else {
                // 2×1 双列展示最多 4 条
                let cw = (w - 10.) * 0.5;
                for (i, item) in v.todos.items.iter().take(4).enumerate() {
                    let col = i % 2;
                    let row = i / 2;
                    let rx = x - 2. + col as f32 * (cw + 12.);
                    let ry = y + 32. + row as f32 * 52.;
                    draw_todo_row(
                        c,
                        Rect::from_xywh(rx, ry, cw + 2., 46.),
                        &item.title,
                        &item.time,
                        item.done,
                        i,
                        fg,
                        sub,
                        v,
                        hits,
                    );
                }
            }
        }
        "ai_quota" => {
            // AI Token 套餐与中转站额度（含官方与 NewAPI/OneAPI 中转，严禁任何 Gemini/Google 条目）
            let snap = &v.data.ai_quota;
            text_styled(c, "AI 额度", x, y, 76., 12.5, fg, Weight::BOLD, TextAlign::Left, 1);
            if !small {
                text_styled(
                    c,
                    &snap.updated_label,
                    x + 76.,
                    y + 1.5,
                    (w - 134.).max(40.),
                    10.,
                    sub,
                    Weight::NORMAL,
                    TextAlign::Left,
                    1,
                );
            }
            let ref_r = Rect::from_xywh(r.right - 64., y - 3., 22., 22.);
            let cfg_r = Rect::from_xywh(r.right - 38., y - 3., 22., 22.);
            let ref_hov = v.hover.is_some_and(|p| ref_r.contains(Point::from(p)));
            let cfg_hov = v.hover.is_some_and(|p| cfg_r.contains(Point::from(p)));
            glass::liquid_circle(c, ref_r.center_x(), ref_r.center_y(), 11., v.dark, ref_hov);
            glass::liquid_circle(c, cfg_r.center_x(), cfg_r.center_y(), 11., v.dark, cfg_hov);
            text_styled(
                c,
                if snap.refreshing { "…" } else { "↻" },
                ref_r.left,
                ref_r.top + 3.5,
                22.,
                11.,
                if ref_hov { accent_blue(v.dark) } else { fg },
                Weight::BOLD,
                TextAlign::Center,
                1,
            );
            text_styled(
                c,
                "⚙",
                cfg_r.left,
                cfg_r.top + 3.5,
                22.,
                11.,
                if cfg_hov { accent_blue(v.dark) } else { fg },
                Weight::BOLD,
                TextAlign::Center,
                1,
            );
            hits.push(HitItem::new(ref_r, "刷新额度", Action::RefreshAiQuota));
            hits.push(HitItem::new(cfg_r, "配置 ai-tokens.json", Action::OpenAiQuotaConfig));

            let track = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            if small {
                for (i, e) in snap.entries.iter().take(2).enumerate() {
                    let sy = y + 32. + i as f32 * 52.;
                    let col = if e.ratio < 0.2 {
                        Color::from_rgb(255, 59, 48)
                    } else {
                        Color::from_rgb(e.color_rgb.0, e.color_rgb.1, e.color_rgb.2)
                    };
                    text_styled(c, &e.name, x, sy, w * 0.55, 11., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, &e.remaining_text, x + w * 0.55, sy, w * 0.45, 11., col, Weight::BOLD, TextAlign::Right, 1);
                    draw_line(c, x + 2., sy + 22., x + w - 2., sy + 22., 5., track);
                    draw_line(c, x + 2., sy + 22., x + 2. + (w - 4.) * e.ratio.clamp(0.05, 1.), sy + 22., 5., col);
                    text_styled(c, &e.tag, x, sy + 29., w, 9., sub, Weight::NORMAL, TextAlign::Left, 1);
                }
            } else if large {
                for (i, e) in snap.entries.iter().take(6).enumerate() {
                    let sy = y + 30. + i as f32 * 48.;
                    if sy + 42. > r.bottom - 24. {
                        break;
                    }
                    let col = if e.ratio < 0.2 {
                        Color::from_rgb(255, 59, 48)
                    } else {
                        Color::from_rgb(e.color_rgb.0, e.color_rgb.1, e.color_rgb.2)
                    };
                    text_styled(c, &e.name, x, sy, 130., 12., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, &e.tag, x + 134., sy + 1., 72., 9.5, sub, Weight::MEDIUM, TextAlign::Left, 1);
                    text_styled(c, &e.remaining_text, x + w - 124., sy, 124., 12., col, Weight::BOLD, TextAlign::Right, 1);
                    draw_line(c, x + 2., sy + 22., x + w - 2., sy + 22., 5., track);
                    draw_line(c, x + 2., sy + 22., x + 2. + (w - 4.) * e.ratio.clamp(0.05, 1.), sy + 22., 5., col);
                    text_styled(
                        c,
                        e.error.as_deref().unwrap_or(&e.detail_text),
                        x,
                        sy + 28.,
                        w,
                        9.2,
                        if e.error.is_some() { Color::from_rgb(255, 59, 48) } else { sub },
                        Weight::NORMAL,
                        TextAlign::Left,
                        1,
                    );
                }
                if snap.is_demo {
                    text_styled(c, "示例数据 · 点击右上 ⚙ 编辑 ai-tokens.json 接入真实密钥", x, r.bottom - 20., w, 9.2, sub, Weight::NORMAL, TextAlign::Center, 1);
                }
            } else {
                // 2×1 为 2×2 四宫格进度条展示
                let cw = (w - 14.) * 0.5;
                for (i, e) in snap.entries.iter().take(4).enumerate() {
                    let cx = x + (i % 2) as f32 * (cw + 14.);
                    let cy = y + 32. + (i / 2) as f32 * 56.;
                    let sr = Rect::from_xywh(cx, cy, cw, 48.);
                    let mut bg = Paint::default();
                    bg.set_anti_alias(true);
                    bg.set_color(Color::from_argb(if v.dark { 24 } else { 14 }, 120, 120, 128));
                    c.draw_round_rect(sr, 10., 10., &bg);

                    let col = if e.ratio < 0.2 {
                        Color::from_rgb(255, 59, 48)
                    } else {
                        Color::from_rgb(e.color_rgb.0, e.color_rgb.1, e.color_rgb.2)
                    };
                    text_styled(c, &e.name, cx + 8., cy + 6., cw * 0.54, 10.5, fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, &e.remaining_text, cx + cw * 0.54, cy + 6., cw * 0.46 - 8., 10.5, col, Weight::BOLD, TextAlign::Right, 1);
                    draw_line(c, cx + 10., cy + 28., cx + cw - 10., cy + 28., 4.5, track);
                    draw_line(c, cx + 10., cy + 28., cx + 10. + (cw - 20.) * e.ratio.clamp(0.05, 1.), cy + 28., 4.5, col);
                    text_styled(
                        c,
                        &format!("{} · {}", e.tag, e.detail_text),
                        cx + 8.,
                        cy + 34.,
                        cw - 16.,
                        8.5,
                        sub,
                        Weight::NORMAL,
                        TextAlign::Left,
                        1,
                    );
                }
            }
        }
        "file_shelf" => {
            // 文件中转站：支持外部拖入暂存与跨进程 OLE CF_HDROP 按住拖出
            let shelf = v.file_shelf;
            text_styled(c, "文件中转站", x, y, 82., 12.5, fg, Weight::BOLD, TextAlign::Left, 1);
            text_styled(
                c,
                &format!("{} 项", shelf.items.len()),
                x + 84.,
                y + 1.5,
                44.,
                10.,
                sub,
                Weight::MEDIUM,
                TextAlign::Left,
                1,
            );
            if !shelf.items.is_empty() && !small {
                let drag_r = Rect::from_xywh(r.right - 122., y - 3., 62., 22.);
                let clr_r = Rect::from_xywh(r.right - 56., y - 3., 40., 22.);
                let dh = v.hover.is_some_and(|p| drag_r.contains(Point::from(p)));
                let ch = v.hover.is_some_and(|p| clr_r.contains(Point::from(p)));
                glass::liquid_pill(c, drag_r, 11., v.dark, dh);
                glass::liquid_pill(c, clr_r, 11., v.dark, ch);
                text_styled(c, "拖出全部", drag_r.left, drag_r.top + 4., 62., 9.8, accent_blue(v.dark), Weight::SEMI_BOLD, TextAlign::Center, 1);
                text_styled(c, "清空", clr_r.left, clr_r.top + 4., 40., 9.8, sub, Weight::MEDIUM, TextAlign::Center, 1);
                hits.push(HitItem::new(drag_r, "按住拖出全部文件", Action::ShelfDragAll));
                hits.push(HitItem::new(clr_r, "清空中转站", Action::ShelfClear));
            }

            if v.file_hovering {
                let mut hr = Paint::default();
                hr.set_anti_alias(true);
                hr.set_style(Style::Stroke);
                hr.set_stroke_width(2.2);
                hr.set_color(accent_blue(v.dark));
                c.draw_round_rect(r.with_outset((-1., -1.)), 21., 21., &hr);
            }

            if shelf.items.is_empty() {
                let drop_r = Rect::from_xywh(x, y + 32., w, (r.bottom - y - 44.).max(54.));
                let mut dp = Paint::default();
                dp.set_anti_alias(true);
                dp.set_style(Style::Stroke);
                dp.set_stroke_width(1.2);
                dp.set_color(Color::from_argb(if v.file_hovering { 190 } else { 70 }, 0, 122, 255));
                c.draw_round_rect(drop_r, 14., 14., &dp);
                text_styled(
                    c,
                    if v.file_hovering { "松手放入中转站暂存" } else { "把文件拖到这里暂存\n按住条目可随时拖出到其他应用" },
                    drop_r.left + 12.,
                    drop_r.center_y() - 16.,
                    drop_r.width() - 24.,
                    11.,
                    if v.file_hovering { accent_blue(v.dark) } else { sub },
                    Weight::MEDIUM,
                    TextAlign::Center,
                    2,
                );
            } else {
                let max_show = if small { 2 } else if large { 6 } else { 3 };
                let row_h = if large { 42. } else { 34. };
                for (i, item) in shelf.items.iter().take(max_show).enumerate() {
                    let ry = y + 30. + i as f32 * (row_h + 6.);
                    if ry + row_h > r.bottom - 10. {
                        break;
                    }
                    let row_r = Rect::from_xywh(x, ry, w, row_h);
                    let rm_r = Rect::from_xywh(row_r.right - 24., ry + (row_h - 20.) * 0.5, 20., 20.);
                    let drag_item_r = Rect::from_xywh(x, ry, (w - 26.).max(40.), row_h);
                    let hov = v.hover.is_some_and(|p| row_r.contains(Point::from(p)));
                    glass::liquid_pill(c, row_r, 10., v.dark, hov);

                    let (br, bg_c, bb) = crate::file_shelf::ext_badge_color(&item.name, item.is_dir);
                    let mut bp = Paint::default();
                    bp.set_anti_alias(true);
                    bp.set_color(Color::from_rgb(br, bg_c, bb));
                    c.draw_round_rect(Rect::from_xywh(x + 8., ry + (row_h - 20.) * 0.5, 20., 20.), 6., 6., &bp);
                    let ext_tag = if item.is_dir {
                        "夹".to_string()
                    } else {
                        std::path::Path::new(&item.name)
                            .extension()
                            .and_then(|s| s.to_str())
                            .map(|s| s.chars().take(3).collect::<String>().to_uppercase())
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| "FILE".into())
                    };
                    text_styled(c, &ext_tag, x + 8., ry + (row_h - 11.) * 0.5, 20., 6.8, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);

                    let sz = if item.is_dir { "文件夹".into() } else { crate::file_shelf::format_size(item.size_bytes) };
                    let sz_w = if small { 0. } else { 54. };
                    text_styled(c, &item.name, x + 34., ry + (row_h - 14.) * 0.5, (w - 64. - sz_w).max(36.), 11.2, fg, Weight::MEDIUM, TextAlign::Left, 1);
                    if !small {
                        text_styled(c, &sz, row_r.right - 26. - sz_w, ry + (row_h - 12.) * 0.5, sz_w, 9.5, sub, Weight::NORMAL, TextAlign::Right, 1);
                    }
                    text_styled(c, "×", rm_r.left, rm_r.top + 3., 20., 11., sub, Weight::BOLD, TextAlign::Center, 1);
                    hits.push(HitItem::new(drag_item_r, "按住拖出文件", Action::ShelfDragItem(i)));
                    hits.push(HitItem::new(rm_r, "移出中转站", Action::ShelfRemove(i)));
                }
            }
        }
        "anniversary" => {
            // Apple 风格纪念日倒数
            let coral = Color::from_rgb(255, 55, 95);
            text_styled(c, "纪念日倒数", x, y, w, 11.5, coral, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, "2027 新年", x, y + 24., w, 13.5, fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
            text_styled(c, "35", x - 2., y + 48., 96., 44., fg, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, "天", x + 56., y + 74., 26., 13., sub, Weight::MEDIUM, TextAlign::Left, 1);
            let track = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            draw_line(c, x, r.bottom - 24., x + w, r.bottom - 24., 5., track);
            draw_line(c, x, r.bottom - 24., x + w * 0.88, r.bottom - 24., 5., coral);
            if !small {
                let rx = x + w * 0.52;
                let rw = w * 0.48;
                let items = [("项目一周年", "18 天", Color::from_rgb(0, 122, 255)), ("春节假期", "71 天", Color::from_rgb(255, 149, 0))];
                for (i, (t, d, col)) in items.into_iter().enumerate() {
                    let ry = y + 24. + i as f32 * 50.;
                    let mut bg = Paint::default();
                    bg.set_anti_alias(true);
                    bg.set_color(Color::from_argb(if v.dark { 38 } else { 20 }, col.r(), col.g(), col.b()));
                    c.draw_round_rect(Rect::from_xywh(rx, ry, rw, 42.), 10., 10., &bg);
                    text_styled(c, t, rx + 10., ry + 6., rw - 20., 11., fg, Weight::SEMI_BOLD, TextAlign::Left, 1);
                    text_styled(c, d, rx + 10., ry + 22., rw - 20., 11., col, Weight::BOLD, TextAlign::Left, 1);
                }
            }
        }
        "today" => {
            // 今日概览（日期 + 天气 + 联动待办统计）
            let (_, mon, day, dow, _, _, _) = super::sidebar::local_datetime();
            let wk = ["周日", "周一", "周二", "周三", "周四", "周五", "周六"][dow.min(6) as usize];
            let pending = v.todos.items.iter().filter(|t| !t.done).count();
            text_styled(c, "今日概览", x, y, 80., 11.5, accent_blue(v.dark), Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("{mon}月{day}日 {wk}"), x, y + 22., w, 18., fg, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("晴朗 22° · {pending} 项待办待处理"), x, y + 50., w, 11.5, sub, Weight::MEDIUM, TextAlign::Left, 1);
            if !small {
                let mut div = Paint::default();
                div.set_color(Color::from_argb(if v.dark { 30 } else { 20 }, 120, 120, 128));
                c.draw_line((x, y + 76.), (x + w, y + 76.), &div);
                let show_rows = if large { 4 } else { 2 };
                for (i, item) in v.todos.items.iter().take(show_rows).enumerate() {
                    draw_todo_row(
                        c,
                        Rect::from_xywh(x - 4., y + 84. + i as f32 * 36., w + 8., 32.),
                        &item.title,
                        &item.time,
                        item.done,
                        i,
                        fg,
                        sub,
                        v,
                        hits,
                    );
                }
            }
        }
        "wireless" => {
            // 无线设备电量四圆环
            let bat = v.data.metrics.battery.unwrap_or(86) as f32;
            let track_col = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            text_styled(c, "无线设备电量", x, y, w, 11.5, sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
            let devs = [
                ("本机", bat, Color::from_rgb(52, 199, 89)),
                ("耳机", 92., Color::from_rgb(0, 122, 255)),
                ("鼠标", 74., Color::from_rgb(48, 209, 88)),
                ("键盘", 61., Color::from_rgb(255, 149, 0)),
            ];
            let count = if small { 2 } else { 4 };
            let cw = w / count as f32;
            let rad = if small { 24. } else { 26. };
            let cy = y + 30. + rad;
            for (i, (name, pct, col)) in devs.into_iter().take(count).enumerate() {
                let cx = x + cw * (i as f32 + 0.5);
                draw_apple_ring(c, cx, cy, rad, 5.2, pct / 100., col, track_col);
                text_styled(c, &format!("{pct:.0}%"), cx - cw * 0.5, cy - 8., cw, 11.5, fg, Weight::BOLD, TextAlign::Center, 1);
                text_styled(c, name, cx - cw * 0.5, cy + rad + 10., cw, 10., sub, Weight::MEDIUM, TextAlign::Center, 1);
            }
        }
        "storage" => {
            // macOS 存储空间多色分段容量条
            let used_pct = v.data.metrics.disk.unwrap_or(55.0).clamp(10., 98.);
            text_styled(c, "存储空间", x, y, 80., 11.5, sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("已用 {used_pct:.0}%"), x + 80., y, (w - 80.).max(40.), 12., fg, Weight::BOLD, TextAlign::Right, 1);
            let by = y + 34.;
            let bw = w;
            let track = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            draw_line(c, x + 4., by, x + bw - 4., by, 12., track);
            let segs = [
                (0.24, Color::from_rgb(0, 122, 255), "应用"),
                (0.16, Color::from_rgb(255, 204, 0), "文档"),
                (0.10, Color::from_rgb(175, 82, 222), "媒体"),
                ((used_pct / 100. - 0.50).max(0.05), Color::from_rgb(142, 142, 147), "系统"),
            ];
            let mut cur = x + 4.;
            for (frac, col, _) in segs {
                let len = (bw - 8.) * frac.min(0.45);
                draw_line(c, cur, by, cur + len, by, 12., col);
                cur += len + 3.;
            }
            for (i, (_, col, lbl)) in segs.into_iter().take(if small { 2 } else { 4 }).enumerate() {
                let lx = x + (i % 2) as f32 * (w * 0.5);
                let ly = y + 58. + (i / 2) as f32 * 24.;
                let mut dp = Paint::default();
                dp.set_anti_alias(true);
                dp.set_color(col);
                c.draw_circle((lx + 5., ly + 7.), 4., &dp);
                text_styled(c, lbl, lx + 14., ly, w * 0.42, 11., fg, Weight::MEDIUM, TextAlign::Left, 1);
            }
        }
        "focus_stats" => {
            // 本周专注统计 7 日橙色柱状图
            let orange = Color::from_rgb(255, 149, 0);
            text_styled(c, "专注统计", x, y, 80., 11.5, orange, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, "本周 14.5 小时", x + 80., y, (w - 80.).max(40.), 12., fg, Weight::BOLD, TextAlign::Right, 1);
            let bars = [("一", 0.55), ("二", 0.80), ("三", 0.45), ("四", 0.92), ("五", 0.76), ("六", 0.35), ("日", 0.62)];
            let bw = w / 7.;
            let max_h = if large { 140. } else { 64. };
            let base_y = y + 36. + max_h;
            for (i, (day_lbl, val)) in bars.into_iter().enumerate() {
                let bx = x + i as f32 * bw + (bw - 12.) * 0.5;
                let bh = max_h * val;
                let mut bp = Paint::default();
                bp.set_anti_alias(true);
                bp.set_color(orange);
                c.draw_round_rect(Rect::from_xywh(bx, base_y - bh, 12., bh), 6., 6., &bp);
                text_styled(c, day_lbl, x + i as f32 * bw, base_y + 6., bw, 9.5, sub, Weight::MEDIUM, TextAlign::Center, 1);
            }
        }
        "schedule" => {
            // 日程倒计时
            let indigo = Color::from_rgb(88, 86, 214);
            text_styled(c, "下一日程", x, y, w, 11., indigo, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, "界面评审", x, y + 22., w, 16., fg, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, "01时 45分", x, y + 52., w, 24., indigo, Weight::BOLD, TextAlign::Left, 1);
            let track = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            draw_line(c, x, r.bottom - 24., x + w, r.bottom - 24., 5., track);
            draw_line(c, x, r.bottom - 24., x + w * 0.66, r.bottom - 24., 5., indigo);
            if !small {
                text_styled(c, "15:00 – 16:00 · 线上会议室", x, y + 88., w, 10.5, sub, Weight::NORMAL, TextAlign::Left, 1);
            }
        }
        "screen_time" => {
            // Apple Screen Time 用机时长
            text_styled(c, "用机时长", x, y, 80., 11.5, sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
            text_styled(c, "日均 5小时18分", x + 80., y, (w - 80.).max(40.), 12., fg, Weight::BOLD, TextAlign::Right, 1);
            let days = [("一", 0.68), ("二", 0.78), ("三", 0.56), ("四", 0.84), ("五", 0.72), ("六", 0.44), ("日", 0.52)];
            let cw = w / 7.;
            let max_h = if large { 130. } else { 62. };
            let base_y = y + 36. + max_h;
            for (i, (dl, f)) in days.into_iter().enumerate() {
                let bx = x + i as f32 * cw + (cw - 12.) * 0.5;
                let bh = max_h * f;
                let mut bp = Paint::default();
                bp.set_anti_alias(true);
                bp.set_color(Color::from_rgb(0, 122, 255));
                c.draw_round_rect(Rect::from_xywh(bx, base_y - bh, 12., bh * 0.62), 4., 4., &bp);
                bp.set_color(Color::from_rgb(48, 176, 199));
                c.draw_round_rect(Rect::from_xywh(bx, base_y - bh * 0.36, 12., bh * 0.36), 4., 4., &bp);
                text_styled(c, dl, x + i as f32 * cw, base_y + 6., cw, 9.5, sub, Weight::MEDIUM, TextAlign::Center, 1);
            }
        }
        "quote" => {
            // 极简杂志日签（点击切换下一句）
            const QUOTES: &[(&str, &str)] = &[
                ("少即是多，克制方见秩序。", "Mies van der Rohe"),
                ("把注意力留给真正重要的少数事物。", "Dieter Rams"),
                ("好设计是尽量少的设计，而是把每像素打磨到恰如其分。", "日签摘录"),
                ("每一件小事完成时，内心都会腾出一块空间。", "今日心境"),
                ("慢下来，让呼吸和屏幕上的光影同步。", "静谧时刻"),
                ("工具最好的样子，是安静待命、随叫随到。", "工程手记"),
            ];
            let (q, author) = QUOTES[v.quote_index % QUOTES.len()];
            text_styled(c, "“", x, y - 6., 32., 28., Color::from_rgb(255, 149, 0), Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, q, x, y + 26., w, if large { 16. } else { 13. }, fg, Weight::SEMI_BOLD, TextAlign::Left, 4);
            text_styled(c, &format!("— {author}"), x, r.bottom - 26., w, 9.5, sub, Weight::MEDIUM, TextAlign::Right, 1);
            let next_r = Rect::from_xywh(r.right - 60., y, 48., 22.);
            glass::liquid_pill(c, next_r, 11., v.dark, false);
            text_styled(c, "换一句", next_r.left, next_r.top + 4., 48., 9.5, sub, Weight::SEMI_BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(next_r, "切换下一句日签", Action::QuoteNext));
        }
        "hydration" => {
            // 喝水提醒组件：单次饮水量、每日目标、开关、间隔切换与独立提醒灵动岛预览
            let rem = v.reminders;
            let cyan = Color::from_rgb(10, 182, 255);
            let track = Color::from_argb(if v.dark { 38 } else { 24 }, 10, 182, 255);
            let rem_s = rem.water_remaining_sec();
            text_styled(
                c,
                "喝水提醒",
                x,
                y,
                74.,
                12.5,
                cyan,
                Weight::BOLD,
                TextAlign::Left,
                1,
            );
            if !small {
                let sw_r = Rect::from_xywh(x + 74., y - 1., 42., 19.);
                let mut sp = Paint::default();
                sp.set_anti_alias(true);
                sp.set_color(if rem.water_enabled { cyan } else { Color::from_argb(60, 120, 120, 128) });
                c.draw_round_rect(sw_r, 9.5, 9.5, &sp);
                text_styled(c, if rem.water_enabled { "开启" } else { "暂停" }, sw_r.left, sw_r.top + 3., 42., 8.8, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(sw_r, "开关喝水提醒", Action::WaterToggle));

                let int_r = Rect::from_xywh(x + 122., y - 1., 52., 19.);
                glass::liquid_pill(c, int_r, 9.5, v.dark, false);
                text_styled(c, &format!("每{}分", rem.water_interval_min), int_r.left, int_r.top + 3., 52., 8.8, sub, Weight::SEMI_BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(int_r, "切换喝水间隔", Action::WaterCycleInterval));

                let isl_r = Rect::from_xywh(r.right - 92., y - 2., 76., 21.);
                glass::liquid_pill(c, isl_r, 10.5, v.dark, false);
                text_styled(c, "试弹灵动岛", isl_r.left, isl_r.top + 3.5, 76., 9., cyan, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(isl_r, "预览喝水提醒灵动岛", Action::WaterPreviewIsland));
            }

            if small {
                text_styled(c, &format!("{}ml", rem.drank_ml), x, y + 26., w, 24., fg, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &format!("目标 {} ml · {}:{:02}", rem.goal_ml, rem_s / 60, rem_s % 60), x, y + 58., w, 9.5, sub, Weight::NORMAL, TextAlign::Left, 1);
                draw_line(c, x + 2., y + 84., x + w - 2., y + 84., 7., track);
                draw_line(c, x + 2., y + 84., x + 2. + (w - 4.) * rem.water_progress().clamp(0.05, 1.), y + 84., 7., cyan);
                let dr = Rect::from_xywh(x, r.bottom - 40., w, 28.);
                let mut dp = Paint::default();
                dp.set_anti_alias(true);
                dp.set_color(cyan);
                c.draw_round_rect(dr, 12., 12., &dp);
                text_styled(c, &format!("+ 喝一杯 ({}ml)", rem.cup_ml), dr.left, dr.top + 6., dr.width(), 10.5, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(dr, "记录喝水一杯", Action::WaterDrink));
            } else {
                text_styled(c, &format!("{}", rem.drank_ml), x, y + 24., 88., 28., fg, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &format!("/ {} ml · 已喝 {} 杯", rem.goal_ml, rem.cups_today), x + 78., y + 36., 150., 11., sub, Weight::MEDIUM, TextAlign::Left, 1);
                text_styled(
                    c,
                    &format!("下次 {:02}:{:02}", rem_s / 60, rem_s % 60),
                    x + w - 84.,
                    y + 37.,
                    84.,
                    10.,
                    cyan,
                    Weight::SEMI_BOLD,
                    TextAlign::Right,
                    1,
                );

                draw_line(c, x + 2., y + 66., x + w - 2., y + 66., 8., track);
                draw_line(c, x + 2., y + 66., x + 2. + (w - 4.) * rem.water_progress().clamp(0.04, 1.), y + 66., 8., cyan);

                // 底部三段调节 + 打卡：单杯毫升 / 每日目标 / +喝一杯
                let by = y + 86.;
                let cup_l = Rect::from_xywh(x, by, 22., 24.);
                let cup_r = Rect::from_xywh(x + 86., by, 22., 24.);
                glass::liquid_pill(c, cup_l, 8., v.dark, false);
                glass::liquid_pill(c, cup_r, 8., v.dark, false);
                text_styled(c, "-", cup_l.left, cup_l.top + 3.5, 22., 12., fg, Weight::BOLD, TextAlign::Center, 1);
                text_styled(c, &format!("{}ml", rem.cup_ml), x + 24., by + 5., 60., 10.2, fg, Weight::SEMI_BOLD, TextAlign::Center, 1);
                text_styled(c, "+", cup_r.left, cup_r.top + 3.5, 22., 12., fg, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(cup_l, "单次减 50ml", Action::WaterCupDelta(-50)));
                hits.push(HitItem::new(cup_r, "单次加 50ml", Action::WaterCupDelta(50)));

                let gl_l = Rect::from_xywh(x + 116., by, 22., 24.);
                let gl_r = Rect::from_xywh(x + 216., by, 22., 24.);
                glass::liquid_pill(c, gl_l, 8., v.dark, false);
                glass::liquid_pill(c, gl_r, 8., v.dark, false);
                text_styled(c, "-", gl_l.left, gl_l.top + 3.5, 22., 12., fg, Weight::BOLD, TextAlign::Center, 1);
                text_styled(c, &format!("目标{}L", (rem.goal_ml as f32 / 1000.)), x + 140., by + 5., 74., 10., sub, Weight::SEMI_BOLD, TextAlign::Center, 1);
                text_styled(c, "+", gl_r.left, gl_r.top + 3.5, 22., 12., fg, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(gl_l, "目标减 250ml", Action::WaterGoalDelta(-250)));
                hits.push(HitItem::new(gl_r, "目标加 250ml", Action::WaterGoalDelta(250)));

                let drink_r = Rect::from_xywh(x + w - 88., by - 2., 88., 28.);
                let mut dp = Paint::default();
                dp.set_anti_alias(true);
                dp.set_color(cyan);
                c.draw_round_rect(drink_r, 12., 12., &dp);
                text_styled(c, &format!("+ {}ml", rem.cup_ml), drink_r.left, drink_r.top + 6., drink_r.width(), 10.5, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(drink_r, "确认喝水一杯", Action::WaterDrink));

                text_styled(
                    c,
                    "每轮单次毫升 · 每日总目标 · 到点弹出黑色顶部灵动岛",
                    x,
                    r.bottom - 20.,
                    w,
                    8.8,
                    sub,
                    Weight::NORMAL,
                    TextAlign::Left,
                    1,
                );

                if large {
                    let cy_y = by + 46.;
                    for cup_i in 0..8 {
                        let cx_i = x + cup_i as f32 * (w / 8.);
                        let filled = cup_i < rem.cups_today as usize;
                        let mut cp = Paint::default();
                        cp.set_anti_alias(true);
                        cp.set_color(if filled { cyan } else { track });
                        c.draw_round_rect(Rect::from_xywh(cx_i + 4., cy_y + 24., w / 8. - 10., 46.), 8., 8., &cp);
                        text_styled(c, &format!("{}杯", cup_i + 1), cx_i, cy_y + 76., w / 8., 9., sub, Weight::MEDIUM, TextAlign::Center, 1);
                    }
                    text_styled(
                        c,
                        &format!("今日达标率 {:.0}% · 每次喝完点灵动岛即消失并重新计时", rem.water_progress() * 100.),
                        x,
                        r.bottom - 24.,
                        w,
                        10.,
                        cyan,
                        Weight::SEMI_BOLD,
                        TextAlign::Center,
                        1,
                    );
                }
            }
        }
        "sedentary" => {
            // 久坐提醒组件：已坐进度环、间距预设、起身打卡与提醒灵动岛预览
            let rem = v.reminders;
            let orange = Color::from_rgb(255, 149, 0);
            let track = Color::from_argb(if v.dark { 38 } else { 22 }, 120, 120, 128);
            let rem_s = rem.stand_remaining_sec();
            let total_s = (rem.stand_interval_min * 60).max(1) as f32;
            let frac = (rem.stand_elapsed_sec / total_s).clamp(0.05, 1.0);
            text_styled(c, "久坐提醒", x, y, 74., 12.5, orange, Weight::BOLD, TextAlign::Left, 1);

            if small {
                let rad = 34.;
                let cx = r.center_x();
                let cy = y + 58.;
                draw_apple_ring(c, cx, cy, rad, 5.5, frac, orange, track);
                text_styled(c, &format!("{:02}:{:02}", rem_s / 60, rem_s % 60), cx - rad, cy - 10., rad * 2., 13., fg, Weight::BOLD, TextAlign::Center, 1);
                let sr = Rect::from_xywh(x, r.bottom - 38., w, 26.);
                let mut sp = Paint::default();
                sp.set_anti_alias(true);
                sp.set_color(orange);
                c.draw_round_rect(sr, 11., 11., &sp);
                text_styled(c, "起身活动", sr.left, sr.top + 5.5, sr.width(), 10.5, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(sr, "起身打卡并重置计时", Action::StandAck));
            } else {
                let sw_r = Rect::from_xywh(x + 74., y - 1., 42., 19.);
                let mut sp = Paint::default();
                sp.set_anti_alias(true);
                sp.set_color(if rem.stand_enabled { orange } else { Color::from_argb(60, 120, 120, 128) });
                c.draw_round_rect(sw_r, 9.5, 9.5, &sp);
                text_styled(c, if rem.stand_enabled { "开启" } else { "暂停" }, sw_r.left, sw_r.top + 3., 42., 8.8, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(sw_r, "开关久坐提醒", Action::StandToggle));

                let isl_r = Rect::from_xywh(r.right - 92., y - 2., 76., 21.);
                glass::liquid_pill(c, isl_r, 10.5, v.dark, false);
                text_styled(c, "试弹灵动岛", isl_r.left, isl_r.top + 3.5, 76., 9., orange, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(isl_r, "预览久坐提醒灵动岛", Action::StandPreviewIsland));

                let rad = if large { 56. } else { 40. };
                let cx = x + rad + 8.;
                let cy = y + 36. + rad;
                draw_apple_ring(c, cx, cy, rad, 6.2, frac, orange, track);
                text_styled(c, &format!("{:02}:{:02}", rem_s / 60, rem_s % 60), cx - rad, cy - 11., rad * 2., 15., fg, Weight::BOLD, TextAlign::Center, 1);
                text_styled(c, "倒计时", cx - rad, cy + 9., rad * 2., 8.8, sub, Weight::MEDIUM, TextAlign::Center, 1);

                let rx = cx + rad + 18.;
                let rw = (x + w - rx).max(120.);
                text_styled(c, &format!("今日已起身 {} 次", rem.stand_count), rx, y + 34., rw, 15., fg, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &format!("每隔 {} 分钟提醒伸展放松", rem.stand_interval_min), rx, y + 60., rw, 10.5, sub, Weight::NORMAL, TextAlign::Left, 1);

                let int_r = Rect::from_xywh(rx, y + 88., 74., 28.);
                glass::liquid_pill(c, int_r, 10., v.dark, false);
                text_styled(c, &format!("间隔 {}分", rem.stand_interval_min), int_r.left, int_r.top + 6.5, int_r.width(), 10.2, fg, Weight::SEMI_BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(int_r, "切换久坐提醒间隔", Action::StandCycleInterval));

                let ack_r = Rect::from_xywh(rx + 82., y + 88., (rw - 82.).max(72.), 28.);
                let mut ap = Paint::default();
                ap.set_anti_alias(true);
                ap.set_color(orange);
                c.draw_round_rect(ack_r, 10., 10., &ap);
                text_styled(c, "起身打卡", ack_r.left, ack_r.top + 6.5, ack_r.width(), 10.5, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(ack_r, "确认已起身活动", Action::StandAck));

                if large {
                    let tips = ["1. 起身远眺窗外 20 秒缓解视疲劳", "2. 转动肩颈与腰背伸展 30 秒", "3. 顺便补充一杯温水保持精力"];
                    for (i, t) in tips.into_iter().enumerate() {
                        let ty = cy + rad + 26. + i as f32 * 40.;
                        let tr = Rect::from_xywh(x, ty, w, 32.);
                        glass::liquid_pill(c, tr, 10., v.dark, false);
                        text_styled(c, t, tr.left + 12., tr.top + 8.5, tr.width() - 24., 11., fg, Weight::MEDIUM, TextAlign::Left, 1);
                    }
                }
            }
        }
        "habit" => {
            // 新设计 1：每日微习惯打卡
            let habits = ["早睡 23:00", "阅读 20m", "正念冥想", "远眺护眼"];
            let done_n = v.extra.habits.iter().filter(|&&b| b).count();
            text_styled(c, "每日习惯", x, y, 80., 12., Color::from_rgb(52, 199, 89), Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("已完成 {done_n}/4"), x + 80., y + 1., (w - 80.).max(40.), 10.5, sub, Weight::SEMI_BOLD, TextAlign::Right, 1);
            let cols = if small { 1 } else { 2 };
            let cw = if cols == 2 { (w - 10.) * 0.5 } else { w };
            let bh = if small { 26. } else if large { 68. } else { 50. };
            for (i, lbl) in habits.into_iter().take(if small { 3 } else { 4 }).enumerate() {
                let bx = x + (i % cols) as f32 * (cw + 10.);
                let by = y + 28. + (i / cols) as f32 * (bh + 8.);
                let br = Rect::from_xywh(bx, by, cw, bh);
                let done = v.extra.habits[i];
                let mut fp = Paint::default();
                fp.set_anti_alias(true);
                fp.set_color(if done {
                    Color::from_argb(if v.dark { 72 } else { 36 }, 52, 199, 89)
                } else {
                    Color::from_argb(if v.dark { 30 } else { 18 }, 120, 120, 128)
                });
                c.draw_round_rect(br, 12., 12., &fp);
                text_styled(
                    c,
                    &format!("{} {}", if done { "✓" } else { "○" }, lbl),
                    br.left + 10.,
                    br.center_y() - 7.5,
                    br.width() - 20.,
                    11.5,
                    if done { Color::from_rgb(40, 178, 76) } else { fg },
                    Weight::SEMI_BOLD,
                    TextAlign::Left,
                    1,
                );
                hits.push(HitItem::new(br, lbl, Action::HabitToggle(i)));
            }
            if large {
                text_styled(c, "连续 6 天保持打卡记录 · 每晚 22:30 自动结算", x, r.bottom - 28., w, 10.5, sub, Weight::NORMAL, TextAlign::Center, 1);
            }
        }
        "countdown_ring" => {
            // 新设计 2：下班倒计时环
            let (_, _, _, _, hr, min, _) = super::sidebar::local_datetime();
            let now_m: u32 = hr * 60 + min;
            let off_m: u32 = 18 * 60;
            let left_m = off_m.saturating_sub(now_m);
            let frac = if now_m < off_m { (now_m as f32 - 9. * 60.).max(0.) / (9. * 60.) } else { 1.0 };
            let teal = Color::from_rgb(48, 176, 199);
            let track = Color::from_argb(if v.dark { 36 } else { 22 }, 120, 120, 128);
            text_styled(c, "下班倒计时", x, y, w, 11.5, teal, Weight::BOLD, TextAlign::Left, 1);
            let rad = if large { 68. } else { 38. };
            let cx = if small || large { r.center_x() } else { x + rad + 12. };
            let cy = if large { y + 116. } else { y + 74. };
            draw_apple_ring(c, cx, cy, rad, 6.2, frac.clamp(0.06, 1.), teal, track);
            let val = if now_m >= off_m {
                "已下班".into()
            } else {
                format!("{:02}:{:02}", left_m / 60, left_m % 60)
            };
            text_styled(c, &val, cx - rad, cy - 11., rad * 2., 16., fg, Weight::BOLD, TextAlign::Center, 1);
            text_styled(c, if now_m >= off_m { "享受夜晚" } else { "距 18:00" }, cx - rad, cy + 10., rad * 2., 9., sub, Weight::MEDIUM, TextAlign::Center, 1);
            if !small && !large {
                text_styled(c, "今日工作进度", cx + rad + 20., y + 48., 160., 14., fg, Weight::BOLD, TextAlign::Left, 1);
                text_styled(c, &format!("已完成 {:.0}% · 注意劳逸结合", frac.clamp(0., 1.) * 100.), cx + rad + 20., y + 74., 180., 10.5, sub, Weight::NORMAL, TextAlign::Left, 1);
            }
        }
        "pomodoro_wave" => {
            // 新设计 3：白噪音与专注声波
            let scenes = [("夏夜雨声", "42 dB"), ("松林清风", "36 dB"), ("壁炉暖火", "44 dB"), ("安静咖啡馆", "48 dB")];
            let (name, db) = scenes[v.extra.ambient_idx % scenes.len()];
            let purple = Color::from_rgb(175, 82, 222);
            text_styled(c, "白噪音氛围", x, y, 80., 11.5, purple, Weight::BOLD, TextAlign::Left, 1);
            let next_r = Rect::from_xywh(r.right - 64., y - 2., 50., 20.);
            glass::liquid_pill(c, next_r, 10., v.dark, false);
            text_styled(c, "换场景", next_r.left, next_r.top + 3.5, 50., 9.2, sub, Weight::SEMI_BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(next_r, "切换氛围场景", Action::AmbientNext));

            text_styled(c, name, x, y + 26., w - 80., 16., fg, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, db, x, y + 50., 80., 10.5, sub, Weight::NORMAL, TextAlign::Left, 1);

            let wave_y = y + 96.;
            let bars = 16;
            let bw = (w - 86.) / bars as f32;
            for i in 0..bars {
                let amp = if v.extra.ambient_playing {
                    0.35 + 0.65 * ((i as f32 * 0.9 + v.time_sec * 3.8).sin().abs())
                } else {
                    0.25 + 0.35 * ((i as f32 * 1.1).sin().abs())
                };
                let bh = 28. * amp;
                let mut bp = Paint::default();
                bp.set_anti_alias(true);
                bp.set_color(if v.extra.ambient_playing { purple } else { Color::from_argb(80, 175, 82, 222) });
                c.draw_round_rect(Rect::from_xywh(x + i as f32 * bw, wave_y - bh * 0.5, (bw - 4.).max(3.), bh), 2., 2., &bp);
            }
            let play_r = Rect::from_xywh(r.right - 74., wave_y - 16., 60., 30.);
            let mut pp = Paint::default();
            pp.set_anti_alias(true);
            pp.set_color(purple);
            c.draw_round_rect(play_r, 15., 15., &pp);
            text_styled(c, if v.extra.ambient_playing { "暂停" } else { "播放" }, play_r.left, play_r.top + 7.5, play_r.width(), 11., Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(play_r, "播放或暂停白噪音", Action::AmbientToggle));
        }
        "color_picker" => {
            // 新设计 4：灵感色板（点击色块直接复制 HEX）
            const PALETTES: &[(&str, [(u8, u8, u8, &str); 4])] = &[
                ("晴空苹果", [(0, 122, 255, "#007AFF"), (52, 199, 89, "#34C759"), (255, 149, 0, "#FF9500"), (255, 59, 48, "#FF3B30")]),
                ("海盐莫兰迪", [(118, 146, 168, "#7692A8"), (164, 184, 172, "#A4B8AC"), (217, 198, 176, "#D9C6B0"), (142, 128, 140, "#8E808C")]),
                ("落日余晖", [(255, 94, 98, "#FF5E62"), (255, 153, 102, "#FF9966"), (255, 204, 112, "#FFCC70"), (118, 75, 162, "#764BA2")]),
            ];
            let (title, cols) = &PALETTES[v.extra.palette_idx % PALETTES.len()];
            text_styled(c, title, x, y, w - 48., 11.5, fg, Weight::BOLD, TextAlign::Left, 1);
            let nx = Rect::from_xywh(r.right - 52., y - 2., 40., 19.);
            glass::liquid_pill(c, nx, 9.5, v.dark, false);
            text_styled(c, "换组", nx.left, nx.top + 3.5, 40., 8.8, accent_blue(v.dark), Weight::BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(nx, "切换色板", Action::PaletteNext));

            let cw = (w - 8.) * 0.5;
            let ch = ((r.height() - 50.) - 8.) * 0.5;
            for (i, (cr, cg, cb, hex)) in cols.iter().enumerate() {
                let br = Rect::from_xywh(x + (i % 2) as f32 * (cw + 8.), y + 26. + (i / 2) as f32 * (ch + 8.), cw, ch);
                let mut cp = Paint::default();
                cp.set_anti_alias(true);
                cp.set_color(Color::from_rgb(*cr, *cg, *cb));
                c.draw_round_rect(br, 10., 10., &cp);
                text_styled(c, hex, br.left + 4., br.bottom - 18., br.width() - 8., 8.5, Color::WHITE, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(br, &format!("复制色值 {hex}"), Action::Copy((*hex).into())));
            }
        }
        "net_speed" => {
            // 新设计 5：实时网速脉冲仪
            let dn = v.data.metrics.download / 1024.;
            let up = v.data.metrics.upload / 1024.;
            let fmt = |kb: f64| if kb >= 1024. { format!("{:.1} MB/s", kb / 1024.) } else { format!("{kb:.0} KB/s") };
            let blue = Color::from_rgb(0, 122, 255);
            let green = Color::from_rgb(52, 199, 89);
            text_styled(c, "实时网速", x, y, w, 11.5, sub, Weight::SEMI_BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("↓ {}", fmt(dn)), x, y + 24., w * 0.5, 19., blue, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("↑ {}", fmt(up)), x + w * 0.5, y + 24., w * 0.5, 19., green, Weight::BOLD, TextAlign::Right, 1);
            let base_y = y + 116.;
            let bars = 18;
            let bw = w / bars as f32;
            for i in 0..bars {
                let h1 = 16. + ((dn as f32 * 0.1 + i as f32 * 0.8 + v.time_sec * 2.).sin().abs() * 34.).min(42.);
                let mut bp = Paint::default();
                bp.set_anti_alias(true);
                bp.set_color(if i % 3 == 0 { green } else { blue });
                c.draw_round_rect(Rect::from_xywh(x + i as f32 * bw + 2., base_y - h1, (bw - 5.).max(4.), h1), 3., 3., &bp);
            }
        }
        "converter" => {
            // 新设计 6：汇率快查参考表
            text_styled(c, "汇率快查 (100 外币)", x, y, w, 11.5, Color::from_rgb(0, 122, 255), Weight::BOLD, TextAlign::Left, 1);
            let rates = [("USD 美元", "¥ 724.6"), ("EUR 欧元", "¥ 782.1"), ("JPY 日元", "¥ 4.78"), ("HKD 港币", "¥ 92.8")];
            let cw = (w - 12.) * 0.5;
            for (i, (cur, val)) in rates.into_iter().take(if small { 2 } else { 4 }).enumerate() {
                let bx = x + (i % 2) as f32 * (cw + 12.);
                let by = y + 30. + (i / 2) as f32 * 54.;
                let rr = Rect::from_xywh(bx, by, cw, 46.);
                glass::liquid_pill(c, rr, 10., v.dark, false);
                text_styled(c, cur, rr.left + 10., rr.top + 6., rr.width() - 20., 10., sub, Weight::MEDIUM, TextAlign::Left, 1);
                text_styled(c, val, rr.left + 10., rr.top + 22., rr.width() - 20., 13., fg, Weight::BOLD, TextAlign::Left, 1);
                hits.push(HitItem::new(rr, "复制汇率", Action::Copy(format!("{cur} = {val}"))));
            }
        }
        "github_grid" => {
            // 新设计 7：专注与任务贡献热力图
            text_styled(c, "专注贡献热力图", x, y, 130., 11.5, Color::from_rgb(52, 199, 89), Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, "近 12 周 148 次", x + 130., y, (w - 130.).max(40.), 10., sub, Weight::MEDIUM, TextAlign::Right, 1);
            let cols = if small { 6 } else { 14 };
            let rows = if large { 7 } else { 5 };
            let sz = ((w - (cols as f32 - 1.) * 4.) / cols as f32).min(16.);
            for col in 0..cols {
                for row in 0..rows {
                    let level = (col * 7 + row * 3 + v.todos.items.len()) % 4;
                    let col_c = match level {
                        0 => Color::from_argb(if v.dark { 28 } else { 20 }, 120, 120, 128),
                        1 => Color::from_rgb(155, 233, 168),
                        2 => Color::from_rgb(64, 196, 99),
                        _ => Color::from_rgb(33, 110, 57),
                    };
                    let mut gp = Paint::default();
                    gp.set_anti_alias(true);
                    gp.set_color(col_c);
                    c.draw_round_rect(Rect::from_xywh(x + col as f32 * (sz + 4.), y + 28. + row as f32 * (sz + 4.), sz, sz), 3.5, 3.5, &gp);
                }
            }
        }
        "wooden_fish" => {
            // 新设计 8：禅意解压木鱼
            let amber = Color::from_rgb(217, 119, 6);
            text_styled(c, "解压木鱼", x, y, w, 11.5, amber, Weight::BOLD, TextAlign::Left, 1);
            text_styled(c, &format!("功德 {}", v.extra.merit_count), x, y + 24., w, 24., fg, Weight::BOLD, TextAlign::Center, 1);
            let mut mp = Paint::default();
            mp.set_anti_alias(true);
            mp.set_color(Color::from_argb(if v.dark { 70 } else { 36 }, 217, 119, 6));
            c.draw_oval(Rect::from_xywh(r.center_x() - 32., y + 64., 64., 40.), &mp);
            draw_line(c, r.center_x() - 18., y + 84., r.center_x() + 18., y + 84., 3., amber);
            let tap_r = Rect::from_xywh(x, r.bottom - 38., w, 26.);
            glass::liquid_pill(c, tap_r, 12., v.dark, false);
            text_styled(c, "轻敲一下 · 功德 +1", tap_r.left, tap_r.top + 5.5, tap_r.width(), 10., amber, Weight::BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(tap_r, "敲木鱼功德 +1", Action::WoodFishTap));
        }
        "breathing" => {
            // 新设计 9：Apple Watch 正念呼吸引导
            let mint = Color::from_rgb(56, 210, 188);
            text_styled(c, "呼吸引导", x, y, w, 11.5, mint, Weight::BOLD, TextAlign::Left, 1);
            let cx = r.center_x();
            let cy = y + 72.;
            let phase = if v.extra.breath_active { (v.time_sec * 0.9).sin() * 0.5 + 0.5 } else { 0.5 };
            let pr = 18. + 10. * phase;
            for k in 0..6 {
                let ang = k as f32 * TAU / 6. + if v.extra.breath_active { v.time_sec * 0.4 } else { 0. };
                let mut fp = Paint::default();
                fp.set_anti_alias(true);
                fp.set_color(Color::from_argb(86, 56, 210, 188));
                c.draw_circle((cx + ang.cos() * (8. + 5. * phase), cy + ang.sin() * (8. + 5. * phase)), pr * 0.65, &fp);
            }
            let hint = if !v.extra.breath_active { "已暂停" } else if phase > 0.5 { "深深吸气" } else { "缓慢呼气" };
            let br = Rect::from_xywh(x, r.bottom - 36., w, 24.);
            glass::liquid_pill(c, br, 12., v.dark, false);
            text_styled(c, hint, br.left, br.top + 4.5, br.width(), 10., mint, Weight::BOLD, TextAlign::Center, 1);
            hits.push(HitItem::new(br, "暂停或继续呼吸", Action::BreathToggle));
        }
        "quick_calc" => {
            // 新设计 10：桌面迷你计算器
            let disp_r = Rect::from_xywh(x, y, w, 30.);
            glass::liquid_pill(c, disp_r, 9., v.dark, false);
            text_styled(c, &v.extra.calc_display, disp_r.left + 8., disp_r.top + 5., disp_r.width() - 16., 15., fg, Weight::BOLD, TextAlign::Right, 1);
            hits.push(HitItem::new(disp_r, "复制结果", Action::Copy(v.extra.calc_display.clone())));
            let keys = [
                "7", "8", "9", "/",
                "4", "5", "6", "*",
                "1", "2", "3", "-",
                "C", "0", "=", "+",
            ];
            let kw = (w - 12.) / 4.;
            let kh = ((r.height() - 62.) - 12.) / 4.;
            for (i, k) in keys.into_iter().enumerate() {
                let kr = Rect::from_xywh(x + (i % 4) as f32 * (kw + 4.), y + 36. + (i / 4) as f32 * (kh + 4.), kw, kh);
                let is_op = matches!(k, "/" | "*" | "-" | "+" | "=");
                let mut kp = Paint::default();
                kp.set_anti_alias(true);
                kp.set_color(if k == "=" {
                    Color::from_rgb(255, 149, 0)
                } else if is_op {
                    Color::from_argb(if v.dark { 58 } else { 32 }, 0, 122, 255)
                } else {
                    Color::from_argb(if v.dark { 36 } else { 20 }, 120, 120, 128)
                });
                c.draw_round_rect(kr, 6., 6., &kp);
                text_styled(c, k, kr.left, kr.center_y() - 6.5, kr.width(), 10.5, if k == "=" { Color::WHITE } else { fg }, Weight::BOLD, TextAlign::Center, 1);
                hits.push(HitItem::new(kr, &format!("计算器按键 {k}"), Action::CalcKey(k.into())));
            }
        }
        _ => {
            // 1:1 复刻 Apple Shortcuts 快捷指令 2×2 系统标准色方块
            let bw = (w - 10.) * 0.5;
            let bh = ((r.height() - 28.) - 10.) * 0.5;
            let items = [
                (Color::from_rgb(0, 122, 255), "文件"),
                (Color::from_rgb(88, 86, 214), "终端"),
                (Color::from_rgb(48, 176, 199), "网页"),
                (Color::from_rgb(255, 149, 0), "便签"),
            ];
            for (idx, (col, name)) in items.into_iter().enumerate() {
                let bx = x + (idx % 2) as f32 * (bw + 10.);
                let by = y + (idx / 2) as f32 * (bh + 10.);
                let br = Rect::from_xywh(bx, by, bw, bh);
                let mut fp = Paint::default();
                fp.set_anti_alias(true);
                fp.set_color(col);
                c.draw_round_rect(br, 14., 14., &fp);

                let icx = bx + 18.;
                let icy = by + 18.;
                let mut ip = Paint::default();
                ip.set_anti_alias(true);
                ip.set_style(Style::Stroke);
                ip.set_stroke_width(1.7);
                ip.set_stroke_cap(Cap::Round);
                ip.set_color(Color::WHITE);
                match idx {
                    0 => {
                        let mut f = Paint::default();
                        f.set_anti_alias(true);
                        f.set_color(Color::WHITE);
                        c.draw_round_rect(Rect::from_xywh(icx - 7., icy - 5.5, 6., 3.), 1., 1., &f);
                        c.draw_round_rect(Rect::from_xywh(icx - 7., icy - 4., 14., 9.5), 2., 2., &f);
                    }
                    1 => {
                        let mut tp = Path::new();
                        tp.move_to((icx - 5., icy - 3.5));
                        tp.line_to((icx, icy));
                        tp.line_to((icx - 5., icy + 3.5));
                        c.draw_path(&tp, &ip);
                        c.draw_line((icx + 1., icy + 3.5), (icx + 6., icy + 3.5), &ip);
                    }
                    2 => {
                        c.draw_circle((icx, icy), 6.2, &ip);
                        c.draw_line((icx - 6.2, icy), (icx + 6.2, icy), &ip);
                        c.draw_oval(Rect::from_xywh(icx - 3., icy - 6.2, 6., 12.4), &ip);
                    }
                    _ => {
                        c.draw_round_rect(Rect::from_xywh(icx - 5.5, icy - 6.5, 11., 13.), 2.2, 2.2, &ip);
                        c.draw_line((icx - 3., icy - 2.), (icx + 3., icy - 2.), &ip);
                        c.draw_line((icx - 3., icy + 2.), (icx + 3., icy + 2.), &ip);
                    }
                }
                text_styled(c, name, bx + 10., br.bottom - 22., bw - 16., 11.5, Color::WHITE, Weight::BOLD, TextAlign::Left, 1);
            }
        }
    }

    if card.stack.len() >= 2 {
        let total = card.stack.len();
        let dot_x = r.right - 9.0;
        let gap = 7.5;
        let cap_h = (total - 1) as f32 * gap + 11.0;
        let sy = r.center_y() - (total - 1) as f32 * gap * 0.5;
        glass::liquid_pill(c, Rect::from_xywh(dot_x - 4.5, r.center_y() - cap_h * 0.5, 9., cap_h), 4.5, v.dark, false);
        for i in 0..total {
            let mut dp = Paint::default();
            dp.set_anti_alias(true);
            dp.set_color(if i == card.page {
                accent_blue(v.dark)
            } else {
                Color::from_argb(95, 100, 116, 139)
            });
            c.draw_circle((dot_x, sy + i as f32 * gap), if i == card.page { 2.4 } else { 1.6 }, &dp);
        }
        hits.push(HitItem::new(
            Rect::from_xywh(r.right - 26., r.top + 8., 22., r.height() - 16.),
            "切换叠放下一张",
            Action::Turn(card.id, 1),
        ));
    }

    if v.edit {
        let del_r = Rect::from_xywh(r.right - 30., r.top + 8., 22., 22.);
        let hovered = v.hover.is_some_and(|p| del_r.contains(Point::from(p)));
        let mut dp = Paint::default();
        dp.set_anti_alias(true);
        dp.set_color(if hovered {
            Color::from_rgb(255, 45, 36)
        } else {
            Color::from_rgb(255, 59, 48)
        });
        c.draw_circle((del_r.center_x(), del_r.center_y()), 11., &dp);
        let mut xp = Paint::default();
        xp.set_anti_alias(true);
        xp.set_style(Style::Stroke);
        xp.set_stroke_width(1.8);
        xp.set_stroke_cap(Cap::Round);
        xp.set_color(Color::WHITE);
        c.draw_line((del_r.center_x() - 4., del_r.center_y() - 4.), (del_r.center_x() + 4., del_r.center_y() + 4.), &xp);
        c.draw_line((del_r.center_x() + 4., del_r.center_y() - 4.), (del_r.center_x() - 4., del_r.center_y() + 4.), &xp);
        hits.push(HitItem::new(del_r, "移除小组件", Action::Remove(card.id)));
    }
    c.restore();
}

pub fn desktop(c: &Canvas, width: f32, height: f32, v: &View<'_>) -> Vec<HitItem> {
    let mut hits = vec![];
    if v.edit {
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_color(Color::from_argb(80, 40, 90, 180));
        for x in (0..width as usize).step_by(192) {
            for y in (0..height as usize).step_by(192) {
                c.draw_circle((x as f32 + 32., y as f32 + 32.), 2., &p);
            }
        }
        let bar = Rect::from_xywh((width - 398.) * 0.5, 18., 398., 44.);
        glass::panel_radius(
            c,
            bar,
            22.,
            v.dark,
            2,
            v.data.blurred_wallpaper(2).map(|img| (img, (width, height))),
        );
        button(
            c,
            Rect::from_xywh(bar.left + 8., bar.top + 6., 124., 32.),
            "＋ 添加组件",
            Action::AddDrawer,
            v,
            &mut hits,
        );
        button(
            c,
            Rect::from_xywh(bar.left + 140., bar.top + 6., 136., 32.),
            "⚙ 侧边栏设置",
            Action::OpenTutorial,
            v,
            &mut hits,
        );
        button(
            c,
            Rect::from_xywh(bar.left + 284., bar.top + 6., 106., 32.),
            "完成编辑",
            Action::Edit,
            v,
            &mut hits,
        );
    }
    for item in &v.board.cards {
        let (w, h) = item.size.dimensions();
        let backdrop = v.data.blurred_wallpaper(item.acrylic).map(|img| (img, (width, height)));
        card(c, Rect::from_xywh(item.x, item.y, w, h), item, v, &mut hits, backdrop);
    }
    overlays(
        c,
        v,
        width,
        height,
        &mut hits,
        v.data.blurred_wallpaper(2).map(|img| (img, (width, height))),
    );
    hits
}

fn live_card(c:&Canvas,r:Rect,card:&Card,v:&View<'_>,x:f32,y:f32,w:f32,fg:Color,sub:Color,hits:&mut Vec<HitItem>) {
 let data=&v.data.live;
 let string=|key:&str,fallback:&str|data[key].as_str().unwrap_or(fallback).to_string();
 let mut title=card.title.clone();let mut value:String;let mut detail:String;
 match card.active(){
  "music"=>{title=string("title","暂无播放");value=string("artist","");detail=if data["isPlaying"].as_bool()==Some(true){"正在播放"}else{"已暂停"}.into();}
  "todo"=>{let todos=data["todos"].as_array();let pending:Vec<_>=todos.into_iter().flatten().filter(|t|t["completed"].as_bool()!=Some(true)).collect();value=format!("{} 项待办",pending.len());detail=pending.first().and_then(|t|t["title"].as_str()).unwrap_or("今天的待办已清空").into();}
  "focus"=>{let t=data["timers"].as_array().and_then(|t|t.first());if let Some(t)=t{value=t["clock"].as_str().unwrap_or("准备开始").into();detail=t["subtitle"].as_str().unwrap_or("").into();}else{value="准备开始".into();detail="在控制中心设置计时".into();}}
  "weather"=>{
   title=data["settings"]["WeatherCity"].as_str().unwrap_or("选择城市").into();value="等待天气数据".into();detail="在控制中心设置城市后同步".into();
   for key in ["widgetData"] {
    if let Some(items)=data[key].as_object(){for weather in items.values(){if let Some(t)=weather["temperature"].as_f64(){value=format!("{t:.0}°");detail=weather["city"].as_str().unwrap_or(&title).into();break;}}}
   }
  }
  "screen_time"=>{value=data["screenTime"]["todayMs"].as_f64().map(|ms|format!("{:.0} 分钟",ms/60000.)).unwrap_or("等待用机记录".into());detail="今日活跃用机 · 点屿运行期间".into();}
  "focus_stats"|"github_grid"=>{value=data["widgetData"]["focusStats"]["todayMs"].as_f64().map(|ms|format!("{:.0} 分钟",ms/60000.)).unwrap_or("暂无专注记录".into());detail="今日专注".into();}
  "today"=>{let n=data["todos"].as_array().map(|ts|ts.iter().filter(|t|t["completed"].as_bool()!=Some(true)).count()).unwrap_or(0);value=format!("{n} 项待办");detail="今日概览 · 点击打开控制中心".into();}
  "system"=>{let m=&v.data.metrics;value=format!("CPU {} · 内存 {}",m.cpu.map(|v|format!("{v:.0}%")).unwrap_or("—".into()),m.memory.map(|v|format!("{v:.0}%")).unwrap_or("—".into()));detail=m.battery.map(|v|format!("本机电量 {v}%")).unwrap_or("未提供本机电池读数".into());}
  "storage"=>{value=v.data.metrics.disk.map(|v|format!("已用 {v:.0}%")).unwrap_or("等待磁盘读数".into());detail="系统盘空间".into();}
  "wireless"=>{let dev=data["systemMetrics"]["wirelessDevices"].as_array().and_then(|ds|ds.iter().find(|d|d["connected"].as_bool()==Some(true)));if let Some(d)=dev{title=d["name"].as_str().unwrap_or("无线设备").into();value=d["batteryPercent"].as_u64().map(|p|format!("{p}%")).unwrap_or("未提供电量".into());detail="已连接".into();}else{value="暂无设备电量".into();detail="仅显示设备真实上报的读数".into();}}
  "activity"=>{value="暂无活动数据".into();detail="尚未连接活动数据来源".into();}
  "converter"=>{value="暂无汇率数据".into();detail="尚未接入实时汇率服务".into();}
  _=>{value="尚未设置".into();detail="在控制中心配置后查看".into();}
 }
 let white=matches!(card.active(),"weather"|"music");let ink=if white{Color::WHITE}else{fg};let muted=if white{Color::from_argb(210,255,255,255)}else{sub};
 text_styled(c,&title,x,y,w,13.,ink,Weight::SEMI_BOLD,TextAlign::Left,1);
 text_styled(c,&value,x,y+40.,w,if card.active()=="weather"{34.}else{22.},ink,Weight::BOLD,TextAlign::Left,2);
 text_styled(c,&detail,x,r.bottom-42.,w,12.,muted,Weight::NORMAL,TextAlign::Left,1);
 hits.push(HitItem::new(r,"打开控制中心",Action::ToggleSettings));
}
