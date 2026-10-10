//! The engine: one fixed-order logic tick, and the runner that drives it.
//!
//! The tick's order is the whole point of this module — copper slot 0 runs
//! before the frame's drawing, slots 1 and 2 after it, and each band's palette
//! is stamped as that band is composed. See spec §4.2.
//!
//! Every `PartCtx` the engine builds reports `standalone = true`: the engine
//! drives one part with no demo shell around it. `exit_requested` starts clear
//! on each call — a request only lasts as long as the call that made it, until
//! a shell grows somewhere to record it.

use std::fmt;
use std::time::Duration;

use crate::compose::{band_range, compose_band, IndexedFrame};
use crate::copper::{CopperRegistry, COPPER_SLOTS};
use crate::palette::{Palette, PaletteTimeline};
use crate::part::{Part, PartCtx, PartStatus};
use crate::surface::Surface;
use crate::tick::Tick;
use crate::view::View;

// Named by the test-only helper and the test bodies at the bottom of this file.
#[cfg(test)]
use crate::{copper::CopperFn, part::PartError};

/// A part could not be started.
#[derive(Debug)]
pub enum EngineError {
    /// `part.init` returned an error. The only error type a caller sees: a
    /// `PartError` is converted here, tagged with the failing part's name.
    Run { part: &'static str, message: String },
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            EngineError::Run { part, message } => write!(f, "part {part}: {message}"),
        }
    }
}

impl std::error::Error for EngineError {}

/// The demo's hardware model: the surface, the view onto it, the live palette,
/// this frame's palette timeline, the copper slots and the logic frame counter.
pub struct Engine {
    surface: Surface,
    view: View,
    palette: Palette,
    timeline: PaletteTimeline,
    copper: CopperRegistry,
    frame_counter: u32,
}

impl Engine {
    /// A black palette, no copper callbacks, frame counter at zero.
    pub fn new(surface_width: usize, surface_height: usize, view: View) -> Engine {
        Engine {
            surface: Surface::new(surface_width, surface_height),
            view,
            palette: Palette::new(),
            timeline: PaletteTimeline::new(Palette::new()),
            copper: CopperRegistry::new(),
            frame_counter: 0,
        }
    }

    pub fn surface(&self) -> &Surface {
        &self.surface
    }

    pub fn surface_mut(&mut self) -> &mut Surface {
        &mut self.surface
    }

    pub fn view(&self) -> &View {
        &self.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.view
    }

    pub fn palette(&self) -> &Palette {
        &self.palette
    }

    /// Call `part.init`. Copper slots are cleared first.
    pub fn start(&mut self, part: &mut dyn Part) -> Result<(), EngineError> {
        let name = part.name();
        for slot in 0..COPPER_SLOTS {
            self.copper.clear(slot);
        }
        // The part's frame 0 is its own, whatever ran in this engine before.
        self.frame_counter = 0;
        let bands = part.band_count();
        let mut ctx = PartCtx::new(
            &mut self.surface,
            &mut self.view,
            &mut self.palette,
            &mut self.timeline,
            &mut self.copper,
            0,
            0,
            bands,
            true,
            false,
        );
        part.init(&mut ctx).map_err(|e| EngineError::Run {
            part: name,
            message: e.message,
        })
    }

    /// One logic tick, in the fixed order of spec §4.2:
    ///   1. copper slot 0   2. part.frame() per band, composing after each
    ///   3. copper slot 1   4. copper slot 2
    pub fn tick(&mut self, part: &mut dyn Part, out: &mut IndexedFrame) -> PartStatus {
        let bands = part.band_count();
        // `ctx.frame` is the counter as it stands at the start of the tick.
        let frame = self.frame_counter;
        self.frame_counter += 1;

        // This tick's timeline starts from the live palette; each band then
        // stamps the palette in force when that band was composed.
        self.timeline.reset(self.palette);

        // Slot 0 is the top of the screen: it runs before any drawing, so what
        // it writes is stamped at scanline 0 by the first band.
        self.run_copper(0, frame, 0, bands);

        let mut status = PartStatus::Continuing;
        for band in 0..bands {
            let band_status = {
                // A fresh context per band. Its borrows of the engine's fields
                // end with this block, which is what frees the surface for the
                // compose and the timeline for the push below.
                let mut ctx = PartCtx::new(
                    &mut self.surface,
                    &mut self.view,
                    &mut self.palette,
                    &mut self.timeline,
                    &mut self.copper,
                    frame,
                    band,
                    bands,
                    true,
                    false,
                );
                part.frame(&mut ctx)
            };
            // `Finished` is sticky: the tick still runs its remaining bands —
            // the original drew them too — but reports the part as done, so
            // the engine can retire it when the tick ends.
            if band_status == PartStatus::Finished {
                status = PartStatus::Finished;
            }
            let (first, last) = band_range(out.height, bands, band);
            compose_band(out, &self.surface, &self.view, first, last);
            self.timeline.push(first, self.palette);
        }

        // Slots 1 and 2 run below the last band — just before and during
        // retrace — so their palette writes land in the *next* tick's frame.
        let last_band = bands.saturating_sub(1);
        self.run_copper(1, frame, last_band, bands);
        self.run_copper(2, frame, last_band, bands);

        // The palette record belongs to the frame, so hand this tick's
        // timeline over: the frame keeps it, and the engine takes the frame's
        // old one to stamp afresh next tick.
        std::mem::swap(&mut out.palettes, &mut self.timeline);

        out.border = self.view.effective(out.height.saturating_sub(1)).border;
        status
    }

