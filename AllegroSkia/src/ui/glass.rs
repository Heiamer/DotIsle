use skia_safe::{
    Canvas, Color, FilterMode, Image, MipmapMode, Paint, Point, RRect, Rect, SamplingOptions,
    TileMode, canvas::SrcRectConstraint, gradient_shader, paint::Style,
};

/// 透明亚克力面板标准圆角版（桌面/组件通用 `22.0` Apple 圆角外观）。
#[allow(dead_code)]
pub fn panel(
    c: &Canvas,
    r: Rect,
    dark: bool,
    level: u8,
    backdrop: Option<(&Image, (f32, f32))>,
) {
    panel_radius(c, r, 22., dark, level, backdrop);
}

/// Apple macOS Notification Center 侧边栏毛玻璃主托盘：
/// 按实际画布坐标切片采样预模糊壁纸 + 中性毛玻璃底罩 + 细腻苹果外/内框。
pub fn panel_radius(
    c: &Canvas,
    r: Rect,
    rad: f32,
    dark: bool,
    level: u8,
    backdrop: Option<(&Image, (f32, f32))>,
) {
    draw_apple_shadow(c, r, rad, dark);

    c.save();
    c.clip_rrect(RRect::new_rect_xy(r, rad, rad), None, true);
    draw_backdrop_slice(c, r, backdrop);

    let lvl = level.min(3);
    let (top_a, bot_a) = match (backdrop.is_some(), dark) {
        (true, false) => (148 + lvl * 16, 128 + lvl * 14),
        (true, true) => (156 + lvl * 14, 178 + lvl * 14),
        (false, false) => (176 + lvl * 14, 156 + lvl * 14),
        (false, true) => (166 + lvl * 14, 188 + lvl * 14),
    };
    let tint_cols = if dark {
        [
            Color::from_argb(top_a, 30, 32, 38),
            Color::from_argb(bot_a, 22, 24, 28),
        ]
    } else {
        [
            Color::from_argb(top_a, 248, 250, 253),
            Color::from_argb(bot_a, 238, 242, 248),
        ]
    };
    if let Some(sh) = gradient_shader::linear(
        (Point::new(r.left, r.top), Point::new(r.left, r.bottom)),
        &tint_cols[..],
        None,
        TileMode::Clamp,
        None,
        None,
    ) {
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        fill.set_shader(sh);
        c.draw_rect(r, &fill);
    }
    c.restore();

    draw_apple_rim(c, r, rad, dark);
}

/// Apple iOS 18 / macOS Sequoia 原生卡片底壳：
/// 带轻盈环境投影、真实背景位置轻微透射，以及饱满高对比苹果渐变/纯色底层与精致细高光内框。
pub fn apple_card_shell(
    c: &Canvas,
    r: Rect,
    rad: f32,
    dark: bool,
    fill_top: Color,
    fill_bot: Color,
    backdrop: Option<(&Image, (f32, f32))>,
) {
    draw_apple_shadow(c, r, rad, dark);

    c.save();
    c.clip_rrect(RRect::new_rect_xy(r, rad, rad), None, true);
    draw_backdrop_slice(c, r, backdrop);

    let cols = [fill_top, fill_bot];
    if let Some(sh) = gradient_shader::linear(
        (Point::new(r.left, r.top), Point::new(r.left, r.bottom)),
        &cols[..],
        None,
        TileMode::Clamp,
        None,
        None,
    ) {
        let mut fill = Paint::default();
        fill.set_anti_alias(true);
        fill.set_shader(sh);
        c.draw_rect(r, &fill);
    }
    c.restore();

    draw_apple_rim(c, r, rad, dark);
}

fn draw_apple_shadow(c: &Canvas, r: Rect, rad: f32, dark: bool) {
    let shadow_layers = [
        (10.0_f32, 7.0_f32, if dark { 22 } else { 10 }),
        (5.0_f32, 3.5_f32, if dark { 26 } else { 12 }),
        (1.5_f32, 1.2_f32, if dark { 30 } else { 14 }),
    ];
    for (expand, dy, alpha) in shadow_layers {
        let mut sp = Paint::default();
        sp.set_anti_alias(true);
        sp.set_color(Color::from_argb(alpha, 0, 0, 0));
        c.draw_round_rect(
            Rect::from_xywh(
                r.left - expand * 0.5,
                r.top + dy,
                r.width() + expand,
                r.height() + expand * 0.45,
            ),
            rad + expand * 0.35,
            rad + expand * 0.35,
            &sp,
        );
    }
}

fn draw_backdrop_slice(c: &Canvas, r: Rect, backdrop: Option<(&Image, (f32, f32))>) {
    if let Some((image, (cw, ch))) = backdrop
        && cw > 1.0
        && ch > 1.0
    {
        let iw = image.width().max(1) as f32;
        let ih = image.height().max(1) as f32;
        let u0 = (r.left / cw).clamp(0.0, 1.0);
        let v0 = (r.top / ch).clamp(0.0, 1.0);
        let u1 = (r.right / cw).clamp(0.0, 1.0);
        let v1 = (r.bottom / ch).clamp(0.0, 1.0);
        if u1 > u0 && v1 > v0 {
            let src = Rect::from_ltrb(u0 * iw, v0 * ih, u1 * iw, v1 * ih);
            c.draw_image_rect_with_sampling_options(
                image,
                Some((&src, SrcRectConstraint::Fast)),
                r,
                SamplingOptions::new(FilterMode::Linear, MipmapMode::None),
                &Paint::default(),
            );
        }
    }
}

