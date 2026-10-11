use egui::{
    Color32, FontId, PointerButton, Pos2, Rect, Rounding, Sense, Stroke, Ui, Vec2,
};

pub struct Playbar;

impl Playbar {
    /// Renders a professional NLE video timeline scrubber bar with:
    /// - Full-width interactive track with cache buffer visualization
    /// - Click & drag scrubbing with sub-frame precision snapping
    /// - Hover cursor with timecode & frame number preview tooltip
    /// - Sleek studio playhead needle & handle
    pub fn show(
        ui: &mut Ui,
        _id_salt: &'static str,
        current_frame: usize,
        total_frames: usize,
        fps: f32,
        cached_frames: &[usize],
    ) -> Option<usize> {
        let total_frames = total_frames.max(1);
        let current_frame = current_frame.min(total_frames - 1);
        let mut seek_to = None;

        let available_w = ui.available_width();
        let track_h = 32.0_f32; // Comfortable click/drag touch target
        let (rect, response) = ui.allocate_exact_size(Vec2::new(available_w, track_h), Sense::click_and_drag());

        // Scrubbing state
        let track_left = rect.min.x + 8.0;
        let track_right = rect.max.x - 8.0;
        let track_width = (track_right - track_left).max(1.0);

        // Helper to convert X coordinate to frame index
        let x_to_frame = |x: f32| -> usize {
            let frac = ((x - track_left) / track_width).clamp(0.0, 1.0);
            let frame = (frac * (total_frames.saturating_sub(1) as f32)).round() as usize;
            frame.min(total_frames - 1)
        };

        // Helper to convert frame index to X coordinate
        let frame_to_x = |frame: usize| -> f32 {
            let frac = if total_frames > 1 {
                frame as f32 / (total_frames - 1) as f32
            } else {
                0.0
            };
            track_left + frac * track_width
        };

        // Handle click or drag
        if response.dragged_by(PointerButton::Primary) || response.clicked_by(PointerButton::Primary) {
            if let Some(pos) = response.interact_pointer_pos() {
                seek_to = Some(x_to_frame(pos.x));
            }
        }

        let painter = ui.painter();

        // 1. Track Bar Geometry
        let bar_h = 10.0_f32;
        let bar_y = rect.center().y - bar_h * 0.5;
        let bar_rect = Rect::from_min_max(
            Pos2::new(track_left, bar_y),
            Pos2::new(track_right, bar_y + bar_h),
        );

        // Background track (dark studio groove)
        painter.rect_filled(
            bar_rect,
            Rounding::same(5.0),
            Color32::from_rgb(16, 16, 20),
        );
        painter.rect_stroke(
            bar_rect,
            Rounding::same(5.0),
            Stroke::new(1.0_f32, Color32::from_rgb(38, 38, 46)),
        );

        // 2. Cached/Buffered Frames Visualization
        // Draw little buffered blocks or spans across the track
        if !cached_frames.is_empty() {
            let buffer_color = Color32::from_rgb(30, 64, 110); // subtle navy/blue cache fill
            let mut cached_sorted = cached_frames.to_vec();
            cached_sorted.sort_unstable();

            // Group consecutive frames into segments
            let mut start = cached_sorted[0];
            let mut end = start;
            for &f in &cached_sorted[1..] {
                if f == end + 1 {
                    end = f;
                } else {
                    let x0 = frame_to_x(start);
                    let x1 = frame_to_x(end) + 2.0;
                    let seg_rect = Rect::from_min_max(
                        Pos2::new(x0.max(track_left), bar_y + 1.0),
                        Pos2::new(x1.min(track_right), bar_y + bar_h - 1.0),
                    );
                    painter.rect_filled(seg_rect, Rounding::same(2.0), buffer_color);
                    start = f;
                    end = f;
                }
            }
            // Last segment
            let x0 = frame_to_x(start);
            let x1 = frame_to_x(end) + 2.0;
            let seg_rect = Rect::from_min_max(
                Pos2::new(x0.max(track_left), bar_y + 1.0),
                Pos2::new(x1.min(track_right), bar_y + bar_h - 1.0),
            );
            painter.rect_filled(seg_rect, Rounding::same(2.0), buffer_color);
        }

        // 3. Played Progress Fill (left of playhead)
        let playhead_x = frame_to_x(current_frame);
        if playhead_x > track_left {
            let progress_rect = Rect::from_min_max(
                Pos2::new(track_left, bar_y + 1.5),
                Pos2::new(playhead_x, bar_y + bar_h - 1.5),
            );
            painter.rect_filled(
                progress_rect,
                Rounding::same(4.0),
                Color32::from_rgb(59, 130, 246), // Accent Blue
            );
        }

        // 4. Subtle Tick Marks (every 25% and 10% along track)
        for pct in [0.25, 0.50, 0.75] {
            let tick_x = track_left + pct * track_width;
            painter.line_segment(
                [
                    Pos2::new(tick_x, bar_y - 2.0),
                    Pos2::new(tick_x, bar_y + bar_h + 2.0),
                ],
                Stroke::new(1.0_f32, Color32::from_rgb(45, 45, 55)),
            );
        }

        // 5. Hover Indicator & Tooltip Bubble
        if let Some(hover_pos) = response.hover_pos() {
            let hover_x = hover_pos.x.clamp(track_left, track_right);
            let hover_frame = x_to_frame(hover_x);

            // Ghost vertical line at cursor
            painter.line_segment(
                [
                    Pos2::new(hover_x, rect.min.y + 2.0),
                    Pos2::new(hover_x, rect.max.y - 2.0),
                ],
                Stroke::new(1.0_f32, Color32::from_white_alpha(90)),
            );

            // Floating timecode tooltip bubble above cursor
            let hover_time_secs = if fps > 0.0 { hover_frame as f32 / fps } else { 0.0 };
            let hm = (hover_time_secs / 60.0) as u32;
            let hs = (hover_time_secs % 60.0) as u32;
            let hf = (hover_frame % (fps as usize).max(1)) as u32;
            let tip_text = format!("{:02}:{:02}:{:02} [{}]", hm, hs, hf, hover_frame + 1);

            let tip_pos = Pos2::new(hover_x, rect.min.y - 6.0);
            let font_id = FontId::monospace(10.5);
            let text_rect = painter.text(
                tip_pos,
                egui::Align2::CENTER_BOTTOM,
                &tip_text,
                font_id,
                Color32::WHITE,
            );

            // Background pill behind tooltip text
            let tip_bg = text_rect.expand2(Vec2::new(6.0, 2.0));
            painter.rect_filled(tip_bg, Rounding::same(3.0), Color32::from_black_alpha(220));
            painter.rect_stroke(tip_bg, Rounding::same(3.0), Stroke::new(1.0_f32, Color32::from_rgb(60, 60, 75)));
            // Repaint text on top of bubble
            painter.text(
                tip_pos,
                egui::Align2::CENTER_BOTTOM,
                tip_text,
                FontId::monospace(10.5),
                Color32::WHITE,
            );
        }

        // 6. Playhead Scrubber Needle & Handle
        // Vertical needle across full height
        painter.line_segment(
            [
                Pos2::new(playhead_x, rect.min.y + 1.0),
                Pos2::new(playhead_x, rect.max.y - 1.0),
            ],
            Stroke::new(2.0_f32, Color32::WHITE),
        );

        // Top diamond / rounded pill handle
        let handle_w = 12.0_f32;
        let handle_h = 14.0_f32;
        let handle_rect = Rect::from_center_size(
            Pos2::new(playhead_x, rect.center().y),
            Vec2::new(handle_w, handle_h),
        );

        // Handle glow & body
        let handle_color = if response.is_pointer_button_down_on() {
            Color32::from_rgb(96, 165, 250) // Bright light blue when dragging
        } else if response.hovered() {
            Color32::WHITE
        } else {
            Color32::from_rgb(220, 225, 235)
        };

        painter.rect_filled(handle_rect, Rounding::same(3.0), handle_color);
        painter.rect_stroke(
            handle_rect,
            Rounding::same(3.0),
            Stroke::new(1.0_f32, Color32::from_black_alpha(150)),
        );

        // Center line on handle
        painter.line_segment(
            [
                Pos2::new(playhead_x, handle_rect.min.y + 3.0),
                Pos2::new(playhead_x, handle_rect.max.y - 3.0),
            ],
            Stroke::new(1.0_f32, Color32::from_black_alpha(100)),
        );

        seek_to
    }
}
