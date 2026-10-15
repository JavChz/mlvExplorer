use super::types::{PlayerView, ZoomMode};
use crate::db::VideoRecord;
use crate::gpu::{RawViewCallback, ViewUniforms};
use crate::mlv::color::camera_to_srgb;
use crate::playback::Playback;
use crate::ui::theme::*;
use crate::ui::widgets::icon;
use eframe::egui_wgpu;
use egui::{Color32, FontId, Pos2, Rect, Rounding, Stroke, Ui, Vec2};

pub struct Canvas;

impl Canvas {
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: Option<&Playback>,
        thumb_id: Option<egui::TextureId>,
    ) {
        let bottom_height = if view.is_fullscreen { 0.0 } else { 76.0 };
        let canvas_height = (ui.available_height() - bottom_height).max(50.0);
        let canvas_size = Vec2::new(ui.available_width(), canvas_height);
        let (rect, canvas_response) = ui.allocate_exact_size(canvas_size, egui::Sense::click_and_drag());

        let painter = ui.painter().with_clip_rect(rect);
        painter.rect_filled(rect, Rounding::ZERO, BG_VIEWER);

        let stretch = if video.stretch_factor > 0.05 { video.stretch_factor } else { 1.0 };
        let aspect = if video.height > 0 {
            (video.width as f32 * stretch) / video.height as f32
        } else {
            16.0 / 9.0
        };

        let mut fit_w = rect.width();
        let mut fit_h = fit_w / aspect;
        if fit_h > rect.height() {
            fit_h = rect.height();
            fit_w = fit_h * aspect;
        }

        if canvas_response.double_clicked() {
            view.is_fullscreen = !view.is_fullscreen;
            if view.is_fullscreen {
                view.zoom_mode = ZoomMode::Fit;
                view.pan_offset = Vec2::ZERO;
            }
            view.last_interaction_time = std::time::Instant::now();
        } else if canvas_response.clicked() {
            // Click toggles play / pause in all modes, while HUD overlay controls show upon mouse movement
            if let Some(pb) = playback {
                view.is_playing = !view.is_playing;
                view.last_frame_time = std::time::Instant::now();
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

        let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
        if canvas_response.hovered() && scroll_delta.abs() > 0.5 {
            let current_scale = match view.zoom_mode {
                ZoomMode::Fit => fit_w / (video.width as f32 * stretch).max(1.0),
                ZoomMode::Scale(s) => s,
            };
            let zoom_factor = if scroll_delta > 0.0 { 1.12 } else { 0.89 };
            let new_scale = (current_scale * zoom_factor).clamp(0.15, 8.0);
            view.zoom_mode = ZoomMode::Scale(new_scale);
        }

        let (display_w, display_h) = match view.zoom_mode {
            ZoomMode::Fit => (fit_w, fit_h),
            ZoomMode::Scale(scale) => {
                (video.width as f32 * stretch * scale, video.height as f32 * scale)
            }
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

        if let Some((frame, pb)) = frame_to_show {
            if cb_rect.width() > 0.5 && cb_rect.height() > 0.5 {
                let u_start = (cb_rect.min.x - display_rect.min.x) / display_rect.width().max(1.0);
                let v_start = (cb_rect.min.y - display_rect.min.y) / display_rect.height().max(1.0);
                let u_size = cb_rect.width() / display_rect.width().max(1.0);
                let v_size = cb_rect.height() / display_rect.height().max(1.0);

                let m = camera_to_srgb(&pb.source.clip.meta);
                let (rx, ry) = pb.source.clip.meta.red_offset();

                let uniforms = ViewUniforms {
                    m0: [m[0][0], m[0][1], m[0][2], 0.0],
                    m1: [m[1][0], m[1][1], m[1][2], 0.0],
                    m2: [m[2][0], m[2][1], m[2][2], 0.0],
                    levels: [frame.black, frame.white, 1.0, 1.0],
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
            let inner_rect = display_rect.intersect(rect);
            let is_loading_clip = playback.is_none();

            if let Some(tid) = thumb_id {
                // Instantly display frame 0 preview from thumbnail cache!
                if cb_rect.width() > 0.5 && cb_rect.height() > 0.5 {
                    let u_start = (cb_rect.min.x - display_rect.min.x) / display_rect.width().max(1.0);
                    let v_start = (cb_rect.min.y - display_rect.min.y) / display_rect.height().max(1.0);
                    let u_size = cb_rect.width() / display_rect.width().max(1.0);
                    let v_size = cb_rect.height() / display_rect.height().max(1.0);
                    let uv = Rect::from_min_max(
                        Pos2::new(u_start.clamp(0.0, 1.0), v_start.clamp(0.0, 1.0)),
                        Pos2::new((u_start + u_size).clamp(0.0, 1.0), (v_start + v_size).clamp(0.0, 1.0)),
                    );
                    painter.image(tid, cb_rect, uv, Color32::WHITE);
                }

                if is_loading_clip {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
                    let time = ui.input(|i| i.time);
                    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let frame_idx = ((time * 12.0) as usize) % spinner_frames.len();
                    let spinner_char = spinner_frames[frame_idx];

                    // Sleek, modern HUD pill at the bottom-center of the canvas
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
                        egui::FontId::proportional(FONT_CAPTION + 1.0),
                        Color32::from_rgb(220, 225, 235),
                    );
                }
            } else {
                painter.rect(
                    inner_rect,
                    Rounding::ZERO,
                    BG_FIELD,
                    Stroke::new(1.0_f32, BORDER_SUBTLE),
                );

                let center = rect.center();
                if is_loading_clip {
                    ui.ctx().request_repaint_after(std::time::Duration::from_millis(16));
                    let time = ui.input(|i| i.time);
                    let spinner_frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let frame_idx = ((time * 12.0) as usize) % spinner_frames.len();
                    let spinner_char = spinner_frames[frame_idx];

                    painter.text(
                        center - Vec2::new(0.0, 26.0),
                        egui::Align2::CENTER_CENTER,
                        format!("{}  Opening MLV Clip…", spinner_char),
                        egui::FontId::proportional(FONT_BODY + 3.0),
                        text_primary(ui),
                    );

                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        &video.file_name,
                        egui::FontId::proportional(FONT_BODY),
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
                        egui::FontId::proportional(FONT_CAPTION),
                        text_secondary(ui),
                    );

                    painter.text(
                        center + Vec2::new(0.0, 44.0),
                        egui::Align2::CENTER_CENTER,
                        "Indexing MLV frames & preparing audio buffer…",
                        egui::FontId::proportional(FONT_CAPTION),
                        text_muted(ui),
                    );
                } else {
                    painter.text(
                        center,
                        egui::Align2::CENTER_CENTER,
                        format!("{}  Buffering Frame…", icon::SPINNER_GAP),
                        egui::FontId::proportional(FONT_BODY),
                        TEXT_MUTED,
                    );
                }
            }
        }

        // Visual cue / OSD overlay for star rating in the center of the viewport
        if let Some((rating, timestamp)) = view.rating_osd {
            let elapsed = timestamp.elapsed().as_secs_f32();
            if elapsed < 1.5 {
                ui.ctx().request_repaint();
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
