//! WGSL 光学液态玻璃（Liquid Glass Beta）合成管线：实现曲面边缘折射、RGB 色散、柔焦与定向高光描边。
use vello::wgpu;

pub const MAX_GLASS_SHAPES: usize = 4;

/// 单个液态玻璃圆角区域的物理像素几何与光学参数。
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct GlassShape {
    /// `[x, y, width, height]`（物理像素坐标）。
    pub rect: [f32; 4],
    /// `[radius_px, opacity, refraction_01, dispersion_01]`。
    pub params0: [f32; 4],
    /// `[blur_01, tint_alpha, glass_mix, 0.0]`。
    pub params1: [f32; 4],
    /// `[seed_r, seed_g, seed_b, body_dye_weight]` in the backdrop texture color space.
    pub body_dye: [f32; 4],
}

impl GlassShape {
    /// 构造默认深色通透液态玻璃形状，自动夹紧各光学参数至安全区间。
    pub fn new(
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius_px: f32,
        opacity: f32,
        refraction_01: f32,
        dispersion_01: f32,
        blur_01: f32,
        tint_01: f32,
        glass_mix: f32,
    ) -> Self {
        let t = tint_01.clamp(0.0, 1.0);
        // Keep zero neutral; make the tint perceptible at ordinary slider settings.
        let tint_alpha = (t.powf(1.05) * 0.44).clamp(0.0, 0.44);
        Self {
            rect: [x, y, w.max(1.0), h.max(1.0)],
            params0: [
                radius_px.max(0.0),
                opacity.clamp(0.0, 1.0),
                refraction_01.clamp(0.0, 1.0),
                dispersion_01.clamp(0.0, 1.0),
            ],
            params1: [
                blur_01.clamp(0.0, 1.0),
                tint_alpha,
                glass_mix.clamp(0.0, 1.0),
                0.0,
            ],
            body_dye: [0.0; 4],
        }
    }

    /// Dye the transmitted light across the full pane, independently of the neutral tint.
    pub fn with_body_dye(mut self, seed: u32, depth: f32, enabled_mix: f32) -> Self {
        let t = depth.clamp(0.0, 1.0);
        // Responsive at the default 6%, smooth down to zero, and never fully opaque.
        let weight = 0.74 * t / (t + 0.065 * (1.0 - t)) * enabled_mix.clamp(0.0, 1.0);
        self.body_dye = [
            ((seed >> 16) & 255) as f32 / 255.0,
            ((seed >> 8) & 255) as f32 / 255.0,
            (seed & 255) as f32 / 255.0,
            weight,
        ];
        self
    }
}

/// 将屏幕参数与至多 4 个液态玻璃区域打包为 16 字节对齐的 WGSL Uniform 字节流。
pub fn encode_uniforms(width: u32, height: u32, shapes: &[GlassShape]) -> Vec<u8> {
    let count = shapes.len().min(MAX_GLASS_SHAPES) as u32;
    let mut out = Vec::with_capacity(16 + MAX_GLASS_SHAPES * 64);
    out.extend_from_slice(&(width.max(1) as f32).to_le_bytes());
    out.extend_from_slice(&(height.max(1) as f32).to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(&0.0f32.to_le_bytes());
    for i in 0..MAX_GLASS_SHAPES {
        let s = shapes.get(i).copied().unwrap_or_default();
        for arr in [s.rect, s.params0, s.params1, s.body_dye] {
            for v in arr {
                out.extend_from_slice(&v.to_le_bytes());
            }
        }
    }
    out
}

const LIQUID_GLASS_WGSL: &str = r#"
struct GlassShape {
    rect: vec4<f32>,
    params0: vec4<f32>,
    params1: vec4<f32>,
    body_dye: vec4<f32>,
};

struct Uniforms {
    viewport: vec2<f32>,
    shape_count: u32,
    _pad: f32,
    shapes: array<GlassShape, 4>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var backdrop_tex: texture_2d<f32>;
@group(0) @binding(2) var fg_tex: texture_2d<f32>;
@group(0) @binding(3) var tex_sampler: sampler;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VsOut {
    var verts = array<vec2<f32>, 3>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(3.0, -1.0),
        vec2<f32>(-1.0, 3.0),
    );
    let p = verts[idx];
    var out: VsOut;
    out.pos = vec4<f32>(p, 0.0, 1.0);
    out.uv = vec2<f32>(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5);
    return out;
}

fn sd_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> f32 {
    let rad = min(r, min(b.x, b.y));
    let q = abs(p) - b + vec2<f32>(rad, rad);
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0, 0.0))) - rad;
}

