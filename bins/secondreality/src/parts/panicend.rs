//! PANICEND — the fake ending: TECHNO's picture collapses to a line and a dot.
//!
//! `PANIC/SHUTDOWN.C` (158 lines) over `PANIC/ASMYT.ASM` (`copyline`) and
//! `PANIC/TWEAK.ASM` (`tw_putpixel`, `tw_getpixel`, `tw_setpalette`,
//! `tw_setrgbpalette`, `tw_setstart`): the demo's shortest part and the only
//! one that **draws nothing of its own**. It inherits TECHNO's 320×400
//! interleaved picture (plane bytes 80..159 of every 160-byte row, displayed
//! through a 640-pixel line stride) and TECHNO's DAC, and then:
//!
//! 1. **The collapse** (`SHUTDOWN.C:57-114`): row 0 blacked, the 64 fade
//!    tables built from the inherited DAC (`b = 3..767`, so entry 0 stays
//!    black), the picture's every fourth row copied into memory rows
//!    150..249, the display start set to row 100, the CRTC halved to 200
//!    lines (`09h = 41h`) and the line offset set to `0xA0` words (1280
//!    pixels, two memory rows), then eleven `copyline` passes that walk the
//!    band inward until only row 200 is left — the last pass and its two
//!    closing copies (`:104-105`) share one inter-wait segment, before the
//!    line loop's first wait (`:118`).
//! 2. **The line and the dot** (`:116-129`): 47 frames of eight symmetric
//!    `tw_putpixel(x, 200, 0)` writes walking in from both edges, then
//!    `tw_putpixel(160, 200, 1)`.
//! 3. **The pulse** (`:130-136`): 60 frames of `cos(a/120·6π)·31 + 32` into
//!    DAC entry 1, then `sleep(1)`, and the part is over.
//!
//! # The evidence split
//!
//! `reference/parts/panicend/frame_008792.rgb` (the collapse) and
//! `frame_008800.rgb` (the line) depend on the predecessor's screen **and**
//! DAC, so they are compared by the handoff unit test below — TECHNO's own
//! port driven to `Done` on the same `Surface` and `Palette`. `frame_009000`
//! (the dot) is self-contained: the screen is index 0 everywhere but one
//! index-1 pixel at (160, 100), and the palette's entries 0 and 1 are black
//! and `(1, 1, 1)` after the pulse, so the standalone `--part panicend` run
//! (on a blank surface and the hardware's default DAC) reproduces it exactly.
//! The process-level test `bins/secondreality/tests/panicend.rs` compares
//! 9000 and records that 8792/8800 are the handoff's.
//!
//! # The preserved artefacts
//!
//! - `getpal` (`:139-157`) writes `0d` to the DAC read index. In the
//!   assembler the tree uses a trailing `d` is **decimal**, so this loads 0,
//!   not `0x0D`: the DAC is read from entry 0 and the fade tables are built
//!   from the inherited palette unrotated. (The port's own `TWEAK.ASM:200`
//!   spells the hex value `0dh`; `cmp ..., 0d` for zero is all over the
//!   tree.) The reference frames pin it: a rotation by thirteen entries
//!   misses every pixel of 8792 and 8800.
//! - The fade tables are zero-initialised and the loop starts at `b = 3`, so
//!   DAC entry 0 is black in all 64; the `:81` white entry 0 is overwritten
//!   by `fadepals[20]` at `:84`.
//! - `copyline` moves 80 **words** = 160 planar bytes with `rep movsw` in
//!   GC write mode 1. Each 16-bit read leaves the VGA latch on the second
//!   planar byte and the 16-bit write stores that latch to **both**
//!   destination planar bytes: a copied row repeats each four-pixel group
//!   twice. That is what makes the reference's line 234 pixels wide at
//!   x = 40..276 with a three-pixel gap at 273..275; a straight byte copy is
//!   233 at 44..276.
//! - The collapse runs `a = 32; a > 2; a = a*5/6`, so its last pass is
//!   `a = 3` and rows 202/198 are the only ones the final copies clear.
//! - Nothing beyond memory row 0 is cleared, and the part never polls
//!   `dis_exit` (`SHUTDOWN.C` has no `dis_exit()` call at all).

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::surface::Surface;
use sr_engine::Palette;

const NAME: &str = "panicend";

