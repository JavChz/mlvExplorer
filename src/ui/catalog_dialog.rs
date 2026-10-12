use crate::config::{AppConfig, ThemeMode};
use crate::db::Database;
use crate::launcher::ExternalLauncher;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Color32, Context, RichText, Rounding, Stroke, Vec2, Window};
use std::path::PathBuf;

pub enum SettingsAction {
    CatalogChanged,
    SwitchCatalog(PathBuf),
}

pub struct SettingsDialog {
    pub is_open: bool,
    pub status_message: Option<(String, bool)>, // (message, is_error)
    pub mlv_app_path_buf: String,
}

impl Default for SettingsDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            status_message: None,
            mlv_app_path_buf: String::new(),
        }
    }
}

impl SettingsDialog {
    pub fn open(&mut self, config: &AppConfig) {
        self.is_open = true;
        self.status_message = None;
        self.mlv_app_path_buf = config
            .mlv_app_path
            .as_ref()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    pub fn show(&mut self, ctx: &Context, db: &Database, config: &mut AppConfig) -> Option<SettingsAction> {
        if !self.is_open {
            return None;
        }

        let mut action = None;
        let title = widgets::icon_label(icon::GEAR, "Settings & Preferences");
        let mut is_open = self.is_open;
        let mut close_requested = false;

        Window::new(RichText::new(title).strong().size(FONT_BODY + 1.0))
            .open(&mut is_open)
            .resizable(true)
            .collapsible(false)
            .constrain(true)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .default_size([500.0, 520.0])
            .min_size([440.0, 380.0])
            .max_size([540.0, 720.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);

                        // ========================================================
                        // 1. APPEARANCE & THEME (Quick Selector)
                        // ========================================================
                        Self::settings_card(
                            ui,
                            icon::SUN,
                            "Appearance & Theme",
                            "Select your preferred visual style for the entire interface.",
                            |ui| {
                                ui.horizontal(|ui| {
                                    let spacing = ui.spacing().item_spacing.x;
                                    let btn_w = ((ui.available_width() - spacing * 2.0) / 3.0).floor().max(80.0);

                                    if Self::theme_toggle_button(ui, "Dark", icon::MOON, config.theme_mode == ThemeMode::Dark, btn_w) {
                                        config.theme_mode = ThemeMode::Dark;
                                        config.save();
                                        apply_theme(ui.ctx(), config.theme_mode);
                                    }
                                    if Self::theme_toggle_button(ui, "Light", icon::SUN, config.theme_mode == ThemeMode::Light, btn_w) {
                                        config.theme_mode = ThemeMode::Light;
                                        config.save();
                                        apply_theme(ui.ctx(), config.theme_mode);
                                    }
                                    if Self::theme_toggle_button(ui, "System", icon::DESKTOP, config.theme_mode == ThemeMode::System, btn_w) {
                                        config.theme_mode = ThemeMode::System;
                                        config.save();
                                        apply_theme(ui.ctx(), config.theme_mode);
                                    }
                                });
                            },
                        );

                        // ========================================================
                        // 2. ACTIVE CATALOG & LIBRARY (Full Width Card)
                        // ========================================================
                        Self::settings_card(
                            ui,
                            icon::DATABASE,
                            "Catalog Library",
                            "Active SQLite catalog database storing all clip metadata, tags, and ratings.",
                            |ui| {
                                ui.label(RichText::new("Active Catalog File:").size(FONT_CAPTION).color(text_muted(ui)));

                                // Full-width Path Input Well
                                let path_str = db.path.to_string_lossy();
                                let btn_reveal_w = 76.0;
                                let spacing = ui.spacing().item_spacing.x;
                                let frame_outer_w = (ui.available_width() - btn_reveal_w - spacing).max(100.0);
                                let h_pad = 8.0;
                                let frame_inner_w = (frame_outer_w - (h_pad * 2.0)).max(50.0);

                                ui.horizontal(|ui| {
                                    egui::Frame::none()
                                        .fill(bg_field(ui))
                                        .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                                        .rounding(Rounding::same(ROUNDING + 1.0))
                                        .inner_margin(egui::Margin::symmetric(h_pad, 4.0))
                                        .show(ui, |ui| {
                                            ui.set_max_width(frame_inner_w);
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(icon::DATABASE).size(FONT_BODY).color(ACCENT));
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(path_str.as_ref())
                                                            .size(FONT_SMALL)
                                                            .monospace()
                                                            .color(text_primary(ui)),
                                                    )
                                                    .truncate(),
                                                )
                                                .on_hover_text(path_str.as_ref());
                                            });
                                        });

                                    if ui.add(
                                        egui::Button::new(RichText::new(widgets::icon_label(icon::FOLDER_OPEN, "Reveal")).size(FONT_SMALL))
                                            .min_size(Vec2::new(btn_reveal_w, CONTROL_HEIGHT + 2.0)),
                                    )
                                    .on_hover_text("Open folder containing catalog database")
                                    .clicked()
                                    {
                                        let _ = open::that(&db.path);
                                    }
                                });

