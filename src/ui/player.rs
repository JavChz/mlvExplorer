use crate::db::{PickFlag, VideoRecord};
use crate::gpu::{RawViewCallback, ViewUniforms};
use crate::mlv::color::camera_to_srgb;
use crate::playback::Playback;
use crate::ui::playbar::Playbar;
use crate::ui::theme::*;
use eframe::egui_wgpu;
use egui::{Color32, Rect, RichText, Rounding, Stroke, Ui, Vec2};
use std::sync::Arc;
use std::time::Instant;

pub enum PlayerAction {
    BackToGrid,
    Seek(usize),
    SetRating(u8),
    SetFlag(PickFlag),
    SetStretch(f32),
    ToggleFullscreen,
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
    pub exposure_ev: f32, // -2.5 to +2.5 EV
    pub wb_temperature: f32, // -50.0 (Cool) to +50.0 (Warm)
    pub wb_tint: f32, // -50.0 (Green) to +50.0 (Magenta)
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
                self.pan_offset = Vec2::ZERO;
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
        ui.horizontal(|ui| {
            if ui.button(RichText::new("⬅ Library [G]").strong()).clicked()
                || ui.input(|i| !self.is_fullscreen && (i.key_pressed(egui::Key::G) || i.key_pressed(egui::Key::Escape)))
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

            // Right-aligned Toolbar: Realtime status, Stretch, Fullscreen, Zoom, WB, Exposure
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(RichText::new("🚀 GPU Debayer: Direct").small().color(Color32::from_rgb(96, 165, 250)));
                ui.separator();

                // Real-Time / Buffer Health Indicator
                let ready_ahead = playback.ready_ahead(self.current_frame, 24);
                let is_realtime = ready_ahead >= 3 || (!self.is_playing && playback.is_ready(self.current_frame));
                let status_text = if !self.is_playing {
                    format!("● READY ({} buf)", playback.cache_count())
                } else if is_realtime {
                    format!("⚡ REAL-TIME ({} buf)", ready_ahead)
                } else {
                    format!("⏳ BUFFERING ({}/{})", ready_ahead, 6)
                };
                let status_col = if is_realtime {
                    Color32::from_rgb(34, 197, 94) // Studio Green
                } else {
                    Color32::from_rgb(234, 179, 8) // Warning Gold
                };
                ui.label(RichText::new(status_text).strong().size(12.0).color(status_col));

                ui.separator();

                // Fullscreen toggle button
                let fs_label = if self.is_fullscreen { "🗗 Exit [F]" } else { "⛶ Fullscreen [F]" };
                if ui.selectable_label(self.is_fullscreen, RichText::new(fs_label).small()).on_hover_text("Toggle Fullscreen Cinema Mode [F]").clicked() {
                    self.is_fullscreen = !self.is_fullscreen;
                    action = Some(PlayerAction::ToggleFullscreen);
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

                // Zoom Controls: Fit, 50%, 100%, 200%, -/+, and custom levels
                if ui.small_button("+").on_hover_text("Zoom In [+]").clicked() {
                    let cur = match self.zoom_mode {
                        ZoomMode::Fit => 1.0,
                        ZoomMode::Scale(s) => s,
                    };
                    self.zoom_mode = ZoomMode::Scale((cur * 1.25).min(8.0));
                }
                if ui.small_button("-").on_hover_text("Zoom Out [-]").clicked() {
                    let cur = match self.zoom_mode {
                        ZoomMode::Fit => 1.0,
                        ZoomMode::Scale(s) => s,
                    };
                    self.zoom_mode = ZoomMode::Scale((cur * 0.8).max(0.15));
                }

                let zoom_display = match self.zoom_mode {
                    ZoomMode::Fit => "Fit".to_string(),
                    ZoomMode::Scale(s) if (s - 0.5).abs() < 0.02 => "50%".to_string(),
                    ZoomMode::Scale(s) if (s - 1.0).abs() < 0.02 => "100% (1:1)".to_string(),
                    ZoomMode::Scale(s) if (s - 2.0).abs() < 0.02 => "200%".to_string(),
                    ZoomMode::Scale(s) => format!("{:.0}%", s * 100.0),
                };

                egui::ComboBox::from_id_salt("player_zoom_select")
                    .selected_text(RichText::new(format!("🔍 {}", zoom_display)).small())
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(self.zoom_mode == ZoomMode::Fit, "⛶ Fit to Viewport [Z]").clicked() {
                            self.zoom_mode = ZoomMode::Fit;
                            self.pan_offset = Vec2::ZERO;
                        }
                        if ui.selectable_label(matches!(self.zoom_mode, ZoomMode::Scale(s) if (s - 0.5).abs() < 0.02), "50% Half").clicked() {
                            self.zoom_mode = ZoomMode::Scale(0.5);
                        }
                        if ui.selectable_label(matches!(self.zoom_mode, ZoomMode::Scale(s) if (s - 1.0).abs() < 0.02), "100% (1:1 Native Pixel) [Z]").clicked() {
                            self.zoom_mode = ZoomMode::Scale(1.0);
                        }
                        if ui.selectable_label(matches!(self.zoom_mode, ZoomMode::Scale(s) if (s - 2.0).abs() < 0.02), "200% (2:1 Peep)").clicked() {
                            self.zoom_mode = ZoomMode::Scale(2.0);
                        }
                    });

                let is_fit = self.zoom_mode == ZoomMode::Fit;
                if ui.selectable_label(is_fit, RichText::new("⛶ Fit").small()).on_hover_text("Fit frame to canvas [Z]").clicked() {
                    self.zoom_mode = ZoomMode::Fit;
                    self.pan_offset = Vec2::ZERO;
                }

                ui.separator();

                // Exposure EV Slider with quick reset
                if self.exposure_ev != 0.0 && ui.small_button("↺").on_hover_text("Reset Exposure [0]").clicked() {
                    self.exposure_ev = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut self.exposure_ev, -2.5..=2.5)
                        .text("EV")
                        .step_by(0.1)
                        .show_value(true),
                );

