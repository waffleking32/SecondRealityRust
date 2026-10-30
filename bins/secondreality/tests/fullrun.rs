//! The port's `--block 1` against the original's whole `SECOND 1` run:
//! part by part (`reference-indices.md` §26.3's boundary table), row by row (block 1's six
//! music traces, `reference/music/second1*`), and frame by frame at the boundaries.
//!
//! **One run answers every test here.** The release binary plays block 1 headless from
//! ALKU's 404 to [`RUN_TO`] — the original's ENDSCRL start plus 2,000 frames, about 48,000
//! ticks — writing its timeline with `--positions` and dumping, for every part, each frame
//! from two before its start to its first frame. That is ten seconds in release and minutes
//! in debug, so the tests are ignored in a debug build only and run by
//!
//! ```text
//! cargo test --release -p secondreality --test fullrun -- --nocapture
//! ```
//!
//! (or in debug with `-- --ignored`, slowly). The dump is read into memory — a hash, the
//! geometry and a black flag per frame — and removed before any test runs.
//!
//! **The seam table has one source**, `xtask/src/rediff/table.rs`'s `FULLRUN_SEAMS`, included
//! here by path: the original's first frame of each part (§26.3), the port's, and the port's
//! start. This file asserts the port's two columns against the run — the start is
//! `Sequence::starts`, and the first frame is the first frame at or after the start that meets
//! §26.3's own definition for that part (its own geometry, its first change to the screen, its
//! first visible frame after a black gap), with **no frame between the start and it** meeting
//! the definition. `cargo xtask rediff` derives every `fullrun` pairing from the same rows. The
//! music is compared at every frame of every block-1 trace after removing **one** offset per
//! trace — 0 for the boot trace, and for a window trace measured at its first row change (A38's
//! lag in `second1d`/`second1e`, the U2E restart's in `second1f`) — and the agreement within
//! [`JITTER_FRAMES`] is pinned with the offset.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, BTreeSet};
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

#[allow(dead_code)]
#[path = "../../../xtask/src/rediff/table.rs"]
mod table;

use table::{
    at, Part, FULLRUN_ACROSS_HANDOVER, FULLRUN_LOADER_FRAMES, FULLRUN_SEAMS, JITTER_FRAMES, TABLE,
};

/// The original's ENDSCRL begins at 46,277 (§26.3); the run goes 2,000 frames past it.
const RUN_TO: u32 = 46_277 + 2_000;

/// Block 1's plan, `MAIN/U2.ASM:789-912` with `whattorun` 7fh: `exe1` … `exe23`.
const PLAN: [&str; 19] = [
    "alku", "u2a", "pam", "beglogo", "glenz", "tunneli", "techno", "panicend", "mntscrl",
    "lnszoom", "plzpart", "minvball", "rayscrl", "3dsinfld", "jplogo", "u2e", "endlogo",
    "cred", "endscrl",
];

