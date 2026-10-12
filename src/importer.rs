use crate::db::Database;
use crate::mlv::MlvClip;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// Progress update message sent from background import worker to UI
#[derive(Debug, Clone)]
pub struct ImportProgress {
    pub current: usize,
    pub total: usize,
    pub current_file: String,
    pub imported: usize,
    pub duplicates: usize,
    pub failed: usize,
}

/// Messages sent via crossbeam channel from importer thread
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
    Cancelled {
        total: usize,
        imported: usize,
        duplicates: usize,
        failed: usize,
    },
}

/// Background importer engine for MLV files and directory trees
pub struct Importer;

impl Importer {
    /// Collect all `.mlv` files recursively from a set of paths, deduplicating them
    pub fn collect_mlv_files(paths: Vec<PathBuf>) -> Vec<PathBuf> {
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

        let mut seen = std::collections::HashSet::new();
        files_to_import.retain(|p| seen.insert(p.clone()));
        files_to_import
    }

    /// Spawn background import thread
    pub fn start(
        paths: Vec<PathBuf>,
        db_path: PathBuf,
        tx: crossbeam_channel::Sender<ImportMessage>,
        cancel_flag: Arc<AtomicBool>,
        ctx: egui::Context,
    ) {
        let files = Self::collect_mlv_files(paths);
        let total = files.len();

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

            for (idx, path) in files.into_iter().enumerate() {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = tx.send(ImportMessage::Cancelled {
                        total,
                        imported,
                        duplicates,
                        failed,
                    });
                    ctx.request_repaint();
                    return;
                }

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
                ctx.request_repaint();

                // 1. Path check: is path already in catalog?
                if db.is_file_imported(&path) {
                    duplicates += 1;
                    continue;
                }

                // 2. Open clip header and extract metadata
                match MlvClip::open(&path) {
                    Ok(clip) => {
                        // 3. Signature check: duplicate file size + frame count + camera serial
                        if db.is_duplicate_clip(&clip.meta) {
                            duplicates += 1;
                        } else {
                            match db.upsert_video(&clip.meta) {
                                Ok(_) => imported += 1,
                                Err(_) => failed += 1,
                            }
                        }
                    }
                    Err(_) => {
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
            ctx.request_repaint();
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_importer_cancellation() {
        let (tx, rx) = crossbeam_channel::unbounded();
        let cancel_flag = Arc::new(AtomicBool::new(true)); // Already cancelled

        let temp_dir = std::env::temp_dir().join(format!("importer_test_{}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()));
        std::fs::create_dir_all(&temp_dir).unwrap();
        let db_path = temp_dir.join("test.db");
        let _ = Database::open(&db_path).unwrap();

        let clip_path = temp_dir.join("test_clip.mlv");
        std::fs::write(&clip_path, b"MLVI").unwrap();
        let dummy_paths = vec![clip_path];

        Importer::start(dummy_paths, db_path, tx, cancel_flag, egui::Context::default());

        // We should receive Cancelled, NOT Complete
        let msg = rx.recv_timeout(std::time::Duration::from_secs(2)).expect("Should receive message");
        match msg {
            ImportMessage::Cancelled { imported, .. } => {
                assert_eq!(imported, 0);
            }
            _ => panic!("Expected Cancelled message"),
        }

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

