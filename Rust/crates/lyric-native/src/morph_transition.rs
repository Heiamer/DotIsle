//! 灵动岛、蓝条与 Mini 岛四向切换的解析式物理弹簧形变引擎。
//! 采用独立轴二阶阻尼谐振子，支持任意时刻无跳变打断与反向回弹。
use crate::layout::Rect;

/// 灵动岛折叠/展开态的四向外观形态（含从蓝条直接展开大卡片）。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IslandMode {
    Normal,
    Strip,
    Mini,
    Card,
}

/// 单轴解析式二阶阻尼弹簧（欠阻尼 / 临界阻尼 / 过阻尼统一解析求解）。
#[derive(Clone, Copy, Debug, Default)]
struct SpringAxis {
    from: f64,
    to: f64,
    vel: f64,
    start: f64,
    omega: f64,
    zeta: f64,
}

impl SpringAxis {
    /// 按物理时间 `now`（秒）解析计算当前位移与速度。
    fn sample(self, now: f64) -> (f64, f64) {
        let t = (now - self.start).max(0.);
        if t == 0. || !self.omega.is_finite() || self.omega <= 0. {
            return (self.from, self.vel);
        }
        let d = self.from - self.to;
        if d == 0. && self.vel == 0. {
            return (self.to, 0.);
        }
        let w = self.omega;
        let z = self.zeta.max(0.05);
        if (z - 1.).abs() <= 1e-5 {
            // 临界阻尼解析解
            let b = self.vel + w * d;
            let e = (-w * t).exp();
            (self.to + (d + b * t) * e, (self.vel - w * b * t) * e)
        } else if z < 1. {
            // 欠阻尼解析解（带来 Apple Dynamic Island 标志性的轻量果冻微过冲与回吸）
            let wd = w * (1. - z * z).sqrt();
            let a = d;
            let b = (self.vel + z * w * d) / wd;
            let e = (-z * w * t).exp();
            let (sin_t, cos_t) = (wd * t).sin_cos();
            let env = a * cos_t + b * sin_t;
            let pos = self.to + e * env;
            let vel = e * ((-z * w * env) + (b * wd * cos_t - a * wd * sin_t));
            (pos, vel)
        } else {
            // 过阻尼双实根解析解
            let s = (z * z - 1.).sqrt();
            let r1 = -w * (z - s);
            let r2 = -w * (z + s);
            let c1 = (self.vel - r2 * d) / (r1 - r2);
            let c2 = d - c1;
            let e1 = c1 * (r1 * t).exp();
            let e2 = c2 * (r2 * t).exp();
            (self.to + e1 + e2, r1 * e1 + r2 * e2)
        }
    }

    /// 继承实时位置与速度重新设定目标，保证中途打断时位置和速度完全 C¹ 连续。
    fn retarget(&mut self, to: f64, now: f64, omega: f64, zeta: f64) {
        let (pos, vel) = self.sample(now);
        *self = Self {
            from: pos,
            to,
            vel,
            start: now,
            omega,
            zeta,
        };
    }

    /// 动画期间目标尺寸微调时仅更新终点，保留现有惯性速度。
    fn update_target(&mut self, to: f64, now: f64) {
        if (self.to - to).abs() < 1e-6 {
            return;
        }
        let (pos, vel) = self.sample(now);
        self.from = pos;
        self.to = to;
        self.vel = vel;
        self.start = now;
    }

    /// 立即静止落位到指定数值。
    fn snap(&mut self, value: f64, now: f64) {
        *self = Self {
            from: value,
            to: value,
            vel: 0.,
            start: now,
            omega: 20.,
            zeta: 1.,
        };
    }

    /// 判断当前轴是否已收敛至目标容差内。
    fn settled(self, now: f64, pos_eps: f64, vel_eps: f64) -> bool {
        let (pos, vel) = self.sample(now);
        (pos - self.to).abs() <= pos_eps && vel.abs() <= vel_eps
    }
}

/// 传递给 Vello 渲染管线与输入裁剪区的形态展示快照。
#[derive(Clone, Copy, Debug)]
pub struct MorphPresentation {
    pub active: bool,
    pub rect: Rect,
    pub radius: f64,
    pub strip_mix: f64,
    pub mini_mix: f64,
    pub card_mix: f64,
    pub classic_alpha: f32,
    pub mini_alpha: f32,
    pub card_alpha: f32,
    pub softness: f64,
    pub shadow_mul: f64,
}

