use crate::db::Database;
use crate::mlv::{render_preview, FrameSource, MlvClip};
use crossbeam_channel::Sender;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

/// Information about a detected system drive or mounted card
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DetectedDrive {
    pub path: PathBuf,
    pub label: String,
    pub is_removable_or_card: bool,
}

/// Detects available drives on the system.
/// Prioritizes drives with a `DCIM` or camera directory.
pub fn detect_available_drives() -> Vec<DetectedDrive> {
    let mut drives = Vec::new();

    #[cfg(target_os = "windows")]
    {
        for letter in b'A'..=b'Z' {
            let drive_str = format!("{}:\\", letter as char);
            let path = PathBuf::from(&drive_str);
            if path.exists() {
                let dcim = path.join("DCIM");
                let has_dcim = dcim.is_dir();
                let label = if has_dcim {
                    format!("{}: (Camera Card / DCIM)", letter as char)
                } else {
                    format!("{}: Drive", letter as char)
                };
                drives.push(DetectedDrive {
                    path,
                    label,
                    is_removable_or_card: has_dcim,
                });
            }
        }
    }

    #[cfg(not(target_os = "windows"))]
    {
        // macOS volumes
        let volumes = Path::new("/Volumes");
        if volumes.is_dir() {
            if let Ok(entries) = fs::read_dir(volumes) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let p = entry.path();
                    if p.is_dir() {
                        let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                        let has_dcim = p.join("DCIM").is_dir();
                        drives.push(DetectedDrive {
                            path: p,
                            label: if has_dcim { format!("{name} (Camera Card)") } else { name },
                            is_removable_or_card: true,
                        });
                    }
                }
            }
        }

        // Linux mount points
        for base in &["/media", "/run/media"] {
            let p_base = Path::new(base);
            if p_base.is_dir() {
                if let Ok(entries) = fs::read_dir(p_base) {
                    for entry in entries.filter_map(|e| e.ok()) {
                        let p = entry.path();
                        if p.is_dir() {
                            let name = p.file_name().unwrap_or_default().to_string_lossy().to_string();
                            let has_dcim = p.join("DCIM").is_dir();
                            drives.push(DetectedDrive {
                                path: p,
                                label: if has_dcim { format!("{name} (Camera Card)") } else { name },
                                is_removable_or_card: true,
                            });
                        }
                    }
                }
            }
        }
    }

    // Sort drives so camera cards appear first
    drives.sort_by(|a, b| b.is_removable_or_card.cmp(&a.is_removable_or_card));
    drives
}

/// Information about a detected MLV clip found on the source card/drive
#[derive(Debug, Clone)]
pub struct DetectedClip {
    pub id: usize,
    pub main_path: PathBuf,
    pub file_name: String,
    pub spanned_chunks: Vec<PathBuf>,
    pub total_size_bytes: u64,
    pub already_in_catalog: bool,
    pub resolution: Option<(u32, u32)>,
    pub fps: Option<f32>,
    pub frame_count: Option<u32>,
    pub duration_secs: Option<f32>,
    pub camera: Option<String>,
    pub date_time: Option<String>,
    pub selected: bool,
}

/// Helper function to format byte counts into human-readable strings
pub fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

