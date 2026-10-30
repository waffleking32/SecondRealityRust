//! The run's numbering, its hold and end, the exit drain and the keys; the restarts, the cue
//! wait and loop mode have their own files.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

use super::*;
use crate::shell::plan::{block_plan, BLOCKS};
use crate::shell::test_support::{colours, music_run, run, run_out, stub_source, try_run, Stub};

mod cue;
mod loop_mode;
mod restart;

#[test]
fn the_index_starts_at_the_anchor_and_the_frame_is_one_behind_it() {
    let mut anchored = run(vec!["beglogo"], stub_source);
    assert_eq!(anchored.index(), 404);
    assert!(anchored.index_is_reference());
    assert_eq!(anchored.frame_index(), None, "no frame has been drawn yet");
    anchored.tick();
    assert_eq!(anchored.frame().pixel(0, 0), 7, "the first tick drew");
    assert_eq!(anchored.index(), 405);
    assert_eq!(anchored.frame_index(), Some(404), "the frame it drew was 404's");

    let mut own = run(vec!["a"], stub_source);
    assert_eq!((own.index(), own.anchor()), (0, None));
    assert!(!own.index_is_reference());
    own.tick();
    assert_eq!(own.frame_index(), Some(0), "an unanchored run counts from zero");
}

#[test]
fn a_retired_part_hands_the_screen_to_the_next_without_clearing_it() {
    // One numbering for the run, and the screen as the last part left it.
    let mut run = run(vec!["a", "b"], stub_source);
    run_out(&mut run, 5);
    assert_eq!(run.part(), "b");
    assert_eq!(run.index(), 5, "two frames from a, one from b, two held");
    assert!(run.frame().pixels.iter().all(|pixel| *pixel == 7), "b cleared a's screen");
}

#[test]
fn a_run_past_its_last_part_holds_the_screen_that_part_left() {
    let mut run = run(vec!["a"], stub_source);
    assert_eq!(run_out(&mut run, 8), 8, "the run keeps ticking after a has retired");
    assert!(run.holding() && !run.finished());
    assert_eq!(run.part(), "a", "the held frames are a's screen");
    assert_eq!(run.index(), 8);
    assert!(run.frame().pixels.iter().all(|pixel| *pixel == 7));
}

#[test]
fn every_blocks_plan_runs_to_its_last_part() {
    for (block, _) in BLOCKS {
        let plan = block_plan(*block).unwrap();
        let last = *plan.last().unwrap();
        let mut run = run(plan, |name| {
            crate::parts::registered(name)
                .map(|name| Box::new(Stub { name, fill: None, retires: Some(0) }) as Box<dyn Part>)
        });
        run_out(&mut run, 64);
        assert!(run.holding(), "{block}: the plan ran out");
        assert_eq!(run.part(), last, "{block}: the last part's screen is held");
    }
}

#[test]
fn starts_records_each_plan_part_at_its_first_index_and_nothing_the_shell_runs() {
    let mut run = run(vec!["a", "b"], stub_source);
    assert_eq!(run.starts(), [(0, "a")], "the first part starts when the run is built");
    run_out(&mut run, 6);
    assert!(run.holding());
    assert_eq!(run.starts(), [(0, "a"), (2, "b")], "the hold is not a start");

    // With no player the cue wait leaves on its first tick (1), and is not a start.
    let mut run = crate::shell::test_support::run(vec!["panicend", "a"], stub_source);
    run_out(&mut run, 3);
    assert_eq!(run.part(), "a");
    assert_eq!(run.starts(), [(0, "panicend"), (2, "a")]);
}

