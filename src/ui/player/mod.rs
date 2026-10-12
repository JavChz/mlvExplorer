pub mod canvas;
pub mod header;
pub mod transport;
pub mod types;

pub use canvas::Canvas;
pub use header::HeaderBar;
pub use transport::TransportBar;
pub use types::{PlayerAction, PlayerView, ZoomMode};

use crate::db::VideoRecord;
use crate::playback::Playback;
use egui::Ui;
use std::time::Instant;

impl PlayerView {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: &Playback,
    ) -> Option<PlayerAction> {
        let mut action = None;
        let total_frames = playback.frame_count().max(1);

        // Notify playback engine of target frame for read-ahead
        playback.set_target(self.current_frame, self.is_looping);

        // Global hotkeys in Player mode
        if ui.input(|i| i.key_pressed(egui::Key::F)) {
            self.is_fullscreen = !self.is_fullscreen;
            action = Some(PlayerAction::ToggleFullscreen);
        }
        if self.is_fullscreen && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.is_fullscreen = false;
            action = Some(PlayerAction::ToggleFullscreen);
        }
        if ui.input(|i| i.key_pressed(egui::Key::Z)) {
            if self.zoom_mode == ZoomMode::Fit {
                self.zoom_mode = ZoomMode::Scale(1.0);
            } else {
                self.zoom_mode = ZoomMode::Fit;
                self.pan_offset = egui::Vec2::ZERO;
            }
        }
        if ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
            let cur = match self.zoom_mode {
                ZoomMode::Fit => 1.0,
                ZoomMode::Scale(s) => s,
            };
            self.zoom_mode = ZoomMode::Scale((cur * 1.25).min(8.0));
        }
        if ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            let cur = match self.zoom_mode {
                ZoomMode::Fit => 1.0,
                ZoomMode::Scale(s) => s,
            };
            self.zoom_mode = ZoomMode::Scale((cur * 0.8).max(0.15));
        }

        // 1. Studio Player Header Bar
        if let Some(header_act) = HeaderBar::show(self, ui, video, playback) {
            action = Some(header_act);
        }

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // Advance frames if currently playing
        let effective_fps = (video.fps * self.playback_speed).max(1.0);
        let frame_duration = std::time::Duration::from_secs_f32(1.0 / effective_fps);

        if self.is_playing {
            let next_target = if self.current_frame + 1 < total_frames {
                self.current_frame + 1
            } else if self.is_looping {
                0
            } else {
                self.current_frame
            };

            let elapsed = self.last_frame_time.elapsed();
            if elapsed >= frame_duration {
                let is_ready = playback.is_ready(next_target);
                if is_ready || elapsed >= frame_duration + std::time::Duration::from_millis(15) {
                    self.last_frame_time = Instant::now();
                    if self.current_frame + 1 < total_frames {
                        self.current_frame += 1;
                    } else if self.is_looping {
                        self.current_frame = 0;
                    } else {
                        self.is_playing = false;
                    }
                    action = Some(PlayerAction::Seek(self.current_frame));
                }
            }
            ui.ctx().request_repaint();
        }

        // 2. Main Viewport Canvas (Cinema Screen)
        Canvas::show(self, ui, video, playback);

        ui.add_space(4.0);

        // 3. Bottom Studio Console: Full Playbar + Transport Controls
        if let Some(transport_act) = TransportBar::show(self, ui, video, playback, total_frames) {
            action = Some(transport_act);
        }

        action
    }
}
