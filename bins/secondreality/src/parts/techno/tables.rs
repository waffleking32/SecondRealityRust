//! TECHNO's palettes and lookup tables: `KOEB.ASM`'s `pal0`, `KOEA.ASM`'s
//! `pal1`/`pal2`, `flip8` and `_blit16t`, and `KOE.C`'s `power0`, bar palette
//! and troll fades, each built from the source's rule.

/// `KOEB.ASM`'s `pal0`: the first pass's colour, `(0,30,40)` at the two
/// positions the 8-triple window can land on.
pub(super) const PAL0: [u8; 48] = [
    0, 30, 40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 30, 40, 0, 0, 0,
    0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
];
/// `KOEA.ASM`'s `pal1`: `(v, v*8/9, v)`.
pub(super) const PAL1: [u8; 48] = [
    30, 26, 30, 60, 53, 60, 50, 44, 50, 40, 35, 40, 30, 26, 30, 20, 17, 20, 10, 8, 10, 0, 0, 0,
    30, 26, 30, 60, 53, 60, 50, 44, 50, 40, 35, 40, 30, 26, 30, 20, 17, 20, 10, 8, 10, 0, 0, 0,
];
/// `KOEA.ASM`'s `pal2`: `(v, v*7/9, v)`.
pub(super) const PAL2: [u8; 48] = [
    0, 0, 0, 10, 7, 10, 20, 15, 20, 30, 23, 30, 40, 31, 40, 50, 38, 50, 60, 46, 60, 30, 23, 30,
    0, 0, 0, 10, 7, 10, 20, 15, 20, 30, 23, 30, 40, 31, 40, 50, 38, 50, 60, 46, 60, 30, 23, 30,
];

/// The byte-reversal `flip8` table (`KOEA.ASM:425-438`).
pub(super) fn flip8_table() -> [u8; 256] {
    std::array::from_fn(|value| (value as u8).reverse_bits())
}

/// `blitinit`'s `_blit16t` (`KOEA.ASM:84-114`), 256 two-byte entries.
///
/// The low byte is the source byte's suffix parity, so the written row is the
/// row's prefix XOR (the XOR-filled polygon); the high byte is the parity bit
/// the chain carries to the next byte.
pub(super) fn blit_table() -> [(u8, u8); 256] {
    let mut table = [(0u8, 0u8); 256];
    for (value, entry) in table.iter_mut().enumerate() {
        let mut dh = 255u8;
        let mut dl = value as u8;
        let mut ah = 0u8;
        let mut carry = 0u8;
        for _ in 0..8 {
            let next = dl >> 7;
            dl = (dl << 1) | carry;
            carry = next;
            if carry != 0 {
                ah ^= dh;
            }
            dh >>= 1;
        }
        *entry = (ah, ((ah >> 1) | ((ah & 1) << 7)) & 0x80);
    }
    table
}

/// `KOE.C:173-211`'s `power0`: 16 rows of 256 signed bytes. Row `b` scales the
/// signed byte by `b/15`; row 15 is the byte itself. (`power1`, the same
/// table, is never read.)
pub(super) fn power_table() -> [i8; 4096] {
    let mut table = [0i8; 4096];
    for b in 0..16usize {
        for c in 0..256usize {
            let a = if c > 127 { c as i32 - 256 } else { c as i32 };
            table[b * 256 + c] = if b == 15 { c as u8 as i8 } else { (a * b as i32 / 15) as i8 };
        }
    }
    table
}

/// The bar palette (`KOE.C:119-168`): 16 blocks of 16 entries. Entry `a`'s
/// base colour is chosen by `a`'s popcount and scaled per block `c` by
/// `(10 + c*9/9)/10`, `(10 + c*7/9)/10` and `(10 + c*5/9)/10`, clamped at 63.
pub(super) fn bar_palette() -> [u8; 768] {
    const BASE: [[i32; 3]; 5] = [
        [0, 0, 0],
        [38 * 64 / 111, 33 * 64 / 111, 44 * 64 / 111],
        [52 * 64 / 111, 45 * 64 / 111, 58 * 64 / 111],
        [67 * 64 / 111, 61 * 64 / 111, 73 * 64 / 111],
        [83 * 64 / 111, 77 * 64 / 111, 89 * 64 / 111],
    ];
    let mut pal = [0u8; 768];
    for c in 0..16i32 {
        for a in 0..16usize {
            let [r, g, b] = BASE[a.count_ones() as usize];
            let at = (c as usize * 16 + a) * 3;
            pal[at] = (r * (10 + c * 9 / 9) / 10).min(63) as u8;
            pal[at + 1] = (g * (10 + c * 7 / 9) / 10).min(63) as u8;
            pal[at + 2] = (b * (10 + c * 5 / 9) / 10).min(63) as u8;
        }
    }
    pal
}

/// `KOE.C:623-633`: the sixteen troll fade tables, `palette + (45 - 3y)`.
pub(super) fn troll_fade(palette: &[u8; 768]) -> Vec<u8> {
    let mut fade = vec![0u8; 16 * 768];
    for y in 0..16usize {
        let step = 45 - 3 * y as i32;
        for a in 0..768usize {
            fade[y * 768 + a] = (i32::from(palette[a]) + step).min(63) as u8;
        }
    }
    fade
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_palette_is_the_popcount_base_scaled_by_the_sources_factors() {
        let pal = bar_palette();
        let entry = |c: usize, a: usize| &pal[(c * 16 + a) * 3..(c * 16 + a) * 3 + 3];
        assert_eq!(entry(0, 0), &[0, 0, 0]);
        assert_eq!(entry(0, 1), &[21, 19, 25], "c=0 a=1 is the base triple");
        assert_eq!(entry(15, 1), &[52, 39, 45]);
        assert_eq!(entry(2, 15), &[56, 48, 56]);
    }

    #[test]
    fn the_troll_fade_tables_add_the_sources_steps() {
        let mut palette = [0u8; 768];
        palette[0] = 10;
        palette[1] = 30;
        let fade = troll_fade(&palette);
        assert_eq!(&fade[..2], &[55, 63], "table 0 adds 45, clamped at 63");
        assert_eq!(&fade[768..770], &[52, 63], "table 1 adds 42");
        assert_eq!(&fade[15 * 768..15 * 768 + 2], &[10, 30], "the last table is the palette");
    }
}
