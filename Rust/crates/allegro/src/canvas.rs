//! 单显示器全屏合批透明画布规划器：按显示器分组、命中测试与精准点击穿透区域合并。
use crate::{snap::Rect2D, stack::StackManager};
use dotisle_domain::desktop_widgets::Placement;

/// 单台显示器的物理与逻辑几何描述。
#[derive(Clone, Debug, PartialEq)]
pub struct MonitorDesc {
    pub key: String,
    pub phys_x: i32,
    pub phys_y: i32,
    pub phys_w: u32,
    pub phys_h: u32,
    pub scale: f64,
}

impl MonitorDesc {
    pub fn logical_rect(&self) -> Rect2D {
        let s = self.scale.max(0.5);
        Rect2D::new(
            self.phys_x as f64 / s,
            self.phys_y as f64 / s,
            self.phys_w as f64 / s,
            self.phys_h as f64 / s,
        )
    }

    pub fn phys_bounds(&self) -> (i32, i32, i32, i32) {
        (
            self.phys_x,
            self.phys_y,
            self.phys_x + self.phys_w as i32,
            self.phys_y + self.phys_h as i32,
        )
    }
}

/// 合批画布上的单个小组件渲染与点击槽位（坐标为画布内逻辑坐标）。
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasSlot {
    pub placement_index: usize,
    pub id: String,
    pub rect: Rect2D,
    pub radius: f64,
    pub hidden: bool,
    pub stack_info: Option<(usize, usize)>,
}

/// 将所有 `Placement` 分配到各自最近的显示器画布，并过滤隐藏的智能叠放后台页。
pub fn plan_monitor_slots(
    monitor: &MonitorDesc,
    all_monitors: &[MonitorDesc],
    placements: &[Placement],
    stacks: &StackManager,
) -> Vec<CanvasSlot> {
    let mon_x = monitor.phys_x as f64 / monitor.scale.max(0.5);
    let mon_y = monitor.phys_y as f64 / monitor.scale.max(0.5);
    let mon_w = monitor.phys_w as f64 / monitor.scale.max(0.5);
    let visible = stacks.visible_indices(placements);
    let mut slots = Vec::new();

    for (idx, stack_info) in visible {
        let p = &placements[idx];
        // 归属距离该组件左上角最近的那台显示器（只在一台显示器绘制，避免跨屏重影）
        let owner_key = all_monitors
            .iter()
            .min_by_key(|m| {
                let lr = m.logical_rect();
                let px = p.x as f64 / m.scale.max(0.5);
                let py = p.y as f64 / m.scale.max(0.5);
                let cx = px.clamp(lr.x, lr.right());
                let cy = py.clamp(lr.y, lr.bottom());
                ((px - cx).powi(2) + (py - cy).powi(2)).round() as i64
            })
            .map(|m| m.key.as_str());

        if owner_key != Some(monitor.key.as_str()) {
            continue;
        }

        let dims = p.dimensions();
        let rect = if p.hidden {
            let bar_w = dims.0.clamp(160.0, 240.0);
            let local_x = (p.x - monitor.phys_x) as f64 / monitor.scale.max(0.5) + (dims.0 - bar_w) / 2.0;
            Rect2D::new(
                local_x.clamp(8.0, (mon_w - bar_w - 8.0).max(8.0)),
                0.0,
                bar_w,
                12.0,
            )
        } else {
            let local_x = (p.x as f64 - monitor.phys_x as f64) / monitor.scale.max(0.5);
            let local_y = (p.y as f64 - monitor.phys_y as f64) / monitor.scale.max(0.5);
            let _ = (mon_x, mon_y);
            Rect2D::new(local_x, local_y, dims.0, dims.1)
        };

        let radius = if p.hidden { 6.0 } else { 24.0 };
        slots.push(CanvasSlot {
            placement_index: idx,
            id: p.id.clone(),
            rect,
            radius,
            hidden: p.hidden,
            stack_info,
        });
    }

    slots
}

/// 按 Z-order（后来者居上）逆序做精准画布命中测试。
pub fn hit_test(slots: &[CanvasSlot], canvas_pt: (f64, f64)) -> Option<usize> {
    slots
        .iter()
        .rposition(|s| s.rect.contains(canvas_pt.0, canvas_pt.1))
}

/// 计算 Win32 `SetWindowRgn` 所需的物理像素圆角矩形列表 `(x0, y0, x1, y1, radius_px)`。
/// 当热键浮层仪表盘打开时直接覆盖全屏以支持空白处点击关闭与防穿透；
/// 普通置底桌面态下仅囊括小组件本体（以及活跃拖放/编辑边界），其余空白桌面 100% 穿透。
pub fn union_input_rects(
    slots: &[CanvasSlot],
    dashboard_open: bool,
    screen_phys_w: u32,
    screen_phys_h: u32,
    scale: f64,
) -> Vec<(i32, i32, i32, i32, i32)> {
    if dashboard_open {
        return vec![(0, 0, screen_phys_w as i32, screen_phys_h as i32, 0)];
    }
    let s = scale.max(0.5);
    slots
        .iter()
        .map(|slot| {
            let x0 = (slot.rect.x * s).floor() as i32;
            let y0 = (slot.rect.y * s).floor() as i32;
            let x1 = (slot.rect.right() * s).ceil() as i32;
            let y1 = (slot.rect.bottom() * s).ceil() as i32;
            let r = (slot.radius * s).round() as i32;
            (x0, y0, x1.max(x0 + 1), y1.max(y0 + 1), r)
        })
        .collect()
}
