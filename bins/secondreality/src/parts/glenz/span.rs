//! The transparent span compositor: `__newgroup`'s edge builder and
//! `ng_pass2`/`ng_pass3` (`GLENZ/NEW.ASM:26-418`).
//!
//! The source splits one frame of drawing into three calls:
//! `cglenzinit` → `__newgroup(0)` (`ng_init`), one `cglenzpolylist` →
//! `__newgroup(1)` per polygon list, and `cglenzdone` → `__newgroup(2)`
//! (`ng_pass2` then `ng_pass3`). The port keeps the same three phases:
//! [`Compositor::clear`], [`Compositor::add`] (called once per [`Poly`]; the
//! source's list loop is the caller's) and [`Compositor::paint`].
//!
//! # The edge rule (`NEW.ASM:311-374`)
//!
//! Each consecutive pair of stored screen points, the closing pair included,
//! becomes one edge. The endpoints are ordered by y with a signed 16-bit
//! compare, swapping x and y together; a horizontal edge is skipped, and an
//! edge with `y2 <= 0` is dropped. `x` is the upper endpoint's x in 16.16,
//! and the slope is `((x2−x1) << 16) / (y2−y1)` with the x difference taken
//! in the word **before** the shift (`neg ax; add ax,dx; shl eax,16`,
//! `:344-346`) and the division truncating toward zero. An edge crossing the
//! top is advanced by `slope · (−y1)` — the asm's 32-bit `imul`, whose high
//! half is discarded — and its `y1` becomes 0.
//!
//! The source merges an edge that duplicates an earlier one by XORing its
//! colour's low byte into the earlier edge (`@@h1-@@h3`, `:376-403`). The port
//! keeps coincident edges separate: the boundary walk XORs every boundary at
//! a position, so two coincident edges and one edge carrying their XOR paint
//! the same pixels.
//!
//! # The boundary rule (`ng_pass2`, `NEW.ASM:174-288`)
//!
//! For each of the source's 200 rows (`mov cx,200`, `:177`) each live edge
//! contributes the boundary `y*320 + clamp(x>>16)` at the edge's colour, then
//! `x += dx`. The low word of `x>>16` is read signed and clamped by signed
//! compares to **1..319** (`:246-253`) — x = 0 clamps to 1, x = 320 to 319.
//! The position is global, not row-local: a boundary past 319 lands at
//! `y*320+319` and the next row continues at `(y+1)*320 + x`, so a fill can
//! cross from the end of one row into the start of the next.
//!
//! `ng_pass2` emits the boundaries row by row into the double-buffered
//! `newdata1` list, insertion-sorting each row by the full 16.16 x. It also
//! tries to XOR a boundary whose clamped x equals the previous one's into
//! the previous entry (`:255-260`), but the register it compares is `cx`,
//! which the preceding `mov cx,ax` (`:267`) has just loaded with the stored
//! **colour word**, not the clamped x — the branch only fires when a colour
//! word equals a column 1..319. (The linked `V.EXE` confirms it: `cmp
//! %ax,%cx` at load offset 0x3daf follows `mov 0x8(%bx),%ax; mov %ax,%cx`
//! at 0x3dc6-0x3dd1.) GLENZ's colour words are always at least 0x4000
//! (`MAIN.C:176-240`'s tables; `demo_glz`/`demo_glz2` write only the low
//! byte over the 0x40 high byte), so in the shipped path the branch is
//! inert. The port emits every boundary and sorts by position; XOR is
//! commutative, so boundaries at one position are equivalent whether the
//! source merges them or not.
//!
//! # The paint rule and the dirty-span substitution (`ng_pass3`, `NEW.ASM:66-172`)
//!
//! The source's screen is not cleared between frames. `ng_pass3` walks the
//! previous frame's boundary list and the new one together, keeping the new
//! and old accumulated colours, and repaints `[prev, position)` only where
//! the two colours differ. Every span's pixels are `colour | bgpic[pixel]`,
//! so a span whose colour did not change already holds exactly the bytes a
//! repaint would write, and the walk is an optimisation with identical
//! pixels. The port drops it: [`Compositor::paint`] repaints every span from
//! `bgpic` and never clears the buffer. The equivalence is not assumed — the
//! test module keeps the source's two-list walk beside the port and compares
//! whole frames below.
//!
//! The list ends with the sentinels `(63999, 0)` and `(-1, 0)` (`:279-285`);
//! the fill is to the boundary exclusive and `-1` stops the walk, so **pixel
//! 63999 is never painted** and `paint` must leave it alone.
//!
//! `bgpic` and `pixels` are the part's 64,000-byte linear 320-pixel-row
//! buffers (D36); the source's `MAXLINES` 512 cap on the edge arena is
//! dropped (the shipped scenes stay under 96 edges).

// The part driver lands in Task 5; until it consumes this layer every item
// here is dead code to the binary target (the tests below use it).
#![allow(dead_code)]

use super::poly::Poly;

