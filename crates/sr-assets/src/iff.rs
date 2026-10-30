//! IFF, in both flavours the tree ships: `FORM .... ILBM` (planar, a row per bitplane) and
//! `FORM .... PBM ` (chunky, a row per scanline).
//!
//! No shipped part loads an `.lbm` at runtime; the `GRAB/` toolchain converts them first
//! (`CREDITS/PICS/SFD.BAT:1`, `START/DOHZ.BAT:1`), so the reader to agree with is the
//! toolchain's `loadlbm2`: `GRAB/LOADLBM.C`, `GRAB/LBM2U.C:161-336`, `GRAB/VIEWLBM.C:148-323`.
//!
//! - **The container is big-endian** (`getaw`, `LOADLBM.C:84-90`).
//! - **A chunk size can include the pad byte.** Eight curated `CMAP`s are `3n+1` bytes and
//!   thirteen `BODY` row streams end a byte before their chunk, so the palette is `size / 3`
//!   entries and a stream may leave a byte unread. The `FORM` size is exact for every file and
//!   the chunks must fill it end to end.
//! - **A row ends when it is full** (`while(lp < xsz)`, `LOADLBM.C:161-179`, `197-232`): an
//!   overshooting run is consumed whole and its excess dropped, as `LOADLBM.C:181`'s `memcpy`
//!   leaves it.
//! - **The palette is 8-bit and divided by four**, the original's one 8→6 rule: `LBM2U.C:170`,
//!   `LBM2P.C:195`, `LBM16.C:194`, `VIEWLBM.C:157`, `VPL.C:97`. The converted twins
//!   (`.UH`/`.UP`/`.EXB`) agree byte for byte (`tests/iff.rs`).
//! - **A truncated stream is an error**, where the original's `getc` loops on `EOF` forever.
//! - `masking` other than 0, `compression` other than 1 and `CAMG`'s EHB/HAM bits are refused;
//!   no curated file carries any of them (no file anywhere has a `CAMG`). A mask plane must be
//!   refused rather than skipped: it sits in the row stream.

use sr_engine::{Palette, Surface};

use crate::{check_geometry, AssetError, Format, Image};

const FORMAT: Format = Format::Iff;

/// `FORM`, the size that follows it, and the four-byte type.
const HEAD: usize = 12;
/// What `BMHD` is worth reading: w, h, x, y, nPlanes, masking, compression,
/// pad, transparent, xAspect, yAspect, pageW, pageH (`LOADLBM.C:123-136` reads
/// every one of them, though it only *uses* w, h, planes and masking).
const BMHD: usize = 20;

fn be16(bytes: &[u8], at: usize) -> u16 {
    u16::from_be_bytes([bytes[at], bytes[at + 1]])
}

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

fn truncated(needed: usize, had: usize) -> AssetError {
    crate::truncated(FORMAT, needed, had)
}

fn unsupported(what: String) -> AssetError {
    crate::unsupported(FORMAT, what)
}

