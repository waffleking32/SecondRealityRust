//! Presentation: the composed indexed frame as RGBA pixels a window can blit.
//!
//! Aspect, scaling and the border are **display** concerns and they live only
//! here. `xtask compare` reads the composed [`IndexedFrame`] directly, so none
//! of them can affect a fidelity measurement: they change what a monitor shows,
//! not what the engine drew. A headless dump presents with
//! [`Aspect::SourcePixels`] at 1 and no border, one output pixel a source pixel.
//!
//! **The window's picture is sized by the display, not by the frame** (D48). The
//! demo's frames are 320×200 (mode 13h, each row scanned twice), 320×400 and
//! 640×400, and a VGA shows all three on the same 640×400 raster of scanned-out
//! dots. [`Aspect::SquarePixels`] presents that raster with square dots at
//! `scale / 2`: at the default scale of 2, every one of them is 640×400 on the
//! screen, and each source pixel is a whole block of output pixels — 2×2 at
//! 320×200, 2×1 at 320×400, 1×1 at 640×400 ([`pixel_block`]). Nothing is
//! interpolated: the block is duplicated. [`Aspect::FourThree`] stretches the
//! same picture into a 4:3 box, which needs a non-integer vertical step.
//!
//! The border is an approximation rather than a modelled overscan region. The
//! original put its overscan colour in the blanking outside the active display,
//! where the monitor's sync timing fixed the width; here it is a flat pad of
//! eight 320-column pixels per side — `8 × scale` output pixels, whatever the
//! frame's geometry — around the picture, which it does not scale. That padding
//! is recorded as the first entry in `docs/fidelity/APPROXIMATIONS.md`.

use crate::compose::IndexedFrame;

/// The overscan pad, in 320-column pixels per side, before scaling.
const BORDER_PX: usize = 8;

/// The width and height of the demo's low-resolution mode, the unit `scale`
/// magnifies. A frame twice as wide or twice as tall is the same picture on the
/// display, at twice the resolution on that axis.
const MODE_WIDTH: usize = 320;
const MODE_HEIGHT: usize = 200;

/// The 4:3 box of one `scale` step: 320×200 stretched 6/5 horizontally, which
/// is what a 4:3 monitor did to it.
const FOUR_THREE_WIDTH: usize = MODE_WIDTH * 6 / 5; // 384
const FOUR_THREE_HEIGHT: usize = FOUR_THREE_WIDTH * 3 / 4; // 288

/// How the source picture is fitted to the output raster.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    /// The 6/5 horizontal stretch of a 4:3 monitor: the picture fills a 4:3
    /// box of 384×288 per `scale` step, whatever the demo mode the frame is in
    /// (a 640-column frame takes at least two steps, so it is never narrowed).
    /// The vertical step is not whole (200 rows to 288 lines a step), so this
    /// resamples — nearest-neighbour, no blending, but not pixel duplication.
    FourThree,
    /// Square display dots, 16:10, every source pixel a whole block of output
    /// pixels ([`pixel_block`]): at `scale` 2, 320×200 is exactly 2×2
    /// duplication into 640×400, and 320×400 and 640×400 frames are the same
    /// 640×400 on the screen. The window's default.
    SquarePixels,
    /// `scale`×`scale` output pixels per source pixel, whatever the frame's
    /// mode, so a 640×400 frame is twice the size of a 320×200 one. What a
    /// headless dump uses (at 1, one output pixel a source pixel); not a window
    /// option.
    SourcePixels,
}

/// Everything the presenter needs beyond the frame itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentOptions {
    pub aspect: Aspect,
    /// Integer nearest-neighbour magnification of a 320×200 picture; at least 1.
    pub scale: u32,
    /// Draw the frame's overscan colour as a border, when it has one.
    pub border: bool,
}

impl PresentOptions {
    /// The window's presentation: square dots at 2×, so a 320×200 frame is
    /// duplicated into 640×400 with no interpolation, and the border on (D48).
    pub const WINDOW: PresentOptions =
        PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: true };
}

impl Default for PresentOptions {
    fn default() -> Self {
        PresentOptions::WINDOW
    }
}

