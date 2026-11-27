//! 智能磁吸对齐与参考线引擎：拖拽和缩放时自动吸附其他组件边缘、中线、等距间距与屏幕边界。
use crate::theme::rgba;
use vello::{
    kurbo::{Affine, Line, Rect, RoundedRect, Stroke},
    Scene,
};

/// 二维逻辑矩形（DP 坐标）。
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect2D {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect2D {
    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }
    pub fn right(self) -> f64 {
        self.x + self.w
    }
    pub fn bottom(self) -> f64 {
        self.y + self.h
    }
    pub fn cx(self) -> f64 {
        self.x + self.w / 2.0
    }
    pub fn cy(self) -> f64 {
        self.y + self.h / 2.0
    }
    pub fn contains(self, px: f64, py: f64) -> bool {
        px >= self.x && px <= self.right() && py >= self.y && py <= self.bottom()
    }
    pub fn to_kurbo(self) -> Rect {
        Rect::new(self.x, self.y, self.right(), self.bottom())
    }
    /// 计算交集面积。
    pub fn intersection_area(self, other: Self) -> f64 {
        let ix = (self.right().min(other.right()) - self.x.max(other.x)).max(0.0);
        let iy = (self.bottom().min(other.bottom()) - self.y.max(other.y)).max(0.0);
        ix * iy
    }
}

/// 用于对齐参考与重叠判定的同屏邻居描述。
#[derive(Clone, Debug)]
pub struct SnapPeer {
    pub id: String,
    pub rect: Rect2D,
    pub hidden: bool,
}

/// 磁吸参数配置。
#[derive(Clone, Copy, Debug)]
pub struct SnapConfig {
    pub threshold: f64,
    pub screen_margin: f64,
    pub gap_step: f64,
    pub stack_overlap_ratio: f64,
}

impl Default for SnapConfig {
    fn default() -> Self {
        Self {
            threshold: 10.0,
            screen_margin: 16.0,
            gap_step: 16.0,
            stack_overlap_ratio: 0.55,
        }
    }
}

/// 单条可视化磁吸参考线。
#[derive(Clone, Debug, PartialEq)]
pub struct SnapGuide {
    pub vertical: bool,
    pub pos: f64,
    pub span_start: f64,
    pub span_end: f64,
}

/// 磁吸计算输出结果。
#[derive(Clone, Debug)]
pub struct SnapResult {
    pub rect: Rect2D,
    pub guides: Vec<SnapGuide>,
    /// 当拖拽到另一个相近尺寸组件上方且重叠面积超过阈值时返回目标 ID，提示可松手合并智能叠放。
    pub stack_target: Option<String>,
}

