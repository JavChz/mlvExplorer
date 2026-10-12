use serde::{Deserialize, Serialize};

/// Video class constants (MLVI.videoClass)
pub const VIDEO_CLASS_RAW: u16 = 0x01;
pub const VIDEO_CLASS_FLAG_LJ92: u16 = 0x20;

/// Parsed, display-ready metadata for a clip (all spanned chunks combined).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MlvMetadata {
    pub file_path: String,
    /// Total size of all chunks (.MLV + .M00, .M01 ...)
    pub file_size: u64,
    pub chunk_count: u16,
    pub video_guid: u64,
    pub video_class: u16,
    pub has_audio: bool,

    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub frame_count: u32,
    pub bits_per_pixel: u16,
    pub black_level: u32,
    pub white_level: u32,
    /// DNG-style CFA pattern (e.g. 0x02010100 = RGGB)
    pub cfa_pattern: u32,
    /// DNG ColorMatrix1 as 9 rationals (num, den)
    pub color_matrix: Vec<i32>,

    // Camera & lens
    pub camera_name: String,
    pub camera_serial: String,
    pub lens_name: String,
    pub focal_length_mm: u16,
    pub aperture_fstop: f32,
    pub iso: u32,
    pub shutter_speed_microseconds: u32,
    pub white_balance_kelvin: u32,
    pub date_recorded: Option<String>,
    pub binning_x: u16,
    pub binning_y: u16,
    pub pixel_aspect_ratio: f32,
    /// Indicates whether clip utilizes 5D3 card spanning (multiple chunk files written in parallel)
    pub is_dual_card: bool,
}

impl MlvMetadata {
    pub fn fps(&self) -> f32 {
        if self.fps_den == 0 || self.fps_num == 0 {
            24.0
        } else {
            self.fps_num as f32 / self.fps_den as f32
        }
    }

    pub fn duration_seconds(&self) -> f32 {
        self.frame_count as f32 / self.fps().max(0.001)
    }

    pub fn is_lj92(&self) -> bool {
        self.video_class & VIDEO_CLASS_FLAG_LJ92 != 0
    }

    pub fn codec_name(&self) -> String {
        if self.is_lj92() {
            format!("RAW {}-bit Lossless (LJ92)", self.bits_per_pixel)
        } else {
            format!("RAW {}-bit Uncompressed", self.bits_per_pixel)
        }
    }

    /// Auto-detect aspect ratio / stretch factor for Magic Lantern anamorphic & binning modes.
    /// In all 1x3 / 3x1 crop mood anamorphic modes, raw pixels are squeezed and require 3.0x horizontal stretch (decompression).
    pub fn default_stretch(&self) -> f32 {
        if self.pixel_aspect_ratio > 0.05 && (self.pixel_aspect_ratio - 1.0).abs() > 0.01 {
            // Some MLV tools / encoders write PAR as H:W (e.g. 1/3 or 1/2) rather than W:H.
            // In our viewer, stretch_factor is a horizontal expansion multiplier (>= 1.0).
            let par = if self.pixel_aspect_ratio < 0.85 {
                1.0 / self.pixel_aspect_ratio
            } else {
                self.pixel_aspect_ratio
            };
            if (par - 3.0).abs() < 0.25 {
                return 3.0;
            } else if (par - 2.0).abs() < 0.2 {
                return 2.0;
            } else if (par - 1.75).abs() < 0.15 {
                return 1.75;
            } else if (par - 1.5).abs() < 0.15 {
                return 1.5;
            } else if (par - 1.333).abs() < 0.15 {
                return 1.333;
            }
            return par;
        }

        // Check if RAWC indicated asymmetric sensor binning (e.g. 1x3 or 3x1)
        if self.binning_x > 0 && self.binning_y > 0 && self.binning_x != self.binning_y {
            let max_bin = self.binning_x.max(self.binning_y) as f32;
            let min_bin = self.binning_x.min(self.binning_y) as f32;
            let ratio = max_bin / min_bin.max(1.0);
            if (ratio - 3.0).abs() < 0.25 {
                return 3.0; // 1x3 or 3x1 mode: always decompress with 3.0x horizontal stretch
            } else if (ratio - 2.0).abs() < 0.2 {
                return 2.0;
            } else if (ratio - 1.5).abs() < 0.15 {
                return 1.5;
            }
            return ratio;
        }

        // Geometric heuristics based on captured raw sensor dimensions:
        // In Magic Lantern 1x3 / Crop Mood anamorphic modes, sensor readout squashes raw video horizontally
        // (e.g. 1736x2214, 1736x2178, 1736x1160, 1536x2160, 1280x2160, 720x1280).
        // Stretched horizontally by 3.0x to restore widescreen aspect ratio!
        if self.width > 0 && self.height > 0 {
            let raw_ar = self.width as f32 / self.height as f32;
            if raw_ar >= 0.35 && raw_ar <= 1.25 {
                return 3.0;
            }
        }

        1.0
    }

    /// Position of the red pixel inside the 2x2 Bayer quad: (rx, ry)
    pub fn red_offset(&self) -> (u32, u32) {
        let p = self.cfa_pattern.to_le_bytes();
        // DNG CFA codes: 0 = R, 1 = G, 2 = B
        if self.cfa_pattern == 0 {
            return (0, 0);
        }
        match p.iter().position(|&c| c == 0) {
            Some(0) => (0, 0),
            Some(1) => (1, 0),
            Some(2) => (0, 1),
            Some(3) => (1, 1),
            _ => (0, 0),
        }
    }
}

/// Location of a single video frame payload on disk
#[derive(Debug, Clone, Copy)]
pub struct FrameRef {
    pub frame_number: u32,
    pub chunk: u16,
    pub offset: u64,
    pub size: u32,
}
