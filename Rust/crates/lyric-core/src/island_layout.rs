//! 灵动岛基础几何与展开形变路径（支持顶部经典模式与底部任务栏向上弹出模式）。
use crate::music_layout::Rect;
fn blend(p: f64, a: f64, b: f64) -> f64 {
    let t = ((p - a) / (b - a)).clamp(0., 1.);
    t * t * (3. - 2. * t)
}
#[derive(Clone, Copy)]
pub struct Layout {
    pub capsule: Rect,
    pub card: Rect,
    pub host: Rect,
    pub anchor_bottom: bool,
    pub taskbar_height: f64,
}
impl Layout {
    /// Host 预留最大高度，切换组件时不需反复重建窗口大小。
    pub fn new(width: f64) -> Self {
        let cw = (width + 112.).max(400.);
        let host = cw.max(440.);
        Self {
            capsule: Rect::new(24. + (host - width) / 2., 24., width, 40.),
            card: Rect::new(24. + (host - cw) / 2., 24., cw, 264.),
            host: Rect::new(0., 0., host + 48., 332.),
            anchor_bottom: false,
            taskbar_height: 48.,
        }
    }
    /// 切换底部任务栏停靠模式：胶囊垂直居中于任务栏槽位，展开卡片位于胶囊上方 10dp。
    pub fn configure_anchor(&mut self, anchor_bottom: bool, taskbar_height: f64) {
        self.anchor_bottom = anchor_bottom;
        self.taskbar_height = taskbar_height.clamp(36., 76.);
        self.sync_vertical();
    }
    pub fn sync_vertical(&mut self) {
        if self.anchor_bottom {
            let tb = self.taskbar_height.clamp(36., 76.);
            // 贴合 Windows 11 原生任务栏控件尺寸（48dp 任务栏下约 38dp 高，上下保留 5dp 安全呼吸边距）
            let cap_h = (tb - 10.).clamp(32., 38.);
            let min_host_h = (tb + 12. + self.card.h + 24.).max(332.);
            if self.host.h < min_host_h {
                self.host.h = min_host_h;
            }
            self.capsule.h = cap_h;
            self.capsule.y = self.host.h - tb + (tb - cap_h) / 2.;
            self.card.y = (self.host.h - tb - 10. - self.card.h).max(16.);
        } else {
            self.capsule.y = 24.;
            self.card.y = 24.;
        }
    }
    pub fn align_dock(&mut self, capsule_center_x: f64, card_center_x: f64) {
        let cap_x = (capsule_center_x - self.capsule.w / 2.)
            .clamp(8., (self.host.w - self.capsule.w - 8.).max(8.));
        let card_x = (card_center_x - self.card.w / 2.)
            .clamp(12., (self.host.w - self.card.w - 12.).max(12.));
        self.capsule.x = cap_x;
        self.card.x = card_x;
    }
    pub fn reserve_compact(&mut self, max: f64) {
        let width = (max + 48.).max(self.host.w);
        self.host.w = width;
        self.capsule.x = (width - self.capsule.w) / 2.;
        self.card.x = (width - self.card.w) / 2.;
        self.sync_vertical();
    }
    pub fn set_compact(&mut self, width: f64, height: f64) {
        let old_center = self.capsule.x + self.capsule.w / 2.;
        self.capsule.w = width;
        self.capsule.h = height;
        if self.anchor_bottom {
            self.capsule.x = (old_center - width / 2.)
                .clamp(8., (self.host.w - width - 8.).max(8.));
        } else {
            self.capsule.x = (self.host.w - width) / 2.;
        }
        self.sync_vertical();
    }
    pub fn morph_shape(self, p: f64, r: f64) -> (Rect, f64) {
        self.morph_shape_with_radii(p, self.capsule.h / 2., r)
    }
    pub fn morph_shape_with_radii(self, p: f64, closed_radius: f64, r: f64) -> (Rect, f64) {
        // Apple Dynamic Island 形态曲线：宽度领先胶囊步进、高度紧随舒展，并保持严格中轴对称
        let over = p - p.clamp(0., 1.);
        let pc = p.clamp(0., 1.);
        let x_t = blend(pc, 0.04, 0.84) + over.clamp(-0.045, 0.045) * 0.55;
        let y_t = blend(pc, 0.14, 0.96) + over.clamp(-0.045, 0.045) * 0.48;
        let a = self.capsule;
        let b = self.card;
        let arc = (std::f64::consts::PI * pc).sin();
        let y_arc = if self.anchor_bottom { -4.5 * arc } else { -7.0 * arc };
        let w = (a.w + (b.w - a.w) * x_t).clamp(4., self.host.w.max(4.));
        let h = (a.h + (b.h - a.h) * y_t).clamp(4., self.host.h.max(4.));
        let cx = (a.x + a.w / 2.) + ((b.x + b.w / 2.) - (a.x + a.w / 2.)) * x_t;
        let x = (cx - w / 2.).clamp(0., (self.host.w - w).max(0.));
        let y = (a.y + (b.y - a.y) * y_t + y_arc).clamp(0., (self.host.h - h).max(0.));
        let rect = Rect::new(x, y, w, h);
        let cr = closed_radius.clamp(0., a.h / 2.);
        (
            rect,
            (cr + (r - cr) * y_t.clamp(0., 1.))
                .min(rect.w.min(rect.h) / 2.)
                .max(0.),
        )
    }
    pub fn satellite(self) -> Rect {
        Rect::new(
            self.capsule.x + self.capsule.w + 8.,
            self.capsule.y,
            self.capsule.h,
            self.capsule.h,
        )
    }
    pub fn compact_cover(self, scale: f64) -> Rect {
        let round = |v: f64| (v * scale).round_ties_even() / scale;
        Rect::new(
            self.capsule.x + round(7.) + round(3.),
            self.capsule.y + round((self.capsule.h - 24.) / 2.),
            24.,
            24.,
        )
    }
}

#[cfg(test)]
mod independent_radius_tests {
    use super::*;
    #[test]
    fn radii_follow_the_same_geometry_and_remain_independent() {
        let l = Layout::new(260.);
        for closed in [0., 8., 20., 48.] {
            for opened in [8., 30., 48.] {
                let (a, ra) = l.morph_shape_with_radii(0., closed, opened);
                let (b, rb) = l.morph_shape_with_radii(1., closed, opened);
                assert_eq!(a, l.capsule);
                assert_eq!(b, l.card);
                assert_eq!(ra, closed.min(l.capsule.h / 2.));
                assert_eq!(rb, opened);
                for i in 0..100 {
                    let p = i as f64 / 100.;
                    let (shape, r) = l.morph_shape_with_radii(p, closed, opened);
                    assert!(r >= 0. && r <= shape.w.min(shape.h) / 2.);
                    assert_eq!(shape, l.morph_shape(p, opened).0);
                }
            }
        }
    }
    #[test]
    fn taskbar_anchor_bottom_grows_upward_inside_host() {
        let mut l = Layout::new(240.);
        l.configure_anchor(true, 48.);
        assert!(l.card.y + l.card.h < l.capsule.y);
        assert_eq!(l.morph_shape(0., 28.).0, l.capsule);
        let (end, _) = l.morph_shape(1., 28.);
        assert!((end.x - l.card.x).abs() < 1e-8);
        assert!((end.y - l.card.y).abs() < 1e-8);
        for i in 0..=100 {
            let p = i as f64 / 100.;
            let (s, _) = l.morph_shape(p, 28.);
            assert!(s.x >= 0. && s.y >= 0. && s.x + s.w <= l.host.w && s.y + s.h <= l.host.h);
        }
    }
}
