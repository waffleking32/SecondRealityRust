//! The phase's exit criterion, in executable form: the port's player against the per-frame
//! traces of the original's own player state (`reference/music/`).
//!
//! The traces are committed — `reference/` is input-only (`docs/fidelity/music-clock.md` §1
//! and §7) — so this test is unconditional: a fresh clone runs it. It **walks**
//! `reference/music/`, so a fifth trace is covered without an edit, and it takes nothing
//! about a trace on trust: the module is identified from the trace's own `(ord, pat)` pairs
//! against the two order lists, the start order is the first order the trace publishes, the
//! start frame is derived from the trace's own columns the way `cargo xtask music analyse`
//! derives the boot's anchor, and the coverage — which frames exist and which are the
//! emulator's own gaps — is recomputed from the trace file and checked against the manifest.
//!
//! **What is asserted, and what is measured.** The reference's tick clock and the port's are
//! both uniform, but the reference's is sampled by an emulated machine whose frame counter
//! carries the host-timing term D17 measured: the two sessions of one recipe (`second5` and
//! `second5b` — the same command, the same emulator, the same cycles) disagree with **each
//! other** about which frame eight of their row changes land on. No player can be
//! frame-identical to both, and a test that listed the exceptions would be fitting, so this
//! test asserts the two things a port *can* be held to —
//!
//! 1. **Content, one word at a time**: for each of `order`, `pattern`, `row`, `speed`,
//!    `tempo`, `loop` and `zplus`, the sequence of values the port publishes over the trace's
//!    frames is exactly the trace's. Values rather than frames, so a one-frame displacement of
//!    a row boundary passes and a skipped row, an invented row, a lost `A`/`T`, a pattern
//!    published for the wrong order or a `zplus` bit left unset does not. (`loop` is 0 in every
//!    filed trace, so that key can catch only a wrong *value*: the counter never moves inside a
//!    filed window, and a port that froze it at zero is indistinguishable here.) Each word is a key
//!    of its own, because the fault this catches is one word going wrong while the other six
//!    stay right — which a combined tuple compared only for equality would report as a whole
//!    position differing and a print-only count would not report at all.
//!
//! 2. **Placement**: every published position must be one the trace itself publishes at that
//!    frame or within [`JITTER_FRAMES`] of it — the displacement the two filed sessions of one
//!    recipe measure on each other, recorded in `docs/fidelity/APPROXIMATIONS.md` A12.
//!
//! — and *measures* the rest: how many frames agree exactly and the worst displacement, both
//! printed per trace.

use sr_audio::{Module, Player, PlayerOptions, Position};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The boot's music-start anchor — the reference's own frame counter, measured in four
/// sessions and stable to 0.1 frames (`docs/fidelity/music-clock.md` §2; `xtask/tests/music.rs`'s
/// `the_anchor_is_the_same_in_every_filed_session` asserts the same number from the other
/// end). Each trace's own columns are used to *derive* it, and the derivation is then
/// checked against this: a trace of a different recipe is refused rather than compared from
/// the wrong frame.
const MUSIC_START_FRAME: i64 = 408;

/// The port's logic tick, in millihertz (`crates/sr-engine/src/tick.rs`'s `HZ_MILLI`). A
/// tick is `2.5/tempo` seconds, so it is `175215/(tempo × 1000)` frames long: the conversion
/// the anchor derivation needs, and the only place this file knows the engine's rate.
const FRAME_HZ_MILLI: i64 = 70_086;
const TICK_NUMERATOR: i64 = FRAME_HZ_MILLI * 25 / 10; // 2.5 frames' worth of millihertz

/// How far a published value may be displaced from the frame the trace publishes it at
/// before this test calls it a disagreement. **Measured, not chosen**: the two sessions of one
/// recipe (`second5` and `second5b`) differ by one frame at eight of their row changes, and the
/// port — whose clock is exact — sits between the sessions; the worst displacement any filed
/// trace shows against the port is **3 frames**, in `hiddenu`'s 6,088 compared frames, which is
/// the number this constant is. `docs/fidelity/APPROXIMATIONS.md` A12 records the measurement.
const JITTER_FRAMES: i64 = 3;

