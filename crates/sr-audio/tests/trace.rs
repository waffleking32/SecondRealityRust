//! The phase's exit criterion, in executable form: the port's player against the per-frame
//! traces of the original's own player state (`reference/music/`).
//!
//! The traces are committed — `reference/` is input-only (`docs/fidelity/music-clock.md` §1
//! and §7) — so this test is unconditional: a fresh clone runs it. It **walks**
//! `reference/music/`, so a later trace is covered without an edit, and it takes nothing
//! about a trace on trust: the module is identified from the trace's own `(ord, pat)` pairs
//! against the two order lists, the start order and anchor are read from the columns of the
//! same recipe's trace that holds the music's start (or, for a recipe the tree has no boot
//! trace of, from the shell's own start path — [`BOOTLESS_RECIPES`]), and the coverage —
//! which frames exist and which are the emulator's own gaps — is recomputed from the trace
//! file and checked against the manifest.
//!
//! **Ten of the sixteen filed traces are windows, not boots.** `second1b`, `second3b` and
//! `hiddenu2` were filed for Phase 4's parts, `second2`, `second3a` and `second3d` for
//! Phase 5's, `second1c`, `second4` and `second3e` for Phase 6's (and `second3e` after a
//! mid-run restart), and `second1d` for Phase 7's block-1 `MUSIC1@0` restart; each begins
//! thousands of frames after the music's start. A window trace's own columns derive the last
//! tick-counter reset before it, not a boot anchor, so the port is
//! simulated from its recipe's anchor — the seek the boot made — and **phase-aligned to the
//! session** at the window's first row change, because the boot's phase is session-dependent
//! (D17) and the two `SECOND U` sessions differ by ten ticks at frames 6499/6500. It is then
//! held to the trace from the window's first frame. That is the only way a later window can be
//! reached by a player that never seeks mid-song. `SECOND 2` has a boot trace now — `second2b`
//! (Phase 7), which measures its recipe's start at 401.1 — and `SECOND 4` still has none, so
//! its seek and anchor come from the shell's own start path and the recipes that share it
//! ([`BOOTLESS_RECIPES`]); `second3e` and `second1d` begin after a **mid-run** restart, which
//! the player cannot reach by playing on, so they are simulated from the restart itself
//! ([`RESTART_TRACES`]).
//!
//! `second1` is a boot trace but on a different recipe: `SECOND 1` has no block restart, so
//! its clock starts eight frames earlier than the restart recipes' 408 (see
//! [`MUSIC_START_FRAME`]).
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

/// The frame the music's start lands on in the recipes that restart their player at boot —
/// `SECOND 3`, `5` and `U`, whose `restartmus` reloads STMIK and so zeroes the tick counter
/// about eight frames after the initial `_zplaysong(0)`. It is measured in four Phase 3
/// sessions and stable to 0.1 frames (`docs/fidelity/music-clock.md` §2; `xtask/tests/music.rs`'s
/// `the_anchor_is_the_same_in_every_filed_session` asserts the same number from the other
/// end).
///
/// It is also the boundary that says whether a trace *holds the start*: a trace whose
/// coverage window begins at or before it does, and its own columns derive the recipe's
/// anchor. `SECOND 1` is the exception that makes the anchor per recipe: block 1 has no
/// restart (its `restartmus` row carries `bx = -1`, "no seek"), so its clock starts at the
/// initial `_zplaysong(0)` instead, and its derived anchor is 400 — measured in
/// `reference/music/second1` (`music-clock.md` §2).
///
/// **An anchor is per recipe, not one number for the tree.** Each trace's own columns are
/// used to *derive* its recipe's anchor, and the derivation is then checked against the
/// recipe's first boot trace: a trace of a different recipe is refused rather than compared
/// from the wrong frame.
const MUSIC_START_FRAME: i64 = 408;

