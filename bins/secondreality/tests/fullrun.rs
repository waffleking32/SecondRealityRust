//! The port's `--block 1` against the original's whole `SECOND 1` run (Phase 9 Task 6):
//! part by part (`reference-indices.md` §26.3's boundary table), row by row (block 1's six
//! music traces, `reference/music/second1*`), and frame by frame at the boundaries.
//!
//! **One run answers every test here.** The release binary plays block 1 headless from
//! ALKU's 404 to [`RUN_TO`] — the original's ENDSCRL start plus 2,000 frames, about 48,000
//! ticks — writing its timeline with `--positions` and dumping only the frames the seam
//! table needs. That is ten seconds in release and minutes in debug, so every test is
//! ignored by default and run by
//!
//! ```text
//! cargo test --release -p secondreality --test fullrun -- --ignored --nocapture
//! ```
//!
//! **What is asserted is what was measured, both ways.** The port's first frame of every
//! part is found from its own frames by §26.3's own definition (its own geometry, its own
//! first change to the screen, or its first visible frame after a black gap), and pinned;
//! the original's is §26.3's, for the session the `fullrun` capture of that part was filed
//! in. The differences are the seam table of §26.8 and `APPROXIMATIONS.md` A53. The music is
//! compared at every frame of every block-1 trace after removing **one** offset per trace —
//! 0 for the boot trace, and for a window trace measured at its first row change (A38's lag
//! in `second1d`/`second1e`, the U2E restart's in `second1f`) — and the agreement within
//! [`JITTER_FRAMES`] is pinned with the offset.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::OnceLock;

/// The original's ENDSCRL begins at 46,277 (§26.3); the run goes 2,000 frames past it.
const RUN_TO: u32 = 46_277 + 2_000;

/// Block 1's plan, `MAIN/U2.ASM:789-912` with `whattorun` 7fh: `exe1` … `exe23`.
const PLAN: [&str; 19] = [
    "alku", "u2a", "pam", "beglogo", "glenz", "tunneli", "techno", "panicend", "mntscrl",
    "lnszoom", "plzpart", "minvball", "rayscrl", "3dsinfld", "jplogo", "u2e", "endlogo",
    "cred", "endscrl",
];

/// Where each part started in the port's run: `Sequence::starts`, as the manifest records
/// it. Measured by this file's first run and pinned: a start that moves is a change to the
/// run that every row of the seam table below inherits.
const PORT_STARTS: [u32; 19] = [
    404, 6008, 7508, 7885, 8057, 11374, 12435, 16423, 16875, 18912, 22096, 26811, 29392,
    31466, 33498, 34286, 38199, 38807, 46430,
];

