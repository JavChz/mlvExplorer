use super::types::GridAction;
use crate::db::{PickFlag, VideoRecord};
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use egui::{Color32, Pos2, Rect, Rounding, Stroke, Ui, Vec2};

pub struct Card;

impl Card {
    pub fn show(
        ui: &mut Ui,
        index: usize,
        vid: &VideoRecord,
        is_selected: bool,
        card_w: f32,
        card_h: f32,
        thumb_h: f32,
        thumb_mgr: &mut ThumbnailManager,
    ) -> Option<GridAction> {
        let mut action = None;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(card_w, card_h),
            egui::Sense::click(),
        );

        if response.clicked() {
            action = Some(GridAction::Select(index));
        }
        if response.double_clicked() {
            action = Some(GridAction::OpenPlayer(index));
        }

        response.context_menu(|ui| {
            if ui.button("▶ Open in Player").clicked() {
                action = Some(GridAction::OpenPlayer(index));
                ui.close_menu();
            }
            if ui.button("🎬 Open in MLVApp").clicked() {
                action = Some(GridAction::OpenInMlvApp(index));
                ui.close_menu();
            }
            ui.separator();
            if ui.button(if vid.flag == PickFlag::Picked { "🏳 Remove Pick" } else { "🏳 Pick" }).clicked() {
                action = Some(GridAction::TogglePick(vid.id));
                ui.close_menu();
            }
        });

        let painter = ui.painter();

        // Card background & borders
        let bg_color = if is_selected {
            BG_CARD_SELECTED
        } else if response.hovered() {
            BG_CARD_HOVER
        } else {
            BG_CARD
        };

        let border_stroke = if is_selected {
            Stroke::new(2.0_f32, BORDER_SELECTED)
        } else {
            Stroke::new(1.0_f32, BORDER_SUBTLE)
        };

        painter.rect(rect, Rounding::same(6.0), bg_color, border_stroke);

        // 1. Thumbnail Area
        let thumb_rect = Rect::from_min_size(
            rect.min + Vec2::new(4.0, 4.0),
            Vec2::new(card_w - 8.0, thumb_h),
        );

        // Fetch or request thumbnail
        let thumb_tex = thumb_mgr.get_or_request(vid.id, &vid.file_path);

        if let Some(tex) = thumb_tex {
            // Calculate display rect respecting anamorphic de-squeeze
            let stretch = if vid.stretch_factor > 0.05 { vid.stretch_factor } else { 1.0 };
            let video_ar = if vid.height > 0 {
                (vid.width as f32 * stretch) / vid.height as f32
            } else {
                16.0 / 9.0
            };

            let mut fit_w = thumb_rect.width();
            let mut fit_h = fit_w / video_ar;
            if fit_h > thumb_rect.height() {
                fit_h = thumb_rect.height();
                fit_w = fit_h * video_ar;
            }
            let fitted_rect = Rect::from_center_size(thumb_rect.center(), Vec2::new(fit_w, fit_h));

            // Dark background for pillarbox/letterbox
            painter.rect_filled(thumb_rect, Rounding::same(4.0), Color32::from_rgb(10, 10, 14));

            // Paint real MLV frame 0 decoded thumbnail fitted & de-squeezed
            painter.image(
                tex.id(),
                fitted_rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // Dark slate placeholder while decoding
            painter.rect_filled(thumb_rect, Rounding::same(4.0), Color32::from_rgb(18, 18, 22));
            painter.text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                "⏳ Loading RAW...",
                egui::FontId::proportional(12.0),
                TEXT_MUTED,
            );
        }

        // 2. Badges over thumbnail
        // Duration pill (bottom-right)
        let dur_str = vid.duration_tc();
        let badge_w = 54.0;
        let badge_rect = Rect::from_min_size(
            Pos2::new(thumb_rect.max.x - badge_w - 4.0, thumb_rect.max.y - 18.0),
            Vec2::new(badge_w, 15.0),
        );
        painter.rect_filled(badge_rect, Rounding::same(3.0), Color32::from_black_alpha(200));
        painter.text(
            badge_rect.center(),
            egui::Align2::CENTER_CENTER,
            dur_str,
            egui::FontId::monospace(9.5),
            TEXT_PRIMARY,
        );