/// Scan a source folder recursively for `.MLV` files and collect metadata & spanned companions
pub fn scan_source(
    source_dir: &Path,
    db: &Database,
    cancel_flag: &Arc<AtomicBool>,
) -> Vec<DetectedClip> {
    let mut mlv_paths = Vec::new();

    // Traverse directory looking for .mlv files
    for entry in walkdir::WalkDir::new(source_dir).into_iter().filter_map(|e| e.ok()) {
        if cancel_flag.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let p = entry.into_path();
        if p.is_file() && p.extension().map_or(false, |ext| ext.eq_ignore_ascii_case("mlv")) {
            mlv_paths.push(p);
        }
    }

    // Deduplicate main paths
    let mut seen = std::collections::HashSet::new();
    mlv_paths.retain(|p| seen.insert(p.clone()));
    mlv_paths.sort();

    let mut detected = Vec::new();

    for (idx, path) in mlv_paths.into_iter().enumerate() {
        if cancel_flag.load(Ordering::Relaxed) {
            break;
        }

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("clip_{idx}.mlv"));

        // Discover companion chunks (.M00, .M01, sibling card folders, etc.)
        let chunks = MlvClip::resolve_spanned_files(&path);

        // Sum size of all chunks
        let mut total_size_bytes = 0u64;
        for c in &chunks {
            if let Ok(meta) = fs::metadata(c) {
                total_size_bytes += meta.len();
            }
        }

        // Check if path or signature already exists in catalog
        let in_catalog_by_path = db.is_file_imported(&path);
        let mut in_catalog_by_sig = false;
        let mut resolution = None;
        let mut fps = None;
        let mut frame_count = None;
        let mut duration_secs = None;
        let mut camera = None;
        let mut date_time = None;

        if let Ok(clip) = MlvClip::open(&path) {
            in_catalog_by_sig = db.is_duplicate_clip(&clip.meta);
            resolution = Some((clip.meta.width, clip.meta.height));
            let fps_val = clip.meta.fps();
            fps = Some(fps_val);
            frame_count = Some(clip.meta.frame_count);
            duration_secs = Some(clip.meta.duration_seconds());
            if !clip.meta.camera_name.is_empty() {
                camera = Some(clip.meta.camera_name.clone());
            }
            if let Some(ref d) = clip.meta.date_recorded {
                date_time = Some(d.clone());
            }
        }

        let already_in_catalog = in_catalog_by_path || in_catalog_by_sig;
        let selected = !already_in_catalog;

        detected.push(DetectedClip {
            id: idx,
            main_path: path,
            file_name,
            spanned_chunks: chunks,
            total_size_bytes,
            already_in_catalog,
            resolution,
            fps,
            frame_count,
            duration_secs,
            camera,
            date_time,
            selected,
        });
    }

    detected
}

/// Decoded thumbnail preview for small view
pub struct ClipThumb {
    pub clip_id: usize,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Spawns a background thread to generate preview thumbnails for detected clips
pub fn spawn_thumb_generator(
    clips: Vec<(usize, PathBuf)>,
    cancel_flag: Arc<AtomicBool>,
    tx: Sender<ClipThumb>,
    ctx: eframe::egui::Context,
) {
    std::thread::spawn(move || {
        for (clip_id, path) in clips {
            if cancel_flag.load(Ordering::Relaxed) {
                break;
            }

            let path_str = path.to_string_lossy();
            if let Ok(source) = FrameSource::open(&path_str) {
                if let Ok(frame) = source.decode(0) {
                    let (w, h, rgba) = render_preview(&frame, &source.clip.meta, 220);
                    if tx.send(ClipThumb { clip_id, width: w, height: h, rgba }).is_err() {
                        break;
                    }
                    ctx.request_repaint();
                }
            }
        }
    });
}

/// Precomputed CRC-32 table (IEEE 802.3 standard polynomial 0xEDB88320)
pub const CRC32_TABLE: [u32; 256] = {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB88320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// Updates a running CRC-32 with the provided byte slice
#[inline]
pub fn crc32_update(mut crc: u32, buf: &[u8]) -> u32 {
    for &b in buf {
        crc = (crc >> 8) ^ CRC32_TABLE[((crc ^ (b as u32)) & 0xFF) as usize];
    }
    crc
}

/// Finalizes a running CRC-32 value
#[inline]
pub fn crc32_finish(crc: u32) -> u32 {
    crc ^ 0xFFFFFFFF
}

/// Computes the complete CRC-32 checksum of a file on disk
pub fn compute_file_crc32(
    path: &Path,
    cancel_flag: &Arc<AtomicBool>,
) -> std::io::Result<u32> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(2 * 1024 * 1024, file);
    let mut buf = vec![0u8; 1024 * 1024];
    let mut crc = 0xFFFFFFFFu32;

    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled"));
        }
        let read = reader.read(&mut buf)?;
        if read == 0 {
            break;
        }
        crc = crc32_update(crc, &buf[..read]);
    }
    Ok(crc32_finish(crc))
}

/// Active ingestion processing stage for a file
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IngestStage {
    Copying,
    VerifyingChecksum,
}

/// Destination organization mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrganizeOption {
    Direct,
    DateFolder,
    CustomFolder,
}

/// Settings for an ingestion copy operation
#[derive(Debug, Clone)]
pub struct IngestOptions {
    pub destination_dir: PathBuf,
    pub organize: OrganizeOption,
    pub custom_subfolder: String,
    pub add_to_catalog: bool,
    pub skip_existing: bool,
    pub verify_checksum: bool,
}

