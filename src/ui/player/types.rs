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
    pub zoom_mode: ZoomMode,
    pub pan_offset: Vec2,
    pub is_fullscreen: bool,
    pub show_overlay_controls: bool,
    pub volume: f32,
    pub is_muted: bool,
    pub last_frame_time: Instant,
    pub last_shown_frame: Option<Arc<crate::mlv::DecodedFrame>>,
    pub last_pointer_pos: Option<egui::Pos2>,
    pub last_interaction_time: Instant,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            current_frame: 0,
            is_playing: false,
            is_looping: true,
            playback_speed: 1.0,
            zoom_mode: ZoomMode::Fit,
            pan_offset: Vec2::ZERO,
            is_fullscreen: false,
            show_overlay_controls: true,
            volume: 1.0,
            is_muted: false,
            last_frame_time: Instant::now(),
            last_shown_frame: None,
            last_pointer_pos: None,
            last_interaction_time: Instant::now(),
        }
    }
}
