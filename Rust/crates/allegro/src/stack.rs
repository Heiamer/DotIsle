//! 智能叠放 (Smart Stack) 引擎：支持同尺寸组件堆叠合并、滚轮翻页、情境自动切换、物理叠层厚度感与胶囊页码指示器。
use crate::{
    snap::Rect2D,
    theme::{rgba, squircle_path},
};
use dotisle_domain::desktop_widgets::{Kind, Placement};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use vello::{
    kurbo::{Affine, Circle, Rect, RoundedRect, Stroke},
    peniko::Fill,
    Scene,
};

/// 单个智能叠放组。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartStack {
    pub id: String,
    pub members: Vec<String>,
    pub active_index: usize,
    #[serde(default)]
    pub last_manual_sec: f64,
}

/// 驱动情境自动翻页的实时桌面上下文信号。
#[derive(Clone, Copy, Debug, Default)]
pub struct ContextSignals {
    pub music_playing: bool,
    pub focus_running: bool,
    pub upcoming_schedule_mins: Option<i64>,
    pub dropzone_count: usize,
}

/// 从 `Placement` 与辅助信号计算叠放可见性筛选与激活成员页数。
#[derive(Clone, Debug, Default)]
pub struct StackManager {
    pub stacks: HashMap<String, SmartStack>,
    serial: u64,
}

impl StackManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// 根据持久化的 `Placement.stack_id` 与 `stack_order` 重建内存叠放索引（幂等）。
    pub fn sync_from_placements(&mut self, items: &[Placement]) {
        let mut grouped: HashMap<String, Vec<(u32, String)>> = HashMap::new();
        for p in items {
            if !p.stack_id.is_empty() {
                grouped
                    .entry(p.stack_id.clone())
                    .or_default()
                    .push((p.stack_order, p.id.clone()));
            }
        }
        // 清理已不足 2 项或已删光的叠放组
        self.stacks.retain(|id, _| grouped.get(id).is_some_and(|m| m.len() >= 2));

        for (id, mut pairs) in grouped {
            if pairs.len() < 2 {
                continue;
            }
            pairs.sort_by_key(|(ord, _)| *ord);
            let members: Vec<String> = pairs.into_iter().map(|(_, mid)| mid).collect();
            let entry = self.stacks.entry(id.clone()).or_insert_with(|| SmartStack {
                id,
                members: members.clone(),
                active_index: 0,
                last_manual_sec: -60.0,
            });
            entry.members = members;
            if entry.active_index >= entry.members.len() {
                entry.active_index = 0;
            }
        }
    }

    /// 生成一个新的叠放 ID，并返回需要写回各成员的 `(widget_id, stack_id, stack_order)` 列表。
    pub fn merge_pair(
        &mut self,
        base_id: &str,
        dragged_id: &str,
        existing_base_stack: &str,
    ) -> (String, Vec<(String, String, u32)>) {
        let stack_id = if !existing_base_stack.is_empty() && self.stacks.contains_key(existing_base_stack) {
            existing_base_stack.to_string()
        } else {
            self.serial = self.serial.wrapping_add(1);
            format!("stack-{}", self.serial)
        };

        let stack = self.stacks.entry(stack_id.clone()).or_insert_with(|| SmartStack {
            id: stack_id.clone(),
            members: vec![base_id.to_string()],
            active_index: 0,
            last_manual_sec: -60.0,
        });

        if !stack.members.iter().any(|m| m == dragged_id) {
            stack.members.push(dragged_id.to_string());
        }
        stack.active_index = stack.members.len().saturating_sub(1);

        let updates = stack
            .members
            .iter()
            .enumerate()
            .map(|(i, mid)| (mid.clone(), stack_id.clone(), i as u32))
            .collect();
        (stack_id, updates)
    }

    /// 从叠放组中拆出指定组件（若剩余成员只剩 1 个，整组解散）。
    pub fn unpack_member(&mut self, widget_id: &str) -> Vec<(String, String, u32)> {
        let mut updates = vec![(widget_id.to_string(), String::new(), 0)];
        let target_stack = self
            .stacks
            .iter()
            .find(|(_, st)| st.members.iter().any(|m| m == widget_id))
            .map(|(sid, _)| sid.clone());
        let Some(sid) = target_stack else {
            return updates;
        };
        if let Some(st) = self.stacks.get_mut(&sid) {
            st.members.retain(|m| m != widget_id);
            if st.members.len() < 2 {
                for left in st.members.drain(..) {
                    updates.push((left, String::new(), 0));
                }
                self.stacks.remove(&sid);
            } else {
                if st.active_index >= st.members.len() {
                    st.active_index = 0;
                }
                for (i, mid) in st.members.iter().enumerate() {
                    updates.push((mid.clone(), sid.clone(), i as u32));
                }
            }
        }
        updates
    }

    /// 手动翻页（delta > 0 下一页，delta < 0 上一页），记录手动时间进入 15 秒自动防抖冷却期。
    pub fn step_by_member(&mut self, widget_id: &str, delta: i32, now_sec: f64) -> bool {
        let Some(st) = self
            .stacks
            .values_mut()
            .find(|st| st.members.iter().any(|m| m == widget_id))
        else {
            return false;
        };
        let n = st.members.len();
        if n < 2 {
            return false;
        }
        st.active_index = (st.active_index as i32 + delta).rem_euclid(n as i32) as usize;
        st.last_manual_sec = now_sec;
        true
    }

    /// 情境自动推荐切换：尊重 15 秒手动冷却窗口，不抢占用户刚手动翻过的叠放页。
    pub fn auto_rotate(&mut self, now_sec: f64, signals: ContextSignals, items: &[Placement]) -> bool {
        let kind_of: HashMap<&str, Kind> = items.iter().map(|p| (p.id.as_str(), p.kind)).collect();
        let mut changed = false;

        for st in self.stacks.values_mut() {
            if st.members.len() < 2 || now_sec - st.last_manual_sec < 15.0 {
                continue;
            }
            let best_idx = st
                .members
                .iter()
                .enumerate()
                .max_by_key(|(_, id)| {
                    let Some(&kind) = kind_of.get(id.as_str()) else {
                        return 0;
                    };
                    context_priority(kind, signals)
                })
                .map(|(i, _)| i)
                .unwrap_or(st.active_index);

            let best_score = st
                .members
                .get(best_idx)
                .and_then(|id| kind_of.get(id.as_str()))
                .map(|&k| context_priority(k, signals))
                .unwrap_or(0);

            if best_score > 0 && best_idx != st.active_index {
                st.active_index = best_idx;
                changed = true;
            }
        }
        changed
    }

    /// 获取某组件当前在叠放中的状态；返回 `None` 代表未叠放，`Some((active_idx, total, is_visible_page))`。
    pub fn status_of(&self, widget_id: &str) -> Option<(usize, usize, bool)> {
        let st = self
            .stacks
            .values()
            .find(|st| st.members.iter().any(|m| m == widget_id))?;
        let active_id = st.members.get(st.active_index)?;
        Some((st.active_index, st.members.len(), active_id == widget_id))
    }

    /// 从一组 `Placement` 过滤出当前各叠放组中唯一应该被绘制的激活成员，返回 `(index, Option<(active, total)>)`。
    pub fn visible_indices(&self, items: &[Placement]) -> Vec<(usize, Option<(usize, usize)>)> {
        let mut hidden_members = HashSet::new();
        let mut info_map = HashMap::new();
        for st in self.stacks.values() {
            if st.members.len() < 2 {
                continue;
            }
            for (i, mid) in st.members.iter().enumerate() {
                if i == st.active_index {
                    info_map.insert(mid.as_str(), (st.active_index, st.members.len()));
                } else {
                    hidden_members.insert(mid.as_str());
                }
            }
        }
        items
            .iter()
            .enumerate()
            .filter(|(_, p)| !hidden_members.contains(p.id.as_str()))
            .map(|(i, p)| (i, info_map.get(p.id.as_str()).copied()))
            .collect()
    }
}

