//! Beam compose: the finished frame, and the scanline band the beam draws.
//!
//! [`compose_band`] samples the surface through the view **as it stands at the
//! moment of the call**. That is what lets a mid-frame VRAM update land on the
//! scanline the beam actually drew it on, instead of a whole frame late.

use crate::palette::{Palette, PaletteTimeline};
use crate::surface::Surface;
use crate::view::View;

/// One finished frame: the pixels the beam drew, and the palette timeline.
///
/// The timeline is stamped by the engine as each band is composed, not by
/// [`compose_band`] — see that function's note.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndexedFrame {
    pub width: u16,
    pub height: u16,
    pub pixels: Vec<u8>, // len == width * height
    pub palettes: PaletteTimeline,
    pub border: Option<u8>, // overscan colour (AC 0x11)
}

impl IndexedFrame {
    /// All pixels 0, `palettes` black with no changes, no border.
    pub fn new(width: u16, height: u16) -> IndexedFrame {
        IndexedFrame {
            width,
            height,
            pixels: vec![0; width as usize * height as usize],
            palettes: PaletteTimeline::new(Palette::BLACK),
            border: None,
        }
    }

    /// The palette in force at `scanline`.
    pub fn palette_for(&self, scanline: u16) -> &Palette {
        self.palettes.palette_at(scanline)
    }

    pub fn pixel(&self, x: u16, y: u16) -> u8 {
        self.pixels[y as usize * self.width as usize + x as usize]
    }

    pub fn row(&self, y: u16) -> &[u8] {
        let start = y as usize * self.width as usize;
        &self.pixels[start..start + self.width as usize]
    }
}

/// Scanlines `[first, last)` of band `band` of `bands`, partitioning
/// `0..height` exactly for any `height` and any `bands > 0`.
///
/// The floor-partition (`height * band / bands`) is what makes the parts meet
/// exactly: a rounded chunk size would leave a gap or an overlap whenever
/// `height` is not divisible by `bands`.
pub fn band_range(height: u16, bands: u16, band: u16) -> (u16, u16) {
    let (height, bands, band) = (height as u32, bands as u32, band as u32);
    (
        (height * band / bands) as u16,
        (height * (band + 1) / bands) as u16,
    )
}

