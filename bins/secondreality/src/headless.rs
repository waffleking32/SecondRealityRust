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

    write_manifest(dir, &run, &frames)
}

/// `--frames N`: step exactly N times, dumping the frame of every tick.
///
/// A run that ends — the last part retired, or the next one is not ported —
/// stops the dump early: a finished sequence produces no further frames.
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
        frames.push(dump(dir, run.part(), run.index() - 1, run.frame())?);
    }
    Ok(frames)
}

/// `--dump-frames A,B,C`: dump exactly those reference indices.
///
/// The alternative is `--frames N`, which dumps every tick up to N; for a
/// part whose keyframes are four numbers spread over 900 frames that is 900
/// frames of 320×400 output to answer four, and every one of them has to be
/// written before the first comparison can be made.
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
    let last = wanted.iter().copied().max().expect("a parsed list is never empty");

    let mut frames = Vec::new();
    loop {
        if run.tick() == SequenceStatus::Finished {
            break;
        }
        let index = run.index() - 1;
        if wanted.contains(&index) {
            frames.push(dump(dir, run.part(), index, run.frame())?);
        }
        if index >= last {
            break;
        }
    }

    // A keyframe the run ended before reaching is a hole in the dump, and a
    // hole nobody mentioned reads as a keyframe that matched.
    for missing in wanted.iter().filter(|index| !frames.iter().any(|f| f.index == **index)) {
        eprintln!(
            "secondreality: the run ended before frame {missing}; nothing was dumped for it"
        );
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

    // A run too short to tick has drawn nothing, and `index() - 1` would name a
    // frame that does not exist. `Sequence::new` puts the cursor at the run's
    // start, so anything at or below it is a run that never drew.
    if run.index() <= run.anchor().unwrap_or(0) {
        return Err(format!("--run-seconds {seconds} ended before the run drew a frame").into());
    }
    Ok(dump(dir, run.part(), run.index() - 1, run.frame())?)
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
/// describe it for the manifest.
fn dump(dir: &Path, part: &'static str, index: u32, frame: &IndexedFrame) -> io::Result<FrameEntry> {
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
    })
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
    /// The part that drew it: a block run's frames come from several, and each
    /// one is named for its own.
    part: String,
    /// This frame's own geometry. The frame is resized from the view every
    /// tick, so a run can legitimately dump frames of more than one size.
    width: u16,
    height: u16,
    idx: String,
    rgb: String,
}

/// The dump's index file.
#[derive(Debug, Serialize)]
struct Manifest<'a> {
    version: u32,
    /// The part that drew the run's first dumped frame.
    part: &'a str,
    width: u16,
    height: u16,
    /// `boot` when the indices are the reference's own frame counter, `part`
    /// when they are the run's ticks from zero.
    index_origin: &'a str,
    /// The reference index the run began at, for a `boot` run.
    anchor: Option<u32>,
    frames: &'a [FrameEntry],
}


#[cfg(test)]
mod tests {
    use super::*;

    /// The integration test in `tests/shell.rs` can only run the test card,
    /// which is unanchored; the other half of the mapping is this one.
    #[test]
    fn the_manifest_word_follows_the_runs_own_numbering() {
        assert_eq!(index_origin(true), "boot");
        assert_eq!(index_origin(false), "part");
    }
}
