//! `.dotlayout` 一键桌面预设导入导出引擎：跨分辨率归一化比例坐标、压缩封装与安全校验。
use crate::{snap::Rect2D, stack::SmartStack};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use dotisle_domain::desktop_widgets::{valid_size, Kind, Placement, Size, Style};
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};

const MAGIC_PREFIX: &str = "DOTLAYOUT1:";

/// 单条归一化桌搭预设条目（保存相对显示器宽高的百分比，跨屏幕自动适配）。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizedItem {
    pub id: String,
    pub kind: Kind,
    pub size: Size,
    pub style: Style,
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub opacity: f32,
    #[serde(default)]
    pub stack_id: String,
    #[serde(default)]
    pub stack_order: u32,
    pub x_ratio: f64,
    pub y_ratio: f64,
    #[serde(default)]
    pub width: f64,
    #[serde(default)]
    pub height: f64,
    #[serde(default)]
    pub config: dotisle_domain::widget_config::Config,
}

/// 可用于社区分享或本地备份的整套桌面预设包。
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LayoutPack {
    pub version: String,
    pub name: String,
    pub author: String,
    pub reference_width: f64,
    pub reference_height: f64,
    pub items: Vec<NormalizedItem>,
    #[serde(default)]
    pub stacks: Vec<SmartStack>,
}

/// 从当前桌面布局与基准屏幕大小构建 `LayoutPack`。
pub fn from_placements(
    name: &str,
    author: &str,
    screen: Rect2D,
    placements: &[Placement],
    stacks: &[SmartStack],
) -> LayoutPack {
    let sw = screen.w.max(1.0);
    let sh = screen.h.max(1.0);
    let items = placements
        .iter()
        .map(|p| NormalizedItem {
            id: p.id.clone(),
            kind: p.kind,
            size: p.size,
            style: p.style,
            material: p.material.clone(),
            opacity: p.opacity,
            stack_id: p.stack_id.clone(),
            stack_order: p.stack_order,
            x_ratio: ((p.x as f64 - screen.x) / sw).clamp(0.0, 0.95),
            y_ratio: ((p.y as f64 - screen.y) / sh).clamp(0.0, 0.95),
            width: p.width,
            height: p.height,
            config: p.config.clone(),
        })
        .collect();

    LayoutPack {
        version: "allegro-layout/v1".into(),
        name: name.chars().take(60).collect(),
        author: author.chars().take(40).collect(),
        reference_width: sw,
        reference_height: sh,
        items,
        stacks: stacks.to_vec(),
    }
}

/// 将 `LayoutPack` 序列化、zlib 压缩并编码为分享字符串（带 `DOTLAYOUT1:` 头部）。
pub fn export_dotlayout(pack: &LayoutPack) -> Result<String, String> {
    if pack.items.len() > 16 {
        return Err("桌面预设最多包含 16 个组件".into());
    }
    let json_bytes = serde_json::to_vec(pack).map_err(|e| e.to_string())?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&json_bytes).map_err(|e| e.to_string())?;
    let compressed = encoder.finish().map_err(|e| e.to_string())?;
    Ok(format!("{MAGIC_PREFIX}{}", STANDARD.encode(compressed)))
}

/// 解析分享文本或原始 JSON 为适配目标显示器的 `Placement` 与 `SmartStack` 列表。
pub fn import_dotlayout(
    raw: &str,
    target_screen: Rect2D,
) -> Result<(LayoutPack, Vec<Placement>, Vec<SmartStack>), String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.len() > 256 * 1024 {
        return Err("布局预设内容为空或体积超出限制".into());
    }
    let pack: LayoutPack = if let Some(encoded) = trimmed.strip_prefix(MAGIC_PREFIX) {
        let compressed = STANDARD
            .decode(encoded.trim())
            .map_err(|_| "预设编码格式无效".to_string())?;
        let mut decoder = ZlibDecoder::new(compressed.as_slice());
        let mut buf = Vec::new();
        decoder
            .by_ref()
            .take(512 * 1024)
            .read_to_end(&mut buf)
            .map_err(|_| "预设解压失败".to_string())?;
        serde_json::from_slice(&buf).map_err(|_| "预设 JSON 结构无效".to_string())?
    } else {
        serde_json::from_str(trimmed).map_err(|_| "预设 JSON 结构无效".to_string())?
    };

    if pack.version != "allegro-layout/v1" {
        return Err("不支持的布局预设版本".into());
    }
    if pack.items.len() > 16 {
        return Err("预设包含的组件数量超过上限 16 个".into());
    }

    let sw = target_screen.w.max(320.0);
    let sh = target_screen.h.max(240.0);
    let mut placements = Vec::with_capacity(pack.items.len());

    for (idx, item) in pack.items.iter().enumerate() {
        item.config.validate(item.kind)?;
        if !valid_size(item.width, item.height) {
            return Err("预设包含无效组件尺寸".into());
        }
        let op = if item.opacity == 0.0 {
            0.0
        } else {
            item.opacity.clamp(0.15, 1.0)
        };
        let dims = if item.width > 0.0 {
            (item.width, item.height)
        } else {
            item.size.dimensions()
        };
        let x = (target_screen.x + sw * item.x_ratio.clamp(0.0, 0.95))
            .round()
            .clamp(target_screen.x, (target_screen.right() - dims.0).max(target_screen.x)) as i32;
        let y = (target_screen.y + sh * item.y_ratio.clamp(0.0, 0.95))
            .round()
            .clamp(target_screen.y, (target_screen.bottom() - dims.1).max(target_screen.y)) as i32;
        let clean_id = if !item.id.is_empty()
            && item.id.len() <= 96
            && item.id.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'-')
        {
            item.id.clone()
        } else {
            format!("imported-{}", idx + 1)
        };

        placements.push(Placement {
            id: clean_id,
            kind: item.kind,
            size: item.size,
            style: item.style,
            config: item.config.clone(),
            x,
            y,
            width: item.width,
            height: item.height,
            hidden: false,
            revision: 1,
            material: item.material.clone(),
            opacity: op,
            stack_id: item.stack_id.clone(),
            stack_order: item.stack_order,
        });
    }

    Ok((pack.clone(), placements, pack.stacks))
}
