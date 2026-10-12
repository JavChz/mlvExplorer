# MLV Explorer 🎬

A photo catalog-style browser, player, and organizer for **Magic Lantern RAW Video (`.mlv`)** files, written in **Rust** with **egui/eframe** and **SQLite**.

Designed for people who shoot with Magic Lantern on Canon DSLRs/Mirrorless (5D Mark III, EOS M, 70D, 6D, 5D Mark II, etc.) and need an fast, responsive tool to ingest, organize, slate, review, and cull raw takes before sending selected clips into [MLV App](https://github.com/ilia3101/MLV-App) for final processing.

This is a hobbie personal project and I won't have time to review PRs, if you want to improve on this. Feel free to fork the repo. My apologies in advance. Also note that this project is a work in progress and may be unstable at times. Haven't tested outside of Windows 11, but in theory should be portable to any system with a modern GPU.

---

## 🌟 Features

### 🖼️ Photo Browser-Style Catalog & Ingestion

- **Contact Sheet Grid**: High-density thumbnail grid with smooth card scaling, aspect-ratio preservation, duration overlays, resolution badges, audio presence indicators, star ratings, and pick/reject flags.
- **Rubberband Box Drag Selection**: Click and drag across the catalog canvas to marquee-select multiple clips at once with interactive bounding box visualization (automatically excludes the scrollbar track).
- **Date Scrubber Tooltip**: Dragging or hovering along the scrollbar displays a floating month & year indicator pill (similar to Google Photos / Apple Photos) for instant chronological navigation across extensive catalogs.
- **Sequential & Modifier Selection**:
  - `Shift` + Click: Continuous range selection from the active clip to the clicked clip.
  - `Ctrl` / `Cmd` + Click: Toggle individual clips in and out of the active selection.
  - `Ctrl+D` / `Cmd+D`: Instantly deselect all clips.
- **Calendar Day Grouping**: Organize footage chronologically by shooting days with collapsible calendar headers (`Saturday, October 11, 2026 · 14 clips · 12.4 GB · 05:22`) and ascending/descending capture-date sorting.
- **Drag-and-Drop Ingestion**: Drop single `.mlv` clips or multi-gigabyte directories directly onto the window. Includes recursive discovery, duplicate detection, and a background import progress bar.
- **Right-Click Deletion Workflows**:
  - **Remove from Catalog**: Remove records, ratings, and tags from the SQLite database while leaving original RAW files on disk completely untouched.
  - **Move to Trash**: Safely sends the active or selected clips (including all 5D3 multi-card spanned slices `.M00`, `.M01`, etc.) directly into the OS Recycle Bin/Trash with modal confirmation.
- **Non-Destructive SQLite Catalog**: Instant database-backed filtering and search. Includes live atomic database backups (`.db`) and portable JSON catalog export/import across multiple editing machines.

### 🎬 Cinema Production Slate & Batch Inspector

- **Batch Editing Across Multiple Clips**: Selecting multiple clips lets you inspect and modify attributes in bulk:
  - **Star Ratings**: Set 1–5 stars or clear ratings for all selected takes.
  - **Pick / Reject Flags**: Flag multiple takes as Picked, Rejected, or Neutral in a single click.
  - **Custom Tags**: Add new tag chips (`#interview`, `#b-roll`) or remove tags from all selected clips simultaneously.
  - **Production Slate**: Batch-assign **Scene**, **Shot**, **Take**, **Camera Unit** (e.g. `A-Cam`, `B-Cam`), and **Location**.
  - **Aspect Ratio Stretch**: Batch de-squeeze anamorphic footage across entire scenes.
  - **Notes**: Update production comments across multiple takes simultaneously.
- **Deep Hardware EXIF Extraction**: Reads camera model, camera serial number, lens name, focal length, aperture ($f$-stop), shutter speed, ISO, kelvin color temperature, frame rate, and raw bit depth.

### 🎥 Cinema Player & Audio Sync

- **Synchronized Audio Playback Engine**: Linear PCM audio parsing (`WAVI` & `AUDF` blocks) with real-time synchronized audio output via `rodio`, timeline scrubbing, volume slider, mute toggle, and track info pills (`48 kHz / 16-bit stereo`).
- **Cinema Fullscreen Experience**: Press **`F`** to toggle immersive borderless fullscreen video mode with auto-hiding transport HUD, or double-click anywhere on the video canvas.
- **Aspect Ratio & Anamorphic De-squeeze**: Explicit format detection from `DISP`/`RAWC` blocks and manual overrides for standard (1:1), anamorphic (1.33x, 1.5x, 1.75x, 2.0x), and Magic Lantern 3x1 / 1x3 anamorphic crop modes.
- **Color Preview & Exposure**: High-speed Bayer demosaicing with sensor black/white normalization, camera color matrix conversion to linear sRGB, daylight white balance, and real-time exposure & white balance adjustment sliders.
- **One-Click Bridge to MLV App**: Dedicated **"Open in MLVApp"** action to instantly launch the current clip in [MLV App](https://github.com/ilia3101/MLV-App) for deep grading, raw export, and CinemaDNG/ProRes conversion.

### 💾 5D Mark III Dual-Card Spanning

- Full support for multi-part spanned recordings created by high-bitrate dual-card recording (CompactFlash + SD card spanning).
- Automatically discovers and stitches primary `.MLV` files with companion slices (`.M00`, `.M01`, `.M02`, etc.) across split directories seamlessly.

---

## ⌨️ Keyboard Shortcuts

| Shortcut                           | Context       | Action                                             |
| :--------------------------------- | :------------ | :------------------------------------------------- |
| **`G`**                            | Global        | Switch to Grid View (Contact Sheet)                |
| **`E`**                            | Global        | Switch to Player / Loupe View                      |
| **`F`**                            | Player View   | Toggle Cinema Fullscreen Video mode                |
| **`Space`**                        | Player View   | Play / Pause playback                              |
| **`Left` / `Right`**               | Player View   | Step 1 frame backward / forward                    |
| **`1` – `5`**                      | Grid / Player | Set Star Rating (1 to 5 stars) on selected clip(s) |
| **`0`**                            | Grid / Player | Clear Star Rating on selected clip(s)              |
| **`P`**                            | Grid / Player | Flag selected clip(s) as **Picked**                |
| **`X`**                            | Grid / Player | Flag selected clip(s) as **Rejected**              |
| **`U`**                            | Grid / Player | **Unflag** / reset selected clip(s) to Neutral     |
| **`Ctrl` + `D`** / **`Cmd` + `D`** | Grid View     | **Deselect All** clips                             |
| **`Del` / `Backspace`**            | Grid View     | **Remove from Catalog** (with confirmation)        |
| **`Esc`**                          | Any           | Exit Fullscreen / Return to Grid View              |

---

## 🚀 Getting Started

### Prerequisites

- **Rust Toolchain**: Rust 1.75+ (install via [rustup.rs](https://rustup.rs/))
- **C/C++ Build Tools**:
  - **Windows**: Visual Studio C++ Build Tools or MinGW
  - **Linux**: `build-essential`, `libasound2-dev`, `libxcb-render0-dev`, `libxcb-shape0-dev`, `libxcb-xfixes0-dev`
  - **macOS**: Xcode Command Line Tools

### Installation & Building

Clone the repository and build with Cargo:

```bash
git clone https://github.com/javch/mlvExplorer.git
cd mlvExplorer

# Run in development mode
cargo run

# Build release binary (optimized)
cargo build --release
```

The resulting executable will be located at:

- **Windows**: `target/release/mlv-explorer.exe`
- **Linux/macOS**: `target/release/mlv-explorer`

---

## 📦 Building Releases & Packaging Pipeline

MLV Explorer includes an automated GitHub Actions release pipeline as well as standalone scripts to produce distribution packages for **Windows x64** (`.zip`) and **Linux** (`.tar.gz` and `.AppImage`).

### 1. Setting & Bumping the Version Number
The canonical version number is configured in **[`Cargo.toml`](file:///d:/Dev/githubprojects/mlvExplorer/Cargo.toml)** on line 3:
```toml
[package]
name = "mlv-explorer"
version = "0.0.1"   # <-- Edit this number and increment for every new release
```
*Note*: Rust's Cargo package manager enforces [SemVer 2.0.0](https://semver.org/), so versions follow `MAJOR.MINOR.PATCH` format (e.g. `0.0.1`, `0.0.2`, `0.1.0`). Changing this single line automatically updates the internal app version, the **Help ➔ About** dialog, and the build packaging scripts.

### 2. Automated GitHub Actions Release Pipeline
Whenever you are ready to publish a new release to GitHub:
```bash
# 1. Update the version in Cargo.toml and commit
git add Cargo.toml
git commit -m "chore: bump version to v0.0.1"
git push origin main

# 2. Tag the commit and push the tag
git tag v0.0.1
git push origin v0.0.1
```
The GitHub Actions pipeline (`.github/workflows/release.yml`) will automatically:
1. Build optimized binaries on Windows and Linux.
2. Package:
   - `mlv-explorer-v0.0.1-windows-x64.zip` (containing `mlv-explorer.exe`, `README.md`, `LICENSE`)
   - `mlv-explorer-v0.0.1-linux-x64.tar.gz` (containing `mlv-explorer`, `README.md`, `LICENSE`)
   - `mlv-explorer-v0.0.1-x86_64.AppImage` (standalone portable Linux bundle)
3. Create a GitHub Release with auto-generated changelog notes and attach all 3 distribution assets.

You can also trigger builds manually without a tag via **GitHub Actions ➔ Release Build Pipeline ➔ Run workflow**.

### 3. Local Release Packaging Commands
To package releases locally on your machine:

#### **Windows (PowerShell)**:
```powershell
# Run the packaging script
.\scripts\package-release.ps1

# The distribution archive will be generated at:
# dist/mlv-explorer-v0.0.1-windows-x64.zip
```

#### **Linux (Bash)**:
```bash
# Make script executable and run
chmod +x ./scripts/package-release.sh
./scripts/package-release.sh

# The distribution archives will be generated in dist/:
# dist/mlv-explorer-v0.0.1-linux-x64.tar.gz
# dist/mlv-explorer-v0.0.1-x86_64.AppImage (if appimagetool is installed)
```

#### **Publishing Local Builds with GitHub CLI**:
```bash
gh release create v0.0.1 dist/* --title "Release v0.0.1" --generate-notes
```

---

## 🔄 Software Updates

MLV Explorer features an in-app software updater:
- Open **Help ➔ Check for Updates...** from the top menu bar.
- The app checks GitHub Releases for new versions without freezing the interface.
- If a new version is detected, the dialog displays release notes, compares versions, and allows you to download the appropriate OS package (`.zip` on Windows, `.AppImage` on Linux) directly with live download progress.

## 🏗️ Architecture & Module Organization

The project is structured into modular crates and subsystems for maintainability:

```text
src/
├── main.rs                 # Application entry point and window bootstrap
├── app.rs                  # Root eframe App coordinator, state, and top menu
├── audio.rs                # PCM audio engine & real-time synchronized playback (rodio)
├── playback.rs             # Video frame stepper, playhead clock, and frame cache
├── db/                     # SQLite database subsystem
│   ├── models.rs           # VideoRecord, Slate, and Query models
│   ├── queries.rs          # SQL queries, catalog filtering, and slate updates
│   └── schema.rs           # Database migrations, tables, and indexes
├── mlv/                    # Magic Lantern RAW parsing & decompression
│   ├── reader.rs           # MLV container block parser (MLVI, RAWI, VIDF, WAVI, AUDF...)
│   ├── decode.rs           # Frame unpacker, Bayer demosaicing, and audio extraction
│   ├── spanning.rs         # 5D Mark III multi-part card spanning resolution
│   ├── color.rs            # Camera matrix transform, white balance, and exposure
│   ├── lj92.rs             # ITU T.81 Lossless JPEG (LJ92) decompression
│   ├── debayer.rs          # WGSL compute shader debayering definitions
│   └── types.rs            # MLV block structures, metadata, and header types
└── ui/                     # egui user interface
    ├── grid/               # Contact sheet thumbnail grid & day grouping banners
    ├── player/             # Cinema video player, transport controls, and fullscreen HUD
    ├── inspector/          # EXIF metadata viewer and Production Slate editor
    ├── sidebar.rs          # Navigation tree, filters, and collections
    ├── top_panel.rs        # Ingestion progress bar and search filter controls
    └── theme.rs            # Dark cinema UI styling, colors, and typography
```

---

## 📜 Upstream Attributions & Acknowledgements

MLV Explorer builds upon reverse engineering, research, specifications, and open-source contributions from the Magic Lantern and raw video community. We gratefully credit the following upstream projects:

1. **[The Magic Lantern Project](https://magiclantern.fm)**:
   - **MLV Format Specification**: Struct layouts, chunk definitions (`mlv.h`, `raw.h`), and metadata block formats (`MLVI`, `RAWI`, `VIDF`, `WAVI`, `AUDF`, `EXPO`, `LENS`, `RTCI`) are derived from the Magic Lantern raw video specification.
   - Original MLV specification is licensed under the **GNU General Public License v2.0 or later**.

2. **[MLV App](https://github.com/ilia3101/MLV-App)** (by Ilia3101 and the MLV App Contributors):
   - Reference implementation for MLV file handling, multi-part companion chunk resolution (`.M00`, `.M01`), dual-card spanning logic, and camera color matrix standards.
   - Licensed under the **GNU General Public License v3.0**.

3. **Lossless JPEG Decoder (`lj92`)**:
   - The ITU-T.81 process 14 lossless JPEG decompressor used for compressed MLV streams is based on the algorithm originally authored by **Andrew Baldwin**, with adaptations by the Magic Lantern and MLV App developer communities.

4. **Rust Ecosystem & Open Source Libraries**:
   - [`eframe` / `egui`](https://github.com/emilk/egui) by Emil Ernerfeldt: Immediate-mode GUI framework powering the native desktop user interface.
   - [`Phosphor Icons`](https://phosphoricons.com) & [`egui-phosphor`](https://github.com/lucasmerlin/hello_egui/tree/main/crates/egui-phosphor) by Tobias Fried and Helena Zhang: Flexible, comprehensive icon family powering toolbar, navigation, and inspector glyphs.
   - [`rodio`](https://github.com/RustAudio/rodio) by Pierre Krieger and the RustAudio contributors: High-performance audio playback engine for synchronized linear PCM raw audio playback.
   - [`rusqlite`](https://github.com/rusqlite/rusqlite): Embedded SQLite client powering instant filtering, hierarchical tags, and atomic backups.
   - [`wgpu`](https://github.com/gfx-rs/wgpu): Portable, high-performance graphics backend (DirectX 12 / Vulkan / Metal).
   - [`rfd`](https://github.com/PolyMeilex/rfd): Rust File Dialog for cross-platform native file and folder pickers.
   - [`trash`](https://github.com/ArturKovacs/trash): Cross-platform safe OS Recycle Bin / Trash integration.
   - [`crossbeam-channel`](https://github.com/crossbeam-rs/crossbeam): High-throughput lock-free concurrency channels powering background ingestion.
   - [`image`](https://github.com/image-rs/image): Fast thumbnail JPEG encoding.
   - [`chrono`](https://github.com/chronotope/chrono): Shooting date parsing and timeline calendar grouping.

---

## 📄 License

MLV Explorer is free and open-source software licensed under the **GNU General Public License v3.0 or later** ([GPL-3.0-or-later](LICENSE)).

```text
MLV Explorer
Copyright (C) 2026 mlvExplorer Contributors

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program. If not, see <https://www.gnu.org/licenses/>.
```
