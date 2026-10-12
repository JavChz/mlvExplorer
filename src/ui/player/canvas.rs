use super::types::{PlayerView, ZoomMode};
use crate::db::VideoRecord;
use crate::gpu::{RawViewCallback, ViewUniforms};
use crate::mlv::color::camera_to_srgb;
use crate::playback::Playback;
use crate::ui::theme::*;
use crate::ui::widgets::icon;
use eframe::egui_wgpu;
use egui::{Rect, Rounding, Stroke, Ui, Vec2};

pub struct Canvas;

impl Canvas {
    pub fn show(
        view: &mut PlayerView,
        ui: &mut Ui,
        video: &VideoRecord,
        playback: &Playback,
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
            view.is_playing = !view.is_playing;
            view.last_frame_time = std::time::Instant::now();
            view.last_interaction_time = std::time::Instant::now();
            let time_sec = view.current_frame as f32 / video.fps.max(1.0);
            if let Ok(mut a) = playback.audio.lock() {
                if view.is_playing {
                    a.play(time_sec);
                } else {
                    a.pause();
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

        let frame_to_show = if let Some(frame) = playback.get(view.current_frame) {
            view.last_shown_frame = Some(frame.clone());
            Some(frame)
        } else {
            view.last_shown_frame.clone()
        };

        if let Some(frame) = frame_to_show {
            if cb_rect.width() > 0.5 && cb_rect.height() > 0.5 {
                let u_start = (cb_rect.min.x - display_rect.min.x) / display_rect.width().max(1.0);
                let v_start = (cb_rect.min.y - display_rect.min.y) / display_rect.height().max(1.0);
                let u_size = cb_rect.width() / display_rect.width().max(1.0);
                let v_size = cb_rect.height() / display_rect.height().max(1.0);

                let mut m = camera_to_srgb(&playback.source.clip.meta);

                let t = view.wb_temperature / 50.0;
                let r_gain = (2.0f32).powf(t * 0.45);
                let b_gain = (2.0f32).powf(-t * 0.45);

                let g = view.wb_tint / 50.0;
                let g_gain = (2.0f32).powf(-g * 0.45);

                for row in &mut m {
                    row[0] *= r_gain;
                    row[1] *= g_gain;
                    row[2] *= b_gain;
                }

                let (rx, ry) = playback.source.clip.meta.red_offset();
                let exp_mult = (2.0f32).powf(view.exposure_ev);

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
            painter.rect(
                display_rect.intersect(rect),
                Rounding::ZERO,
                BG_FIELD,
                Stroke::new(1.0_f32, BORDER_SUBTLE),
            );
            painter.text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("{}  Buffering Clip…", icon::SPINNER_GAP),
                egui::FontId::proportional(FONT_BODY),
                TEXT_MUTED,
            );
        }
    }
}