    /// Run copper slot `slot` with a context built from this tick's state.
    ///
    /// The callback is taken out of the registry for the duration of the call
    /// and put back afterwards: one that stayed in its slot would keep the
    /// registry borrowed while its own `PartCtx` needs to borrow the same
    /// registry.
    ///
    /// `band` is the band the slot's scanline falls in: slot 0 fires at the top
    /// of the screen, before band 0, and slots 1 and 2 fire below the last
    /// drawn scanline.
    fn run_copper(&mut self, slot: usize, frame: u32, band: u16, bands: u16) {
        let Some(mut f) = self.copper.take(slot) else {
            return;
        };
        {
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                frame,
                band,
                bands,
                true,
                false,
            );
            f(&mut ctx);
        }
        self.copper.restore(slot, f);
    }

    /// Test-only: register a copper function without going through a `PartCtx`,
    /// which the tests cannot build before `start` has run.
    #[cfg(test)]
    fn set_copper_for_test(&mut self, slot: usize, f: CopperFn) {
        self.copper.set(slot, f);
    }
}

/// Drives an `Engine` over a `Tick`, owning the output frame.
pub struct Runner {
    engine: Engine,
    part: Box<dyn Part>,
    frame: IndexedFrame,
    tick: Tick,
    part_frame: u32,
    status: PartStatus,
}

impl Runner {
    /// Build the engine, start `part`, and size the output frame to the view's
    /// displayed geometry — the surface may be a larger canvas.
    pub fn new(
        surface_width: usize,
        surface_height: usize,
        view: View,
        mut part: Box<dyn Part>,
    ) -> Result<Runner, EngineError> {
        let frame = IndexedFrame::new(view.width, view.height);
        let mut engine = Engine::new(surface_width, surface_height, view);
        engine.start(&mut *part)?;
        Ok(Runner {
            engine,
            part,
            frame,
            tick: Tick::new(),
            part_frame: 0,
            status: PartStatus::Continuing,
        })
    }

    /// One logic tick.
    pub fn step(&mut self) -> PartStatus {
        if self.status == PartStatus::Finished {
            return PartStatus::Finished;
        }
        self.status = self.engine.tick(&mut *self.part, &mut self.frame);
        self.part_frame += 1;
        self.status
    }

