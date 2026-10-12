use crate::config::AppConfig;
use crate::db::Database;
use crate::launcher::ExternalLauncher;
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use egui::{Context, RichText, Stroke, Window};
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
        let title = widgets::icon_label(icon::GEAR, "Settings & Catalog Manager");

        Window::new(RichText::new(title).strong().size(FONT_BODY))
            .open(&mut self.is_open)
            .resizable(false)
            .collapsible(false)
            .pivot(egui::Align2::CENTER_CENTER)
            .default_pos(ctx.screen_rect().center())
            .fixed_size([540.0, 480.0])
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                    // ==========================================
                    // 1. CATALOG MANAGEMENT (Lightroom Style)
                    // ==========================================
                    widgets::section(ui, "Catalog Management", true, None, |ui| {
                        // Current Catalog Path
                        ui.label(RichText::new("Active Catalog:").size(FONT_CAPTION).color(text_muted(ui)));
                        ui.horizontal(|ui| {
                            let path_str = db.path.to_string_lossy();
                            ui.add(
                                egui::Label::new(RichText::new(path_str.as_ref()).size(FONT_SMALL).monospace().color(text_primary(ui)))
                                    .truncate(),
                            );
                        });

                        ui.add_space(4.0);

                        // Actions: Create New, Open, Import Into Current
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = egui::vec2(6.0, 6.0);

                            // 1. Create New Catalog
                            if widgets::button(ui, widgets::icon_label(icon::FILE_PLUS, "Create New Catalog…")).clicked() {
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
                                                format!("Created and switched to new catalog: {}", new_path.display()),
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

                            // 2. Open Catalog
                            if widgets::button(ui, widgets::icon_label(icon::FOLDER_OPEN, "Open Catalog…")).clicked() {
                                if let Some(open_path) = rfd::FileDialog::new()
                                    .set_title("Open Existing MLV Explorer Catalog")
                                    .add_filter("SQLite Catalog (.db)", &["db", "sqlite"])
                                    .pick_file()
                                {
                                    if open_path.is_file() {
                                        config.last_catalog_path = Some(open_path.clone());
                                        config.save();
                                        self.status_message = Some((
                                            format!("Switched to catalog: {}", open_path.display()),
                                            false,
                                        ));
                                        action = Some(SettingsAction::SwitchCatalog(open_path));
                                    }
                                }
                            }

                            // 3. Import Catalog Into This Catalog
                            if widgets::button(ui, widgets::icon_label(icon::STACK, "Import Catalog to Current…")).clicked() {
                                if let Some(import_path) = rfd::FileDialog::new()
                                    .set_title("Import Another Catalog into Current Library")
                                    .add_filter("Catalog (.db, .json)", &["db", "sqlite", "json"])
                                    .pick_file()
                                {
                                    let ext = import_path.extension().and_then(|s| s.to_str()).unwrap_or("").to_lowercase();
                                    if ext == "json" {
                                        match db.import_catalog_json(&import_path) {
                                            Ok(count) => {
                                                self.status_message = Some((
                                                    format!("Successfully merged {count} clips from JSON catalog"),
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
                                                    format!("Successfully merged {count} clips from external database catalog"),
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
                    });

                    ui.add_space(4.0);

                    // ==========================================
                    // 2. BACKUP & ARCHIVAL
                    // ==========================================
                    widgets::section(ui, "Backups (Catalog Only)", true, None, |ui| {
                        // Prominent note explaining backups are catalog-only
                        egui::Frame::none()
                            .fill(bg_field(ui))
                            .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
                            .rounding(egui::Rounding::same(ROUNDING))
                            .inner_margin(egui::Margin::symmetric(8.0, 6.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(icon::INFO).color(ACCENT_HOVER));
                                    ui.label(
                                        RichText::new(
                                            "Catalog backups save your metadata, ratings, keywords, slate, and notes.\nOriginal RAW video files on disk are NOT duplicated or moved."
                                        )
                                        .size(FONT_SMALL)
                                        .color(text_secondary(ui)),
                                    );
                                });
                            });

                        ui.add_space(4.0);

                        // Backup folder configuration
                        ui.label(RichText::new("Backup Folder:").size(FONT_CAPTION).color(text_muted(ui)));
                        ui.horizontal(|ui| {
                            let folder_display = config
                                .backup_folder
                                .as_ref()
                                .map(|p| p.to_string_lossy().to_string())
                                .unwrap_or_else(|| "Not set (will prompt)".to_string());

                            ui.add_sized(
                                [ui.available_width() - 90.0, CONTROL_HEIGHT],
                                egui::Label::new(RichText::new(folder_display).size(FONT_SMALL).monospace().color(text_primary(ui)))
                                    .truncate(),
                            );

                            if widgets::button(ui, "Browse…").clicked() {
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

                        ui.horizontal(|ui| {
                            // Instant Backup Button
                            if widgets::primary_button(ui, widgets::icon_label(icon::FLOPPY_DISK, "Create Backup Now")).clicked() {
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

                            // Export JSON
                            if widgets::button(ui, widgets::icon_label(icon::UPLOAD_SIMPLE, "Export JSON…")).clicked() {
                                if let Some(dest) = rfd::FileDialog::new()
                                    .set_file_name("mlv_catalog_export.json")
                                    .add_filter("JSON Document", &["json"])
                                    .save_file()
                                {
                                    match db.export_catalog_json(&dest) {
                                        Ok(_) => {
                                            self.status_message = Some((
                                                format!("Exported portable JSON to {}", dest.display()),
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
                    });

                    ui.add_space(4.0);

                    // ==========================================
                    // 3. EXTERNAL APPLICATIONS (MLV App)
                    // ==========================================
                    widgets::section(ui, "MLV App Integration", true, None, |ui| {
                        ui.label(RichText::new("Executable Path:").size(FONT_CAPTION).color(text_muted(ui)));
                        ui.horizontal(|ui| {
                            let text_w = ui.available_width() - 170.0;
                            let resp = ui.add_sized(
                                [text_w, CONTROL_HEIGHT],
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

                            if widgets::button(ui, "Browse…").clicked() {
                                let filter_ext = if cfg!(target_os = "windows") {
                                    &["exe"][..]
                                } else if cfg!(target_os = "macos") {
                                    &["app"][..]
                                } else {
                                    &["AppImage", "bin", "*"][..]
                                };

                                if let Some(picked) = rfd::FileDialog::new()
                                    .set_title("Locate MLV App executable")
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

                            if widgets::button(ui, "Auto-Detect").clicked() {
                                if let Some(detected) = ExternalLauncher::detect_mlv_app() {
                                    self.mlv_app_path_buf = detected.to_string_lossy().to_string();
                                    config.mlv_app_path = Some(detected.clone());
                                    config.save();
                                    let _ = db.set_setting("mlv_app_path", &detected.to_string_lossy());
                                    self.status_message = Some((
                                        format!("Found MLV App: {}", detected.display()),
                                        false,
                                    ));
                                } else {
                                    self.status_message = Some((
                                        "Could not automatically detect MLV App in standard directories. Please browse manually.".to_string(),
                                        true,
                                    ));
                                }
                            }
                        });
                    });

                    ui.add_space(6.0);

                    // ==========================================
                    // Status Notification Banner
                    // ==========================================
                    if let Some((ref msg, is_err)) = self.status_message {
                        let col = if is_err { FLAG_REJECTED } else { FLAG_PICKED };
                        ui.label(RichText::new(msg).size(FONT_SMALL).color(col));
                    }
                });
            });

        action
    }
}
