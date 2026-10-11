use crate::db::VideoRecord;
use crate::gpu::{RawViewCallback, ViewUniforms};
use crate::mlv::color::camera_to_srgb;
use crate::playback::Playback;
use crate::ui::theme::*;
use eframe::egui_wgpu;
use egui::{Color32, Rect, RichText, Rounding, Stroke, Ui, Vec2};
use std::time::Instant;

pub enum PlayerAction {
    BackToGrid,
    Seek(usize),
}

pub struct PlayerView {
    pub current_frame: usize,
    pub is_playing: bool,
    pub is_looping: bool,
    pub playback_speed: f32,
    pub exposure_ev: f32, // -2.0 to +2.0 EV
    pub zoom_fit: bool,
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
            zoom_fit: true,
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

        // Header Bar: Back to Grid, Filename, Resolution, FPS, Timecode
        ui.horizontal(|ui| {
            if ui.button("⬅ Grid [G]").clicked() || ui.input(|i| i.key_pressed(egui::Key::G) || i.key_pressed(egui::Key::Escape)) {
                action = Some(PlayerAction::BackToGrid);
            }

            ui.separator();

            ui.label(RichText::new(&video.file_name).strong().size(13.5).color(TEXT_PRIMARY));
            ui.label(RichText::new(format!(
                "· {}×{} · {:.2} fps · {}",
                video.width, video.height, video.fps, video.codec
            )).color(TEXT_MUTED));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Exposure EV slider
                ui.add(egui::Slider::new(&mut self.exposure_ev, -2.0..=2.0).text("EV").step_by(0.1));
                ui.label(RichText::new("Exp:").color(TEXT_SECONDARY));

                ui.separator();

                // Decode performance badge
                let dec_fps = playback.decode_fps();
                let dec_col = if dec_fps >= video.fps { Color32::from_rgb(34, 197, 94) } else { Color32::from_rgb(234, 179, 8) };
                ui.label(RichText::new(format!("GPU/Decoder: {:.1} fps", dec_fps)).small().color(dec_col));
            });
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // Advance frames if currently playing
        let effective_fps = (video.fps * self.playback_speed).max(1.0);
        let frame_duration = std::time::Duration::from_secs_f32(1.0 / effective_fps);

        if self.is_playing && self.last_frame_time.elapsed() >= frame_duration {
            self.last_frame_time = Instant::now();
            if self.current_frame + 1 < total_frames {
                self.current_frame += 1;
            } else if self.is_looping {
                self.current_frame = 0;
            } else {
                self.is_playing = false;
            }
            action = Some(PlayerAction::Seek(self.current_frame));
            ui.ctx().request_repaint();
        }

        // Main Viewport Canvas
        let available_size = ui.available_size() - Vec2::new(0.0, 72.0);
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        let painter = ui.painter();
        // Deep letterbox cinema background
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(10, 10, 12));

        // Calculate aspect-ratio preserved display rect
        let aspect = if video.height > 0 { video.width as f32 / video.height as f32 } else { 16.0 / 9.0 };
        let mut display_w = rect.width();
        let mut display_h = display_w / aspect;

        if display_h > rect.height() {
            display_h = rect.height();
            display_w = display_h * aspect;
        }

        let center = rect.center();
        let display_rect = Rect::from_center_size(center, Vec2::new(display_w, display_h));

        // Check if current frame is ready in cache
        if let Some(frame) = playback.get(self.current_frame) {
            // Paint using GPU WGSL shader callback!
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
            // Subtle buffering indicator
            painter.rect(display_rect, Rounding::ZERO, Color32::from_rgb(18, 18, 22), Stroke::new(1.0_f32, BORDER_SUBTLE));
            painter.text(
                center,
                egui::Align2::CENTER_CENTER,
                format!("Buffering Frame {}...", self.current_frame + 1),
                egui::FontId::proportional(14.0),
                TEXT_MUTED,
            );
        }

        ui.add_space(4.0);

        // Bottom Transport Bar & Scrubber
        ui.vertical(|ui| {
            let mut scrubber_val = self.current_frame;
            let slider_res = ui.add(
                egui::Slider::new(&mut scrubber_val, 0..=(total_frames.saturating_sub(1)))
                    .show_value(false)
            );
            if slider_res.changed() {
                self.current_frame = scrubber_val;
                action = Some(PlayerAction::Seek(self.current_frame));
            }

            // Transport Controls: Play, Step, Loop, Speed, Timecode
            ui.horizontal(|ui| {
                // Play / Pause
                let play_text = if self.is_playing { "⏸ Pause [Space]" } else { "▶ Play [Space]" };
                if ui.button(RichText::new(play_text).strong()).clicked() || ui.input(|i| i.key_pressed(egui::Key::Space)) {
                    self.is_playing = !self.is_playing;
                    self.last_frame_time = Instant::now();
                }

                // Step -1
                if ui.button("⏮ -1 [←]").clicked() || ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    if self.current_frame > 0 {
                        self.current_frame -= 1;
                        action = Some(PlayerAction::Seek(self.current_frame));
                    }
                }

                // Step +1
                if ui.button("+1 [→] ⏭").clicked() || ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    if self.current_frame + 1 < total_frames {
                        self.current_frame += 1;
                        action = Some(PlayerAction::Seek(self.current_frame));
                    }
                }

                // Loop Toggle
                let loop_col = if self.is_looping { ACCENT_BLUE } else { TEXT_MUTED };
                if ui.selectable_label(self.is_looping, RichText::new("🔁 Loop").color(loop_col)).clicked() {
                    self.is_looping = !self.is_looping;
                }

                ui.separator();

                // Speed buttons
                ui.label(RichText::new("Speed:").color(TEXT_SECONDARY));
                for spd in [0.5, 1.0, 2.0] {
                    let is_active = (self.playback_speed - spd).abs() < 0.05;
                    if ui.selectable_label(is_active, format!("{}x", spd)).clicked() {
                        self.playback_speed = spd;
                    }
                }

                // Timecode & Frame Counter (Right-aligned)
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let cur_time = if video.fps > 0.0 { self.current_frame as f32 / video.fps } else { 0.0 };
                    let m = (cur_time / 60.0) as u32;
                    let s = (cur_time % 60.0) as u32;
                    let f = (self.current_frame % (video.fps as usize).max(1)) as u32;

                    ui.label(RichText::new(format!(
                        "TC {:02}:{:02}:{:02}  [{}/{}]",
                        m, s, f, self.current_frame + 1, total_frames
                    )).monospace().strong().size(12.5).color(TEXT_PRIMARY));
                });
            });
        });

        action
    }
}
