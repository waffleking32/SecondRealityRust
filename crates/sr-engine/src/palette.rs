//! The 256-entry colour table that stands in for the VGA's DAC.
//!
//! Six bits are kept internally and widened to eight only for output — see
//! [`dac_expand`]. Widening early would lose the precision the original's
//! fades and copper effects depend on.

/// Widen a 6-bit DAC value to 8 bits by rounding to nearest.
///
/// `(v * 259 + 33) >> 6` is `round(v * 255 / 63)` — the mapping the reference
/// oracle's DAC applies, measured against it in `docs/decisions.md` D14. Bit
/// replication, `(v << 2) | (v >> 4)`, is the cruder "wire the top bits up"
/// approximation and differs from it on ten of the sixty-four values.
/// Input above 63 is masked; the DAC had no eighth bit to receive it.
pub const fn dac_expand(v: u8) -> u8 {
    let v = (v & 0x3F) as u16;
    ((v * 259 + 33) >> 6) as u8
}

/// A 256-entry RGB palette, six bits per channel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Palette {
    /// One `[r, g, b]` triple per entry; every component is 0..=63.
    entries: [[u8; 3]; 256],
}

impl Default for Palette {
    /// Every entry black — what [`Palette::new`] produces.
    fn default() -> Self {
        Palette::BLACK
    }
}

impl Palette {
    /// Every entry black.
    pub const BLACK: Palette = Palette {
        entries: [[0, 0, 0]; 256],
    };

    /// A palette of 256 black entries.
    pub fn new() -> Palette {
        Palette::BLACK
    }

    /// 768 bytes of 8-bit RGB — an IFF `CMAP`, a module's palette. Each
    /// component is shifted down by 2, which is the original's own conversion
    /// (`GRAB/LBM2U.C:170` writes `getc(f1)/4`); the DAC's 6 bits are the top 6.
    ///
    /// **Not a `.PAL`.** The tree's five curated `.PAL` files hold the *output* of
    /// that same conversion — three of them are cross-measured against the `CMAP`
    /// of the picture beside them, and the parts' own reads say it for all five —
    /// so they are six-bit DAC values already and shifting them down
    /// again darkens every colour by four. `sr_assets::pal::decode` reads them as
    /// they are; `sr_assets::pal`'s docs carry the measurements that settle it.
    pub fn from_vga(bytes: &[u8]) -> Palette {
        let mut p = Palette::new();
        for (entry, rgb) in p.entries.iter_mut().zip(bytes.chunks_exact(3)) {
            *entry = [rgb[0] >> 2, rgb[1] >> 2, rgb[2] >> 2];
        }
        p
    }

    /// Round-trips `from_vga` for input already in 0..=63.
    pub fn to_vga(&self) -> [u8; 768] {
        let mut out = [0u8; 768];
        for (i, entry) in self.entries.iter().enumerate() {
            out[i * 3] = entry[0] << 2;
            out[i * 3 + 1] = entry[1] << 2;
            out[i * 3 + 2] = entry[2] << 2;
        }
        out
    }

    /// The `[r, g, b]` triple at `index`.
    pub fn entry(&self, index: u8) -> [u8; 3] {
        self.entries[index as usize]
    }

    /// Components above 63 are masked with `& 0x3F`.
    pub fn set_entry(&mut self, index: u8, rgb: [u8; 3]) {
        self.entries[index as usize] = [rgb[0] & 0x3F, rgb[1] & 0x3F, rgb[2] & 0x3F];
    }

    /// Replaces every entry with `other`'s.
    pub fn copy_from(&mut self, other: &Palette) {
        self.entries = other.entries;
    }

    /// 256 RGBA quads, 1024 bytes, through `dac_expand`.
    pub fn write_rgba(&self, out: &mut [u8; 1024]) {
        for (i, entry) in self.entries.iter().enumerate() {
            out[i * 4] = dac_expand(entry[0]);
            out[i * 4 + 1] = dac_expand(entry[1]);
            out[i * 4 + 2] = dac_expand(entry[2]);
            out[i * 4 + 3] = 255;
        }
    }
}