/// The visible width in pixels; `yrowad` advances by this much per row.
const WIDTH: i32 = 320;
/// `ng_pass2`'s row count (`mov cx,200`, `NEW.ASM:177`).
const ROWS: i16 = 200;
/// The sentinel the walk fills up to but never paints (`NEW.ASM:280`).
const LAST_PIXEL: i32 = 63_999;

/// One edge of the source's `ne` arena (`NEW.ASM:16-22`): `NE_X`, `NE_Y1`,
/// `NE_Y2`, `NE_COLOR` and `NE_DX`.
///
/// `x` is the 16.16 position on the edge's current row; [`Compositor::paint`]
/// walks a local copy, so the stored edge never changes.
struct Edge {
    x: i32,
    y1: i16,
    y2: i16,
    dx: i32,
    /// `NE_COLOR`'s low byte: `ng_pass3` XORs `cl` from the colour word and
    /// nothing else of it reaches a pixel.
    colour: u8,
}

/// The transparent-copper compositor.
#[derive(Default)]
pub struct Compositor {
    edges: Vec<Edge>,
}

impl Compositor {
    /// An empty compositor (`cglenzinit` blanking `nep` and the edge arena).
    pub fn new() -> Self {
        Self::default()
    }

    /// `ng_init` (`NEW.ASM:26-47`): drop the previous frame's edges.
    pub fn clear(&mut self) {
        self.edges.clear();
    }

