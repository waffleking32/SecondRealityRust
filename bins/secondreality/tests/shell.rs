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
fn a_block_the_port_cannot_start_at_is_a_command_line_error_in_either_mode() {
    // The window drives the same `Sequence` a headless dump does, so `--block`
    // is no longer refused outside `--headless` — and a block the port cannot
    // start at is refused in both. `SECOND 1` begins at ALKU, which is not
    // ported; the window never opens, so this is answerable without a display.
    //
    // Checked as an exit code because nothing else here is: `--block 2` is
    // accepted and would open a window, which a test must not do.
    let out = run(&["--block", "1"]);
    assert_eq!(out.status.code(), Some(USAGE));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("alku is not ported yet"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

#[test]
fn a_part_the_port_has_not_reached_is_a_command_line_error() {
    // The demo's own parts are named in the refusal: "glenz is not ported yet"
    // is a different answer from "glenzz is not a part", and a script can act
    // on the first.
    let out = run(&["--headless", "--part", "glenz", "--dump-dir", "/tmp/x", "--frames", "1"]);
    assert_eq!(out.status.code(), Some(USAGE));
    assert!(
        String::from_utf8_lossy(&out.stderr).contains("glenz is not ported yet"),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
}

// --- the run's music ---------------------------------------------------------

/// The reference index of ENDLOGO's first drawn frame under `SECOND 5`: the boot's first
/// graphics frame, and where a `--block 5` run begins (`shell::boot_anchor`).
const BLOCK5_ANCHOR: u32 = 404;

/// One frame of a filed trace: the published position at a reference index.
///
/// `reference/music/second5` is a `SECOND 5` session — the recipe a `--block 5` run is
/// comparable with — and its `trace.csv` holds a row a frame, exactly the frames the trace
/// *has* (the manifest's gaps are missing rows, not zero rows).
fn trace_position(name: &str, frame: u32) -> (u8, u8) {
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
            return (order as u8, row as u8);
        }
    }
    panic!("{name} holds no frame {frame}")
}

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `--block 5` headless run that dumps the frames asked for, and the manifest it wrote.
fn block5_dump(name: &str, frames: &str) -> (PathBuf, serde_json::Value) {
    let dir = scratch(name);
    let out = run(&[
        "--headless",
        "--block",
        "5",
        "--dump-frames",
        frames,
        "--dump-dir",
        dir.to_str().unwrap(),
    ]);
    assert_eq!(out.status.code(), Some(OK), "{}", String::from_utf8_lossy(&out.stderr));
    let manifest = manifest(&dir);
    (dir, manifest)
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
