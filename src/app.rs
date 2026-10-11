use crate::db::{default_catalog_path, Database, Facets, FilterCriteria, PickFlag, VideoRecord};
use crate::gpu;
use crate::mlv::{FrameSource, MlvClip};
use crate::playback::Playback;
use crate::thumbnails::ThumbnailManager;
use crate::ui::*;
use eframe::egui::{self, RichText};
use std::path::{Path, PathBuf};

#[derive(Debug, PartialEq, Eq)]
pub enum ViewMode {
    Grid,
    Player,
}

#[derive(Debug, Clone)]
pub struct ImportProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
}

pub enum ImportMessage {
    Progress {
        current: usize,
        total: usize,
        current_file: String,
        imported: usize,
        duplicates: usize,
        failed: usize,
    },
    Complete {
        total: usize,
        imported: usize,
        duplicates: usize,
        failed: usize,
    },
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

    // Background Importer
    import_tx: crossbeam_channel::Sender<ImportMessage>,
    import_rx: crossbeam_channel::Receiver<ImportMessage>,
    import_progress: Option<ImportProgress>,

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

        let (import_tx, import_rx) = crossbeam_channel::unbounded();

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
            import_tx,
            import_rx,
            import_progress: None,
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

    /// Start a background import of files or directories, skipping duplicates and tracking progress
    pub fn start_import(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let mut files_to_import = Vec::new();
        for path in paths {
            if path.is_file() {
                if path.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("mlv")) {
                    files_to_import.push(path);
                }
            } else if path.is_dir() {
                for entry in walkdir::WalkDir::new(path).into_iter().filter_map(|e| e.ok()) {
                    let p = entry.into_path();
                    if p.is_file() && p.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("mlv")) {
                        files_to_import.push(p);
                    }
                }
            }
        }

        // Deduplicate paths in the current batch
        let mut seen = std::collections::HashSet::new();
        files_to_import.retain(|p| seen.insert(p.clone()));

        if files_to_import.is_empty() {
            self.status_bar_text = "No .MLV files found in selected path(s).".to_string();
            return;
        }

        let total = files_to_import.len();
        self.import_progress = Some(ImportProgress {
            current: 0,
            total,
            current_file: String::new(),
            imported: 0,
            duplicates: 0,
            failed: 0,
        });

        let tx = self.import_tx.clone();
        let db_path = self.db.path.clone();
        let ctx_clone = ctx.clone();

        std::thread::spawn(move || {
            let db = match Database::open(&db_path) {
                Ok(db) => db,
                Err(e) => {
                    eprintln!("Failed to open DB for import worker: {e}");
                    return;
                }
            };

            let mut imported = 0;
            let mut duplicates = 0;
            let mut failed = 0;

            for (idx, path) in files_to_import.into_iter().enumerate() {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "clip.mlv".to_string());

                let _ = tx.send(ImportMessage::Progress {
                    current: idx + 1,
                    total,
                    current_file: file_name.clone(),
                    imported,
                    duplicates,
                    failed,
                });
                ctx_clone.request_repaint();

                // 1. Path-based check: is this path or its canonical equivalent already in the catalog?
                if db.is_file_imported(&path) {
                    duplicates += 1;
                    continue;
                }

                // 2. Open clip header & check content signature
                match MlvClip::open(&path) {
                    Ok(clip) => {
                        // Check if identical content (file size, frames, date, camera) already exists
                        match db.insert_new_video(&clip.meta) {
                            Ok(Some(_id)) => {
                                imported += 1;
                            }
                            Ok(None) => {
                                duplicates += 1;
                            }
                            Err(e) => {
                                eprintln!("Error saving clip {}: {}", path.display(), e);
                                failed += 1;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("Failed to read MLV file {}: {}", path.display(), e);
                        failed += 1;
                    }
                }
            }

            let _ = tx.send(ImportMessage::Complete {
                total,
                imported,
                duplicates,
                failed,
            });
            ctx_clone.request_repaint();
        });
    }

    /// Open a given clip file path in MLV App executable
    pub fn open_in_mlv_app(&mut self, file_path: &Path) {
        let abs_path = match std::fs::canonicalize(file_path) {
            Ok(p) => p,
            Err(_) => file_path.to_path_buf(),
        };

        // 1. Check if user already configured/saved an executable path in database
        let mut exe_path: Option<PathBuf> = self
            .db
            .get_setting("mlv_app_path")
            .ok()
            .flatten()
            .map(PathBuf::from)
            .filter(|p| p.is_file());

        // 2. If not saved or file was moved/deleted, search standard installation paths & PATH
        if exe_path.is_none() {
            let mut candidates: Vec<PathBuf> = Vec::new();

            // Check standard program directories on Windows
            if let Ok(prog_files) = std::env::var("ProgramFiles") {
                candidates.push(PathBuf::from(&prog_files).join("MLV App").join("MLV App.exe"));
                candidates.push(PathBuf::from(&prog_files).join("MLVApp").join("MLV App.exe"));
                candidates.push(PathBuf::from(&prog_files).join("MLVApp").join("mlvapp.exe"));
            }
            if let Ok(prog_files_x86) = std::env::var("ProgramFiles(x86)") {
                candidates.push(PathBuf::from(&prog_files_x86).join("MLV App").join("MLV App.exe"));
                candidates.push(PathBuf::from(&prog_files_x86).join("MLVApp").join("mlvapp.exe"));
            }
            if let Ok(local_appdata) = std::env::var("LOCALAPPDATA") {
                candidates.push(PathBuf::from(&local_appdata).join("Programs").join("MLV App").join("MLV App.exe"));
                candidates.push(PathBuf::from(&local_appdata).join("MLV App").join("MLV App.exe"));
            }

            // Also check Desktop, Downloads, and common user folders where portable builds are extracted
            if let Ok(user_profile) = std::env::var("USERPROFILE") {
                let up = PathBuf::from(user_profile);
                candidates.push(up.join("Downloads").join("MLV App.exe"));
                candidates.push(up.join("Desktop").join("MLV App.exe"));
            }

            // Check system PATH
            if let Ok(path_var) = std::env::var("PATH") {
                for dir in std::env::split_paths(&path_var) {
                    let c1 = dir.join("MLV App.exe");
                    let c2 = dir.join("mlvapp.exe");
                    let c3 = dir.join("mlv_app.exe");
                    if c1.is_file() {
                        candidates.push(c1);
                        break;
                    }
                    if c2.is_file() {
                        candidates.push(c2);
                        break;
                    }
                    if c3.is_file() {
                        candidates.push(c3);
                        break;
                    }
                }
            }

            for cand in candidates {
                if cand.is_file() {
                    exe_path = Some(cand);
                    break;
                }
            }
        }

        // 3. If still not found, open a file picker to prompt user to locate MLV App
        if exe_path.is_none() {
            let filter_ext = if cfg!(target_os = "windows") {
                &["exe"][..]
            } else if cfg!(target_os = "macos") {
                &["app"][..]
            } else {
                &["AppImage", "bin", "*"][..]
            };

            if let Some(picked) = rfd::FileDialog::new()
                .set_title("Locate MLV App executable (MLV App.exe)")
                .add_filter("MLV App Executable", filter_ext)
                .pick_file()
            {
                if picked.is_file() {
                    let _ = self.db.set_setting("mlv_app_path", &picked.to_string_lossy());
                    exe_path = Some(picked);
                }
            }
        }

        // 4. Launch MLV App with the clip path as argument
        if let Some(exe) = exe_path {
            let _ = self.db.set_setting("mlv_app_path", &exe.to_string_lossy());
            match std::process::Command::new(&exe).arg(&abs_path).spawn() {
                Ok(_) => {
                    self.status_bar_text = format!("Launched MLV App with: {}", abs_path.display());
                }
                Err(e) => {
                    self.status_bar_text = format!("Failed to launch MLV App ({}): {}", exe.display(), e);
                }
            }
        } else {
            self.status_bar_text = "MLV App executable not configured or not found.".to_string();
        }
    }

    /// Handle drag-and-drop file imports from OS
    fn handle_drag_and_drop(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            let paths: Vec<PathBuf> = dropped_files.into_iter().filter_map(|f| f.path).collect();
            if !paths.is_empty() {
                self.start_import(paths, ctx);
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
        // Poll incoming import progress messages
        while let Ok(msg) = self.import_rx.try_recv() {
            match msg {
                ImportMessage::Progress { current, total, current_file, imported, duplicates, failed } => {
                    self.import_progress = Some(ImportProgress {
                        current,
                        total,
                        current_file,
                        imported,
                        duplicates,
                        failed,
                    });
                }
                ImportMessage::Complete { total: _, imported, duplicates, failed } => {
                    self.import_progress = None;
                    let mut summary = format!("Import complete: {} added", imported);
                    if duplicates > 0 {
                        summary.push_str(&format!(", {} duplicates skipped", duplicates));
                    }
                    if failed > 0 {
                        summary.push_str(&format!(", {} failed", failed));
                    }
                    self.status_bar_text = summary;
                    self.reload_catalog();
                }
            }
        }

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
                                    self.start_import(files, ctx);
                                }
                            }
                            TopBarAction::ImportFolder => {
                                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                                    self.start_import(vec![folder], ctx);
                                }
                            }
                            TopBarAction::OpenCatalogModal => {
                                self.catalog_dialog.is_open = true;
                            }
                        }
                    }
                });
        }

        // Import Progress Banner (shown below Top Bar when an import is actively running)
        if let Some(prog) = &self.import_progress {
            egui::TopBottomPanel::top("import_progress_banner")
                .exact_height(32.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(RichText::new(format!("Importing ({}/{}):", prog.current, prog.total)).strong().color(TEXT_PRIMARY));
                        ui.label(RichText::new(&prog.current_file).color(ACCENT_BLUE));

                        let fraction = if prog.total > 0 { prog.current as f32 / prog.total as f32 } else { 0.0 };
                        ui.add(
                            egui::ProgressBar::new(fraction)
                                .show_percentage()
                                .animate(true)
                                .desired_width(200.0),
                        );

                        ui.label(
                            RichText::new(format!("{} added · {} duplicates skipped", prog.imported, prog.duplicates))
                                .small()
                                .color(TEXT_MUTED),
                        );
                    });
                });
        }

        // 2. Status Bar (Bottom)
        if !is_fullscreen_player {
            egui::TopBottomPanel::bottom("studio_status_bar")
                .exact_height(24.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if let Some(prog) = &self.import_progress {
                            ui.spinner();
                            let fraction = if prog.total > 0 { prog.current as f32 / prog.total as f32 } else { 0.0 };
                            ui.add(
                                egui::ProgressBar::new(fraction)
                                    .animate(true)
                                    .desired_width(120.0),
                            );
                            ui.label(
                                RichText::new(format!(
                                    "Importing {}/{} ({}) · {} skipped",
                                    prog.current, prog.total, prog.current_file, prog.duplicates
                                ))
                                .small()
                                .color(ACCENT_BLUE),
                            );
                        } else {
                            ui.label(RichText::new(&self.status_bar_text).small().color(TEXT_MUTED));
                        }
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
                    if let Some(act) = self.inspector.show(ui, sel_vid.as_deref_mut(), &self.db, &self.all_tags) {
                        match act {
                            crate::ui::inspector::InspectorAction::OpenInMlvApp => {
                                if let Some(idx) = self.selected_video_idx {
                                    if let Some(vid) = self.videos.get(idx) {
                                        let path = PathBuf::from(&vid.file_path);
                                        self.open_in_mlv_app(&path);
                                    }
                                }
                            }
                        }
                    }
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
                            GridAction::OpenInMlvApp(idx) => {
                                self.selected_video_idx = Some(idx);
                                if let Some(vid) = self.videos.get(idx) {
                                    let path = PathBuf::from(&vid.file_path);
                                    self.open_in_mlv_app(&path);
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
                                            PlayerAction::OpenInMlvApp => {
                                                let path = PathBuf::from(&vid.file_path);
                                                self.open_in_mlv_app(&path);
                                            }
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
