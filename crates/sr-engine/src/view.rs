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
    /// CRTC display start (0x0C/0x0D), in pixels rather than plane bytes:
    /// `u32` because a 640×400 buffer's 256,000 pixels exceed 16 bits (D16).
    pub start_addr: u32,
    /// Extra pixel offset within a line, added to `start_addr`. A line is at
    /// most as wide as the surface, so 16 bits are room enough for it.
    pub x_offset: u16,
    /// Pixel stride between displayed scanlines (CRTC 0x13), or `None` for the
    /// surface's own row stride, resolved in [`View::source_index`]. Any other
    /// stride shears a picture drawn through [`Surface`], without an error.
    pub y_line_offset: Option<u16>,
    /// Fine horizontal shift, **in pixels**, 0..=7. In 256-colour modes AC 0x13
    /// shifts `(value & 0x7) / 2` pixels (`vga_attr.cpp:225-227`), so a part
    /// converts the register value (CRED; `docs/fidelity/cred.md` §3).
    pub pan: u8,
    pub double_scan: bool, // CRTC reg 9 bit 7
    pub split: Option<Box<Split>>,
    pub border: Option<u8>, // overscan colour (AC 0x11)
}

/// A line-compare split: from `scanline` down, the beam reads `view` instead.
///
/// **`scanline` is the first scanline the lower view draws**, counted within
/// the enclosing view. The line-compare register names the last line *above*
/// the split (`(line_compare + 1) / lines_scaled - vblank_skip`,
/// `vga_draw.cpp:1586-1593`; the beam resets at `:1276`), so a value copied out
/// of the original's assembly is one less than this (D16).
///
/// **The lower view's origin is 0.** The hardware zeroes the address counter
/// and panning at the split (`VGA_ProcessSplit`, `vga_draw.cpp:1139-1158`), so
/// a part leaves the lower view's `start_addr`, `x_offset` and `pan` as
/// [`View::new`] sets them; only the test card moves them.
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

    /// Index into `surface` for displayed pixel (x, y), with the stride
    /// resolved per effective view. Wraps, like the CRTC address counter.
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

    /// The surface row displayed scanline `y` reads, on a 320-wide surface.
    fn row_of(v: &View, y: u16) -> usize {
        v.source_index(&Surface::new(320, 200), 0, y) / 320
    }

    /// A view that has not asked for a stride reads the surface's own rows.
    #[rstest]
    fn the_line_stride_defaults_to_the_surface_row_stride() {
        let mut s = Surface::new(640, 400);
        s.set(3, 2, 7);
        let v = View::new(320, 200);
        assert_eq!(v.stride(640), 640);
        assert_eq!(v.source_index(&s, 3, 2), 2 * 640 + 3);
        assert_eq!(s.pixels()[v.source_index(&s, 3, 2)], 7);
    }

    /// CRTC 0x13 may pack rows more tightly than the surface's.
    #[rstest]
    fn an_explicit_line_stride_overrides_the_surface_row_stride() {
        let mut v = View::new(320, 200);
        v.y_line_offset = Some(320);
        let s = Surface::new(640, 400);
        assert_eq!(v.stride(640), 320);
        assert_eq!(v.source_index(&s, 3, 2), 2 * 320 + 3);
    }

    #[rstest]
    fn source_index_wraps_past_the_end() {
        let mut v = View::new(4, 1);
        v.start_addr = 6;
        assert_eq!(v.source_index(&Surface::new(4, 2), 3, 0), 1);
    }

    /// ENDSCRL's ring of 400 640-pixel lines: origin 256,000 is the surface's
    /// length and wraps to the top; 255,360 is the last line.
    #[test]
    fn an_origin_beyond_u16_is_not_wrapped_away() {
        let mut v = View::new(640, 400);
        v.set_origin(256_000);
        let s = Surface::new(640, 400);
        assert_eq!(v.source_index(&s, 0, 0), 0);
        v.set_origin(255_360);
        assert_eq!(v.source_index(&s, 0, 0), 255_360);
    }

    /// CRED shows 200 rows from logical line 400 of 320-pixel lines, stepping two
    /// logical lines a row (CRTC 0x13 = `0x5013`, `CREDITS/TWEAK.ASM:43`).
    #[test]
    fn creds_origin_reads_every_other_logical_line() {
        let mut v = View::new(320, 200);
        v.set_origin(400 * 320);
        v.y_line_offset = Some(640);
        let s = Surface::new(640, 400);
        assert_eq!(v.source_index(&s, 0, 1), 400 * 320 + 640);
        assert_eq!(v.source_index(&s, 319, 199), 400 * 320 + 199 * 640 + 319);
    }

    #[test]
    fn set_origin_writes_the_display_start_and_clears_the_offset() {
        let mut v = View::new(320, 200);
        v.x_offset = 7;
        v.set_origin(128_000);
        assert_eq!(v.start_addr, 128_000);
        assert_eq!(v.x_offset, 0);
        assert_eq!(v.source_index(&Surface::new(640, 400), 3, 2), 128_000 + 2 * 640 + 3);
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
        #[case] source_row: usize,
    ) {
        let mut v = View::new(320, 400);
        v.double_scan = true;
        assert_eq!(row_of(&v, screen_y), source_row);
    }

    /// Below a split the CRTC's counters restart, so the first lower row reads
    /// the lower view's row 0, not row 120.
    #[rstest]
    fn a_split_restarts_the_scanline_count_where_it_starts(mut v320: View) {
        v320.split = Some(Box::new(Split { scanline: 120, view: View::new(320, 200) }));
        assert_eq!(row_of(&v320, 119), 119);
        assert_eq!(row_of(&v320, 120), 0);
        assert_eq!(row_of(&v320, 125), 5);
    }

    #[rstest]
    fn split_beyond_height_is_inert(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(999);
        v320.split = Some(Box::new(Split { scanline: 500, view: lower }));
        assert_eq!(v320.effective(199).start_addr, 0);
        assert_eq!(v320.source_index(&Surface::new(320, 200), 0, 199), 199 * 320);
    }

    /// Each split's scanline counts within the view it starts from: the inner
    /// split, 150 lines into the outer one's lower view, begins at frame line 200.
    #[rstest]
    fn nested_splits_resolve_to_the_deepest_match() {
        let mut mid = View::new(320, 200);
        mid.set_origin(1000);
        let mut low = View::new(320, 200);
        low.set_origin(2000);
        mid.split = Some(Box::new(Split { scanline: 150, view: low }));
        let mut top = View::new(320, 200);
        top.split = Some(Box::new(Split { scanline: 50, view: mid }));
        assert_eq!(top.effective(49).start_addr, 0);
        assert_eq!(top.effective(50).start_addr, 1000);
        assert_eq!(top.effective(199).start_addr, 1000);
        assert_eq!(top.effective(200).start_addr, 2000);
    }
}