/// 计算拖拽移动时的双轴磁吸位置、参考线及智能叠放目标。
pub fn snap_drag(
    moving_id: &str,
    raw: Rect2D,
    work_area: Rect2D,
    peers: &[SnapPeer],
    cfg: SnapConfig,
) -> SnapResult {
    let mut best_dx: Option<(f64, SnapGuide)> = None;
    let mut best_dy: Option<(f64, SnapGuide)> = None;

    let try_x = |best: &mut Option<(f64, SnapGuide)>, target_x: f64, cur_x: f64, line_x: f64, y0: f64, y1: f64| {
        let delta = target_x - cur_x;
        if delta.abs() <= cfg.threshold {
            let take = best.as_ref().is_none_or(|(bd, _)| delta.abs() < bd.abs());
            if take {
                *best = Some((
                    delta,
                    SnapGuide {
                        vertical: true,
                        pos: line_x,
                        span_start: y0.min(y1),
                        span_end: y0.max(y1),
                    },
                ));
            }
        }
    };

    let try_y = |best: &mut Option<(f64, SnapGuide)>, target_y: f64, cur_y: f64, line_y: f64, x0: f64, x1: f64| {
        let delta = target_y - cur_y;
        if delta.abs() <= cfg.threshold {
            let take = best.as_ref().is_none_or(|(bd, _)| delta.abs() < bd.abs());
            if take {
                *best = Some((
                    delta,
                    SnapGuide {
                        vertical: false,
                        pos: line_y,
                        span_start: x0.min(x1),
                        span_end: x0.max(x1),
                    },
                ));
            }
        }
    };

    // 1. 吸附屏幕安全边距与屏幕正中心线
    try_x(
        &mut best_dx,
        work_area.x + cfg.screen_margin,
        raw.x,
        work_area.x + cfg.screen_margin,
        raw.y,
        raw.bottom(),
    );
    try_x(
        &mut best_dx,
        work_area.right() - cfg.screen_margin - raw.w,
        raw.x,
        work_area.right() - cfg.screen_margin,
        raw.y,
        raw.bottom(),
    );
    try_x(
        &mut best_dx,
        work_area.cx() - raw.w / 2.0,
        raw.x,
        work_area.cx(),
        work_area.y + 12.0,
        work_area.bottom() - 12.0,
    );

    try_y(
        &mut best_dy,
        work_area.y + cfg.screen_margin,
        raw.y,
        work_area.y + cfg.screen_margin,
        raw.x,
        raw.right(),
    );
    try_y(
        &mut best_dy,
        work_area.bottom() - cfg.screen_margin - raw.h,
        raw.y,
        work_area.bottom() - cfg.screen_margin,
        raw.x,
        raw.right(),
    );
    try_y(
        &mut best_dy,
        work_area.cy() - raw.h / 2.0,
        raw.y,
        work_area.cy(),
        work_area.x + 12.0,
        work_area.right() - 12.0,
    );

    // 2. 吸附其他活跃小组件的四边、中线以及 16dp 黄金间距
    for p in peers.iter().filter(|p| !p.hidden && p.id != moving_id) {
        let pr = p.rect;
        let span_y0 = raw.y.min(pr.y);
        let span_y1 = raw.bottom().max(pr.bottom());
        let span_x0 = raw.x.min(pr.x);
        let span_x1 = raw.right().max(pr.right());

        // X 轴边缘与中心对齐
        try_x(&mut best_dx, pr.x, raw.x, pr.x, span_y0, span_y1);
        try_x(&mut best_dx, pr.right() - raw.w, raw.x, pr.right(), span_y0, span_y1);
        try_x(&mut best_dx, pr.cx() - raw.w / 2.0, raw.x, pr.cx(), span_y0, span_y1);
        // 与相邻组件保持 16dp 间距
        try_x(&mut best_dx, pr.right() + cfg.gap_step, raw.x, pr.right() + cfg.gap_step * 0.5, span_y0, span_y1);
        try_x(&mut best_dx, pr.x - cfg.gap_step - raw.w, raw.x, pr.x - cfg.gap_step * 0.5, span_y0, span_y1);

        // Y 轴边缘与中心对齐
        try_y(&mut best_dy, pr.y, raw.y, pr.y, span_x0, span_x1);
        try_y(&mut best_dy, pr.bottom() - raw.h, raw.y, pr.bottom(), span_x0, span_x1);
        try_y(&mut best_dy, pr.cy() - raw.h / 2.0, raw.y, pr.cy(), span_x0, span_x1);
        try_y(&mut best_dy, pr.bottom() + cfg.gap_step, raw.y, pr.bottom() + cfg.gap_step * 0.5, span_x0, span_x1);
        try_y(&mut best_dy, pr.y - cfg.gap_step - raw.h, raw.y, pr.y - cfg.gap_step * 0.5, span_x0, span_x1);
    }

    let mut snapped = raw;
    let mut guides = Vec::new();
    if let Some((dx, g)) = best_dx {
        snapped.x += dx;
        guides.push(g);
    }
    if let Some((dy, g)) = best_dy {
        snapped.y += dy;
        guides.push(g);
    }

    // 限制在工作区合理可见范围内
    snapped.x = snapped.x.clamp(work_area.x, (work_area.right() - snapped.w).max(work_area.x));
    snapped.y = snapped.y.clamp(work_area.y, (work_area.bottom() - snapped.h).max(work_area.y));

    // 3. 检测是否满足智能叠放合并条件（重叠 >= threshold 且尺寸相近）
    let stack_target = detect_stack_target(moving_id, raw, peers, cfg.stack_overlap_ratio);
    if stack_target.is_some() {
        // 准备叠放时不再画干扰的网格对齐线
        guides.clear();
    }

    SnapResult {
        rect: snapped,
        guides,
        stack_target,
    }
}