/// Messages emitted during ingest execution
#[derive(Debug, Clone)]
pub enum IngestMessage {
    Progress {
        current_clip: usize,
        total_clips: usize,
        current_file: String,
        stage: IngestStage,
        file_bytes_copied: u64,
        file_total_bytes: u64,
        total_bytes_copied: u64,
        total_bytes: u64,
        speed_mbps: f32,
    },
    ClipComplete {
        clip_id: usize,
        file_name: String,
        dest_main_path: PathBuf,
    },
    Complete {
        clips_copied: usize,
        files_copied: usize,
        total_bytes: u64,
        failed: usize,
        checksums_verified: bool,
    },
    Cancelled,
    Error(String),
}

/// Background ingestion engine that copies all selected MLV files and companion chunks
pub struct Ingester;

impl Ingester {
    pub fn start(
        clips: Vec<DetectedClip>,
        options: IngestOptions,
        db_path: PathBuf,
        tx: Sender<IngestMessage>,
        cancel_flag: Arc<AtomicBool>,
        ctx: eframe::egui::Context,
    ) {
        std::thread::spawn(move || {
            let total_clips = clips.len();
            let mut total_bytes = 0u64;

            // Pre-calculate total bytes across all files (including spans)
            for clip in &clips {
                total_bytes += clip.total_size_bytes;
            }

            let mut clips_copied = 0;
            let mut files_copied = 0;
            let mut bytes_copied_overall = 0u64;
            let mut failed = 0;

            let db = Database::open(&db_path).ok();
            let start_time = Instant::now();
            let mut last_speed_check = Instant::now();
            let mut bytes_since_check = 0u64;
            let mut current_speed = 0.0f32;

            for (clip_idx, clip) in clips.into_iter().enumerate() {
                if cancel_flag.load(Ordering::Relaxed) {
                    let _ = tx.send(IngestMessage::Cancelled);
                    ctx.request_repaint();
                    return;
                }

                // Determine target directory for this clip
                let target_dir = match options.organize {
                    OrganizeOption::Direct => options.destination_dir.clone(),
                    OrganizeOption::DateFolder => {
                        let date_sub = clip
                            .date_time
                            .as_deref()
                            .and_then(|dt| dt.split_whitespace().next())
                            .map(|d| d.replace(':', "-").replace('/', "-"))
                            .unwrap_or_else(|| chrono::Local::now().format("%Y-%m-%d").to_string());
                        options.destination_dir.join(date_sub)
                    }
                    OrganizeOption::CustomFolder => {
                        let sub = options.custom_subfolder.trim();
                        if sub.is_empty() {
                            options.destination_dir.clone()
                        } else {
                            options.destination_dir.join(sub)
                        }
                    }
                };

                if let Err(e) = fs::create_dir_all(&target_dir) {
                    let _ = tx.send(IngestMessage::Error(format!(
                        "Failed to create target directory {}: {e}",
                        target_dir.display()
                    )));
                    ctx.request_repaint();
                    return;
                }

                let mut clip_had_error = false;
                let mut dest_main_path = target_dir.join(&clip.file_name);

                // Copy every chunk (main MLV + spanned .M00, .M01, etc.)
                for chunk_path in &clip.spanned_chunks {
                    if cancel_flag.load(Ordering::Relaxed) {
                        let _ = tx.send(IngestMessage::Cancelled);
                        ctx.request_repaint();
                        return;
                    }

                    let file_name = match chunk_path.file_name() {
                        Some(name) => name.to_string_lossy().to_string(),
                        None => continue,
                    };

                    let dest_path = target_dir.join(&file_name);
                    if chunk_path == &clip.main_path {
                        dest_main_path = dest_path.clone();
                    }

                    let chunk_size = fs::metadata(chunk_path).map(|m| m.len()).unwrap_or(0);

                    // Skip existing file if requested and matches size
                    if dest_path.exists() && options.skip_existing {
                        if let Ok(existing_meta) = fs::metadata(&dest_path) {
                            if existing_meta.len() == chunk_size {
                                bytes_copied_overall += chunk_size;
                                files_copied += 1;
                                continue;
                            }
                        }
                    }

                    // Perform streaming copy with progress and cancellation
                    match Self::copy_stream(
                        chunk_path,
                        &dest_path,
                        chunk_size,
                        options.verify_checksum,
                        &cancel_flag,
                        &tx,
                        &ctx,
                        clip_idx + 1,
                        total_clips,
                        &file_name,
                        bytes_copied_overall,
                        total_bytes,
                        &mut last_speed_check,
                        &mut bytes_since_check,
                        &mut current_speed,
                    ) {
                        Ok(copied) => {
                            bytes_copied_overall += copied;
                            files_copied += 1;
                        }
                        Err(e) => {
                            if cancel_flag.load(Ordering::Relaxed) {
                                let _ = fs::remove_file(&dest_path);
                                let _ = tx.send(IngestMessage::Cancelled);
                                ctx.request_repaint();
                                return;
                            }
                            clip_had_error = true;
                            failed += 1;
                            eprintln!("Error copying chunk {}: {e}", chunk_path.display());
                        }
                    }
                }

                if !clip_had_error {
                    clips_copied += 1;

                    // Automatically ingest newly copied clip into catalog if requested
                    if options.add_to_catalog {
                        if let Some(ref database) = db {
                            if let Ok(copied_clip) = MlvClip::open(&dest_main_path) {
                                let _ = database.upsert_video(&copied_clip.meta);
                            }
                        }
                    }

                    let _ = tx.send(IngestMessage::ClipComplete {
                        clip_id: clip.id,
                        file_name: clip.file_name.clone(),
                        dest_main_path,
                    });
                    ctx.request_repaint();
                }
            }

            let _ = start_time;
            let _ = tx.send(IngestMessage::Complete {
                clips_copied,
                files_copied,
                total_bytes: bytes_copied_overall,
                failed,
                checksums_verified: options.verify_checksum,
            });
            ctx.request_repaint();
        });
    }

