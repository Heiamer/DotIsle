use super::{Action, HitItem, text_styled};
use crate::reminders::{ActiveAlert, ReminderState};
use skia_safe::{
    Canvas, Color, Contains, Paint, Path, Point, Rect, TileMode,
    font_style::Weight,
    gradient_shader,
    paint::{Cap, Style},
    textlayout::TextAlign,
};

pub const ISLAND_WIN_W: f32 = 420.;
pub const ISLAND_WIN_H: f32 = 112.;

/// 绘制屏幕顶部居中悬浮的「喝水 / 久坐提醒灵动岛」，点击打卡后立即收缩隐藏。
pub fn draw(
    c: &Canvas,
    width: f32,
    _height: f32,
    progress: f32,
    alert: ActiveAlert,
    rem: &ReminderState,
    hover: Option<(f32, f32)>,
) -> Vec<HitItem> {
    let p = progress.clamp(0., 1.);
    if p <= 0.01 {
        return vec![];
    }

    let w = 136. + (376. - 136.) * p;
    let h = 34. + (70. - 34.) * p;
    let x = (width - w) * 0.5;
    let y = 14. * p;
    let rad = (h * 0.5).min(35.);
    let rect = Rect::from_xywh(x, y, w, h);

    let (accent, accent_soft, action) = match alert {
        ActiveAlert::Water => (
            Color::from_rgb(10, 182, 255),
            Color::from_rgb(56, 210, 255),
            Action::IslandAckWater,
        ),
        ActiveAlert::Stand => (
            Color::from_rgb(255, 149, 0),
            Color::from_rgb(255, 184, 56),
            Action::IslandAckStand,
        ),
    };

    // 外层环境光晕 + 软阴影
    let mut shadow = Paint::default();
    shadow.set_anti_alias(true);
    shadow.set_color(Color::from_argb((84. * p) as u8, 0, 0, 0));
    c.draw_round_rect(rect.with_offset((0., 5.)).with_outset((2., 2.)), rad + 2., rad + 2., &shadow);

    let mut glow = Paint::default();
    glow.set_anti_alias(true);
    glow.set_color(Color::from_argb((34. * p) as u8, accent.r(), accent.g(), accent.b()));
    c.draw_round_rect(rect.with_outset((1.5, 1.5)), rad + 1.5, rad + 1.5, &glow);

    // 纯黑 Apple 灵动岛主体药丸
    let mut shell = Paint::default();
    shell.set_anti_alias(true);
    shell.set_color(Color::from_argb((246. * p) as u8, 16, 17, 20));
    c.draw_round_rect(rect, rad, rad, &shell);

    let mut rim = Paint::default();
    rim.set_anti_alias(true);
    rim.set_style(Style::Stroke);
    rim.set_stroke_width(1.0);
    rim.set_color(Color::from_argb((58. * p) as u8, 255, 255, 255));
    c.draw_round_rect(rect, rad, rad, &rim);

    let mut hits = vec![];
    if p > 0.55 {
        let hovered = hover.is_some_and(|pt| rect.contains(Point::from(pt)));

        // 左侧渐变圆形图标徽章
        let icx = x + 31.;
        let icy = rect.center_y();
        let ir = 17.0;
        let grad_cols = [accent_soft, accent];
        if let Some(sh) = gradient_shader::linear(
            (Point::new(icx, icy - ir), Point::new(icx, icy + ir)),
            &grad_cols[..],
            None,
            TileMode::Clamp,
            None,
            None,
        ) {
            let mut ip = Paint::default();
            ip.set_anti_alias(true);
            ip.set_shader(sh);
            c.draw_circle((icx, icy), ir, &ip);
        }

        match alert {
            ActiveAlert::Water => {
                // 白色液滴矢量图形
                let mut drop_p = Path::new();
                drop_p.move_to((icx, icy - 8.));
                drop_p.cubic_to((icx + 6., icy - 1.), (icx + 6.5, icy + 6.5), (icx, icy + 7.5));
                drop_p.cubic_to((icx - 6.5, icy + 6.5), (icx - 6., icy - 1.), (icx, icy - 8.));
                drop_p.close();
                let mut wp = Paint::default();
                wp.set_anti_alias(true);
                wp.set_color(Color::WHITE);
                c.draw_path(&drop_p, &wp);
            }
            ActiveAlert::Stand => {
                // 白色起身站立矢量图形
                let mut sp = Paint::default();
                sp.set_anti_alias(true);
                sp.set_color(Color::WHITE);
                c.draw_circle((icx, icy - 5.5), 2.8, &sp);
                sp.set_style(Style::Stroke);
                sp.set_stroke_width(2.0);
                sp.set_stroke_cap(Cap::Round);
                c.draw_line((icx, icy - 1.5), (icx, icy + 4.5), &sp);
                c.draw_line((icx - 5., icy + 1.), (icx + 5., icy - 1.), &sp);
                c.draw_line((icx, icy + 4.5), (icx - 3.5, icy + 9.), &sp);
                c.draw_line((icx, icy + 4.5), (icx + 3.5, icy + 9.), &sp);
            }
        }

        // 中部主副标题 + 饮水进度细条
        let tx = icx + ir + 12.;
        let pill_w = if alert == ActiveAlert::Water { 96. } else { 86. };
        let tw = (rect.right - 14. - pill_w - 10. - tx).max(120.);

        let (title, sub_line) = match alert {
            ActiveAlert::Water => (
                format!("该喝水啦 · 本次 {} ml", rem.cup_ml),
                format!("今日已饮 {} / {} ml · 点击打卡关闭", rem.drank_ml, rem.goal_ml),
            ),
            ActiveAlert::Stand => (
                format!("久坐提醒 · 已坐 {} 分钟", rem.stand_interval_min),
                format!("起身活动一下 · 今日已起身 {} 次", rem.stand_count),
            ),
        };
        text_styled(c, &title, tx, y + 13., tw, 13.5, Color::WHITE, Weight::BOLD, TextAlign::Left, 1);
        text_styled(
            c,
            &sub_line,
            tx,
            y + 33.,
            tw,
            10.2,
            Color::from_rgb(174, 182, 196),
            Weight::MEDIUM,
            TextAlign::Left,
            1,
        );

        if alert == ActiveAlert::Water {
            let py = y + 53.;
            let mut tp = Paint::default();
            tp.set_anti_alias(true);
            tp.set_color(Color::from_argb(46, 255, 255, 255));
            c.draw_round_rect(Rect::from_xywh(tx, py, tw, 4.), 2., 2., &tp);
            let frac = rem.water_progress().max(0.05);
            tp.set_color(accent);
            c.draw_round_rect(Rect::from_xywh(tx, py, tw * frac, 4.), 2., 2., &tp);
        }

        // 右侧高亮动作按钮
        let btn_r = Rect::from_xywh(rect.right - pill_w - 14., icy - 16., pill_w, 32.);
        let mut bp = Paint::default();
        bp.set_anti_alias(true);
        bp.set_color(if hovered { accent_soft } else { accent });
        c.draw_round_rect(btn_r, 16., 16., &bp);
        let btn_lbl = match alert {
            ActiveAlert::Water => format!("✓ 已喝 +{}ml", rem.cup_ml),
            ActiveAlert::Stand => "✓ 已活动".into(),
        };
        text_styled(
            c,
            &btn_lbl,
            btn_r.left,
            btn_r.top + 8.,
            btn_r.width(),
            11.,
            Color::WHITE,
            Weight::BOLD,
            TextAlign::Center,
            1,
        );

        hits.push(HitItem::new(
            rect,
            match alert {
                ActiveAlert::Water => "点击确认喝完并关闭提醒",
                ActiveAlert::Stand => "点击确认已起身活动并关闭提醒",
            },
            action,
        ));
    }

    hits
}