/// Decode one `ILBM` or `PBM ` file (through [`crate::load`]).
pub(crate) fn decode(bytes: &[u8]) -> Result<Image, AssetError> {
    if bytes.len() < HEAD {
        return Err(truncated(HEAD, bytes.len()));
    }
    if &bytes[0..4] != b"FORM" {
        return Err(AssetError::UnknownFormat);
    }
    // The FORM's size bounds every chunk read below, so a cut file is refused first.
    let end = 8u64 + be32(bytes, 4) as u64;
    if end > bytes.len() as u64 {
        return Err(truncated(end.min(usize::MAX as u64) as usize, bytes.len()));
    }
    let end = end as usize;

    let planar = match &bytes[8..12] {
        b"ILBM" => true,
        b"PBM " => false,
        other => {
            return Err(unsupported(format!(
                "the FORM's type is {:?}; the tree ships only ILBM and PBM",
                String::from_utf8_lossy(other)
            )))
        }
    };

    let mut header = None;
    let mut cmap = None;
    let mut camg = None;
    let mut body = None;
    let mut at = HEAD;
    while at + 8 <= end {
        let id = &bytes[at..at + 4];
        let size = be32(bytes, at + 4) as u64;
        let data = at as u64 + 8;
        let stop = data + size;
        if stop > end as u64 {
            return Err(truncated(stop as usize, bytes.len()));
        }
        let (data, stop) = (data as usize, stop as usize);
        match id {
            b"BMHD" => {
                if size < BMHD as u64 {
                    return Err(unsupported(format!(
                        "the BMHD chunk is {size} bytes; a picture header is {BMHD}"
                    )));
                }
                header = Some(&bytes[data..data + BMHD]);
            }
            b"CMAP" => cmap = Some(&bytes[data..stop]),
            b"CAMG" => {
                if size < 4 {
                    return Err(unsupported(format!(
                        "the CAMG chunk is {size} bytes; a display mode is 4"
                    )));
                }
                camg = Some(be32(bytes, data));
            }
            b"BODY" => body = Some((data, stop)),
            // `CRNG`, `DPPS`, `DPPV`, `TINY` and anything else: these carry
            // Deluxe Paint's colour ranges and tool settings, which change no
            // pixel, and the original skips them (`LOADLBM.C:139-150`).
            _ => {}
        }
        // An odd chunk is padded to even.
        at = stop + (size as usize & 1);
    }
    // The `FORM`'s bytes have to be chunks, end to end, as they are in every file of either tree.
    if at != end {
        return Err(unsupported(format!(
            "the chunk walk ends at byte {at} of a FORM that ends at {end}"
        )));
    }

    let header = header.ok_or_else(|| unsupported("no BMHD chunk: no geometry".to_string()))?;
    let (width, height) = (be16(header, 0) as u32, be16(header, 2) as u32);
    let planes = header[8];
    let masking = header[9];
    let compression = header[10];

    check_geometry(Format::Iff, width, height)?;

    // A mask plane follows each row's colour planes (`LOADLBM.C:234-251`).
    if masking != 0 {
        return Err(unsupported(format!(
            "masking is {masking}: a stencil/mask plane follows each row's colour planes, and no \
             curated file carries one"
        )));
    }
    // `LOADLBM.C:132` never reads the field as such and byte-run-decodes whatever it finds.
    if compression != 1 {
        return Err(unsupported(format!(
            "compression is {compression}; the tree's files are all ByteRun1 (1)"
        )));
    }
    // EHB and HAM make the stored index not the colour shown.
    if let Some(mode) = camg {
        if mode & 0x80 != 0 {
            return Err(unsupported(format!(
                "CAMG is {mode:#x}: the extra-half-brite bit halves the palette, and this decoder \
                 produces indexed pixels"
            )));
        }
        if mode & 0x800 != 0 {
            return Err(unsupported(format!(
                "CAMG is {mode:#x}: the hold-and-modify bit makes a pixel's colour depend on its \
                 neighbour, and this decoder produces indexed pixels"
            )));
        }
    }
    // Only the planar flavour reads the count, as in the original.
    if planar && !(1..=8).contains(&planes) {
        return Err(unsupported(format!(
            "nPlanes is {planes}; ILBM needs 1..=8 to fit an indexed pixel"
        )));
    }

    let (body_at, body_end) =
        body.ok_or_else(|| unsupported("no BODY chunk: the container carries no pixels".to_string()))?;

    let mut surface = Surface::new(width as usize, height as usize);
    if planar {
        decode_planar(bytes, (body_at, body_end), &mut surface, planes)?;
    } else {
        decode_chunky(bytes, (body_at, body_end), &mut surface)?;
    }

    Ok(Image {
        surface,
        palette: read_palette(cmap),
    })
}

