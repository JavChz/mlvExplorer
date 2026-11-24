//! Focus pixel (Canon Hybrid CMOS AF PDAF dots) & bad-pixel healing.
//! Supports MLVApp-compatible `.fpm` maps plus automatic PDAF/hot-pixel healing.

use super::types::MlvMetadata;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum FocusPixelMode {
    #[default]
    Auto,
    FpmOnly,
    Off,
}

impl FocusPixelMode {
    pub fn label(self) -> &'static str {
        match self {
            FocusPixelMode::Auto => "Auto",
            FocusPixelMode::FpmOnly => "FPM Only",
            FocusPixelMode::Off => "Off",
        }
    }

    pub fn short_label(self) -> &'static str {
        match self {
            FocusPixelMode::Auto => "FP: Auto",
            FocusPixelMode::FpmOnly => "FP: FPM",
            FocusPixelMode::Off => "FP: Off",
        }
    }

    pub fn next(self) -> Self {
        match self {
            FocusPixelMode::Auto => FocusPixelMode::FpmOnly,
            FocusPixelMode::FpmOnly => FocusPixelMode::Off,
            FocusPixelMode::Off => FocusPixelMode::Auto,
        }
    }

    pub fn as_u32(self) -> u32 {
        match self {
            FocusPixelMode::Off => 0,
            FocusPixelMode::FpmOnly => 1,
            FocusPixelMode::Auto => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FocusPixelMap {
    pub width: u32,
    pub height: u32,
    /// 1 = focus pixel, 0 = normal pixel (`width * height` bytes)
    pub mask: Vec<u8>,
    pub pixel_count: usize,
    pub source_name: String,
}

/// Returns true if the camera has embedded Hybrid CMOS AF PDAF focus pixels on the sensor.
pub fn has_pdaf_sensor(camera_model_id: u32, camera_name: &str) -> bool {
    // Known Canon DIGIC 5 / Hybrid CMOS AF Canon model IDs in MLV IDNT blocks
    if matches!(
        camera_model_id,
        0x80000331 // EOS M
            | 0x80000301 // EOS 650D / Rebel T4i / Kiss X6i
            | 0x80000326 // EOS 700D / Rebel T5i / Kiss X7i
            | 0x80000346 // EOS 100D / Rebel SL1 / Kiss X7
            | 0x80000355 // EOS M2
            | 0x80000325 // EOS 70D
    ) {
        return true;
    }

    let lower = camera_name.to_ascii_lowercase();
    [
        "eos m",
        "eos-m",
        "650d",
        "700d",
        "100d",
        "t4i",
        "t5i",
        "sl1",
        "x6i",
        "x7i",
        "kiss x7",
        "70d",
    ]
    .iter()
    .any(|needle| lower.contains(needle))
}

/// Directory storing user/downloaded `.fpm` focus pixel maps: `%APPDATA%/MLV Explorer/pixel_maps`.
pub fn pixel_maps_dir() -> PathBuf {
    let dir = crate::config::app_data_dir().join("pixel_maps");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

impl FocusPixelMap {
    /// Parse an `.fpm` text file into a dense `R8Uint` mask (`width * height`).
    pub fn parse_fpm(
        content: &str,
        width: u32,
        height: u32,
        offset_x: i32,
        offset_y: i32,
        source_name: String,
    ) -> Option<Self> {
        if width == 0 || height == 0 {
            return None;
        }
        let w = width as usize;
        let h = height as usize;
        let mut mask = vec![0u8; w.checked_mul(h)?];
        let mut pixel_count = 0usize;

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with("//") {
                continue;
            }
            let mut parts = line.split(|c: char| c.is_whitespace() || c == ',' || c == ';')
                .filter(|s| !s.is_empty());
            let Some(sx) = parts.next() else { continue; };
            let Some(sy) = parts.next() else { continue; };
            let Ok(x) = sx.parse::<i32>() else { continue; };
            let Ok(y) = sy.parse::<i32>() else { continue; };

            let px = x - offset_x;
            let py = y - offset_y;
            if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                let idx = (py as usize) * w + (px as usize);
                if mask[idx] == 0 {
                    mask[idx] = 1;
                    pixel_count += 1;
                }
            }
        }

        if pixel_count == 0 {
            None
        } else {
            Some(Self {
                width,
                height,
                mask,
                pixel_count,
                source_name,
            })
        }
    }

    /// Search `%APPDATA%/MLV Explorer/pixel_maps`, MLV App's `pixel_maps` folder,
    /// and the clip's own folder for a matching `.fpm` map.
    pub fn load_for_clip(meta: &MlvMetadata, mlv_app_path: Option<&Path>) -> Option<Self> {
        if meta.camera_model_id == 0 || meta.width == 0 || meta.height == 0 {
            return None;
        }

        let mut dirs = vec![pixel_maps_dir()];
        if let Some(app_exe) = mlv_app_path {
            if let Some(parent) = app_exe.parent() {
                dirs.push(parent.join("pixel_maps"));
                dirs.push(parent.to_path_buf());
                if let Some(grand) = parent.parent() {
                    dirs.push(grand.join("Resources").join("pixel_maps"));
                }
            }
        }
        if !meta.file_path.is_empty() {
            if let Some(clip_dir) = Path::new(&meta.file_path).parent() {
                dirs.push(clip_dir.join("pixel_maps"));
                dirs.push(clip_dir.to_path_buf());
            }
        }

        let id = meta.camera_model_id;
        let frame_candidates = [
            format!("{id:08X}_{}x{}.fpm", meta.width, meta.height),
            format!("{id:08x}_{}x{}.fpm", meta.width, meta.height),
            format!("{id:x}_{}x{}.fpm", meta.width, meta.height),
        ];

        for dir in &dirs {
            if !dir.is_dir() {
                continue;
            }
            for fname in &frame_candidates {
                let p = dir.join(fname);
                if p.is_file() {
                    if let Ok(text) = std::fs::read_to_string(&p) {
                        if let Some(map) =
                            Self::parse_fpm(&text, meta.width, meta.height, 0, 0, fname.clone())
                        {
                            return Some(map);
                        }
                    }
                }
            }
        }

        if meta.sensor_res_x > 0
            && meta.sensor_res_y > 0
            && (meta.sensor_res_x as u32 != meta.width || meta.sensor_res_y as u32 != meta.height)
        {
            let sensor_candidates = [
                format!("{id:08X}_{}x{}.fpm", meta.sensor_res_x, meta.sensor_res_y),
                format!("{id:08x}_{}x{}.fpm", meta.sensor_res_x, meta.sensor_res_y),
                format!("{id:x}_{}x{}.fpm", meta.sensor_res_x, meta.sensor_res_y),
            ];
            let ox = meta.crop_x as i32;
            let oy = meta.crop_y as i32;
            for dir in &dirs {
                if !dir.is_dir() {
                    continue;
                }
                for fname in &sensor_candidates {
                    let p = dir.join(fname);
                    if p.is_file() {
                        if let Ok(text) = std::fs::read_to_string(&p) {
                            if let Some(map) = Self::parse_fpm(
                                &text,
                                meta.width,
                                meta.height,
                                ox,
                                oy,
                                fname.clone(),
                            ) {
                                return Some(map);
                            }
                        }
                    }
                }
            }
        }

        None
    }
}

#[inline]
fn clamp_cfa(coord: isize, limit: usize) -> usize {
    if coord < 0 {
        let mut c = coord;
        while c < 0 {
            c += 2;
        }
        c.min(limit as isize - 1) as usize
    } else if coord >= limit as isize {
        let mut c = coord;
        while c >= limit as isize {
            c -= 2;
        }
        c.max(0) as usize
    } else {
        coord as usize
    }
}

#[inline]
fn sort4(mut a: f32, mut b: f32, mut c: f32, mut d: f32) -> (f32, f32, f32, f32) {
    if a > b { std::mem::swap(&mut a, &mut b); }
    if c > d { std::mem::swap(&mut c, &mut d); }
    if a > c { std::mem::swap(&mut a, &mut c); }
    if b > d { std::mem::swap(&mut b, &mut d); }
    if b > c { std::mem::swap(&mut b, &mut c); }
    (a, b, c, d)
}

/// Fast CPU Bayer focus-pixel / bad-pixel healer used for thumbnails.
pub fn heal_bayer_cpu(
    data: &mut [u16],
    width: usize,
    height: usize,
    red_offset: (u32, u32),
    black: f32,
    white: f32,
    is_pdaf_camera: bool,
    fpm: Option<&FocusPixelMap>,
) {
    if width < 6 || height < 6 || data.len() < width * height {
        return;
    }
    if !is_pdaf_camera && fpm.is_none() {
        return;
    }

    let orig = data.to_vec();
    let range = (white - black).max(1.0);
    let rx = red_offset.0 as usize & 1;
    let ry = red_offset.1 as usize & 1;

    let sample_norm = |x: isize, y: isize| -> f32 {
        let cx = clamp_cfa(x, width);
        let cy = clamp_cfa(y, height);
        ((orig[cy * width + cx] as f32 - black) / range).max(0.0)
    };

    let sample_raw = |x: isize, y: isize| -> f32 {
        let cx = clamp_cfa(x, width);
        let cy = clamp_cfa(y, height);
        orig[cy * width + cx] as f32
    };

    let is_marked = |x: isize, y: isize| -> bool {
        if let Some(map) = fpm {
            if x >= 0 && y >= 0 && (x as usize) < width && (y as usize) < height {
                return map.mask[y as usize * width + x as usize] != 0;
            }
        }
        false
    };

    for y in 0..height {
        let yi = y as isize;
        let is_g_row_parity = (y & 1) ^ ry;
        for x in 0..width {
            let xi = x as isize;
            let idx = y * width + x;
            let marked = is_marked(xi, yi);
            if fpm.is_some() && !marked && !is_pdaf_camera {
                continue;
            }

            let is_g = ((x & 1) ^ rx) != is_g_row_parity;
            let (n0, n1, n2, n3) = if is_g {
                (
                    sample_norm(xi - 1, yi - 1),
                    sample_norm(xi + 1, yi + 1),
                    sample_norm(xi + 1, yi - 1),
                    sample_norm(xi - 1, yi + 1),
                )
            } else {
                (
                    sample_norm(xi - 2, yi),
                    sample_norm(xi + 2, yi),
                    sample_norm(xi, yi - 2),
                    sample_norm(xi, yi + 2),
                )
            };

            let mut should_heal = marked;
            if !should_heal && is_pdaf_camera {
                let c = sample_norm(xi, yi);
                let (s0, s1, s2, s3) = sort4(n0, n1, n2, n3);
                let med4 = 0.5 * (s1 + s2);
                let spread = s3 - s0;
                let is_dark = c < s0 && (med4 - c) > (0.04 * med4 + 0.004).max(1.65 * spread);
                if is_dark {
                    // Cross-channel guard: ensure immediate opposite-color neighbors aren't all dark too
                    let o0 = sample_norm(xi - 1, yi);
                    let o1 = sample_norm(xi + 1, yi);
                    let o2 = sample_norm(xi, yi - 1);
                    let o3 = sample_norm(xi, yi + 1);
                    let opp_max = o0.max(o1).max(o2).max(o3);
                    if opp_max > 0.25 * med4 || c < 0.72 * s0 {
                        should_heal = true;
                    }
                } else {
                    let is_hot = c > s3 && (c - med4) > (0.20 * med4 + 0.025).max(3.0 * spread);
                    if is_hot {
                        should_heal = true;
                    }
                }
            }

            if should_heal {
                let (r0, r1, r2, r3, m0, m1, m2, m3) = if is_g {
                    (
                        sample_raw(xi - 1, yi - 1),
                        sample_raw(xi + 1, yi + 1),
                        sample_raw(xi + 1, yi - 1),
                        sample_raw(xi - 1, yi + 1),
                        is_marked(xi - 1, yi - 1),
                        is_marked(xi + 1, yi + 1),
                        is_marked(xi + 1, yi - 1),
                        is_marked(xi - 1, yi + 1),
                    )
                } else {
                    (
                        sample_raw(xi - 2, yi),
                        sample_raw(xi + 2, yi),
                        sample_raw(xi, yi - 2),
                        sample_raw(xi, yi + 2),
                        is_marked(xi - 2, yi),
                        is_marked(xi + 2, yi),
                        is_marked(xi, yi - 2),
                        is_marked(xi, yi + 2),
                    )
                };

                let healed = if !m0 && !m1 && !m2 && !m3 {
                    if (r0 - r1).abs() <= (r2 - r3).abs() {
                        0.5 * (r0 + r1)
                    } else {
                        0.5 * (r2 + r3)
                    }
                } else if !m0 && !m1 {
                    0.5 * (r0 + r1)
                } else if !m2 && !m3 {
                    0.5 * (r2 + r3)
                } else {
                    let mut sum = 0.0;
                    let mut cnt = 0.0;
                    for (v, bad) in [(r0, m0), (r1, m1), (r2, m2), (r3, m3)] {
                        if !bad {
                            sum += v;
                            cnt += 1.0;
                        }
                    }
                    if cnt > 0.0 { sum / cnt } else { 0.25 * (r0 + r1 + r2 + r3) }
                };

                data[idx] = healed.round().clamp(0.0, 65535.0) as u16;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pdaf_camera_detection() {
        assert!(has_pdaf_sensor(0x80000331, "Canon EOS M"));
        assert!(has_pdaf_sensor(0x80000301, "Canon EOS 650D"));
        assert!(has_pdaf_sensor(0x80000326, "Canon EOS Rebel T5i"));
        assert!(has_pdaf_sensor(0x80000346, "Canon EOS 100D"));
        assert!(has_pdaf_sensor(0, "Canon EOS M"));
        assert!(!has_pdaf_sensor(0x80000285, "Canon EOS 5D Mark III"));
    }

    #[test]
    fn test_parse_fpm_comments_and_offsets() {
        let content = "\
# Canon EOS M focus pixel map
// another comment
10 20
11,21
9999 9999
5 5
";
        let map = FocusPixelMap::parse_fpm(content, 100, 100, 2, 4, "test.fpm".into()).unwrap();
        assert_eq!(map.pixel_count, 3);
        assert_eq!(map.mask[16 * 100 + 8], 1);
        assert_eq!(map.mask[17 * 100 + 9], 1);
        assert_eq!(map.mask[1 * 100 + 3], 1);
    }

    #[test]
    fn test_heal_bayer_cpu_heals_dark_pdaf_preserves_edge() {
        let w = 16;
        let h = 16;
        let mut frame = vec![8000u16; w * h];
        // Create a sharp vertical high-contrast step edge on the right half (x >= 10)
        for y in 0..h {
            for x in 10..w {
                frame[y * w + x] = 14000;
            }
        }
        // Inject an isolated dark PDAF pixel on the left flat region at (4, 4) (R site)
        frame[4 * w + 4] = 2800;

        heal_bayer_cpu(&mut frame, w, h, (0, 0), 2048.0, 15000.0, true, None);

        // Dark PDAF pixel should be healed cleanly close to 8000
        assert!((frame[4 * w + 4] as i32 - 8000).abs() < 200);
        // Step edge pixels should remain untouched
        assert_eq!(frame[4 * w + 9], 8000);
        assert_eq!(frame[4 * w + 10], 14000);
    }
}
