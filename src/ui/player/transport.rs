use super::types::{PlayerAction, PlayerView};
use crate::db::VideoRecord;
use crate::playback::Playback;
use crate::ui::playbar::Playbar;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, RichText, Rounding, Stroke, Ui, Vec2};
use std::time::Instant;

pub struct TransportBar;

impl TransportBar {
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: &Playback,
        total_frames: usize,
    ) -> Option<PlayerAction> {
        let mut action = None;

        egui::Frame::none()
            .fill(ui.visuals().panel_fill)
            .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            .show(ui, |ui| {
                // A. Scrubber Playbar
                let cached_frames = playback.cached_frames();
                if let Some(new_frame) = Playbar::show(
                    ui,
                    "player_main_playbar",
                    view.current_frame,
                    total_frames,
                    video.fps,
                    &cached_frames,
                ) {
                    view.current_frame = new_frame;
                    action = Some(PlayerAction::Seek(view.current_frame));
                }

                ui.add_space(2.0);

                // B. Transport Controls Row
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

                    // Left: Timecode Readout & Frame Counter
                    let tc_cur = widgets::timecode(view.current_frame, video.fps);
                    let tc_tot = widgets::timecode(total_frames, video.fps);

                    ui.label(
                        RichText::new(tc_cur)
                            .monospace()
                            .strong()
                            .size(13.5)
                            .color(text_primary(ui)),
                    );
                    ui.label(
                        RichText::new(format!("/  {}", tc_tot))
                            .monospace()
                            .size(FONT_SMALL)
                            .color(text_muted(ui)),
                    );
                    ui.label(
                        RichText::new(format!("[{}/{}]", view.current_frame + 1, total_frames))
                            .monospace()
                            .size(FONT_CAPTION)
                            .color(text_secondary(ui)),
                    );

                    ui.add_space(12.0);
                    widgets::toolbar_divider(ui);

                    // Center: Transport Buttons
                    // Jump to Start
                    if widgets::icon_button(ui, icon::SKIP_BACK, false, "Jump to Start [Home]").clicked() {
                        view.current_frame = 0;
                        action = Some(PlayerAction::Seek(0));
                    }

                    // Step -1 Frame
                    if widgets::icon_button(ui, icon::CARET_LEFT, false, "Step -1 Frame [Left Arrow]").clicked() {
                        if view.current_frame > 0 {
                            view.current_frame -= 1;
                            action = Some(PlayerAction::Seek(view.current_frame));
                        }
                    }

                    // PLAY / PAUSE (Main Button)
                    let play_glyph = if view.is_playing { icon::PAUSE } else { icon::PLAY };
                    let play_tip = if view.is_playing { "Pause [Space]" } else { "Play [Space]" };
                    let play_btn = egui::Button::new(
                        RichText::new(play_glyph)
                            .size(15.0)
                            .color(if view.is_playing { Color32::WHITE } else { text_primary(ui) }),
                    )
                    .fill(if view.is_playing { ACCENT } else { bg_card(ui) })
                    .stroke(if view.is_playing { Stroke::new(1.0_f32, ACCENT_HOVER) } else { Stroke::new(1.0_f32, border_subtle(ui)) })
                    .rounding(Rounding::same(ROUNDING))
                    .min_size(Vec2::new(32.0, CONTROL_HEIGHT));

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

                    // Jump to End
                    if widgets::icon_button(ui, icon::SKIP_FORWARD, false, "Jump to End [End]").clicked() {
                        view.current_frame = total_frames.saturating_sub(1);
                        action = Some(PlayerAction::Seek(view.current_frame));
                    }

                    widgets::toolbar_divider(ui);

                    // Loop Toggle
                    if widgets::icon_button(ui, icon::REPEAT, view.is_looping, "Toggle Loop").clicked() {
                        view.is_looping = !view.is_looping;
                    }

                    widgets::toolbar_divider(ui);

                    // Speed Selector Pills
                    ui.label(RichText::new("Speed:").size(FONT_SMALL).color(text_muted(ui)));
                    for spd in [0.5, 1.0, 2.0, 4.0] {
                        let is_active = (view.playback_speed - spd).abs() < 0.05;
                        if widgets::toggle_button(ui, is_active, format!("{}x", spd)).clicked() {
                            view.playback_speed = spd;
                            if let Ok(mut a) = playback.audio.lock() {
                                a.set_speed(spd);
                            }
                        }
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

                        if widgets::icon_button(ui, mute_glyph, view.is_muted, "Toggle Audio Mute").clicked() {
                            view.is_muted = !view.is_muted;
                            if let Ok(mut a) = playback.audio.lock() {
                                a.set_muted(view.is_muted);
                            }
                        }

                        let mut vol = view.volume;
                        if ui.add_sized([64.0, 18.0], egui::Slider::new(&mut vol, 0.0..=1.0).show_value(false))
                            .on_hover_text(format!("Audio Volume: {}%", (view.volume * 100.0).round()))
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
                        ui.label(RichText::new("No Audio").size(FONT_CAPTION).color(text_muted(ui)));
                    }

                    // Right side: Quick Rating & Pick buttons on player bar!
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.add_space(4.0);

                        // Pick / Reject Flags
                        if let Some(new_flag) = widgets::flag_buttons(ui, video.flag, true) {
                            action = Some(PlayerAction::SetFlag(new_flag));
                        }

                        widgets::toolbar_divider(ui);

                        // Rating stars
                        if let Some(new_r) = widgets::rating_stars(ui, video.rating, 14.0) {
                            action = Some(PlayerAction::SetRating(new_r));
                        }
                    });
                });
            });

        action
    }
}
