//! Neutral "view" transform: camera RGB -> linear sRGB (daylight WB baked in).
//! This is not grading — just a faithful preview, dcraw-style.

use super::decode::DecodedFrame;
use super::focus_pixels::{has_pdaf_sensor, heal_bayer_cpu};
use super::types::MlvMetadata;

pub type Mat3 = [[f32; 3]; 3];

const XYZ_FROM_SRGB: Mat3 = [
    [0.412453, 0.357580, 0.180423],
    [0.212671, 0.715160, 0.072169],
    [0.019334, 0.119193, 0.950227],
];

/// Generic Canon fallback ColorMatrix1 (5D Mark III), used if the file lacks one.
const FALLBACK_CAM_XYZ: [f32; 9] = [
    0.6722, -0.0635, -0.0963, -0.4287, 1.2460, 0.2028, -0.0908, 0.2162, 0.5668,
];

fn mul(a: &Mat3, b: &Mat3) -> Mat3 {
    let mut r = [[0.0; 3]; 3];
    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = (0..3).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    r
}

fn invert(m: &Mat3) -> Option<Mat3> {
    let det = m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1])
        - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0])
        + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
    if det.abs() < 1e-9 {
        return None;
    }
    let d = 1.0 / det;
    Some([
        [
            (m[1][1] * m[2][2] - m[1][2] * m[2][1]) * d,
            (m[0][2] * m[2][1] - m[0][1] * m[2][2]) * d,
            (m[0][1] * m[1][2] - m[0][2] * m[1][1]) * d,
        ],
        [
            (m[1][2] * m[2][0] - m[1][0] * m[2][2]) * d,
            (m[0][0] * m[2][2] - m[0][2] * m[2][0]) * d,
            (m[0][2] * m[1][0] - m[0][0] * m[1][2]) * d,
        ],
        [
            (m[1][0] * m[2][1] - m[1][1] * m[2][0]) * d,
            (m[0][1] * m[2][0] - m[0][0] * m[2][1]) * d,
            (m[0][0] * m[1][1] - m[0][1] * m[1][0]) * d,
        ],
    ])
}

/// Returns M such that  srgb_linear = M * cam_normalized  (WB included).
pub fn camera_to_srgb(meta: &MlvMetadata) -> Mat3 {
    let mut cam_xyz = [0f32; 9];
    let mut valid = meta.color_matrix.len() >= 18;
    if valid {
        for i in 0..9 {
            let num = meta.color_matrix[i * 2] as f32;
            let den = meta.color_matrix[i * 2 + 1] as f32;
            if den == 0.0 {
                valid = false;
                break;
            }
            cam_xyz[i] = num / den;
        }
        if cam_xyz.iter().all(|v| v.abs() < 1e-6) {
            valid = false;
        }
    }
    if !valid {
        cam_xyz = FALLBACK_CAM_XYZ;
    }
    let cx: Mat3 = [
        [cam_xyz[0], cam_xyz[1], cam_xyz[2]],
        [cam_xyz[3], cam_xyz[4], cam_xyz[5]],
        [cam_xyz[6], cam_xyz[7], cam_xyz[8]],
    ];
    let mut cam_rgb = mul(&cx, &XYZ_FROM_SRGB);
    let mut pre_mul = [1.0f32; 3];
    for i in 0..3 {
        let s: f32 = cam_rgb[i].iter().sum();
        if s.abs() > 1e-6 {
            for j in 0..3 {
                cam_rgb[i][j] /= s;
            }
            pre_mul[i] = 1.0 / s;
        }
    }
    let min = pre_mul.iter().cloned().fold(f32::MAX, f32::min).max(1e-6);
    for p in &mut pre_mul {
        *p /= min;
    }
    let rgb_cam = invert(&cam_rgb).unwrap_or([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]);
    let mut m = rgb_cam;
    for row in &mut m {
        for j in 0..3 {
            row[j] *= pre_mul[j];
        }
    }
    m
}

#[inline]
fn planckian_srgb_spd(kelvin: f32) -> [f32; 3] {
    let temp = kelvin.clamp(2200.0, 12000.0);
    let t = 1000.0 / temp;
    let x = if temp <= 4000.0 {
        -0.2661239 * t * t * t - 0.2343589 * t * t + 0.8776956 * t + 0.179910
    } else {
        -3.0258469 * t * t * t + 2.1070379 * t * t + 0.2226347 * t + 0.240390
    };
    let x2 = x * x;
    let x3 = x2 * x;
    let y = if temp <= 4000.0 {
        -0.9549476 * x3 - 1.3741859 * x2 + 2.0913701 * x - 0.1674886
    } else {
        3.0817580 * x3 - 5.8733867 * x2 + 3.75112997 * x - 0.3700148
    };
    let y = y.max(0.05);
    let big_x = x / y;
    let big_y = 1.0_f32;
    let big_z = (1.0 - x - y) / y;
    // XYZ -> linear sRGB
    let r = (3.2404542 * big_x - 1.5371385 * big_y - 0.4985314 * big_z).max(0.01);
    let g = (-0.9692660 * big_x + 1.8760108 * big_y + 0.0415560 * big_z).max(0.01);
    let b = (0.0556434 * big_x - 0.2040259 * big_y + 1.0572252 * big_z).max(0.01);
    [r, g, b]
}

