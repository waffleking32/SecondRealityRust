//! The engine: one fixed-order logic tick (design §4.2).
//!
//! The order is the original copper's (`MAIN/COPPER.ASM:449-468`): copper slot
//! 0, the music's poll (`call _zpollme`, `:466`), the part's bands with a
//! compose after each, then slots 1 and 2. An exit request made in any context
//! is read back out of it before it is dropped, so it lasts until the next
//! [`Engine::start`].

use crate::compose::{band_range, compose_band, IndexedFrame};
use crate::copper::COPPER_SLOTS;
use crate::machine::Machine;
use crate::music::Music;
use crate::palette::{Palette, PaletteTimeline};
use crate::part::{Part, PartError, PartStatus};
use crate::surface::Surface;
use crate::view::View;

/// The machine, this tick's palette timeline, the run's music and the part's
/// frame counter.
pub struct Engine {
    machine: Machine,
    timeline: PaletteTimeline,
    /// The run's player. It outlives the parts, as the original's music engine
    /// outlives the part EXEs that come and go around it.
    music: Option<Box<dyn Music>>,
    /// Set by `set_music`, consumed by the next `start`.
    music_fresh: bool,
    frame_counter: u32,
    exit_requested: bool,
}

impl Engine {
    /// A black palette, no copper callbacks, no music, frame counter at zero.
    pub fn new(surface_width: usize, surface_height: usize, view: View) -> Engine {
        Engine {
            machine: Machine::new(surface_width, surface_height, view),
            timeline: PaletteTimeline::new(Palette::BLACK),
            music: None,
            music_fresh: false,
            frame_counter: 0,
            exit_requested: false,
        }
    }

    pub fn surface(&self) -> &Surface {
        &self.machine.surface
    }

    pub fn surface_mut(&mut self) -> &mut Surface {
        &mut self.machine.surface
    }

    pub fn view(&self) -> &View {
        &self.machine.view
    }

    pub fn view_mut(&mut self) -> &mut View {
        &mut self.machine.view
    }

    pub fn palette(&self) -> &Palette {
        &self.machine.palette
    }

    /// The live palette. `start` leaves it alone: a part inherits the DAC the
    /// part before it left, as the original's separate EXEs do.
    pub fn palette_mut(&mut self) -> &mut Palette {
        &mut self.machine.palette
    }

    /// Whether a part, a copper callback or [`request_exit`](Engine::request_exit)
    /// has asked to quit. Sticky until the next `start`.
    pub fn exit_requested(&self) -> bool {
        self.exit_requested
    }

    /// Install the run's music, polled once a tick from then on; `start` does
    /// not touch it. The next `start`'s init context reports
    /// [`PartCtx::music_fresh`](crate::part::PartCtx::music_fresh).
    pub fn set_music(&mut self, music: Box<dyn Music>) {
        self.music = Some(music);
        self.music_fresh = true;
    }

    /// The run's music, for a reader such as the window's status line.
    pub fn music(&self) -> Option<&dyn Music> {
        self.music.as_deref()
    }

    /// Set the exit flag from outside the part: the keyboard's flag
    /// (`DIS/DISINT.ASM:11`, set by `checkkeys` at `:112`, handed to the part by
    /// `exit_2` at `:186-190`). The part's next context carries it.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }

    /// Call `part.init`, after clearing the copper slots, the frame counter and
    /// the exit flag.
    pub fn start(&mut self, part: &mut dyn Part) -> Result<(), PartError> {
        let name = part.name();
        for slot in 0..COPPER_SLOTS {
            self.machine.copper.clear(slot);
        }
        self.frame_counter = 0;
        self.exit_requested = false;
        let music_fresh = std::mem::take(&mut self.music_fresh);
        let bands = part.band_count();
        let mut ctx = self.machine.ctx(self.music.as_deref_mut(), 0, 0, bands, false, music_fresh);
        let started = part.init(&mut ctx);
        self.exit_requested |= ctx.exit_requested;
        started.map_err(|e| PartError::new(name, e.message))
    }

