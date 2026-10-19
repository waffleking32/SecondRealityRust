//! Headless mode: drive the sequence with no display and dump its frames.
//!
//! Nothing here touches SDL — `--headless` is dispatched before video is
//! initialised, so this is the mode that runs on a machine with no display and
//! the one `xtask compare` will call. Both modes hand the same `Sequence` real
//! elapsed wall-clock time, so the rate measured here is the rate the window
//! ticks at.
//!
//! Dumped frames are native geometry, one output pixel per source pixel, no
//! border: a dump is 1:1 with an emulator capture, which is the whole point of
//! the `.rgb`. The `.idx` is the raw indexed frame, palette and all.
//!
//! **A frame's index is the reference's own**, and the manifest says which
//! numbering it is in: `index_origin` is `boot` for a run that began at a
//! measured block entry and `part` otherwise, and a `part` run's indices are
//! not comparable with anything under `reference/`.

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sr_engine::present::{present, Aspect, FrameBuffer, PresentOptions};
use sr_engine::{IndexedFrame, Tick};

use crate::cli::Cli;
use crate::shell::{first_unreachable, Sequence, SequenceStatus};

/// What the manifest says it is. The comparison tool refuses anything else.
const MANIFEST_VERSION: u32 = 1;

/// The presentation a dump uses. Deliberately not the window's: display
/// scaling, aspect and border never reach a fidelity measurement.
const DUMP_OPTIONS: PresentOptions = PresentOptions {
    aspect: Aspect::SquarePixels,
    scale: 1,
    border: false,
};

/// How long `--run-seconds` sleeps between wall-clock samples: long enough not
/// to spin, short enough that no interval comes near the 250 ms the tick
/// discards as stale. The accumulator carries the fraction between samples, so
/// sleeping is not the same as rounding.
const SAMPLE_PERIOD: Duration = Duration::from_millis(10);

/// Run what the command line named, dumping to `cli.dump_dir`.
pub fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let dir = cli
        .dump_dir
        .as_deref()
        .ok_or("headless mode needs --dump-dir")?;
    fs::create_dir_all(dir)?;

    let mut run = crate::new_sequence(cli)?;
    // The music, before the first tick: `--headless` opens no sound card (`--no-audio` is
    // therefore a no-op here, which is why a run with and without it is the same run), and
    // `--audio-dump` writes the WAV the tick's samples go to.
    if let Some(path) = cli.audio_dump.as_deref() {
        run.dump_audio(path)?;
    }
    let frames = match (cli.frames, cli.run_seconds, cli.dump_frames.as_deref()) {
        // A parsed command line carries exactly one of the three, so the order
        // here is not a precedence rule — the last arm only catches a `Cli`
        // built by hand, which must not silently dump nothing.
        (Some(count), _, _) => by_tick(&mut run, dir, count)?,
        (None, Some(seconds), _) => vec![run_for(&mut run, dir, seconds)?],
        (None, None, Some(wanted)) => by_index(&mut run, dir, wanted)?,
        (None, None, None) => {
            return Err("headless mode needs --frames, --dump-frames or --run-seconds".into())
        }
    };

    // A dump that stopped half way is a failure and not a shorter recording (the sink keeps
    // the first write it could not make rather than printing on a tick).
    if let Some(error) = run.audio_error() {
        return Err(format!("--audio-dump: {error}").into());
    }
    write_manifest(dir, &run, &frames)
}

/// `--frames N`: step exactly N times, dumping the frame of every tick.
///
/// N ticks, whether or not the run's parts last that long: a run past its last
/// part holds the screen that part left, so its later frames are that screen
/// and not nothing. A run that *ends* stops the dump early, because a finished
/// sequence produces no further frames.
fn by_tick(
    run: &mut Sequence,
    dir: &Path,
    count: u32,
) -> Result<Vec<FrameEntry>, Box<dyn Error>> {
    let mut frames = Vec::new();
    for _ in 0..count {
        if run.tick() == SequenceStatus::Finished {
            break;
        }
        // The frame just drawn is the run's cursor minus one, so a part the
        // shell started at its anchor dumps that anchor's own frame first.
        frames.push(dump(dir, run, run.index() - 1)?);
    }
    Ok(frames)
}

