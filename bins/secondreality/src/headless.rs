//! Headless mode: drive the sequence with no display and dump its frames. Nothing here
//! touches SDL, so it runs on a machine with no display.
//!
//! Dumped frames are native geometry, one output pixel per source pixel, no border, so a
//! `.rgb` is 1:1 with an emulator capture; the `.idx` is the raw indexed frame. A frame's
//! index is the reference's own for a run that began at a block entry (`index_origin`
//! `boot`) and the run's ticks from zero otherwise (`part`), which compare with nothing
//! under `reference/`.

use std::error::Error;
use std::fs;
use std::io::{self, BufWriter, Write as _};
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

/// A dump's presentation: one output pixel a source pixel, whatever the window's options.
const DUMP_OPTIONS: PresentOptions = PresentOptions {
    aspect: Aspect::SourcePixels,
    scale: 1,
    border: false,
};

/// How long `--run-seconds` sleeps between wall-clock samples: no spin, and far below the
/// 250 ms the tick discards as stale.
const SAMPLE_PERIOD: Duration = Duration::from_millis(10);

/// Run what the command line named, dumping to `cli.dump_dir`.
pub fn run(cli: &Cli) -> Result<(), Box<dyn Error>> {
    let dir = cli
        .dump_dir
        .as_deref()
        .ok_or("headless mode needs --dump-dir")?;
    fs::create_dir_all(dir)?;

    let mut run = crate::new_sequence(cli)?;
    // No sound card here (`--no-audio` is a no-op); `--audio-dump` takes the tick's samples.
    if let Some(path) = cli.audio_dump.as_deref() {
        run.dump_audio(path)?;
    }
    let mut positions = match cli.positions.as_deref() {
        Some(path) => Some(
            Positions::create(path).map_err(|e| format!("--positions {}: {e}", path.display()))?,
        ),
        None => None,
    };
    let frames = match (cli.frames, cli.run_seconds, cli.dump_frames.as_deref()) {
        // `Cli::validate` allows exactly one, and no `--positions` with `--run-seconds`.
        (Some(count), _, _) => by_tick(&mut run, dir, count, positions.as_mut())?,
        (None, Some(seconds), _) => vec![run_for(&mut run, dir, seconds)?],
        (None, None, Some(wanted)) => by_index(&mut run, dir, wanted, positions.as_mut())?,
        (None, None, None) => {
            return Err("headless mode needs --frames, --dump-frames or --run-seconds".into())
        }
    };
    if let Some(positions) = positions {
        positions.finish()?;
    }
    crate::audio::dump_result(&run)?;
    write_manifest(dir, &run, &frames)
}

/// `--frames N`: step N times, dumping every tick's frame. A held run dumps the held
/// screen, an ended demo run its blank; only a *finished* run stops early.
fn by_tick(
    run: &mut Sequence,
    dir: &Path,
    count: u32,
    mut positions: Option<&mut Positions>,
) -> Result<Vec<FrameEntry>, Box<dyn Error>> {
    let mut frames = Vec::new();
    for _ in 0..count {
        if run.tick() == SequenceStatus::Finished {
            break;
        }
        if let Some(positions) = positions.as_deref_mut() {
            positions.record(run)?;
        }
        // The frame just drawn is the cursor minus one.
        frames.push(dump(dir, run, run.index() - 1)?);
    }
    Ok(frames)
}

