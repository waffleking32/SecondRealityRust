//! Loop mode (`MAIN/U2.ASM:914-933`, D42) and `/gravis`.

use super::super::*;
use super::restart::assert_handed_over;
use crate::shell::plan::PART_ORDER;
use crate::shell::run_plan;
use crate::shell::test_support::{colours, music_run, Stub};

/// Every demo name builds a part that draws nothing and never retires (the tests walk the
/// plan with Ctrl), except CRED, which fills the screen and retires on its second tick.
fn loop_source(name: &str) -> Option<Box<dyn Part>> {
    let (name, _) = PART_ORDER.iter().find(|(part, _)| *part == name)?;
    let retires = (*name == "cred").then_some(1);
    Some(Box::new(Stub { name, fill: retires.map(|_| 7), retires }))
}

/// A run of `block`'s plan under `looping`, with the real players.
fn loop_run(block: char, looping: Looping) -> Sequence {
    let settings = RunSettings { ending: Ending::Exit, looping, ..RunSettings::default() };
    music_run(run_plan(block, looping).unwrap(), loop_source, settings)
}

/// Tick `run`, pressing Ctrl after every tick that leaves a part other than CRED live, until
/// `until` holds after a tick. Returns the parts started on the way, and every player
/// install as the part current at it and the module it seeked.
#[allow(clippy::type_complexity)]
fn walk(
    run: &mut Sequence,
    until: impl Fn(&Sequence) -> bool,
) -> (Vec<&'static str>, Vec<(&'static str, (ModuleId, u8))>) {
    let mut parts = Vec::new();
    let mut installs = Vec::new();
    let mut player = run.audio_player();
    let mut last = None;
    for _ in 0..10_000 {
        run.tick();
        let now = run.audio_player();
        let fresh = match (&player, &now) {
            (Some(a), Some(b)) => !std::sync::Arc::ptr_eq(a, b),
            (None, Some(_)) => true,
            _ => false,
        };
        if fresh {
            installs.push((run.part(), run.music_module().unwrap()));
        }
        player = now;
        if until(run) {
            return (parts, installs);
        }
        let in_seam = matches!(run.stage, Stage::Seam(_));
        if last != Some((run.part(), run.pass())) && run.live.is_some() && !in_seam {
            parts.push(run.part());
        }
        last = Some((run.part(), run.pass()));
        if run.part() != "cred" {
            run.skip_part();
        }
    }
    panic!("the run never got there; it drew {parts:?}");
}

#[test]
fn a_loop_pass_fades_then_restarts_music0_at_order_0_and_starts_alku() {
    let mut run = loop_run('5', Looping::On);
    assert_eq!(run.pass(), 1);
    assert_eq!(run.music_module(), Some((ModuleId::Music0, 25)), "block 5's own start");
    walk(&mut run, |run| run.part() == "cred");
    run.tick();
    assert!(run.live.is_none(), "CRED has retired");
    let picture = colours(run.frame());
    assert!(picture.iter().all(|colour| *colour != [0, 0, 0]), "CRED's picture is up");
    assert_eq!(run.music_volume(), Some(0x40));

    // `fademusic` over CRED's picture (`:926-928`).
    let pass_one = run.audio_player().unwrap();
    for tick in 1..=EXIT_FADE_TICKS {
        assert_eq!(run.tick(), SequenceStatus::Continuing);
        assert_eq!(run.part(), "cred", "fade tick {tick}");
        assert_eq!(colours(run.frame()), picture, "fade tick {tick}: the picture is held");
        assert!(!run.holding() && !run.ended(), "fade tick {tick}: a seam is not the end");
        assert_eq!(run.pass(), 1);
        assert_eq!(crate::audio::lock(&pass_one).global_volume(), (0x40 - tick) as u8);
        if tick < EXIT_FADE_TICKS {
            assert_eq!(run.music_module(), Some((ModuleId::Music0, 25)), "fade tick {tick}");
        }
    }
    assert!(crate::audio::is_faded(&crate::audio::lock(&pass_one)));
    // `restartmus` at the fade's end (`:929-932`), continuing from pass 1's player (D45).
    assert_eq!(run.music_module(), Some(LOOP_RESTART));
    assert_eq!(run.music_volume(), Some(0x40), "a reloaded module is at its own 0x40");
    assert_eq!(run.music_start_frame(), Some(run.index()), "installed at this boundary");
    assert!(
        (0..sr_audio::CHANNELS).any(|index| {
            crate::audio::lock(&pass_one).channel(index).unwrap().volume != 0
        }),
        "pass 1's MUSIC0 hands over a sounding channel"
    );
    assert_handed_over(&pass_one, &run.audio_player().unwrap());
    for tick in 1..=LOOP_RELOAD_TICKS {
        run.tick();
        assert_eq!(run.part(), "cred", "reload tick {tick}");
        assert_eq!(colours(run.frame()), picture, "reload tick {tick}");
        assert_eq!(run.music_module(), Some(LOOP_RESTART));
        assert_eq!(run.music_volume(), Some(0x40), "reload tick {tick}: no fade");
        assert_eq!(run.pass(), 1);
    }
    run.tick();
    assert_eq!(run.part(), "alku", "`jmp @@partsagain` (`:933`)");
    assert_eq!(run.pass(), 2);
    assert!(!run.holding());
}

