//! The shipped part executables, unpacked to the image DOS would run.
//!
//! The released demo's parts are the EXEs in `MAIN/DATA/` (`MAIN/PACKFINA.INC` packs them into
//! `SECOND.EXE`), and every one but U2A's and U2E's is compressed, often twice: an outer
//! **PKLITE 1.14** layer and, under it, **LZEXE 0.91** (ALKU, ENDSCRL, PLZPART) or Microsoft
//! **EXEPACK** (ten more). [`unpack`] peels layer after layer until none is left and returns the
//! load image with its relocations and entry registers — the bytes the part's own code and data
//! are, which is where the release's text, tables and pictures live when the tree's sources do
//! not hold them (`docs/fidelity/APPROXIMATIONS.md` A10, A11).
//!
//! Each decoder is a transliteration of the stub that ships in front of its data, not of a
//! format description: the PKLITE small and large decoders read their Huffman tables out of the
//! stub itself (at the stub's own `cs:` offsets), and the LZEXE and EXEPACK loops follow their
//! stubs' register use. The bit readers refill **eagerly** — the stubs reload the 16-bit word
//! the moment its last bit is taken (`shr bp,1; dec dx; jz reload`), before the byte the bit
//! announced is read — and getting that order wrong desynchronises the byte stream within a
//! word.
//!
//! What a layer cannot read is refused, never guessed: an unknown PKLITE stub, a back-reference
//! before the start of the output, a stream that runs off the end of the file. The images were
//! checked against the emulator's own loads: each EXE run alone under DOS unpacks itself, exits
//! on `ERROR: DIS not loaded`, and leaves its image in memory, which matched this module's
//! output with its relocations applied except for the C runtime's startup writes
//! (`docs/fidelity/release-images.md`).

use crate::{AssetError, Format};

/// A program as DOS would hand it control: the load image (relative to its load segment), the
/// offsets of the words the loader adds the load segment to, and the entry registers
/// (segments relative to the load segment).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadImage {
    pub image: Vec<u8>,
    pub relocations: Vec<u32>,
    pub cs: u16,
    pub ip: u16,
    pub ss: u16,
    pub sp: u16,
    /// The layers peeled, outermost first.
    pub layers: Vec<Packer>,
}

/// A compression layer [`unpack`] knows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Packer {
    /// PKLITE 1.14 without "large" compression.
    PkliteSmall,
    /// PKLITE 1.14 with "large" compression (version word bit 0x2000).
    PkliteLarge,
    Lzexe091,
    Exepack,
}

fn truncated(needed: usize, had: usize) -> AssetError {
    crate::truncated(Format::Exe, needed, had)
}

fn unsupported(what: impl Into<String>) -> AssetError {
    crate::unsupported(Format::Exe, what)
}

fn word(b: &[u8], at: usize) -> Result<u16, AssetError> {
    match (b.get(at), b.get(at + 1)) {
        (Some(&lo), Some(&hi)) => Ok(u16::from_le_bytes([lo, hi])),
        _ => Err(truncated(at + 2, b.len())),
    }
}

/// Read an MZ file into its load image, then peel every packer layer.
pub fn unpack(file: &[u8]) -> Result<LoadImage, AssetError> {
    let mut prog = load_mz(file)?;
    loop {
        let next = if prog.layers.is_empty() && is_pklite(file) {
            Some(pklite(file)?)
        } else {
            match inner_packer(&prog)? {
                Some(Packer::Lzexe091) => Some(lzexe091(&prog)?),
                Some(Packer::Exepack) => Some(exepack(&prog)?),
                _ => None,
            }
        };
        match next {
            Some(mut n) => {
                let mut layers = prog.layers;
                layers.append(&mut n.layers);
                n.layers = layers;
                prog = n;
            }
            None => return Ok(prog),
        }
    }
}