/// One memory row in the interleaved model: 160 plane bytes, four pixels each.
const PLANE_ROW: usize = 160;
/// The surface's row in the interleaved model: 160 plane bytes × 4 pixels.
const LINE_PIXELS: usize = 640;
/// The displayed width.
const SCREEN_WIDTH: usize = 320;
/// The displayed height after `09h = 41h`.
const SCREEN_HEIGHT: u16 = 200;
/// The 400-line height the mode change leaves.
const FULL_HEIGHT: u16 = 400;
/// CRTC 0x13 before the mode change: 80 words = 160 plane bytes = 640 pixels.
const LINE_OFFSET_BEFORE: u16 = 640;
/// CRTC 0x13 after `0xa013`: 0xA0 words × 8 = 1280 pixels, two memory rows.
const LINE_OFFSET_AFTER: u16 = 0xA0 * 8;
/// `tw_setstart(100*160)`: display start in plane bytes, four pixels each.
const START_ROW_100: u32 = (100 * PLANE_ROW) as u32 * 4;
/// `copyline`'s count: 80 words.
const COPY_WORDS: usize = 80;
/// The collapse's `a` values: `a = 32; a > 2; a = a*5/6` in C integer
/// arithmetic, which ends at 3.
const COLLAPSE_A: [i32; 11] = [32, 26, 21, 17, 14, 11, 9, 7, 5, 4, 3];
/// `for(x=20;x<=160;x+=3)`: 47 iterations.
const LINE_START: i32 = 20;
const LINE_STEP: i32 = 3;
const LINE_END: i32 = 160;
const LINE_STEPS: u32 = ((LINE_END - LINE_START) / LINE_STEP) as u32 + 1;
/// `for(a=0;a<60;a++)`.
const PULSE_FRAMES: u32 = 60;

/// The tick numbers the source's straight line of waits produces. Tick 0 is
/// the copy and the first wait; ticks 1 and 2 are the mode change's two
/// waits; 3..=13 are the eleven collapse passes; 14 is the final copies and
/// the line's first iteration; the pulse starts with the dot's tick and the
/// part is over at 121 (`sleep(1)`).
const COLLAPSE_FIRST_STEP: u32 = 3;
const COLLAPSE_LAST_STEP: u32 = COLLAPSE_FIRST_STEP + COLLAPSE_A.len() as u32 - 1;
const LINE_FIRST_STEP: u32 = COLLAPSE_LAST_STEP + 1;
const LINE_SECOND_STEP: u32 = LINE_FIRST_STEP + 1;
const LINE_LAST_STEP: u32 = LINE_FIRST_STEP + LINE_STEPS - 1;
const DOT_STEP: u32 = LINE_LAST_STEP + 1;
const PULSE_FIRST_STEP: u32 = DOT_STEP + 1;
const PULSE_LAST_STEP: u32 = DOT_STEP + PULSE_FRAMES - 1;
/// The tick `sleep(1)` ends the part on. The match's catch-all retires there;
/// the tests name the number.
#[cfg(test)]
const FINISHED_STEP: u32 = PULSE_LAST_STEP + 1;

/// The part.
pub struct Panicend {
    /// The live DAC, 768 bytes, inherited from the predecessor in `init`.
    dac: [u8; 768],
    /// `fadepals[64][768]`: `(a*63 + kuvapal[b]*(64-a))/64` for `b = 3..767`.
    fadepals: Vec<u8>,
    /// The tick this part is on. The source is a straight line punctuated by
    /// `dis_waitb()`; one call is one wait, which is one frame.
    step: u32,
    /// The CRTC state this tick's writes leave: display start in pixels,
    /// displayed height, and the line offset in pixels.
    view_start: u32,
    view_height: u16,
    view_offset: u16,
}

impl Panicend {
    pub fn new() -> Panicend {
        Panicend {
            dac: [0; 768],
            fadepals: Vec::new(),
            step: 0,
            view_start: 0,
            view_height: FULL_HEIGHT,
            view_offset: LINE_OFFSET_BEFORE,
        }
    }

    /// `tw_setrgbpalette(1, b, b, b)` (`SHUTDOWN.C:134`).
    fn set_entry_one(&mut self, value: u8) {
        self.dac[3] = value;
        self.dac[4] = value;
        self.dac[5] = value;
    }
}

// ---------------------------------------------------------------- the DAC --
/// `getpal` (`SHUTDOWN.C:139-157`): the read index is `0d`, decimal zero, and
/// 768 bytes are read from it — the inherited DAC, entry for entry.
///
/// The assembler the tree uses reads a trailing `d` as **decimal**; the hex
/// spelling for 13 is `0dh`, which is what `TWEAK.ASM:200` writes for the
/// start-address register. The reference frames settle it: the fade tables
/// built from a rotation by thirteen entries miss every pixel of 8792 and
/// 8800 (see `docs/fidelity/panicend.md`).
fn getpal(dac: &[u8; 768]) -> [u8; 768] {
    *dac
}

/// `SHUTDOWN.C:62`: the 64 fade tables.
fn fade_tables(kuvapal: &[u8; 768]) -> Vec<u8> {
    let mut tables = vec![0u8; 64 * 768];
    for a in 0..64usize {
        for b in 3..768usize {
            let mixed = (a as u32 * 63 + kuvapal[b] as u32 * (64 - a) as u32) / 64;
            tables[a * 768 + b] = mixed as u8;
        }
    }
    tables
}

