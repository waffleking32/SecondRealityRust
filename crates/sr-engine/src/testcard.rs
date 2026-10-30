//! The synthetic test card: the engine exercised with no demo asset at all.
//!
//! Five phases, one per visually significant hardware behaviour of design §2.4:
//! raster bars with a per-scanline palette, CRTC start-address scrolling, a
//! line-compare split, double-scan, and beam compose inside the overscan
//! border. Each phase lasts [`TestCard::PHASE_TICKS`] logic ticks and the card
//! runs forever. The tests below drive every phase through
//! [`Engine`](crate::Engine), so the card is also the engine's end-to-end test.

use crate::compose::band_range;
use crate::part::{Part, PartCtx, PartError, PartStatus};
use crate::surface::{Surface, MAX_WIDTH};
use crate::view::{Split, View};

/// The card's picture width, in pixels: the geometry most parts use.
const WIDTH: u16 = 320;
/// The card's picture height, in scanlines.
const HEIGHT: u16 = 200;
/// The scanline the split picture starts on, and the surface row its lower half
/// starts at.
const SPLIT_ROW: u16 = 100;
/// Rows the split window walks down before restarting; three keep it inside a
/// 400-row surface (rows 100..=201).
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

/// The test card part. Stateless: the phase is derived from `ctx.frame`.
#[derive(Default)]
pub struct TestCard;

impl TestCard {
    /// How many logic ticks each phase lasts.
    pub const PHASE_TICKS: u32 = 32;
    /// How many phases there are.
    pub const PHASE_COUNT: u32 = 5;

    pub fn new() -> TestCard {
        TestCard
    }

    /// 0 bars + per-scanline palette, 1 scroll, 2 split, 3 double-scan,
    /// 4 beam compose + border.
    fn phase(frame: u32) -> u32 {
        (frame / Self::PHASE_TICKS) % Self::PHASE_COUNT
    }
}

impl Part for TestCard {
    fn name(&self) -> &'static str {
        "testcard"
    }

    /// A 320×200 picture with the 16-colour bar palette loaded. The line stride
    /// stays unset, so the card tests the default (the surface's row stride).
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        *ctx.view = View::new(WIDTH, HEIGHT);
        for (entry, rgb) in BAR_PALETTE.iter().enumerate() {
            ctx.set_palette_entry(entry as u8, *rgb);
        }
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // The view outlives the phase, so clear what the other phases set.
        ctx.view.split = None;
        ctx.view.double_scan = false;
        ctx.view.border = None;
        ctx.view.x_offset = 0;
        ctx.view.height = HEIGHT;

        match Self::phase(ctx.frame) {
            0 => self.bars(ctx),
            1 => self.scroll(ctx),
            2 => self.split(ctx),
            3 => self.double_scan(ctx),
            _ => self.beam_compose(ctx),
        }
        PartStatus::Continuing
    }

    /// Eight bands, so a palette write made for band `n` is stamped at that
    /// band's first scanline: the raster bars and the staircase.
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
    /// pixel per tick through `x_offset` (phase 2 exercises `start_addr`).
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
            // The lower half, plus the rows the window's walk reaches below it.
            fill_rows(
                ctx.surface,
                SPLIT_ROW as usize,
                (HEIGHT + SPLIT_WALK_ROWS - 1) as usize,
                |x, _| BAR_COUNT - 1 - bar_index(x),
            );
        }
        // One row per tick, while the split scanline stays at 100.
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
        // This band's own scanlines, as the engine partitions the frame.
        let (first, last) = band_range(HEIGHT, ctx.bands, ctx.band);
        fill_rows(ctx.surface, first as usize, last as usize, |_, _| 0);
        if ctx.band == 0 {
            // Below the picture too: the tick after phase 3 still composes 400 lines.
            fill_rows(ctx.surface, HEIGHT as usize, ctx.surface.height(), |_, _| 0);
        }
        // Six-bit DAC values: the ramp stays inside 0..=63.
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

/// Write `index(x, y)` into every column of surface rows `[first, last)`,
/// clamped to the surface.
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
    use crate::compose::IndexedFrame;
    use crate::engine::Engine;
    use crate::surface::MAX_HEIGHT;
    use rstest::*;

    /// The frame of tick `phase * PHASE_TICKS + n`.
    fn frames(phase: u32, n: u32) -> IndexedFrame {
        let mut card = TestCard::new();
        let mut engine = Engine::new(MAX_WIDTH, MAX_HEIGHT, View::new(320, 200));
        engine.start(&mut card).unwrap();
        let mut frame = IndexedFrame::new(320, 200);
        for _ in 0..=phase * TestCard::PHASE_TICKS + n {
            assert_eq!(engine.tick(&mut card, &mut frame), PartStatus::Continuing);
        }
        frame
    }

    #[rstest]
    #[case(0, 2)]
    #[case(1, 16)]
    #[case(2, 8)]
    #[case(3, 1)]
    #[case(4, 9)]
    fn the_test_card_is_deterministic(#[case] phase: u32, #[case] n: u32) {
        assert_eq!(frames(phase, n), frames(phase, n), "phase {phase} tick {n} is not reproducible");
    }

    #[test]
    fn phase_bars_leaves_every_band_a_distinct_palette() {
        let f = frames(0, 2);
        let mut seen = std::collections::HashSet::new();
        for y in (0..200).step_by(25) { seen.insert(f.palette_for(y).entry(0)); }
        assert_eq!(seen.len(), 8, "expected one distinct palette per band");
    }

    #[test]
    fn phase_scroll_moves_the_view_between_ticks() {
        assert_ne!(frames(1, 0), frames(1, 1), "scrolling should change the composed frame");
    }

    /// Surface rows 0..100 hold bars and rows 100..200 the inverse; the split
    /// view starts at surface row 100.
    #[test]
    fn phase_split_reads_a_different_source_below_the_split_line() {
        let f = frames(2, 8);
        assert_eq!(f.pixel(0, 99), 0);
        assert_eq!(f.pixel(0, 100), 15);
    }

    #[test]
    fn phase_double_scan_duplicates_every_source_row() {
        let f = frames(3, 1);
        for y in 0..200u16 {
            assert_eq!(f.row(2 * y), f.row(2 * y + 1), "row {y} not duplicated");
        }
    }

    #[test]
    fn phase_beam_compose_staircases_the_palette_down_the_frame() {
        let f = frames(4, 2);
        let top = f.palette_for(0).entry(0);
        let bottom = f.palette_for(199).entry(0);
        assert!(bottom[0] > top[0], "the staircase should brighten downward");
        assert!(f.border.is_some());
    }

    /// The card never finishes: `frames` asserts `Continuing` on every tick.
    #[test]
    fn the_test_card_never_reports_finished() {
        frames(TestCard::PHASE_COUNT, 4);
    }

    /// One picture row is one surface row: a view whose stride were the view's
    /// own width (320) would put two picture rows into one surface row.
    #[test]
    fn every_source_row_reaches_its_own_pair_of_frame_rows() {
        let f = frames(3, 1);
        for y in 0..199u16 {
            assert_ne!(f.row(2 * y), f.row(2 * y + 2), "source rows {y} and {} coincide", y + 1);
        }
    }
}
