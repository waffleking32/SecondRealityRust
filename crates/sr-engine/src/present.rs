//! Presentation: the composed indexed frame as RGBA pixels a window can blit.
//!
//! Aspect, scaling and the border are **display** concerns and they live only
//! here. `xtask compare` reads the composed [`IndexedFrame`] directly, so none
//! of them can affect a fidelity measurement: they change what a monitor shows,
//! not what the engine drew.
//!
//! The border is an approximation rather than a modelled overscan region. The
//! original put its overscan colour in the blanking outside the active display,
//! where the monitor's sync timing fixed the width; here it is a flat pad of
//! eight source pixels per side, scaled with the picture. That padding is
//! recorded as the first entry in `docs/fidelity/APPROXIMATIONS.md`.

use crate::compose::IndexedFrame;

/// The overscan pad, in source pixels per side, before scaling.
const BORDER_PX: usize = 8;

/// How the source picture is fitted to the output raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    /// The 6/5 horizontal stretch of a 4:3 monitor: 320×200 fills a 4:3 box.
    FourThree,
    /// One output pixel per source pixel; 320×200 stays 16:10.
    SquarePixels,
}

/// Everything the presenter needs beyond the frame itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentOptions {
    pub aspect: Aspect,
    /// Integer nearest-neighbour magnification; at least 1.
    pub scale: u32,
    /// Draw the frame's overscan colour as a border, when it has one.
    pub border: bool,
}

impl Default for PresentOptions {
    fn default() -> Self {
        PresentOptions { aspect: Aspect::FourThree, scale: 1, border: true }
    }
}

/// An RGBA8 raster: `width * height` pixels, four bytes each.
#[derive(Debug)]
pub struct FrameBuffer {
    width: usize,
    height: usize,
    rgba: Vec<u8>, // len == width * height * 4
}

