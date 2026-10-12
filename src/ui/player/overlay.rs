use super::types::{PlayerAction, PlayerView};
use crate::db::VideoRecord;
use crate::playback::Playback;
use crate::ui::playbar::Playbar;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{
    Color32, Frame, Margin, Pos2, Rect, RichText, Rounding, Stroke, Ui, Vec2,
};
use std::time::Instant;

pub struct FullscreenOverlay;

impl FullscreenOverlay {
    /// Renders a modern floating translucent HUD centered at the bottom of the screen
    /// with playback controls (play/pause, fast reverse, fast forward, step, timecode, volume, loop).
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: &Playback,
        total_frames: usize,
        alpha: f32,
    ) -> Option<PlayerAction> {
        if alpha <= 0.001 {
            return None;
        }

        let mut action = None;
        let screen_rect = ui.ctx().screen_rect();
        let overlay_w = 660.0f32.min(screen_rect.width() - 32.0);
        let overlay_h = 76.0;

        // Position bottom-center with padding from bottom edge
        let center_x = screen_rect.center().x;
        let bottom_y = screen_rect.max.y - 28.0;
        let overlay_rect = Rect::from_min_size(
            Pos2::new(center_x - overlay_w * 0.5, bottom_y - overlay_h),
            Vec2::new(overlay_w, overlay_h),
        );

        // Alpha-modulated colors
        let bg_color = Color32::from_rgba_premultiplied(
            (16.0 * alpha) as u8,
            (18.0 * alpha) as u8,
            (22.0 * alpha) as u8,
            (230.0 * alpha) as u8,
        );
        let border_color = Color32::from_rgba_premultiplied(
            (60.0 * alpha) as u8,
            (64.0 * alpha) as u8,
            (74.0 * alpha) as u8,
            (180.0 * alpha) as u8,
        );
        let shadow_color = Color32::from_rgba_premultiplied(
            0,
            0,
            0,
            (120.0 * alpha) as u8,
        );

        // Draw shadow and rounded capsule container
        let painter = ui.painter();
        painter.rect_filled(
            overlay_rect.expand(2.0),
            Rounding::same(12.0),
            shadow_color,
        );
        painter.rect(
            overlay_rect,
            Rounding::same(12.0),
            bg_color,
            Stroke::new(1.0_f32, border_color),
        );

        // Allocate a child UI specifically inside the overlay rectangle
        let mut overlay_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(overlay_rect)
                .layout(egui::Layout::top_down(egui::Align::Center)),
        );

        Frame::none()
            .inner_margin(Margin::symmetric(14.0, 8.0))
            .show(&mut overlay_ui, |ui| {
                // 1. Sleek Scrubber Bar
                let cached_frames = playback.cached_frames();
                if let Some(new_frame) = Playbar::show(
                    ui,
                    "fs_overlay_playbar",
                    view.current_frame,
                    total_frames,
                    video.fps,
                    &cached_frames,
                ) {
                    view.current_frame = new_frame;
                    action = Some(PlayerAction::Seek(view.current_frame));
                }

                ui.add_space(4.0);

                // 2. Transport & Audio Controls Row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(6.0, 0.0);

                    // Left: Timecode readout
                    let tc_cur = widgets::timecode(view.current_frame, video.fps);
                    let tc_tot = widgets::timecode(total_frames, video.fps);
                    ui.label(
                        RichText::new(tc_cur)
                            .monospace()
                            .strong()
                            .size(13.0)
                            .color(Color32::WHITE),
                    );
                    ui.label(
                        RichText::new(format!("/ {}", tc_tot))
                            .monospace()
                            .size(FONT_SMALL)
                            .color(TEXT_MUTED),
                    );

                    ui.add_space(8.0);
                    widgets::toolbar_divider(ui);
                    ui.add_space(4.0);

                    // Center transport buttons:
                    let jump_step = (video.fps.round() as usize).max(1);

                    // Fast reverse button (Jump back 1 sec or 24 frames)
                    if ui.add(
                        egui::Button::new(
                            RichText::new(format!("{} 1s", icon::CARET_DOUBLE_LEFT))
                                .size(12.0)
                                .color(TEXT_PRIMARY),
                        )
                        .fill(BG_CARD)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(36.0, 22.0)),
                    )
                    .on_hover_text("Fast Reverse (1 sec) [Shift+Left]")
                    .clicked()
                    {
                        view.current_frame = view.current_frame.saturating_sub(jump_step);
                        action = Some(PlayerAction::Seek(view.current_frame));
                    }

                    // Step -1 Frame
                    if widgets::icon_button(ui, icon::CARET_LEFT, false, "Step -1 Frame [Left Arrow]").clicked() {
                        if view.current_frame > 0 {
                            view.current_frame -= 1;
                            action = Some(PlayerAction::Seek(view.current_frame));
                        }
                    }

                    // PLAY / PAUSE (Prominent Modern Center Button)
                    let play_glyph = if view.is_playing { icon::PAUSE } else { icon::PLAY };
                    let play_tip = if view.is_playing { "Pause [Space]" } else { "Play [Space]" };
                    let play_btn = egui::Button::new(
                        RichText::new(play_glyph)
                            .size(16.0)
                            .color(Color32::WHITE),
                    )
                    .fill(if view.is_playing { ACCENT } else { BG_CARD })
                    .stroke(if view.is_playing {
                        Stroke::new(1.0_f32, ACCENT_HOVER)
                    } else {
                        Stroke::new(1.0_f32, BORDER_SUBTLE)
                    })
                    .rounding(Rounding::same(ROUNDING))
                    .min_size(Vec2::new(34.0, 24.0));

                    if ui.add(play_btn).on_hover_text(play_tip).clicked() {
                        view.is_playing = !view.is_playing;
                        view.last_frame_time = Instant::now();
                        if view.is_playing && view.current_frame + 1 >= total_frames {
                            view.current_frame = 0;
                        }
                        let time_sec = view.current_frame as f32 / video.fps.max(1.0);
                        if let Ok(mut a) = playback.audio.lock() {
                            if view.is_playing {
                                a.play(time_sec);
                            } else {
                                a.pause();
                            }
                        }
                    }

                    // Step +1 Frame
                    if widgets::icon_button(ui, icon::CARET_RIGHT, false, "Step +1 Frame [Right Arrow]").clicked() {
                        if view.current_frame + 1 < total_frames {
                            view.current_frame += 1;
                            action = Some(PlayerAction::Seek(view.current_frame));
                        }
                    }

                    // Fast Forward button (Jump forward 1 sec)
                    if ui.add(
                        egui::Button::new(
                            RichText::new(format!("1s {}", icon::CARET_DOUBLE_RIGHT))
                                .size(12.0)
                                .color(TEXT_PRIMARY),
                        )
                        .fill(BG_CARD)
                        .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(36.0, 22.0)),
                    )
                    .on_hover_text("Fast Forward (1 sec) [Shift+Right]")
                    .clicked()
                    {
                        if view.current_frame + jump_step < total_frames {
                            view.current_frame += jump_step;
                        } else {
                            view.current_frame = total_frames.saturating_sub(1);
                        }
                        action = Some(PlayerAction::Seek(view.current_frame));
                    }

                    // Loop Toggle
                    if widgets::icon_button(ui, icon::REPEAT, view.is_looping, "Toggle Loop").clicked() {
                        view.is_looping = !view.is_looping;
                    }

                    widgets::toolbar_divider(ui);

                    // Audio Volume & Mute Controls
                    let has_audio = playback.audio.lock().map(|a| a.has_audio()).unwrap_or(false);
                    if has_audio {
                        let mute_glyph = if view.is_muted || view.volume == 0.0 {
                            icon::SPEAKER_SLASH
                        } else if view.volume < 0.5 {
                            icon::SPEAKER_LOW
                        } else {
                            icon::SPEAKER_HIGH
                        };

                        if widgets::icon_button(ui, mute_glyph, view.is_muted, "Toggle Audio Mute [M]").clicked() {
                            view.is_muted = !view.is_muted;
                            if let Ok(mut a) = playback.audio.lock() {
                                a.set_muted(view.is_muted);
                            }
                        }

                        let mut vol = view.volume;
                        if ui
                            .add_sized([56.0, 18.0], egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false))
                            .on_hover_text(format!("Volume: {}%", (view.volume * 100.0).round()))
                            .changed()
                        {
                            view.volume = vol;
                            if view.is_muted && vol > 0.0 {
                                view.is_muted = false;
                            }
                            if let Ok(mut a) = playback.audio.lock() {
                                a.set_volume(view.volume);
                                a.set_muted(view.is_muted);
                            }
                        }
                    } else {
                        ui.label(RichText::new("No Audio").size(FONT_CAPTION).color(TEXT_MUTED));
                    }

                    // Right side: Exit fullscreen button
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if widgets::icon_button(ui, icon::CORNERS_IN, false, "Exit Fullscreen [Esc / F]").clicked() {
                            view.is_fullscreen = false;
                            action = Some(PlayerAction::ToggleFullscreen);
                        }
                    });
                });
            });

        action
    }
}
