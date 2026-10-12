use crate::db::{FilterCriteria, PickFlag, SortOrder};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, RichText, Rounding, Stroke, Ui, Vec2};

pub enum TopBarAction {
    ImportFiles,
    ImportFolder,
    OpenCatalogModal,
    SwitchToGrid,
    SwitchToPlayer,
}

pub struct TopBar;

impl TopBar {
    pub fn show(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        is_player_mode: bool,
        can_open_player: bool,
        _cameras: &[String],
        _lenses: &[String],
    ) -> (bool, Option<TopBarAction>) {
        let mut filter_changed = false;
        let mut action = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

            // App Brand Name & Icon
            ui.add_space(4.0);
            ui.label(
                RichText::new(icon::FILM_STRIP)
                    .size(15.0)
                    .color(ACCENT),
            );
            ui.label(
                RichText::new("MLV EXPLORER")
                    .strong()
                    .size(FONT_BODY)
                    .color(TEXT_PRIMARY)
                    .extra_letter_spacing(0.8),
            );
            ui.add_space(8.0);

            widgets::toolbar_divider(ui);

            // Workspace View Mode Switcher (Grid / Player segmented control)
            let grid_active = !is_player_mode;
            let player_active = is_player_mode;

            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                let grid_btn = egui::Button::new(
                    RichText::new(format!("{}  Grid", icon::GRID_FOUR))
                        .size(FONT_SMALL)
                        .color(if grid_active { Color32::WHITE } else { TEXT_SECONDARY }),
                )
                .fill(if grid_active { ACCENT_DIM } else { Color32::TRANSPARENT })
                .stroke(if grid_active { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                .rounding(Rounding::same(ROUNDING))
                .min_size(Vec2::new(0.0, CONTROL_HEIGHT));

                if ui.add(grid_btn).on_hover_text("Grid View [G]").clicked() {
                    action = Some(TopBarAction::SwitchToGrid);
                }

                ui.add_enabled_ui(can_open_player, |ui| {
                    let player_btn = egui::Button::new(
                        RichText::new(format!("{}  Player", icon::PLAY))
                            .size(FONT_SMALL)
                            .color(if player_active { Color32::WHITE } else { TEXT_SECONDARY }),
                    )
                    .fill(if player_active { ACCENT_DIM } else { Color32::TRANSPARENT })
                    .stroke(if player_active { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                    .rounding(Rounding::same(ROUNDING))
                    .min_size(Vec2::new(0.0, CONTROL_HEIGHT));

                    if ui.add(player_btn).on_hover_text("Player View [E]").clicked() {
                        action = Some(TopBarAction::SwitchToPlayer);
                    }
                });
            });

            widgets::toolbar_divider(ui);

            // Filter Search Input with Clear Button and dedicated Hint Color
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                let search_width = 170.0;
                let search_edit = ui.add_sized(
                    [search_width, CONTROL_HEIGHT],
                    egui::TextEdit::singleline(&mut filter.search_text)
                        .hint_text(widgets::hint("Search clips, cameras, lenses…"))
                        .margin(egui::vec2(6.0, 3.0)),
                );
                if search_edit.changed() {
                    filter_changed = true;
                }

                if !filter.search_text.is_empty() {
                    if widgets::icon_button(ui, icon::X, false, "Clear search").clicked() {
                        filter.search_text.clear();
                        filter_changed = true;
                    }
                }
            });

            widgets::toolbar_divider(ui);

            // Sort Selector
            ui.label(RichText::new("Sort:").size(FONT_SMALL).color(TEXT_MUTED));
            egui::ComboBox::from_id_salt("topbar_sort_order_combo")
                .selected_text(RichText::new(filter.sort.label()).size(FONT_SMALL))
                .show_ui(ui, |ui| {
                    for sort_opt in SortOrder::ALL {
                        if ui.selectable_value(&mut filter.sort, sort_opt, sort_opt.label()).clicked() {
                            filter_changed = true;
                        }
                    }
                });

            widgets::toolbar_divider(ui);

            // Rating Filter Quick Toggles (All, 1★+, 2★+, 3★+, 4★+, 5★)
            ui.label(RichText::new("Rating:").size(FONT_SMALL).color(TEXT_MUTED));
            let r_all = filter.min_rating.is_none();
            if widgets::toggle_button(ui, r_all, "All").clicked() {
                filter.min_rating = None;
                filter_changed = true;
            }
            for r in 1..=5 {
                let is_sel = filter.min_rating == Some(r);
                let star_label = format!("{}★+", r);
                let btn = egui::Button::new(
                    RichText::new(star_label)
                        .size(FONT_CAPTION)
                        .color(if is_sel { RATING } else { TEXT_SECONDARY }),
                )
                .fill(if is_sel { ACCENT_DIM } else { Color32::TRANSPARENT })
                .stroke(if is_sel { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                .min_size(Vec2::new(0.0, CONTROL_HEIGHT));

                if ui.add(btn).clicked() {
                    filter.min_rating = if is_sel { None } else { Some(r) };
                    filter_changed = true;
                }
            }

            widgets::toolbar_divider(ui);

            // Pick / Reject Flags filter
            ui.label(RichText::new("Flag:").size(FONT_SMALL).color(TEXT_MUTED));
            let flag_all = filter.flag.is_none();
            if widgets::toggle_button(ui, flag_all, "All").clicked() {
                filter.flag = None;
                filter_changed = true;
            }

            let is_pick = filter.flag == Some(PickFlag::Picked);
            if widgets::icon_button_colored(
                ui,
                icon::FLAG,
                is_pick,
                Some(FLAG_PICKED),
                "Filter Picks",
            ).clicked() {
                filter.flag = if is_pick { None } else { Some(PickFlag::Picked) };
                filter_changed = true;
            }

            let is_rej = filter.flag == Some(PickFlag::Rejected);
            if widgets::icon_button_colored(
                ui,
                icon::X_CIRCLE,
                is_rej,
                Some(FLAG_REJECTED),
                "Filter Rejected",
            ).clicked() {
                filter.flag = if is_rej { None } else { Some(PickFlag::Rejected) };
                filter_changed = true;
            }

            // Right-aligned actions: Import & Catalog
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                if filter.is_filtered() {
                    let reset_btn = egui::Button::new(
                        RichText::new(format!("{} Reset", icon::ARROW_COUNTER_CLOCKWISE))
                            .size(FONT_SMALL)
                            .color(TEXT_MUTED),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0_f32, BORDER_SUBTLE))
                    .min_size(Vec2::new(0.0, CONTROL_HEIGHT));

                    if ui.add(reset_btn).on_hover_text("Clear all filters").clicked() {
                        *filter = FilterCriteria::default();
                        filter_changed = true;
                    }
                    widgets::toolbar_divider(ui);
                }

                if widgets::button(ui, widgets::icon_label(icon::DATABASE, "Backup")).on_hover_text("Catalog Backup & Migration").clicked() {
                    action = Some(TopBarAction::OpenCatalogModal);
                }

                if widgets::button(ui, widgets::icon_label(icon::FOLDER_OPEN, "Import Folder")).clicked() {
                    action = Some(TopBarAction::ImportFolder);
                }

                if widgets::primary_button(ui, widgets::icon_label(icon::FILE_PLUS, "Import Files")).clicked() {
                    action = Some(TopBarAction::ImportFiles);
                }
            });
        });

        (filter_changed, action)
    }
}