/// 识别与被拖动卡片高度重叠且尺寸相近的目标组件。
pub fn detect_stack_target(
    moving_id: &str,
    raw: Rect2D,
    peers: &[SnapPeer],
    ratio: f64,
) -> Option<String> {
    let raw_area = (raw.w * raw.h).max(1.0);
    let mut best: Option<(f64, String)> = None;
    for p in peers.iter().filter(|p| !p.hidden && p.id != moving_id) {
        let size_compat = (p.rect.w - raw.w).abs() <= 48.0 && (p.rect.h - raw.h).abs() <= 48.0;
        if !size_compat {
            continue;
        }
        let inter = raw.intersection_area(p.rect);
        let peer_area = (p.rect.w * p.rect.h).max(1.0);
        let ov = inter / raw_area.min(peer_area);
        if ov >= ratio && best.as_ref().is_none_or(|(b, _)| ov > *b) {
            best = Some((ov, p.id.clone()));
        }
    }
    best.map(|(_, id)| id)
}

/// 计算缩放边缘时的磁吸尺寸。
pub fn snap_resize(
    moving_id: &str,
    left: bool,
    right: bool,
    top: bool,
    bottom: bool,
    raw: Rect2D,
    peers: &[SnapPeer],
    cfg: SnapConfig,
) -> (Rect2D, Vec<SnapGuide>) {
    let mut r = raw;
    let mut guides = Vec::new();

    for p in peers.iter().filter(|p| !p.hidden && p.id != moving_id) {
        let pr = p.rect;
        if right && (r.right() - pr.right()).abs() <= cfg.threshold {
            r.w = (pr.right() - r.x).max(160.0);
            guides.push(SnapGuide {
                vertical: true,
                pos: pr.right(),
                span_start: r.y.min(pr.y),
                span_end: r.bottom().max(pr.bottom()),
            });
        } else if right && (r.w - pr.w).abs() <= cfg.threshold {
            r.w = pr.w.max(160.0);
            guides.push(SnapGuide {
                vertical: true,
                pos: r.right(),
                span_start: r.y.min(pr.y),
                span_end: r.bottom().max(pr.bottom()),
            });
        } else if left && (r.x - pr.x).abs() <= cfg.threshold {
            let old_right = r.right();
            r.x = pr.x;
            r.w = (old_right - r.x).max(160.0);
            guides.push(SnapGuide {
                vertical: true,
                pos: pr.x,
                span_start: r.y.min(pr.y),
                span_end: r.bottom().max(pr.bottom()),
            });
        }

        if bottom && (r.bottom() - pr.bottom()).abs() <= cfg.threshold {
            r.h = (pr.bottom() - r.y).max(120.0);
            guides.push(SnapGuide {
                vertical: false,
                pos: pr.bottom(),
                span_start: r.x.min(pr.x),
                span_end: r.right().max(pr.right()),
            });
        } else if bottom && (r.h - pr.h).abs() <= cfg.threshold {
            r.h = pr.h.max(120.0);
            guides.push(SnapGuide {
                vertical: false,
                pos: r.bottom(),
                span_start: r.x.min(pr.x),
                span_end: r.right().max(pr.right()),
            });
        } else if top && (r.y - pr.y).abs() <= cfg.threshold {
            let old_bot = r.bottom();
            r.y = pr.y;
            r.h = (old_bot - r.y).max(120.0);
            guides.push(SnapGuide {
                vertical: false,
                pos: pr.y,
                span_start: r.x.min(pr.x),
                span_end: r.right().max(pr.right()),
            });
        }
    }

    (r, guides)
}

