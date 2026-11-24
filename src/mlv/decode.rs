//! Frame reading + decoding (uncompressed bit-unpack or LJ92) into linear u16 Bayer data.

use super::color::{camera_to_srgb, Mat3};
use super::focus_pixels::{has_pdaf_sensor, FocusPixelMap};
use super::lj92;
use super::reader::MlvClip;
use rayon::prelude::*;
use std::fs::{File, OpenOptions};
use std::path::Path;
use std::sync::Arc;

/// A decoded Bayer frame ready for GPU upload / CPU preview.
pub struct DecodedFrame {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub data: Vec<u16>,
    pub black: f32,
    pub white: f32,
}

/// Open clip + file handles, shareable across decoder threads.
pub struct FrameSource {
    pub clip: MlvClip,
    pub camera_matrix: Mat3,
    pub focus_pixel_map: Option<Arc<FocusPixelMap>>,
    pub is_pdaf_camera: bool,
    files: Vec<File>,
}

fn open_seq_file(path: &Path) -> Result<File, String> {
    let mut opts = OpenOptions::new();
    opts.read(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_SEQUENTIAL_SCAN: u32 = 0x0800_0000;
        opts.custom_flags(FILE_FLAG_SEQUENTIAL_SCAN);
    }
    opts.open(path).map_err(|e| format!("{}: {e}", path.display()))
}

impl FrameSource {
    pub fn open(path: &str) -> Result<Arc<Self>, String> {
        Self::open_with_fpm(path, None)
    }

    pub fn open_with_fpm(path: &str, mlv_app_path: Option<&Path>) -> Result<Arc<Self>, String> {
        let clip = MlvClip::open(path)?;
        let files = clip
            .chunks
            .iter()
            .map(|p| File::open(p).map_err(|e| format!("{}: {e}", p.display())))
            .collect::<Result<Vec<_>, _>>()?;
        let camera_matrix = camera_to_srgb(&clip.meta);
        let is_pdaf_camera = has_pdaf_sensor(clip.meta.camera_model_id, &clip.meta.camera_name);
        let focus_pixel_map = FocusPixelMap::load_for_clip(&clip.meta, mlv_app_path).map(Arc::new);
        Ok(Arc::new(Self {
            clip,
            camera_matrix,
            focus_pixel_map,
            is_pdaf_camera,
            files,
        }))
    }

    /// Fast opener that scans only up to the first frame in the primary `.MLV` file.
    pub fn open_first_frame(path: &str) -> Result<Arc<Self>, String> {
        let clip = MlvClip::open_first_frame(path)?;
        let files = clip
            .chunks
            .iter()
            .map(|p| File::open(p).map_err(|e| format!("{}: {e}", p.display())))
            .collect::<Result<Vec<_>, _>>()?;
        let camera_matrix = camera_to_srgb(&clip.meta);
        let is_pdaf_camera = has_pdaf_sensor(clip.meta.camera_model_id, &clip.meta.camera_name);
        let focus_pixel_map = FocusPixelMap::load_for_clip(&clip.meta, None).map(Arc::new);
        Ok(Arc::new(Self {
            clip,
            camera_matrix,
            focus_pixel_map,
            is_pdaf_camera,
            files,
        }))
    }

    /// Open fresh chunk file handles optimized for sequential reads on a dedicated I/O thread.
    pub fn open_chunk_files(&self) -> Result<Vec<File>, String> {
        self.clip.chunks.iter().map(|p| open_seq_file(p)).collect()
    }

    pub fn frame_count(&self) -> usize {
        self.clip.frames.len()
    }