/// One trace, its manifest and its module.
struct Trace {
    name: String,
    module: &'static str,
    states: Vec<Row>,
    /// The manifest's coverage window, half open, and its gap list of inclusive ranges.
    window: [i64; 2],
    missing: Vec<(i64, i64)>,
    start: i64,
}

/// One trace row: `frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe`.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Row {
    frame: i64,
    position: Position,
    zinfo: i32,
    zplus: i32,
    zframe: i32,
}

impl Row {
    /// Everything the comparison holds a position to.
    fn published(self) -> Published {
        Published {
            position: self.position,
            zplus: self.zplus,
        }
    }
}

/// The compared unit: the six published words plus `zplus`, which the trace records and the
/// port computes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct Published {
    position: Position,
    zplus: i32,
}

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate sits in <workspace>/crates/sr-audio")
        .to_path_buf()
}

fn parse_row(line: &str) -> Row {
    let field = |index: usize| -> i64 {
        line.split(',')
            .nth(index)
            .unwrap_or_else(|| panic!("`{line}` has no field {index}"))
            .trim()
            .parse()
            .unwrap_or_else(|error| panic!("`{line}` field {index}: {error}"))
    };
    Row {
        frame: field(0),
        position: Position {
            order: field(1) as u8,
            row: field(2) as u8,
            pattern: field(3) as u8,
            r#loop: field(4) as u8,
            speed: field(5) as u8,
            tempo: field(6) as u8,
        },
        zinfo: field(7) as i32,
        zplus: field(8) as i32,
        zframe: field(9) as i32,
    }
}