/// 在全屏合批画布顶层绘制磁吸辅助线与智能叠放合并高亮框。
pub fn paint_guides(scene: &mut Scene, guides: &[SnapGuide], stack_highlight: Option<Rect2D>) {
    let blue = rgba(0x389BFF, 0.85);
    let glow = rgba(0x60A5FA, 0.25);

    for g in guides {
        let (a, b) = if g.vertical {
            ((g.pos, g.span_start), (g.pos, g.span_end))
        } else {
            ((g.span_start, g.pos), (g.span_end, g.pos))
        };
        scene.stroke(&Stroke::new(3.5), Affine::IDENTITY, glow, None, &Line::new(a, b));
        scene.stroke(&Stroke::new(1.2), Affine::IDENTITY, blue, None, &Line::new(a, b));
    }

    if let Some(target) = stack_highlight {
        let expanded = Rect::new(target.x - 4., target.y - 4., target.right() + 4., target.bottom() + 4.);
        scene.stroke(
            &Stroke::new(2.5),
            Affine::IDENTITY,
            rgba(0x3B82F6, 0.92),
            None,
            &RoundedRect::from_rect(expanded, 26.0),
        );
    }
}

/// 全屏小组件编辑网格度量参数（单格 176×176 + 间距 16，与 Small/Medium/Large 天然契合）。
#[derive(Clone, Copy, Debug)]
pub struct GridSpec {
    pub origin_x: f64,
    pub origin_y: f64,
    pub cell: f64,
    pub gap: f64,
    pub cols: usize,
    pub rows: usize,
}

impl GridSpec {
    /// 单个网格步距（格宽 + 间距）。
    pub fn step(&self) -> f64 {
        self.cell + self.gap
    }

    /// 根据起始列行与跨越格数生成标准逻辑矩形（多格跨越自动补足中间间隙）。
    pub fn slot_rect(&self, col: usize, row: usize, span_x: usize, span_y: usize) -> Rect2D {
        let sx = span_x.max(1);
        let sy = span_y.max(1);
        Rect2D::new(
            self.origin_x + col as f64 * self.step(),
            self.origin_y + row as f64 * self.step(),
            sx as f64 * self.cell + (sx - 1) as f64 * self.gap,
            sy as f64 * self.cell + (sy - 1) as f64 * self.gap,
        )
    }
}

/// 按当前显示器逻辑分辨率计算铺满桌面的 Apple 标准区块网格（1×1=176×176、2×1=368×176、2×2=368×368）。
pub fn compute_grid(screen_w: f64, screen_h: f64) -> GridSpec {
    let cell = dotisle_domain::desktop_widgets::WIDGET_CELL;
    let gap = dotisle_domain::desktop_widgets::WIDGET_GAP;
    let step = cell + gap;
    let cols = (((screen_w - 48.0 + gap) / step).floor() as usize).max(2);
    let rows = (((screen_h - 116.0 + gap) / step).floor() as usize).max(2);
    let total_w = cols as f64 * cell + (cols - 1) as f64 * gap;
    let origin_x = ((screen_w - total_w) * 0.5).max(16.0);
    let origin_y = 64.0;
    GridSpec {
        origin_x,
        origin_y,
        cell,
        gap,
        cols,
        rows,
    }
}

/// 将任意矩形吸附至最近的桌面标准网格槽位并规整尺寸跨度。
pub fn snap_rect_to_grid(r: Rect2D, grid: &GridSpec) -> Rect2D {
    let span_x = ((r.w + grid.gap) / grid.step())
        .round()
        .clamp(1.0, grid.cols as f64) as usize;
    let span_y = ((r.h + grid.gap) / grid.step())
        .round()
        .clamp(1.0, grid.rows as f64) as usize;
    let max_col = grid.cols.saturating_sub(span_x);
    let max_row = grid.rows.saturating_sub(span_y);
    let col = ((r.x - grid.origin_x) / grid.step())
        .round()
        .clamp(0.0, max_col as f64) as usize;
    let row = ((r.y - grid.origin_y) / grid.step())
        .round()
        .clamp(0.0, max_row as f64) as usize;
    grid.slot_rect(col, row, span_x, span_y)
}

