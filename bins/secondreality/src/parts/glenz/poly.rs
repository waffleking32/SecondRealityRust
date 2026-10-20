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
//! `demo_glz` keeps one pool of even DAC groups, slots 2..30 (and 32 when they
//! are all taken), in `rolused`, and two ways of keying `rolcol`: the visible
//! path zero-extends the **full colour word** (`movzx bx,es:[bx-2]`,
//! `VEC.ASM:450`), the hidden path uses the **low byte** (`mov bl,al`,
//! `VEC.ASM:417`). The source's `rolcol` is only 256 bytes, so the two keys
//! address different memory; this port models the two key spaces explicitly —
//! a full-word map and a 256-byte low-byte table — over the one pool. As the
//! Phase 7 brief pins, a visible allocation records its slot under both keys;
//! a hidden face reads and clears only the low-byte entry and the pool bit,
//! leaving the full-word entry behind. That asymmetry is the allocator's
//! behaviour, not an accident of this port.
//!
//! The visible path also writes the face's 16-entry DAC group: the group's
//! shade comes from the cross product, its base from the colour word's bit 1,
//! and each entry adds one `backpal` row (shifted right twice) to a cumulative
//! 8-bit accumulator, clamped to 63 when written.
//!
//! # Source disagreements recorded, not silently resolved
//!
//! The vendored development snapshot (`VEC.ASM`, and the tree's own unpacked
//! `V.EXE`) differs from the Phase 7 brief in three places: its `demo_glz2`
//! reduces the **visible** face (`jc @@1` skips the hidden one), stores
//! `(al>>1)&1` without the `<<2`, and its `REPT` ramp restores the base
//! between entries (`mov bx,di; mov cx,si`, `VEC.ASM:498-499`), making the
//! group non-cumulative. The brief — and the plan's colour-path row — pins
//! the other behaviour: hidden → 0/4, a cumulative wrapping ramp. The port
//! follows the brief; the shipped release's part (the PKLITE-packed
//! `MAIN/DATA/GLENZ.EXE`) is the D24 tiebreaker and is left to Task 7.

// The part driver lands in Tasks 4-7; until it consumes this layer every item
// here is dead code to the binary target (the tests below use it).
#![allow(dead_code)]

use std::collections::HashMap;

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

/// The colour allocator's persistent state: `rolcol`, `rolused` and the
/// frame's `lightshift` (`VEC.ASM:396-397`; `MAIN.C:394`).
///
/// The source keeps one 256-byte `rolcol` indexed two ways (see the module
/// docs). This port keeps the two key spaces apart — `visible` is the
/// full-word map, `hidden` the low-byte table — over the one `used` pool.
/// `used` has one entry per DAC group; slots are the even numbers 2..30, plus
/// 32 for the exhausted case.
pub struct ColourState {
    /// `_lightshift` (`MAIN.C:394`, `:500`): 9, or 10 once the second
    /// object's scale overtakes the first's.
    pub lightshift: i16,
    /// The visible path's slots, keyed by the full 16-bit colour word.
    visible: HashMap<u16, u8>,
    /// The hidden path's slots, keyed by the colour word's low byte.
    hidden: [u8; 256],
    /// `rolused`: whether a slot's DAC group is allocated.
    used: [bool; 256],
}

impl Default for ColourState {
    fn default() -> Self {
        Self::new()
    }
}

impl ColourState {
    /// A fresh allocator: `lightshift` 9 (`MAIN.C:394`) and both tables
    /// zeroed (the source's `rolcol`/`rolused` start zeroed, `VEC.ASM:396-397`).
    /// The part owns one instance for the run, so the pool persists across
    /// frames.
    pub fn new() -> Self {
        ColourState {
            lightshift: 9,
            visible: HashMap::new(),
            hidden: [0; 256],
            used: [false; 256],
        }
    }

