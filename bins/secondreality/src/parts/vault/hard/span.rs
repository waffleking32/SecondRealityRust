//! HARD's span compositor: `VNEW.ASM`'s `__newgroup(0/1/2)`, the c-prefixed
//! fork of GLENZ's transparent copper.
//!
//! The differences from `glenz::span::Compositor` are the whole behaviour:
//!
//! - **128 rows, not 200.** `NROWS equ 128` (`VNEW.ASM:4`); `ng_pass2` runs
//!   `mov cx,NROWS` rows (`:162`) and the edge builder drops an edge whose
//!   upper y is at or below 128 (`cmp bx,NROWS; jge @@n1`, `:341`).
//! - **Opaque, into the A2D0 band.** The fill is a plain `rep stosb` of the
//!   colour byte (`:86-135`); there is no `bgpic` and no OR (GLENZ's
//!   `NEW.ASM:54-64`). The destination is the part's own 64,000-byte
//!   linear 320-pixel-row buffer (`gs:vram = 0A2D0h`, `VDATA.ASM:23`), not
//!   the visible screen.
//! - **No clear.** `ng_init` (`:27-56`) resets the span lists and toggles
//!   the double buffer; it never touches a pixel. `ng_pass3` (`:58-157`)
//!   walks the previous frame's boundary list and the new one together and
//!   repaints only the spans whose accumulated colour changed — opaquely, so
//!   a span that leaves a polygon is overwritten with 0 while a span that
//!   was 0 in both frames keeps whatever earlier frames left there.
//!
//! The boundary walk is the source's: for each row each live edge contributes
//! `row*320 + clamp(x >> 16)` at the polygon colour's low byte, with the
//! 16.16 x advanced by the edge's slope and the word clamped to **1..319**
//! (`:233-238`); the list ends with `(63999, 0)` and `(-1, 0)` (`:265-270`),
//! and the walk stops at the `-1`, so pixel 63999 is never painted. Boundary
//! colours XOR as the walk crosses them.
//!
//! The source also merges a coincident edge into an earlier one by XORing
//! its low colour byte (`@@h1-@@h3`, `:376-403`) and tries to merge a
//! boundary into the previous one when a register that holds the previous
//! **colour word** equals the clamped column (`cmp cx,ax`, `:255-260`). The
//! first is equivalent to keeping both edges because the walk XORs every
//! boundary at a position; the second can never fire in HARD, whose colour
//! words are `0x40xx` and whose columns are 1..319, so the port does not
//! model it (as GLENZ's port does not; D36).

use super::poly::Poly;

/// The visible width in pixels (`yrowad` advances by 320, `VNEW.ASM:267`).
const WIDTH: i32 = 320;
/// `NROWS equ 128` (`VNEW.ASM:4`): the rows `ng_pass2` walks.
const ROWS: i16 = 128;
/// The sentinel the walk fills up to but never paints (`VNEW.ASM:266`).
const LAST_PIXEL: u16 = 63_999;
/// The second sentinel's position word, `dw -1` (`VNEW.ASM:269`); the walk
/// ends when the new list reaches it (`cmp cx,-1`, `:291`).
const END_OF_LIST: u16 = u16::MAX;

/// One edge of the source's `ne` arena (`VNEW.ASM:16-22`): `NE_X`, `NE_Y1`,
/// `NE_Y2`, `NE_COLOR` and `NE_DX`.
///
/// `x` is the 16.16 position on the edge's current row; [`Compositor::paint`]
/// walks a local copy, so the stored edge never changes.
struct Edge {
    x: i32,
    y1: i16,
    y2: i16,
    dx: i32,
    /// `NE_COLOR`'s low byte: `ng_pass3` XORs `cl` and nothing else of the
    /// colour word reaches a pixel.
    colour: u8,
}