/// 一键自动齐整排布画布内所有组件到无重叠的最近网格槽位。
pub fn auto_pack_grid(items: &[(String, Rect2D)], grid: &GridSpec) -> Vec<(String, Rect2D)> {
    let mut occupied = vec![vec![false; grid.cols]; grid.rows];
    let can_fit = |occ: &[Vec<bool>], c: usize, r: usize, sx: usize, sy: usize| -> bool {
        if c + sx > grid.cols || r + sy > grid.rows {
            return false;
        }
        for rr in r..(r + sy) {
            for cc in c..(c + sx) {
                if occ[rr][cc] {
                    return false;
                }
            }
        }
        true
    };

    let mut out = Vec::with_capacity(items.len());
    for (id, r) in items {
        let sx = ((r.w + grid.gap) / grid.step())
            .round()
            .clamp(1.0, grid.cols as f64) as usize;
        let sy = ((r.h + grid.gap) / grid.step())
            .round()
            .clamp(1.0, grid.rows as f64) as usize;
        let pref_c = ((r.x - grid.origin_x) / grid.step())
            .round()
            .clamp(0.0, grid.cols.saturating_sub(sx) as f64) as usize;
        let pref_r = ((r.y - grid.origin_y) / grid.step())
            .round()
            .clamp(0.0, grid.rows.saturating_sub(sy) as f64) as usize;

        let mut chosen = (pref_c, pref_r);
        if !can_fit(&occupied, pref_c, pref_r, sx, sy) {
            let mut best_dist = f64::MAX;
            for rr in 0..=grid.rows.saturating_sub(sy) {
                for cc in 0..=grid.cols.saturating_sub(sx) {
                    if can_fit(&occupied, cc, rr, sx, sy) {
                        let d = ((cc as f64 - pref_c as f64).powi(2))
                            + ((rr as f64 - pref_r as f64).powi(2));
                        if d < best_dist {
                            best_dist = d;
                            chosen = (cc, rr);
                        }
                    }
                }
            }
        }
        for rr in chosen.1..(chosen.1 + sy).min(grid.rows) {
            for cc in chosen.0..(chosen.0 + sx).min(grid.cols) {
                occupied[rr][cc] = true;
            }
        }
        out.push((id.clone(), grid.slot_rect(chosen.0, chosen.1, sx, sy)));
    }
    out
}

/// 在桌面全屏画布铺满 Apple 超椭圆网格编辑槽位，并高亮正在落位的目标区域。
pub fn paint_edit_grid(
    scene: &mut Scene,
    grid: &GridSpec,
    highlight: Option<Rect2D>,
    dark: bool,
) {
    use crate::theme::squircle_path;
    use vello::peniko::Fill;

    let cell_fill = if dark { rgba(0xFFFFFF, 0.05) } else { rgba(0xFFFFFF, 0.24) };
    let cell_rim = if dark { rgba(0xFFFFFF, 0.16) } else { rgba(0x0F172A, 0.14) };
    let cross_col = if dark { rgba(0xFFFFFF, 0.22) } else { rgba(0x0F172A, 0.18) };

    for row in 0..grid.rows {
        for col in 0..grid.cols {
            let sr = grid.slot_rect(col, row, 1, 1);
            let rect_k = Rect::new(sr.x, sr.y, sr.right(), sr.bottom());
            let shape = squircle_path(rect_k, 24.0);
            scene.fill(Fill::NonZero, Affine::IDENTITY, cell_fill, None, &shape);
            scene.stroke(&Stroke::new(1.0), Affine::IDENTITY, cell_rim, None, &shape);
            // 槽位中心精致准星十字
            let cx = sr.cx();
            let cy = sr.cy();
            scene.stroke(&Stroke::new(1.1), Affine::IDENTITY, cross_col, None, &Line::new((cx - 5., cy), (cx + 5., cy)));
            scene.stroke(&Stroke::new(1.1), Affine::IDENTITY, cross_col, None, &Line::new((cx, cy - 5.), (cx, cy + 5.)));
        }
    }

    if let Some(hl) = highlight {
        let rect_k = Rect::new(hl.x, hl.y, hl.right(), hl.bottom());
        let shape = squircle_path(rect_k, 24.0);
        scene.fill(Fill::NonZero, Affine::IDENTITY, rgba(0x0A84FF, 0.20), None, &shape);
        scene.stroke(&Stroke::new(2.2), Affine::IDENTITY, rgba(0x0A84FF, 0.92), None, &shape);
    }
}