    /// Stream a file using a 2 MB buffer, reporting progress, verifying size, headers, and checksums
    fn copy_stream(
        src: &Path,
        dest: &Path,
        file_total: u64,
        verify_checksum: bool,
        cancel_flag: &Arc<AtomicBool>,
        tx: &Sender<IngestMessage>,
        ctx: &eframe::egui::Context,
        current_clip: usize,
        total_clips: usize,
        current_file: &str,
        bytes_copied_before: u64,
        total_bytes: u64,
        last_speed_check: &mut Instant,
        bytes_since_check: &mut u64,
        current_speed: &mut f32,
    ) -> std::io::Result<u64> {
        let src_file = File::open(src)?;
        let mut reader = BufReader::with_capacity(2 * 1024 * 1024, src_file);

        let dest_file = File::create(dest)?;
        let mut writer = BufWriter::with_capacity(2 * 1024 * 1024, dest_file);

        let mut buf = vec![0u8; 1024 * 1024]; // 1MB chunk
        let mut file_copied = 0u64;
        let mut src_crc = 0xFFFFFFFFu32;

        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                drop(writer);
                let _ = fs::remove_file(dest);
                return Err(std::io::Error::new(std::io::ErrorKind::Interrupted, "Cancelled"));
            }

            let bytes_read = reader.read(&mut buf)?;
            if bytes_read == 0 {
                break;
            }

            if verify_checksum {
                src_crc = crc32_update(src_crc, &buf[..bytes_read]);
            }

            writer.write_all(&buf[..bytes_read])?;
            file_copied += bytes_read as u64;
            *bytes_since_check += bytes_read as u64;

