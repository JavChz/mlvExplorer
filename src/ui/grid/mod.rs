pub mod card;
pub mod types;

pub use card::Card;
pub use types::{GridAction, GridView};

use crate::db::{SortOrder, VideoRecord};
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Align2, Color32, Pos2, Rect, RichText, Rounding, Stroke, Ui, Vec2};

struct DayGroup {
    day_key: String,
    title: String,
    indices: Vec<usize>,
    total_bytes: u64,
    total_duration_secs: f32,
}

fn format_day_info(date_str: Option<&str>) -> (String, String) {
    let Some(d) = date_str else {
        return ("0000-00-00".to_string(), "Undated / Unknown Date".to_string());
    };
    let clean = d.trim();
    if clean.len() < 10 {
        return ("0000-00-00".to_string(), "Undated / Unknown Date".to_string());
    }
    let date_part = &clean[..10];
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
        let formatted = naive.format("%A, %B %e, %Y").to_string();
        (date_part.to_string(), formatted)
    } else {
        (date_part.to_string(), date_part.to_string())
    }
}

fn format_duration(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    let m = s / 60;
    let rem = s % 60;
    format!("{:02}:{:02}", m, rem)
}

fn format_month_label(date_str: Option<&str>) -> String {
    let Some(d) = date_str else {
        return "Undated".to_string();
    };
    let clean = d.trim();
    if clean.len() < 7 {
        return "Undated".to_string();
    }
    let date_part = &clean[..10.min(clean.len())];
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
        naive.format("%B %Y").to_string()
    } else if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(clean) {
        dt.format("%B %Y").to_string()
    } else {
        date_part.to_string()
    }
}

