//! The CRTC replacement: the transform the beam samples the surface through.
//!
//! Display start, line offset, fine pan, double-scan, the line-compare split
//! and the overscan colour, in one register set.

use crate::surface::Surface;

/// One CRTC register set: where the beam starts in the surface, how far it
/// steps per scanline, and how wide the displayed picture is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct View {
    pub width: u16,      // displayed width in pixels
    pub height: u16,     // displayed height in scanlines
    pub start_addr: u16, // CRTC display start (0x0C/0x0D), in pixels
    pub x_offset: u16,   // planar byte-to-pixel offset; added to start_addr
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

    /// Sets `x_offset` and zeroes `start_addr`.
    pub fn set_origin(&mut self, pixel: u16) {
        self.start_addr = 0;
        self.x_offset = pixel;
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
        assert_eq!(v320.effective(119).x_offset, 0);
        assert_eq!(v320.effective(120).x_offset, 320 * 100);
    }

    #[rstest]
    fn split_beyond_height_is_inert(mut v320: View) {
        let mut lower = View::new(320, 200);
        lower.set_origin(999);
        v320.split = Some(Box::new(Split {
            scanline: 500,
            view: lower,
        }));
        assert_eq!(v320.effective(199).x_offset, 0);
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
        assert_eq!(top.effective(49).x_offset, 0);
        assert_eq!(top.effective(50).x_offset, 1000);
        assert_eq!(top.effective(199).x_offset, 1000);
        assert_eq!(top.effective(200).x_offset, 2000);
    }
}