/// Per-frame record of which palette was in force from which scanline.
/// `changes` is kept sorted by scanline and holds at most one entry per scanline.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PaletteTimeline {
    base: Palette,
    changes: Vec<(u16, Palette)>,
}

impl PaletteTimeline {
    /// `base` in force from scanline 0, no changes yet.
    pub fn new(base: Palette) -> PaletteTimeline {
        PaletteTimeline {
            base,
            changes: Vec::new(),
        }
    }

    /// The palette in force before the first change.
    pub fn base(&self) -> &Palette {
        &self.base
    }

    /// Record `pal` as the palette from `scanline` onward, replacing any
    /// existing entry at that exact scanline.
    pub fn push(&mut self, scanline: u16, pal: Palette) {
        let at = self.changes.partition_point(|(s, _)| *s < scanline);
        match self.changes.get_mut(at) {
            Some((s, existing)) if *s == scanline => *existing = pal,
            _ => self.changes.insert(at, (scanline, pal)),
        }
    }

    /// The palette in force at `scanline`: the last change at or before it,
    /// else the base.
    pub fn palette_at(&self, scanline: u16) -> &Palette {
        match self.changes.partition_point(|(s, _)| *s <= scanline) {
            0 => &self.base,
            n => &self.changes[n - 1].1,
        }
    }

    /// How many scanlines carry a change.
    pub fn change_count(&self) -> usize {
        self.changes.len()
    }

    /// Reset to `base` and drop every change.
    pub fn reset(&mut self, base: Palette) {
        self.base = base;
        self.changes.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[test]
    fn from_vga_keeps_six_bit_precision() {
        let mut vga = [0u8; 768];
        vga[0..3].copy_from_slice(&[255, 128, 4]);
        let p = Palette::from_vga(&vga);
        assert_eq!(p.entry(0), [63, 32, 1]);
    }

    #[test]
    fn to_vga_round_trips_values_already_in_range() {
        let mut p = Palette::new();
        p.set_entry(5, [63, 32, 1]);
        let vga = p.to_vga();
        assert_eq!(&vga[15..18], &[252, 128, 4]);
        assert_eq!(Palette::from_vga(&vga).entry(5), [63, 32, 1]);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(1, 4)]
    #[case(16, 65)]
    #[case(32, 130)]
    #[case(63, 255)]
    // 15, 48 and 52 are three of the ten values where rounding and bit
    // replication disagree; replication gives 60, 195 and 211.
    #[case(15, 61)]
    #[case(48, 194)]
    #[case(52, 210)]
    // Above six bits the DAC had nowhere to put the extra bits, so they are masked.
    #[case(64, 0)]
    #[case(255, 255)]
    fn dac_expand_rounds_to_nearest(#[case] input: u8, #[case] expected: u8) {
        assert_eq!(dac_expand(input), expected);
    }

    /// The rule is `round(v * 255 / 63)` over the whole six-bit range, which is
    /// what the emulated DAC does — D14.
    #[test]
    fn dac_expand_is_the_ideal_even_mapping() {
        for v in 0u8..=63 {
            let ideal = ((v as u32 * 255 + 31) / 63) as u8;
            assert_eq!(dac_expand(v), ideal, "at {v}");
        }
    }

    #[test]
    fn set_entry_masks_instead_of_overflowing_into_the_next_channel() {
        let mut p = Palette::new();
        p.set_entry(1, [255, 64, 63]);
        assert_eq!(p.entry(1), [63, 0, 63]);
    }

    #[test]
    fn write_rgba_emits_256_quads() {
        let mut p = Palette::new();
        p.set_entry(0, [63, 0, 32]);
        p.set_entry(255, [0, 63, 0]);
        let mut out = [0u8; 1024];
        p.write_rgba(&mut out);
        // dac_expand(32) = (32 * 259 + 33) >> 6 = 8321 >> 6 = 130.
        assert_eq!(&out[0..4], &[255, 0, 130, 255]);
        assert_eq!(&out[1020..1024], &[0, 255, 0, 255]);
    }
}
