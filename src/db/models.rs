use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Hash)]
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
    pub rating: u8,
    pub flag: PickFlag,
    pub notes: String,
    pub color_label: String,
    #[serde(default)]
    pub codec: String,
    #[serde(default)]
    pub has_audio: bool,
    #[serde(default)]
    pub chunk_count: u16,
    pub tags: Vec<String>,
}

impl VideoRecord {
    pub fn duration_tc(&self) -> String {
        let t = self.duration_seconds.max(0.0);
        let m = (t / 60.0) as u32;
        let s = (t % 60.0) as u32;
        let f = ((t.fract()) * self.fps.max(1.0)) as u32;
        format!("{:02}:{:02}:{:02}", m, s, f)
    }

    pub fn shutter_str(&self) -> String {
        if self.shutter_speed == 0 {
            return "—".into();
        }
        let s = self.shutter_speed as f64 / 1_000_000.0;
        if s >= 0.5 {
            format!("{:.1}s", s)
        } else {
            format!("1/{}", (1.0 / s).round() as u32)
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagRecord {
    pub id: i64,
    pub name: String,
    pub color: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SortOrder {
    #[default]
    CaptureNewest,
    CaptureOldest,
    FileName,
    DurationLongest,
    RatingHighest,
    ImportNewest,
}

impl SortOrder {
    pub const ALL: [SortOrder; 6] = [
        SortOrder::CaptureNewest,
        SortOrder::CaptureOldest,
        SortOrder::FileName,
        SortOrder::DurationLongest,
        SortOrder::RatingHighest,
        SortOrder::ImportNewest,
    ];
    pub fn label(&self) -> &'static str {
        match self {
            SortOrder::CaptureNewest => "Capture Time (newest)",
            SortOrder::CaptureOldest => "Capture Time (oldest)",
            SortOrder::FileName => "File Name",
            SortOrder::DurationLongest => "Duration",
            SortOrder::RatingHighest => "Rating",
            SortOrder::ImportNewest => "Added Order",
        }
    }
    pub fn sql(&self) -> &'static str {
        match self {
            SortOrder::CaptureNewest => "v.date_recorded DESC, v.file_name ASC",
            SortOrder::CaptureOldest => "v.date_recorded ASC, v.file_name ASC",
            SortOrder::FileName => "v.file_name COLLATE NOCASE ASC",
            SortOrder::DurationLongest => "v.duration_seconds DESC",
            SortOrder::RatingHighest => "v.rating DESC, v.date_recorded DESC",
            SortOrder::ImportNewest => "v.id DESC",
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct FilterCriteria {
    pub search_text: String,
    pub camera: Option<String>,
    pub lens: Option<String>,
    pub min_rating: Option<u8>,
    pub flag: Option<PickFlag>,
    pub tag: Option<String>,
    pub sort: SortOrder,
}

impl FilterCriteria {
    pub fn is_filtered(&self) -> bool {
        !self.search_text.trim().is_empty()
            || self.camera.is_some()
            || self.lens.is_some()
            || self.min_rating.is_some()
            || self.flag.is_some()
            || self.tag.is_some()
    }
}

/// Facet entry for the navigator (name + count)
#[derive(Debug, Clone)]
pub struct Facet {
    pub name: String,
    pub count: usize,
}

#[derive(Debug, Clone, Default)]
pub struct Facets {
    pub total: usize,
    pub picked: usize,
    pub rejected: usize,
    pub unflagged: usize,
    pub cameras: Vec<Facet>,
    pub lenses: Vec<Facet>,
    pub tags: Vec<Facet>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CatalogBackup {
    pub exported_at: String,
    pub app_version: String,
    pub videos: Vec<VideoRecord>,
    pub tags: Vec<TagRecord>,
}
