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
//! are all taken), in `rolused`, keyed in `rolcol` by the colour word. The
//! tree's visible path zero-extends the **full colour word**
//! (`movzx bx,es:[bx-2]`, `VEC.ASM:450`) and its hidden path uses the **low
//! byte** (`mov bl,al`, `VEC.ASM:417`); with a 256-byte `rolcol` the full-word
//! keys fall outside it and the two maps are independent, so the visible map
//! accumulates every word ever seen and the fifteen-slot pool exhausts — a
//! sixteenth word gets slot 32, whose DAC group starts at entry 0 (clobbering
//! the low palette) and whose stored colour is 0 (painting nothing). The
//! reference contradicts that: at 2300 its low palette is exactly the frame's
//! `pal` and every face is drawn (D24's arbitration, Task 7). The reference's
//! picture therefore identifies the shipped build's reading as the **in-bounds
//! low-byte key on both paths** (the packed EXE cannot be disassembled to
//! confirm it), and the port adopts that reading: one 256-byte map over the
//! one `used` pool, a hidden face freeing the slot its visible face took. The
//! residual evidence and the two model outputs are in `docs/fidelity/glenz.md`
//! §7 and §11 and `APPROXIMATIONS.md` A37.
//!
//! The visible path also writes the face's 16-entry DAC group: the group's
//! base comes from the colour word's bit 1 and the cross-product shade — a
//! reused slot keeps the computed shade and `R = 7|lightshift`, while a fresh
//! allocation's base is the allocation scan's leavings: `mov ax,bp`
//! (`VEC.ASM:460`) clears AH, where `mov ah,al` (`:447`) had put the shade,
//! and the scan's `mov cx,15`/`loop @@rc3` (`:454-459`) has overwritten CL
//! with `16 − slot/2`, so a fresh group is built with shade 0 and, for a
//! bit-1 word, that residual as R — and entry j is the base plus
//! one `backpal` row shifted right twice — the
//! source restores the base every entry (`mov bx,di; mov cx,si` after each
//! `setpalxxx`, `VEC.ASM:498-499`), so the ramp is not cumulative — with each
//! channel clamped to 63 when written.
//!
//! `demo_glz2` reduces a **visible** face to `(word >> 1) & 1` and leaves a
//! hidden face's word unchanged: `jc @@1` returns on the hidden carry
//! (`VEC.ASM:403-410`).

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
/// `slots` is the reading the reference's picture identifies — `rolcol` read
/// in bounds by the colour word's **low byte** (see the module docs; D24):
/// the visible and hidden paths share it, so a hidden face frees the slot its
/// visible face took. `used` has one entry per DAC group; slots are the even
/// numbers 2..30, plus 32 for the exhausted case.
pub struct ColourState {
    /// `_lightshift` (`MAIN.C:394`, `:500`): 9, or 10 once the second
    /// object's scale overtakes the first's.
    pub lightshift: i16,
    /// The slots, keyed by the colour word's low byte (the reading the
    /// reference's picture identifies; D24). Both paths share it.
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
    /// A fresh allocator: `lightshift` 9 (`MAIN.C:394`) and both tables
    /// zeroed (the source's `rolcol`/`rolused` start zeroed, `VEC.ASM:396-397`).
    /// The part owns one instance for the run, so the pool persists across
    /// frames.
    pub fn new() -> Self {
        ColourState {
            lightshift: 9,
            slots: [0; 256],
            used: [false; 256],
        }
    }

    /// Test-only: the slot stored under a colour word's low byte.
    #[cfg(test)]
    pub(crate) fn slot_of(&self, colour: u16) -> u8 {
        self.slots[colour as u8 as usize]
    }

