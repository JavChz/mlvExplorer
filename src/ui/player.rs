use crate::db::{PickFlag, VideoRecord};
use crate::gpu::{RawViewCallback, ViewUniforms};
use crate::mlv::color::camera_to_srgb;
use crate::playback::Playback;
use crate::ui::playbar::Playbar;
use crate::ui::theme::*;
use eframe::egui_wgpu;
use egui::{Color32, Rect, RichText, Rounding, Stroke, Ui, Vec2};
use std::time::Instant;

pub enum PlayerAction {
    BackToGrid,
    Seek(usize),
    SetRating(u8),
    SetFlag(PickFlag),
    SetStretch(f32),
}

pub struct PlayerView {
    pub current_frame: usize,
    pub is_playing: bool,
    pub is_looping: bool,
    pub playback_speed: f32,
    pub exposure_ev: f32, // -2.0 to +2.0 EV
    pub zoom_100: bool,
    pub last_frame_time: Instant,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            current_frame: 0,
            is_playing: false,
            is_looping: true,
            playback_speed: 1.0,
            exposure_ev: 0.0,
            zoom_100: false,
            last_frame_time: Instant::now(),
        }
    }
}

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

        // 1. Studio Player Header Bar
        ui.horizontal(|ui| {
            if ui.button(RichText::new("⬅ Library [G]").strong()).clicked()
                || ui.input(|i| i.key_pressed(egui::Key::G) || i.key_pressed(egui::Key::Escape))
            {
                action = Some(PlayerAction::BackToGrid);
            }

            ui.separator();

            // Clip Name
            ui.label(RichText::new(&video.file_name).strong().size(14.0).color(TEXT_PRIMARY));

            // Metadata Badges
            ui.label(RichText::new(format!("{} × {}", video.width, video.height)).small().color(TEXT_SECONDARY));
            ui.label(RichText::new("·").color(TEXT_MUTED));
            ui.label(RichText::new(format!("{:.3} fps", video.fps)).small().color(TEXT_SECONDARY));
            ui.label(RichText::new("·").color(TEXT_MUTED));
            ui.label(RichText::new(&video.codec).small().color(Color32::from_rgb(147, 197, 253)));

            if !video.camera_name.is_empty() {
                ui.label(RichText::new("·").color(TEXT_MUTED));
                ui.label(RichText::new(&video.camera_name).small().color(TEXT_MUTED));
            }

            // Right-aligned Toolbar: De-Squeeze, Zoom, Exposure, Decode Perf
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Exposure EV with quick reset
                if self.exposure_ev != 0.0 && ui.small_button("↺ 0").clicked() {
                    self.exposure_ev = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut self.exposure_ev, -2.5..=2.5)
                        .text("EV")
                        .step_by(0.1)
                        .show_value(true),
                );
                ui.label(RichText::new("Exposure:").small().color(TEXT_SECONDARY));

                ui.separator();

                // 1:1 vs Fit toggle
                let zoom_label = if self.zoom_100 { "🔍 100% (1:1)" } else { "⛶ Fit" };
                if ui.selectable_label(self.zoom_100, RichText::new(zoom_label).small()).clicked() {
                    self.zoom_100 = !self.zoom_100;
                }

                ui.separator();

                // Anamorphic De-Squeeze (Stretch Factor) Dropdown
                let cur_stretch = video.stretch_factor;
                let stretch_name = match cur_stretch {
                    s if (s - 3.0).abs() < 0.05 => "3.0x (1x3 Anamorphic)",
                    s if (s - 2.0).abs() < 0.05 => "2.0x Anamorphic",
                    s if (s - 1.75).abs() < 0.05 => "1.75x Anamorphic",
                    s if (s - 1.5).abs() < 0.05 => "1.5x Anamorphic",
                    s if (s - 1.333).abs() < 0.05 => "1.33x Anamorphic",
                    s if (s - 0.333).abs() < 0.05 => "0.33x (3x1)",
                    s if (s - 1.0).abs() < 0.05 => "1.0x (1:1 Square)",
                    s => &format!("{:.2}x", s),
                };

                egui::ComboBox::from_id_salt("player_desqueeze_select")
                    .selected_text(RichText::new(format!("Stretch: {}", stretch_name)).small())
                    .show_ui(ui, |ui| {
                        for (factor, label) in [
                            (1.0f32, "1.0x (1:1 Square - Off)"),
                            (3.0f32, "3.0x (1x3 Anamorphic / EOS M Crop Mood)"),
                            (2.0f32, "2.0x (Standard 2x Anamorphic)"),
                            (1.75f32, "1.75x Anamorphic"),
                            (1.5f32, "1.5x Anamorphic"),
                            (1.333f32, "1.33x Anamorphic"),
                            (0.333f32, "0.33x (3x1 Binning)"),
                        ] {
                            let is_sel = (cur_stretch - factor).abs() < 0.02;
                            if ui.selectable_label(is_sel, label).clicked() {
                                action = Some(PlayerAction::SetStretch(factor));
                            }
                        }
                    });

                ui.separator();

                // Decoder Performance
                let dec_fps = playback.decode_fps();
                let dec_col = if dec_fps >= video.fps {
                    Color32::from_rgb(34, 197, 94)
                } else {
                    Color32::from_rgb(234, 179, 8)
                };
                ui.label(RichText::new(format!("Decoder: {:.1} fps", dec_fps)).small().color(dec_col));
            });
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // Advance frames if currently playing
        let effective_fps = (video.fps * self.playback_speed).max(1.0);
        let frame_duration = std::time::Duration::from_secs_f32(1.0 / effective_fps);

        if self.is_playing && self.last_frame_time.elapsed() >= frame_duration {
            // Buffer-aware pacing: If the next frame is ready or already cached, advance.
            // If it's not ready yet, do not jump ahead and skip frames; wait up to a short threshold
            // so playback never stutters or drops frames unnecessarily!
            let next_target = if self.current_frame + 1 < total_frames {
                self.current_frame + 1
            } else if self.is_looping {
                0
            } else {
                self.current_frame
            };

            let is_next_ready = playback.is_ready(next_target);
            if is_next_ready || self.last_frame_time.elapsed() >= frame_duration * 3 {
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
            ui.ctx().request_repaint();
        }

        // 2. Main Viewport Canvas (Cinema Screen)
        // Reserve 88px for the bottom professional timeline & transport bar
        let available_size = ui.available_size() - Vec2::new(0.0, 92.0);
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        let painter = ui.painter();
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(10, 10, 12));

        // Calculate aspect-ratio preserved display rect respecting anamorphic de-squeeze
        let stretch = if video.stretch_factor > 0.05 { video.stretch_factor } else { 1.0 };
        let aspect = if video.height > 0 {
            (video.width as f32 * stretch) / video.height as f32
        } else {
            16.0 / 9.0
        };

        let (display_w, display_h) = if self.zoom_100 {
            // 1:1 pixel size with horizontal stretch applied
            (video.width as f32 * stretch, video.height as f32)
        } else {
            // Fit to available viewport
            let mut dw = rect.width();
            let mut dh = dw / aspect;
            if dh > rect.height() {
                dh = rect.height();
                dw = dh * aspect;
            }
            (dw, dh)
        };

        let center = rect.center();
        let display_rect = Rect::from_center_size(center, Vec2::new(display_w, display_h));

        // Render decoded frame via GPU WGSL shader callback
        if let Some(frame) = playback.get(self.current_frame) {
            let m = camera_to_srgb(&playback.source.clip.meta);
            let (rx, ry) = playback.source.clip.meta.red_offset();
            let exp_mult = (2.0f32).powf(self.exposure_ev);

            let uniforms = ViewUniforms {
                m0: [m[0][0], m[0][1], m[0][2], 0.0],
                m1: [m[1][0], m[1][1], m[1][2], 0.0],
                m2: [m[2][0], m[2][1], m[2][2], 0.0],
                levels: [frame.black, frame.white, exp_mult, 1.0],
                info: [rx, ry, frame.width, frame.height],
                view: [0.0, 0.0, 1.0, 1.0],
            };

            let key = (video.id as u64) << 32 | (self.current_frame as u64);
            let cb = RawViewCallback {
                frame: frame.clone(),
                key,
                uniforms,
            };

            let paint_cb = egui_wgpu::Callback::new_paint_callback(display_rect, cb);
            painter.add(paint_cb);
        } else {
            // Buffering placeholder
            painter.rect(
                display_rect,
                Rounding::ZERO,
                Color32::from_rgb(16, 16, 20),
                Stroke::new(1.0_f32, BORDER_SUBTLE),
            );
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                format!("⏳ Buffering Frame {}...", self.current_frame + 1),
                egui::FontId::proportional(15.0),
                TEXT_MUTED,
            );
        }

        ui.add_space(4.0);

        // 3. Bottom Studio Console: Full Playbar + Transport Controls
        ui.group(|ui| {
            // A. Full-Width Professional Scrubber Playbar
            let cached_frames = playback.cached_frames();
            if let Some(new_frame) = Playbar::show(
                ui,
                "player_main_playbar",
                self.current_frame,
                total_frames,
                video.fps,
                &cached_frames,
            ) {
                self.current_frame = new_frame;
                action = Some(PlayerAction::Seek(self.current_frame));
            }

            ui.add_space(4.0);

            // B. Transport Controls Row
            ui.horizontal(|ui| {
                // Left: Timecode Readout & Frame Counter
                let cur_time = if video.fps > 0.0 {
                    self.current_frame as f32 / video.fps
                } else {
                    0.0
                };
                let cm = (cur_time / 60.0) as u32;
                let cs = (cur_time % 60.0) as u32;
                let cf = (self.current_frame % (video.fps as usize).max(1)) as u32;

                let tot_time = video.duration_seconds;
                let tm = (tot_time / 60.0) as u32;
                let ts = (tot_time % 60.0) as u32;

                ui.label(
                    RichText::new(format!("{:02}:{:02}:{:02}", cm, cs, cf))
                        .monospace()
                        .strong()
                        .size(14.0)
                        .color(Color32::WHITE),
                );
                ui.label(
                    RichText::new(format!("/ {:02}:{:02}", tm, ts))
                        .monospace()
                        .size(11.5)
                        .color(TEXT_MUTED),
                );
                ui.label(
                    RichText::new(format!("[{}/{}]", self.current_frame + 1, total_frames))
                        .monospace()
                        .small()
                        .color(TEXT_SECONDARY),
                );

                ui.add_space(20.0);

                // Center: Transport Buttons
                // Jump to Start
                if ui.button(RichText::new("⏮").size(13.0)).on_hover_text("Jump to Start [Home]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Home))
                {
                    self.current_frame = 0;
                    action = Some(PlayerAction::Seek(0));
                }

                // Step -1 Frame
                if ui.button(RichText::new("◀").size(13.0)).on_hover_text("Step -1 Frame [Left Arrow]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::ArrowLeft))
                {
                    if self.current_frame > 0 {
                        self.current_frame -= 1;
                        action = Some(PlayerAction::Seek(self.current_frame));
                    }
                }

                // PLAY / PAUSE (Prominent Studio Button)
                let play_icon = if self.is_playing { " ⏸ PAUSE " } else { " ▶ PLAY  " };
                let play_bg = if self.is_playing {
                    Color32::from_rgb(59, 130, 246)
                } else {
                    Color32::from_rgb(38, 42, 52)
                };
                let play_btn = egui::Button::new(RichText::new(play_icon).strong().size(13.0).color(Color32::WHITE))
                    .fill(play_bg);

                if ui.add(play_btn).on_hover_text("Play / Pause [Space]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Space))
                {
                    self.is_playing = !self.is_playing;
                    self.last_frame_time = Instant::now();
                }

                // Step +1 Frame
                if ui.button(RichText::new("▶").size(13.0)).on_hover_text("Step +1 Frame [Right Arrow]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::ArrowRight))
                {
                    if self.current_frame + 1 < total_frames {
                        self.current_frame += 1;
                        action = Some(PlayerAction::Seek(self.current_frame));
                    }
                }

                // Jump to End
                if ui.button(RichText::new("⏭").size(13.0)).on_hover_text("Jump to End [End]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::End))
                {
                    self.current_frame = total_frames.saturating_sub(1);
                    action = Some(PlayerAction::Seek(self.current_frame));
                }

                ui.separator();

                // Loop Toggle
                let loop_col = if self.is_looping {
                    ACCENT_BLUE
                } else {
                    TEXT_MUTED
                };
                if ui
                    .selectable_label(self.is_looping, RichText::new("🔁 Loop").color(loop_col))
                    .clicked()
                {
                    self.is_looping = !self.is_looping;
                }

                // Speed Selector Pills (0.5x, 1x, 2x, 4x)
                ui.separator();
                ui.label(RichText::new("Speed:").small().color(TEXT_SECONDARY));
                for spd in [0.5, 1.0, 2.0, 4.0] {
                    let is_active = (self.playback_speed - spd).abs() < 0.05;
                    if ui.selectable_label(is_active, format!("{}x", spd)).clicked() {
                        self.playback_speed = spd;
                    }
                }

                // Right side: Quick Rating & Pick buttons on player bar!
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Pick / Reject Flags
                    let is_pick = video.flag == PickFlag::Picked;
                    if ui.selectable_label(is_pick, RichText::new("🏳 Pick [P]").color(FLAG_PICKED)).clicked() {
                        let new_flag = if is_pick { PickFlag::Unflagged } else { PickFlag::Picked };
                        action = Some(PlayerAction::SetFlag(new_flag));
                    }

                    let is_rej = video.flag == PickFlag::Rejected;
                    if ui.selectable_label(is_rej, RichText::new("❌ Reject [X]").color(FLAG_REJECTED)).clicked() {
                        let new_flag = if is_rej { PickFlag::Unflagged } else { PickFlag::Rejected };
                        action = Some(PlayerAction::SetFlag(new_flag));
                    }

                    ui.separator();

                    // Star Rating (1-5★)
                    for star in (1..=5).rev() {
                        let is_lit = star <= video.rating;
                        let star_str = if is_lit { "★" } else { "☆" };
                        let star_col = if is_lit { ACCENT_GOLD } else { TEXT_MUTED };
                        if ui.small_button(RichText::new(star_str).color(star_col)).clicked() {
                            let new_r = if video.rating == star { 0 } else { star };
                            action = Some(PlayerAction::SetRating(new_r));
                        }
                    }
                    ui.label(RichText::new("Rating:").small().color(TEXT_SECONDARY));
                });
            });
        });

        action
    }
}
