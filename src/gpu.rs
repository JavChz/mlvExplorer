//! Multi-pass GPU RAW viewer:
//! Pass 1a: Focus Pixel / Bad Pixel healing (`R16Uint` + optional `.fpm` `R8Uint` mask -> `R16Uint`)
//! Pass 1b: Full-resolution 5x5 Malvar-He-Cutler demosaic (`R16Uint` -> `Rgba16Float` mip 0)
//! Pass 1c: GPU mipmap chain generation (`Rgba16Float` mip 0 -> mip 1..N)
//! Pass 2:  Display shader with hardware trilinear mipmaps + Catmull-Rom bicubic,
//!          color matrix + WB, exposure, highlight roll-off, 90-deg rotation, zebras & focus peaking.

use crate::mlv::{DecodedFrame, FocusPixelMap};
use eframe::egui_wgpu::{self, wgpu};
use std::sync::Arc;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct PreprocessUniforms {
    /// black, white, pdaf_strength (1.0 or 0.0), focus_mode (0=Off, 1=FpmOnly, 2=Auto)
    pub levels: [f32; 4],
    /// red_x, red_y, width, height
    pub info: [u32; 4],
    /// has_fpm_mask (0/1), reserved...
    pub flags: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DisplayUniforms {
    pub m0: [f32; 4],
    pub m1: [f32; 4],
    pub m2: [f32; 4],
    /// exposure_mul, apply_gamma (1/0), highlight_rolloff (1/0), rotation_steps (0..3)
    pub params: [f32; 4],
    /// zebras (1/0), peaking (1/0), width, height
    pub overlays: [f32; 4],
    /// view rect in rotated normalized coords: x0, y0, sx, sy
    pub view: [f32; 4],
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct PreprocessKey {
    frame_key: u64,
    fpm_key: u64,
    focus_mode: u32,
    pdaf_bits: u32,
    black_bits: u32,
    white_bits: u32,
}

struct Resources {
    // Layouts & pipelines
    heal_bgl: wgpu::BindGroupLayout,
    heal_pipeline: wgpu::RenderPipeline,
    demosaic_bgl: wgpu::BindGroupLayout,
    demosaic_pipeline: wgpu::RenderPipeline,
    mip_bgl: wgpu::BindGroupLayout,
    mip_pipeline: wgpu::RenderPipeline,
    display_bgl: wgpu::BindGroupLayout,
    display_pipeline: wgpu::RenderPipeline,

    // Uniforms & samplers
    preprocess_uniforms: wgpu::Buffer,
    display_uniforms: wgpu::Buffer,
    linear_sampler: wgpu::Sampler,

    // Per-resolution textures & bind groups
    tex_dims: (u32, u32),
    mip_levels: u32,
    raw_tex: Option<wgpu::Texture>,
    fpm_tex: Option<wgpu::Texture>,
    raw_clean_tex: Option<wgpu::Texture>,
    raw_clean_view: Option<wgpu::TextureView>,
    demosaiced_tex: Option<wgpu::Texture>,
    heal_bg: Option<wgpu::BindGroup>,
    demosaic_bg: Option<wgpu::BindGroup>,
    mip_views: Vec<wgpu::TextureView>,
    mip_bgs: Vec<wgpu::BindGroup>,
    display_bg: Option<wgpu::BindGroup>,

    uploaded_frame: u64,
    uploaded_fpm: u64,
    last_preprocess: Option<PreprocessKey>,
    apply_gamma: bool,
}

const PREPROCESS_SHADER: &str = r#"
struct PU {
    levels: vec4<f32>,
    info: vec4<u32>,
    flags: vec4<u32>,
};
@group(0) @binding(0) var<uniform> pu: PU;
@group(0) @binding(1) var raw_tex: texture_2d<u32>;
@group(0) @binding(2) var fpm_tex: texture_2d<u32>;

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

fn clamp_cfa(x: i32, y: i32) -> vec2<i32> {
    let w = i32(pu.info.z);
    let h = i32(pu.info.w);
    var cx = x;
    var cy = y;
    if (cx < 0) { cx = cx + 2 * ((-cx + 1) / 2); }
    if (cy < 0) { cy = cy + 2 * ((-cy + 1) / 2); }
    if (cx >= w) { cx = cx - 2 * ((cx - w) / 2 + 1); }
    if (cy >= h) { cy = cy - 2 * ((cy - h) / 2 + 1); }
    return vec2<i32>(clamp(cx, 0, w - 1), clamp(cy, 0, h - 1));
}

fn raw_at(x: i32, y: i32) -> f32 {
    return f32(textureLoad(raw_tex, clamp_cfa(x, y), 0).r);
}

fn norm_raw(v: f32) -> f32 {
    let range = max(pu.levels.y - pu.levels.x, 1.0);
    return max((v - pu.levels.x) / range, 0.0);
}

fn fpm_marked(x: i32, y: i32) -> bool {
    if (pu.flags.x == 0u) {
        return false;
    }
    return textureLoad(fpm_tex, clamp_cfa(x, y), 0).r != 0u;
}

fn sort4(v0: f32, v1: f32, v2: f32, v3: f32) -> vec4<f32> {
    var a = v0; var b = v1; var c = v2; var d = v3;
    var t: f32;
    if (a > b) { t = a; a = b; b = t; }
    if (c > d) { t = c; c = d; d = t; }
    if (a > c) { t = a; a = c; c = t; }
    if (b > d) { t = b; b = d; d = t; }
    if (b > c) { t = b; b = c; c = t; }
    return vec4<f32>(a, b, c, d);
}

@fragment
fn heal_fs(in: VsOut) -> @location(0) u32 {
    let coord = vec2<i32>(in.pos.xy);
    let x = coord.x;
    let y = coord.y;
    let center_u = textureLoad(raw_tex, coord, 0).r;
    let mode = u32(pu.levels.w + 0.5);
    if (mode == 0u) {
        return center_u;
    }

    let marked = fpm_marked(x, y);
    if (mode == 1u && !marked) {
        return center_u;
    }

    let a = (u32(x) & 1u) ^ pu.info.x;
    let b = (u32(y) & 1u) ^ pu.info.y;
    let is_g = (a != b);

    var r0: f32; var r1: f32; var r2: f32; var r3: f32;
    var m0: bool; var m1: bool; var m2: bool; var m3: bool;
    if (is_g) {
        r0 = raw_at(x - 1, y - 1); r1 = raw_at(x + 1, y + 1);
        r2 = raw_at(x + 1, y - 1); r3 = raw_at(x - 1, y + 1);
        m0 = fpm_marked(x - 1, y - 1); m1 = fpm_marked(x + 1, y + 1);
        m2 = fpm_marked(x + 1, y - 1); m3 = fpm_marked(x - 1, y + 1);
    } else {
        r0 = raw_at(x - 2, y); r1 = raw_at(x + 2, y);
        r2 = raw_at(x, y - 2); r3 = raw_at(x, y + 2);
        m0 = fpm_marked(x - 2, y); m1 = fpm_marked(x + 2, y);
        m2 = fpm_marked(x, y - 2); m3 = fpm_marked(x, y + 2);
    }

    var should_heal = marked;
    if (!should_heal && mode == 2u) {
        let c = norm_raw(f32(center_u));
        let n0 = norm_raw(r0); let n1 = norm_raw(r1);
        let n2 = norm_raw(r2); let n3 = norm_raw(r3);
        let s = sort4(n0, n1, n2, n3);
        let med4 = 0.5 * (s.y + s.z);
        let pdaf = pu.levels.z > 0.5;

        if (pdaf) {
            // Detect isolated PDAF dark dot or horizontal PDAF pair while preserving edges/lines
            let tail_spread = s.w - s.y;
            let full_spread = s.w - s.x;
            let dark_single = (c < s.x) && ((med4 - c) > max(0.042 * med4 + 0.0045, 1.65 * full_spread));
            let dark_pair = (c < s.y) && ((med4 - c) > max(0.055 * med4 + 0.006, 2.0 * tail_spread));
            let hot = (c > s.w) && ((c - med4) > max(0.20 * med4 + 0.025, 3.0 * full_spread));
            if (dark_single || dark_pair || hot) {
                should_heal = true;
            }
        } else {
            // Conservative dead/hot pixel guard on non-PDAF sensors
            let full_spread = s.w - s.x;
            let dead = (c < s.x) && ((med4 - c) > max(0.25 * med4 + 0.03, 3.2 * full_spread));
            let hot = (c > s.w) && ((c - med4) > max(0.25 * med4 + 0.03, 3.2 * full_spread));
            if (dead || hot) {
                should_heal = true;
            }
        }
    }

    if (!should_heal) {
        return center_u;
    }

    var healed: f32;
    if (!m0 && !m1 && !m2 && !m3) {
        if (abs(r0 - r1) <= abs(r2 - r3)) {
            healed = 0.5 * (r0 + r1);
        } else {
            healed = 0.5 * (r2 + r3);
        }
    } else if (!m0 && !m1) {
        healed = 0.5 * (r0 + r1);
    } else if (!m2 && !m3) {
        healed = 0.5 * (r2 + r3);
    } else {
        var sum = 0.0;
        var cnt = 0.0;
        if (!m0) { sum = sum + r0; cnt = cnt + 1.0; }
        if (!m1) { sum = sum + r1; cnt = cnt + 1.0; }
        if (!m2) { sum = sum + r2; cnt = cnt + 1.0; }
        if (!m3) { sum = sum + r3; cnt = cnt + 1.0; }
        if (cnt > 0.0) {
            healed = sum / cnt;
        } else {
            healed = 0.25 * (r0 + r1 + r2 + r3);
        }
    }

    return u32(clamp(round(healed), 0.0, 65535.0));
}
"#;

const DEMOSAIC_SHADER: &str = r#"
struct PU {
    levels: vec4<f32>,
    info: vec4<u32>,
    flags: vec4<u32>,
};
@group(0) @binding(0) var<uniform> pu: PU;
@group(0) @binding(1) var clean_raw: texture_2d<u32>;

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
    let w = i32(pu.info.z);
    let h = i32(pu.info.w);
    var cx = x; var cy = y;
    if (cx < 0) { cx = cx + 2 * ((-cx + 1) / 2); }
    if (cy < 0) { cy = cy + 2 * ((-cy + 1) / 2); }
    if (cx >= w) { cx = cx - 2 * ((cx - w) / 2 + 1); }
    if (cy >= h) { cy = cy - 2 * ((cy - h) / 2 + 1); }
    let v = f32(textureLoad(clean_raw, vec2<i32>(clamp(cx, 0, w - 1), clamp(cy, 0, h - 1)), 0).r);
    return (v - pu.levels.x) / max(pu.levels.y - pu.levels.x, 1.0);
}

// Malvar-He-Cutler 5x5 demosaic -> linear camera RGB
@fragment
fn demosaic_fs(in: VsOut) -> @location(0) vec4<f32> {
    let x = i32(in.pos.x);
    let y = i32(in.pos.y);
    let a = (u32(x) & 1u) ^ pu.info.x;
    let b = (u32(y) & 1u) ^ pu.info.y;
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

    var cam: vec3<f32>;
    if (a == 0u && b == 0u) { cam = vec3<f32>(c, g_at_rb, diag); }
    else if (a == 1u && b == 1u) { cam = vec3<f32>(diag, g_at_rb, c); }
    else if (b == 0u) { cam = vec3<f32>(horiz, c, vert); }
    else { cam = vec3<f32>(vert, c, horiz); }

    return vec4<f32>(max(cam, vec3<f32>(0.0)), 1.0);
}
"#;

const MIP_SHADER: &str = r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var src_samp: sampler;

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

@fragment
fn mip_fs(in: VsOut) -> @location(0) vec4<f32> {
    return textureSampleLevel(src_tex, src_samp, in.uv, 0.0);
}
"#;

const DISPLAY_SHADER: &str = r#"
struct DU {
    m0: vec4<f32>, m1: vec4<f32>, m2: vec4<f32>,
    params: vec4<f32>,
    overlays: vec4<f32>,
    view: vec4<f32>,
};
@group(0) @binding(0) var<uniform> du: DU;
@group(0) @binding(1) var demosaiced_tex: texture_2d<f32>;
@group(0) @binding(2) var linear_samp: sampler;

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

fn cubic_weights(f: f32) -> vec4<f32> {
    let f2 = f * f;
    let f3 = f2 * f;
    return vec4<f32>(
        -0.5 * f + f2 - 0.5 * f3,
        1.0 - 2.5 * f2 + 1.5 * f3,
        0.5 * f + 2.0 * f2 - 1.5 * f3,
        -0.5 * f2 + 0.5 * f3
    );
}

fn sample_catmull_rom(uv: vec2<f32>, dims: vec2<f32>) -> vec3<f32> {
    let max_coord = vec2<i32>(i32(dims.x) - 1, i32(dims.y) - 1);
    let pos = uv * dims - 0.5;
    let base = vec2<i32>(floor(pos));
    let f = fract(pos);
    let wx = cubic_weights(f.x);
    let wy = cubic_weights(f.y);

    var sum = vec3<f32>(0.0);
    for (var j = -1; j <= 2; j = j + 1) {
        let cy = clamp(base.y + j, 0, max_coord.y);
        let wy_j = wy[j + 1];
        var row = vec3<f32>(0.0);
        for (var i = -1; i <= 2; i = i + 1) {
            let cx = clamp(base.x + i, 0, max_coord.x);
            let s = textureLoad(demosaiced_tex, vec2<i32>(cx, cy), 0).rgb;
            row = row + s * wx[i + 1];
        }
        sum = sum + row * wy_j;
    }
    return max(sum, vec3<f32>(0.0));
}

fn rolloff_shoulder(x: f32) -> f32 {
    if (x <= 0.80) {
        return x;
    }
    let over = x - 0.80;
    return 0.80 + (0.20 * over) / (0.20 + over);
}

fn srgb(x: vec3<f32>) -> vec3<f32> {
    let c = clamp(x, vec3<f32>(0.0), vec3<f32>(1.0));
    let lo = c * 12.92;
    let hi = 1.055 * pow(c, vec3<f32>(1.0 / 2.4)) - 0.055;
    return select(hi, lo, c <= vec3<f32>(0.0031308));
}

fn cam_luma(c: vec3<f32>) -> f32 {
    return dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
}

@fragment
fn display_fs(in: VsOut) -> @location(0) vec4<f32> {
    let duv = du.view.xy + in.uv * du.view.zw;
    let rot = u32(du.params.w + 0.5) & 3u;
    var suv: vec2<f32>;
    if (rot == 1u) {
        suv = vec2<f32>(duv.y, 1.0 - duv.x);
    } else if (rot == 2u) {
        suv = vec2<f32>(1.0 - duv.x, 1.0 - duv.y);
    } else if (rot == 3u) {
        suv = vec2<f32>(1.0 - duv.y, duv.x);
    } else {
        suv = duv;
    }

    let dims = du.overlays.zw;
    let p = suv * dims;
    let fp = max(length(dpdx(p)), length(dpdy(p)));

    // Always sample trilinear mipmap in uniform control flow
    let tri = textureSample(demosaiced_tex, linear_samp, suv).rgb;
    var cam = tri;
    if (fp < 1.35) {
        let bicubic = sample_catmull_rom(suv, dims);
        let blend = smoothstep(1.0, 1.35, fp);
        cam = mix(bicubic, tri, blend);
    }

    cam = max(cam, vec3<f32>(0.0)) * du.params.x;
    var rgb = vec3<f32>(dot(du.m0.xyz, cam), dot(du.m1.xyz, cam), dot(du.m2.xyz, cam));
    rgb = max(rgb, vec3<f32>(0.0));

    let pre_clip_max = max(rgb.r, max(rgb.g, rgb.b));

    if (du.params.z > 0.5) {
        let peak = pre_clip_max;
        if (peak > 0.80) {
            rgb = rgb * (rolloff_shoulder(peak) / peak);
        }
    }

    var outc = clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0));
    if (du.params.y > 0.5) {
        outc = srgb(outc);
    }

    // Focus peaking (Sobel edge energy on full-res linear demosaiced frame)
    if (du.overlays.y > 0.5) {
        let max_c = vec2<i32>(i32(dims.x) - 1, i32(dims.y) - 1);
        let ic = vec2<i32>(clamp(p, vec2<f32>(0.0), dims - 1.0));
        let tl = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>(-1, -1), vec2<i32>(0), max_c), 0).rgb);
        let tc = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>( 0, -1), vec2<i32>(0), max_c), 0).rgb);
        let tr = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>( 1, -1), vec2<i32>(0), max_c), 0).rgb);
        let ml = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>(-1,  0), vec2<i32>(0), max_c), 0).rgb);
        let mr = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>( 1,  0), vec2<i32>(0), max_c), 0).rgb);
        let bl = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>(-1,  1), vec2<i32>(0), max_c), 0).rgb);
        let bc = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>( 0,  1), vec2<i32>(0), max_c), 0).rgb);
        let br = cam_luma(textureLoad(demosaiced_tex, clamp(ic + vec2<i32>( 1,  1), vec2<i32>(0), max_c), 0).rgb);
        let gx = (-tl + tr - 2.0 * ml + 2.0 * mr - bl + br) * du.params.x;
        let gy = (-tl - 2.0 * tc - tr + bl + 2.0 * bc + br) * du.params.x;
        let edge = sqrt(gx * gx + gy * gy);
        if (edge > 0.24) {
            let k = clamp((edge - 0.24) * 2.5, 0.0, 0.9);
            outc = mix(outc, vec3<f32>(0.15, 1.0, 0.45), k);
        }
    }

    // Highlight clipping zebras
    if (du.overlays.x > 0.5 && pre_clip_max >= 0.98) {
        let stripe = ((i32(in.pos.x) + i32(in.pos.y)) / 5) & 1;
        outc = select(vec3<f32>(0.15, 0.15, 0.15), vec3<f32>(1.0, 0.25, 0.25), stripe == 0);
    }

    return vec4<f32>(outc, 1.0);
}
"#;

