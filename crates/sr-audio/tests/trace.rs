//! The port's player against every filed per-frame trace of the original's player state
//! (`reference/music/`, committed, so this runs on a fresh clone). The test walks the tree, so
//! a new trace is covered without an edit, and takes nothing about a trace on trust: the module
//! is identified from its `(ord, pat)` pairs, the anchor and start order are derived from the
//! columns of the recipe's boot trace (or [`BOOTLESS_RECIPES`]), and the coverage is recomputed
//! from the file and checked against the manifest.
//!
//! A window trace (one that begins after the music's start) is simulated from its recipe's
//! anchor and phase-aligned to its session at its first row change, because the boot's phase
//! is session-dependent (D17). A trace after a mid-run restart is simulated through the restart
//! ([`RESTART_TRACES`]).
//!
//! Two sessions of one recipe disagree with each other about which frame some row changes land
//! on (A12), so no player can be frame-identical to every trace. What is asserted:
//!
//! 1. **Content, word by word**: for `order`, `pattern`, `row`, `speed`, `tempo`, `loop` and
//!    `zplus`, the sequence of distinct values the port publishes is the trace's. A one-frame
//!    displacement passes; a skipped or invented row, a lost `A`/`T`, a wrong pattern or a
//!    wrong `zplus` bit does not.
//! 2. **Placement**: every published value is one the trace publishes within
//!    [`JITTER_FRAMES`] of that frame.
//!
//! [`KNOWN_DISAGREEMENTS`] pins the two block-1 traces that differ (A54). The agreeing frames
//! and the worst displacement are printed per trace (`-- --nocapture`).

mod common;

use common::root as workspace;
use sr_audio::{Player, PlayerOptions, Position};
use std::collections::BTreeMap;

/// The frame the music's start lands on in the recipes that restart their player at boot
/// (`SECOND 3`, `5`, `U`), 408 (`music-clock.md` §2). A trace whose window begins at or before
/// it holds the start, and its own columns derive its recipe's anchor; `SECOND 1` has no boot
/// restart and derives 400.
const MUSIC_START_FRAME: i64 = 408;

/// Recipes with no boot trace: the anchor and the order the start seeks, which a window cannot
/// measure. `SECOND 4`'s only trace, `second4`, is the `MAIN/U2.ASM:880-885`
/// `restartmus(0, 18, 45000)` session from the block's first graphics frame; its own columns put
/// the clock's start at 408.2 (`music-clock.md` §2.1).
const BOOTLESS_RECIPES: &[(&str, i64, u8)] = &[
    ("4", MUSIC_START_FRAME, 18),
];

/// Traces whose window begins after a **mid-run** restart: the trace, its module, the frame
/// the restarted clock starts on, the order it seeks, and the session it continues from
/// ([`Before`]; the port plays that session first and hands its state over with
/// `Player::restart_from`, D45).
///
/// - `second3e`: MUSIC0's order-18 restart at U2E (`MAIN/U2.ASM:880-885`), `second4`'s session
///   shifted by 17,442 frames: 408.2 + 17,442 = 17,850 (`music-clock.md` §2.1).
/// - `second1d`, `second1e`: block 1's MUSIC1@0 restart at GLENZ (`MAIN/U2.ASM:813-818`) at
///   8,056, derived by [`block_ones_music1_restart_takes_order_0s_break`].
/// - `second1f`: block 1's MUSIC0@18 restart at U2E, 33,647 frames after `second4`'s:
///   408.2 + 33,647 ≈ 34,055 (`reference-indices.md` §26.6, A53).
const RESTART_TRACES: &[(&str, &str, i64, u8, Before)] = &[
    ("second3e", "MUSIC0", 17_850, 18, Before::Boot),
    ("second1d", "MUSIC1", 8_056, 0, Before::Boot),
    ("second1e", "MUSIC1", 8_056, 0, Before::Boot),
    ("second1f", "MUSIC0", 34_055, 18, Before::Restart("second1d")),
];