/// Recipes whose only filed trace is a window, and what that trace cannot measure: the order
/// the boot seeks and the anchor the port's simulation starts at. A window's own columns
/// derive the last tick reset before it, never the seek, so a recipe with no boot trace has
/// no filed measurement of either — these two values are the shell source's start path plus
/// the anchor the recipes sharing that path measure.
///
/// **`SECOND 2` no longer needs this table.** Its boot trace `second2b` (Phase 7, 380..3799)
/// holds the block's first frames and measures its own anchor (401.1 by the rows,
/// `music-clock.md` §2), so `second2`'s window (4700..9299) is simulated from that measured
/// start and phase-aligned like every window. The old row here — the shell's `ax = 1, bx = 0`
/// path of `MAIN/U2.ASM:766-775`, `restartmus(1, 0, 48700)`, MUSIC1 at order 0 — is still the
/// source's recipe, but the tree now measures it rather than assuming the restart recipes' 408.
///
/// **`SECOND 4`'s only trace is `second4` (500..4399), and its start is the restart, not the
/// boot's initial play.** A block-4 boot's first show part is U2E, so the shell reaches the
/// `MAIN/U2.ASM:880-885` `restartmus(0, 18, 45000)` at the boot's first graphics frame and
/// the whole window is that restarted MUSIC0 session at order **18**. Its own columns derive
/// the session's clock start at 408.2 — the restart recipes' anchor — so this is the one
/// bootless recipe whose anchor is a filed measurement rather than a shared shell path
/// (`music-clock.md` §2.1).
const BOOTLESS_RECIPES: &[(&str, i64, u8)] = &[
    ("4", MUSIC_START_FRAME, 18),
];

/// Traces whose window begins after a **mid-run** restart, with the session that restart
/// installed: the trace, its module, the frame the restarted clock starts on and the order it
/// seeks.
///
/// `SECOND 3` reaches U2E after JPLOGO and the shell restarts MUSIC0 at order 18 there
/// (`MAIN/U2.ASM:880-885`). `second3e`'s session is `second4`'s — the same restart fired on a
/// block-4 boot — shifted by 17,441–17,442 frames at the order boundaries (17,442 at the
/// 18000/18100 rows this start is measured from), so its clock starts at
/// 408.2 + 17,442 = 17,850 ±1 (`docs/fidelity/music-clock.md` §2.1). A trace after a
/// restart is simulated from the restart, not from its recipe's boot anchor: the player never
/// seeks mid-song, so it could not reach the window at all.
///
/// `SECOND 1` reaches GLENZ between BEGLOGO and GLENZ (`MAIN/U2.ASM:813-818`) and the shell
/// restarts MUSIC1 at order 0 there. A restart keeps the previous module's channel volumes
/// (D45), so MUSIC1's order 0 is **4** rows long here, not a boot's 64
/// ([`block_ones_music1_restart_takes_order_0s_break`]). `second1d`'s window opens at 8,100
/// and its first `1:5` is at 8,102; the restarted player, continuing from block 1's MUSIC0,
/// first publishes `1:5` on its 46th poll, so the restart's `_zplaysong(0)` is at
/// 8,102 − 46 = **8,056** — which that test derives and holds this row to. (Before Task 6b the
/// derivation went through a 64-row order 0 and gave 7,815, while `second1b` still publishes
/// MUSIC0 at 7,959.) The reload hides the restart frame itself from the trace (the wide window
/// refuses, §7), so this is a back-extrapolation from the window's first rows rather than a
/// measured boot frame; the comparison's phase alignment absorbs its sub-frame error.
///
/// **Each row names the session its restart continues from** ([`Before`]): the port plays that
/// session first and hands its state over (`Player::restart_from`), as the shell does. Under
/// that chain `second3e` and `second1f` align at +1 and +0 frames from the rows below, and
/// their every figure is the one a fresh restart gave: MUSIC0's order 18 has no row whose
/// length depends on a carried volume.
///
/// `second1e` and `second1f` are block 1's full run (Phase 9 Task 2,
/// `docs/fidelity/reference-indices.md` §26). `second1e` (8,800…) is the same `MUSIC1@0`
/// session `second1d` is in, so it shares that restart. `second1f` opens after the shell's
/// `MUSIC0@18` restart between JPLOGO and U2E (`MAIN/U2.ASM:880-885`); its order boundaries sit
/// 33,647 frames after `second4`'s at its first rows, so its clock starts at
/// 408.2 + 33,647 ≈ **34,055** — `second3e`'s derivation, which the carry-over does not move.
/// This row is the records' one figure for the restart (`reference-indices.md` §26.6, A53):
/// `--nocapture` prints `second1f`'s phase as +0 from it, and the order-18 offsets
/// 33,644–33,647 bracket it in [34,052, 34,055].
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

