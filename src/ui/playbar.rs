use egui::{
    Color32, FontId, PointerButton, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2,
};
use crate::ui::theme::*;
use crate::ui::widgets;

pub struct Playbar;

impl Playbar {
    /// Renders a professional NLE video timeline scrubber bar with:
    /// - Interactive track with pre-merged cached frame ranges visualization
    /// - Optional In/Out loop bracket overlay
    /// - Click & drag scrubbing with frame precision snapping
    /// - Hover cursor with timecode tooltip
    /// - Studio playhead needle & handle
    pub fn show(
        ui: &mut Ui,
        _id_salt: &'static str,
        current_frame: usize,
        total_frames: usize,
        fps: f32,
        cached_ranges: &[(usize, usize)],
        loop_range: Option<(usize, usize)>,
    ) -> Option<usize> {
        let total_frames = total_frames.max(1);
        let current_frame = current_frame.min(total_frames - 1);
        let mut seek_to = None;

        let available_w = ui.available_width();
        let track_h = 24.0;
        let (rect, response) = ui.allocate_exact_size(Vec2::new(available_w, track_h), Sense::click_and_drag());

        let track_left = rect.min.x + 6.0;
        let track_right = rect.max.x - 6.0;
        let track_width = (track_right - track_left).max(1.0);

        let x_to_frame = |x: f32| -> usize {
            let frac = ((x - track_left) / track_width).clamp(0.0, 1.0);
            let frame = (frac * (total_frames.saturating_sub(1) as f32)).round() as usize;
            frame.min(total_frames - 1)
        };

        let frame_to_x = |frame: usize| -> f32 {
            let frac = if total_frames > 1 {
                frame as f32 / (total_frames - 1) as f32
            } else {
                0.0
            };
            track_left + frac * track_width
        };

        if response.dragged_by(PointerButton::Primary) || response.clicked_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                seek_to = Some(x_to_frame(pos.x));
            }
        }

        let painter = ui.painter();

        // 1. Track Bar Geometry
        let bar_h = 6.0;
        let bar_y = rect.center().y - bar_h * 0.5;
        let bar_rect = Rect::from_min_max(
            Pos2::new(track_left, bar_y),
            Pos2::new(track_right, bar_y + bar_h),
        );

        // Background track (dark groove)
        painter.rect_filled(
            bar_rect,
            Rounding::same(ROUNDING),
            BG_FIELD,
        );
        painter.rect_stroke(
            bar_rect,
            Rounding::same(ROUNDING),
            Stroke::new(1.0_f32, BORDER_SUBTLE),
        );

        // 2. Cached/Buffered Frame Ranges Visualization (no cloning or sorting!)
        if !cached_ranges.is_empty() {
            let buffer_color = ACCENT_DIM;
            for &(start, end) in cached_ranges {
                let x0 = frame_to_x(start);
                let x1 = frame_to_x(end) + 1.5;
                let seg_rect = Rect::from_min_max(
                    Pos2::new(x0.max(track_left), bar_y + 1.0),
                    Pos2::new(x1.min(track_right), bar_y + bar_h - 1.0),
                );
                painter.rect_filled(seg_rect, Rounding::ZERO, buffer_color);
            }
        }

        // 3. Played Progress Fill (left of playhead, or from loop_in when loop range set)
        let playhead_x = frame_to_x(current_frame);
        let prog_start_x = loop_range.map(|(a, _)| frame_to_x(a)).unwrap_or(track_left);
        if playhead_x > prog_start_x {
            let progress_rect = Rect::from_min_max(
                Pos2::new(prog_start_x.max(track_left), bar_y + 1.0),
                Pos2::new(playhead_x, bar_y + bar_h - 1.0),
            );
            painter.rect_filled(
                progress_rect,
                Rounding::same(1.0),
                ACCENT,
            );
        }

        // 3b. Optional In/Out Loop Range Markers
        if let Some((in_f, out_f)) = loop_range {
            let ix = frame_to_x(in_f).clamp(track_left, track_right);
            let ox = frame_to_x(out_f).clamp(track_left, track_right);
            let marker_col = WARNING;

            if ix > track_left {
                let dim_l = Rect::from_min_max(
                    Pos2::new(track_left, bar_y + 1.0),
                    Pos2::new(ix, bar_y + bar_h - 1.0),
                );
                painter.rect_filled(dim_l, Rounding::ZERO, Color32::from_black_alpha(120));
            }
            if ox < track_right {
                let dim_r = Rect::from_min_max(
                    Pos2::new(ox, bar_y + 1.0),
                    Pos2::new(track_right, bar_y + bar_h - 1.0),
                );
                painter.rect_filled(dim_r, Rounding::ZERO, Color32::from_black_alpha(120));
            }

            // Bracket ticks at In and Out
            painter.line_segment(
                [Pos2::new(ix, bar_y - 3.0), Pos2::new(ix, bar_y + bar_h + 3.0)],
                Stroke::new(2.0_f32, marker_col),
            );
            painter.line_segment(
                [Pos2::new(ix, bar_y - 3.0), Pos2::new((ix + 4.0).min(ox), bar_y - 3.0)],
                Stroke::new(2.0_f32, marker_col),
            );
            painter.line_segment(
                [Pos2::new(ox, bar_y - 3.0), Pos2::new(ox, bar_y + bar_h + 3.0)],
                Stroke::new(2.0_f32, marker_col),
            );
            painter.line_segment(
                [Pos2::new((ox - 4.0).max(ix), bar_y - 3.0), Pos2::new(ox, bar_y - 3.0)],
                Stroke::new(2.0_f32, marker_col),
            );
        }

        // 4. Subtle Tick Marks (every 25%)
        for pct in [0.25, 0.50, 0.75] {
            let tick_x = track_left + pct * track_width;
            painter.line_segment(
                [
                    Pos2::new(tick_x, bar_y - 2.0),
                    Pos2::new(tick_x, bar_y + bar_h + 2.0),
                ],
                Stroke::new(1.0_f32, BORDER_STRONG),
            );
        }

        // 5. Hover Indicator & Tooltip Bubble
        if let Some(hover_pos) = response.hover_pos() {
            let hover_x = hover_pos.x.clamp(track_left, track_right);
            let hover_frame = x_to_frame(hover_x);

            painter.line_segment(
                [
                    Pos2::new(hover_x, rect.min.y + 1.0),
                    Pos2::new(hover_x, rect.max.y - 1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_white_alpha(80)),
            );

            let tip_text = format!("{}  ·  {}", widgets::timecode(hover_frame, fps), hover_frame + 1);
            let tip_pos = Pos2::new(hover_x, rect.min.y - 4.0);
            widgets::paint_badge(
                painter,
                tip_pos,
                egui::Align2::CENTER_BOTTOM,
                &tip_text,
                FontId::monospace(10.0),
                TEXT_PRIMARY,
                Color32::from_black_alpha(220),
            );
        }

        // 6. Playhead Scrubber Needle & Handle
        painter.line_segment(
            [
                Pos2::new(playhead_x, rect.min.y + 1.0),
                Pos2::new(playhead_x, rect.max.y - 1.0),
            ],
            Stroke::new(1.5_f32, Color32::WHITE),
        );

        let handle_w = 8.0;
        let handle_h = 12.0;
        let handle_rect = Rect::from_center_size(
            Pos2::new(playhead_x, rect.center().y),
            Vec2::new(handle_w, handle_h),
        );

        let handle_color = if response.is_pointer_button_down_on() {
            ACCENT_HOVER
        } else if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_gray(215)
        };

        painter.rect_filled(handle_rect, Rounding::same(2.0), handle_color);
        painter.rect_stroke(
            handle_rect,
            Rounding::same(2.0),
            Stroke::new(1.0_f32, Color32::from_black_alpha(150)),
        );

        seek_to
    }
}