impl Default for MorphPresentation {
    fn default() -> Self {
        Self {
            active: false,
            rect: Rect::default(),
            radius: 0.,
            strip_mix: 0.,
            mini_mix: 0.,
            card_mix: 0.,
            classic_alpha: 1.,
            mini_alpha: 0.,
            card_alpha: 0.,
            softness: 0.,
            shadow_mul: 1.,
        }
    }
}

/// 四向形态平滑转换控制器。
pub struct IslandMorph {
    active: bool,
    mode: IslandMode,
    from_mode: IslandMode,
    animate: bool,
    speed_scale: f64,
    cx: SpringAxis,
    y: SpringAxis,
    w: SpringAxis,
    h: SpringAxis,
    radius: SpringAxis,
    strip_mix: SpringAxis,
    mini_mix: SpringAxis,
    card_mix: SpringAxis,
}

impl Default for IslandMorph {
    fn default() -> Self {
        Self::new()
    }
}

impl IslandMorph {
    pub fn new() -> Self {
        Self {
            active: false,
            mode: IslandMode::Normal,
            from_mode: IslandMode::Normal,
            animate: true,
            speed_scale: 1.,
            cx: SpringAxis::default(),
            y: SpringAxis::default(),
            w: SpringAxis::default(),
            h: SpringAxis::default(),
            radius: SpringAxis::default(),
            strip_mix: SpringAxis::default(),
            mini_mix: SpringAxis::default(),
            card_mix: SpringAxis::default(),
        }
    }

    pub fn active(&self) -> bool {
        self.active
    }

    pub fn mode(&self) -> IslandMode {
        self.mode
    }