/// The `CMAP`, `size / 3` entries divided by four (`LBM2U.C:170`); the rest stay black.
///
/// The original reads a flat 768 bytes whatever the chunk says (`LOADLBM.C:99-103`), running
/// into the next chunk; no file can tell the two apart, because every one of the 21 curated
/// files with fewer than 256 entries uses only the indices its palette defines.
fn read_palette(cmap: Option<&[u8]>) -> Palette {
    let mut palette = Palette::new();
    let Some(cmap) = cmap else {
        return palette;
    };
    for (i, rgb) in cmap.chunks_exact(3).take(256).enumerate() {
        palette.set_entry(i as u8, [rgb[0] / 4, rgb[1] / 4, rgb[2] / 4]);
    }
    palette
}

/// `PBM `: each scanline is one ByteRun1 stream that decodes to `w` bytes, one
/// byte per pixel (`LOADLBM.C:151-185`).
fn decode_chunky(
    bytes: &[u8],
    (at, end): (usize, usize),
    surface: &mut Surface,
) -> Result<(), AssetError> {
    let mut at = at;
    for y in 0..surface.height() {
        at = row_stream(bytes, at, end, surface.row_mut(y))?;
    }
    Ok(())
}

/// `ILBM`: each scanline is `planes` ByteRun1 streams, one per bitplane, plane 0
/// (the low colour bit) first, and the streams are read one after another within
/// the row (`LOADLBM.C:186-256`).
///
/// A plane row is `((w + 15) / 16) * 2` bytes, where the original's pixel-counting loop takes
/// `ceil(w / 8)`; the two differ only when `ceil(w / 8)` is odd, and every `ILBM` in either tree
/// is a multiple of 16 wide. Such a file would decode sheared, not refused.
fn decode_planar(
    bytes: &[u8],
    (at, end): (usize, usize),
    surface: &mut Surface,
    planes: u8,
) -> Result<(), AssetError> {
    let w = surface.width();
    let row_bytes = w.div_ceil(16) * 2;
    let mut plane_row = vec![0u8; row_bytes];
    let mut at = at;
    for y in 0..surface.height() {
        let row = surface.row_mut(y);
        for plane in 0..planes {
            at = row_stream(bytes, at, end, &mut plane_row)?;
            let mask = 1u8 << plane;
            // The byte's most significant bit is the leftmost of its eight
            // pixels, and each plane contributes one bit of the colour
            // (`LOADLBM.C:207-214`).
            for (i, &byte) in plane_row.iter().enumerate() {
                if byte == 0 {
                    continue;
                }
                for bit in 0..8 {
                    let x = i * 8 + bit;
                    if x >= w {
                        break;
                    }
                    if byte & (0x80 >> bit) != 0 {
                        row[x] |= mask;
                    }
                }
            }
        }
    }
    Ok(())
}

/// One row: consume ByteRun1 opcodes from `at` until `out.len()` bytes have been
/// produced, and answer where the next row starts.
///
/// An opcode below `0x80` is that many plus one literal bytes; `0x80` and above
/// are one byte repeated `257 - opcode` times, so the shortest run is 2 and the
/// longest 129 (`LOADLBM.C:164-170`). The stream's end is the `BODY`'s, not the file's.
fn row_stream(bytes: &[u8], mut at: usize, end: usize, out: &mut [u8]) -> Result<usize, AssetError> {
    let target = out.len();
    let mut produced = 0;
    while produced < target {
        if at >= end {
            return Err(truncated(at + 1, end));
        }
        let op = bytes[at];
        at += 1;
        if op > 0x7F {
            let run = 257 - op as usize;
            debug_assert!((2..=129).contains(&run));
            if at >= end {
                return Err(truncated(at + 1, end));
            }
            let value = bytes[at];
            at += 1;
            for _ in 0..run {
                if produced < target {
                    out[produced] = value;
                }
                produced += 1;
            }
        } else {
            for _ in 0..=op {
                if at >= end {
                    return Err(truncated(at + 1, end));
                }
                if produced < target {
                    out[produced] = bytes[at];
                }
                produced += 1;
                at += 1;
            }
        }
    }
    Ok(at)
}
