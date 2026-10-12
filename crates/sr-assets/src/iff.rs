//! IFF, in both flavours the tree ships: `FORM .... ILBM` (planar — a row per
//! bitplane) and `FORM .... PBM ` (chunky — a row per scanline).
//!
//! **No shipped part loads an `.lbm` at runtime.** Every one of them is source
//! art the `GRAB/` toolchain converts first — `CREDITS/PICS/SFD.BAT:1` runs
//! `..\lbm2u.exe pic01.lbm pic01.uh`, `START/DOHZ.BAT:1` runs
//! `lbm2p hzpic.lbm hzpic.p`, `TECHNO/DOPIC.BAT:1` and `LENS/DOOBJS.BAT:2` the
//! same for theirs — so the original this decoder has to agree with is that
//! toolchain's reader: `GRAB/LOADLBM.C`, `GRAB/LBM2U.C:161-336` and
//! `GRAB/VIEWLBM.C:148-323`, three copies of one `loadlbm2`, all of them
//! `ByteRun1` with the same row loop.
//!
//! The container is **big-endian**: `getaw` reads `getc(f1)*256 + getc(f1)`
//! (`LOADLBM.C:84-90`), and that is what makes `BEG/SRTITLE.LBM`'s header bytes
//! `01 40 01 90` come out as the 320x400 the picture really is. A decoder that
//! read them little-endian would put 16,385 x 36,865 in the same file and be
//! refused by [`check_geometry`](crate::check_geometry) — which is why the
//! geometry here is pinned by a test against the file's own pixels.
//!
//! Four things these writers do that a decoder written from the IFF spec alone
//! gets wrong, each one measured over the tree:
//!
//! - **A chunk size can include the pad byte.** Eight of the curated `ILBM`
//!   files carry a `CMAP` one byte longer than its entries need — 334 bytes for
//!   111 triples, the extra byte being the pad that makes the chunk even — and
//!   thirteen carry a `BODY` whose row stream ends one byte before the chunk
//!   does. Both writers made the chunk even by counting the pad byte in the
//!   size. So the palette is `size / 3` entries and a leftover byte is padding,
//!   and the row stream may leave a byte unread: a decoder that refused a
//!   `CMAP` whose length is not a multiple of three would refuse eight of the
//!   tree's files, and one that demanded the stream fill the `BODY` exactly
//!   would refuse thirteen. The `FORM` size, by contrast, is exact — `size + 8`
//!   equals the file length for all 37 curated files, byte for byte — and bounds
//!   every read below, and the chunks have to fill it end to end: the walk
//!   refuses a `FORM` whose bytes are not all accounted for by chunks.
//! - **A row ends when it is full, not when its opcodes run out.** The
//!   original's loop is `while(lp < xsz)` (`LOADLBM.C:161-179` for chunky,
//!   `197-232` for planar), checked between opcodes; a run that overshoots is
//!   consumed whole and the excess written past the row, where the
//!   `memcpy(pic + y*320, linebuf, xsz)` of `LOADLBM.C:181` never reads it. The
//!   port consumes the opcode and drops the excess, which is what that copy
//!   leaves visible. (No curated file overshoots: all 37 rows' streams land on
//!   their row bounds.)
//! - **The palette is 8-bit and is truncated with `/4`.** Same line in five
//!   tools: `LBM2U.C:170`, `LBM2P.C:195`, `LBM16.C:194`, `VIEWLBM.C:157` and
//!   `VPL.C:97` all read `a = getc(f1)/4` — the 8->6 conversion the original
//!   actually used, which is why the `.UH`/`.UP`/`.16`/`.EXB` files it wrote
//!   hold those values. [`crate::six_bit`] is the *display* rule's inverse
//!   ([`dac_expand`](sr_engine::dac_expand) widens 6->8) and a different
//!   function: it disagrees with `/4` on 42 of the 256 values — every `4k+3` up
//!   to 83, where it rounds up by one, and every fourth value from 172 to 252,
//!   where it rounds down by one — so it would move the tree's converted
//!   palettes by one 6-bit step on those bytes. The twins agree with `/4` byte
//!   for byte, and `tests/iff.rs` pins the palette against them, 768 bytes at a
//!   time.
//! - **A truncated stream is an error, not a hang.** The original's `getc`
//!   returns `EOF`, which `if(a > 127)` reads as a literal of length 0 and
//!   loops on forever; every read here is bounded by the `BODY`'s end and
//!   reports [`AssetError::Truncated`] instead.
//!
//! `masking` (a stencil/mask plane), `CAMG & 0x80` (EHB) and `CAMG & 0x800`
//! (HAM), and a `compression` other than 1 are refused with
//! [`AssetError::Unsupported`] rather than guessed at. **No file in either tree
//! needs any of them**: measured over the curated 37 and the 86 upstream IFF
//! files, every `masking` byte is 0 in the curated tree (`10` upstream files
//! carry a mask plane — `masking` 1 or 2 — and none of them was curated), every
//! `compression` byte is 1 in both trees, and **no file anywhere carries a
//! `CAMG` chunk at all**. A mask plane is the one that has to be refused rather
//! than skipped: it is decoded into the row stream, so ignoring it would read
//! the next row out of the mask's bytes.
//!
//! The tree's asset of record for the whole decoder is `BEG/SRTITLE.LBM`: 320 x
//! 400, chunky, and the same picture as the RLE-packed `BEG/SRTITLE.UP` the
//! BEGLOGO part links in (`BEG/MAKEFILE:21` — 128,000 pixels, no difference),
//! which is the cross-format test in `tests/iff.rs`, and `VISU/C/U2ABG.LBM`
//! against its `.UH` for the planar half.

