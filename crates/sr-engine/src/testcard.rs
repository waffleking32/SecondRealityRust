//! The synthetic test card: the engine exercised with no demo asset at all.
//!
//! Five phases, one per visually-significant hardware behaviour of spec §2.4 —
//! raster bars with a per-scanline palette, CRTC start-address scrolling, a
//! line-compare split, double-scan, and beam compose inside the overscan
//! border. Each phase lasts [`TestCard::PHASE_TICKS`] logic ticks and the card
//! runs forever, so it can be left on screen.
//!
//! The card is also the engine's end-to-end test: every phase has a property
//! test at the bottom of this file that drives a [`Runner`](crate::Runner), so
//! a behaviour that stops reaching the composed frame fails a test rather than
//! quietly regressing.

use crate::compose::band_range;
use crate::part::{Part, PartCtx, PartError, PartStatus};
use crate::surface::{Surface, MAX_WIDTH};
use crate::view::{Split, View};

// Named by the test helper and the test bodies at the bottom of this file.
#[cfg(test)]
use crate::{engine::Runner, surface::MAX_HEIGHT};

/// The card's picture width, in pixels: the geometry most parts use.
const WIDTH: u16 = 320;
/// The card's picture height, in scanlines.
const HEIGHT: u16 = 200;
/// The scanline the split picture starts on, and the surface row its lower half
/// starts at.
const SPLIT_ROW: u16 = 100;
/// How many rows the split window walks down before the phase restarts, and so
/// how many rows below the picture the phase fills.
///
/// The walk *was* bounded by the view's origin being `u16`: 64,000 (row 100) +
/// 2 × 640 = 65,280 was the last origin that fitted, which is why the constant
/// used to derive itself from `u16::MAX`. The origin is `u32` now (Phase 2,
/// `docs/decisions.md` D16) and no longer bounds it — following that derivation
/// through would ask for a walk of some 6.7 million rows, one surface row per
/// step, which is a hang rather than a test. What bounds the walk is the
/// surface: it only has to stay inside one, and three rows do — the window
/// reads rows 100..=201 of a 400-row surface. Three is also all the card wants
/// to exercise: a window that is demonstrably live, stepping a row per tick,
/// over rows the tests can name.
const SPLIT_WALK_ROWS: u16 = 3;
/// Bars across the picture; one palette entry each.
const BAR_COUNT: u8 = 16;
/// A bar is this many pixels wide: sixteen of them fill a 320-pixel picture.
const BAR_WIDTH: usize = 20;
/// The stripe pattern's fixed seed. A hash keeps the pattern irregular, a fixed
/// seed keeps the phase reproducible.
const STRIPE_SEED: u32 = 0x5EED_1993;
/// The palette entry the overscan border shows.
const BORDER_ENTRY: u8 = 9;
/// The border's colour: bright yellow, and visibly not part of any picture.
const BORDER_RGB: [u8; 3] = [63, 63, 0];

/// The 16-colour bar palette, at the DAC's six bits per channel.
const BAR_PALETTE: [[u8; 3]; 16] = [
    [0, 0, 0],    // black
    [0, 0, 42],   // blue
    [0, 42, 0],   // green
    [0, 42, 42],  // cyan
    [42, 0, 0],   // red
    [42, 0, 42],  // magenta
    [42, 21, 0],  // brown
    [42, 42, 42], // light grey
    [21, 21, 21], // dark grey
    [21, 21, 63], // light blue
    [21, 63, 21], // light green
    [21, 63, 63], // light cyan
    [63, 21, 21], // light red
    [63, 21, 63], // light magenta
    [63, 63, 21], // yellow
    [63, 63, 63], // white
];

/// A part that cycles through the five visually-significant hardware
/// behaviours of spec §2.4, one phase at a time, so the engine can be
/// exercised end to end with no demo assets.
pub struct TestCard {
    /// The logic tick the current call belongs to, copied out of `ctx.frame` so
    /// [`phase`](TestCard::phase) can be asked without a context.
    tick: u32,
}

impl TestCard {
    /// How many logic ticks each phase lasts.
    pub const PHASE_TICKS: u32 = 32;
    /// How many phases there are: the five behaviours of spec §2.4.
    pub const PHASE_COUNT: u32 = 5;

    /// Starts at phase 0, tick 0.
    pub fn new() -> TestCard {
        TestCard { tick: 0 }
    }

