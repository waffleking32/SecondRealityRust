//! RIX3 (ColoRIX): the container behind `.CLX` and `.SCI`.
//!
//! The tree holds no writer for this format (an outside paint program's), and the parts never
//! read a header field: they seek to the palette at 10 and the pixels at 778.
//! `WATER/KOE.PAS:35-40` (`seek(f,10)`, `blockread(f,pal,768)` to the DAC, then 64,000 pixel
//! bytes to the screen), `FOREST/READ2.PAS:110-121` (`seek(f,x*640+778)`, the palette at `+10`)
//! and `WATER/DEMO.PAS:108-109` (`+10` and `+778`).
//!
//! So: `"RIX3"`, a little-endian `u16` width and height, then two bytes, a 768-byte palette and
//! one byte per pixel, row-major. The two bytes at 8 and 9 are `AF 00` in all eleven RIX3 files
//! of either tree; by the format's outside documentation (no `file:line` exists for it) byte 8 is
//! the palette type, `0xAF` VGA, and byte 9 the storage type, `0x00` linear. Any other value is
//! refused, on the measured constant rather than on the meaning.
//!
//! - **The palette is six bits** and copied verbatim ([`crate::six_bit_palette`]): four of the
//!   palettes are the `CMAP / 4` of a `.LBM` beside them upstream.
//! - **The payload is whole rows.** `WATER/MIEKKA.SCI` declares 400x250 and carries 35 rows, a
//!   crop the shipped part links and reads (`WATER/DEMO.PAS:109`, `WATER/MAKEOB.BAT:2`), so the
//!   height is the rows the file carries. A payload past the canvas, or short of a whole row,
//!   is [`AssetError::Truncated`]. The header's canvas is still checked against the engine's
//!   largest surface.

use sr_engine::{Palette, Surface};

use crate::{check_geometry, le16 as word, six_bit_palette, AssetError, Format, Image};

const FORMAT: Format = Format::Rix3;

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

fn truncated(needed: usize, had: usize) -> AssetError {
    crate::truncated(FORMAT, needed, had)
}

fn unsupported(what: String) -> AssetError {
    crate::unsupported(FORMAT, what)
}

/// Decode one RIX3 screen (through [`crate::load`]).
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

    if bytes.len() < PIXELS_AT {
        return Err(truncated(PIXELS_AT, bytes.len()));
    }
    check_geometry(Format::Rix3, width, height)?;

    if palette_type != PALETTE_TYPE_VGA {
        return Err(unsupported(format!(
            "the palette type is {palette_type:#04x}; every RIX3 file in the tree carries 0xaf \
             (a VGA palette)"
        )));
    }
    if storage != STORAGE_LINEAR {
        return Err(unsupported(format!(
            "the storage type is {storage:#04x}; every RIX3 file in the tree carries 0x00 \
             (linear)"
        )));
    }

    let mut palette = Palette::new();
    six_bit_palette(&bytes[PALETTE_AT..PIXELS_AT], 0, &mut palette)?;

    let (w, h) = (width as usize, height as usize);
    let payload = bytes.len() - PIXELS_AT;
    if payload > w * h {
        return Err(truncated(PIXELS_AT + w * h, bytes.len()));
    }
    // `rows` rows are complete; `needed` names the end of the next one.
    let rows = payload / w;
    if rows == 0 || rows * w != payload {
        return Err(truncated(PIXELS_AT + (rows + 1) * w, bytes.len()));
    }

    let mut surface = Surface::new(w, rows);
    surface.pixels_mut().copy_from_slice(&bytes[PIXELS_AT..]);
    Ok(Image { surface, palette })
}
