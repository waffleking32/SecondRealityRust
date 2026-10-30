//! The Origin PIC family: the container behind `.UH`, `.U`, `.UP`, `.P`, `.16` and `.X`.
//!
//! Five magic-carrying variants share one 16-byte little-endian header (a sixth, headerless
//! case is [`crate::raw_surface`]'s):
//!
//! | first two bytes | variant | palette | pixels |
//! |---|---|---|---|
//! | `FC FC` | raw 8-bit (`GRAB/LBM2U.C`) or bit-planes (`GRAB/LBM16.C`) | `colors*3` at 16 | `para*16` |
//! | `FD FC` | RLE (`GRAB/LBM2P.C`) | `colors*3` at 16 | `para*16` |
//! | `55 48` | `UH` (`GRAB/GLVP.C:961-986`) | none | 16 |
//! | `55 68` | `Uh` (`GRAB/GLVP.C:988-1014`) | 768 at 16 | 784 |
//!
//! `para` is `(16 + colors*3 + 15) / 16` (`LBM2U.C:57`); the GLVP writers hardcode 1 and 49.
//! The palette is six bits already and copied verbatim (`LBM2U.C:170`). The rules:
//!
//! - **An RLE row may decode wider than the header's width.** `START/HZPIC.P` does it on 153 of
//!   400 rows; the original overflows its `char rowbuf[640]` (`START/MAIN.C:13`), the port keeps
//!   the first `w` pixels. APPROXIMATIONS A2.
//! - **Run opcode `0x80` is refused.** `ENDPIC/READP.C:44-51` copies 256 pixels for it,
//!   `START/READP.C:37-38` none, and the writer never emits it.
//! - A row that decodes *shorter* than `w` is zero-filled; the writers cannot produce one
//!   (`LBM2P.C:81`).
//! - **A bit-plane payload's plane count comes from its length** (`LBM16.C:61` hardcodes
//!   `colors=16`), so a `FC FC` payload cut to a whole smaller number of planes decodes as a
//!   smaller-depth picture; nothing in the bytes can tell it apart. Only a payload that is no
//!   whole number of planes is refused.

use sr_engine::{Palette, Surface};

use crate::{check_geometry, le16 as word, six_bit_palette, AssetError, Format, Image};

const FORMAT: Format = Format::Origin;

/// `FC FC`, raw 8-bit or bit-planes (`LBM2U.C:58`, `LBM16.C:63`).
const MAGIC_RAW: u16 = 0xFCFC;
/// `FD FC` as stored — the discriminator is byte 0 (`LBM2P.C:61`).
const MAGIC_RLE: u16 = 0xFCFD;
/// `55 48`, `saveunpheader` (`GLVP.C:968`).
const MAGIC_UH: u16 = 0x4855;
/// `55 68`, `saveunpheaderpal` (`GLVP.C:995`).
const MAGIC_UH_PAL: u16 = 0x6855;

const HEADER: usize = 16;

fn truncated(needed: usize, had: usize) -> AssetError {
    crate::truncated(FORMAT, needed, had)
}

/// Decode the Origin family, dispatching on the magic and on nothing else.
pub(crate) fn decode(bytes: &[u8]) -> Result<Image, AssetError> {
    if bytes.len() < HEADER {
        return Err(truncated(HEADER, bytes.len()));
    }
    match word(bytes, 0) {
        MAGIC_RAW | MAGIC_RLE => decode_fcfc(bytes),
        MAGIC_UH | MAGIC_UH_PAL => decode_glvp(bytes),
        // No fallback to "bare": a file with no magic has no geometry.
        _ => Err(AssetError::UnknownFormat),
    }
}

/// `FC FC` and `FD FC`: `w`, `h`, `colors`, `para` in words 1..4, a `colors*3`
/// palette at 16, and pixels at `para*16`.
fn decode_fcfc(bytes: &[u8]) -> Result<Image, AssetError> {
    let width = word(bytes, 2) as u32;
    let height = word(bytes, 4) as u32;
    let colors = word(bytes, 6);
    let para = word(bytes, 8) as usize;
    let rle = word(bytes, 0) == MAGIC_RLE;

    if colors == 0 || colors > 256 {
        return Err(AssetError::Palette {
            what: format!("the header's colour count is {colors}, outside 1..=256"),
        });
    }
    check_geometry(Format::Origin, width, height)?;

    let palette_len = colors as usize * 3;
    if HEADER + palette_len > bytes.len() {
        return Err(truncated(HEADER + palette_len, bytes.len()));
    }
    // Every writer's `para` clears the palette (`LBM2U.C:57`).
    let pixels_at = para * 16;
    if pixels_at < HEADER + palette_len {
        return Err(unsupported(format!(
            "the header puts the pixels at {pixels_at}, inside the {palette_len}-byte palette \
             that starts at {HEADER}"
        )));
    }
    let palette = read_palette(&bytes[HEADER..HEADER + palette_len])?;

    // `para` is authoritative (`LBM2U.C:67` pads to it with 'X'); one past the end of the
    // file shows up as a payload that is not the geometry's length.
    render(bytes, width, height, pixels_at, palette, rle)
}