/// Relative sRGB white-balance multipliers relative to 5600K daylight (`[1.0, 1.0, 1.0]`).
/// Increasing `kelvin` warms the image; `tint` in `-50..50` shifts green(-) to magenta(+).
pub fn wb_multipliers_for_kelvin(kelvin: f32, tint: f32) -> [f32; 3] {
    let ref_spd = planckian_srgb_spd(5600.0);
    let cur_spd = planckian_srgb_spd(kelvin);
    let mut r = ref_spd[0] / cur_spd[0];
    let mut g = ref_spd[1] / cur_spd[1];
    let mut b = ref_spd[2] / cur_spd[2];
    // Normalize so green channel stays at unit luminance before tint
    let norm = g.max(0.01);
    r /= norm;
    g /= norm;
    b /= norm;
    let tint_g = 2.0_f32.powf(-tint.clamp(-50.0, 50.0) / 120.0);
    [r, g * tint_g, b]
}

#[inline]
pub fn srgb_oetf(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Fast CPU preview: box-averages Bayer quads across each `step x step` tile to hit ~target_w,
/// with focus-pixel suppression on Hybrid CMOS AF sensors. Returns (w, h, rgba).
pub fn render_preview(
    frame: &DecodedFrame,
    meta: &MlvMetadata,
    target_w: u32,
) -> (u32, u32, Vec<u8>) {
    let (w, h) = (frame.width as usize, frame.height as usize);
    if w < 2 || h < 2 || frame.data.len() < w * h {
        return (0, 0, Vec::new());
    }
    let step = (((w / target_w.max(1) as usize) / 2) * 2).max(2);
    let ow = (w / step).max(1);
    let oh = (h / step).max(1);
    let quads = (step / 2).max(1);
    let inv_q = 1.0 / (quads * quads) as f32;

    let mut m = camera_to_srgb(meta);
    if meta.white_balance_kelvin >= 2200 && meta.white_balance_kelvin <= 12000 {
        let wb = wb_multipliers_for_kelvin(meta.white_balance_kelvin as f32, 0.0);
        for c in 0..3 {
            for j in 0..3 {
                m[c][j] *= wb[c];
            }
        }
    }

    let (rx, ry) = meta.red_offset();
    let is_pdaf = has_pdaf_sensor(meta.camera_model_id, &meta.camera_name);
    let healed_buf;
    let src_data: &[u16] = if is_pdaf {
        let mut tmp = frame.data[..w * h].to_vec();
        heal_bayer_cpu(&mut tmp, w, h, (rx, ry), frame.black, frame.white, true, None);
        healed_buf = tmp;
        &healed_buf
    } else {
        &frame.data
    };

    let (rx, ry) = (rx as usize, ry as usize);
    let (bx, by) = (rx ^ 1, ry ^ 1);
    let black = frame.black;
    let range = (frame.white - frame.black).max(1.0);

    // 4096-entry gamma LUT
    let lut: Vec<u8> = (0..4096)
        .map(|i| (srgb_oetf(i as f32 / 4095.0) * 255.0 + 0.5) as u8)
        .collect();

    let mut out = vec![0u8; ow * oh * 4];
    let px = |x: usize, y: usize| -> f32 {
        let v = src_data[y * w + x] as f32;
        ((v - black) / range).max(0.0)
    };
    for oy in 0..oh {
        let y0 = oy * step;
        for ox in 0..ow {
            let x0 = ox * step;
            let mut r = 0.0f32;
            let mut g = 0.0f32;
            let mut b = 0.0f32;
            let mut count = 0usize;
            for qy in 0..quads {
                let y = y0 + qy * 2;
                if y + 1 >= h {
                    break;
                }
                for qx in 0..quads {
                    let x = x0 + qx * 2;
                    if x + 1 >= w {
                        break;
                    }
                    r += px(x + rx, y + ry);
                    b += px(x + bx, y + by);
                    g += 0.5 * (px(x + bx, y + ry) + px(x + rx, y + by));
                    count += 1;
                }
            }
            if count == 0 {
                continue;
            }
            let scale = if count == quads * quads { inv_q } else { 1.0 / count as f32 };
            r *= scale;
            g *= scale;
            b *= scale;

            let o = (oy * ow + ox) * 4;
            for c in 0..3 {
                let v = m[c][0] * r + m[c][1] * g + m[c][2] * b;
                out[o + c] = lut[(v.clamp(0.0, 1.0) * 4095.0) as usize];
            }
            out[o + 3] = 255;
        }
    }
    (ow as u32, oh as u32, out)
}
