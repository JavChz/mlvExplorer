use super::types::{PlayerAction, PlayerView, ZoomMode};
use crate::db::VideoRecord;
use crate::playback::Playback;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{RichText, Ui, Vec2};

pub struct HeaderBar;

impl HeaderBar {
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: &Playback,
    ) -> Option<PlayerAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

            let back_btn = egui::Button::new(
                RichText::new(format!("{}  Grid", icon::CARET_LEFT))
                    .size(FONT_BODY)
                    .color(text_primary(ui)),
            )
            .min_size(Vec2::new(0.0, CONTROL_HEIGHT));

            if ui.add(back_btn).on_hover_text("Return to Grid [G]").clicked() {
                action = Some(PlayerAction::BackToGrid);
            }

            widgets::toolbar_divider(ui);

            // Clip Name
            ui.label(RichText::new(&video.file_name).strong().size(FONT_BODY).color(text_primary(ui)));

            // Metadata Badges
            ui.label(RichText::new(format!("{} × {}", video.width, video.height)).size(FONT_SMALL).color(text_muted(ui)));
            ui.label(RichText::new("·").color(text_muted(ui)));
            ui.label(RichText::new(widgets::format_fps(video.fps)).size(FONT_SMALL).color(text_muted(ui)));
            ui.label(RichText::new("·").color(text_muted(ui)));
            ui.label(RichText::new(&video.codec).size(FONT_SMALL).color(text_muted(ui)));

            if video.chunk_count > 1 {
                ui.label(RichText::new("·").color(text_muted(ui)));
                ui.label(
                    RichText::new(format!("{} Spanned ({} files)", icon::STACK, video.chunk_count))
                        .size(FONT_SMALL)
                        .color(WARNING),
                )
                .on_hover_text(format!(
                    "Card-spanned footage: recorded across {} files (.MLV, .M00…)\nAll chunks are interleaved and played back seamlessly.",
                    video.chunk_count
                ));
            }

            if let Some(slate) = video.slate_label() {
                ui.label(RichText::new("·").color(text_muted(ui)));
                ui.label(
                    RichText::new(format!("{} {}", icon::FILM_STRIP, slate))
                        .size(FONT_SMALL)
                        .color(WARNING),
                );
            }

            // Right-aligned Toolbar: Realtime status, Stretch, Fullscreen, Zoom, WB, Exposure, MLVApp
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                if widgets::button(ui, widgets::icon_label(icon::ARROW_SQUARE_OUT, "MLV App"))
                    .on_hover_text("Open directly in MLV App for grading & export")
                    .clicked()
                {
                    action = Some(PlayerAction::OpenInMlvApp);
                }

                widgets::toolbar_divider(ui);

                // Fullscreen toggle button
                let fs_icon = if view.is_fullscreen { icon::CORNERS_IN } else { icon::CORNERS_OUT };
                let fs_tip = if view.is_fullscreen { "Exit Fullscreen [F]" } else { "Enter Fullscreen [F]" };
                if widgets::icon_button(ui, fs_icon, view.is_fullscreen, fs_tip).clicked() {
                    view.is_fullscreen = !view.is_fullscreen;
                    if view.is_fullscreen {
                        view.show_overlay_controls = false;
                        view.zoom_mode = super::types::ZoomMode::Fit;
                        view.pan_offset = egui::Vec2::ZERO;
                    }
                    action = Some(PlayerAction::ToggleFullscreen);
                }

                widgets::toolbar_divider(ui);

                // Buffer status badge
                let ready_ahead = playback.ready_ahead(view.current_frame, 24);
                let is_realtime = ready_ahead >= 3 || (!view.is_playing && playback.is_ready(view.current_frame));
                let (status_text, status_col) = if !view.is_playing {
                    (format!("READY ({})", playback.cache_count()), text_secondary(ui))
                } else if is_realtime {
                    ("REAL-TIME".to_string(), FLAG_PICKED)
                } else {
                    ("BUFFERING".to_string(), WARNING)
                };
                ui.label(RichText::new(status_text).size(FONT_CAPTION).color(status_col));

                widgets::toolbar_divider(ui);

                // Anamorphic De-Squeeze Dropdown
                if let Some(factor) = widgets::desqueeze_combo(ui, "player_desqueeze_combo", video.stretch_factor, 100.0) {
                    action = Some(PlayerAction::SetStretch(factor));
                }

                widgets::toolbar_divider(ui);

                // Zoom Controls
                if widgets::icon_button(ui, icon::PLUS, false, "Zoom In [+]").clicked() {
                    let cur = match view.zoom_mode {
                        ZoomMode::Fit => 1.0,
                        ZoomMode::Scale(s) => s,
                    };
                    view.zoom_mode = ZoomMode::Scale((cur * 1.25).min(8.0));
                }
                if widgets::icon_button(ui, icon::MINUS, false, "Zoom Out [-]").clicked() {
                    let cur = match view.zoom_mode {
                        ZoomMode::Fit => 1.0,
                        ZoomMode::Scale(s) => s,
                    };
                    view.zoom_mode = ZoomMode::Scale((cur * 0.8).max(0.15));
                }

                let zoom_display = match view.zoom_mode {
                    ZoomMode::Fit => "Fit".to_string(),
                    ZoomMode::Scale(s) if (s - 0.5).abs() < 0.02 => "50%".to_string(),
                    ZoomMode::Scale(s) if (s - 1.0).abs() < 0.02 => "100%".to_string(),
                    ZoomMode::Scale(s) if (s - 2.0).abs() < 0.02 => "200%".to_string(),
                    ZoomMode::Scale(s) => format!("{:.0}%", s * 100.0),
                };

                egui::ComboBox::from_id_salt("player_zoom_select")
                    .selected_text(RichText::new(zoom_display).size(FONT_SMALL))
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(view.zoom_mode == ZoomMode::Fit, "Fit Viewport [Z]").clicked() {
                            view.zoom_mode = ZoomMode::Fit;
                            view.pan_offset = Vec2::ZERO;
                        }
                        if ui.selectable_label(matches!(view.zoom_mode, ZoomMode::Scale(s) if (s - 0.5).abs() < 0.02), "50%").clicked() {
                            view.zoom_mode = ZoomMode::Scale(0.5);
                        }
                        if ui.selectable_label(matches!(view.zoom_mode, ZoomMode::Scale(s) if (s - 1.0).abs() < 0.02), "100% (1:1) [Z]").clicked() {
                            view.zoom_mode = ZoomMode::Scale(1.0);
                        }
                        if ui.selectable_label(matches!(view.zoom_mode, ZoomMode::Scale(s) if (s - 2.0).abs() < 0.02), "200%").clicked() {
                            view.zoom_mode = ZoomMode::Scale(2.0);
                        }
                    });

                let is_fit = view.zoom_mode == ZoomMode::Fit;
                if widgets::toggle_button(ui, is_fit, "Fit").on_hover_text("Fit frame [Z]").clicked() {
                    view.zoom_mode = ZoomMode::Fit;
                    view.pan_offset = Vec2::ZERO;
                }

                widgets::toolbar_divider(ui);

                // Exposure EV
                if view.exposure_ev != 0.0 && widgets::icon_button(ui, icon::ARROW_COUNTER_CLOCKWISE, false, "Reset Exposure").clicked() {
                    view.exposure_ev = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut view.exposure_ev, -2.5..=2.5)
                        .text("EV")
                        .step_by(0.1)
                        .show_value(true),
                );

                widgets::toolbar_divider(ui);

                // White Balance Tint
                if view.wb_tint != 0.0 && widgets::icon_button(ui, icon::ARROW_COUNTER_CLOCKWISE, false, "Reset Tint").clicked() {
                    view.wb_tint = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut view.wb_tint, -50.0..=50.0)
                        .text("Tint")
                        .step_by(1.0)
                        .show_value(true),
                );

                // White Balance Temp
                if view.wb_temperature != 0.0 && widgets::icon_button(ui, icon::ARROW_COUNTER_CLOCKWISE, false, "Reset Temp").clicked() {
                    view.wb_temperature = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut view.wb_temperature, -50.0..=50.0)
                        .text("WB")
                        .step_by(1.0)
                        .show_value(true),
                );
            });
        });

        action
    }
}