/// The measured disagreements of block 1's full-run traces (Phase 9 Task 2,
/// `docs/fidelity/reference-indices.md` §26.6, `docs/fidelity/music-clock.md` §2.3), registered
/// as `docs/fidelity/APPROXIMATIONS.md` A54 by Task 6. Every row must name a filed trace
/// (`every_known_disagreement_names_a_trace_the_walk_found`).
const KNOWN_DISAGREEMENTS: &[KnownDisagreement] = &[
    // **Registered: `docs/fidelity/APPROXIMATIONS.md` A54** (Phase 9 Task 6, which found no
    // seam that explains it). `second1e` (8,800…33,949, MUSIC1 orders 4…98) matches the port in
    // every word's value sequence; its placement drifts with the port early, reaching 8
    // frames at order 96 row 63 (frame 33,305; the comparison prints where each worst is first
    // reached) — one past `JITTER_FRAMES`, over a window four times `second1`'s 5,538 frames.
    // The port's own block-1 run shows the same drift against it: its offset is +5 frames at
    // the window's first row change and −3 at its last (`bins/secondreality/tests/fullrun.rs`'s
    // `last_offset`; Task 6, before the restart carry-over D45, measured 248 and 240).
    KnownDisagreement {
        trace: "second1e",
        content: &[],
        unreachable: &[],
        worst_displacement: 8,
    },
    // **Registered: `docs/fidelity/APPROXIMATIONS.md` A54** (Phase 9 Task 6: a sampling
    // fact and a session's row rate, not a seam). `second1f`
    // (34,105…48,276, MUSIC0 orders 18…45) holds the first speed-1 stretch of any filed trace —
    // order 26, pattern 71, frames 38,328…38,370, a row every 1.46 frames. The trace's one
    // sample a frame publishes row 25 at 38,364 and row 27 at 38,366, so row 26 is never
    // current at a sample; the port, later in phase, publishes order 26 row 26 at frame
    // 38,376. That is the one content difference (index 502 of the row sequence: trace 27,
    // port 26, every other value of every word identical) and the one frame whose value the
    // trace never publishes. Its placement drifts with the port late, by up to 12 frames over
    // orders 18…25 and about 9 after — the session's MUSIC0 rows run 11 frames ahead of
    // `second4`'s over orders 19…25 (`music-clock.md` §2.3).
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

/// The port's logic tick, in millihertz (`crates/sr-engine/src/tick.rs`'s `HZ_MILLI`). A
/// tick is `2.5/tempo` seconds, so it is `175215/(tempo × 1000)` frames long: the conversion
/// the anchor derivation needs, and the only place this file knows the engine's rate.
const FRAME_HZ_MILLI: i64 = 70_086;
const TICK_NUMERATOR: i64 = FRAME_HZ_MILLI * 25 / 10; // 2.5 frames' worth of millihertz

/// How far a published value may be displaced from the frame the trace publishes it at
/// before this test calls it a disagreement. **Measured, not chosen**: the two sessions of one
/// recipe (`second5` and `second5b`) differ by one frame at eight of their row changes, and the
/// port — whose clock is exact — sits between the sessions; the worst displacement any filed
/// trace shows against the port is **7 frames**, in `second1`'s 5,538 compared frames, which is
/// the number this constant is. That 7 is the exact clock's accumulated difference from the
/// reference's own measured tick rate — 48.06 a second at tempo 120 against the port's 48.00
/// (`music-clock.md` §2's `second1` row, §3's rule) — over a window twice as long as the ones
/// Phase 3 filed; it is still under one 6-tick row (8.76 frames) at that trace's speed, so a
/// skipped row is still caught. `docs/fidelity/APPROXIMATIONS.md` A12 records the measurement.
const JITTER_FRAMES: i64 = 7;

/// How far a window trace's session may sit from its recipe's boot phase before this test
/// refuses to align it. The ten window shifts measure **−16 (`hiddenu2`), −1 (`second1d`),
/// +1 (`second1b`, `second3b`, `second1c`, `second4`, `second3e`), +2 (`second2`,
/// `second3a`) and +4 (`second3d`) frames** (printed by
/// `the_port_reproduces_every_filed_trace`): the two `SECOND U` sessions, `hiddenu` (frames
/// 146…6499) and `hiddenu2` (6500…8199), read `_np_zframe` 4175 and 4185 at adjacent frames —
/// ten ticks, about fifteen frames — which is the largest session phase spread the tree
/// measures at a window (`music-clock.md` §2). The bound is **a guard, not a measurement**: 20
/// is the largest measured shift plus a quarter of a speed-6 row; it keeps the alignment from
/// hiding a trace that is a different song position, while the content comparison is what
/// actually refuses one.
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

/// Load a module as the shell does, with MUSIC0's tempo patch (`sr_audio::Song::load`).
fn load(name: &str) -> Module {
    sr_audio::Song::named(name)
        .unwrap_or_else(|| panic!("{name} is not a song"))
        .load(&workspace())
        .unwrap_or_else(|error| panic!("{error}"))
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

/// Every filed trace, walked from the tree rather than listed.
///
/// **An anchor is per recipe.** A trace whose window begins at or before
/// [`MUSIC_START_FRAME`] holds the music's start, and its own columns derive the anchor its
/// recipe booted on (`second5`/`second5b` both derive 408; `second1` derives 400 because
/// block 1 has no restart). A trace filed from a later window does **not** hold the seek:
/// its own columns derive the last tick-counter reset before the window (a part's
/// `_dis_setmframe(0)`), not the boot anchor, so it is simulated from its recipe's anchor —
/// the port must play the rows the reference did to reach that window at all.
fn traces() -> Vec<Trace> {
    let root = workspace().join("reference/music");
    let mut names: Vec<String> = std::fs::read_dir(&root)
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()))
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    // The filed traces are the comparison's *input*, and the phase's headline check is that
    // the port reproduces **all** of them. A walk that silently narrowed to fewer because a
    // directory went missing would still pass every assertion below; the floor is what makes
    // that a failure. It is a floor, not a list, so a later trace is covered with no edit.
    assert!(
        names.len() >= 16,
        "the filed traces: {} under {} — the phase's sixteen are expected",
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
///
/// This is how `cargo xtask music analyse` identifies it, and for the same reason: a module's
/// speed is not an identifier (in `second3`'s window MUSIC1 plays at speed 6 for orders 42…45
/// and at 3 from 47 on, and `second3b` holds speed 6 again at order 86 — `music-clock.md` §3),
/// but its order list is.
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
/// tick-lengths before `F`.
///
/// For a trace that holds the start this is its recipe's anchor and `music analyse` prints
/// the same number, to 0.1 frames. For a trace filed from a later window it is the origin of
/// the *last* reset the counter made before the window — a part's `_dis_setmframe(0)` — and
/// is **not** a boot anchor; that is why a window trace is simulated from its recipe's
/// anchor rather than from this.
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
/// published.
///
/// A trace that holds the start is simulated from the anchor directly — the seek the boot
/// made. A trace filed from a later window is **phase-aligned to its own session** first:
/// the boot's phase is session-dependent (D17), and the windows show it — `hiddenu` and
/// `hiddenu2` are adjacent frames of the same recipe that read `_np_zframe` 4175 and 4185,
/// ten ticks (about fifteen frames) apart. No fixed clock can sit on both, so a window trace's
/// port run is shifted by the offset the trace's own first row change measures, and the
/// content and the within-window placement (within [`JITTER_FRAMES`]) are what is asserted.
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
        let mut player = Player::new(load(module), PlayerOptions::default());
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
        //
        //    A trace filed from a later window begins on a *frame*, not on a row boundary, so
        //    the window's first frame can hold a row on one side that the other side's window
        //    does not: `second3b`'s trace publishes row 19 at frame 10,200 and the port row 20,
        //    with every later value equal. For a window trace (not a boot trace, whose seek
        //    makes both publish the same zeroes) one such leading value is dropped from
        //    whichever sequence is one row ahead before the content is compared; a row skipped
        //    or invented anywhere else still fails.
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
        // The frame the trace file begins on. A window's first frame can straddle the row
        // boundary its session crossed just before the window opened, so the port may still
        // be publishing the row from one frame earlier — a row the trace file, which begins
        // at this frame, cannot hold. That is the same window-edge fact the content
        // comparison drops one leading value for; the placement check must not call it a
        // value the port invented.
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
                    // Only the window's own first frame, only for a trace filed as a
                    // window, and only when the port's value is the row immediately before
                    // the trace's (same order): the previous row of that session, which the
                    // file begins too late to contain.
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
            // Exactly the recorded disagreement: a row that starts to agree fails as surely
            // as one that stops (the table's doc comment).
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

    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}

/// **Block 1's `MUSIC1@0` restart plays order 0 for four rows, not 64** — A38's cause, found
/// by Phase 9 Task 6 and fixed by Task 6b (D45). MUSIC1's order 0 is pattern 73, whose row 0
/// is a note-off on every channel and whose row 3 holds a `C00` on channel 2 and nothing else.
///
/// - **A boot** — a fresh player, as `second2b`'s block-2 start is — has channel 2 at volume 0,
///   so the "volume 0" countdown frees the channel on row 3 before its command is dispatched,
///   and order 0 plays its 64 rows (`second2b` publishes `0:4` at 428 and on).
/// - **A mid-song restart** keeps the channel volumes (`_zplaysong`'s initialiser never writes
///   `avol`: `tests/player.rs`'s `the_song_initialiser_writes_exactly_what_initialise_writes`),
///   and block 1's MUSIC0 hands channel 2 over at 63 (asserted below), so the break is taken.
///   The port's restart is `Player::restart_from` and then `play_song`, and it takes it too:
///   order 0 is 4 rows, 12 ticks.
///
/// **The derivation of `second1d`'s and `second1e`'s [`RESTART_TRACES`] anchor.** `second1d`'s
/// window opens at 8,100 and its first `1:5` is at **8,102**. The port's restarted player
/// first publishes `1:5` on its *k*th poll from the restart (counting the restart's own poll as
/// 0), so the restart's first poll is 8,102 − *k*. With the carried 4-row order 0, *k* is 46
/// and the restart is at **8,056** — inside `(7,959, 8,100)`, between `second1b`'s last MUSIC0
/// frame and `second1d`'s first, and one frame before the port's own BEGLOGO→GLENZ boundary
/// (8,057), whose restarted clock begins four frames after it (D34, `MusicStart::restart`).
/// The 64-row reading this replaces had *k* = 288 and put the restart at 7,814, while
/// `second1b` still publishes MUSIC0. The comparison's phase alignment absorbs the anchor's
/// sub-frame error, as it did before.
#[test]
fn block_ones_music1_restart_takes_order_0s_break() {
    let module = load("MUSIC1");
    assert_eq!(module.orders()[0], 73, "order 0 is pattern 73");
    let pattern = module.pattern(73).expect("pattern 73");
    assert!(
        pattern.rows[0].iter().filter(|cell| !cell.is_empty()).all(|cell| cell.note == 0xfe
            && cell.instrument == 0),
        "row 0 is a note-off, with no instrument, on every channel it names"
    );
    let row3: Vec<(usize, u8, u8)> = pattern.rows[3]
        .iter()
        .enumerate()
        .filter(|(_, cell)| !cell.is_empty())
        .map(|(channel, cell)| (channel, cell.command, cell.info))
        .collect();
    assert_eq!(row3, [(2, 3, 0)], "row 3 is `C00` on channel 2 and nothing else");

    // What MUSIC0 hands over: block 1's MUSIC0 from order 0 at frame 400 (`second1`'s anchor,
    // the port's `BLOCK_ONE_MUSIC_START_FRAME`), one poll a frame, up to the port's boundary at
    // 8,057. Channel 2 is sounding there, at full volume.
    let mut music0 = Player::new(load("MUSIC0"), PlayerOptions::default());
    music0.play_song(0);
    let mut volumes = Vec::new();
    for frame in 400..8_057 {
        music0.poll();
        if frame >= 8_052 {
            volumes.push(music0.channel(2).unwrap().volume);
        }
    }
    println!("MUSIC0's channel 2 at 8052..=8056: volume {volumes:?}");
    assert_eq!(volumes, [63; 5]);

    // Order 0, played from a player its `_zplaysong(0)` has just sought: how many rows, ticks
    // and polls it lasts, and the poll each position is first published on.
    let order_zero = |player: &mut Player| {
        let mut polls = 0usize;
        let mut first: BTreeMap<(u8, u8), usize> = BTreeMap::new();
        let mut rows = 0;
        let mut last_row = None;
        while player.engine_position().order == 0 {
            player.poll();
            let state = player.state();
            first.entry((state.order, state.row)).or_insert(polls);
            if player.engine_position().order == 0 && last_row != Some(player.engine_position().row) {
                last_row = Some(player.engine_position().row);
                rows += 1;
            }
            polls += 1;
            assert!(polls < 1_000, "order 0 never ended");
        }
        assert_eq!(player.engine_position().row, 0, "order 1 begins at row 0");
        let ticks = player.zframe();
        while !first.contains_key(&(1, 5)) {
            player.poll();
            let state = player.state();
            first.entry((state.order, state.row)).or_insert(polls);
            polls += 1;
        }
        (rows, ticks, first)
    };

    // A boot: 64 rows of speed 3, channel 2 freed on row 3.
    let mut boot = Player::new(load("MUSIC1"), PlayerOptions::default());
    boot.play_song(0);
    let (rows, ticks, fresh) = order_zero(&mut boot);
    assert_eq!((rows, ticks), (64, 64 * 3), "a boot plays order 0's 64 rows");

    // The port's restart: the same module, continuing from MUSIC0's state.
    let mut restart = Player::new(load("MUSIC1"), PlayerOptions::default());
    restart.restart_from(&music0, 0);
    let zframe = restart.zframe();
    let (rows, ticks, carried) = order_zero(&mut restart);
    assert_eq!(
        (rows, ticks - zframe),
        (4, 4 * 3),
        "the restart takes row 3's `C00`: order 0 is 4 rows"
    );
    println!(
        "the restart publishes 1:0 on poll {} and 1:5 on poll {}; a boot 1:5 on poll {}",
        carried[&(1, 0)],
        carried[&(1, 5)],
        fresh[&(1, 5)]
    );

    // `second1d`'s first `1:5`, and the last MUSIC0 frame `second1b` holds.
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
    assert_eq!(identify("second1b", &rows("second1b"), 7_600), "MUSIC0");

    let start_4 = at - carried[&(1, 5)] as i64;
    let start_64 = at - fresh[&(1, 5)] as i64;
    println!("second1d's 1:5 at {at}: the restart's first poll is {start_4} (a 64-row order 0: {start_64})");
    assert_eq!(start_64, 7_814, "the reading this replaces");
    assert!(start_64 < music0_until, "a 64-row order 0 would start while MUSIC0 still plays");
    assert_eq!(carried[&(1, 5)], 46, "the restart publishes 1:5 on its 46th poll");
    assert_eq!(start_4, 8_056, "the restart, one frame before the port's boundary 8,057");
    assert!(
        music0_until < start_4 && start_4 < 8_100,
        "between MUSIC0's last trace frame and second1d's first"
    );
    for name in ["second1d", "second1e"] {
        let row = RESTART_TRACES.iter().find(|row| row.0 == name).unwrap();
        assert_eq!((row.1, row.2, row.3), ("MUSIC1", start_4, 0), "{name}'s restart row");
    }
}

/// A pinned disagreement is a claim about a filed trace, so a row whose trace the walk does
/// not find — renamed, removed, or misspelt — is a row that pins nothing: the comparison
/// above looks each trace up in the table, never the other way round, and would pass with
/// the row silently unused.
#[test]
fn every_known_disagreement_names_a_trace_the_walk_found() {
    let walked: Vec<String> = traces().into_iter().map(|trace| trace.name).collect();
    for known in KNOWN_DISAGREEMENTS {
        assert!(
            walked.iter().any(|name| name == known.trace),
            "KNOWN_DISAGREEMENTS names {}, which is not a filed trace ({walked:?})",
            known.trace
        );
    }
    let mut names: Vec<&str> = KNOWN_DISAGREEMENTS.iter().map(|known| known.trace).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), KNOWN_DISAGREEMENTS.len(), "a trace is pinned twice");
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
        // change in a trace that holds the start is the seek (`_zplaysong`), which enters an
        // order at row 0 *without* publishing it — the position words stay at their stale
        // zeroes until the first row advance — so the seek is where the row is 1, and every
        // later order change is at row 0. A trace filed from a later window does not hold
        // the seek: every order change it can see is a natural one, at row 0.
        let mut last_order = None;
        let mut seen_change = !trace.boot || trace.start_order == 0;
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