/// The session a [`RESTART_TRACES`] row's restart replaces. A mid-song restart keeps the
/// previous module's channel state (D45, `Player::restart_from`), so the port must play the
/// session before it to reach the restart with the state the reference had.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Before {
    /// The trace's own recipe's boot session: its module, anchor and start order as the
    /// recipe's boot trace measures them (`second1` for `SECOND 1`, `second3` for `SECOND 3`).
    Boot,
    /// The session of the named [`RESTART_TRACES`] row, itself played from its own chain.
    Restart(&'static str),
}

/// One session of a chain: the module, the frame its `_zplaysong` is at and the order it seeks.
type Session = (&'static str, i64, u8);

/// One content difference the comparison measured: the word, the index in its sequence of
/// distinct values, the two values there (`None` past a sequence's end), and whether the
/// sequences are identical once that one value is set aside — an extra value in the longer
/// sequence, or a substitution when they are the same length.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct ContentMismatch {
    key: &'static str,
    index: usize,
    trace: Option<u8>,
    port: Option<u8>,
    rest_identical: bool,
}

/// A filed trace whose measured disagreement with the port is pinned, not tolerated: the
/// comparison must find **exactly** these content differences, exactly these frames at which
/// the port publishes an `(order, row)` the trace never publishes, and exactly this worst
/// placement displacement. Better or worse is a failure — a row that starts to agree is a
/// change to the player this table must be edited for, the same as one that stops agreeing.
/// Every trace not listed here is held to the unchanged rule (content identical, no
/// unreachable value, worst displacement within [`JITTER_FRAMES`]).
struct KnownDisagreement {
    trace: &'static str,
    content: &'static [ContentMismatch],
    unreachable: &'static [i64],
    worst_displacement: i64,
}

/// The measured disagreements of block 1's full-run traces (A54, `music-clock.md` §2.3).
const KNOWN_DISAGREEMENTS: &[KnownDisagreement] = &[
    // `second1e` (MUSIC1 orders 4…98): every value sequence matches; placement drifts to 8
    // frames at order 96 row 63 (frame 33,305), one past `JITTER_FRAMES`.
    KnownDisagreement {
        trace: "second1e",
        content: &[],
        unreachable: &[],
        worst_displacement: 8,
    },
    // `second1f` (MUSIC0 orders 18…45): order 26's speed-1 stretch publishes a row every 1.46
    // frames, and the trace's one sample a frame never sees row 26, which the port publishes at
    // 38,376. Placement drifts by up to 12 frames.
    KnownDisagreement {
        trace: "second1f",
        content: &[ContentMismatch {
            key: "row",
            index: 502,
            trace: Some(27),
            port: Some(26),
            rest_identical: true,
        }],
        unreachable: &[38_376],
        worst_displacement: 12,
    },
];

/// Whether two sequences agree once the value at `at` is set aside — a value only one side
/// holds (an extra row in the port's sequence or in the trace's), or a value each holds
/// differently — under the rule every other trace is held to: the common prefix identical,
/// and at most one leftover value at the end (the window-edge tolerance of
/// `the_port_reproduces_every_filed_trace`, which can leave the two sequences the same length
/// even when one of them holds an extra value).
fn identical_but_one<T: PartialEq>(trace: &[T], port: &[T], at: usize) -> bool {
    fn without<T>(items: &[T], skip: Option<usize>) -> Vec<&T> {
        items
            .iter()
            .enumerate()
            .filter(|(i, _)| Some(*i) != skip)
            .map(|(_, item)| item)
            .collect()
    }
    [(None, Some(at)), (Some(at), None), (Some(at), Some(at))]
        .into_iter()
        .any(|(trace_skip, port_skip)| {
            let (trace, port) = (without(trace, trace_skip), without(port, port_skip));
            let common = trace.len().min(port.len());
            trace.len().abs_diff(port.len()) <= 1 && trace[..common] == port[..common]
        })
}