/// How a part's first frame is found — §26.3's definitions, and the two rulings for the
/// boundaries §26.3 can only bracket.
#[derive(Clone, Copy, Debug)]
enum First {
    /// The run's first frame: ALKU at the boot's 404.
    Anchor,
    /// The part's own first frame, its start: U2A's, whose screen nothing distinguishes from
    /// the one before it. The release's ALKU closes by rebuilding its last window as the
    /// 320×200 page U2A's background is (image `0x13ce`, A20), so the screen's geometry
    /// changes two frames before U2A starts and its first frame is the closing page's bytes.
    /// The original's 5961 is the first 320×200 frame by the same reading — ALKU's closing
    /// page or U2A's identical background, which no frame tells apart (A53).
    Start,
    /// The first frame of this geometry: the frame before is another one.
    Geometry(u64, u64),
    /// The first frame that differs from the one before it, after a frame that held.
    Change,
    /// The first frame that is no longer this filed held picture: GLENZ's collapse, whose
    /// first ticks hold BEGLOGO's final screen (`fullrun/03`'s 8300) through the music wait.
    Leaves(&'static str, u32),
    /// The first frame that is not black after a black frame.
    Visible,
    /// The first frame byte-identical to this filed reference frame: U2E's band (34120, the
    /// ruling that aligns U2E on it), JPLOGO's opening black (33263, the ruling that aligns
    /// JPLOGO on a frame's bytes rather than its bracket) and PAM's white (7707).
    Bytes(&'static str, u32),
}

/// §26.3's definition for each part, in `FULLRUN_SEAMS`' order. Where §26.3 gives two
/// sessions, `FULLRUN_SEAMS`' original is the `fullrun` capture's: BEGLOGO 7922 (`fullrun/02`
/// shows PAM's white at 7920 and never publishes 7921), GLENZ 8522 (`fullrun/03`'s collapse),
/// TUNNELI 11158 (`fullrun/04` holds d2's 11160 bytes at 11159); 3DSINFLD is h5's 31272 and
/// ENDSCRL l2r's 46277.
const RULES: [(&str, First); 19] = [
    ("alku", First::Anchor),
    ("u2a", First::Start),
    ("pam", First::Bytes("02-u2a-pam", 7707)),
    ("beglogo", First::Change),
    ("glenz", First::Leaves("03-beglogo", 8300)),
    ("tunneli", First::Visible),
    ("techno", First::Visible),
    ("panicend", First::Change),
    ("mntscrl", First::Change),
    ("lnszoom", First::Visible),
    ("plzpart", First::Geometry(320, 400)),
    ("minvball", First::Visible),
    ("rayscrl", First::Visible),
    ("3dsinfld", First::Visible),
    ("jplogo", First::Bytes("09-jplogo", 33263)),
    ("u2e", First::Bytes("10-u2e", 34120)),
    ("endlogo", First::Geometry(320, 400)),
    ("cred", First::Geometry(320, 200)),
    ("endscrl", First::Geometry(640, 400)),
];

/// The post-PANICEND cue wait's exit (`MAIN/U2.ASM:833-853`): the original's first frame
/// with `musplus` > 0 is 16630 (§26.6); the port's is the wait's last tick, the frame
/// before MNTSCRL starts.
const CUE_EXIT: (u32, u32) = (16_630, 16_632);

/// How far the worst-displacement search looks either side of a frame: past it, a
/// position is counted as one the trace never publishes nearby.
const SEARCH_FRAMES: u32 = 60;

/// Per block-1 trace: its name, the offset measured at its first row change (port frame
/// minus trace frame), and — at that offset — how many of its frames the port publishes
/// within [`JITTER_FRAMES`] of the trace, the worst displacement (searched to
/// [`SEARCH_FRAMES`]) and how many it publishes nowhere that near; how many agree within the
/// window with no offset at all; and the offset measured the same way at the trace's **last**
/// row change, which is how far the session's lag moves across the window.
struct Agreement {
    trace: &'static str,
    offset: i64,
    frames: usize,
    within: usize,
    worst: u32,
    beyond: usize,
    unshifted: usize,
    last_offset: i64,
}

/// Measured by this file's run, with the restart carry-over D45 (`music-clock.md` §2):
///
/// - `second1`, `second1c`, `second1b` — block 1 up to BEGLOGO, under the boot's MUSIC0:
///   the run's own clock, 97.4 / 100 / 99.7 % within the window at offset 0 / −1 / −1, as
///   `crates/sr-audio/tests/trace.rs` and the shell's own block-1 test hold them (`second1`'s
///   worst 7 is A12's; `second1b`'s one frame past the search is its last, where the port is
///   a row ahead of the window's edge).
/// - `second1d`, `second1e` — the `MUSIC1@0` session from GLENZ to JPLOGO: **5 frames late**
///   at the first row change of each (A38), 100 % and 75.3 % within the window at that offset.
///   `second1e`'s lag falls from +5 to −3 at its last row change — the drift its worst 8 is
///   (A54) — so with no offset at all 91.8 % of it agrees.
/// - `second1f` — the `MUSIC0@18` session from U2E to ENDSCRL: **8 frames early** at its first
///   row change and 0 at the last the run reaches, 44.8 % within the window at −8 and 95.7 %
///   with no offset; worst 12, and the one frame (38,376) whose position the trace never
///   publishes: the speed-1 row (A54).
const AGREEMENTS: [Agreement; 6] = [
    Agreement { trace: "second1", offset: 0, frames: 5541, within: 5399, worst: 7, beyond: 0, unshifted: 5399, last_offset: 3 },
    Agreement { trace: "second1c", offset: -1, frames: 1699, within: 1699, worst: 1, beyond: 0, unshifted: 1698, last_offset: -2 },
    Agreement { trace: "second1b", offset: -1, frames: 359, within: 358, worst: 1, beyond: 1, unshifted: 357, last_offset: -2 },
    Agreement { trace: "second1d", offset: 5, frames: 699, within: 699, worst: 2, beyond: 0, unshifted: 227, last_offset: 5 },
    Agreement { trace: "second1e", offset: 5, frames: 25_143, within: 18_928, worst: 8, beyond: 0, unshifted: 23_083, last_offset: -3 },
    Agreement { trace: "second1f", offset: -8, frames: 14_168, within: 6352, worst: 12, beyond: 1, unshifted: 13_554, last_offset: 0 },
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// One line of `--positions`.
struct Tick {
    pass: u32,
    part: String,
    position: Option<(u8, u8)>,
}

/// What the tests need of a dumped frame: its geometry, whether every pixel is black, and a
/// hash of its `.rgb` bytes — and the bytes themselves for the frames compared pixel by pixel.
struct Shot {
    geometry: (u64, u64),
    black: bool,
    hash: u64,
}

/// The run every test reads, held in memory: its dump directory is gone by the time a test
/// runs.
struct Run {
    starts: Vec<(u32, String)>,
    shots: BTreeMap<u32, Shot>,
    kept: BTreeMap<u32, Vec<u8>>,
    timeline: BTreeMap<u32, Tick>,
}

impl Run {
    fn shot(&self, index: u32) -> &Shot {
        self.shots.get(&index).unwrap_or_else(|| panic!("the run dumped no frame {index}"))
    }
}

fn hash(bytes: &[u8]) -> u64 {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    hasher.finish()
}

/// The `fullrun` frames compared pixel by pixel here: BEGLOGO's ramp and ENDLOGO's hold, the
/// two frames the parts' line-offset fix is read at.
fn kept() -> [(u32, &'static str, u32); 2] {
    [(7930, "03-beglogo", 7930), (at(38_300, Part::Endlogo), "11-endlogo", 38_300)]
}

/// Every frame the seam checks read: from two before each part's start to its first frame.
fn dumped() -> Vec<u32> {
    let mut indices: Vec<u32> = FULLRUN_SEAMS
        .iter()
        .skip(1)
        .flat_map(|seam| seam.start - 2..=seam.port)
        .collect();
    indices.extend(kept().iter().map(|(index, ..)| *index));
    indices.push(RUN_TO);
    indices.sort_unstable();
    indices.dedup();
    indices
}

fn run() -> &'static Run {
    static RUN: OnceLock<Run> = OnceLock::new();
    RUN.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("sr-fullrun-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let positions = dir.join("positions.csv");
        let frames = dir.join("frames");
        let list = dumped().iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
            .current_dir(repository_root())
            .args(["--block", "1", "--headless", "--dump-frames", &list, "--dump-dir"])
            .arg(&frames)
            .arg("--positions")
            .arg(&positions)
            .output()
            .unwrap();
        let read = || -> Run {
            assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
            let manifest: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(frames.join("manifest.json")).unwrap(),
            )
            .unwrap();
            let starts = manifest["starts"]
                .as_array()
                .expect("the manifest lists the run's starts")
                .iter()
                .map(|start| {
                    (
                        start["index"].as_u64().unwrap() as u32,
                        start["part"].as_str().unwrap().to_string(),
                    )
                })
                .collect();
            let keep: BTreeSet<u32> = kept().iter().map(|(index, ..)| *index).collect();
            let mut shots = BTreeMap::new();
            let mut kept_bytes = BTreeMap::new();
            for entry in manifest["frames"].as_array().unwrap() {
                let index = entry["index"].as_u64().unwrap() as u32;
                let bytes = std::fs::read(frames.join(entry["rgb"].as_str().unwrap())).unwrap();
                shots.insert(
                    index,
                    Shot {
                        geometry: (entry["width"].as_u64().unwrap(), entry["height"].as_u64().unwrap()),
                        black: bytes.iter().all(|byte| *byte == 0),
                        hash: hash(&bytes),
                    },
                );
                if keep.contains(&index) {
                    kept_bytes.insert(index, bytes);
                }
            }
            let text = std::fs::read_to_string(&positions).unwrap();
            let mut lines = text.lines();
            assert_eq!(lines.next(), Some("index,pass,part,order,row"), "the CSV's header");
            let timeline = lines
                .map(|line| {
                    let fields: Vec<&str> = line.split(',').collect();
                    assert_eq!(fields.len(), 5, "{line}");
                    let position = (!fields[3].is_empty())
                        .then(|| (fields[3].parse().unwrap(), fields[4].parse().unwrap()));
                    (
                        fields[0].parse().unwrap(),
                        Tick { pass: fields[1].parse().unwrap(), part: fields[2].to_string(), position },
                    )
                })
                .collect();
            Run { starts, shots, kept: kept_bytes, timeline }
        };
        // Read, then remove the dump whatever the reading found: a failed run must not leave
        // its hundreds of megabytes behind either.
        let run = std::panic::catch_unwind(std::panic::AssertUnwindSafe(read));
        let _ = std::fs::remove_dir_all(&dir);
        run.unwrap_or_else(|panic| std::panic::resume_unwind(panic))
    })
}

/// A filed `fullrun` frame's `.rgb`.
fn reference(capture: &str, index: u32) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/fullrun/{capture}/frame_{index:06}.rgb")),
    )
    .unwrap()
}

fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len(), "a short read is not a comparison");
    a.chunks_exact(3).zip(b.chunks_exact(3)).filter(|(x, y)| x != y).count()
}

/// Whether `index` meets `rule`: it holds and the frame before does not (and, for a change,
/// the frame before that is the one before held).
fn meets(run: &Run, rule: First, index: u32) -> bool {
    let now = run.shot(index);
    let before = run.shot(index - 1);
    let same = |a: &Shot, b: &Shot| a.geometry == b.geometry && a.hash == b.hash;
    match rule {
        First::Anchor => unreachable!("the anchor is the run's first start"),
        First::Start => unreachable!("a start is the part's own"),
        First::Geometry(width, height) => {
            now.geometry == (width, height) && before.geometry != (width, height)
        }
        First::Change => !same(now, before) && same(before, run.shot(index - 2)),
        First::Leaves(capture, at) => {
            let held = hash(&reference(capture, at));
            before.hash == held && now.hash != held
        }
        First::Visible => !now.black && before.black,
        First::Bytes(capture, at) => {
            let wanted = hash(&reference(capture, at));
            now.hash == wanted && before.hash != wanted
        }
    }
}

/// The part (an index into `FULLRUN_SEAMS`) a reference index lies in: the last whose
/// original first frame is at or before it.
fn part_of(reference: u32) -> usize {
    FULLRUN_SEAMS.iter().rposition(|seam| seam.original <= reference).unwrap()
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: a 48,000-tick run; cargo test --release -p secondreality --test fullrun")]
fn the_full_run_plays_every_part_in_the_originals_order() {
    let run = run();
    let names: Vec<&str> = run.starts.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(names, PLAN, "block 1's parts, once each, in the demo's own order");
    let seams: Vec<(&str, u32)> = FULLRUN_SEAMS.iter().map(|seam| (seam.part, seam.start)).collect();
    let starts: Vec<(&str, u32)> =
        run.starts.iter().map(|(index, name)| (name.as_str(), *index)).collect();
    assert_eq!(starts, seams, "Sequence::starts is FULLRUN_SEAMS' start column");

    // The timeline is one line a tick, from the boot's 404 to the run's last frame, all on
    // the first pass, and each part's lines begin at its start.
    let first = *run.timeline.keys().next().unwrap();
    let last = *run.timeline.keys().last().unwrap();
    assert_eq!((first, last), (404, RUN_TO));
    assert_eq!(run.timeline.len() as u32, RUN_TO - 404 + 1, "no tick is missing");
    assert!(run.timeline.values().all(|tick| tick.pass == 1));
    for (index, name) in &run.starts {
        assert_eq!(&run.timeline[index].part, name, "{name} draws its start's frame");
    }
    // The shell's two restarts and its cue wait are in the run: MUSIC1 at order 0 at
    // GLENZ (`MAIN/U2.ASM:813-818`) and MUSIC0 at order 18 at U2E (`:880-885`) — each
    // boundary publishes `0:0` — the new player's ring, which a restart does not carry
    // (`Player::restart_from`, D45) — and PANICEND's picture is up
    // until the cue's exit (`:833-853`), MNTSCRL starting on the next tick.
    // The new player publishes `0:0` until its first row comes through the ring (A55): its
    // first other position, and so the length of that stretch, is pinned per restart.
    for (part, first, at) in [(Part::Glenz, (0, 1), 8_075), (Part::U2e, (18, 1), 34_065)] {
        let seam = FULLRUN_SEAMS[part as usize];
        assert_eq!(run.timeline[&seam.start].position, Some((0, 0)), "{}: the restart", seam.part);
        let (frame, tick) = run
            .timeline
            .range(seam.start..)
            .find(|(_, tick)| tick.position != Some((0, 0)))
            .expect("the new song publishes a row");
        assert_eq!((*frame, tick.position), (at, Some(first)), "{}: the first new row", seam.part);
        println!("{}: 0:0 from {} for {} frames, then {first:?}", seam.part, seam.start, at - seam.start);
    }
    assert_eq!(run.timeline[&CUE_EXIT.1].part, "panicend");
    // It exits on the row the original's does (`second1e`: 42:0 at 16629, 42:1 at 16630):
    // `musplus` comes back positive at order 42 row 1.
    assert_eq!(run.timeline[&(CUE_EXIT.1 - 1)].position, Some((42, 0)));
    assert_eq!(run.timeline[&CUE_EXIT.1].position, Some((42, 1)), "the cue's exit row");
    assert_eq!(CUE_EXIT.1 + 1, FULLRUN_SEAMS[Part::Mntscrl as usize].start, "MNTSCRL next");
    assert_eq!(run.timeline[&RUN_TO].part, "endscrl", "the run is in ENDSCRL at its end");
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: a 48,000-tick run; cargo test --release -p secondreality --test fullrun")]
fn the_seam_table_is_the_ports_own_frames() {
    let run = run();
    let names: Vec<&str> = RULES.iter().map(|(name, _)| *name).collect();
    let seams: Vec<&str> = FULLRUN_SEAMS.iter().map(|seam| seam.part).collect();
    assert_eq!(names, seams, "one rule a part, in FULLRUN_SEAMS' order");

    println!("part        original  port   difference  change");
    let mut previous: Option<i64> = None;
    for (part, (seam, (_, rule))) in FULLRUN_SEAMS.iter().zip(RULES).enumerate() {
        if let First::Anchor = rule {
            assert_eq!((seam.port, seam.start), (404, 404));
        } else if let First::Start = rule {
            assert_eq!(seam.port, seam.start, "{}: its first frame is its start", seam.part);
            assert_eq!(run.timeline[&(seam.port - 1)].part, FULLRUN_SEAMS[part - 1].part);
        } else {
            // The first frame that meets the rule at or after the part's start is the pinned
            // one: none of the frames between them meets it.
            let found = (seam.start..=seam.port).find(|index| meets(run, rule, *index));
            assert_eq!(
                found,
                Some(seam.port),
                "{}: the first frame from its start at {} that is its first by {rule:?}",
                seam.part,
                seam.start
            );
        }
        assert_eq!(run.timeline[&seam.port].part, seam.part, "{}'s frame", seam.part);
        let difference = i64::from(seam.port) - i64::from(seam.original);
        let change = previous.map_or(String::new(), |before| format!("{:+}", difference - before));
        println!(
            "{:<10} {:>8} {:>6} {:>+11} {:>7}",
            seam.part, seam.original, seam.port, difference, change
        );
        previous = Some(difference);
    }
    println!(
        "cue exit   {:>8} {:>6} {:>+11}",
        CUE_EXIT.0,
        CUE_EXIT.1,
        i64::from(CUE_EXIT.1) - i64::from(CUE_EXIT.0)
    );

    // Every `fullrun` pairing `cargo xtask rediff` makes lands in its frame's part in the
    // port's own timeline: a loader frame on the last frame of the part whose screen it is,
    // and the one frame paired across a handover (BEGLOGO's held 8300) in the next part.
    let loader: BTreeSet<u32> = FULLRUN_LOADER_FRAMES.iter().map(|(reference, _)| *reference).collect();
    for entry in TABLE.iter().filter(|entry| entry.capture.starts_with("fullrun/")) {
        for frame in entry.frames {
            let part = part_of(frame.reference);
            let expected = if FULLRUN_ACROSS_HANDOVER.contains(&frame.reference) {
                FULLRUN_SEAMS[part + 1].part
            } else {
                FULLRUN_SEAMS[part].part
            };
            assert_eq!(
                run.timeline[&frame.rendered].part,
                expected,
                "{} {} → {} (loader frame: {})",
                entry.capture,
                frame.reference,
                frame.rendered,
                loader.contains(&frame.reference)
            );
        }
    }

    // The two parts that inherit their predecessor's line offset read their own rows:
    // BEGLOGO's ramp is the original's 7930 byte for byte at 7930, and ENDLOGO's hold —
    // `endlogo`'s 900 — is the original's 38300 at ENDLOGO's start difference.
    for (index, capture, reference_index) in kept() {
        assert_eq!(
            differing_pixels(&run.kept[&index], &reference(capture, reference_index)),
            0,
            "{capture} {reference_index} at the port's {index}"
        );
    }
}

/// `reference/music/<name>/trace.csv` as `(frame, (order, row))`.
fn trace(name: &str) -> Vec<(u32, (u8, u8))> {
    let path = repository_root().join(format!("reference/music/{name}/trace.csv"));
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines = text.lines();
    let header: Vec<&str> = lines.next().unwrap().split(',').collect();
    let column = |wanted: &str| header.iter().position(|name| *name == wanted).unwrap();
    let (frame, ord, row) = (column("frame"), column("ord"), column("row"));
    lines
        .map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            (
                fields[frame].parse().unwrap(),
                (fields[ord].parse().unwrap(), fields[row].parse().unwrap()),
            )
        })
        .collect()
}