/// `--dump-frames A,B,C`: dump exactly those reference indices.
///
/// The alternative is `--frames N`, which dumps every tick up to N; for a
/// part whose keyframes are four numbers spread over 900 frames that is 900
/// frames of 320×400 output to answer four, and every one of them has to be
/// written before the first comparison can be made.
///
/// **Every requested index is produced, or the run is a failure.** A run that
/// has retired its last part keeps ticking and holds the screen that part left
/// (see [`Sequence`]), so the usual reason an index is still ahead — the part
/// ended first, as BEGLOGO ends at its fade and the reference's 900 is the hold
/// after it — is not a hole. A requested index the run cannot reach is one
/// where the run *ended*, which is a part the port has that could not be
/// started: that is a failure with an exit code, because a missing keyframe in
/// a dump reads as a keyframe that matched.
fn by_index(
    run: &mut Sequence,
    dir: &Path,
    wanted: &[u32],
) -> Result<Vec<FrameEntry>, Box<dyn Error>> {
    let start = run.index();
    // An index the run starts after is a mistake in the command, not an empty
    // dump: the run walks past it on the way to the others. The command line
    // refuses it too (`Cli::dump_indices_are_reachable`) — this is the same
    // rule, for a `Cli` built by hand.
    if let Some(unreachable) = first_unreachable(start, wanted) {
        return Err(format!(
            "--dump-frames asks for {unreachable}, before the run's own start at {start} \
             ({} begins there); the run can never reach it",
            run.part()
        )
        .into());
    }
    // A hand-built `Cli` can carry an empty list, and a dump of nothing must say
    // so rather than write a manifest with no frames in it.
    let Some(last) = wanted.iter().copied().max() else {
        return Err("--dump-frames was given no indices to dump".into());
    };

    let mut frames = Vec::new();
    loop {
        if run.tick() == SequenceStatus::Finished {
            break;
        }
        let index = run.index() - 1;
        if wanted.contains(&index) {
            frames.push(dump(dir, run, index)?);
        }
        if index >= last {
            break;
        }
    }

    let missing: Vec<u32> = wanted
        .iter()
        .copied()
        .filter(|index| !frames.iter().any(|frame| frame.index == *index))
        .collect();
    if !missing.is_empty() {
        return Err(format!(
            "the run ended at frame {} before reaching {}; nothing was dumped for \
             {missing:?}",
            run.index() - 1,
            missing[0],
        )
        .into());
    }
    Ok(frames)
}

/// `--run-seconds S`: the run is driven by real elapsed wall-clock time, and
/// only the last frame is dumped — ten seconds is 700 ticks, which as 320×200
/// RGB8 is 130 MB of output nobody wants.
///
/// Prints the achieved tick rate. That number is the "runs at the correct
/// speed" check, and it is the same tick the window paces itself with.
fn run_for(run: &mut Sequence, dir: &Path, seconds: f64) -> Result<FrameEntry, Box<dyn Error>> {
    // Not `from_secs_f64`: a duration too large for `Duration` is a bad
    // argument, not a reason to panic.
    let duration = Duration::try_from_secs_f64(seconds)
        .map_err(|_| format!("--run-seconds {seconds} is out of range"))?;
    let start = Instant::now();
    let mut sampled = start;
    let mut ticks = 0u64;

    loop {
        thread::sleep(SAMPLE_PERIOD);
        let now = Instant::now();
        ticks += u64::from(run.advance(now - sampled));
        sampled = now;
        if run.finished() {
            break;
        }
        if now - start >= duration {
            break;
        }
    }

    // Hand the tick the rest of the run, so the count covers every moment
    // between the first sample and now. A finished run returns 0.
    let now = Instant::now();
    ticks += u64::from(run.advance(now - sampled));
    let elapsed = now - start;
    println!(
        "{}: {ticks} ticks in {:.3} s = {:.3} Hz (nominal {:.3} Hz)",
        run.part(),
        elapsed.as_secs_f64(),
        ticks as f64 / elapsed.as_secs_f64(),
        Tick::HZ_MILLI as f64 / 1000.0,
    );

    // A run too short to tick has drawn nothing, and the frame's index would
    // name a frame that does not exist. `Sequence::new` puts the cursor at the
    // run's start, so anything at or below it is a run that never drew.
    let Some(index) = run.frame_index() else {
        return Err(format!("--run-seconds {seconds} ended before the run drew a frame").into());
    };
    Ok(dump(dir, run, index)?)
}

/// The manifest's word for a run's numbering: `boot` when its indices are the
/// reference's own frame counter, `part` when they are the run's ticks from
/// zero.
///
/// Two dumps whose indices look comparable and are not is the failure this
/// field prevents, so it is a function of the run's numbering and nothing
/// else.
fn index_origin(is_reference: bool) -> &'static str {
    if is_reference {
        "boot"
    } else {
        "part"
    }
}

