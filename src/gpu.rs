//! GPU RAW viewer: uploads the 16-bit Bayer frame as an R16Uint texture and
//! demosaics (Malvar-He-Cutler) + color-transforms in a WGSL fragment shader.
//! Runs through wgpu => Vulkan / DX12 / Metal (NVIDIA, AMD, Intel, Apple).

use crate::mlv::DecodedFrame;
use eframe::egui_wgpu::{self, wgpu};
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ViewUniforms {
    pub m0: [f32; 4],
    pub m1: [f32; 4],
    pub m2: [f32; 4],
    /// black, white, exposure multiplier, apply_gamma (1/0)
    pub levels: [f32; 4],
    /// red_x, red_y, width, height
    pub info: [u32; 4],
    /// view rect in normalized raw coords: x0, y0, sx, sy
    pub view: [f32; 4],
}

struct Resources {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    uniforms: wgpu::Buffer,
    texture: Option<(wgpu::Texture, u32, u32)>,
    bind_group: Option<wgpu::BindGroup>,
    uploaded: u64,
    apply_gamma: bool,
}

const SHADER: &str = r#"
struct U {
    m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32>,
    levels: vec4<f32>,
    info: vec4<u32>,
    view: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var raw: texture_2d<u32>;

struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };

@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    let x = f32((i << 1u) & 2u);
    let y = f32(i & 2u);
    var o: VsOut;
    o.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    o.uv = vec2<f32>(x, y);
    return o;
}

fn px(x: i32, y: i32) -> f32 {
    let w = i32(u.info.z);
    let h = i32(u.info.w);
    var cx = x; var cy = y;
    // mirror by 2 to preserve CFA parity
    if (cx < 0) { cx = cx + 2 * ((-cx + 1) / 2); }
    if (cy < 0) { cy = cy + 2 * ((-cy + 1) / 2); }
    if (cx >= w) { cx = cx - 2 * ((cx - w) / 2 + 1); }
    if (cy >= h) { cy = cy - 2 * ((cy - h) / 2 + 1); }
    let v = f32(textureLoad(raw, vec2<i32>(cx, cy), 0).r);
    return (v - u.levels.x) / (u.levels.y - u.levels.x);
}

// Malvar-He-Cutler 5x5 demosaic -> camera RGB
fn demosaic(x: i32, y: i32) -> vec3<f32> {
    let a = (u32(x) & 1u) ^ u.info.x;
    let b = (u32(y) & 1u) ^ u.info.y;
    let c  = px(x, y);
    let n1 = px(x, y - 1); let s1 = px(x, y + 1);
    let w1 = px(x - 1, y); let e1 = px(x + 1, y);
    let n2 = px(x, y - 2); let s2 = px(x, y + 2);
    let w2 = px(x - 2, y); let e2 = px(x + 2, y);
    let dg = px(x - 1, y - 1) + px(x + 1, y - 1) + px(x - 1, y + 1) + px(x + 1, y + 1);

    let g_at_rb = (4.0 * c + 2.0 * (n1 + s1 + e1 + w1) - (n2 + s2 + e2 + w2)) / 8.0;
    let horiz   = (5.0 * c + 4.0 * (e1 + w1) - (e2 + w2) - dg + 0.5 * (n2 + s2)) / 8.0;
    let vert    = (5.0 * c + 4.0 * (n1 + s1) - (n2 + s2) - dg + 0.5 * (e2 + w2)) / 8.0;
    let diag    = (6.0 * c + 2.0 * dg - 1.5 * (n2 + s2 + e2 + w2)) / 8.0;

    if (a == 0u && b == 0u) { return vec3<f32>(c, g_at_rb, diag); }   // R site
    if (a == 1u && b == 1u) { return vec3<f32>(diag, g_at_rb, c); }   // B site
    if (b == 0u) { return vec3<f32>(horiz, c, vert); }                // G on red row
    return vec3<f32>(vert, c, horiz);                                 // G on blue row
}

// Box-filtered superpixels for minified display (avoids aliasing)
fn superpixel(x0: i32, y0: i32, n: i32) -> vec3<f32> {
    let rx = i32(u.info.x); let ry = i32(u.info.y);
    let bx = rx ^ 1; let by = ry ^ 1;
    var acc = vec3<f32>(0.0);
    for (var j = 0; j < n; j = j + 1) {
        for (var i = 0; i < n; i = i + 1) {
            let qx = x0 + i * 2; let qy = y0 + j * 2;
            let r = px(qx + rx, qy + ry);
            let bl = px(qx + bx, qy + by);
            let g = 0.5 * (px(qx + bx, qy + ry) + px(qx + rx, qy + by));
            acc = acc + vec3<f32>(r, g, bl);
        }
    }
    return acc / f32(n * n);
}

