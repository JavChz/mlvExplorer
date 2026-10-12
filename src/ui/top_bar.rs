use crate::config::ThemeMode;
use crate::db::{FilterCriteria, PickFlag};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, RichText, Rounding, Stroke, Ui, Vec2};

pub enum TopBarAction {
    // File
    NewCatalog,
    OpenCatalog,
    ImportExistingCatalog,
    ExportCatalog,
    BackupCatalog,
    ImportFiles,
    ImportFolder,
    Exit,

    // Edit
    ManageKeywords,
    CopyClipSettings,
    PasteClipSettings,
    SelectAll,
    DeselectAll,
    RemoveFromCatalog,
    MoveToTrash,

    // View
    SwitchToGrid,
    SwitchToPlayer,
    ToggleFullscreen,
    ZoomIn,
    ZoomOut,
    ZoomReset,

    // Preferences
    SetTheme(ThemeMode),
    OpenSettingsModal,

    // Help
    OpenShortcutsModal,
    CheckForUpdates,
    OpenAboutModal,
}

pub struct TopBar;

impl TopBar {
    /// Renders the top native menu bar (~26px) with:
    /// File, Edit, View, Preferences, Help on the left,
    /// and the Grid / Player segmented switcher on the top right.
    pub fn show_menu_bar(
        ui: &mut Ui,
        is_player_mode: bool,
        can_open_player: bool,
    ) -> Option<TopBarAction> {
        let mut action = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);

