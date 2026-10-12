//! Frame reading + decoding (uncompressed bit-unpack or LJ92) into linear u16 Bayer data.

use super::lj92;
use super::reader::MlvClip;
use rayon::prelude::*;
use std::fs::File;
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
    files: Vec<File>,
}

impl FrameSource {
    pub fn open(path: &str) -> Result<Arc<Self>, String> {
        let clip = MlvClip::open(path)?;
        let files = clip
            .chunks
            .iter()
            .map(|p| File::open(p).map_err(|e| format!("{}: {e}", p.display())))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Arc::new(Self { clip, files }))
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
        let mut samples = Vec::new();

        for af in &self.clip.audio_frames {
            let mut raw = vec![0u8; af.size as usize];
            if self.read_at(af.chunk as usize, af.offset, &mut raw).is_ok() {
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

    fn read_at(&self, chunk: usize, offset: u64, buf: &mut [u8]) -> std::io::Result<()> {
        let f = &self.files[chunk];
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

    /// Read + decode a frame. Thread-safe.
    pub fn decode(&self, index: usize) -> Result<DecodedFrame, String> {
        let fr = *self.clip.frames.get(index).ok_or("frame index out of range")?;
        let mut packed = vec![0u8; fr.size as usize];
        self.read_at(fr.chunk as usize, fr.offset, &mut packed)
            .map_err(|e| format!("read error: {e}"))?;

        let m = &self.clip.meta;
        let (w, h) = (m.width as usize, m.height as usize);
        let mut data = Vec::new();
        let mut black = m.black_level as f32;
        let mut white = m.white_level as f32;

        if m.is_lj92() {
            let info = lj92::decode(&packed, &mut data)?;
            data.resize(w * h, 0);
            // If precision is lower than the stored levels assume, rescale levels.
            let max = ((1u32 << info.precision.clamp(8, 16)) - 1) as f32;
            if white > max {
                let s = (white / max).log2().round().exp2();
                black /= s;
                white /= s;
            }
        } else {
            unpack_bits(&packed, w, h, m.bits_per_pixel as usize, &mut data);
        }

        Ok(DecodedFrame {
            index,
            width: w as u32,
            height: h as u32,
            data,
            black,
            white: white.max(black + 1.0),
        })
    }
}

/// Unpack ML's bit-packed RAW (stream of 16-bit LE words, MSB-first bits).
pub fn unpack_bits(src: &[u8], w: usize, h: usize, bpp: usize, out: &mut Vec<u16>) {
    out.clear();
    out.resize(w * h, 0);
    if w == 0 || h == 0 || !(8..=16).contains(&bpp) {
        return;
    }
    let words = src.len() / 2;
    let word = |i: usize| -> u32 {
        if i < words {
            u16::from_le_bytes([src[i * 2], src[i * 2 + 1]]) as u32
        } else {
            0
        }
    };
    let mask = (1u32 << bpp) - 1;
    out.par_chunks_mut(w).enumerate().for_each(|(y, row)| {
        let mut bit = y * w * bpp;
        for px in row.iter_mut() {
            let wi = bit >> 4;
            let off = (bit & 15) as u32;
            let data = (word(wi) << 16) | word(wi + 1);
            *px = ((data >> (32 - bpp as u32 - off)) & mask) as u16;
            bit += bpp;
        }
    });
}