/// `--dump-frames A,B,C`: dump exactly those indices. **Every one is produced, or the run
/// fails**: a hold answers indices past the last part, and a run that finished first
/// is an error, because a missing keyframe reads as one that matched.
fn by_index(
    run: &mut Sequence,
    dir: &Path,
    wanted: &[u32],
    mut positions: Option<&mut Positions>,
) -> Result<Vec<FrameEntry>, Box<dyn Error>> {
    let start = run.index();
    // `Cli::dump_indices_are_reachable`'s rule, against the run's own start.
    if let Some(unreachable) = first_unreachable(start, wanted) {
        return Err(format!(
            "--dump-frames asks for {unreachable}, before the run's own start at {start} \
             ({} begins there); the run can never reach it",
            run.part()
        )
        .into());
    }
    let Some(last) = wanted.iter().copied().max() else {
        return Err("--dump-frames was given no indices to dump".into());
    };

    let mut frames = Vec::new();
    loop {
        if run.tick() == SequenceStatus::Finished {
            break;
        }
        if let Some(positions) = positions.as_deref_mut() {
            positions.record(run)?;
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

/// `--run-seconds S`: drive by wall-clock time, dump only the last frame, and print the
/// achieved tick rate (the window paces to the same tick).
fn run_for(run: &mut Sequence, dir: &Path, seconds: f64) -> Result<FrameEntry, Box<dyn Error>> {
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

    // The rest of the run, so the count covers every moment up to now.
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

    let Some(index) = run.frame_index() else {
        return Err(format!("--run-seconds {seconds} ended before the run drew a frame").into());
    };
    Ok(dump(dir, run, index)?)
}

/// `--positions FILE`: one CSV line a tick, `index,pass,part,order,row` — the frame's index,
/// [`Sequence::pass`], the part named as [`FrameEntry::part`], and the published music
/// position (empty with no player). `tests/fullrun.rs` reads it against the traces (D13).
struct Positions {
    out: BufWriter<fs::File>,
}

impl Positions {
    /// Create (or truncate) `path` and write the header.
    fn create(path: &Path) -> io::Result<Positions> {
        let mut out = BufWriter::new(fs::File::create(path)?);
        writeln!(out, "index,pass,part,order,row")?;
        Ok(Positions { out })
    }

    /// The line for the tick `run` has just drawn.
    fn record(&mut self, run: &Sequence) -> io::Result<()> {
        let (order, row) = match run.music_position() {
            Some(position) => (position.order.to_string(), position.row.to_string()),
            None => (String::new(), String::new()),
        };
        writeln!(
            self.out,
            "{},{},{},{order},{row}",
            run.index() - 1,
            run.pass(),
            run.part()
        )
    }

    /// Flush, so a write that fails is the run's failure and not a short file.
    fn finish(mut self) -> io::Result<()> {
        self.out.flush()
    }
}

/// Write one frame as `<part>_<index:06>.idx` and `.rgb`, and describe it for the manifest.
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

/// The published music position as `order:row`, or `None` for a run with no player.
fn music_position(run: &Sequence) -> Option<String> {
    run.music_position()
        .map(|position| format!("{}:{}", position.order, position.row))
}

/// `<part>_<index:06>.<extension>`.
fn file_name(part: &str, index: u32, extension: &str) -> String {
    format!("{part}_{index:06}.{extension}")
}

/// The frame as RGB8: `present` under the dump options, alpha dropped.
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
        // The first frame's geometry: the comparison tool picks its mode from it.
        width: first.width,
        height: first.height,
        index_origin: if run.index_is_reference() { "boot" } else { "part" },
        anchor: run.anchor(),
        music: run.music_module().and_then(|(module, start_order)| {
            Some(MusicSetup {
                module: module.name(),
                start_order,
                start_frame: run.music_start_frame()?,
            })
        }),
        starts: run
            .starts()
            .iter()
            .map(|&(index, part)| Start { index, part })
            .collect(),
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
    /// The part whose picture the frame is: during a hold, the last part that ran (D21).
    part: String,
    /// This frame's own geometry, which can change within a run.
    width: u16,
    height: u16,
    idx: String,
    rgb: String,
    /// The published `order:row` when this frame was drawn (D13), or `null` with no
    /// player. `xtask compare` does not read it.
    music_position: Option<String>,
}

/// The dump's index file.
#[derive(Debug, Serialize)]
struct Manifest<'a> {
    version: u32,
    /// The part of the first dumped frame (see `FrameEntry::part`).
    part: &'a str,
    width: u16,
    height: u16,
    /// `boot` when the indices are the reference's own frame counter, `part`
    /// when they are the run's ticks from zero.
    index_origin: &'a str,
    /// The reference index the run began at, for a `boot` run.
    anchor: Option<u32>,
    /// What the run's **current** player was started as (D13), or `null` with no player.
    music: Option<MusicSetup<'a>>,
    /// Every plan part the run started, at its first tick's index ([`Sequence::starts`]).
    starts: Vec<Start>,
    frames: &'a [FrameEntry],
}

/// One entry of the manifest's `starts`.
#[derive(Debug, Serialize)]
struct Start {
    index: u32,
    part: &'static str,
}

/// What a run's music started as, for the manifest.
#[derive(Debug, Serialize)]
struct MusicSetup<'a> {
    /// `MUSIC0` or `MUSIC1`.
    module: &'a str,
    /// The order the player was seeked to: the entry point's, or the mid-run restart's.
    start_order: u8,
    /// The run frame the music is aligned to ([`shell::MusicStart`](crate::shell::MusicStart)):
    /// a waiting entry's clock start, a pre-polled entry's first drawn frame (block 4: 418,
    /// clock at 408), or a restart's boundary.
    start_frame: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::Sequence;
    use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
    use sr_engine::view::View;
    use std::path::PathBuf;

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

    /// A part that cannot start: the one way a run finishes.
    struct Fails;

    impl Part for Fails {
        fn name(&self) -> &'static str { "fails" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
            Err(PartError::new("fails", "no"))
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
        // BEGLOGO's 900 in miniature: answered by the held picture.
        let dir = scratch("hold");
        let mut run = run_of(false);
        let frames = by_index(&mut run, &dir, &[0, 1, 5], None).unwrap();
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
        let dir = scratch("ended");
        let mut run = run_of(true);
        let error = by_index(&mut run, &dir, &[0, 5], None).unwrap_err().to_string();
        assert!(error.contains("before reaching 5"), "{error}");
        assert!(run.finished(), "the run ended, so there is no hold to answer from");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn an_index_past_a_demo_runs_exit_stage_is_its_blank_screen() {
        // An ended demo run keeps composing its blank, so an index past it is answered.
        let dir = scratch("exit");
        let mut run = crate::shell::Sequence::with_run(
            8,
            8,
            View::new(8, 8),
            vec!["short"],
            |name| (name == "short").then(|| Box::new(Short(1)) as Box<dyn Part>),
            None,
            crate::shell::RunSettings {
                ending: crate::shell::Ending::Exit,
                ..crate::shell::RunSettings::default()
            },
        )
        .unwrap();
        let frames = by_index(&mut run, &dir, &[1, 100], None).unwrap();
        assert_eq!(frames.iter().map(|f| f.index).collect::<Vec<_>>(), [1, 100]);
        assert!(run.ended(), "two frames and 84 exit ticks are behind index 100");
        assert!(!run.finished(), "ended is not finished");

        let picture = std::fs::read(dir.join("short_000001.rgb")).unwrap();
        assert!(picture.iter().any(|&byte| byte != 0), "the part's own picture");
        let blank = std::fs::read(dir.join("short_000100.rgb")).unwrap();
        assert!(blank.iter().all(|&byte| byte == 0), "the exit stage's blank screen");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn positions_writes_one_line_a_tick_with_the_part_whose_picture_is_up() {
        // The hold's frames 2..=5 are named for `short`; no player, so no position.
        let dir = scratch("positions");
        let path = dir.join("positions.csv");
        let mut run = run_of(false);
        let mut log = Positions::create(&path).unwrap();
        by_index(&mut run, &dir, &[5], Some(&mut log)).unwrap();
        log.finish().unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(lines[0], "index,pass,part,order,row");
        assert_eq!(
            &lines[1..],
            ["0,1,short,,", "1,1,short,,", "2,1,short,,", "3,1,short,,", "4,1,short,,",
             "5,1,short,,"]
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_manifest_names_every_start() {
        let dir = scratch("starts");
        let mut run = run_of(false);
        let frames = by_index(&mut run, &dir, &[3], None).unwrap();
        write_manifest(&dir, &run, &frames).unwrap();
        let manifest: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap())
                .unwrap();
        assert_eq!(manifest["starts"], serde_json::json!([{ "index": 0, "part": "short" }]));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
