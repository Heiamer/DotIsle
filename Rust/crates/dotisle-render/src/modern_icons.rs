//! 灵动岛音乐展开卡片原创 24×24 光学网格矢量图标集（圆润比例、统一线宽与重心补偿）。
use vello::{
    kurbo::{Affine, BezPath, Cap, Circle, Join, Rect, RoundedRect, Stroke},
    peniko::{Color, Fill},
    Scene,
};
use crate::transport_icons::Glyph;

fn col(argb: u32) -> Color {
    Color::from_rgba8((argb >> 16) as u8, (argb >> 8) as u8, argb as u8, (argb >> 24) as u8)
}

fn fill_path(scene: &mut Scene, tr: Affine, p: &BezPath, c: Color) {
    scene.fill(Fill::NonZero, tr, c, None, p);
}

fn stroke_path(scene: &mut Scene, tr: Affine, p: &BezPath, width: f64, c: Color) {
    let s = Stroke::new(width).with_caps(Cap::Round).with_join(Join::Round);
    scene.stroke(&s, tr, c, None, p);
}

/// 绘制全新设计的高识别度音乐控制卡片矢量图标。
pub fn paint(scene: &mut Scene, glyph: Glyph, r: Rect, argb: u32) {
    let c = col(argb);
    let tr = Affine::translate((r.x0, r.y0))
        * Affine::scale_non_uniform(r.width() / 24., r.height() / 24.);

    match glyph {
        Glyph::Play => {
            // 重心右移光学平衡的圆角实心播放三角
            let mut p = BezPath::new();
            p.move_to((8.4, 5.8));
            p.curve_to((8.4, 4.75), (9.55, 4.15), (10.45, 4.7));
            p.line_to((18.6, 10.65));
            p.curve_to((19.45, 11.22), (19.45, 12.78), (18.6, 13.35));
            p.line_to((10.45, 19.3));
            p.curve_to((9.55, 19.85), (8.4, 19.25), (8.4, 18.2));
            p.close_path();
            fill_path(scene, tr, &p, c);
        }
        Glyph::Pause => {
            // 双圆角饱满胶囊竖条
            let left = RoundedRect::from_rect(Rect::new(6.8, 5.2, 10.7, 18.8), 1.9);
            let right = RoundedRect::from_rect(Rect::new(13.3, 5.2, 17.2, 18.8), 1.9);
            scene.fill(Fill::NonZero, tr, c, None, &left);
            scene.fill(Fill::NonZero, tr, c, None, &right);
        }
        Glyph::Next => {
            // 饱满圆角前向箭头 + 圆头立柱
            let mut arrow = BezPath::new();
            arrow.move_to((5.2, 6.4));
            arrow.curve_to((5.2, 5.45), (6.25, 4.92), (7.05, 5.45));
            arrow.line_to((15.2, 10.85));
            arrow.curve_to((15.9, 11.35), (15.9, 12.65), (15.2, 13.15));
            arrow.line_to((7.05, 18.55));
            arrow.curve_to((6.25, 19.08), (5.2, 18.55), (5.2, 17.6));
            arrow.close_path();
            fill_path(scene, tr, &arrow, c);
            let bar = RoundedRect::from_rect(Rect::new(16.9, 5.6, 19.3, 18.4), 1.2);
            scene.fill(Fill::NonZero, tr, c, None, &bar);
        }
        Glyph::Prev => {
            // 饱满圆角回退箭头 + 圆头立柱
            let bar = RoundedRect::from_rect(Rect::new(4.7, 5.6, 7.1, 18.4), 1.2);
            scene.fill(Fill::NonZero, tr, c, None, &bar);
            let mut arrow = BezPath::new();
            arrow.move_to((18.8, 6.4));
            arrow.curve_to((18.8, 5.45), (17.75, 4.92), (16.95, 5.45));
            arrow.line_to((8.8, 10.85));
            arrow.curve_to((8.1, 11.35), (8.1, 12.65), (8.8, 13.15));
            arrow.line_to((16.95, 18.55));
            arrow.curve_to((17.75, 19.08), (18.8, 18.55), (18.8, 17.6));
            arrow.close_path();
            fill_path(scene, tr, &arrow, c);
        }
        Glyph::Heart => {
            // 舒展对称苹果级双弧心形
            let mut p = BezPath::new();
            p.move_to((12.0, 20.1));
            p.curve_to((11.6, 19.8), (4.2, 14.95), (3.4, 9.75));
            p.curve_to((2.75, 6.2), (5.55, 3.8), (8.45, 4.1));
            p.curve_to((10.2, 4.28), (11.35, 5.35), (12.0, 6.55));
            p.curve_to((12.65, 5.35), (13.8, 4.28), (15.55, 4.1));
            p.curve_to((18.45, 3.8), (21.25, 6.2), (20.6, 9.75));
            p.curve_to((19.8, 14.95), (12.4, 19.8), (12.0, 20.1));
            p.close_path();
            stroke_path(scene, tr, &p, 1.9, c);
        }
        Glyph::Volume => {
            // 精致圆角音箱主体 + 两道同心弧形声波
            let mut body = BezPath::new();
            body.move_to((3.6, 10.0));
            body.curve_to((3.6, 9.15), (4.25, 8.6), (5.05, 8.6));
            body.line_to((7.7, 8.6));
            body.line_to((12.15, 5.15));
            body.curve_to((12.95, 4.55), (13.9, 5.1), (13.9, 6.1));
            body.line_to((13.9, 17.9));
            body.curve_to((13.9, 18.9), (12.95, 19.45), (12.15, 18.85));
            body.line_to((7.7, 15.4));
            body.line_to((5.05, 15.4));
            body.curve_to((4.25, 15.4), (3.6, 14.85), (3.6, 14.0));
            body.close_path();
            stroke_path(scene, tr, &body, 1.75, c);

            let mut w1 = BezPath::new();
            w1.move_to((16.75, 9.15));
            w1.curve_to((17.85, 10.65), (17.85, 13.35), (16.75, 14.85));
            stroke_path(scene, tr, &w1, 1.75, c);

            let mut w2 = BezPath::new();
            w2.move_to((19.25, 6.85));
            w2.curve_to((21.15, 9.75), (21.15, 14.25), (19.25, 17.15));
            stroke_path(scene, tr, &w2, 1.75, c);
        }
        Glyph::Translate => {
            // 简洁干净「文 / A」双语翻译徽标
            let mut p = BezPath::new();
            // 「文」字结构
            p.move_to((8.2, 3.6));
            p.line_to((8.2, 5.8));
            p.move_to((3.4, 5.9));
            p.line_to((13.0, 5.9));
            p.move_to((11.1, 6.1));
            p.curve_to((10.2, 10.3), (7.6, 13.1), (3.8, 14.9));
            p.move_to((5.4, 8.4));
            p.curve_to((7.1, 11.4), (9.6, 13.5), (12.6, 14.7));
            // 右下字母 A
            p.move_to((13.2, 20.2));
            p.line_to((16.8, 11.4));
            p.line_to((20.4, 20.2));
            p.move_to((14.4, 17.5));
            p.line_to((19.2, 17.5));
            stroke_path(scene, tr, &p, 1.75, c);
        }
        Glyph::Settings => {
            // 6 齿精密流畅几何微齿轮 + 中心光学圆环
            let center = (12.0, 12.0);
            let mut gear = BezPath::new();
            let pts = 24;
            for i in 0..pts {
                let a = (i as f64) * std::f64::consts::TAU / (pts as f64) - std::f64::consts::FRAC_PI_2;
                let r_cur = match i % 4 {
                    0 | 1 => 9.1,
                    _ => 6.9,
                };
                let pt = (center.0 + r_cur * a.cos(), center.1 + r_cur * a.sin());
                if i == 0 {
                    gear.move_to(pt);
                } else {
                    gear.line_to(pt);
                }
            }
            gear.close_path();
            stroke_path(scene, tr, &gear, 1.65, c);
            scene.stroke(&Stroke::new(1.75), tr, c, None, &Circle::new(center, 2.75));
        }
        Glyph::Shuffle => {
            // 自由悬浮四向移动拖拽锚标（对称圆角双轴箭头）
            let mut p = BezPath::new();
            p.move_to((12.0, 3.6));
            p.line_to((12.0, 20.4));
            p.move_to((3.6, 12.0));
            p.line_to((20.4, 12.0));
            // 上/下箭头
            p.move_to((9.2, 6.2));
            p.line_to((12.0, 3.4));
            p.line_to((14.8, 6.2));
            p.move_to((9.2, 17.8));
            p.line_to((12.0, 20.6));
            p.line_to((14.8, 17.8));
            // 左/右箭头
            p.move_to((6.2, 9.2));
            p.line_to((3.4, 12.0));
            p.line_to((6.2, 14.8));
            p.move_to((17.8, 9.2));
            p.line_to((20.6, 12.0));
            p.line_to((17.8, 14.8));
            stroke_path(scene, tr, &p, 1.75, c);
        }
    }
}