        // Codec / Bit-depth pill (bottom-left)
        let bpp_str = format!("{}-bit", vid.bits_per_pixel);
        let bpp_rect = Rect::from_min_size(
            Pos2::new(thumb_rect.min.x + 4.0, thumb_rect.max.y - 18.0),
            Vec2::new(38.0, 15.0),
        );
        painter.rect_filled(bpp_rect, Rounding::same(3.0), Color32::from_black_alpha(200));
        painter.text(
            bpp_rect.center(),
            egui::Align2::CENTER_CENTER,
            bpp_str,
            egui::FontId::proportional(9.5),
            Color32::from_rgb(147, 197, 253),
        );

        // Pick / Flag Icon (top-left)
        if vid.flag != PickFlag::Unflagged {
            let (flag_icon, flag_color) = match vid.flag {
                PickFlag::Picked => ("🏳", FLAG_PICKED),
                PickFlag::Rejected => ("❌", FLAG_REJECTED),
                PickFlag::Unflagged => ("", Color32::TRANSPARENT),
            };
            let flag_rect = Rect::from_min_size(
                thumb_rect.min + Vec2::new(4.0, 4.0),
                Vec2::new(18.0, 18.0),
            );
            painter.rect_filled(flag_rect, Rounding::same(3.0), Color32::from_black_alpha(180));
            painter.text(
                flag_rect.center(),
                egui::Align2::CENTER_CENTER,
                flag_icon,
                egui::FontId::proportional(11.0),
                flag_color,
            );
        }

        // Dual Card / Spanned Pill (top-right)
        if vid.chunk_count > 1 {
            let span_rect = Rect::from_min_size(
                Pos2::new(thumb_rect.max.x - 52.0, thumb_rect.min.y + 4.0),
                Vec2::new(48.0, 15.0),
            );
            painter.rect_filled(span_rect, Rounding::same(3.0), Color32::from_rgb(245, 158, 11));
            painter.text(
                span_rect.center(),
                egui::Align2::CENTER_CENTER,
                format!("⚡ {}x", vid.chunk_count),
                egui::FontId::proportional(9.5),
                Color32::BLACK,
            );
        }

        // 3. Card Footer (File metadata & rating)
        let mut cursor_y = thumb_rect.max.y + 6.0;

        // Filename
        let fname_clipped = if vid.file_name.len() > 26 {
            format!("{}...", &vid.file_name[..23])
        } else {
            vid.file_name.clone()
        };
        painter.text(
            Pos2::new(rect.min.x + 8.0, cursor_y),
            egui::Align2::LEFT_TOP,
            fname_clipped,
            egui::FontId::proportional(12.5),
            TEXT_PRIMARY,
        );
        cursor_y += 18.0;

        // Camera & Specs Line
        let specs_str = format!("{} · {}p · {:.0}fps", vid.camera_name, vid.height, vid.fps);
        let specs_clipped = if specs_str.len() > 30 {
            format!("{}...", &specs_str[..27])
        } else {
            specs_str
        };
        painter.text(
            Pos2::new(rect.min.x + 8.0, cursor_y),
            egui::Align2::LEFT_TOP,
            specs_clipped,
            egui::FontId::proportional(10.5),
            TEXT_MUTED,
        );
        cursor_y += 16.0;

        // Star ratings (★) clickable or interactive
        let mut star_x = rect.min.x + 8.0;
        for s in 1..=5 {
            let is_lit = s <= vid.rating;
            let star_char = if is_lit { "★" } else { "☆" };
            let star_col = if is_lit { ACCENT_GOLD } else { TEXT_MUTED };

            let star_rect = Rect::from_min_size(Pos2::new(star_x, cursor_y), Vec2::new(14.0, 14.0));
            if ui.rect_contains_pointer(star_rect) && ui.input(|inp| inp.pointer.primary_clicked()) {
                let new_r = if vid.rating == s { 0 } else { s };
                action = Some(GridAction::SetRating(vid.id, new_r));
            }

            painter.text(
                star_rect.center(),
                egui::Align2::CENTER_CENTER,
                star_char,
                egui::FontId::proportional(13.0),
                star_col,
            );
            star_x += 16.0;
        }

        action
    }
}