/// `UH` and `Uh`: `w` and `h` in words 2 and 3; the palette, when there is one,
/// at 16 and the pixels at word 1's paragraph offset.
///
/// Word 1 is the constant `1` from `saveunpheader` (`GLVP.C:969`) and `49` from
/// `saveunpheaderpal` (`GLVP.C:996`); anything else is refused.
fn decode_glvp(bytes: &[u8]) -> Result<Image, AssetError> {
    let word1 = word(bytes, 2);
    let width = word(bytes, 4) as u32;
    let height = word(bytes, 6) as u32;
    let with_palette = word(bytes, 0) == MAGIC_UH_PAL;

    let (pixels_at, palette) = if with_palette {
        if word1 != 49 {
            return Err(unsupported(format!(
                "the `Uh` container's word 1 is {word1}; GLVP.C:996 writes 49 and no other \
                 value exists in the tree"
            )));
        }
        if HEADER + 768 > bytes.len() {
            return Err(truncated(HEADER + 768, bytes.len()));
        }
        (word1 as usize * 16, read_palette(&bytes[HEADER..HEADER + 768])?)
    } else {
        if word1 != 1 {
            return Err(unsupported(format!(
                "the `UH` container's word 1 is {word1}; GLVP.C:969 writes 1 and no other \
                 value exists in the tree"
            )));
        }
        (HEADER, Palette::new())
    };

    check_geometry(Format::Origin, width, height)?;
    render(bytes, width, height, pixels_at, palette, false)
}

fn unsupported(what: impl Into<String>) -> AssetError {
    crate::unsupported(FORMAT, what)
}

/// `colors` six-bit triples (`LBM2U.C:170-171`); entries past them stay black (`DDSTARS/TEXTS.16`
/// carries 16).
fn read_palette(bytes: &[u8]) -> Result<Palette, AssetError> {
    let mut palette = Palette::new();
    six_bit_palette(bytes, 0, &mut palette)?;
    Ok(palette)
}

/// The payload from `pixels_at`: raw 8-bit, bit-planes, or RLE, whichever the
/// length fits.
fn render(
    bytes: &[u8],
    width: u32,
    height: u32,
    pixels_at: usize,
    palette: Palette,
    rle: bool,
) -> Result<Image, AssetError> {
    let (w, h) = (width as usize, height as usize);
    let payload = bytes.len().checked_sub(pixels_at).ok_or_else(|| {
        truncated(pixels_at + w * h, bytes.len())
    })?;

    let mut surface = Surface::new(w, h);
    if rle {
        decode_rows(bytes, pixels_at, &mut surface)?;
        return Ok(Image { surface, palette });
    }

    if payload == w * h {
        surface
            .pixels_mut()
            .copy_from_slice(&bytes[pixels_at..pixels_at + w * h]);
        return Ok(Image { surface, palette });
    }

    // `LBM16.C:74-93` is the only writer of bit-planes, always as `FC FC` (four planes, or
    // the two of `TEXTS.16`, `DDSTARS/DOTEXTS.BAT:1`). A GLVP file is raw by construction,
    // so a short one is a truncation, not planes.
    if word(bytes, 0) == MAGIC_RAW {
        let plane_row = w.div_ceil(8);
        let per_row = h * plane_row;
        if per_row > 0 && payload % per_row == 0 {
            let planes = payload / per_row;
            if (1..=8).contains(&planes) {
                decode_planes(bytes, pixels_at, &mut surface, planes);
                return Ok(Image { surface, palette });
            }
        }
    }

    Err(truncated(pixels_at + w * h, bytes.len()))
}

/// `LBM16.C:74-93`: per row, `planes` bit-planes of `w/8` bytes each, plane 0
/// (the low colour bit) first, the most significant bit of a byte the leftmost
/// pixel of its group of eight.
fn decode_planes(bytes: &[u8], pixels_at: usize, surface: &mut Surface, planes: usize) {
    let w = surface.width();
    let row_bytes = w.div_ceil(8);
    let mut p = pixels_at;
    for y in 0..surface.height() {
        let row = surface.row_mut(y);
        for plane in 0..planes {
            for xb in 0..row_bytes {
                let byte = bytes[p];
                p += 1;
                for bit in 0..8usize {
                    let x = xb * 8 + bit;
                    if x >= w {
                        break;
                    }
                    if byte & (0x80 >> bit) != 0 {
                        row[x] |= 1 << plane;
                    }
                }
            }
        }
    }
}

/// `LBM2P.C:72-94` / `ENDPIC/READP.C:10-41`: each row is a `u16` count of the
/// bytes that follow, then that many bytes of opcode stream. An opcode below
/// `0x80` is one literal pixel; `0x80` and above are a run, the next byte
/// repeated `opcode & 0x7F` times.
/// The row-length prefix is authoritative; pixels past `w` are counted and dropped (A2).
fn decode_rows(bytes: &[u8], pixels_at: usize, surface: &mut Surface) -> Result<(), AssetError> {
    let w = surface.width();
    let mut p = pixels_at;
    for y in 0..surface.height() {
        if p + 2 > bytes.len() {
            return Err(truncated(p + 2, bytes.len()));
        }
        let n = word(bytes, p) as usize;
        p += 2;
        if p + n > bytes.len() {
            return Err(truncated(p + n, bytes.len()));
        }
        let end = p + n;
        let row = surface.row_mut(y);
        let mut x = 0;
        while p < end {
            let op = bytes[p];
            p += 1;
            if op < 0x80 {
                if x < w {
                    row[x] = op;
                }
                x += 1;
            } else {
                let run = (op & 0x7F) as usize;
                if run == 0 {
                    return Err(unsupported(
                        "RLE run opcode 0x80: ENDPIC/READP.C:44-51 copies 256 pixels, \
                         START/READP.C:37-38 none, and LBM2P.C never writes one",
                    ));
                }
                if p >= end {
                    return Err(truncated(p + 1, end));
                }
                let value = bytes[p];
                p += 1;
                for _ in 0..run {
                    if x < w {
                        row[x] = value;
                    }
                    x += 1;
                }
            }
        }
        p = end;
    }
    Ok(())
}
