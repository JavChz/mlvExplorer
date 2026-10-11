# MLV Explorer 🎬

A high-performance **Lightroom-style RAW video library, player, and catalog manager** for Magic Lantern Video (`.mlv`) files, built with **Rust**, **egui/eframe**, and **SQLite**.

Designed specifically for filmmakers and archival workflows to rapidly ingest, browse, inspect, tag, and play back Magic Lantern RAW videos without waiting for heavy NLE transcoding.

---

## 🌟 Key Features

- **Lightroom-Style Workflow**:
  - **Grid View (Contact Sheet)**: Smooth thumbnail grid with interactive card zoom, duration tags, resolution pills, star ratings, and pick/reject status flags.
  - **Loupe / Player View**: Full-screen single video player with interactive timeline scrubber, frame stepping (`Left`/`Right` arrow keys), loop playback, and timecode display.
  - **Navigator Sidebar**: Instant views for *All Videos*, *Flagged / Picks*, *Rejected*, and *Untagged*.
  - **Metadata & Inspector Panel**: Deep EXIF & RAW metadata extraction: Camera model, serial number, lens name, focal length, aperture ($f$-stop), shutter speed, ISO, kelvin white balance, frame rate, and raw bit depth.
  - **Tagging & Notes**: Custom tag chips (`#b-roll`, `#interview`), 1-5 star ratings, pick/reject status flags, and per-shot notes.
  - **Filter Bar**: Real-time filtering by Camera, Lens, ISO range, Star rating ($\ge 3\star$), and Pick flags.
  - **Drag-and-Drop Ingestion**: Drop single `.mlv` files or entire folder trees directly into the window for automatic scanning and indexing.
- **SQLite Catalog with Full Backup & Import**:
  - **Atomic Live Database Backup (`.db`)**: Creates a complete binary backup snapshot while the app is actively running via SQLite's native backup API.
  - **Portable JSON Export / Import (`.json`)**: Export and merge catalogs across different drives, editing machines, or operating systems.
- **Fast Raw Engine**:
  - Direct block parser for `MLVI`, `RAWI`, `VIDF`, `LENS`, `EXPO`, `RTCI`, `IDNT`, `WBAL`.
  - Half-size Bayer demosaic binning ($2 \times 2$ quad to RGB) for instant thumbnail generation with zero moiré or interpolation lag.

---

## 🔬 Research: MLV GPU Acceleration & Existing Projects

### 1. Existing Open-Source Projects
* **[MLV-App](https://github.com/ilia3101/MLV-App)** (C++/Qt/Cocoa):
  - The standard tool for MLV grading and export.
  - Traditionally CPU-bound for demosaicing (AMaZE, LMMSE, Bilinear), using OpenGL primarily for viewport blitting.
  - Community forks like **MLV-App-GPU** experiment with OpenCL/GL compute shaders, and the mobile port features OpenGL ES compute demosaicing.
* **[vkdt](https://github.com/hanatos/vkdt)** (Vulkan / GLSL Compute):
  - Created by Johannes Hanika (original creator of *darktable*).
  - A modern RAW processing engine that processes Magic Lantern MLV video directly on the GPU using Vulkan compute shaders. Highly recommended reference for Vulkan pipeline design.
* **[Fast CinemaDNG / Fast MLV](https://www.fastcompression.com)** (CUDA):
  - Commercial / specialized CUDA engine for NVIDIA GPUs. Achieves real-time 200+ FPS playback by carrying out 12/14-bit unpacking, demosaicing, and tone curves entirely within NVIDIA CUDA cores.
* **[LibMLV](https://github.com/ilia3101/LibMLV)**:
  - C parser library with an explicit roadmap towards a Rust rewrite.

### 2. Recommended GPU Architecture for Rust + egui
For high-performance cross-platform playback without requiring CUDA toolkit installations:
- **`wgpu` (WebGPU for native Rust)**:
  - `eframe` natively supports `wgpu`.
  - Works on **NVIDIA**, AMD, and Intel GPUs via Vulkan (Linux/Windows) and DirectX 12 (Windows).
  - **Pipeline**:
    1. Upload unpacked 16-bit Bayer sensor data to GPU texture (`TextureFormat::R16Uint`).
    2. Execute a single compute or fragment shader (`cs_debayer.wgsl` included in `src/mlv/debayer.rs`):
       - Subtract sensor black level & normalize against white saturation level.
       - Apply white balance multipliers ($R$, $G$, $B$).
       - Perform 1-pass Malvar-He-Cutler or Bilinear Bayer interpolation.
       - Apply camera color matrix to Rec.709 sRGB + Gamma 2.2 transfer function.
    3. Blit directly into egui's viewport via `egui::PaintCallback` or `egui::TextureHandle`.

---

## ⌨️ Lightroom Keyboard Shortcuts

| Shortcut | Action |
| :--- | :--- |
| **`G`** | Switch to Grid View (Contact Sheet) |
| **`E`** | Switch to Loupe / Player View |
| **`Space`** | Play / Pause video |
| **`Left` / `Right`** | Step 1 frame backward / forward |
| **`1` – `5`** | Set Star Rating (1 to 5 stars) |
| **`0`** | Clear Star Rating |
| **`P`** | Flag as **Picked** |
| **`X`** | Flag as **Rejected** |
| **`U`** | **Unflag** / reset status |
| **`Esc`** | Return to Grid View |

---

## 🛠️ Building & Running

### Prerequisites
1. **Rust Toolchain**: Install Rust via [rustup.rs](https://rustup.rs/):
   ```powershell
   winget install Rustlang.Rustup
   ```
2. **C++ Build Tools**: Visual Studio C++ Build Tools (or MinGW/GCC).

### Compilation
```powershell
# Run in debug mode
cargo run

# Build optimized release binary
cargo build --release
```
The compiled binary will be located at `target/release/mlv-explorer.exe`.