/// How the port's first frame of a part is found — §26.3's three definitions, and the two
/// rulings for the boundaries §26.3 can only bracket.
#[derive(Clone, Copy, Debug)]
enum First {
    /// The run's first frame: ALKU at the boot's 404.
    Anchor,
    /// The first frame of this geometry: the frame before is another one.
    Geometry(u64, u64),
    /// The first frame that differs from the one before it.
    Change,
    /// The first frame that is not black after a black gap.
    Visible,
    /// The first frame byte-identical to this filed reference frame: U2E's band (34120, the
    /// ruling that aligns U2E on it) and JPLOGO's opening black (33263, the ruling that
    /// aligns JPLOGO on a frame's hash rather than on its bracket).
    Bytes(&'static str, u32),
}

/// One row of the seam table: the part, the original's first frame (§26.3, in the session
/// the part's `fullrun` capture was filed in where §26.3 gives two), the rule, and the
/// port's first frame that rule finds.
struct Seam {
    part: &'static str,
    original: u32,
    rule: First,
    port: u32,
}

const fn seam(part: &'static str, original: u32, rule: First, port: u32) -> Seam {
    Seam { part, original, rule, port }
}

/// §26.3's first frames, and the port's.
///
/// - BEGLOGO 7922: `fullrun/02`'s session still shows PAM's white at 7920 and never
///   publishes 7921 (b2r2's 7922; L1 b's session has 7920).
/// - GLENZ 8522: `fullrun/03`'s own first frame of the collapse (c3r; c2's is 8521).
/// - TUNNELI 11158: `fullrun/04` holds d2's 11160 bytes at 11159, one frame earlier.
/// - 3DSINFLD 31272: h5's 8-pixel frame (`fullrun/08`'s session is still black there).
/// - JPLOGO: §26.3 brackets the first 320×400 frame in (33261, 33263]; it is aligned on
///   `fullrun/09`'s 33263, the black frame JPLOGO opens on — the port's only frame of
///   that hash.
/// - U2E: §26.3 brackets the first 320×200 frame in (34114, 34120]; it is aligned on the
///   band, 34120, which is byte-identical to block 4's (§18.7).
/// - ENDSCRL 46277: l2r's first 640×400 frame.
const SEAMS: [Seam; 19] = [
    seam("alku", 404, First::Anchor, 404),
    seam("u2a", 5961, First::Geometry(320, 200), 6008),
    seam("pam", 7707, First::Bytes("02-u2a-pam", 7707), 7704),
    seam("beglogo", 7922, First::Change, 7885),
    seam("glenz", 8522, First::Change, 8770),
    seam("tunneli", 11158, First::Visible, 11394),
    seam("techno", 12214, First::Visible, 12458),
    seam("panicend", 16193, First::Change, 16423),
    seam("mntscrl", 16637, First::Change, 16875),
    seam("lnszoom", 18656, First::Visible, 18913),
    seam("plzpart", 21927, First::Geometry(320, 400), 22171),
    seam("minvball", 26673, First::Visible, 26912),
    seam("rayscrl", 29174, First::Visible, 29422),
    seam("3dsinfld", 31272, First::Visible, 31531),
    seam("jplogo", 33263, First::Bytes("09-jplogo", 33263), 33498),
    seam("u2e", 34120, First::Bytes("10-u2e", 34120), 34318),
    seam("endlogo", 37967, First::Geometry(320, 400), 38199),
    seam("cred", 38583, First::Geometry(320, 200), 38807),
    seam("endscrl", 46277, First::Geometry(640, 400), 46432),
];

/// The post-PANICEND cue wait's exit (`MAIN/U2.ASM:833-853`): the original's first frame
/// with `musplus` > 0 is 16630 (§26.6); the port's is the wait's last tick, the frame
/// before MNTSCRL starts.
const CUE_EXIT: (u32, u32) = (16_630, 16_874);

/// How far a published position may be from the frame the trace publishes it at — the
/// shell's own window (`src/shell.rs`'s `JITTER_FRAMES`, A12).
const JITTER_FRAMES: u32 = 3;

/// How far the worst-displacement search looks either side of a frame: past it, a
/// position is counted as one the trace never publishes nearby.
const SEARCH_FRAMES: u32 = 60;

/// Per block-1 trace: its name, the offset measured at its first row change (port frame
/// minus trace frame), and — at that offset — how many of its frames the port publishes
/// within [`JITTER_FRAMES`] of the trace, the worst displacement (searched to
/// [`SEARCH_FRAMES`]) and how many it publishes nowhere that near; and how many agree within
/// the window with no offset at all.
struct Agreement {
    trace: &'static str,
    offset: i64,
    frames: usize,
    within: usize,
    worst: u32,
    beyond: usize,
    unshifted: usize,
}

/// Measured by this file's run (Phase 9 Task 6) and recorded in `reference-indices.md`
/// §26.8 and `music-clock.md` §2.4:
///
/// - `second1`, `second1c`, `second1b` — block 1 up to BEGLOGO, under the boot's MUSIC0:
///   the run's own clock, 97.4 / 100 / 99.7 % within the window at offset 0 / −1 / −1, as
///   `crates/sr-audio/tests/trace.rs` and the shell's own block-1 test hold them (`second1`'s
///   worst 7 is A12's; `second1b`'s one frame past the search is its last, where the port is
///   a row ahead of the window's edge).
/// - `second1d`, `second1e` — the `MUSIC1@0` session from GLENZ to JPLOGO: **247 and 248
///   frames late** (A38's lag, which every part of the session inherits), and within the
///   window at that offset 100 % and 69.4 % — `second1e`'s worst 8 is the drift the trace
///   test pins (A54).
/// - `second1f` — the `MUSIC0@18` session from U2E to ENDSCRL: **235 frames late**, 44.4 %
///   within the window, worst 12, and the one frame (38,376) whose position the trace never
///   publishes: the speed-1 row (A54).
///
/// With no offset at all, no frame of the three post-GLENZ traces agrees.
const AGREEMENTS: [Agreement; 6] = [
    Agreement { trace: "second1", offset: 0, frames: 5541, within: 5399, worst: 7, beyond: 0, unshifted: 5399 },
    Agreement { trace: "second1c", offset: -1, frames: 1699, within: 1699, worst: 1, beyond: 0, unshifted: 1698 },
    Agreement { trace: "second1b", offset: -1, frames: 359, within: 358, worst: 1, beyond: 1, unshifted: 357 },
    Agreement { trace: "second1d", offset: 247, frames: 699, within: 699, worst: 2, beyond: 0, unshifted: 0 },
    Agreement { trace: "second1e", offset: 248, frames: 25_143, within: 17_439, worst: 8, beyond: 0, unshifted: 0 },
    Agreement { trace: "second1f", offset: 235, frames: 13_934, within: 6187, worst: 12, beyond: 1, unshifted: 0 },
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

/// The run every test reads.
struct Run {
    dir: PathBuf,
    manifest: serde_json::Value,
    timeline: BTreeMap<u32, Tick>,
}

impl Run {
    fn starts(&self) -> Vec<(u32, String)> {
        self.manifest["starts"]
            .as_array()
            .expect("the manifest lists the run's starts")
            .iter()
            .map(|start| {
                (
                    start["index"].as_u64().unwrap() as u32,
                    start["part"].as_str().unwrap().to_string(),
                )
            })
            .collect()
    }

    fn entry(&self, index: u32) -> &serde_json::Value {
        self.manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|frame| frame["index"] == index)
            .unwrap_or_else(|| panic!("the run dumped no frame {index}"))
    }

    /// The dumped frame's `.rgb` and its geometry.
    fn frame(&self, index: u32) -> (Vec<u8>, (u64, u64)) {
        let entry = self.entry(index);
        let bytes = std::fs::read(self.dir.join(entry["rgb"].as_str().unwrap())).unwrap();
        (bytes, (entry["width"].as_u64().unwrap(), entry["height"].as_u64().unwrap()))
    }
}

/// The indices the seam checks read: each pinned first frame and the two before it.
fn seam_frames() -> Vec<u32> {
    let mut indices: Vec<u32> = SEAMS
        .iter()
        .filter(|seam| !matches!(seam.rule, First::Anchor))
        .flat_map(|seam| [seam.port - 2, seam.port - 1, seam.port])
        .collect();
    // The two frames the parts' line-offset fix is read at (BEGLOGO's ramp and ENDLOGO's
    // hold), and the run's last.
    indices.extend([7930, 38_532, RUN_TO]);
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
        let list = seam_frames().iter().map(u32::to_string).collect::<Vec<_>>().join(",");
        let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
            .current_dir(repository_root())
            .args(["--block", "1", "--headless", "--dump-frames", &list, "--dump-dir"])
            .arg(dir.join("frames"))
            .arg("--positions")
            .arg(&positions)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let manifest = serde_json::from_str(
            &std::fs::read_to_string(dir.join("frames/manifest.json")).unwrap(),
        )
        .unwrap();
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
        Run { dir: dir.join("frames"), manifest, timeline }
    })
}

