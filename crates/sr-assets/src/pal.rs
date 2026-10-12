//! `.PAL`: 768 bytes of VGA DAC palette, and nothing else — no header, no magic,
//! no length field.
//!
//! Because there is no magic, a `.PAL` is **loaded by construction**: [`sniff`](crate::sniff)
//! never returns [`Format::Pal`], and this function is the door, not `load`.
//! (All five curated `.PAL` files are exactly 768 bytes, so their size identifies
//! nothing — a size-based sniff would hand every 768-byte file in the tree to
//! this decoder, including the three `OriginBare` dumps whose pixels are simply
//! 64,000 bytes long and the RIX3 palettes sitting at offset 10 inside their
//! files.)
//!
//! **The bytes are six-bit DAC values and are copied verbatim.** They are already
//! the output of the original's only 8→6 rule, the integer `/4` of
//! `GRAB/LBM2U.C:170`, and applying any conversion again divides them by four:
//!
//! - The parts write them to the DAC untouched. `PAM/ANIM.C:19-21` —
//!   `fopen("pal.pal","rb")`, `fread(pal,1,768,data)`, `tw_setpalette(pal)`.
//!   `VISU/C/MAIN.C:234-241` — `fread(matpal,768,1,in)` for `<scene>.PAL`, at
//!   `:262` written into the `.00M` the parts read their material palette from
//!   (`VISU/C/U2A.C:243-246` — `cp = (char *)(scenem+16)` at `:243` and the
//!   live DAC write `outp(0x3c8,0); for(…;a<768;…) outp(0x3c9,cp[a]);` at
//!   `:245-246`; `VISU/C/U2E.C:337` is the same pointer), and at `:386` handed to
//!   `vid_setpal`. `PANIC/SHUTDOWN.C:41-42` — `read(fff,kuvapal,768)`, with the
//!   fade table at `:62` computed as `(a*63 + kuvapal[b]*(64-a))/64`, arithmetic
//!   that only makes sense over 0..63.
//! - And the files are the `/4` by measurement. Three curated `.PAL` files are
//!   the `CMAP` of the picture beside them divided by four, 768 bytes of 768:
//!   `START/HZPIC.PAL` against `START/HZPIC.LBM`, `VISU/C/SCENE/U2E.PAL` against
//!   `U2E.LBM`, and `VISU/C/U2A.PAL` against `U2A.LBM`. [`crate::six_bit`] — the
//!   inverse of the engine's 6→8 *display* widening — would differ on 92, 282 and
//!   234 of those bytes; `tests/pal.rs` pins each count, because a byte above 63
//!   is otherwise the only thing a reading test could catch.
//! - The engine's own [`Palette::from_vga`] is **not** this function: it shifts
//!   each component right by two, which is the same 8→6 rule as `/4`. It is for
//!   the eight-bit palettes it names, not for a file that has already been
//!   converted.
//!
//! The length is exact in both directions. A `.PAL` is 768 bytes and all of them
//! are the palette; a file of any other length is [`AssetError::Truncated`] with
//! `needed` 768 and `had` the length it was handed — the variant's fields are
//! file offsets, and `had` being larger than `needed` is the over-long case. A
//! component above 63 is [`AssetError::Palette`], never a clamp:
//! [`Palette::set_entry`] masks with `& 0x3F`, so an unchecked byte would come
//! back as a different colour with nothing to show for it.

use sr_engine::Palette;

use crate::{AssetError, Format};

/// Exactly what a `.PAL` is: 256 entries of three bytes, with no header.
const LEN: usize = 768;

/// Decode a `.PAL` — its 768 bytes, as they are stored.
///
/// This is the door for the format, because nothing in a `.PAL` says what it is:
/// [`crate::load`] cannot take it either, since a palette has no pixels to put in
/// an [`Image`](crate::Image).
pub fn decode(bytes: &[u8]) -> Result<Palette, AssetError> {
    if bytes.len() != LEN {
        return Err(AssetError::Truncated {
            format: Format::Pal,
            needed: LEN,
            had: bytes.len(),
        });
    }
    let mut palette = Palette::new();
    for (i, rgb) in bytes.chunks_exact(3).enumerate() {
        if let Some(&bad) = rgb.iter().find(|&&c| c > 63) {
            return Err(AssetError::Palette {
                what: format!(
                    "entry {i} is {} {} {}, and {bad} is not six bits; the original's own \
                     converter divides an eight-bit CMAP by four (LBM2U.C:170) and every .PAL in \
                     the tree holds six-bit values, so this is not one of them",
                    rgb[0], rgb[1], rgb[2]
                ),
            });
        }
        palette.set_entry(i as u8, [rgb[0], rgb[1], rgb[2]]);
    }
    Ok(palette)
}
