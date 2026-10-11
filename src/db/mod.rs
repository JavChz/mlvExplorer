pub mod models;

pub use models::*;
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: i64 = 3;

pub struct Database {
    conn: Connection,
    pub path: PathBuf,
    /// Set when an older catalog was migrated and metadata must be re-read.
    pub needs_rescan: bool,
}

/// Default catalog location: %APPDATA%/MLV Explorer/catalog.db (or ~/.local/share/...)
pub fn default_catalog_path() -> PathBuf {
    let base = std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("XDG_DATA_HOME").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
        .unwrap_or_else(|| PathBuf::from("."));
    let dir = base.join("MLV Explorer");
    let _ = std::fs::create_dir_all(&dir);
    dir.join("catalog.db")
}

pub fn open_connection(path: &Path) -> Result<Connection> {
    let conn = Connection::open(path)?;
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    Ok(conn)
}

impl Database {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        let conn = open_connection(&path)?;
        let mut db = Self {
            conn,
            path,
            needs_rescan: false,
        };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&mut self) -> Result<()> {
        self.conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS videos (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                file_path TEXT NOT NULL UNIQUE,
                file_name TEXT NOT NULL,
                file_size INTEGER NOT NULL DEFAULT 0,
                width INTEGER NOT NULL DEFAULT 0,
                height INTEGER NOT NULL DEFAULT 0,
                fps REAL NOT NULL DEFAULT 0,
                frame_count INTEGER NOT NULL DEFAULT 0,
                duration_seconds REAL NOT NULL DEFAULT 0,
                bits_per_pixel INTEGER NOT NULL DEFAULT 0,
                camera_name TEXT NOT NULL DEFAULT '',
                camera_serial TEXT NOT NULL DEFAULT '',
                lens_name TEXT NOT NULL DEFAULT '',
                focal_length_mm INTEGER NOT NULL DEFAULT 0,
                aperture_fstop REAL NOT NULL DEFAULT 0,
                iso INTEGER NOT NULL DEFAULT 0,
                shutter_speed INTEGER NOT NULL DEFAULT 0,
                white_balance_kelvin INTEGER NOT NULL DEFAULT 0,
                date_recorded TEXT,
                imported_at TEXT NOT NULL DEFAULT '',
                rating INTEGER NOT NULL DEFAULT 0,
                flag INTEGER NOT NULL DEFAULT 0,
                notes TEXT NOT NULL DEFAULT '',
                color_label TEXT NOT NULL DEFAULT ''
            );
            CREATE TABLE IF NOT EXISTS tags (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE,
                color TEXT NOT NULL DEFAULT '#5B8DEF'
            );
            CREATE TABLE IF NOT EXISTS video_tags (
                video_id INTEGER NOT NULL,
                tag_id INTEGER NOT NULL,
                PRIMARY KEY (video_id, tag_id)
            );
            CREATE TABLE IF NOT EXISTS thumbs (
                video_id INTEGER PRIMARY KEY,
                jpeg BLOB NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_videos_camera ON videos(camera_name);
            CREATE INDEX IF NOT EXISTS idx_videos_lens ON videos(lens_name);
            CREATE INDEX IF NOT EXISTS idx_videos_rating ON videos(rating);
            CREATE INDEX IF NOT EXISTS idx_videos_flag ON videos(flag);
            CREATE INDEX IF NOT EXISTS idx_videos_date ON videos(date_recorded);
            "#,
        )?;

        // Additive migrations (ignore "duplicate column" errors)
        for sql in [
            "ALTER TABLE videos ADD COLUMN codec TEXT NOT NULL DEFAULT ''",
            "ALTER TABLE videos ADD COLUMN has_audio INTEGER NOT NULL DEFAULT 0",
            "ALTER TABLE videos ADD COLUMN chunk_count INTEGER NOT NULL DEFAULT 1",
            "ALTER TABLE videos ADD COLUMN stretch_factor REAL NOT NULL DEFAULT 1.0",
        ] {
            let _ = self.conn.execute(sql, []);
        }

        let version: i64 = self.conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
        if version < SCHEMA_VERSION {
            // v1 parsed metadata incorrectly: force re-read + regenerate thumbnails
            let has_rows: i64 = self.conn.query_row("SELECT COUNT(*) FROM videos", [], |r| r.get(0))?;
            self.needs_rescan = has_rows > 0;
            self.conn.execute("DELETE FROM thumbs", [])?;
            self.conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
        }
        Ok(())
    }

    /// Insert or update clip metadata; preserves rating/flags/tags. Returns row id.
    pub fn upsert_video(&self, meta: &crate::mlv::MlvMetadata) -> Result<i64> {
        let file_name = Path::new(&meta.file_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| meta.file_path.clone());
        let imported_at = chrono::Local::now().to_rfc3339();
        let stretch = meta.default_stretch();

        self.conn.query_row(
            r#"
            INSERT INTO videos (
                file_path, file_name, file_size, width, height, fps,
                frame_count, duration_seconds, bits_per_pixel, camera_name,
                camera_serial, lens_name, focal_length_mm, aperture_fstop,
                iso, shutter_speed, white_balance_kelvin, date_recorded,
                imported_at, codec, has_audio, chunk_count, stretch_factor
            ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)
            ON CONFLICT(file_path) DO UPDATE SET
                file_name = excluded.file_name,
                file_size = excluded.file_size,
                width = excluded.width,
                height = excluded.height,
                fps = excluded.fps,
                frame_count = excluded.frame_count,
                duration_seconds = excluded.duration_seconds,
                bits_per_pixel = excluded.bits_per_pixel,
                camera_name = excluded.camera_name,
                camera_serial = excluded.camera_serial,
                lens_name = excluded.lens_name,
                focal_length_mm = excluded.focal_length_mm,
                aperture_fstop = excluded.aperture_fstop,
                iso = excluded.iso,
                shutter_speed = excluded.shutter_speed,
                white_balance_kelvin = excluded.white_balance_kelvin,
                date_recorded = excluded.date_recorded,
                codec = excluded.codec,
                has_audio = excluded.has_audio,
                chunk_count = excluded.chunk_count,
                stretch_factor = CASE WHEN videos.stretch_factor = 1.0 AND excluded.stretch_factor != 1.0 THEN excluded.stretch_factor ELSE videos.stretch_factor END
            RETURNING id
            "#,
            params![
                meta.file_path,
                file_name,
                meta.file_size as i64,
                meta.width,
                meta.height,
                meta.fps(),
                meta.frame_count,
                meta.duration_seconds(),
                meta.bits_per_pixel,
                if meta.camera_name.is_empty() { "Unknown Camera" } else { &meta.camera_name },
                meta.camera_serial,
                if meta.lens_name.is_empty() { "Unknown Lens" } else { &meta.lens_name },
                meta.focal_length_mm,
                meta.aperture_fstop,
                meta.iso,
                meta.shutter_speed_microseconds,
                meta.white_balance_kelvin,
                meta.date_recorded,
                imported_at,
                meta.codec_name(),
                meta.has_audio,
                meta.chunk_count,
                stretch,
            ],
            |r| r.get(0),
        )
    }

    pub fn get_videos(&self, filter: &FilterCriteria) -> Result<Vec<VideoRecord>> {
        let mut query = String::from(
            r#"
            SELECT v.id, v.file_path, v.file_name, v.file_size, v.width, v.height,
                   v.fps, v.frame_count, v.duration_seconds, v.bits_per_pixel,
                   v.camera_name, v.camera_serial, v.lens_name, v.focal_length_mm,
                   v.aperture_fstop, v.iso, v.shutter_speed, v.white_balance_kelvin,
                   v.date_recorded, v.imported_at, v.rating, v.flag, v.notes, v.color_label,
                   v.codec, v.has_audio, v.chunk_count, v.stretch_factor,
                   (SELECT GROUP_CONCAT(t.name, '\u001f') FROM video_tags vt JOIN tags t ON t.id = vt.tag_id WHERE vt.video_id = v.id) AS tags
            FROM videos v
            WHERE 1=1
            "#,
        );
        let mut p: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        let search = filter.search_text.trim();
        if !search.is_empty() {
            query.push_str(
                " AND (v.file_name LIKE ?1 OR v.camera_name LIKE ?1 OR v.lens_name LIKE ?1 OR v.notes LIKE ?1 \
                 OR v.id IN (SELECT vt.video_id FROM video_tags vt JOIN tags t ON t.id = vt.tag_id WHERE t.name LIKE ?1))",
            );
            p.push(Box::new(format!("%{search}%")));
        }
        let mut idx = p.len();
        let mut push = |q: &mut String, clause: &str, v: Box<dyn rusqlite::ToSql>, p: &mut Vec<Box<dyn rusqlite::ToSql>>| {
            idx += 1;
            q.push_str(&clause.replace('?', &format!("?{idx}")));
            p.push(v);
        };
        if let Some(cam) = &filter.camera {
            push(&mut query, " AND v.camera_name = ?", Box::new(cam.clone()), &mut p);
        }
        if let Some(lens) = &filter.lens {
            push(&mut query, " AND v.lens_name = ?", Box::new(lens.clone()), &mut p);
        }
        if let Some(r) = filter.min_rating {
            push(&mut query, " AND v.rating >= ?", Box::new(r as i64), &mut p);
        }
        if let Some(f) = filter.flag {
            push(&mut query, " AND v.flag = ?", Box::new(f as i64), &mut p);
        }
        if let Some(tag) = &filter.tag {
            push(
                &mut query,
                " AND v.id IN (SELECT vt.video_id FROM video_tags vt JOIN tags t ON t.id = vt.tag_id WHERE t.name = ?)",
                Box::new(tag.clone()),
                &mut p,
            );
        }
        query.push_str(" ORDER BY ");
        query.push_str(filter.sort.sql());

        let mut stmt = self.conn.prepare(&query)?;
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), |row| {
            let tags: Option<String> = row.get(28)?;
            Ok(VideoRecord {
                id: row.get(0)?,
                file_path: row.get(1)?,
                file_name: row.get(2)?,
                file_size: row.get::<_, i64>(3)? as u64,
                width: row.get(4)?,
                height: row.get(5)?,
                fps: row.get(6)?,
                frame_count: row.get(7)?,
                duration_seconds: row.get(8)?,
                bits_per_pixel: row.get(9)?,
                camera_name: row.get(10)?,
                camera_serial: row.get(11)?,
                lens_name: row.get(12)?,
                focal_length_mm: row.get(13)?,
                aperture_fstop: row.get(14)?,
                iso: row.get(15)?,
                shutter_speed: row.get(16)?,
                white_balance_kelvin: row.get(17)?,
                date_recorded: row.get(18)?,
                imported_at: row.get(19)?,
                rating: row.get(20)?,
                flag: PickFlag::from(row.get::<_, i64>(21)?),
                notes: row.get(22)?,
                color_label: row.get(23)?,
                codec: row.get(24)?,
                has_audio: row.get::<_, i64>(25)? != 0,
                chunk_count: row.get(26)?,
                stretch_factor: row.get::<_, Option<f64>>(27)?.map(|v| v as f32).unwrap_or(1.0),
                tags: tags
                    .map(|s| s.split('\u{1f}').map(|t| t.to_string()).collect())
                    .unwrap_or_default(),
            })
        })?;
        rows.collect()
    }

    pub fn set_stretch_factor(&self, video_id: i64, stretch: f32) -> Result<()> {
        self.conn.execute("UPDATE videos SET stretch_factor = ?1 WHERE id = ?2", params![stretch as f64, video_id])?;
        Ok(())
    }

    pub fn all_paths(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT file_path FROM videos")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }

    pub fn facets(&self) -> Result<Facets> {
        let mut f = Facets::default();
        self.conn.query_row(
            "SELECT COUNT(*), SUM(flag=1), SUM(flag=2), SUM(flag=0) FROM videos",
            [],
            |r| {
                f.total = r.get::<_, i64>(0)? as usize;
                f.picked = r.get::<_, Option<i64>>(1)?.unwrap_or(0) as usize;
                f.rejected = r.get::<_, Option<i64>>(2)?.unwrap_or(0) as usize;
                f.unflagged = r.get::<_, Option<i64>>(3)?.unwrap_or(0) as usize;
                Ok(())
            },
        )?;
        let facet = |sql: &str| -> Result<Vec<Facet>> {
            let mut stmt = self.conn.prepare(sql)?;
            let rows = stmt.query_map([], |r| {
                Ok(Facet {
                    name: r.get(0)?,
                    count: r.get::<_, i64>(1)? as usize,
                })
            })?;
            rows.collect()
        };
        f.cameras = facet("SELECT camera_name, COUNT(*) FROM videos GROUP BY camera_name ORDER BY camera_name")?;
        f.lenses = facet("SELECT lens_name, COUNT(*) FROM videos GROUP BY lens_name ORDER BY lens_name")?;
        f.tags = facet(
            "SELECT t.name, COUNT(vt.video_id) FROM tags t JOIN video_tags vt ON vt.tag_id = t.id GROUP BY t.id ORDER BY t.name COLLATE NOCASE",
        )?;
        Ok(f)
    }

    pub fn set_rating(&self, video_id: i64, rating: u8) -> Result<()> {
        self.conn.execute("UPDATE videos SET rating = ?1 WHERE id = ?2", params![rating as i64, video_id])?;
        Ok(())
    }

    pub fn set_flag(&self, video_id: i64, flag: PickFlag) -> Result<()> {
        self.conn.execute("UPDATE videos SET flag = ?1 WHERE id = ?2", params![flag as i64, video_id])?;
        Ok(())
    }

    pub fn set_notes(&self, video_id: i64, notes: &str) -> Result<()> {
        self.conn.execute("UPDATE videos SET notes = ?1 WHERE id = ?2", params![notes, video_id])?;
        Ok(())
    }

    pub fn add_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        let tag_name = tag_name.trim();
        if tag_name.is_empty() {
            return Ok(());
        }
        self.conn.execute("INSERT OR IGNORE INTO tags (name) VALUES (?1)", params![tag_name])?;
        let tag_id: i64 = self.conn.query_row("SELECT id FROM tags WHERE name = ?1", params![tag_name], |r| r.get(0))?;
        self.conn.execute(
            "INSERT OR IGNORE INTO video_tags (video_id, tag_id) VALUES (?1, ?2)",
            params![video_id, tag_id],
        )?;
        Ok(())
    }

    pub fn remove_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM video_tags WHERE video_id = ?1 AND tag_id = (SELECT id FROM tags WHERE name = ?2)",
            params![video_id, tag_name],
        )?;
        self.conn.execute(
            "DELETE FROM tags WHERE id NOT IN (SELECT DISTINCT tag_id FROM video_tags)",
            [],
        )?;
        Ok(())
    }

    pub fn all_tag_names(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare("SELECT name FROM tags ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| r.get(0))?;
        rows.collect()
    }

    pub fn remove_video(&self, video_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM video_tags WHERE video_id = ?1", params![video_id])?;
        self.conn.execute("DELETE FROM thumbs WHERE video_id = ?1", params![video_id])?;
        self.conn.execute("DELETE FROM videos WHERE id = ?1", params![video_id])?;
        Ok(())
    }

    pub fn get_all_tags(&self) -> Result<Vec<TagRecord>> {
        let mut stmt = self.conn.prepare("SELECT id, name, color FROM tags ORDER BY name")?;
        let rows = stmt.query_map([], |r| {
            Ok(TagRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
            })
        })?;
        rows.collect()
    }

    pub fn backup_database_to_file<P: AsRef<Path>>(&self, dest_path: P) -> Result<()> {
        let mut dest = Connection::open(dest_path)?;
        let backup = rusqlite::backup::Backup::new(&self.conn, &mut dest)?;
        backup.run_to_completion(64, std::time::Duration::from_millis(10), None)?;
        Ok(())
    }

    pub fn export_catalog_json<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let videos = self.get_videos(&FilterCriteria::default()).map_err(|e| e.to_string())?;
        let tags = self.get_all_tags().map_err(|e| e.to_string())?;
        let backup = CatalogBackup {
            exported_at: chrono::Local::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            videos,
            tags,
        };
        let json = serde_json::to_string_pretty(&backup).map_err(|e| e.to_string())?;
        std::fs::write(path, json).map_err(|e| e.to_string())
    }

    /// Merge a JSON catalog. Organization data (rating/flag/notes/tags) is applied by file path.
    pub fn import_catalog_json<P: AsRef<Path>>(&self, path: P) -> Result<usize, String> {
        let content = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let backup: CatalogBackup = serde_json::from_str(&content).map_err(|e| e.to_string())?;
        let mut count = 0;
        for vid in &backup.videos {
            let existing: Option<i64> = self
                .conn
                .query_row("SELECT id FROM videos WHERE file_path = ?1", params![vid.file_path], |r| r.get(0))
                .optional()
                .map_err(|e| e.to_string())?;
            let row_id = match existing {
                Some(id) => id,
                None => {
                    let meta = crate::mlv::MlvMetadata {
                        file_path: vid.file_path.clone(),
                        file_size: vid.file_size,
                        width: vid.width,
                        height: vid.height,
                        fps_num: (vid.fps * 1000.0) as u32,
                        fps_den: 1000,
                        frame_count: vid.frame_count,
                        bits_per_pixel: vid.bits_per_pixel,
                        camera_name: vid.camera_name.clone(),
                        camera_serial: vid.camera_serial.clone(),
                        lens_name: vid.lens_name.clone(),
                        focal_length_mm: vid.focal_length_mm,
                        aperture_fstop: vid.aperture_fstop,
                        iso: vid.iso,
                        shutter_speed_microseconds: vid.shutter_speed,
                        white_balance_kelvin: vid.white_balance_kelvin,
                        date_recorded: vid.date_recorded.clone(),
                        chunk_count: vid.chunk_count.max(1),
                        has_audio: vid.has_audio,
                        ..Default::default()
                    };
                    self.upsert_video(&meta).map_err(|e| e.to_string())?
                }
            };
            let _ = self.set_rating(row_id, vid.rating);
            let _ = self.set_flag(row_id, vid.flag);
            let _ = self.set_notes(row_id, &vid.notes);
            if vid.stretch_factor > 0.05 {
                let _ = self.set_stretch_factor(row_id, vid.stretch_factor);
            }
            for t in &vid.tags {
                let _ = self.add_tag(row_id, t);
            }
            count += 1;
        }
        Ok(count)
    }
}

// ---- thumbnail storage (used from worker threads with their own connection) ----

pub fn load_thumb(conn: &Connection, video_id: i64) -> Option<Vec<u8>> {
    conn.query_row("SELECT jpeg FROM thumbs WHERE video_id = ?1", params![video_id], |r| r.get(0))
        .optional()
        .ok()
        .flatten()
}

pub fn store_thumb(conn: &Connection, video_id: i64, jpeg: &[u8]) {
    let _ = conn.execute(
        "INSERT OR REPLACE INTO thumbs (video_id, jpeg) VALUES (?1, ?2)",
        params![video_id, jpeg],
    );
}

pub fn delete_thumb(conn: &Connection, video_id: i64) {
    let _ = conn.execute("DELETE FROM thumbs WHERE video_id = ?1", params![video_id]);
}
