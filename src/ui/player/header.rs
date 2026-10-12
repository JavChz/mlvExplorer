use super::types::{PlayerAction, PlayerView, ZoomMode};
use crate::db::VideoRecord;
use crate::playback::Playback;
use crate::ui::theme::*;
use egui::{Color32, RichText, Ui, Vec2};

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
            if ui.button(RichText::new("⬅ Library [G]").strong()).clicked() {
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

            if video.chunk_count > 1 {
                ui.label(RichText::new("·").color(TEXT_MUTED));
                ui.label(
                    RichText::new(format!("⚡ 5D3 Dual-Card Spanned ({} Chunks)", video.chunk_count))
                        .small()
                        .strong()
                        .color(Color32::from_rgb(245, 158, 11)),
                )
                .on_hover_text("This clip was captured across CF + SD cards (card spanning). All chunks are dynamically interleaved and decoded seamlessly.");
            }

            if !video.camera_name.is_empty() {
                ui.label(RichText::new("·").color(TEXT_MUTED));
                ui.label(RichText::new(&video.camera_name).small().color(TEXT_MUTED));
            }

            if let Some(slate) = video.slate_label() {
                ui.label(RichText::new("·").color(TEXT_MUTED));
                let mut full_slate = format!("🎬 {}", slate);
                if let Some(ref loc) = video.location {
                    if !loc.trim().is_empty() {
                        full_slate.push_str(&format!(" ({})", loc.trim()));
                    }
                }
                ui.label(
                    RichText::new(full_slate)
                        .small()
                        .strong()
                        .color(Color32::from_rgb(245, 158, 11)),
                );
            }

            // Right-aligned Toolbar: Realtime status, Stretch, Fullscreen, Zoom, WB, Exposure, MLVApp
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button(RichText::new("🎬 Open in MLVApp").small().strong().color(Color32::from_rgb(243, 244, 246)))
                    .on_hover_text("Open this clip directly in MLV App for advanced grading & audio export")
                    .clicked()
                {
                    action = Some(PlayerAction::OpenInMlvApp);
                }

                ui.separator();
                ui.label(RichText::new("🚀 GPU Debayer: Direct").small().color(Color32::from_rgb(96, 165, 250)));
                ui.separator();

                // Real-Time / Buffer Health Indicator
                let ready_ahead = playback.ready_ahead(view.current_frame, 24);
                let is_realtime = ready_ahead >= 3 || (!view.is_playing && playback.is_ready(view.current_frame));
                let status_text = if !view.is_playing {
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
                let fs_label = if view.is_fullscreen { "🗗 Exit [F]" } else { "⛶ Fullscreen [F]" };
                if ui.selectable_label(view.is_fullscreen, RichText::new(fs_label).small()).on_hover_text("Toggle Fullscreen Cinema Mode [F]").clicked() {
                    view.is_fullscreen = !view.is_fullscreen;
                    if view.is_fullscreen {
                        view.show_overlay_controls = false;
                        view.zoom_mode = super::types::ZoomMode::Fit;
                        view.pan_offset = egui::Vec2::ZERO;
                    }
                    action = Some(PlayerAction::ToggleFullscreen);
                }

                ui.separator();

                // Anamorphic De-Squeeze (Stretch Factor) Dropdown
                let cur_stretch = video.stretch_factor;
                let stretch_name = match cur_stretch {
                    s if (s - 3.0).abs() < 0.05 => "3.0x (1x3 / 3x1 Anamorphic)",
                    s if (s - 2.0).abs() < 0.05 => "2.0x Anamorphic",
                    s if (s - 1.75).abs() < 0.05 => "1.75x Anamorphic",
                    s if (s - 1.5).abs() < 0.05 => "1.5x Anamorphic",
                    s if (s - 1.333).abs() < 0.05 => "1.33x Anamorphic",
                    s if (s - 1.0).abs() < 0.05 => "1.0x (1:1 Square)",
                    s => &format!("{:.2}x", s),
                };

                egui::ComboBox::from_id_salt("player_desqueeze_select")
                    .selected_text(RichText::new(format!("Stretch: {}", stretch_name)).small())
                    .show_ui(ui, |ui| {
                        for (factor, label) in [
                            (1.0f32, "1.0x (1:1 Square - Off)"),
                            (3.0f32, "3.0x (1x3 / 3x1 Anamorphic / EOS M Crop Mood)"),
                            (2.0f32, "2.0x (Standard 2x Anamorphic)"),
                            (1.75f32, "1.75x Anamorphic"),
                            (1.5f32, "1.5x Anamorphic"),
                            (1.333f32, "1.33x Anamorphic"),
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
                    let cur = match view.zoom_mode {
                        ZoomMode::Fit => 1.0,
                        ZoomMode::Scale(s) => s,
                    };
                    view.zoom_mode = ZoomMode::Scale((cur * 1.25).min(8.0));
                }
                if ui.small_button("-").on_hover_text("Zoom Out [-]").clicked() {
                    let cur = match view.zoom_mode {
                        ZoomMode::Fit => 1.0,
                        ZoomMode::Scale(s) => s,
                    };
                    view.zoom_mode = ZoomMode::Scale((cur * 0.8).max(0.15));
                }

                let zoom_display = match view.zoom_mode {
                    ZoomMode::Fit => "Fit".to_string(),
                    ZoomMode::Scale(s) if (s - 0.5).abs() < 0.02 => "50%".to_string(),
                    ZoomMode::Scale(s) if (s - 1.0).abs() < 0.02 => "100% (1:1)".to_string(),
                    ZoomMode::Scale(s) if (s - 2.0).abs() < 0.02 => "200%".to_string(),
                    ZoomMode::Scale(s) => format!("{:.0}%", s * 100.0),
                };

                egui::ComboBox::from_id_salt("player_zoom_select")
                    .selected_text(RichText::new(format!("🔍 {}", zoom_display)).small())
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(view.zoom_mode == ZoomMode::Fit, "⛶ Fit to Viewport [Z]").clicked() {
                            view.zoom_mode = ZoomMode::Fit;
                            view.pan_offset = Vec2::ZERO;
                        }
                        if ui.selectable_label(matches!(view.zoom_mode, ZoomMode::Scale(s) if (s - 0.5).abs() < 0.02), "50% Half").clicked() {
                            view.zoom_mode = ZoomMode::Scale(0.5);
                        }
                        if ui.selectable_label(matches!(view.zoom_mode, ZoomMode::Scale(s) if (s - 1.0).abs() < 0.02), "100% (1:1 Native Pixel) [Z]").clicked() {
                            view.zoom_mode = ZoomMode::Scale(1.0);
                        }
                        if ui.selectable_label(matches!(view.zoom_mode, ZoomMode::Scale(s) if (s - 2.0).abs() < 0.02), "200% (2:1 Peep)").clicked() {
                            view.zoom_mode = ZoomMode::Scale(2.0);
                        }
                    });

                let is_fit = view.zoom_mode == ZoomMode::Fit;
                if ui.selectable_label(is_fit, RichText::new("⛶ Fit").small()).on_hover_text("Fit frame to canvas [Z]").clicked() {
                    view.zoom_mode = ZoomMode::Fit;
                    view.pan_offset = Vec2::ZERO;
                }

                ui.separator();

                // Exposure EV Slider with quick reset
                if view.exposure_ev != 0.0 && ui.small_button("↺").on_hover_text("Reset Exposure [0]").clicked() {
                    view.exposure_ev = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut view.exposure_ev, -2.5..=2.5)
                        .text("EV")
                        .step_by(0.1)
                        .show_value(true),
                );

                ui.separator();

                // White Balance Tint Slider (Green <-> Magenta)
                if view.wb_tint != 0.0 && ui.small_button("↺").on_hover_text("Reset Tint [0]").clicked() {
                    view.wb_tint = 0.0;
                }
                ui.add(
                    egui::Slider::new(&mut view.wb_tint, -50.0..=50.0)
                        .text("Tint")
                        .step_by(1.0)
                        .show_value(true),
                );

                // White Balance Temperature Slider (Cool <-> Warm)
                if view.wb_temperature != 0.0 && ui.small_button("↺").on_hover_text("Reset Temperature [0]").clicked() {
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
