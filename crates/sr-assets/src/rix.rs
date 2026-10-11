//! RIX3 (ColoRIX): the container behind `.CLX` and `.SCI`.
//!
//! The tree holds no writer for this format — it is an outside paint program's,
//! not the demo's — so the layout is established from the files and from what the
//! parts that read them ask for. Those parts never read a single header field:
//! the palette's offset and the pixels' offset are the only two they use.
//!
//! | reader | what it does |
//! |---|---|
//! | `WATER/KOE.PAS:35-40` | `assign(f,'bkg.clx')`, `seek(f,10)`, `blockread(f,pal,768)`, `setrgb(x,pal[x*3+0],…)` — which writes ports `0x3C8`/`0x3C9`, the DAC — then `blockread(f,mem[$a000:0],64000)` straight to the screen |
//! | `FOREST/READ2.PAS:110-121` | `seek(f,x*640+778)` per row of `o2.sci`, and `move(mem[seg(hback):ofs(hback)+10],pal,768)` for the `hillback.clx` the linker put at `hback` (`FOREST/MAKEOB.BAT:1`) |
//! | `WATER/DEMO.PAS:108-109` | `move(mem[seg(_miekka):ofs(_miekka)+10],pal,768)` and `move(…+778,font,400*34)` |
//!
//! So: a 10-byte header, a 768-byte palette at 10, pixels at 778, one byte per
//! pixel, row-major, `width` bytes to a row.
//!
//! The header is `"RIX3"`, a little-endian `u16` width, a little-endian `u16`
//! height, and then **two bytes that are two fields** — a palette type and a
//! storage type — rather than one `u16`. The split and the value are measured:
//! the palette is at 10 = 4 + 2 + 2 + 1 + 1, a 12-byte header would put it at 12,
//! which is the third byte of the first palette entry in every one of the 11 RIX3
//! files in either tree, and **all four curated files** (all 11 upstream) carry
//! `AF 00` at 8 and 9 while the 768 bytes at 10 are exactly a `.LBM`'s `CMAP / 4`
//! (below).
//!
//! **What those two bytes mean is not in the tree, and is not cited to it.** No
//! reader in the tree touches bytes 4 through 9: `WATER/KOE.PAS:35-40`,
//! `FOREST/READ2.PAS:110-121`, `FOREST/READ.PAS:117-118` and
//! `WATER/DEMO.PAS:108-109` seek to 10 and 778 and to nothing else. So there is
//! no `file:line` anywhere for the field *names*, and none is claimed here. The
//! names come from the format's documentation outside the tree (ColoRIX, the
//! "new" VGA Paint format), which reads byte 8 as the palette type with `0xAF` =
//! VGA (six-bit DAC triples) and byte 9 as the storage type with `0x00` = linear,
//! `0x80` = compressed, `0x40` = extension block, `0x20` = encrypted. On that
//! external reading — and on the measured constant, which is why the refusal
//! cannot reject a file in the tree — a byte 8 other than `0xAF` or a byte 9
//! other than `0x00` is refused with [`AssetError::Unsupported`]: reading one
//! would mean inventing a palette layout or a pixel layout the tree never shows.
//! The parts' own reads are *consistent* with both names — `KOE.PAS:39` hands the
//! 768 bytes to ports `0x3c8`/`0x3c9` untranslated, `KOE.PAS:40` reads the pixels
//! as a flat block — but consistency is not citation, and the refusals below rest
//! on the constant, not on the meaning.
//!
//! **The palette is six bits and is copied verbatim**, like the Origin family's
//! and unlike the IFF `CMAP`'s, which is eight bits and goes through the
//! original's `/4` (`GRAB/LBM2U.C:170`). These bytes are the `/4`'s *output*:
//! four of the eleven RIX3 palettes in the tree are the `/4` of the `CMAP` of a
//! `.LBM` beside them, all 768 bytes of each — `FOREST/HILLBACK.CLX` against
//! `FOREST/FINAL.LBM`, `WATER/BKG.CLX` against `WATER/FINAL2.LBM`,
//! `FOREST/O2.SCI` against `FOREST/FONA2.LBM`, and `WATER/MIEKKA.SCI` against
//! `WATER/SWORD.LBM` (measured upstream, where both sides of each pair live: the
//! curated tree keeps the `.CLX`/`.SCI` side, and `tests/rix.rs` pins the rule
//! against the files' own bytes). A
//! byte above 63 is out of contract and refused, because `Palette::set_entry`
//! would mask it and silently change a colour. [`crate::six_bit`] must not run
//! here: it is the inverse of the engine's 6→8 *display* widening, so over an
//! already-six-bit palette it divides every component by four — and a test that
//! only checked `<= 63` would still pass.
//!
//! **The payload is whole rows of the header's width.** Ten of the tree's eleven
//! RIX3 files carry exactly `width * height` pixel bytes. The eleventh does not:
//! `WATER/MIEKKA.SCI` declares a 400x250 canvas and carries 14,000 pixel bytes,
//! which is 35 whole rows of 400. It is a crop, not a truncation — the cut lands
//! on a row boundary (once in 400 bytes, so the odds against a truncation doing
//! it by accident are 400:1), the payload is the first 14,000 bytes of
//! `WATER/SWORD.SCI`'s pixels with that file's palette, and the part that ships
//! it reads 400*34 of those bytes (`WATER/DEMO.PAS:109`) from an `.OBJ` whose
//! payload is this file byte for byte (`WATER/MAKEOB.BAT:2`). So the height here
//! is the number of rows the file actually carries, and it is `min` of the
//! header's height and the payload's rows — never padded up to the header's
//! canvas, which would put 215 rows of black in a file that does not contain
//! them, and never refused, which would refuse a file the shipped part links and
//! reads. A payload that stops part-way through a row is a truncation and is
//! [`AssetError::Truncated`].
//!
//! Refused, all of them before a pixel is read:
//!
//! | construct | why | does any file need it? |
//! |---|---|---|
//! | palette type other than `0xAF` | the 768 bytes at 10 would not be the DAC triples every reader in the tree assumes (the name is the external reading above, the constant is measured) | no — all 11 files carry `0xAF` |
//! | storage type other than `0x00` | a compressed, planar, extension-block, encrypted or direct-colour layout this decoder does not read | no — all 11 carry `0x00` |
//! | a canvas over the engine's largest surface | `check_geometry`, the same area rule as A2 and A3 | no |
//! | a palette byte above 63 | not a six-bit DAC value | no |
//! | a payload past the canvas, or short of a whole row | the file and its header disagree | no — ten files are exact, one is a whole-row crop |
//!
//! The header's canvas is checked, not the decoded surface: a header that claims
//! more than the engine could ever hold is a broken file even where the payload
//! is small, and refusing it keeps this decoder's bound the same as the other
//! two's.