                                ui.add_space(4.0);

                                // Proportional 3-Column Catalog Action Buttons
                                ui.horizontal(|ui| {
                                    let spacing = ui.spacing().item_spacing.x;
                                    let btn_w = ((ui.available_width() - spacing * 2.0) / 3.0).floor().max(80.0);

                                    // 1. Create New Catalog
                                    let btn_new = egui::Button::new(RichText::new(widgets::icon_label(icon::FILE_PLUS, "New Catalog…")).size(FONT_SMALL))
                                        .min_size(Vec2::new(btn_w, CONTROL_HEIGHT + 2.0));
                                    if ui.add(btn_new).on_hover_text("Create a fresh, empty catalog database").clicked() {
                                        if let Some(new_path) = rfd::FileDialog::new()
                                            .set_title("Create New MLV Explorer Catalog")
                                            .set_file_name("my_mlv_catalog.db")
                                            .add_filter("SQLite Catalog (.db)", &["db", "sqlite"])
                                            .save_file()
                                        {
                                            match Database::open(&new_path) {
                                                Ok(_) => {
                                                    config.last_catalog_path = Some(new_path.clone());
                                                    config.save();
                                                    self.status_message = Some((
                                                        format!("Created catalog: {}", new_path.file_name().unwrap_or_default().to_string_lossy()),
                                                        false,
                                                    ));
                                                    action = Some(SettingsAction::SwitchCatalog(new_path));
                                                }
                                                Err(e) => {
                                                    self.status_message = Some((
                                                        format!("Failed to create catalog: {e}"),
                                                        true,
                                                    ));
                                                }
                                            }
                                        }
                                    }

                                    // 2. Open Existing Catalog
                                    let btn_open = egui::Button::new(RichText::new(widgets::icon_label(icon::FOLDER, "Open Catalog…")).size(FONT_SMALL))
                                        .min_size(Vec2::new(btn_w, CONTROL_HEIGHT + 2.0));
                                    if ui.add(btn_open).on_hover_text("Switch to an existing .db catalog file").clicked() {
                                        if let Some(open_path) = rfd::FileDialog::new()
                                            .set_title("Open Existing MLV Explorer Catalog")
                                            .add_filter("SQLite Catalog (.db)", &["db", "sqlite"])
                                            .pick_file()
                                        {
                                            if open_path.is_file() {
                                                config.last_catalog_path = Some(open_path.clone());
                                                config.save();
                                                self.status_message = Some((
                                                    format!("Switched to catalog: {}", open_path.file_name().unwrap_or_default().to_string_lossy()),
                                                    false,
                                                ));
                                                action = Some(SettingsAction::SwitchCatalog(open_path));
                                            }
                                        }
                                    }

                                    // 3. Import / Merge Catalog
                                    let btn_import = egui::Button::new(RichText::new(widgets::icon_label(icon::STACK, "Import & Merge…")).size(FONT_SMALL))
                                        .min_size(Vec2::new(btn_w, CONTROL_HEIGHT + 2.0));
                                    if ui.add(btn_import).on_hover_text("Merge metadata from external .db or .json catalog").clicked() {
                                        if let Some(import_path) = rfd::FileDialog::new()
                                            .set_title("Import Catalog into Current Library")
                                            .add_filter("Catalog (.db, .json)", &["db", "sqlite", "json"])
                                            .pick_file()
                                        {
                                            let ext = import_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                                            if ext == "json" {
                                                match db.import_catalog_json(&import_path) {
                                                    Ok(count) => {
                                                        self.status_message = Some((
                                                            format!("Merged {count} clips from JSON catalog"),
                                                            false,
                                                        ));
                                                        action = Some(SettingsAction::CatalogChanged);
                                                    }
                                                    Err(e) => {
                                                        self.status_message = Some((format!("JSON Import error: {e}"), true));
                                                    }
                                                }
                                            } else {
                                                match db.import_from_catalog_db(&import_path) {
                                                    Ok(count) => {
                                                        self.status_message = Some((
                                                            format!("Merged {count} clips from database catalog"),
                                                            false,
                                                        ));
                                                        action = Some(SettingsAction::CatalogChanged);
                                                    }
                                                    Err(e) => {
                                                        self.status_message = Some((format!("Database Import error: {e}"), true));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                });
                            },
                        );

                        // ========================================================
                        // 3. BACKUP & ARCHIVAL (Clean Card with Full Width Banner)
                        // ========================================================
                        Self::settings_card(
                            ui,
                            icon::FLOPPY_DISK,
                            "Catalog Backups & Archival",
                            "Create point-in-time snapshots and portable JSON archives of your library.",
                            |ui| {
                                // Full-width Prominent Info Callout Banner
                                let is_dark = ui.visuals().dark_mode;
                                let banner_bg = if is_dark {
                                    ACCENT_DIM.gamma_multiply(0.40)
                                } else {
                                    Color32::from_rgb(234, 243, 255)
                                };
                                let banner_stroke = if is_dark {
                                    Stroke::new(1.0_f32, ACCENT.gamma_multiply(0.60))
                                } else {
                                    Stroke::new(1.0_f32, Color32::from_rgb(180, 210, 250))
                                };

                                egui::Frame::none()
                                    .fill(banner_bg)
                                    .stroke(banner_stroke)
                                    .rounding(Rounding::same(ROUNDING + 1.0))
                                    .inner_margin(egui::Margin::symmetric(12.0, 8.0))
                                    .show(ui, |ui| {
                                        ui.set_max_width(ui.available_width());
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(icon::INFO).size(18.0).color(ACCENT_HOVER));
                                            ui.vertical(|ui| {
                                                ui.label(
                                                    RichText::new("Catalog-Only Snapshots")
                                                        .strong()
                                                        .size(FONT_BODY)
                                                        .color(text_primary(ui)),
                                                );
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(
                                                            "Backups save your metadata, ratings, keywords, and slates.\nOriginal RAW video files (.MLV, .M00…) on disk are not duplicated.",
                                                        )
                                                        .size(FONT_SMALL)
                                                        .color(text_secondary(ui)),
                                                    )
                                                    .wrap(),
                                                );
                                            });
                                        });
                                    });

