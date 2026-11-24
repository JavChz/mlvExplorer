pub mod canvas;
pub mod header;
pub mod overlay;
pub mod transport;
pub mod types;

pub use canvas::Canvas;
pub use header::HeaderBar;
pub use overlay::FullscreenOverlay;
pub use transport::TransportBar;
pub use types::{PlayerAction, PlayerView, PreviewLook, ZoomMode};

use crate::db::VideoRecord;
use crate::playback::Playback;
use crate::ui::widgets;
use egui::Ui;
use std::time::{Duration, Instant};

impl PlayerView {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: Option<&Playback>,
        load_error: Option<&str>,
        clip_pos: Option<(usize, usize)>,
        gpu_info: &str,
        thumb_id: Option<egui::TextureId>,
    ) -> Option<PlayerAction> {
        let mut action = None;
        let total_frames = playback
            .map(|p| p.frame_count())
            .unwrap_or(video.frame_count as usize)
            .max(1);
        if self.current_frame >= total_frames {
            self.current_frame = total_frames - 1;
        }
        let loop_range = self.active_loop_range(total_frames);

        // Notify playback engine of target frame + loop range + direction and poll async audio upload
        if let Some(pb) = playback {
            pb.set_loop_range(loop_range);
            pb.set_reverse(self.reverse_shuttle);
            let newly_loaded = pb.update_audio();
            if newly_loaded && self.is_playing && !self.reverse_shuttle && !self.is_buffering {
                let time_sec = self.current_frame as f32 / video.fps.max(1.0);
                if let Ok(mut a) = pb.audio.lock() {
                    a.play(time_sec);
                }
            }
            pb.set_target(self.current_frame, self.is_looping);
        }

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
                || i.raw.events.iter().any(|e| {
                    matches!(
                        e,
                        egui::Event::PointerButton {
                            button: egui::PointerButton::Extra1,
                            pressed: true,
                            ..
                        }
                    )
                })
        });
        if mouse_back || (!editing_text && ui.input(|i| i.modifiers.alt && i.key_pressed(egui::Key::ArrowLeft))) {
            action = Some(PlayerAction::BackToGrid);
        } else if self.is_fullscreen && ui.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.is_fullscreen = false;
            self.last_interaction_time = Instant::now();
            action = Some(PlayerAction::ToggleFullscreen);
        } else if !self.is_fullscreen
            && (ui.input(|i| i.key_pressed(egui::Key::Escape))
                || (!editing_text && ui.input(|i| i.key_pressed(egui::Key::G))))
        {
            action = Some(PlayerAction::BackToGrid);
        }

        if !editing_text {
            let mods = ui.input(|i| i.modifiers);
            let no_cmd = !mods.ctrl && !mods.command && !mods.alt;

            // Focus Pixel Mode cycle [H]
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::H)) {
                let next = self.focus_pixel_mode.next();
                self.focus_pixel_mode = next;
                self.trigger_status_cue(format!("Focus Pixels: {}", next.label()));
                action = Some(PlayerAction::SetFocusPixelMode(next));
            }

            // Rotate 90° CW [R]
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::R)) {
                let next_rot = (video.rotation + 90) % 360;
                self.trigger_status_cue(format!("Rotation: {}°", next_rot));
                action = Some(PlayerAction::SetRotation(next_rot));
            }

            // Toggle Performance HUD [Shift+D]
            if no_cmd && mods.shift && ui.input(|i| i.key_pressed(egui::Key::D)) {
                self.show_perf_hud = !self.show_perf_hud;
                self.trigger_status_cue(if self.show_perf_hud {
                    "Performance HUD: On"
                } else {
                    "Performance HUD: Off"
                });
                action = Some(PlayerAction::TogglePerfHud(self.show_perf_hud));
            }

            // In / Out Loop Range Markers [I / O / Alt+X]
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::I)) {
                self.loop_in = Some(self.current_frame);
                if let Some(out_f) = self.loop_out {
                    if out_f < self.current_frame {
                        self.loop_out = Some(self.current_frame);
                    }
                }
                self.trigger_status_cue(format!(
                    "Loop In: {}",
                    widgets::timecode(self.current_frame, video.fps)
                ));
            }
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::O)) {
                self.loop_out = Some(self.current_frame);
                if let Some(in_f) = self.loop_in {
                    if in_f > self.current_frame {
                        self.loop_in = Some(self.current_frame);
                    }
                }
                self.trigger_status_cue(format!(
                    "Loop Out: {}",
                    widgets::timecode(self.current_frame, video.fps)
                ));
            }
            if mods.alt && !mods.ctrl && !mods.command && ui.input(|i| i.key_pressed(egui::Key::X)) {
                self.loop_in = None;
                self.loop_out = None;
                self.trigger_status_cue("Loop In/Out Cleared");
            }

            // Previous / Next Clip [ [ / PgUp ] and [ ] / PgDn ]
            if no_cmd
                && ui.input(|i| i.key_pressed(egui::Key::OpenBracket) || i.key_pressed(egui::Key::PageUp))
            {
                action = Some(PlayerAction::PrevClip);
            }
            if no_cmd
                && ui.input(|i| i.key_pressed(egui::Key::CloseBracket) || i.key_pressed(egui::Key::PageDown))
            {
                action = Some(PlayerAction::NextClip);
            }

            // Play / Pause [Space] and J/K/L Shuttle
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::Space)) {
                if playback.is_some() {
                    self.is_playing = !self.is_playing;
                    self.reverse_shuttle = false;
                    self.clear_buffering();
                    if self.is_playing && self.current_frame + 1 >= total_frames {
                        self.current_frame = 0;
                    }
                    self.reanchor_clock();
                    let time_sec = self.current_frame as f32 / video.fps.max(1.0);
                    if let Some(pb) = playback {
                        if let Ok(mut a) = pb.audio.lock() {
                            if self.is_playing {
                                a.play(time_sec);
                            } else {
                                a.pause();
                            }
                        }
                    }
                }
            }

            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::K)) {
                self.is_playing = false;
                self.reverse_shuttle = false;
                self.playback_speed = 1.0;
                self.clear_buffering();
                self.reanchor_clock();
                if let Some(pb) = playback {
                    if let Ok(mut a) = pb.audio.lock() {
                        a.pause();
                        a.set_speed(1.0);
                    }
                }
                self.trigger_status_cue("Paused (1x)");
            }

            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::L)) {
                if playback.is_some() {
                    if !self.is_playing || self.reverse_shuttle {
                        self.is_playing = true;
                        self.reverse_shuttle = false;
                        self.playback_speed = 1.0;
                        self.clear_buffering();
                    } else {
                        self.playback_speed = if self.playback_speed < 1.5 {
                            2.0
                        } else if self.playback_speed < 3.0 {
                            4.0
                        } else {
                            4.0
                        };
                    }
                    self.reanchor_clock();
                    let time_sec = self.current_frame as f32 / video.fps.max(1.0);
                    if let Some(pb) = playback {
                        if let Ok(mut a) = pb.audio.lock() {
                            a.set_speed(self.playback_speed);
                            a.play(time_sec);
                        }
                    }
                    self.trigger_status_cue(format!("Forward {}x", self.playback_speed));
                }
            }

            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::J)) {
                if playback.is_some() {
                    if !self.is_playing || !self.reverse_shuttle {
                        self.is_playing = true;
                        self.reverse_shuttle = true;
                        self.playback_speed = 1.0;
                        self.clear_buffering();
                    } else {
                        self.playback_speed = if self.playback_speed < 1.5 {
                            2.0
                        } else if self.playback_speed < 3.0 {
                            4.0
                        } else {
                            4.0
                        };
                    }
                    self.reanchor_clock();
                    if let Some(pb) = playback {
                        if let Ok(mut a) = pb.audio.lock() {
                            a.pause();
                        }
                    }
                    self.trigger_status_cue(format!("Reverse {}x", self.playback_speed));
                }
            }

            // Frame stepping [Left / Right / Home / End]
            let jump_1s = (video.fps.round() as usize).max(1);
            if !mods.alt && !mods.ctrl && !mods.command && ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                if mods.shift {
                    self.current_frame = self.current_frame.saturating_sub(jump_1s);
                } else if self.current_frame > 0 {
                    self.current_frame -= 1;
                }
                self.clear_buffering();
                self.reanchor_clock();
                action = Some(PlayerAction::Seek(self.current_frame));
            }
            if !mods.alt && !mods.ctrl && !mods.command && ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                if mods.shift {
                    self.current_frame = (self.current_frame + jump_1s).min(total_frames.saturating_sub(1));
                } else if self.current_frame + 1 < total_frames {
                    self.current_frame += 1;
                }
                self.clear_buffering();
                self.reanchor_clock();
                action = Some(PlayerAction::Seek(self.current_frame));
            }
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::Home)) {
                self.current_frame = 0;
                self.clear_buffering();
                self.reanchor_clock();
                action = Some(PlayerAction::Seek(0));
            }
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::End)) {
                self.current_frame = total_frames.saturating_sub(1);
                self.clear_buffering();
                self.reanchor_clock();
                action = Some(PlayerAction::Seek(self.current_frame));
            }

            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::Z)) {
                if self.zoom_mode == ZoomMode::Fit {
                    self.zoom_mode = ZoomMode::Scale(1.0);
                } else {
                    self.zoom_mode = ZoomMode::Fit;
                    self.pan_offset = egui::Vec2::ZERO;
                }
            }
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::Plus) || i.key_pressed(egui::Key::Equals)) {
                let cur = match self.zoom_mode {
                    ZoomMode::Fit => 1.0,
                    ZoomMode::Scale(s) => s,
                };
                self.zoom_mode = ZoomMode::Scale((cur * 1.25).min(8.0));
            }
            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::Minus)) {
                let cur = match self.zoom_mode {
                    ZoomMode::Fit => 1.0,
                    ZoomMode::Scale(s) => s,
                };
                self.zoom_mode = ZoomMode::Scale((cur * 0.8).max(0.15));
            }

            if no_cmd && ui.input(|i| i.key_pressed(egui::Key::M)) {
                self.is_muted = !self.is_muted;
                if let Some(pb) = playback {
                    if let Ok(mut a) = pb.audio.lock() {
                        a.set_muted(self.is_muted);
                    }
                }
            }
        }

        // Advance frames using drift-free monotonic anchor clock (or synced audio clock),
        // with Rebuffer-Hold and failed-frame skip so playback never freezes on "BUFFERING".
        let (range_start, range_end) = self
            .active_loop_range(total_frames)
            .unwrap_or((0, total_frames.saturating_sub(1)));
        let span = (range_end.saturating_sub(range_start) + 1).max(1);

        if let Some(pb) = playback.filter(|_| self.is_playing) {
            pb.set_reverse(self.reverse_shuttle);

            // Skip any permanently-failed frames cleanly
            if pb.is_failed(self.current_frame) {
                let mut stepped = false;
                for step in 1..=span {
                    let cand = if !self.reverse_shuttle {
                        range_start + ((self.current_frame.saturating_sub(range_start)) + step) % span
                    } else {
                        range_start
                            + ((self.current_frame.saturating_sub(range_start)) + span - (step % span)) % span
                    };
                    if !self.is_looping {
                        if !self.reverse_shuttle && cand < self.current_frame {
                            break;
                        }
                        if self.reverse_shuttle && cand > self.current_frame {
                            break;
                        }
                    }
                    if !pb.is_failed(cand) {
                        self.current_frame = cand;
                        self.reanchor_clock();
                        pb.set_target(self.current_frame, self.is_looping);
                        stepped = true;
                        break;
                    }
                }
                if !stepped {
                    self.is_playing = false;
                    self.reverse_shuttle = false;
                    self.clear_buffering();
                    if let Ok(mut a) = pb.audio.lock() {
                        a.pause();
                    }
                }
            }

            if self.is_playing {
                let pre_snap = pb.snapshot(self.current_frame);
                let remaining_in_dir = if self.is_looping {
                    span
                } else if !self.reverse_shuttle {
                    range_end.saturating_sub(self.current_frame) + 1
                } else {
                    self.current_frame.saturating_sub(range_start) + 1
                };
                let rebuffer_need = 4.min(remaining_in_dir.max(1));
                let current_ready = pre_snap.is_current_ready;
                let still_rebuffer_hold = self.is_buffering
                    && pre_snap.ready_ahead < rebuffer_need
                    && self
                        .buffering_since
                        .map(|t| t.elapsed() < Duration::from_millis(450))
                        .unwrap_or(false);

                if !current_ready && pb.fatal_error().is_some() {
                    self.is_playing = false;
                    self.reverse_shuttle = false;
                    self.clear_buffering();
                    if let Ok(mut a) = pb.audio.lock() {
                        a.pause();
                    }
                } else if !current_ready || still_rebuffer_hold {
                    self.is_buffering = true;
                    self.buffering_since.get_or_insert_with(Instant::now);
                    self.last_audio_time = None;
                    self.reanchor_clock();
                    if let Ok(mut a) = pb.audio.lock() {
                        if a.is_playing() {
                            a.pause();
                        }
                    }
                    ui.ctx().request_repaint_after(Duration::from_millis(32));
                } else {
                    if self.is_buffering {
                        self.is_buffering = false;
                        self.buffering_since = None;
                        self.last_audio_time = None;
                        self.reanchor_clock();
                        if !self.reverse_shuttle {
                            let time_sec = self.current_frame as f32 / video.fps.max(1.0);
                            if let Ok(mut a) = pb.audio.lock() {
                                if a.has_audio() {
                                    a.set_speed(self.playback_speed);
                                    a.seek(time_sec, true);
                                }
                            }
                        }
                    }

                    let mut audio_synced = false;
                    if !self.reverse_shuttle {
                        if let Ok(mut a) = pb.audio.lock() {
                            if a.has_audio() {
                                a.set_speed(self.playback_speed);

                                if !a.is_playing()
                                    && (self.current_frame as f32 / video.fps.max(1.0))
                                        < (total_frames as f32 / video.fps.max(1.0) - 0.05)
                                {
                                    let time_sec = self.current_frame as f32 / video.fps.max(1.0);
                                    a.play(time_sec);
                                }

                                if a.is_playing() {
                                    if let Some(audio_time) = a.current_playback_time() {
                                        let mut stalled = false;
                                        if let Some((prev_t, since)) = self.last_audio_time {
                                            if (audio_time - prev_t).abs() <= 1e-4 {
                                                if since.elapsed() > Duration::from_millis(300) {
                                                    stalled = true;
                                                }
                                            } else {
                                                self.last_audio_time = Some((audio_time, Instant::now()));
                                            }
                                        } else {
                                            self.last_audio_time = Some((audio_time, Instant::now()));
                                        }

                                        if !stalled {
                                            let target_frame = (audio_time * video.fps.max(1.0)).floor() as usize;
                                            if target_frame > range_end {
                                                if self.is_looping {
                                                    self.current_frame = range_start;
                                                    let t_sec = range_start as f32 / video.fps.max(1.0);
                                                    a.seek(t_sec, true);
                                                    self.last_audio_time = Some((t_sec, Instant::now()));
                                                    self.reanchor_clock();
                                                } else {
                                                    self.current_frame = range_end;
                                                    self.is_playing = false;
                                                    a.pause();
                                                }
                                            } else {
                                                self.current_frame = target_frame.max(range_start);
                                                self.reanchor_clock();
                                            }
                                            audio_synced = true;
                                        }
                                    }
                                }
                            }
                        }
                    }

                    if !audio_synced {
                        let effective_fps = (video.fps * self.playback_speed).max(1.0) as f64;
                        let elapsed = self.playback_anchor_time.elapsed().as_secs_f64();
                        if !self.reverse_shuttle {
                            let desired = self.playback_anchor_frame + elapsed * effective_fps;
                            let raw_f = desired.floor().max(0.0) as usize;
                            if raw_f > range_end {
                                if self.is_looping {
                                    self.current_frame = range_start + ((raw_f - range_start) % span);
                                    self.reanchor_clock();
                                    if let Ok(mut a) = pb.audio.lock() {
                                        if a.has_audio() {
                                            a.seek(self.current_frame as f32 / video.fps.max(1.0), true);
                                        }
                                    }
                                } else {
                                    self.current_frame = range_end;
                                    self.is_playing = false;
                                }
                            } else {
                                self.current_frame = raw_f.max(range_start);
                            }
                        } else {
                            let desired = self.playback_anchor_frame - elapsed * effective_fps;
                            if desired < range_start as f64 {
                                if self.is_looping {
                                    let steps_back = (range_start as f64 - desired).ceil() as usize;
                                    let offset = steps_back.saturating_sub(1) % span;
                                    self.current_frame = range_end.saturating_sub(offset);
                                    self.reanchor_clock();
                                } else {
                                    self.current_frame = range_start;
                                    self.is_playing = false;
                                    self.reverse_shuttle = false;
                                }
                            } else {
                                self.current_frame = (desired.floor() as usize).clamp(range_start, range_end);
                            }
                        }
                    }

                    if self.is_playing {
                        let frame_period = 1.0 / (video.fps * self.playback_speed).max(1.0) as f64;
                        let elapsed_frames = self.playback_anchor_time.elapsed().as_secs_f64() / frame_period;
                        let frac_to_next = 1.0 - elapsed_frames.fract();
                        let wait_ms = (frac_to_next * frame_period * 1000.0).clamp(2.0, 16.0);
                        ui.ctx().request_repaint_after(Duration::from_millis(wait_ms.round() as u64));
                    }
                }
            }
        } else if playback.is_none() {
            self.is_playing = false;
            self.reverse_shuttle = false;
            self.clear_buffering();
        }

        // Update read-ahead target after frame advance and take one single snapshot for all UI bars
        let snapshot = playback.map(|pb| {
            pb.set_target(self.current_frame, self.is_looping);
            pb.snapshot(self.current_frame)
        });

        // 1. Studio Player Header Bar (Shown in regular windowed player)
        if !self.is_fullscreen {
            if let Some(header_act) = HeaderBar::show(self, ui, video, playback, snapshot.as_ref(), load_error, clip_pos) {
                action = Some(header_act);
            }

            ui.add_space(2.0);
            ui.separator();
            ui.add_space(2.0);
        }

        // 2. Main Viewport Canvas (Cinema Screen)
        if let Some(canvas_act) = Canvas::show(self, ui, video, playback, snapshot.as_ref(), load_error, gpu_info, thumb_id) {
            action = Some(canvas_act);
        }

        // 3. Bottom Controls:
        if !self.is_fullscreen {
            ui.add_space(4.0);
            if let Some(transport_act) = TransportBar::show(self, ui, video, playback, snapshot.as_ref(), total_frames) {
                action = Some(transport_act);
            }
        } else {
            let idle_secs = self.last_interaction_time.elapsed().as_secs_f32();
            let alpha = if idle_secs < 2.5 {
                ui.ctx().request_repaint_after(Duration::from_secs_f32((2.5 - idle_secs).max(0.05)));
                1.0
            } else if idle_secs < 3.0 {
                ui.ctx().request_repaint_after(Duration::from_millis(16));
                1.0 - (idle_secs - 2.5) / 0.5
            } else {
                0.0
            };

            if alpha > 0.0 {
                if let Some(overlay_act) = FullscreenOverlay::show(
                    self,
                    ui,
                    video,
                    playback,
                    snapshot.as_ref(),
                    clip_pos,
                    total_frames,
                    alpha,
                ) {
                    action = Some(overlay_act);
                }
            }
        }

        action
    }
}
