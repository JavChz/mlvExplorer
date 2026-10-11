use crate::db::{Database, FilterCriteria, PickFlag, VideoRecord};
use crate::mlv::MlvReader;
use crate::ui::{
    catalog_modal::CatalogModal, filter_bar::FilterBar, grid_view::{GridAction, GridView},
    metadata_panel::MetadataPanel, player_view::{PlayerAction, PlayerView},
};
use eframe::egui;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    Player,
}

pub struct MlvExplorerApp {
    db: Database,
    view_mode: ViewMode,
    videos: Vec<VideoRecord>,
    selected_video_idx: Option<usize>,
    filter: FilterCriteria,
    cameras: Vec<String>,
    lenses: Vec<String>,

    // UI components
    grid_view: GridView,
    player_view: PlayerView,
    metadata_panel: MetadataPanel,
    catalog_modal: CatalogModal,

    // Layout toggles
    show_left_sidebar: bool,
    show_right_sidebar: bool,
    show_filter_bar: bool,
    
    // Status bar notification
    status_bar_text: String,
}

impl MlvExplorerApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        // Open or create SQLite catalog database in user config / local app folder
        let db_path = "mlv_catalog.db";
        let db = Database::open(db_path).expect("Failed to open SQLite database");

        let mut app = Self {
            db,
            view_mode: ViewMode::Grid,
            videos: Vec::new(),
            selected_video_idx: None,
            filter: FilterCriteria::default(),
            cameras: Vec::new(),
            lenses: Vec::new(),
            grid_view: GridView::default(),
            player_view: PlayerView::default(),
            metadata_panel: MetadataPanel::default(),
            catalog_modal: CatalogModal::default(),
            show_left_sidebar: true,
            show_right_sidebar: true,
            show_filter_bar: true,
            status_bar_text: "Ready. Drag and drop .MLV files to import.".to_string(),
        };

        app.reload_catalog();
        app
    }

    /// Refresh catalog data from database
    pub fn reload_catalog(&mut self) {
        if let Ok(videos) = self.db.get_videos(&self.filter) {
            self.videos = videos;
            if let Some(idx) = self.selected_video_idx {
                if idx >= self.videos.len() {
                    self.selected_video_idx = if self.videos.is_empty() { None } else { Some(0) };
                }
            }
        }
        if let Ok(cams) = self.db.get_cameras() {
            self.cameras = cams;
        }
        if let Ok(lenses) = self.db.get_lenses() {
            self.lenses = lenses;
        }
    }

    /// Ingest a file or directory recursively
    pub fn ingest_path(&mut self, path: &Path) {
        let mut count = 0;
        if path.is_file() {
            if path.extension().and_then(|s| s.to_str()).map_or(false, |ext| ext.eq_ignore_ascii_case("mlv")) {
                if self.ingest_single_mlv(path) {
                    count += 1;
                }
            }
        } else if path.is_dir() {
            for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                let p = entry.path();
                if p.is_file() && p.extension().and_then(|s| s.to_str()).map_or(false, |ext| ext.eq_ignore_ascii_case("mlv")) {
                    if self.ingest_single_mlv(p) {
                        count += 1;
                    }
                }
            }
        }

        if count > 0 {
            self.status_bar_text = format!("Imported {} MLV file(s).", count);
            self.reload_catalog();
        }
    }

    fn ingest_single_mlv(&mut self, path: &Path) -> bool {
        match MlvReader::open(path) {
            Ok(reader) => {
                let _ = self.db.upsert_video(&reader.metadata);
                true
            }
            Err(e) => {
                eprintln!("Error reading {}: {}", path.display(), e);
                false
            }
        }
    }

    /// Handle drag-and-drop file imports from OS
    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(path) = file.path {
                    self.ingest_path(&path);
                }
            }
        }
    }

    /// Handle global Lightroom-like keyboard shortcuts
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|i| i.clone());

        // Switch views: G -> Grid, E -> Loupe/Player
        if input.key_pressed(egui::Key::G) {
            self.view_mode = ViewMode::Grid;
        }
        if input.key_pressed(egui::Key::E) {
            if self.selected_video_idx.is_some() {
                self.view_mode = ViewMode::Player;
            }
        }

        // Ratings 1..5, 0 to clear
        if let Some(idx) = self.selected_video_idx {
            if let Some(video) = self.videos.get_mut(idx) {
                let mut rating_to_set = None;
                if input.key_pressed(egui::Key::Num1) { rating_to_set = Some(1); }
                if input.key_pressed(egui::Key::Num2) { rating_to_set = Some(2); }
                if input.key_pressed(egui::Key::Num3) { rating_to_set = Some(3); }
                if input.key_pressed(egui::Key::Num4) { rating_to_set = Some(4); }
                if input.key_pressed(egui::Key::Num5) { rating_to_set = Some(5); }
                if input.key_pressed(egui::Key::Num0) { rating_to_set = Some(0); }

                if let Some(r) = rating_to_set {
                    video.rating = r;
                    let _ = self.db.set_rating(video.id, r);
                }

                // Flags: P -> Pick, X -> Reject, U -> Unflag
                if input.key_pressed(egui::Key::P) {
                    video.flag = PickFlag::Picked;
                    let _ = self.db.set_flag(video.id, PickFlag::Picked);
                }
                if input.key_pressed(egui::Key::X) {
                    video.flag = PickFlag::Rejected;
                    let _ = self.db.set_flag(video.id, PickFlag::Rejected);
                }
                if input.key_pressed(egui::Key::U) {
                    video.flag = PickFlag::Unflagged;
                    let _ = self.db.set_flag(video.id, PickFlag::Unflagged);
                }
            }
        }
    }
}

