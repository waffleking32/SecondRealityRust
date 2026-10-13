//! The CRTC replacement: the transform the beam samples the surface through.
//!
//! Display start, line offset, fine pan, double-scan, the line-compare split
//! and the overscan colour, in one register set.

use crate::surface::Surface;

/// One CRTC register set: where the beam starts in the surface, how far it
/// steps per scanline, and how wide the displayed picture is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub width: u16,  // displayed width in pixels
    pub height: u16, // displayed height in scanlines
    /// CRTC display start (0x0C/0x0D), in pixels.
    ///
    /// `u32`, where the CRTC's register pair is 16 bits, because the register
    /// counts *bytes within a plane* and this `View` counts pixels: a whole
    /// 640×400 buffer is 256,000 of them, which 16 bits do not reach. CRED's
    /// display start is 400 × 320 = 128,000 and ENDSCRL's ring walks up to
    /// 256,000; both are here rather than in `x_offset`, whose 16 bits cannot
    /// hold either. See `docs/decisions.md` D16.
    pub start_addr: u32,
    /// Extra pixel offset within a line, added to `start_addr`. A line is at
    /// most as wide as the surface, so 16 bits are room enough for it.
    pub x_offset: u16,
    /// Pixel stride between displayed scanlines (CRTC 0x13), or `None` — the
    /// default — for the surface's own row stride.
    ///
    /// `None` is what a part wants unless it has deliberately packed its rows
    /// more tightly than the surface's: a picture drawn through [`Surface`] is
    /// laid out at the surface's stride, and reading it back at any other
    /// stride shears it, without an error anywhere. The stride is resolved
    /// where the surface is in hand, in [`View::source_index`], so a view built
    /// before the surface existed cannot get it wrong.
    pub y_line_offset: Option<u16>,
    pub pan: u8,           // fine horizontal shift, 0..=7 (AC 0x13)
    pub double_scan: bool, // CRTC reg 9 bit 7
    pub split: Option<Box<Split>>,
    pub border: Option<u8>, // overscan colour (AC 0x11)
}

/// A line-compare split: from `scanline` down, the beam reads `view` instead.
///
/// # The two conventions a part author has to know
///
/// **`scanline` is the first displayed scanline the lower view draws**, counted
/// within the enclosing view. The hardware's line-compare register names the
/// last line *above* the split: the emulator applies the register at
/// `(line_compare + 1) / lines_scaled - vblank_skip` (`vga_draw.cpp:1586-1593`)
/// and resets the beam at the start of that scanline (`vga_draw.cpp:1276`). A
/// value copied straight out of the original's assembly is therefore **one
/// less** than the `scanline` to write here. `lines_scaled` and `vblank_skip`
/// are special cases — a mode forced to single scan, and a vertical-blanking
/// trick — that Phase 0 has not met; the first part that splits settles the
/// mapping by measurement against a capture rather than by trusting the +1.
///
/// **The lower view's origin is 0.** At the split the hardware sets the address
/// counter to 0, clears panning and resets the sub-row phase
/// (`VGA_ProcessSplit`, `vga_draw.cpp:1139-1158`), so the lower region reads
/// from the top of video memory. [`View::new`] gives exactly that, which means
/// a part modelling the hardware leaves `start_addr`, `x_offset` and `pan`
/// alone. They are settable because the test card uses them to prove the view
/// switch lands on named scanlines — a picture the hardware cannot produce.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Split {
    pub scanline: u16, // first scanline using `view`, counted within the enclosing view
    pub view: View,
}

impl View {
    /// Origin at 0, line stride unset (the surface's own row stride), no split,
    /// no border.
    pub fn new(width: u16, height: u16) -> View {
        View {
            width,
            height,
            start_addr: 0,
            x_offset: 0,
            y_line_offset: None,
            pan: 0,
            double_scan: false,
            split: None,
            border: None,
        }
    }

    /// The stride the beam reads with, against a surface whose own rows are
    /// `surface_stride` pixels apart.
    pub fn stride(&self, surface_stride: u16) -> u16 {
        self.y_line_offset.unwrap_or(surface_stride)
    }

    /// Sets `start_addr` and zeroes `x_offset`: the origin is a display start,
    /// and `x_offset` is only an offset within a line on top of one.
    pub fn set_origin(&mut self, pixel: u32) {
        self.start_addr = pixel;
        self.x_offset = 0;
    }

