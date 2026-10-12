use crate::db::models::{
    CatalogBackup, Facet, Facets, FilterCriteria, PickFlag, TagNode, TagRecord, VideoRecord,
};
use rusqlite::{params, Connection, OptionalExtension, Result};
use std::path::{Path, PathBuf};

/// High-level query and mutation operations on the MLV database
pub struct Queries<'a> {
    pub conn: &'a Connection,
}

impl<'a> Queries<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
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
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18,
                ?19, ?20, ?21, ?22, ?23
            )
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

    /// Check if a file path (or canonicalized equivalent) is already in the database
    pub fn is_file_imported<P: AsRef<Path>>(&self, path: P) -> bool {
        let path_ref = path.as_ref();
        let p_str = path_ref.to_string_lossy().to_string();
        let norm = p_str.replace('\\', "/").to_lowercase();
        let count: i64 = self
            .conn
            .query_row(
                "SELECT COUNT(*) FROM videos WHERE file_path = ?1 COLLATE NOCASE OR replace(file_path, '\\', '/') = ?2 COLLATE NOCASE",
                params![p_str, norm],
                |r| r.get(0),
            )
            .unwrap_or(0);
        if count > 0 {
            return true;
        }
        if let Ok(canon) = std::fs::canonicalize(path_ref) {
            let c_str = canon.to_string_lossy().to_string();
            let c_norm = c_str.replace('\\', "/").to_lowercase();
            let c_count: i64 = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM videos WHERE file_path = ?1 COLLATE NOCASE OR replace(file_path, '\\', '/') = ?2 COLLATE NOCASE",
                    params![c_str, c_norm],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if c_count > 0 {
                return true;
            }
        }
        false
    }

    /// Check if a clip with the exact same content signature already exists in the catalog
    pub fn is_duplicate_clip(&self, meta: &crate::mlv::MlvMetadata) -> bool {
        if self.is_file_imported(&meta.file_path) {
            return true;
        }
        if meta.file_size > 0 && meta.frame_count > 0 {
            let date = meta.date_recorded.as_deref().unwrap_or("");
            let count: i64 = self
                .conn
                .query_row(
                    "SELECT COUNT(*) FROM videos WHERE file_size = ?1 AND frame_count = ?2 AND camera_serial = ?3 AND ifnull(date_recorded, '') = ?4",
                    params![meta.file_size as i64, meta.frame_count, meta.camera_serial, date],
                    |r| r.get(0),
                )
                .unwrap_or(0);
            if count > 0 {
                return true;
            }
        }
        false
    }

    /// Insert a new video clip, returning Ok(None) if it already exists (duplicate prevention)
    pub fn insert_new_video(&self, meta: &crate::mlv::MlvMetadata) -> Result<Option<i64>> {
        if self.is_duplicate_clip(meta) {
            return Ok(None);
        }
        self.upsert_video(meta).map(Some)
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
                   v.scene, v.shot, v.take, v.location, v.camera_unit,
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
                 OR v.scene LIKE ?1 OR v.location LIKE ?1 \
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
                " AND v.id IN (
                    WITH RECURSIVE tag_family(id) AS (
                        SELECT id FROM tags WHERE name = ?
                        UNION ALL
                        SELECT t.id FROM tags t JOIN tag_family tf ON t.parent_id = tf.id
                    )
                    SELECT vt.video_id FROM video_tags vt WHERE vt.tag_id IN (SELECT id FROM tag_family)
                )",
                Box::new(tag.clone()),
                &mut p,
            );
        }
        query.push_str(" ORDER BY ");
        query.push_str(filter.sort.sql());

        let mut stmt = self.conn.prepare(&query)?;
        let refs: Vec<&dyn rusqlite::ToSql> = p.iter().map(|b| b.as_ref()).collect();
        let rows = stmt.query_map(refs.as_slice(), |row| {
            let tags: Option<String> = row.get(33)?;
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
                scene: row.get(28)?,
                shot: row.get(29)?,
                take: row.get(30)?,
                location: row.get(31)?,
                camera_unit: row.get(32)?,
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
        f.tag_tree = self.get_tag_tree().unwrap_or_default();
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

    pub fn set_production_slate(
        &self,
        video_id: i64,
        scene: Option<&str>,
        shot: Option<&str>,
        take: Option<&str>,
        location: Option<&str>,
        camera_unit: Option<&str>,
    ) -> Result<()> {
        self.conn.execute(
            "UPDATE videos SET scene = ?1, shot = ?2, take = ?3, location = ?4, camera_unit = ?5 WHERE id = ?6",
            params![scene, shot, take, location, camera_unit, video_id],
        )?;
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

    pub fn create_tag(&self, name: &str, parent_id: Option<i64>) -> Result<i64> {
        let name = name.trim();
        if name.is_empty() {
            return Err(rusqlite::Error::InvalidQuery);
        }
        self.conn.execute(
            "INSERT OR IGNORE INTO tags (name, parent_id) VALUES (?1, ?2)",
            params![name, parent_id],
        )?;
        let tag_id: i64 = self.conn.query_row("SELECT id FROM tags WHERE name = ?1", params![name], |r| r.get(0))?;
        if parent_id.is_some() {
            self.conn.execute(
                "UPDATE tags SET parent_id = ?1 WHERE id = ?2",
                params![parent_id, tag_id],
            )?;
        }
        Ok(tag_id)
    }

    pub fn set_tag_parent(&self, tag_id: i64, parent_id: Option<i64>) -> Result<()> {
        if let Some(pid) = parent_id {
            if pid == tag_id {
                return Ok(());
            }
            // Check if pid is already a descendant of tag_id (would create a cycle)
            let mut curr = Some(pid);
            while let Some(c) = curr {
                if c == tag_id {
                    return Ok(()); // Cycle detected, refuse to set
                }
                curr = self.conn.query_row(
                    "SELECT parent_id FROM tags WHERE id = ?1",
                    params![c],
                    |r| r.get::<_, Option<i64>>(0),
                ).optional()?.flatten();
            }
        }
        self.conn.execute(
            "UPDATE tags SET parent_id = ?1 WHERE id = ?2",
            params![parent_id, tag_id],
        )?;
        Ok(())
    }

    pub fn rename_tag(&self, old_name: &str, new_name: &str) -> Result<()> {
        let clean = new_name.trim();
        if clean.is_empty() {
            return Ok(());
        }
        self.conn.execute(
            "UPDATE tags SET name = ?1 WHERE name = ?2",
            params![clean, old_name],
        )?;
        Ok(())
    }

    pub fn remove_tag(&self, video_id: i64, tag_name: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM video_tags WHERE video_id = ?1 AND tag_id = (SELECT id FROM tags WHERE name = ?2)",
            params![video_id, tag_name],
        )?;
        Ok(())
    }

    pub fn delete_keyword(&self, tag_name: &str) -> Result<()> {
        let tag_id: Option<i64> = self.conn.query_row(
            "SELECT id FROM tags WHERE name = ?1",
            params![tag_name],
            |r| r.get(0),
        ).optional()?;

        if let Some(id) = tag_id {
            // Update children to point to null or re-parent
            self.conn.execute("UPDATE tags SET parent_id = NULL WHERE parent_id = ?1", params![id])?;
            self.conn.execute("DELETE FROM video_tags WHERE tag_id = ?1", params![id])?;
            self.conn.execute("DELETE FROM tags WHERE id = ?1", params![id])?;
        }
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
        let mut stmt = self.conn.prepare("SELECT id, name, color, parent_id FROM tags ORDER BY name COLLATE NOCASE")?;
        let rows = stmt.query_map([], |r| {
            Ok(TagRecord {
                id: r.get(0)?,
                name: r.get(1)?,
                color: r.get(2)?,
                parent_id: r.get(3)?,
            })
        })?;
        rows.collect()
    }

    /// Builds a hierarchical keyword tree with direct counts and aggregated family counts
    pub fn get_tag_tree(&self) -> Result<Vec<TagNode>> {
        let tags = self.get_all_tags()?;
        
        // Fetch direct counts for each tag
        let mut direct_counts: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();
        {
            let mut stmt = self.conn.prepare(
                "SELECT tag_id, COUNT(video_id) FROM video_tags GROUP BY tag_id"
            )?;
            let mut rows = stmt.query([])?;
            while let Some(row) = rows.next()? {
                let tag_id: i64 = row.get(0)?;
                let count: i64 = row.get(1)?;
                direct_counts.insert(tag_id, count as usize);
            }
        }

        // Build adjacency list parent -> children
        let mut children_map: std::collections::HashMap<Option<i64>, Vec<TagRecord>> = std::collections::HashMap::new();
        for t in tags {
            children_map.entry(t.parent_id).or_default().push(t);
        }

        // Recursive helper to build tree and calculate total counts
        fn build_subtree(
            parent_id: Option<i64>,
            children_map: &std::collections::HashMap<Option<i64>, Vec<TagRecord>>,
            direct_counts: &std::collections::HashMap<i64, usize>,
        ) -> Vec<TagNode> {
            let mut nodes = Vec::new();
            if let Some(list) = children_map.get(&parent_id) {
                for item in list {
                    let direct = direct_counts.get(&item.id).copied().unwrap_or(0);
                    let sub_children = build_subtree(Some(item.id), children_map, direct_counts);
                    let sub_sum: usize = sub_children.iter().map(|c| c.total_count).sum();
                    let total = direct + sub_sum;
                    nodes.push(TagNode {
                        id: item.id,
                        name: item.name.clone(),
                        parent_id: item.parent_id,
                        direct_count: direct,
                        total_count: total,
                        children: sub_children,
                    });
                }
            }
            nodes
        }

        Ok(build_subtree(None, &children_map, &direct_counts))
    }

    pub fn backup_database_to_file<P: AsRef<Path>>(&self, dest_path: P) -> Result<()> {
        let mut dest = Connection::open(dest_path)?;
        let backup = rusqlite::backup::Backup::new(self.conn, &mut dest)?;
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
            let _ = self.set_production_slate(
                row_id,
                vid.scene.as_deref(),
                vid.shot.as_deref(),
                vid.take.as_deref(),
                vid.location.as_deref(),
                vid.camera_unit.as_deref(),
            );
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

    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Remove multiple videos by ID from catalog
    pub fn remove_videos(&self, video_ids: &[i64]) -> Result<usize> {
        if video_ids.is_empty() {
            return Ok(0);
        }
        let mut count = 0;
        for &id in video_ids {
            self.remove_video(id)?;
            count += 1;
        }
        Ok(count)
    }

    /// Creates an atomic snapshot backup of the SQLite database in the specified folder with a timestamped filename.
    pub fn backup_to_folder<P: AsRef<Path>>(&self, folder: P) -> Result<PathBuf, String> {
        let folder = folder.as_ref();
        if !folder.is_dir() {
            std::fs::create_dir_all(folder).map_err(|e| e.to_string())?;
        }
        let now = chrono::Local::now();
        let timestamp = now.format("%Y%m%d_%H%M%S");
        let backup_filename = format!("mlv_catalog_backup_{}.db", timestamp);
        let dest_path = folder.join(backup_filename);

        self.backup_database_to_file(&dest_path).map_err(|e| e.to_string())?;
        Ok(dest_path)
    }

    /// Merges an external SQLite catalog database into the current catalog.
    pub fn import_from_catalog_db<P: AsRef<Path>>(&self, source_path: P) -> Result<usize, String> {
        let source_path = source_path.as_ref();
        let src_conn = Connection::open(source_path).map_err(|e| format!("Cannot open source catalog: {e}"))?;
        
        let src_queries = Queries::new(&src_conn);
        let src_videos = src_queries.get_videos(&FilterCriteria::default())
            .map_err(|e| format!("Failed to read source catalog videos: {e}"))?;
        
        let mut count = 0;
        for vid in &src_videos {
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
            let _ = self.set_production_slate(
                row_id,
                vid.scene.as_deref(),
                vid.shot.as_deref(),
                vid.take.as_deref(),
                vid.location.as_deref(),
                vid.camera_unit.as_deref(),
            );
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