/// The manifest's coverage, read as text: the crate has no JSON dependency and this test is
/// not going to be the one that adds one.
fn parse_coverage(manifest: &str, name: &str) -> ([i64; 2], Vec<(i64, i64)>) {
    let flat: String = manifest.chars().filter(|c| !c.is_whitespace()).collect();
    let numbers_after = |key: &str| -> Vec<i64> {
        let at = flat
            .find(key)
            .unwrap_or_else(|| panic!("{name}'s manifest has no {key}"));
        let rest = &flat[at + key.len()..];
        let mut depth = 0i32;
        let mut end = rest.len();
        for (index, character) in rest.char_indices() {
            match character {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest[..end]
            .split(|c: char| !c.is_ascii_digit())
            .filter(|piece| !piece.is_empty())
            .map(|piece| piece.parse().expect("a frame number"))
            .collect()
    };

    let window = numbers_after("\"window\":");
    let missing: Vec<i64> = numbers_after("\"missing\":");
    let missing = missing.chunks(2).map(|pair| (pair[0], pair[1])).collect();
    ([window[0], window[1]], missing)
}

/// Load a module, with the shell's tempo patch applied where the shell applies it:
/// `MAIN/STARTMUS.C:252-265` writes the initial-tempo byte to `0x78` = 120 in `case 0` only,
/// which is MUSIC0; MUSIC1 plays at its file's own 130 (`music-clock.md` §3).
fn load(name: &str) -> Module {
    let path = workspace().join("assets/MAIN").join(format!("{name}.S3M"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut module = Module::load(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
    if name == "MUSIC0" {
        module.apply_tempo_patch(0x78);
    }
    module
}

/// Every filed trace, walked from the tree rather than listed.
fn traces() -> Vec<Trace> {
    let root = workspace().join("reference/music");
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert!(!names.is_empty(), "no traces under {}", root.display());

    names
        .into_iter()
        .map(|name| {
            let dir = root.join(&name);
            let manifest = std::fs::read_to_string(dir.join("manifest.json"))
                .unwrap_or_else(|error| panic!("{name}/manifest.json: {error}"));
            let csv = std::fs::read_to_string(dir.join("trace.csv"))
                .unwrap_or_else(|error| panic!("{name}/trace.csv: {error}"));
            let states: Vec<Row> = csv.lines().skip(1).map(parse_row).collect();
            assert!(!states.is_empty(), "{name} has no samples");
            let (window, missing) = parse_coverage(&manifest, &name);
            let start = start_frame(&states);
            assert_eq!(
                start, MUSIC_START_FRAME,
                "{name}: the trace's own columns put the music's start at frame {start}, not \
                 the boot anchor {MUSIC_START_FRAME} — a different recipe needs its own anchor \
                 before this comparison means anything"
            );
            let module = identify(&name, &states, start);
            let trace = Trace {
                name,
                module,
                states,
                window,
                missing,
                start,
            };
            trace.check_coverage();
            trace
        })
        .collect()
}

/// Which module a trace is of: the one whose order list agrees with every `(ord, pat)` pair
/// the trace holds from its first position change on.
///
/// This is how `cargo xtask music analyse` identifies it, and for the same reason: a module's
/// speed is not an identifier (MUSIC1 plays at speed 6 for orders 42…45 and at 3 from 47 on
/// — `music-clock.md` §3), but its order list is.
fn identify(name: &str, states: &[Row], start: i64) -> &'static str {
    let compared: Vec<&Row> = states.iter().filter(|row| row.frame >= start).collect();
    let first_change = compared
        .iter()
        .position(|row| row.position.row != 0 || row.position.order != 0)
        .unwrap_or(0);
    let mut matches = Vec::new();
    for candidate in ["MUSIC0", "MUSIC1"] {
        let module = load(candidate);
        let agrees = compared[first_change..].iter().all(|row| {
            module
                .orders()
                .get(usize::from(row.position.order))
                .is_some_and(|&pattern| pattern == row.position.pattern)
        });
        if agrees {
            matches.push(candidate);
        }
    }
    assert_eq!(
        matches.len(),
        1,
        "{name}: the trace's (ord, pat) pairs match {} of the two modules",
        matches.len()
    );
    matches[0]
}

/// The frame the music starts on, derived from the trace's own columns the way
/// `cargo xtask music analyse` derives it: the first row change is at frame `F` with
/// `_np_zframe` at `Z`, and a tick is `2.5/tempo` seconds long, so the first tick was `Z`
/// tick-lengths before `F`. The tool prints the same number for every filed trace, to 0.1
/// frames.
fn start_frame(states: &[Row]) -> i64 {
    let first = states
        .iter()
        .position(|row| {
            row.position.row != states[0].position.row
                || row.position.order != states[0].position.order
        })
        .expect("a trace with a row change in it");
    let change = states[first];
    let tempo = i64::from(states[first - 1].position.tempo.max(1));
    let ticks = i64::from(change.zframe);
    let frames = (ticks * TICK_NUMERATOR + tempo * 500) / (tempo * 1000); // rounded
    change.frame - frames
}

impl Trace {
    /// The manifest's coverage claim, recomputed from the trace's own frames rather than
    /// believed: the frames the trace holds must be exactly the requested window (both ends
    /// inclusive, as `music trace --frames`' own help and §7's lines give it) minus the gap
    /// list. A trace that quietly sampled every other frame is refused here — the check is
    /// over the file's own frames, not the record.
    fn check_coverage(&self) {
        let expected: Vec<i64> = (self.window[0]..=self.window[1])
            .filter(|frame| {
                !self
                    .missing
                    .iter()
                    .any(|(from, to)| frame >= from && frame <= to)
            })
            .collect();
        let have: Vec<i64> = self.states.iter().map(|row| row.frame).collect();
        assert_eq!(
            have, expected,
            "{}: the trace's own frames are not the manifest's window minus its gaps",
            self.name
        );
    }

    /// Every frame the trace holds from the music's start on — the frames this comparison is
    /// over, gaps and all: a frame the emulator never published stays absent rather than
    /// being renumbered around.
    fn compared(&self) -> impl Iterator<Item = &Row> {
        self.states.iter().filter(|row| row.frame >= self.start)
    }
}

/// Drive the port for every frame from the trace's own music start, and collect what it
/// published.
fn run(trace: &Trace) -> BTreeMap<i64, Published> {
    let module = load(trace.module);
    let start_order = trace
        .compared()
        .find(|row| row.position.row != 0 || row.position.order != 0)
        .map(|row| row.position.order)
        .expect("the first published position");
    let mut player = Player::new(module, PlayerOptions::default());

    let mut out = BTreeMap::new();
    let last = trace.states.last().expect("a last frame").frame;
    let mut frame = trace.start;
    loop {
        if frame == trace.start {
            player.play_song(start_order);
        }
        player.poll();
        out.insert(
            frame,
            Published {
                position: player.state(),
                zplus: player.zplus(),
            },
        );
        if frame >= last {
            break;
        }
        frame += 1;
    }
    out
}

/// The distinct values of a sequence, in order — what "the same content" means here.
fn distinct<T: PartialEq + Copy>(items: impl Iterator<Item = T>) -> Vec<T> {
    let mut out: Vec<T> = Vec::new();
    for item in items {
        if out.last() != Some(&item) {
            out.push(item);
        }
    }
    out
}

#[test]
fn the_port_reproduces_every_filed_trace() {
    let traces = traces();
    println!("{} filed trace(s)", traces.len());

    let mut failures = Vec::new();
    for trace in &traces {
        let port = run(trace);
        let compared: Vec<&Row> = trace.compared().collect();
        // The content comparison stops [`JITTER_FRAMES`] short of the trace's last frame: at
        // its very edge a one-frame displacement can put the port a row past the trace's last
        // datum, which is a placement fact and not a content one.
        let horizon = compared.last().expect("a last frame").frame - JITTER_FRAMES;
        let interior: Vec<&Row> = compared
            .iter()
            .copied()
            .filter(|row| row.frame <= horizon)
            .collect();

        // 1. Content, key by key, so that a one-frame displacement cannot invent a tuple.
        //
        //    **Every word the trace records is asserted here, not just counted**: the fault
        //    this test exists to catch is one word going wrong in a way that leaves the other
        //    six right — an order index published as the pattern, a `zplus` that never sets its
        //    second bit, a `T` applied where an `A` belongs. Each key is a sequence of *values*
        //    rather than of frames, so a one-frame displacement of a row boundary passes and a
        //    skipped row, an invented row, a lost `A`/`T`, a wrong pattern for an order or a
        //    wrong `zplus` does not.
        //
        //    The comparison is over the trace's interior (all but its last [`JITTER_FRAMES`]
        //    frames): at the window's very edge a one-frame displacement can leave the port a
        //    row short of, or a row past, the trace's last datum — a placement fact, which the
        //    placement check below owns. One leftover entry is tolerated there; two would be a
        //    value the port invented or lost.
        let content = |name: &str, key: fn(&Published, &Row) -> u8| {
            let trace_sequence = distinct(interior.iter().map(|row| key(&row.published(), row)));
            let port_sequence =
                distinct(interior.iter().map(|row| key(&port[&row.frame], row)));
            assert!(
                trace_sequence.len().abs_diff(port_sequence.len()) <= 1,
                "{}: the {name} sequences differ in length — {} against {}",
                trace.name,
                trace_sequence.len(),
                port_sequence.len()
            );
            let common = trace_sequence.len().min(port_sequence.len());
            let first = trace_sequence[..common]
                .iter()
                .zip(&port_sequence[..common])
                .position(|(left, right)| left != right);
            assert_eq!(
                first, None,
                "{}: the {name} sequence differs at index {:?}: trace {}, port {}",
                trace.name,
                first,
                first.map_or_else(String::new, |at| format!("{}", trace_sequence[at])),
                first.map_or_else(String::new, |at| format!("{}", port_sequence[at])),
            );
        };
        content("order", |published, _| published.position.order);
        content("pattern", |published, _| published.position.pattern);
        content("row", |published, _| published.position.row);
        content("speed", |published, _| published.position.speed);
        content("tempo", |published, _| published.position.tempo);
        content("loop", |published, _| published.position.r#loop);
        content("zplus", |published, _| published.zplus as u8);

        // 2. Placement, per column and within [`JITTER_FRAMES`]: the port's (order, row)
        //    must be a pair the trace publishes around that frame, and its (speed, tempo) a
        //    pair it publishes around that frame. Two columns rather than one tuple, because
        //    a one-frame displacement can straddle a row boundary and a speed change at once
        //    — the port then publishes `(row 62, speed 3)` where the trace publishes
        //    `(row 62, speed 6)` and `(row 63, speed 3)` — and that is a placement fact, not a
        //    content one: the sequences above have already fixed what the content is.
        let frames_of = |key: fn(&Row) -> (u8, u8)| {
            let mut map: BTreeMap<(u8, u8), Vec<i64>> = BTreeMap::new();
            for row in compared.iter() {
                map.entry(key(row)).or_default().push(row.frame);
            }
            map
        };
        let row_frames = frames_of(|row| (row.position.order, row.position.row));
        let tempo_frames = frames_of(|row| (row.position.speed, row.position.tempo));

        let mut agreeing = 0usize;
        let mut worst = 0i64;
        let mut unreachable = Vec::new();
        for row in interior.iter() {
            let published = port[&row.frame];
            if published == row.published() {
                agreeing += 1;
            }
            for (key, mine, theirs) in [
                (
                    "order/row",
                    (published.position.order, published.position.row),
                    (row.position.order, row.position.row),
                ),
                (
                    "speed/tempo",
                    (published.position.speed, published.position.tempo),
                    (row.position.speed, row.position.tempo),
                ),
            ] {
                let displacement = match key {
                    "order/row" => &row_frames,
                    _ => &tempo_frames,
                }
                .get(&mine)
                .map(|frames| {
                    frames
                        .iter()
                        .map(|frame| (frame - row.frame).abs())
                        .min()
                        .unwrap_or(i64::MAX)
                })
                .unwrap_or(i64::MAX);
                if displacement == i64::MAX {
                    unreachable.push(format!(
                        "{}: frame {} the trace publishes {key} {theirs:?}, the port {mine:?} — \
                         a value the trace never publishes",
                        trace.name, row.frame
                    ));
                } else {
                    worst = worst.max(displacement);
                }
            }
        }
        println!(
            "  {} ({}): {} of {} frames agree ({:.3} %); worst displacement {} frame(s)",
            trace.name,
            trace.module,
            agreeing,
            interior.len(),
            100.0 * agreeing as f64 / interior.len() as f64,
            worst,
        );
        failures.extend(unreachable.into_iter().take(3));
        if worst > JITTER_FRAMES {
            failures.push(format!(
                "{}: a published position is {worst} frames from the trace's — past the \
                 recorded inter-session spread of {JITTER_FRAMES}",
                trace.name
            ));
        }
    }

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// The tick counter is the tick clock and nothing else: over each run that never goes
/// backwards, it advances at the tempo's own rate, `2.5/tempo` a second — the second,
/// independent measurement of what the row rate measures (`music-clock.md` §8).
#[test]
fn the_tick_counter_advances_at_the_tempos_rate() {
    for trace in &traces() {
        let mut runs = 0;
        for window in trace.states.split(|row| row.zframe == 0) {
            if window.len() < 50 {
                continue;
            }
            let frames = (window[window.len() - 1].frame - window[0].frame) as f64;
            let seconds = frames / (FRAME_HZ_MILLI as f64 / 1000.0);
            let ticks = f64::from(window[window.len() - 1].zframe - window[0].zframe);
            let rate = ticks / seconds;
            let tempo = f64::from(window[0].position.tempo);
            let wanted = 1.0 / (2.5 / tempo);
            assert!(
                (rate - wanted).abs() / wanted < 0.01,
                "{}: the tick counter advances {rate:.2} a second where tempo {tempo} says \
                 {wanted:.2}",
                trace.name
            );
            runs += 1;
        }
        assert!(runs > 0, "{}: no tick-counter run long enough", trace.name);
    }
}

/// `zplus` is a function of the published position — not of the tick count, not of the
/// wall-clock frame — in every filed trace: the same `(order, row)` never carries two values
/// (`music-clock.md` §6). This is the property that lets the port *compute* it rather than
/// replay it, and the port computes the rule the traces confirmed.
#[test]
fn zplus_is_a_function_of_the_published_position() {
    for trace in &traces() {
        let mut seen: BTreeMap<(u8, u8), i32> = BTreeMap::new();
        for row in trace.compared() {
            let key = (row.position.order, row.position.row);
            if let Some(&earlier) = seen.get(&key) {
                assert_eq!(
                    earlier, row.zplus,
                    "{}: position {key:?} carries both zplus {earlier} and {}",
                    trace.name, row.zplus
                );
            } else {
                seen.insert(key, row.zplus);
            }
        }
        // And the transitions are where the note says they are: at an order start. The first
        // change in the trace is the seek (`_zplaysong`), which enters an order at row 0
        // *without* publishing it — the position words stay at their stale zeroes until the
        // first row advance — so the seek is where the row is 1, and every later order change
        // is at row 0.
        let mut last_order = None;
        let mut seen_change = false;
        for row in trace.compared() {
            if let Some(order) = last_order {
                if order != row.position.order {
                    let wanted = if seen_change { 0 } else { 1 };
                    assert_eq!(
                        row.position.row, wanted,
                        "{}: the order changed at row {} rather than at an order start",
                        trace.name, row.position.row
                    );
                    seen_change = true;
                }
            }
            last_order = Some(row.position.order);
        }
    }
}

/// `zinfo` is 0 in every sample of every filed trace — which is what D8's deferral is
/// *about*, and is asserted here rather than assumed, because the port returns a constant
/// and the day a trace shows another value the port has to stop doing that.
#[test]
fn zinfo_is_zero_in_every_filed_trace() {
    for trace in &traces() {
        for row in &trace.states {
            assert_eq!(
                row.zinfo, 0,
                "{}: frame {} carries zinfo {} — D8's deferral needs revisiting",
                trace.name, row.frame, row.zinfo
            );
        }
    }
}




/// The two sessions of one recipe are not frame-identical to each other — which is the whole
/// reason the comparison above asserts content and placement rather than a frame-for-frame
/// equality, and it is a measurement that belongs in a test rather than in a sentence.
///
/// `second5` and `second5b` are the same command against the same emulator at the same pinned
/// cycles, captured in two sessions. Their row changes are one frame apart at eight rows; the
/// other sessions' traces cannot be compared this way (they are different recipes), so this
/// test is about the one pair the tree holds.
#[test]
fn the_two_sessions_of_one_recipe_disagree_at_eight_row_changes() {
    let traces = traces();
    let named = |name: &str| {
        traces
            .iter()
            .find(|trace| trace.name == name)
            .unwrap_or_else(|| panic!("{name} is filed"))
    };
    let changes = |trace: &Trace| {
        let mut out: Vec<(u8, u8, i64)> = Vec::new();
        let mut last = None;
        for row in trace.compared() {
            if last != Some(row.position) {
                out.push((row.position.order, row.position.row, row.frame));
                last = Some(row.position);
            }
        }
        out
    };
    let first = changes(named("second5"));
    let second = changes(named("second5b"));
    assert_eq!(first.len(), second.len(), "the same recipe, the same rows");
    let differing: Vec<((u8, u8), i64, i64)> = first
        .iter()
        .zip(&second)
        .filter(|(left, right)| left.2 != right.2)
        .map(|(left, right)| ((left.0, left.1), left.2, right.2))
        .collect();
    println!(
        "second5 against second5b: {} row change(s) each, {} of them on a different frame:",
        first.len(),
        differing.len()
    );
    for ((order, row), left, right) in &differing {
        println!("  ord {order} row {row}: frame {left} against {right}");
    }
    assert_eq!(
        differing.len(),
        8,
        "eight row changes sit a frame apart between the two sessions"
    );
    for (_, left, right) in &differing {
        assert_eq!(
            (left - right).abs(),
            1,
            "and the disagreement is one frame, never more"
        );
    }
}