    /// Apply `demo_glz`/`demo_glz2` to one face.
    ///
    /// `cross` is the signed cross product from [`cross`]; negative is the
    /// asm's hidden face. Returns the colour word to store in the polygon
    /// list, and, for a visible face under [`DemoMode::Glz`], writes the
    /// face's 16 DAC entries.
    ///
    /// - [`DemoMode::Glz2`] visible keeps `colour`; hidden returns
    ///   `((colour >> 1) & 1) << 2` (0 or 4). No DAC writes.
    /// - [`DemoMode::Glz`] visible allocates or reuses a slot under the full
    ///   colour word, returns `(slot << 3) as u8` and writes the group;
    ///   hidden clears the slot stored under the colour's low byte and
    ///   `used[slot]` (harmless when that entry is 0) and returns
    ///   `((colour >> 1) & 1) << 2`.
    pub fn face(
        &mut self,
        colour: u16,
        cross: i32,
        mode: DemoMode,
        backpal: &[[u8; 3]; 16],
        palette: &mut Palette,
    ) -> u16 {
        if cross < 0 {
            // `demo_glz`'s hidden branch (`VEC.ASM:413-429`) frees the slot
            // stored under the low byte; `demo_glz2` (`:403-410`) leaves the
            // allocator alone. Both store the reduced word.
            if mode == DemoMode::Glz {
                self.hidden_face(colour);
            }
            return ((colour >> 1) & 1) << 2;
        }
        match mode {
            DemoMode::Glz2 => colour,
            DemoMode::Glz => self.visible_face(colour, cross, backpal, palette),
        }
    }

    /// `demo_glz`'s visible branch (`VEC.ASM:430-505`): take the full-word
    /// slot (allocating the first free even slot 2..30, or 32 when they are
    /// all taken), return its DAC base and write the group's 16 entries.
    fn visible_face(
        &mut self,
        colour: u16,
        cross: i32,
        backpal: &[[u8; 3]; 16],
        palette: &mut Palette,
    ) -> u16 {
        let slot = match self.visible.get(&colour) {
            // `mov al,cs:rolcol[bx]; cmp al,0; jne @@rc1` (`VEC.ASM:450-453`):
            // a stored slot is reused without touching the pool bit.
            Some(&slot) => slot,
            None => {
                // `mov cx,15; mov bp,2` … `loop @@rc3` (`VEC.ASM:454-459`):
                // fifteen even candidates. The loop exits with bp = 32 when
                // none is free.
                let slot = (2u8..=30)
                    .step_by(2)
                    .find(|&candidate| !self.used[candidate as usize])
                    .unwrap_or(32);
                // `mov cs:rolcol[bx],al; mov cs:rolused[bp],1`
                // (`VEC.ASM:461-462`), recorded in both key spaces.
                self.visible.insert(colour, slot);
                self.hidden[colour as u8 as usize] = slot;
                self.used[slot as usize] = true;
                slot
            }
        };

        // `shl al,3` (`VEC.ASM:465`): the group's first DAC entry; slot 32
        // wraps to 0 in the byte store.
        let start = (u16::from(slot) << 3) as u8;

        // The shade (`VEC.ASM:430-446`): `low16(cross >> 7)` when
        // lightshift is 9, otherwise the wrapping 16-bit sum of
        // `low16(cross >> 8)` and `low16(cross >> 9)`; the signed word is
        // clamped to 0..63.
        let shade = if self.lightshift == 9 {
            ((cross >> 7) as i16).clamp(0, 63)
        } else {
            ((cross >> 8) as i16)
                .wrapping_add((cross >> 9) as i16)
                .clamp(0, 63)
        } as u8;

        // `mov cl,cs:_lightshift; cmp cl,9; je @@x9; … mov cl,7`
        // (`VEC.ASM:430-440`): R's base is 7 in the first phase, otherwise
        // the lightshift itself.
        let r_base = if self.lightshift == 9 {
            7
        } else {
            self.lightshift as u8
        };
        // `test bp,2` (`VEC.ASM:472-480`): bit 1 of the colour word selects
        // the blue/grey base (R, shade/2, shade); clear selects all-shade.
        let base = if colour & 2 != 0 {
            [r_base, shade >> 1, shade]
        } else {
            [shade, shade, shade]
        };

        // The 16-entry group (`VEC.ASM:486-501`): each entry adds one row of
        // `backpal` shifted right twice to an 8-bit accumulator, and the
        // value written is that accumulator clamped to 63. The accumulator is
        // not written back by the DAC output, so it keeps its unclamped
        // (wrapped) value for the next entry.
        let mut accumulator = base;
        for (j, back) in backpal.iter().enumerate() {
            accumulator[0] = accumulator[0].wrapping_add(back[0] >> 2);
            accumulator[1] = accumulator[1].wrapping_add(back[1] >> 2);
            accumulator[2] = accumulator[2].wrapping_add(back[2] >> 2);
            palette.set_entry(
                start.wrapping_add(j as u8),
                [
                    accumulator[0].min(63),
                    accumulator[1].min(63),
                    accumulator[2].min(63),
                ],
            );
        }

        // `mov es:[bx-2],al` (`VEC.ASM:466`): the stored colour is the group
        // base (the source keeps the word's high byte, which the pixel
        // compositor never reads).
        u16::from(start)
    }

