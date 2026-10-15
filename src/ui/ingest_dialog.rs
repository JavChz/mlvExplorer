use crate::config::AppConfig;
use crate::db::Database;
use crate::ingester::{
    detect_available_drives, format_bytes, scan_source, spawn_thumb_generator, ClipThumb,
    DetectedClip, DetectedDrive, IngestMessage, IngestOptions, Ingester, OrganizeOption,
};
use crate::ui::theme::*;
use crate::ui::widgets::{self, icon};
use crossbeam_channel::{unbounded, Receiver};
use egui::{
    Color32, ColorImage, Context, FontId, Rect, RichText, Rounding, Sense, Stroke, TextureHandle,
    TextureOptions, Ui, Vec2, Window,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

pub enum IngestDialogAction {
    Finished(usize), // number of clips ingested
}

#[derive(Debug, Clone)]
struct ProgressState {
    current_clip: usize,
    total_clips: usize,
    current_file: String,
    stage: crate::ingester::IngestStage,
    file_bytes_copied: u64,
    file_total_bytes: u64,
    total_bytes_copied: u64,
    total_bytes: u64,
    speed_mbps: f32,
}

#[derive(Debug, Clone)]
struct ResultState {
    clips_copied: usize,
    files_copied: usize,
    total_bytes: u64,
    failed: usize,
    checksums_verified: bool,
    is_cancelled: bool,
    error: Option<String>,
}

pub struct IngestDialog {
    pub is_open: bool,
    pub source_path: Option<PathBuf>,
    pub detected_drives: Vec<DetectedDrive>,
    pub clips: Vec<DetectedClip>,

    // Background Scanning
    pub is_scanning: bool,
    scan_rx: Option<Receiver<Vec<DetectedClip>>>,
    scan_cancel: Option<Arc<AtomicBool>>,

    // Ingest Options
    pub destination_path_buf: String,
    pub organize: OrganizeOption,
    pub custom_subfolder: String,
    pub add_to_catalog: bool,
    pub skip_existing: bool,
    pub verify_checksum: bool,

    // Thumbnail Preview Cache
    textures: HashMap<usize, TextureHandle>,
    thumb_rx: Option<Receiver<ClipThumb>>,
    thumb_cancel: Option<Arc<AtomicBool>>,

    // Background Ingest Execution
    pub is_ingesting: bool,
    ingest_rx: Option<Receiver<IngestMessage>>,
    ingest_cancel: Option<Arc<AtomicBool>>,
    progress: Option<ProgressState>,
    result: Option<ResultState>,
}

impl Default for IngestDialog {
    fn default() -> Self {
        Self {
            is_open: false,
            source_path: None,
            detected_drives: Vec::new(),
            clips: Vec::new(),
            is_scanning: false,
            scan_rx: None,
            scan_cancel: None,
            destination_path_buf: String::new(),
            organize: OrganizeOption::Direct,
            custom_subfolder: String::new(),
            add_to_catalog: true,
            skip_existing: true,
            verify_checksum: true,
            textures: HashMap::new(),
            thumb_rx: None,
            thumb_cancel: None,
            is_ingesting: false,
            ingest_rx: None,
            ingest_cancel: None,
            progress: None,
            result: None,
        }
    }
}

impl IngestDialog {
    pub fn open(&mut self, config: &AppConfig, db: &Database, ctx: &Context) {
        self.is_open = true;
        self.result = None;
        self.progress = None;
        self.is_ingesting = false;
        self.textures.clear();

        // Restore destination from config if previously saved
        if let Some(ref dest) = config.last_ingest_destination {
            self.destination_path_buf = dest.to_string_lossy().to_string();
        }

        // Detect available drives / card readers
        self.detected_drives = detect_available_drives();

        // Select the first camera card drive if detected, or first drive
        let initial_source = self
            .detected_drives
            .iter()
            .find(|d| d.is_removable_or_card)
            .or_else(|| self.detected_drives.first())
            .map(|d| d.path.clone());

        if let Some(source) = initial_source {
            self.set_source_and_scan(source, db, ctx);
        }
    }

    pub fn set_source_and_scan(&mut self, source: PathBuf, db: &Database, ctx: &Context) {
        self.cancel_scanning();
        self.cancel_thumbs();

        self.source_path = Some(source.clone());
        self.clips.clear();
        self.textures.clear();
        self.is_scanning = true;

        let cancel = Arc::new(AtomicBool::new(false));
        self.scan_cancel = Some(cancel.clone());

        let (tx, rx) = unbounded();
        self.scan_rx = Some(rx);

        let db_path = db.path.clone();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let scan_db = Database::open(&db_path).unwrap_or_else(|_| Database::open(crate::db::default_catalog_path()).unwrap());
            let found = scan_source(&source, &scan_db, &cancel);
            let _ = tx.send(found);
            ctx_clone.request_repaint();
        });
    }

    fn cancel_scanning(&mut self) {
        if let Some(cancel) = self.scan_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.scan_rx = None;
        self.is_scanning = false;
    }

    fn cancel_thumbs(&mut self) {
        if let Some(cancel) = self.thumb_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
        self.thumb_rx = None;
    }

    fn cancel_ingest(&mut self) {
        if let Some(cancel) = self.ingest_cancel.take() {
            cancel.store(true, Ordering::Relaxed);
        }
    }

    /// Poll incoming scan, thumbnail, and ingestion channels
    fn poll_channels(&mut self, ctx: &Context) {
        // 1. Scan completion
        if let Some(ref rx) = self.scan_rx {
            if let Ok(clips) = rx.try_recv() {
                self.clips = clips;
                self.is_scanning = false;
                self.scan_rx = None;
                self.scan_cancel = None;

                // Launch preview thumbnail generation for the "small view"
                self.cancel_thumbs();
                let cancel = Arc::new(AtomicBool::new(false));
                self.thumb_cancel = Some(cancel.clone());
                let (tx, rx) = unbounded();
                self.thumb_rx = Some(rx);

                let clip_list: Vec<(usize, PathBuf)> = self
                    .clips
                    .iter()
                    .map(|c| (c.id, c.main_path.clone()))
                    .collect();

                spawn_thumb_generator(clip_list, cancel, tx, ctx.clone());
            }
        }

        // 2. Incoming thumbnails
        if let Some(ref rx) = self.thumb_rx {
            while let Ok(thumb) = rx.try_recv() {
                if thumb.width > 0 && thumb.height > 0 && !thumb.rgba.is_empty() {
                    let image = ColorImage::from_rgba_unmultiplied(
                        [thumb.width as usize, thumb.height as usize],
                        &thumb.rgba,
                    );
                    let handle = ctx.load_texture(
                        format!("ingest_thumb_{}", thumb.clip_id),
                        image,
                        TextureOptions::LINEAR,
                    );
                    self.textures.insert(thumb.clip_id, handle);
                }
            }
        }

        // 3. Ingestion progress
        if let Some(ref rx) = self.ingest_rx {
            let mut should_stop = false;
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    IngestMessage::Progress {
                        current_clip,
                        total_clips,
                        current_file,
                        stage,
                        file_bytes_copied,
                        file_total_bytes,
                        total_bytes_copied,
                        total_bytes,
                        speed_mbps,
                    } => {
                        self.progress = Some(ProgressState {
                            current_clip,
                            total_clips,
                            current_file,
                            stage,
                            file_bytes_copied,
                            file_total_bytes,
                            total_bytes_copied,
                            total_bytes,
                            speed_mbps,
                        });
                    }
                    IngestMessage::ClipComplete { clip_id, .. } => {
                        if let Some(clip) = self.clips.iter_mut().find(|c| c.id == clip_id) {
                            clip.already_in_catalog = true;
                            clip.selected = false;
                        }
                    }
                    IngestMessage::Complete {
                        clips_copied,
                        files_copied,
                        total_bytes,
                        failed,
                        checksums_verified,
                    } => {
                        self.is_ingesting = false;
                        self.progress = None;
                        self.result = Some(ResultState {
                            clips_copied,
                            files_copied,
                            total_bytes,
                            failed,
                            checksums_verified,
                            is_cancelled: false,
                            error: None,
                        });
                        should_stop = true;
                    }
                    IngestMessage::Cancelled => {
                        self.is_ingesting = false;
                        self.progress = None;
                        self.result = Some(ResultState {
                            clips_copied: 0,
                            files_copied: 0,
                            total_bytes: 0,
                            failed: 0,
                            checksums_verified: false,
                            is_cancelled: true,
                            error: None,
                        });
                        should_stop = true;
                    }
                    IngestMessage::Error(err) => {
                        self.is_ingesting = false;
                        self.progress = None;
                        self.result = Some(ResultState {
                            clips_copied: 0,
                            files_copied: 0,
                            total_bytes: 0,
                            failed: 0,
                            checksums_verified: false,
                            is_cancelled: false,
                            error: Some(err),
                        });
                        should_stop = true;
                    }
                }
            }
            if should_stop {
                self.ingest_rx = None;
                self.ingest_cancel = None;
            }
        }
    }

    pub fn show(
        &mut self,
        ctx: &Context,
        db: &Database,
        config: &mut AppConfig,
    ) -> Option<IngestDialogAction> {
        if !self.is_open {
            return None;
        }

        self.poll_channels(ctx);

        let mut action = None;
        let mut is_open = self.is_open;

        Window::new(
            RichText::new(widgets::icon_label(
                icon::DOWNLOAD_SIMPLE,
                "Ingest Videos from SD / Card",
            ))
            .strong()
            .size(FONT_BODY + 1.0),
        )
        .open(&mut is_open)
        .resizable(true)
        .collapsible(false)
        .constrain(true)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.screen_rect().center())
        .default_size([940.0, 680.0])
        .min_size([720.0, 520.0])
        .show(ctx, |ui| {
            if self.is_ingesting {
                self.show_ingesting_view(ui);
            } else if let Some(ref res) = self.result.clone() {
                if let Some(act) = self.show_result_view(ui, res) {
                    action = Some(act);
                }
            } else {
                if let Some(act) = self.show_wizard_view(ui, db, config, ctx) {
                    action = Some(act);
                }
            }
        });

        if !is_open {
            self.cancel_scanning();
            self.cancel_thumbs();
            if self.is_ingesting {
                self.cancel_ingest();
            }
            self.is_open = false;
        }

        action
    }

    /// Renders the main Lightroom-style Ingestion configuration view
    fn show_wizard_view(
        &mut self,
        ui: &mut Ui,
        db: &Database,
        config: &mut AppConfig,
        ctx: &Context,
    ) -> Option<IngestDialogAction> {
        ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);

        // ====================================================================
        // 1. SOURCE SELECTOR (Top Bar)
        // ====================================================================
        egui::Frame::none()
            .fill(bg_section(ui))
            .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
            .rounding(Rounding::same(ROUNDING))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(widgets::icon_label(icon::HARD_DRIVE, "Source Card / Drive:")).strong().size(FONT_SMALL));

                    // Quick drive pills
                    let mut drive_to_scan = None;
                    for drive in &self.detected_drives {
                        let is_current = self.source_path.as_ref() == Some(&drive.path);
                        let icon_glyph = if drive.is_removable_or_card { icon::SIM_CARD } else { icon::HARD_DRIVE };
                        let pill_text = format!("{} {}", icon_glyph, drive.label);
                        if widgets::toggle_button(ui, is_current, pill_text).clicked() && !is_current {
                            drive_to_scan = Some(drive.path.clone());
                        }
                    }

                    if let Some(target) = drive_to_scan {
                        self.set_source_and_scan(target, db, ctx);
                    }

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(widgets::icon_label(icon::ARROWS_CLOCKWISE, "Rescan")).on_hover_text("Rescan current source for MLV files").clicked() {
                            if let Some(ref src) = self.source_path.clone() {
                                self.set_source_and_scan(src.clone(), db, ctx);
                            }
                        }

                        if ui.button(widgets::icon_label(icon::FOLDER_OPEN, "Browse Card / Folder...")).clicked() {
                            if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                self.set_source_and_scan(folder, db, ctx);
                            }
                        }
                    });
                });

                if let Some(ref src) = self.source_path {
                    ui.add_space(2.0);
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Path:").size(FONT_CAPTION).color(text_muted(ui)));
                        ui.label(RichText::new(src.to_string_lossy()).size(FONT_CAPTION).color(text_secondary(ui)));
                    });
                }
            });

        // ====================================================================
        // 2. SELECTION CONTROLS BAR (Above the Small View Grid)
        // ====================================================================
        let selected_count = self.clips.iter().filter(|c| c.selected).count();
        let selected_bytes: u64 = self.clips.iter().filter(|c| c.selected).map(|c| c.total_size_bytes).sum();
        let total_clips = self.clips.len();

        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 6.0;

            if ui.button("Select All").clicked() {
                for c in &mut self.clips {
                    c.selected = true;
                }
            }
            if ui.button("Deselect All").clicked() {
                for c in &mut self.clips {
                    c.selected = false;
                }
            }
            if ui.button("Select New Only").on_hover_text("Select only clips not yet in catalog").clicked() {
                for c in &mut self.clips {
                    c.selected = !c.already_in_catalog;
                }
            }

            widgets::toolbar_divider(ui);

            if self.is_scanning {
                ui.spinner();
                ui.label(RichText::new("Scanning source for MLV files...").size(FONT_SMALL).color(ACCENT_HOVER));
            } else {
                ui.label(
                    RichText::new(format!(
                        "{} of {} clips selected  ·  {}",
                        selected_count,
                        total_clips,
                        format_bytes(selected_bytes)
                    ))
                    .size(FONT_SMALL)
                    .color(text_primary(ui))
                    .strong(),
                );
            }
        });

        // ====================================================================
        // 3. "SMALL VIEW" PREVIEW GRID (Lightroom Style)
        // ====================================================================
        let available_height = ui.available_height() - 170.0; // reserve space for destination & action footer
        egui::Frame::none()
            .fill(BG_VIEWER)
            .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
            .rounding(Rounding::same(ROUNDING))
            .inner_margin(egui::Margin::same(8.0))
            .show(ui, |ui| {
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .max_height(available_height.max(160.0))
                    .show(ui, |ui| {
                        if self.clips.is_empty() && !self.is_scanning {
                            ui.vertical_centered(|ui| {
                                ui.add_space(60.0);
                                ui.label(RichText::new(icon::FILM_STRIP).size(48.0).color(text_disabled(ui)));
                                ui.add_space(8.0);
                                ui.label(
                                    RichText::new("No MLV files found in this source folder.")
                                        .size(FONT_BODY)
                                        .color(text_muted(ui)),
                                );
                                ui.label(
                                    RichText::new("Insert an SD or CF card, or click 'Browse Card / Folder...' to locate your clips.")
                                        .size(FONT_SMALL)
                                        .color(text_hint(ui)),
                                );
                            });
                        } else {
                            self.render_small_view_grid(ui);
                        }
                    });
            });

        // ====================================================================
        // 4. DESTINATION & SETTINGS SECTION
        // ====================================================================
        egui::Frame::none()
            .fill(bg_card(ui))
            .stroke(Stroke::new(1.0_f32, border_subtle(ui)))
            .rounding(Rounding::same(ROUNDING))
            .inner_margin(egui::Margin::symmetric(12.0, 8.0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = Vec2::new(8.0, 6.0);

                // Row 1: Destination folder
                ui.horizontal(|ui| {
                    ui.label(RichText::new(widgets::icon_label(icon::FOLDER, "Destination Folder:")).strong().size(FONT_SMALL));

                    let response = ui.add_sized(
                        [ui.available_width() - 110.0, CONTROL_HEIGHT],
                        egui::TextEdit::singleline(&mut self.destination_path_buf)
                            .hint_text("Specify folder on your computer (e.g. D:\\Footage\\2026-10-14)..."),
                    );

                    if response.changed() {
                        let path = PathBuf::from(self.destination_path_buf.trim());
                        if path.exists() {
                            config.last_ingest_destination = Some(path);
                            config.save();
                        }
                    }

                    if ui.button("Browse...").clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.destination_path_buf = folder.to_string_lossy().to_string();
                            config.last_ingest_destination = Some(folder);
                            config.save();
                        }
                    }
                });

                // Row 2: Organization options & Auto-add
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Organize:").size(FONT_SMALL).color(text_secondary(ui)));

                    ui.radio_value(&mut self.organize, OrganizeOption::Direct, "Directly in folder");
                    ui.radio_value(&mut self.organize, OrganizeOption::DateFolder, "Date subfolder (YYYY-MM-DD)");
                    ui.radio_value(&mut self.organize, OrganizeOption::CustomFolder, "Custom subfolder:");

                    if self.organize == OrganizeOption::CustomFolder {
                        ui.add_sized(
                            [140.0, CONTROL_HEIGHT],
                            egui::TextEdit::singleline(&mut self.custom_subfolder).hint_text("e.g. Scene_1"),
                        );
                    }

                    widgets::toolbar_divider(ui);

                    ui.checkbox(&mut self.add_to_catalog, "Add to Catalog after copy");
                    ui.checkbox(&mut self.skip_existing, "Skip already copied files");
                    ui.checkbox(&mut self.verify_checksum, "Bit-for-bit checksums (CRC-32)")
                        .on_hover_text("Calculates CRC-32 checksum during copy and reads back destination files to verify 100% bit-for-bit integrity. Exact byte size and MLVI container headers are always verified.");
                });
            });

        // ====================================================================
        // 5. ACTION FOOTER
        // ====================================================================
        let dest_valid = !self.destination_path_buf.trim().is_empty();
        let can_start = dest_valid && selected_count > 0 && !self.is_scanning;

        ui.horizontal(|ui| {
            if !dest_valid {
                ui.label(
                    RichText::new("⚠️ Please specify a destination folder on your computer to copy files into.")
                        .size(FONT_SMALL)
                        .color(WARNING),
                );
            } else if selected_count == 0 {
                ui.label(
                    RichText::new("Select at least one clip from the card above to begin ingest.")
                        .size(FONT_SMALL)
                        .color(text_muted(ui)),
                );
            } else {
                ui.label(
                    RichText::new(format!(
                        "Ready to ingest {} clips ({}) to destination",
                        selected_count,
                        format_bytes(selected_bytes)
                    ))
                    .size(FONT_SMALL)
                    .color(FLAG_PICKED),
                );
            }

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let start_btn = egui::Button::new(
                    RichText::new(widgets::icon_label(icon::DOWNLOAD_SIMPLE, "Start Ingest"))
                        .color(Color32::WHITE)
                        .strong(),
                )
                .fill(if can_start { ACCENT } else { bg_card(ui) })
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(120.0, 28.0));

                if ui.add_enabled(can_start, start_btn).clicked() {
                    self.start_ingestion(db, ctx);
                }

                let cancel_btn = egui::Button::new("Cancel").min_size(Vec2::new(80.0, 28.0));
                if ui.add(cancel_btn).clicked() {
                    self.is_open = false;
                }
            });
        });

        None
    }

    /// Renders the Lightroom-style compact grid of cards ("small view")
    fn render_small_view_grid(&mut self, ui: &mut Ui) {
        let card_width = 175.0;
        let card_height = 185.0;
        let spacing = 8.0;

        let available_width = ui.available_width();
        let cols = ((available_width + spacing) / (card_width + spacing)).floor().max(1.0) as usize;

        egui::Grid::new("ingest_small_view_grid")
            .spacing([spacing, spacing])
            .show(ui, |ui| {
                for (idx, clip) in self.clips.iter_mut().enumerate() {
                    Self::render_clip_card(ui, clip, &self.textures, card_width, card_height);
                    if (idx + 1) % cols == 0 {
                        ui.end_row();
                    }
                }
            });
    }

    /// Renders an individual compact clip card with checkbox, thumbnail, and badges
    fn render_clip_card(
        ui: &mut Ui,
        clip: &mut DetectedClip,
        textures: &HashMap<usize, TextureHandle>,
        width: f32,
        height: f32,
    ) {
        let (rect, resp) = ui.allocate_exact_size(Vec2::new(width, height), Sense::click());
        let is_hovered = resp.hovered();
        let painter = ui.painter();

        // Card background & border styling
        let bg_color = if clip.selected {
            BG_CARD_SELECTED
        } else if is_hovered {
            BG_CARD_HOVER
        } else {
            bg_card(ui)
        };

        let stroke_color = if clip.selected {
            ACCENT
        } else if is_hovered {
            border_strong(ui)
        } else {
            border_subtle(ui)
        };

        painter.rect_filled(rect, Rounding::same(ROUNDING), bg_color);
        painter.rect_stroke(rect, Rounding::same(ROUNDING), Stroke::new(1.0_f32, stroke_color));

        // Clicking anywhere on the card toggles its selection
        if resp.clicked() {
            clip.selected = !clip.selected;
        }

        // Inside card layout
        let mut card_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink(6.0))
                .layout(egui::Layout::top_down(egui::Align::Min)),
        );

        // Header: Checkbox + Filename
        card_ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 4.0;
            ui.checkbox(&mut clip.selected, "");
            ui.label(
                RichText::new(&clip.file_name)
                    .size(FONT_SMALL)
                    .strong()
                    .color(if clip.selected { Color32::WHITE } else { text_primary(ui) }),
            )
            .on_hover_text(&clip.file_name);
        });

        card_ui.add_space(4.0);

        // Thumbnail area (16:9 ratio, ~163 x 92)
        let thumb_rect = Rect::from_min_size(
            card_ui.cursor().min,
            Vec2::new(width - 12.0, 92.0),
        );
        card_ui.allocate_rect(thumb_rect, Sense::hover());

        let thumb_painter = card_ui.painter();
        thumb_painter.rect_filled(thumb_rect, Rounding::same(2.0), BG_THUMB);

        if let Some(texture) = textures.get(&clip.id) {
            thumb_painter.image(
                texture.id(),
                thumb_rect,
                Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                Color32::WHITE,
            );
        } else {
            // Placeholder while loading
            let disabled_col = text_disabled(&card_ui);
            thumb_painter.text(
                thumb_rect.center(),
                egui::Align2::CENTER_CENTER,
                icon::FILM_STRIP,
                FontId::proportional(24.0),
                disabled_col,
            );
        }

        card_ui.add_space(4.0);

        // Metadata badges & info
        card_ui.horizontal(|ui| {
            ui.label(
                RichText::new(format_bytes(clip.total_size_bytes))
                    .size(FONT_CAPTION)
                    .color(text_muted(ui)),
            );

            if let Some((w, h)) = clip.resolution {
                ui.label(
                    RichText::new(format!("{w}x{h}"))
                        .size(FONT_CAPTION)
                        .color(text_hint(ui)),
                );
            }
        });

        card_ui.horizontal(|ui| {
            // Spanned chunks badge (important for 5D3 card spanning!)
            if clip.spanned_chunks.len() > 1 {
                egui::Frame::none()
                    .fill(ACCENT_DIM)
                    .rounding(Rounding::same(2.0))
                    .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new(format!("{} Spanned ({})", icon::STACK, clip.spanned_chunks.len()))
                                .size(9.5)
                                .color(Color32::WHITE),
                        );
                    })
                    .response
                    .on_hover_text(format!(
                        "This clip spans {} companion files (.M00, .M01, etc.). All will be copied together.",
                        clip.spanned_chunks.len()
                    ));
            }

            // Already in Catalog indicator
            if clip.already_in_catalog {
                egui::Frame::none()
                    .fill(Color32::from_rgb(45, 45, 50))
                    .rounding(Rounding::same(2.0))
                    .inner_margin(egui::Margin::symmetric(4.0, 1.0))
                    .show(ui, |ui| {
                        ui.label(
                            RichText::new("In Catalog")
                                .size(9.5)
                                .color(Color32::from_rgb(180, 180, 190)),
                        );
                    })
                    .response
                    .on_hover_text("This clip is already present in your MLV Explorer catalog.");
            }
        });
    }

    /// Starts the background ingestion process
    fn start_ingestion(&mut self, db: &Database, ctx: &Context) {
        let dest = PathBuf::from(self.destination_path_buf.trim());
        let selected_clips: Vec<DetectedClip> = self
            .clips
            .iter()
            .filter(|c| c.selected)
            .cloned()
            .collect();

        if selected_clips.is_empty() {
            return;
        }

        self.is_ingesting = true;
        self.progress = Some(ProgressState {
            current_clip: 0,
            total_clips: selected_clips.len(),
            current_file: String::new(),
            stage: crate::ingester::IngestStage::Copying,
            file_bytes_copied: 0,
            file_total_bytes: 0,
            total_bytes_copied: 0,
            total_bytes: selected_clips.iter().map(|c| c.total_size_bytes).sum(),
            speed_mbps: 0.0,
        });

        let cancel = Arc::new(AtomicBool::new(false));
        self.ingest_cancel = Some(cancel.clone());

        let (tx, rx) = unbounded();
        self.ingest_rx = Some(rx);

        let options = IngestOptions {
            destination_dir: dest,
            organize: self.organize,
            custom_subfolder: self.custom_subfolder.clone(),
            add_to_catalog: self.add_to_catalog,
            skip_existing: self.skip_existing,
            verify_checksum: self.verify_checksum,
        };

        Ingester::start(
            selected_clips,
            options,
            db.path.clone(),
            tx,
            cancel,
            ctx.clone(),
        );
    }

    /// Renders active ingestion progress view
    fn show_ingesting_view(&mut self, ui: &mut Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(40.0);

            ui.label(RichText::new(icon::DOWNLOAD_SIMPLE).size(56.0).color(ACCENT));
            ui.add_space(8.0);

            if let Some(ref prog) = self.progress {
                let stage_prefix = match prog.stage {
                    crate::ingester::IngestStage::Copying => "Ingesting",
                    crate::ingester::IngestStage::VerifyingChecksum => "Verifying Checksum (CRC-32)",
                };
                ui.label(
                    RichText::new(format!(
                        "{} Clip {} of {}: {}",
                        stage_prefix, prog.current_clip, prog.total_clips, prog.current_file
                    ))
                    .strong()
                    .size(FONT_BODY + 2.0)
                    .color(text_primary(ui)),
                );

                ui.add_space(16.0);

                // Overall progress bar
                let fraction = if prog.total_bytes > 0 {
                    (prog.total_bytes_copied as f32 / prog.total_bytes as f32).clamp(0.0, 1.0)
                } else {
                    0.0
                };

                let bar = egui::ProgressBar::new(fraction)
                    .show_percentage()
                    .animate(true)
                    .desired_width(500.0);
                ui.add(bar);

                ui.add_space(6.0);

                let status_sub = match prog.stage {
                    crate::ingester::IngestStage::Copying => format!(
                        "{} / {} copied  ·  {:.1} MB/s",
                        format_bytes(prog.total_bytes_copied),
                        format_bytes(prog.total_bytes),
                        prog.speed_mbps
                    ),
                    crate::ingester::IngestStage::VerifyingChecksum => format!(
                        "Verifying destination integrity against card  ·  {:.1} MB/s",
                        prog.speed_mbps
                    ),
                };

                ui.label(
                    RichText::new(status_sub)
                        .size(FONT_SMALL)
                        .color(text_secondary(ui)),
                );

                ui.add_space(20.0);

                // Current file progress bar
                if prog.file_total_bytes > 0 {
                    let file_fraction = (prog.file_bytes_copied as f32 / prog.file_total_bytes as f32).clamp(0.0, 1.0);
                    let chunk_label = match prog.stage {
                        crate::ingester::IngestStage::Copying => format!("Copying: {}", prog.current_file),
                        crate::ingester::IngestStage::VerifyingChecksum => format!("Verifying hash: {}", prog.current_file),
                    };
                    ui.label(RichText::new(chunk_label).size(FONT_CAPTION).color(text_muted(ui)));
                    ui.add(
                        egui::ProgressBar::new(file_fraction)
                            .desired_width(400.0)
                            .fill(ACCENT_HOVER),
                    );
                }
            } else {
                ui.spinner();
                ui.label(RichText::new("Preparing files for ingest...").size(FONT_BODY));
            }

            ui.add_space(30.0);

            let cancel_btn = egui::Button::new(
                RichText::new(widgets::icon_label(icon::X, "Cancel Ingest"))
                    .color(Color32::WHITE),
            )
            .fill(DANGER_FILL)
            .min_size(Vec2::new(140.0, 30.0));

            if ui.add(cancel_btn).clicked() {
                self.cancel_ingest();
            }
        });
    }

    /// Renders completion or error result view
    fn show_result_view(&mut self, ui: &mut Ui, res: &ResultState) -> Option<IngestDialogAction> {
        let mut action = None;

        ui.vertical_centered(|ui| {
            ui.add_space(50.0);

            if let Some(ref err) = res.error {
                ui.label(RichText::new(icon::X_CIRCLE).size(64.0).color(FLAG_REJECTED));
                ui.add_space(12.0);
                ui.label(RichText::new("Ingest Failed").strong().size(FONT_BODY + 3.0).color(FLAG_REJECTED));
                ui.add_space(8.0);
                ui.label(RichText::new(err).size(FONT_SMALL).color(text_secondary(ui)));
            } else if res.is_cancelled {
                ui.label(RichText::new(icon::X).size(64.0).color(WARNING));
                ui.add_space(12.0);
                ui.label(RichText::new("Ingest Cancelled").strong().size(FONT_BODY + 3.0).color(WARNING));
                ui.add_space(8.0);
                ui.label(
                    RichText::new("The file copy was stopped. Partial files have been safely cleaned up.")
                        .size(FONT_SMALL)
                        .color(text_muted(ui)),
                );
            } else {
                ui.label(RichText::new(icon::CHECK).size(64.0).color(FLAG_PICKED));
                ui.add_space(12.0);
                ui.label(RichText::new("Ingest Complete!").strong().size(FONT_BODY + 4.0).color(FLAG_PICKED));
                ui.add_space(8.0);
                let mut summary = format!(
                    "Successfully ingested {} clips ({} total files including spans, {}).",
                    res.clips_copied,
                    res.files_copied,
                    format_bytes(res.total_bytes)
                );
                if res.failed > 0 {
                    summary.push_str(&format!(" {} file(s) failed.", res.failed));
                }
                ui.label(
                    RichText::new(summary)
                        .size(FONT_BODY)
                        .color(text_primary(ui)),
                );

                ui.add_space(4.0);
                if res.checksums_verified {
                    ui.label(
                        RichText::new("✓ Bit-for-bit CRC-32 checksums verified: 100% Match")
                            .size(FONT_SMALL)
                            .color(FLAG_PICKED),
                    );
                } else {
                    ui.label(
                        RichText::new("✓ Exact byte sizes and MLVI container headers verified")
                            .size(FONT_SMALL)
                            .color(FLAG_PICKED),
                    );
                }

                if self.add_to_catalog {
                    ui.add_space(2.0);
                    ui.label(
                        RichText::new("Clips have been added to your catalog and are ready to view.")
                            .size(FONT_SMALL)
                            .color(ACCENT_HOVER),
                    );
                }
            }

            ui.add_space(30.0);

            let done_btn = egui::Button::new(RichText::new("Done").color(Color32::WHITE))
                .fill(ACCENT)
                .stroke(Stroke::NONE)
                .min_size(Vec2::new(120.0, 32.0));

            if ui.add(done_btn).clicked() {
                let copied_count = res.clips_copied;
                self.result = None;
                self.is_open = false;
                action = Some(IngestDialogAction::Finished(copied_count));
            }
        });

        action
    }
}