impl GridView {
    pub fn show(
        &mut self,
        ui: &mut Ui,
        videos: &[VideoRecord],
        selected_indices: &[usize],
        thumb_mgr: &mut ThumbnailManager,
        sort_order: &mut SortOrder,
    ) -> Option<GridAction> {
        let mut action = None;

        // Top toolbar for Sort, Grouping, Zoom & Library Stats
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

            // Sort Selector
            ui.label(RichText::new("Sort:").size(FONT_SMALL).color(TEXT_MUTED));
            egui::ComboBox::from_id_salt("grid_sort_selector")
                .selected_text(RichText::new(sort_order.label()).size(FONT_SMALL))
                .show_ui(ui, |ui| {
                    for opt in SortOrder::ALL {
                        if ui.selectable_value(sort_order, opt, opt.label()).clicked() {
                            action = Some(GridAction::SortChanged);
                        }
                    }
                });

            widgets::toolbar_divider(ui);

            // Group by Day Toggle
            let group_btn = egui::Button::new(
                RichText::new(format!("{}  Group by Day", icon::CALENDAR))
                    .size(FONT_SMALL)
                    .color(if self.group_by_day { Color32::WHITE } else { TEXT_SECONDARY }),
            )
            .fill(if self.group_by_day { ACCENT_DIM } else { Color32::TRANSPARENT })
            .stroke(if self.group_by_day { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
            .rounding(Rounding::same(ROUNDING))
            .min_size(Vec2::new(0.0, CONTROL_HEIGHT));

            if ui
                .add(group_btn)
                .on_hover_text("Group clips by capture date with section banners")
                .clicked()
            {
                self.group_by_day = !self.group_by_day;
            }

            widgets::toolbar_divider(ui);

            // Thumbnail Zoom Slider
            ui.label(RichText::new("Zoom:").size(FONT_SMALL).color(TEXT_MUTED));
            ui.add(
                egui::Slider::new(&mut self.card_size, 180.0..=440.0)
                    .show_value(false)
            );

            // Right-aligned Total Stats
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let total_size: u64 = videos.iter().map(|v| v.file_size).sum();
                let total_dur: f32 = videos.iter().map(|v| v.duration_seconds).sum();
                ui.label(
                    RichText::new(format!(
                        "{} clips  ·  {}  ·  {}",
                        videos.len(),
                        widgets::format_bytes(total_size),
                        format_duration(total_dur)
                    ))
                    .size(FONT_CAPTION)
                    .color(TEXT_MUTED),
                );
            });
        });

        ui.add_space(2.0);
        let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(sep_rect, Rounding::ZERO, BORDER_SUBTLE);
        ui.add_space(4.0);

        if videos.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(RichText::new(icon::FILM_STRIP).size(48.0).color(TEXT_DISABLED));
                ui.add_space(8.0);
                ui.label(RichText::new("No Magic Lantern Clips Matching Filter").size(15.0).strong().color(TEXT_PRIMARY));
                ui.add_space(4.0);
                ui.label(RichText::new("Drag and drop .MLV files or folders anywhere onto the window.").size(FONT_SMALL).color(TEXT_HINT));
            });
            return action;
        }

        let card_w = self.card_size;
        let thumb_h = card_w * (9.0 / 16.0); // 16:9 thumbnail
        let footer_h = 66.0;
        let card_h = thumb_h + footer_h;

        let mut card_rects: Vec<(usize, Rect)> = Vec::with_capacity(videos.len());

        let scroll_output = egui::ScrollArea::vertical()
            .drag_to_scroll(false)
            .show(ui, |ui| {
            let available_width = ui.available_width();
            let columns = ((available_width / (card_w + 12.0)).floor() as usize).max(1);

            if self.group_by_day {
                // Group clips by calendar capture day
                let mut groups: Vec<DayGroup> = Vec::new();
                for (i, vid) in videos.iter().enumerate() {
                    let (key, title) = format_day_info(vid.date_recorded.as_deref());
                    if let Some(last) = groups.last_mut() {
                        if last.day_key == key {
                            last.indices.push(i);
                            last.total_bytes += vid.file_size;
                            last.total_duration_secs += vid.duration_seconds;
                            continue;
                        }
                    }
                    groups.push(DayGroup {
                        day_key: key,
                        title,
                        indices: vec![i],
                        total_bytes: vid.file_size,
                        total_duration_secs: vid.duration_seconds,
                    });
                }

                for (grp_idx, grp) in groups.iter().enumerate() {
                    // Day Section Header Banner
                    ui.add_space(6.0);
                    let (banner_rect, _) = ui.allocate_exact_size(Vec2::new(available_width, 26.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect(
                        banner_rect,
                        Rounding::same(ROUNDING),
                        BG_SECTION,
                        Stroke::new(1.0_f32, BORDER_SUBTLE),
                    );

                    // Left title
                    painter.text(
                        Pos2::new(banner_rect.min.x + 10.0, banner_rect.center().y),
                        Align2::LEFT_CENTER,
                        format!("{}  {}", icon::CALENDAR, grp.title),
                        egui::FontId::proportional(FONT_BODY),
                        TEXT_PRIMARY,
                    );

                    // Right stats
                    let grp_info = format!(
                        "{} clips  ·  {}  ·  {}",
                        grp.indices.len(),
                        widgets::format_bytes(grp.total_bytes),
                        format_duration(grp.total_duration_secs)
                    );
                    painter.text(
                        Pos2::new(banner_rect.max.x - 10.0, banner_rect.center().y),
                        Align2::RIGHT_CENTER,
                        grp_info,
                        egui::FontId::proportional(FONT_CAPTION),
                        TEXT_MUTED,
                    );

                    ui.add_space(6.0);

                    // Grid for this day
                    egui::Grid::new(format!("day_grid_{}", grp_idx))
                        .spacing([12.0, 12.0])
                        .show(ui, |ui| {
                            for (c_idx, &i) in grp.indices.iter().enumerate() {
                                let vid = &videos[i];
                                let is_selected = selected_indices.contains(&i);

                                let (card_act, rect) = Card::show(
                                    ui,
                                    i,
                                    vid,
                                    is_selected,
                                    selected_indices,
                                    card_w,
                                    card_h,
                                    thumb_h,
                                    thumb_mgr,
                                );
                                card_rects.push((i, rect));

                                if let Some(a) = card_act {
                                    action = Some(a);
                                }

                                if (c_idx + 1) % columns == 0 {
                                    ui.end_row();
                                }
                            }
                        });

                    ui.add_space(10.0);
                }
            } else {
                // Continuous flat grid
                egui::Grid::new("video_cards_grid_flat")
                    .spacing([12.0, 12.0])
                    .show(ui, |ui| {
                        for (i, vid) in videos.iter().enumerate() {
                            let is_selected = selected_indices.contains(&i);

                            let (card_act, rect) = Card::show(
                                ui,
                                i,
                                vid,
                                is_selected,
                                selected_indices,
                                card_w,
                                card_h,
                                thumb_h,
                                thumb_mgr,
                            );
                            card_rects.push((i, rect));

                            if let Some(a) = card_act {
                                action = Some(a);
                            }

                            if (i + 1) % columns == 0 {
                                ui.end_row();
                            }
                        }
                    });
            }
        });

        // Mouse Rubberband Box Selection
        let inner_rect = scroll_output.inner_rect;
        let pointer = ui.input(|i| i.pointer.clone());

        // Gutter margin: exclude the right scrollbar track from initiating rubberband selection
        let scrollbar_gutter_w = 16.0_f32;
        let valid_selection_rect = Rect::from_min_max(
            inner_rect.min,
            Pos2::new(inner_rect.max.x - scrollbar_gutter_w, inner_rect.max.y),
        );

        if pointer.primary_pressed() {
            if let Some(press_pos) = pointer.press_origin() {
                if valid_selection_rect.contains(press_pos) {
                    let hit_tooltip = self.scrollbar_tooltip_rect.map_or(false, |r| r.expand(6.0).contains(press_pos));
                    let hit_card = card_rects.iter().any(|(_, r)| r.contains(press_pos));
                    if !hit_card && !hit_tooltip {
                        self.drag_start_pos = Some(press_pos);
                        self.drag_current_pos = Some(press_pos);
                    }
                }
            }
        }

        if self.drag_start_pos.is_some() {
            if pointer.primary_down() {
                if let Some(pos) = pointer.latest_pos() {
                    self.drag_current_pos = Some(pos);
                    ui.ctx().request_repaint();
                }
            }
        }

        if let (Some(start), Some(curr)) = (self.drag_start_pos, self.drag_current_pos) {
            let drag_dist = (curr - start).length();
            if drag_dist > 4.0 {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                let box_rect = Rect::from_two_pos(start, curr);

                // Draw translucent selection rectangle with subtle accent border
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("grid_selection_box"),
                ));
                painter.rect(
                    box_rect,
                    Rounding::same(ROUNDING),
                    ACCENT.gamma_multiply(0.18),
                    Stroke::new(1.0_f32, ACCENT),
                );

                let selected_in_box: Vec<usize> = card_rects
                    .iter()
                    .filter(|(_, r)| r.intersects(box_rect))
                    .map(|(idx, _)| *idx)
                    .collect();

                if pointer.primary_released() {
                    self.drag_start_pos = None;
                    self.drag_current_pos = None;
                    action = Some(GridAction::BoxSelect(selected_in_box));
                }
            } else if pointer.primary_released() {
                self.drag_start_pos = None;
                self.drag_current_pos = None;
                if action.is_none() {
                    action = Some(GridAction::DeselectAll);
                }
            }
        }

        if pointer.primary_released() {
            self.drag_start_pos = None;
            self.drag_current_pos = None;
        }

        // --- Date Scrubber Tooltip on Scrollbar ---
        // Shows floating month/year badge next to the scroll handle when hovering or dragging scrollbar
        let total_content_h = scroll_output.content_size.y;
        let view_h = inner_rect.height();
        if total_content_h > view_h && !videos.is_empty() {
            let scrollbar_zone = Rect::from_min_max(
                Pos2::new(inner_rect.max.x - scrollbar_gutter_w - 6.0, inner_rect.min.y),
                Pos2::new(inner_rect.max.x + 8.0, inner_rect.max.y),
            );

            let pointer_pos = pointer.latest_pos();
            let is_scrollbar_hovered = pointer_pos.map_or(false, |p| scrollbar_zone.contains(p));
            let is_tooltip_hovered = pointer_pos.map_or(false, |p| {
                self.scrollbar_tooltip_rect.map_or(false, |r| r.expand(4.0).contains(p))
            });
            let is_scrollbar_dragging = pointer.primary_down()
                && self.drag_start_pos.is_none()
                && (pointer.press_origin().map_or(false, |p| {
                    scrollbar_zone.contains(p)
                        || self.scrollbar_tooltip_rect.map_or(false, |r| r.expand(4.0).contains(p))
                }));

            if is_scrollbar_hovered || is_tooltip_hovered || is_scrollbar_dragging {
                let scroll_fraction = if is_scrollbar_dragging || is_scrollbar_hovered {
                    if let Some(p) = pointer_pos {
                        ((p.y - inner_rect.min.y) / view_h.max(1.0)).clamp(0.0, 1.0)
                    } else {
                        (scroll_output.state.offset.y / (total_content_h - view_h).max(1.0)).clamp(0.0, 1.0)
                    }
                } else {
                    (scroll_output.state.offset.y / (total_content_h - view_h).max(1.0)).clamp(0.0, 1.0)
                };

                let approx_idx = ((scroll_fraction * (videos.len().saturating_sub(1) as f32)).round() as usize)
                    .min(videos.len().saturating_sub(1));

                let mut date_str = videos[approx_idx].date_recorded.as_deref();
                if date_str.is_none() {
                    for offset in 1..=5 {
                        if approx_idx >= offset && videos[approx_idx - offset].date_recorded.is_some() {
                            date_str = videos[approx_idx - offset].date_recorded.as_deref();
                            break;
                        }
                        if approx_idx + offset < videos.len() && videos[approx_idx + offset].date_recorded.is_some() {
                            date_str = videos[approx_idx + offset].date_recorded.as_deref();
                            break;
                        }
                    }
                }

                let tooltip_text = format_month_label(date_str);

                let bubble_y = if let Some(p) = pointer_pos {
                    if is_scrollbar_dragging || is_scrollbar_hovered {
                        p.y.clamp(inner_rect.min.y + 18.0, inner_rect.max.y - 18.0)
                    } else {
                        (inner_rect.min.y + scroll_fraction * view_h).clamp(inner_rect.min.y + 18.0, inner_rect.max.y - 18.0)
                    }
                } else {
                    (inner_rect.min.y + scroll_fraction * view_h).clamp(inner_rect.min.y + 18.0, inner_rect.max.y - 18.0)
                };

                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Tooltip,
                    egui::Id::new("scrollbar_date_scrubber_tooltip"),
                ));

                let display_str = format!("{}  {}", icon::CALENDAR, tooltip_text);
                let font_id = egui::FontId::proportional(FONT_SMALL);
                let text_layout = painter.layout_no_wrap(
                    display_str,
                    font_id,
                    TEXT_PRIMARY,
                );
                let text_size = text_layout.size();

                let padding_x = 10.0;
                let padding_y = 6.0;
                let bubble_w = text_size.x + padding_x * 2.0;
                let bubble_h = text_size.y + padding_y * 2.0;

                let bubble_right = inner_rect.max.x - scrollbar_gutter_w - 8.0;
                let bubble_rect = Rect::from_min_size(
                    Pos2::new(bubble_right - bubble_w, bubble_y - bubble_h / 2.0),
                    Vec2::new(bubble_w, bubble_h),
                );

                self.scrollbar_tooltip_rect = Some(bubble_rect);

                // Drop shadow
                painter.rect_filled(
                    bubble_rect.translate(Vec2::new(0.0, 2.0)),
                    Rounding::same(ROUNDING),
                    Color32::from_black_alpha(100),
                );

                // Pill container background
                painter.rect(
                    bubble_rect,
                    Rounding::same(ROUNDING),
                    BG_PANEL,
                    Stroke::new(1.0_f32, BORDER_STRONG),
                );

                // Text with calendar icon
                painter.galley(
                    Pos2::new(bubble_rect.min.x + padding_x, bubble_rect.min.y + padding_y),
                    text_layout,
                    TEXT_PRIMARY,
                );

                ui.ctx().request_repaint();
            } else {
                self.scrollbar_tooltip_rect = None;
            }
        } else {
            self.scrollbar_tooltip_rect = None;
        }

        action
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_month_label() {
        assert_eq!(format_month_label(Some("2024-06-15 14:30:00")), "June 2024");
        assert_eq!(format_month_label(Some("2026-10-11")), "October 2026");
        assert_eq!(format_month_label(Some("2023-01-01T12:00:00Z")), "January 2023");
        assert_eq!(format_month_label(None), "Undated");
        assert_eq!(format_month_label(Some("")), "Undated");
    }
}