    /// 0 bars+per-scanline palette, 1 scroll, 2 split, 3 double-scan,
    /// 4 beam compose + border. Derived from the tick counter.
    pub fn phase(&self) -> u32 {
        (self.tick / Self::PHASE_TICKS) % Self::PHASE_COUNT
    }
}

impl Part for TestCard {
    fn name(&self) -> &'static str {
        "testcard"
    }

    /// A 320×200 picture with the 16-colour bar palette loaded.
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        // The phases change how the picture is scanned out, never how big it
        // is, so the view starts from a plain 320×200 again. The line stride is
        // deliberately left unset: the card draws through `Surface`, which lays
        // rows out at `MAX_WIDTH`, so that is the stride the beam must read —
        // and leaving it unset is what makes the card a live test of the
        // default rather than of an explicit value.
        *ctx.view = View::new(WIDTH, HEIGHT);
        for (entry, rgb) in BAR_PALETTE.iter().enumerate() {
            ctx.set_palette_entry(entry as u8, *rgb);
        }
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.tick = ctx.frame;

        // The view lives in the `Engine` and outlives this call, this tick and
        // this phase, so every phase clears the fields it does not own before
        // setting its own. A phase that left `split` or `double_scan` set would
        // make the next phase's test fail for the wrong reason.
        ctx.view.split = None;
        ctx.view.double_scan = false;
        ctx.view.border = None;
        // `x_offset` is phase 1's pan. The split phase's origin needs no
        // clearing here: it is set on the lower view, which `split` builds
        // afresh every tick, so it is discarded with the view that held it.
        ctx.view.x_offset = 0;
        ctx.view.height = HEIGHT;

        match self.phase() {
            0 => self.bars(ctx),
            1 => self.scroll(ctx),
            2 => self.split(ctx),
            3 => self.double_scan(ctx),
            _ => self.beam_compose(ctx),
        }
        PartStatus::Continuing
    }

    /// Eight bands, so the card always runs in beam-compose mode: the engine
    /// composes after each band, and a palette write made for band `n` is
    /// stamped at that band's first scanline. That is what makes the raster
    /// bars and the beam-compose staircase visible, and what gives the per-band
    /// palette assertion below something to read.
    fn band_count(&self) -> u16 {
        8
    }
}

impl TestCard {
    /// Phase 0 — raster bars: sixteen vertical bars, with palette entry 0
    /// re-stamped once per band, so the beam meets eight different palettes on
    /// its way down the frame.
    fn bars(&self, ctx: &mut PartCtx) {
        if ctx.band == 0 {
            fill_surface(ctx.surface, |x, _| bar_index(x));
        }
        ctx.set_palette_entry(0, [(ctx.band as u8) * 8, 0, 0]);
    }

    /// Phase 1 — CRTC start address: a fixed 1-in-8 stripe pattern, panned one
    /// pixel per tick.
    ///
    /// The pan is the scrolling, and at one pixel per tick it stays inside one
    /// 640-pixel line — so it rides in `x_offset`, which 16 bits are room
    /// enough for, rather than in `start_addr`. Phase 2's window is what
    /// exercises `start_addr`, and the card covering both fields is why this
    /// one did not move when the origin widened.
    fn scroll(&self, ctx: &mut PartCtx) {
        if ctx.band == 0 {
            fill_surface(ctx.surface, |x, _| stripe_index(x));
        }
        ctx.view.x_offset = (ctx.frame % WIDTH as u32) as u16;
    }

    /// Phase 2 — line compare: bars above scanline 100 and the inverse pattern
    /// below, read through a second view whose origin walks down the surface's
    /// lower half.
    fn split(&self, ctx: &mut PartCtx) {
        if ctx.band == 0 {
            fill_rows(ctx.surface, 0, SPLIT_ROW as usize, |x, _| bar_index(x));
            // The lower half of the picture, plus the rows the window's walk
            // reaches below it.
            fill_rows(
                ctx.surface,
                SPLIT_ROW as usize,
                (HEIGHT + SPLIT_WALK_ROWS - 1) as usize,
                |x, _| BAR_COUNT - 1 - bar_index(x),
            );
        }
        // One row per tick, so the window is live while its scanline stays at
        // 100 and the tests can name exact pixels.
        let row = SPLIT_ROW + (ctx.frame % SPLIT_WALK_ROWS as u32) as u16;
        let mut lower = View::new(WIDTH, HEIGHT);
        lower.set_origin(MAX_WIDTH as u32 * row as u32);
        ctx.view.split = Some(Box::new(Split {
            scanline: SPLIT_ROW,
            view: lower,
        }));
    }