fn sdf_grad_rounded_box(p: vec2<f32>, b: vec2<f32>, r: f32) -> vec2<f32> {
    let rad = min(r, min(b.x, b.y));
    let q = abs(p) - b + vec2<f32>(rad, rad);
    let s = vec2<f32>(select(-1.0, 1.0, p.x >= 0.0), select(-1.0, 1.0, p.y >= 0.0));
    var g = vec2<f32>(0.0, 0.0);
    if q.x > 0.0 || q.y > 0.0 {
        let m = max(q, vec2<f32>(0.0, 0.0));
        let len = max(length(m), 1e-4);
        g = m / len;
    } else if q.x > q.y {
        g = vec2<f32>(1.0, 0.0);
    } else {
        g = vec2<f32>(0.0, 1.0);
    }
    return g * s;
}

fn sample_backdrop_blur(uv: vec2<f32>, blur_px: f32, inv_vp: vec2<f32>) -> vec3<f32> {
    if blur_px <= 0.35 {
        return textureSampleLevel(backdrop_tex, tex_sampler, clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999)), 0.0).rgb;
    }
    // 12 点双环旋转采样，在保持低 GPU 开销的同时输出平滑光斑感
    var acc = textureSampleLevel(backdrop_tex, tex_sampler, clamp(uv, vec2<f32>(0.001), vec2<f32>(0.999)), 0.0).rgb * 0.22;
    let ring1 = array<vec2<f32>, 6>(
        vec2<f32>(1.0, 0.0),
        vec2<f32>(0.5, 0.866),
        vec2<f32>(-0.5, 0.866),
        vec2<f32>(-1.0, 0.0),
        vec2<f32>(-0.5, -0.866),
        vec2<f32>(0.5, -0.866),
    );
    let ring2 = array<vec2<f32>, 6>(
        vec2<f32>(0.866, 0.5),
        vec2<f32>(0.0, 1.0),
        vec2<f32>(-0.866, 0.5),
        vec2<f32>(-0.866, -0.5),
        vec2<f32>(0.0, -1.0),
        vec2<f32>(0.866, -0.5),
    );
    for (var i = 0u; i < 6u; i = i + 1u) {
        let u1 = clamp(uv + ring1[i] * blur_px * 0.45 * inv_vp, vec2<f32>(0.001), vec2<f32>(0.999));
        let u2 = clamp(uv + ring2[i] * blur_px * 1.00 * inv_vp, vec2<f32>(0.001), vec2<f32>(0.999));
        acc += textureSampleLevel(backdrop_tex, tex_sampler, u1, 0.0).rgb * 0.08;
        acc += textureSampleLevel(backdrop_tex, tex_sampler, u2, 0.0).rgb * 0.05;
    }
    return acc;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let fg = textureSample(fg_tex, tex_sampler, in.uv);
    let px = in.uv * u.viewport;
    let inv_vp = 1.0 / max(u.viewport, vec2<f32>(1.0, 1.0));

    var glass_rgb = vec3<f32>(0.0);
    var glass_a = 0.0;

    for (var i = 0u; i < u.shape_count; i = i + 1u) {
        let sh = u.shapes[i];
        let pos = sh.rect.xy;
        let size = sh.rect.zw;
        let radius = sh.params0.x;
        let opacity = sh.params0.y;
        let refraction = sh.params0.z;
        let dispersion = sh.params0.w;
        let blur = sh.params1.x;
        let tint_alpha = sh.params1.y;
        let mix_w = sh.params1.z;
        let alpha_eff = opacity * mix_w;
        if alpha_eff <= 0.002 || size.x <= 1.0 || size.y <= 1.0 {
            continue;
        }

        let half = size * 0.5;
        let center = pos + half;
        let rel = px - center;
        let d = sd_rounded_box(rel, half, radius);

        // 外部柔化环境阴影（d > 0 且距边界 < 22px）
        if d > 0.0 {
            if d < 22.0 {
                let sh_a = exp(-d * 0.19) * 0.16 * alpha_eff;
                glass_rgb = vec3<f32>(0.0);
                glass_a = max(glass_a, sh_a);
            }
            continue;
        }

        // 抗锯齿边界覆盖
        let cov = clamp(0.5 - d, 0.0, 1.0);
        let grad = sdf_grad_rounded_box(rel, half, radius);

        // 曲面透镜边缘折射带：14~32px 边缘锐利折射，中心极致清透
        let band = clamp(min(half.x, half.y) * 0.46, 14.0, 32.0);
        let edge_t = clamp((-d) / band, 0.0, 1.0);
        let slope = pow(1.0 - edge_t, 2.05);

        // 中心采用轻度光学柔焦保留背景质感，弯折带极低模糊以凸显棱镜拉伸锐度
        let blur_px = blur * 8.0 * mix(1.0, 0.10, slope);

        // Snell 边缘向内折射 + RGB 三通道差异折射率色散
        let refract_px = -grad * slope * (refraction * 42.0);
        let disp_px = grad * slope * (dispersion * 8.5);

        let uv_r = in.uv + (refract_px + disp_px) * inv_vp;
        let uv_g = in.uv + refract_px * inv_vp;
        let uv_b = in.uv + (refract_px - disp_px) * inv_vp;

        let col_g = sample_backdrop_blur(uv_g, blur_px, inv_vp);
        var refracted = col_g;
        if dispersion > 0.01 && slope > 0.02 {
            let r_ch = sample_backdrop_blur(uv_r, blur_px * 0.65, inv_vp).r;
            let b_ch = sample_backdrop_blur(uv_b, blur_px * 0.65, inv_vp).b;
            refracted = vec3<f32>(r_ch, col_g.g, b_ch);
        }

        let luma_weights = vec3<f32>(0.2126, 0.7152, 0.0722);
        let luma = dot(refracted, luma_weights);
        let vivid = clamp(mix(vec3<f32>(luma), refracted, 1.22), vec3<f32>(0.0), vec3<f32>(1.0));
        let light_bg = smoothstep(0.45, 0.88, luma);
        let dye_weight = sh.body_dye.a;
        let neutral = mix(vec3<f32>(0.05, 0.07, 0.12), vec3<f32>(0.88, 0.92, 0.98), light_bg);
        let neutral_weight = tint_alpha * mix(1.0, 0.25, light_bg) * (1.0 - dye_weight);
        var tinted = mix(vivid, neutral, neutral_weight);

        // Keep the pane visible even on an almost-white or almost-black background.
        // This offsets the transmitted light instead of covering the backdrop with a flat fill.
        let lightness_offset = mix(0.093, -0.108, smoothstep(0.18, 0.88, luma));
        let pane_gradient = -0.019 * rel.y / max(half.y, 1.0);
        tinted = clamp(tinted + vec3<f32>(lightness_offset + pane_gradient), vec3<f32>(0.0), vec3<f32>(1.0));
        if dye_weight > 0.0 {
            let seed = sh.body_dye.rgb;
            let seed_luma = dot(seed, luma_weights);
            let local_luma = dot(tinted, luma_weights);
            let dye_luma = mix(local_luma, clamp(local_luma, 0.24, 0.77), 0.62)
                + (seed_luma - 0.5) * 0.12;
            let dye = clamp(vec3<f32>(dye_luma) + (seed - vec3<f32>(seed_luma)) * mix(0.86, 0.80, light_bg),
                vec3<f32>(0.0), vec3<f32>(1.0));
            // This is transmitted body color, not a highlight. Do not attenuate on light backgrounds.
            tinted = mix(tinted, dye, dye_weight);
        }
        // A wider contrast band separates the pane from mid-tone or busy backgrounds as well.
        let contour = (1.0 - smoothstep(0.0, 0.52, edge_t)) * 0.17;
        let contour_ink = mix(0.78, 0.24, smoothstep(0.35, 0.60, luma));
        tinted = mix(tinted, vec3<f32>(contour_ink), contour);

        // 双角对角线菲涅尔镜面反光带 + 1.2px 定向晶亮高光棱边 + 亮底外缘极细冷调轮廓线
        let light_dir = normalize(vec2<f32>(-0.50, -0.86));
        let fresnel = 0.30 + 0.70 * pow(abs(dot(grad, light_dir)), 1.55);
        let top_sheen = (1.0 - smoothstep(-0.95, 0.48, rel.y / max(half.y, 1.0))) * pow(1.0 - edge_t, 2.3) * (0.11 * (1.0 - 0.55 * light_bg));
        let inner_sheen = pow(1.0 - edge_t, 3.6) * (0.16 * (1.0 - 0.50 * light_bg)) * fresnel + top_sheen;
        let rim = smoothstep(-1.80, -0.25, d) * (1.0 - smoothstep(-0.20, 0.55, d));
        let rim_color = mix(vec3<f32>(0.84, 0.93, 1.0), vec3<f32>(1.0, 1.0, 1.0), fresnel);
        let outer_hairline = smoothstep(-1.10, -0.20, d) * (1.0 - smoothstep(-0.08, 0.40, d));

        tinted = tinted * (1.0 - outer_hairline * light_bg * 0.27)
            + vec3<f32>(inner_sheen)
            + rim_color * (rim * (0.54 + 0.40 * fresnel) * (1.0 - 0.40 * light_bg));
        tinted = clamp(tinted, vec3<f32>(0.0), vec3<f32>(1.0));

        let layer_a = cov * alpha_eff;
        glass_rgb = tinted * layer_a + glass_rgb * (1.0 - layer_a);
        glass_a = layer_a + glass_a * (1.0 - layer_a);
    }

    // 标准预乘 Alpha SrcOver：Vello 前景（文字/封面/图标）严格覆盖在折射玻璃之上
    let out_rgb = fg.rgb + glass_rgb * (1.0 - fg.a);
    let out_a = fg.a + glass_a * (1.0 - fg.a);
    return vec4<f32>(out_rgb, out_a);
}
"#;