/// A filed `fullrun` frame's `.rgb`.
fn reference(capture: &str, index: u32) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/fullrun/{capture}/frame_{index:06}.rgb")),
    )
    .unwrap()
}

fn black(rgb: &[u8]) -> bool {
    rgb.iter().all(|byte| *byte == 0)
}

fn differing_pixels(a: &[u8], b: &[u8]) -> usize {
    assert_eq!(a.len(), b.len(), "a short read is not a comparison");
    a.chunks_exact(3).zip(b.chunks_exact(3)).filter(|(x, y)| x != y).count()
}

/// Whether `index` is the first frame `rule` names: it holds and the frame before does not
/// (and, for a change, the frame before that is the one before held).
fn is_first(run: &Run, rule: First, index: u32) -> bool {
    let (now, now_geometry) = run.frame(index);
    let (before, before_geometry) = run.frame(index - 1);
    match rule {
        First::Anchor => unreachable!("the anchor is the run's first start"),
        First::Geometry(width, height) => {
            now_geometry == (width, height) && before_geometry != (width, height)
        }
        First::Change => {
            let (earlier, earlier_geometry) = run.frame(index - 2);
            (now_geometry != before_geometry || now != before)
                && earlier_geometry == before_geometry
                && earlier == before
        }
        First::Visible => !black(&now) && black(&before),
        First::Bytes(capture, at) => {
            let wanted = reference(capture, at);
            now == wanted && before != wanted
        }
    }
}