    /// Apply `demo_glz`/`demo_glz2` to one face.
    ///
    /// `cross` is the signed cross product from [`cross`]; negative is the
    /// asm's hidden face. Returns the colour word to store in the polygon
    /// list, and, for a visible face under [`DemoMode::Glz`], writes the
    /// face's 16 DAC entries.
    ///
    /// - [`DemoMode::Glz2`] visible returns `(colour >> 1) & 1`; hidden keeps
    ///   `colour` unchanged (`jc @@1` returns on the hidden carry). No DAC
    ///   writes.
    /// - [`DemoMode::Glz`] visible allocates or reuses a slot under the
    ///   colour word's low byte, returns `(slot << 3) as u8` and writes the
    ///   group; hidden clears the slot stored under the same low byte and
    ///   `used[slot]` (harmless when that entry is 0) and returns
    ///   `((colour >> 1) & 1) << 2`. The two paths share the map, so a hidden
    ///   face frees the slot its visible face took (D24's shipped reading).
    pub fn face(
        &mut self,
        colour: u16,
        cross: i32,
        mode: DemoMode,
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
                    // `demo_glz`'s hidden branch (`VEC.ASM:413-429`) frees
                    // the slot stored under the low byte and stores the
                    // reduced word.
                    self.hidden_face(colour);
                    ((colour >> 1) & 1) << 2
                } else {
                    self.visible_face(colour, cross, backpal, palette)
                }
            }
        }
    }

    /// `demo_glz`'s visible branch (`VEC.ASM:430-505`): take the slot stored
    /// under the colour word's low byte (allocating the first free even slot
    /// 2..30, or 32 when they are all taken), return its DAC base and write
    /// the group's 16 entries.
    ///
    /// The shade and R base in the group are the computed ones on a reused
    /// slot, but on a freshly allocated one they are what the allocation scan
    /// leaves behind: `mov cx,15; … loop @@rc3` (`VEC.ASM:454-459`)
    /// overwrites CL with `16 − slot/2`, and `mov ax,bp` (`:460`) clears AH
    /// (where `mov ah,al`, `:447`, had put the shade). So a fresh group has
    /// shade 0 and, for a bit-1 word, R = `16 − slot/2`; a bit-1-clear word's
    /// base is all zero (`mov cl,ah`). The reuse branch (`jne @@rc1`, `:453`)
    /// skips the scan and keeps CL = 7|lightshift and the computed shade.
    fn visible_face(
        &mut self,
        colour: u16,
        cross: i32,
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
        let computed_shade = if self.lightshift == 9 {
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
        } else if self.lightshift == 9 {
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

        // The 16-entry group (`VEC.ASM:486-501`): entry j is the base plus
        // one row of `backpal` shifted right twice, and the value written is
        // clamped to 63. The source restores the base after every entry
        // (`mov bx,di; mov cx,si`, `:498-499`), so the rows do not
        // accumulate; the add is an 8-bit register add.
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

    /// `demo_glz`'s hidden branch (`VEC.ASM:413-429`): key the low-byte map
    /// by the colour's low byte, clear that entry and the slot's `used` bit.
    /// When the entry is 0 the slot is 0 and `used[0]` is cleared, which the
    /// allocator never reads. The visible path uses the same map, so this
    /// frees the slot the face took when it was visible.
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
        // The stored triangle is visible (cross 100), and Glz2 reduces a
        // visible word to `(0x4002 >> 1) & 1` = 1.
        assert_eq!(polys[0].colour, 1);
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
        // Two stored pairs give cross 0 (the port's guard): visible, so
        // Glz2 reduces 0x4004 to 0.
        assert_eq!(polys[1].colour, 0);
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

    /// `demo_glz2` (`VEC.ASM:403-410`): `jc @@1` returns on the hidden carry,
    /// so a visible face is reduced to `(word >> 1) & 1` and a hidden face
    /// keeps its colour word. Nothing is allocated or written to the DAC.
    #[test]
    fn glz2_reduces_the_visible_word_and_keeps_the_hidden_one() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let face = |state: &mut ColourState, palette: &mut Palette, colour, c| {
            state.face(colour, c, DemoMode::Glz2, &NO_BACKPAL, palette)
        };

        // Bit 1 set reduces to 1, clear to 0.
        assert_eq!(face(&mut state, &mut palette, 0x4002, 100), 1);
        assert_eq!(face(&mut state, &mut palette, 0x4004, 100), 0);
        assert_eq!(face(&mut state, &mut palette, 0x4006, 100), 1);
        assert_eq!(face(&mut state, &mut palette, 0x4008, 100), 0);
        assert_eq!(face(&mut state, &mut palette, 0x4030, 100), 0);
        // Hidden faces are untouched.
        assert_eq!(face(&mut state, &mut palette, 0x4002, -100), 0x4002);
        assert_eq!(face(&mut state, &mut palette, 0x4004, -100), 0x4004);
        assert_eq!(face(&mut state, &mut palette, 0x4030, -100), 0x4030);

        assert_eq!(palette, Palette::new());
        assert!(state.slots.iter().all(|&slot| slot == 0));
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
        assert_eq!(state.slot_of(0x4002), 2);
        assert_eq!(state.slot_of(0x4004), 4);
        assert_eq!(state.slot_of(0x4006), 6);
        assert!(state.used[2] && state.used[4] && state.used[6]);
        // A colour already allocated keeps its slot (`jne @@rc1`).
        assert_eq!(face(&mut state, &mut palette, 0x4004), 32);
        assert!(state.used.iter().filter(|&&used| used).count() == 3);
    }

    /// The two paths share the low-byte map (D24's shipped reading): a hidden
    /// face frees the slot the same word's visible face took, so the next new
    /// colour reuses it instead of allocating a new one.
    #[test]
    fn glz_hidden_face_frees_the_visible_slot() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let colour = 0x4002;
        let face = |state: &mut ColourState, palette: &mut Palette, colour, c| {
            state.face(colour, c, DemoMode::Glz, &NO_BACKPAL, palette)
        };

        assert_eq!(face(&mut state, &mut palette, colour, 100), 16);
        assert_eq!(state.slot_of(colour), 2);
        assert!(state.used[2]);

        // Hidden: `0x4002`'s bit 1 is set, so the reduced word is 4. The
        // low-byte entry holds slot 2, which is freed.
        assert_eq!(face(&mut state, &mut palette, colour, -100), 4);
        assert_eq!(state.slot_of(colour), 0);
        assert!(!state.used[2]);

        // The freed slot is the next allocation's: the next colour gets
        // slot 2 again, not slot 4.
        assert_eq!(face(&mut state, &mut palette, 0x4004, 100), 16);
        assert_eq!(state.slot_of(0x4004), 2);
        assert!(state.used[2]);
        assert!(!state.used[4]);
    }

    /// Sixteen distinct visible colour words with no hidden face between
    /// them: the 15 even slots 2..30 fill first, then the sixteenth gets slot
    /// 32 — colour 0, and its group is written at DAC entry 0. (The shipped
    /// animation never reaches this: its hidden faces free slots through the
    /// shared low-byte map. The scan's exhausted branch is still pinned.)
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
        assert_eq!(state.slot_of(0x4020), 32);
        assert!(state.used[32]);
        // The group starts at `(32 << 3) as u8 == 0`. Shade 0, bit 1 clear:
        // base [0,0,0], and every entry is that base plus the same backpal
        // row (the base is restored each entry), [1,2,3].
        assert_eq!(palette.entry(0), [1, 2, 3]);
        assert_eq!(palette.entry(15), [1, 2, 3]);
        // Nothing beyond the group was written.
        for index in 16..=255u8 {
            assert_eq!(palette.entry(index), [0, 0, 0], "entry {index}");
        }
    }

    /// The visible group's ramp on a **reused** slot: `lightshift == 9`, a
    /// known shade and backpal, and the group at `slot << 3`. The base is
    /// restored each entry, so the ramp is not cumulative.
    #[test]
    fn glz_writes_the_backpal_ramp_at_slot_shifted_by_3() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[8, 4, 12]; 16];

        // Warm the allocation: the first face takes slot 2 but builds its
        // group with the fresh-slot shade 0. The second face reuses slot 2
        // and its computed shade.
        assert_eq!(
            state.face(0x4002, 256, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        // cross 256 → shade (256 >> 7) = 2 on the reused slot. 0x4002 has bit
        // 1 set, so the base is R=7, G=shade/2=1, B=shade=2; the group is
        // entries 16..31.
        assert_eq!(
            state.face(0x4002, 256, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        // Every entry is the base plus backpal[j] >> 2 = [2,1,3]; the source
        // restores the base after each one.
        for j in 0..16u8 {
            assert_eq!(palette.entry(16 + j), [9, 2, 5], "entry {j}");
        }
        // Nothing before or after the group was touched.
        for index in 0..16u8 {
            assert_eq!(palette.entry(index), [0, 0, 0]);
        }
        for index in 32..=255u8 {
            assert_eq!(palette.entry(index), [0, 0, 0]);
        }
    }

    /// The ramp restores the base every entry and clamps each written value
    /// to 63; the add is the 8-bit accumulator add (with a 6-bit backpal and
    /// a clamped base the sum is at most 126, so the clamp is the visible
    /// guard — a cumulative implementation would wrap at entry 12).
    #[test]
    fn glz_group_ramp_restores_the_base_and_clamps_each_entry() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[60, 0, 0]; 16];

        // Warm the allocation; the second, reused face has the computed shade.
        assert_eq!(
            state.face(0x4004, 10000, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        // cross 10000 → shade 63. 0x4004 has bit 1 clear: R=G=B=63.
        assert_eq!(
            state.face(0x4004, 10000, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        // 63 + (60 >> 2) = 78 clamps to 63 on every entry (a cumulative
        // 8-bit accumulator would reach 258 and wrap at entry 12).
        for j in 0..16u8 {
            assert_eq!(palette.entry(16 + j), [63, 63, 63], "entry {j}");
        }
    }

    /// A freshly allocated slot builds its group from the allocation scan's
    /// leavings: shade 0 (`mov ax,bp` clears AH) and, for a bit-1 word,
    /// R = 16 − slot/2 (`mov cx,15`/`loop` clobbers CL).
    #[test]
    fn glz_fresh_allocation_uses_the_scan_residual_and_zero_shade() {
        let mut state = ColourState::new();
        let mut palette = Palette::new();
        let backpal = [[8, 4, 12]; 16];

        // Slot 2: the scan leaves CL = 16 − 1 = 15 and AH = 0; 0x4002 has bit
        // 1 set, so the base is [15,0,0] and every entry adds
        // backpal[j] >> 2 = [2,1,3].
        assert_eq!(
            state.face(0x4002, 256, DemoMode::Glz, &backpal, &mut palette),
            16
        );
        for j in 0..16u8 {
            assert_eq!(palette.entry(16 + j), [17, 1, 3], "entry {j}");
        }

        // Slot 4: CL = 14, so a bit-1 word's base is [14,0,0].
        assert_eq!(
            state.face(0x4006, 10000, DemoMode::Glz, &backpal, &mut palette),
            32
        );
        for j in 0..16u8 {
            assert_eq!(palette.entry(32 + j), [16, 1, 3], "entry {j}");
        }

        // A bit-1-clear word's fresh base is all zero (`mov cl,ah`): 0x4004
        // takes slot 6 and the group is the backpal row alone.
        assert_eq!(
            state.face(0x4004, 10000, DemoMode::Glz, &backpal, &mut palette),
            48
        );
        for j in 0..16u8 {
            assert_eq!(palette.entry(48 + j), [2, 1, 3], "entry {j}");
        }

        // Fill slots 8..30 with twelve more bit-1 words (their low bytes are
        // distinct from the three above and from each other), then the
        // sixteenth distinct word exhausts the pool: slot 32 leaves
        // CL = 16 − 16 = 0.
        for k in 0..12u8 {
            let colour = 0x500a + 4 * u16::from(k);
            state.face(colour, 0, DemoMode::Glz, &backpal, &mut palette);
        }
        for slot in (8u8..=30).step_by(2) {
            assert!(state.used[slot as usize], "slot {slot}");
        }
        assert_eq!(
            state.face(0x6008, 0, DemoMode::Glz, &backpal, &mut palette),
            0
        );
        for j in 0..16u8 {
            assert_eq!(palette.entry(j), [2, 1, 3], "entry {j}");
        }
    }

    /// `lightshift != 9` uses the source's literal two-shift shade,
    /// `clamp_i16(ax1 + ax2)` with `ax1 = low16(cross >> 8)` and
    /// `ax2 = (ax1 >> 1) | (high16(cross) << 15)`, and R's base becomes
    /// `lightshift`.
    #[test]
    fn glz_lightshift_10_uses_the_two_shift_shade_and_its_r_base() {
        let mut state = ColourState::new();
        state.lightshift = 10;
        let mut palette = Palette::new();
        let face = |state: &mut ColourState, palette: &mut Palette, c| {
            state.face(0x4002, c, DemoMode::Glz, &NO_BACKPAL, palette)
        };
        let expect_group = |palette: &Palette, expected: [u8; 3]| {
            for j in 0..16u8 {
                assert_eq!(palette.entry(16 + j), expected, "entry {j}");
            }
        };

        // Warm the allocation: the first face takes slot 2 with the
        // fresh-slot shade 0; every call below reuses it and uses the
        // computed shade.
        assert_eq!(face(&mut state, &mut palette, 0), 16);

        // 1280 >> 8 = 5, high word 0, so ax2 = 2 and the shade is 7. Bit 1
        // set: R=10, G=3, B=7.
        assert_eq!(face(&mut state, &mut palette, 1280), 16);
        expect_group(&palette, [10, 3, 7]);

        // The sum wraps at 16 bits: ax1 = −1 and the high word 0x7fff is odd,
        // so ax2 = 0x7fff | 0x8000 = −1; the shade is −2 and clamps to 0.
        assert_eq!(face(&mut state, &mut palette, 0x7fff_ffff), 16);
        expect_group(&palette, [10, 0, 0]);

        // 0x5000: ax1 = 80, ax2 = 40, sum 120 clamps to 63; G = 31.
        assert_eq!(face(&mut state, &mut palette, 0x5000), 16);
        expect_group(&palette, [10, 31, 63]);

        // The literal second shift, not `low16(cross >> 9)`: with cross
        // 0x0001_0100, ax1 = 0x0101 (257) and the high word 1 is odd, so
        // ax2 = 0x0080 | 0x8000 = −32640 and the shade clamps to 0. The
        // `low16(cross>>8) + low16(cross>>9)` form would give 257 + 128 =
        // 385 → 63.
        assert_eq!(face(&mut state, &mut palette, 0x0001_0100), 16);
        expect_group(&palette, [10, 0, 0]);
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
        // The first winding is negative (hidden), so its word is kept; the
        // second is visible, so it is reduced to `(word >> 1) & 1` = 0.
        assert_eq!(polys[0].colour, 0x4002);
        assert_eq!(polys[1].colour, 0);
    }
}