use sr_engine::{Palette, Surface};

use crate::{check_geometry, AssetError, Format, Image};

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
    AssetError::Truncated {
        format: Format::Iff,
        needed,
        had,
    }
}

fn unsupported(what: String) -> AssetError {
    AssetError::Unsupported {
        format: Format::Iff,
        what,
    }
}

/// Decode one `ILBM` or `PBM ` file. Reached through [`crate::load`], which is
/// the crate's door for every family.
pub(crate) fn decode(bytes: &[u8]) -> Result<Image, AssetError> {
    if bytes.len() < HEAD {
        return Err(truncated(HEAD, bytes.len()));
    }
    if &bytes[0..4] != b"FORM" {
        return Err(AssetError::UnknownFormat);
    }
    // The FORM's size is the file's length less the eight bytes `FORM` and the
    // size field take: exact for every curated file (measured). It bounds every
    // chunk read below, so a cut file is refused before a chunk is looked at —
    // which is also what keeps the chunk walk from ever reading past its input.
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
        // An odd chunk is padded to even, and these writers count the pad byte
        // in the size rather than leaving it out (see the module docs) — either
        // way the next chunk starts after it.
        at = stop + (size as usize & 1);
    }
    // The `FORM`'s bytes have to be chunks, end to end. `at` lands past `end`
    // when the last chunk is odd-sized and the file carries no final pad byte
    // for it, and short of `end` when bytes follow the last chunk; it lands
    // exactly on `end` for every file in either tree (measured). Anything else
    // means the size field and the content disagree, and guessing at the rest
    // would invent a picture.
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

    // The refusals, all of them before a byte of the picture is read.
    //
    // `masking` is 0 in every curated file. A nonzero value means a mask plane
    // is stored in each row after the colour planes (the original reads and
    // discards it, `LOADLBM.C:234-251`), so a decoder that ignored the byte —
    // which is what `masking` looks like if you only read the field — would take
    // the mask's bytes for the next row's pixels. Upstream has ten such files
    // (`masking` 1 and 2); none was curated.
    if masking != 0 {
        return Err(unsupported(format!(
            "masking is {masking}: a stencil/mask plane follows each row's colour planes, and no \
             curated file carries one"
        )));
    }
    // Deluxe Paint writes 1 (ByteRun1) and nothing else: every `compression`
    // byte in the curated tree and upstream is 1. The original never reads the
    // field as such — `LOADLBM.C:132` takes it and its pad byte together as the
    // misnamed `colors` — and byte-run-decodes whatever it finds, which is only
    // safe because 1 is what is there.
    if compression != 1 {
        return Err(unsupported(format!(
            "compression is {compression}; the tree's files are all ByteRun1 (1)"
        )));
    }
    // No file in either tree carries a CAMG chunk. Its two colour-changing bits
    // are refused anyway: EHB halves the palette's intensity at render time, and
    // HAM builds colours per pixel out of the previous one — both make the index
    // stored here not the colour shown, which this decoder cannot represent.
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
    // Planes are only consulted for the planar flavour, as in the original: its
    // chunky branch never reads the count. Eight is the most an indexed byte can
    // name, and the tree's files are 6, 7 and 8 (curated) or 4, 6, 7 and 8
    // (upstream).
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

