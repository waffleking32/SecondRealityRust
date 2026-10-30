//! GLENZ's point-list builder and colour allocator: `ceasypolylist`
//! (`VEC.ASM:514-553`) over `adddot` (`:260-275`), `checkhiddenbx`
//! (`:277-304`) and the two colour rules `demo_glz`/`demo_glz2`
//! (`VEC.ASM:403-506`), with the shipped polygon tables from
//! `MAIN.C:176-240`.
//!
//! `ceasypolylist` walks a polygon table; for each polygon it copies each
//! vertex's projected screen pair into an output list, skipping a vertex whose
//! index equals the last one copied (the tracker is carried across polygon
//! boundaries, so a polygon that starts where the previous one ended loses its
//! first point), drops the closing duplicate, computes `checkhiddenbx`'s
//! 32-bit cross product on the first three stored pairs and calls the current
//! colour rule, which returns the colour to draw the polygon with.
//!
//! # The colour allocator
//!
//! `demo_glz` keeps one pool of even DAC groups, slots 2..30 (32 when all are
//! taken), in `rolused`, keyed in `rolcol`. The tree keys the visible path by
//! the full colour word (`VEC.ASM:450`) and the hidden path by the low byte
//! (`:417`); the reference's picture identifies the low byte on both paths,
//! and the port adopts that: one 256-entry map, so a hidden face frees the
//! slot its visible face took (D24; A37; `glenz.md` §7, §11.2). The visible
//! path also writes the face's 16-entry DAC group ([`ColourState::face`]).
//!
//! `demo_glz2` reduces a visible face to `(word >> 1) & 1` and leaves a hidden
//! face's word unchanged (`jc @@1`, `VEC.ASM:403-410`).

use sr_engine::Palette;

use super::math::PVert;

/// One triangle of `epolys`/`epolysb` (`MAIN.C:176-240`): the colour word and
/// the three vertex indices into the projected list.
///
/// The source writes each table as one flat `int` array — `3, 0x4002, 0, 1,
/// 8, 3, 0x4004, …` — terminated by a zero count; [`EPOLYS`] and [`EPOLYSB`]
/// keep the same order and values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PolyDef {
    /// The polygon's colour word, e.g. `0x4002`.
    pub colour: u16,
    /// Vertex indices into the projected point list.
    pub points: &'static [u16],
}

/// `epolys` (`MAIN.C:176-207`), the first object's 24 triangles.
///
/// Colours `0x4002..=0x4030` in steps of two, in the source's order.
#[rustfmt::skip]
pub const EPOLYS: &[PolyDef] = &[
    PolyDef { colour: 0x4002, points: &[0, 1, 8] },
    PolyDef { colour: 0x4004, points: &[1, 2, 8] },
    PolyDef { colour: 0x4006, points: &[2, 3, 8] },
    PolyDef { colour: 0x4008, points: &[3, 0, 8] },
    PolyDef { colour: 0x400a, points: &[2, 1, 10] },
    PolyDef { colour: 0x400c, points: &[1, 5, 10] },
    PolyDef { colour: 0x400e, points: &[5, 6, 10] },
    PolyDef { colour: 0x4010, points: &[6, 2, 10] },
    PolyDef { colour: 0x4012, points: &[2, 6, 12] },
    PolyDef { colour: 0x4014, points: &[6, 7, 12] },
    PolyDef { colour: 0x4016, points: &[7, 3, 12] },
    PolyDef { colour: 0x4018, points: &[3, 2, 12] },
    PolyDef { colour: 0x401a, points: &[0, 3, 11] },
    PolyDef { colour: 0x401c, points: &[3, 7, 11] },
    PolyDef { colour: 0x401e, points: &[7, 4, 11] },
    PolyDef { colour: 0x4020, points: &[4, 0, 11] },
    PolyDef { colour: 0x4022, points: &[5, 1, 13] },
    PolyDef { colour: 0x4024, points: &[1, 0, 13] },
    PolyDef { colour: 0x4026, points: &[0, 4, 13] },
    PolyDef { colour: 0x4028, points: &[4, 5, 13] },
    PolyDef { colour: 0x402a, points: &[5, 4, 9] },
    PolyDef { colour: 0x402c, points: &[4, 7, 9] },
    PolyDef { colour: 0x402e, points: &[7, 6, 9] },
    PolyDef { colour: 0x4030, points: &[6, 5, 9] },
];

