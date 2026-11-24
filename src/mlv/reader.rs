//! MLV container parser.
//!
//! Struct layouts follow `mlv.h` / `raw.h` from the Magic Lantern project
//! (all little-endian, packed). Every block starts with:
//!   blockType[4] | blockSize u32 | timestamp u64   (16 bytes)

use super::types::*;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

/// Little-endian reader over a byte slice that never panics (returns 0 past the end).
struct Rd<'a> {
    b: &'a [u8],
    p: usize,
}

impl<'a> Rd<'a> {
    fn new(b: &'a [u8], p: usize) -> Self {
        Self { b, p }
    }
    fn take<const N: usize>(&mut self) -> [u8; N] {
        let mut out = [0u8; N];
        if self.p + N <= self.b.len() {
            out.copy_from_slice(&self.b[self.p..self.p + N]);
        }
        self.p += N;
        out
    }
    fn u8(&mut self) -> u8 {
        self.take::<1>()[0]
    }
    fn u16(&mut self) -> u16 {
        u16::from_le_bytes(self.take())
    }
    fn u32(&mut self) -> u32 {
        u32::from_le_bytes(self.take())
    }
    fn i32(&mut self) -> i32 {
        i32::from_le_bytes(self.take())
    }
    fn u64(&mut self) -> u64 {
        u64::from_le_bytes(self.take())
    }
    fn cstr<const N: usize>(&mut self) -> String {
        let raw = self.take::<N>();
        let end = raw.iter().position(|&c| c == 0).unwrap_or(N);
        String::from_utf8_lossy(&raw[..end]).trim().to_string()
    }
}

/// A fully indexed clip: metadata + sorted frame table across all chunks.
pub struct MlvClip {
    pub meta: MlvMetadata,
    pub frames: Vec<FrameRef>,
    pub audio_frames: Vec<AudioChunkRef>,
    pub chunks: Vec<PathBuf>,
}

impl MlvClip {
    /// Reads the 64-bit `fileGuid` from the initial `MLVI` block of a chunk file.
    pub fn read_file_guid(path: &Path) -> Option<u64> {
        let mut f = File::open(path).ok()?;
        let mut hdr = [0u8; 24];
        f.read_exact(&mut hdr).ok()?;
        if &hdr[0..4] != b"MLVI" {
            return None;
        }
        Some(u64::from_le_bytes([
            hdr[16], hdr[17], hdr[18], hdr[19], hdr[20], hdr[21], hdr[22], hdr[23],
        ]))
    }

