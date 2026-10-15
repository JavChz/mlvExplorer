pub mod canvas;
pub mod header;
pub mod overlay;
pub mod transport;
pub mod types;

pub use canvas::Canvas;
pub use header::HeaderBar;
pub use overlay::FullscreenOverlay;
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

        let editing_text = ui.ctx().wants_keyboard_input();

        // Mouse motion & interaction tracking for fullscreen HUD auto-fade
        let current_pointer_pos = ui.input(|i| i.pointer.hover_pos());
        let mouse_moved = match (current_pointer_pos, self.last_pointer_pos) {
            (Some(cur), Some(last)) => (cur - last).length_sq() > 4.0,
            (Some(_), None) => true,
            _ => false,
        };
        let any_mouse_down = ui.input(|i| i.pointer.any_down() || i.pointer.any_pressed());
        if mouse_moved || any_mouse_down {
            self.last_pointer_pos = current_pointer_pos;
            self.last_interaction_time = Instant::now();
        }

        // Global hotkeys in Player mode
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::F)) {
            self.is_fullscreen = !self.is_fullscreen;
            if self.is_fullscreen {
                self.zoom_mode = ZoomMode::Fit;
                self.pan_offset = egui::Vec2::ZERO;
            }
            self.last_interaction_time = Instant::now();
            action = Some(PlayerAction::ToggleFullscreen);
        }
        let mouse_back = ui.input(|i| {
            i.pointer.button_pressed(egui::PointerButton::Extra1)
                || i.raw.events.iter().any(|e| matches!(e, egui::Event::PointerButton { button: egui::PointerButton::Extra1, pressed: true, .. }))
        });
        if mouse_back || (!editing_text && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))) {
            action = Some(PlayerAction::BackToGrid);
        } else if self.is_fullscreen && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.is_fullscreen = false;
            self.last_interaction_time = Instant::now();
            action = Some(PlayerAction::ToggleFullscreen);
        } else if !self.is_fullscreen && (ui.input(|i| i.key_pressed(egui::Key::Escape)) || (!editing_text && ui.input(|i| i.key_pressed(egui::Key::G)))) {
            action = Some(PlayerAction::BackToGrid);
        }

        // Play / Pause and Frame Stepping
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::Space)) {
            self.is_playing = !self.is_playing;
            self.last_frame_time = Instant::now();
            if self.is_playing && self.current_frame + 1 >= total_frames {
                self.current_frame = 0;
            }
            let time_sec = self.current_frame as f32 / video.fps.max(1.0);
            if let Ok(mut a) = playback.audio.lock() {
                if self.is_playing {
                    a.play(time_sec);
                } else {
                    a.pause();
                }
            }
        }
        let jump_1s = (video.fps.round() as usize).max(1);
        let shift_down = ui.input(|i| i.modifiers.shift);

        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
            if shift_down {
                self.current_frame = self.current_frame.saturating_sub(jump_1s);
            } else if self.current_frame > 0 {
                self.current_frame -= 1;
            }
            action = Some(PlayerAction::Seek(self.current_frame));
        }
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
            if shift_down {
                self.current_frame = (self.current_frame + jump_1s).min(total_frames.saturating_sub(1));
            } else if self.current_frame + 1 < total_frames {
                self.current_frame += 1;
            }
            action = Some(PlayerAction::Seek(self.current_frame));
        }
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::Home)) {
            self.current_frame = 0;
            action = Some(PlayerAction::Seek(0));
        }
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::End)) {
            self.current_frame = total_frames.saturating_sub(1);
            action = Some(PlayerAction::Seek(self.current_frame));
        }

        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::Z)) {
            if self.zoom_mode == ZoomMode::Fit {
                self.zoom_mode = ZoomMode::Scale(1.0);
            } else {
                self.zoom_mode = ZoomMode::Fit;
                self.pan_offset = egui::Vec2::ZERO;
            }
        }
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
            let cur = match self.zoom_mode {
                ZoomMode::Fit => 1.0,
                ZoomMode::Scale(s) => s,
            };
            self.zoom_mode = ZoomMode::Scale((cur * 1.25).min(8.0));
        }
        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::Minus)) {
            let cur = match self.zoom_mode {
                ZoomMode::Fit => 1.0,
                ZoomMode::Scale(s) => s,
            };
            self.zoom_mode = ZoomMode::Scale((cur * 0.8).max(0.15));
        }

        if !editing_text && ui.input(|i| i.key_pressed(egui::Key::M)) {
            self.is_muted = !self.is_muted;
            if let Ok(mut a) = playback.audio.lock() {
                a.set_muted(self.is_muted);
            }
        }

        // 1. Studio Player Header Bar (Shown in regular windowed player)
        if !self.is_fullscreen {
            if let Some(header_act) = HeaderBar::show(self, ui, video, playback) {
                action = Some(header_act);
            }

            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);
        }

        // Advance frames if currently playing
        let effective_fps = (video.fps * self.playback_speed).max(1.0);
        let frame_duration = std::time::Duration::from_secs_f32(1.0 / effective_fps);

        if self.is_playing {
            let has_audio = playback.audio.lock().map(|a| a.has_audio()).unwrap_or(false);
            if has_audio {
                let mut should_loop = false;
                let mut should_stop = false;

                if let Ok(mut a) = playback.audio.lock() {
                    a.set_speed(self.playback_speed);

                    if a.is_finished() {
                        if self.is_looping {
                            should_loop = true;
                        } else {
                            should_stop = true;
                        }
                    } else if let Some(audio_time) = a.current_playback_time() {
                        let target_frame = (audio_time * video.fps).floor() as usize;
                        if target_frame >= total_frames {
                            if self.is_looping {
                                should_loop = true;
                            } else {
                                should_stop = true;
                            }
                        } else {
                            self.current_frame = target_frame;
                        }
                    }

                    if should_loop {
                        self.current_frame = 0;
                        a.seek(0.0, true);
                    } else if should_stop {
                        self.is_playing = false;
                        a.pause();
                    }
                }
            } else {
                let elapsed = self.last_frame_time.elapsed();
                if elapsed >= frame_duration {
                    self.last_frame_time = Instant::now();
                    if self.current_frame + 1 < total_frames {
                        self.current_frame += 1;
                    } else if self.is_looping {
                        self.current_frame = 0;
                    } else {
                        self.is_playing = false;
                    }
                }
            }
            ui.ctx().request_repaint();
        }

        // 2. Main Viewport Canvas (Cinema Screen)
        Canvas::show(self, ui, video, playback);

        // 3. Bottom Controls:
        if !self.is_fullscreen {
            // In normal windowed mode, show standard bottom Studio Console
            ui.add_space(4.0);
            if let Some(transport_act) = TransportBar::show(self, ui, video, playback, total_frames) {
                action = Some(transport_act);
            }
        } else {
            // In fullscreen mode, calculate smooth fade alpha based on mouse idle time (3.0s idle timeout)
            let idle_secs = self.last_interaction_time.elapsed().as_secs_f32();
            let alpha = if idle_secs < 2.5 {
                1.0
            } else if idle_secs < 3.0 {
                // Smooth 0.5s fade out
                1.0 - (idle_secs - 2.5) / 0.5
            } else {
                0.0
            };

            if alpha > 0.0 {
                ui.ctx().request_repaint();
                if let Some(overlay_act) = FullscreenOverlay::show(self, ui, video, playback, total_frames, alpha) {
                    action = Some(overlay_act);
                }
            }
        }

        action
    }
}