#[test]
#[ignore = "release only: a 48,000-tick run; cargo test --release -p secondreality --test fullrun -- --ignored"]
fn the_full_run_plays_every_part_in_the_originals_order() {
    let run = run();
    let starts = run.starts();
    let names: Vec<&str> = starts.iter().map(|(_, name)| name.as_str()).collect();
    assert_eq!(names, PLAN, "block 1's parts, once each, in the demo's own order");
    let indices: Vec<u32> = starts.iter().map(|(index, _)| *index).collect();
    assert_eq!(indices, PORT_STARTS);

    // The timeline is one line a tick, from the boot's 404 to the run's last frame, all on
    // the first pass, and each part's lines begin at its start.
    let first = *run.timeline.keys().next().unwrap();
    let last = *run.timeline.keys().last().unwrap();
    assert_eq!((first, last), (404, RUN_TO));
    assert_eq!(run.timeline.len() as u32, RUN_TO - 404 + 1, "no tick is missing");
    assert!(run.timeline.values().all(|tick| tick.pass == 1));
    for (index, name) in &starts {
        assert_eq!(&run.timeline[index].part, name, "{name} draws its start's frame");
    }
    // The shell's two restarts and its cue wait are in the run: MUSIC1 at order 0 at
    // GLENZ (`MAIN/U2.ASM:813-818`) and MUSIC0 at order 18 at U2E (`:880-885`) — each
    // boundary publishes `0:0`, the fresh player's ring — and PANICEND's picture is up
    // until the cue's exit (`:833-853`), MNTSCRL starting on the next tick.
    for part in ["glenz", "u2e"] {
        let (at, _) = starts.iter().find(|(_, name)| name == part).unwrap();
        assert_eq!(run.timeline[at].position, Some((0, 0)), "{part}: the restart");
    }
    assert_eq!(run.timeline[&CUE_EXIT.1].part, "panicend");
    // It exits on the row the original's does (`second1e`: 42:0 at 16629, 42:1 at 16630):
    // `musplus` comes back positive at order 42 row 1.
    assert_eq!(run.timeline[&(CUE_EXIT.1 - 1)].position, Some((42, 0)));
    assert_eq!(run.timeline[&CUE_EXIT.1].position, Some((42, 1)), "the cue's exit row");
    assert_eq!(CUE_EXIT.1 + 1, PORT_STARTS[8], "MNTSCRL starts the tick after the cue");
    assert_eq!(run.timeline[&(RUN_TO)].part, "endscrl", "the run is in ENDSCRL at its end");
}

#[test]
#[ignore = "release only: a 48,000-tick run; cargo test --release -p secondreality --test fullrun -- --ignored"]
fn the_seam_table_is_the_ports_own_frames() {
    let run = run();
    println!("part        original  port   difference  change");
    let mut previous: Option<i64> = None;
    for seam in &SEAMS {
        if let First::Anchor = seam.rule {
            assert_eq!(seam.port, PORT_STARTS[0]);
        } else {
            assert!(
                is_first(run, seam.rule, seam.port),
                "{}: {} is not the port's first frame by {:?}",
                seam.part,
                seam.port,
                seam.rule
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

    // The two parts that inherit their predecessor's line offset read their own rows:
    // BEGLOGO's ramp is the original's 7930 byte for byte at 7930, and ENDLOGO's hold —
    // `endlogo`'s 900 — is the original's 38300 at ENDLOGO's start difference.
    let (ramp, _) = run.frame(7930);
    assert_eq!(differing_pixels(&ramp, &reference("03-beglogo", 7930)), 0, "BEGLOGO's 7930");
    let (hold, _) = run.frame(38_532);
    assert_eq!(differing_pixels(&hold, &reference("11-endlogo", 38_300)), 0, "ENDLOGO's 38300");
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

#[test]
#[ignore = "release only: a 48,000-tick run; cargo test --release -p secondreality --test fullrun -- --ignored"]
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
        // The offset. A boot trace (`second1`, whose window opens before the music's start)
        // is the run's own anchor, 0, as `crates/sr-audio/tests/trace.rs` holds it; a
        // window trace's is measured at its first row change: the port's change to the
        // same position, the nearest one from 20 frames before to 300 after (the window
        // holds one session of one module: a position recurs only orders later).
        let boot = rows.first().is_some_and(|(frame, _)| *frame <= 404);
        let (at, position) = rows
            .windows(2)
            .find(|pair| pair[0].1 != pair[1].1)
            .map(|pair| pair[1])
            .expect("a row change in the trace");
        let offset = if boot {
            0
        } else {
            let port_change = (at.saturating_sub(20)..=at + 300)
                .filter(|index| {
                    port.get(index) == Some(&position)
                        && port.get(&(index - 1)) != Some(&position)
                })
                .min_by_key(|index| index.abs_diff(at))
                .unwrap_or_else(|| {
                    panic!("{}: the port never changes to {position:?}", expected.trace)
                });
            i64::from(port_change) - i64::from(at)
        };

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
             {unshifted} ({:.3} %)",
            expected.trace,
            100.0 * within as f64 / frames as f64,
            100.0 * unshifted as f64 / frames as f64,
        );
        measured.push((expected.trace, offset, frames, within, worst, beyond, unshifted));
    }
    let wanted: Vec<_> = AGREEMENTS
        .iter()
        .map(|a| (a.trace, a.offset, a.frames, a.within, a.worst, a.beyond, a.unshifted))
        .collect();
    assert_eq!(measured, wanted, "the music's agreement with block 1's traces");
}
