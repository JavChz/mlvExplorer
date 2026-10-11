use crate::db::VideoRecord;
use egui::{Color32, Rect, Rounding, Stroke, Ui, Vec2};
use std::time::Instant;

pub struct PlayerView {
    pub current_frame: usize,
    pub is_playing: bool,
    pub is_looping: bool,
    pub playback_speed: f32,
    pub last_frame_time: Instant,
}

impl Default for PlayerView {
    fn default() -> Self {
        Self {
            current_frame: 0,
            is_playing: false,
            is_looping: true,
            playback_speed: 1.0,
            last_frame_time: Instant::now(),
        }
    }
}

pub enum PlayerAction {
    BackToGrid,
    FrameChanged(usize),
}

impl PlayerView {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        video: &VideoRecord,
    ) -> Option<PlayerAction> {
        let mut action = None;

        // Header Bar: Back button, Title, Resolution, FPS
        ui.horizontal(|ui| {
            if ui.button("⬅ Grid View [G]").clicked() || ui.input(|i| i.key_pressed(egui::Key::G) || i.key_pressed(egui::Key::Escape)) {
                action = Some(PlayerAction::BackToGrid);
            }
            ui.separator();
            ui.heading(&video.file_name);
            ui.separator();
            ui.label(format!("{} × {} | {:.2} fps | {}-bit", video.width, video.height, video.fps, video.bits_per_pixel));
        });

        ui.separator();

        let total_frames = video.frame_count.max(1) as usize;

        // Advance frames if playing
        let fps = (video.fps * self.playback_speed).max(1.0);
        let frame_interval = std::time::Duration::from_secs_f32(1.0 / fps);

        if self.is_playing && self.last_frame_time.elapsed() >= frame_interval {
            self.last_frame_time = Instant::now();
            if self.current_frame + 1 < total_frames {
                self.current_frame += 1;
            } else if self.is_looping {
                self.current_frame = 0;
            } else {
                self.is_playing = false;
            }
            action = Some(PlayerAction::FrameChanged(self.current_frame));
            ui.ctx().request_repaint(); // Request next frame redraw
        }

        // Main Player Canvas (GPU Viewport Area)
        let available_size = ui.available_size() - Vec2::new(0.0, 80.0);
        let (rect, _response) = ui.allocate_exact_size(available_size, egui::Sense::hover());

        let painter = ui.painter();
        // Dark viewport letterbox background
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(12, 12, 15));

        // Draw centered video frame placeholder / display rect (16:9 or matching video aspect ratio)
        let aspect = if video.height > 0 { video.width as f32 / video.height as f32 } else { 16.0 / 9.0 };
        let mut display_w = rect.width();
        let mut display_h = display_w / aspect;

        if display_h > rect.height() {
            display_h = rect.height();
            display_w = display_h * aspect;
        }

        let center = rect.center();
        let display_rect = Rect::from_center_size(center, Vec2::new(display_w, display_h));

        painter.rect(display_rect, Rounding::ZERO, Color32::from_rgb(20, 22, 28), Stroke::new(1.0, Color32::from_rgb(40, 45, 55)));

        // Center Overlay Status
        let frame_time_secs = if video.fps > 0.0 { self.current_frame as f32 / video.fps } else { 0.0 };
        let status_text = format!(
            "Frame {} / {} ({:.2}s / {:.2}s)\nGPU Raw Demosaic Preview",
            self.current_frame + 1,
            total_frames,
            frame_time_secs,
            video.duration_seconds
        );
        painter.text(
            center,
            egui::Align2::CENTER_CENTER,
            status_text,
            egui::FontId::proportional(16.0),
            Color32::from_rgb(180, 190, 210),
        );

        ui.add_space(8.0);

        // Timeline Scrubber & Transport Controls
        ui.vertical(|ui| {
            // Scrubber Slider
            let mut frame_val = self.current_frame;
            let slider = ui.add(
                egui::Slider::new(&mut frame_val, 0..=(total_frames.saturating_sub(1)))
                    .show_value(false)
            );
            if slider.changed() {
                self.current_frame = frame_val;
                action = Some(PlayerAction::FrameChanged(self.current_frame));
            }

            ui.horizontal(|ui| {
                // Play / Pause Button
                let play_label = if self.is_playing { "⏸ Pause [Space]" } else { "▶ Play [Space]" };
                if ui.button(play_label).clicked() || ui.input(|i| i.key_pressed(egui::Key::Space)) {
                    self.is_playing = !self.is_playing;
                    self.last_frame_time = Instant::now();
                }

                // Step Backward 1 frame
                if ui.button("⏮ -1 Frame [Left]").clicked() || ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                    if self.current_frame > 0 {
                        self.current_frame -= 1;
                        action = Some(PlayerAction::FrameChanged(self.current_frame));
                    }
                }

                // Step Forward 1 frame
                if ui.button("+1 Frame ⏭ [Right]").clicked() || ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                    if self.current_frame + 1 < total_frames {
                        self.current_frame += 1;
                        action = Some(PlayerAction::FrameChanged(self.current_frame));
                    }
                }

                // Loop Toggle
                let loop_label = if self.is_looping { "🔁 Loop: ON" } else { "➡ Loop: OFF" };
                if ui.selectable_label(self.is_looping, loop_label).clicked() {
                    self.is_looping = !self.is_looping;
                }

                ui.separator();

                // Playback speed
                ui.label("Speed:");
                for spd in [0.5, 1.0, 2.0] {
                    if ui.selectable_label((self.playback_speed - spd).abs() < 0.01, format!("{}x", spd)).clicked() {
                        self.playback_speed = spd;
                    }
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!(
                        "TC: {:02}:{:02}:{:02}",
                        (frame_time_secs / 60.0) as u32,
                        (frame_time_secs % 60.0) as u32,
                        (self.current_frame % (video.fps as usize).max(1))
                    )).monospace().strong());
                });
            });
        });

        action
    }
}