/// Install GPU resources once at startup. Returns adapter description.
pub fn init(render_state: &egui_wgpu::RenderState) -> String {
    let device = &render_state.device;

    let pre_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mlv_preprocess"),
        source: wgpu::ShaderSource::Wgsl(PREPROCESS_SHADER.into()),
    });
    let dem_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mlv_demosaic"),
        source: wgpu::ShaderSource::Wgsl(DEMOSAIC_SHADER.into()),
    });
    let mip_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mlv_mip"),
        source: wgpu::ShaderSource::Wgsl(MIP_SHADER.into()),
    });
    let disp_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("mlv_display"),
        source: wgpu::ShaderSource::Wgsl(DISPLAY_SHADER.into()),
    });

    // Pass 1a layout: uniform + raw R16Uint + fpm R8Uint
    let heal_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mlv_heal_bgl"),
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
            wgpu::BindGroupLayoutEntry {
                binding: 2,
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
    let heal_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mlv_heal_pl"),
        bind_group_layouts: &[&heal_bgl],
        push_constant_ranges: &[],
    });
    let heal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mlv_heal_pipeline"),
        layout: Some(&heal_pl),
        vertex: wgpu::VertexState {
            module: &pre_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &pre_shader,
            entry_point: "heal_fs",
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::R16Uint,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // Pass 1b layout: uniform + raw_clean R16Uint -> Rgba16Float
    let demosaic_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mlv_demosaic_bgl"),
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
    let demosaic_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mlv_demosaic_pl"),
        bind_group_layouts: &[&demosaic_bgl],
        push_constant_ranges: &[],
    });
    let demosaic_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mlv_demosaic_pipeline"),
        layout: Some(&demosaic_pl),
        vertex: wgpu::VertexState {
            module: &dem_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &dem_shader,
            entry_point: "demosaic_fs",
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // Pass 1c layout: mip downsample Rgba16Float -> Rgba16Float
    let mip_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mlv_mip_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let mip_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mlv_mip_pl"),
        bind_group_layouts: &[&mip_bgl],
        push_constant_ranges: &[],
    });
    let mip_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mlv_mip_pipeline"),
        layout: Some(&mip_pl),
        vertex: wgpu::VertexState {
            module: &mip_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &mip_shader,
            entry_point: "mip_fs",
            targets: &[Some(wgpu::ColorTargetState {
                format: wgpu::TextureFormat::Rgba16Float,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    // Pass 2 layout: DisplayUniforms + mipmapped Rgba16Float + linear_sampler -> target_format
    let display_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("mlv_display_bgl"),
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
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });
    let display_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("mlv_display_pl"),
        bind_group_layouts: &[&display_bgl],
        push_constant_ranges: &[],
    });
    let display_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("mlv_display_pipeline"),
        layout: Some(&display_pl),
        vertex: wgpu::VertexState {
            module: &disp_shader,
            entry_point: "vs_main",
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &disp_shader,
            entry_point: "display_fs",
            targets: &[Some(render_state.target_format.into())],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview: None,
        cache: None,
    });

    let preprocess_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mlv_pre_uniforms"),
        size: std::mem::size_of::<PreprocessUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let display_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mlv_disp_uniforms"),
        size: std::mem::size_of::<DisplayUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("mlv_trilinear_sampler"),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        mipmap_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    render_state
        .renderer
        .write()
        .callback_resources
        .insert(Resources {
            heal_bgl,
            heal_pipeline,
            demosaic_bgl,
            demosaic_pipeline,
            mip_bgl,
            mip_pipeline,
            display_bgl,
            display_pipeline,
            preprocess_uniforms,
            display_uniforms,
            linear_sampler,
            tex_dims: (0, 0),
            mip_levels: 1,
            raw_tex: None,
            fpm_tex: None,
            raw_clean_tex: None,
            raw_clean_view: None,
            demosaiced_tex: None,
            heal_bg: None,
            demosaic_bg: None,
            mip_views: Vec::new(),
            mip_bgs: Vec::new(),
            display_bg: None,
            uploaded_frame: u64::MAX,
            uploaded_fpm: u64::MAX,
            last_preprocess: None,
            apply_gamma: !render_state.target_format.is_srgb(),
        });

    let info = render_state.adapter.get_info();
    format!("{} · {:?}", info.name, info.backend)
}