fn context_priority(kind: Kind, signals: ContextSignals) -> i32 {
    match kind {
        Kind::Focus if signals.focus_running => 100,
        Kind::Schedule if signals.upcoming_schedule_mins.is_some_and(|m| (0..=60).contains(&m)) => 90,
        Kind::Dropzone if signals.dropzone_count > 0 => 80,
        Kind::Music if signals.music_playing => 70,
        _ => 0,
    }
}

/// 在主卡片底层绘制 Smart Stack Concept 10 风格的物理卡片叠层厚度感（绘制在主卡之前）。
pub fn paint_stack_depth_layers(
    scene: &mut Scene,
    card_rect: Rect2D,
    total: usize,
    dark: bool,
) {
    if total < 2 {
        return;
    }
    let layers = (total - 1).min(2);
    // 先绘最底层、再绘次底层，形成从前向后逐步收进且略微暗化的玻璃叠层轮廓
    for depth in (1..=layers).rev() {
        let inset = depth as f64 * 10.0;
        let drop = depth as f64 * 4.5;
        let x0 = card_rect.x + inset;
        let x1 = (card_rect.right() - inset).max(x0 + 24.0);
        let y0 = card_rect.y + drop;
        let y1 = (card_rect.bottom() + drop).min(card_rect.bottom() + 9.0);
        let r = Rect::new(x0, y0, x1, y1);
        let rad = (24.0 - depth as f64 * 3.0).max(12.0);
        let shape = squircle_path(r, rad);
        let fill_a = if dark {
            0.34 - depth as f32 * 0.08
        } else {
            0.38 - depth as f32 * 0.09
        };
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            rgba(if dark { 0x161B26 } else { 0xE8EEF8 }, fill_a),
            None,
            &shape,
        );
        scene.stroke(
            &Stroke::new(0.85),
            Affine::IDENTITY,
            rgba(0xFFFFFF, if dark { 0.16 } else { 0.56 }),
            None,
            &shape,
        );
    }
}

/// 在叠放卡片右侧边缘绘制 Smart Stack 半透明胶囊分页指示器。
pub fn paint_stack_dots(
    scene: &mut Scene,
    card_rect: Rect2D,
    active_index: usize,
    total: usize,
    accent: u32,
) {
    if total < 2 {
        return;
    }
    let gap = 7.5;
    let total_h = (total - 1) as f64 * gap;
    let x = card_rect.right() - 8.5;
    let cy = card_rect.cy();
    let y0 = cy - total_h / 2.0;

    let cap = Rect::new(x - 4.5, y0 - 5.5, x + 4.5, y0 + total_h + 5.5);
    scene.fill(
        Fill::NonZero,
        Affine::IDENTITY,
        rgba(0xFFFFFF, 0.24),
        None,
        &RoundedRect::from_rect(cap, 4.5),
    );
    scene.stroke(
        &Stroke::new(0.65),
        Affine::IDENTITY,
        rgba(0xFFFFFF, 0.42),
        None,
        &RoundedRect::from_rect(cap, 4.5),
    );

    for i in 0..total {
        let is_cur = i == active_index;
        let r = if is_cur { 2.6 } else { 1.8 };
        let col = if is_cur {
            rgba(accent, 0.96)
        } else {
            rgba(0x64748B, 0.52)
        };
        scene.fill(
            Fill::NonZero,
            Affine::IDENTITY,
            col,
            None,
            &Circle::new((x, y0 + i as f64 * gap), r),
        );
    }
}