    /// Discovers all files belonging to a clip (the primary `.MLV` file plus any spanned
    /// `.M00`, `.M01`, ... companion files in the same folder or sibling card folders).
    pub fn resolve_spanned_files(path: &Path) -> Vec<PathBuf> {
        let main = path.to_path_buf();
        let parent = main.parent().map(|p| p.to_path_buf());
        let stem = main
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let main_guid = Self::read_file_guid(&main);

        let mut chunks = vec![main.clone()];

        // 1. Direct sequential spanning: NAME.M00, NAME.M01, ... or NAME.m00, NAME.m01 ...
        for i in 0..100 {
            let p_upper = main.with_extension(format!("M{:02}", i));
            let p_lower = main.with_extension(format!("m{:02}", i));
            if p_upper.exists() && !chunks.contains(&p_upper) {
                chunks.push(p_upper);
            } else if p_lower.exists() && !chunks.contains(&p_lower) {
                chunks.push(p_lower);
            } else {
                break;
            }
        }

        // Helper to scan a directory for matching-stem companion chunks
        let scan_dir_for_companions = |dir: &Path, chunks: &mut Vec<PathBuf>| {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let ep = entry.path();
                    if ep.is_file() && !chunks.contains(&ep) {
                        let is_mlv_ext = ep.extension().map_or(false, |ext| {
                            let e = ext.to_string_lossy().to_uppercase();
                            e == "MLV"
                                || (e.starts_with('M')
                                    && e.len() == 3
                                    && e[1..].chars().all(|c| c.is_ascii_digit()))
                        });
                        if is_mlv_ext {
                            let ep_stem = ep
                                .file_stem()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_default();
                            if ep_stem.eq_ignore_ascii_case(&stem) {
                                // If we have a main GUID, verify the companion matches
                                if let (Some(mg), Some(cg)) = (main_guid, Self::read_file_guid(&ep)) {
                                    if mg != cg {
                                        continue;
                                    }
                                }
                                chunks.push(ep);
                            }
                        }
                    }
                }
            }
        };

        // 2. Discover companion files in the same folder or sibling card folders (e.g. CF/ and SD/)
        if let Some(ref dir) = parent {
            scan_dir_for_companions(dir, &mut chunks);
            if let Some(grandparent) = dir.parent() {
                if grandparent.parent().is_some() {
                    if let Ok(sib_entries) = std::fs::read_dir(grandparent) {
                        let candidate_names = [
                            format!("{stem}.M00"),
                            format!("{stem}.m00"),
                            format!("{stem}.M01"),
                            format!("{stem}.m01"),
                            format!("{stem}.MLV"),
                            format!("{stem}.mlv"),
                        ];
                        for sib in sib_entries.filter_map(|e| e.ok()).take(64) {
                            let sp = sib.path();
                            if sp.is_dir() && sp != *dir {
                                if candidate_names.iter().any(|name| sp.join(name).is_file()) {
                                    scan_dir_for_companions(&sp, &mut chunks);
                                }
                            }
                        }
                    }
                }
            }
        }

        chunks
    }

    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let main = path.as_ref().to_path_buf();
        let chunks = Self::resolve_spanned_files(&main);

        let mut meta = MlvMetadata {
            file_path: main.to_string_lossy().to_string(),
            ..Default::default()
        };
        let mut frames: Vec<FrameRef> = Vec::new();
        let mut audio_frames: Vec<AudioChunkRef> = Vec::new();
        let mut guid: Option<u64> = None;
        let mut valid_chunks = Vec::new();

        for (ci, chunk_path) in chunks.iter().enumerate() {
            if ci > 0 {
                if let (Some(expected), Some(cg)) = (guid, Self::read_file_guid(chunk_path)) {
                    if cg != expected {
                        continue; // not part of this recording
                    }
                }
            }
            let chunk_idx = valid_chunks.len() as u16;
            let mut chunk_frames = Vec::new();
            let mut chunk_audio = Vec::new();
            match scan_chunk(
                chunk_path,
                chunk_idx,
                &mut meta,
                &mut chunk_frames,
                &mut chunk_audio,
                ci == 0,
            ) {
                Ok(g) => {
                    if let Some(expected) = guid {
                        if g != expected {
                            continue; // not part of this recording
                        }
                    } else {
                        guid = Some(g);
                    }
                    valid_chunks.push(chunk_path.clone());
                    frames.append(&mut chunk_frames);
                    audio_frames.append(&mut chunk_audio);
                }
                Err(e) if ci == 0 => return Err(e),
                Err(_) => {}
            }
        }

        frames.sort_by_key(|f| f.frame_number);
        frames.dedup_by_key(|f| f.frame_number);

        if !audio_frames.is_empty() {
            let has_nonzero_frames = audio_frames.iter().any(|a| a.frame_number > 0);
            if has_nonzero_frames {
                audio_frames.sort_by_key(|a| (a.frame_number, a.timestamp));
                audio_frames.dedup_by_key(|a| a.frame_number);
            } else {
                audio_frames.sort_by_key(|a| a.timestamp);
                audio_frames.dedup_by_key(|a| a.timestamp);
            }
        }

        meta.video_guid = guid.unwrap_or(0);
        meta.chunk_count = valid_chunks.len() as u16;
        meta.frame_count = frames.len() as u32;
        meta.is_dual_card = meta.chunk_count > 1;
        meta.file_size = valid_chunks
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum();

        sanitize_levels(&mut meta);

        if meta.width == 0 || meta.height == 0 {
            return Err("MLV has no RAWI block (unsupported video class)".into());
        }
        if frames.is_empty() {
            return Err("MLV contains no video frames".into());
        }

        Ok(Self {
            meta,
            frames,
            audio_frames,
            chunks: valid_chunks,
        })
    }

    /// Fast-path opener that reads only the main `.MLV` file up to the first video frame
    /// and essential header blocks (`RAWI`, `RAWC`, `IDNT`). Used for fast thumbnail generation.
    pub fn open_first_frame<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let main = path.as_ref().to_path_buf();
        let mut meta = MlvMetadata {
            file_path: main.to_string_lossy().to_string(),
            ..Default::default()
        };
        let mut frames = Vec::with_capacity(1);
        let mut audio_frames = Vec::new();
        let guid = scan_chunk_inner(
            &main,
            0,
            &mut meta,
            &mut frames,
            &mut audio_frames,
            true,
            true,
        )?;
        meta.video_guid = guid;
        meta.chunk_count = 1;
        meta.frame_count = frames.len() as u32;
        sanitize_levels(&mut meta);

        if meta.width == 0 || meta.height == 0 {
            return Err("MLV has no RAWI block (unsupported video class)".into());
        }
        if frames.is_empty() {
            return Err("MLV has no video frames".into());
        }

        Ok(Self {
            meta,
            frames,
            audio_frames,
            chunks: vec![main],
        })
    }
}