    /// 启动或中途打断切换至新的目标形态（支持 `from_card` 卡片直连过渡）。
    #[allow(clippy::too_many_arguments)]
    pub fn transition_with_card(
        &mut self,
        from_rect: Rect,
        from_radius: f64,
        from_strip: f64,
        from_mini: f64,
        from_card: f64,
        to_mode: IslandMode,
        to_rect: Rect,
        to_radius: f64,
        now: f64,
        animate: bool,
        duration_ms: f64,
    ) {
        let (to_strip, to_mini, to_card) = match to_mode {
            IslandMode::Normal => (0., 0., 0.),
            IslandMode::Strip => (1., 0., 0.),
            IslandMode::Mini => (0., 1., 0.),
            IslandMode::Card => (0., 0., 1.),
        };

        if !animate {
            self.cx.snap(to_rect.x + to_rect.w / 2., now);
            self.y.snap(to_rect.y, now);
            self.w.snap(to_rect.w, now);
            self.h.snap(to_rect.h, now);
            self.radius.snap(to_radius, now);
            self.strip_mix.snap(to_strip, now);
            self.mini_mix.snap(to_mini, now);
            self.card_mix.snap(to_card, now);
            self.active = false;
            self.animate = false;
            self.from_mode = to_mode;
            self.mode = to_mode;
            return;
        }

        // 仅在静止起步时从起点 snap；若已有进行中的形变，直接继承实时位置与速度！
        if !self.active {
            self.cx.snap(from_rect.x + from_rect.w / 2., now);
            self.y.snap(from_rect.y, now);
            self.w.snap(from_rect.w, now);
            self.h.snap(from_rect.h, now);
            self.radius.snap(from_radius, now);
            self.strip_mix.snap(from_strip.clamp(0., 1.), now);
            self.mini_mix.snap(from_mini.clamp(0., 1.), now);
            self.card_mix.snap(from_card.clamp(0., 1.), now);
            self.from_mode = if from_strip > 0.5 {
                IslandMode::Strip
            } else if from_mini > 0.5 {
                IslandMode::Mini
            } else if from_card > 0.5 {
                IslandMode::Card
            } else {
                IslandMode::Normal
            };
        } else {
            self.from_mode = self.mode;
        }

        self.mode = to_mode;
        self.animate = true;
        self.active = true;
        self.speed_scale = (480. / duration_ms.clamp(150., 1200.)).clamp(0.65, 1.45);
        let k = self.speed_scale;

        // 针对各条核心路径调校差异化的 Apple 级弹簧频率与阻尼比，保证收束与直接展开卡片清晰丝滑
        let (p_cx, p_y, p_w, p_h, p_rad, p_strip, p_mini, p_card) = match (self.from_mode, to_mode) {
            // 1. 灵动岛 / Mini 岛 / 大卡片 -> 蓝条：沉稳流畅凝聚成顶部状态条
            (_, IslandMode::Strip) => (
                (15.5, 0.94),
                (15.5, 0.92),
                (14.5, 0.88),
                (13.5, 0.86),
                (15.0, 0.95),
                (14.0, 1.00),
                (18.0, 1.00),
                (18.0, 1.00),
            ),
            // 2. 蓝条 -> 大卡片：一气呵成直接展开为大卡片，无中间停顿
            (IslandMode::Strip, IslandMode::Card) => (
                (16.0, 0.92),
                (16.0, 0.90),
                (14.5, 0.82),
                (16.0, 0.82),
                (15.5, 0.92),
                (24.0, 1.00),
                (20.0, 1.00),
                (16.0, 0.94),
            ),
            // 3. 蓝条 -> 灵动岛：高度领先弹起，宽度带轻量果冻微过冲回吸，蓝条材质平滑让位
            (IslandMode::Strip, IslandMode::Normal) => (
                (16.5, 0.92),
                (16.5, 0.90),
                (15.0, 0.78),
                (17.5, 0.80),
                (16.0, 0.92),
                (19.0, 1.00),
                (18.0, 1.00),
                (20.0, 1.00),
            ),
            // 4. 灵动岛 -> Mini 岛：柔顺收束为细巧胶囊，清晰可见尺寸收缩过程
            (IslandMode::Normal, IslandMode::Mini) => (
                (15.5, 0.94),
                (15.5, 0.92),
                (14.5, 0.86),
                (15.0, 0.85),
                (15.5, 0.95),
                (20.0, 1.00),
                (14.0, 1.00),
                (20.0, 1.00),
            ),
            // 5. 其余展开路径（Mini -> Normal / Normal -> Card 等）
            _ => (
                (16.5, 0.92),
                (16.5, 0.90),
                (15.2, 0.78),
                (17.2, 0.79),
                (16.0, 0.92),
                (20.0, 1.00),
                (16.5, 1.00),
                (15.0, 0.96),
            ),
        };

        self.cx.retarget(to_rect.x + to_rect.w / 2., now, p_cx.0 * k, p_cx.1);
        self.y.retarget(to_rect.y, now, p_y.0 * k, p_y.1);
        self.w.retarget(to_rect.w, now, p_w.0 * k, p_w.1);
        self.h.retarget(to_rect.h, now, p_h.0 * k, p_h.1);
        self.radius.retarget(to_radius, now, p_rad.0 * k, p_rad.1);
        self.strip_mix.retarget(to_strip, now, p_strip.0 * k, p_strip.1);
        self.mini_mix.retarget(to_mini, now, p_mini.0 * k, p_mini.1);
        self.card_mix.retarget(to_card, now, p_card.0 * k, p_card.1);
    }

    /// 兼容原有四参数签名的形态切换入口。
    #[allow(clippy::too_many_arguments)]
    pub fn transition(
        &mut self,
        from_rect: Rect,
        from_radius: f64,
        from_strip: f64,
        from_mini: f64,
        to_mode: IslandMode,
        to_rect: Rect,
        to_radius: f64,
        now: f64,
        animate: bool,
        duration_ms: f64,
    ) {
        self.transition_with_card(
            from_rect,
            from_radius,
            from_strip,
            from_mini,
            0.,
            to_mode,
            to_rect,
            to_radius,
            now,
            animate,
            duration_ms,
        );
    }

    /// 过渡进行中若目标几何尺寸因歌词变化而微调，平滑更新目标位置与大小。
    pub fn update_target(&mut self, to_rect: Rect, to_radius: f64, now: f64) {
        if !self.active {
            return;
        }
        self.cx.update_target(to_rect.x + to_rect.w / 2., now);
        self.y.update_target(to_rect.y, now);
        self.w.update_target(to_rect.w, now);
        self.h.update_target(to_rect.h, now);
        self.radius.update_target(to_radius, now);
    }

    /// 强行中止当前过渡（如切入任务栏模式或无指示隐藏）。
    pub fn cancel(&mut self) {
        self.active = false;
    }