    /// Read all AUDF blocks across chunks and return linear 16-bit PCM samples
    pub fn load_audio_pcm(&self) -> Option<(super::types::AudioMetadata, Vec<i16>)> {
        if self.clip.audio_frames.is_empty() {
            return None;
        }
        let audio_meta = self.clip.meta.audio.clone().unwrap_or_default();
        let total_bytes: usize = self.clip.audio_frames.iter().map(|af| af.size as usize).sum();
        let mut samples = Vec::with_capacity(total_bytes / 2);
        let mut raw = Vec::new();

        for af in &self.clip.audio_frames {
            raw.resize(af.size as usize, 0);
            if read_at_file(&self.files[af.chunk as usize], af.offset, &mut raw).is_ok() {
                let sample_count = raw.len() / 2;
                for i in 0..sample_count {
                    let s = i16::from_le_bytes([raw[i * 2], raw[i * 2 + 1]]);
                    samples.push(s);
                }
            }
        }

        if samples.is_empty() {
            None
        } else {
            Some((audio_meta, samples))
        }
    }

    /// Read raw compressed/packed frame payload from explicit chunk handles into `packed`.
    pub fn read_raw_frame_into(
        &self,
        files: &[File],
        index: usize,
        packed: &mut Vec<u8>,
    ) -> Result<(), String> {
        let fr = *self.clip.frames.get(index).ok_or("frame index out of range")?;
        let f = files.get(fr.chunk as usize).ok_or("chunk handle out of range")?;
        packed.resize(fr.size as usize, 0);
        read_at_file(f, fr.offset, packed).map_err(|e| format!("read error: {e}"))
    }

    /// Decode an already-read raw frame payload, recycling `out_buf` capacity.
    pub fn decode_packed(
        &self,
        index: usize,
        packed: &[u8],
        mut out_buf: Vec<u16>,
        parallel_unpack: bool,
    ) -> Result<DecodedFrame, String> {
        let m = &self.clip.meta;
        let (w, h) = (m.width as usize, m.height as usize);
        let mut black = m.black_level as f32;
        let mut white = m.white_level as f32;

        if m.is_lj92() {
            let info = lj92::decode(packed, &mut out_buf)?;
            if out_buf.len() != w * h {
                out_buf.resize(w * h, 0);
            }
            // If precision is lower than the stored levels assume, rescale levels.
            let max = ((1u32 << info.precision.clamp(8, 16)) - 1) as f32;
            if white > max {
                let s = (white / max).log2().round().exp2();
                black /= s;
                white /= s;
            }
        } else {
            unpack_bits_opt(
                packed,
                w,
                h,
                m.bits_per_pixel as usize,
                &mut out_buf,
                parallel_unpack,
            );
        }

        Ok(DecodedFrame {
            index,
            width: w as u32,
            height: h as u32,
            data: out_buf,
            black,
            white: white.max(black + 1.0),
        })
    }

    /// Read + decode a frame. Thread-safe.
    pub fn decode(&self, index: usize) -> Result<DecodedFrame, String> {
        let mut packed = Vec::new();
        self.read_raw_frame_into(&self.files, index, &mut packed)?;
        self.decode_packed(index, &packed, Vec::new(), true)
    }
}

fn read_at_file(f: &File, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        use std::os::windows::fs::FileExt;
        let mut done = 0usize;
        while done < buf.len() {
            let n = f.seek_read(&mut buf[done..], offset + done as u64)?;
            if n == 0 {
                return Err(std::io::ErrorKind::UnexpectedEof.into());
            }
            done += n;
        }
        Ok(())
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::FileExt;
        f.read_exact_at(buf, offset)
    }
}

#[inline]
fn read_word(src: &[u8], i: usize) -> u32 {
    let idx = i * 2;
    if idx + 1 < src.len() {
        u16::from_le_bytes([src[idx], src[idx + 1]]) as u32
    } else {
        0
    }
}

#[inline]
fn unpack_row_generic(src: &[u8], row: &mut [u16], start_bit: usize, bpp: usize) {
    let mask = (1u32 << bpp) - 1;
    let bpp_u32 = bpp as u32;
    let mut bit = start_bit;
    for px in row.iter_mut() {
        let wi = bit >> 4;
        let off = (bit & 15) as u32;
        let data = (read_word(src, wi) << 16) | read_word(src, wi + 1);
        *px = ((data >> (32 - bpp_u32 - off)) & mask) as u16;
        bit += bpp;
    }
}