impl FrameBuffer {
    /// A `width` × `height` raster, every byte zero.
    pub fn new(width: usize, height: usize) -> FrameBuffer {
        FrameBuffer {
            width,
            height,
            rgba: vec![0; width * height * 4],
        }
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    pub fn rgba_mut(&mut self) -> &mut [u8] {
        &mut self.rgba
    }

    /// The RGBA quad at `(x, y)`.
    ///
    /// # Panics
    /// When `(x, y)` is outside the raster.
    pub fn pixel(&self, x: usize, y: usize) -> [u8; 4] {
        let at = (y * self.width + x) * 4;
        [
            self.rgba[at],
            self.rgba[at + 1],
            self.rgba[at + 2],
            self.rgba[at + 3],
        ]
    }
}

/// The shape of the output raster: the border pad, the picture inside it, and
/// the total. Both [`output_size`] and [`present`] derive their geometry from
/// this one calculation, so the two cannot disagree about it.
struct Raster {
    width: usize,
    height: usize,
    pad: usize,
    inner_w: usize,
    inner_h: usize,
}

fn raster(frame: &IndexedFrame, opts: &PresentOptions) -> Raster {
    debug_assert!(opts.scale >= 1, "scale is a magnification, so it cannot be 0");
    let scale = opts.scale.max(1) as usize;
    let (inner_w, inner_h) = match opts.aspect {
        Aspect::FourThree => {
            // The 6/5 stretch lands on whole pixels whenever the source width is
            // a multiple of 5 — every geometry the demo uses is. A narrower
            // width would truncate, which is what the assert is there to catch.
            debug_assert!(
                frame.width % 5 == 0,
                "a 4:3 box needs a source width that is a multiple of 5, and {} is not",
                frame.width
            );
            let w = frame.width as usize * 6 * scale / 5;
            (w, w * 3 / 4)
        }
        Aspect::SquarePixels => (
            frame.width as usize * scale,
            frame.height as usize * scale,
        ),
    };
    let pad = if opts.border && frame.border.is_some() {
        BORDER_PX * scale
    } else {
        0
    };
    Raster {
        width: inner_w + 2 * pad,
        height: inner_h + 2 * pad,
        pad,
        inner_w,
        inner_h,
    }
}

/// Output size for `frame` under `opts`, in pixels.
///
/// The border is padded uniformly on all four sides, so a bordered raster is
/// not itself exactly 4:3 — the picture inside it is.
pub fn output_size(frame: &IndexedFrame, opts: &PresentOptions) -> (usize, usize) {
    let r = raster(frame, opts);
    (r.width, r.height)
}

/// Rasterise `frame` to RGBA: per-scanline palette, nearest-neighbour scale,
/// 4:3 or square pixels, and an optional overscan border.
///
/// `out` is resized to [`output_size`] and every pixel in it is written, so a
/// caller may pass a fresh `FrameBuffer::new(0, 0)`.
pub fn present(frame: &IndexedFrame, opts: &PresentOptions, out: &mut FrameBuffer) {
    let r = raster(frame, opts);
    if out.width != r.width || out.height != r.height {
        *out = FrameBuffer::new(r.width, r.height);
    }

    // One 256-entry RGBA table per palette in force, rebuilt when the source
    // scanline changes. `Palette::write_rgba` is where the 6-bit DAC values are
    // widened, so the presenter widens them exactly the way an upload does.
    let mut table = [0u8; 1024];

    // The border is drawn in the palette in force at the top of the frame.
    if let Some(index) = frame.border.filter(|_| r.pad > 0) {
        frame.palettes.palette_at(0).write_rgba(&mut table);
        let at = index as usize * 4;
        let quad = [table[at], table[at + 1], table[at + 2], table[at + 3]];
        for pixel in out.rgba.chunks_exact_mut(4) {
            pixel.copy_from_slice(&quad);
        }
    }

    if r.inner_w == 0 || r.inner_h == 0 {
        return;
    }

    let src_w = frame.width as usize;
    let src_h = frame.height as usize;
    let mut table_row = usize::MAX; // the source scanline `table` holds
    for y in 0..r.inner_h {
        let src_y = y * src_h / r.inner_h;
        if src_y != table_row {
            // src_y < frame.height, which is a u16.
            frame.palettes.palette_at(src_y as u16).write_rgba(&mut table);
            table_row = src_y;
        }
        let dst_row = (y + r.pad) * r.width + r.pad;
        for x in 0..r.inner_w {
            let src_x = x * src_w / r.inner_w;
            let index = frame.pixels[src_y * src_w + src_x] as usize;
            let src = index * 4;
            let dst = (dst_row + x) * 4;
            out.rgba[dst..dst + 4].copy_from_slice(&table[src..src + 4]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::palette::{Palette, PaletteTimeline};
    use rstest::*;

    /// 2x2, row 0 = [1, 2], row 1 = [3, 1].
    /// Scanline 0 uses the base palette; scanline 1 uses one where entry 1 is blue.
    fn two_row_frame() -> IndexedFrame {
        let mut f = IndexedFrame::new(2, 2);
        f.pixels.copy_from_slice(&[1, 2, 3, 1]);

        let mut base = Palette::BLACK;
        base.set_entry(1, [63, 0, 0]); // red
        base.set_entry(2, [0, 63, 0]); // green

        let mut lower = Palette::BLACK;
        lower.set_entry(1, [0, 0, 63]); // blue

        f.palettes = PaletteTimeline::new(base);
        f.palettes.push(1, lower);
        f
    }

    #[test]
    fn square_pixels_scale_is_exact() {
        let f = IndexedFrame::new(320, 200);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: false };
        assert_eq!(output_size(&f, &opts), (640, 400));
    }

    #[rstest]
    #[case(320, 200)]
    #[case(320, 400)]
    #[case(640, 400)]
    fn four_three_output_is_exactly_four_by_three(#[case] w: u16, #[case] h: u16) {
        let f = IndexedFrame::new(w, h);
        let opts = PresentOptions::default();
        let (ow, oh) = output_size(&f, &opts);
        // The 6/5 horizontal stretch must land on whole pixels. It does for
        // every geometry the demo uses; a width that is not a multiple of 5
        // would truncate, which is why the implementation asserts on it.
        assert_eq!(ow, w as usize * 6 / 5, "{w} is not a multiple of 5");
        assert_eq!(ow * 3, oh * 4, "{w}x{h} -> {ow}x{oh}");
    }

    #[test]
    fn four_three_scales_uniformly() {
        let f = IndexedFrame::new(320, 200);
        let opts = PresentOptions { aspect: Aspect::FourThree, scale: 2, border: false };
        assert_eq!(output_size(&f, &opts), (768, 576));
    }

    #[test]
    fn per_scanline_palettes_are_used() {
        let f = two_row_frame();
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 1, border: false };
        present(&f, &opts, &mut out);
        // Row 0 reads the base palette, where entry 2 is green.
        assert_eq!(out.pixel(1, 0), [0, 255, 0, 255]);
        // Row 1 reads the pushed palette, where entry 1 is blue rather than red.
        assert_eq!(out.pixel(1, 1), [0, 0, 255, 255]);
    }

    #[test]
    fn nearest_neighbour_replicates_each_source_pixel() {
        let f = two_row_frame();
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 3, border: false };
        present(&f, &opts, &mut out);
        for y in 0..3 { for x in 0..3 { assert_eq!(out.pixel(x, y), [255, 0, 0, 255]); } }
        assert_eq!(out.pixel(3, 0), [0, 255, 0, 255]);
    }

    #[test]
    fn the_border_pads_the_picture_by_eight_source_pixels_scaled() {
        let mut f = two_row_frame();
        f.border = Some(9);
        let mut pal = *f.palettes.base();
        pal.set_entry(9, [63, 63, 0]);
        f.palettes.reset(pal);
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: true };
        present(&f, &opts, &mut out);
        assert_eq!(out.width(), 2 * 2 + 2 * 16);
        assert_eq!(out.pixel(0, 0), [255, 255, 0, 255]);
        assert_eq!(out.pixel(16, 16), [255, 0, 0, 255]);
    }

    #[test]
    fn the_border_is_omitted_when_disabled_or_unset() {
        let mut f = two_row_frame();
        f.border = Some(9);
        let opts = PresentOptions { aspect: Aspect::SquarePixels, scale: 1, border: false };
        let mut out = FrameBuffer::new(0, 0);
        present(&f, &opts, &mut out);
        assert_eq!(out.width(), 2);
    }
}