fn load_mz(file: &[u8]) -> Result<LoadImage, AssetError> {
    if file.get(0..2) != Some(b"MZ") {
        return Err(AssetError::UnknownFormat);
    }
    let last = word(file, 2)? as usize;
    let pages = word(file, 4)? as usize;
    let nrel = word(file, 6)? as usize;
    let header = word(file, 8)? as usize * 16;
    let relat = word(file, 0x18)? as usize;
    let size = if last == 0 {
        pages * 512
    } else {
        (pages.max(1) - 1) * 512 + last
    };
    if size > file.len() || header > size {
        return Err(truncated(size, file.len()));
    }
    let mut relocations = Vec::with_capacity(nrel);
    for k in 0..nrel {
        let off = word(file, relat + 4 * k)? as u32;
        let seg = word(file, relat + 4 * k + 2)? as u32;
        relocations.push(seg * 16 + off);
    }
    Ok(LoadImage {
        image: file[header..size].to_vec(),
        relocations,
        ss: word(file, 0x0e)?,
        sp: word(file, 0x10)?,
        ip: word(file, 0x14)?,
        cs: word(file, 0x16)?,
        layers: Vec::new(),
    })
}

fn is_pklite(file: &[u8]) -> bool {
    file.get(0x1e..0x24) == Some(b"PKLITE")
}

/// The stub's bit reader: a 16-bit word shifted out low bit first, the next word loaded as
/// soon as the last bit of this one is taken, and bytes read from the same stream in between.
struct Bits<'a> {
    src: &'a [u8],
    pos: usize,
    word: u16,
    left: u8,
}

impl<'a> Bits<'a> {
    fn new(src: &'a [u8], pos: usize) -> Result<Self, AssetError> {
        let word = word(src, pos)?;
        Ok(Bits {
            src,
            pos: pos + 2,
            word,
            left: 16,
        })
    }

    fn bit(&mut self) -> Result<u16, AssetError> {
        let b = self.word & 1;
        self.word >>= 1;
        self.left -= 1;
        if self.left == 0 {
            self.word = word(self.src, self.pos)?;
            self.pos += 2;
            self.left = 16;
        }
        Ok(b)
    }

    /// `rcl bx,1` with the next bit.
    fn more(&mut self, v: u16) -> Result<u16, AssetError> {
        Ok((v << 1) | self.bit()?)
    }

    /// `n` bits, the first read the most significant.
    fn bits(&mut self, n: u8) -> Result<u16, AssetError> {
        (0..n).try_fold(0, |v, _| self.more(v))
    }

    fn byte(&mut self) -> Result<u8, AssetError> {
        let b = *self
            .src
            .get(self.pos)
            .ok_or_else(|| truncated(self.pos + 1, self.src.len()))?;
        self.pos += 1;
        Ok(b)
    }

    fn word(&mut self) -> Result<u16, AssetError> {
        let w = word(self.src, self.pos)?;
        self.pos += 2;
        Ok(w)
    }
}

fn copy_back(out: &mut Vec<u8>, dist: usize, count: usize) -> Result<(), AssetError> {
    if dist == 0 || dist > out.len() {
        return Err(unsupported(format!(
            "a back-reference {dist} bytes behind output position {}",
            out.len()
        )));
    }
    for _ in 0..count {
        out.push(out[out.len() - dist]);
    }
    Ok(())
}

