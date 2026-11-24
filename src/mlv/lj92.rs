//! Lossless JPEG (ITU T.81 process 14, SOF3) decoder — the "LJ92" compression
//! Magic Lantern uses for compressed MLV. Output is written linearly
//! (row-major, components interleaved), which matches the raw Bayer layout.

const LUT_BITS: u32 = 12;

struct Huffman {
    lut: Vec<u16>, // (len << 8) | symbol ; len == 0 => slow path
    maxcode: [i32; 18],
    valptr: [i32; 17],
    mincode: [i32; 17],
    vals: Vec<u8>,
}

impl Huffman {
    fn new(counts: &[u8; 16], vals: Vec<u8>) -> Self {
        let mut h = Huffman {
            lut: vec![0u16; 1 << LUT_BITS],
            maxcode: [-1; 18],
            valptr: [0; 17],
            mincode: [0; 17],
            vals,
        };
        let mut code: i32 = 0;
        let mut k: i32 = 0;
        for l in 1..=16usize {
            let n = counts[l - 1] as i32;
            h.valptr[l] = k;
            h.mincode[l] = code;
            for i in 0..n {
                let c = (code + i) as u32;
                if l as u32 <= LUT_BITS {
                    let sym = h.vals.get((k + i) as usize).copied().unwrap_or(0) as u16;
                    let shift = LUT_BITS - l as u32;
                    let start = (c as usize) << shift;
                    let end = start.saturating_add(1usize << shift);
                    if start < (1 << LUT_BITS) {
                        for e in &mut h.lut[start..end.min(1 << LUT_BITS)] {
                            *e = ((l as u16) << 8) | sym;
                        }
                    }
                }
            }
            code += n;
            k += n;
            h.maxcode[l] = if n > 0 { code - 1 } else { -1 };
            code <<= 1;
        }
        h.maxcode[17] = i32::MAX;
        h
    }
}

struct Bits<'a> {
    d: &'a [u8],
    p: usize,
    acc: u64,
    n: u32,
}

impl<'a> Bits<'a> {
    #[inline(always)]
    fn ensure(&mut self) {
        if self.n >= 32 {
            return;
        }
        if self.p + 4 <= self.d.len() {
            let w = u32::from_be_bytes([
                self.d[self.p],
                self.d[self.p + 1],
                self.d[self.p + 2],
                self.d[self.p + 3],
            ]);
            let inv = !w;
            let has_ff = (inv.wrapping_sub(0x0101_0101) & !inv & 0x8080_8080) != 0;
            if !has_ff {
                self.acc |= (w as u64) << (32 - self.n);
                self.n += 32;
                self.p += 4;
                return;
            }
        }
        self.fill_slow();
    }

    #[cold]
    #[inline(never)]
    fn fill_slow(&mut self) {
        while self.n <= 56 {
            let mut b = 0u8;
            if self.p < self.d.len() {
                b = self.d[self.p];
                if b == 0xFF {
                    let next = self.d.get(self.p + 1).copied().unwrap_or(0);
                    if next == 0x00 {
                        self.p += 2;
                    } else {
                        b = 0; // marker: feed zeros, don't advance
                    }
                } else {
                    self.p += 1;
                }
            }
            self.acc |= (b as u64) << (56 - self.n);
            self.n += 8;
        }
    }
    #[inline(always)]
    fn peek(&self, k: u32) -> u32 {
        (self.acc >> (64 - k)) as u32
    }
    #[inline(always)]
    fn consume(&mut self, k: u32) {
        if k == 0 || k >= 64 {
            return;
        }
        self.acc <<= k;
        self.n = self.n.saturating_sub(k);
    }
    #[inline(always)]
    fn decode(&mut self, h: &Huffman) -> u32 {
        let e = h.lut[self.peek(LUT_BITS) as usize];
        let len = (e >> 8) as u32;
        if len > 0 {
            self.consume(len);
            return (e & 0xFF) as u32;
        }
        for l in (LUT_BITS + 1)..=16 {
            let code = self.peek(l) as i32;
            if code <= h.maxcode[l as usize] {
                self.consume(l);
                let idx = h.valptr[l as usize] + code - h.mincode[l as usize];
                return h.vals.get(idx as usize).copied().unwrap_or(0) as u32;
            }
        }
        self.consume(1);
        0
    }
    #[inline(always)]
    fn diff(&mut self, ssss: u32) -> i32 {
        match ssss {
            0 => 0,
            16 => 32768,
            s if (1..16).contains(&s) => {
                let v = self.peek(s) as i32;
                self.consume(s);
                if v < (1 << (s - 1)) {
                    v - (1 << s) + 1
                } else {
                    v
                }
            }
            _ => 0,
        }
    }
    #[inline(always)]
    fn next_diff(&mut self, h: &Huffman) -> i32 {
        self.ensure();
        let ssss = self.decode(h);
        self.diff(ssss)
    }
}