    /// `demo_glz`'s hidden branch (`VEC.ASM:413-429`): key `rolcol` by the
    /// colour's low byte, clear that entry and the slot's `used` bit. When
    /// the entry is 0 the slot is 0 and `used[0]` is cleared, which the
    /// allocator never reads.
    fn hidden_face(&mut self, colour: u16) {
        let key = colour as u8;
        let slot = self.hidden[key as usize];
        self.hidden[key as usize] = 0;
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
        let colour = state.face(def.colour, cross(&points), mode, backpal, palette);
        polys.push(Poly { colour, points });
    }
    polys
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A projected vertex with the flags unused by this layer.
    fn vert(x: i16, y: i16) -> PVert {
        PVert {
            x,
            y,
            flags: 0,
            z: 128,
        }
    }

    const NO_BACKPAL: [[u8; 3]; 16] = [[0, 0, 0]; 16];

    fn def(colour: u16, points: &'static [u16]) -> PolyDef {
        PolyDef { colour, points }
    }

    /// `MAIN.C:176-207` and `:209-240`, pinned: 24 triangles each, the
    /// source's colours and order.
    #[test]
    fn the_shipped_polygon_tables_are_the_sources() {
        assert_eq!(EPOLYS.len(), 24);
        assert_eq!(EPOLYSB.len(), 24);
        for (i, def) in EPOLYS.iter().enumerate() {
            assert_eq!(def.colour, 0x4002 + 2 * i as u16);
            assert_eq!(def.points.len(), 3);
            assert!(def.points.iter().all(|&p| p < 14));
        }
        for (i, def) in EPOLYSB.iter().enumerate() {
            let expected = if i % 2 == 0 { 0x4004 } else { 0x4002 };
            assert_eq!(def.colour, expected);
            assert_eq!(def.points.len(), 3);
            assert!(def.points.iter().all(|&p| p < 14));
        }
        assert_eq!(EPOLYS[0].points, [0, 1, 8]);
        assert_eq!(EPOLYS[23].points, [6, 5, 9]);
        assert_eq!(EPOLYSB[0].points, [0, 1, 8]);
        assert_eq!(EPOLYSB[23].points, [6, 5, 9]);
    }