/// `epolysb` (`MAIN.C:209-240`), the second object's 24 triangles.
///
/// The same twelve faces with the colours alternating `0x4004`, `0x4002`,
/// starting with `0x4004`.
#[rustfmt::skip]
pub const EPOLYSB: &[PolyDef] = &[
    PolyDef { colour: 0x4004, points: &[0, 1, 8] },
    PolyDef { colour: 0x4002, points: &[1, 2, 8] },
    PolyDef { colour: 0x4004, points: &[2, 3, 8] },
    PolyDef { colour: 0x4002, points: &[3, 0, 8] },
    PolyDef { colour: 0x4004, points: &[2, 1, 10] },
    PolyDef { colour: 0x4002, points: &[1, 5, 10] },
    PolyDef { colour: 0x4004, points: &[5, 6, 10] },
    PolyDef { colour: 0x4002, points: &[6, 2, 10] },
    PolyDef { colour: 0x4004, points: &[2, 6, 12] },
    PolyDef { colour: 0x4002, points: &[6, 7, 12] },
    PolyDef { colour: 0x4004, points: &[7, 3, 12] },
    PolyDef { colour: 0x4002, points: &[3, 2, 12] },
    PolyDef { colour: 0x4004, points: &[0, 3, 11] },
    PolyDef { colour: 0x4002, points: &[3, 7, 11] },
    PolyDef { colour: 0x4004, points: &[7, 4, 11] },
    PolyDef { colour: 0x4002, points: &[4, 0, 11] },
    PolyDef { colour: 0x4004, points: &[5, 1, 13] },
    PolyDef { colour: 0x4002, points: &[1, 0, 13] },
    PolyDef { colour: 0x4004, points: &[0, 4, 13] },
    PolyDef { colour: 0x4002, points: &[4, 5, 13] },
    PolyDef { colour: 0x4004, points: &[5, 4, 9] },
    PolyDef { colour: 0x4002, points: &[4, 7, 9] },
    PolyDef { colour: 0x4004, points: &[7, 6, 9] },
    PolyDef { colour: 0x4002, points: &[6, 5, 9] },
];

/// One output polygon: the colour rule's answer and the projected screen
/// pairs.
///
/// This is the source's `polylist` record — `[count, colour, x0, y0, x1,
/// y1, …]` — with the count implicit in [`points`](Poly::points).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Poly {
    /// The colour word the colour rule returned (a palette index group's
    /// base, or the hidden face's reduced word).
    pub colour: u16,
    /// The stored screen pairs, in order.
    pub points: Vec<(i16, i16)>,
}

/// `demomode[]` (`VEC.ASM:508-512`): which colour rule `ceasypolylist`
/// calls.
///
/// `MAIN.C:597` points `demomode[0]` at `demomode[1]` (`demo_glz`) for the
/// first object; `MAIN.C:620` points it at `demomode[2]` (`demo_glz2`) for the
/// second.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DemoMode {
    /// `demo_glz` (`VEC.ASM:412-506`): the slot pool and the DAC groups.
    Glz,
    /// `demo_glz2` (`VEC.ASM:403-410`): only the hidden-face reduction.
    Glz2,
}

/// `checkhiddenbx` (`VEC.ASM:277-304`): the signed cross product of the first
/// three stored points.
///
/// The formula is `(x0−x1)(y0−y2) − (y0−y1)(x0−x2)`, with each coordinate
/// difference the asm's 16-bit `sub` (it wraps in the word, then `imul`
/// sign-extends) and the two 16×16→32 products subtracted at 32 bits. A
/// negative result is the hidden face: `cmp dx,0; jl` tests the high word.
///
/// The source reads three pairs unconditionally. The shipped tables always
/// leave three after the carried dedupe; a shorter list returns 0 here, where
/// the source would read whatever follows the record in `polylist`.
pub fn cross(points: &[(i16, i16)]) -> i32 {
    if points.len() < 3 {
        return 0;
    }
    let (x0, y0) = points[0];
    let (x1, y1) = points[1];
    let (x2, y2) = points[2];
    // `sub ax,es:[bx+4]` / `sub si,es:[bx+8+2]` (`VEC.ASM:281-283`): the words
    // wrap, and `imul si` sign-extends them.
    let dx01 = x0.wrapping_sub(x1);
    let dy02 = y0.wrapping_sub(y2);
    let dy01 = y0.wrapping_sub(y1);
    let dx02 = x0.wrapping_sub(x2);
    let first = i32::from(dx01) * i32::from(dy02);
    let second = i32::from(dy01) * i32::from(dx02);
    // `sub ax,si` / `sbb dx,cx` (`VEC.ASM:295-296`): the 32-bit difference.
    first.wrapping_sub(second)
}