    /// One logic tick: copper slot 0, the music's poll, `part.frame()` per band
    /// with a compose after each, then copper slots 1 and 2.
    ///
    /// The frame follows the view's geometry as it stands when the tick begins,
    /// so a part that changes it gets a frame of the new size from the next tick.
    pub fn tick(&mut self, part: &mut dyn Part, out: &mut IndexedFrame) -> PartStatus {
        let view = &self.machine.view;
        if out.width != view.width || out.height != view.height {
            *out = IndexedFrame::new(view.width, view.height);
        }
        let bands = part.band_count();
        let frame = self.frame_counter;
        self.frame_counter += 1;

        self.timeline.reset(self.machine.palette);

        // Slot 0 runs before any drawing, so the first band stamps what it writes.
        self.run_copper(0, frame, 0, bands);

        // After slot 0 (`MAIN/COPPER.ASM:458-464`), before the frame draws.
        if let Some(music) = self.music.as_mut() {
            music.poll();
        }

        let mut status = PartStatus::Continuing;
        for band in 0..bands {
            let mut ctx =
                self.machine.ctx(self.music.as_deref_mut(), frame, band, bands, self.exit_requested, false);
            // `Finished` from any band makes the tick report `Finished`, but the
            // tick still runs the rest of its bands, as the original drew them.
            if part.frame(&mut ctx) == PartStatus::Finished {
                status = PartStatus::Finished;
            }
            self.exit_requested |= ctx.exit_requested;
            let (first, last) = band_range(out.height, bands, band);
            compose_band(out, &self.machine.surface, &self.machine.view, first, last);
            self.timeline.push(first, self.machine.palette);
        }

        // Slots 1 and 2 run below the last band, so their palette writes land in
        // the next tick's frame.
        let last_band = bands.saturating_sub(1);
        self.run_copper(1, frame, last_band, bands);
        self.run_copper(2, frame, last_band, bands);

        // The frame keeps this tick's timeline; the engine reuses the frame's old one.
        std::mem::swap(&mut out.palettes, &mut self.timeline);

        out.border = self.machine.view.effective(out.height.saturating_sub(1)).border;
        status
    }