/// The `CMAP`: entry 0 first, three bytes each, from the top.
///
/// `size / 3` entries, because eight of the curated files' palettes are 3n+1
/// bytes long with the last byte the pad that made the chunk even (measured:
/// `CREDITS/PICS/PIC01.LBM` is 334 bytes, 111 triples and a zero). The rest of
/// the 256-entry palette stays black.
///
/// The original instead reads a flat 768 bytes whatever the chunk says
/// (`LOADLBM.C:99-103`), running past the end of the shorter palettes and into
/// the chunk that follows them — so *its* entries beyond the real one hold
/// whatever the picture's own bytes are. Nothing in the tree can tell the two
/// apart, and the reason is worth keeping: twenty-one of the curated files carry
/// fewer than 256 entries, and every one of them uses exactly the indices its
/// palette defines — 34 entries and a highest pixel of 33 for
/// `CREDITS/PICS/PIC18.LBM`, 111 and 110 for `PIC01.LBM`, 240 and 239 for the
/// eight like `PIC05.LBM` (measured over all 37).
///
/// The conversion is `/4`, the original's own: `LBM2U.C:170`, `LBM2P.C:195`,
/// `LBM16.C:194`, `VIEWLBM.C:157` and `VPL.C:97` are five independent copies of
/// `a = getc(f1)/4`, which is how the `.UH`, `.UP`, `.16` and `.EXB` files this
/// tree ships got their six-bit palettes. The tree's converted twins are the
/// evidence and the test: their 768 palette bytes equal the `CMAP` divided by
/// four exactly, for `LENS/LENSPIC.LBM`/`LENS/LENS.EXB` and `BEG/SRTITLE.LBM`/
/// `BEG/SRTITLE.UP` alike, where [`crate::six_bit`] would differ on 457 and 1 of the
/// bytes respectively. (The engine widens the other way at upload through
/// `dac_expand`, which rounds; that is the display rule and not this one.)
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
/// A plane row is `((w + 15) / 16) * 2` bytes. The original has no such width —
/// its loop counts *pixels* (`lp` advances eight per source byte, so it stops
/// after `ceil(w / 8)` bytes) — and the two differ by one byte whenever
/// `ceil(w / 8)` is odd. No file in either tree can tell them apart: every one of
/// the 38 upstream `ILBM` files is a multiple of 16 wide (measured), where both
/// come to `w / 8`. A `PBM ` row has no planes, so the question does not arise
/// there.
///
/// **The bound, said out loud rather than left in the arithmetic:** the two
/// widths agree exactly when `ceil(w / 8)` is even, which is every width either
/// tree holds. At an odd-`ceil` width this reader would take one byte per plane
/// row more than the original's loop does, so such a file would decode
/// *sheared* rather than being refused — the row's length is in no header, and
/// no check could tell the padded reading from the unpadded one. No file of
/// that shape is in either tree; a decoder meeting one would have to choose
/// between the two readings with nothing in the file to choose by.
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
/// longest 129 (`LOADLBM.C:164-170`). The opcode that fills the row is consumed
/// whole and anything it produced past the row's end is dropped — what the
/// original's `memcpy(pic + y*320, linebuf, xsz)` leaves of an overrunning row.
///
/// The stream's end is the `BODY`'s, not the file's: these writers count a pad
/// byte into an odd chunk size, so a row stream that stops one byte short of the
/// chunk is a row stream that filled its rows, not a truncation. A stream that
/// stops *before* the last row is full is [`AssetError::Truncated`], where the
/// original would read `EOF` forever.
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
            // `0x80` is `257 - 128` = 129 and `0xFF` is 2 (`LOADLBM.C:164-170`),
            // so a run is never zero. It can be longer than the row has left —
            // the clamp below is exactly for that.
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