/// `SHUTDOWN.C:133`: `b = cos(a/120.0*3*2*π)*31.0+32`, truncated.
fn pulse_value(a: u32) -> u8 {
    let value = (a as f64 / 120.0 * 3.0 * 2.0 * std::f64::consts::PI).cos() * 31.0 + 32.0;
    value as u8
}

// ------------------------------------------------------------ the surface --
/// `tw_putpixel` (`TWEAK.ASM:77-107`): the interleaved surface's pixel `(x, y)`.
fn putpixel(surface: &mut Surface, x: i32, y: i32, colour: u8) {
    surface.pixels_mut()[y as usize * LINE_PIXELS + x as usize] = colour;
}

/// `tw_getpixel` (`TWEAK.ASM:109-140`): the interleaved surface's pixel `(x, y)`.
fn getpixel(surface: &Surface, x: i32, y: i32) -> u8 {
    surface.pixels()[y as usize * LINE_PIXELS + x as usize]
}

/// `copyline` (`ASMYT.ASM:9-24`) as the emulator runs it: 80 word moves in
/// write mode 1, each storing the latch loaded by the pair's **second**
/// planar byte to both destination planar bytes.
///
/// One planar byte is four surface pixels, so a word move writes eight
/// destination pixels: the source's second four-pixel group, twice.
fn copyline(surface: &mut Surface, from: usize, to: usize) {
    for word in 0..COPY_WORDS {
        let source = 4 * (from + 2 * word + 1);
        let mut pixel = [0u8; 4];
        pixel.copy_from_slice(&surface.pixels()[source..source + 4]);
        let dest = 4 * (to + 2 * word);
        let pixels = surface.pixels_mut();
        pixels[dest..dest + 4].copy_from_slice(&pixel);
        pixels[dest + 4..dest + 8].copy_from_slice(&pixel);
    }
}

// ---------------------------------------------------------------- the part --
impl Panicend {
    /// `SHUTDOWN.C:61`: black the first displayed row.
    fn black_row_zero(&mut self, surface: &mut Surface) {
        for x in 0..SCREEN_WIDTH as i32 {
            putpixel(surface, x, 0, 0);
        }
    }

    /// `SHUTDOWN.C:63-64`: the picture's every fourth row into memory rows
    /// 150..249.
    fn first_copy(&mut self, surface: &mut Surface) {
        for y in 0..100i32 {
            for x in 0..SCREEN_WIDTH as i32 {
                let value = getpixel(surface, x + 320, y * 4);
                putpixel(surface, x, y + 150, value);
            }
        }
    }

    /// `SHUTDOWN.C:91-102`: one `a` pass of the collapse.
    fn collapse(&mut self, surface: &mut Surface, a: i32) {
        // `for(b=a/2;b<=a;b++)`: copy row 0 (black) over the band's outer
        // rows, two memory rows apart.
        for b in (a / 2)..=a {
            let b = b as usize;
            copyline(surface, 0, 200 * PLANE_ROW - b * 320);
            copyline(surface, 0, 200 * PLANE_ROW + b * 320);
        }
        // `for(b=0;b<a;b++)`: the picture's rows, spread over 80..480, into
        // the band's inner rows.
        for b in 0..a {
            let from = 80 + (400 * b / a) as usize * PLANE_ROW;
            let to = (200 * PLANE_ROW as i32 + (b - a / 2) * 320) as usize;
            copyline(surface, from, to);
        }
    }

    /// `SHUTDOWN.C:116-127`: one iteration of the line loop.
    fn line_iteration(&mut self, surface: &mut Surface, x: i32) {
        putpixel(surface, x, 200, 0);
        putpixel(surface, 320 - x, 200, 0);
        putpixel(surface, x + 1, 200, 0);
        putpixel(surface, 319 - x, 200, 0);
        putpixel(surface, x + 2, 200, 0);
        putpixel(surface, 318 - x, 200, 0);
        putpixel(surface, x + 3, 200, 0);
        putpixel(surface, 317 - x, 200, 0);
    }

    /// Put this tick's DAC and CRTC state on the engine.
    fn present(&self, ctx: &mut PartCtx) {
        let mut palette = Palette::new();
        for index in 0..256usize {
            palette.set_entry(
                index as u8,
                [
                    self.dac[index * 3],
                    self.dac[index * 3 + 1],
                    self.dac[index * 3 + 2],
                ],
            );
        }
        ctx.set_palette(palette);
        ctx.view.width = SCREEN_WIDTH as u16;
        ctx.view.height = self.view_height;
        ctx.view.start_addr = self.view_start;
        ctx.view.x_offset = 0;
        ctx.view.y_line_offset = Some(self.view_offset);
        ctx.view.pan = 0;
        ctx.view.double_scan = false;
        ctx.view.border = Some(0);
        ctx.view.split = None;
    }
}