                ui.separator();

                // White Balance Tint Slider (Green <-> Magenta)
                if self.wb_tint != 0.0 && ui.small_button("↺").on_hover_text("Reset Tint [0]").clicked() {
                    self.wb_tint = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut self.wb_tint, -50.0..=50.0)
                        .text("Tint")
                        .step_by(1.0)
                        .show_value(true),
                );

                // White Balance Temperature Slider (Cool <-> Warm)
                if self.wb_temperature != 0.0 && ui.small_button("↺").on_hover_text("Reset Temperature [0]").clicked() {
                    self.wb_temperature = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut self.wb_temperature, -50.0..=50.0)
                        .text("WB")
                        .step_by(1.0)
                        .show_value(true),
                );
            });
        });

        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);

        // Advance frames if currently playing
        let effective_fps = (video.fps * self.playback_speed).max(1.0);
        let frame_duration = std::time::Duration::from_secs_f32(1.0 / effective_fps);

        if self.is_playing {
            // Buffer-paced playback engine:
            // Smoothly advances at the native recorded frame rate.
            let next_target = if self.current_frame + 1 < total_frames {
                self.current_frame + 1
            } else if self.is_looping {
                0
            } else {
                self.current_frame
            };

            let elapsed = self.last_frame_time.elapsed();
            if elapsed >= frame_duration {
                // If next frame is ready or we've waited up to 1.5x frame duration, advance!
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
        // Reserve exactly 86.0px for the bottom professional timeline & transport bar
        let bottom_height = 86.0;
        let canvas_height = (ui.available_height() - bottom_height).max(50.0);
        let canvas_size = Vec2::new(ui.available_width(), canvas_height);
        let (rect, canvas_response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());

        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, Rounding::ZERO, Color32::from_rgb(10, 10, 12));

        // Calculate aspect-ratio preserved display rect respecting anamorphic de-squeeze
        let stretch = if video.stretch_factor > 0.05 { video.stretch_factor } else { 1.0 };
        let aspect = if video.height > 0 {
            (video.width as f32 * stretch) / video.height as f32
        } else {
            16.0 / 9.0
        };

        // Fit dimensions: maximum rectangle inside canvas `rect` preserving aspect ratio
        let mut fit_w = rect.width();
        let mut fit_h = fit_w / aspect;
        if fit_h > rect.height() {
            fit_h = rect.height();
            fit_w = fit_h * aspect;
        }

        // Double-click canvas toggles Fit <-> 100%
        if canvas_response.double_clicked() {
            if self.zoom_mode == ZoomMode::Fit {
                self.zoom_mode = ZoomMode::Scale(1.0);
                self.pan_offset = Vec2::ZERO;
            } else {
                self.zoom_mode = ZoomMode::Fit;
                self.pan_offset = Vec2::ZERO;
            }
        }

        // Mouse scroll wheel zooms in / out
        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if canvas_response.hovered() && scroll_delta.abs() > 0.5 {
            let current_scale = match self.zoom_mode {
                ZoomMode::Fit => fit_w / (video.width as f32 * stretch).max(1.0),
                ZoomMode::Scale(s) => s,
            };
            let zoom_factor = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
            let new_scale = (current_scale * zoom_factor).clamp(0.15, 8.0);
            self.zoom_mode = ZoomMode::Scale(new_scale);
        }

        // Current display dimensions based on zoom mode
        let (display_w, display_h) = match self.zoom_mode {
            ZoomMode::Fit => (fit_w, fit_h),
            ZoomMode::Scale(scale) => {
                (video.width as f32 * stretch * scale, video.height as f32 * scale)
            }
        };

        // Pan handling when zoomed in
        if matches!(self.zoom_mode, ZoomMode::Scale(_)) {
            if canvas_response.dragged() {
                self.pan_offset += canvas_response.drag_delta();
            }
            if canvas_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if canvas_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        } else {
            self.pan_offset = Vec2::ZERO;
        }

        // Clamp pan offset so footage remains visible within the canvas
        let max_pan_x = (display_w - rect.width()).max(0.0) * 0.5 + 80.0;
        let max_pan_y = (display_h - rect.height()).max(0.0) * 0.5 + 80.0;
        self.pan_offset.x = self.pan_offset.x.clamp(-max_pan_x, max_pan_x);
        self.pan_offset.y = self.pan_offset.y.clamp(-max_pan_y, max_pan_y);

        let display_center = rect.center() + self.pan_offset;
        let display_rect = Rect::from_center_size(display_center, Vec2::new(display_w, display_h));

        // Strictly bound the rendering viewport callback inside canvas `rect` to eliminate overflow
        let cb_rect = display_rect.intersect(rect);

        // Fetch current frame or fallback to last displayed frame (prevents flickering during scrub/buffer)
        let frame_to_show = if let Some(frame) = playback.get(self.current_frame) {
            self.last_shown_frame = Some(frame.clone());
            Some(frame)
        } else {
            self.last_shown_frame.clone()
        };

        // Render decoded frame via GPU WGSL shader callback
        if let Some(frame) = frame_to_show {
            if cb_rect.width() > 0.5 && cb_rect.height() > 0.5 {
                // Calculate visible texture sub-rectangle in normalized coordinates [0..1]
                let u_start = (cb_rect.min.x - display_rect.min.x) / display_rect.width().max(1.0);
                let v_start = (cb_rect.min.y - display_rect.min.y) / display_rect.height().max(1.0);
                let u_size = cb_rect.width() / display_rect.width().max(1.0);
                let v_size = cb_rect.height() / display_rect.height().max(1.0);

                let mut m = camera_to_srgb(&playback.source.clip.meta);

                // Apply White Balance gains (Temperature & Tint)
                let t = self.wb_temperature / 50.0;
                let r_gain = (2.0f32).powf(t * 0.45);
                let b_gain = (2.0f32).powf(-t * 0.45);

                let g = self.wb_tint / 50.0;
                let g_gain = (2.0f32).powf(-g * 0.45);

                for row in &mut m {
                    row[0] *= r_gain;
                    row[1] *= g_gain;
                    row[2] *= b_gain;
                }

                let (rx, ry) = playback.source.clip.meta.red_offset();
                let exp_mult = (2.0f32).powf(self.exposure_ev);

                let uniforms = ViewUniforms {
                    m0: [m[0][0], m[0][1], m[0][2], 0.0],
                    m1: [m[1][0], m[1][1], m[1][2], 0.0],
                    m2: [m[2][0], m[2][1], m[2][2], 0.0],
                    levels: [frame.black, frame.white, exp_mult, 1.0],
                    info: [rx, ry, frame.width, frame.height],
                    view: [
                        u_start.clamp(0.0, 1.0),
                        v_start.clamp(0.0, 1.0),
                        u_size.clamp(0.0, 1.0),
                        v_size.clamp(0.0, 1.0),
                    ],
                };

                let key = (video.id as u64) << 32 | (frame.index as u64);
                let cb = RawViewCallback {
                    frame,
                    key,
                    uniforms,
                };

                let paint_cb = egui_wgpu::Callback::new_paint_callback(cb_rect, cb);
                painter.add(paint_cb);
            }
        } else {
            // Initial placeholder before first frame completes decode
            painter.rect(
                display_rect.intersect(rect),
                Rounding::ZERO,
                Color32::from_rgb(16, 16, 20),
                Stroke::new(1.0_f32, BORDER_SUBTLE),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("⏳ Pre-buffering Clip..."),
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