    /// Run copper slot `slot`. The callback is out of the registry while it
    /// runs, because its own context borrows the registry.
    fn run_copper(&mut self, slot: usize, frame: u32, band: u16, bands: u16) {
        let Some(mut f) = self.machine.copper.take(slot) else {
            return;
        };
        let mut ctx =
            self.machine.ctx(self.music.as_deref_mut(), frame, band, bands, self.exit_requested, false);
        f(&mut ctx);
        let exit = ctx.exit_requested;
        self.machine.copper.set(slot, f);
        self.exit_requested |= exit;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::copper::CopperFn;
    use crate::music::MusicPosition;
    use crate::music::NoMusic;
    use crate::part::PartCtx;
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Default, Clone)]
    struct Log(Rc<RefCell<Vec<String>>>);

    impl Log {
        fn push(&self, line: impl Into<String>) {
            self.0.borrow_mut().push(line.into());
        }
    }

    #[derive(Default)]
    struct ProbePart { log: Log, bands: u16, finish_after: Option<u32> }

    impl Part for ProbePart {
        fn name(&self) -> &'static str { "probe" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            self.log.push(format!("frame:{}:{}", ctx.frame, ctx.band));
            // Finished from the *first* band, so "every band still ran" means something.
            match self.finish_after {
                Some(n) if ctx.frame >= n && ctx.band == 0 => PartStatus::Finished,
                _ => PartStatus::Continuing,
            }
        }
        fn band_count(&self) -> u16 { self.bands }
    }

    /// Records every call made on it and answers with the null readings.
    #[derive(Default)]
    struct RecordingMusic { log: Log }

    impl Music for RecordingMusic {
        fn poll(&mut self) { self.log.push("music:poll"); }
        fn position(&self) -> MusicPosition {
            self.log.push("music:position");
            MusicPosition::default()
        }
        fn musplus(&mut self) -> i32 { self.log.push("music:musplus"); -32 }
        fn musrow(&mut self) -> i32 { self.log.push("music:musrow"); 0 }
        fn muscode(&mut self, wanted: i32) -> i32 { self.log.push(format!("music:muscode:{wanted}")); 0 }
        fn sync(&mut self) -> i32 { self.log.push("music:sync"); 0 }
        fn getmframe(&mut self) -> i32 { self.log.push("music:getmframe"); 0 }
        fn setmframe(&mut self, frame: i32) { self.log.push(format!("music:setmframe:{frame}")); }
    }

    /// Asks the music for everything the six calls can hand it, or logs that it has none.
    struct AsksMusic { log: Log }

    impl Part for AsksMusic {
        fn name(&self) -> &'static str { "asks-music" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            let Some(music) = ctx.music() else {
                self.log.push("part:none");
                return PartStatus::Continuing;
            };
            let order = music.position().order;
            let plus = music.musplus();
            let row = music.musrow();
            let code = music.muscode(1);
            let frame = music.getmframe();
            music.setmframe(frame + 1);
            self.log.push(format!("part:{order}:{plus}:{row}:{code}:{frame}"));
            PartStatus::Continuing
        }
    }

    /// Draws nothing; used with copper callbacks that do.
    struct Noop;

    impl Part for Noop {
        fn name(&self) -> &'static str { "noop" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
    }

    fn copper_logging(log: &Log, slot: usize) -> CopperFn {
        let log = log.clone();
        Box::new(move |_ctx| log.push(format!("copper:{slot}")))
    }

    fn engine_with(part: &mut dyn Part) -> (Engine, IndexedFrame) {
        let mut e = Engine::new(8, 4, View::new(8, 4));
        e.start(part).unwrap();
        (e, IndexedFrame::new(8, 4))
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
        let mut part = ProbePart { log: log.clone(), bands: 3, finish_after: Some(0) };
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
        for y in 0..4u16 {
            assert_eq!(f.palette_for(y).entry(0), [y as u8 * 8, 0, 0]);
        }
    }

    #[test]
    fn a_whole_frame_part_stamps_one_palette_for_the_whole_frame() {
        struct Flat;
        impl Part for Flat {
            fn name(&self) -> &'static str { "flat" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                ctx.set_palette_entry(0, [40, 0, 0]);
                PartStatus::Continuing
            }
        }
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
        e.machine.copper.set(0, Box::new(|ctx: &mut PartCtx| ctx.set_palette_entry(1, [5, 0, 0])));
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(0).entry(1), [5, 0, 0]);
    }

    #[test]
    fn copper_slot_two_runs_after_the_frame_and_affects_the_next_one() {
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        e.machine.copper.set(2, Box::new(|ctx: &mut PartCtx| ctx.set_palette_entry(2, [9, 0, 0])));
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(3).entry(2), [0, 0, 0]);
        e.tick(&mut part, &mut f);
        assert_eq!(f.palette_for(0).entry(2), [9, 0, 0]);
    }

    /// `copper_doit` runs slot 0, then `_zpollme`, then the rest (`MAIN/COPPER.ASM:458-468`).
    #[test]
    fn the_music_is_polled_between_copper_slot_zero_and_the_first_band() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 2, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.set_music(Box::new(RecordingMusic { log: log.clone() }));
        for slot in 0..COPPER_SLOTS {
            e.machine.copper.set(slot, copper_logging(&log, slot));
        }
        e.tick(&mut part, &mut f);
        assert_eq!(
            log.0.borrow().as_slice(),
            ["copper:0", "music:poll", "frame:0:0", "frame:0:1", "copper:1", "copper:2"]
        );
    }

    /// One poll per displayed frame, however many bands (`docs/fidelity/music-clock.md` §5).
    #[test]
    fn a_tick_polls_the_music_exactly_once() {
        let log = Log::default();
        let mut part = ProbePart { log: log.clone(), bands: 4, ..Default::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.set_music(Box::new(RecordingMusic { log: log.clone() }));
        e.tick(&mut part, &mut f);
        e.tick(&mut part, &mut f);
        let polls = log.0.borrow().iter().filter(|line| line.as_str() == "music:poll").count();
        assert_eq!(polls, 2);
    }

    #[test]
    fn a_part_reaches_the_music_through_its_context() {
        let log = Log::default();
        let mut part = AsksMusic { log: log.clone() };
        let (mut e, mut f) = engine_with(&mut part);

        e.tick(&mut part, &mut f);
        assert_eq!(log.0.borrow().as_slice(), ["part:none"]);

        e.set_music(Box::new(RecordingMusic { log: log.clone() }));
        e.tick(&mut part, &mut f);
        assert_eq!(
            log.0.borrow().as_slice(),
            [
                "part:none",
                "music:poll",
                "music:position",
                "music:musplus",
                "music:musrow",
                "music:muscode:1",
                "music:getmframe",
                "music:setmframe:1",
                "part:0:-32:0:0:0",
            ]
        );
    }

    /// A part boundary is not a music boundary: `start` leaves the player alone.
    #[test]
    fn the_music_outlives_the_part_that_is_running() {
        let log = Log::default();
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        assert!(e.music().is_none());
        e.set_music(Box::new(RecordingMusic { log: log.clone() }));
        assert_eq!(e.music().map(|music| music.position()), Some(MusicPosition::default()));

        e.start(&mut part).unwrap();
        assert!(e.music().is_some());
        e.tick(&mut part, &mut f);
        assert_eq!(log.0.borrow().as_slice(), ["music:position", "music:poll"]);
    }

    #[test]
    fn the_music_is_fresh_only_for_the_first_part_after_set_music() {
        #[derive(Clone, Default)]
        struct Freshness(Rc<RefCell<Vec<bool>>>);

        struct ReportsFreshness { log: Freshness }

        impl Part for ReportsFreshness {
            fn name(&self) -> &'static str { "reports-freshness" }
            fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
                self.log.0.borrow_mut().push(ctx.music_fresh());
                Ok(())
            }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                self.log.0.borrow_mut().push(ctx.music_fresh());
                PartStatus::Continuing
            }
        }

        let log = Freshness::default();
        let mut first = ReportsFreshness { log: log.clone() };
        let mut second = ReportsFreshness { log: log.clone() };
        let mut third = ReportsFreshness { log: log.clone() };
        let mut e = Engine::new(8, 4, View::new(8, 4));
        let mut f = IndexedFrame::new(8, 4);

        e.set_music(Box::new(NoMusic));
        e.start(&mut first).unwrap();
        let copper = Freshness::default();
        for slot in 0..COPPER_SLOTS {
            let copper = copper.clone();
            e.machine.copper.set(
                slot,
                Box::new(move |ctx: &mut PartCtx| copper.0.borrow_mut().push(ctx.music_fresh())),
            );
        }
        e.tick(&mut first, &mut f);
        assert_eq!(copper.0.borrow().as_slice(), [false, false, false], "copper is never fresh");
        e.start(&mut second).unwrap();
        assert_eq!(
            log.0.borrow().as_slice(),
            [true, false, false],
            "the part started after set_music is fresh; its bands and the next part are not"
        );

        e.set_music(Box::new(NoMusic));
        e.start(&mut third).unwrap();
        assert_eq!(
            log.0.borrow().as_slice(),
            [true, false, false, true],
            "a second set_music re-arms the flag"
        );
    }

    /// The frame is the displayed area: a part that raises the height during
    /// tick 1 gets a taller frame from tick 2.
    #[test]
    fn the_frame_follows_the_view_when_a_part_changes_its_geometry() {
        struct GrowsHeight;
        impl Part for GrowsHeight {
            fn name(&self) -> &'static str { "grows-height" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                if ctx.frame >= 1 { ctx.view.height = 8; }
                PartStatus::Continuing
            }
        }
        let mut part = GrowsHeight;
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        e.tick(&mut part, &mut f);
        assert_eq!((f.width, f.height), (8, 4));
        e.tick(&mut part, &mut f);
        assert_eq!((f.width, f.height), (8, 8));
    }

    /// The contexts are per call; the exit flag is not.
    #[test]
    fn a_request_from_one_band_is_visible_to_the_next() {
        struct QuitsInBandZero { log: Log }

        impl Part for QuitsInBandZero {
            fn name(&self) -> &'static str { "quits-in-band-0" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                self.log.push(ctx.exit_requested.to_string());
                if ctx.band == 0 {
                    ctx.request_exit();
                }
                PartStatus::Continuing
            }
            fn band_count(&self) -> u16 { 2 }
        }
        let mut part = QuitsInBandZero { log: Log::default() };
        let (mut e, mut f) = engine_with(&mut part);
        assert!(!e.exit_requested());
        e.tick(&mut part, &mut f);
        assert_eq!(part.log.0.borrow().as_slice(), ["false", "true"]);
        assert!(e.exit_requested());
    }

    #[test]
    fn a_copper_callback_can_request_exit_and_the_engine_observes_it() {
        let mut part = Noop;
        let (mut e, mut f) = engine_with(&mut part);
        e.machine.copper.set(1, Box::new(|ctx: &mut PartCtx| ctx.request_exit()));
        e.tick(&mut part, &mut f);
        assert!(e.exit_requested());
    }

    #[test]
    fn a_part_can_request_exit_from_init() {
        struct QuitsInInit;
        impl Part for QuitsInInit {
            fn name(&self) -> &'static str { "quits-in-init" }
            fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
                ctx.request_exit();
                Ok(())
            }
            fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
        }
        let mut part = QuitsInInit;
        let (e, _f) = engine_with(&mut part);
        assert!(e.exit_requested());
    }

    /// The keyboard's flag (`DIS/DISINT.ASM:151-153`, `:186-190`) reaches the
    /// part's own context, in every band and every later tick, until `start`.
    #[test]
    fn a_request_from_outside_the_part_reaches_the_parts_own_context() {
        struct Reports { log: Log }

        impl Part for Reports {
            fn name(&self) -> &'static str { "reports" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
            fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
                self.log.push(format!("{}:{}", ctx.band, ctx.exit_requested));
                PartStatus::Continuing
            }
            fn band_count(&self) -> u16 { 2 }
        }
        let mut part = Reports { log: Log::default() };
        let (mut e, mut f) = engine_with(&mut part);
        e.tick(&mut part, &mut f);
        assert_eq!(part.log.0.borrow().as_slice(), ["0:false", "1:false"]);

        e.request_exit();
        assert!(e.exit_requested());
        e.tick(&mut part, &mut f);
        e.tick(&mut part, &mut f);
        assert_eq!(
            part.log.0.borrow().as_slice(),
            ["0:false", "1:false", "0:true", "1:true", "0:true", "1:true"]
        );

        e.start(&mut part).unwrap();
        e.tick(&mut part, &mut f);
        assert_eq!(part.log.0.borrow().as_slice()[6..], ["0:false", "1:false"]);
        assert!(!e.exit_requested());
    }

    #[test]
    fn a_failed_init_is_reported_under_the_parts_name() {
        struct Fails;
        impl Part for Fails {
            fn name(&self) -> &'static str { "fails" }
            fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
                Err(PartError::new("other", "no"))
            }
            fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
        }
        let mut e = Engine::new(8, 4, View::new(8, 4));
        let error = e.start(&mut Fails).unwrap_err();
        assert_eq!(error.to_string(), "part fails: no");
    }
}