#[inline]
fn unpack_row(src: &[u8], row: &mut [u16], y: usize, w: usize, bpp: usize) {
    let start_bit = y * w * bpp;
    if (start_bit & 15) != 0 {
        unpack_row_generic(src, row, start_bit, bpp);
        return;
    }
    let byte_off = start_bit >> 3;
    let src_row = src.get(byte_off..).unwrap_or(&[]);

    match bpp {
        14 => {
            let groups = w / 8;
            let bytes_needed = groups * 14;
            if src_row.len() >= bytes_needed {
                let mut dst_i = 0;
                for chunk in src_row[..bytes_needed].chunks_exact(14) {
                    let w0 = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let w1 = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let w2 = u16::from_le_bytes([chunk[4], chunk[5]]);
                    let w3 = u16::from_le_bytes([chunk[6], chunk[7]]);
                    let w4 = u16::from_le_bytes([chunk[8], chunk[9]]);
                    let w5 = u16::from_le_bytes([chunk[10], chunk[11]]);
                    let w6 = u16::from_le_bytes([chunk[12], chunk[13]]);

                    row[dst_i] = w0 >> 2;
                    row[dst_i + 1] = ((w0 & 0x0003) << 12) | (w1 >> 4);
                    row[dst_i + 2] = ((w1 & 0x000F) << 10) | (w2 >> 6);
                    row[dst_i + 3] = ((w2 & 0x003F) << 8) | (w3 >> 8);
                    row[dst_i + 4] = ((w3 & 0x00FF) << 6) | (w4 >> 10);
                    row[dst_i + 5] = ((w4 & 0x03FF) << 4) | (w5 >> 12);
                    row[dst_i + 6] = ((w5 & 0x0FFF) << 2) | (w6 >> 14);
                    row[dst_i + 7] = w6 & 0x3FFF;
                    dst_i += 8;
                }
                if dst_i < w {
                    unpack_row_generic(src, &mut row[dst_i..], start_bit + dst_i * 14, 14);
                }
            } else {
                unpack_row_generic(src, row, start_bit, 14);
            }
        }
        12 => {
            let groups = w / 4;
            let bytes_needed = groups * 6;
            if src_row.len() >= bytes_needed {
                let mut dst_i = 0;
                for chunk in src_row[..bytes_needed].chunks_exact(6) {
                    let w0 = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let w1 = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let w2 = u16::from_le_bytes([chunk[4], chunk[5]]);

                    row[dst_i] = w0 >> 4;
                    row[dst_i + 1] = ((w0 & 0x000F) << 8) | (w1 >> 8);
                    row[dst_i + 2] = ((w1 & 0x00FF) << 4) | (w2 >> 12);
                    row[dst_i + 3] = w2 & 0x0FFF;
                    dst_i += 4;
                }
                if dst_i < w {
                    unpack_row_generic(src, &mut row[dst_i..], start_bit + dst_i * 12, 12);
                }
            } else {
                unpack_row_generic(src, row, start_bit, 12);
            }
        }
        10 => {
            let groups = w / 8;
            let bytes_needed = groups * 10;
            if src_row.len() >= bytes_needed {
                let mut dst_i = 0;
                for chunk in src_row[..bytes_needed].chunks_exact(10) {
                    let w0 = u16::from_le_bytes([chunk[0], chunk[1]]);
                    let w1 = u16::from_le_bytes([chunk[2], chunk[3]]);
                    let w2 = u16::from_le_bytes([chunk[4], chunk[5]]);
                    let w3 = u16::from_le_bytes([chunk[6], chunk[7]]);
                    let w4 = u16::from_le_bytes([chunk[8], chunk[9]]);

                    row[dst_i] = w0 >> 6;
                    row[dst_i + 1] = ((w0 & 0x003F) << 4) | (w1 >> 12);
                    row[dst_i + 2] = (w1 >> 2) & 0x03FF;
                    row[dst_i + 3] = ((w1 & 0x0003) << 8) | (w2 >> 8);
                    row[dst_i + 4] = ((w2 & 0x00FF) << 2) | (w3 >> 14);
                    row[dst_i + 5] = (w3 >> 4) & 0x03FF;
                    row[dst_i + 6] = ((w3 & 0x000F) << 6) | (w4 >> 10);
                    row[dst_i + 7] = w4 & 0x03FF;
                    dst_i += 8;
                }
                if dst_i < w {
                    unpack_row_generic(src, &mut row[dst_i..], start_bit + dst_i * 10, 10);
                }
            } else {
                unpack_row_generic(src, row, start_bit, 10);
            }
        }
        16 => {
            let bytes_needed = w * 2;
            if src_row.len() >= bytes_needed {
                for (dst, chunk) in row.iter_mut().zip(src_row[..bytes_needed].chunks_exact(2)) {
                    *dst = u16::from_le_bytes([chunk[0], chunk[1]]);
                }
            } else {
                unpack_row_generic(src, row, start_bit, 16);
            }
        }
        _ => unpack_row_generic(src, row, start_bit, bpp),
    }
}