    /// Phase 3 — double-scan: a pattern whose neighbouring rows differ, shown
    /// with every source row on two scanlines.
    fn double_scan(&self, ctx: &mut PartCtx) {
        if ctx.band == 0 {
            fill_rows(ctx.surface, 0, HEIGHT as usize, |x, y| {
                ((x / BAR_WIDTH + y) % BAR_COUNT as usize) as u8
            });
        }
        ctx.view.double_scan = true;
        ctx.view.height = HEIGHT * 2;
    }

    /// Phase 4 — beam compose: each band draws its own scanlines and stamps its
    /// own palette entry 0, so the frame becomes a staircase of eight bands
    /// inside the overscan border.
    fn beam_compose(&self, ctx: &mut PartCtx) {
        // The band's own scanlines, computed the way the engine partitions the
        // frame for this band — drawn now, so the beam composes exactly what
        // this call wrote.
        let (first, last) = band_range(HEIGHT, ctx.bands, ctx.band);
        fill_rows(ctx.surface, first as usize, last as usize, |_, _| 0);
        if ctx.band == 0 {
            // The lines below the picture too: for the one tick after phase 3
            // the frame is still 400 lines tall while this phase's view says
            // 200, and the tail would otherwise show what phase 3 left there.
            fill_rows(ctx.surface, HEIGHT as usize, ctx.surface.height(), |_, _| 0);
        }
        // 6-bit DAC values, so the ramp must stay inside 0..=63 to be monotonic
        // (band * 24 would wrap bands 3..7 back down).
        ctx.set_palette_entry(0, [8 + (ctx.band as u8) * 7, 0, 0]);
        ctx.set_palette_entry(BORDER_ENTRY, BORDER_RGB);
        ctx.view.border = Some(BORDER_ENTRY);
    }
}

/// Which of the sixteen bars column `x` falls in. The pattern repeats every
/// 320 pixels, so the 640-wide surface carries two copies of it.
fn bar_index(x: usize) -> u8 {
    ((x / BAR_WIDTH) % BAR_COUNT as usize) as u8
}

/// Phase 1's pattern: every eighth column is a bright stripe, and the rest take
/// a colour from the bar palette at a position hashed from the column and a
/// fixed seed. Constant down every column, so panning the view shifts the whole
/// pattern without tearing it.
fn stripe_index(x: usize) -> u8 {
    const STRIPE_PERIOD: usize = 8;
    const COLOURS: u32 = (BAR_COUNT - 1) as u32;
    if x % STRIPE_PERIOD == 0 {
        BAR_COUNT - 1
    } else {
        let hash = (x / STRIPE_PERIOD) as u32;
        (1 + (hash.wrapping_mul(2_654_435_761) ^ STRIPE_SEED) % COLOURS) as u8
    }
}

/// Fill every row of the surface.
fn fill_surface(surface: &mut Surface, index: impl FnMut(usize, usize) -> u8) {
    let rows = surface.height();
    fill_rows(surface, 0, rows, index);
}

