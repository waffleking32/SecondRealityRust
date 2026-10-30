//! `.PAL`: 768 bytes of six-bit VGA DAC palette, and nothing else. No header, no magic, so
//! [`sniff`](crate::sniff) never returns [`Format::Pal`] and [`decode`] is the only way in.
//!
//! The parts write the bytes to the DAC untouched: `PAM/ANIM.C:19-21` (`fread(pal,1,768,data)`,
//! `tw_setpalette(pal)`), `VISU/C/MAIN.C:234-241` (copied into the `.00M` at `:262`, written to
//! the DAC at `VISU/C/U2A.C:243-246`) and `PANIC/SHUTDOWN.C:41-42`, whose fade at `:62` is
//! arithmetic over 0..63. Three of the files are the `CMAP / 4` of the picture beside them,
//! byte for byte (`tests/pal.rs`), so they are copied verbatim ([`crate::six_bit_palette`]).
//! [`Palette::from_vga`]'s `>> 2` is for eight-bit data, not these.

use sr_engine::Palette;

use crate::{six_bit_palette, truncated, AssetError, Format};

/// Exactly what a `.PAL` is: 256 entries of three bytes.
const LEN: usize = 768;

/// Decode a `.PAL`. Any length but 768 is [`AssetError::Truncated`] (`had` above `needed` for
/// a file that is too long).
pub fn decode(bytes: &[u8]) -> Result<Palette, AssetError> {
    if bytes.len() != LEN {
        return Err(truncated(Format::Pal, LEN, bytes.len()));
    }
    let mut palette = Palette::new();
    six_bit_palette(bytes, 0, &mut palette)?;
    Ok(palette)
}