/// Scan one chunk file; appends frames and fills metadata. Returns the file GUID.
fn scan_chunk(
    path: &Path,
    chunk_idx: u16,
    meta: &mut MlvMetadata,
    frames: &mut Vec<FrameRef>,
    audio_frames: &mut Vec<AudioChunkRef>,
    is_main: bool,
) -> Result<u64, String> {
    scan_chunk_inner(path, chunk_idx, meta, frames, audio_frames, is_main, false)
}

fn scan_chunk_inner(
    path: &Path,
    chunk_idx: u16,
    meta: &mut MlvMetadata,
    frames: &mut Vec<FrameRef>,
    audio_frames: &mut Vec<AudioChunkRef>,
    is_main: bool,
    stop_after_first_frame: bool,
) -> Result<u64, String> {
    let mut r = File::open(path).map_err(|e| format!("Cannot open {}: {e}", path.display()))?;
    let file_size = r.metadata().map_err(|e| e.to_string())?.len();

    let mut offset: u64 = 0;
    let mut guid = 0u64;
    let mut hdr = [0u8; 16];
    let mut buf: Vec<u8> = Vec::with_capacity(1024);

    while offset + 16 <= file_size {
        if r.read_exact(&mut hdr).is_err() {
            break;
        }
        let ty = [hdr[0], hdr[1], hdr[2], hdr[3]];
        let size = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as u64;

        if offset == 0 && &ty != b"MLVI" {
            return Err("Not an MLV file (missing MLVI header)".into());
        }
        if size < 16 || offset + size > file_size {
            break; // truncated / corrupt tail
        }

        match &ty {
            b"VIDF" => {
                if size < 32 {
                    break;
                }
                let mut sub = [0u8; 16];
                if r.read_exact(&mut sub).is_err() {
                    break;
                }
                let mut s = Rd::new(&sub, 0);
                let frame_number = s.u32();
                let crop_x = s.u16();
                let crop_y = s.u16();
                let pan_x = s.u16();
                let pan_y = s.u16();
                let frame_space = s.u32() as u64;
                if frames.is_empty() {
                    meta.crop_x = crop_x;
                    meta.crop_y = crop_y;
                    meta.pan_x = pan_x;
                    meta.pan_y = pan_y;
                }
                let data_off = offset + 32 + frame_space;
                let data_size = size.saturating_sub(32).saturating_sub(frame_space);
                frames.push(FrameRef {
                    frame_number,
                    chunk: chunk_idx,
                    offset: data_off,
                    size: data_size as u32,
                });
                if stop_after_first_frame && meta.width > 0 && meta.height > 0 {
                    break;
                }
                let skip = size as i64 - 32;
                if skip > 0 && r.seek(SeekFrom::Current(skip)).is_err() {
                    break;
                }
            }
            b"MLVI" | b"RAWI" | b"RAWC" | b"IDNT" | b"LENS" | b"EXPO" | b"RTCI" | b"WBAL" | b"DISP" | b"WAVI" => {
                if size > 64 * 1024 * 1024 {
                    let skip = size as i64 - 16;
                    if skip > 0 && r.seek(SeekFrom::Current(skip)).is_err() {
                        break;
                    }
                } else {
                    buf.clear();
                    buf.extend_from_slice(&hdr);
                    buf.resize(size as usize, 0);
                    if r.read_exact(&mut buf[16..]).is_err() {
                        break;
                    }
                    let g = parse_meta_block(&ty, &buf, meta, is_main);
                    if &ty == b"MLVI" {
                        guid = g;
                    }
                    if stop_after_first_frame && meta.width > 0 && meta.height > 0 && !frames.is_empty() {
                        break;
                    }
                }
            }
            b"AUDF" => {
                if size < 24 {
                    break;
                }
                meta.has_audio = true;
                if stop_after_first_frame {
                    let skip = size as i64 - 16;
                    if skip > 0 && r.seek(SeekFrom::Current(skip)).is_err() {
                        break;
                    }
                } else {
                    let timestamp = u64::from_le_bytes([
                        hdr[8], hdr[9], hdr[10], hdr[11], hdr[12], hdr[13], hdr[14], hdr[15],
                    ]);
                    let mut sub = [0u8; 8];
                    if r.read_exact(&mut sub).is_err() {
                        break;
                    }
                    let mut s = Rd::new(&sub, 0);
                    let frame_number = s.u32();
                    let frame_space = s.u32() as u64;
                    let data_off = offset + 24 + frame_space;
                    let data_size = size.saturating_sub(24).saturating_sub(frame_space);
                    audio_frames.push(AudioChunkRef {
                        timestamp,
                        frame_number,
                        chunk: chunk_idx,
                        offset: data_off,
                        size: data_size as u32,
                    });
                    let skip = size as i64 - 24;
                    if skip > 0 && r.seek(SeekFrom::Current(skip)).is_err() {
                        break;
                    }
                }
            }
            _ => {
                let skip = size as i64 - 16;
                if skip > 0 && r.seek(SeekFrom::Current(skip)).is_err() {
                    break;
                }
            }
        }
        offset += size;
    }

    Ok(guid)
}