/// Per-frame paint callback.
pub struct RawViewCallback {
    pub frame: Arc<DecodedFrame>,
    /// unique key (clip id << 32 | frame index) to avoid re-uploads
    pub key: u64,
    /// unique key for FPM map (clip id when FPM present, 0 otherwise)
    pub fpm_key: u64,
    pub fpm_map: Option<Arc<FocusPixelMap>>,
    pub preprocess: PreprocessUniforms,
    pub display: DisplayUniforms,
}

impl Resources {
    fn rebuild_for_size(&mut self, device: &wgpu::Device, w: u32, h: u32) {
        let max_dim = w.max(h).max(1) as f32;
        let mip_levels = ((max_dim.log2().floor() as u32) + 1).clamp(1, 6);

        let raw_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mlv_raw"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let fpm_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mlv_fpm_mask"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let raw_clean_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mlv_raw_clean"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R16Uint,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let demosaiced_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("mlv_demosaiced"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: mip_levels,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let raw_view = raw_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let fpm_view = fpm_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let clean_view = raw_clean_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let full_dem_view = demosaiced_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let mut mip_views = Vec::with_capacity(mip_levels as usize);
        for level in 0..mip_levels {
            mip_views.push(demosaiced_tex.create_view(&wgpu::TextureViewDescriptor {
                label: Some("mlv_demosaiced_mip"),
                base_mip_level: level,
                mip_level_count: Some(1),
                ..Default::default()
            }));
        }

        let mut mip_bgs = Vec::with_capacity(mip_levels.saturating_sub(1) as usize);
        for level in 0..mip_levels.saturating_sub(1) as usize {
            mip_bgs.push(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("mlv_mip_bg"),
                layout: &self.mip_bgl,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&mip_views[level]),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                    },
                ],
            }));
        }

        self.heal_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mlv_heal_bg"),
            layout: &self.heal_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.preprocess_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&raw_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&fpm_view),
                },
            ],
        }));

        self.demosaic_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mlv_demosaic_bg"),
            layout: &self.demosaic_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.preprocess_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&clean_view),
                },
            ],
        }));

        self.display_bg = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("mlv_display_bg"),
            layout: &self.display_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.display_uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&full_dem_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&self.linear_sampler),
                },
            ],
        }));

        self.tex_dims = (w, h);
        self.mip_levels = mip_levels;
        self.raw_tex = Some(raw_tex);
        self.fpm_tex = Some(fpm_tex);
        self.raw_clean_tex = Some(raw_clean_tex);
        self.raw_clean_view = Some(clean_view);
        self.demosaiced_tex = Some(demosaiced_tex);
        self.mip_views = mip_views;
        self.mip_bgs = mip_bgs;
        self.uploaded_frame = u64::MAX;
        self.uploaded_fpm = u64::MAX;
        self.last_preprocess = None;
    }
}

