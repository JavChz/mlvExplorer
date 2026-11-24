use crossbeam_channel::Receiver;
use serde::Deserialize;
use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;

pub const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
pub const GITHUB_REPO: &str = "JavChz/mlvExplorer";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InstallKind {
    Installed,
    Portable,
}

impl InstallKind {
    pub fn label(self) -> &'static str {
        match self {
            InstallKind::Installed => "Installed",
            InstallKind::Portable => "Portable",
        }
    }
}

/// Detect whether the currently running binary was installed (Inno Setup / `.deb` / system path)
/// or launched as a standalone portable executable.
pub fn detect_install_kind() -> InstallKind {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            if dir.join(".installed").is_file() || dir.join("unins000.exe").is_file() {
                return InstallKind::Installed;
            }
            #[cfg(target_os = "linux")]
            {
                let s = exe.to_string_lossy();
                if s.starts_with("/usr/") || s.starts_with("/opt/") {
                    return InstallKind::Installed;
                }
            }
        }
    }
    InstallKind::Portable
}

/// True if an asset filename is an OS installer package (`.exe`, `.msi`, `.deb`).
pub fn is_installer_asset(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    lower.ends_with(".exe") || lower.ends_with(".msi") || lower.ends_with(".deb")
}

/// Launch a downloaded installer binary/package using the OS-appropriate mechanism.
pub fn launch_installer(path: &std::path::Path) -> Result<(), String> {
    let lower = path.to_string_lossy().to_ascii_lowercase();
    #[cfg(windows)]
    if lower.ends_with(".exe") {
        std::process::Command::new(path)
            .spawn()
            .map(|_| ())
            .map_err(|e| format!("Failed to launch installer: {e}"))?;
        return Ok(());
    }
    let _ = lower;
    open::that(path).map_err(|e| format!("Failed to open installer: {e}"))
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubAsset {
    pub name: String,
    pub browser_download_url: String,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    pub name: Option<String>,
    pub html_url: String,
    pub body: Option<String>,
    pub published_at: Option<String>,
    #[serde(default)]
    pub assets: Vec<GithubAsset>,
}

#[derive(Debug, Clone)]
pub enum UpdateStatus {
    Idle,
    Checking,
    UpToDate {
        current_version: String,
    },
    UpdateAvailable {
        current_version: String,
        latest_version: String,
        release_title: String,
        release_notes: String,
        release_url: String,
        install_kind: InstallKind,
        target_asset_name: Option<String>,
        target_download_url: Option<String>,
        target_asset_size: Option<u64>,
        is_installer: bool,
        alt_asset: Option<(String, String, u64)>,
    },
    Downloading {
        latest_version: String,
        asset_name: String,
        bytes_downloaded: u64,
        total_bytes: Option<u64>,
    },
    Downloaded {
        latest_version: String,
        asset_name: String,
        file_path: PathBuf,
        is_installer: bool,
    },
    Error(String),
}

pub struct UpdateManager {
    pub status: UpdateStatus,
    receiver: Option<Receiver<UpdateStatus>>,
    cancel_flag: Arc<AtomicBool>,
}

impl Default for UpdateManager {
    fn default() -> Self {
        Self {
            status: UpdateStatus::Idle,
            receiver: None,
            cancel_flag: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl UpdateManager {
    pub fn new() -> Self {
        Self::default()
    }

    /// Poll for status updates from background threads
    pub fn update(&mut self) {
        if let Some(ref rx) = self.receiver {
            while let Ok(new_status) = rx.try_recv() {
                self.status = new_status;
            }
        }
    }

    /// Check if currently busy checking or downloading
    pub fn is_busy(&self) -> bool {
        matches!(
            self.status,
            UpdateStatus::Checking | UpdateStatus::Downloading { .. }
        )
    }

    /// Cancel any active download
    pub fn cancel(&mut self) {
        self.cancel_flag.store(true, Ordering::SeqCst);
        self.status = UpdateStatus::Idle;
        self.receiver = None;
    }

    /// Start an interactive asynchronous update check against GitHub Releases
    pub fn check_for_updates(&mut self) {
        self.check_for_updates_internal(false);
    }

    /// Silent background check (used on startup): only transitions state if an update is found.
    pub fn check_for_updates_silent(&mut self) {
        self.check_for_updates_internal(true);
    }

    fn check_for_updates_internal(&mut self, silent: bool) {
        self.cancel_flag.store(false, Ordering::SeqCst);
        self.status = if silent {
            UpdateStatus::Idle
        } else {
            UpdateStatus::Checking
        };

        let (tx, rx) = crossbeam_channel::unbounded();
        self.receiver = Some(rx);

        thread::spawn(move || {
            let api_url = format!("https://api.github.com/repos/{}/releases/latest", GITHUB_REPO);
            let response = ureq::get(&api_url)
                .set("User-Agent", "mlv-explorer-updater")
                .set("Accept", "application/vnd.github.v3+json")
                .timeout(std::time::Duration::from_secs(10))
                .call();

            match response {
                Ok(resp) => match resp.into_json::<GithubRelease>() {
                    Ok(release) => {
                        let clean_tag = release.tag_name.trim_start_matches('v').trim();
                        if is_newer_version(CURRENT_VERSION, clean_tag) {
                            let install_kind = detect_install_kind();
                            let os = std::env::consts::OS;
                            let (asset_name, download_url, asset_size) =
                                find_best_asset_for(&release.assets, os, install_kind);
                            let is_installer = asset_name
                                .as_deref()
                                .map(is_installer_asset)
                                .unwrap_or(false);

                            let other_kind = match install_kind {
                                InstallKind::Installed => InstallKind::Portable,
                                InstallKind::Portable => InstallKind::Installed,
                            };
                            let (alt_name, alt_url, alt_size) =
                                find_best_asset_for(&release.assets, os, other_kind);
                            let alt_asset = match (alt_name, alt_url, alt_size) {
                                (Some(an), Some(au), Some(asz))
                                    if asset_name.as_deref() != Some(an.as_str()) =>
                                {
                                    Some((an, au, asz))
                                }
                                _ => None,
                            };

                            let _ = tx.send(UpdateStatus::UpdateAvailable {
                                current_version: CURRENT_VERSION.to_string(),
                                latest_version: clean_tag.to_string(),
                                release_title: release
                                    .name
                                    .unwrap_or_else(|| release.tag_name.clone()),
                                release_notes: release.body.unwrap_or_default(),
                                release_url: release.html_url,
                                install_kind,
                                target_asset_name: asset_name,
                                target_download_url: download_url,
                                target_asset_size: asset_size,
                                is_installer,
                                alt_asset,
                            });
                        } else if silent {
                            let _ = tx.send(UpdateStatus::Idle);
                        } else {
                            let _ = tx.send(UpdateStatus::UpToDate {
                                current_version: CURRENT_VERSION.to_string(),
                            });
                        }
                    }
                    Err(e) => {
                        if silent {
                            let _ = tx.send(UpdateStatus::Idle);
                        } else {
                            let _ = tx.send(UpdateStatus::Error(format!(
                                "Failed to parse release response: {e}"
                            )));
                        }
                    }
                },
                Err(ureq::Error::Status(404, _)) => {
                    if silent {
                        let _ = tx.send(UpdateStatus::Idle);
                    } else {
                        let _ = tx.send(UpdateStatus::UpToDate {
                            current_version: CURRENT_VERSION.to_string(),
                        });
                    }
                }
                Err(e) => {
                    if silent {
                        let _ = tx.send(UpdateStatus::Idle);
                    } else {
                        let _ = tx.send(UpdateStatus::Error(format!(
                            "Network error checking for updates: {e}"
                        )));
                    }
                }
            }
        });
    }

    /// Start downloading the chosen asset in a background thread with progress reporting
    pub fn download_update(
        &mut self,
        latest_version: String,
        asset_name: String,
        download_url: String,
    ) {
        self.cancel_flag = Arc::new(AtomicBool::new(false));
        let cancel = Arc::clone(&self.cancel_flag);
        let is_installer = is_installer_asset(&asset_name);

        self.status = UpdateStatus::Downloading {
            latest_version: latest_version.clone(),
            asset_name: asset_name.clone(),
            bytes_downloaded: 0,
            total_bytes: None,
        };

        let (tx, rx) = crossbeam_channel::unbounded();
        self.receiver = Some(rx);

        thread::spawn(move || {
            let target_dir = get_download_dir();
            let dest_path = target_dir.join(&asset_name);

            let resp = match ureq::get(&download_url)
                .set("User-Agent", "mlv-explorer-updater")
                .timeout(std::time::Duration::from_secs(60))
                .call()
            {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(UpdateStatus::Error(format!(
                        "Failed to initiate download: {e}"
                    )));
                    return;
                }
            };

            let total_bytes = resp
                .header("Content-Length")
                .and_then(|val| val.parse::<u64>().ok());

            let mut reader = resp.into_reader();
            let mut file = match File::create(&dest_path) {
                Ok(f) => f,
                Err(e) => {
                    let _ = tx.send(UpdateStatus::Error(format!(
                        "Failed to create destination file {}: {e}",
                        dest_path.display()
                    )));
                    return;
                }
            };

            let mut buffer = [0u8; 65536];
            let mut bytes_downloaded: u64 = 0;
            let mut last_reported = std::time::Instant::now();

            loop {
                if cancel.load(Ordering::SeqCst) {
                    let _ = std::fs::remove_file(&dest_path);
                    return;
                }

                match reader.read(&mut buffer) {
                    Ok(0) => break, // EOF
                    Ok(n) => {
                        if let Err(e) = file.write_all(&buffer[..n]) {
                            let _ = tx.send(UpdateStatus::Error(format!("Disk write error: {e}")));
                            return;
                        }
                        bytes_downloaded += n as u64;

                        // Throttle progress events to every 100ms
                        if last_reported.elapsed() >= std::time::Duration::from_millis(100) {
                            let _ = tx.send(UpdateStatus::Downloading {
                                latest_version: latest_version.clone(),
                                asset_name: asset_name.clone(),
                                bytes_downloaded,
                                total_bytes,
                            });
                            last_reported = std::time::Instant::now();
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(UpdateStatus::Error(format!(
                            "Download stream interrupted: {e}"
                        )));
                        return;
                    }
                }
            }

            let _ = file.flush();

            let _ = tx.send(UpdateStatus::Downloaded {
                latest_version,
                asset_name,
                file_path: dest_path,
                is_installer,
            });
        });
    }
}

/// Parse semantic version string into numeric parts (major, minor, patch)
fn parse_version(v: &str) -> Option<(u64, u64, u64)> {
    let parts: Vec<&str> = v.trim_start_matches('v').split('.').collect();
    if parts.is_empty() {
        return None;
    }
    let major = parts.get(0).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let minor = parts.get(1).and_then(|s| s.parse::<u64>().ok()).unwrap_or(0);
    let patch = parts
        .get(2)
        .and_then(|s| s.split('-').next())
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    Some((major, minor, patch))
}

/// Compare two versions. Returns true if `latest` is newer than `current`.
pub fn is_newer_version(current: &str, latest: &str) -> bool {
    let c = parse_version(current);
    let l = parse_version(latest);

    match (c, l) {
        (Some((c_maj, c_min, c_pat)), Some((l_maj, l_min, l_pat))) => {
            if l_maj != c_maj {
                l_maj > c_maj
            } else if l_min != c_min {
                l_min > c_min
            } else {
                l_pat > c_pat
            }
        }
        _ => latest != current,
    }
}

/// Select the best release asset for a given OS (`"windows"` / `"linux"`) and installation mode.
pub fn find_best_asset_for(
    assets: &[GithubAsset],
    os: &str,
    kind: InstallKind,
) -> (Option<String>, Option<String>, Option<u64>) {
    let pick = |rules: &[fn(&str) -> bool]| -> Option<&GithubAsset> {
        for rule in rules {
            if let Some(found) = assets.iter().find(|a| rule(&a.name.to_ascii_lowercase())) {
                return Some(found);
            }
        }
        None
    };

    let selected = match (os, kind) {
        ("windows", InstallKind::Installed) => pick(&[
            |n| n.ends_with(".exe") && (n.contains("setup") || n.contains("installer")),
            |n| n.ends_with(".exe"),
            |n| n.ends_with(".msi"),
            |n| n.ends_with(".zip") && (n.contains("windows") || n.contains("win")),
            |n| n.ends_with(".zip"),
        ]),
        ("windows", InstallKind::Portable) => pick(&[
            |n| n.ends_with(".zip") && n.contains("portable"),
            |n| n.ends_with(".zip") && (n.contains("windows") || n.contains("win")),
            |n| n.ends_with(".zip"),
            |n| n.ends_with(".exe"),
        ]),
        ("linux", InstallKind::Installed) => pick(&[
            |n| n.ends_with(".deb"),
            |n| n.ends_with(".appimage"),
            |n| n.contains("linux") && n.ends_with(".tar.gz"),
            |n| n.ends_with(".tar.gz"),
        ]),
        ("linux", InstallKind::Portable) => pick(&[
            |n| n.ends_with(".appimage"),
            |n| n.contains("linux") && n.ends_with(".tar.gz"),
            |n| n.ends_with(".tar.gz"),
            |n| n.ends_with(".deb"),
        ]),
        _ => None,
    };

    if let Some(asset) = selected {
        (
            Some(asset.name.clone()),
            Some(asset.browser_download_url.clone()),
            Some(asset.size),
        )
    } else {
        (None, None, None)
    }
}

/// Find the appropriate download directory on user system
fn get_download_dir() -> PathBuf {
    if let Ok(home) = std::env::var("USERPROFILE") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.exists() {
            return downloads;
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        let downloads = PathBuf::from(home).join("Downloads");
        if downloads.exists() {
            return downloads;
        }
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_assets() -> Vec<GithubAsset> {
        vec![
            GithubAsset {
                name: "mlv-explorer-v0.0.3-windows-x64-portable.zip".to_string(),
                browser_download_url: "https://example.com/win-portable.zip".to_string(),
                size: 1024,
            },
            GithubAsset {
                name: "mlv-explorer-v0.0.3-windows-x64-setup.exe".to_string(),
                browser_download_url: "https://example.com/win-setup.exe".to_string(),
                size: 2048,
            },
            GithubAsset {
                name: "mlv-explorer-v0.0.3-linux-x64-portable.tar.gz".to_string(),
                browser_download_url: "https://example.com/linux.tar.gz".to_string(),
                size: 3072,
            },
            GithubAsset {
                name: "mlv-explorer-v0.0.3-x86_64.AppImage".to_string(),
                browser_download_url: "https://example.com/linux.AppImage".to_string(),
                size: 4096,
            },
            GithubAsset {
                name: "mlv-explorer-v0.0.3-linux-amd64.deb".to_string(),
                browser_download_url: "https://example.com/linux.deb".to_string(),
                size: 5120,
            },
        ]
    }

    #[test]
    fn test_version_comparisons() {
        assert!(is_newer_version("0.0.1", "0.0.2"));
        assert!(is_newer_version("0.0.1", "0.1.0"));
        assert!(is_newer_version("0.0.1", "1.0.0"));
        assert!(is_newer_version("0.0.1", "v0.0.2"));
        assert!(!is_newer_version("0.0.2", "0.0.1"));
        assert!(!is_newer_version("0.0.1", "0.0.1"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
    }

    #[test]
    fn test_asset_matching_installed_vs_portable_windows() {
        let assets = sample_assets();

        let (name_inst, _, _) = find_best_asset_for(&assets, "windows", InstallKind::Installed);
        assert_eq!(
            name_inst.as_deref(),
            Some("mlv-explorer-v0.0.3-windows-x64-setup.exe")
        );
        assert!(is_installer_asset(name_inst.as_deref().unwrap()));

        let (name_port, _, _) = find_best_asset_for(&assets, "windows", InstallKind::Portable);
        assert_eq!(
            name_port.as_deref(),
            Some("mlv-explorer-v0.0.3-windows-x64-portable.zip")
        );
        assert!(!is_installer_asset(name_port.as_deref().unwrap()));
    }

    #[test]
    fn test_asset_matching_installed_vs_portable_linux() {
        let assets = sample_assets();

        let (name_inst, _, _) = find_best_asset_for(&assets, "linux", InstallKind::Installed);
        assert_eq!(
            name_inst.as_deref(),
            Some("mlv-explorer-v0.0.3-linux-amd64.deb")
        );
        assert!(is_installer_asset(name_inst.as_deref().unwrap()));

        let (name_port, _, _) = find_best_asset_for(&assets, "linux", InstallKind::Portable);
        assert_eq!(
            name_port.as_deref(),
            Some("mlv-explorer-v0.0.3-x86_64.AppImage")
        );
        assert!(!is_installer_asset(name_port.as_deref().unwrap()));
    }
}
