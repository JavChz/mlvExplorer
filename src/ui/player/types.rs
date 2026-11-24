use crate::db::PickFlag;
use crate::mlv::FocusPixelMode;
use egui::Vec2;
use std::sync::Arc;
use std::time::Instant;

pub enum PlayerAction {
    BackToGrid,
    Seek(usize),
    SetRating(u8),
    SetFlag(PickFlag),
    SetStretch(f32),
    SetRotation(u16),
    SetFocusPixelMode(FocusPixelMode),
    TogglePerfHud(bool),
    SetHighlightRolloff(bool),
    RetryLoad,
    PrevClip,
    NextClip,
    ToggleFullscreen,
    ShowInExplorer,
    OpenInMlvApp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ZoomMode {
    Fit,
    Scale(f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PreviewLook {
    pub exposure_ev: f32,
    pub wb_override: bool,
    pub wb_kelvin: f32,
    pub wb_tint: f32,
    pub highlight_rolloff: bool,
    pub zebras: bool,
    pub focus_peaking: bool,
}

impl Default for PreviewLook {
    fn default() -> Self {
        Self {
            exposure_ev: 0.0,
            wb_override: false,
            wb_kelvin: 5600.0,
            wb_tint: 0.0,
            highlight_rolloff: true,
            zebras: false,
            focus_peaking: false,
        }
    }
}

impl PreviewLook {
    pub fn is_modified(&self) -> bool {
        self.exposure_ev.abs() > 0.01 || self.wb_override || self.zebras || self.focus_peaking
    }
}

pub struct PlayerView {
    pub current_frame: usize,
    pub is_playing: bool,
    pub is_looping: bool,
    pub playback_speed: f32,
    pub reverse_shuttle: bool,
    pub zoom_mode: ZoomMode,
    pub pan_offset: Vec2,
    pub is_fullscreen: bool,
    pub show_overlay_controls: bool,
    pub volume: f32,
    pub is_muted: bool,
    pub last_frame_time: Instant,
    pub playback_anchor_time: Instant,
    pub playback_anchor_frame: f64,
    pub is_buffering: bool,
    pub buffering_since: Option<Instant>,
    pub last_audio_time: Option<(f32, Instant)>,
    pub last_shown_frame: Option<Arc<crate::mlv::DecodedFrame>>,
    pub last_pointer_pos: Option<egui::Pos2>,
    pub last_interaction_time: Instant,
    pub rating_osd: Option<(u8, Instant)>,
    pub status_osd: Option<(String, Instant)>,
    pub focus_pixel_mode: FocusPixelMode,
    pub show_perf_hud: bool,
    pub look: PreviewLook,
    pub loop_in: Option<usize>,
    pub loop_out: Option<usize>,
    pub display_fps_times: Vec<Instant>,
    pub last_counted_frame: Option<usize>,
}

impl PlayerView {
    pub fn trigger_rating_cue(&mut self, rating: u8) {
        self.rating_osd = Some((rating, Instant::now()));
    }

    pub fn trigger_status_cue(&mut self, msg: impl Into<String>) {
        self.status_osd = Some((msg.into(), Instant::now()));
    }

    pub fn reanchor_clock(&mut self) {
        self.playback_anchor_time = Instant::now();
        self.playback_anchor_frame = self.current_frame as f64;
        self.last_frame_time = self.playback_anchor_time;
    }

    pub fn clear_buffering(&mut self) {
        self.is_buffering = false;
        self.buffering_since = None;
        self.last_audio_time = None;
    }

    pub fn active_loop_range(&self, total_frames: usize) -> Option<(usize, usize)> {
        if total_frames == 0 || (self.loop_in.is_none() && self.loop_out.is_none()) {
            return None;
        }
        let max_f = total_frames - 1;
        let a = self.loop_in.unwrap_or(0).min(max_f);
        let b = self.loop_out.unwrap_or(max_f).min(max_f);
        Some((a.min(b), a.max(b)))
    }

    pub fn record_playback_frame(&mut self, frame: usize) -> f32 {
        let now = Instant::now();
        if self.is_playing && self.last_counted_frame != Some(frame) {
            self.last_counted_frame = Some(frame);
            self.display_fps_times.push(now);
        }
        self.display_fps_times
            .retain(|t| now.duration_since(*t).as_secs_f32() < 1.0);
        self.display_fps_times.len() as f32
    }
}

impl Default for PlayerView {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            current_frame: 0,
            is_playing: false,
            is_looping: true,
            playback_speed: 1.0,
            reverse_shuttle: false,
            zoom_mode: ZoomMode::Fit,
            pan_offset: Vec2::ZERO,
            is_fullscreen: false,
            show_overlay_controls: true,
            volume: 1.0,
            is_muted: false,
            last_frame_time: now,
            playback_anchor_time: now,
            playback_anchor_frame: 0.0,
            is_buffering: false,
            buffering_since: None,
            last_audio_time: None,
            last_shown_frame: None,
            last_pointer_pos: None,
            last_interaction_time: now,
            rating_osd: None,
            status_osd: None,
            focus_pixel_mode: FocusPixelMode::Auto,
            show_perf_hud: false,
            look: PreviewLook::default(),
            loop_in: None,
            loop_out: None,
            display_fps_times: Vec::new(),
            last_counted_frame: None,
        }
    }
}
