//! Allegro 现代桌搭光学材质引擎：耀光（Lumen）· 亚克力（Acrylic）。
use dotisle_domain::desktop_widgets::Style;
use serde::{Deserialize, Serialize};
use vello::{
    kurbo::{Affine, BezPath, Rect, Stroke},
    peniko::{BlendMode, Color, Fill, Gradient, ImageBrush, ImageData},
    Scene,
};

/// 桌面与侧边栏小组件可选的两种透明亚克力光学挡位。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WidgetMaterial {
    /// 柔雾：略高雾度的透明亚克力基底，适合长文本、清单与高可读性卡片。
    Lumen,
    /// 清透：高透亮度的透明亚克力玻璃基底（默认）。
    #[default]
    Acrylic,
}

impl WidgetMaterial {
    pub const ALL: [Self; 2] = [Self::Lumen, Self::Acrylic];

    /// 从配置字符串解析材质（所有历史外观统一映射为透明亚克力双挡位）。
    pub fn from_str_opt(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "lumen" | "lumenglass" | "lightfield" | "solid" | "soft" | "mist" | "耀光" | "柔雾" => {
                Self::Lumen
            }
            "acrylic" | "frosted" | "亚克力" | "清透" | "liquidglass" | "liquid" | "glass"
            | "transparent" | "clear" | "polaroid" | "液态玻璃" | "透明" | "" => Self::Acrylic,
            _ => Self::Acrylic,
        }
    }

    /// 序列化使用的规范键名。
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Lumen => "lumen",
            Self::Acrylic => "acrylic",
        }
    }

    /// 界面显示中文名称。
    pub fn label(self) -> &'static str {
        match self {
            Self::Lumen => "柔雾",
            Self::Acrylic => "清透",
        }
    }

    /// 切换至下一种材质（用于右键快捷菜单轮切）。
    pub fn next(self) -> Self {
        let i = Self::ALL.iter().position(|&m| m == self).unwrap_or(0);
        Self::ALL[(i + 1) % Self::ALL.len()]
    }
}

/// 单个小组件的完整外观参数。
#[derive(Clone, Copy, Debug)]
pub struct WidgetAppearance {
    pub material: WidgetMaterial,
    pub style: Style,
    pub opacity: f32,
    pub radius: f64,
    pub accent: Option<u32>,
    pub blur_wallpaper: bool,
}

impl Default for WidgetAppearance {
    fn default() -> Self {
        Self {
            material: WidgetMaterial::Acrylic,
            style: Style::Original,
            opacity: 1.0,
            radius: 24.0,
            accent: None,
            blur_wallpaper: true,
        }
    }
}

/// 现代苹果/光场视觉令牌（同时驱动底壳绘制与正文/控件对比色）。
#[derive(Clone, Copy, Debug)]
pub struct ThemeTokens {
    pub surface: u32,
    pub container: u32,
    pub high: u32,
    pub on_surface: u32,
    pub on_surface_variant: u32,
    pub primary: u32,
    pub on_primary: u32,
    pub border: u32,
    pub radius: f64,
}