/// The port's change to the trace's row change at `(at, position)`: the nearest one from 20
/// frames before to 300 after (the window holds one session of one module: a position recurs
/// only orders later), as an offset from `at`.
fn offset_at(port: &BTreeMap<u32, (u8, u8)>, trace: &str, at: u32, position: (u8, u8)) -> i64 {
    let port_change = (at.saturating_sub(20)..=at + 300)
        .filter(|index| {
            port.get(index) == Some(&position) && port.get(&(index - 1)) != Some(&position)
        })
        .min_by_key(|index| index.abs_diff(at))
        .unwrap_or_else(|| panic!("{trace}: the port never changes to {position:?} near {at}"));
    i64::from(port_change) - i64::from(at)
}

#[test]
#[cfg_attr(debug_assertions, ignore = "release only: a 48,000-tick run; cargo test --release -p secondreality --test fullrun")]
fn the_full_runs_music_is_where_the_trace_holds_it() {
    let run = run();
    let port: BTreeMap<u32, (u8, u8)> = run
        .timeline
        .iter()
        .filter_map(|(index, tick)| tick.position.map(|position| (*index, position)))
        .collect();
    let mut measured = Vec::new();
    for expected in &AGREEMENTS {
        let rows: Vec<(u32, (u8, u8))> =
            trace(expected.trace).into_iter().filter(|(frame, _)| *frame >= 404).collect();
        let changes: Vec<(u32, (u8, u8))> = rows
            .windows(2)
            .filter(|pair| pair[0].1 != pair[1].1)
            .map(|pair| pair[1])
            .collect();
        // The offset. A boot trace (`second1`, whose window opens before the music's start)
        // is the run's own anchor, 0, as `crates/sr-audio/tests/trace.rs` holds it; a
        // window trace's is measured at its first row change.
        let boot = rows.first().is_some_and(|(frame, _)| *frame <= 404);
        let (at, position) = changes[0];
        let offset = if boot { 0 } else { offset_at(&port, expected.trace, at, position) };
        // And at its last row change the run can answer (one whose search window ends by
        // [`RUN_TO`]: `second1f` runs on past the run's end): how far the lag has moved across
        // the window. A boot trace's is measured too — its drift from the anchor.
        let (last_at, last_position) =
            *changes.iter().rev().find(|(frame, _)| frame + 300 <= RUN_TO).unwrap();
        let last_offset = offset_at(&port, expected.trace, last_at, last_position);

        // Every frame of the trace: the port's position at that frame plus the offset must
        // be one the trace publishes within the window.
        let published: BTreeMap<u32, (u8, u8)> = rows.iter().copied().collect();
        let near = |frame: u32, position: (u8, u8), radius: u32| -> Option<u32> {
            (frame.saturating_sub(radius)..=frame + radius)
                .filter(|candidate| published.get(candidate) == Some(&position))
                .map(|candidate| candidate.abs_diff(frame))
                .min()
        };
        let mut frames = 0;
        let mut within = 0;
        let mut worst = 0;
        let mut beyond = 0;
        let mut unshifted = 0;
        for (frame, _) in &rows {
            let Some(mine) = port.get(&((i64::from(*frame) + offset) as u32)) else {
                continue;
            };
            frames += 1;
            match near(*frame, *mine, SEARCH_FRAMES) {
                Some(distance) => {
                    worst = worst.max(distance);
                    if distance <= JITTER_FRAMES {
                        within += 1;
                    }
                }
                None => beyond += 1,
            }
            if port.get(frame).is_some_and(|raw| near(*frame, *raw, JITTER_FRAMES).is_some()) {
                unshifted += 1;
            }
        }
        println!(
            "{}: offset {offset:+} at {at} {position:?}; {within} of {frames} frames within \
             {JITTER_FRAMES} ({:.3} %), worst {worst}, {beyond} past {SEARCH_FRAMES}; with no offset \
             {unshifted} ({:.3} %); offset {last_offset:+} at its last row change ({last_at})",
            expected.trace,
            100.0 * within as f64 / frames as f64,
            100.0 * unshifted as f64 / frames as f64,
        );
        measured.push((expected.trace, offset, frames, within, worst, beyond, unshifted, last_offset));
    }
    let wanted: Vec<_> = AGREEMENTS
        .iter()
        .map(|a| (a.trace, a.offset, a.frames, a.within, a.worst, a.beyond, a.unshifted, a.last_offset))
        .collect();
    assert_eq!(measured, wanted, "the music's agreement with block 1's traces");
}