/// HARD's 128-row opaque compositor.
///
/// `pixels` is the A2D0 band (D36's linear 320-pixel-row buffer); it is never
/// cleared, by [`Compositor::new`] or by a paint. A part owns one compositor
/// for the run, as the source's `newdata1` and `vram` persist.
pub struct Compositor {
    /// The 64,000-byte work band the engine draws into.
    pub pixels: [u8; 64_000],
    /// The edges [`add`](Compositor::add) has collected since the last
    /// paint (the source's `ne` arena; `cglenzinit` resets it each frame).
    edges: Vec<Edge>,
    /// The previous paint's boundary list, the double-buffered half
    /// `ng_pass3` compares the new one against.
    previous: Vec<(u16, u16)>,
}

impl Compositor {
    /// The source's first `cglenzinit`: an empty edge arena, zeroed pixels
    /// and a fresh half holding only the `-1` sentinel (`ng_init`,
    /// `VNEW.ASM:27-56`; the band itself is not cleared — the zeroes are the
    /// part's own initial BSS).
    pub fn new() -> Self {
        Compositor {
            pixels: [0; 64_000],
            edges: Vec::new(),
            // `ng_init` writes -1 into each fresh half's first word
            // (`VNEW.ASM:47-53`): the pair (0xFFFF, 0), the walk's end.
            previous: vec![(END_OF_LIST, 0)],
        }
    }

    /// `__newgroup(1)` (`VNEW.ASM:278-405`): collect one polygon list's
    /// edges.
    ///
    /// The source resets the count before each list and appends to the frame
    /// arena; the port accumulates across calls until [`paint`](Self::paint)
    /// (the part calls `cglenzinit`, one `cglenzpolylist` per list and
    /// `cglenzdone` — `BP.C:270-295`).
    pub fn add(&mut self, polys: &[Poly]) {
        for poly in polys {
            let points = &poly.points;
            if points.is_empty() {
                continue;
            }
            for (index, &(x1, y1)) in points.iter().enumerate() {
                let (x2, y2) = points[(index + 1) % points.len()];
                // `cmp bx,cx; jg @@i1` (`VNEW.ASM:311-320`): a signed
                // 16-bit y compare, swapping x with it.
                let (xa, ya, xb, yb) = if y1 > y2 {
                    (x2, y2, x1, y1)
                } else {
                    (x1, y1, x2, y2)
                };
                // `cmp cx,0; je @@n1` (horizontal, `:348-350`),
                // `cmp bx,NROWS; jge @@n1` (`:341-343`) and
                // `cmp dx,0; jle @@n1` (`:355-360`).
                if ya == yb || yb <= 0 || ya >= ROWS {
                    continue;
                }
                // `mov [NE_X],0; mov [NE_X+2],ax`: the upper x in 16.16.
                let mut x = i32::from(xa) << 16;
                // `neg ax; add ax,dx; shl eax,16; cdq; sub cx,bx;
                // movzx ecx,cx; idiv ecx` (`:330-339`): the x difference
                // wraps in the word before the shift, the row delta is
                // zero-extended, and the divide truncates toward zero.
                let x_difference = i32::from(xb.wrapping_sub(xa)) << 16;
                let y_difference = i32::from(yb.wrapping_sub(ya) as u16);
                let slope = x_difference / y_difference;
                // `cmp bx,0; jge @@nc; neg bx; movzx ebx,bx; imul ebx;
                // add [NE_X],eax` (`:355-366`): an edge crossing the top is
                // advanced by the slope, not clamped at its first row.
                let mut y1 = ya;
                if y1 < 0 {
                    x = x.wrapping_add(slope.wrapping_mul(-i32::from(y1)));
                    y1 = 0;
                }
                self.edges.push(Edge {
                    x,
                    y1,
                    y2: yb,
                    dx: slope,
                    colour: poly.colour as u8,
                });
            }
        }
    }