fn srgb(x: vec3<f32>) -> vec3<f32> {
    let c = clamp(x, vec3<f32>(0.0), vec3<f32>(1.0));
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let size = vec2<f32>(f32(u.info.z), f32(u.info.w));
    let p = (u.view.xy + in.uv * u.view.zw) * size;
    let fp = max(length(dpdx(p)), length(dpdy(p)));
    var cam: vec3<f32>;
    if (fp < 1.6) {
        cam = demosaic(i32(floor(p.x)), i32(floor(p.y)));
    } else {
        let n = clamp(i32(ceil(fp * 0.5)), 1, 4);
        let q = vec2<i32>(floor(p * 0.5)) * 2 - vec2<i32>((n - 1) / 2 * 2);
        cam = superpixel(q.x, q.y, n);
    }
    cam = max(cam, vec3<f32>(0.0)) * u.levels.z;
    let rgb = vec3<f32>(dot(u.m0.xyz, cam), dot(u.m1.xyz, cam), dot(u.m2.xyz, cam));
    var outc = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if (u.levels.w > 0.5) { outc = srgb(outc); }
    return vec4<f32>(outc, 1.0);
}
"#;

/// Install GPU resources once at startup. Returns adapter description.
pub fn init(render_state: &egui_wgpu::RenderState) -> String {
    let device = &render_state.device;
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mlv_debayer"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mlv_bgl"),
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
                    sample_type: wgpu::TextureSampleType::Uint,
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
        ],
    });
    let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mlv_pl"),
        bind_group_layouts: &[&layout],
        push_constant_ranges: &[],
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mlv_pipeline"),
        layout: Some(&pl),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: "fs_main",
            targets: &[Some(render_state.target_format.into())],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });
    let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mlv_uniforms"),
        size: std::mem::size_of::<ViewUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(Resources {
            pipeline,
            layout,
            uniforms,
            texture: None,
            bind_group: None,
            uploaded: u64::MAX,
            apply_gamma: !render_state.target_format.is_srgb(),
        });

    let info = render_state.adapter.get_info();
    format!("{} · {:?}", info.name, info.backend)
}

/// Per-frame paint callback.
pub struct RawViewCallback {
    pub frame: Arc<DecodedFrame>,
    /// unique key (clip generation << 32 | frame index) to avoid re-uploads
    pub key: u64,
    pub uniforms: ViewUniforms,
}

impl egui_wgpu::CallbackTrait for RawViewCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        res: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = res.get_mut::<Resources>() else {
            return Vec::new();
        };
        let (w, h) = (self.frame.width, self.frame.height);
        if w == 0 || h == 0 || self.frame.data.len() < (w * h) as usize {
            return Vec::new();
        }

        let need_new = !matches!(&r.texture, Some((_, tw, th)) if *tw == w && *th == h);
        if need_new {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mlv_raw"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R16Uint,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            r.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mlv_bg"),
                layout: &r.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: r.uniforms.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                ],
            }));
            r.texture = Some((tex, w, h));
            r.uploaded = u64::MAX;
        }

        if r.uploaded != self.key {
            if let Some((tex, _, _)) = &r.texture {
                queue.write_texture(
                    wgpu::ImageCopyTexture {
                        texture: tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    bytemuck::cast_slice(&self.frame.data[..(w * h) as usize]),
                    wgpu::ImageDataLayout {
                        offset: 0,
                        bytes_per_row: Some(w * 2),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
                r.uploaded = self.key;
            }
        }

        let mut u = self.uniforms;
        u.levels[3] = if r.apply_gamma { 1.0 } else { 0.0 };
        queue.write_buffer(&r.uniforms, 0, bytemuck::bytes_of(&u));
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        res: &egui_wgpu::CallbackResources,
    ) {
        let Some(r) = res.get::<Resources>() else {
            return;
        };
        let Some(bg) = &r.bind_group else {
            return;
        };
        render_pass.set_pipeline(&r.pipeline);
        render_pass.set_bind_group(0, bg, &[]);
        render_pass.draw(0..3, 0..1);
    }
}
