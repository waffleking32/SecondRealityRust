//! The shell's contract with the outside world: what the binary exits with.
//!
//! The shell's own behaviour — the anchor table, the numbering, the DAC the
//! run starts from — is unit-tested in `src/shell.rs`, where the parts it
//! drives can be stubbed. What is left over is the part only a real process
//! can answer: whether a command line that cannot produce a comparable dump
//! says so, and says so as an *exit code* a script can branch on.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// Exit codes the binary uses. `2` is the command line's own verdict and `1` is
/// a run that failed once it had started; `xtask compare` uses the same two for
/// "could not compare" and "could not be run", which is why a shell test is
/// about the code and not about the message.
const USAGE: i32 = 2;
const OK: i32 = 0;

/// Run the binary the way the repository's own commands do: **from the repository root**,
/// which is where `assets/` is and where a part and the music read their media from
/// (`bins/secondreality/tests/endlogo.rs` does the same, and for the same reason).
fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(args)
        .output()
        .unwrap()
}

/// A dump directory of this test's own, removed first so a stale manifest
/// cannot answer for a run that produced nothing.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("sr-shell-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

fn manifest(dir: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap()
}

#[test]
fn a_sparse_dump_writes_exactly_the_indices_it_was_asked_for() {
    let dir = scratch("sparse");
    let out = run(&[
        "--headless",
        "--part",
        "testcard",
        "--dump-frames",
        "3",
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));

    // Exactly one frame, and it is the one asked for: the alternative — every
    // tick from zero — is the 900-frame dump `--dump-frames` exists to avoid.
    let manifest = manifest(&dir);
    assert_eq!(manifest["frames"].as_array().unwrap().len(), 1);
    assert_eq!(manifest["frames"][0]["index"], 3);
    assert_eq!(manifest["frames"][0]["idx"], "testcard_000003.idx");
    assert!(dir.join("testcard_000003.rgb").is_file());
    assert!(!dir.join("testcard_000002.idx").exists(), "the run dumped more than it was asked");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_dump_manifest_says_which_numbering_its_indices_are_in() {
    let dir = scratch("numbering");
    let out = run(&[
        "--headless",
        "--part",
        "testcard",
        "--dump-frames",
        "0",
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));

    // A run that began at no measured landmark numbers its own frames, and the
    // manifest has to say so: two dumps whose indices look comparable and are
    // not is the failure this field prevents.
    let manifest = manifest(&dir);
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["version"], 1);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_command_line_that_cannot_produce_a_dump_is_refused_before_it_runs() {
    let dir = scratch("refusals");
    let dir = dir.to_str().unwrap().to_string();
    let cases: &[&[&str]] = &[
        // An unknown block, and a rejected one.
        &["--headless", "--block", "7", "--dump-dir", &dir, "--dump-frames", "1"],
        &["--headless", "--block", "0", "--dump-dir", &dir, "--dump-frames", "1"],
        // Two ways of bounding the run at once.
        &["--headless", "--dump-dir", &dir, "--frames", "2", "--dump-frames", "3"],
        &["--headless", "--dump-dir", &dir, "--run-seconds", "1", "--dump-frames", "3"],
        // A dump with no headless run to write it, and a headless run with
        // nowhere to write.
        &["--dump-dir", &dir, "--dump-frames", "3"],
        &["--headless", "--dump-frames", "3"],
        // A list with a hole in it.
        &["--headless", "--dump-dir", &dir, "--dump-frames", "430,,470"],
        // A block starts where the reference says it does, so an index before
        // that is one the run walks away from. Before the block's parts are
        // ported this is refused for a different reason, and after them for
        // this one; both are the command line's answer, which is the contract.
        &["--headless", "--block", "5", "--dump-dir", &dir, "--dump-frames", "100"],
    ];
    for case in cases {
        let out = run(case);
        assert_eq!(
            out.status.code(),
            Some(USAGE),
            "{case:?} exited {}: {}",
            out.status.code().map_or("signal".to_string(), |c| c.to_string()),
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

#[test]
fn the_vault_menu_refuses_a_headless_run() {
    // The menu is the window's own surface: a headless run names its part with
    // `--part` instead. The refusal has to be the vault's, not a missing dump
    // directory — the message says which flag to use instead — and it is the
    // command line's exit code.
    let out = run(&["--vault", "--headless"]);
    assert_eq!(
        out.status.code(),
        Some(USAGE),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("the vault menu needs a window; name a part with --part"),
        "{stderr}"
    );
    // The usage the refusal prints names the flag too.
    assert!(
        stderr.contains("--vault"),
        "the usage does not list --vault: {stderr}"
    );
}

#[test]
fn a_part_anchored_at_a_boot_index_refuses_a_dump_before_it() {
    // PAM's first drawn frame is the reference's 7710 (`reference-indices.md`
    // §6.1), so a run that starts there walks away from 7709 — the same
    // `first_unreachable` rule a block's anchor enforces. The run to check it
    // never has to start.
    let dir = scratch("pam-anchor");
    let out = run(&[
        "--headless",
        "--part",
        "pam",
        "--dump-frames",
        "7709",
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(USAGE),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("7709"),
        "the refusal names the index: {}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_part_the_port_has_runs_from_its_own_numbering() {
    // GLENZ was the last part of the demo's order the port had not reached; Task
    // 6 gives it its MUSIC1 row, so `--part glenz` is a run. It has no measured
    // boot anchor, so its frames are its own — the manifest says so — and its
    // first frames are inside the part's music wait (`MAIN.C:290`: the picture
    // holds while `musplus < -19`). Two ticks there are enough for the smoke
    // test: the run starts, the row installs MUSIC1 at order 0, and the dump
    // numbers itself from zero.
    let dir = scratch("glenz-part");
    let out = run(&[
        "--headless",
        "--part",
        "glenz",
        "--dump-frames",
        "0,1",
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(
        out.status.code(),
        Some(OK),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let manifest = manifest(&dir);
    assert_eq!(manifest["part"], "glenz");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 0, "the run's own numbering");
    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2, "both asked-for frames were dumped");
    for (frame, index) in frames.iter().zip([0u32, 1]) {
        assert_eq!(frame["index"], index);
        assert_eq!(frame["part"], "glenz");
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 400, "the inherited Mode X screen, waiting on the music");
        assert!(
            dir.join(frame["idx"].as_str().unwrap()).is_file(),
            "frame {index} wrote the .idx it names"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// --- the run's music ---------------------------------------------------------

/// The reference index of ENDLOGO's first drawn frame under `SECOND 5`: the boot's first
/// graphics frame, and where a `--block 5` run begins (`shell::boot_anchor`).
const BLOCK5_ANCHOR: u32 = 404;

/// The published position a trace holds at a reference index, or `None` when
/// that session does not cover the frame (a trace's gaps are missing rows, not
/// zero rows).
fn trace_position_opt(name: &str, frame: u32) -> Option<(u8, u8)> {
    let path = repository_root().join("reference/music").join(name).join("trace.csv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    for line in text.lines().skip(1) {
        let mut fields = line.split(',');
        let number = |fields: &mut std::str::Split<'_, char>| -> u32 {
            fields.next().unwrap().trim().parse().unwrap()
        };
        let index = number(&mut fields);
        let order = number(&mut fields);
        let row = number(&mut fields);
        if index == frame {
            return Some((order as u8, row as u8));
        }
    }
    None
}

/// One frame of a filed trace: the published position at a reference index.
///
/// `reference/music/second5` is a `SECOND 5` session — the recipe a `--block 5` run is
/// comparable with — and its `trace.csv` holds a row a frame, exactly the frames the trace
/// *has* (the manifest's gaps are missing rows, not zero rows).
fn trace_position(name: &str, frame: u32) -> (u8, u8) {
    trace_position_opt(name, frame).unwrap_or_else(|| panic!("{name} holds no frame {frame}"))
}

/// The position as the manifest spells it — `order:row`.
fn position_string((order, row): (u8, u8)) -> String {
    format!("{order}:{row}")
}

/// The manifest's `order:row` spelling back into its two numbers.
fn parse_position(text: &str) -> (u8, u8) {
    let (order, row) = text
        .split_once(':')
        .unwrap_or_else(|| panic!("{text:?} is not `order:row`"));
    (order.parse().unwrap(), row.parse().unwrap())
}

/// The `SECOND 3` music traces, which together tile the block's boot timeline:
/// `second3` covers 144..2899, `second3a` 2900..10199, `second3b` 10200..15049,
/// `second3d` 15000..17799 and `second3e` 18000..21999 (the mid-run restart and
/// U2E's opening, D34). `second3e` does not overlap `second3d`'s window; the
/// 17,800..17,999 frames no filed trace holds are the handover.
const BLOCK3_TRACES: &[&str] = &["second3", "second3a", "second3b", "second3d", "second3e"];

/// The published position block 3's traces hold at a boot frame, from the
/// **first** session that covers it.
///
/// The sessions overlap at one seam, 15000..15049 in both `second3b` and
/// `second3d`, and they are different sessions: `second3b` reads `91:60` at
/// 15000 where `second3d` reads `91:59` (zframes 6888/6884), a one-row phase
/// difference. So a frame takes the first trace that holds it rather than
/// being required to agree across the seam — the same rule as
/// [`block1_trace_position`], and for the same reason. Every index the chain
/// test reads is covered by exactly one trace.
fn block3_trace_position(frame: u32) -> (u8, u8) {
    for name in BLOCK3_TRACES {
        if let Some(position) = trace_position_opt(name, frame) {
            return position;
        }
    }
    panic!("no block-3 trace holds boot frame {frame}")
}

/// The published position the `SECOND 1` traces hold at a boot frame, from the
/// first session that covers it: `second1` covers 148..5999, `second1c`
/// 5900..7599 (U2A's gates) and `second1b` 7600..7959 (PAM's opening).
///
/// **These are three different sessions, not one tiled window.** `second1` and
/// `second1c` overlap over 5900..5999 and their published rows differ by one
/// there (the sessions' own sample phase), so a frame takes the first trace
/// that holds it rather than being required to agree across them — the same
/// first-covering rule [`block3_trace_position`] follows for its
/// `second3b`/`second3d` seam, whose comment records the one-row phase at
/// 15000. Every index the chain test reads is covered by exactly one of the
/// three.
fn block1_trace_position(frame: u32) -> (u8, u8) {
    for name in ["second1", "second1c", "second1b"] {
        if let Some(position) = trace_position_opt(name, frame) {
            return position;
        }
    }
    panic!("no block-1 trace holds boot frame {frame}")
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `--block <block>` headless run that dumps the frames asked for, and the manifest it
/// wrote.
fn block_dump(name: &str, block: &str, frames: &str) -> (PathBuf, serde_json::Value) {
    let dir = scratch(name);
    let out = run(&[
        "--headless",
        "--block",
        block,
        "--dump-frames",
        frames,
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));
    let manifest = manifest(&dir);
    (dir, manifest)
}

/// A `--block 5` headless run that dumps the frames asked for, and the manifest it wrote.
fn block5_dump(name: &str, frames: &str) -> (PathBuf, serde_json::Value) {
    block_dump(name, "5", frames)
}

/// One dumped frame's published music position, by its index, as the manifest spells it.
fn frame_position(frames: &[serde_json::Value], index: u32) -> String {
    let frame = frames
        .iter()
        .find(|frame| frame["index"].as_u64() == Some(u64::from(index)))
        .unwrap_or_else(|| panic!("the dump holds no frame {index}"));
    frame["music_position"]
        .as_str()
        .unwrap_or_else(|| panic!("frame {index} has no music position"))
        .to_string()
}

#[test]
fn a_block_five_run_plays_music_one_at_the_order_its_entry_point_names() {
    // The shell's own answer for this run: `SECOND 5`'s restart is `restartmus(0, 25, ...)`
    // (`MAIN/U2.ASM:898-905`), so the run begins on MUSIC0 at order 25 and the manifest says
    // so — the module, the order, and the frame the music's own clock starts on.
    let frames = format!("{BLOCK5_ANCHOR},430,500,900,1180");
    let (dir, manifest) = block5_dump("music", &frames);
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 25);
    assert_eq!(manifest["music"]["start_frame"], 408);

    // And every dumped frame's published position is the trace's at that same reference
    // index: the run's music is where the original's was, frame for frame, or the frames
    // are not the demo's.
    for entry in manifest["frames"].as_array().unwrap() {
        let index = entry["index"].as_u64().unwrap() as u32;
        let (order, row) = trace_position("second5", index);
        assert_eq!(
            entry["music_position"],
            serde_json::json!(format!("{order}:{row}")),
            "frame {index}: the trace holds {order}:{row}"
        );
    }

    // The first drawn frame is four frames before the music's own start, and the trace is
    // explicit about what that holds: the seek publishes nothing, so the position is the
    // ring's priming — `0:0` — until the first row advance comes out of it ten polls later.
    let first = &manifest["frames"][0];
    assert_eq!(first["index"], BLOCK5_ANCHOR);
    assert_eq!(first["music_position"], "0:0");
    assert_eq!(trace_position("second5", BLOCK5_ANCHOR), (0, 0));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering() {
    // `SECOND 1` runs ALKU, U2A and PAM (`MAIN/U2.ASM:790-799`), and ALKU is a
    // block entry measured at the boot's first graphics frame, 404. Its music is
    // the initial `_zplaysong(0)` — MUSIC0 at order 0 (`:766-775`) — whose clock
    // starts at `BLOCK_ONE_MUSIC_START_FRAME` (400); the run draws from 404 and
    // pre-polls the four frames in between, so the player's first run tick is
    // the run's own first frame.
    //
    // **The chain's own handover indices are measured, not fitted.** The port's
    // parts hand over with no loader gap (D31 §3): ALKU's first frame is boot
    // 404 and its last is chain 6,007, so U2A starts at **6,008** — 48 frames
    // after the reference's U2A (`reference-indices.md` §5 puts its first frame
    // at 5,960/5,961). U2A's animation stream then ends before the reference's
    // second gate (`D33`: the stream ends ~7,510 and the reference holds the
    // cleared background until PAM's 7,710), so PAM's part starts at chain
    // **7,508**. What the table below asserts is therefore (part, geometry,
    // music position): the music is still the reference's at the same boot
    // index — including at the gate frame 6,988 and at PAM's measured first
    // drawn frame 7,710 — and a part boundary that moves is recorded as
    // measured, not absorbed into an offset.
    // The first twelve frames are the PAM stretch Task 6 inherits; the last
    // thirteen cross PAM's end into BEGLOGO, BEGLOGO's 172 ticks into GLENZ, the
    // restarted MUSIC1's first order change and GLENZ's music wait to the first
    // zoomer body. 8,523..8,528 is the measured hold→zoom transition (the first
    // four are the held picture).
    let frames = "900,6007,6008,6500,6988,7300,7507,7508,7618,7709,7710,7712,\
                  7884,7885,8055,8056,8057,8086,8087,8523,8524,8525,8526,8527,8528";
    let (dir, manifest) = block_dump("block1", "1", frames);
    assert_eq!(manifest["part"], "alku");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    // **The manifest's music is the player last installed**, and this run
    // restarts it: the entry install was block 1's initial `_zplaysong(0)` —
    // MUSIC0 at order 0 from 400, pre-polled to the run's 404 — and the
    // `MAIN/U2.ASM:813-818` site replaces it before GLENZ with MUSIC1 at order
    // 0, seeked at the boundary 8,057. A block-2 run's manifest names the same
    // module and order at its entry install instead; see
    // `a_block_two_run_reaches_glenz_without_reloading_music_one`.
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 8_057);

    let expected: &[(u32, &str, u16, u16)] = &[
        (900, "alku", 320, 400),
        (6007, "alku", 320, 400),
        (6008, "u2a", 320, 200),
        (6500, "u2a", 320, 200),
        (6988, "u2a", 320, 200),
        (7300, "u2a", 320, 200),
        (7507, "u2a", 320, 200),
        (7508, "pam", 320, 200),
        (7618, "pam", 320, 200),
        (7709, "pam", 320, 200),
        (7710, "pam", 320, 200),
        (7712, "pam", 320, 200),
        // PAM's last frame, and BEGLOGO's first: PAM's 45 four-tick steps end
        // the part on 7,884.
        (7884, "pam", 320, 200),
        (7885, "beglogo", 320, 400),
        // BEGLOGO's 172 ticks from 7,885: last frame 8,056, and GLENZ's first
        // tick on 8,057 under the `:813-818` restart's player.
        (8055, "beglogo", 320, 400),
        (8056, "beglogo", 320, 400),
        (8057, "glenz", 320, 400),
        // MUSIC1's order 0 ends after four rows (below).
        (8086, "glenz", 320, 400),
        (8087, "glenz", 320, 400),
        // The wait's hold, and the first zoomer body at 8,527 (below).
        (8523, "glenz", 320, 400),
        (8524, "glenz", 320, 400),
        (8525, "glenz", 320, 400),
        (8526, "glenz", 320, 400),
        (8527, "glenz", 320, 400),
        (8528, "glenz", 320, 400),
    ];
    let dumped = manifest["frames"].as_array().unwrap();
    assert_eq!(dumped.len(), expected.len(), "every asked-for frame was dumped");
    for (frame, (index, part, width, height)) in dumped.iter().zip(expected) {
        assert_eq!(frame["index"], *index);
        assert_eq!(frame["part"], *part, "chain frame {index}");
        assert_eq!(frame["width"], *width, "chain frame {index}'s width");
        assert_eq!(frame["height"], *height, "chain frame {index}'s height");
    }

    // The music's positions at the boundary, the first gate and PAM's start are
    // the traces' at the same boot index (the reference's own numbering, which
    // the run shares). 6,988 is U2A's first gate (`U2A.C:238`: order > 10 and
    // row > 46; `second1c` reads 11:47); 7,618 is its second, which the
    // reference holds in `second1b` as 12:55 even though the port's U2A has
    // already retired at its stream's end; 7,710 is PAM's measured first drawn
    // frame (`reference-indices.md` §6.1, §6.4).
    for index in [900u32, 6008, 6988, 7508, 7618, 7710, 7884, 7885] {
        let (order, row) = block1_trace_position(index);
        assert_eq!(
            frame_position(dumped, index),
            position_string((order, row)),
            "chain frame {index}: the trace holds {order}:{row}"
        );
    }
    assert_eq!(block1_trace_position(6_988), (11, 47), "U2A's first gate");
    assert_eq!(block1_trace_position(7_618), (12, 55), "U2A's second gate");

    // **The GLENZ boundary's music, measured** (Task 6 Step 5; D38; re-measured by
    // Task 6b after the restart carry-over, D45).
    //
    // The restart seeks MUSIC1 to order 0 row 0 at 8,057, and the new player
    // continues from MUSIC0's channel state: channel 2 is still at 63, so order
    // 0's row-3 `C00` is taken and order 0 lasts **4 rows** — the restart
    // publishes `1:0` at 8,087, 30 ticks after the boundary (D34's four of
    // waiting, then 26 polls: four rows of speed 3 and the ring's ten). A fresh
    // player plays order 0's 64 rows and publishes `0:4` there instead (measured
    // with the shell's `continue_from` call removed).
    assert_eq!(frame_position(dumped, 8_086), "0:3", "order 0's last row");
    assert_eq!(frame_position(dumped, 8_087), "1:0", "order 0 is 4 rows long");
    //
    // The part's wait is `while (dis_musplus() < -19)` (`GLENZ/MAIN.C:290`), and
    // `musplus` is −19 at order 2 row 45 — the very position the reference's
    // own wait exits on (8,521 is `2:45` in `second1d`). The port's published
    // position reaches it at **8,527**, whose frame is the first that differs
    // from the held picture: the zoomer's first body. That is 470 ticks after
    // the boundary, and **6 frames after the reference's measured 8,521** —
    // the restarted clock's lag (`second1d`'s +5 at its first row change, D34's
    // four-tick wait from a boundary one frame after the trace's restart,
    // 8,056) and the frame the published row takes to reach the part. Task 6
    // measured 8,770 and 249 here, before the carry-over.
    let rgb = |index: u32| std::fs::read(dir.join(format!("glenz_{index:06}.rgb"))).unwrap();
    let hold = rgb(8_057);
    for index in [8_523u32, 8_524, 8_525, 8_526] {
        assert_eq!(
            rgb(index),
            hold,
            "frame {index} still holds BEGLOGO's last picture"
        );
    }
    assert_ne!(rgb(8_527), hold, "the zoomer's first body draws over it");
    assert_eq!(frame_position(dumped, 8_526), "2:44", "the wait's last holding tick");
    assert_eq!(frame_position(dumped, 8_527), "2:45", "musplus −19: the wait's exit");
    // The reference's own exit, read from the filed trace, publishes the same
    // position the port's 8,527 answers: the wait's condition is the source's.
    assert_eq!(
        trace_position("second1d", 8_521),
        (2, 45),
        "the reference's measured block-1 wait exit"
    );
    let zoomer_start = 8_527u32;
    assert_eq!(
        zoomer_start - 8_057,
        470,
        "the port's WaitMusic: the boundary to its first zoomer body"
    );
    assert_eq!(
        zoomer_start - 8_521,
        6,
        "the measured difference from the reference's exit (D38's assigned consequence)"
    );

    // The dump names the run's numbering and wrote the files it claims.
    assert!(dir.join("alku_000900.idx").is_file(), "the dump wrote the frame it named");
    assert!(dir.join("alku_000900.rgb").is_file());
    assert!(dir.join("pam_007710.rgb").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_two_run_reaches_glenz_without_reloading_music_one() {
    // `SECOND 2`'s mask is `0x1e`: BEGLOGO first (`MAIN/U2.ASM:801-811`), then
    // GLENZ (`:812-815`). BEGLOGO is 172 ticks from the boot's 404 — 32 vertical
    // blanks, the mode set's tick, the nine frames A5 records as unmodelled,
    // 129 fade steps and the closing write at `BEG/BEG.C:86`
    // (32+1+9+129+1 = 172) — so its last frame is 575 and GLENZ's first is **576**.
    //
    // Block 2's `whattorun & 1` is **clear**, so `:813-814` skips the restart the
    // block-1 run takes; its initial start already sought MUSIC1 to order 0
    // (`:772-775`). The manifest proves the equality check kept the player: the
    // music is still the entry install at 408, not a reload at 576.
    let frames = "574,575,576,577,1100,1116,1117";
    let (dir, manifest) = block_dump("block2-glens", "2", frames);
    assert_eq!(manifest["part"], "beglogo");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(
        manifest["music"]["start_frame"],
        408,
        "the initial start's clock, four ticks after the boot's 404 — not a restart at 576"
    );

    let expected: &[(u32, &str, u16, u16)] = &[
        // BEGLOGO's last two frames.
        (574, "beglogo", 320, 400),
        (575, "beglogo", 320, 400),
        // GLENZ's first ticks: the part's `init` installs the 320×400 view the
        // zoomer closes over (BEGLOGO's own, inherited and re-set), and the
        // music wait holds the picture until the zoomer's first body.
        (576, "glenz", 320, 400),
        (577, "glenz", 320, 400),
        (1100, "glenz", 320, 400),
        (1116, "glenz", 320, 400),
        (1117, "glenz", 320, 400),
    ];
    let dumped = manifest["frames"].as_array().unwrap();
    assert_eq!(dumped.len(), expected.len(), "every asked-for frame was dumped");
    for (frame, (index, part, width, height)) in dumped.iter().zip(expected) {
        assert_eq!(frame["index"], *index);
        assert_eq!(frame["part"], *part, "chain frame {index}");
        assert_eq!(frame["width"], *width, "chain frame {index}'s width");
        assert_eq!(frame["height"], *height, "chain frame {index}'s height");
    }

    // **The wait's measured exit, recorded for Task 7.** The reference's
    // `second2b` holds order 2 row 44 at 1,110 and exits at 1,111 (`2:45`, the
    // `musplus` −19 the wait ends on); the port's position is `2:44` through
    // 1,116 and `2:45` at **1,117**, the first frame whose picture differs from
    // the hold. The six-tick difference is a session-phase difference of A12's
    // class — larger than A12's measured 5-frame worst for `second2b` itself
    // (the table's overall worst is 7, on `second1`), so Task 7 registers it
    // rather than absorbing it, and aligns GLENZ's moving keyframes by this
    // measured offset (`L_p − L_r`), never by a fitted one.
    let rgb = |index: u32| std::fs::read(dir.join(format!("glenz_{index:06}.rgb"))).unwrap();
    let hold = rgb(576);
    assert_eq!(rgb(1_116), hold, "1,116 still holds the picture");
    assert_ne!(rgb(1_117), hold, "the zoomer's first body draws at 1,117");
    assert_eq!(frame_position(dumped, 1_116), "2:44");
    assert_eq!(frame_position(dumped, 1_117), "2:45", "the wait's exit condition");
    // The reference's own rows, read from the filed trace (as the block-1 test
    // reads `second1d`), so a stale trace cannot let these numbers pass.
    assert_eq!(trace_position("second2b", 1_110), (2, 44), "the reference's last holding tick");
    assert_eq!(trace_position("second2b", 1_111), (2, 45), "the reference's measured exit");
    assert_eq!(
        1_117 - 1_111,
        6,
        "the measured difference from the reference's block-2 wait exit"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_three_run_starts_at_mntscrl_with_its_own_numbering() {
    // `SECOND 3`'s entry point is MNTSCRL (`MAIN/U2.ASM:865-866`, guarded `& 4`), and
    // its restart is MUSIC1 at order 42 (`:856-863`). The part's first graphics frame
    // is a measured *window*, not a frame (`reference-indices.md` §11.5), so the run
    // numbers its own frames from zero and the manifest says so rather than wearing the
    // reference's number.
    let (dir, manifest) = block_dump("block3", "3", "1100");
    assert_eq!(manifest["part"], "mntscrl");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 42);
    assert_eq!(manifest["music"]["start_frame"], 0, "part-numbered, so the music starts with it");

    let frame = &manifest["frames"][0];
    assert_eq!(frame["index"], 1100);
    assert_eq!(frame["part"], "mntscrl");
    assert!(dir.join("mntscrl_001100.idx").is_file());
    assert!(dir.join("mntscrl_001100.rgb").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

/// MNTSCRL's tick 0 draws at the reference's boot frame 419: the part's fade-in pins the
/// mapping (`docs/fidelity/mntscrl.md` §2, `reference-indices.md` §11.1). `--block 3` begins
/// there, so chain tick `t` names the reference's boot `419 + t` — the translation that
/// turns a filed keyframe into a chain index.
const MNTSCRL_CHAIN_ORIGIN: u32 = 419;

#[test]
fn a_block_three_run_is_the_whole_chain_and_its_mid_chain_entries_do_not_pre_roll() {
    // **Where the indices come from.** `--block 3` begins at MNTSCRL, whose tick 0 draws at
    // boot 419 (see [`MNTSCRL_CHAIN_ORIGIN`]). The chain's own counter starts there, so a
    // part's filed keyframe at boot `K` is chain tick `K - 419`:
    //
    //   part        keyframe  chain   the note's mapping
    //   MNTSCRL        1,100    681   boot =   419 + tick  (mntscrl.md §2)
    //   LNS&ZOOM       2,700  2,281   boot = 2,452 + tick  (lnszoom.md §7.1)
    //   PLZPART        6,000  5,581   boot = 5,723 + tick  (plzpart.md §2; plasma)
    //   PLZPART        8,500  8,081   boot = 5,723 + tick  (the vect/cube screen)
    //   MINVBALL      10,900 10,481   boot = 10,368 + tick (minvball.md §3)
    //   RAYSCRL       13,000 12,581   boot = 12,962 + tick (rayscrl.md §2)
    //   3DSINFLD      15,500 15,081   boot = 15,036 + tick (3dsinfld.md §6, derived)
    //   JPLOGO        17,300 16,881   boot = 17,065 + tick (jplogo.md §3)
    //
    // Three of those are asked for one to three ticks later — PLZPART 8,082, MINVBALL
    // 10,482, JPLOGO 16,884 — because a port part hands over with no loader gap, so at the
    // same chain index its player is a frame or two off the reference's boot counter. The
    // shifted index is the first tick whose position the trace holds at `index + 419`; it is
    // the same part either way, and the position assertion below is what says so.
    //
    // **The seam.** MINVBALL and RAYSCRL are the two parts reached mid-chain, and the chain
    // is the only run in which a mid-chain start happens. Their `init` pre-rolls (256 and
    // 268 polls) are for `--part` runs; if they fired here the player would arrive already
    // advanced and the trace positions below would not be reachable. The frames at 9,942
    // (PLZPART's last) and 9,943 (MINVBALL's first) are the handoff; 12,537 is inside
    // RAYSCRL's gate and 12,546 is the tick its `musplus >= 0` confirmation opens on.
    //
    // One run to chain 17,721 — tens of seconds in a debug build — and only the listed
    // frames are written; the chain is not played to U2E's 3,600-tick animation, which the
    // part's own integration test owns.
    let (dir, manifest) = block_dump(
        "chain3",
        "3",
        "681,2281,5581,8082,9942,9943,9944,9946,10482,12537,12546,12581,15081,16884,\
         17417,17418,17431,17440,17581,17600,17721",
    );

    // The run's numbering: block 3 begins at a part whose first drawn frame is a window, so
    // the indices are the run's own and the manifest says so. The manifest's music is the
    // player **last installed**: the mid-run restart at the U2E boundary (`MAIN/U2.ASM:880-885`)
    // replaces the entry's MUSIC1@42 with MUSIC0@18, and the entry install is asserted by
    // `a_block_three_run_starts_at_mntscrl_with_its_own_numbering`'s run, which stops long
    // before the boundary.
    assert_eq!(manifest["part"], "mntscrl");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 18);
    assert_eq!(manifest["music"]["start_frame"], 17_418);

    // Every part of the demo's block-3 plan appears, at the chain index above, with the
    // geometry the part's own note records: PLZPART's plasma is 320x400 and its vect screen
    // 320x200, JPLOGO is 320x400, and the rest are mode 13h's 320x200.
    let expected: &[(u32, &str, u16, u16)] = &[
        (681, "mntscrl", 320, 200),
        (2281, "lnszoom", 320, 200),
        (5581, "plzpart", 320, 400),
        (8082, "plzpart", 320, 200),
        (9942, "plzpart", 320, 200),
        (9943, "minvball", 320, 200),
        (9944, "minvball", 320, 200),
        (9946, "minvball", 320, 200),
        (10482, "minvball", 320, 200),
        (12537, "rayscrl", 320, 200),
        (12546, "rayscrl", 320, 200),
        (12581, "rayscrl", 320, 200),
        (15081, "3dsinfld", 320, 200),
        (16884, "jplogo", 320, 400),
        // The U2E boundary. The port's JPLOGO hands over at 17,417/17,418,
        // nine ticks before D34's reference-derived chain 17,427 (the measured
        // seam; see the restart's assertions below).
        (17417, "jplogo", 320, 400),
        (17418, "u2e", 320, 200),
        (17431, "u2e", 320, 200),
        (17440, "u2e", 320, 200),
        (17581, "u2e", 320, 200),
        (17600, "u2e", 320, 200),
        (17721, "u2e", 320, 200),
    ];
    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), expected.len(), "every asked-for frame was dumped");
    for (frame, (index, part, width, height)) in frames.iter().zip(expected) {
        assert_eq!(frame["index"], *index);
        assert_eq!(
            frame["part"], *part,
            "chain frame {index} belongs to {part}, not {}",
            frame["part"]
        );
        assert_eq!(frame["width"], *width, "chain frame {index}'s width");
        assert_eq!(frame["height"], *height, "chain frame {index}'s height");
        assert!(
            dir.join(frame["idx"].as_str().unwrap()).is_file(),
            "chain frame {index} wrote the .idx it names"
        );
        assert!(
            dir.join(frame["rgb"].as_str().unwrap()).is_file(),
            "chain frame {index} wrote the .rgb it names"
        );
    }

    // Each mapped index's published position is the trace's at the reference frame the
    // chain tick names (`index + 419`): the chain's music is the original's, part for part.
    for index in [681u32, 2281, 5581, 8082, 9944, 10482, 12537, 12581, 15081, 16884] {
        let boot = index + MNTSCRL_CHAIN_ORIGIN;
        let (order, row) = block3_trace_position(boot);
        assert_eq!(
            frame_position(frames, index),
            position_string((order, row)),
            "chain frame {index}: the trace at boot {boot} holds {order}:{row}"
        );
    }

    // **MINVBALL's mid-chain entry.** Its pre-roll is 256 polls — what
    // `--part minvball` runs to land the reference's 75:61 at tick 0 — and it must not fire
    // here: the player has run continuously through PLZPART, and a 256-poll jump would be
    // most of an order ahead. The handoff shows the continuous position instead: PLZPART's
    // last frame publishes 75:60 and MINVBALL's first publishes 75:60, one poll later on
    // the same row. The reference's MINVBALL start (boot 10,368) is 75:61, and the run
    // reaches it at chain 9,946 — three ticks in, its 130-tick fade covering the difference
    // between the port's gapless handoff and the reference's loader gap.
    assert_eq!(frame_position(frames, 9942), "75:60", "PLZPART's last frame");
    assert_eq!(frame_position(frames, 9943), "75:60", "MINVBALL's first frame");
    assert_eq!(
        frame_position(frames, 9946),
        position_string(block3_trace_position(10_368)),
        "MINVBALL reaches the trace's start position"
    );
    assert_eq!(block3_trace_position(10_368), (75, 61));

    // **RAYSCRL's mid-chain entry.** Its pre-roll is 268 polls — the span from order 86's
    // row 0 to order 88's row 0, which is the part's own `--part` catch-up. It must not
    // fire here either: at chain 12,537 — still inside RAYSCRL's init gate — the player is
    // at 86:62, the trace's position six frames before the reference's start (`second3b`,
    // boot 12,956), and only at 12,546 does the part's own `musplus >= 0` gate (seven
    // confirming reads) open on 88:0, the trace's position at boot 12,962, the reference's
    // RAYSCRL start. A 268-poll pre-roll on top of 86:62 would advance the player most of a
    // further order, out of order 86 entirely: it could not publish 86:62 at the gate.
    assert_eq!(frame_position(frames, 12537), "86:62", "the seek order, not past it");
    assert_eq!(
        frame_position(frames, 12546),
        position_string(block3_trace_position(12_962)),
        "RAYSCRL's gate opens on the trace's start position"
    );
    assert_eq!(block3_trace_position(12_962), (88, 0));

    // **The mid-run restart at the U2E boundary** (`MAIN/U2.ASM:880-885`, D34).
    // The part switch and its geometry are in the table above; what is left is the
    // restarted player's own clock. The port starts U2E at chain **17,418**, nine
    // ticks before D34's reference-derived chain 17,427: the port's parts hand over
    // with no loader gap (D31 §3), and JPLOGO's end is the seam that accumulates the
    // difference. The number is measured and recorded rather than fitted away.
    //
    // The fresh player is seeked to order 18 at the boundary, waits D34's four
    // engine ticks, and publishes nothing while its ten-deep ring primes — 0:0 at
    // the boundary and at D34's first-tick chain index 17,431 (where the restarted
    // clock's first row advance lands: 17,418 + 4 waits + the row's own 8.76
    // frames), and the first published row 18:1 at 17,440.
    assert_eq!(frame_position(frames, 17_418), "0:0", "the restarted player's ring primes");
    assert_eq!(frame_position(frames, 17_431), "0:0", "still priming at D34's first tick");
    assert_eq!(frame_position(frames, 17_440), "18:1", "the restarted session's first row");
    assert_eq!(manifest["music"]["start_frame"], 17_418, "the boundary the restart was installed at");

    // **The restarted session against `second3e`.** D34 derives `second3e` as
    // `second4`'s session shifted by 17,441–17,442 frames; at the chain's mapping
    // (chain tick 0 = boot 419) a boot frame `B` is chain `B - 419`. The port's
    // nine-tick lead at this seam is under one MUSIC0 row (speed 6, tempo 120:
    // 8.76 frames), so the position at the mapped index is the trace's within one
    // row — and the order, which is what U2E's gate reads, is exact. The port's
    // own rows are pinned here too (D25: `docs/fidelity/u2e.md` §5 quotes them):
    // 18:17 at 17,581 against `second3e`'s 18:16; 18:19 at 17,600 against 18:18;
    // and the order-19 boundary 19:1 at 17,721 against 19:0.
    let restart_positions: &[(u32, &str, (u8, u8))] = &[
        (17_581, "18:17", (18, 16)),
        (17_600, "18:19", (18, 18)),
        (17_721, "19:1", (19, 0)),
    ];
    for &(index, measured, trace) in restart_positions {
        let boot = index + MNTSCRL_CHAIN_ORIGIN;
        assert_eq!(
            frame_position(frames, index),
            measured,
            "chain frame {index}: the port's measured row"
        );
        let (order, row) = block3_trace_position(boot);
        assert_eq!((order, row), trace, "the trace's own position at boot {boot}");
        let (port_order, port_row) = parse_position(measured);
        assert_eq!(
            port_order, order,
            "chain frame {index}: the trace at boot {boot} is in order {order}"
        );
        assert!(
            port_row.abs_diff(row) <= 1,
            "chain frame {index}: the port publishes {port_order}:{port_row}, the trace \
             {order}:{row} at boot {boot} — more than one row's lead is a shifted restart"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_u_run_starts_at_ddstars_with_its_own_numbering() {
    // `SECOND U` is the hidden block: DDSTARS alone (`MAIN/U2.ASM:889-896`), under
    // MUSIC0 at order 70 (`:889-894`). DDSTARS has no measured anchor either — its
    // boot mapping is derived from the part's own filed keyframes, not a first drawn
    // frame — so a run of it is part-numbered like block 3's.
    let (dir, manifest) = block_dump("blockU", "U", "1900");
    assert_eq!(manifest["part"], "ddstars");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 70);
    assert_eq!(manifest["music"]["start_frame"], 0);

    let frame = &manifest["frames"][0];
    assert_eq!(frame["index"], 1900);
    assert_eq!(frame["part"], "ddstars");
    assert!(dir.join("ddstars_001900.idx").is_file());
    assert!(dir.join("ddstars_001900.rgb").is_file());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_run_whose_entry_point_has_no_music_says_so_in_its_manifest() {
    // The test card is the port's own part and not the demo's: `music_for_part` answers
    // `None` for it, so the run installs no player and the manifest carries no music —
    // which is what `PartCtx::music()` returning `None` looks like from outside.
    let dir = scratch("no-music");
    let out = run(&[
        "--headless",
        "--part",
        "testcard",
        "--dump-frames",
        "2",
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));
    let manifest = manifest(&dir);
    assert_eq!(manifest["music"], serde_json::Value::Null);
    assert_eq!(manifest["frames"][0]["music_position"], serde_json::Value::Null);

    let _ = std::fs::remove_dir_all(&dir);
}