                                ui.add_space(6.0);

                                // Backup Destination Folder Well
                                ui.label(RichText::new("Backup Destination Folder:").size(FONT_CAPTION).color(text_muted(ui)));
                                let folder_display = config
                                    .backup_folder
                                    .as_ref()
                                    .map(|p| p.to_string_lossy().to_string())
                                    .unwrap_or_else(|| "Not configured (will prompt on each backup)".to_string());
                                let is_configured = config.backup_folder.is_some();

                                let btn_browse_w = 76.0;
                                let spacing = ui.spacing().item_spacing.x;
                                let frame_outer_w = (ui.available_width() - btn_browse_w - spacing).max(100.0);
                                let h_pad = 8.0;
                                let frame_inner_w = (frame_outer_w - (h_pad * 2.0)).max(50.0);

                                ui.horizontal(|ui| {
                                    egui::Frame::none()
                                        .fill(bg_field(ui))
                                        .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                                        .rounding(Rounding::same(ROUNDING + 1.0))
                                        .inner_margin(egui::Margin::symmetric(h_pad, 4.0))
                                        .show(ui, |ui| {
                                            ui.set_max_width(frame_inner_w);
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(icon::FOLDER).size(FONT_BODY).color(text_muted(ui)));
                                                let text_col = if is_configured { text_primary(ui) } else { text_hint(ui) };
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&folder_display)
                                                            .size(FONT_SMALL)
                                                            .monospace()
                                                            .color(text_col),
                                                    )
                                                    .truncate(),
                                                )
                                                .on_hover_text(&folder_display);
                                            });
                                        });

                                    if ui.add(
                                        egui::Button::new(RichText::new(widgets::icon_label(icon::FOLDER_OPEN, "Browse…")).size(FONT_SMALL))
                                            .min_size(Vec2::new(btn_browse_w, CONTROL_HEIGHT + 2.0)),
                                    ).clicked() {
                                        if let Some(folder) = rfd::FileDialog::new()
                                            .set_title("Select Catalog Backup Directory")
                                            .pick_folder()
                                        {
                                            config.backup_folder = Some(folder);
                                            config.save();
                                        }
                                    }
                                });

                                ui.add_space(4.0);

                                // Proportional 2-Column Action Buttons
                                ui.horizontal(|ui| {
                                    let spacing = ui.spacing().item_spacing.x;
                                    let btn_w = ((ui.available_width() - spacing) / 2.0).floor().max(100.0);

                                    // 1. Instant Backup
                                    let btn_backup = egui::Button::new(
                                        RichText::new(widgets::icon_label(icon::FLOPPY_DISK, "Create Backup Now"))
                                            .strong()
                                            .size(FONT_SMALL)
                                            .color(Color32::WHITE),
                                    )
                                    .fill(ACCENT)
                                    .stroke(Stroke::NONE)
                                    .min_size(Vec2::new(btn_w, CONTROL_HEIGHT + 2.0));

                                    if ui.add(btn_backup).on_hover_text("Create timestamped SQLite backup snapshot now").clicked() {
                                        let backup_dir = config.backup_folder.clone().or_else(|| {
                                            rfd::FileDialog::new()
                                                .set_title("Select Backup Destination Folder")
                                                .pick_folder()
                                        });

                                        if let Some(dir) = backup_dir {
                                            config.backup_folder = Some(dir.clone());
                                            config.save();

                                            match db.backup_to_folder(&dir) {
                                                Ok(path) => {
                                                    self.status_message = Some((
                                                        format!("Catalog snapshot saved: {}", path.file_name().unwrap_or_default().to_string_lossy()),
                                                        false,
                                                    ));
                                                }
                                                Err(e) => {
                                                    self.status_message = Some((format!("Backup failed: {e}"), true));
                                                }
                                            }
                                        }
                                    }

                                    // 2. Export JSON
                                    let btn_json = egui::Button::new(
                                        RichText::new(widgets::icon_label(icon::UPLOAD_SIMPLE, "Export JSON Archive…"))
                                            .size(FONT_SMALL),
                                    )
                                    .min_size(Vec2::new(btn_w, CONTROL_HEIGHT + 2.0));

                                    if ui.add(btn_json).on_hover_text("Export human-readable portable JSON catalog").clicked() {
                                        if let Some(dest) = rfd::FileDialog::new()
                                            .set_file_name("mlv_catalog_export.json")
                                            .add_filter("JSON Document", &["json"])
                                            .save_file()
                                        {
                                            match db.export_catalog_json(&dest) {
                                                Ok(_) => {
                                                    self.status_message = Some((
                                                        format!("Exported portable JSON to {}", dest.file_name().unwrap_or_default().to_string_lossy()),
                                                        false,
                                                    ));
                                                }
                                                Err(e) => {
                                                    self.status_message = Some((format!("Export error: {e}"), true));
                                                }
                                            }
                                        }
                                    }
                                });
                            },
                        );

                        // ========================================================
                        // 4. MLV APP INTEGRATION (External Executable Card)
                        // ========================================================
                        Self::settings_card(
                            ui,
                            icon::ARROW_SQUARE_OUT,
                            "MLV App Integration",
                            "Configure MLV App executable for direct external grading, debayering, and exports.",
                            |ui| {
                                // Status Badge
                                let exe_exists = config.mlv_app_path.as_ref().map(|p| p.is_file()).unwrap_or(false);
                                let is_dark = ui.visuals().dark_mode;
                                let (status_text, status_col, status_bg) = if exe_exists {
                                    ("✓ Configured", FLAG_PICKED, if is_dark { Color32::from_rgb(20, 42, 26) } else { Color32::from_rgb(228, 248, 232) })
                                } else if config.mlv_app_path.is_some() {
                                    ("⚠ File Not Found", FLAG_REJECTED, if is_dark { Color32::from_rgb(46, 20, 20) } else { Color32::from_rgb(254, 232, 232) })
                                } else {
                                    ("Optional (Not Configured)", text_muted(ui), bg_field(ui))
                                };

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new("Executable Path:").size(FONT_CAPTION).color(text_muted(ui)));
                                    ui.allocate_ui_with_layout(
                                        egui::vec2(ui.available_width(), 18.0),
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            egui::Frame::none()
                                                .fill(status_bg)
                                                .stroke(Stroke::new(1.0_f32, status_col.gamma_multiply(0.4)))
                                                .rounding(Rounding::same(ROUNDING))
                                                .inner_margin(egui::Margin::symmetric(6.0, 2.0))
                                                .show(ui, |ui| {
                                                    ui.label(RichText::new(status_text).size(FONT_CAPTION).color(status_col));
                                                });
                                        },
                                    );
                                });

                                ui.add_space(2.0);

                                // Full-width Input + Actions Row
                                ui.horizontal(|ui| {
                                    let btn_browse_w = 76.0;
                                    let btn_auto_w = 96.0;
                                    let spacing = ui.spacing().item_spacing.x;
                                    let text_w = (ui.available_width() - btn_browse_w - btn_auto_w - (spacing * 2.0)).max(100.0);

                                    let resp = ui.add_sized(
                                        [text_w, CONTROL_HEIGHT + 2.0],
                                        egui::TextEdit::singleline(&mut self.mlv_app_path_buf)
                                            .hint_text(widgets::hint("e.g. C:\\Program Files\\MLV App\\MLV App.exe"))
                                            .margin(egui::vec2(6.0, 3.0)),
                                    );
                                    if resp.changed() {
                                        let p = PathBuf::from(self.mlv_app_path_buf.trim());
                                        config.mlv_app_path = if self.mlv_app_path_buf.trim().is_empty() { None } else { Some(p) };
                                        config.save();
                                        if let Some(ref p) = config.mlv_app_path {
                                            let _ = db.set_setting("mlv_app_path", &p.to_string_lossy());
                                        }
                                    }

                                    if ui.add(
                                        egui::Button::new(RichText::new(widgets::icon_label(icon::FOLDER_OPEN, "Browse…")).size(FONT_SMALL))
                                            .min_size(Vec2::new(btn_browse_w, CONTROL_HEIGHT + 2.0)),
                                    ).clicked() {
                                        let filter_ext = if cfg!(target_os = "windows") {
                                            &["exe"][..]
                                        } else if cfg!(target_os = "macos") {
                                            &["app"][..]
                                        } else {
                                            &["AppImage", "bin", "*"][..]
                                        };

                                        if let Some(picked) = rfd::FileDialog::new()
                                            .set_title("Locate MLV App Executable")
                                            .add_filter("MLV App Executable", filter_ext)
                                            .pick_file()
                                        {
                                            self.mlv_app_path_buf = picked.to_string_lossy().to_string();
                                            config.mlv_app_path = Some(picked.clone());
                                            config.save();
                                            let _ = db.set_setting("mlv_app_path", &picked.to_string_lossy());
                                            self.status_message = Some(("MLV App path saved.".to_string(), false));
                                        }
                                    }

                                    if ui.add(
                                        egui::Button::new(RichText::new(widgets::icon_label(icon::MAGNIFYING_GLASS, "Auto-Detect")).size(FONT_SMALL))
                                            .min_size(Vec2::new(btn_auto_w, CONTROL_HEIGHT + 2.0)),
                                    ).on_hover_text("Search common installation directories for MLV App").clicked() {
                                        if let Some(detected) = ExternalLauncher::detect_mlv_app() {
                                            self.mlv_app_path_buf = detected.to_string_lossy().to_string();
                                            config.mlv_app_path = Some(detected.clone());
                                            config.save();
                                            let _ = db.set_setting("mlv_app_path", &detected.to_string_lossy());
                                            self.status_message = Some((
                                                format!("Found MLV App: {}", detected.file_name().unwrap_or_default().to_string_lossy()),
                                                false,
                                            ));
                                        } else {
                                            self.status_message = Some((
                                                "Could not automatically detect MLV App. Please browse manually.".to_string(),
                                                true,
                                            ));
                                        }
                                    }
                                });
                            },
                        );

                        ui.add_space(6.0);
                        ui.separator();
                        ui.add_space(2.0);

                        // ========================================================
                        // Bottom Modal Footer: Status Banner & Done Button
                        // ========================================================
                        ui.horizontal(|ui| {
                            let done_btn_w = 72.0;
                            let spacing = ui.spacing().item_spacing.x;
                            let msg_w = (ui.available_width() - done_btn_w - spacing).max(50.0);

                            if let Some((ref msg, is_err)) = self.status_message {
                                let col = if is_err { FLAG_REJECTED } else { FLAG_PICKED };
                                let glyph = if is_err { icon::WARNING_CIRCLE } else { icon::CHECK_CIRCLE };
                                ui.allocate_ui_with_layout(
                                    egui::vec2(msg_w, CONTROL_HEIGHT),
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.add(
                                            egui::Label::new(RichText::new(format!("{} {}", glyph, msg)).size(FONT_SMALL).color(col))
                                                .truncate(),
                                        )
                                        .on_hover_text(msg);
                                    },
                                );
                            } else {
                                ui.add_space(msg_w);
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                if widgets::primary_button(ui, "Done").clicked() {
                                    close_requested = true;
                                }
                            });
                        });
                    });
            });

        self.is_open = is_open && !close_requested;

        action
    }

    /// Renders an elegant, rounded container card for settings groups
    fn settings_card<R>(
        ui: &mut egui::Ui,
        icon_glyph: &str,
        title: &str,
        subtitle: &str,
        content: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        egui::Frame::none()
            .fill(bg_card(ui))
            .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
            .rounding(Rounding::same(ROUNDING + 3.0))
            .inner_margin(egui::Margin::symmetric(12.0, 10.0))
            .show(ui, |ui| {
                ui.set_max_width(ui.available_width());

                // Header
                ui.horizontal(|ui| {
                    ui.label(RichText::new(icon_glyph).size(15.0).color(ACCENT));
                    ui.label(RichText::new(title).strong().size(FONT_BODY).color(text_primary(ui)));
                });

                if !subtitle.is_empty() {
                    ui.add_space(2.0);
                    ui.add(
                        egui::Label::new(RichText::new(subtitle).size(FONT_CAPTION).color(text_muted(ui)))
                            .wrap(),
                    );
                }

                ui.add_space(6.0);
                content(ui)
            })
            .inner
    }

    /// Renders a segmented theme toggle button
    fn theme_toggle_button(ui: &mut egui::Ui, label: &str, icon_glyph: &str, is_selected: bool, width: f32) -> bool {
        let is_dark = ui.visuals().dark_mode;
        let text_col = if is_selected {
            Color32::WHITE
        } else {
            text_secondary(ui)
        };
        let fill = if is_selected {
            if is_dark { ACCENT_DIM } else { ACCENT }
        } else {
            bg_field(ui)
        };
        let stroke = if is_selected {
            Stroke::new(1.0_f32, ACCENT)
        } else {
            Stroke::new(1.0_f32, border_subtle(ui))
        };

        let content = format!("{} {}", icon_glyph, label);
        let btn = egui::Button::new(RichText::new(content).size(FONT_SMALL).color(text_col))
            .fill(fill)
            .stroke(stroke)
            .rounding(Rounding::same(ROUNDING + 1.0))
            .min_size(Vec2::new(width, CONTROL_HEIGHT + 2.0));

        ui.add(btn).clicked()
    }
}