/// Write `index(x, y)` into surface rows `[first, last)`, clamped to the
/// surface. Every column of each row is written, so a horizontally panned view
/// never runs off the end of the pattern.
///
/// Rows are addressed through the flat pixel run rather than `Surface::row_mut`,
/// which panics on an out-of-range row.
fn fill_rows(
    surface: &mut Surface,
    first: usize,
    last: usize,
    mut index: impl FnMut(usize, usize) -> u8,
) {
    let width = surface.width();
    let last = last.min(surface.height());
    let first = first.min(last);
    for (offset, row) in surface.pixels_mut()[first * width..last * width]
        .chunks_mut(width)
        .enumerate()
    {
        let y = first + offset;
        for (x, pixel) in row.iter_mut().enumerate() {
            *pixel = index(x, y);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::*;

    fn runner() -> Runner {
        // The view leaves its line stride unset, exactly as the card's own
        // `init` does: a 320-wide picture in a `MAX_WIDTH`-wide surface.
        Runner::new(
            MAX_WIDTH,
            MAX_HEIGHT,
            View::new(320, 200),
            Box::new(TestCard::new()),
        )
        .unwrap()
    }

    /// Runs until the part's frame counter reads `phase * PHASE_TICKS + n`.
    /// The counter is read at the *start* of a tick, so reaching tick `target`
    /// takes `target + 1` steps, and `part_frame()` then reads `target + 1`.
    fn frames(phase: u32, n: u32) -> Runner {
        let mut r = runner();
        let target = phase * TestCard::PHASE_TICKS + n;
        for _ in 0..=target { r.step(); }
        assert_eq!(r.part_frame(), target + 1);
        r
    }

    #[rstest]
    #[case(0, 2)]
    #[case(1, 16)]
    #[case(2, 8)]
    #[case(3, 1)]
    #[case(4, 9)]
    fn the_test_card_is_deterministic(#[case] phase: u32, #[case] n: u32) {
        let a = frames(phase, n).frame().clone();
        let b = frames(phase, n).frame().clone();
        assert_eq!(a, b, "phase {phase} tick {n} is not reproducible");
    }

    #[test]
    fn phase_bars_writes_more_than_one_palette_per_frame() {
        assert!(frames(0, 2).frame().palettes.change_count() > 1);
    }

    #[test]
    fn phase_bars_leaves_every_band_a_distinct_palette() {
        let r = frames(0, 2);
        let f = r.frame();
        let mut seen = std::collections::HashSet::new();
        for y in (0..200).step_by(25) { seen.insert(f.palette_for(y).entry(0)); }
        assert_eq!(seen.len(), 8, "expected one distinct palette per band");
    }

    #[test]
    fn phase_scroll_moves_the_view_between_ticks() {
        let a = frames(1, 0).frame().clone();
        let b = frames(1, 1).frame().clone();
        assert_ne!(a, b, "scrolling should change the composed frame");
    }

    #[test]
    fn phase_split_reads_a_different_source_below_the_split_line() {
        let f = frames(2, 8).frame().clone();
        // Surface rows 0..100 hold bars, rows 100..200 hold the inverse. The
        // split view's origin is surface row 100, so displayed row 100 must
        // jump to the inverse pattern.
        assert_eq!(f.pixel(0, 99), 0);
        assert_eq!(f.pixel(0, 100), 15);
    }

    #[test]
    fn phase_double_scan_duplicates_every_source_row() {
        let r = frames(3, 1);
        let f = r.frame();
        for y in 0..200u16 {
            assert_eq!(f.row(2 * y), f.row(2 * y + 1), "row {y} not duplicated");
        }
    }

    #[test]
    fn phase_beam_compose_staircases_the_palette_down_the_frame() {
        let r = frames(4, 2);
        let f = r.frame();
        let top = f.palette_for(0).entry(0);
        let bottom = f.palette_for(199).entry(0);
        assert_ne!(top, bottom, "beam compose should stamp a different palette per band");
        assert!(bottom[0] > top[0], "the staircase should brighten downward");
    }

    #[test]
    fn phase_beam_compose_sets_the_overscan_border() {
        assert!(frames(4, 2).frame().border.is_some());
    }

    #[test]
    fn the_test_card_never_reports_finished() {
        let mut r = runner();
        for _ in 0..(TestCard::PHASE_TICKS * TestCard::PHASE_COUNT + 4) {
            assert_eq!(r.step(), PartStatus::Continuing);
        }
    }

    /// The beam reads the picture at the surface's row stride, so one picture
    /// row is one surface row and not half of one.
    ///
    /// None of the tests above pin that: every other phase's pattern is
    /// constant down a column, and phase 3's row pairs come from one source row
    /// whatever the stride, so a view that set `y_line_offset = Some(320)` —
    /// the view's own width rather than the surface's — passes them all. This
    /// one does not: with the wrong stride, two picture rows share a surface
    /// row, one as its left half and one as its right half, and land on the
    /// same values.
    #[test]
    fn every_source_row_reaches_its_own_pair_of_frame_rows() {
        let f = frames(3, 1).frame().clone();
        for y in 0..199u16 {
            assert_ne!(
                f.row(2 * y),
                f.row(2 * y + 2),
                "source rows {y} and {} landed on the same picture row",
                y + 1
            );
        }
    }
}
