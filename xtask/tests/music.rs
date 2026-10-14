//! `cargo xtask music` end to end, and the trace as a `reference verify` citizen.
//!
//! Two things are checked here that nothing else can: that the **symbol map**
//! the trace's every address comes from is produced by a command (so
//! `docs/fidelity/music-clock.md`'s numbers have one behind them), and that the
//! **verifier sees the new kind of capture** — `reference verify` reads the 24
//! frame captures filed before `kind` existed *and* the music traces, and a
//! flipped byte of a trace is a failure with both digests in it, while a `kind`
//! it does not know is refused by name rather than walked past. Review Focus 5
//! of the phase plan is exactly this, and a verifier that has never been seen to
//! fail is worth nothing.
//!
//! The commands that drive the emulator are not run here: they need the
//! emulator and minutes of wall clock. What is run is everything they refuse
//! *before* touching it, because those refusals are what keep a trace from
//! being filed in the wrong place or with the wrong recipe.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root: `xtask/..`, where `reference/` lives.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Run the built `xtask` binary from the repository root, so the relative
/// defaults the commands document are the ones they use.
fn xtask(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(repo_root())
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", env!("CARGO_BIN_EXE_xtask")));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// A scratch directory for this test, emptied first.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("xtask-music-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

const OBJECT: &str = "upstream/SecondReality/MAIN/STMIK.300";

// -------------------------------------------------------------------------------------
// The symbol map
// -------------------------------------------------------------------------------------

/// The map the trace reads through, printed by a command: the census, the one
/// segment, and the seventeen words the phase reads at their measured offsets.
#[test]
fn the_symbols_command_prints_the_map() {
    let run = xtask(&["music", "symbols", "--object", OBJECT]);
    assert_eq!(run.code, 0, "{}", run.output());
    let text = run.output();
    for expected in [
        "24849 bytes, 278 record(s)",
        "0x80 THEADR    1 record(s)",
        "0x88 COMENT    5 record(s)",
        "0x8A MODEND    1 record(s)",
        "0x90 PUBDEF    45 record(s)",
        "0x96 LNAMES    2 record(s)",
        "0x98 SEGDEF    1 record(s)",
        "0xA0 LEDATA    105 record(s)",
        "0xA2 LIDATA    118 record(s)",
        "29607 bytes",
        "0x08F7   2295 _np_zinfo",
        "0x08FD   2301 _np_row",
        "0x0903   2307 _np_loop",
        "0x2002   8194 _np_speed",
        "0x090F   2319 _np_tempo",
        "needle",
    ] {
        assert!(text.contains(expected), "{expected} is not in:\n{text}");
    }
}

/// A symbol map cannot be read from an object that is not one, and the refusal
/// says which file.
#[test]
fn a_file_that_is_not_an_object_is_refused_by_name() {
    let run = xtask(&["music", "symbols", "--object", "assets/MAIN/MUSIC0.S3M"]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("MUSIC0.S3M"), "{}", run.output());
}

// -------------------------------------------------------------------------------------
// What `music trace` refuses before it starts anything
// -------------------------------------------------------------------------------------

/// `reference/` is input-only: the one place inside it a trace may be written is
/// a new directory under `reference/music/`, and everything else is refused
/// **by name** — including a frame capture's own directory, and
/// `reference/music` itself.
#[test]
fn a_trace_may_not_be_written_among_the_reference() {
    for out in [
        "reference/parts/beglogo",
        "reference/check_a",
        "reference/original",
        "reference/music",
    ] {
        let run = xtask(&[
            "music", "trace", "--cycles", "30000", "--frames", "0..10", "--out", out,
        ]);
        assert_eq!(run.code, 2, "{out}: {}", run.output());
        let text = run.output();
        assert!(
            text.contains("reference/") && text.contains("reference/music"),
            "{out} is refused by name: {text}"
        );
    }
    // And a frame capture that holds a manifest is refused for what it is.
    let out = fixture("verify-capture");
    let run = xtask(&[
        "music",
        "trace",
        "--cycles",
        "30000",
        "--frames",
        "0..10",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(
        run.output().contains("frame capture"),
        "{}",
        run.output()
    );
}

/// A trace with no pinned cycles is not reproducible and is refused, and a
/// window that is empty or has a hole in it is not a window.
#[test]
fn a_trace_needs_a_pinned_clock_and_one_window() {
    let run = xtask(&["music", "trace", "--cycles", "0", "--frames", "0..10"]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("cycles"), "{}", run.output());

    for frames in ["", "10..10", "10..0", "0,2", "0..5,9..12"] {
        let run = xtask(&[
            "music",
            "trace",
            "--cycles",
            "30000",
            "--frames",
            frames,
            "--out",
            "out/should-not-be-written",
        ]);
        assert_eq!(run.code, 2, "--frames {frames:?}: {}", run.output());
    }
}

/// The keys are part of the measurement — the card they select is in the
/// manifest — so a name the emulator's mapper does not know is refused before
/// anything starts (D17).
#[test]
fn a_key_that_answers_nothing_is_refused() {
    let run = xtask(&[
        "music",
        "trace",
        "--cycles",
        "30000",
        "--frames",
        "0..10",
        "--keys",
        "KBD_kp_8",
        "--out",
        "out/should-not-be-written",
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("KBD_kp_8"), "{}", run.output());
}

/// A directory that already holds a trace is not written into without
/// `--force`.
#[test]
fn an_existing_trace_is_not_replaced_without_force() {
    let run = xtask(&[
        "music",
        "trace",
        "--cycles",
        "30000",
        "--frames",
        "0..10",
        "--out",
        "reference/music/second5",
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("--force"), "{}", run.output());
}

// -------------------------------------------------------------------------------------
// The trace as a reference, and the verifier that has to see it
// -------------------------------------------------------------------------------------

/// The trace `reference/music/second5` is the phase's oracle: it holds one row
/// per frame with the columns the plan names, keyed by the guest's own frame
/// counter.
#[test]
fn the_filed_trace_is_what_the_phase_reads() {
    let dir = repo_root().join("reference/music/second5");
    let text = fs::read_to_string(dir.join("trace.csv")).unwrap();
    let mut lines = text.lines();
    assert_eq!(
        lines.next().unwrap(),
        "frame,ord,row,pat,loop,speed,tempo,zinfo,zplus,zframe"
    );
    let rows: Vec<&str> = lines.filter(|line| !line.trim().is_empty()).collect();
    assert!(rows.len() > 1000, "{} row(s)", rows.len());
    let frames: Vec<u64> = rows
        .iter()
        .map(|row| row.split(',').next().unwrap().parse().unwrap())
        .collect();
    assert!(
        frames.windows(2).all(|pair| pair[1] > pair[0]),
        "the frames are strictly increasing"
    );
    assert!(
        *frames.first().unwrap() > 0 && *frames.last().unwrap() < 1300,
        "the trace is the 0..1300 window's: {}..{}",
        frames[0],
        frames.last().unwrap()
    );
    // The manifest says what the file is, and the two agree.
    let manifest = fs::read_to_string(dir.join("manifest.json")).unwrap();
    assert!(manifest.contains("\"kind\": \"music-trace\""));
    assert!(manifest.contains("\"per_frame\": false"));
    let value: serde_json::Value = serde_json::from_str(&manifest).unwrap();
    assert_eq!(value["trace"]["rows"], rows.len());
    assert_eq!(value["coverage"]["samples"], rows.len());
    // The three frames the trace does not hold are named with what the guest's
    // own counter did around them — not silently missing.
    let missing: Vec<u64> = value["coverage"]["missing"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|run| {
            let from = run[0].as_u64().unwrap();
            let to = run[1].as_u64().unwrap();
            from..=to
        })
        .collect();
    assert!(
        missing.contains(&409) && missing.contains(&1220),
        "{missing:?}"
    );
    let skipped = value["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 2, "{skipped:?}");
}

/// `reference verify --tree reference` passes with the traces in it, and the 24
/// frame captures filed before `kind` existed are still read as frame captures.
#[test]
fn the_reference_tree_passes_with_the_traces_in_it() {
    let run = xtask(&["reference", "verify", "--tree", "reference"]);
    assert_eq!(run.code, 0, "{}", run.output());
    let text = run.output();
    assert!(text.contains("music trace"), "{text}");
    assert!(text.contains("27 capture(s)"), "{text}");
}

/// **The test that matters most**: a copy of a trace with one byte flipped is a
/// finding — exit 1, naming the file and both digests. A verifier that has never
/// been seen to fail is worth nothing.
#[test]
fn a_flipped_byte_of_a_trace_fails_the_verifier() {
    let tree = scratch("flipped");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    for file in ["trace.csv", "manifest.json"] {
        fs::copy(
            repo_root().join("reference/music/second5").join(file),
            capture.join(file),
        )
        .unwrap();
    }
    let path = capture.join("trace.csv");
    let mut bytes = fs::read(&path).unwrap();
    // A flip that stays valid UTF-8, so the digest is what catches it.
    let at = bytes.iter().position(|byte| *byte == b'4').unwrap();
    bytes[at] = b'5';
    fs::write(&path, &bytes).unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 1, "{}", run.output());
    let text = run.output();
    assert!(text.contains("trace.csv"), "{text}");
    assert!(text.contains("sha256"), "{text}");
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(capture.join("manifest.json")).unwrap()).unwrap();
    let recorded = manifest["trace"]["sha256"].as_str().unwrap();
    assert!(
        text.contains(recorded),
        "the digest the manifest records is in the finding: {text}"
    );
}

/// A `kind` the tool does not know is a **failure and not a skip**: a verifier
/// that walked past it would report a green run over a capture it never read.
#[test]
fn a_capture_of_an_unknown_kind_is_refused_by_name() {
    let tree = scratch("unknown-kind");
    let capture = tree.join("mystery");
    fs::create_dir_all(&capture).unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("reference/music/second5/manifest.json")).unwrap(),
    )
    .unwrap();
    manifest["kind"] = serde_json::json!("video-trace");
    fs::write(
        capture.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 2, "{}", run.output());
    let text = run.output();
    assert!(text.contains("video-trace"), "the kind is named: {text}");
    assert!(text.contains("does not know") || text.contains("refused"), "{text}");
}

/// A trace whose rows do not match the coverage its manifest claims is a
/// finding: the coverage is recomputed from the file's own frames rather than
/// believed.
#[test]
fn a_coverage_claim_the_file_does_not_support_is_a_finding() {
    let tree = scratch("coverage");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    fs::copy(
        repo_root().join("reference/music/second5/trace.csv"),
        capture.join("trace.csv"),
    )
    .unwrap();
    let mut manifest: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(repo_root().join("reference/music/second5/manifest.json")).unwrap(),
    )
    .unwrap();
    // Claim a window the rows do not cover, and say it is per-frame.
    manifest["coverage"]["window"] = serde_json::json!([0, 2000]);
    manifest["coverage"]["per_frame"] = serde_json::json!(true);
    fs::write(
        capture.join("manifest.json"),
        format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap()),
    )
    .unwrap();

    let run = xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()]);
    assert_eq!(run.code, 1, "{}", run.output());
    let text = run.output();
    assert!(
        text.contains("coverage") || text.contains("per-frame"),
        "{text}"
    );
    // The trace file itself still hashes as recorded, so the only finding is
    // the coverage claim — which is the point: the digest is not the only check.
    assert!(!text.contains("sha256"), "{text}");
}