            // Recalculate speed every 250ms
            let elapsed = last_speed_check.elapsed();
            if elapsed.as_millis() >= 250 {
                let secs = elapsed.as_secs_f32();
                if secs > 0.0 {
                    *current_speed = (*bytes_since_check as f32 / (1024.0 * 1024.0)) / secs;
                }
                *last_speed_check = Instant::now();
                *bytes_since_check = 0;

                let _ = tx.send(IngestMessage::Progress {
                    current_clip,
                    total_clips,
                    current_file: current_file.to_string(),
                    stage: IngestStage::Copying,
                    file_bytes_copied: file_copied,
                    file_total_bytes: file_total,
                    total_bytes_copied: bytes_copied_before + file_copied,
                    total_bytes,
                    speed_mbps: *current_speed,
                });
                ctx.request_repaint();
            }
        }

        writer.flush()?;
        drop(writer);

        // 1. Post-copy Exact Size Verification
        let dest_meta = fs::metadata(dest)?;
        if dest_meta.len() != file_total {
            let _ = fs::remove_file(dest);
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Integrity check failed: file size mismatch (source: {} bytes, destination: {} bytes)",
                    file_total,
                    dest_meta.len()
                ),
            ));
        }

        // 2. MLVI Container Header Integrity Verification
        if file_total >= 4 {
            let mut magic = [0u8; 4];
            let mut df = File::open(dest)?;
            df.read_exact(&mut magic)?;
            if &magic != b"MLVI" {
                let _ = fs::remove_file(dest);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Integrity check failed: MLVI magic header missing or corrupted",
                ));
            }
        }

        // 3. Bit-for-bit Checksum Verification (if enabled)
        if verify_checksum {
            let final_src_crc = crc32_finish(src_crc);

            let _ = tx.send(IngestMessage::Progress {
                current_clip,
                total_clips,
                current_file: current_file.to_string(),
                stage: IngestStage::VerifyingChecksum,
                file_bytes_copied: file_total,
                file_total_bytes: file_total,
                total_bytes_copied: bytes_copied_before + file_copied,
                total_bytes,
                speed_mbps: *current_speed,
            });
            ctx.request_repaint();

            let dest_crc = compute_file_crc32(dest, cancel_flag)?;
            if dest_crc != final_src_crc {
                let _ = fs::remove_file(dest);
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Checksum verification mismatch: source CRC32 (0x{:08X}) != destination CRC32 (0x{:08X})",
                        final_src_crc, dest_crc
                    ),
                ));
            }
        }

        Ok(file_copied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc32_algorithm() {
        // Standard check vector: CRC-32 of "123456789" is 0xCBF43926
        let crc = crc32_finish(crc32_update(0xFFFFFFFF, b"123456789"));
        assert_eq!(crc, 0xCBF43926);
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024 * 2), "2.00 GB");
    }

    #[test]
    fn test_detect_available_drives() {
        let drives = detect_available_drives();
        // Should return a valid list (on Windows, at least C:\ exists)
        #[cfg(target_os = "windows")]
        assert!(!drives.is_empty());
    }

    #[test]
    fn test_scan_and_ingest_copy() {
        let temp_base = std::env::temp_dir().join(format!(
            "mlv_ingest_test_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let source_dir = temp_base.join("sd_card");
        let dest_dir = temp_base.join("ingested_footage");
        fs::create_dir_all(&source_dir).unwrap();
        fs::create_dir_all(&dest_dir).unwrap();

        let db_path = temp_base.join("test.db");
        let db = Database::open(&db_path).unwrap();

        // Create a dummy .MLV and a companion .M00 file
        let main_clip = source_dir.join("CLIP0001.MLV");
        let span_clip = source_dir.join("CLIP0001.M00");
        fs::write(&main_clip, b"MLVI dummy data for clip 1").unwrap();
        fs::write(&span_clip, b"MLVI span data 00").unwrap();

        let cancel = Arc::new(AtomicBool::new(false));
        let clips = scan_source(&source_dir, &db, &cancel);

        assert_eq!(clips.len(), 1);
        assert_eq!(clips[0].file_name, "CLIP0001.MLV");
        assert_eq!(clips[0].spanned_chunks.len(), 2);
        assert!(clips[0].selected);

        // Perform ingest copy into dest_dir
        let (tx, rx) = crossbeam_channel::unbounded();
        let options = IngestOptions {
            destination_dir: dest_dir.clone(),
            organize: OrganizeOption::Direct,
            custom_subfolder: String::new(),
            add_to_catalog: false,
            skip_existing: false,
            verify_checksum: true,
        };

        Ingester::start(
            clips,
            options,
            db_path,
            tx,
            cancel,
            eframe::egui::Context::default(),
        );

        // Wait for complete message
        let msg = rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .expect("Should receive progress or complete message");

        // Keep receiving until complete
        let mut completed = false;
        let mut msg_opt = Some(msg);
        while let Some(m) = msg_opt {
            if let IngestMessage::Complete { clips_copied, files_copied, .. } = m {
                assert_eq!(clips_copied, 1);
                assert_eq!(files_copied, 2);
                completed = true;
                break;
            }
            msg_opt = rx.recv_timeout(std::time::Duration::from_secs(2)).ok();
        }
        assert!(completed, "Ingestion should successfully complete");

        // Verify destination files exist
        assert!(dest_dir.join("CLIP0001.MLV").exists());
        assert!(dest_dir.join("CLIP0001.M00").exists());

        let _ = fs::remove_dir_all(&temp_base);
    }
}