impl eframe::App for MlvExplorerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_drag_and_drop(ctx);
        self.handle_keyboard_shortcuts(ctx);

        // Top Navigation & Menu Bar
        egui::TopBottomPanel::top("top_menu_bar").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("📁 File", |ui| {
                    if ui.button("Import MLV Files...").clicked() {
                        ui.close_menu();
                        if let Some(files) = rfd::FileDialog::new()
                            .add_filter("Magic Lantern Video", &["mlv", "MLV"])
                            .pick_files()
                        {
                            for f in files {
                                self.ingest_path(&f);
                            }
                        }
                    }
                    if ui.button("Import Folder...").clicked() {
                        ui.close_menu();
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.ingest_path(&folder);
                        }
                    }
                    ui.separator();
                    if ui.button("Backup & Import Catalog...").clicked() {
                        ui.close_menu();
                        self.catalog_modal.is_open = true;
                    }
                });

                ui.menu_button("👁 View", |ui| {
                    if ui.selectable_label(self.view_mode == ViewMode::Grid, "Grid View [G]").clicked() {
                        self.view_mode = ViewMode::Grid;
                        ui.close_menu();
                    }
                    if ui.selectable_label(self.view_mode == ViewMode::Player, "Loupe / Player [E]").clicked() {
                        if self.selected_video_idx.is_some() {
                            self.view_mode = ViewMode::Player;
                        }
                        ui.close_menu();
                    }
                    ui.separator();
                    ui.checkbox(&mut self.show_left_sidebar, "Navigator (Left Sidebar)");
                    ui.checkbox(&mut self.show_right_sidebar, "Metadata (Right Sidebar)");
                    ui.checkbox(&mut self.show_filter_bar, "Filter Bar");
                });

                ui.separator();

                // Quick View switcher pills
                let grid_btn = ui.selectable_label(self.view_mode == ViewMode::Grid, "▦ Grid");
                if grid_btn.clicked() {
                    self.view_mode = ViewMode::Grid;
                }
                let player_btn = ui.selectable_label(self.view_mode == ViewMode::Player, "▶ Player");
                if player_btn.clicked() && self.selected_video_idx.is_some() {
                    self.view_mode = ViewMode::Player;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("🗄 Catalog Backup").clicked() {
                        self.catalog_modal.is_open = true;
                    }
                });
            });
        });

        // Filter Bar (Top Sub-panel)
        if self.show_filter_bar {
            egui::TopBottomPanel::top("filter_bar_panel").show(ctx, |ui| {
                if FilterBar::show(ui, &mut self.filter, &self.cameras, &self.lenses) {
                    self.reload_catalog();
                }
            });
        }

        // Status Bar (Bottom Panel)
        egui::TopBottomPanel::bottom("status_bar_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(&self.status_bar_text).small());
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(egui::RichText::new(format!("Total: {} videos", self.videos.len())).small());
                });
            });
        });

        // Left Navigator Sidebar (Catalog, Quick Collections, Tags)
        if self.show_left_sidebar {
            egui::SidePanel::left("left_navigator_panel")
                .default_width(200.0)
                .width_range(160.0..=300.0)
                .show(ctx, |ui| {
                    ui.heading("Catalog");
                    ui.separator();

                    if ui.selectable_label(self.filter.flag.is_none() && self.filter.selected_tag.is_none(), "📂 All Videos").clicked() {
                        self.filter.flag = None;
                        self.filter.selected_tag = None;
                        self.reload_catalog();
                    }

                    if ui.selectable_label(self.filter.flag == Some(PickFlag::Picked), "🏳 Picked / Flagged").clicked() {
                        self.filter.flag = Some(PickFlag::Picked);
                        self.reload_catalog();
                    }

                    if ui.selectable_label(self.filter.flag == Some(PickFlag::Rejected), "❌ Rejected").clicked() {
                        self.filter.flag = Some(PickFlag::Rejected);
                        self.reload_catalog();
                    }

                    if ui.selectable_label(self.filter.flag == Some(PickFlag::Unflagged), "⚪ Unflagged").clicked() {
                        self.filter.flag = Some(PickFlag::Unflagged);
                        self.reload_catalog();
                    }

                    ui.add_space(12.0);
                    ui.heading("Quick Actions");
                    ui.separator();
                    if ui.button("📥 Import Videos...").clicked() {
                        if let Some(files) = rfd::FileDialog::new()
                            .add_filter("Magic Lantern Video", &["mlv", "MLV"])
                            .pick_files()
                        {
                            for f in files {
                                self.ingest_path(&f);
                            }
                        }
                    }
                    if ui.button("📁 Import Folder...").clicked() {
                        if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                            self.ingest_path(&folder);
                        }
                    }

                    ui.add_space(16.0);
                    ui.heading("Shortcuts");
                    ui.separator();
                    ui.label(egui::RichText::new("G : Grid View\nE : Player View\nSpace : Play / Pause\n1-5 : Star Rating\nP : Pick Flag\nX : Reject Flag\nU : Unflag").small());
                });
        }

        // Right Sidebar (Metadata & Tags Inspector)
        if self.show_right_sidebar {
            egui::SidePanel::right("right_metadata_panel")
                .default_width(280.0)
                .width_range(240.0..=400.0)
                .show(ctx, |ui| {
                    let mut selected_video = self.selected_video_idx.and_then(|idx| self.videos.get_mut(idx));
                    self.metadata_panel.show(ui, selected_video.as_deref_mut(), &self.db);
                });
        }

        // Central Workspace (Grid or Player View)
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.view_mode {
                ViewMode::Grid => {
                    if let Some(action) = self.grid_view.show(ui, &self.videos, self.selected_video_idx) {
                        match action {
                            GridAction::Select(idx) => {
                                self.selected_video_idx = Some(idx);
                            }
                            GridAction::OpenPlayer(idx) => {
                                self.selected_video_idx = Some(idx);
                                self.view_mode = ViewMode::Player;
                            }
                            GridAction::TogglePick(id) => {
                                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                                    let new_flag = if vid.flag == PickFlag::Picked { PickFlag::Unflagged } else { PickFlag::Picked };
                                    vid.flag = new_flag;
                                    let _ = self.db.set_flag(id, new_flag);
                                }
                            }
                            GridAction::SetRating(id, rating) => {
                                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                                    vid.rating = rating;
                                    let _ = self.db.set_rating(id, rating);
                                }
                            }
                        }
                    }
                }
                ViewMode::Player => {
                    if let Some(idx) = self.selected_video_idx {
                        if let Some(vid) = self.videos.get(idx) {
                            if let Some(action) = self.player_view.show(ui, vid) {
                                match action {
                                    PlayerAction::BackToGrid => {
                                        self.view_mode = ViewMode::Grid;
                                    }
                                    PlayerAction::FrameChanged(_frame_idx) => {
                                        // Request next frame render
                                    }
                                }
                            }
                        }
                    } else {
                        self.view_mode = ViewMode::Grid;
                    }
                }
            }
        });

        // Backup & Import Modal dialog
        if self.catalog_modal.show(ctx, &self.db) {
            self.reload_catalog();
        }
    }
}