/// PKLITE 1.14's decoder, from its stub at `header + 0x42` (the image's `0x142`, copied to its
/// own segment's offset 0 before it runs): `fd 8c db 53 83 c3 NN` is `std; mov bx,ds; push bx;
/// add bx,NN`, and `PSP + NN` paragraphs is where the compressed stream starts.
fn pklite(file: &[u8]) -> Result<LoadImage, AssetError> {
    let header = word(file, 8)? as usize * 16;
    let version = word(file, 0x1c)?;
    let large = version & 0x2000 != 0;
    if version & 0x0fff != 0x010e || version & 0x1000 != 0 {
        return Err(unsupported(format!(
            "PKLITE version word {version:#06x}, not 1.14 without extra compression"
        )));
    }
    let dec = header + 0x42;
    if file.get(dec..dec + 6) != Some(&[0xfd, 0x8c, 0xdb, 0x53, 0x83, 0xc3][..]) {
        return Err(unsupported(
            "a PKLITE stub whose decoder is not at header + 0x42",
        ));
    }
    let start_para = *file
        .get(dec + 6)
        .ok_or_else(|| truncated(dec + 7, file.len()))? as usize;
    if start_para < 0x10 {
        return Err(unsupported("a PKLITE stream that starts inside the PSP"));
    }
    let table = |off: usize| -> Result<u16, AssetError> {
        file.get(dec + off)
            .map(|&b| b as u16)
            .ok_or_else(|| truncated(dec + off + 1, file.len()))
    };
    let (escape, off_table) = if large { (0x19, 0x22d) } else { (0x0a, 0x171) };

    let mut r = Bits::new(file, header + (start_para - 0x10) * 16)?;
    let mut out = Vec::new();
    loop {
        if r.bit()? == 0 {
            out.push(r.byte()?);
            continue;
        }
        let mut bx = r.bits(2)?;
        let mut count = if !large {
            if bx != 0 {
                bx = r.more(bx)?;
                if bx >= 6 {
                    bx = r.more(bx)?;
                }
            }
            table(0x161 + bx as usize)?
        } else if bx >= 2 {
            bx
        } else {
            bx = r.more(bx)?;
            if bx == 0 {
                table(0x207)?
            } else {
                bx = r.more(bx)?;
                if bx >= 5 {
                    bx = r.more(bx)?;
                }
                if bx <= 12 {
                    table(0x207 + bx as usize)?
                } else {
                    bx = r.more(bx & 3)?;
                    if bx >= 5 {
                        bx = r.more(bx)?;
                    }
                    if bx <= 12 {
                        table(0x212 + bx as usize)?
                    } else {
                        bx = r.more(bx & 3)?;
                        if bx >= 5 {
                            bx = r.more(bx)?;
                        }
                        table(0x21d + bx as usize)?
                    }
                }
            }
        };
        if count == escape {
            let extra = r.byte()?;
            count = escape + extra as u16;
            if extra == 0xff {
                break;
            }
            // The large stub's 0xFE is a segment step: it renormalises its pointers and
            // copies nothing.
            if large && extra == 0xfe {
                continue;
            }
        }
        let mut high = 0u16;
        if count != 2 && r.bit()? == 0 {
            let mut v = r.bits(3)?;
            high = if v < 2 {
                table(off_table + v as usize)?
            } else {
                v = r.more(v)?;
                if v < 8 {
                    table(off_table + v as usize)?
                } else {
                    v = r.more(v)?;
                    if v < 0x17 {
                        table(off_table + v as usize)?
                    } else {
                        r.more(v)? & 0xdf
                    }
                }
            };
        }
        let dist = (high << 8 | r.byte()? as u16) as usize;
        copy_back(&mut out, dist, count as usize)?;
    }
    let mut relocations = Vec::new();
    loop {
        let n = r.byte()?;
        if n == 0 {
            break;
        }
        let seg = r.word()? as u32;
        for _ in 0..n {
            relocations.push(seg * 16 + r.word()? as u32);
        }
    }
    let (ss, sp, cs, ip) = (r.word()?, r.word()?, r.word()?, r.word()?);
    Ok(LoadImage {
        image: out,
        relocations,
        cs,
        ip,
        ss,
        sp,
        layers: vec![if large {
            Packer::PkliteLarge
        } else {
            Packer::PkliteSmall
        }],
    })
}

/// Which packer's stub sits at the program's entry, if any. A program with relocations of its
/// own is the program itself; both packers here leave none in the layer above them.
fn inner_packer(p: &LoadImage) -> Result<Option<Packer>, AssetError> {
    if !p.relocations.is_empty() {
        return Ok(None);
    }
    let base = p.cs as usize * 16;
    let Some(head) = p.image.get(base..base + 16) else {
        return Ok(None);
    };
    if &head[14..16] == b"RB" {
        return Ok(Some(Packer::Exepack));
    }
    if p.ip == 0x0e && p.image[base..].windows(5).any(|w| w == b"*FAB*") {
        return Ok(Some(Packer::Lzexe091));
    }
    Ok(None)
}