/// Copy displayed scanlines `[first, last)` from `surface` through `view`
/// into `frame`, sampling the surface as it stands now — this is the beam.
///
/// Deliberately does not touch `frame.palettes`: which palette was in force at
/// which scanline is the engine's business, so the "palette in force when the
/// beam reached this scanline" rule lives in one place.
pub fn compose_band(
    frame: &mut IndexedFrame,
    surface: &Surface,
    view: &View,
    first: u16,
    last: u16,
) {
    let width = frame.width as usize;
    let source = surface.pixels();
    for y in first..last {
        let start = y as usize * width;
        let row = &mut frame.pixels[start..start + width];
        for (x, pixel) in row.iter_mut().enumerate() {
            *pixel = source[view.source_index(surface, x as u16, y)];
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::Palette;
    use crate::view::Split;
    use rstest::*;

    fn striped_surface() -> Surface {
        // Each row is filled with its own index, so the source row is readable
        // from any pixel in the composed frame.
        let mut s = Surface::new(8, 4);
        for y in 0..4 { s.row_mut(y).fill(y as u8 + 1); }
        s
    }

    #[test]
    fn band_range_partitions_exactly_for_any_band_count() {
        for &height in &[200u16, 400, 1, 7] {
            for bands in 1..=16u16 {
                let mut cursor = 0u16;
                for b in 0..bands {
                    let (first, last) = band_range(height, bands, b);
                    assert_eq!(first, cursor, "gap or overlap at band {b} of {bands} h{height}");
                    assert!(last >= first);
                    cursor = last;
                }
                assert_eq!(cursor, height, "bands {bands} did not cover height {height}");
            }
        }
    }

    #[rstest]
    #[case(200, 1, 0, (0, 200))]
    #[case(200, 7, 0, (0, 28))]
    #[case(200, 7, 6, (171, 200))]
    #[case(200, 8, 3, (75, 100))]
    #[case(400, 8, 7, (350, 400))]
    fn band_range_splits_evenly_when_height_is_not_divisible(
        #[case] height: u16,
        #[case] bands: u16,
        #[case] band: u16,
        #[case] expected: (u16, u16),
    ) {
        assert_eq!(band_range(height, bands, band), expected);
    }

    #[test]
    fn compose_band_copies_the_right_source_rows() {
        let s = striped_surface();
        let v = View::new(8, 4);
        let mut f = IndexedFrame::new(8, 4);
        compose_band(&mut f, &s, &v, 1, 3);
        assert_eq!(f.row(0), &[0; 8]);
        assert_eq!(f.row(1), &[2; 8]);
        assert_eq!(f.row(2), &[3; 8]);
        assert_eq!(f.row(3), &[0; 8]);
    }

    #[test]
    fn compose_band_reads_the_rows_the_part_wrote() {
        // A part draws with `Surface::set` or `row_mut`, which lay rows out at
        // the surface's own stride. A view that has not asked for anything else
        // must read them back at that same stride.
        //
        // The bug this pins was live: `View::new` defaulted the stride to the
        // *view's* width, so a 320-wide picture on the 640-wide surface read
        // each row's other half — a sheared picture, with no error anywhere.
        let mut s = Surface::new(640, 400);
        for y in 0..4 {
            s.row_mut(y)[..320].fill((y + 1) as u8);
        }
        let v = View::new(320, 4);
        let mut f = IndexedFrame::new(320, 4);
        compose_band(&mut f, &s, &v, 0, 4);
        for y in 0..4 {
            assert_eq!(f.row(y as u16), &[(y + 1) as u8; 320][..], "row {y}");
        }
    }

    #[test]
    fn compose_band_uses_the_split_view_for_lower_scanlines() {
        let s = striped_surface();
        let mut v = View::new(8, 4);
        let mut lower = View::new(8, 4);
        lower.set_origin(8); // one surface row down
        v.split = Some(Box::new(Split { scanline: 2, view: lower }));
        let mut f = IndexedFrame::new(8, 4);
        compose_band(&mut f, &s, &v, 0, 4);
        assert_eq!(f.row(0), &[1; 8]);
        assert_eq!(f.row(1), &[2; 8]);
        // Row 2 is the first post-split scanline, so it counts from 0 within the
        // lower view: origin 8 lands on surface row 1. Row 3 counts from 1,
        // landing on surface row 2. Counting from the frame top instead would
        // give rows 3 and 0 — which is the bug this test exists to catch.
        assert_eq!(f.row(2), &[2; 8]);
        assert_eq!(f.row(3), &[3; 8]);
    }

    #[test]
    fn a_lower_view_resolves_its_own_line_stride() {
        // The stride is resolved per effective view, not once for the enclosing
        // one. A split's lower view is built by the part and is usually a fresh
        // `View::new`, so resolving only the outer view would read the lower
        // half at the upper half's stride.
        let mut s = Surface::new(640, 400);
        for y in 0..2 {
            s.row_mut(y).fill(1);
        }
        for y in 2..4 {
            s.row_mut(y)[..320].fill(2);
        }
        let mut v = View::new(320, 4);
        let mut lower = View::new(320, 4);
        lower.set_origin(640 * 2); // surface row 2
        v.split = Some(Box::new(Split { scanline: 2, view: lower }));

        let mut f = IndexedFrame::new(320, 4);
        compose_band(&mut f, &s, &v, 0, 4);
        assert_eq!(f.row(0), &[1; 320][..]);
        assert_eq!(f.row(1), &[1; 320][..]);
        assert_eq!(f.row(2), &[2; 320][..]);
        assert_eq!(f.row(3), &[2; 320][..], "the lower view's second row");
    }

    #[test]
    fn timeline_returns_the_last_change_at_or_before_the_scanline() {
        let mut tl = PaletteTimeline::new(Palette::BLACK);
        let mut a = Palette::BLACK; a.set_entry(0, [1, 0, 0]);
        let mut b = Palette::BLACK; b.set_entry(0, [2, 0, 0]);
        tl.push(10, a);
        tl.push(20, b);
        assert_eq!(tl.palette_at(0).entry(0), [0, 0, 0]);
        assert_eq!(tl.palette_at(10).entry(0), [1, 0, 0]);
        assert_eq!(tl.palette_at(19).entry(0), [1, 0, 0]);
        assert_eq!(tl.palette_at(20).entry(0), [2, 0, 0]);
        assert_eq!(tl.palette_at(399).entry(0), [2, 0, 0]);
    }

    #[test]
    fn timeline_replaces_a_change_at_the_same_scanline() {
        let mut tl = PaletteTimeline::new(Palette::BLACK);
        let mut a = Palette::BLACK; a.set_entry(0, [1, 0, 0]);
        let mut b = Palette::BLACK; b.set_entry(0, [2, 0, 0]);
        tl.push(10, a);
        tl.push(10, b);
        assert_eq!(tl.change_count(), 1);
        assert_eq!(tl.palette_at(10).entry(0), [2, 0, 0]);
    }

    #[test]
    fn timeline_reset_drops_changes_and_restores_the_base() {
        let mut tl = PaletteTimeline::new(Palette::BLACK);
        let mut a = Palette::BLACK; a.set_entry(0, [1, 0, 0]);
        tl.push(10, a);
        tl.reset(Palette::BLACK);
        assert_eq!(tl.change_count(), 0);
        assert_eq!(tl.palette_at(10).entry(0), [0, 0, 0]);
    }
}
