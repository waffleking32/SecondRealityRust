//! The post-PANICEND cue wait in a run (`MAIN/U2.ASM:826-853`, D38).

use crate::shell::test_support::{run, stub, stub_source, CueMusic};

#[test]
fn the_post_panicend_wait_is_between_the_part_and_the_next_one() {
    // PANICEND draws on the first tick, the five scripted cue ticks hold its picture and
    // draw nothing, and GLENZ starts when the cue turns positive.
    let mut run = run(vec!["panicend", "glenz"], stub_source);
    run.engine.set_music(Box::new(CueMusic::new([-3, -1, -32, 0, 1])));
    let start = run.index();
    run.tick();
    assert_eq!(run.part(), "panicend");
    let held = run.frame().pixels.clone();
    assert!(held.iter().all(|pixel| *pixel == 9), "PANICEND's picture is up");
    for tick in 0..5 {
        run.tick();
        assert_eq!(run.part(), "panicend", "cue tick {tick}");
        assert!(!run.holding());
        assert_eq!(run.frame().pixels, held, "cue tick {tick} drew nothing");
    }
    assert_eq!(run.index(), start + 6, "the part, then five cue ticks");
    run.tick();
    assert_eq!(run.part(), "glenz");
    assert_eq!(run.index(), start + 7);
}

#[test]
fn a_plan_that_ends_at_panicend_waits_and_then_holds() {
    let mut run = run(vec!["panicend"], stub_source);
    run.engine.set_music(Box::new(CueMusic::new([-32, -32, 1])));
    run.tick(); // PANICEND's one frame
    run.tick(); // cue: -32
    assert!(!run.holding(), "the wait comes before the hold");
    run.tick(); // cue: -32
    run.tick(); // cue: 1, the exit
    assert!(!run.holding(), "the wait retires at the end of its exit tick");
    run.tick();
    assert!(run.holding());
    assert_eq!(run.part(), "panicend");
}

#[test]
fn an_exit_on_panicends_last_tick_skips_the_cue_wait() {
    // ESC on PANICEND's final tick leaves it `Finished` with the flag set; `@@zh5` reads
    // `dis_exit` first and jumps to `@@skip5`.
    let mut run = run(vec!["panicend", "mntscrl"], |name| match name {
        "mntscrl" => stub("mntscrl", None, None),
        other => stub_source(other),
    });
    run.engine.set_music(Box::new(CueMusic::new([-32; 4])));
    assert!(run.request_exit());
    run.tick();
    assert_eq!(run.part(), "panicend");
    run.tick();
    assert_eq!(run.part(), "mntscrl", "the plan goes on");
    assert_eq!(run.index(), 2, "no cue tick between them");
    assert!(!run.holding());
}