impl egui_wgpu::CallbackTrait for RawViewCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        res: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let Some(r) = res.get_mut::<Resources>() else {
            return Vec::new();
        };
        let (w, h) = (self.frame.width, self.frame.height);
        if w == 0 || h == 0 || self.frame.data.len() < (w * h) as usize {
            return Vec::new();
        }

        if r.tex_dims != (w, h) || r.raw_tex.is_none() {
            r.rebuild_for_size(device, w, h);
        }

        if r.uploaded_frame != self.key {
            if let Some(tex) = &r.raw_tex {
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
                r.uploaded_frame = self.key;
            }
        }

        if r.uploaded_fpm != self.fpm_key {
            if let (Some(tex), Some(map)) = (&r.fpm_tex, &self.fpm_map) {
                if map.width == w && map.height == h && map.mask.len() >= (w * h) as usize {
                    queue.write_texture(
                        wgpu::ImageCopyTexture {
                            texture: tex,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect: wgpu::TextureAspect::All,
                        },
                        &map.mask[..(w * h) as usize],
                        wgpu::ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(w),
                            rows_per_image: Some(h),
                        },
                        wgpu::Extent3d {
                            width: w,
                            height: h,
                            depth_or_array_layers: 1,
                        },
                    );
                }
            }
            r.uploaded_fpm = self.fpm_key;
        }

        let cur_key = PreprocessKey {
            frame_key: self.key,
            fpm_key: self.fpm_key,
            focus_mode: (self.preprocess.levels[3] + 0.5) as u32,
            pdaf_bits: self.preprocess.levels[2].to_bits(),
            black_bits: self.preprocess.levels[0].to_bits(),
            white_bits: self.preprocess.levels[1].to_bits(),
        };

        if r.last_preprocess != Some(cur_key) {
            queue.write_buffer(&r.preprocess_uniforms, 0, bytemuck::bytes_of(&self.preprocess));

            if let (Some(clean_view), Some(heal_bg)) = (&r.raw_clean_view, &r.heal_bg) {
                let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mlv_pass1a_heal"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: clean_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&r.heal_pipeline);
                rp.set_bind_group(0, heal_bg, &[]);
                rp.draw(0..3, 0..1);
            }

            if let (Some(dem_mip0), Some(dem_bg)) = (r.mip_views.first(), &r.demosaic_bg) {
                let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mlv_pass1b_demosaic"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: dem_mip0,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                rp.set_pipeline(&r.demosaic_pipeline);
                rp.set_bind_group(0, dem_bg, &[]);
                rp.draw(0..3, 0..1);
            }

            for level in 1..r.mip_levels as usize {
                if let (Some(dst_view), Some(bg)) = (r.mip_views.get(level), r.mip_bgs.get(level - 1)) {
                    let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("mlv_pass1c_mip"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: dst_view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: None,
                        occlusion_query_set: None,
                    });
                    rp.set_pipeline(&r.mip_pipeline);
                    rp.set_bind_group(0, bg, &[]);
                    rp.draw(0..3, 0..1);
                }
            }

            r.last_preprocess = Some(cur_key);
        }

        let mut du = self.display;
        du.params[1] = if r.apply_gamma { 1.0 } else { 0.0 };
        queue.write_buffer(&r.display_uniforms, 0, bytemuck::bytes_of(&du));
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
        let Some(bg) = &r.display_bg else {
            return;
        };
        render_pass.set_pipeline(&r.display_pipeline);
        render_pass.set_bind_group(0, bg, &[]);
        render_pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wgsl_shaders_parse_and_validate() {
        use wgpu::naga::{
            front::wgsl,
            valid::{Capabilities, ValidationFlags, Validator},
        };
        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::empty());
        for (name, src) in [
            ("PREPROCESS_SHADER", PREPROCESS_SHADER),
            ("DEMOSAIC_SHADER", DEMOSAIC_SHADER),
            ("MIP_SHADER", MIP_SHADER),
            ("DISPLAY_SHADER", DISPLAY_SHADER),
        ] {
            let module = wgsl::parse_str(src)
                .unwrap_or_else(|e| panic!("WGSL parse error in {name}: {}", e.emit_to_string(src)));
            validator
                .validate(&module)
                .unwrap_or_else(|e| panic!("WGSL validation error in {name}: {e:?}"));
        }
    }
}
