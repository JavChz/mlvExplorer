use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PickFlag {
    Unflagged = 0,
    Picked = 1,
    Rejected = 2,
}

impl From<i64> for PickFlag {
    fn from(val: i64) -> Self {
        match val {
            1 => PickFlag::Picked,
            2 => PickFlag::Rejected,
            _ => PickFlag::Unflagged,
        }
    }
}

impl From<PickFlag> for i64 {
    fn from(flag: PickFlag) -> Self {
        flag as i64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoRecord {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_size: u64,
    pub width: u32,
    pub height: u32,
    pub fps: f32,
    pub frame_count: u32,
    pub duration_seconds: f32,
    pub bits_per_pixel: u16,
    pub camera_name: String,
    pub camera_serial: String,
    pub lens_name: String,
    pub focal_length_mm: u16,
    pub aperture_fstop: f32,
    pub iso: u32,
    pub shutter_speed: u32,
    pub white_balance_kelvin: u32,
    pub date_recorded: Option<String>,
    pub imported_at: String,
    pub rating: u8, // 0..5
    pub flag: PickFlag,
    pub notes: String,
    pub color_label: String,
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Default)]
pub struct FilterCriteria {
    pub search_text: String,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub min_iso: Option<u32>,
    pub max_iso: Option<u32>,
    pub min_rating: Option<u8>,
    pub flag: Option<PickFlag>,
    pub selected_tag: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogBackup {
    pub exported_at: String,
    pub app_version: String,
    pub videos: Vec<VideoRecord>,
    pub tags: Vec<TagRecord>,
}