    /// `__newgroup(2)` (`VNEW.ASM:278-405`): `ng_pass2` builds the new
    /// boundary list, then `ng_pass3` repaints the spans whose accumulated
    /// colour differs from the previous paint's.
    ///
    /// The edge arena is consumed, as `cglenzdone` ends the frame's list.
    pub fn paint(&mut self) {
        // `ng_pass2` (`VNEW.ASM:159-276`): one boundary per live edge per
        // row, the 16.16 x advanced by the slope each row.
        let mut boundaries: Vec<(u16, u16)> = Vec::new();
        for edge in &self.edges {
            let mut x = edge.x;
            let last_row = edge.y2.min(ROWS);
            let mut row = edge.y1;
            while row < last_row {
                // `shr eax,16` and the signed word compares at `:233-238`:
                // column 0 clamps to 1 and column 320 to 319.
                let column = ((x >> 16) as i16).clamp(1, 319);
                let position = (i32::from(row) * WIDTH) as u16;
                boundaries.push((position.wrapping_add(column as u16), u16::from(edge.colour)));
                x = x.wrapping_add(edge.dx);
                row += 1;
            }
        }
        boundaries.sort_by_key(|&(position, _)| position);
        // `mov word ptr fs:[bx+0],63999` / `…,-1` (`VNEW.ASM:265-270`).
        boundaries.push((LAST_PIXEL, 0));
        boundaries.push((END_OF_LIST, 0));

        // `ng_pass3` (`VNEW.ASM:58-157`): walk the previous paint's list
        // and the new one together. A span is repainted opaquely, in the new
        // list's accumulated colour, only where the two accumulated colours
        // differ; a span that was colour 0 in both keeps its earlier pixels.
        let previous = &self.previous;
        let mut old_index = 0usize;
        let mut new_index = 0usize;
        let (mut old_position, mut old_colour) = previous[0];
        let (mut new_position, mut new_colour) = boundaries[0];
        let mut new_accumulated = 0u8;
        let mut old_accumulated = 0u8;
        let mut destination = 0usize;
        loop {
            if old_position < new_position {
                // `@@23`: the old list's boundary comes first.
                repaint(
                    &mut self.pixels,
                    destination,
                    usize::from(old_position),
                    new_accumulated,
                    old_accumulated,
                );
                destination = usize::from(old_position);
                old_accumulated ^= old_colour as u8;
                old_index += 1;
                (old_position, old_colour) = previous[old_index];
            } else if new_position < old_position {
                // `@@21`: the new list's boundary comes first.
                repaint(
                    &mut self.pixels,
                    destination,
                    usize::from(new_position),
                    new_accumulated,
                    old_accumulated,
                );
                destination = usize::from(new_position);
                new_accumulated ^= new_colour as u8;
                new_index += 1;
                (new_position, new_colour) = boundaries[new_index];
            } else {
                // `@@22`: equal positions. `cmp cx,-1; je @@20` ends the
                // walk at the second sentinel; otherwise the source falls
                // through to `@@23` and advances the old list.
                if new_position == END_OF_LIST {
                    break;
                }
                repaint(
                    &mut self.pixels,
                    destination,
                    usize::from(old_position),
                    new_accumulated,
                    old_accumulated,
                );
                destination = usize::from(old_position);
                old_accumulated ^= old_colour as u8;
                old_index += 1;
                (old_position, old_colour) = previous[old_index];
            }
        }

        self.previous = boundaries;
        self.edges.clear();
    }
}