use sr_engine::{Palette, Surface};

use crate::{check_geometry, AssetError, Format, Image};

/// `"RIX3"`, width, height, palette type, storage type.
const HEADER: usize = 10;
const PALETTE_AT: usize = HEADER;
const PALETTE_LEN: usize = 768;
/// Where every reader in the tree seeks to: `READ2.PAS:114`'s `x*640+778`,
/// `DEMO.PAS:109`'s `+778`.
const PIXELS_AT: usize = PALETTE_AT + PALETTE_LEN;
/// A VGA palette: 768 bytes of six-bit DAC triples.
const PALETTE_TYPE_VGA: u8 = 0xAF;
/// Linear storage: one byte per pixel, row-major.
const STORAGE_LINEAR: u8 = 0x00;

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

fn truncated(needed: usize, had: usize) -> AssetError {
    AssetError::Truncated {
        format: Format::Rix3,
        needed,
        had,
    }
}

fn unsupported(what: String) -> AssetError {
    AssetError::Unsupported {
        format: Format::Rix3,
        what,
    }
}

/// Decode one RIX3 screen. Reached through [`crate::load`], which is the crate's
/// door for every family.
pub(crate) fn decode(bytes: &[u8]) -> Result<Image, AssetError> {
    if bytes.len() < HEADER {
        return Err(truncated(HEADER, bytes.len()));
    }
    if &bytes[0..4] != b"RIX3" {
        return Err(AssetError::UnknownFormat);
    }
    let width = word(bytes, 4) as u32;
    let height = word(bytes, 6) as u32;
    let palette_type = bytes[8];
    let storage = bytes[9];

    // The palette bounds its own read, and every reader in the tree seeks past
    // the header without looking at it — so a file that stops inside the palette
    // is refused here rather than at the pixel offset.
    if bytes.len() < PIXELS_AT {
        return Err(truncated(PIXELS_AT, bytes.len()));
    }
    check_geometry(Format::Rix3, width, height)?;

    if palette_type != PALETTE_TYPE_VGA {
        return Err(unsupported(format!(
            "the palette type is {palette_type:#04x}; every RIX3 file in the tree carries 0xaf, \
             which the format's own documentation (not the tree: no reader here reads this byte) \
             names a VGA palette, and which is what the parts write to ports 0x3c8/0x3c9"
        )));
    }
    if storage != STORAGE_LINEAR {
        return Err(unsupported(format!(
            "the storage type is {storage:#04x}, not linear (0x00), which is the only value any \
             RIX3 file in the tree carries; by the format's own documentation 0x80 is compressed, \
             0x40 an extension block and 0x20 encryption, and the low nibble names planar storage, \
             none of which this decoder reads"
        )));
    }

    let palette = read_palette(&bytes[PALETTE_AT..PIXELS_AT])?;

    let (w, h) = (width as usize, height as usize);
    let payload = bytes.len() - PIXELS_AT;
    // The canvas is the header's; bytes past it are not a bigger picture.
    if payload > w * h {
        return Err(truncated(PIXELS_AT + w * h, bytes.len()));
    }
    // And the rows are the file's, one whole row at a time: `rows` rows are
    // complete and the next one — which `needed` names the end of — is not.
    let rows = payload / w;
    if rows == 0 || rows * w != payload {
        return Err(truncated(PIXELS_AT + (rows + 1) * w, bytes.len()));
    }

    let mut surface = Surface::new(w, rows);
    surface.pixels_mut().copy_from_slice(&bytes[PIXELS_AT..]);
    Ok(Image { surface, palette })
}

/// The 768 bytes at 10, entry 0 first, each component already six bits.
///
/// Copied verbatim — no widening, no re-quantising, no `six_bit`. The writers of
/// these files are the original's own toolchain, and its 8→6 conversion is the
/// integer `/4` of `GRAB/LBM2U.C:170`; the tree's RIX3 palettes are what came
/// out of it, which is why four of them equal a `.LBM`'s `CMAP / 4` byte for
/// byte. A byte above 63 means this is not that palette, and is refused rather
/// than clamped.
fn read_palette(bytes: &[u8]) -> Result<Palette, AssetError> {
    let mut palette = Palette::new();
    for (i, rgb) in bytes.chunks_exact(3).enumerate() {
        if let Some(&bad) = rgb.iter().find(|&&c| c > 63) {
            return Err(AssetError::Palette {
                what: format!(
                    "entry {i} has the value {bad}, which is not six bits; the RIX3 palettes are \
                     the original's own /4 of an eight-bit CMAP (LBM2U.C:170) and hold six-bit \
                     values"
                ),
            });
        }
        palette.set_entry(i as u8, [rgb[0], rgb[1], rgb[2]]);
    }
    Ok(palette)
}
