use crate::db::PickFlag;
use egui::Vec2;
use std::sync::Arc;
use std::time::Instant;

pub enum PlayerAction {
    BackToGrid,
    Seek(usize),
    SetRating(u8),
    SetFlag(PickFlag),
    SetStretch(f32),
    ToggleFullscreen,
    OpenInMlvApp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomMode {
    Fit,
    Scale(f32),
}

pub struct PlayerView {
    pub current_frame: usize,
    pub is_playing: bool,
    pub is_looping: bool,
    pub playback_speed: f32,
    pub exposure_ev: f32,       // -2.5 to +2.5 EV
    pub wb_temperature: f32,    // -50.0 (Cool) to +50.0 (Warm)
    pub wb_tint: f32,           // -50.0 (Green) to +50.0 (Magenta)
    pub zoom_mode: ZoomMode,
    pub pan_offset: Vec2,
    pub is_fullscreen: bool,
    pub last_frame_time: Instant,
    pub last_shown_frame: Option<Arc<crate::mlv::DecodedFrame>>,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            current_frame: 0,
            is_playing: false,
            is_looping: true,
            playback_speed: 1.0,
            exposure_ev: 0.0,
            wb_temperature: 0.0,
            wb_tint: 0.0,
            zoom_mode: ZoomMode::Fit,
            pan_offset: Vec2::ZERO,
            is_fullscreen: false,
            last_frame_time: Instant::now(),
            last_shown_frame: None,
        }
    }
}