// -------------------------------------------------------------------------------------
// The analysis, which is where the note's numbers come from
// -------------------------------------------------------------------------------------

/// `music analyse` reads a filed trace back and prints the measurements the
/// note records, including the ones that can only be read off the trial: where
/// the music starts, the rate against the module's own header, the
/// quantisation, the machine's words, `zplus`/`zinfo` and the order list.
#[test]
fn the_analysis_prints_the_notes_numbers() {
    let run = xtask(&["music", "analyse", "--trace", "reference/music/second5"]);
    assert_eq!(run.code, 0, "{}", run.output());
    let text = run.output();
    for section in [
        "1. where the music starts",
        "2. the tick rate",
        "3. the quantisation",
        "4. the soundcard and mixer, from the guest",
        "5. zplus and zinfo",
        "6. the order list, and the 254/255 entries",
    ] {
        assert!(text.contains(section), "{section} is not in:\n{text}");
    }
    assert!(text.contains("first advancing frame: 410"), "{text}");
    assert!(text.contains("_zoutputmode     5"), "{text}");
    assert!(text.contains("_zmixspeed       20000"), "{text}");
    assert!(
        text.contains("order 27 (254): never held — the counter went from ord 26 to ord 28"),
        "{text}"
    );
}

/// The analysis checks the trace against the digest its manifest records before
/// reading a number out of it: a trace that does not hash as recorded is not
/// analysed, because every number would be about bytes nobody filed.
#[test]
fn the_analysis_refuses_a_trace_that_does_not_hash() {
    let tree = scratch("analysis-digest");
    let capture = tree.join("second5");
    fs::create_dir_all(&capture).unwrap();
    for file in ["trace.csv", "manifest.json"] {
        fs::copy(
            repo_root().join("reference/music/second5").join(file),
            capture.join(file),
        )
        .unwrap();
    }
    let path = capture.join("trace.csv");
    let mut bytes = fs::read(&path).unwrap();
    bytes.push(b'\n');
    fs::write(&path, &bytes).unwrap();

    let run = xtask(&[
        "music",
        "analyse",
        "--trace",
        capture.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("sha256"), "{}", run.output());
}

/// `--at` places an address against the object's symbols: the phase's own
/// cross-check of `docs/fidelity/reference-indices.md` §1.7's physical address
/// `0x105B0E`, which has to turn out to be a *named* word or a measured
/// explanation, not a guess.
#[test]
fn the_cross_check_places_an_address_against_the_symbols() {
    let run = xtask(&[
        "music",
        "analyse",
        "--trace",
        "reference/music/second5",
        "--at",
        "0x105B0E",
    ]);
    assert_eq!(run.code, 0, "{}", run.output());
    let text = run.output();
    assert!(text.contains("0x105B0E"), "{text}");
    // The measured answer: it is not _np_zframe, it is the unnamed word at
    // offset 0x091E, and the trace's own rate at it is reported.
    assert!(text.contains("0x091E"), "{text}");
    assert!(text.contains("_np_zframe"), "{text}");
}
