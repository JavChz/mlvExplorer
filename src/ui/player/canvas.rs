use super::types::{PlayerAction, PlayerView, ZoomMode};
use crate::db::VideoRecord;
use crate::gpu::{DisplayUniforms, PreprocessUniforms, RawViewCallback};
use crate::mlv::color::wb_multipliers_for_kelvin;
use crate::playback::{Playback, PlaybackSnapshot};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use eframe::egui_wgpu;
use egui::{Color32, FontId, Pos2, Rect, RichText, Rounding, Stroke, Ui, Vec2};
use std::time::Duration;

pub struct Canvas;

impl Canvas {
    #[allow(clippy::too_many_arguments)]
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: Option<&Playback>,
        snapshot: Option<&PlaybackSnapshot>,
        load_error: Option<&str>,
        gpu_info: &str,
        thumb_id: Option<egui::TextureId>,
    ) -> Option<PlayerAction> {
        let mut action = None;
        let bottom_height = if view.is_fullscreen { 0.0 } else { 76.0 };
        let canvas_height = (ui.available_height() - bottom_height).max(50.0);
        let canvas_size = Vec2::new(ui.available_width(), canvas_height);
        let (rect, canvas_response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());

        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, Rounding::ZERO, BG_VIEWER);

        let stretch = if video.stretch_factor > 0.05 { video.stretch_factor } else { 1.0 };
        let rot = video.rotation % 360;
        let (eff_w, eff_h) = if rot == 90 || rot == 270 {
            (video.height as f32, video.width as f32 * stretch)
        } else {
            (video.width as f32 * stretch, video.height as f32)
        };
        let aspect = if eff_h > 0.0 {
            eff_w / eff_h
        } else {
            16.0 / 9.0
        };

        let mut fit_w = rect.width();
        let mut fit_h = fit_w / aspect;
        if fit_h > rect.height() {
            fit_h = rect.height();
            fit_w = fit_h * aspect;
        }

        if load_error.is_none() && canvas_response.double_clicked() {
            view.is_fullscreen = !view.is_fullscreen;
            if view.is_fullscreen {
                view.zoom_mode = ZoomMode::Fit;
                view.pan_offset = Vec2::ZERO;
            }
            view.last_interaction_time = std::time::Instant::now();
        } else if load_error.is_none() && canvas_response.clicked() {
            // Click toggles play / pause in all modes
            if let Some(pb) = playback {
                view.is_playing = !view.is_playing;
                view.reverse_shuttle = false;
                view.clear_buffering();
                view.reanchor_clock();
                view.last_interaction_time = std::time::Instant::now();
                let time_sec = view.current_frame as f32 / video.fps.max(1.0);
                if let Ok(mut a) = pb.audio.lock() {
                    if view.is_playing {
                        a.play(time_sec);
                    } else {
                        a.pause();
                    }
                }
            }
        }

        if load_error.is_none() {
            canvas_response.context_menu(|ui| {
                if ui
                    .button(widgets::icon_label(
                        icon::FOLDER_OPEN,
                        crate::launcher::ExternalLauncher::reveal_label(),
                    ))
                    .clicked()
                {
                    action = Some(PlayerAction::ShowInExplorer);
                    ui.close_menu();
                }
                if ui
                    .button(widgets::icon_label(icon::ARROW_SQUARE_OUT, "Open in MLV App"))
                    .clicked()
                {
                    action = Some(PlayerAction::OpenInMlvApp);
                    ui.close_menu();
                }
            });
        }

        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if canvas_response.hovered() && scroll_delta.abs() > 0.5 {
            let old_scale = match view.zoom_mode {
                ZoomMode::Fit => fit_w / eff_w.max(1.0),
                ZoomMode::Scale(s) => s,
            };
            let zoom_factor = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
            let new_scale = (old_scale * zoom_factor).clamp(0.15, 8.0);
            let ratio = new_scale / old_scale.max(0.0001);
            if let Some(cursor) = ui.input(|i| i.pointer.hover_pos()) {
                view.pan_offset = (cursor - rect.center()) * (1.0 - ratio) + view.pan_offset * ratio;
            }
            view.zoom_mode = ZoomMode::Scale(new_scale);
        }

        let (display_w, display_h) = match view.zoom_mode {
            ZoomMode::Fit => (fit_w, fit_h),
            ZoomMode::Scale(scale) => (eff_w * scale, eff_h * scale),
        };

        if matches!(view.zoom_mode, ZoomMode::Scale(_)) {
            if canvas_response.dragged() {
                view.pan_offset += canvas_response.drag_delta();
            }
            if canvas_response.dragged() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
            } else if canvas_response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Grab);
            }
        } else {
            view.pan_offset = Vec2::ZERO;
        }

        let max_pan_x = (display_w - rect.width()).max(0.0) * 0.5 + 80.0;
        let max_pan_y = (display_h - rect.height()).max(0.0) * 0.5 + 80.0;
        view.pan_offset.x = view.pan_offset.x.clamp(-max_pan_x, max_pan_x);
        view.pan_offset.y = view.pan_offset.y.clamp(-max_pan_y, max_pan_y);

        let display_center = rect.center() + view.pan_offset;
        let display_rect = Rect::from_center_size(display_center, Vec2::new(display_w, display_h));
        let cb_rect = display_rect.intersect(rect);

        let frame_to_show = if let Some(pb) = playback {
            if let Some(frame) = pb.get(view.current_frame) {
                view.last_shown_frame = Some(frame.clone());
                Some((frame, pb))
            } else {
                view.last_shown_frame.clone().map(|f| (f, pb))
            }
        } else {
            None
        };

        let achieved_fps = view.record_playback_frame(view.current_frame);

        if let Some((frame, pb)) = frame_to_show {
            if cb_rect.width() > 0.5 && cb_rect.height() > 0.5 {
                let u_start = (cb_rect.min.x - display_rect.min.x) / display_rect.width().max(1.0);
                let v_start = (cb_rect.min.y - display_rect.min.y) / display_rect.height().max(1.0);
                let u_size = cb_rect.width() / display_rect.width().max(1.0);
                let v_size = cb_rect.height() / display_rect.height().max(1.0);

                let mut m = pb.source.camera_matrix;
                if view.look.wb_override {
                    let wb = wb_multipliers_for_kelvin(view.look.wb_kelvin, view.look.wb_tint);
                    for c in 0..3 {
                        for j in 0..3 {
                            m[c][j] *= wb[c];
                        }
                    }
                } else {
                    let clip_k = pb.source.clip.meta.white_balance_kelvin;
                    if (2200..=12000).contains(&clip_k) {
                        let wb = wb_multipliers_for_kelvin(clip_k as f32, 0.0);
                        for c in 0..3 {
                            for j in 0..3 {
                                m[c][j] *= wb[c];
                            }
                        }
                    }
                }

                let (rx, ry) = pb.source.clip.meta.red_offset();
                let fpm_map = pb.source.focus_pixel_map.clone();
                let fpm_key = if fpm_map.is_some() { video.id as u64 } else { 0 };

                let preprocess = PreprocessUniforms {
                    levels: [
                        frame.black,
                        frame.white,
                        if pb.source.is_pdaf_camera { 1.0 } else { 0.0 },
                        view.focus_pixel_mode.as_u32() as f32,
                    ],
                    info: [rx, ry, frame.width, frame.height],
                    flags: [if fpm_map.is_some() { 1 } else { 0 }, 0, 0, 0],
                };

                let rot_steps = match rot {
                    90 => 1.0,
                    180 => 2.0,
                    270 => 3.0,
                    _ => 0.0,
                };
                let display = DisplayUniforms {
                    m0: [m[0][0], m[0][1], m[0][2], 0.0],
                    m1: [m[1][0], m[1][1], m[1][2], 0.0],
                    m2: [m[2][0], m[2][1], m[2][2], 0.0],
                    params: [
                        2.0_f32.powf(view.look.exposure_ev),
                        1.0, // overwritten by gpu.rs apply_gamma
                        if view.look.highlight_rolloff { 1.0 } else { 0.0 },
                        rot_steps,
                    ],
                    overlays: [
                        if view.look.zebras { 1.0 } else { 0.0 },
                        if view.look.focus_peaking { 1.0 } else { 0.0 },
                        frame.width as f32,
                        frame.height as f32,
                    ],
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
                    fpm_key,
                    fpm_map,
                    preprocess,
                    display,
                };

                let paint_cb = egui_wgpu::Callback::new_paint_callback(cb_rect, cb);
                painter.add(paint_cb);
            }
        } else {
            let inner_rect = display_rect.intersect(rect);
            let is_loading_clip = playback.is_none();

            if let Some(tid) = thumb_id {
                if cb_rect.width() > 0.5 && cb_rect.height() > 0.5 {
                    let u_start = (cb_rect.min.x - display_rect.min.x) / display_rect.width().max(1.0);
                    let v_start = (cb_rect.min.y - display_rect.min.y) / display_rect.height().max(1.0);
                    let u_size = cb_rect.width() / display_rect.width().max(1.0);
                    let v_size = cb_rect.height() / display_rect.height().max(1.0);
                    let u0 = u_start.clamp(0.0, 1.0);
                    let v0 = v_start.clamp(0.0, 1.0);
                    let u1 = (u_start + u_size).clamp(0.0, 1.0);
                    let v1 = (v_start + v_size).clamp(0.0, 1.0);

                    if rot == 0 {
                        let uv = Rect::from_min_max(Pos2::new(u0, v0), Pos2::new(u1, v1));
                        painter.image(tid, cb_rect, uv, Color32::WHITE);
                    } else {
                        let map_uv = |dx: f32, dy: f32| -> Pos2 {
                            match rot {
                                90 => Pos2::new(dy, 1.0 - dx),
                                180 => Pos2::new(1.0 - dx, 1.0 - dy),
                                _ => Pos2::new(1.0 - dy, dx),
                            }
                        };
                        let mut mesh = egui::Mesh::with_texture(tid);
                        mesh.colored_vertex(cb_rect.left_top(), Color32::WHITE);
                        mesh.colored_vertex(cb_rect.right_top(), Color32::WHITE);
                        mesh.colored_vertex(cb_rect.right_bottom(), Color32::WHITE);
                        mesh.colored_vertex(cb_rect.left_bottom(), Color32::WHITE);
                        mesh.vertices[0].uv = map_uv(u0, v0);
                        mesh.vertices[1].uv = map_uv(u1, v0);
                        mesh.vertices[2].uv = map_uv(u1, v1);
                        mesh.vertices[3].uv = map_uv(u0, v1);
                        mesh.add_triangle(0, 1, 2);
                        mesh.add_triangle(0, 2, 3);
                        painter.add(egui::Shape::mesh(mesh));
                    }
                }
                if load_error.is_some() {
                    painter.rect_filled(cb_rect, Rounding::ZERO, Color32::from_black_alpha(140));
                }
            } else {
                painter.rect(
                    inner_rect,
                    Rounding::ZERO,
                    BG_FIELD,
                    Stroke::new(1.0_f32, BORDER_SUBTLE),
                );
            }

            if let Some(err) = load_error {
                let card_w = 460.0_f32.min((rect.width() - 32.0).max(200.0));
                let card_h = 166.0;
                let card_rect = Rect::from_center_size(rect.center(), Vec2::new(card_w, card_h));
                painter.rect(
                    card_rect,
                    Rounding::same(10.0),
                    Color32::from_rgba_unmultiplied(20, 22, 28, 240),
                    Stroke::new(1.0_f32, FLAG_REJECTED.gamma_multiply(0.75)),
                );
                let mut err_ui = ui.new_child(
                    egui::UiBuilder::new()
                        .max_rect(card_rect.shrink(16.0))
                        .layout(egui::Layout::top_down(egui::Align::Center)),
                );
                err_ui.spacing_mut().item_spacing.y = 8.0;
                err_ui.label(
                    RichText::new("Could not open MLV stream")
                        .strong()
                        .size(FONT_BODY + 2.0)
                        .color(FLAG_REJECTED),
                );
                err_ui.label(RichText::new(&video.file_name).size(FONT_SMALL).color(ACCENT_HOVER));
                err_ui.add(
                    egui::Label::new(
                        RichText::new(err)
                            .size(FONT_CAPTION)
                            .color(text_secondary(ui)),
                    )
                    .wrap(),
                );
                err_ui.add_space(4.0);
                err_ui.horizontal(|ui| {
                    let avail = ui.available_width();
                    let pad = ((avail - 190.0) * 0.5).max(0.0);
                    ui.add_space(pad);
                    if ui.button("Retry").clicked() {
                        action = Some(PlayerAction::RetryLoad);
                    }
                    if ui.button("Back to Grid").clicked() {
                        action = Some(PlayerAction::BackToGrid);
                    }
                });
            } else if is_loading_clip {
                ui.ctx().request_repaint_after(Duration::from_millis(32));
                let time = ui.input(|i| i.time);
                let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame_idx = ((time * 12.0) as usize) % spinner_frames.len();
                let spinner_char = spinner_frames[frame_idx];

                if thumb_id.is_some() {
                    let badge_w = 170.0;
                    let badge_h = 28.0;
                    let badge_rect = Rect::from_center_size(
                        Pos2::new(cb_rect.center().x, cb_rect.max.y - 36.0),
                        Vec2::new(badge_w, badge_h),
                    );
                    painter.rect(
                        badge_rect,
                        Rounding::same(14.0),
                        Color32::from_black_alpha(200),
                        Stroke::new(1.0_f32, Color32::from_white_alpha(45)),
                    );
                    painter.text(
                        badge_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        format!("{}  Opening Stream…", spinner_char),
                        FontId::proportional(FONT_CAPTION + 1.0),
                        Color32::from_rgb(220, 225, 235),
                    );
                } else {
                    let center = rect.center();
                    painter.text(
                        center - Vec2::new(0.0, 26.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{}  Opening MLV Clip…", spinner_char),
                        FontId::proportional(FONT_BODY + 3.0),
                        text_primary(ui),
                    );
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        &video.file_name,
                        FontId::proportional(FONT_BODY),
                        ACCENT_HOVER,
                    );
                    let specs_text = format!(
                        "{} × {} · {} · {} frames",
                        video.width,
                        video.height,
                        crate::ui::widgets::format_fps(video.fps),
                        video.frame_count
                    );
                    painter.text(
                        center + Vec2::new(0.0, 24.0),
                        egui::Align2::CENTER_CENTER,
                        specs_text,
                        FontId::proportional(FONT_CAPTION),
                        text_secondary(ui),
                    );
                    painter.text(
                        center + Vec2::new(0.0, 44.0),
                        egui::Align2::CENTER_CENTER,
                        "Indexing MLV frames & preparing audio buffer…",
                        FontId::proportional(FONT_CAPTION),
                        text_muted(ui),
                    );
                }
            } else {
                ui.ctx().request_repaint_after(Duration::from_millis(32));
                let time = ui.input(|i| i.time);
                let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame_idx = ((time * 12.0) as usize) % spinner_frames.len();
                let spinner_char = spinner_frames[frame_idx];
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("{}  Buffering Frame…", spinner_char),
                    FontId::proportional(FONT_BODY),
                    TEXT_MUTED,
                );
            }
        }

        // Subtle buffering indicator pill when playback holds cleanly on a cached frame
        if view.is_buffering && playback.is_some() && load_error.is_none() {
            ui.ctx().request_repaint_after(Duration::from_millis(32));
            let time = ui.input(|i| i.time);
            let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let frame_idx = ((time * 12.0) as usize) % spinner_frames.len();
            let spinner_char = spinner_frames[frame_idx];
            let pill_y = if view.is_fullscreen { rect.min.y + 72.0 } else { rect.max.y - 36.0 };
            let badge_rect = Rect::from_center_size(
                Pos2::new(rect.center().x, pill_y),
                Vec2::new(136.0, 26.0),
            );
            painter.rect(
                badge_rect,
                Rounding::same(13.0),
                Color32::from_black_alpha(205),
                Stroke::new(1.0_f32, WARNING.gamma_multiply(0.65)),
            );
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}  Buffering…", spinner_char),
                FontId::proportional(FONT_CAPTION + 1.0),
                WARNING,
            );
        }

        // Performance Telemetry HUD (top-left of canvas when toggled via Shift+D or badge click)
        if view.show_perf_hud {
            Self::draw_perf_hud(&painter, rect, view, video, playback, snapshot, load_error, gpu_info, achieved_fps);
        }

        // Visual cue / OSD overlay for star rating in the center of the viewport
        if let Some((rating, timestamp)) = view.rating_osd {
            let elapsed = timestamp.elapsed().as_secs_f32();
            if elapsed < 1.5 {
                ui.ctx().request_repaint_after(Duration::from_millis(16));
                let alpha = if elapsed < 0.9 {
                    1.0
                } else {
                    ((1.5 - elapsed) / 0.6).clamp(0.0, 1.0)
                };
                Self::draw_rating_hud(ui, &painter, rect.center(), rating, alpha);
            } else {
                view.rating_osd = None;
            }
        }

        // Visual cue / OSD pill for Focus Pixels, Rotation, Loop In/Out, Clip Switch
        if let Some((ref msg, timestamp)) = view.status_osd {
            let elapsed = timestamp.elapsed().as_secs_f32();
            if elapsed < 1.3 {
                ui.ctx().request_repaint_after(Duration::from_millis(16));
                let alpha = if elapsed < 0.8 {
                    1.0
                } else {
                    ((1.3 - elapsed) / 0.5).clamp(0.0, 1.0)
                };
                Self::draw_status_pill(&painter, rect, msg, alpha);
            } else {
                view.status_osd = None;
            }
        }

        action
    }

    #[allow(clippy::too_many_arguments)]
    fn draw_perf_hud(
        painter: &egui::Painter,
        rect: Rect,
        view: &PlayerView,
        video: &VideoRecord,
        playback: Option<&Playback>,
        snapshot: Option<&PlaybackSnapshot>,
        load_error: Option<&str>,
        gpu_info: &str,
        achieved_fps: f32,
    ) {
        let fp_detail = match playback {
            Some(pb) => {
                if let Some(ref fpm) = pb.source.focus_pixel_map {
                    format!("{} ({})", view.focus_pixel_mode.label(), fpm.source_name)
                } else if pb.source.is_pdaf_camera {
                    format!("{} (GPU PDAF)", view.focus_pixel_mode.label())
                } else {
                    format!("{} (Guard)", view.focus_pixel_mode.label())
                }
            }
            None => view.focus_pixel_mode.label().to_string(),
        };

        let mut lines = if let Some(err) = load_error {
            vec![
                format!("Status   : Open Failed ({})", video.file_name),
                format!("Error    : {}", err),
                format!("GPU      : {}", gpu_info),
            ]
        } else if let Some(snap) = snapshot {
            let cache_secs = snap.cache_count as f32 / video.fps.max(1.0);
            vec![
                format!("Playback : {:.1} / {:.2} fps ({:.2}x)", achieved_fps, video.fps, view.playback_speed),
                format!("Decode   : {:.0} fps  |  Ahead: {}", snap.decode_fps, snap.ready_ahead),
                format!("Cache    : {}/{} frames ({:.1}s)", snap.cache_count, snap.ahead_budget, cache_secs),
                format!("Focus Px : {}", fp_detail),
                format!("GPU      : {}", gpu_info),
            ]
        } else {
            vec![
                format!("Status   : Opening {}...", video.file_name),
                format!("Focus Px : {}", fp_detail),
                format!("GPU      : {}", gpu_info),
            ]
        };
        if let Some(snap) = snapshot {
            if snap.failed_count > 0 || snap.last_error.is_some() {
                let err_txt = snap.last_error.as_deref().unwrap_or("");
                lines.push(format!("Errors   : {} failed | {}", snap.failed_count, err_txt));
            }
        }

        let hud_w = 310.0_f32.min(rect.width() - 24.0);
        let hud_h = 14.0 + lines.len() as f32 * 16.0;
        let hud_rect = Rect::from_min_size(rect.min + Vec2::new(12.0, 12.0), Vec2::new(hud_w, hud_h));

        painter.rect(
            hud_rect,
            Rounding::same(8.0),
            Color32::from_black_alpha(205),
            Stroke::new(1.0_f32, Color32::from_white_alpha(40)),
        );

        let font = FontId::monospace(11.0);
        for (i, line) in lines.iter().enumerate() {
            painter.text(
                Pos2::new(hud_rect.min.x + 10.0, hud_rect.min.y + 8.0 + i as f32 * 16.0),
                egui::Align2::LEFT_TOP,
                line,
                font.clone(),
                Color32::from_rgb(220, 232, 245),
            );
        }
    }

    fn draw_status_pill(painter: &egui::Painter, rect: Rect, msg: &str, alpha: f32) {
        if alpha <= 0.005 {
            return;
        }
        let font = FontId::proportional(FONT_BODY);
        let galley = painter.layout_no_wrap(msg.to_string(), font, Color32::WHITE);
        let w = galley.size().x + 24.0;
        let h = galley.size().y + 12.0;
        let pill_rect = Rect::from_center_size(
            Pos2::new(rect.center().x, rect.max.y - 44.0),
            Vec2::new(w, h),
        );
        let bg = Color32::from_rgba_premultiplied(
            (16.0 * alpha) as u8,
            (18.0 * alpha) as u8,
            (24.0 * alpha) as u8,
            (220.0 * alpha) as u8,
        );
        let border = Color32::from_rgba_premultiplied(
            (90.0 * alpha) as u8,
            (120.0 * alpha) as u8,
            (175.0 * alpha) as u8,
            (180.0 * alpha) as u8,
        );
        let fg = Color32::from_rgba_premultiplied(
            (240.0 * alpha) as u8,
            (244.0 * alpha) as u8,
            (250.0 * alpha) as u8,
            (250.0 * alpha) as u8,
        );
        painter.rect(pill_rect, Rounding::same(h * 0.5), bg, Stroke::new(1.0_f32, border));
        painter.galley(pill_rect.center() - galley.size() * 0.5, galley, fg);
    }

    fn draw_rating_hud(
        _ui: &Ui,
        painter: &egui::Painter,
        center: egui::Pos2,
        rating: u8,
        alpha: f32,
    ) {
        if alpha <= 0.005 {
            return;
        }

        let hud_w = 200.0;
        let hud_h = 76.0;
        let hud_rect = Rect::from_center_size(center, Vec2::new(hud_w, hud_h));

        let bg_color = Color32::from_rgba_premultiplied(
            (16.0 * alpha) as u8,
            (18.0 * alpha) as u8,
            (24.0 * alpha) as u8,
            (225.0 * alpha) as u8,
        );
        let border_color = Color32::from_rgba_premultiplied(
            (80.0 * alpha) as u8,
            (88.0 * alpha) as u8,
            (105.0 * alpha) as u8,
            (180.0 * alpha) as u8,
        );
        let shadow_color = Color32::from_rgba_premultiplied(
            0,
            0,
            0,
            (130.0 * alpha) as u8,
        );

        // Shadow & rounded container
        painter.rect_filled(hud_rect.expand(2.0), Rounding::same(12.0), shadow_color);
        painter.rect(hud_rect, Rounding::same(12.0), bg_color, Stroke::new(1.0_f32, border_color));

        // 5 Star row
        let star_size = 22.0;
        let star_spacing = 4.0;
        let total_stars_w = 5.0 * star_size + 4.0 * star_spacing;
        let start_x = hud_rect.center().x - total_stars_w * 0.5 + star_size * 0.5;
        let stars_y = hud_rect.center().y - 10.0;

        for i in 0..5u8 {
            let star_pos = Pos2::new(start_x + i as f32 * (star_size + star_spacing), stars_y);
            let is_lit = i < rating;
            let (font, col) = if is_lit {
                let gold = Color32::from_rgba_premultiplied(
                    (255.0 * alpha) as u8,
                    (195.0 * alpha) as u8,
                    0,
                    (255.0 * alpha) as u8,
                );
                (icon_fill_font(star_size), gold)
            } else {
                let unlit = Color32::from_rgba_premultiplied(
                    (80.0 * alpha) as u8,
                    (85.0 * alpha) as u8,
                    (95.0 * alpha) as u8,
                    (150.0 * alpha) as u8,
                );
                (FontId::proportional(star_size), unlit)
            };
            painter.text(star_pos, egui::Align2::CENTER_CENTER, icon::STAR, font, col);
        }

        // Subtitle text label (e.g. "4 Stars" or "Rating Cleared")
        let (label_text, label_col) = if rating == 0 {
            (
                "Rating Cleared".to_string(),
                Color32::from_rgba_premultiplied(
                    (160.0 * alpha) as u8,
                    (165.0 * alpha) as u8,
                    (175.0 * alpha) as u8,
                    (200.0 * alpha) as u8,
                ),
            )
        } else if rating == 1 {
            (
                "1 Star".to_string(),
                Color32::from_rgba_premultiplied(
                    (240.0 * alpha) as u8,
                    (240.0 * alpha) as u8,
                    (245.0 * alpha) as u8,
                    (240.0 * alpha) as u8,
                ),
            )
        } else {
            (
                format!("{rating} Stars"),
                Color32::from_rgba_premultiplied(
                    (240.0 * alpha) as u8,
                    (240.0 * alpha) as u8,
                    (245.0 * alpha) as u8,
                    (240.0 * alpha) as u8,
                ),
            )
        };

        painter.text(
            Pos2::new(hud_rect.center().x, hud_rect.center().y + 18.0),
            egui::Align2::CENTER_CENTER,
            label_text,
            FontId::proportional(FONT_SMALL),
            label_col,
        );
    }
}