    /// Feed wall-clock time; steps once per due tick, dropping stale frames.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        if self.status == PartStatus::Finished {
            return 0;
        }
        let due = self.tick.advance(elapsed);
        let mut steps = 0;
        while steps < due && self.status == PartStatus::Continuing {
            self.step();
            steps += 1;
        }
        steps
    }

    /// The last presented frame.
    pub fn frame(&self) -> &IndexedFrame {
        &self.frame
    }

    pub fn status(&self) -> PartStatus {
        self.status
    }

    /// Logic ticks run since the part started.
    pub fn part_frame(&self) -> u32 {
        self.part_frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default, Clone)]
    struct Log(Rc<RefCell<Vec<String>>>);

    #[derive(Default)]
    struct ProbePart { log: Log, bands: u16, finish_after: Option<u32> }

    /// Whole-frame part that sets one palette entry and otherwise does nothing.
    struct Flat;

    impl Part for Flat {
        fn name(&self) -> &'static str { "flat" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            ctx.set_palette_entry(0, [40, 0, 0]);
            PartStatus::Continuing
        }
    }

    /// Draws nothing and sets no palette; used with copper callbacks that do.
    struct Noop;

    impl Part for Noop {
        fn name(&self) -> &'static str { "noop" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
    }

    /// Reports `Finished` from band 0 of the tick whose `ctx.frame` reaches `n`.
    struct FinishesAt(u32);

    impl Part for FinishesAt {
        fn name(&self) -> &'static str { "finishes-at" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            if ctx.frame >= self.0 { PartStatus::Finished } else { PartStatus::Continuing }
        }
    }

    impl Part for ProbePart {
        fn name(&self) -> &'static str { "probe" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            self.log.0.borrow_mut().push(format!("frame:{}:{}", ctx.frame, ctx.band));
            if let Some(n) = self.finish_after {
                // Reports Finished from the *first* band, so the assertion that
                // every band still ran actually proves something.
                if ctx.frame >= n && ctx.band == 0 { return PartStatus::Finished; }
            }
            PartStatus::Continuing
        }
        fn band_count(&self) -> u16 { self.bands }
    }

    fn copper_logging(log: &Log, slot: usize) -> CopperFn {
        let log = log.clone();
        Box::new(move |_ctx| log.0.borrow_mut().push(format!("copper:{slot}")))
    }

    fn engine_with(part: &mut dyn Part) -> (Engine, IndexedFrame) {
        let mut e = Engine::new(8, 4, View::new(8, 4));
        e.start(part).unwrap();
        (e, IndexedFrame::new(8, 4))
    }

    #[test]
    fn copper_slots_and_frame_calls_run_in_the_fixed_order() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 2, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(0, copper_logging(&log, 0));
        e.set_copper_for_test(1, copper_logging(&log, 1));
        e.set_copper_for_test(2, copper_logging(&log, 2));
        e.tick(&mut part, &mut f);
        assert_eq!(
            log.0.borrow().as_slice(),
            ["copper:0", "frame:0:0", "frame:0:1", "copper:1", "copper:2"]
        );
    }

    #[test]
    fn frame_counter_advances_once_per_tick_not_once_per_band() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 4, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        e.tick(&mut part, &mut f);
        assert_eq!(
            log.0.borrow().as_slice(),
            ["frame:0:0", "frame:0:1", "frame:0:2", "frame:0:3",
             "frame:1:0", "frame:1:1", "frame:1:2", "frame:1:3"]
        );
    }

    #[test]
    fn a_finished_band_still_lets_the_tick_finish_its_bands() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 3, finish_after: Some(0), ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        assert_eq!(e.tick(&mut part, &mut f), PartStatus::Finished);
        assert_eq!(log.0.borrow().len(), 3);
    }

    #[test]
    fn a_palette_set_during_a_band_applies_from_that_band() {
        struct Ramp;
        impl Part for Ramp {
            fn name(&self) -> &'static str { "ramp" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                ctx.set_palette_entry(0, [(ctx.band as u8) * 8, 0, 0]);
                PartStatus::Continuing
            }
            fn band_count(&self) -> u16 { 4 }
        }
        let mut part = Ramp;
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        // Four bands over 4 scanlines: one palette change per band.
        assert_eq!(f.palette_for(0).entry(0), [0, 0, 0]);
        assert_eq!(f.palette_for(1).entry(0), [8, 0, 0]);
        assert_eq!(f.palette_for(2).entry(0), [16, 0, 0]);
        assert_eq!(f.palette_for(3).entry(0), [24, 0, 0]);
    }

    #[test]
    fn a_whole_frame_part_stamps_one_palette_for_the_whole_frame() {
        let mut part = Flat;
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        assert_eq!(f.palettes.change_count(), 1);
        assert_eq!(f.palette_for(0).entry(0), [40, 0, 0]);
        assert_eq!(f.palette_for(3).entry(0), [40, 0, 0]);
    }

    #[test]
    fn copper_slot_zero_sees_its_palette_change_on_the_same_frame() {
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(0, Box::new(|ctx: &mut PartCtx| ctx.set_palette_entry(1, [5, 0, 0])));
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(0).entry(1), [5, 0, 0]);
    }

    #[test]
    fn copper_slot_two_runs_after_the_frame_and_affects_the_next_one() {
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(2, Box::new(|ctx: &mut PartCtx| ctx.set_palette_entry(2, [9, 0, 0])));
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(3).entry(2), [0, 0, 0]);
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(0).entry(2), [9, 0, 0]);
    }

    #[test]
    fn copper_slot_zero_can_record_before_the_part_draws() {
        // Guards the original's use of slot 0 to poll the music mixer: the
        // slot must run before any of the tick's drawing work.
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 1, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.set_copper_for_test(0, copper_logging(&log, 0));
        e.tick(&mut part, &mut f);
        assert_eq!(log.0.borrow()[0], "copper:0");
    }

    #[test]
    fn runner_presents_the_frame_after_the_tick() {
        let mut r = Runner::new(8, 4, View::new(8, 4), Box::new(Flat)).unwrap();
        r.step();
        assert_eq!(r.frame().width, 8);
        assert_eq!(r.part_frame(), 1);
    }

    #[test]
    fn runner_advance_runs_one_step_per_due_tick() {
        let mut r = Runner::new(8, 4, View::new(8, 4), Box::new(Flat)).unwrap();
        // `advance` clamps at `Tick::MAX_ELAPSED`, so a one-second call is a
        // 250 ms call: 17 ticks, not 70.
        assert_eq!(r.advance(Duration::from_secs(1)), 17);
        assert_eq!(r.part_frame(), 17);
    }

    #[test]
    fn runner_stops_stepping_after_the_part_finishes() {
        let mut r = Runner::new(8, 4, View::new(8, 4), Box::new(FinishesAt(2))).unwrap();
        r.step(); r.step();
        assert_eq!(r.status(), PartStatus::Continuing);
        assert_eq!(r.step(), PartStatus::Finished);
        let frames_before = r.part_frame();
        assert_eq!(r.advance(Duration::from_secs(1)), 0, "a finished part must not keep ticking");
        assert_eq!(r.part_frame(), frames_before);
    }
}