/// How many display dots of the 640×400 raster one source pixel covers along an
/// axis: 2 for the 320-column (or 200-row) mode, 1 for the doubled one, and
/// `None` for a length the demo's modes do not have.
fn dots(len: usize, mode: usize) -> Option<usize> {
    if len == mode {
        Some(2)
    } else if len == 2 * mode {
        Some(1)
    } else {
        None
    }
}

/// The block of output pixels — `(columns, rows)` — one source pixel of a
/// `width`×`height` frame becomes under [`Aspect::SquarePixels`] at `scale`.
///
/// Both are whole numbers and at least 1, so the presenter duplicates and never
/// blends. For the demo's geometries the block is the display's: a dot is
/// `scale / 2` output pixels, so at an even scale 320×200 gets `scale`×`scale`,
/// 320×400 gets `scale`×`scale/2` and 640×400 `scale/2`×`scale/2`, and all three
/// are `320·scale`×`200·scale` on the screen. At an odd scale a doubled axis
/// cannot be halved into whole pixels, so such a frame takes the largest whole
/// dot size that is not bigger than the request, `max(1, scale / 2)` — at
/// `--scale 1` a 640×400 frame stays 640×400 rather than losing every other
/// pixel, and at 3 it is 640×400 inside 960×600 neighbours. A frame of any other
/// geometry (the settings screen's 520×325, a test's) is `scale`×`scale`.
pub fn pixel_block(width: u16, height: u16, scale: u32) -> (usize, usize) {
    let scale = scale.max(1) as usize;
    match (dots(width as usize, MODE_WIDTH), dots(height as usize, MODE_HEIGHT)) {
        (Some(dx), Some(dy)) => {
            if (dx * scale) % 2 == 0 && (dy * scale) % 2 == 0 {
                (dx * scale / 2, dy * scale / 2)
            } else {
                let dot = (scale / 2).max(1);
                (dx * dot, dy * dot)
            }
        }
        _ => (scale, scale),
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
    let (w, h) = (frame.width as usize, frame.height as usize);
    let (inner_w, inner_h) = match opts.aspect {
        Aspect::FourThree => {
            if dots(w, MODE_WIDTH).is_some() && dots(h, MODE_HEIGHT).is_some() {
                // One of the demo's own modes: the display's 4:3 box, the same
                // for 320×200, 320×400 and 640×400 — except that a 640-column
                // frame is never given fewer output columns than it has, so at
                // `--scale 1` it keeps the 768×576 it always had rather than
                // dropping two columns in five.
                let steps = scale.max(w / MODE_WIDTH);
                (FOUR_THREE_WIDTH * steps, FOUR_THREE_HEIGHT * steps)
            } else {
                // Any other frame is stretched 6/5 from its own width. That lands
                // on whole pixels whenever the width is a multiple of 5 — the
                // settings screen's is; a narrower width would truncate, which is
                // what the assert is there to catch.
                debug_assert!(
                    w % 5 == 0,
                    "a 4:3 box needs a source width that is a multiple of 5, and {w} is not"
                );
                let w = w * 6 * scale / 5;
                (w, w * 3 / 4)
            }
        }
        Aspect::SquarePixels => {
            let (bx, by) = pixel_block(frame.width, frame.height, opts.scale);
            (w * bx, h * by)
        }
        Aspect::SourcePixels => (w * scale, h * scale),
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
/// 4:3, square dots or source pixels, and an optional overscan border.
///
/// Every output pixel is a copy of one source pixel's colour — the one the
/// output position falls in, `x * src_w / inner_w` — and no two are ever
/// mixed. Where the picture is a whole multiple of the frame (square dots and
/// source pixels, always) that is exact duplication of a block.
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
    #[case(320, 200, 1, (384, 288))]
    #[case(320, 400, 1, (384, 288))]
    // A 640-column frame is never narrowed: at 1 it keeps the 768×576 it had.
    #[case(640, 400, 1, (768, 576))]
    #[case(320, 200, 2, (768, 576))]
    #[case(320, 400, 2, (768, 576))]
    #[case(640, 400, 2, (768, 576))]
    // Not one of the demo's modes: stretched 6/5 from its own width.
    #[case(520, 325, 1, (624, 468))]
    fn four_three_output_is_exactly_four_by_three(
        #[case] w: u16,
        #[case] h: u16,
        #[case] scale: u32,
        #[case] expected: (usize, usize),
    ) {
        let f = IndexedFrame::new(w, h);
        let opts = PresentOptions { aspect: Aspect::FourThree, scale, border: false };
        let (ow, oh) = output_size(&f, &opts);
        assert_eq!((ow, oh), expected, "{w}x{h} at {scale}");
        assert_eq!(ow * 3, oh * 4, "{w}x{h} -> {ow}x{oh}");
    }

    /// A `w`×`h` frame in which every pixel differs from each of its four
    /// neighbours, under a palette of 256 distinct colours: a blended output
    /// pixel cannot equal any one source pixel's colour by accident.
    fn distinct_frame(w: u16, h: u16) -> IndexedFrame {
        let mut f = IndexedFrame::new(w, h);
        for y in 0..h as usize {
            for x in 0..w as usize {
                f.pixels[y * w as usize + x] = ((x + 3 * y) % 256) as u8;
            }
        }
        let mut pal = Palette::BLACK;
        for i in 0..=255u8 {
            pal.set_entry(i, [i & 63, (i >> 6) * 21, 63 - (i & 63)]);
        }
        f.palettes.reset(pal);
        f
    }

    /// The frame one output pixel a source pixel: the colours every scaled
    /// presentation must be made of.
    fn source(f: &IndexedFrame) -> FrameBuffer {
        let mut out = FrameBuffer::new(0, 0);
        let opts = PresentOptions { aspect: Aspect::SourcePixels, scale: 1, border: false };
        present(f, &opts, &mut out);
        out
    }

    /// Every output pixel of `out`'s picture, from `(pad, pad)`, is the source
    /// pixel whose `bx`×`by` block it is in — duplication, and nothing else.
    fn assert_duplicated(out: &FrameBuffer, src: &FrameBuffer, (bx, by): (usize, usize), pad: usize) {
        assert_eq!(out.width(), src.width() * bx + 2 * pad, "the picture is the frame times the block");
        assert_eq!(out.height(), src.height() * by + 2 * pad);
        for y in 0..src.height() * by {
            for x in 0..src.width() * bx {
                assert_eq!(
                    out.pixel(x + pad, y + pad),
                    src.pixel(x / bx, y / by),
                    "output ({x}, {y}) is not a copy of source ({}, {})",
                    x / bx,
                    y / by
                );
            }
        }
    }

    #[test]
    fn the_default_is_square_dots_at_two_with_the_border() {
        // The window's default (D48): exact 2×2 duplication, not the 4:3 stretch.
        assert_eq!(
            PresentOptions::default(),
            PresentOptions { aspect: Aspect::SquarePixels, scale: 2, border: true }
        );
        assert_eq!(PresentOptions::default(), PresentOptions::WINDOW);
    }

    #[test]
    fn a_320x200_frame_at_the_default_is_every_pixel_duplicated_into_640x400() {
        let f = distinct_frame(320, 200);
        let mut out = FrameBuffer::new(0, 0);
        present(&f, &PresentOptions::default(), &mut out);
        assert_eq!((out.width(), out.height()), (640, 400));
        assert_duplicated(&out, &source(&f), (2, 2), 0);
        // A 2×2 block's four pixels are one colour, and its neighbour is not:
        // the pattern is visible, so the check above could see a blend.
        assert_eq!(out.pixel(0, 0), out.pixel(1, 1));
        assert_ne!(out.pixel(1, 0), out.pixel(2, 0));
        assert_ne!(out.pixel(0, 1), out.pixel(0, 2));
    }

    #[test]
    fn the_border_pads_a_default_frame_without_scaling_its_picture() {
        let mut f = distinct_frame(320, 200);
        f.border = Some(7);
        let mut out = FrameBuffer::new(0, 0);
        present(&f, &PresentOptions::default(), &mut out);
        // Eight 320-column pixels a side, at 2: 16 output pixels of flat colour,
        // and the picture inside still exactly 2×2.
        assert_eq!((out.width(), out.height()), (640 + 32, 400 + 32));
        let src = source(&f);
        assert_duplicated(&out, &src, (2, 2), 16);
        let border = src.pixel(7, 0); // index 7 sits at (7, 0) in the pattern
        for y in 0..out.height() {
            for x in 0..out.width() {
                if !(16..656).contains(&x) || !(16..416).contains(&y) {
                    assert_eq!(out.pixel(x, y), border, "pad pixel ({x}, {y})");
                }
            }
        }
    }

    #[rstest]
    #[case(320, 200, 1, (1, 1))]
    // A halved row cannot be whole at 1, so a dot is one pixel: 320×400 is
    // 640×400, the right shape, rather than half of its rows dropped.
    #[case(320, 400, 1, (2, 1))]
    #[case(640, 400, 1, (1, 1))]
    #[case(320, 200, 2, (2, 2))]
    #[case(320, 400, 2, (2, 1))]
    #[case(640, 400, 2, (1, 1))]
    #[case(640, 200, 2, (1, 2))]
    #[case(320, 200, 3, (3, 3))]
    #[case(320, 400, 3, (2, 1))]
    #[case(640, 400, 3, (1, 1))]
    #[case(320, 200, 4, (4, 4))]
    #[case(320, 400, 4, (4, 2))]
    #[case(640, 400, 4, (2, 2))]
    #[case(520, 325, 2, (2, 2))] // the settings screen: not a demo mode, scale×scale
    #[case(2, 2, 3, (3, 3))]
    fn the_pixel_block_is_whole(
        #[case] w: u16,
        #[case] h: u16,
        #[case] scale: u32,
        #[case] block: (usize, usize),
    ) {
        assert_eq!(pixel_block(w, h, scale), block, "{w}x{h} at {scale}");
    }

    #[rstest]
    #[case(320, 200)]
    #[case(320, 400)]
    #[case(640, 400)]
    fn the_demos_modes_are_one_size_on_the_screen_at_an_even_scale(#[case] w: u16, #[case] h: u16) {
        // A 640×400 part (ENDSCRL) and a double-scanned 320×400 one (ALKU) are
        // the size their 320×200 neighbours are, so the window does not jump.
        for scale in [2, 4] {
            let opts = PresentOptions { aspect: Aspect::SquarePixels, scale, border: false };
            let s = scale as usize;
            assert_eq!(output_size(&IndexedFrame::new(w, h), &opts), (320 * s, 200 * s));
        }
    }

    #[rstest]
    #[case(320, 200)]
    #[case(320, 400)]
    #[case(640, 400)]
    fn square_dots_duplicate_every_mode_at_every_scale(#[case] w: u16, #[case] h: u16) {
        let f = distinct_frame(w, h);
        let src = source(&f);
        for scale in 1..=3 {
            let opts = PresentOptions { aspect: Aspect::SquarePixels, scale, border: false };
            let mut out = FrameBuffer::new(0, 0);
            present(&f, &opts, &mut out);
            assert_duplicated(&out, &src, pixel_block(w, h, scale), 0);
        }
    }

    #[test]
    fn a_640x400_frame_at_the_default_is_shown_one_to_one() {
        let f = distinct_frame(640, 400);
        let mut out = FrameBuffer::new(0, 0);
        present(&f, &PresentOptions::default(), &mut out);
        assert_eq!(out.rgba(), source(&f).rgba());
    }

    #[rstest]
    #[case(320, 200)]
    #[case(320, 400)]
    #[case(640, 400)]
    fn source_pixels_at_one_is_the_frame_as_composed(#[case] w: u16, #[case] h: u16) {
        // What a dump writes: one output pixel a source pixel, each the palette
        // colour of its index on its scanline.
        let f = distinct_frame(w, h);
        let out = source(&f);
        assert_eq!((out.width(), out.height()), (w as usize, h as usize));
        let mut table = [0u8; 1024];
        f.palettes.base().write_rgba(&mut table);
        let index = f.pixels[w as usize + 5] as usize; // (5, 1)
        assert_eq!(out.pixel(5, 1), [table[index * 4], table[index * 4 + 1], table[index * 4 + 2], table[index * 4 + 3]]);
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