    /// `__newgroup(1)` (`NEW.ASM:308-418`): add one polygon's edges.
    ///
    /// `ceasypolylist` stores a vertex pair per point and drops a closing
    /// duplicate; `add` supplies the closing pair itself with
    /// `(index + 1) % len`.
    pub fn add(&mut self, poly: &Poly) {
        let points = &poly.points;
        if points.is_empty() {
            return;
        }
        for (index, &(x1, y1)) in points.iter().enumerate() {
            let (x2, y2) = points[(index + 1) % points.len()];
            // `cmp bx,cx; jg @@i1` (`NEW.ASM:331-338`): signed 16-bit y,
            // swapping each endpoint's x with its y.
            let (xa, ya, xb, yb) = if y1 > y2 {
                (x2, y2, x1, y1)
            } else {
                (x1, y1, x2, y2)
            };
            // `cmp cx,0; je @@n1` (horizontal) and `cmp dx,0; jle @@n1`
            // (wholly above the screen), `NEW.ASM:350-360`.
            if ya == yb || yb <= 0 {
                continue;
            }
            // `mov [NE_X],0; mov [NE_X+2],ax`: the upper x in 16.16.
            let mut x = (xa as i32) << 16;
            // `neg ax; add ax,dx; shl eax,16; cdq; idiv ecx` (`:344-353`):
            // the difference wraps in the word and is sign-extended by the
            // shift; the divisor is the difference zero-extended by `movzx`.
            let x_difference = (xb.wrapping_sub(xa) as i32) << 16;
            let y_difference = (yb.wrapping_sub(ya) as u16) as i32;
            let slope = x_difference / y_difference;
            // `cmp bx,0; jge @@nc` / `neg bx; movzx ebx,bx; imul ebx`
            // (`:355-366`): clip the part above the screen.
            let mut y1 = ya;
            if y1 < 0 {
                x = x.wrapping_add(slope.wrapping_mul(-(y1 as i32)));
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

    /// `ng_pass2` + `ng_pass3` (`NEW.ASM:174-288`, `:66-172`): repaint every
    /// span of the collected polygons over `bgpic`.
    ///
    /// Both buffers are the part's 64,000-byte linear 320-pixel-row stores.
    /// `pixels` is not cleared and pixel 63999 is left untouched.
    pub fn paint(&self, bgpic: &[u8], pixels: &mut [u8]) {
        // A row's boundary is emitted from the same edge state whether it is
        // merged into a row list or not; collect them all and sort by the
        // source's global position.
        let mut boundaries: Vec<(i32, u8)> = Vec::with_capacity(self.edges.len() * ROWS as usize);
        for edge in &self.edges {
            let mut x = edge.x;
            let last_row = edge.y2.min(ROWS);
            for row in edge.y1..last_row {
                // `shr eax,16` and the 16-bit signed compares at `:246-253`.
                let xi = ((x >> 16) as i16) as i32;
                let xi = xi.clamp(1, 319);
                boundaries.push((row as i32 * WIDTH + xi, edge.colour));
                x = x.wrapping_add(edge.dx);
            }
        }
        boundaries.sort_by_key(|&(position, _)| position);
        // The source appends `(63999,0)` then `(-1,0)` (`:279-285`); the -1
        // ends the walk, so the buffer's last pixel is never painted.
        boundaries.push((LAST_PIXEL, 0));
        boundaries.push((-1, 0));

        let mut colour = 0u8;
        let mut previous = 0usize;
        for (position, boundary_colour) in boundaries {
            if position < 0 {
                break;
            }
            let position = position as usize;
            // fillmacro (`NEW.ASM:54-64`): `colour | bgpic[i]`.
            for (pixel, background) in pixels[previous..position]
                .iter_mut()
                .zip(&bgpic[previous..position])
            {
                *pixel = colour | *background;
            }
            colour ^= boundary_colour;
            previous = position;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PIXELS: usize = 320 * 200;

    fn make_poly(colour: u16, points: &[(i16, i16)]) -> Poly {
        Poly {
            colour,
            points: points.to_vec(),
        }
    }

    fn flat(value: u8) -> Vec<u8> {
        vec![value; PIXELS]
    }

    /// The port's frame: clear, add every polygon, paint.
    fn paint_frame(polys: &[Poly], bgpic: &[u8], pixels: &mut [u8]) {
        let mut compositor = Compositor::new();
        compositor.clear();
        for poly in polys {
            compositor.add(poly);
        }
        compositor.paint(bgpic, pixels);
    }

    fn pixel(pixels: &[u8], x: usize, y: usize) -> u8 {
        pixels[y * 320 + x]
    }

    /// The triangle (10,0)-(30,10)-(10,20) has a vertical left edge at
    /// x = 10 and a chevron whose right edge runs 10 → 30 and back to 12.
    /// With a uniform 0x10 background the span bytes are hand-checkable.
    #[test]
    fn a_triangle_paints_its_hand_computed_spans() {
        let bgpic = flat(0x10);
        let mut pixels = bgpic.clone();
        paint_frame(
            &[make_poly(0x0003, &[(10, 0), (30, 10), (10, 20)])],
            &bgpic,
            &mut pixels,
        );

        // Row 0: the chevron's first row and the vertical edge both sit on
        // x = 10, so the row has no coloured pixel.
        assert_eq!(pixel(&pixels, 9, 0), 0x10);
        assert_eq!(pixel(&pixels, 10, 0), 0x10);
        assert_eq!(pixel(&pixels, 11, 0), 0x10);

        // Rows 1..9: [10, 10 + 2y).
        assert_eq!(pixel(&pixels, 9, 1), 0x10);
        assert_eq!(pixel(&pixels, 10, 1), 0x13);
        assert_eq!(pixel(&pixels, 11, 2), 0x13);
        assert_eq!(pixel(&pixels, 13, 2), 0x13);
        assert_eq!(pixel(&pixels, 14, 2), 0x10);
        assert_eq!(pixel(&pixels, 19, 5), 0x13);
        assert_eq!(pixel(&pixels, 20, 5), 0x10);

        // Row 10: the chevron reaches its right vertex, [10, 30).
        assert_eq!(pixel(&pixels, 9, 10), 0x10);
        for x in 10..30 {
            assert_eq!(pixel(&pixels, x, 10), 0x13, "x={x}");
        }
        assert_eq!(pixel(&pixels, 30, 10), 0x10);

        // Row 19: [10, 12); row 20 is past the last edge.
        assert_eq!(pixel(&pixels, 10, 19), 0x13);
        assert_eq!(pixel(&pixels, 11, 19), 0x13);
        assert_eq!(pixel(&pixels, 12, 19), 0x10);
        assert!((0..320).all(|x| pixel(&pixels, x, 20) == 0x10));

        // 90 pixels in rows 1..9, 110 in rows 10..19.
        assert_eq!(pixels.iter().filter(|&&p| p == 0x13).count(), 200);
    }

    /// Two squares with colour words 0x0002 and 0x0004 over a 0x01
    /// background: the union of the colours is the XOR of the active
    /// boundaries, OR-ed with the background.
    #[test]
    fn overlapping_polygons_xor_their_colours_and_or_the_background() {
        let bgpic = flat(0x01);
        let mut pixels = bgpic.clone();
        paint_frame(
            &[
                make_poly(0x0002, &[(10, 10), (30, 10), (30, 30), (10, 30)]),
                make_poly(0x0004, &[(20, 20), (40, 20), (40, 40), (20, 40)]),
            ],
            &bgpic,
            &mut pixels,
        );

        assert_eq!(pixel(&pixels, 5, 15), 0x01);
        assert_eq!(pixel(&pixels, 15, 15), 0x03);
        assert_eq!(pixel(&pixels, 35, 25), 0x05);
        assert_eq!(pixel(&pixels, 25, 25), 0x07);
        assert_eq!(pixel(&pixels, 15, 25), 0x03);
        assert_eq!(pixel(&pixels, 45, 45), 0x01);
        assert_eq!(pixel(&pixels, 5, 45), 0x01);
    }

    /// The boundary is clamped to 1..319, not 0..320: a triangle reaching
    /// x = −10 and x = 330 leaves column 0 and column 319 as background.
    #[test]
    fn boundaries_clamp_to_columns_one_and_319() {
        let bgpic = flat(0x20);
        let mut pixels = bgpic.clone();
        paint_frame(
            &[make_poly(0x0005, &[(-10, 0), (330, 0), (160, 100)])],
            &bgpic,
            &mut pixels,
        );

        // Row 0: boundaries at 1 and 319.
        assert_eq!(pixel(&pixels, 0, 0), 0x20);
        assert_eq!(pixel(&pixels, 1, 0), 0x25);
        assert_eq!(pixel(&pixels, 318, 0), 0x25);
        assert_eq!(pixel(&pixels, 319, 0), 0x20);

        // Row 7: the right edge is still clamped, the left one no longer
        // needs to be: [1, 318).
        assert_eq!(pixel(&pixels, 0, 7), 0x20);
        assert_eq!(pixel(&pixels, 1, 7), 0x25);
        assert_eq!(pixel(&pixels, 317, 7), 0x25);
        assert_eq!(pixel(&pixels, 318, 7), 0x20);
        assert_eq!(pixel(&pixels, 319, 7), 0x20);

        // Row 99: both edges are inside the screen, [158, 161).
        assert_eq!(pixel(&pixels, 157, 99), 0x20);
        assert_eq!(pixel(&pixels, 158, 99), 0x25);
        assert_eq!(pixel(&pixels, 160, 99), 0x25);
        assert_eq!(pixel(&pixels, 161, 99), 0x20);
    }

    /// A horizontal edge is skipped before the division, and its row is
    /// past the last span of the triangle that owns it.
    #[test]
    fn a_horizontal_edge_adds_no_boundaries() {
        let bgpic = flat(0x40);
        let mut pixels = bgpic.clone();
        // A triangle with a horizontal base at y = 20.
        paint_frame(
            &[make_poly(0x0006, &[(20, 0), (10, 20), (30, 20)])],
            &bgpic,
            &mut pixels,
        );
        assert!((0..320).all(|x| pixel(&pixels, x, 20) == 0x40));
        // Row 19 still has the two slanted edges: [10, 29).
        assert_eq!(pixel(&pixels, 9, 19), 0x40);
        assert_eq!(pixel(&pixels, 10, 19), 0x46);
        assert_eq!(pixel(&pixels, 28, 19), 0x46);
        assert_eq!(pixel(&pixels, 29, 19), 0x40);

        // Two horizontal edges alone produce nothing at all.
        let mut pixels = bgpic.clone();
        paint_frame(&[make_poly(0x0006, &[(10, 10), (30, 10)])], &bgpic, &mut pixels);
        assert!(pixels[..PIXELS - 1].iter().all(|&p| p == 0x40));
    }

    /// Edges wholly above or below the screen are dropped; an edge crossing
    /// the top is clipped by advancing x, not by clamping its first row.
    #[test]
    fn edges_off_the_screen_are_dropped_or_clipped() {
        let bgpic = flat(0x20);
        let mut pixels = flat(0x99);
        paint_frame(
            &[
                // Wholly above: every edge ends at or above y = 0.
                make_poly(0x0003, &[(10, -20), (30, -10), (10, -5)]),
                // Wholly below: y1 >= 200, so no row is emitted.
                make_poly(0x0003, &[(10, 250), (30, 250), (30, 300), (10, 300)]),
            ],
            &bgpic,
            &mut pixels,
        );
        assert!(pixels[..PIXELS - 1].iter().all(|&p| p == 0x20));
        // Paint never touches the last pixel.
        assert_eq!(pixels[PIXELS - 1], 0x99);

        // The edge (0,-10)-(20,10) has slope 1.0; clipping at y = 0 starts
        // it at x = 10 instead of x = 0 (which would clamp to 1 and give a
        // zero-width span against the vertical edge at x = 0).
        let mut pixels = bgpic.clone();
        paint_frame(
            &[make_poly(0x0005, &[(0, -10), (20, 10), (0, 10)])],
            &bgpic,
            &mut pixels,
        );
        assert_eq!(pixel(&pixels, 0, 0), 0x20);
        for x in 1..10 {
            assert_eq!(pixel(&pixels, x, 0), 0x25, "x={x}");
        }
        assert_eq!(pixel(&pixels, 10, 0), 0x20);
        // Row 9: [1, 19).
        assert_eq!(pixel(&pixels, 1, 9), 0x25);
        assert_eq!(pixel(&pixels, 18, 9), 0x25);
        assert_eq!(pixel(&pixels, 19, 9), 0x20);
    }

    /// The slope's numerator is the **16-bit wrapped** x difference shifted
    /// left 16 — 32767 → −32768 is +1, not −65535 — and `idiv` truncates a
    /// negative non-exact quotient toward zero.
    #[test]
    fn the_slope_wraps_the_x_difference_and_truncates_toward_zero() {
        // The upper x is 32767, the lower −32768: the word difference wraps
        // to +1, so dx = (1 << 16) / 3 = 21845 (truncated).
        let mut compositor = Compositor::new();
        compositor.add(&make_poly(
            0x4003,
            &[(32767, 0), (-32768, 3), (-32768, 0)],
        ));
        assert_eq!(compositor.edges.len(), 2);
        assert_eq!(compositor.edges[0].x, 32767 << 16);
        assert_eq!(compositor.edges[0].y1, 0);
        assert_eq!(compositor.edges[0].y2, 3);
        assert_eq!(compositor.edges[0].dx, 65_536 / 3);
        // The other edge is the vertical x = −32768, which clamps to 1.
        assert_eq!(compositor.edges[1].dx, 0);

        let bgpic = flat(0x10);
        let mut pixels = bgpic.clone();
        compositor.paint(&bgpic, &mut pixels);
        for row in 0..3 {
            assert_eq!(pixel(&pixels, 0, row), 0x10, "row {row}");
            assert_eq!(pixel(&pixels, 1, row), 0x13, "row {row}");
            assert_eq!(pixel(&pixels, 318, row), 0x13, "row {row}");
            assert_eq!(pixel(&pixels, 319, row), 0x10, "row {row}");
        }

        // A negative, non-exact slope: (10 − 331) << 16 / 7 =
        // −21_037_056 / 7 = −3_005_293 (truncated), not −3_005_294
        // (floored).
        let mut compositor = Compositor::new();
        compositor.add(&make_poly(0x4003, &[(331, 0), (10, 7), (10, 0)]));
        assert_eq!(compositor.edges.len(), 2);
        assert_eq!(compositor.edges[0].x, 331 << 16);
        assert_eq!(compositor.edges[0].dx, -3_005_293);
        assert_eq!(compositor.edges[1].dx, 0);

        let mut pixels = bgpic.clone();
        compositor.paint(&bgpic, &mut pixels);
        // The row positions from the truncated 16.16 step: 331 (clamped to
        // 319), then 285, 239, 193, 147, 101, 55.
        let right_edge = [319, 285, 239, 193, 147, 101, 55];
        for (row, &right) in right_edge.iter().enumerate() {
            assert_eq!(pixel(&pixels, 9, row), 0x10, "row {row}");
            assert_eq!(pixel(&pixels, 10, row), 0x13, "row {row}");
            assert_eq!(pixel(&pixels, right - 1, row), 0x13, "row {row}");
            assert_eq!(pixel(&pixels, right, row), 0x10, "row {row}");
        }
        // y2 = 7 ends the edge at row 6.
        assert!((0..320).all(|x| pixel(&pixels, x, 7) == 0x10));
    }

    /// Positions are global, not row-local: a boundary clamped to 319 at the
    /// end of one row and the next row's boundary at x = 282 are 283 pixels
    /// apart in the linear buffer, and the fill between them is background.
    #[test]
    fn a_row_crossing_span_uses_the_global_position() {
        let bgpic = flat(0x08);
        let mut pixels = bgpic.clone();
        paint_frame(
            &[make_poly(0x0007, &[(350, 0), (350, 10), (10, 10)])],
            &bgpic,
            &mut pixels,
        );

        // Row 0: both boundaries clamp to 319.
        assert!((0..320).all(|x| pixel(&pixels, x, 0) == 0x08));

        // Row 1: the slant is at 316, [316, 319).
        assert_eq!(pixel(&pixels, 315, 1), 0x08);
        assert_eq!(pixel(&pixels, 316, 1), 0x0F);
        assert_eq!(pixel(&pixels, 318, 1), 0x0F);
        assert_eq!(pixel(&pixels, 319, 1), 0x08);

        // Row 2: 282 and 319; the fill from row 1's 319 to row 2's 282 is
        // background across the row boundary.
        assert_eq!(pixel(&pixels, 319, 1), 0x08);
        assert_eq!(pixel(&pixels, 0, 2), 0x08);
        assert_eq!(pixel(&pixels, 281, 2), 0x08);
        assert_eq!(pixel(&pixels, 282, 2), 0x0F);
        assert_eq!(pixel(&pixels, 318, 2), 0x0F);
        assert_eq!(pixel(&pixels, 319, 2), 0x08);

        // Row 9: 44..318; row 10 has no boundary at all.
        assert_eq!(pixel(&pixels, 43, 9), 0x08);
        assert_eq!(pixel(&pixels, 44, 9), 0x0F);
        assert_eq!(pixel(&pixels, 318, 9), 0x0F);
        assert_eq!(pixel(&pixels, 319, 9), 0x08);
        assert!((0..320).all(|x| pixel(&pixels, x, 10) == 0x08));
    }

    /// The source XORs a duplicate edge's low byte into the earlier edge;
    /// keeping both edges is equivalent because the walk XORs every boundary
    /// at a position.
    #[test]
    fn coincident_edges_xor_like_the_merged_edge() {
        let square = [(10, 10), (30, 10), (30, 30), (10, 30)];

        let bgpic = flat(0x08);
        let mut pixels = bgpic.clone();
        paint_frame(
            &[
                make_poly(0x0003, &square),
                make_poly(0x0005, &square),
            ],
            &bgpic,
            &mut pixels,
        );
        // 3 ^ 5 = 6, OR 8.
        assert_eq!(pixel(&pixels, 20, 20), 0x0E);
        assert_eq!(pixel(&pixels, 10, 10), 0x0E);
        assert_eq!(pixel(&pixels, 29, 29), 0x0E);
        assert_eq!(pixel(&pixels, 5, 5), 0x08);
        assert_eq!(pixel(&pixels, 35, 35), 0x08);

        // The same polygon twice in the same colour cancels out.
        let mut pixels = bgpic.clone();
        paint_frame(
            &[
                make_poly(0x0003, &square),
                make_poly(0x0003, &square),
            ],
            &bgpic,
            &mut pixels,
        );
        assert_eq!(pixel(&pixels, 20, 20), 0x08);
    }

    /// The walk ends at the `-1` sentinel after filling to the `(63999, 0)`
    /// sentinel, so the last pixel is never written, and `paint` never
    /// clears the buffer first.
    #[test]
    fn paint_leaves_pixel_63999_untouched() {
        let bgpic = flat(0x11);
        let mut pixels = bgpic.clone();
        pixels[PIXELS - 1] = 0xEE;
        paint_frame(
            &[make_poly(0x0007, &[(0, 0), (320, 0), (320, 200), (0, 200)])],
            &bgpic,
            &mut pixels,
        );
        // The full-screen quad's boundary is column 319 of row 199.
        assert_eq!(pixels[PIXELS - 2], 0x17);
        assert_eq!(pixels[PIXELS - 1], 0xEE);
    }

    // ---- The source's two-list walk, kept beside the port (D36's test) ----

    /// The source's `ne` edge (`NEW.ASM:16-22`) with the full colour word.
    #[derive(Clone, Copy)]
    struct SourceEdge {
        x: i32,
        y1: i16,
        y2: i16,
        dx: i32,
        colour: u16,
    }

    /// `__newgroup(1)` (`NEW.ASM:311-403`), including the scan that XORs a
    /// duplicate edge into the existing one.
    fn source_edges(polys: &[Poly]) -> Vec<SourceEdge> {
        let mut edges: Vec<SourceEdge> = Vec::new();
        for poly in polys {
            let points = &poly.points;
            if points.is_empty() {
                continue;
            }
            for (index, &(x1, y1)) in points.iter().enumerate() {
                let (x2, y2) = points[(index + 1) % points.len()];
                let (xa, ya, xb, yb) = if y1 > y2 {
                    (x2, y2, x1, y1)
                } else {
                    (x1, y1, x2, y2)
                };
                if ya == yb || yb <= 0 {
                    continue;
                }
                let mut x = (xa as i32) << 16;
                let slope = (((xb.wrapping_sub(xa)) as i32) << 16)
                    / ((yb.wrapping_sub(ya) as u16) as i32);
                let mut y1 = ya;
                if y1 < 0 {
                    x = x.wrapping_add(slope.wrapping_mul(-(y1 as i32)));
                    y1 = 0;
                }
                // `@@h1-@@h3`: same clipped y1, y2, 16.16 x and dx.
                if let Some(existing) = edges
                    .iter_mut()
                    .find(|e| e.y1 == y1 && e.y2 == yb && e.x == x && e.dx == slope)
                {
                    existing.colour ^= poly.colour & 0xFF;
                } else {
                    edges.push(SourceEdge {
                        x,
                        y1,
                        y2: yb,
                        dx: slope,
                        colour: poly.colour,
                    });
                }
            }
        }
        edges
    }

    /// `ng_pass2` (`NEW.ASM:174-288`): the per-row insertion-sorted boundary
    /// list, with the "same x" XOR quirk, the global `row*320 + x` positions
    /// and the `(63999,0)` / `(-1,0)` sentinels. Returns the new list and the
    /// number of times the literal "same x" compare fired.
    fn source_pass2(edges: &mut [SourceEdge]) -> (Vec<(u16, u16)>, usize) {
        let mut list: Vec<(u16, u16)> = Vec::new();
        let mut same_x_hits = 0usize;
        for row in 0..ROWS as i32 {
            // Edges added this row (y1 <= row) that have not reached y2.
            let mut active: Vec<usize> = (0..edges.len())
                .filter(|&i| edges[i].y1 as i32 <= row && row < edges[i].y2 as i32)
                .collect();
            // The source sorts the row by `NE_X`, signed 32-bit; ties keep
            // their carried order.
            active.sort_by_key(|&i| edges[i].x);
            // `mov cx,8000h` (`NEW.ASM:231`), then `mov cx,ax` at `@@14`
            // (`:267`) after AX has been loaded with `NE_COLOR`: the
            // register the "same x pos" compare reads is the previous
            // boundary's **colour word**, not its clamped x. Kept literal;
            // with GLENZ's 0x40xx colour words it never equals a column.
            let mut previous_colour: u16 = 0x8000;
            for i in active {
                let edge = &mut edges[i];
                let ax = ((edge.x >> 16) as i16) as i32;
                let ax = ax.clamp(1, 319);
                if previous_colour == ax as u16 {
                    // `@@16`/`@@14`'s "same x pos": XOR the colour word
                    // into the previous boundary instead of storing a new
                    // one. GLENZ's colour words never equal a column, so
                    // this must stay at zero over the corpus.
                    same_x_hits += 1;
                    list.last_mut().unwrap().1 ^= edge.colour;
                } else {
                    list.push((row as u16 * 320 + ax as u16, edge.colour));
                    previous_colour = edge.colour;
                }
                edge.x = edge.x.wrapping_add(edge.dx);
            }
        }
        list.push((63_999, 0));
        list.push((u16::MAX, 0));
        (list, same_x_hits)
    }

    /// `ng_pass3` (`NEW.ASM:66-172`): walk the previous frame's double-
    /// buffered list and the new one together, repainting only the spans
    /// whose accumulated colour changed. Returns the painted pixel count.
    fn source_pass3(
        previous: &[(u16, u16)],
        current: &[(u16, u16)],
        bgpic: &[u8],
        screen: &mut [u8],
    ) -> usize {
        let mut old_index = 1;
        let mut new_index = 1;
        let (mut old_position, mut old_colour) = previous[0];
        let (mut new_position, mut new_colour) = current[0];
        let mut destination = 0usize;
        let mut new_accumulated = 0u8;
        let mut old_accumulated = 0u8;
        let mut painted = 0usize;

        let fill = |start: usize, end: usize, colour: u8, screen: &mut [u8]| {
            for (pixel, background) in screen[start..end].iter_mut().zip(&bgpic[start..end]) {
                *pixel = colour | *background;
            }
            end - start
        };

        loop {
            if old_position < new_position {
                // `@@23`: the old list's next boundary comes first.
                if new_accumulated != old_accumulated {
                    painted += fill(
                        destination,
                        old_position as usize,
                        new_accumulated,
                        screen,
                    );
                }
                destination = old_position as usize;
                old_accumulated ^= old_colour as u8;
                let (position, colour) = previous[old_index];
                old_index += 1;
                old_position = position;
                old_colour = colour;
            } else if new_position < old_position {
                // `@@21`'s cx < dx: the new list's next boundary comes first.
                if new_accumulated != old_accumulated {
                    painted += fill(
                        destination,
                        new_position as usize,
                        new_accumulated,
                        screen,
                    );
                }
                destination = new_position as usize;
                new_accumulated ^= new_colour as u8;
                let (position, colour) = current[new_index];
                new_index += 1;
                new_position = position;
                new_colour = colour;
            } else {
                // `@@22`: equal positions; -1 ends the walk.
                if new_position == u16::MAX {
                    break;
                }
                if new_accumulated != old_accumulated {
                    painted += fill(
                        destination,
                        old_position as usize,
                        new_accumulated,
                        screen,
                    );
                }
                destination = old_position as usize;
                old_accumulated ^= old_colour as u8;
                let (position, colour) = previous[old_index];
                old_index += 1;
                old_position = position;
                old_colour = colour;
            }
        }
        painted
    }

    /// The previous frame's list (the toggled `newdata1` half) and the
    /// screen those spans produced.
    struct SourceModel {
        previous: Vec<(u16, u16)>,
        screen: Vec<u8>,
    }

    impl SourceModel {
        fn new(bgpic: &[u8]) -> Self {
            // `ng_init` writes a -1 sentinel into a fresh half; the second
            // word is never read before the first pass3 ends.
            Self {
                previous: vec![(u16::MAX, 0)],
                screen: bgpic.to_vec(),
            }
        }

        fn frame(&mut self, bgpic: &[u8], polys: &[Poly]) -> (usize, usize) {
            let mut edges = source_edges(polys);
            let (current, same_x_hits) = source_pass2(&mut edges);
            let painted = source_pass3(&self.previous, &current, bgpic, &mut self.screen);
            self.previous = current;
            (painted, same_x_hits)
        }
    }

    // A fixed-point 8.8 rotation at 45-degree steps, so the corpus is
    // generated deterministically without a dependency on the maths layer.
    const COS: [i32; 8] = [256, 181, 0, -181, -256, -181, 0, 181];
    const SIN: [i32; 8] = [0, 181, 256, 181, 0, -181, -256, -181];

    const HEXAGON: &[(i32, i32)] = &[
        (0, -110),
        (95, -55),
        (95, 55),
        (0, 110),
        (-95, 55),
        (-95, -55),
    ];
    const TRIANGLE: &[(i32, i32)] = &[(0, -190), (165, 95), (-165, 95)];

    fn rotate(points: &[(i32, i32)], step: usize, tx: i32, ty: i32) -> Vec<(i16, i16)> {
        let cos = COS[step % 8];
        let sin = SIN[step % 8];
        points
            .iter()
            .map(|&(x, y)| {
                let rx = (x * cos - y * sin) >> 8;
                let ry = (x * sin + y * cos) >> 8;
                ((rx + tx) as i16, (ry + ty) as i16)
            })
            .collect()
    }

    struct FrameSpec {
        step: usize,
        colours: [u16; 3],
    }

    /// Frames 5 and 6 and frames 10 and 11 are identical (no span changes);
    /// frame 7 holds frame 6's geometry and changes only the colours. The
    /// colour words keep GLENZ's 0x40 high byte (`MAIN.C:176-240`), so the
    /// reference's literal "same x" compare stays inert exactly as it does
    /// in the shipped part.
    const SCENES: &[FrameSpec] = &[
        FrameSpec { step: 0, colours: [0x4002, 0x4004, 0x4006] },
        FrameSpec { step: 1, colours: [0x4002, 0x4004, 0x4006] },
        FrameSpec { step: 2, colours: [0x4003, 0x4005, 0x4007] },
        FrameSpec { step: 3, colours: [0x4003, 0x4005, 0x4007] },
        FrameSpec { step: 4, colours: [0x4005, 0x4007, 0x4009] },
        FrameSpec { step: 5, colours: [0x4005, 0x4007, 0x4009] },
        FrameSpec { step: 5, colours: [0x4005, 0x4007, 0x4009] },
        FrameSpec { step: 5, colours: [0x4008, 0x400A, 0x400C] },
        FrameSpec { step: 6, colours: [0x4008, 0x400A, 0x400C] },
        FrameSpec { step: 7, colours: [0x4009, 0x400B, 0x400D] },
        FrameSpec { step: 0, colours: [0x4009, 0x400B, 0x400D] },
        FrameSpec { step: 0, colours: [0x4009, 0x400B, 0x400D] },
    ];

    fn scene(spec: &FrameSpec) -> Vec<Poly> {
        let hexagon = rotate(HEXAGON, spec.step, 150, 100);
        vec![
            // A full-screen backdrop: its vertical edges clamp to 1 and
            // 319, so the coloured span covers 63,600 pixels.
            make_poly(spec.colours[0], &[(0, 0), (320, 0), (320, 200), (0, 200)]),
            make_poly(spec.colours[1], &hexagon),
            make_poly(
                spec.colours[2],
                &rotate(TRIANGLE, spec.step, 160, 110),
            ),
            // The hexagon again in a second colour: coincident edges the
            // source merges and the port keeps apart.
            make_poly(spec.colours[1] ^ 0x0003, &hexagon),
        ]
    }

    /// D36's proof: the port repaints every span from `bgpic`, and that is
    /// byte-identical to the source's double-buffered dirty-span walk over a
    /// corpus of moving, rotating polygons — including frames whose colours
    /// change across nearly every span and frames where nothing changes.
    #[test]
    fn the_full_repaint_equals_the_source_two_list_walk() {
        let bgpic: Vec<u8> = (0..PIXELS)
            .map(|i| ((i * 37 + 11) % 251) as u8)
            .collect();

        // The FC bounce leaves its last frame on the screen and `MAIN.C:398`
        // copies it into `bgpic`; the screen therefore starts as `bgpic`,
        // which is exactly the empty boundary list's composite (colour 0
        // everywhere), so the dirty walk's skips are sound from frame 0.
        let mut repaint = bgpic.clone();
        let mut model = SourceModel::new(&bgpic);
        // The source never paints the last pixel, so both buffers start with
        // a byte that a repaint would not produce there.
        repaint[PIXELS - 1] = 0x5A;
        model.screen[PIXELS - 1] = 0x5A;

        let mut frames: Vec<Vec<Poly>> = SCENES.iter().map(scene).collect();
        // A frame with no polygons at all: every previously coloured span
        // must return to `bgpic`.
        frames.push(Vec::new());

        let mut compositor = Compositor::new();
        let mut painted = Vec::with_capacity(frames.len());
        let mut same_x_hits = 0usize;
        for (index, polys) in frames.iter().enumerate() {
            compositor.clear();
            for poly in polys {
                compositor.add(poly);
            }
            compositor.paint(&bgpic, &mut repaint);
            let (painted_here, hits) = model.frame(&bgpic, polys);
            painted.push(painted_here);
            same_x_hits += hits;
            assert_eq!(
                repaint, model.screen,
                "the repaint and the two-list walk differ at frame {index}"
            );
        }
        // The literal "same x" compare reads the previous boundary's colour
        // word; the equivalence is only valid while the corpus stays in
        // GLENZ's 0x40xx words, where it never equals a clamped column.
        assert_eq!(same_x_hits, 0, "the corpus left GLENZ's colour domain");

        // The held frames repaint nothing: the two-list walk leaves the
        // screen alone, and the repaint rewrites the same bytes.
        assert_eq!(painted[6], 0, "frame 6 repeats frame 5");
        assert_eq!(painted[11], 0, "frame 11 repeats frame 10");
        // Frame 7 keeps frame 6's geometry and changes the colours: the
        // backdrop's 318x200-pixel spans alone are 63,600 pixels.
        assert!(
            painted[7] > 60_000,
            "frame 7 painted only {} pixels",
            painted[7]
        );
        // The final empty frame restores the background: the backdrop's
        // 318 pixels on each of the 200 rows were coloured, and columns 0
        // and 319 (including the never-painted 63999) were not.
        assert_eq!(painted[12], 318 * 200);
        // Pixel 63999 was never written by either implementation.
        assert_eq!(repaint[PIXELS - 1], 0x5A);
    }
}