            // Left side: Standard Native Menus
            egui::menu::bar(ui, |ui| {
                // File Menu
                ui.menu_button("File", |ui| {
                    if ui.add(egui::Button::new(widgets::icon_label(icon::FILE_PLUS, "New Catalog..."))).clicked() {
                        action = Some(TopBarAction::NewCatalog);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::FOLDER_OPEN, "Open Catalog..."))).clicked() {
                        action = Some(TopBarAction::OpenCatalog);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::DATABASE, "Import from Existing Catalog..."))).clicked() {
                        action = Some(TopBarAction::ImportExistingCatalog);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::EXPORT, "Export Catalog (JSON)..."))).clicked() {
                        action = Some(TopBarAction::ExportCatalog);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::FLOPPY_DISK, "Backup Catalog..."))).clicked() {
                        action = Some(TopBarAction::BackupCatalog);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::FILM_STRIP, "Import Video File(s)..."))).clicked() {
                        action = Some(TopBarAction::ImportFiles);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::FOLDER, "Import Videos from Folder..."))).clicked() {
                        action = Some(TopBarAction::ImportFolder);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::SIGN_OUT, "Exit"))).clicked() {
                        action = Some(TopBarAction::Exit);
                        ui.close_menu();
                    }
                });

                // Edit Menu
                ui.menu_button("Edit", |ui| {
                    if ui.add(egui::Button::new(widgets::icon_label(icon::TAG, "Manage Keywords..."))).clicked() {
                        action = Some(TopBarAction::ManageKeywords);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::COPY, "Copy Clip Settings   [Ctrl+C]"))).clicked() {
                        action = Some(TopBarAction::CopyClipSettings);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::CLIPBOARD, "Paste Clip Settings   [Ctrl+V]"))).clicked() {
                        action = Some(TopBarAction::PasteClipSettings);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new("Select All   [Ctrl+A]")).clicked() {
                        action = Some(TopBarAction::SelectAll);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Deselect All   [Ctrl+D]")).clicked() {
                        action = Some(TopBarAction::DeselectAll);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new("Remove from Catalog   [Del]")).clicked() {
                        action = Some(TopBarAction::RemoveFromCatalog);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Move to Trash   [Shift+Del]")).clicked() {
                        action = Some(TopBarAction::MoveToTrash);
                        ui.close_menu();
                    }
                });

                // View Menu
                ui.menu_button("View", |ui| {
                    if ui.add(egui::Button::new(widgets::icon_label(icon::GRID_FOUR, "Grid View   [G]"))).clicked() {
                        action = Some(TopBarAction::SwitchToGrid);
                        ui.close_menu();
                    }
                    ui.add_enabled_ui(can_open_player, |ui| {
                        if ui.add(egui::Button::new(widgets::icon_label(icon::PLAY, "Player View   [E]"))).clicked() {
                            action = Some(TopBarAction::SwitchToPlayer);
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::CORNERS_OUT, "Toggle Cinema Fullscreen   [F]"))).clicked() {
                        action = Some(TopBarAction::ToggleFullscreen);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::MAGNIFYING_GLASS_PLUS, "Zoom In   [Ctrl++]"))).clicked() {
                        action = Some(TopBarAction::ZoomIn);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new(widgets::icon_label(icon::MAGNIFYING_GLASS_MINUS, "Zoom Out   [Ctrl+-]"))).clicked() {
                        action = Some(TopBarAction::ZoomOut);
                        ui.close_menu();
                    }
                    if ui.add(egui::Button::new("Reset Zoom   [Ctrl+0]")).clicked() {
                        action = Some(TopBarAction::ZoomReset);
                        ui.close_menu();
                    }
                });

                // Preferences Menu
                ui.menu_button("Preferences", |ui| {
                    ui.menu_button("Theme", |ui| {
                        if ui.add(egui::Button::new("Dark Mode (Cinema)")).clicked() {
                            action = Some(TopBarAction::SetTheme(ThemeMode::Dark));
                            ui.close_menu();
                        }
                        if ui.add(egui::Button::new("Light Mode (Studio)")).clicked() {
                            action = Some(TopBarAction::SetTheme(ThemeMode::Light));
                            ui.close_menu();
                        }
                        if ui.add(egui::Button::new("System Default")).clicked() {
                            action = Some(TopBarAction::SetTheme(ThemeMode::System));
                            ui.close_menu();
                        }
                    });

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::GEAR, "Settings & Catalog Manager..."))).clicked() {
                        action = Some(TopBarAction::OpenSettingsModal);
                        ui.close_menu();
                    }
                });

                // Help Menu
                ui.menu_button("Help", |ui| {
                    if ui.add(egui::Button::new(widgets::icon_label(icon::KEYBOARD, "Keyboard Shortcuts   [F1]"))).clicked() {
                        action = Some(TopBarAction::OpenShortcutsModal);
                        ui.close_menu();
                    }

                    if ui.add(egui::Button::new(widgets::icon_label(icon::ARROWS_CLOCKWISE, "Check for Updates..."))).clicked() {
                        action = Some(TopBarAction::CheckForUpdates);
                        ui.close_menu();
                    }

                    ui.separator();

                    if ui.add(egui::Button::new(widgets::icon_label(icon::INFO, "About MLV Explorer..."))).clicked() {
                        action = Some(TopBarAction::OpenAboutModal);
                        ui.close_menu();
                    }
                });
            });

            // Right-aligned: Import buttons + Grid / Player segmented switcher
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                let grid_active = !is_player_mode;
                let player_active = is_player_mode;

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);
                    let is_dark = ui.visuals().dark_mode;
                    let active_fill = if is_dark { ACCENT_DIM } else { ACCENT };

                    ui.add_enabled_ui(can_open_player, |ui| {
                        let player_btn = egui::Button::new(
                            RichText::new(format!("{}  Player", icon::PLAY))
                                .size(FONT_SMALL)
                                .color(if player_active { Color32::WHITE } else { text_secondary(ui) }),
                        )
                        .fill(if player_active { active_fill } else { Color32::TRANSPARENT })
                        .stroke(if player_active { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(0.0, 22.0));

                        if ui.add(player_btn).on_hover_text("Player View [E]").clicked() {
                            action = Some(TopBarAction::SwitchToPlayer);
                        }
                    });

                    let grid_btn = egui::Button::new(
                        RichText::new(format!("{}  Grid", icon::GRID_FOUR))
                            .size(FONT_SMALL)
                            .color(if grid_active { Color32::WHITE } else { text_secondary(ui) }),
                    )
                    .fill(if grid_active { active_fill } else { Color32::TRANSPARENT })
                    .stroke(if grid_active { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                    .rounding(Rounding::same(ROUNDING))
                    .min_size(Vec2::new(0.0, 22.0));

                    if ui.add(grid_btn).on_hover_text("Grid View [G]").clicked() {
                        action = Some(TopBarAction::SwitchToGrid);
                    }
                });

                widgets::toolbar_divider(ui);

                let import_file_btn = egui::Button::new(
                    RichText::new(widgets::icon_label(icon::FILE_PLUS, "Import Files"))
                        .size(FONT_SMALL)
                        .color(Color32::WHITE)
                        .strong(),
                )
                .fill(ACCENT)
                .stroke(Stroke::NONE)
                .rounding(Rounding::same(ROUNDING))
                .min_size(Vec2::new(0.0, 22.0));

                if ui.add(import_file_btn).clicked() {
                    action = Some(TopBarAction::ImportFiles);
                }

                let import_folder_btn = egui::Button::new(
                    RichText::new(widgets::icon_label(icon::FOLDER_OPEN, "Import Folder"))
                        .size(FONT_SMALL)
                        .color(text_primary(ui)),
                )
                .fill(bg_card(ui))
                .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                .rounding(Rounding::same(ROUNDING))
                .min_size(Vec2::new(0.0, 22.0));

                if ui.add(import_folder_btn).clicked() {
                    action = Some(TopBarAction::ImportFolder);
                }
            });
        });

        action
    }

    /// Renders the Filter Bar inside the central clips panel (aligned next to the Library header).
    /// Contains: Search box, Rating dropdown, Flag pills, and Clear all filters button.
    pub fn show_filter_bar(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
    ) -> bool {
        let mut filter_changed = false;
        let is_dark = ui.visuals().dark_mode;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);

            // 1. Search Box with Dedicated Clear Button
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);
                let search_width = 210.0;
                let search_edit = ui.add_sized(
                    [search_width, 28.0],
                    egui::TextEdit::singleline(&mut filter.search_text)
                        .hint_text(widgets::hint("Search clips, cameras, lenses…"))
                        .margin(egui::vec2(8.0, 5.0)),
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

            // 2. Rating Filter Dropdown
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.label(RichText::new("Rating:").size(FONT_BODY).color(text_muted(ui)));

                let rating_text = match filter.min_rating {
                    None => "All Ratings".to_string(),
                    Some(5) => "★★★★★ (5 Stars)".to_string(),
                    Some(r) => format!("{}★+ ({} or more)", r, r),
                };

                egui::ComboBox::from_id_salt("filter_bar_rating_combo")
                    .selected_text(RichText::new(rating_text).size(FONT_BODY))
                    .width(145.0)
                    .show_ui(ui, |ui| {
                        if ui.selectable_label(filter.min_rating.is_none(), "All Ratings").clicked() {
                            filter.min_rating = None;
                            filter_changed = true;
                        }
                        for r in 1..=5 {
                            let label = if r == 5 {
                                "★★★★★  (5 Stars)".to_string()
                            } else {
                                let stars = "★".repeat(r as usize);
                                format!("{stars}+  ({r} or more)")
                            };
                            if ui.selectable_label(filter.min_rating == Some(r), label).clicked() {
                                filter.min_rating = Some(r);
                                filter_changed = true;
                            }
                        }
                    });
            });

            widgets::toolbar_divider(ui);

            // 3. Pick / Reject Flags Segmented Pill Group
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 0.0);
                ui.label(RichText::new("Flag:").size(FONT_BODY).color(text_muted(ui)));

                egui::Frame::none()
                    .fill(bg_field(ui))
                    .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                    .rounding(Rounding::same(ROUNDING + 2.0))
                    .inner_margin(egui::Margin::symmetric(3.0, 2.0))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(2.0, 0.0);

                        // "All" segment
                        let flag_all = filter.flag.is_none();
                        let all_btn = egui::Button::new(
                            RichText::new("All")
                                .size(FONT_SMALL + 0.5)
                                .color(if flag_all { Color32::WHITE } else { text_secondary(ui) })
                                .strong(),
                        )
                        .fill(if flag_all { if is_dark { ACCENT_DIM } else { ACCENT } } else { Color32::TRANSPARENT })
                        .stroke(if flag_all { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(32.0, 24.0));

                        if ui.add(all_btn).clicked() {
                            filter.flag = None;
                            filter_changed = true;
                        }

                        // Picked Flag (Green)
                        let is_pick = filter.flag == Some(PickFlag::Picked);
                        let pick_btn = egui::Button::new(
                            RichText::new(icon::FLAG)
                                .size(14.0)
                                .color(if is_pick { Color32::WHITE } else { FLAG_PICKED }),
                        )
                        .fill(if is_pick { FLAG_PICKED } else { Color32::TRANSPARENT })
                        .stroke(if is_pick { Stroke::new(1.0_f32, FLAG_PICKED) } else { Stroke::NONE })
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(30.0, 24.0));

                        if ui.add(pick_btn).on_hover_text("Show Picked clips [P]").clicked() {
                            filter.flag = if is_pick { None } else { Some(PickFlag::Picked) };
                            filter_changed = true;
                        }

                        // Rejected Flag (Red)
                        let is_rej = filter.flag == Some(PickFlag::Rejected);
                        let rej_btn = egui::Button::new(
                            RichText::new(icon::X_CIRCLE)
                                .size(14.0)
                                .color(if is_rej { Color32::WHITE } else { FLAG_REJECTED }),
                        )
                        .fill(if is_rej { FLAG_REJECTED } else { Color32::TRANSPARENT })
                        .stroke(if is_rej { Stroke::new(1.0_f32, FLAG_REJECTED) } else { Stroke::NONE })
                        .rounding(Rounding::same(ROUNDING))
                        .min_size(Vec2::new(30.0, 24.0));

                        if ui.add(rej_btn).on_hover_text("Show Rejected clips [X]").clicked() {
                            filter.flag = if is_rej { None } else { Some(PickFlag::Rejected) };
                            filter_changed = true;
                        }
                    });
            });

            widgets::toolbar_divider(ui);

            // 4. Clear All Filters Button
            let is_filtered = filter.is_filtered();
            ui.add_enabled_ui(is_filtered, |ui| {
                let clear_btn = egui::Button::new(
                    RichText::new(widgets::icon_label(icon::ARROW_COUNTER_CLOCKWISE, "Clear all filters"))
                        .size(FONT_BODY)
                        .color(if is_filtered { text_primary(ui) } else { text_disabled(ui) }),
                )
                .fill(if is_filtered { bg_card(ui) } else { Color32::TRANSPARENT })
                .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                .rounding(Rounding::same(ROUNDING))
                .min_size(Vec2::new(0.0, 28.0));

                if ui
                    .add(clear_btn)
                    .on_hover_text("Reset search, rating, flag, camera, lens, and keyword filters")
                    .clicked()
                {
                    let sort = filter.sort;
                    *filter = FilterCriteria {
                        sort,
                        ..Default::default()
                    };
                    filter_changed = true;
                }
            });
        });

        ui.add_space(4.0);
        let (sep_rect, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().rect_filled(sep_rect, Rounding::ZERO, border_subtle(ui));
        ui.add_space(4.0);

        filter_changed
    }
}