/// Parses a metadata block. Returns the GUID for MLVI blocks, 0 otherwise.
fn parse_meta_block(ty: &[u8; 4], b: &[u8], m: &mut MlvMetadata, is_main: bool) -> u64 {
    // Payload starts after the 16-byte generic header.
    let mut r = Rd::new(b, 16);
    match ty {
        b"MLVI" => {
            // fileMagic[4] blockSize[4] versionString[8] | fileGuid ...
            let guid = r.u64();
            let _file_num = r.u16();
            let _file_count = r.u16();
            let _file_flags = r.u32();
            let video_class = r.u16();
            let audio_class = r.u16();
            let _video_frames = r.u32();
            let _audio_frames = r.u32();
            let fps_num = r.u32();
            let fps_den = r.u32();
            if is_main {
                m.video_class = video_class;
                m.has_audio |= audio_class != 0;
                m.fps_num = fps_num;
                m.fps_den = fps_den;
            }
            return guid;
        }
        b"RAWI" => {
            let x_res = r.u16() as u32;
            let y_res = r.u16() as u32;
            // struct raw_info
            let _api_version = r.i32();
            let _buffer = r.u32();
            let height = r.i32();
            let width = r.i32();
            let _pitch = r.i32();
            let _frame_size = r.i32();
            let bpp = r.i32();
            let black = r.i32();
            let white = r.i32();
            for _ in 0..4 {
                r.i32(); // crop
            }
            let active_area = [r.i32(), r.i32(), r.i32(), r.i32()];
            r.i32();
            r.i32(); // exposure_bias[2]
            let cfa = r.u32();
            let _illuminant = r.i32();
            let mut cm = Vec::with_capacity(18);
            for _ in 0..18 {
                cm.push(r.i32());
            }

            m.width = if x_res > 0 { x_res } else { width.max(0) as u32 };
            m.height = if y_res > 0 { y_res } else { height.max(0) as u32 };
            m.bits_per_pixel = bpp.clamp(0, 16) as u16;
            m.black_level = black.max(0) as u32;
            m.white_level = white.max(0) as u32;
            m.active_area = active_area;
            m.cfa_pattern = cfa;
            m.color_matrix = cm;
        }
        b"IDNT" => {
            let name = r.cstr::<32>();
            let model = r.u32();
            let serial = r.cstr::<32>();
            if !name.is_empty() {
                m.camera_name = name;
            }
            if model != 0 {
                m.camera_model_id = model;
            }
            if !serial.is_empty() {
                m.camera_serial = serial;
            }
        }
        b"LENS" => {
            let focal = r.u16();
            let _focus_dist = r.u16();
            let aperture = r.u16();
            let _stab = r.u8();
            let _af = r.u8();
            let _flags = r.u32();
            let _lens_id = r.u32();
            let name = r.cstr::<32>();
            if m.lens_name.is_empty() && !name.is_empty() {
                m.lens_name = name;
            }
            if m.focal_length_mm == 0 {
                m.focal_length_mm = focal;
            }
            if m.aperture_fstop == 0.0 && aperture > 0 {
                m.aperture_fstop = aperture as f32 / 100.0;
            }
        }
        b"EXPO" => {
            let _iso_mode = r.u32();
            let iso = r.u32();
            let _iso_analog = r.u32();
            let _digital_gain = r.u32();
            let shutter = r.u64();
            if m.iso == 0 {
                m.iso = iso;
                m.shutter_speed_microseconds = shutter.min(u32::MAX as u64) as u32;
            }
        }
        b"RTCI" => {
            let sec = r.u16();
            let min = r.u16();
            let hour = r.u16();
            let mday = r.u16();
            let mon = r.u16();
            let year = r.u16();
            if m.date_recorded.is_none() && mday > 0 && mday <= 31 && mon < 12 {
                m.date_recorded = Some(format!(
                    "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                    year as u32 + 1900,
                    mon + 1,
                    mday,
                    hour,
                    min,
                    sec
                ));
            }
        }
        b"WBAL" => {
            let _mode = r.u32();
            let kelvin = r.u32();
            if m.white_balance_kelvin == 0 {
                m.white_balance_kelvin = kelvin;
            }
        }
        b"RAWC" => {
            // mlv_rawc_hdr_t
            let sensor_res_x = r.u16();
            let sensor_res_y = r.u16();
            let sensor_crop = r.u16();
            let _reserved = r.u16();
            let bin_x = r.u8() as u16;
            let skip_x = r.u8() as u16;
            let bin_y = r.u8() as u16;
            let skip_y = r.u8() as u16;
            if sensor_res_x > 0 {
                m.sensor_res_x = sensor_res_x;
            }
            if sensor_res_y > 0 {
                m.sensor_res_y = sensor_res_y;
            }
            if sensor_crop > 0 {
                m.sensor_crop = sensor_crop;
            }
            if bin_x > 0 {
                m.binning_x = bin_x;
            }
            m.skipping_x = skip_x;
            if bin_y > 0 {
                m.binning_y = bin_y;
            }
            m.skipping_y = skip_y;
        }
        b"DISP" => {
            // Display / aspect ratio block if present
            let _mode = r.u32();
            let num = r.u32();
            let den = r.u32();
            if num > 0 && den > 0 {
                m.pixel_aspect_ratio = num as f32 / den as f32;
            }
        }
        b"WAVI" => {
            let format = r.u16();
            let channels = r.u16();
            let sampling_rate = r.u32();
            let _bytes_per_sec = r.u32();
            let _block_align = r.u16();
            let bits_per_sample = r.u16();
            m.has_audio = true;
            m.audio = Some(AudioMetadata {
                format,
                channels: if channels == 0 { 2 } else { channels },
                sample_rate: if sampling_rate == 0 { 48000 } else { sampling_rate },
                bits_per_sample: if bits_per_sample == 0 { 16 } else { bits_per_sample },
            });
        }
        _ => {}
    }
    0
}