/// 解析 `#RRGGBB` 或 `RRGGBB` 十六进制主题色。
pub fn parse_hex_rgb(raw: &str) -> Option<u32> {
    let hex = raw.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// 在两个 RGB 颜色之间按比例混合。
pub fn mix_rgb(a: u32, b: u32, t: f64) -> u32 {
    let t = t.clamp(0., 1.);
    let ch = |shift: u32| -> u32 {
        let x = ((a >> shift) & 0xff) as f64;
        let y = ((b >> shift) & 0xff) as f64;
        (x + (y - x) * t).round() as u32
    };
    (ch(16) << 16) | (ch(8) << 8) | ch(0)
}

/// 将 RGB 与浮点透明度转为 Vello Color。
pub fn rgba(rgb: u32, a: f32) -> Color {
    Color::from_rgba8(
        (rgb >> 16) as u8,
        (rgb >> 8) as u8,
        rgb as u8,
        (a.clamp(0., 1.) * 255.).round() as u8,
    )
}

/// 生成现代精工视觉令牌（所有组件统一使用现代光场配色，兼容旧 `Style` 参数不再呈现弃用皮肤）。
pub fn resolve_theme(_style: Style, dark: bool, custom_accent: Option<u32>) -> ThemeTokens {
    let mut t = if dark {
        ThemeTokens {
            surface: 0x181B24,
            container: 0x222634,
            high: 0x2D3244,
            on_surface: 0xF5F7FA,
            on_surface_variant: 0xA8B0C2,
            primary: 0x389BFF,
            on_primary: 0xFFFFFF,
            border: 0xFFFFFF,
            radius: 24.,
        }
    } else {
        ThemeTokens {
            surface: 0xF5F6FA,
            container: 0xFFFFFF,
            high: 0xEDF0F7,
            on_surface: 0x181B22,
            on_surface_variant: 0x5E6575,
            primary: 0x007AFF,
            on_primary: 0xFFFFFF,
            border: 0x0F172A,
            radius: 24.,
        }
    };
    if let Some(accent) = custom_accent {
        t.primary = accent & 0x00FF_FFFF;
    }
    t
}

/// 生成 G2 连续曲率超椭圆圆角路径（Apple 级顺滑边角）。
pub fn squircle_path(r: Rect, radius: f64) -> BezPath {
    let rad = if radius.is_finite() {
        radius
            .max(0.)
            .min(r.width().max(0.) / 2.)
            .min(r.height().max(0.) / 2.)
    } else {
        0.
    };
    let mut p = BezPath::new();
    if rad <= 1.5 {
        p.move_to((r.x0, r.y0));
        p.line_to((r.x1, r.y0));
        p.line_to((r.x1, r.y1));
        p.line_to((r.x0, r.y1));
        p.close_path();
        return p;
    }
    let corner = |p: &mut BezPath, cx: f64, cy: f64, xx: f64, xy: f64, yx: f64, yy: f64| {
        let pt = |x: f64, y: f64| (cx + (xx * x + yx * y) * rad, cy + (xy * x + yy * y) * rad);
        p.curve_to(pt(0.62, 0.), pt(0.40, 0.), pt(0.20, 0.20));
        p.curve_to(pt(0., 0.40), pt(0., 0.62), pt(0., 1.));
    };
    p.move_to((r.x1 - rad, r.y0));
    corner(&mut p, r.x1, r.y0, -1., 0., 0., 1.);
    p.line_to((r.x1, r.y1 - rad));
    corner(&mut p, r.x1, r.y1, 0., -1., -1., 0.);
    p.line_to((r.x0 + rad, r.y1));
    corner(&mut p, r.x0, r.y1, 1., 0., 0., -1.);
    p.line_to((r.x0, r.y0 + rad));
    corner(&mut p, r.x0, r.y0, 0., 1., 1., 0.);
    p.close_path();
    p
}

/// 在精密卡片内部绘制 Smart Stack Concept 10 风格的低对比度微点阵肌理。
pub fn paint_micro_dot_grid(scene: &mut Scene, r: Rect, radius: f64, dark: bool) {
    use vello::kurbo::Circle;
    let shape = squircle_path(r, radius);
    scene.push_clip_layer(Affine::IDENTITY, &shape);
    let step = 11.0;
    let col = rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.048 } else { 0.042 });
    let mut y = r.y0 + 10.0;
    while y < r.y1 - 8.0 {
        let mut x = r.x0 + 10.0;
        while x < r.x1 - 8.0 {
            scene.fill(Fill::NonZero, Affine::IDENTITY, col, None, &Circle::new((x, y), 0.75));
            x += step;
        }
        y += step;
    }
    scene.pop_layer();
}