    /// 推进并检查动画是否已全部收敛静止；返回是否仍在活跃动画中。
    pub fn tick(&mut self, now: f64) -> bool {
        if !self.active {
            return false;
        }
        let geom_done = self.cx.settled(now, 0.08, 0.5)
            && self.y.settled(now, 0.08, 0.5)
            && self.w.settled(now, 0.08, 0.5)
            && self.h.settled(now, 0.08, 0.5)
            && self.radius.settled(now, 0.08, 0.5);
        let mix_done = self.strip_mix.settled(now, 0.002, 0.01)
            && self.mini_mix.settled(now, 0.002, 0.01)
            && self.card_mix.settled(now, 0.002, 0.01);
        if geom_done && mix_done {
            self.cx.snap(self.cx.to, now);
            self.y.snap(self.y.to, now);
            self.w.snap(self.w.to, now);
            self.h.snap(self.h.to, now);
            self.radius.snap(self.radius.to, now);
            self.strip_mix.snap(self.strip_mix.to, now);
            self.mini_mix.snap(self.mini_mix.to, now);
            self.card_mix.snap(self.card_mix.to, now);
            self.active = false;
            return false;
        }
        true
    }

    /// 采样给定时刻的形态展示快照，保证几何始终安全落在宿主矩形内。
    pub fn presentation(&self, now: f64, host: Rect) -> MorphPresentation {
        if !self.active {
            return MorphPresentation::default();
        }
        let max_w = (host.w - 8.).max(4.);
        let max_h = (host.h - 8.).max(3.);
        let w = self.w.sample(now).0.clamp(4., max_w);
        let h = self.h.sample(now).0.clamp(3., max_h);
        let cx = self.cx.sample(now).0;
        let x = (cx - w / 2.).clamp(4., (host.w - w - 4.).max(4.));
        let y = self.y.sample(now).0.clamp(2., (host.h - h - 2.).max(2.));
        let radius = self.radius.sample(now).0.clamp(0., w.min(h) / 2.);
        let strip_mix = self.strip_mix.sample(now).0.clamp(0., 1.);
        let mini_mix = self.mini_mix.sample(now).0.clamp(0., 1.);
        let card_mix = self.card_mix.sample(now).0.clamp(0., 1.);

        // 零黑洞内容接力曲线：
        // 1) Strip <-> Card：蓝条直接变为大卡片时，card_alpha 随壳体撑开平顺浮现；
        // 2) Strip <-> Normal <-> Mini：胶囊内容对称互补过渡。
        let card_mask = 1. - smoothstep(card_mix, 0.05, 0.55);
        let classic_from_strip = 1. - smoothstep(strip_mix, 0.05, 0.88);
        let classic_from_mini = 1. - smoothstep(mini_mix, 0.15, 0.85);
        let classic_alpha = (classic_from_strip * classic_from_mini * card_mask) as f32;
        let mini_alpha = (smoothstep(mini_mix, 0.15, 0.85) * classic_from_strip * card_mask) as f32;
        let card_alpha = (smoothstep(card_mix, 0.04, 0.62) * (1. - smoothstep(strip_mix, 0.12, 0.82))) as f32;

        // 仅在快速形变中段施加热力学零端斜率微柔焦，两端静止文字绝对锐利
        let u = strip_mix.max(mini_mix).max(card_mix);
        let softness = if self.animate && self.active {
            let env = 16. * u * u * (1. - u) * (1. - u);
            let s = 1.15 * env;
            if s < 0.015 { 0. } else { s }
        } else {
            0.
        };

        // 阴影强度随蓝条化消失、随 Mini 化柔和至极淡投影、大卡片化保留饱满立体投影
        let mini_shadow = 10. / 56.;
        let base_shadow = 1. - (1. - mini_shadow) * mini_mix * (1. - card_mix);
        let shadow_mul = (base_shadow * (1. - strip_mix)).clamp(0., 1.);

        MorphPresentation {
            active: true,
            rect: Rect::new(x, y, w, h),
            radius,
            strip_mix,
            mini_mix,
            card_mix,
            classic_alpha,
            mini_alpha,
            card_alpha,
            softness,
            shadow_mul,
        }
    }
}