/// Write one frame as `<part>_<index:06>.idx` and `<part>_<index:06>.rgb`, and
/// describe it for the manifest — including where the music was when it was drawn.
fn dump(dir: &Path, run: &Sequence, index: u32) -> io::Result<FrameEntry> {
    let part = run.part();
    let frame = run.frame();
    let idx = file_name(part, index, "idx");
    let rgb = file_name(part, index, "rgb");
    fs::write(dir.join(&idx), &frame.pixels)?;
    fs::write(dir.join(&rgb), rgb8(frame))?;
    Ok(FrameEntry {
        index,
        part: part.to_string(),
        width: frame.width,
        height: frame.height,
        idx,
        rgb,
        music_position: music_position(run),
    })
}

/// The published music position, as the status line spells it — `order:row` — or `None`
/// for a run with no player.
fn music_position(run: &Sequence) -> Option<String> {
    run.music_position()
        .map(|position| format!("{}:{}", position.order, position.row))
}

/// `<part>_<index:06>.<extension>`.
fn file_name(part: &str, index: u32, extension: &str) -> String {
    format!("{part}_{index:06}.{extension}")
}

/// The frame as RGB8, one output pixel per source pixel: `present` under the
/// dump options, with its constant alpha dropped.
fn rgb8(frame: &IndexedFrame) -> Vec<u8> {
    let mut buffer = FrameBuffer::new(0, 0);
    present(frame, &DUMP_OPTIONS, &mut buffer);
    let mut rgb = Vec::with_capacity(buffer.width() * buffer.height() * 3);
    for rgba in buffer.rgba().chunks_exact(4) {
        rgb.extend_from_slice(&rgba[..3]);
    }
    rgb
}

/// Write `manifest.json`: what was dumped and how to read it.
fn write_manifest(dir: &Path, run: &Sequence, frames: &[FrameEntry]) -> Result<(), Box<dyn Error>> {
    let first = frames.first().ok_or("the run drew no frames to dump")?;
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        part: &first.part,
        // The first frame's geometry, not the last's: a part that switches to
        // double-scan mid-run changes the frame size, and this pair is what
        // the comparison tool picks its mode from.
        width: first.width,
        height: first.height,
        // Which numbering the indices below are in. Without it, a dump that
        // began at a block entry and one that began at zero look comparable
        // and are not.
        index_origin: index_origin(run.index_is_reference()),
        anchor: run.anchor(),
        // A run that has a module has a start frame too — both come from the same
        // `Option<RunMusic>` — so this is one `and_then` and not a default that no run
        // reaches.
        music: run.music_module().and_then(|(module, start_order)| {
            Some(MusicSetup {
                module: module.name(),
                start_order,
                start_frame: run.music_start_frame()?,
            })
        }),
        frames,
    };
    let mut json = serde_json::to_vec_pretty(&manifest)?;
    json.push(b'\n');
    fs::write(dir.join("manifest.json"), json)?;
    Ok(())
}

/// What one dumped frame is and where it lives.
#[derive(Debug, Serialize)]
struct FrameEntry {
    index: u32,
    /// The part the frame belongs to: a block run's frames come from several,
    /// and each one is named for its own.
    ///
    /// **A frame drawn during a hold is named for the part whose screen it
    /// is**, which is the last part that ran — the frame is that part's picture
    /// held, and nothing here tells it apart from a frame the part drew (D21
    /// says what that costs a comparison). It is not the name of a part that
    /// drew it, because during a hold none did.
    part: String,
    /// This frame's own geometry. The frame is resized from the view every
    /// tick, so a run can legitimately dump frames of more than one size.
    width: u16,
    height: u16,
    idx: String,
    rgb: String,
    /// Where the music was when this frame was drawn — the published `order:row`, the
    /// same spelling the window's status line uses — or `null` for a run with no player.
    ///
    /// **This is what makes a dump's frames answerable for the run that drew them.** The
    /// demo is music-counted (D13): the same part drawn at a different music position is a
    /// different demo, and a comparison that cannot see the position is comparing frames
    /// without knowing that. `xtask compare` ignores the field (it reads `index`, `idx`
    /// and `rgb`, and a manifest's other keys are not its business), so this is additive.
    music_position: Option<String>,
}