/// LZEXE 0.91: a 16-byte header at the entry segment (`ip, cs, sp, ss, packed paragraphs, …`),
/// the packed image from offset 0, and a delta-coded relocation table at the stub's `0x158`.
fn lzexe091(p: &LoadImage) -> Result<LoadImage, AssetError> {
    let base = p.cs as usize * 16;
    let h = |k: usize| word(&p.image, base + 2 * k);
    let (ip, cs, sp, ss, packed) = (h(0)?, h(1)?, h(2)?, h(3)?, h(4)? as usize * 16);
    let src = p
        .image
        .get(..packed)
        .ok_or_else(|| truncated(packed, p.image.len()))?;
    let mut r = Bits::new(src, 0)?;
    let mut out = Vec::new();
    loop {
        if r.bit()? == 1 {
            out.push(r.byte()?);
            continue;
        }
        let (dist, count) = if r.bit()? == 1 {
            let ax = r.word()?;
            let dist = 0x2000 - ((ax & 0xff) | ((ax >> 11) << 8)) as usize;
            let n = (ax >> 8) & 7;
            if n != 0 {
                (dist, n as usize + 2)
            } else {
                match r.byte()? {
                    0 => break,
                    1 => continue,
                    n => (dist, n as usize + 1),
                }
            }
        } else {
            let n = r.bits(2)? as usize + 2;
            (0x100 - r.byte()? as usize, n)
        };
        copy_back(&mut out, dist, count)?;
    }
    let mut at = base + 0x158;
    let byte = |at: usize| {
        p.image
            .get(at)
            .copied()
            .ok_or_else(|| truncated(at + 1, p.image.len()))
    };
    let (mut seg, mut off, mut relocations) = (0u32, 0u32, Vec::new());
    loop {
        let mut span = byte(at)? as u32;
        at += 1;
        if span == 0 {
            span = word(&p.image, at)? as u32;
            at += 2;
            if span == 0 {
                seg += 0xfff;
                continue;
            }
            if span == 1 {
                break;
            }
        }
        off += span;
        seg += off >> 4;
        off &= 0xf;
        relocations.push(seg * 16 + off);
    }
    Ok(LoadImage {
        image: out,
        relocations,
        cs,
        ip,
        ss,
        sp,
        layers: vec![Packer::Lzexe091],
    })
}

/// EXEPACK: an `RB` header at the entry segment (`ip, cs, mem_start, exepack_size, sp, ss,
/// dest_len`), the image unpacked backwards from the byte before it (trailing `0xFF` padding
/// skipped, at most 16), and sixteen relocation groups at the stub's `0x132`. The bytes the
/// commands never reach stay where they are.
fn exepack(p: &LoadImage) -> Result<LoadImage, AssetError> {
    let base = p.cs as usize * 16;
    let h = |k: usize| word(&p.image, base + 2 * k);
    let (ip, cs, sp, ss, dest) = (h(0)?, h(1)?, h(4)?, h(5)?, h(6)? as usize * 16);
    let img = &p.image;
    let mut s = base
        .checked_sub(1)
        .ok_or_else(|| unsupported("an EXEPACK header at offset 0"))?;
    let mut pad = 0;
    while s > 0 && img[s] == 0xff && pad < 16 {
        s -= 1;
        pad += 1;
    }
    let mut out = vec![0u8; dest];
    let mut t = dest;
    let under = || unsupported("an EXEPACK command stream that runs before the image");
    loop {
        // `s` wraps below zero after the last byte is consumed; `s >= img.len()` catches that.
        if s < 2 || s >= img.len() {
            return Err(under());
        }
        let cmd = img[s];
        let n = u16::from_le_bytes([img[s - 2], img[s - 1]]) as usize;
        s = s.wrapping_sub(3);
        if n > t {
            return Err(unsupported("an EXEPACK run longer than the output left"));
        }
        match cmd & 0xfe {
            0xb0 => {
                let v = *img.get(s).ok_or_else(under)?;
                s = s.wrapping_sub(1);
                out[t - n..t].fill(v);
            }
            0xb2 => {
                let end = s.wrapping_add(1);
                if n > end {
                    return Err(under());
                }
                out[t - n..t].copy_from_slice(&img[end - n..end]);
                s = s.wrapping_sub(n);
            }
            other => return Err(unsupported(format!("EXEPACK command {other:#04x}"))),
        }
        t -= n;
        if cmd & 1 != 0 {
            break;
        }
    }
    if s.wrapping_add(1) != t {
        return Err(unsupported(
            "an EXEPACK stream whose untouched head does not sit in place",
        ));
    }
    out[..t].copy_from_slice(&img[..t]);
    let mut at = base + 0x132;
    let mut relocations = Vec::new();
    for group in 0..16u32 {
        let n = word(img, at)?;
        at += 2;
        for _ in 0..n {
            relocations.push(group * 0x10000 + word(img, at)? as u32);
            at += 2;
        }
    }
    Ok(LoadImage {
        image: out,
        relocations,
        cs,
        ip,
        ss,
        sp,
        layers: vec![Packer::Exepack],
    })
}