#[test]
fn the_seam_is_not_a_part_escape_and_ctrl_leave_it_alone() {
    let mut run = loop_run('5', Looping::On);
    walk(&mut run, |run| run.part() == "cred");
    run.tick();
    let at = run.index();
    run.tick();
    assert!(!run.skip_part(), "no part to skip in the seam");
    run.request_exit();
    for _ in 0..EXIT_FADE_TICKS + LOOP_RELOAD_TICKS - 1 {
        run.tick();
        assert_eq!(run.part(), "cred");
    }
    assert_eq!(run.index(), at + EXIT_FADE_TICKS + LOOP_RELOAD_TICKS, "the seam's length");
    run.tick();
    run.tick();
    assert_eq!(run.part(), "alku", "the press did not reach ALKU");
    assert!(run.live.is_some());
}

#[test]
fn a_loop_pass_runs_every_block_whatever_the_entry() {
    // `mov cs:whattorun,07fh` (`:919`): pass 2 is block 1's plan, and 7fh has no bit 7.
    let expected = run_plan('1', Looping::On).unwrap();
    let mut run = loop_run('3', Looping::On);
    let (first, _) = walk(&mut run, |run| run.pass() == 2);
    assert_eq!(first, run_plan('3', Looping::On).unwrap(), "pass 1 is block 3's");
    assert_eq!(run.plan, expected);
    let (second, _) = walk(&mut run, |run| run.pass() == 3);
    assert_eq!(second, expected, "and pass 2 plays it");
    assert_eq!(run.plan, expected, "pass 3 is the same");

    let mut run = loop_run('U', Looping::On);
    let (first, _) = walk(&mut run, |run| run.pass() == 2);
    assert_eq!(first, ["ddstars"]);
    assert_eq!(run.plan, expected);
}

#[test]
fn starts_runs_across_passes_and_leaves_the_seam_out() {
    let mut run = loop_run('5', Looping::On);
    walk(&mut run, |run| run.pass() == 2);
    let names: Vec<&str> = run.starts().iter().map(|(_, name)| *name).collect();
    assert_eq!(names, ["endlogo", "cred", "alku"]);
    let (alku, _) = run.starts()[2];
    assert_eq!(alku, run.index() - 1, "pass 2's ALKU drew the tick that ended the walk");
    let (cred, _) = run.starts()[1];
    assert_eq!(alku - cred, 2 + EXIT_FADE_TICKS + LOOP_RELOAD_TICKS, "CRED's two ticks, the seam");
    assert_eq!(run.starts()[0], (404, "endlogo"));
}

#[test]
fn a_loop_pass_fires_only_the_full_runs_restarts() {
    // Under 7fh, `:813-818` (bit 0) and `:880-885` (bit 3) fire; `:858-863` needs bit 1
    // clear and `:900-905` bit 3 clear, so neither does.
    for block in ['3', '5'] {
        let mut run = loop_run(block, Looping::On);
        walk(&mut run, |run| run.pass() == 2);
        let (_, installs) = walk(&mut run, |run| run.pass() == 3);
        assert_eq!(
            installs,
            [("glenz", (ModuleId::Music1, 0)), ("u2e", (ModuleId::Music0, 18)), ("cred", LOOP_RESTART)],
            "block {block}"
        );
    }
}

#[test]
fn gravis_runs_once_without_the_end_scroller_and_ends() {
    // `looping` 2: ENDSCRL skipped (`:914-917`), then `@@partsdone` (`:923-924`).
    let mut run = loop_run('5', Looping::NoEndScroller);
    walk(&mut run, |run| run.part() == "cred");
    run.tick();
    for _ in 0..EXIT_FADE_TICKS + EXIT_BLANK_TICKS {
        assert!(!run.ended());
        run.tick();
        assert!(run.holding(), "the exit stage is the hold");
        assert_eq!((run.part(), run.pass()), ("cred", 1));
    }
    assert!(run.ended());
    assert_eq!(run.music_module(), Some((ModuleId::Music0, 25)), "no restart");
}

#[test]
fn looping_leaves_pass_ones_music_alone() {
    // Until the seam, a looping run's music is the plain run's at every tick: ENDLOGO for
    // 2,001 ticks, then CRED's two.
    let mut plain = loop_run('5', Looping::Off);
    let mut looping = loop_run('5', Looping::On);
    for tick in 0..=2_002 {
        plain.tick();
        looping.tick();
        assert_eq!(plain.music_position(), looping.music_position(), "tick {tick}");
        assert_eq!(plain.music_volume(), looping.music_volume(), "tick {tick}");
        assert_eq!(plain.part(), looping.part(), "tick {tick}");
        if tick == 2_000 {
            plain.skip_part();
            looping.skip_part();
        }
    }
}
