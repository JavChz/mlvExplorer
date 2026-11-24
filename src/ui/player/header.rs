use super::types::{PlayerAction, PlayerView, PreviewLook, ZoomMode};
use crate::db::VideoRecord;
use crate::mlv::FocusPixelMode;
use crate::playback::{Playback, PlaybackSnapshot};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, RichText, Ui, Vec2};

pub struct HeaderBar;

impl HeaderBar {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: Option<&Playback>,
        snapshot: Option<&PlaybackSnapshot>,
        load_error: Option<&str>,
        clip_pos: Option<(usize, usize)>,
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

            if ui.add(back_btn).on_hover_text("Return to Grid [Mouse Back / Alt+Left / Esc / G]").clicked() {
                action = Some(PlayerAction::BackToGrid);
            }

            // Prev / Next Clip navigation
            if let Some((pos, total)) = clip_pos {
                if total > 1 {
                    widgets::toolbar_divider(ui);
                    ui.add_enabled_ui(pos > 1, |ui| {
                        if widgets::icon_button(ui, icon::CARET_DOUBLE_LEFT, false, "Previous Clip [[ / PgUp]").clicked() {
                            action = Some(PlayerAction::PrevClip);
                        }
                    });
                    ui.label(
                        RichText::new(format!("{}/{}", pos, total))
                            .monospace()
                            .size(FONT_CAPTION)
                            .color(text_secondary(ui)),
                    );
                    ui.add_enabled_ui(pos < total, |ui| {
                        if widgets::icon_button(ui, icon::CARET_DOUBLE_RIGHT, false, "Next Clip [] / PgDn]").clicked() {
                            action = Some(PlayerAction::NextClip);
                        }
                    });
                }
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

            // Right-aligned Toolbar: MLVApp, Fullscreen, Perf HUD badge, Look Popover, Focus Pixels, Rotate, Desqueeze, Zoom
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                if widgets::primary_button(ui, widgets::icon_label(icon::ARROW_SQUARE_OUT, "MLV App"))
                    .on_hover_text("Open directly in MLV App for grading & export [Ctrl+M]")
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
                        view.zoom_mode = ZoomMode::Fit;
                        view.pan_offset = Vec2::ZERO;
                    }
                    action = Some(PlayerAction::ToggleFullscreen);
                }

                widgets::toolbar_divider(ui);

                // Clickable Buffer / Performance HUD badge
                let (status_text, status_col) = if load_error.is_some() {
                    ("OPEN ERROR".to_string(), FLAG_REJECTED)
                } else if let Some(snap) = snapshot {
                    if !view.is_playing {
                        (format!("READY ({})", snap.cache_count), text_secondary(ui))
                    } else if view.is_buffering {
                        (format!("BUFFERING ({})", snap.ready_ahead), WARNING)
                    } else {
                        (format!("REAL-TIME · {:.0}fps", snap.decode_fps), FLAG_PICKED)
                    }
                } else {
                    ("LOADING CLIP…".to_string(), ACCENT_HOVER)
                };
                if ui
                    .add(
                        egui::Button::new(RichText::new(status_text).size(FONT_CAPTION).color(status_col))
                            .fill(if view.show_perf_hud { ACCENT_DIM } else { Color32::TRANSPARENT })
                            .stroke(if view.show_perf_hud {
                                egui::Stroke::new(1.0_f32, ACCENT)
                            } else {
                                egui::Stroke::NONE
                            })
                            .min_size(Vec2::new(0.0, CONTROL_HEIGHT)),
                    )
                    .on_hover_text("Click to toggle Performance Telemetry HUD [Shift+D]")
                    .clicked()
                {
                    view.show_perf_hud = !view.show_perf_hud;
                    action = Some(PlayerAction::TogglePerfHud(view.show_perf_hud));
                }

                widgets::toolbar_divider(ui);

                // Preview Look Popover (Exposure, WB, Highlight Roll-off, Zebras, Focus Peaking)
                let look_active = view.look.is_modified();
                let look_btn = ui
                    .add(
                        egui::Button::new(
                            RichText::new(widgets::icon_label(icon::SLIDERS_HORIZONTAL, "Look"))
                                .size(FONT_SMALL)
                                .color(if look_active { Color32::WHITE } else { text_secondary(ui) }),
                        )
                        .fill(if look_active { ACCENT_DIM } else { Color32::TRANSPARENT })
                        .stroke(if look_active {
                            egui::Stroke::new(1.0_f32, ACCENT)
                        } else {
                            egui::Stroke::NONE
                        })
                        .min_size(Vec2::new(0.0, CONTROL_HEIGHT)),
                    )
                    .on_hover_text("Non-destructive Preview Look (Exposure, White Balance, Roll-off, Zebras, Peaking)");

                let look_pop_id = ui.make_persistent_id("player_look_popover");
                if look_btn.clicked() {
                    ui.memory_mut(|m| m.toggle_popup(look_pop_id));
                }
                egui::popup_below_widget(ui, look_pop_id, &look_btn, egui::PopupCloseBehavior::CloseOnClickOutside, |ui| {
                    ui.set_min_width(250.0);
                    ui.spacing_mut().item_spacing.y = 6.0;
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Preview Look (Non-Destructive)").strong().size(FONT_SMALL));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("Reset").clicked() {
                                let rolloff = view.look.highlight_rolloff;
                                view.look = PreviewLook {
                                    highlight_rolloff: rolloff,
                                    ..Default::default()
                                };
                            }
                        });
                    });
                    ui.separator();

                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Exposure:").size(FONT_SMALL).color(text_secondary(ui)));
                        ui.add(
                            egui::Slider::new(&mut view.look.exposure_ev, -3.0..=3.0)
                                .step_by(0.05)
                                .suffix(" EV"),
                        );
                    });

                    ui.checkbox(&mut view.look.wb_override, "Custom White Balance");
                    ui.add_enabled_ui(view.look.wb_override, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Temp:").size(FONT_SMALL).color(text_secondary(ui)));
                            ui.add(
                                egui::Slider::new(&mut view.look.wb_kelvin, 2500.0..=10000.0)
                                    .step_by(50.0)
                                    .suffix(" K"),
                            );
                        });
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Tint:").size(FONT_SMALL).color(text_secondary(ui)));
                            ui.add(egui::Slider::new(&mut view.look.wb_tint, -50.0..=50.0).step_by(1.0));
                        });
                    });

                    ui.separator();
                    if ui.checkbox(&mut view.look.highlight_rolloff, "Soft Highlight Roll-off").changed() {
                        action = Some(PlayerAction::SetHighlightRolloff(view.look.highlight_rolloff));
                    }
                    ui.checkbox(&mut view.look.zebras, "Clip Zebras (>= 98%)");
                    ui.checkbox(&mut view.look.focus_peaking, "Focus Peaking (Edge Highlight)");
                });

                // Focus Pixel Removal toggle button (Auto / FPM Only / Off)
                let fp_active = view.focus_pixel_mode != FocusPixelMode::Off;
                let fp_tip = {
                    let fpm_note = playback
                        .and_then(|p| p.source.focus_pixel_map.as_ref())
                        .map(|m| format!("Loaded .fpm map: {} ({} pixels)", m.source_name, m.pixel_count))
                        .unwrap_or_else(|| {
                            if playback.map_or(false, |p| p.source.is_pdaf_camera) {
                                "Canon Hybrid CMOS AF sensor detected — GPU Auto PDAF Heal active".to_string()
                            } else {
                                "No .fpm map found — GPU Auto bad/hot-pixel guard active".to_string()
                            }
                        });
                    format!(
                        "Focus Pixel Removal: {} [H to cycle]\n{}",
                        view.focus_pixel_mode.label(),
                        fpm_note
                    )
                };
                if widgets::toggle_button(ui, fp_active, widgets::icon_label(icon::SPARKLE, view.focus_pixel_mode.short_label()))
                    .on_hover_text(fp_tip)
                    .clicked()
                {
                    let next = view.focus_pixel_mode.next();
                    view.focus_pixel_mode = next;
                    view.trigger_status_cue(format!("Focus Pixels: {}", next.label()));
                    action = Some(PlayerAction::SetFocusPixelMode(next));
                }

                // Rotate 90 deg CW button
                let rot_label = if video.rotation % 360 != 0 {
                    format!("{}°", video.rotation % 360)
                } else {
                    String::new()
                };
                let rot_tip = if rot_label.is_empty() {
                    "Rotate 90° Clockwise [R]".to_string()
                } else {
                    format!("Rotate 90° Clockwise (currently {}) [R]", rot_label)
                };
                if widgets::icon_button(ui, icon::ARROW_CLOCKWISE, video.rotation % 360 != 0, &rot_tip).clicked() {
                    let next_rot = (video.rotation + 90) % 360;
                    view.trigger_status_cue(format!("Rotation: {}°", next_rot));
                    action = Some(PlayerAction::SetRotation(next_rot));
                }

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
            });
        });

        action
    }
}