/// 惰性创建的 WGSL 液态玻璃渲染管线与纹理资源，仅在用户选用 `LiquidGlass` 时分配。
pub struct LiquidGlassPipeline {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    fg_texture: wgpu::Texture,
    fg_view: wgpu::TextureView,
    backdrop_texture: wgpu::Texture,
    backdrop_view: wgpu::TextureView,
    bind_group: wgpu::BindGroup,
    format: wgpu::TextureFormat,
    fg_size: (u32, u32),
    backdrop_size: (u32, u32),
}

impl LiquidGlassPipeline {
    /// 初始化 WGSL 着色器管线与初始 1×1 占位纹理，后续按窗口或背景尺寸自动扩容。
    pub fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("DotIsle LiquidGlass Shader"),
            source: wgpu::ShaderSource::Wgsl(LIQUID_GLASS_WGSL.into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("DotIsle LiquidGlass BGL"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("DotIsle LiquidGlass PL"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("DotIsle LiquidGlass Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("DotIsle LiquidGlass Linear"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let init_uniform = encode_uniforms(width.max(1), height.max(1), &[]);
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("DotIsle LiquidGlass Uniforms"),
            size: init_uniform.len() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        queue.write_buffer(&uniform_buf, 0, &init_uniform);

        let w = width.max(1);
        let h = height.max(1);
        let fg_texture = Self::create_fg_tex(device, w, h, format);
        let fg_view = fg_texture.create_view(&Default::default());

        let backdrop_texture = Self::create_backdrop_tex(device, 2, 2);
        let backdrop_view = backdrop_texture.create_view(&Default::default());
        // 深色微渐变占位背景，防止首帧未采集到时产生闪烁
        let placeholder = [
            18, 22, 32, 255, 22, 26, 38, 255, 16, 20, 28, 255, 20, 24, 34, 255u8,
        ];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &backdrop_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &placeholder,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = Self::make_bind_group(
            device,
            &bind_group_layout,
            &uniform_buf,
            &backdrop_view,
            &fg_view,
            &sampler,
        );

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buf,
            fg_texture,
            fg_view,
            backdrop_texture,
            backdrop_view,
            bind_group,
            format,
            fg_size: (w, h),
            backdrop_size: (2, 2),
        }
    }

    fn create_fg_tex(
        device: &wgpu::Device,
        w: u32,
        h: u32,
        format: wgpu::TextureFormat,
    ) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DotIsle LiquidGlass FG"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        })
    }

    fn create_backdrop_tex(device: &wgpu::Device, w: u32, h: u32) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("DotIsle LiquidGlass Backdrop"),
            size: wgpu::Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        uniform_buf: &wgpu::Buffer,
        backdrop_view: &wgpu::TextureView,
        fg_view: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("DotIsle LiquidGlass BG"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(backdrop_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(fg_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// 获取供 Vello Hybrid 绘制前景控件/歌词/封面的离屏纹理视图，并在窗口缩放时按需重建。
    pub fn ensure_fg_view(&mut self, device: &wgpu::Device, w: u32, h: u32) -> &wgpu::TextureView {
        let w = w.max(1);
        let h = h.max(1);
        if self.fg_size != (w, h) {
            self.fg_texture = Self::create_fg_tex(device, w, h, self.format);
            self.fg_view = self.fg_texture.create_view(&Default::default());
            self.fg_size = (w, h);
            self.bind_group = Self::make_bind_group(
                device,
                &self.bind_group_layout,
                &self.uniform_buf,
                &self.backdrop_view,
                &self.fg_view,
                &self.sampler,
            );
        }
        &self.fg_view
    }

    /// 上传已裁剪好的 RGBA8 背景画面到 GPU，作为实时折射采样底图。
    pub fn update_backdrop(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, rgba: &[u8], w: u32, h: u32) {
        let w = w.max(1);
        let h = h.max(1);
        let needed = (w as usize) * (h as usize) * 4;
        if rgba.len() < needed {
            return;
        }
        if self.backdrop_size != (w, h) {
            self.backdrop_texture = Self::create_backdrop_tex(device, w, h);
            self.backdrop_view = self.backdrop_texture.create_view(&Default::default());
            self.backdrop_size = (w, h);
            self.bind_group = Self::make_bind_group(
                device,
                &self.bind_group_layout,
                &self.uniform_buf,
                &self.backdrop_view,
                &self.fg_view,
                &self.sampler,
            );
        }
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.backdrop_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba[..needed],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
    }

    /// 执行液态玻璃折射/色散/高光 RenderPass，并与前景视图复合写入 `target_view`。
    pub fn compose(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        width: u32,
        height: u32,
        shapes: &[GlassShape],
        target_view: &wgpu::TextureView,
    ) {
        let uniform_bytes = encode_uniforms(width, height, shapes);
        queue.write_buffer(&self.uniform_buf, 0, &uniform_bytes);
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("DotIsle LiquidGlass Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target_view,
                resolve_target: None,
                depth_slice: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniform_encoding_layout_matches_wgsl_alignment() {
        let s = GlassShape::new(
            20.0,
            10.0,
            260.0,
            40.0,
            20.0,
            1.0,
            0.7,
            0.4,
            0.35,
            0.4,
            1.0,
        );
        let bytes = encode_uniforms(640, 240, &[s]);
        assert_eq!(bytes.len(), 16 + MAX_GLASS_SHAPES * 64);
        assert_eq!(f32::from_le_bytes(bytes[0..4].try_into().unwrap()), 640.0);
        assert_eq!(f32::from_le_bytes(bytes[4..8].try_into().unwrap()), 240.0);
        assert_eq!(u32::from_le_bytes(bytes[8..12].try_into().unwrap()), 1);
    }

    #[test]
    fn glass_body_dye_encodes_seed_and_obeys_slider_and_form_weight() {
        let glass = GlassShape::new(0., 0., 120., 40., 20., 1., 0.76, 0.46, 0.08, 0.06, 1.);
        let make = |depth, enabled_mix| glass.with_body_dye(0xff0080, depth, enabled_mix);
        let normal = make(0.06, 1.);
        assert_eq!(&normal.body_dye[..3], &[1., 0., 128. / 255.]);
        assert_eq!(make(0., 1.).body_dye[3], 0.);
        assert_eq!(make(0.06, 0.).body_dye[3], 0.);
        assert_eq!(make(0.06, 0.5).body_dye[3], normal.body_dye[3] * 0.5);
        assert!((0.35..0.38).contains(&normal.body_dye[3]));
        assert!(make(1., 1.).body_dye[3] > normal.body_dye[3]);
        let bytes = encode_uniforms(120, 40, &[normal]);
        assert_eq!(f32::from_le_bytes(bytes[76..80].try_into().unwrap()), normal.body_dye[3]);
    }

    #[test]
    fn glass_tint_is_monotonic_and_zero_can_be_explicitly_neutral() {
        let make = |tint_01: f32| {
            GlassShape::new(
                0.0,
                0.0,
                120.0,
                40.0,
                20.0,
                1.0,
                0.72,
                0.42,
                0.12,
                tint_01,
                1.0,
            )
            .params1[1]
        };
        assert_eq!(make(0.0), 0.0);
        assert!((0.018..0.034).contains(&make(0.06)));
        assert!((0.15..0.22).contains(&make(0.42)));
        assert!(make(1.0) <= 0.44);
    }
}