/// The colour allocator's persistent state, `rolcol` and `rolused`
/// (`VEC.ASM:396-397`), which persist across frames.
pub struct ColourState {
    /// `rolcol`, keyed by the colour word's low byte on both paths.
    slots: [u8; 256],
    /// `rolused`: whether a slot's DAC group is allocated.
    used: [bool; 256],
}

impl Default for ColourState {
    fn default() -> Self {
        Self::new()
    }
}

impl ColourState {
    /// Both tables zeroed, as the source's start.
    pub fn new() -> Self {
        ColourState {
            slots: [0; 256],
            used: [false; 256],
        }
    }

    /// Test-only: the slot stored under a colour word's low byte.
    #[cfg(test)]
    pub(crate) fn slot_of(&self, colour: u16) -> u8 {
        self.slots[colour as u8 as usize]
    }

    /// Apply `demo_glz`/`demo_glz2` to one face whose [`cross`] product is
    /// `cross` (negative is hidden), and return the colour word to store.
    /// `lightshift` is the frame's `_lightshift` (`MAIN.C:394`, `:500`).
    pub fn face(
        &mut self,
        colour: u16,
        cross: i32,
        mode: DemoMode,
        lightshift: i16,
        backpal: &[[u8; 3]; 16],
        palette: &mut Palette,
    ) -> u16 {
        match mode {
            DemoMode::Glz2 => {
                if cross < 0 {
                    colour
                } else {
                    (colour >> 1) & 1
                }
            }
            DemoMode::Glz => {
                if cross < 0 {
                    self.hidden_face(colour);
                    ((colour >> 1) & 1) << 2
                } else {
                    self.visible_face(colour, cross, lightshift, backpal, palette)
                }
            }
        }
    }