/// The dump's index file.
#[derive(Debug, Serialize)]
struct Manifest<'a> {
    version: u32,
    /// The part the run's first dumped frame belongs to — which, for a frame
    /// dumped during a hold, is the last part that ran (see `FrameEntry`).
    part: &'a str,
    width: u16,
    height: u16,
    /// `boot` when the indices are the reference's own frame counter, `part`
    /// when they are the run's ticks from zero.
    index_origin: &'a str,
    /// The reference index the run began at, for a `boot` run.
    anchor: Option<u32>,
    /// The music the run's **current** player was started as: which module, the order it
    /// was seeked to, and the run frame it is aligned to — the entry point's row, or the
    /// mid-run restart's — recorded the way a capture records the argument it started the
    /// original with (D13). `null` for a run with no player.
    music: Option<MusicSetup<'a>>,
    frames: &'a [FrameEntry],
}

/// What a run's music started as, for the manifest.
#[derive(Debug, Serialize)]
struct MusicSetup<'a> {
    /// `MUSIC0` or `MUSIC1`.
    module: &'a str,
    /// The order the run's current player was seeked to — the entry point's, or the
    /// mid-run restart's.
    start_order: u8,
    /// The run frame, in this manifest's own numbering, the music is **aligned to**
    /// ([`shell::MusicStart`](crate::shell::MusicStart)): for a waiting entry the clock's
    /// start; for a pre-polled entry (block 4: 418 while the clock starts at 408) the
    /// run's first drawn frame, with the intervening frames pre-polled; and for a mid-run
    /// restart its boundary, whose clock begins `MusicStart::waiting` frames later.
    start_frame: u32,
}


#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::Sequence;
    use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
    use sr_engine::view::View;
    use std::path::PathBuf;

    /// The integration test in `tests/shell.rs` can only run the test card,
    /// which is unanchored; the other half of the mapping is this one.
    #[test]
    fn the_manifest_word_follows_the_runs_own_numbering() {
        assert_eq!(index_origin(true), "boot");
        assert_eq!(index_origin(false), "part");
    }

    /// Fills the surface with 7 for `frames` frames and then retires.
    struct Short(u32);

    impl Part for Short {
        fn name(&self) -> &'static str { "short" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            ctx.surface.clear(7);
            if ctx.frame >= self.0 { PartStatus::Finished } else { PartStatus::Continuing }
        }
    }

    /// A part the port has that cannot start — the one way a run *ends*.
    struct Fails;

    impl Part for Fails {
        fn name(&self) -> &'static str { "fails" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
            Err(PartError { part: "fails", message: "no".to_string() })
        }
        fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
    }

    /// A run of one short stub, or of a stub and then a part that cannot start.
    fn run_of(failing: bool) -> Sequence {
        let plan = if failing { vec!["short", "fails"] } else { vec!["short"] };
        Sequence::with_source(8, 8, View::new(8, 8), plan, |name| match name {
            "short" => Some(Box::new(Short(1))),
            "fails" => Some(Box::new(Fails)),
            _ => None,
        })
        .unwrap()
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sr-headless-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn an_index_past_the_end_of_the_runs_parts_is_dumped_with_the_screen_held() {
        // BEGLOGO's 900 in miniature: the part retires after two frames and the
        // run has nothing left to draw, but the index is still answered — with
        // the picture the part left, because that is what the loader shows.
        let dir = scratch("hold");
        let mut run = run_of(false);
        let frames = by_index(&mut run, &dir, &[0, 1, 5]).unwrap();
        assert_eq!(frames.len(), 3, "every requested index was dumped");
        assert_eq!(frames.iter().map(|f| f.index).collect::<Vec<_>>(), [0, 1, 5]);
        assert!(!run.finished(), "and by holding, not by ending the run");
        assert_eq!(run.part(), "short", "the held frames are the last part's own");

        let held = std::fs::read(dir.join("short_000005.rgb")).unwrap();
        let last = std::fs::read(dir.join("short_000001.rgb")).unwrap();
        assert_eq!(held, last, "the hold rendered a different screen");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_index_a_run_that_ended_cannot_reach_is_a_failure() {
        // The run ends because a part it *has* cannot start, and there is no
        // hold after that: a dump missing frame 5 has to say so loudly, because
        // a hole in a dump reads as a keyframe that matched.
        let dir = scratch("ended");
        let mut run = run_of(true);
        let error = by_index(&mut run, &dir, &[0, 5]).unwrap_err().to_string();
        assert!(error.contains("before reaching 5"), "{error}");
        assert!(run.finished(), "the run ended, so there is no hold to answer from");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_empty_list_of_indices_is_refused_rather_than_dumped() {
        let dir = scratch("empty");
        let mut run = run_of(false);
        let error = by_index(&mut run, &dir, &[]).unwrap_err().to_string();
        assert!(error.contains("no indices"), "{error}");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
