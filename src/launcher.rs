use crate::db::Database;
use std::path::{Path, PathBuf};

/// Strips Windows extended-length prefix (`\\?\` or `\\?\UNC\`) from canonicalized paths,
/// which causes Qt applications like MLV App (using QDir/QFileInfo) to fail locating files or chunks.
pub fn clean_windows_path(p: &Path) -> PathBuf {
    let s = p.to_string_lossy();
    if let Some(stripped) = s.strip_prefix(r"\\?\UNC\") {
        PathBuf::from(format!(r"\\{}", stripped))
    } else if let Some(stripped) = s.strip_prefix(r"\\?\") {
        PathBuf::from(stripped)
    } else {
        p.to_path_buf()
    }
}

/// Resolves a file path to its primary `.MLV` file if given a spanned chunk (e.g. `.M00`, `.M01`).
pub fn resolve_primary_mlv_path(file_path: &Path) -> PathBuf {
    if let Some(ext) = file_path.extension() {
        let ext_str = ext.to_string_lossy();
        if ext_str.len() == 3
            && (ext_str.starts_with('M') || ext_str.starts_with('m'))
            && ext_str[1..].chars().all(|c| c.is_ascii_digit())
        {
            // It's a chunk extension like .M00, .m00, .M01
            let mlv_path = file_path.with_extension(if ext_str.starts_with('M') { "MLV" } else { "mlv" });
            if mlv_path.exists() {
                return mlv_path;
            }
        }
    }
    file_path.to_path_buf()
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

/// Handles discovering and launching external applications (e.g. MLV App)
pub struct ExternalLauncher;

impl ExternalLauncher {
    /// Generates an MLV App Session XML (`.masxml`) file containing all specified clips.
    /// This allows opening any number of clips (selections, keywords, or entire catalogs)
    /// in a single instance and session playlist of MLV App instead of launching separate windows.
    pub fn generate_masxml_session(file_paths: &[PathBuf]) -> Result<PathBuf, String> {
        let temp_dir = std::env::temp_dir();
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0);
        let session_file = temp_dir.join(format!("mlv_explorer_session_{}.masxml", timestamp));

        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<mlv_files version=\"4\" mlvapp=\"1.14\">\n");

        let mut count = 0;
        let mut seen = std::collections::HashSet::new();

        for p in file_paths {
            let primary = resolve_primary_mlv_path(p);
            let cleaned = clean_windows_path(&primary);
            let path_str = cleaned.to_string_lossy().to_string();
            let norm_key = path_str.to_lowercase();
            if seen.insert(norm_key) {
                // Use forward slashes for cross-platform Qt compatibility
                let qt_path = path_str.replace('\\', "/");
                let file_name = cleaned
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "clip.MLV".to_string());
                xml.push_str(&format!(
                    "  <clip file=\"{}\" relative=\"{}\" mark=\"0\"/>\n",
                    xml_escape(&qt_path),
                    xml_escape(&file_name)
                ));
                count += 1;
            }
        }

        xml.push_str("</mlv_files>\n");

        if count == 0 {
            return Err("No valid .MLV clips to include in MLV App session.".to_string());
        }

        std::fs::write(&session_file, xml.as_bytes())
            .map_err(|e| format!("Failed to create MLV App session file ({}): {}", session_file.display(), e))?;

        Ok(session_file)
    }

    /// Resolves the MLV App executable path, checking configured path, DB setting, PATH, and standard directories.
    pub fn get_or_locate_mlv_app(
        db: &Database,
        configured_path: Option<&PathBuf>,
    ) -> Option<PathBuf> {
        // 1. Check if configured path is provided, or user already saved one in database
        let mut exe_path: Option<PathBuf> = configured_path
            .cloned()
            .filter(|p| p.is_file())
            .or_else(|| {
                db.get_setting("mlv_app_path")
                    .ok()
                    .flatten()
                    .map(PathBuf::from)
                    .filter(|p| p.is_file())
            });

        // 2. If not saved or file was moved/deleted, search standard installation paths & PATH
        if exe_path.is_none() {
            exe_path = Self::detect_mlv_app();
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
                    let _ = db.set_setting("mlv_app_path", &picked.to_string_lossy());
                    exe_path = Some(picked);
                }
            }
        }

        exe_path
    }

    /// Launches MLV App with one or more clip file paths.
    /// If multiple clips are provided, creates an MLV App session (`.masxml`) so all clips
    /// load into a single instance and playlist session.
    pub fn open_files_in_mlv_app(
        db: &Database,
        configured_path: Option<&PathBuf>,
        file_paths: &[PathBuf],
    ) -> Result<String, String> {
        if file_paths.is_empty() {
            return Err("No clips selected to open in MLV App.".to_string());
        }

        let exe = match Self::get_or_locate_mlv_app(db, configured_path) {
            Some(e) => e,
            None => return Err("MLV App executable not configured or not found.".to_string()),
        };

        let clean_exe = clean_windows_path(&exe);
        let _ = db.set_setting("mlv_app_path", &clean_exe.to_string_lossy());

        let mut cmd = std::process::Command::new(&clean_exe);

        let launch_target = if file_paths.len() == 1 {
            let primary = resolve_primary_mlv_path(&file_paths[0]);
            let abs_path = match std::fs::canonicalize(&primary) {
                Ok(p) => clean_windows_path(&p),
                Err(_) => clean_windows_path(&primary),
            };
            if let Some(parent_dir) = abs_path.parent() {
                cmd.current_dir(parent_dir);
            }
            abs_path
        } else {
            let session_path = Self::generate_masxml_session(file_paths)?;
            if let Some(parent) = file_paths[0].parent() {
                cmd.current_dir(parent);
            }
            session_path
        };

        cmd.arg(&launch_target);

        match cmd.spawn() {
            Ok(_) => {
                if file_paths.len() == 1 {
                    Ok(format!("Launched MLV App with: {}", launch_target.display()))
                } else {
                    Ok(format!("Opened {} clip(s) in a single MLV App session", file_paths.len()))
                }
            }
            Err(e) => Err(format!("Failed to launch MLV App ({}): {}", clean_exe.display(), e)),
        }
    }

    /// Launches MLV App with the given single clip file path.
    pub fn open_in_mlv_app(
        db: &Database,
        configured_path: Option<&PathBuf>,
        file_path: &Path,
    ) -> Result<String, String> {
        Self::open_files_in_mlv_app(db, configured_path, &[file_path.to_path_buf()])
    }

    /// Searches standard paths to locate an MLV App installation executable.
    pub fn detect_mlv_app() -> Option<PathBuf> {
        let mut candidates: Vec<PathBuf> = Vec::new();

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
        if let Ok(user_profile) = std::env::var("USERPROFILE") {
            let up = PathBuf::from(user_profile);
            candidates.push(up.join("Downloads").join("MLV App.exe"));
            candidates.push(up.join("Desktop").join("MLV App.exe"));
        }
        if let Ok(path_var) = std::env::var("PATH") {
            for dir in std::env::split_paths(&path_var) {
                let c1 = dir.join("MLV App.exe");
                let c2 = dir.join("mlvapp.exe");
                let c3 = dir.join("mlv_app.exe");
                if c1.is_file() { candidates.push(c1); break; }
                if c2.is_file() { candidates.push(c2); break; }
                if c3.is_file() { candidates.push(c3); break; }
            }
        }

        candidates.into_iter().find(|p| p.is_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clean_windows_path() {
        let unc_extended = Path::new(r"\\?\UNC\server\share\file.mlv");
        assert_eq!(clean_windows_path(unc_extended), PathBuf::from(r"\\server\share\file.mlv"));

        let extended = Path::new(r"\\?\C:\footage\M01-1234.MLV");
        assert_eq!(clean_windows_path(extended), PathBuf::from(r"C:\footage\M01-1234.MLV"));

        let normal = Path::new(r"C:\footage\M01-1234.MLV");
        assert_eq!(clean_windows_path(normal), PathBuf::from(r"C:\footage\M01-1234.MLV"));
    }

    #[test]
    fn test_resolve_primary_mlv_path() {
        let temp_dir = std::env::temp_dir().join("mlv_resolve_test");
        let _ = std::fs::create_dir_all(&temp_dir);

        let mlv_file = temp_dir.join("test_clip.MLV");
        let m00_file = temp_dir.join("test_clip.M00");
        let _ = std::fs::write(&mlv_file, b"test");

        // When given the .M00 chunk and .MLV exists, it resolves to .MLV
        let resolved = resolve_primary_mlv_path(&m00_file);
        assert_eq!(resolved, mlv_file);

        // When given .MLV directly, stays .MLV
        let resolved_direct = resolve_primary_mlv_path(&mlv_file);
        assert_eq!(resolved_direct, mlv_file);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_generate_masxml_session() {
        let temp_dir = std::env::temp_dir().join("mlv_session_test");
        let _ = std::fs::create_dir_all(&temp_dir);

        let clip1 = temp_dir.join("A01_001.MLV");
        let clip2 = temp_dir.join("A01_002.MLV");
        let _ = std::fs::write(&clip1, b"MLVI");
        let _ = std::fs::write(&clip2, b"MLVI");

        let session_file = ExternalLauncher::generate_masxml_session(&[clip1.clone(), clip2.clone()]).unwrap();
        assert!(session_file.exists());

        let content = std::fs::read_to_string(&session_file).unwrap();
        assert!(content.contains("<mlv_files version=\"4\" mlvapp=\"1.14\">"));
        assert!(content.contains("A01_001.MLV"));
        assert!(content.contains("A01_002.MLV"));
        assert!(content.contains("</mlv_files>"));

        let _ = std::fs::remove_file(&session_file);
        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

