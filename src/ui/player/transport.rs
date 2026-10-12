use super::types::{PlayerAction, PlayerView};
use crate::db::{PickFlag, VideoRecord};
use crate::playback::Playback;
use crate::ui::playbar::Playbar;
use crate::ui::theme::*;
use egui::{Color32, RichText, Ui};
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

        ui.group(|ui| {
            // A. Full-Width Professional Scrubber Playbar
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

            ui.add_space(4.0);

            // B. Transport Controls Row
            ui.horizontal(|ui| {
                // Left: Timecode Readout & Frame Counter
                let cur_time = if video.fps > 0.0 {
                    view.current_frame as f32 / video.fps
                } else {
                    0.0
                };
                let cm = (cur_time / 60.0) as u32;
                let cs = (cur_time % 60.0) as u32;
                let cf = (view.current_frame % (video.fps as usize).max(1)) as u32;

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
                    RichText::new(format!("[{}/{}]", view.current_frame + 1, total_frames))
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
                    view.current_frame = 0;
                    action = Some(PlayerAction::Seek(0));
                }

                // Step -1 Frame
                if ui.button(RichText::new("◀").size(13.0)).on_hover_text("Step -1 Frame [Left Arrow]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::ArrowLeft))
                {
                    if view.current_frame > 0 {
                        view.current_frame -= 1;
                        action = Some(PlayerAction::Seek(view.current_frame));
                    }
                }

                // PLAY / PAUSE (Prominent Studio Button)
                let play_icon = if view.is_playing { " ⏸ PAUSE " } else { " ▶ PLAY  " };
                let play_bg = if view.is_playing {
                    Color32::from_rgb(59, 130, 246)
                } else {
                    Color32::from_rgb(38, 42, 52)
                };
                let play_btn = egui::Button::new(RichText::new(play_icon).strong().size(13.0).color(Color32::WHITE))
                    .fill(play_bg);

                if ui.add(play_btn).on_hover_text("Play / Pause [Space]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::Space))
                {
                    view.is_playing = !view.is_playing;
                    view.last_frame_time = Instant::now();
                }

                // Step +1 Frame
                if ui.button(RichText::new("▶").size(13.0)).on_hover_text("Step +1 Frame [Right Arrow]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::ArrowRight))
                {
                    if view.current_frame + 1 < total_frames {
                        view.current_frame += 1;
                        action = Some(PlayerAction::Seek(view.current_frame));
                    }
                }

                // Jump to End
                if ui.button(RichText::new("⏭").size(13.0)).on_hover_text("Jump to End [End]").clicked()
                    || ui.input(|i| i.key_pressed(egui::Key::End))
                {
                    view.current_frame = total_frames.saturating_sub(1);
                    action = Some(PlayerAction::Seek(view.current_frame));
                }

                ui.separator();

                // Loop Toggle
                let loop_col = if view.is_looping {
                    ACCENT_BLUE
                } else {
                    TEXT_MUTED
                };
                if ui
                    .selectable_label(view.is_looping, RichText::new("🔁 Loop").color(loop_col))
                    .clicked()
                {
                    view.is_looping = !view.is_looping;
                }

                // Speed Selector Pills (0.5x, 1x, 2x, 4x)
                ui.separator();
                ui.label(RichText::new("Speed:").small().color(TEXT_SECONDARY));
                for spd in [0.5, 1.0, 2.0, 4.0] {
                    let is_active = (view.playback_speed - spd).abs() < 0.05;
                    if ui.selectable_label(is_active, format!("{}x", spd)).clicked() {
                        view.playback_speed = spd;
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