pub struct Lj92Info {
    pub width: usize,
    pub height: usize,
    pub components: usize,
    pub precision: u8,
}

/// Decode an LJ92 stream into `out` (resized to width*height*components).
pub fn decode(data: &[u8], out: &mut Vec<u16>) -> Result<Lj92Info, String> {
    if data.len() < 4 || data[0] != 0xFF || data[1] != 0xD8 {
        return Err("LJ92: missing SOI".into());
    }
    let mut tables: [Option<Huffman>; 4] = [None, None, None, None];
    let mut precision = 0u8;
    let mut height = 0usize;
    let mut width = 0usize;
    let mut ncomp = 0usize;
    let mut comp_ids = [0u8; 4];

    let mut p = 2usize;
    loop {
        // find next marker
        while p < data.len() && data[p] != 0xFF {
            p += 1;
        }
        while p < data.len() && data[p] == 0xFF {
            p += 1;
        }
        if p >= data.len() {
            return Err("LJ92: unexpected end".into());
        }
        let marker = data[p];
        p += 1;
        if marker == 0x00 || marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if marker == 0xD9 {
            return Err("LJ92: EOI before scan".into());
        }
        if p + 2 > data.len() {
            return Err("LJ92: truncated segment".into());
        }
        let len = u16::from_be_bytes([data[p], data[p + 1]]) as usize;
        if len < 2 || p + len > data.len() {
            return Err("LJ92: bad segment length".into());
        }
        let seg = &data[p + 2..p + len];
        match marker {
            0xC4 => {
                let mut q = 0usize;
                while q + 17 <= seg.len() {
                    let th = (seg[q] & 0x0F) as usize;
                    let mut counts = [0u8; 16];
                    counts.copy_from_slice(&seg[q + 1..q + 17]);
                    let total: usize = counts.iter().map(|&c| c as usize).sum();
                    let vals = seg[q + 17..(q + 17 + total).min(seg.len())].to_vec();
                    if th < 4 {
                        tables[th] = Some(Huffman::new(&counts, vals));
                    }
                    q += 17 + total;
                }
            }
            0xC3 => {
                if seg.len() < 6 {
                    return Err("LJ92: bad SOF3".into());
                }
                precision = seg[0];
                height = u16::from_be_bytes([seg[1], seg[2]]) as usize;
                width = u16::from_be_bytes([seg[3], seg[4]]) as usize;
                ncomp = (seg[5] as usize).min(4);
                for c in 0..ncomp {
                    comp_ids[c] = seg.get(6 + c * 3).copied().unwrap_or(0);
                }
            }
            0xDA => {
                if seg.is_empty() {
                    return Err("LJ92: empty SOS".into());
                }
                let ns = (seg[0] as usize).min(4);
                if seg.len() < 4 + ns * 2 {
                    return Err("LJ92: truncated SOS".into());
                }
                let mut comp_table = [0usize; 4];
                for i in 0..ns {
                    let cid = seg[1 + i * 2];
                    let td = (seg[2 + i * 2] >> 4) as usize;
                    let ci = comp_ids[..ncomp].iter().position(|&c| c == cid).unwrap_or(i);
                    comp_table[ci.min(3)] = td.min(3);
                }
                let predictor = seg[1 + ns * 2];
                let pt = seg[3 + ns * 2] & 0x0F;
                let scan_start = p + len;
                if scan_start > data.len() {
                    return Err("LJ92: truncated scan".into());
                }
                return decode_scan(
                    &data[scan_start..],
                    &tables,
                    &comp_table,
                    width,
                    height,
                    ncomp,
                    precision,
                    predictor,
                    pt,
                    out,
                );
            }
            _ => {}
        }
        p += len;
    }
}

