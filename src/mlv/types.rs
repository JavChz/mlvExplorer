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