fn smoothstep(x: f64, a: f64, b: f64) -> f64 {
    let span = (b - a).abs().max(1e-6);
    let t = ((x - a) / span).clamp(0., 1.);
    t * t * (3. - 2. * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host() -> Rect {
        Rect::new(0., 0., 528., 332.)
    }
    fn normal() -> Rect {
        Rect::new(134., 24., 260., 40.)
    }
    fn strip() -> Rect {
        Rect::new(236., 24., 56., 5.)
    }
    fn mini() -> Rect {
        Rect::new(174., 24., 180., 14.)
    }

    #[test]
    fn all_four_transitions_land_on_exact_endpoints_and_settle_within_budget() {
        let cases = [
            (IslandMode::Normal, normal(), 20., 0., 0., IslandMode::Strip, strip(), 2.5),
            (IslandMode::Strip, strip(), 2.5, 1., 0., IslandMode::Normal, normal(), 20.),
            (IslandMode::Normal, normal(), 20., 0., 0., IslandMode::Mini, mini(), 7.),
            (IslandMode::Mini, mini(), 7., 0., 1., IslandMode::Normal, normal(), 20.),
        ];
        for (fm, fr, frad, fs, fmin, tm, tr, trad) in cases {
            let mut m = IslandMorph::new();
            m.transition(fr, frad, fs, fmin, tm, tr, trad, 0., true, 480.);
            assert!(m.active());
            let p0 = m.presentation(0., host());
            assert!((p0.rect.w - fr.w).abs() < 1e-6);
            assert!((p0.rect.h - fr.h).abs() < 1e-6);
            assert!((p0.radius - frad).abs() < 1e-6);

            let mut t = 0.;
            while m.tick(t) && t < 1.5 {
                let p = m.presentation(t, host());
                assert!(p.rect.w.is_finite() && p.rect.h.is_finite() && p.rect.w >= 4. && p.rect.h >= 3.);
                assert!(p.radius >= 0. && p.radius <= p.rect.w.min(p.rect.h) / 2. + 1e-9);
                assert!(p.rect.x >= 4. && p.rect.x + p.rect.w <= host().w - 4. + 1e-9);
                t += 1. / 120.;
            }
            assert!(!m.active(), "transition {:?} -> {:?} did not settle (t={t})", fm, tm);
            assert!(t < 1.05, "transition {:?} -> {:?} took too long: {t}s", fm, tm);
            assert!((m.cx.to - (tr.x + tr.w / 2.)).abs() < 1e-6);
            assert!((m.w.from - tr.w).abs() < 1e-6);
            assert!((m.h.from - tr.h).abs() < 1e-6);
        }
    }

    #[test]
    fn restore_transitions_exhibit_subtle_apple_overshoot_without_jagged_oscillation() {
        for (from_rect, from_rad, from_strip, from_mini) in [
            (strip(), 2.5, 1., 0.),
            (mini(), 7., 0., 1.),
        ] {
            let mut m = IslandMorph::new();
            m.transition(
                from_rect,
                from_rad,
                from_strip,
                from_mini,
                IslandMode::Normal,
                normal(),
                20.,
                0.,
                true,
                480.,
            );
            let mut max_w = 0.0_f64;
            let mut min_after_peak = f64::INFINITY;
            let mut passed_target = false;
            for i in 0..=120 {
                let t = i as f64 / 120.;
                let p = m.presentation(t, host());
                max_w = max_w.max(p.rect.w);
                if p.rect.w >= normal().w {
                    passed_target = true;
                }
                if passed_target {
                    min_after_peak = min_after_peak.min(p.rect.w);
                }
            }
            // 应当呈现相对尺寸变化行程 1% ~ 6% 的克制果冻微过冲
            let delta_w = (normal().w - from_rect.w).abs();
            assert!(
                max_w > normal().w + delta_w * 0.01 && max_w < normal().w + delta_w * 0.06,
                "expected subtle overshoot, got max_w={max_w}, delta_w={delta_w}"
            );
            assert!(min_after_peak >= normal().w - 1.5);
        }
    }

    #[test]
    fn mid_flight_interruption_preserves_position_and_velocity_continuity() {
        let mut m = IslandMorph::new();
        m.transition(normal(), 20., 0., 0., IslandMode::Strip, strip(), 2.5, 0., true, 480.);
        for at in [0.02, 0.08, 0.16] {
            let before = m.presentation(at, host());
            let (_, vel_w_before) = m.w.sample(at);
            let (_, vel_h_before) = m.h.sample(at);
            m.transition(normal(), 20., 0., 0., IslandMode::Normal, normal(), 20., at, true, 480.);
            let after = m.presentation(at, host());
            let (_, vel_w_after) = m.w.sample(at);
            let (_, vel_h_after) = m.h.sample(at);
            assert!((before.rect.w - after.rect.w).abs() < 1e-6);
            assert!((before.rect.h - after.rect.h).abs() < 1e-6);
            assert!((before.strip_mix - after.strip_mix).abs() < 1e-6);
            assert!((vel_w_before - vel_w_after).abs() < 1e-6);
            assert!((vel_h_before - vel_h_after).abs() < 1e-6);
        }
    }

    #[test]
    fn reduced_motion_snaps_immediately() {
        let mut m = IslandMorph::new();
        m.transition(normal(), 20., 0., 0., IslandMode::Strip, strip(), 2.5, 1.0, false, 480.);
        assert!(!m.active());
        assert_eq!(m.mode(), IslandMode::Strip);
    }

    #[test]
    fn normal_mini_morph_never_has_blank_black_frame() {
        for (fm, fr, frad, fm_mix, tm, tr, trad) in [
            (IslandMode::Normal, normal(), 20., 0., IslandMode::Mini, mini(), 7.),
            (IslandMode::Mini, mini(), 7., 1., IslandMode::Normal, normal(), 20.),
        ] {
            let mut m = IslandMorph::new();
            m.transition(fr, frad, 0., fm_mix, tm, tr, trad, 0., true, 480.);
            for i in 0..=120 {
                let p = m.presentation(i as f64 / 120., host());
                let sum = p.classic_alpha + p.mini_alpha;
                assert!(
                    sum >= 0.99,
                    "{:?} -> {:?} dropped content sum={sum} at step {i}",
                    fm,
                    tm
                );
                assert!(
                    p.classic_alpha.max(p.mini_alpha) >= 0.5 - 1e-4,
                    "{:?} -> {:?} content dimmer at step {i}: ({}, {})",
                    fm,
                    tm,
                    p.classic_alpha,
                    p.mini_alpha
                );
            }
        }
    }

    #[test]
    fn strip_restore_reveals_content_alongside_dark_shell() {
        let mut m = IslandMorph::new();
        m.transition(strip(), 2.5, 1., 0., IslandMode::Normal, normal(), 20., 0., true, 480.);
        for i in 0..=120 {
            let p = m.presentation(i as f64 / 120., host());
            if p.strip_mix <= 0.65 {
                assert!(
                    p.classic_alpha >= 0.15,
                    "dark shell appeared without content: strip_mix={}, classic_alpha={}",
                    p.strip_mix,
                    p.classic_alpha
                );
            }
        }
    }

    #[test]
    fn strip_to_card_single_continuous_morph() {
        let card_rect = Rect::new(84., 24., 360., 184.);
        let mut m = IslandMorph::new();
        m.transition_with_card(
            strip(),
            2.5,
            1.,
            0.,
            0.,
            IslandMode::Card,
            card_rect,
            28.,
            0.,
            true,
            480.,
        );
        assert!(m.active());
        assert_eq!(m.mode(), IslandMode::Card);
        let mut prev_h = strip().h;
        let mut saw_card_mid = false;
        for i in 1..=90 {
            let t = i as f64 / 120.;
            let p = m.presentation(t, host());
            if p.card_mix > 0.2 && p.card_mix < 0.9 {
                saw_card_mid = true;
                assert!(p.card_alpha > 0.05 && p.rect.w > strip().w);
            }
            if t < 0.15 {
                assert!(p.rect.h >= prev_h - 0.1);
                prev_h = p.rect.h;
            }
        }
        assert!(saw_card_mid);
    }

    #[test]
    fn card_to_strip_morph() {
        let card_rect = Rect::new(84., 24., 360., 184.);
        let mut m = IslandMorph::new();
        m.transition_with_card(
            card_rect,
            28.,
            0.,
            0.,
            1.,
            IslandMode::Strip,
            strip(),
            2.5,
            0.,
            true,
            440.,
        );
        let p0 = m.presentation(0., host());
        assert!((p0.rect.w - card_rect.w).abs() < 1e-6);
        assert!((p0.card_mix - 1.0).abs() < 1e-6);
        let mut t = 0.;
        while m.tick(t) && t < 1.5 {
            t += 1. / 120.;
        }
        assert!(!m.active());
    }
}
