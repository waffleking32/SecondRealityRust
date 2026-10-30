//! The mid-run restarts (`MAIN/U2.ASM:880-885`, `:813-818`; D34, D45).

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;

use sr_audio::{PlayerOptions, Quality};
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};

use super::super::*;
use crate::shell::test_support::{music_run, stub, stub_source};

/// The new player continues from the old one's state: every channel the old one's with the
/// song initialiser's stores applied, and `_np_zframe` kept (D45).
pub(super) fn assert_handed_over(old: &crate::audio::Shared, new: &crate::audio::Shared) {
    assert!(!Arc::ptr_eq(old, new), "a new player, as `restartmus` loads the module again");
    let (old, new) = (crate::audio::lock(old), crate::audio::lock(new));
    for index in 0..sr_audio::CHANNELS {
        let mut expected = *old.channel(index).unwrap();
        expected.initialise();
        assert_eq!(new.channel(index), Some(&expected), "channel {index} carried across");
    }
    assert_eq!(new.zframe(), old.zframe(), "the tick counter carried across");
}

/// What U2E's `init` saw of `music_fresh` (a `static`: a [`PartSource`] cannot capture).
static FRESH_AT_RESTART: AtomicBool = AtomicBool::new(false);

/// Stands in for U2E and records its init's fresh flag.
struct FreshProbe;

impl Part for FreshProbe {
    fn name(&self) -> &'static str { "u2e" }
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        FRESH_AT_RESTART.store(ctx.music_fresh(), Ordering::SeqCst);
        Ok(())
    }
    fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
}

/// MNTSCRL retiring at once, then U2E.
fn restart_source(name: &str) -> Option<Box<dyn Part>> {
    match name {
        "mntscrl" => stub("mntscrl", None, Some(0)),
        "u2e" => Some(Box::new(FreshProbe)),
        other => stub_source(other),
    }
}