/// 将**清透 / 柔雾透明亚克力**光学底壳绘制到 Vello 场景（融合 Deskset 轻柔空气感与 Smart Stack 360° 镜面亮边）。
pub fn paint_shell(
    scene: &mut Scene,
    r: Rect,
    appearance: WidgetAppearance,
    tokens: ThemeTokens,
    accent: u32,
    dark: bool,
    wallpaper_crop: Option<&ImageData>,
) {
    let rad = appearance.radius.min(tokens.radius);
    let shape = squircle_path(r, rad);
    let op = appearance.opacity.clamp(0.15, 1.0);

    // 1. Deskset 柔和环境漫射阴影（轻盈贴浮，绝无沉重死黑边框）
    let is_sheer = wallpaper_crop.is_none() && !appearance.blur_wallpaper;
    let (far_a, mid_a, near_a) = if is_sheer {
        (0.036 * op, 0.028 * op, 0.020 * op)
    } else {
        (0.055 * op, 0.042 * op, 0.028 * op)
    };
    scene.fill(
        Fill::NonZero,
        Affine::translate((0., 10.0)),
        rgba(0x090D16, far_a),
        None,
        &squircle_path(r, rad),
    );
    scene.fill(
        Fill::NonZero,
        Affine::translate((0., 4.0)),
        rgba(0x090D16, mid_a),
        None,
        &squircle_path(r, rad),
    );
    scene.fill(
        Fill::NonZero,
        Affine::translate((0., 1.2)),
        rgba(0x090D16, near_a),
        None,
        &squircle_path(r, rad),
    );

    // 2. 裁切容器内绘制统一透明亚克力材质层
    scene.push_layer(BlendMode::default(), op, Affine::IDENTITY, &shape);

    if let Some(bg) = wallpaper_crop {
        let sx = r.width() / (bg.width as f64).max(1.);
        let sy = r.height() / (bg.height as f64).max(1.);
        let tr = Affine::translate((r.x0, r.y0)) * Affine::scale_non_uniform(sx, sy);
        scene.draw_image(&ImageBrush::from(bg.clone()), tr);
    }

    let is_soft = appearance.material == WidgetMaterial::Lumen;
    if is_sheer {
        // 侧边栏内透光容器：透出后方透明亚克力壁纸磨砂层
        let top_a = if dark {
            if is_soft { 0.56 } else { 0.42 }
        } else {
            if is_soft { 0.62 } else { 0.46 }
        };
        let bot_a = if dark {
            if is_soft { 0.46 } else { 0.30 }
        } else {
            if is_soft { 0.48 } else { 0.30 }
        };
        let base_rgb = if dark {
            mix_rgb(0x1D2230, accent, 0.04)
        } else {
            mix_rgb(0xFFFFFF, accent, 0.015)
        };
        let grad = Gradient::new_linear((r.x0, r.y0), (r.x1, r.y1)).with_stops([
            (0.0, rgba(base_rgb, top_a)),
            (1.0, rgba(base_rgb, bot_a)),
        ]);
        scene.fill(Fill::NonZero, Affine::IDENTITY, &grad, None, &shape);
    } else {
        // 桌面端透明亚克力：真实模糊壁纸之上叠加轻盈润色层
        let tint = if dark {
            mix_rgb(0x161A24, accent, 0.04)
        } else {
            mix_rgb(0xF8FAFE, accent, 0.018)
        };
        let tint_a = match (wallpaper_crop.is_some(), is_soft) {
            (true, false) => if dark { 0.44 } else { 0.48 },
            (true, true) => if dark { 0.62 } else { 0.64 },
            (false, false) => if dark { 0.64 } else { 0.68 },
            (false, true) => if dark { 0.78 } else { 0.80 },
        };
        scene.fill(Fill::NonZero, Affine::IDENTITY, rgba(tint, tint_a), None, &shape);
    }

    // 3. 左上角轻微光学散射晕 + 顶部弧光折射带（无浓重色块）
    let glow = Gradient::new_radial(
        (r.x0 + r.width() * 0.20, r.y0 + r.height() * 0.10),
        r.width().max(r.height()) as f32 * 0.78,
    )
    .with_stops([
        (0.0, rgba(0xFFFFFF, if dark { 0.075 } else { 0.24 })),
        (0.55, rgba(accent, if dark { 0.025 } else { 0.015 })),
        (1.0, rgba(0xFFFFFF, 0.0)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &glow, None, &shape);

    let top_sheen = Gradient::new_linear((r.x0, r.y0), (r.x0, r.y0 + 42.0)).with_stops([
        (0.0, rgba(0xFFFFFF, if dark { 0.12 } else { 0.34 })),
        (1.0, rgba(0xFFFFFF, 0.0)),
    ]);
    scene.fill(Fill::NonZero, Affine::IDENTITY, &top_sheen, None, &shape);

    // 4. Smart Stack 360° 光学镜面高光双描边（外分离线 + 内侧斜向渐变亮边）
    scene.stroke(
        &Stroke::new(0.75),
        Affine::IDENTITY,
        rgba(if dark { 0xFFFFFF } else { 0x0F172A }, if dark { 0.14 } else { 0.07 }),
        None,
        &shape,
    );
    let inner = Rect::new(
        r.x0 + 1.0,
        r.y0 + 1.0,
        (r.x1 - 1.0).max(r.x0 + 2.0),
        (r.y1 - 1.0).max(r.y0 + 2.0),
    );
    let specular = Gradient::new_linear((inner.x0, inner.y0), (inner.x1, inner.y1)).with_stops([
        (0.0, rgba(0xFFFFFF, if dark { 0.38 } else { 0.85 })),
        (0.42, rgba(0xFFFFFF, if dark { 0.08 } else { 0.18 })),
        (1.0, rgba(0xFFFFFF, if dark { 0.22 } else { 0.50 })),
    ]);
    scene.stroke(
        &Stroke::new(1.0),
        Affine::IDENTITY,
        &specular,
        None,
        &squircle_path(inner, (rad - 1.0).max(2.0)),
    );

    scene.pop_layer();
}

/// 在卡片外部围绕边缘绘制激活态呼吸高光框。
pub fn paint_focus_ring(scene: &mut Scene, r: Rect, radius: f64, accent: u32) {
    scene.stroke(
        &Stroke::new(1.8),
        Affine::IDENTITY,
        rgba(accent, 0.78),
        None,
        &squircle_path(r, radius),
    );
}
