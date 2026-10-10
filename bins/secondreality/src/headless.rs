//! Headless mode: drive the runner with no display and dump its frames.
//!
//! Nothing here touches SDL — `--headless` is dispatched before video is
//! initialised, so this is the mode that runs on a machine with no display and
//! the one `xtask compare` will call. Both modes hand the same `Runner` real
//! elapsed wall-clock time, so the rate measured here is the rate the window
//! ticks at.
//!
//! Dumped frames are native geometry, one output pixel per source pixel, no
//! border: a dump is 1:1 with an emulator capture, which is the whole point of
//! the `.rgb`. The `.idx` is the raw indexed frame, palette and all.

use std::error::Error;
use std::fs;
use std::io;
use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use serde::Serialize;
use sr_engine::part::PartStatus;
use sr_engine::present::{present, Aspect, FrameBuffer, PresentOptions};
use sr_engine::{IndexedFrame, Runner, Tick};

use crate::cli::Cli;

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

/// Run the part the command line named, dumping to `cli.dump_dir`.
pub fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let dir = cli
        .dump_dir
        .as_deref()
        .ok_or("headless mode needs --dump-dir")?;
    fs::create_dir_all(dir)?;

    let mut runner = crate::new_runner(&cli.part)?;
    let frames = match (cli.frames, cli.run_seconds) {
        // A parsed command line carries exactly one of the two, so the order
        // here is not a precedence rule — the last arm only catches a `Cli`
        // built by hand, which must not silently dump nothing.
        (Some(count), _) => by_tick(&mut runner, &cli.part, dir, count)?,
        (None, Some(seconds)) => vec![run_for(&mut runner, &cli.part, dir, seconds)?],
        (None, None) => return Err("headless mode needs --frames or --run-seconds".into()),
    };

    write_manifest(dir, &cli.part, &frames)
}

/// `--frames N`: step exactly N times, dumping the frame of every tick.
///
/// A part that retires — reports `Finished`, or asks to quit — stops the dump
/// early: the runner stops ticking, so there are no further frames to write.
fn by_tick(
    runner: &mut Runner,
    part: &str,
    dir: &Path,
    count: u32,
) -> Result<Vec<FrameEntry>, Box<dyn Error>> {
    let mut frames = Vec::new();
    for _ in 0..count {
        let before = runner.part_frame();
        runner.step();
        if runner.part_frame() == before {
            break;
        }
        // The frame just drawn is tick `part_frame() - 1` of the run, counted
        // from zero, so the two names the test card writes are index 0 and 1.
        frames.push(dump(dir, part, runner.part_frame() - 1, runner.frame())?);
    }
    Ok(frames)
}

/// `--run-seconds S`: the runner is driven by real elapsed wall-clock time,
/// and only the last frame is dumped — ten seconds is 700 ticks, which as
/// 320×200 RGB8 is 130 MB of output nobody wants.
///
/// Prints the achieved tick rate. That number is the "runs at the correct
/// speed" check, and it is the same tick the window paces itself with.
fn run_for(
    runner: &mut Runner,
    part: &str,
    dir: &Path,
    seconds: f64,
) -> Result<FrameEntry, Box<dyn Error>> {
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
        ticks += u64::from(runner.advance(now - sampled));
        sampled = now;
        if runner.exit_requested() || runner.status() == PartStatus::Finished {
            break;
        }
        if now - start >= duration {
            break;
        }
    }

    // Hand the tick the rest of the run, so the count covers every moment
    // between the first sample and now. A retired runner returns 0.
    let now = Instant::now();
    ticks += u64::from(runner.advance(now - sampled));
    let elapsed = now - start;
    println!(
        "{part}: {ticks} ticks in {:.3} s = {:.3} Hz (nominal {:.3} Hz)",
        elapsed.as_secs_f64(),
        ticks as f64 / elapsed.as_secs_f64(),
        Tick::HZ_MILLI as f64 / 1000.0,
    );

    Ok(dump(dir, part, runner.part_frame().saturating_sub(1), runner.frame())?)
}

/// Write one frame as `<part>_<index:06>.idx` and `<part>_<index:06>.rgb`, and
/// describe it for the manifest.
fn dump(dir: &Path, part: &str, index: u32, frame: &IndexedFrame) -> io::Result<FrameEntry> {
    let idx = file_name(part, index, "idx");
    let rgb = file_name(part, index, "rgb");
    fs::write(dir.join(&idx), &frame.pixels)?;
    fs::write(dir.join(&rgb), rgb8(frame))?;
    Ok(FrameEntry {
        index,
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
fn write_manifest(dir: &Path, part: &str, frames: &[FrameEntry]) -> Result<(), Box<dyn Error>> {
    let first = frames.first().ok_or("the part drew no frames to dump")?;
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        part,
        // The first frame's geometry, not the last's: a part that switches to
        // double-scan mid-run changes the frame size, and this pair is what
        // the comparison tool picks its mode from.
        width: first.width,
        height: first.height,
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
    part: &'a str,
    width: u16,
    height: u16,
    frames: &'a [FrameEntry],
}