#[test]
fn a_restart_installs_the_next_modules_order_at_the_u2e_boundary() {
    FRESH_AT_RESTART.store(false, Ordering::SeqCst);
    let mut run = music_run(vec!["mntscrl", "u2e"], restart_source, RunSettings::default());
    assert_eq!(run.music_module(), Some((ModuleId::Music1, 42)), "MNTSCRL's entry row");
    assert_eq!(run.music.as_ref().unwrap().start, MusicStart { frame: 0, waiting: 0, pre: 0 });
    // The dump and the scratch are the run's and cross the restart. The marker is past one
    // poll's samples (~570), so the drain before the boundary cannot overwrite it.
    run.music.as_mut().unwrap().scratch[1000] = 700;
    let dir = std::env::temp_dir().join(format!("sr-shell-restart-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    run.dump_audio(&dir.join("music.wav")).unwrap();
    let entry = run.audio_player().unwrap();

    run.tick();
    assert_eq!(run.part(), "mntscrl");
    run.tick();
    assert_eq!(run.part(), "u2e");
    assert_eq!(run.music_module(), Some((ModuleId::Music0, 18)), "restartmus(0, 18)");
    let music = run.music.as_ref().unwrap();
    assert_eq!(music.start, MusicStart { frame: 1, waiting: 4, pre: 0 }, "D34's four-tick wait");
    assert_eq!(music.scratch[1000], 700, "the scratch is the run's");
    assert!(music.dump.is_some(), "and so is the dump");
    assert!(!Arc::ptr_eq(&music.player, &entry), "a fresh player");
    assert!(FRESH_AT_RESTART.load(Ordering::SeqCst), "U2E's init saw the fresh flag");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_restart_keeps_the_runs_sound_quality() {
    // The run's options reach the restart's player too (D43, D45).
    let quality = |run: &Sequence| crate::audio::lock(&run.audio_player().unwrap()).options();
    let enhanced = PlayerOptions { quality: Quality::Enhanced, ..PlayerOptions::default() };
    for (options, format) in [(enhanced, (48_000, 2)), (PlayerOptions::default(), (20_000, 2))] {
        let settings = RunSettings { player: options, ..RunSettings::default() };
        let mut run = music_run(vec!["mntscrl", "u2e"], restart_source, settings);
        assert_eq!(quality(&run), options, "the entry install");
        let entry = run.audio_player().unwrap();
        run.tick();
        run.tick();
        assert_eq!(run.music_module(), Some((ModuleId::Music0, 18)), "the U2E restart");
        assert!(!Arc::ptr_eq(&run.audio_player().unwrap(), &entry), "a new player");
        assert_eq!(quality(&run), options, "the restart's player");
        assert_eq!(run.audio_format(), Some(format), "and the stream it describes");
    }
}

/// How many inits the probe has seen, and how many of them were fresh.
static SAME_ORDER_INITS: AtomicU32 = AtomicU32::new(0);
static SAME_ORDER_FRESH_INITS: AtomicU32 = AtomicU32::new(0);

/// U2E drawing two frames, recording its inits.
struct SameOrderProbe;

impl Part for SameOrderProbe {
    fn name(&self) -> &'static str { "u2e" }
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
        SAME_ORDER_INITS.fetch_add(1, Ordering::SeqCst);
        if ctx.music_fresh() {
            SAME_ORDER_FRESH_INITS.fetch_add(1, Ordering::SeqCst);
        }
        Ok(())
    }
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        if ctx.frame >= 1 {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
}

#[test]
fn a_run_already_on_the_restarts_module_does_not_reinstall_it() {
    // Two U2Es: the second's boundary is the restart site, under MUSIC0@18 already. Tick 2
    // starts the second, whose init must run without a fresh flag.
    SAME_ORDER_INITS.store(0, Ordering::SeqCst);
    SAME_ORDER_FRESH_INITS.store(0, Ordering::SeqCst);
    let source = |name: &str| -> Option<Box<dyn Part>> {
        match name {
            "u2e" => Some(Box::new(SameOrderProbe)),
            other => stub_source(other),
        }
    };
    let mut run = music_run(vec!["u2e", "u2e"], source, RunSettings::default());
    assert_eq!(run.music_module(), Some((ModuleId::Music0, 18)));
    let player = run.audio_player().unwrap();
    for _ in 0..3 {
        run.tick();
        assert_eq!(run.part(), "u2e");
    }
    assert_eq!(SAME_ORDER_INITS.load(Ordering::SeqCst), 2, "the second U2E started");
    assert!(Arc::ptr_eq(&run.audio_player().unwrap(), &player), "the same player");
    assert_eq!(SAME_ORDER_FRESH_INITS.load(Ordering::SeqCst), 1, "only the entry was fresh");
}

#[test]
fn the_block_one_run_restarts_music1_at_glenz() {
    // Block 1's initial `_zplaysong(0)` (`MAIN/U2.ASM:766-775`), then `:813-818`'s
    // `restartmus(1, 0)` between BEGLOGO and GLENZ; ALKU then GLENZ is its shortest form.
    let source = |name: &str| match name {
        "alku" => stub("alku", None, Some(0)),
        other => stub_source(other),
    };
    let mut run = music_run(vec!["alku", "glenz"], source, RunSettings::default());
    assert_eq!(run.music_module(), Some((ModuleId::Music0, 0)));
    assert_eq!(run.music.as_ref().unwrap().start, MusicStart { frame: 404, waiting: 0, pre: 4 });
    let entry = run.audio_player().unwrap();
    run.tick();
    assert_eq!(run.part(), "alku");
    run.tick();
    assert_eq!(run.part(), "glenz");
    assert_eq!(run.music_module(), Some((ModuleId::Music1, 0)), "restartmus(1, 0)");
    assert_eq!(
        run.music.as_ref().unwrap().start,
        MusicStart { frame: 405, waiting: 4, pre: 0 },
        "the boundary's index, four ticks of waiting (D38)"
    );
    // The old player is not polled after the boundary, so its state is the handover's.
    assert_handed_over(&entry, &run.audio_player().unwrap());
}

#[test]
fn the_block_two_run_does_not_reload_at_glenz() {
    // `SECOND 2`'s mask has bit 0 clear, so `:813-814` skips the restart, and its initial
    // start is already MUSIC1@0 (`:772-775`): the same player and clock go on.
    let source = |name: &str| match name {
        "beglogo" => stub("beglogo", None, Some(0)),
        other => stub_source(other),
    };
    let mut run = music_run(vec!["beglogo", "glenz"], source, RunSettings::default());
    assert_eq!(run.music_module(), Some((ModuleId::Music1, 0)));
    let entry = run.music.as_ref().unwrap().start;
    assert_eq!(entry, MusicStart { frame: 408, waiting: 4, pre: 0 });
    let player = run.audio_player().unwrap();
    run.tick();
    run.tick();
    assert_eq!(run.part(), "glenz");
    assert_eq!(run.music_module(), Some((ModuleId::Music1, 0)));
    assert!(Arc::ptr_eq(&run.audio_player().unwrap(), &player), "the same player");
    assert_eq!(run.music.as_ref().unwrap().start, entry, "and the same clock");
}
