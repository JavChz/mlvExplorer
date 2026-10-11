use serde::{Deserialize, Serialize};

/// 4-character block type identifier in MLV
pub type BlockType = [u8; 4];

pub const BLOCK_MLVI: BlockType = *b"MLVI";
pub const BLOCK_RAWI: BlockType = *b"RAWI";
pub const BLOCK_VIDF: BlockType = *b"VIDF";
pub const BLOCK_AUDF: BlockType = *b"AUDF";
pub const BLOCK_LENS: BlockType = *b"LENS";
pub const BLOCK_EXPO: BlockType = *b"EXPO";
pub const BLOCK_RTCI: BlockType = *b"RTCI";
pub const BLOCK_IDNT: BlockType = *b"IDNT";
pub const BLOCK_WBAL: BlockType = *b"WBAL";
pub const BLOCK_DISP: BlockType = *b"DISP";
pub const BLOCK_INFO: BlockType = *b"INFO";

/// Common MLV Block Header (16 bytes)
#[derive(Debug, Clone, Copy)]
pub struct MlvBlockHeader {
    pub block_type: BlockType,
    pub block_size: u32,
    pub timestamp: u64,
}

/// Parsed metadata for a video file
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MlvMetadata {
    pub file_path: String,
    pub file_size: u64,
    pub video_guid: u64,
    pub file_num: u16,
    pub file_count: u16,
    pub width: u32,
    pub height: u32,
    pub raw_width: u32,
    pub raw_height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub frame_count: u32,
    pub bits_per_pixel: u16,
    pub black_level: u32,
    pub white_level: u32,
    pub cfa_pattern: u32, // 0 = RGGB, 1 = GRBG, 2 = GBRG, 3 = BGGR
    
    // Camera & Lens info
    pub camera_name: String,
    pub camera_serial: String,
    pub lens_name: String,
    pub focal_length_mm: u16,
    pub aperture_fstop: f32, // e.g. 2.8
    pub iso: u32,
    pub shutter_speed_microseconds: u32,
    pub white_balance_kelvin: u32,
    pub date_recorded: Option<String>,
}

impl MlvMetadata {
    pub fn fps(&self) -> f32 {
        if self.fps_den == 0 {
            24.0
        } else {
            self.fps_num as f32 / self.fps_den as f32
        }
    }

    pub fn duration_seconds(&self) -> f32 {
        let fps = self.fps();
        if fps <= 0.0 {
            0.0
        } else {
            self.frame_count as f32 / fps
        }
    }

    pub fn duration_formatted(&self) -> String {
        let total_secs = self.duration_seconds();
        let mins = (total_secs / 60.0) as u32;
        let secs = (total_secs % 60.0) as u32;
        let ms = ((total_secs.fract()) * 100.0) as u32;
        format!("{:02}:{:02}.{:02}", mins, secs, ms)
    }

    pub fn resolution_formatted(&self) -> String {
        format!("{}x{}", self.width, self.height)
    }

    pub fn cfa_name(&self) -> &'static str {
        match self.cfa_pattern {
            0 => "RGGB",
            1 => "GRBG",
            2 => "GBRG",
            3 => "BGGR",
            _ => "Unknown Bayer",
        }
    }
}

/// Information about a single video frame offset in the MLV file
#[derive(Debug, Clone, Copy)]
pub struct FrameIndexEntry {
    pub frame_number: u32,
    pub file_offset: u64,
    pub payload_size: u32,
    pub timestamp: u64,
}
