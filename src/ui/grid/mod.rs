pub mod card;
pub mod types;

pub use card::Card;
pub use types::{GridAction, GridView};

use crate::db::{SortOrder, VideoRecord};
use crate::thumbnails::ThumbnailManager;
use crate::ui::theme::*;
use egui::{Color32, Pos2, Rect, RichText, Rounding, Stroke, Ui, Vec2};

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

fn format_month_label(date_str: Option<&str>) -> String {
    let Some(d) = date_str else {
        return "Undated".to_string();
    };
    let clean = d.trim();
    if clean.len() < 10 {
        return "Undated".to_string();
    }
    let date_part = &clean[..10];
    if let Ok(naive) = chrono::NaiveDate::parse_from_str(date_part, "%Y-%m-%d") {
        naive.format("%B %Y").to_string()
    } else {
        date_part.to_string()
    }
}

fn format_bytes(bytes: u64) -> String {
    if bytes >= 1024 * 1024 * 1024 {
        format!("{:.1} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    } else {
        format!("{:.0} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

fn format_duration(secs: f32) -> String {
    let s = secs.max(0.0) as u32;
    let m = s / 60;
    let rem = s % 60;
    format!("{:02}:{:02}", m, rem)
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
            // Sort Selector
            ui.label(RichText::new("Sort by:").small().color(TEXT_SECONDARY));
            egui::ComboBox::from_id_salt("grid_sort_selector")
                .selected_text(RichText::new(sort_order.label()).small())
                .show_ui(ui, |ui| {
                    for opt in SortOrder::ALL {
                        if ui.selectable_value(sort_order, opt, opt.label()).clicked() {
                            action = Some(GridAction::SortChanged);
                        }
                    }
                });

            ui.separator();

            // Group by Day Toggle
            let group_btn = ui
                .selectable_label(self.group_by_day, RichText::new("📅 Group by Day").small())
                .on_hover_text("Group clips by calendar capture date with clear day section headers");
            if group_btn.clicked() {
                self.group_by_day = !self.group_by_day;
            }

            ui.separator();

            // Thumbnail Zoom Slider
            ui.label(RichText::new("Zoom:").small().color(TEXT_SECONDARY));
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
                        "{} clips · {} · {}",
                        videos.len(),
                        format_bytes(total_size),
                        format_duration(total_dur)
                    ))
                    .small()
                    .color(TEXT_MUTED),
                );
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        if videos.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(100.0);
                ui.label(RichText::new("🎬").size(48.0));
                ui.add_space(8.0);
                ui.heading("No Magic Lantern Clips Matching Filter");
                ui.add_space(4.0);
                ui.label(RichText::new("Drag and drop .MLV files or folders anywhere onto the window.").color(TEXT_MUTED));
            });
            return action;
        }

        let card_w = self.card_size;
        let thumb_h = card_w * (9.0 / 16.0); // 16:9 thumbnail
        let footer_h = 68.0;
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
                    let (banner_rect, _) = ui.allocate_exact_size(Vec2::new(available_width, 28.0), egui::Sense::hover());
                    let painter = ui.painter();
                    painter.rect(
                        banner_rect,
                        Rounding::same(4.0),
                        Color32::from_rgb(22, 23, 29),
                        Stroke::new(1.0_f32, Color32::from_rgb(38, 42, 54)),
                    );

                    // Left title
                    painter.text(
                        Pos2::new(banner_rect.min.x + 10.0, banner_rect.center().y),
                        egui::Align2::LEFT_CENTER,
                        format!("📅 {}", grp.title),
                        egui::FontId::proportional(12.5),
                        Color32::from_rgb(243, 244, 246),
                    );

                    // Right badges
                    let grp_info = format!(
                        "{} clips · {} · {}",
                        grp.indices.len(),
                        format_bytes(grp.total_bytes),
                        format_duration(grp.total_duration_secs)
                    );
                    painter.text(
                        Pos2::new(banner_rect.max.x - 10.0, banner_rect.center().y),
                        egui::Align2::RIGHT_CENTER,
                        grp_info,
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(156, 163, 175),
                    );

                    ui.add_space(8.0);

                    // Grid for this day
                    egui::Grid::new(format!("day_grid_{}", grp_idx))
                        .spacing([12.0, 14.0])
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
                    .spacing([12.0, 14.0])
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

        // Reserve a right margin strip for the vertical scrollbar so dragging the scroll handle never triggers box select
        let scrollbar_width = ui.spacing().scroll.bar_width.max(16.0);
        let content_selectable_rect = Rect::from_min_max(
            inner_rect.min,
            Pos2::new((inner_rect.max.x - scrollbar_width).max(inner_rect.min.x), inner_rect.max.y),
        );

        // Check if user pressed left mouse button
        if pointer.primary_pressed() {
            if let Some(press_pos) = pointer.press_origin() {
                // Must be strictly within selectable content area and not touching the scrollbar or a card
                if content_selectable_rect.contains(press_pos) {
                    let hit_card = card_rects.iter().any(|(_, r)| r.contains(press_pos));
                    if !hit_card {
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
                    ui.ctx().request_repaint(); // Keep repainting while dragging
                }
            }
        }

        if let (Some(start), Some(curr)) = (self.drag_start_pos, self.drag_current_pos) {
            let drag_dist = (curr - start).length();
            if drag_dist > 4.0 {
                ui.ctx().set_cursor_icon(egui::CursorIcon::Crosshair);
                let box_rect = Rect::from_two_pos(start, curr);

                // Draw translucent selection rectangle with vibrant blue border on top layer
                let painter = ui.ctx().layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("grid_selection_box"),
                ));
                painter.rect(
                    box_rect,
                    Rounding::same(2.0),
                    Color32::from_rgba_unmultiplied(59, 130, 246, 50),
                    Stroke::new(1.5_f32, Color32::from_rgb(96, 165, 250)),
                );

                // Find all cards intersecting box_rect
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
                // Clicked on empty space without dragging -> Deselect all
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
        // Shows the current month/year floating tooltip next to the scroll handle (like Google Photos / Apple Photos)
        let total_content_h = scroll_output.content_size.y;
        let view_h = inner_rect.height();
        if total_content_h > view_h && !videos.is_empty() {
            let scrollbar_zone = Rect::from_min_max(
                Pos2::new(inner_rect.max.x - scrollbar_width - 8.0, inner_rect.min.y),
                Pos2::new(inner_rect.max.x + 4.0, inner_rect.max.y),
            );

            let pointer_pos = pointer.latest_pos();
            let is_scrollbar_hovered = pointer_pos.map_or(false, |p| scrollbar_zone.contains(p));
            let is_scrollbar_dragging = is_scrollbar_hovered && pointer.primary_down() && self.drag_start_pos.is_none();

            if is_scrollbar_hovered || is_scrollbar_dragging {
                // Determine the scroll fraction: if dragging or hovering over scrollbar, calculate from pointer Y; otherwise from current scroll offset
                let scroll_fraction = if let Some(p) = pointer_pos {
                    if is_scrollbar_dragging {
                        ((p.y - inner_rect.min.y) / view_h).clamp(0.0, 1.0)
                    } else {
                        (scroll_output.state.offset.y / (total_content_h - view_h).max(1.0)).clamp(0.0, 1.0)
                    }
                } else {
                    (scroll_output.state.offset.y / (total_content_h - view_h).max(1.0)).clamp(0.0, 1.0)
                };

                // Approximate video index corresponding to this scroll fraction
                let approx_idx = ((scroll_fraction * (videos.len().saturating_sub(1) as f32)).round() as usize)
                    .min(videos.len().saturating_sub(1));

                let date_str = videos[approx_idx].date_recorded.as_deref();
                let tooltip_text = format_month_label(date_str);

                // Compute Y position for the bubble: follow the mouse Y if dragging, otherwise follow scrollbar handle position
                let bubble_y = if let Some(p) = pointer_pos {
                    if is_scrollbar_dragging {
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

                // Measure text
                let font_id = egui::FontId::proportional(12.5);
                let text_layout = painter.layout_no_wrap(
                    tooltip_text,
                    font_id,
                    Color32::from_rgb(255, 255, 255),
                );
                let text_size = text_layout.size();

                let padding_x = 10.0;
                let padding_y = 6.0;
                let bubble_w = text_size.x + padding_x * 2.0;
                let bubble_h = text_size.y + padding_y * 2.0;

                // Position bubble to the left of the scrollbar
                let bubble_right = inner_rect.max.x - scrollbar_width - 10.0;
                let bubble_rect = Rect::from_min_size(
                    Pos2::new(bubble_right - bubble_w, bubble_y - bubble_h / 2.0),
                    Vec2::new(bubble_w, bubble_h),
                );

                // Draw drop shadow
                let shadow_rect = bubble_rect.translate(Vec2::new(0.0, 2.0));
                painter.rect_filled(
                    shadow_rect,
                    Rounding::same(6.0),
                    Color32::from_black_alpha(80),
                );

                // Draw pill background (glassmorphic dark pill with subtle border)
                painter.rect(
                    bubble_rect,
                    Rounding::same(6.0),
                    Color32::from_rgb(26, 28, 36),
                    Stroke::new(1.0_f32, Color32::from_rgb(60, 66, 82)),
                );

                // Draw text
                painter.galley(
                    Pos2::new(bubble_rect.min.x + padding_x, bubble_rect.min.y + padding_y),
                    text_layout,
                    Color32::from_rgb(243, 244, 246),
                );
            }
        }

        action
    }
}
