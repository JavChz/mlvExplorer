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

/// Handles discovering and launching external applications (e.g. MLV App)
pub struct ExternalLauncher;

impl ExternalLauncher {
    /// Launches MLV App with the given clip file path.
    /// Returns a status message string describing the outcome.
    pub fn open_in_mlv_app(
        db: &Database,
        configured_path: Option<&PathBuf>,
        file_path: &Path,
    ) -> Result<String, String> {
        // Resolve chunk (.M00/.M01) to primary .MLV if applicable
        let primary_path = resolve_primary_mlv_path(file_path);

        let abs_path = match std::fs::canonicalize(&primary_path) {
            Ok(p) => clean_windows_path(&p),
            Err(_) => clean_windows_path(&primary_path),
        };

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
                    let _ = db.set_setting("mlv_app_path", &picked.to_string_lossy());
                    exe_path = Some(picked);
                }
            }
        }

        // 4. Launch MLV App with the clip path as argument
        if let Some(exe) = exe_path {
            let clean_exe = clean_windows_path(&exe);
            let _ = db.set_setting("mlv_app_path", &clean_exe.to_string_lossy());

            let mut cmd = std::process::Command::new(&clean_exe);
            if let Some(parent_dir) = abs_path.parent() {
                cmd.current_dir(parent_dir);
            }
            cmd.arg(&abs_path);

            match cmd.spawn() {
                Ok(_) => Ok(format!("Launched MLV App with: {}", abs_path.display())),
                Err(e) => Err(format!("Failed to launch MLV App ({}): {}", clean_exe.display(), e)),
            }
        } else {
            Err("MLV App executable not configured or not found.".to_string())
        }
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
}