/// Unpack ML's bit-packed RAW (stream of 16-bit LE words, MSB-first bits).
pub fn unpack_bits(src: &[u8], w: usize, h: usize, bpp: usize, out: &mut Vec<u16>) {
    unpack_bits_opt(src, w, h, bpp, out, true);
}

pub fn unpack_bits_opt(
    src: &[u8],
    w: usize,
    h: usize,
    bpp: usize,
    out: &mut Vec<u16>,
    parallel: bool,
) {
    let total = w * h;
    if out.len() != total {
        out.resize(total, 0);
    }
    if w == 0 || h == 0 || !(8..=16).contains(&bpp) {
        out.fill(0);
        return;
    }

    if parallel && h >= 128 {
        out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
            unpack_row(src, row, y, w, bpp);
        });
    } else {
        for (y, row) in out.chunks_exact_mut(w).enumerate() {
            unpack_row(src, row, y, w, bpp);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unpack_bits_reference(src: &[u8], w: usize, h: usize, bpp: usize) -> Vec<u16> {
        let mut out = vec![0u16; w * h];
        let words = src.len() / 2;
        let word = |i: usize| -> u32 {
            if i < words {
                u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]) as u32
            } else {
                0
            }
        };
        let mask = (1u32 << bpp) - 1;
        for y in 0..h {
            let mut bit = y * w * bpp;
            for x in 0..w {
                let wi = bit >> 4;
                let off = (bit & 15) as u32;
                let data = (word(wi) << 16) | word(wi + 1);
                out[y * w + x] = ((data >> (32 - bpp as u32 - off)) & mask) as u16;
                bit += bpp;
            }
        }
        out
    }

    #[test]
    fn test_unpack_bits_specialized_matches_reference() {
        for bpp in [10usize, 12, 14, 16] {
            for &(w, h) in &[(64usize, 12usize), (67usize, 9usize)] {
                let total_bytes = (w * h * bpp).div_ceil(16) * 2 + 4;
                let src: Vec<u8> = (0..total_bytes)
                    .map(|i| ((i * 137 + bpp * 29) & 0xFF) as u8)
                    .collect();

                let expected = unpack_bits_reference(&src, w, h, bpp);
                let mut actual = Vec::new();
                unpack_bits_opt(&src, w, h, bpp, &mut actual, false);
                assert_eq!(actual, expected, "Mismatch for bpp={bpp}, {w}x{h}");
            }
        }
    }
}
