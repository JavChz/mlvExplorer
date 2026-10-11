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
                    let start = (c << shift) as usize;
                    let end = start + (1usize << shift);
                    for e in &mut h.lut[start..end.min(1 << LUT_BITS)] {
                        *e = ((l as u16) << 8) | sym;
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
    fn fill(&mut self) {
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
        self.acc <<= k;
        self.n -= k;
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
            s => {
                let v = self.peek(s) as i32;
                self.consume(s);
                if v < (1 << (s - 1)) {
                    v - (1 << s) + 1
                } else {
                    v
                }
            }
        }
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
        if marker == 0xD8 || (0xD0..=0xD7).contains(&marker) || marker == 0x01 {
            continue;
        }
        if marker == 0xD9 {
            return Err("LJ92: EOI before scan".into());
        }
        if p + 2 > data.len() {
            return Err("LJ92: truncated segment".into());
        }
        let len = u16::from_be_bytes([data[p], data[p + 1]]) as usize;
        let seg = &data[p + 2..(p + len).min(data.len())];
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
                let ns = (seg[0] as usize).min(4);
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
    out.clear();
    out.resize(row * height, 0);
    let mut bits = Bits { d: scan, p: 0, acc: 0, n: 0 };
    let initial = 1i32 << (precision as i32 - pt as i32 - 1).max(0);

    for y in 0..height {
        let base = y * row;
        for x in 0..width {
            for c in 0..nc {
                bits.fill();
                let ssss = bits.decode(huff[c]);
                let diff = bits.diff(ssss);
                let i = base + x * nc + c;
                let pred = if y == 0 {
                    if x == 0 {
                        initial
                    } else {
                        out[i - nc] as i32
                    }
                } else if x == 0 {
                    out[i - row] as i32
                } else {
                    let ra = out[i - nc] as i32;
                    let rb = out[i - row] as i32;
                    let rc = out[i - row - nc] as i32;
                    match predictor {
                        1 => ra,
                        2 => rb,
                        3 => rc,
                        4 => ra + rb - rc,
                        5 => ra + ((rb - rc) >> 1),
                        6 => rb + ((ra - rc) >> 1),
                        7 => (ra + rb) >> 1,
                        _ => ra,
                    }
                };
                out[i] = (pred + diff) as u16;
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