impl Part for Panicend {
    fn name(&self) -> &'static str {
        NAME
    }

    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        for index in 0..256usize {
            let entry = ctx.palette.entry(index as u8);
            self.dac[index * 3] = entry[0];
            self.dac[index * 3 + 1] = entry[1];
            self.dac[index * 3 + 2] = entry[2];
        }
        self.fadepals = fade_tables(&getpal(&self.dac));
        self.step = 0;
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        let step = self.step;
        self.step += 1;
        match step {
            // The blacked row, the fade tables, the first copy, and
            // `tw_setstart(100*160)`; the display is still 400 lines with a
            // 160-byte line offset.
            0 => {
                self.black_row_zero(ctx.surface);
                self.first_copy(ctx.surface);
                self.view_start = START_ROW_100;
                self.view_height = FULL_HEIGHT;
                self.view_offset = LINE_OFFSET_BEFORE;
            }
            // `09h = 41h`, `GC 05h = 41h`, map mask 0x0F, the fadepals[3]
            // upload, the white entry 0, and `tw_setstart(0)`. The line
            // offset is still 0x50 until :87.
            1 => {
                self.dac.copy_from_slice(&self.fadepals[3 * 768..4 * 768]);
                self.dac[..3].copy_from_slice(&[63, 63, 63]);
                self.view_start = 0;
                self.view_height = SCREEN_HEIGHT;
                self.view_offset = LINE_OFFSET_BEFORE;
            }
            // `tw_setpalette(fadepals[20])` and `13h = 0xa0` (0xa013h is
            // index 0x13, data 0xA0): 1280 pixels, two memory rows.
            2 => {
                self.dac.copy_from_slice(&self.fadepals[20 * 768..21 * 768]);
                self.view_offset = LINE_OFFSET_AFTER;
            }
            // The eleven `a` passes, each uploading `fadepals[63-a]`. The
            // source's two closing copies (`SHUTDOWN.C:104-105`) are in the
            // same inter-wait segment as the last pass: they run after the
            // `a = 3` copies and before the line loop's first `dis_waitb()`
            // (`:118`), so they belong to the tick that displays `a = 3`.
            s @ COLLAPSE_FIRST_STEP..=COLLAPSE_LAST_STEP => {
                let a = COLLAPSE_A[(s - COLLAPSE_FIRST_STEP) as usize];
                let at = (63 - a) as usize * 768;
                self.dac.copy_from_slice(&self.fadepals[at..at + 768]);
                self.collapse(ctx.surface, a);
                if s == COLLAPSE_LAST_STEP {
                    copyline(ctx.surface, 0, 202 * PLANE_ROW);
                    copyline(ctx.surface, 0, 198 * PLANE_ROW);
                }
            }
            // The line loop's first iteration; the GC is back in write mode 0
            // and the map mask is 1111.
            LINE_FIRST_STEP => {
                self.line_iteration(ctx.surface, LINE_START);
            }
            // The rest of the line loop.
            s @ LINE_SECOND_STEP..=LINE_LAST_STEP => {
                let x = LINE_START + LINE_STEP * (s - LINE_FIRST_STEP) as i32;
                self.line_iteration(ctx.surface, x);
            }
            // `tw_putpixel(160, 200, 1)` and the pulse's first value.
            DOT_STEP => {
                putpixel(ctx.surface, 160, 200, 1);
                self.set_entry_one(pulse_value(0));
            }
            // The rest of the pulse; the next tick is `sleep(1)` — the shell
            // holds the last frame, and the part is over.
            s @ PULSE_FIRST_STEP..=PULSE_LAST_STEP => {
                self.set_entry_one(pulse_value(s - DOT_STEP));
            }
            _ => return PartStatus::Finished,
        }
        self.present(ctx);
        PartStatus::Continuing
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::view::View;
    use sr_engine::{dac_expand, IndexedFrame};

    /// The engine's pieces, one of each, as a part sees them.
    struct Rig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
    }

    impl Rig {
        fn new(surface: Surface, view: View, palette: Palette) -> Rig {
            Rig {
                surface,
                view,
                palette,
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
            }
        }

        fn blank() -> Rig {
            Rig::new(
                Surface::new(LINE_PIXELS, FULL_HEIGHT as usize),
                View::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT),
                Palette::new(),
            )
        }

        fn tick(&mut self, part: &mut Panicend, exit_requested: bool) -> PartStatus {
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                None,
                0,
                0,
                1,
                true,
                exit_requested,
                false,
            );
            part.frame(&mut ctx)
        }

        fn init(&mut self, part: &mut Panicend) {
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                None,
                0,
                0,
                1,
                true,
                false,
                false,
            );
            part.init(&mut ctx).unwrap();
        }

        /// The displayed frame as palette indices, through the view as it
        /// stands after the tick (single band, so one palette).
        fn displayed(&self) -> Vec<u8> {
            let mut out = vec![0u8; self.view.width as usize * self.view.height as usize];
            for y in 0..self.view.height {
                for x in 0..self.view.width {
                    let at = self.view.source_index(&self.surface, x, y);
                    out[y as usize * self.view.width as usize + x as usize] =
                        self.surface.pixels()[at];
                }
            }
            out
        }
    }

    fn find(bytes: &[u8], needle: &[u8]) -> bool {
        bytes.windows(needle.len()).any(|window| window == needle)
    }

    #[test]
    fn getpal_reads_from_dac_entry_zero_because_0d_is_decimal() {
        // `PANIC/SHUTDOWN.C:148` is `mov al, 0d`: in the assembler's syntax a
        // trailing `d` is decimal, so the read index is 0. The hex spelling
        // the survey read it as is `0dh` — TWEAK's own start-address write.
        let source =
            std::fs::read("../../upstream/SecondReality/PANIC/SHUTDOWN.C").unwrap();
        assert!(
            find(&source, b"mov\tal, 0d\r\n"),
            "SHUTDOWN.C:148 is not `mov al, 0d`"
        );
        assert!(
            !find(&source, b"mov\tal, 0dh\r\n"),
            "if the source spelled the index 0dh, the read would start at 13"
        );
        let tweak = std::fs::read("../../upstream/SecondReality/PANIC/TWEAK.ASM").unwrap();
        assert!(
            find(&tweak, b"mov\tal, 0dh\r\n"),
            "TWEAK.ASM:200 is the hex spelling this file does not use"
        );

        let mut dac = [0u8; 768];
        for (i, byte) in dac.iter_mut().enumerate() {
            *byte = (i * 7 % 64) as u8;
        }
        assert_eq!(getpal(&dac), dac, "the DAC is read from entry 0, unrotated");
        // The reading the survey made — a read from entry 13 — would differ.
        let mut rotated = [0u8; 768];
        for i in 0..256 {
            let from = (i + 13) % 256;
            rotated[i * 3..i * 3 + 3].copy_from_slice(&dac[from * 3..from * 3 + 3]);
        }
        assert_ne!(getpal(&dac), rotated);
    }

    #[test]
    fn the_fade_tables_leave_entry_zero_black_and_start_at_byte_three() {
        let mut kuvapal = [0u8; 768];
        for (i, byte) in kuvapal.iter_mut().enumerate() {
            *byte = (i % 61) as u8;
        }
        let tables = fade_tables(&kuvapal);
        assert_eq!(tables.len(), 64 * 768);
        for a in 0..64u32 {
            let table = &tables[a as usize * 768..(a as usize + 1) * 768];
            assert_eq!(&table[..3], &[0, 0, 0], "a={a}: entry 0 is BSS");
            for b in 3..768usize {
                let want = ((a * 63 + kuvapal[b] as u32 * (64 - a)) / 64) as u8;
                assert_eq!(table[b], want, "a={a} b={b}");
            }
        }
        // Pinned by hand: the last uploaded table (a = 60) at entry 1 is the
        // inherited colour mixed four sixteenths of the way to white.
        let last = &tables[60 * 768..61 * 768];
        assert_eq!(last[0], 0);
        assert_eq!(last[3], ((60 * 63 + kuvapal[3] as u32 * 4) / 64) as u8);
    }

    #[test]
    fn copyline_moves_eighty_words_and_stores_the_second_byte_of_each_pair_twice() {
        let mut surface = Surface::new(LINE_PIXELS, FULL_HEIGHT as usize);
        // Planar row 10: each planar byte's four pixels are `byte ^ 0xA5`.
        for b in 0..PLANE_ROW {
            for p in 0..4 {
                surface.pixels_mut()[10 * LINE_PIXELS + 4 * b + p] =
                    (b as u8).wrapping_mul(7).wrapping_add(3) ^ 0xA5;
            }
        }
        copyline(&mut surface, 10 * PLANE_ROW, 50 * PLANE_ROW);
        for k in 0..COPY_WORDS {
            let source = 4 * (10 * PLANE_ROW + 2 * k + 1);
            let want = &surface.pixels()[source..source + 4];
            for i in 0..8usize {
                assert_eq!(
                    surface.pixels()[50 * LINE_PIXELS + 8 * k + i],
                    want[i % 4],
                    "pair {k} pixel {i}"
                );
            }
        }
        // The count is 80 words = 160 planar bytes = 640 surface pixels, not
        // the 320 a "one displayed row" reading would copy.
        assert_eq!(surface.pixels()[50 * LINE_PIXELS + 639], surface.pixels()[50 * LINE_PIXELS + 635]);
        assert_eq!(surface.pixels()[50 * LINE_PIXELS + 640], 0, "the copy stops here");
    }

    #[test]
    fn the_line_walks_in_from_both_edges_and_the_dot_is_at_one_sixty_one_hundred() {
        // A surface with a known non-zero fill makes the zero writes visible.
        let mut rig = Rig::new(
            {
                let mut surface = Surface::new(LINE_PIXELS, FULL_HEIGHT as usize);
                surface.pixels_mut().fill(7);
                surface
            },
            View::new(SCREEN_WIDTH as u16, FULL_HEIGHT),
            Palette::new(),
        );
        let mut part = Panicend::new();
        rig.init(&mut part);
        // Ticks 0..=13 are the mode change and the eleven collapse passes
        // (the last of which includes the source's two closing copies);
        // tick 14 is the line's first iteration.
        for _ in 0..=14 {
            assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        }
        let row = 200 * LINE_PIXELS;
        for x in [20, 300, 21, 299, 22, 298, 23, 297] {
            assert_eq!(rig.surface.pixels()[row + x], 0, "the first iteration's x={x}");
        }
        // The 47 iterations cover x = 20..300 on row 200.
        for _ in 15..=60 {
            assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        }
        for x in 20..=300usize {
            assert_eq!(rig.surface.pixels()[row + x], 0, "row 200, x={x}");
        }
        // Tick 61 writes the dot in colour 1; the display maps memory row 200
        // to displayed row 100 with the 1280-pixel line offset.
        assert_eq!(rig.tick(&mut part, false), PartStatus::Continuing);
        assert_eq!(rig.surface.pixels()[row + 160], 1, "the dot at source (160, 200)");
        assert_eq!((rig.view.width, rig.view.height), (320, 200));
        assert_eq!(rig.view.y_line_offset, Some(1280));
        let displayed = rig.displayed();
        assert_eq!(displayed[100 * 320 + 160], 1, "the dot is at (160, 100)");
        // The line's writes are row 200's x = 20..300; the display's row 100
        // is memory row 200, so x = 0..19 keep the surface's fill.
        assert_eq!(&displayed[100 * 320..100 * 320 + 20], &[7u8; 20]);

        // On a blank inherited surface the same run leaves exactly one
        // visible pixel: the whole picture at 9000.
        let mut blank = Rig::blank();
        let mut part = Panicend::new();
        blank.init(&mut part);
        for _ in 0..=61 {
            assert_eq!(blank.tick(&mut part, false), PartStatus::Continuing);
        }
        let displayed = blank.displayed();
        let visible: Vec<usize> = displayed
            .iter()
            .enumerate()
            .filter(|(_, value)| **value != 0)
            .map(|(at, _)| at)
            .collect();
        assert_eq!(visible, vec![100 * 320 + 160], "one index-1 dot at (160, 100)");
        assert_eq!(displayed[100 * 320 + 160], 1);
    }

    #[test]
    fn the_part_ignores_the_exit_flag_and_completes_its_finite_acts() {
        // The source never calls `dis_exit()`: the part's acts are a fixed
        // sequence of waits, and a run with no exit request gets all of them.
        let mut plain = Rig::blank();
        let mut part = Panicend::new();
        plain.init(&mut part);
        let mut finished_at = None;
        for tick in 0..200u32 {
            if plain.tick(&mut part, false) == PartStatus::Finished {
                finished_at = Some(tick);
                break;
            }
        }
        assert_eq!(
            finished_at,
            Some(FINISHED_STEP),
            "the pulse's last wait at {} then `sleep(1)`",
            PULSE_LAST_STEP
        );

        // Set the flag on every tick, from outside, as the engine hands it
        // to the part: the part does not read it and its acts are identical.
        let mut flagged = Rig::blank();
        let mut part = Panicend::new();
        flagged.init(&mut part);
        let mut also_finished_at = None;
        for tick in 0..200u32 {
            if flagged.tick(&mut part, true) == PartStatus::Finished {
                also_finished_at = Some(tick);
                break;
            }
        }
        assert_eq!(also_finished_at, Some(FINISHED_STEP));
        assert_eq!(plain.displayed(), flagged.displayed(), "the flag changes nothing");
        assert_eq!(plain.palette, flagged.palette);
        // The pulse's last value is `cos(59/120*6π)*31+32` truncated: 1, so
        // the held dot is drawn through DAC entry 1 = (1, 1, 1), and entry 0
        // is the fadepals[60] BSS zero. That is what makes 9000
        // self-contained.
        assert_eq!(plain.palette.entry(1), [1, 1, 1]);
        assert_eq!(plain.palette.entry(0), [0, 0, 0]);
    }

    #[test]
    fn the_reference_frames_are_the_collapse_and_the_line_they_describe() {
        // The numbers the note quotes, produced from the reference files.
        let collapse = std::fs::read("../../reference/parts/panicend/frame_008792.rgb").unwrap();
        let line = std::fs::read("../../reference/parts/panicend/frame_008800.rgb").unwrap();
        let row = |frame: &[u8], y: usize| -> Vec<usize> {
            (0..SCREEN_WIDTH)
                .filter(|x| {
                    let at = (y * SCREEN_WIDTH + x) * 3;
                    frame[at..at + 3].iter().any(|value| *value != 0)
                })
                .collect()
        };
        for y in 97..=103usize {
            let visible = row(&collapse, y);
            assert_eq!(visible.len(), 234, "8792 row {y}");
            assert_eq!((visible[0], *visible.last().unwrap()), (40, 276));
            for x in 273..=275usize {
                assert!(!visible.contains(&x), "8792 row {y} has a gap at {x}");
            }
        }
        for y in 0..200usize {
            let visible = row(&line, y);
            if y == 100 {
                assert_eq!(visible.len(), 234, "8800's line");
                assert_eq!((visible[0], *visible.last().unwrap()), (40, 276));
                for x in 273..=275usize {
                    assert!(!visible.contains(&x), "8800 has a gap at {x}");
                }
            } else {
                assert!(visible.is_empty(), "8800 has nothing at row {y}");
            }
        }
        let idx = std::fs::read("../../reference/parts/panicend/frame_009000.idx").unwrap();
        assert_eq!(idx.len(), SCREEN_WIDTH * SCREEN_HEIGHT as usize);
        let visible: Vec<(usize, u8)> = idx
            .iter()
            .enumerate()
            .filter(|(_, index)| **index != 0)
            .map(|(at, index)| (at, *index))
            .collect();
        assert_eq!(visible, vec![(100 * 320 + 160, 1)], "9000's dot");
    }

    #[test]
    fn the_shell_retires_the_part_at_the_end_of_the_tick_the_exit_was_set_in() {
        use crate::shell::Sequence;

        // `SHUTDOWN.C` polls no exit flag, so the shell must own the
        // retirement: `Sequence::tick` reads `Engine::exit_requested` back
        // after the band and drops the part at the end of that tick.
        let mut run = Sequence::with_source(
            LINE_PIXELS,
            FULL_HEIGHT as usize,
            View::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT),
            vec!["panicend"],
            crate::parts::new_part,
        )
        .unwrap();
        for _ in 0..3 {
            run.tick();
        }
        assert!(!run.holding());
        assert!(run.request_exit(), "a part is live to ask");
        run.tick();
        assert!(
            !run.holding(),
            "the retirement is at the end of the tick, not before it"
        );
        run.tick();
        assert!(run.holding(), "the shell retired the part, not the part itself");
    }

    #[test]
    fn the_shell_also_retires_a_part_that_requests_its_own_exit() {
        use crate::shell::Sequence;
        use sr_engine::part::Part;

        struct Stub {
            ticks: u32,
        }
        impl Part for Stub {
            fn name(&self) -> &'static str {
                "stub"
            }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
                Ok(())
            }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                self.ticks += 1;
                if self.ticks == 2 {
                    ctx.request_exit();
                }
                PartStatus::Continuing
            }
        }
        fn source(name: &str) -> Option<Box<dyn Part>> {
            if name == "panicend" {
                Some(Box::new(Stub { ticks: 0 }))
            } else {
                None
            }
        }
        let mut run = Sequence::with_source(
            LINE_PIXELS,
            FULL_HEIGHT as usize,
            View::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT),
            vec!["panicend"],
            source,
        )
        .unwrap();
        run.tick();
        assert!(!run.holding());
        run.tick(); // the stub's second tick requests the exit
        assert!(!run.holding());
        run.tick();
        assert!(run.holding(), "the shell retires either kind of exit the same way");
    }

    // --- the handoff: TECHNO's end state, then this part ---------------------

    /// Drive TECHNO's port to `Done` on a real player, then return the
    /// inherited `(surface, view, palette)`.
    fn techno_handoff() -> (Surface, View, Palette) {
        use crate::audio::{lock, test_player, PlayerMusic};
        use crate::shell::ModuleId;
        use sr_engine::music::Music;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(18);
        let mut surface = Surface::new(LINE_PIXELS, FULL_HEIGHT as usize);
        let mut view = View::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT);
        let mut palette = Palette::new();
        let mut timeline = PaletteTimeline::new(Palette::new());
        let mut copper = CopperRegistry::new();
        let mut music = PlayerMusic::new(player, 0);
        let mut techno = crate::parts::techno::Techno::at("../../");
        {
            let mut ctx = PartCtx::new(
                &mut surface,
                &mut view,
                &mut palette,
                &mut timeline,
                &mut copper,
                Some(&mut music),
                0,
                0,
                1,
                true,
                false,
                true,
            );
            techno.init(&mut ctx).unwrap();
        }
        let mut finished = false;
        for _ in 0..4200u32 {
            music.poll();
            let status = {
                let mut ctx = PartCtx::new(
                    &mut surface,
                    &mut view,
                    &mut palette,
                    &mut timeline,
                    &mut copper,
                    Some(&mut music),
                    0,
                    0,
                    1,
                    true,
                    false,
                    false,
                );
                techno.frame(&mut ctx)
            };
            if status == PartStatus::Finished {
                finished = true;
                break;
            }
        }
        assert!(finished, "TECHNO's troll reaches Done");
        (surface, view, palette)
    }

    /// One PANICEND tick in the engine's order: the frame is sized from the
    /// view as the tick starts, the part draws, and the band is composed
    /// through the view as it then stands.
    fn panicend_tick(
        surface: &mut Surface,
        view: &mut View,
        palette: &mut Palette,
        timeline: &mut PaletteTimeline,
        copper: &mut CopperRegistry,
        part: &mut Panicend,
    ) -> IndexedFrame {
        let mut frame = IndexedFrame::new(view.width, view.height);
        {
            let mut ctx = PartCtx::new(
                surface,
                view,
                palette,
                timeline,
                copper,
                None,
                0,
                0,
                1,
                true,
                false,
                false,
            );
            part.frame(&mut ctx);
        }
        let height = frame.height;
        sr_engine::compose_band(&mut frame, surface, view, 0, height);
        frame
    }

    fn rgb(frame: &IndexedFrame, palette: &Palette) -> Vec<u8> {
        let mut out = Vec::with_capacity(frame.pixels.len() * 3);
        for index in &frame.pixels {
            let entry = palette.entry(*index);
            out.push(dac_expand(entry[0]));
            out.push(dac_expand(entry[1]));
            out.push(dac_expand(entry[2]));
        }
        out
    }

    #[test]
    fn the_handoff_matches_the_collapse_8792_and_the_line_8800() {
        let (mut surface, mut view, mut palette) = techno_handoff();
        // TECHNO leaves the troll: an interleaved 320×400 picture from
        // pixel 320 of a 640-pixel line, displayed at start 320.
        assert_eq!((view.width, view.height), (320, 400));
        assert_eq!(view.start_addr, 320);
        assert_eq!(view.y_line_offset, Some(640));

        let mut timeline = PaletteTimeline::new(Palette::new());
        let mut copper = CopperRegistry::new();
        let mut part = Panicend::new();
        {
            let mut ctx = PartCtx::new(
                &mut surface,
                &mut view,
                &mut palette,
                &mut timeline,
                &mut copper,
                None,
                0,
                0,
                1,
                true,
                false,
                false,
            );
            part.init(&mut ctx).unwrap();
        }
        let mut frames = Vec::new();
        for _ in 0..20 {
            let frame = panicend_tick(
                &mut surface,
                &mut view,
                &mut palette,
                &mut timeline,
                &mut copper,
                &mut part,
            );
            // The palette in force at this tick: the collapse uploads a new
            // fade table at the start of each pass.
            frames.push(rgb(&frame, &palette));
        }

        let reference = |name: &str| {
            std::fs::read(format!("../../reference/parts/panicend/{name}")).unwrap()
        };
        let compare = |mine: &[u8], theirs: &[u8], which: &str| {
            assert_eq!(mine.len(), theirs.len(), "{which}: the frame's size");
            let mut differing = 0usize;
            let mut max_delta = 0u8;
            let mut first = None;
            for (at, (a, b)) in mine.chunks_exact(3).zip(theirs.chunks_exact(3)).enumerate() {
                let delta = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap();
                if delta != 0 {
                    differing += 1;
                    first.get_or_insert((at, a, b));
                }
                max_delta = max_delta.max(delta);
            }
            assert_eq!(
                differing, 0,
                "{which}: {differing} px differ, max delta {max_delta}, first {first:?}"
            );
        };
        // Tick 10 is the a = 7 collapse pass, with fadepals[56] uploaded in
        // the same iteration; tick 13 is the a = 3 pass and its two closing
        // copies (the source's `SHUTDOWN.C:104-105` segment), which leave
        // only row 200 — the same bytes as 8800; tick 18 is that row plus
        // five line iterations, none of which reaches x = 40.
        compare(&frames[10], &reference("frame_008792.rgb"), "8792 at tick 10");
        compare(
            &frames[13],
            &reference("frame_008800.rgb"),
            "8800 at tick 13 (the closing copies)",
        );
        compare(&frames[18], &reference("frame_008800.rgb"), "8800 at tick 18");
        // The mapping is pinned by its neighbours: tick 9 is a different
        // picture and ticks 17..19 are the held line (8799 = 8800 = 8801).
        assert_ne!(frames[9], reference("frame_008792.rgb"));
        assert_eq!(frames[17], reference("frame_008800.rgb"));
        assert_eq!(frames[19], reference("frame_008800.rgb"));
    }
}