    /// `demo_glz`'s visible branch (`VEC.ASM:430-505`): take the slot stored
    /// under the colour word's low byte (allocating the first free even slot
    /// 2..30, or 32 when they are all taken), return its DAC base and write
    /// the group's 16 entries: the base plus one `backpal` row `>> 2` each,
    /// the base restored every entry (`:498-499`), clamped to 63.
    ///
    /// A reused slot's base has the computed shade and R = 7 or `lightshift`;
    /// a fresh one's is the allocation scan's leavings: shade 0 and, for a
    /// bit-1 word, R = `16 − slot/2`.
    fn visible_face(
        &mut self,
        colour: u16,
        cross: i32,
        lightshift: i16,
        backpal: &[[u8; 3]; 16],
        palette: &mut Palette,
    ) -> u16 {
        let (slot, fresh) = match self.slots[colour as u8 as usize] {
            // `mov al,cs:rolcol[bx]; cmp al,0; jne @@rc1` (`VEC.ASM:450-453`):
            // a stored slot is reused without touching the pool bit.
            0 => {
                // `mov cx,15; mov bp,2` … `loop @@rc3` (`VEC.ASM:454-459`):
                // fifteen even candidates. The loop exits with bp = 32 when
                // none is free.
                let slot = (2u8..=30)
                    .step_by(2)
                    .find(|&candidate| !self.used[candidate as usize])
                    .unwrap_or(32);
                // `mov cs:rolcol[bx],al; mov cs:rolused[bp],1`
                // (`VEC.ASM:461-462`): the low-byte map and the pool only.
                self.slots[colour as u8 as usize] = slot;
                self.used[slot as usize] = true;
                (slot, true)
            }
            slot => (slot, false),
        };

        // `shl al,3` (`VEC.ASM:465`): the group's first DAC entry; slot 32
        // wraps to 0 in the byte store.
        let start = (u16::from(slot) << 3) as u8;

        // The shade (`VEC.ASM:430-446`): `low16(cross >> 7)` when
        // lightshift is 9. Otherwise the source's literal pair: `ax1` is
        // `low16(cross >> 8)`, and the second `shrd` shifts the 32-bit pair
        // (high word, already-shifted `ax1`) right by one, so
        // `ax2 = (ax1 >> 1) | (high16(cross) << 15)`. The 16-bit sum is
        // clamped signed to 0..63.
        let computed_shade = if lightshift == 9 {
            ((cross >> 7) as i16).clamp(0, 63)
        } else {
            let ax1 = (cross >> 8) as i16;
            let ax2 = (((ax1 as u16) >> 1) | (((cross >> 16) as u16) << 15)) as i16;
            ax1.wrapping_add(ax2).clamp(0, 63)
        } as u8;
        // `mov ax,bp` (`VEC.ASM:460`) replaces AH with the slot's high byte
        // (0) on the allocation branch; the reused branch keeps the computed
        // shade that `mov ah,al` (`:447`) left there.
        let shade = if fresh { 0 } else { computed_shade };

        // `mov cl,cs:_lightshift; cmp cl,9; je @@x9; … mov cl,7`
        // (`VEC.ASM:430-440`): on a reused slot R's base is 7 in the first
        // phase, otherwise the lightshift itself. A fresh allocation's scan
        // has overwritten CL (`mov cx,15; … loop @@rc3`, `:454-459`), leaving
        // the residual 16 − slot/2 (slot 2 → 15, …, 30 → 1, 32 → 0).
        let r_base = if fresh {
            16 - slot / 2
        } else if lightshift == 9 {
            7
        } else {
            lightshift as u8
        };
        // `test bp,2` (`VEC.ASM:472-480`): bit 1 of the colour word selects
        // the blue/grey base (R, shade/2, shade); clear selects all-shade.
        let base = if colour & 2 != 0 {
            [r_base, shade >> 1, shade]
        } else {
            [shade, shade, shade]
        };

        // The 16-entry group (`VEC.ASM:486-501`), an 8-bit register add.
        for (j, back) in backpal.iter().enumerate() {
            palette.set_entry(
                start.wrapping_add(j as u8),
                [
                    base[0].wrapping_add(back[0] >> 2).min(63),
                    base[1].wrapping_add(back[1] >> 2).min(63),
                    base[2].wrapping_add(back[2] >> 2).min(63),
                ],
            );
        }

        // `mov es:[bx-2],al` (`VEC.ASM:466`): the stored colour is the group
        // base (the source keeps the word's high byte, which the pixel
        // compositor never reads).
        u16::from(start)
    }

    /// `demo_glz`'s hidden branch (`VEC.ASM:413-429`): clear the low byte's
    /// entry and its slot's `used` bit (`used[0]` when the entry is 0, which
    /// the allocator never reads).
    fn hidden_face(&mut self, colour: u16) {
        let key = colour as u8;
        let slot = self.slots[key as usize];
        self.slots[key as usize] = 0;
        self.used[slot as usize] = false;
    }
}