/// One `ng_pass3` fill (`VNEW.ASM:86-135`): `rep stosb` of the new
/// accumulated colour, only when it differs from the old accumulated one.
fn repaint(pixels: &mut [u8], start: usize, end: usize, new: u8, old: u8) {
    if new == old {
        return;
    }
    for byte in &mut pixels[start..end] {
        *byte = new;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXELS: usize = 64_000;
    /// The byte the tests pre-fill the band with, so an untouched pixel is
    /// distinguishable from a painted 0.
    const FILL: u8 = 0xEE;

    fn poly(colour: u16, points: &[(i16, i16)]) -> Poly {
        Poly {
            colour,
            points: points.to_vec(),
        }
    }

    fn pixel(pixels: &[u8], x: usize, y: usize) -> u8 {
        pixels[y * 320 + x]
    }

    /// A fresh compositor with the band pre-filled, one paint.
    fn painted(polys: &[Poly]) -> [u8; PIXELS] {
        let mut compositor = Compositor::new();
        compositor.pixels = [FILL; PIXELS];
        compositor.add(polys);
        compositor.paint();
        compositor.pixels
    }

    /// The direct span walk the port is compared against: the same
    /// `__newgroup(1)` edges, then for each of the 128 rows the crossings
    /// sorted and XOR-accumulated, filling only a span whose accumulated
    /// colour is non-zero (the first frame's previous list is all zeroes).
    /// A closed polygon contributes an even number of crossings per row, so
    /// the accumulated colour is 0 at the row's end and the source's
    /// tail-to-the-next-boundary span is a no-op.
    fn span_walk(polys: &[Poly]) -> [u8; PIXELS] {
        struct WalkEdge {
            x: i32,
            y1: i16,
            y2: i16,
            dx: i32,
            colour: u8,
        }
        let mut edges = Vec::new();
        for p in polys {
            if p.points.is_empty() {
                continue;
            }
            for (i, &(x1, y1)) in p.points.iter().enumerate() {
                let (x2, y2) = p.points[(i + 1) % p.points.len()];
                let (xa, ya, xb, yb) = if y1 > y2 {
                    (x2, y2, x1, y1)
                } else {
                    (x1, y1, x2, y2)
                };
                if ya == yb || yb <= 0 || ya >= ROWS {
                    continue;
                }
                let mut x = i32::from(xa) << 16;
                let slope =
                    (i32::from(xb.wrapping_sub(xa)) << 16) / i32::from(yb.wrapping_sub(ya) as u16);
                let mut y1 = ya;
                if y1 < 0 {
                    x = x.wrapping_add(slope.wrapping_mul(-i32::from(y1)));
                    y1 = 0;
                }
                edges.push(WalkEdge {
                    x,
                    y1,
                    y2: yb,
                    dx: slope,
                    colour: p.colour as u8,
                });
            }
        }

        let mut pixels = [FILL; PIXELS];
        for row in 0..ROWS {
            let mut crossings: Vec<(i16, u8)> = Vec::new();
            for edge in &edges {
                if row < edge.y1 || row >= edge.y2 {
                    continue;
                }
                let x = edge
                    .x
                    .wrapping_add(edge.dx.wrapping_mul(i32::from(row - edge.y1)));
                crossings.push((((x >> 16) as i16).clamp(1, 319), edge.colour));
            }
            crossings.sort_by_key(|&(x, _)| x);
            let mut accumulated = 0u8;
            let mut previous = 0usize;
            for (x, colour) in crossings {
                let position = row as usize * 320 + x as usize;
                if accumulated != 0 {
                    for byte in &mut pixels[previous..position] {
                        *byte = accumulated;
                    }
                }
                previous = position;
                accumulated ^= colour;
            }
            assert_eq!(accumulated, 0, "row {row} leaves an unpaired boundary");
        }
        pixels
    }

    /// The 128-row bound, the opaque fill, the XOR of overlapping boundaries,
    /// the x clamp, the sentinels and the direct span walk.
    #[test]
    fn the_opaque_compositor_paints_only_128_rows_and_never_clears() {
        // A polygon spanning rows 120..140 is painted on 120..127 only.
        let pixels = painted(&[poly(
            0x0005,
            &[(10, 120), (200, 120), (200, 140), (10, 140)],
        )]);
        assert_eq!(pixel(&pixels, 10, 119), FILL);
        assert_eq!(pixel(&pixels, 9, 120), FILL);
        assert_eq!(pixel(&pixels, 10, 120), 0x05);
        assert_eq!(pixel(&pixels, 199, 120), 0x05);
        assert_eq!(pixel(&pixels, 200, 120), FILL);
        assert_eq!(pixel(&pixels, 10, 127), 0x05);
        assert_eq!(pixel(&pixels, 199, 127), 0x05);
        for row in 128..200 {
            assert!(
                (0..320).all(|x| pixel(&pixels, x, row) == FILL),
                "row {row}"
            );
        }
        // The (63999,0) sentinel: the last pixel is never painted.
        assert_eq!(pixels[PIXELS - 1], FILL);

        // A second polygon over the first: pixels covered only by the first
        // keep its colour, the overlap XORs, and pixels outside both keep the
        // pre-fill (the band is never cleared).
        let pixels = painted(&[
            poly(0x0003, &[(10, 60), (200, 60), (200, 140), (10, 140)]),
            poly(0x0005, &[(100, 60), (150, 60), (150, 80), (100, 80)]),
        ]);
        assert_eq!(pixel(&pixels, 5, 70), FILL);
        assert_eq!(pixel(&pixels, 50, 70), 0x03);
        assert_eq!(pixel(&pixels, 120, 70), 0x06); // 3 ^ 5
        assert_eq!(pixel(&pixels, 180, 70), 0x03);
        assert_eq!(pixel(&pixels, 120, 80), 0x03); // below the smaller polygon
        assert_eq!(pixel(&pixels, 205, 70), FILL);
        assert_eq!(pixel(&pixels, 120, 59), FILL);

        // The x clamp is 1..319: the off-screen triangle leaves columns 0
        // and 319 as background, and its row-99 edges sit at 158 and 161.
        let pixels = painted(&[poly(0x0007, &[(-10, 0), (330, 0), (160, 100)])]);
        for row in [0usize, 50, 99] {
            assert_eq!(pixel(&pixels, 0, row), FILL, "row {row}");
            assert_eq!(pixel(&pixels, 319, row), FILL, "row {row}");
        }
        assert_eq!(pixel(&pixels, 1, 0), 0x07);
        assert_eq!(pixel(&pixels, 318, 0), 0x07);
        assert_eq!(pixel(&pixels, 157, 99), FILL);
        assert_eq!(pixel(&pixels, 158, 99), 0x07);
        assert_eq!(pixel(&pixels, 160, 99), 0x07);
        assert_eq!(pixel(&pixels, 161, 99), FILL);

        // Equal colours cancel: the two coincident boundaries XOR to 0, so
        // the span is not painted and the pre-fill shows through.
        let pixels = painted(&[
            poly(0x0003, &[(10, 10), (30, 10), (30, 30), (10, 30)]),
            poly(0x0003, &[(10, 10), (30, 10), (30, 30), (10, 30)]),
        ]);
        assert_eq!(pixel(&pixels, 20, 20), FILL);

        // The direct span walk over the same edges is byte-identical.
        let scene = [
            poly(0x0003, &[(10, 10), (120, 30), (60, 90)]),
            poly(0x0005, &[(40, 40), (150, 40), (150, 110), (40, 110)]),
            poly(0x0007, &[(-20, 60), (300, 50), (160, 126)]),
        ];
        assert_eq!(painted(&scene), span_walk(&scene));
    }

    /// The dirty-span walk across two paints: the previous frame's boundary
    /// list is the comparison, so a span that leaves a polygon is repainted
    /// opaquely with 0, the overlap takes the new list's colour (not the
    /// two frames' XOR), and spans outside both lists are untouched.
    #[test]
    fn the_second_frame_paints_only_the_spans_whose_colour_changed() {
        let mut compositor = Compositor::new();
        compositor.pixels = [FILL; PIXELS];
        compositor.add(&[poly(0x0003, &[(10, 10), (30, 10), (30, 30), (10, 30)])]);
        compositor.paint();
        assert_eq!(pixel(&compositor.pixels, 20, 20), 0x03);
        assert_eq!(pixel(&compositor.pixels, 5, 5), FILL);

        compositor.add(&[poly(0x0005, &[(20, 20), (40, 20), (40, 40), (20, 40)])]);
        compositor.paint();
        // The first polygon's uncovered part becomes 0 — the source writes
        // the new list's colour 0 over the old accumulated 3.
        assert_eq!(pixel(&compositor.pixels, 15, 15), 0x00);
        // The overlap and the new polygon's own part take colour 5.
        assert_eq!(pixel(&compositor.pixels, 25, 25), 0x05);
        assert_eq!(pixel(&compositor.pixels, 35, 35), 0x05);
        // Spans that were 0 in both frames keep the pre-fill.
        assert_eq!(pixel(&compositor.pixels, 5, 5), FILL);
        assert_eq!(pixel(&compositor.pixels, 45, 45), FILL);
        assert_eq!(compositor.pixels[PIXELS - 1], FILL);
    }
}
