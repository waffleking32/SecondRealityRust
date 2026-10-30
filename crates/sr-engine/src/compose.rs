//! Beam compose: the finished frame, and the scanline band the beam draws.
//!
//! [`compose_band`] samples the surface through the view **as it stands at the
//! moment of the call**. That is what lets a mid-frame VRAM update land on the
//! scanline the beam actually drew it on, instead of a whole frame late.

use crate::palette::{Palette, PaletteTimeline};
use crate::surface::Surface;
use crate::view::View;

/// One finished frame: the pixels the beam drew, and the palette timeline the
/// engine stamped as each band was composed.
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

/// Scanlines `[first, last)` of band `band` of `bands`. The floor partition
/// (`height * band / bands`) covers `0..height` with no gap or overlap.
pub fn band_range(height: u16, bands: u16, band: u16) -> (u16, u16) {
    let (height, bands, band) = (height as u32, bands as u32, band as u32);
    (
        (height * band / bands) as u16,
        (height * (band + 1) / bands) as u16,
    )
}

/// Copy displayed scanlines `[first, last)` from `surface` through `view`
/// into `frame`: the beam. `frame.palettes` is the engine's to stamp.
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
        // Below the split the count restarts: rows 2 and 3 read surface rows 1 and 2.
        assert_eq!(f.row(2), &[2; 8]);
        assert_eq!(f.row(3), &[3; 8]);
    }

    #[test]
    fn a_lower_view_resolves_its_own_line_stride() {
        // The stride is resolved per effective view, not once for the enclosing one.
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
}
