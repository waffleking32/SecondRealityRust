//! PANICEND — the fake ending: TECHNO's picture collapses to a line and a dot.
//!
//! `PANIC/SHUTDOWN.C` (158 lines) over `PANIC/ASMYT.ASM` (`copyline`) and
//! `PANIC/TWEAK.ASM` (`tw_putpixel`, `tw_getpixel`, `tw_setpalette`,
//! `tw_setrgbpalette`, `tw_setstart`). It draws nothing of its own: it inherits
//! TECHNO's 320×400 interleaved picture (plane bytes 80..159 of every 160-byte
//! row, through a 640-pixel line stride) and TECHNO's DAC, and then:
//!
//! 1. **The collapse** (`SHUTDOWN.C:57-114`): row 0 blacked, the 64 fade tables
//!    built, every fourth picture row copied into memory rows 150..249, the
//!    start set to row 100, the CRTC halved to 200 lines (`09h = 41h`) and the
//!    line offset set to `0xA0` words (two memory rows), then eleven
//!    `copyline` passes walking the band in to row 200. The last pass and its
//!    two closing copies (`:104-105`) share one tick, before the line loop's
//!    first wait (`:118`).
//! 2. **The line and the dot** (`:116-129`): 47 frames of eight symmetric
//!    `tw_putpixel(x, 200, 0)` writes, then `tw_putpixel(160, 200, 1)`.
//! 3. **The pulse** (`:130-136`): 60 frames of `cos(a/120·6π)·31 + 32` into DAC
//!    entry 1, then `sleep(1)`.
//!
//! **Preserved** (`bugs.md` §13): `getpal`'s `mov al, 0d` (`:148`) is decimal
//! zero, so the fade tables use the inherited DAC unrotated; the tables start
//! at `b = 3`, so entry 0 is black in all 64; `copyline` (`ASMYT.ASM:9-24`)
//! stores each word's second planar byte twice through the write-mode-1 latch;
//! `a = a*5/6` ends at 3. The part never polls `dis_exit`: the shell retires it.
//!
//! **Evidence.** 8792 and 8800 need TECHNO's screen and DAC, so the handoff test
//! here compares them; 9000 (the dot) is self-contained and a `--part` run
//! reproduces it (A63). Measurements: `docs/fidelity/panicend.md`.

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
/// waits; 3..=13 are the eleven collapse passes, the last of which also runs
/// the two closing copies; 14 is the line's first iteration; the pulse starts
/// with the dot's tick and the part is over at 121 (`sleep(1)`).
const COLLAPSE_FIRST_STEP: u32 = 3;
const COLLAPSE_LAST_STEP: u32 = COLLAPSE_FIRST_STEP + COLLAPSE_A.len() as u32 - 1;
const LINE_FIRST_STEP: u32 = COLLAPSE_LAST_STEP + 1;
const LINE_SECOND_STEP: u32 = LINE_FIRST_STEP + 1;
const LINE_LAST_STEP: u32 = LINE_FIRST_STEP + LINE_STEPS - 1;
const DOT_STEP: u32 = LINE_LAST_STEP + 1;
const PULSE_FIRST_STEP: u32 = DOT_STEP + 1;
const PULSE_LAST_STEP: u32 = DOT_STEP + PULSE_FRAMES - 1;

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
/// `SHUTDOWN.C:62`: the 64 fade tables, from `getpal`'s read of the DAC from
/// entry 0 (`:148`'s `0d` is decimal; the handoff test pins it).
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
        self.fadepals = fade_tables(&self.dac);
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
    use crate::audio::PlayerMusic;
    use crate::parts::test_rig::{reference, rgb_of, Rig};
    use sr_engine::IndexedFrame;

    /// The tick `sleep(1)` ends the part on: the match's catch-all.
    const FINISHED_STEP: u32 = PULSE_LAST_STEP + 1;

    /// A 320×200 view over the part's 640×400 surface. The part sets no
    /// copper and reads no `ctx.frame`, so a step is its `frame` alone.
    fn blank_rig() -> Rig {
        Rig::new(SCREEN_WIDTH as u16, SCREEN_HEIGHT)
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
        let mut rig = Rig::new(SCREEN_WIDTH as u16, FULL_HEIGHT);
        rig.surface.pixels_mut().fill(7);
        let mut part = Panicend::new();
        part.init(&mut rig.ctx(0)).unwrap();
        // Ticks 0..=13 are the mode change and the eleven collapse passes
        // (the last of which includes the source's two closing copies);
        // tick 14 is the line's first iteration.
        for _ in 0..=14 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        }
        let row = 200 * LINE_PIXELS;
        for x in [20, 300, 21, 299, 22, 298, 23, 297] {
            assert_eq!(rig.surface.pixels()[row + x], 0, "the first iteration's x={x}");
        }
        // The 47 iterations cover x = 20..300 on row 200.
        for _ in 15..=60 {
            assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        }
        for x in 20..=300usize {
            assert_eq!(rig.surface.pixels()[row + x], 0, "row 200, x={x}");
        }
        // Tick 61 writes the dot in colour 1; the display maps memory row 200
        // to displayed row 100 with the 1280-pixel line offset.
        assert_eq!(rig.step(&mut part), PartStatus::Continuing);
        assert_eq!(rig.surface.pixels()[row + 160], 1, "the dot at source (160, 200)");
        assert_eq!((rig.view.width, rig.view.height), (320, 200));
        assert_eq!(rig.view.y_line_offset, Some(1280));
        let displayed = rig.rendered_indices();
        assert_eq!(displayed[100 * 320 + 160], 1, "the dot is at (160, 100)");
        // The line's writes are row 200's x = 20..300; the display's row 100
        // is memory row 200, so x = 0..19 keep the surface's fill.
        assert_eq!(&displayed[100 * 320..100 * 320 + 20], &[7u8; 20]);

        // On a blank inherited surface the same run leaves exactly one
        // visible pixel: the whole picture at 9000.
        let mut blank = blank_rig();
        let mut part = Panicend::new();
        part.init(&mut blank.ctx(0)).unwrap();
        for _ in 0..=61 {
            assert_eq!(blank.step(&mut part), PartStatus::Continuing);
        }
        let displayed = blank.rendered_indices();
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
        let mut plain = blank_rig();
        let mut part = Panicend::new();
        part.init(&mut plain.ctx(0)).unwrap();
        let mut finished_at = None;
        for tick in 0..200u32 {
            if plain.step(&mut part) == PartStatus::Finished {
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
        let mut flagged = blank_rig();
        let mut part = Panicend::new();
        part.init(&mut flagged.ctx(0)).unwrap();
        let mut also_finished_at = None;
        for tick in 0..200u32 {
            if flagged.step_exit(&mut part) == PartStatus::Finished {
                also_finished_at = Some(tick);
                break;
            }
        }
        assert_eq!(also_finished_at, Some(FINISHED_STEP));
        assert_eq!(plain.rendered_indices(), flagged.rendered_indices(), "the flag changes nothing");
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
        let collapse = reference("panicend", 8792, "rgb");
        let line = reference("panicend", 8800, "rgb");
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
        let idx = reference("panicend", 9000, "idx");
        assert_eq!(idx.len(), SCREEN_WIDTH * SCREEN_HEIGHT as usize);
        let visible: Vec<(usize, u8)> = idx
            .iter()
            .enumerate()
            .filter(|(_, index)| **index != 0)
            .map(|(at, index)| (at, *index))
            .collect();
        assert_eq!(visible, vec![(100 * 320 + 160, 1)], "9000's dot");
    }

    // --- the handoff: TECHNO's end state, then this part ---------------------

    /// Drive TECHNO's port to `Done` on a real player, then return the rig it
    /// leaves, without the player: what this part inherits.
    fn techno_handoff() -> Rig {
        use crate::audio::{lock, test_player};
        use crate::shell::ModuleId;

        let player = test_player(ModuleId::Music1).unwrap();
        lock(&player).play_song(18);
        let mut rig = blank_rig().with(PlayerMusic::new(player, 0));
        let mut techno = crate::parts::techno::Techno::new();
        techno.init(&mut rig.init_ctx(0)).unwrap();
        let finished = (0..4200u32).any(|_| rig.step(&mut techno) == PartStatus::Finished);
        assert!(finished, "TECHNO's troll reaches Done");
        Rig { machine: rig.machine, music: None, tick: 0 }
    }

    /// One PANICEND tick in the engine's order: the frame is sized from the
    /// view as the tick starts, the part draws, and the band is composed
    /// through the view as it then stands.
    fn panicend_tick(rig: &mut Rig, part: &mut Panicend) -> IndexedFrame {
        let mut frame = IndexedFrame::new(rig.view.width, rig.view.height);
        part.frame(&mut rig.ctx(0));
        let height = frame.height;
        sr_engine::compose_band(&mut frame, &rig.surface, &rig.view, 0, height);
        frame
    }

    #[test]
    fn the_handoff_matches_the_collapse_8792_and_the_line_8800() {
        let mut rig = techno_handoff();
        // TECHNO leaves the troll: an interleaved 320×400 picture from
        // pixel 320 of a 640-pixel line, displayed at start 320.
        assert_eq!((rig.view.width, rig.view.height), (320, 400));
        assert_eq!(rig.view.start_addr, 320);
        assert_eq!(rig.view.y_line_offset, Some(640));

        let mut part = Panicend::new();
        part.init(&mut rig.ctx(0)).unwrap();
        let mut frames = Vec::new();
        for _ in 0..20 {
            let frame = panicend_tick(&mut rig, &mut part);
            // The palette in force at this tick: the collapse uploads a new
            // fade table at the start of each pass.
            frames.push(rgb_of(&frame.pixels, &rig.palette));
        }

        let reference = |index: u32| reference("panicend", index, "rgb");
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
        compare(&frames[10], &reference(8792), "8792 at tick 10");
        compare(
            &frames[13],
            &reference(8800),
            "8800 at tick 13 (the closing copies)",
        );
        compare(&frames[18], &reference(8800), "8800 at tick 18");
        // The mapping is pinned by its neighbours: tick 9 is a different
        // picture and ticks 17..19 are the held line (8799 = 8800 = 8801).
        assert_ne!(frames[9], reference(8792));
        assert_eq!(frames[17], reference(8800));
        assert_eq!(frames[19], reference(8800));
    }
}