/// Black/white levels in some files are stored at 14-bit scale even for 10/12-bit data.
fn sanitize_levels(m: &mut MlvMetadata) {
    let bpp = if (8..=16).contains(&m.bits_per_pixel) { m.bits_per_pixel } else { 14 };
    m.bits_per_pixel = bpp;
    let max = (1u32 << bpp) - 1;
    if m.white_level > max && bpp < 14 {
        let shift = 14 - bpp as u32;
        m.white_level >>= shift;
        m.black_level >>= shift;
    }
    if m.white_level == 0 || m.white_level > max {
        m.white_level = max;
    }
    if m.black_level >= m.white_level {
        m.black_level = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_block(ty: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut b = Vec::with_capacity(16 + payload.len());
        b.extend_from_slice(ty);
        b.extend_from_slice(&((16 + payload.len()) as u32).to_le_bytes());
        b.extend_from_slice(&0u64.to_le_bytes());
        b.extend_from_slice(payload);
        b
    }

    #[test]
    fn test_open_rejects_zero_frame_mlv_without_panic() {
        let mut dir = std::env::temp_dir();
        dir.push(format!("mlv_zero_frame_{}.MLV", std::process::id()));
        {
            let mut f = File::create(&dir).unwrap();
            // Minimal MLVI payload (>= 36 bytes after 16-byte header)
            let mut mlvi = vec![0u8; 36];
            mlvi[0..8].copy_from_slice(&0x1234_5678_9ABC_DEF0u64.to_le_bytes());
            mlvi[28..32].copy_from_slice(&24u32.to_le_bytes());
            mlvi[32..36].copy_from_slice(&1u32.to_le_bytes());
            f.write_all(&make_block(b"MLVI", &mlvi)).unwrap();

            // Minimal RAWI payload with x_res=64, y_res=36
            let mut rawi = vec![0u8; 164];
            rawi[0..2].copy_from_slice(&64u16.to_le_bytes());
            rawi[2..4].copy_from_slice(&36u16.to_le_bytes());
            f.write_all(&make_block(b"RAWI", &rawi)).unwrap();
        }
        let res = MlvClip::open(&dir);
        let _ = std::fs::remove_file(&dir);
        match res {
            Err(err) => assert!(err.contains("no video frames"), "Unexpected error: {err}"),
            Ok(_) => panic!("Expected error when opening 0-frame MLV"),
        }
    }
}