    /// `adddot`'s skip: a point equal to the one just copied is not stored.
    #[test]
    fn build_skips_a_repeated_neighbour() {
        let projected = [vert(0, 0), vert(10, 0), vert(0, 10), vert(20, 20)];
        let defs = [def(0x4002, &[0, 1, 1, 2])];
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let polys = build(
            &defs,
            &projected,
            DemoMode::Glz2,
            &mut state,
            &NO_BACKPAL,
            &mut palette,
        );
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0].points, [(0, 0), (10, 0), (0, 10)]);
        assert_eq!(polys[0].colour, 0x4002);
    }

    /// The closing duplicate: the last stored pair is dropped when it equals
    /// the first (`VEC.ASM:539-543`).
    #[test]
    fn build_drops_the_closing_duplicate() {
        let projected = [vert(0, 0), vert(10, 0), vert(0, 10)];
        let defs = [def(0x4002, &[0, 1, 2, 0])];
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let polys = build(
            &defs,
            &projected,
            DemoMode::Glz2,
            &mut state,
            &NO_BACKPAL,
            &mut palette,
        );
        assert_eq!(polys[0].points, [(0, 0), (10, 0), (0, 10)]);
    }

    /// The tracker is carried across polygons, and it is the *stored* index
    /// that counts: the dropped closing duplicate still becomes the last
    /// index, so the next polygon's leading repeat vanishes too.
    #[test]
    fn build_carries_the_last_index_across_polygons() {
        let projected = [
            vert(0, 0),
            vert(10, 0),
            vert(0, 10),
            vert(5, 20),
            vert(30, 5),
        ];
        let defs = [def(0x4002, &[0, 1, 2, 0]), def(0x4004, &[0, 3, 4])];
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let polys = build(
            &defs,
            &projected,
            DemoMode::Glz2,
            &mut state,
            &NO_BACKPAL,
            &mut palette,
        );
        assert_eq!(polys[0].points, [(0, 0), (10, 0), (0, 10)]);
        // The second polygon's leading 0 equals the closing duplicate the
        // first polygon dropped, so only its last two vertices are stored.
        assert_eq!(polys[1].points, [(5, 20), (30, 5)]);
        assert_eq!(polys[1].colour, 0x4004);
    }

    /// The cross product on hand-computed triangles, and both signs.
    #[test]
    fn cross_is_the_signed_product_of_the_first_three_points() {
        // (0−10)(0−10) − (0−0)(0−0) = 100.
        assert_eq!(cross(&[(0, 0), (10, 0), (0, 10)]), 100);
        // The reversed winding is hidden.
        assert_eq!(cross(&[(0, 0), (0, 10), (10, 0)]), -100);
        assert!(cross(&[(0, 0), (0, 10), (10, 0)]) < 0);
        // Collinear.
        assert_eq!(cross(&[(0, 0), (10, 10), (20, 20)]), 0);
        // Both products nonzero: 3·3 − 3·(−3) = 18.
        assert_eq!(cross(&[(4, 4), (1, 1), (7, 1)]), 18);
        // 4·(−2) − (−5)·(−4) = −28.
        assert_eq!(cross(&[(5, 2), (1, 7), (9, 4)]), -28);
        // Fewer than three pairs: the port's guard (the source reads past the
        // record).
        assert_eq!(cross(&[(0, 0), (10, 0)]), 0);
        assert_eq!(cross(&[]), 0);
    }

    /// The coordinate differences wrap at 16 bits before the sign-extending
    /// multiply (`sub ax,…` / `imul si`, `VEC.ASM:281-290`).
    #[test]
    fn cross_wraps_the_coordinate_differences_at_16_bits() {
        // x0−x1 = −32768−32767 = −65535, which stores as 1. The product with
        // y0−y2 = −32767 is −32767; the second product is 0.
        assert_eq!(cross(&[(-32768, 0), (32767, 0), (0, 32767)]), -32767);
        // The other direction stores as −1: (−1)·(−32767) = 32767.
        assert_eq!(cross(&[(32767, 0), (-32768, 0), (0, 32767)]), 32767);
        // y0−y1 = 32768 stores as −32768 and x0−x2 as −32768, so the second
        // product is 2^30 and the first is 0.
        assert_eq!(cross(&[(0, 0), (0, -32768), (-32768, 0)]), -1_073_741_824);
    }

    /// `demo_glz2` (`VEC.ASM:403-410`): visible keeps the colour word, hidden
    /// returns `((word >> 1) & 1) << 2`, and nothing is allocated or written
    /// to the DAC.
    #[test]
    fn glz2_keeps_the_visible_word_and_reduces_the_hidden_one() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let face = |state: &mut ColourState, palette: &mut Palette, colour, c| {
            state.face(colour, c, DemoMode::Glz2, &NO_BACKPAL, palette)
        };

        assert_eq!(face(&mut state, &mut palette, 0x4002, 100), 0x4002);
        assert_eq!(face(&mut state, &mut palette, 0x4004, 100), 0x4004);
        // Bit 1 set reduces to 4, clear to 0.
        assert_eq!(face(&mut state, &mut palette, 0x4002, -100), 4);
        assert_eq!(face(&mut state, &mut palette, 0x4004, -100), 0);
        assert_eq!(face(&mut state, &mut palette, 0x4006, -100), 4);
        assert_eq!(face(&mut state, &mut palette, 0x4008, -100), 0);
        assert_eq!(face(&mut state, &mut palette, 0x4030, -100), 0);

        assert_eq!(palette, Palette::new());
        assert!(state.visible.is_empty());
        assert!(state.hidden.iter().all(|&slot| slot == 0));
        assert!(state.used.iter().all(|&used| !used));
    }

    /// `demo_glz`'s visible path: slots are taken from 2 upward in allocation
    /// order, and the returned colour is the group's DAC index.
    #[test]
    fn glz_allocates_even_slots_in_order() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let face = |state: &mut ColourState, palette: &mut Palette, colour| {
            state.face(colour, 100, DemoMode::Glz, &NO_BACKPAL, palette)
        };

        assert_eq!(face(&mut state, &mut palette, 0x4002), 16);
        assert_eq!(face(&mut state, &mut palette, 0x4004), 32);
        assert_eq!(face(&mut state, &mut palette, 0x4006), 48);
        assert_eq!(state.visible.get(&0x4002), Some(&2));
        assert_eq!(state.visible.get(&0x4004), Some(&4));
        assert_eq!(state.visible.get(&0x4006), Some(&6));
        assert!(state.used[2] && state.used[4] && state.used[6]);
        // A colour already allocated keeps its slot (`jne @@rc1`).
        assert_eq!(face(&mut state, &mut palette, 0x4004), 32);
        assert_eq!(state.visible.len(), 3);
    }

    /// The two key spaces: a visible face stores the slot under the full word
    /// and the low byte; a hidden face of the same word frees the low-byte
    /// entry and the pool bit, not the full-word entry.
    #[test]
    fn glz_hidden_frees_the_low_byte_entry_not_the_full_word_entry() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let colour = 0x4002;

        assert_eq!(
            state.face(colour, 100, DemoMode::Glz, &NO_BACKPAL, &mut palette),
            16
        );
        assert_eq!(state.visible.get(&colour), Some(&2));
        assert_eq!(state.hidden[colour as u8 as usize], 2);
        assert!(state.used[2]);

        // Hidden: `0x4002`'s bit 1 is set, so the reduced word is 4.
        assert_eq!(
            state.face(colour, -100, DemoMode::Glz, &NO_BACKPAL, &mut palette),
            4
        );
        assert_eq!(state.hidden[colour as u8 as usize], 0);
        assert!(!state.used[2]);
        // The full-word entry the visible path wrote is untouched: the hidden
        // path keyed the low byte.
        assert_eq!(state.visible.get(&colour), Some(&2));
    }

    /// A freed slot is reused by the next new allocation: allocate three,
    /// hide the middle, allocate a fourth.
    #[test]
    fn glz_reuses_a_slot_freed_by_a_hide() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let face = |state: &mut ColourState, palette: &mut Palette, colour, c| {
            state.face(colour, c, DemoMode::Glz, &NO_BACKPAL, palette)
        };

        assert_eq!(face(&mut state, &mut palette, 0x4002, 100), 16);
        assert_eq!(face(&mut state, &mut palette, 0x4004, 100), 32);
        assert_eq!(face(&mut state, &mut palette, 0x4006, 100), 48);
        // 0x4004 has bit 1 clear, so its hidden word is 0.
        assert_eq!(face(&mut state, &mut palette, 0x4004, -100), 0);
        assert!(!state.used[4]);
        // The first free slot from 2 upward is 4, so the new colour gets it.
        assert_eq!(face(&mut state, &mut palette, 0x4008, 100), 32);
        assert_eq!(state.visible.get(&0x4008), Some(&4));
        assert!(state.used[4]);
        // The next one takes 8 (2, 4 and 6 are taken again).
        assert_eq!(face(&mut state, &mut palette, 0x400a, 100), 64);
    }

    /// Sixteen simultaneously allocated colours: the 15 even slots 2..30 fill
    /// first, then the sixteenth gets slot 32 — colour 0, and its group is
    /// written at DAC entry 0.
    #[test]
    fn glz_pool_exhaustion_uses_slot_32_and_entry_0() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[4, 8, 12]; 16];

        for i in 0..15u16 {
            let colour = 0x4002 + 2 * i;
            let expected = 16 + 16 * i;
            assert_eq!(
                state.face(colour, 0, DemoMode::Glz, &backpal, &mut palette),
                expected,
                "allocation {i}"
            );
        }
        for slot in (2u8..=30).step_by(2) {
            assert!(state.used[slot as usize], "slot {slot}");
        }

        // The sixteenth colour finds no free even slot in 2..30.
        palette = Palette::new();
        assert_eq!(
            state.face(0x4020, 0, DemoMode::Glz, &backpal, &mut palette),
            0
        );
        assert_eq!(state.visible.get(&0x4020), Some(&32));
        assert!(state.used[32]);
        // The group starts at `(32 << 3) as u8 == 0`. Shade 0, bit 1 clear:
        // base [0,0,0], and each entry adds one backpal row: [1,2,3],
        // [2,4,6], … [16,32,48].
        assert_eq!(palette.entry(0), [1, 2, 3]);
        assert_eq!(palette.entry(15), [16, 32, 48]);
        // Nothing beyond the group was written.
        for index in 16..=255u8 {
            assert_eq!(palette.entry(index), [0, 0, 0], "entry {index}");
        }
    }

    /// The visible group's ramp: `lightshift == 9`, a known shade and
    /// backpal, and the group at `slot << 3`.
    #[test]
    fn glz_writes_the_cumulative_backpal_ramp_at_slot_shifted_by_3() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[8, 4, 12]; 16];

        // cross 256 → shade (256 >> 7) = 2. 0x4002 has bit 1 set, so the base
        // is R=7, G=shade/2=1, B=shade=2. The first slot is 2: the group is
        // entries 16..31.
        assert_eq!(
            state.face(0x4002, 256, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        // Each entry adds backpal[j] >> 2 = [2,1,3] cumulatively.
        for j in 0..16u16 {
            assert_eq!(
                palette.entry(16 + j as u8),
                [
                    7 + 2 * (j + 1) as u8,
                    1 + (j + 1) as u8,
                    2 + 3 * (j + 1) as u8
                ],
                "entry {j}"
            );
        }
        // Nothing before or after the group was touched.
        for index in 0..16u8 {
            assert_eq!(palette.entry(index), [0, 0, 0]);
        }
        for index in 32..=255u8 {
            assert_eq!(palette.entry(index), [0, 0, 0]);
        }
    }

    /// The ramp's accumulator is an unclamped u8 that keeps its wrapped value
    /// between entries; only the written value is clamped to 63.
    #[test]
    fn glz_group_ramp_wraps_the_u8_and_clamps_each_written_entry() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[60, 0, 0]; 16];

        // cross 10000 → shade 63. 0x4004 has bit 1 clear: R=G=B=63.
        assert_eq!(
            state.face(0x4004, 10000, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        // R accumulates 63 + 15 per entry: entries 0..11 write the clamp 63,
        // entry 12 wraps 258 to 2, then 17, 32, 47.
        for j in 0..12u8 {
            assert_eq!(palette.entry(16 + j), [63, 63, 63], "entry {j}");
        }
        assert_eq!(palette.entry(28), [2, 63, 63]);
        assert_eq!(palette.entry(29), [17, 63, 63]);
        assert_eq!(palette.entry(30), [32, 63, 63]);
        assert_eq!(palette.entry(31), [47, 63, 63]);
    }

    /// `lightshift != 9` uses `low16(cross >> 9) + low16(cross >> 8)` as a
    /// wrapping 16-bit sum, clamped signed to 0..63, and R's base becomes
    /// `lightshift`.
    #[test]
    fn glz_lightshift_10_uses_the_two_shift_shade_and_its_r_base() {
        let mut state = ColourState::new();
        state.lightshift = 10;
        let mut palette = Palette::new();
        let face = |state: &mut ColourState, palette: &mut Palette, c| {
            state.face(0x4002, c, DemoMode::Glz, &NO_BACKPAL, palette)
        };

        // 1280 >> 8 = 5, >> 9 = 2, shade 7. Bit 1 set: R=10, G=3, B=7.
        assert_eq!(face(&mut state, &mut palette, 1280), 16);
        for j in 0..16u8 {
            assert_eq!(palette.entry(16 + j), [10, 3, 7], "entry {j}");
        }

        // The sum wraps at 16 bits: both shifts' low words are 0xFFFF (−1),
        // so the shade is −2 and clamps to 0.
        assert_eq!(face(&mut state, &mut palette, 0x7fff_ffff), 16);
        for j in 0..16u8 {
            assert_eq!(palette.entry(16 + j), [10, 0, 0], "entry {j}");
        }

        // 0x5000 >> 8 = 80, >> 9 = 40, sum 120 clamps to 63; G = 31.
        assert_eq!(face(&mut state, &mut palette, 0x5000), 16);
        for j in 0..16u8 {
            assert_eq!(palette.entry(16 + j), [10, 31, 63], "entry {j}");
        }
    }

    /// `build` hands the colour rule the cross product of the polygon's first
    /// three stored points: a reversed winding is hidden to `Glz2`.
    #[test]
    fn build_applies_the_colour_rule_to_the_cross_product() {
        let projected = [vert(0, 0), vert(0, 10), vert(10, 0), vert(0, 0)];
        let defs = [def(0x4002, &[0, 1, 2]), def(0x4004, &[3, 2, 1])];
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let polys = build(
            &defs,
            &projected,
            DemoMode::Glz2,
            &mut state,
            &NO_BACKPAL,
            &mut palette,
        );
        // The first winding is negative (hidden), the second positive.
        assert_eq!(polys[0].colour, 4);
        assert_eq!(polys[1].colour, 0x4004);
    }
}