#[allow(clippy::too_many_arguments)]
fn decode_scan(
    scan: &[u8],
    tables: &[Option<Huffman>; 4],
    comp_table: &[usize; 4],
    width: usize,
    height: usize,
    nc: usize,
    precision: u8,
    predictor: u8,
    pt: u8,
    out: &mut Vec<u16>,
) -> Result<Lj92Info, String> {
    if width == 0 || height == 0 || nc == 0 {
        return Err("LJ92: missing frame header".into());
    }
    let mut huff: Vec<&Huffman> = Vec::with_capacity(nc);
    for c in 0..nc {
        huff.push(
            tables[comp_table[c]]
                .as_ref()
                .or(tables.iter().flatten().next())
                .ok_or("LJ92: missing Huffman table")?,
        );
    }

    let row = width * nc;
    let total = row * height;
    if out.len() != total {
        out.resize(total, 0);
    }
    let mut bits = Bits { d: scan, p: 0, acc: 0, n: 0 };
    let initial = 1i32 << (precision as i32 - pt as i32 - 1).max(0);

    // Peel y == 0 (always horizontal prediction from left neighbor, initial at x == 0)
    {
        let first_row = &mut out[..row];
        for c in 0..nc {
            let diff = bits.next_diff(huff[c]);
            first_row[c] = (initial + diff) as u16;
        }
        for x in 1..width {
            let base = x * nc;
            for c in 0..nc {
                let diff = bits.next_diff(huff[c]);
                let pred = first_row[base - nc + c] as i32;
                first_row[base + c] = (pred + diff) as u16;
            }
        }
    }

    match (nc, predictor) {
        (2, 1) => {
            let (h0, h1) = (huff[0], huff[1]);
            for y in 1..height {
                let (prev_all, cur_row) = out[..(y + 1) * row].split_at_mut(y * row);
                let prev_row = &prev_all[(y - 1) * row..];
                let mut ra0 = (prev_row[0] as i32 + bits.next_diff(h0)) as u16;
                let mut ra1 = (prev_row[1] as i32 + bits.next_diff(h1)) as u16;
                cur_row[0] = ra0;
                cur_row[1] = ra1;
                for dst in cur_row[2..].chunks_exact_mut(2) {
                    ra0 = (ra0 as i32 + bits.next_diff(h0)) as u16;
                    ra1 = (ra1 as i32 + bits.next_diff(h1)) as u16;
                    dst[0] = ra0;
                    dst[1] = ra1;
                }
            }
        }
        (2, 7) => {
            let (h0, h1) = (huff[0], huff[1]);
            for y in 1..height {
                let (prev_all, cur_row) = out[..(y + 1) * row].split_at_mut(y * row);
                let prev_row = &prev_all[(y - 1) * row..];
                let mut ra0 = (prev_row[0] as i32 + bits.next_diff(h0)) as u16;
                let mut ra1 = (prev_row[1] as i32 + bits.next_diff(h1)) as u16;
                cur_row[0] = ra0;
                cur_row[1] = ra1;
                for (prev, dst) in prev_row[2..].chunks_exact(2).zip(cur_row[2..].chunks_exact_mut(2)) {
                    let pred0 = (ra0 as i32 + prev[0] as i32) >> 1;
                    ra0 = (pred0 + bits.next_diff(h0)) as u16;
                    let pred1 = (ra1 as i32 + prev[1] as i32) >> 1;
                    ra1 = (pred1 + bits.next_diff(h1)) as u16;
                    dst[0] = ra0;
                    dst[1] = ra1;
                }
            }
        }
        (4, 1) => {
            let (h0, h1, h2, h3) = (huff[0], huff[1], huff[2], huff[3]);
            for y in 1..height {
                let (prev_all, cur_row) = out[..(y + 1) * row].split_at_mut(y * row);
                let prev_row = &prev_all[(y - 1) * row..];
                let mut ra0 = (prev_row[0] as i32 + bits.next_diff(h0)) as u16;
                let mut ra1 = (prev_row[1] as i32 + bits.next_diff(h1)) as u16;
                let mut ra2 = (prev_row[2] as i32 + bits.next_diff(h2)) as u16;
                let mut ra3 = (prev_row[3] as i32 + bits.next_diff(h3)) as u16;
                cur_row[0] = ra0;
                cur_row[1] = ra1;
                cur_row[2] = ra2;
                cur_row[3] = ra3;
                for dst in cur_row[4..].chunks_exact_mut(4) {
                    ra0 = (ra0 as i32 + bits.next_diff(h0)) as u16;
                    ra1 = (ra1 as i32 + bits.next_diff(h1)) as u16;
                    ra2 = (ra2 as i32 + bits.next_diff(h2)) as u16;
                    ra3 = (ra3 as i32 + bits.next_diff(h3)) as u16;
                    dst[0] = ra0;
                    dst[1] = ra1;
                    dst[2] = ra2;
                    dst[3] = ra3;
                }
            }
        }
        (1, 1) => {
            let h0 = huff[0];
            for y in 1..height {
                let (prev_all, cur_row) = out[..(y + 1) * row].split_at_mut(y * row);
                let prev_row = &prev_all[(y - 1) * row..];
                let mut ra0 = (prev_row[0] as i32 + bits.next_diff(h0)) as u16;
                cur_row[0] = ra0;
                for dst in &mut cur_row[1..] {
                    ra0 = (ra0 as i32 + bits.next_diff(h0)) as u16;
                    *dst = ra0;
                }
            }
        }
        _ => {
            for y in 1..height {
                let (prev_all, cur_row) = out[..(y + 1) * row].split_at_mut(y * row);
                let prev_row = &prev_all[(y - 1) * row..];
                for c in 0..nc {
                    cur_row[c] = (prev_row[c] as i32 + bits.next_diff(huff[c])) as u16;
                }
                for x in 1..width {
                    let base = x * nc;
                    for c in 0..nc {
                        let i = base + c;
                        let ra = cur_row[i - nc] as i32;
                        let rb = prev_row[i] as i32;
                        let rc = prev_row[i - nc] as i32;
                        let pred = match predictor {
                            1 => ra,
                            2 => rb,
                            3 => rc,
                            4 => ra + rb - rc,
                            5 => ra + ((rb - rc) >> 1),
                            6 => rb + ((ra - rc) >> 1),
                            7 => (ra + rb) >> 1,
                            _ => ra,
                        };
                        cur_row[i] = (pred + bits.next_diff(huff[c])) as u16;
                    }
                }
            }
        }
    }

    Ok(Lj92Info {
        width,
        height,
        components: nc,
        precision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_lj92_test_stream(
        pixels: &[u16],
        width: usize,
        height: usize,
        nc: usize,
        precision: u8,
        predictor: u8,
    ) -> Vec<u8> {
        // Simple 17-symbol Huffman table (ssss = 0..=16) with fixed 5-bit codes for ssss 0..=15
        // and 5-bit code for 16 -> actually counts[4] = 16 would exceed 5-bit for 17 symbols,
        // so use lengths: 1 code of len 4 (ssss=0), 14 codes of len 5 (ssss=1..=14), 2 codes of len 6 (ssss=15,16).
        // Let's verify Kraft sum: 1/16 + 14/32 + 2/64 = 4/64 + 28/64 + 2/64 = 34/64 <= 1. Valid!
        let mut counts = [0u8; 16];
        counts[3] = 1; // len 4: ssss 0
        counts[4] = 14; // len 5: ssss 1..=14
        counts[5] = 2; // len 6: ssss 15, 16
        let vals: Vec<u8> = (0..=16).collect();

        // Assign canonical Huffman codes
        let mut codes = [(0u32, 0u32); 17]; // (code, len)
        let mut code = 0u32;
        let mut k = 0usize;
        for l in 1..=16u32 {
            let n = counts[l as usize - 1] as usize;
            for _ in 0..n {
                codes[vals[k] as usize] = (code, l);
                code += 1;
                k += 1;
            }
            code <<= 1;
        }

        let mut out = Vec::new();
        // SOI
        out.extend_from_slice(&[0xFF, 0xD8]);
        // DHT
        let dht_len = 2 + 1 + 16 + vals.len();
        out.extend_from_slice(&[0xFF, 0xC4]);
        out.extend_from_slice(&(dht_len as u16).to_be_bytes());
        out.push(0x00); // table 0
        out.extend_from_slice(&counts);
        out.extend_from_slice(&vals);
        // SOF3
        let sof_len = 2 + 6 + nc * 3;
        out.extend_from_slice(&[0xFF, 0xC3]);
        out.extend_from_slice(&(sof_len as u16).to_be_bytes());
        out.push(precision);
        out.extend_from_slice(&(height as u16).to_be_bytes());
        out.extend_from_slice(&(width as u16).to_be_bytes());
        out.push(nc as u8);
        for c in 0..nc {
            out.push((c + 1) as u8);
            out.push(0x11);
            out.push(0x00);
        }
        // SOS
        let sos_len = 2 + 1 + nc * 2 + 3;
        out.extend_from_slice(&[0xFF, 0xDA]);
        out.extend_from_slice(&(sos_len as u16).to_be_bytes());
        out.push(nc as u8);
        for c in 0..nc {
            out.push((c + 1) as u8);
            out.push(0x00);
        }
        out.push(predictor);
        out.push(0x00);
        out.push(0x00); // pt = 0

        // Encode scan bits with 0xFF 0x00 stuffing
        let mut acc = 0u64;
        let mut bits = 0u32;
        let mut push_bits = |val: u32, len: u32, buf: &mut Vec<u8>| {
            if len == 0 {
                return;
            }
            acc = (acc << len) | (val as u64 & ((1u64 << len) - 1));
            bits += len;
            while bits >= 8 {
                bits -= 8;
                let b = ((acc >> bits) & 0xFF) as u8;
                buf.push(b);
                if b == 0xFF {
                    buf.push(0x00);
                }
            }
        };

        let row = width * nc;
        let initial = 1i32 << (precision as i32 - 1);
        for y in 0..height {
            for x in 0..width {
                for c in 0..nc {
                    let i = y * row + x * nc + c;
                    let pred = if y == 0 {
                        if x == 0 { initial } else { pixels[i - nc] as i32 }
                    } else if x == 0 {
                        pixels[i - row] as i32
                    } else {
                        let ra = pixels[i - nc] as i32;
                        let rb = pixels[i - row] as i32;
                        let _rc = pixels[i - row - nc] as i32;
                        match predictor {
                            1 => ra,
                            7 => (ra + rb) >> 1,
                            _ => ra,
                        }
                    };
                    let mut diff = pixels[i] as i32 - pred;
                    // Wrap modulo 65536 to signed 16-bit difference if needed
                    if diff > 32768 {
                        diff -= 65536;
                    } else if diff < -32768 {
                        diff += 65536;
                    }
                    let ssss = if diff == 0 {
                        0
                    } else if diff == 32768 || diff == -32768 {
                        16
                    } else {
                        let mag = diff.unsigned_abs();
                        32 - mag.leading_zeros()
                    };
                    let (hcode, hlen) = codes[ssss as usize];
                    push_bits(hcode, hlen, &mut out);
                    if ssss > 0 && ssss < 16 {
                        let enc = if diff < 0 {
                            (diff + (1 << ssss) - 1) as u32
                        } else {
                            diff as u32
                        };
                        push_bits(enc, ssss, &mut out);
                    }
                }
            }
        }
        if bits > 0 {
            let pad = 8 - bits;
            let b = ((acc << pad) & 0xFF) as u8;
            out.push(b);
            if b == 0xFF {
                out.push(0x00);
            }
        }
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    #[test]
    fn test_lj92_decode_bit_exact_variants_and_stuffing() {
        for &(nc, predictor) in &[(1usize, 1u8), (2, 1), (2, 7), (4, 1)] {
            let w = 23;
            let h = 15;
            let mut expected = Vec::with_capacity(w * h * nc);
            for i in 0..(w * h * nc) {
                // Mix flat runs, steep edges, and 0xFF-inducing negative/positive jumps
                let v = match i % 7 {
                    0 => 2048,
                    1 => 16383,
                    2 => 0,
                    3 => 12345,
                    4 => 16380,
                    5 => 4095,
                    _ => ((i * 7919) % 16384) as u16,
                };
                expected.push(v);
            }
            let stream = encode_lj92_test_stream(&expected, w, h, nc, 14, predictor);
            assert!(
                stream.windows(2).any(|w| w == [0xFF, 0x00]),
                "Test stream should exercise 0xFF 0x00 byte-stuffing"
            );

            let mut decoded = Vec::new();
            let info = decode(&stream, &mut decoded).unwrap();
            assert_eq!(info.width, w);
            assert_eq!(info.height, h);
            assert_eq!(info.components, nc);
            assert_eq!(info.precision, 14);
            assert_eq!(decoded, expected, "Mismatch for nc={nc}, predictor={predictor}");

            // Also test buffer reuse path (second decode reuses already-sized `decoded`)
            let info2 = decode(&stream, &mut decoded).unwrap();
            assert_eq!(info2.width, w);
            assert_eq!(decoded, expected);
        }
    }

    #[test]
    fn test_lj92_malformed_inputs_do_not_panic() {
        let mut out = Vec::new();
        // Bad segment length (< 2)
        assert!(decode(&[0xFF, 0xD8, 0xFF, 0xC3, 0x00, 0x00, 0xFF, 0xD9], &mut out).is_err());
        // Empty SOS segment
        assert!(decode(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x02], &mut out).is_err());
        // Truncated SOS claiming ns = 2
        assert!(decode(&[0xFF, 0xD8, 0xFF, 0xDA, 0x00, 0x04, 0x02, 0x00], &mut out).is_err());

        // Huffman table containing out-of-range symbol > 16 plus valid SOF3/SOS: must not panic
        let mut counts = [0u8; 16];
        counts[0] = 1; // 1-bit code -> symbol 25 (corrupt)
        let h = Huffman::new(&counts, vec![25u8]);
        let mut bits = Bits {
            d: &[0x00, 0x00, 0x00, 0x00],
            p: 0,
            acc: 0,
            n: 0,
        };
        assert_eq!(bits.next_diff(&h), 0);

        // Overflowing code count in LUT length <= 12 must not slice panic
        let bad_counts = [255u8; 16];
        let _ = Huffman::new(&bad_counts, vec![0u8; 255]);
    }
}