    /// The view in force at displayed scanline `y`, and the scanline number
    /// within that view. Below a split the inner number restarts at 0: line
    /// compare resets the CRTC's address and scanline counters, which is the
    /// whole mechanism by which a split shows anything different.
    ///
    /// A split at or beyond `height` is inert. `Box` makes cycles impossible.
    pub fn effective_at(&self, y: u16) -> (&View, u16) {
        let mut v = self;
        let mut ly = y;
        while let Some(s) = &v.split {
            if ly < s.scanline {
                break;
            }
            ly -= s.scanline;
            v = &s.view;
        }
        (v, ly)
    }

    /// Convenience: `self.effective_at(y).0`.
    pub fn effective(&self, y: u16) -> &View {
        self.effective_at(y).0
    }

    /// Surface row sampled by displayed scanline `y`, after double-scan.
    pub fn surface_row(&self, y: u16) -> u16 {
        let (v, ly) = self.effective_at(y);
        ly >> v.double_scan as u16
    }

    /// Index into `surface` for displayed pixel (x, y). Wraps, like the CRTC
    /// address counter.
    ///
    /// The stride is resolved here, per view — including the lower view of a
    /// split — because this is where the surface is in hand. Resolving it once
    /// for the enclosing view would leave a split's lower view reading at the
    /// wrong stride.
    pub fn source_index(&self, surface: &Surface, x: u16, y: u16) -> usize {
        let (v, ly) = self.effective_at(y);
        let stride = v.stride(surface.width() as u16);
        (v.start_addr as usize
            + v.x_offset as usize
            + (ly >> v.double_scan as u16) as usize * stride as usize
            + v.pan as usize
            + x as usize)
            % surface.pixels().len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    #[fixture]
    fn v320() -> View {
        View::new(320, 200)
    }

    #[rstest]
    fn new_defaults_to_a_contiguous_origin(v320: View) {
        assert_eq!(v320.start_addr, 0);
        assert_eq!(v320.x_offset, 0);
        assert_eq!(v320.y_line_offset, None);
        assert_eq!(v320.pan, 0);
        assert!(!v320.double_scan);
        assert!(v320.split.is_none());
        assert!(v320.border.is_none());
    }

    /// A view that has not asked for a stride reads the surface's own rows.
    /// Getting this wrong shears the picture and reports nothing.
    #[rstest]
    fn the_line_stride_defaults_to_the_surface_row_stride() {
        let mut s = Surface::new(640, 400);
        s.set(3, 2, 7);
        let v = View::new(320, 200);
        assert_eq!(v.stride(640), 640);
        assert_eq!(v.source_index(&s, 3, 2), 2 * 640 + 3);
        assert_eq!(s.pixels()[v.source_index(&s, 3, 2)], 7);
    }

    /// CRTC 0x13 is independent of the view's width: a part may pack its rows
    /// more tightly than the surface's.
    #[rstest]
    fn an_explicit_line_stride_overrides_the_surface_row_stride() {
        let mut v = View::new(320, 200);
        v.y_line_offset = Some(320);
        let s = Surface::new(640, 400);
        assert_eq!(v.stride(640), 320);
        assert_eq!(v.source_index(&s, 3, 2), 2 * 320 + 3);
    }

    #[rstest]
    fn source_index_follows_the_line_offset(mut v320: View) {
        v320.set_origin(10);
        assert_eq!(v320.source_index(&Surface::new(320, 200), 3, 2), 10 + 2 * 320 + 3);
    }

    #[rstest]
    fn source_index_wraps_past_the_end() {
        let mut v = View::new(4, 1);
        v.start_addr = 6;
        assert_eq!(v.source_index(&Surface::new(4, 2), 3, 0), 1);
    }

    /// The origin is a pixel address into the surface, and the surface holds
    /// more of them than a `u16` can name.
    ///
    /// ENDSCRL is the case that forces it: its display start walks a 400-line
    /// ring of 640-pixel lines, so the last line of the ring starts at
    /// 399 × 640 = 255,360 and the step past it lands on 256,000 — the
    /// surface's own length, where the beam wraps to the top of the ring. An
    /// origin that wrapped at 65,536 instead would read the middle of the ring
    /// and paint a scroller several hundred lines out, with no error anywhere.
    #[test]
    fn an_origin_beyond_u16_is_not_wrapped_away() {
        // ENDSCRL's ring: the beam starts at the 400th 640-pixel line.
        let mut v = View::new(640, 400);
        v.set_origin(256_000);
        let s = Surface::new(640, 400);
        // 256,000 % 256,000 == 0: the beam wraps to the top of the ring, which is
        // what the surface of 400 ring lines means.
        assert_eq!(v.source_index(&s, 0, 0), 0);
        v.set_origin(255_360);
        assert_eq!(v.source_index(&s, 0, 0), 255_360);
    }

    /// The second case that forces the widening: CRED's display start. The part
    /// writes the surface at 320-pixel logical lines and shows 200 rows from
    /// logical line 400 — pixel 400 × 320 = 128,000 — with each displayed row
    /// stepping *two* logical lines, because CRTC 0x13 = `0x5013` is a
    /// 160-planar-byte line offset, 640 chunky pixels (`CREDITS/TWEAK.ASM:43`).
    #[test]
    fn creds_origin_reads_every_other_logical_line() {
        // CRED displays 200 rows from logical line 400 with a 160-byte line
        // offset: row ly starts at line 400 + 2*ly of a 320-px logical line.
        let mut v = View::new(320, 200);
        v.set_origin(400 * 320);
        v.y_line_offset = Some(640);
        let s = Surface::new(640, 400);
        assert_eq!(v.source_index(&s, 0, 1), 400 * 320 + 640);
        assert_eq!(v.source_index(&s, 319, 199), 400 * 320 + 199 * 640 + 319);
    }

    /// `set_origin` names a display start, and the display start is the field
    /// that carries one: the origin is where the beam begins reading, and
    /// `x_offset` is a within-line offset on top of it, which a newly named
    /// origin replaces.
    #[test]
    fn set_origin_writes_the_display_start_and_clears_the_offset() {
        let mut v = View::new(320, 200);
        v.x_offset = 7;
        v.set_origin(128_000);
        assert_eq!(v.start_addr, 128_000);
        assert_eq!(v.x_offset, 0);
    }

    #[rstest]
    fn pan_shifts_by_one_pixel_per_unit(mut v320: View) {
        v320.pan = 5;
        assert_eq!(v320.source_index(&Surface::new(320, 200), 0, 0), 5);
    }

    #[rstest]
    #[case(0, 0)]
    #[case(1, 0)]
    #[case(2, 1)]
    #[case(399, 199)]
    fn double_scan_maps_two_scanlines_to_one_surface_row(
        #[case] screen_y: u16,
        #[case] source_row: u16,
    ) {
        let mut v = View::new(320, 200);
        v.double_scan = true;
        assert_eq!(v.surface_row(screen_y), source_row);
    }

    #[rstest]
    fn single_scan_advances_one_row_per_scanline() {
        let v = View::new(320, 200);
        assert_eq!(v.surface_row(0), 0);
        assert_eq!(v.surface_row(1), 1);
        assert_eq!(v.surface_row(199), 199);
    }

    #[rstest]
    fn a_split_restarts_the_scanline_count_where_it_starts(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(0);
        v320.split = Some(Box::new(Split { scanline: 120, view: lower }));
        // Above the split the row is the frame row.
        assert_eq!(v320.surface_row(119), 119);
        // Below it the CRTC's counters restart, so the first post-split row
        // is 0, not 120. Counting from the frame top makes line compare a
        // no-op — see the note in the Interfaces block.
        assert_eq!(v320.surface_row(120), 0);
        assert_eq!(v320.surface_row(125), 5);
    }

    #[rstest]
    fn split_switches_view_at_its_scanline(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(320 * 100);
        v320.split = Some(Box::new(Split {
            scanline: 120,
            view: lower,
        }));
        // The origin a view shows is its `start_addr`, so that is what says
        // which view is in force: the enclosing one above the split, the lower
        // one from the split down.
        assert_eq!(v320.effective(119).start_addr, 0);
        assert_eq!(v320.effective(120).start_addr, 320 * 100);
    }

    #[rstest]
    fn split_beyond_height_is_inert(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(999);
        v320.split = Some(Box::new(Split {
            scanline: 500,
            view: lower,
        }));
        assert_eq!(v320.effective(199).start_addr, 0);
        assert_eq!(v320.source_index(&Surface::new(320, 200), 0, 199), 199 * 320);
    }

    #[rstest]
    fn nested_splits_resolve_to_the_deepest_match() {
        let mut mid = View::new(320, 200);
        mid.set_origin(1000);
        let mut low = View::new(320, 200);
        low.set_origin(2000);
        mid.split = Some(Box::new(Split {
            scanline: 150,
            view: low,
        }));
        let mut top = View::new(320, 200);
        top.split = Some(Box::new(Split {
            scanline: 50,
            view: mid,
        }));
        // Each split's scanline is counted within the view it starts from: the
        // inner split is 150 lines after the outer one, so it begins at frame
        // scanline 200, not 150.
        assert_eq!(top.effective(49).start_addr, 0);
        assert_eq!(top.effective(50).start_addr, 1000);
        assert_eq!(top.effective(199).start_addr, 1000);
        assert_eq!(top.effective(200).start_addr, 2000);
    }
}
