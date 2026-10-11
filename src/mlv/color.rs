//! Neutral "view" transform: camera RGB -> linear sRGB (daylight WB baked in).
//! This is not grading — just a faithful preview, dcraw-style.

use super::decode::DecodedFrame;
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
pub fn srgb_oetf(x: f32) -> f32 {
    let x = x.clamp(0.0, 1.0);
    if x <= 0.0031308 {
        x * 12.92
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

/// Fast CPU preview: picks Bayer quads ("superpixels") on a stride to hit ~target_w.
/// Returns (w, h, rgba).
pub fn render_preview(
    frame: &DecodedFrame,
    meta: &MlvMetadata,
    target_w: u32,
) -> (u32, u32, Vec<u8>) {
    let (w, h) = (frame.width as usize, frame.height as usize);
    let step = (((w / target_w.max(1) as usize) / 2) * 2).max(2);
    let ow = w / step;
    let oh = h / step;
    let m = camera_to_srgb(meta);
    let (rx, ry) = meta.red_offset();
    let (rx, ry) = (rx as usize, ry as usize);
    let (bx, by) = (rx ^ 1, ry ^ 1);
    let black = frame.black;
    let range = (frame.white - frame.black).max(1.0);

    // 256-entry gamma LUT
    let lut: Vec<u8> = (0..4096)
        .map(|i| (srgb_oetf(i as f32 / 4095.0) * 255.0 + 0.5) as u8)
        .collect();

    let mut out = vec![0u8; ow * oh * 4];
    let px = |x: usize, y: usize| -> f32 {
        let v = frame.data.get(y * w + x).copied().unwrap_or(0) as f32;
        ((v - black) / range).max(0.0)
    };
    for oy in 0..oh {
        let y0 = oy * step;
        for ox in 0..ow {
            let x0 = ox * step;
            if x0 + 1 >= w || y0 + 1 >= h {
                continue;
            }
            let r = px(x0 + rx, y0 + ry);
            let b = px(x0 + bx, y0 + by);
            let g = 0.5 * (px(x0 + bx, y0 + ry) + px(x0 + rx, y0 + by));
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