fn draw_apple_rim(c: &Canvas, r: Rect, rad: f32, dark: bool) {
    let mut outer = Paint::default();
    outer.set_anti_alias(true);
    outer.set_style(Style::Stroke);
    outer.set_stroke_width(0.75);
    outer.set_color(if dark {
        Color::from_argb(34, 255, 255, 255)
    } else {
        Color::from_argb(22, 0, 0, 0)
    });
    c.draw_round_rect(r, rad, rad, &outer);

    let inner_r = Rect::from_xywh(
        r.left + 0.75,
        r.top + 0.75,
        (r.width() - 1.5).max(2.),
        (r.height() - 1.5).max(2.),
    );
    let mut inner = Paint::default();
    inner.set_anti_alias(true);
    inner.set_style(Style::Stroke);
    inner.set_stroke_width(0.8);
    inner.set_color(if dark {
        Color::from_argb(38, 255, 255, 255)
    } else {
        Color::from_argb(165, 255, 255, 255)
    });
    c.draw_round_rect(inner_r, (rad - 0.75).max(1.5), (rad - 0.75).max(1.5), &inner);
}

/// 统一绘制精简高光描边（用于按钮和胶囊）。
pub fn draw_specular_rim(c: &Canvas, r: Rect, rad: f32, dark: bool, active: bool) {
    let mut outer = Paint::default();
    outer.set_anti_alias(true);
    outer.set_style(Style::Stroke);
    outer.set_stroke_width(0.65);
    outer.set_color(if dark {
        Color::from_argb(if active { 80 } else { 32 }, 255, 255, 255)
    } else {
        Color::from_argb(if active { 40 } else { 20 }, 0, 0, 0)
    });
    c.draw_round_rect(r, rad, rad, &outer);
}

/// Apple 风格圆角胶囊控件。
pub fn liquid_pill(c: &Canvas, r: Rect, rad: f32, dark: bool, active: bool) {
    let mut fill = Paint::default();
    fill.set_anti_alias(true);
    fill.set_color(match (dark, active) {
        (true, true) => Color::from_argb(88, 255, 255, 255),
        (true, false) => Color::from_argb(42, 255, 255, 255),
        (false, true) => Color::from_argb(235, 255, 255, 255),
        (false, false) => Color::from_argb(135, 255, 255, 255),
    });
    c.draw_round_rect(r, rad, rad, &fill);
    draw_specular_rim(c, r, rad, dark, active);
}

/// Apple 风格圆形系统按钮。
pub fn liquid_circle(c: &Canvas, cx: f32, cy: f32, rad: f32, dark: bool, active: bool) {
    let r = Rect::from_xywh(cx - rad, cy - rad, rad * 2., rad * 2.);
    liquid_pill(c, r, rad, dark, active);
}

/// 局部柔和光晕（如天气卡片右上暖阳光场）。
pub fn glow_blob(
    c: &Canvas,
    r: Rect,
    rad: f32,
    rgb: (u8, u8, u8),
    cx_frac: f32,
    cy_frac: f32,
    radius_frac: f32,
    alpha: u8,
) {
    c.save();
    c.clip_rrect(RRect::new_rect_xy(r, rad, rad), None, true);
    let center = Point::new(r.left + r.width() * cx_frac, r.top + r.height() * cy_frac);
    let radius = r.width().max(r.height()) * radius_frac;
    let cols = [
        Color::from_argb(alpha, rgb.0, rgb.1, rgb.2),
        Color::from_argb(alpha / 3, rgb.0, rgb.1, rgb.2),
        Color::from_argb(0, rgb.0, rgb.1, rgb.2),
    ];
    let pos = [0.0_f32, 0.50, 1.0];
    if let Some(sh) =
        gradient_shader::radial(center, radius, &cols[..], &pos[..], TileMode::Clamp, None, None)
    {
        let mut p = Paint::default();
        p.set_shader(sh);
        c.draw_rect(r, &p);
    }
    c.restore();
}

/// Apple 物理卡片叠层底部厚度边。
pub fn stack_depth(c: &Canvas, r: Rect, total: usize, dark: bool) {
    if total < 2 {
        return;
    }
    let layers = (total - 1).min(2);
    for depth in (1..=layers).rev() {
        let inset = depth as f32 * 10.0;
        let drop = depth as f32 * 4.5;
        let lr = Rect::from_xywh(
            r.left + inset,
            r.top + drop,
            (r.width() - inset * 2.0).max(24.0),
            r.height(),
        );
        let rad = (22.0 - depth as f32 * 3.0).max(12.0);
        let mut p = Paint::default();
        p.set_anti_alias(true);
        p.set_color(if dark {
            Color::from_argb((120 - depth as u8 * 25).max(40), 40, 42, 48)
        } else {
            Color::from_argb((170 - depth as u8 * 35).max(60), 245, 245, 248)
        });
        c.draw_round_rect(lr, rad, rad, &p);
        draw_specular_rim(c, lr, rad, dark, false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_safe::{Data, RuntimeEffect};
    #[test]
    fn filters_shader_textlayout() {
        let effect = RuntimeEffect::make_for_shader(
            "half4 main(float2 p){return half4(0.2,0.4,0.8,0.8);}",
            None,
        )
        .unwrap();
        assert!(effect.make_shader(Data::new_empty(), &[], None).is_some());
        let png = crate::surface::render_offscreen_png(220, 220, 1., |c| {
            let r = Rect::from_xywh(20., 20., 180., 170.);
            stack_depth(c, r, 3, false);
            apple_card_shell(
                c,
                r,
                22.,
                false,
                Color::from_argb(235, 255, 255, 255),
                Color::from_argb(235, 245, 247, 250),
                None,
            );
            liquid_pill(c, Rect::from_xywh(36., 140., 148., 28.), 14., false, true);
            crate::ui::text(c, "Apple 风格", 36., 42., 140., 18., Color::BLACK);
        })
        .unwrap();
        assert!(png.len() > 800);
    }
}
