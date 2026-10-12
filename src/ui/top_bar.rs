use crate::config::ThemeMode;
use crate::db::{FilterCriteria, PickFlag, SortOrder};
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

            // Right-aligned: Grid / Player segmented switcher
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
            });
        });

        action
    }

    /// Renders the ergonomic, dedicated Workspace Toolbar row (~42px)
    /// containing: Search, Sort dropdown, Rating pills, Flag pills, Reset, and Import buttons.
    pub fn show_toolbar(
        ui: &mut Ui,
        filter: &mut FilterCriteria,
        card_size: &mut f32,
    ) -> (bool, Option<TopBarAction>) {
        let mut filter_changed = false;
        let mut action = None;

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

            // 1. Search Box with Dedicated Clear Button
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing = egui::vec2(3.0, 0.0);
                let search_width = 190.0;
                let search_edit = ui.add_sized(
                    [search_width, 24.0],
                    egui::TextEdit::singleline(&mut filter.search_text)
                        .hint_text(widgets::hint("Search clips, cameras, lenses…"))
                        .margin(egui::vec2(6.0, 4.0)),
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

            // 2. Sort Selector
            ui.label(RichText::new("Sort:").size(FONT_SMALL).color(text_muted(ui)));
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

            // 3. Rating Filter Quick Toggles (All, 1★+, 2★+, 3★+, 4★+, 5★)
            ui.label(RichText::new("Rating:").size(FONT_SMALL).color(text_muted(ui)));
            let r_all = filter.min_rating.is_none();
            if widgets::toggle_button(ui, r_all, "All").clicked() {
                filter.min_rating = None;
                filter_changed = true;
            }
            let is_dark = ui.visuals().dark_mode;
            for r in 1..=5 {
                let is_sel = filter.min_rating == Some(r);
                let star_label = format!("{}★+", r);
                let star_text_col = if is_sel { RATING } else { text_secondary(ui) };
                let star_fill = if is_sel {
                    if is_dark { ACCENT_DIM } else { Color32::from_rgb(220, 234, 254) }
                } else {
                    Color32::TRANSPARENT
                };
                let btn = egui::Button::new(
                    RichText::new(star_label)
                        .size(FONT_CAPTION)
                        .color(star_text_col),
                )
                .fill(star_fill)
                .stroke(if is_sel { Stroke::new(1.0_f32, ACCENT) } else { Stroke::NONE })
                .min_size(Vec2::new(0.0, 22.0));

                if ui.add(btn).clicked() {
                    filter.min_rating = if is_sel { None } else { Some(r) };
                    filter_changed = true;
                }
            }

            widgets::toolbar_divider(ui);

            // 4. Pick / Reject Flags filter
            ui.label(RichText::new("Flag:").size(FONT_SMALL).color(text_muted(ui)));
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
                "Filter Picked",
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

            // 5. Card Zoom Slider
            widgets::toolbar_divider(ui);
            ui.label(RichText::new(icon::MAGNIFYING_GLASS).size(FONT_SMALL).color(text_muted(ui)));
            ui.add(
                egui::Slider::new(card_size, 160.0..=480.0)
                    .show_value(false)
                    .trailing_fill(true),
            );

            // Right-aligned Actions: Reset filter & Import Buttons
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(4.0);

                if filter.is_filtered() {
                    let reset_btn = egui::Button::new(
                        RichText::new(format!("{} Reset", icon::ARROW_COUNTER_CLOCKWISE))
                            .size(FONT_SMALL)
                            .color(text_muted(ui)),
                    )
                    .fill(Color32::TRANSPARENT)
                    .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                    .min_size(Vec2::new(0.0, 24.0));

                    if ui.add(reset_btn).on_hover_text("Clear all filters").clicked() {
                        *filter = FilterCriteria::default();
                        filter_changed = true;
                    }
                    widgets::toolbar_divider(ui);
                }

                if widgets::primary_button(ui, widgets::icon_label(icon::FILE_PLUS, "Import Files")).clicked() {
                    action = Some(TopBarAction::ImportFiles);
                }

                if widgets::button(ui, widgets::icon_label(icon::FOLDER_OPEN, "Import Folder")).clicked() {
                    action = Some(TopBarAction::ImportFolder);
                }
            });
        });

        (filter_changed, action)
    }
}
