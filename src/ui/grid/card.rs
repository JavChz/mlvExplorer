use super::types::GridAction;
use crate::db::{PickFlag, VideoRecord};
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Align2, Color32, FontId, Pos2, Rect, Rounding, Stroke, Ui, Vec2};

pub struct Card;

impl Card {
    pub fn show(
        ui: &mut Ui,
        index: usize,
        vid: &VideoRecord,
        is_selected: bool,
        selected_indices: &[usize],
        card_w: f32,
        card_h: f32,
        thumb_h: f32,
        thumb_mgr: &mut ThumbnailManager,
    ) -> (Option<GridAction>, Rect) {
        let mut action = None;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(card_w, card_h),
            egui::Sense::click(),
        );

        if response.clicked() {
            let modifiers = ui.input(|i| i.modifiers);
            action = Some(GridAction::Select {
                index,
                extend: modifiers.shift,
                toggle: modifiers.ctrl || modifiers.command,
            });
        }
        if response.double_clicked() {
            action = Some(GridAction::OpenPlayer(index));
        }

        let target_indices = if is_selected && selected_indices.len() > 1 {
            selected_indices.to_vec()
        } else {
            vec![index]
        };

        response.context_menu(|ui| {
            if ui.button(widgets::icon_label(icon::PLAY, "Open in Player")).clicked() {
                action = Some(GridAction::OpenPlayer(index));
                ui.close_menu();
            }
            if ui.button(widgets::icon_label(icon::ARROW_SQUARE_OUT, "Open in MLV App")).clicked() {
                action = Some(GridAction::OpenInMlvApp(index));
                ui.close_menu();
            }
            ui.separator();
            let (pick_icon, pick_text) = if vid.flag == PickFlag::Picked {
                (icon::X, "Remove Pick Flag")
            } else {
                (icon::FLAG, "Flag as Pick")
            };
            if ui.button(widgets::icon_label(pick_icon, pick_text)).clicked() {
                action = Some(GridAction::TogglePick(vid.id));
                ui.close_menu();
            }
            ui.separator();
            let count_label = if target_indices.len() > 1 {
                format!(" ({} clips)", target_indices.len())
            } else {
                String::new()
            };
            if ui.button(widgets::icon_label(icon::PROHIBIT, &format!("Remove from Catalog{}", count_label))).clicked() {
                action = Some(GridAction::RequestRemoveFromCatalog(target_indices.clone()));
                ui.close_menu();
            }
            if ui.button(
                egui::RichText::new(widgets::icon_label(icon::TRASH, &format!("Move to Trash{}", count_label)))
                    .color(FLAG_REJECTED)
            ).clicked() {
                action = Some(GridAction::RequestMoveToTrash(target_indices.clone()));
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
            Stroke::new(1.5_f32, BORDER_SELECTED)
        } else if response.hovered() {
            Stroke::new(1.0_f32, BORDER_STRONG)
        } else {
            Stroke::new(1.0_f32, BORDER_SUBTLE)
        };

        painter.rect(rect, Rounding::same(ROUNDING + 1.0), bg_color, border_stroke);

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

            // Letterbox background
            painter.rect_filled(thumb_rect, Rounding::same(ROUNDING), BG_THUMB);

            // Paint thumbnail
            painter.image(
                tex.id(),
                fitted_rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            painter.rect_filled(thumb_rect, Rounding::same(ROUNDING), BG_FIELD);
            painter.text(
                thumb_rect.center(),
                Align2::CENTER_CENTER,
                format!("{}  Loading…", icon::SPINNER_GAP),
                FontId::proportional(FONT_SMALL),
                TEXT_MUTED,
            );
        }

        // 2. Badges over thumbnail
        // Duration badge (bottom-right)
        let dur_str = vid.duration_tc();
        widgets::paint_badge(
            painter,
            Pos2::new(thumb_rect.max.x - 4.0, thumb_rect.max.y - 4.0),
            Align2::RIGHT_BOTTOM,
            &dur_str,
            FontId::monospace(9.5),
            TEXT_PRIMARY,
            Color32::from_black_alpha(190),
        );

        // Bit-depth & audio badge (bottom-left)
        let bpp_str = if vid.has_audio {
            format!("{}-bit · 🔊", vid.bits_per_pixel)
        } else {
            format!("{}-bit", vid.bits_per_pixel)
        };
        widgets::paint_badge(
            painter,
            Pos2::new(thumb_rect.min.x + 4.0, thumb_rect.max.y - 4.0),
            Align2::LEFT_BOTTOM,
            &bpp_str,
            FontId::proportional(9.5),
            TEXT_SECONDARY,
            Color32::from_black_alpha(190),
        );

        // Pick / Reject Flag Icon (top-left)
        if vid.flag != PickFlag::Unflagged {
            let (flag_icon, flag_color) = match vid.flag {
                PickFlag::Picked => (icon::FLAG, FLAG_PICKED),
                PickFlag::Rejected => (icon::X_CIRCLE, FLAG_REJECTED),
                PickFlag::Unflagged => ("", Color32::TRANSPARENT),
            };
            let flag_badge = Rect::from_min_size(
                thumb_rect.min + Vec2::new(4.0, 4.0),
                Vec2::new(20.0, 20.0),
            );
            painter.rect_filled(flag_badge, Rounding::same(ROUNDING), Color32::from_black_alpha(200));
            painter.text(
                flag_badge.center(),
                Align2::CENTER_CENTER,
                flag_icon,
                icon_fill_font(12.0),
                flag_color,
            );
        }

        // Dual Card / Spanned Pill (top-right)
        if vid.chunk_count > 1 {
            let span_str = if card_w >= 240.0 {
                format!("{} Spanned ({})", icon::STACK, vid.chunk_count)
            } else {
                format!("{} Spanned", icon::STACK)
            };
            let badge_rect = widgets::paint_badge(
                painter,
                Pos2::new(thumb_rect.max.x - 4.0, thumb_rect.min.y + 4.0),
                Align2::RIGHT_TOP,
                &span_str,
                FontId::proportional(10.0),
                Color32::WHITE,
                Color32::from_black_alpha(200),
            );
            // Interactive tooltip explaining spanned media
            let span_resp = ui.interact(badge_rect, ui.id().with(("spanned_tip", vid.id)), egui::Sense::hover());
            span_resp.on_hover_text(format!(
                "Card-spanned footage ({} split files: .MLV, .M00…)\nRecorded across multiple cards/chunks and seamlessly joined.",
                vid.chunk_count
            ));
        }

        // 3. Card Footer (File metadata & rating)
        let mut cursor_y = thumb_rect.max.y + 6.0;

        // Filename (elided safely)
        let max_text_w = card_w - 16.0;
        let fname_font = FontId::proportional(FONT_BODY);
        let elided_fname = widgets::elide(ui, &vid.file_name, &fname_font, max_text_w);
        painter.text(
            Pos2::new(rect.min.x + 8.0, cursor_y),
            Align2::LEFT_TOP,
            elided_fname,
            fname_font,
            TEXT_PRIMARY,
        );
        cursor_y += 18.0;

        // Camera & Specs Line
        let specs_str = format!("{} · {}p · {:.0}fps", vid.camera_name, vid.height, vid.fps);
        let specs_font = FontId::proportional(FONT_CAPTION);
        let elided_specs = widgets::elide(ui, &specs_str, &specs_font, max_text_w);
        painter.text(
            Pos2::new(rect.min.x + 8.0, cursor_y),
            Align2::LEFT_TOP,
            elided_specs,
            specs_font,
            TEXT_MUTED,
        );
        cursor_y += 16.0;

        // Star ratings (★) interactive clickable row
        let star_rects = widgets::paint_stars(
            painter,
            Pos2::new(rect.min.x + 8.0, cursor_y + 6.0),
            vid.rating,
            12.0,
            2.0,
        );

        for (s_idx, s_rect) in star_rects.iter().enumerate() {
            if ui.rect_contains_pointer(*s_rect) && ui.input(|inp| inp.pointer.primary_clicked()) {
                let star = (s_idx + 1) as u8;
                let new_r = if vid.rating == star { 0 } else { star };
                action = Some(GridAction::SetRating(vid.id, new_r));
            }
        }

        // Production Slate Label (e.g. Sc 12 · Sh 1 · Tk 3)
        if let Some(slate) = vid.slate_label() {
            let slate_font = FontId::proportional(FONT_CAPTION);
            let slate_str = format!("{} {}", icon::FILM_STRIP, slate);
            let max_slate_w = (rect.max.x - 8.0) - (rect.min.x + 8.0 + 72.0);
            let elided_slate = widgets::elide(ui, &slate_str, &slate_font, max_slate_w);
            painter.text(
                Pos2::new(rect.max.x - 8.0, cursor_y + 6.0),
                Align2::RIGHT_CENTER,
                elided_slate,
                slate_font,
                WARNING,
            );
        }

        (action, rect)
    }
}