#[test]
fn a_demo_run_fades_then_blanks_then_ends() {
    // `@@partsdone` (`MAIN/U2.ASM:935-974`): 64 ticks of fade over the last picture, 20 of
    // text mode's blank, then the end. `pam` gives the run MUSIC0 and retires at once; `a`'s
    // picture is the one held.
    let settings = RunSettings { ending: Ending::Exit, ..RunSettings::default() };
    let mut run = music_run(vec!["pam", "a"], stub_source, settings);
    for _ in 0..3 {
        assert_eq!(run.tick(), SequenceStatus::Continuing);
        assert!(!run.holding(), "the plan's parts are still playing");
    }
    let picture = colours(run.frame());
    assert!(picture.iter().all(|colour| *colour != [0, 0, 0]));
    assert_eq!(run.music_volume(), Some(0x40), "the module's own global volume");

    for tick in 1..=EXIT_FADE_TICKS {
        assert_eq!(run.tick(), SequenceStatus::Continuing);
        assert!(run.holding(), "tick {tick}: the exit stage is the hold");
        assert_eq!(colours(run.frame()), picture, "tick {tick}: the picture is held");
        assert_eq!(run.music_volume(), Some((0x40 - tick) as u8), "tick {tick}: a step a tick");
        assert!(!run.ended());
    }
    let last = EXIT_FADE_TICKS + EXIT_BLANK_TICKS;
    for tick in EXIT_FADE_TICKS + 1..=last {
        assert_eq!(run.tick(), SequenceStatus::Continuing);
        assert!(colours(run.frame()).iter().all(|colour| *colour == [0, 0, 0]), "tick {tick}");
        assert!(run.frame().pixels.iter().all(|&pixel| pixel == 0), "tick {tick}: index 0");
        assert_eq!(run.ended(), tick == last, "tick {tick}");
    }
    // An ended run is not a finished one, keeps composing the blank, and its fade stays at 0.
    assert!(!run.finished());
    assert_eq!(run.tick(), SequenceStatus::Continuing);
    assert!(run.ended());
    assert!(colours(run.frame()).iter().all(|colour| *colour == [0, 0, 0]));
    assert_eq!(run.music_volume(), Some(0));
}

#[test]
fn a_part_run_still_holds_after_its_end() {
    let mut run = run(vec!["a"], stub_source);
    run_out(&mut run, 2);
    let picture = colours(run.frame());
    assert!(picture.iter().all(|colour| *colour != [0, 0, 0]));
    run_out(&mut run, 1_000);
    assert!(run.holding() && !run.ended(), "a held run never ends");
    assert_eq!(colours(run.frame()), picture);
}

#[test]
fn a_run_of_no_parts_is_refused_rather_than_ticked() {
    let Err(error) = try_run(Vec::new(), stub_source) else {
        panic!("a run with nothing in it has no part to name");
    };
    assert!(matches!(error, ShellError::EmptyPlan), "{error:?}");
    assert_eq!(error.to_string(), "a run needs at least one part");
}

struct Fails;

impl Part for Fails {
    fn name(&self) -> &'static str { "fails" }
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        Err(PartError::new("fails", "no"))
    }
    fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
}

#[test]
fn a_part_whose_init_fails_is_reported_as_an_engine_error() {
    let Err(error) = try_run(vec!["fails"], |_| Some(Box::new(Fails))) else {
        panic!("a part whose init failed must not produce a run");
    };
    assert!(matches!(error, ShellError::Engine(_)), "{error:?}");
}

#[test]
fn a_part_that_fails_to_start_mid_run_ends_it_rather_than_being_skipped() {
    // A part that cannot start and a name the source cannot build both end the run.
    for second in ["fails", "unbuilt"] {
        let mut run = run(vec!["a", second], |name| match name {
            "fails" => Some(Box::new(Fails)),
            other => stub_source(other),
        });
        assert_eq!(run_out(&mut run, 8), 3, "{second}: a's two frames, then the failed start");
        assert!(run.finished());
        assert_eq!(run.part(), "a", "the run ends where the part failed");
        assert_eq!(run.tick(), SequenceStatus::Finished, "and stays finished");
    }
}

/// Asks to quit through its own context, as a part's `dis_exit` does.
struct Quits;

impl Part for Quits {
    fn name(&self) -> &'static str { "quits" }
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        ctx.request_exit();
        PartStatus::Continuing
    }
}

#[test]
fn a_part_that_asks_to_quit_hands_the_screen_to_the_next() {
    let mut run = run(vec!["quits", "b"], |name| match name {
        "quits" => Some(Box::new(Quits)),
        other => stub_source(other),
    });
    run_out(&mut run, 8);
    assert_eq!(run.part(), "b");
}

// --- the exit drain (`U2E.C:541-555`) ----------------------------------------------

/// Keeps drawing after the exit flag and finishes itself on its fourth tick.
struct Draining;