/// The frame rate in millihertz, `sr_engine::tick`'s `HZ_MILLI` (sr-audio does not depend on
/// sr-engine). A tick is `2.5/tempo` seconds, `175215/(tempo × 1000)` frames.
const FRAME_HZ_MILLI: i64 = 70_086;
const TICK_NUMERATOR: i64 = FRAME_HZ_MILLI * 25 / 10; // 2.5 frames' worth of millihertz

/// How far a published value may sit from the frame the trace publishes it at. **Measured**:
/// the worst displacement of any trace not in [`KNOWN_DISAGREEMENTS`] is 7 frames, in
/// `second1`, the exact clock's drift from the reference's measured 48.06 ticks a second
/// (`music-clock.md` §2, §3), still under one 6-tick row (8.76 frames). A12 records it.
const JITTER_FRAMES: i64 = 7;

/// How far a window trace's session may sit from its recipe's boot phase. The measured shifts
/// run from −16 (`hiddenu2`) to +4 (`second3d`) frames; 20 is a guard against aligning a trace
/// that is a different song position, not a measurement.
const PHASE_ALIGN_FRAMES: i64 = 20;

/// One trace, its manifest and its module.
struct Trace {
    name: String,
    module: &'static str,
    states: Vec<Row>,
    /// The manifest's coverage window, half open, and its gap list of inclusive ranges.
    window: [i64; 2],
    missing: Vec<(i64, i64)>,
    /// The first frame this comparison holds the port to: the recipe's anchor for a trace
    /// that covers the music's start, the window's own first frame for a trace filed from a
    /// later stretch of the run (whose rows all lie after the anchor).
    start: i64,
    /// The frame the port's simulation begins at — always the recipe's anchor, because the
    /// player must play the same rows the reference did to reach a later window.
    run_from: i64,
    /// The order the boot's seek plays, read from a trace of the same recipe that holds the
    /// start (the first position it publishes).
    start_order: u8,
    /// Whether this trace's window holds the music's start. A trace filed from a later
    /// window does not hold the seek, so its first order change is a natural one at row 0
    /// rather than the seek's first published row 1.
    boot: bool,
    /// The sessions the port plays before this trace's own, oldest first — empty but for a
    /// [`RESTART_TRACES`] row, whose restart continues from the last of them (D45).
    before: Vec<Session>,
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

/// The manifest's coverage, read as text: the crate has no JSON dependency.
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

/// The manifest's `arguments`, read as text for the same reason `parse_coverage` is: a trace's
/// recipe is the command that produced it, and two traces of one recipe share an anchor.
fn parse_arguments(manifest: &str, name: &str) -> Vec<String> {
    let key = "\"arguments\":";
    let at = manifest
        .find(key)
        .unwrap_or_else(|| panic!("{name}'s manifest has no {key}"));
    let rest = &manifest[at + key.len()..];
    let open = rest
        .find('[')
        .unwrap_or_else(|| panic!("{name}'s args are not a list"));
    let close = open
        + rest[open..]
            .find(']')
            .unwrap_or_else(|| panic!("{name}'s args are not a list"));
    rest[open + 1..close]
        .split(',')
        .map(|piece| piece.trim().trim_matches('"').to_string())
        .filter(|piece| !piece.is_empty())
        .collect()
}

/// A trace as read from the tree, before its recipe's anchor is known.
struct Filed {
    name: String,
    module: &'static str,
    states: Vec<Row>,
    window: [i64; 2],
    missing: Vec<(i64, i64)>,
    /// The command's arguments: the recipe key two traces share.
    arguments: Vec<String>,
}

/// Every filed trace, walked from the tree. An anchor is per recipe, derived from the recipe's
/// boot trace; a window trace's own columns derive only the last tick-counter reset before it.
fn traces() -> Vec<Trace> {
    let root = workspace().join("reference/music");
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    // A floor, not a list: a walk that lost a directory fails, a new trace needs no edit.
    assert!(
        names.len() >= 16,
        "the filed traces: {} under {}, sixteen expected",
        names.len(),
        root.display()
    );

    let filed: Vec<Filed> = names
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
            let arguments = parse_arguments(&manifest, &name);
            let module = identify(&name, &states, window[0]);
            Filed {
                name,
                module,
                states,
                window,
                missing,
                arguments,
            }
        })
        .collect();

    // Each recipe's anchor and start order, from the traces that hold the start. A recipe
    // with two boot sessions (second5 and second5b) must derive the same anchor twice; that
    // is what makes it a property of the boot rather than of a session.
    let mut recipes: BTreeMap<String, (i64, u8, &'static str)> = BTreeMap::new();
    for trace in filed.iter().filter(|trace| trace.window[0] <= MUSIC_START_FRAME) {
        let anchor = start_frame(&trace.states);
        let start_order = trace
            .states
            .iter()
            .find(|row| row.position.row != 0 || row.position.order != 0)
            .map(|row| row.position.order)
            .expect("the first published position");
        let key = trace.arguments.join(",");
        match recipes.get(&key) {
            Some(&(seen, order, _)) => {
                assert_eq!(
                    anchor, seen,
                    "{}: a second boot session of the same recipe derives anchor {anchor}, not \
                     {seen}",
                    trace.name
                );
                assert_eq!(
                    start_order, order,
                    "{}: the same recipe seeks a different order",
                    trace.name
                );
            }
            None => {
                recipes.insert(key, (anchor, start_order, trace.module));
            }
        }
    }
    assert!(!recipes.is_empty(), "no filed trace holds the music's start");

    // A restart row's chain: the sessions before it, oldest first, ending with the one its
    // restart replaces.
    let arguments_of: BTreeMap<String, String> = filed
        .iter()
        .map(|trace| (trace.name.clone(), trace.arguments.join(",")))
        .collect();
    fn chain(
        name: &str,
        arguments_of: &BTreeMap<String, String>,
        recipes: &BTreeMap<String, (i64, u8, &'static str)>,
    ) -> Vec<Session> {
        let &(_, _, _, _, before) = RESTART_TRACES
            .iter()
            .find(|row| row.0 == name)
            .unwrap_or_else(|| panic!("{name} is not a restart row"));
        match before {
            Before::Boot => {
                let key = &arguments_of[name];
                let &(anchor, order, module) = recipes
                    .get(key)
                    .unwrap_or_else(|| panic!("{name}: no boot trace of its recipe ({key})"));
                vec![(module, anchor, order)]
            }
            Before::Restart(previous) => {
                let &(_, module, anchor, order, _) =
                    RESTART_TRACES.iter().find(|row| row.0 == previous).unwrap();
                let mut sessions = chain(previous, arguments_of, recipes);
                sessions.push((module, anchor, order));
                sessions
            }
        }
    }

    filed
        .into_iter()
        .map(|trace| {
            let key = trace.arguments.join(",");
            let before = if RESTART_TRACES.iter().any(|row| row.0 == trace.name) {
                chain(&trace.name, &arguments_of, &recipes)
            } else {
                Vec::new()
            };
            let (module, anchor, start_order) = match RESTART_TRACES
                .iter()
                .find(|(name, ..)| *name == trace.name)
            {
                Some(&(_, module, anchor, start_order, _)) => (module, anchor, start_order),
                None => {
                    // A recipe the tree has no boot trace of: its window traces carry no
                    // seek, so the start order and the simulation's anchor come from the
                    // shell path and the recipes that share it ([`BOOTLESS_RECIPES`]). A
                    // recipe with neither is refused rather than compared from a guessed
                    // start.
                    let (anchor, start_order) = match recipes.get(&key) {
                        Some(&(anchor, order, _)) => (anchor, order),
                        None => {
                            let &(_, anchor, start_order) = BOOTLESS_RECIPES
                                .iter()
                                .find(|(recipe, _, _)| *recipe == key.as_str())
                                .unwrap_or_else(|| {
                                    panic!(
                                        "{}: no filed trace of its recipe ({key}) holds the \
                                         music's start, so the recipe's anchor is unknown",
                                        trace.name
                                    )
                                });
                            (anchor, start_order)
                        }
                    };
                    (trace.module, anchor, start_order)
                }
            };
            let boot = trace.window[0] <= MUSIC_START_FRAME;
            let derived = start_frame(&trace.states);
            if boot {
                assert_eq!(
                    derived, anchor,
                    "{}: the trace's own columns put its recipe's music start at frame \
                     {derived}, not {anchor}",
                    trace.name
                );
            } else {
                assert!(
                    derived <= trace.window[0],
                    "{}: the trace's own columns put the tick counter's last reset at frame \
                     {derived}, after its window starts at {} — a window trace cannot begin \
                     before the reset its columns assume",
                    trace.name,
                    trace.window[0]
                );
            }
            let start = if boot { anchor } else { trace.window[0] };
            let result = Trace {
                name: trace.name,
                module,
                states: trace.states,
                window: trace.window,
                missing: trace.missing,
                start,
                run_from: anchor,
                start_order,
                boot,
                before,
            };
            result.check_coverage();
            result
        })
        .collect()
}

/// Which module a trace is of: the one whose order list agrees with every `(ord, pat)` pair
/// the trace holds from its first position change on.
/// A module's speed is not an identifier (`music-clock.md` §3); its order list is.
fn identify(name: &str, states: &[Row], start: i64) -> &'static str {
    let compared: Vec<&Row> = states.iter().filter(|row| row.frame >= start).collect();
    let first_change = compared
        .iter()
        .position(|row| row.position.row != 0 || row.position.order != 0)
        .unwrap_or(0);
    let mut matches = Vec::new();
    for candidate in ["MUSIC0", "MUSIC1"] {
        let module = common::module(candidate);
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
/// tick-lengths before `F`.
/// For a window trace it is the last reset before the window, not a boot anchor.
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

    /// Every frame the trace holds that the comparison holds the port to — the frames this
    /// comparison is over, gaps and all: a frame the emulator never published stays absent
    /// rather than being renumbered around. A trace that holds the start is compared from its
    /// recipe's anchor; a later window from its own first frame.
    fn compared(&self) -> impl Iterator<Item = &Row> {
        self.states.iter().filter(|row| row.frame >= self.start)
    }
}

/// Drive the port from the recipe's anchor to the trace's last frame, and collect what it
/// published. A window trace's run is shifted by the phase its own first row change measures
/// ([`phase_shift`]): `hiddenu` and `hiddenu2`, adjacent frames of one recipe, read `_np_zframe`
/// 4175 and 4185, so no fixed clock sits on both (D17).
fn run(trace: &Trace) -> BTreeMap<i64, Published> {
    let unshifted = simulate(trace, trace.run_from);
    let shift = phase_shift(trace, &unshifted);
    assert!(
        shift.abs() <= PHASE_ALIGN_FRAMES,
        "{}: the session's phase is {shift} frames from the boot's — past the measured \
         session spread of {PHASE_ALIGN_FRAMES}",
        trace.name
    );
    if !trace.boot {
        println!(
            "  {}: the session's phase is {shift:+} frame(s) from the boot's",
            trace.name
        );
    }
    if shift == 0 {
        unshifted
    } else {
        simulate(trace, trace.run_from + shift)
    }
}

/// The port's published state at every frame from `from` to the trace's last, one poll a
/// frame. Frames before the comparison's start are kept: the phase alignment below looks for
/// a row change there.
///
/// A [`RESTART_TRACES`] row is played through its chain: each earlier session from its own
/// `_zplaysong`, one poll a frame, and at the next session's frame a new player for that module
/// takes the old one's state over (`Player::restart_from`, D45) before it seeks — the trace's
/// own session last, at `from`. Only the trace's own session is collected.
fn simulate(trace: &Trace, from: i64) -> BTreeMap<i64, Published> {
    // A session's `_zplaysong`: a boot onto a fresh player, or a restart from the one before.
    let start = |module: &str, previous: Option<&Player>, order: u8| {
        let mut player = Player::new(common::module(module), PlayerOptions::default());
        match previous {
            Some(old) => player.restart_from(old, order),
            None => player.play_song(order),
        }
        player
    };
    let mut previous: Option<Player> = None;
    for (index, &(module, frame, order)) in trace.before.iter().enumerate() {
        let until = trace.before.get(index + 1).map_or(from, |next| next.1);
        let mut player = start(module, previous.as_ref(), order);
        for _ in frame..until {
            player.poll();
        }
        previous = Some(player);
    }
    let mut player = start(trace.module, previous.as_ref(), trace.start_order);

    let mut out = BTreeMap::new();
    let last = trace.states.last().expect("a last frame").frame;
    let mut frame = from;
    loop {
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

/// The frame offset that puts the port's row boundaries on the session's, measured from the
/// first row change the window publishes: its `(order, row → row)` change is unique within
/// the order, and the nearest port change of the same shape is the same row. Zero for a trace
/// that holds the seek, whose phase the anchor already fixes.
fn phase_shift(trace: &Trace, port: &BTreeMap<i64, Published>) -> i64 {
    if trace.boot {
        return 0;
    }
    let mut previous = trace.states.first().expect("a first sample");
    let mut wanted = None;
    for row in trace.states.iter().skip(1) {
        if row.position.order != previous.position.order || row.position.row != previous.position.row
        {
            wanted = Some((
                row.frame,
                previous.position.order,
                previous.position.row,
                row.position.row,
            ));
            break;
        }
        previous = row;
    }
    let (trace_frame, order, from_row, to_row) =
        wanted.expect("a row change in the window's first sample");
    let mut previous: Option<&Published> = None;
    let mut best: Option<(i64, i64)> = None;
    for (frame, published) in port {
        if let Some(before) = previous {
            if before.position.order == order
                && before.position.row == from_row
                && published.position.order == order
                && published.position.row == to_row
            {
                let distance = (frame - trace_frame).abs();
                if best.is_none_or(|(seen, _)| distance < seen) {
                    best = Some((distance, *frame));
                }
            }
        }
        previous = Some(published);
    }
    let (_, port_frame) = best.expect("the port publishes the trace's first change");
    trace_frame - port_frame
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

        // 1. Content, key by key. Over the interior, one leftover value at the end is
        //    tolerated (a window-edge displacement). A window trace begins on a frame, not a
        //    row boundary, so one leading value is dropped from whichever side is a row ahead
        //    (`second3b` publishes row 19 at 10,200 and the port row 20).
        let known = KNOWN_DISAGREEMENTS
            .iter()
            .find(|known| known.trace == trace.name);
        let mut content_found: Vec<ContentMismatch> = Vec::new();
        let mut content = |name: &'static str, key: fn(&Published, &Row) -> u8| {
            let mut trace_sequence =
                distinct(interior.iter().map(|row| key(&row.published(), row)));
            let mut port_sequence =
                distinct(interior.iter().map(|row| key(&port[&row.frame], row)));
            if !trace.boot && trace_sequence.first() != port_sequence.first() {
                if trace_sequence.get(1) == port_sequence.first() {
                    trace_sequence.remove(0);
                } else if port_sequence.get(1) == trace_sequence.first() {
                    port_sequence.remove(0);
                }
            }
            let common = trace_sequence.len().min(port_sequence.len());
            let first = trace_sequence[..common]
                .iter()
                .zip(&port_sequence[..common])
                .position(|(left, right)| left != right);
            if known.is_some() {
                // A pinned trace: record what differs, and compare it with the table below.
                let lengths_close = trace_sequence.len().abs_diff(port_sequence.len()) <= 1;
                if first.is_some() || !lengths_close {
                    let at = first.unwrap_or(common);
                    content_found.push(ContentMismatch {
                        key: name,
                        index: at,
                        trace: trace_sequence.get(at).copied(),
                        port: port_sequence.get(at).copied(),
                        rest_identical: identical_but_one(&trace_sequence, &port_sequence, at),
                    });
                }
                return;
            }
            assert!(
                trace_sequence.len().abs_diff(port_sequence.len()) <= 1,
                "{}: the {name} sequences differ in length — {} against {}",
                trace.name,
                trace_sequence.len(),
                port_sequence.len()
            );
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

        // 2. Placement, within [`JITTER_FRAMES`], per column pair: (order, row) and
        //    (speed, tempo) separately, because a one-frame displacement can straddle a row
        //    boundary and a speed change at once.
        let frames_of = |key: fn(&Row) -> (u8, u8)| {
            let mut map: BTreeMap<(u8, u8), Vec<i64>> = BTreeMap::new();
            for row in compared.iter() {
                map.entry(key(row)).or_default().push(row.frame);
            }
            map
        };
        let row_frames = frames_of(|row| (row.position.order, row.position.row));
        let tempo_frames = frames_of(|row| (row.position.speed, row.position.tempo));
        // A window's first frame can straddle the row boundary its session crossed just before
        // it, so the port may still publish the row the file begins too late to hold.
        let window_edge = compared.first().map(|row| row.frame);

        let mut agreeing = 0usize;
        let mut worst = 0i64;
        // Where the worst displacement grows, frame by frame: the drift A54 quotes.
        let mut growth: Vec<(i64, i64)> = Vec::new();
        let mut unreachable = Vec::new();
        let mut unreachable_frames: Vec<i64> = Vec::new();
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
                    let leading_row = !trace.boot
                        && Some(row.frame) == window_edge
                        && key == "order/row"
                        && mine.0 == theirs.0
                        && mine.1.checked_add(1) == Some(theirs.1);
                    if leading_row {
                        continue;
                    }
                    unreachable_frames.push(row.frame);
                    unreachable.push(format!(
                        "{}: frame {} the trace publishes {key} {theirs:?}, the port {mine:?} — \
                         a value the trace never publishes",
                        trace.name, row.frame
                    ));
                } else {
                    if displacement > worst {
                        growth.push((displacement, row.frame));
                    }
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
        if let Some(known) = known {
            unreachable_frames.dedup();
            println!(
                "  {}: pinned disagreement — content {:?}, unreachable at {:?}, worst \
                 displacement {worst}, first reached (displacement, frame) {growth:?}",
                trace.name, content_found, unreachable_frames
            );
            if content_found.as_slice() != known.content {
                failures.push(format!(
                    "{}: the content differences are {content_found:?}, not the recorded {:?}",
                    trace.name, known.content
                ));
            }
            if unreachable_frames.as_slice() != known.unreachable {
                failures.push(format!(
                    "{}: the port publishes a value the trace never does at {unreachable_frames:?}, \
                     not the recorded {:?}",
                    trace.name, known.unreachable
                ));
            }
            if worst != known.worst_displacement {
                failures.push(format!(
                    "{}: the worst displacement is {worst} frames, not the recorded {}",
                    trace.name, known.worst_displacement
                ));
            }
            continue;
        }
        failures.extend(unreachable.into_iter().take(3));
        if worst > JITTER_FRAMES {
            failures.push(format!(
                "{}: a published position is {worst} frames from the trace's — past the \
                 recorded inter-session spread of {JITTER_FRAMES}",
                trace.name
            ));
        }
    }

    for known in KNOWN_DISAGREEMENTS {
        assert!(
            traces.iter().any(|trace| trace.name == known.trace),
            "KNOWN_DISAGREEMENTS names {}, which is not a filed trace",
            known.trace
        );
    }
    let mut names: Vec<&str> = KNOWN_DISAGREEMENTS.iter().map(|k| k.trace).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), KNOWN_DISAGREEMENTS.len(), "a trace is pinned twice");
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// **Block 1's `MUSIC1@0` restart plays order 0 for four rows, not a boot's 64** (D45, A38):
/// the restart keeps MUSIC0's channel volumes, so pattern 73's row-3 `C00` is taken
/// (`tests/effects.rs`'s `the_volume_zero_optimisation_stops_a_pattern_break_from_running` has
/// the boot's side). This derives `second1d`'s and `second1e`'s [`RESTART_TRACES`] anchor:
/// `second1d`'s first `1:5` is at 8,102 and the restarted player publishes `1:5` on its 46th
/// poll, so the restart is at **8,056**, between `second1b`'s last MUSIC0 frame (7,959) and
/// `second1d`'s first (8,100), one frame before the port's boundary 8,057.
#[test]
fn block_ones_music1_restart_takes_order_0s_break() {
    // Block 1's MUSIC0 from its anchor 400 to the port's boundary 8,057, then the restart.
    let mut music0 = Player::new(common::module("MUSIC0"), PlayerOptions::default());
    music0.play_song(0);
    for _ in 400..8_057 {
        music0.poll();
    }
    let mut restart = Player::new(common::module("MUSIC1"), PlayerOptions::default());
    restart.restart_from(&music0, 0);
    let zframe = restart.zframe();

    // Order 0's rows and ticks, and the poll each position is first published on.
    let mut first: BTreeMap<(u8, u8), usize> = BTreeMap::new();
    let (mut polls, mut rows, mut last_row) = (0usize, 0, None);
    while restart.engine_position().order == 0 {
        restart.poll();
        let state = restart.state();
        first.entry((state.order, state.row)).or_insert(polls);
        let engine = restart.engine_position();
        if engine.order == 0 && last_row != Some(engine.row) {
            last_row = Some(engine.row);
            rows += 1;
        }
        polls += 1;
        assert!(polls < 1_000, "order 0 never ended");
    }
    assert_eq!((rows, restart.zframe() - zframe), (4, 4 * 3), "order 0 is 4 rows at speed 3");
    while !first.contains_key(&(1, 5)) {
        restart.poll();
        let state = restart.state();
        first.entry((state.order, state.row)).or_insert(polls);
        polls += 1;
    }

    let rows = |name: &str| -> Vec<Row> {
        std::fs::read_to_string(workspace().join(format!("reference/music/{name}/trace.csv")))
            .unwrap()
            .lines()
            .skip(1)
            .map(parse_row)
            .collect()
    };
    let at = rows("second1d")
        .iter()
        .find(|row| (row.position.order, row.position.row) == (1, 5))
        .expect("second1d publishes 1:5")
        .frame;
    let music0_until = rows("second1b").last().unwrap().frame;
    assert_eq!((at, music0_until), (8_102, 7_959));
    let start = at - first[&(1, 5)] as i64;
    println!("second1d's 1:5 at {at}, published on the restart's poll {}: the restart is at {start}", first[&(1, 5)]);
    assert_eq!(first[&(1, 5)], 46, "the restart publishes 1:5 on its 46th poll");
    assert_eq!(start, 8_056);
    assert!(music0_until < start && start < 8_100);
    for name in ["second1d", "second1e"] {
        let row = RESTART_TRACES.iter().find(|row| row.0 == name).unwrap();
        assert_eq!((row.1, row.2, row.3), ("MUSIC1", start, 0), "{name}'s restart row");
    }
}

/// **What the comparison assumes of the traces.** `zinfo` is 0 in every sample, which is
/// why the port returns a constant (D8). And the two sessions of one recipe, `second5` and
/// `second5b`, put eight of their row changes one frame apart, which is why the comparison
/// asserts content and placement rather than frame-for-frame equality (A12).
#[test]
fn the_filed_traces_are_what_the_comparison_assumes() {
    let traces = traces();
    for trace in &traces {
        for row in &trace.states {
            assert_eq!(row.zinfo, 0, "{}: frame {} carries zinfo {}", trace.name, row.frame, row.zinfo);
        }
    }

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