/// `ceasypolylist` (`VEC.ASM:514-553`): build the frame's polygon list.
///
/// Each definition's point indices are turned into the projected pairs; a
/// point equal to the one last copied is skipped (the tracker starts at -1
/// per call and persists across polygons), and after the polygon's points the
/// last pair is dropped if it equals the first. The cross product of the
/// first three stored pairs then goes to [`ColourState::face`].
pub fn build(
    defs: &[PolyDef],
    projected: &[PVert],
    mode: DemoMode,
    state: &mut ColourState,
    lightshift: i16,
    backpal: &[[u8; 3]; 16],
    palette: &mut Palette,
) -> Vec<Poly> {
    let mut polys = Vec::with_capacity(defs.len());
    // `mov bp,-1` (`VEC.ASM:525`): the index tracker is per call and carried
    // across polygons; `adddot` updates it whenever it copies a pair.
    let mut last_index: i32 = -1;
    for def in defs {
        let mut points = Vec::with_capacity(def.points.len());
        for &index in def.points {
            if i32::from(index) == last_index {
                continue;
            }
            last_index = i32::from(index);
            let point = projected[index as usize];
            points.push((point.x, point.y));
        }
        // `mov eax,es:[bx]; cmp eax,es:[di-4]; jne @@4; sub di,4`
        // (`VEC.ASM:539-543`): drop the closing duplicate.
        if points.len() > 1 && points.first() == points.last() {
            points.pop();
        }
        // `call checkhiddenbx; call cs:_demomode` (`VEC.ASM:548-549`).
        let colour = state.face(def.colour, cross(&points), mode, lightshift, backpal, palette);
        polys.push(Poly { colour, points });
    }
    polys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A projected vertex with the flags unused by this layer.
    fn vert(x: i16, y: i16) -> PVert {
        PVert { x, y, flags: 0, z: 128 }
    }

    const NO_BACKPAL: [[u8; 3]; 16] = [[0, 0, 0]; 16];

    fn def(colour: u16, points: &'static [u16]) -> PolyDef {
        PolyDef { colour, points }
    }

    /// `build` under `demo_glz2`, which writes no DAC.
    fn build_glz2(defs: &[PolyDef], projected: &[PVert]) -> Vec<Poly> {
        let mut palette = Palette::new();
        let mut state = ColourState::new();
        build(defs, projected, DemoMode::Glz2, &mut state, 9, &NO_BACKPAL, &mut palette)
    }

    /// One `demo_glz` face at `lightshift` 9.
    fn glz(
        state: &mut ColourState,
        palette: &mut Palette,
        colour: u16,
        cross: i32,
        backpal: &[[u8; 3]; 16],
    ) -> u16 {
        state.face(colour, cross, DemoMode::Glz, 9, backpal, palette)
    }

    fn assert_group(palette: &Palette, start: u8, expected: [u8; 3]) {
        for j in 0..16u8 {
            assert_eq!(palette.entry(start.wrapping_add(j)), expected, "entry {}", start + j);
        }
    }

    /// `MAIN.C:176-240`: 24 triangles each, the source's colours and order.
    #[test]
    fn the_shipped_polygon_tables_are_the_sources() {
        assert_eq!((EPOLYS.len(), EPOLYSB.len()), (24, 24));
        for (i, (a, b)) in EPOLYS.iter().zip(EPOLYSB).enumerate() {
            assert_eq!(a.colour, 0x4002 + 2 * i as u16);
            assert_eq!(b.colour, if i % 2 == 0 { 0x4004 } else { 0x4002 });
            assert_eq!(a.points, b.points, "the same faces");
            assert!(a.points.len() == 3 && a.points.iter().all(|&p| p < 14));
        }
        assert_eq!(EPOLYS[0].points, [0, 1, 8]);
        assert_eq!(EPOLYS[23].points, [6, 5, 9]);
    }

    /// `adddot`'s skip of a repeated neighbour, and the closing duplicate's
    /// drop (`VEC.ASM:539-543`). The stored triangle is visible, and Glz2
    /// reduces 0x4002 to 1.
    #[test]
    fn build_skips_a_repeated_neighbour_and_drops_the_closing_duplicate() {
        let projected = [vert(0, 0), vert(10, 0), vert(0, 10)];
        for points in [&[0u16, 1, 1, 2][..], &[0, 1, 2, 0]] {
            let polys = build_glz2(&[def(0x4002, points)], &projected);
            assert_eq!(polys[0].points, [(0, 0), (10, 0), (0, 10)], "{points:?}");
            assert_eq!(polys[0].colour, 1);
        }
    }

    /// The tracker is carried across polygons and counts the dropped closing
    /// duplicate, so the next polygon's leading repeat vanishes too.
    #[test]
    fn build_carries_the_last_index_across_polygons() {
        let projected = [vert(0, 0), vert(10, 0), vert(0, 10), vert(5, 20), vert(30, 5)];
        let polys = build_glz2(&[def(0x4002, &[0, 1, 2, 0]), def(0x4004, &[0, 3, 4])], &projected);
        assert_eq!(polys[0].points, [(0, 0), (10, 0), (0, 10)]);
        assert_eq!(polys[1].points, [(5, 20), (30, 5)]);
        // Two stored pairs give cross 0 (the port's guard): visible, so 0.
        assert_eq!(polys[1].colour, 0);
    }

    /// `build` hands the colour rule the first three stored points' cross
    /// product: a reversed winding keeps its word under Glz2.
    #[test]
    fn build_applies_the_colour_rule_to_the_cross_product() {
        let projected = [vert(0, 0), vert(0, 10), vert(10, 0), vert(0, 0)];
        let polys = build_glz2(&[def(0x4002, &[0, 1, 2]), def(0x4004, &[3, 2, 1])], &projected);
        assert_eq!((polys[0].colour, polys[1].colour), (0x4002, 0));
    }

    /// The cross product on hand-computed triangles, and both signs.
    #[test]
    fn cross_is_the_signed_product_of_the_first_three_points() {
        assert_eq!(cross(&[(0, 0), (10, 0), (0, 10)]), 100);
        assert_eq!(cross(&[(0, 0), (0, 10), (10, 0)]), -100);
        assert_eq!(cross(&[(0, 0), (10, 10), (20, 20)]), 0);
        // 3·3 − 3·(−3) = 18; 4·(−2) − (−5)·(−4) = −28.
        assert_eq!(cross(&[(4, 4), (1, 1), (7, 1)]), 18);
        assert_eq!(cross(&[(5, 2), (1, 7), (9, 4)]), -28);
        // Fewer than three pairs: the port's guard (`glenz.md` §8).
        assert_eq!(cross(&[(0, 0), (10, 0)]), 0);
        assert_eq!(cross(&[]), 0);
    }

    /// The coordinate differences wrap at 16 bits before the sign-extending
    /// multiply (`VEC.ASM:281-290`).
    #[test]
    fn cross_wraps_the_coordinate_differences_at_16_bits() {
        // x0−x1 = −65535 stores as 1, times y0−y2 = −32767.
        assert_eq!(cross(&[(-32768, 0), (32767, 0), (0, 32767)]), -32767);
        assert_eq!(cross(&[(32767, 0), (-32768, 0), (0, 32767)]), 32767);
        // y0−y1 and x0−x2 store as −32768: the second product is 2^30.
        assert_eq!(cross(&[(0, 0), (0, -32768), (-32768, 0)]), -1_073_741_824);
    }

    /// `demo_glz2`: a visible face is `(word >> 1) & 1`, a hidden one keeps
    /// its word, and nothing is allocated or written to the DAC.
    #[test]
    fn glz2_reduces_the_visible_word_and_keeps_the_hidden_one() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        for (colour, visible) in [(0x4002, 1), (0x4004, 0), (0x4006, 1), (0x4030, 0)] {
            let mut face = |c| state.face(colour, c, DemoMode::Glz2, 9, &NO_BACKPAL, &mut palette);
            assert_eq!(face(100), visible, "{colour:#x}");
            assert_eq!(face(-100), colour, "{colour:#x} hidden");
        }
        assert_eq!(palette, Palette::new());
        assert!(state.slots.iter().all(|&slot| slot == 0));
        assert!(state.used.iter().all(|&used| !used));
    }

    /// `demo_glz`'s visible path takes slots from 2 upward, returns the
    /// group's DAC index, and keeps an allocated colour's slot (`jne @@rc1`).
    #[test]
    fn glz_allocates_even_slots_in_order() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        for (colour, start) in [(0x4002, 16), (0x4004, 32), (0x4006, 48), (0x4004, 32)] {
            assert_eq!(glz(&mut state, &mut palette, colour, 100, &NO_BACKPAL), start);
        }
        let slots = [0x4002, 0x4004, 0x4006].map(|colour| state.slot_of(colour));
        assert_eq!(slots, [2, 4, 6]);
        assert_eq!(state.used.iter().filter(|&&used| used).count(), 3);
    }

    /// The two paths share the low-byte map: a hidden face frees the slot
    /// the same word's visible face took, and the next new colour reuses it.
    #[test]
    fn glz_hidden_face_frees_the_visible_slot() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        assert_eq!(glz(&mut state, &mut palette, 0x4002, 100, &NO_BACKPAL), 16);
        assert!(state.used[2]);
        // Hidden: 0x4002's bit 1 is set, so the reduced word is 4.
        assert_eq!(glz(&mut state, &mut palette, 0x4002, -100, &NO_BACKPAL), 4);
        assert_eq!(state.slot_of(0x4002), 0);
        assert!(!state.used[2]);
        assert_eq!(glz(&mut state, &mut palette, 0x4004, 100, &NO_BACKPAL), 16);
        assert_eq!(state.slot_of(0x4004), 2);
        assert!(!state.used[4]);
    }

    /// Sixteen distinct visible words with no hidden face between them: the
    /// fifteen slots 2..30 fill, then slot 32 is colour 0 and its group is
    /// written at DAC entry 0. The shipped animation never reaches this (its
    /// hidden faces free slots), but the scan's exhausted branch is pinned.
    #[test]
    fn glz_pool_exhaustion_uses_slot_32_and_entry_0() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[4, 8, 12]; 16];
        for i in 0..15u16 {
            assert_eq!(glz(&mut state, &mut palette, 0x4002 + 2 * i, 0, &backpal), 16 + 16 * i);
        }
        palette = Palette::new();
        assert_eq!(glz(&mut state, &mut palette, 0x4020, 0, &backpal), 0);
        assert_eq!(state.slot_of(0x4020), 32);
        assert!(state.used[32]);
        // Shade 0, bit 1 clear: base 0 plus the backpal row >> 2.
        assert_group(&palette, 0, [1, 2, 3]);
        assert!((16..=255u8).all(|index| palette.entry(index) == [0, 0, 0]));
    }

    /// A reused slot's group at `lightshift` 9: cross 256 is shade 2, bit 1
    /// set gives R=7, G=1, B=2, plus `backpal >> 2` = [2,1,3] on every entry
    /// (the base restored each time), and nothing outside the group.
    #[test]
    fn glz_writes_the_backpal_ramp_at_slot_shifted_by_3() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[8, 4, 12]; 16];
        assert_eq!(glz(&mut state, &mut palette, 0x4002, 256, &backpal), 16);
        assert_eq!(glz(&mut state, &mut palette, 0x4002, 256, &backpal), 16);
        assert_group(&palette, 16, [9, 2, 5]);
        assert!((0..16u8).chain(32..=255).all(|index| palette.entry(index) == [0, 0, 0]));
    }

    /// Each written value clamps to 63: shade 63 plus `60 >> 2` (a cumulative
    /// 8-bit ramp would wrap at entry 12).
    #[test]
    fn glz_group_ramp_restores_the_base_and_clamps_each_entry() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[60, 0, 0]; 16];
        glz(&mut state, &mut palette, 0x4004, 10000, &backpal);
        assert_eq!(glz(&mut state, &mut palette, 0x4004, 10000, &backpal), 16);
        assert_group(&palette, 16, [63, 63, 63]);
    }

    /// A fresh slot's base is the allocation scan's leavings
    /// (`VEC.ASM:454-460`): shade 0 (`mov ax,bp` clears AH) and, for a bit-1
    /// word, R = 16 − slot/2 (`loop @@rc3` leaves CL); a bit-1-clear word's
    /// base is 0.
    #[test]
    fn glz_fresh_allocation_uses_the_scan_residual_and_zero_shade() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[8, 4, 12]; 16];
        assert_eq!(glz(&mut state, &mut palette, 0x4002, 256, &backpal), 16);
        assert_group(&palette, 16, [17, 1, 3]);
        assert_eq!(glz(&mut state, &mut palette, 0x4006, 10000, &backpal), 32);
        assert_group(&palette, 32, [16, 1, 3]);
        assert_eq!(glz(&mut state, &mut palette, 0x4004, 10000, &backpal), 48);
        assert_group(&palette, 48, [2, 1, 3]);
    }

    /// `lightshift != 9`: the literal two-shift shade, `ax1 + ax2` with
    /// `ax1 = low16(cross >> 8)` and `ax2 = (ax1 >> 1) | (high16(cross) << 15)`
    /// clamped to 0..63, and R's base is `lightshift`.
    #[test]
    fn glz_lightshift_10_uses_the_two_shift_shade_and_its_r_base() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let mut face = |palette: &mut Palette, c| {
            state.face(0x4002, c, DemoMode::Glz, 10, &NO_BACKPAL, palette)
        };
        // The first face takes slot 2 (fresh); the rest reuse it.
        assert_eq!(face(&mut palette, 0), 16);
        for (cross, group) in [
            // 5 + 2 = 7.
            (1280, [10, 3, 7]),
            // ax1 = −1 and the odd high word make ax2 = −1: −2 clamps to 0.
            (0x7fff_ffff, [10, 0, 0]),
            // 80 + 40 clamps to 63.
            (0x5000, [10, 31, 63]),
            // ax1 = 257, ax2 = 0x0080 | 0x8000: negative, 0. The
            // `low16(cross >> 9)` form would give 385 → 63.
            (0x0001_0100, [10, 0, 0]),
        ] {
            assert_eq!(face(&mut palette, cross), 16);
            assert_group(&palette, 16, group);
        }
    }
}