impl Part for Draining {
    fn name(&self) -> &'static str { "draining" }
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.frame >= 3 {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
    fn drains_after_exit(&self) -> bool { true }
}

#[test]
fn a_part_that_drains_after_exit_keeps_ticking_until_it_finishes() {
    let mut run = run(vec!["draining", "b"], |name| match name {
        "draining" => Some(Box::new(Draining)),
        other => stub_source(other),
    });
    run_out(&mut run, 2);
    assert!(run.request_exit(), "a part is live to ask");
    run.tick();
    assert_eq!(run.part(), "draining", "the exit flag does not retire a draining part");
    assert_eq!(run.frame_index(), Some(2), "and the tick drew");
    run.tick();
    assert_eq!(run.part(), "draining", "its own Finished tick drew too");
    run.tick();
    assert_eq!(run.part(), "b");
}

#[test]
fn a_part_that_does_not_drain_retires_on_the_exit_tick() {
    let mut run = run(vec!["holds", "b"], stub_source);
    run_out(&mut run, 2);
    assert!(run.request_exit());
    run.tick();
    assert_eq!(run.part(), "holds", "the exit tick still drew the part once");
    run.tick();
    assert_eq!(run.part(), "b", "the next tick has retired it");
}

// --- the keyboard (`DIS/DISINT.ASM:112-156`, `MAIN/U2.ASM:205-207`) ----------------

/// What a watching part has recorded. One per test, because tests run in parallel and a
/// [`PartSource`] is a plain `fn` that cannot capture.
struct Watched {
    frames: AtomicU32,
    asked_to_quit: AtomicBool,
}

static BY_ESCAPE: Watched = Watched { frames: AtomicU32::new(0), asked_to_quit: AtomicBool::new(false) };
static BY_CTRL: Watched = Watched { frames: AtomicU32::new(0), asked_to_quit: AtomicBool::new(false) };

/// Draws nothing, never retires, and records what it was handed.
struct Keyed {
    watched: &'static Watched,
}

impl Part for Keyed {
    fn name(&self) -> &'static str { "keyed" }
    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        self.watched.frames.fetch_add(1, Ordering::SeqCst);
        if ctx.exit_requested {
            self.watched.asked_to_quit.store(true, Ordering::SeqCst);
        }
        PartStatus::Continuing
    }
}

/// A run of `first`, a watching part, then `b`.
fn keyed_run(first: &'static str) -> Sequence {
    run(vec![first, "b"], |name| match name {
        "asked" => Some(Box::new(Keyed { watched: &BY_ESCAPE })),
        "unasked" => Some(Box::new(Keyed { watched: &BY_CTRL })),
        other => stub_source(other),
    })
}

#[test]
fn the_exit_flag_asks_the_running_part_to_quit_and_the_run_goes_on() {
    // ESC ends a part, not the demo: the part sees the flag once more and the shell runs the
    // next part.
    let mut run = keyed_run("asked");
    run_out(&mut run, 2);
    let frames = || BY_ESCAPE.frames.load(Ordering::SeqCst);
    assert_eq!(frames(), 2);
    assert!(!BY_ESCAPE.asked_to_quit.load(Ordering::SeqCst));
    assert!(run.request_exit(), "there is a running part to ask");
    run.tick();
    assert_eq!(frames(), 3, "the part runs once more");
    assert!(BY_ESCAPE.asked_to_quit.load(Ordering::SeqCst), "and is handed the flag");
    run.tick();
    assert_eq!(run.part(), "b");
    assert_eq!(run.index(), 4);
    assert_eq!(frames(), 3, "the retired part is not run again");
}

#[test]
fn ctrl_skips_the_running_part_without_running_it_again() {
    // Unlike ESC, the skipped part draws nothing more, and the next one draws at the index
    // it would have drawn at.
    let mut run = keyed_run("unasked");
    run_out(&mut run, 3);
    let frames = || BY_CTRL.frames.load(Ordering::SeqCst);
    assert_eq!(frames(), 3);
    assert!(run.skip_part(), "there is a running part to skip");
    run.tick();
    assert_eq!(frames(), 3);
    assert_eq!(run.part(), "b");
    assert_eq!(run.frame_index(), Some(3));
}

#[test]
fn neither_key_has_anything_to_act_on_once_the_run_is_holding() {
    let mut run = run(vec!["a"], stub_source);
    run_out(&mut run, 8);
    assert!(run.holding());
    assert!(!run.request_exit(), "no part is running to ask");
    assert!(!run.skip_part(), "and none to skip");
    assert_eq!(run.part(), "a");
}
