use crate::db::{default_catalog_path, Database, Facets, FilterCriteria, PickFlag, VideoRecord};
use crate::gpu;
use crate::mlv::{FrameSource, MlvClip};
use crate::playback::Playback;
use crate::thumbnails::ThumbnailManager;
use crate::ui::*;
use eframe::egui::{self, RichText};
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
    facets: Facets,
    all_tags: Vec<String>,
    cameras: Vec<String>,
    lenses: Vec<String>,

    // Media & Playback
    thumb_mgr: ThumbnailManager,
    current_playback: Option<(i64, Playback)>, // (video_id, Playback)

    // UI Panels & Windows
    grid_view: GridView,
    player_view: PlayerView,
    inspector: Inspector,
    catalog_dialog: CatalogDialog,

    // Status bar & notification
    gpu_info: String,
    status_bar_text: String,
}

impl MlvExplorerApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Apply professional dark studio theme
        theme::apply_theme(&cc.egui_ctx);

        // Initialize GPU debayer shaders via wgpu
        let gpu_info = if let Some(render_state) = &cc.wgpu_render_state {
            gpu::init(render_state)
        } else {
            "Software Viewport".to_string()
        };

        // Open SQLite catalog
        let db_path = default_catalog_path();
        let db = Database::open(&db_path).expect("Failed to initialize SQLite catalog");

        let thumb_mgr = ThumbnailManager::new(db_path.clone(), cc.egui_ctx.clone());

        let mut app = Self {
            db,
            view_mode: ViewMode::Grid,
            videos: Vec::new(),
            selected_video_idx: None,
            filter: FilterCriteria::default(),
            facets: Facets::default(),
            all_tags: Vec::new(),
            cameras: Vec::new(),
            lenses: Vec::new(),
            thumb_mgr,
            current_playback: None,
            grid_view: GridView::default(),
            player_view: PlayerView::default(),
            inspector: Inspector::default(),
            catalog_dialog: CatalogDialog::default(),
            gpu_info,
            status_bar_text: "Ready. Drag and drop .MLV files or folders to import.".to_string(),
        };

        // If database was migrated from v1 (which had broken offsets), rescan all files
        if app.db.needs_rescan {
            app.rescan_all_files();
        }

        app.reload_catalog();
        app
    }

    /// Rescan all files in catalog to correct corrupted metadata from previous versions
    fn rescan_all_files(&mut self) {
        if let Ok(paths) = self.db.all_paths() {
            let mut count = 0;
            for p in &paths {
                if let Ok(clip) = MlvClip::open(p) {
                    let _ = self.db.upsert_video(&clip.meta);
                    count += 1;
                }
            }
            if count > 0 {
                self.status_bar_text = format!("Refreshed metadata for {} clips.", count);
            }
        }
    }

    /// Reload catalog data, facets, and filter criteria
    pub fn reload_catalog(&mut self) {
        if let Ok(videos) = self.db.get_videos(&self.filter) {
            self.videos = videos;
            if let Some(idx) = self.selected_video_idx {
                if idx >= self.videos.len() {
                    self.selected_video_idx = if self.videos.is_empty() { None } else { Some(0) };
                }
            }
        }

        if let Ok(facets) = self.db.facets() {
            self.cameras = facets.cameras.iter().map(|c| c.name.clone()).collect();
            self.lenses = facets.lenses.iter().map(|l| l.name.clone()).collect();
            self.facets = facets;
        }

        if let Ok(tags) = self.db.all_tag_names() {
            self.all_tags = tags;
        }

        // Keep playback in sync with selected video
        self.sync_playback();
    }

    /// Synchronize active video playback engine with the selected video
    fn sync_playback(&mut self) {
        let Some(idx) = self.selected_video_idx else {
            self.current_playback = None;
            return;
        };

        let Some(vid) = self.videos.get(idx) else {
            self.current_playback = None;
            return;
        };

        let current_id = self.current_playback.as_ref().map(|(id, _)| *id);
        if current_id != Some(vid.id) {
            self.current_playback = None;
        }
    }

    /// Ensure active playback engine is loaded for current video
    fn ensure_playback(&mut self, ctx: &egui::Context) -> bool {
        let Some(idx) = self.selected_video_idx else { return false; };
        let Some(vid) = self.videos.get(idx) else { return false; };

        let needs_load = match &self.current_playback {
            Some((id, _)) => *id != vid.id,
            None => true,
        };

        if needs_load {
            match FrameSource::open(&vid.file_path) {
                Ok(source) => {
                    let pb = Playback::new(source, ctx.clone());
                    self.current_playback = Some((vid.id, pb));
                    self.player_view.current_frame = 0;
                    true
                }
                Err(e) => {
                    self.status_bar_text = format!("Error opening MLV: {e}");
                    false
                }
            }
        } else {
            true
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
        match MlvClip::open(path) {
            Ok(clip) => {
                let _ = self.db.upsert_video(&clip.meta);
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

    /// Global keyboard shortcuts
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        let input = ctx.input(|i| i.clone());

        // Mode switching: G -> Grid, E / Enter -> Player, F -> Fullscreen Player
        if input.key_pressed(egui::Key::G) {
            self.view_mode = ViewMode::Grid;
            self.player_view.is_fullscreen = false;
        }
        if input.key_pressed(egui::Key::E) || (self.view_mode == ViewMode::Grid && input.key_pressed(egui::Key::Enter)) {
            if self.selected_video_idx.is_some() {
                self.view_mode = ViewMode::Player;
                self.ensure_playback(ctx);
            }
        }
        if input.key_pressed(egui::Key::F) {
            if self.view_mode == ViewMode::Grid {
                if self.selected_video_idx.is_some() {
                    self.view_mode = ViewMode::Player;
                    self.player_view.is_fullscreen = true;
                    self.ensure_playback(ctx);
                }
            } else {
                self.player_view.is_fullscreen = !self.player_view.is_fullscreen;
            }
        }

        // Navigation in Grid (Left / Right / Up / Down)
        if self.view_mode == ViewMode::Grid && !self.videos.is_empty() {
            if input.key_pressed(egui::Key::ArrowRight) {
                if let Some(idx) = self.selected_video_idx {
                    if idx + 1 < self.videos.len() {
                        self.selected_video_idx = Some(idx + 1);
                    }
                } else {
                    self.selected_video_idx = Some(0);
                }
            }
            if input.key_pressed(egui::Key::ArrowLeft) {
                if let Some(idx) = self.selected_video_idx {
                    if idx > 0 {
                        self.selected_video_idx = Some(idx - 1);
                    }
                }
            }
        }

        // Rating & Flags shortcuts on selected video
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
        self.thumb_mgr.update(ctx);
        self.handle_drag_and_drop(ctx);
        self.handle_keyboard_shortcuts(ctx);

        let is_fullscreen_player = self.view_mode == ViewMode::Player && self.player_view.is_fullscreen;

        // Sync fullscreen state with window viewport
        let was_fullscreen = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
        if was_fullscreen != is_fullscreen_player {
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(is_fullscreen_player));
        }

        // 1. Studio Top Bar
        if !is_fullscreen_player {
            egui::TopBottomPanel::top("studio_top_bar")
                .exact_height(38.0)
                .show(ctx, |ui| {
                    let can_open_player = self.selected_video_idx.is_some();
                    let is_player_mode = self.view_mode == ViewMode::Player;

                    let (filter_changed, action) = TopBar::show(
                        ui,
                        &mut self.filter,
                        is_player_mode,
                        can_open_player,
                        &self.cameras,
                        &self.lenses,
                    );

                    if filter_changed {
                        self.reload_catalog();
                    }

                    if let Some(act) = action {
                        match act {
                            TopBarAction::SwitchToGrid => {
                                self.view_mode = ViewMode::Grid;
                            }
                            TopBarAction::SwitchToPlayer => {
                                if self.selected_video_idx.is_some() {
                                    self.view_mode = ViewMode::Player;
                                    self.ensure_playback(ctx);
                                }
                            }
                            TopBarAction::ImportFiles => {
                                if let Some(files) = rfd::FileDialog::new()
                                    .add_filter("Magic Lantern RAW Video", &["mlv", "MLV"])
                                    .pick_files()
                                {
                                    for f in files {
                                        self.ingest_path(&f);
                                    }
                                }
                            }
                            TopBarAction::ImportFolder => {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.ingest_path(&folder);
                                }
                            }
                            TopBarAction::OpenCatalogModal => {
                                self.catalog_dialog.is_open = true;
                            }
                        }
                    }
                });
        }

        // 2. Status Bar (Bottom)
        if !is_fullscreen_player {
            egui::TopBottomPanel::bottom("studio_status_bar")
                .exact_height(24.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(&self.status_bar_text).small().color(TEXT_MUTED));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(RichText::new(&self.gpu_info).small().color(TEXT_MUTED));
                            ui.separator();
                            ui.label(RichText::new(format!("{} clips", self.videos.len())).small().color(TEXT_SECONDARY));
                        });
                    });
                });
        }

        // 3. Left Navigator Sidebar
        if !is_fullscreen_player {
            egui::SidePanel::left("studio_left_navigator")
                .default_width(220.0)
                .width_range(180.0..=320.0)
                .show(ctx, |ui| {
                    if Navigator::show(ui, &mut self.filter, &self.facets) {
                        self.reload_catalog();
                    }
                });
        }

        // 4. Right Metadata Inspector Sidebar
        if !is_fullscreen_player {
            egui::SidePanel::right("studio_right_inspector")
                .default_width(280.0)
                .width_range(240.0..=380.0)
                .show(ctx, |ui| {
                    let mut sel_vid = self.selected_video_idx.and_then(|idx| self.videos.get_mut(idx));
                    self.inspector.show(ui, sel_vid.as_deref_mut(), &self.db, &self.all_tags);
                });
        }

        // 5. Central Workspace: Grid or Player View
        egui::CentralPanel::default().show(ctx, |ui| {
            match self.view_mode {
                ViewMode::Grid => {
                    if let Some(action) = self.grid_view.show(
                        ui,
                        &self.videos,
                        self.selected_video_idx,
                        &mut self.thumb_mgr,
                    ) {
                        match action {
                            GridAction::Select(idx) => {
                                self.selected_video_idx = Some(idx);
                            }
                            GridAction::OpenPlayer(idx) => {
                                self.selected_video_idx = Some(idx);
                                self.view_mode = ViewMode::Player;
                                self.ensure_playback(ctx);
                            }
                            GridAction::TogglePick(id) => {
                                if let Some(vid) = self.videos.iter_mut().find(|v| v.id == id) {
                                    let new_flag = if vid.flag == PickFlag::Picked { PickFlag::Unflagged } else { PickFlag::Picked };
                                    vid.flag = new_flag;
                                    let _ = self.db.set_flag(id, new_flag);
                                    let _ = self.reload_catalog();
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
                    let has_pb = self.ensure_playback(ctx);
                    if has_pb {
                        if let Some((_id, ref mut pb)) = self.current_playback {
                            if let Some(idx) = self.selected_video_idx {
                                if let Some(vid) = self.videos.get(idx) {
                                    if let Some(act) = self.player_view.show(ui, vid, pb) {
                                        match act {
                                            PlayerAction::BackToGrid => {
                                                self.view_mode = ViewMode::Grid;
                                                self.player_view.is_fullscreen = false;
                                            }
                                            PlayerAction::Seek(frame) => {
                                                pb.set_target(frame, self.player_view.is_looping);
                                            }
                                            PlayerAction::SetRating(rating) => {
                                                let id = vid.id;
                                                let _ = self.db.set_rating(id, rating);
                                                self.reload_catalog();
                                            }
                                            PlayerAction::SetFlag(flag) => {
                                                let id = vid.id;
                                                let _ = self.db.set_flag(id, flag);
                                                self.reload_catalog();
                                            }
                                            PlayerAction::SetStretch(factor) => {
                                                let id = vid.id;
                                                let _ = self.db.set_stretch_factor(id, factor);
                                                self.reload_catalog();
                                            }
                                            PlayerAction::ToggleFullscreen => {}
                                        }
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

        // 6. Catalog Backup & Import Modal
        if self.catalog_dialog.show(ctx, &self.db) {
            self.reload_catalog();
        }
    }
}
