//! `cargo xtask reference verify` end to end: the exit code *is* the contract.
//!
//! The tool tells a script three things apart — "every capture hashes as its
//! manifest records" (0), "a check failed" (1) and "the tool could not check"
//! (2) — so the tests here run the built binary and read its exit code, the way
//! the script that gates a phase will. One test walks the **real** reference
//! tree (`xtask/../reference`, 24 captures, 105 frame files): Phase 1's
//! close-out recorded that no test reads it, and this is the one that does.
//!
//! The negative half is the point of the tool: a verifier that has never failed
//! is untested, so the byte-flip test below is the one that matters most.
//!
//! Fixtures live in `tests/fixtures/verify-capture/`, a two-frame capture with
//! the digests `capture` writes. A test that writes anything writes it under
//! the temporary directory: `reference/` is input-only, and the only thing in
//! this repository allowed to write inside it is `reference backfill`, once.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root: `xtask/..`, where `reference/` lives.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// The directory the binary is run from: the manifest's own, since `--tree`
/// is an argument and the tool reads only what it is given.
fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// `tests/fixtures/<name>` — a tree to point the binary's `--tree` at.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A scratch directory for this test, emptied first.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("xtask-verify-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// Copy a tree, for a test that has to change one of its own: the fixtures are
/// read-only inputs, like `reference/` itself.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    /// Both streams: the tool prints its report to stdout and its refusals to
    /// stderr, and a test about a message does not care which.
    fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Run the built `xtask` binary in `dir` against the tree at `tree`.
fn xtask(dir: &Path, tree: &Path, subcommand: &str) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(["reference", subcommand, "--tree"])
        .arg(tree)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", env!("CARGO_BIN_EXE_xtask")));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// The fixture capture copied somewhere writable, with its path.
fn copied_capture(name: &str) -> PathBuf {
    let tree = scratch(name);
    copy_tree(&fixture("verify-capture"), &tree);
    tree
}

#[test]
fn a_capture_that_hashes_as_recorded_checks_out() {
    let run = xtask(
        &manifest_dir(),
        &fixture("verify-capture"),
        "verify",
    );
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("verdict: ok — 1 capture(s), 3 file(s)"),
        "{}",
        run.output()
    );
}

#[test]
fn a_byte_changed_in_a_frame_is_reported_and_exits_one() {
    // The whole reason the tool exists: a file *changed in place*, which
    // `compare` of the tree against itself cannot see (the frames still exist
    // and are still the right length).
    let tree = copied_capture("flip-rgb");
    let frame = tree.join("frame_000010.rgb");
    let mut bytes = fs::read(&frame).unwrap();
    bytes[5] ^= 1;
    fs::write(&frame, &bytes).unwrap();

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000010.rgb"), "{}", run.output());
    assert!(run.stdout.contains("verdict: 1 finding(s)"), "{}", run.output());
}

#[test]
fn a_byte_changed_in_an_index_plane_is_reported_and_exits_one() {
    // The indexed side is what `compare` measures with whenever both sides
    // have it, so an `.idx` changed in place is a change to the reference.
    let tree = copied_capture("flip-idx");
    let frame = tree.join("frame_000010.idx");
    let mut bytes = fs::read(&frame).unwrap();
    bytes[0] ^= 0x80;
    fs::write(&frame, &bytes).unwrap();

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000010.idx"), "{}", run.output());
}

#[test]
fn a_frame_the_manifest_does_not_name_is_reported_and_exits_one() {
    let tree = copied_capture("loose");
    fs::write(tree.join("frame_000999.rgb"), [0u8; 24]).unwrap();

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(
        run.stdout.contains("FAIL frame_000999.rgb"),
        "a stray file is a failure, not a curiosity: {}",
        run.output()
    );
    assert!(
        run.output().contains("does not name"),
        "{}",
        run.output()
    );
}

#[test]
fn a_frame_the_manifest_names_that_is_missing_is_reported_and_exits_one() {
    let tree = copied_capture("missing");
    fs::remove_file(tree.join("frame_000020.rgb")).unwrap();

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000020.rgb"), "{}", run.output());
}

#[test]
fn a_tree_with_no_manifest_in_it_cannot_be_checked_and_exits_two() {
    // "Could not check" is not "checked out": a tree this tool measured
    // nothing in must never read as a green run.
    let tree = scratch("empty");

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("holds no manifest.json"), "{}", run.output());
}

#[test]
fn a_manifest_this_tool_does_not_read_cannot_be_checked_and_exits_two() {
    let tree = copied_capture("version");
    let path = tree.join("manifest.json");
    let json = fs::read_to_string(&path).unwrap().replace("\"version\": 1", "\"version\": 2");
    fs::write(&path, json).unwrap();

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("version 2"), "{}", run.output());
}

#[test]
fn the_reference_tree_the_repository_ships_checks_out() {
    // The sentence Phase 1's close-out could not write: **a test reads
    // `reference/`**. 24 frame captures and 105 frame files, every digest
    // against the file beside it — and, since Phase 3, the three music traces
    // under `music/`, which are a second *kind* of capture the verifier reads
    // through its own checks (a flipped byte of one is
    // `xtask/tests/music.rs`'s finding, not this file's).
    let reference = repo_root().join("reference");
    let run = xtask(&repo_root(), &reference, "verify");

    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout
            .contains("verdict: ok — 28 capture(s), 109 file(s)"),
        "{}",
        run.output()
    );
    for trace in ["music/second5", "music/second5b", "music/second3", "music/hiddenu"] {
        assert!(
            run.stdout
                .contains(&format!("ok   {trace}: music trace")),
            "{trace} is checked as itself:\n{}",
            run.output()
        );
    }
}

#[test]
fn backfill_records_the_digests_of_a_tree_filed_without_them() {
    // The exact state the tree was in before this task: frames filed by Phase
    // 1, manifests that record none of their digests.
    let tree = copied_capture("backfill");
    let manifest_path = tree.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&manifest_path).unwrap()).unwrap();
    for frame in manifest["frames"].as_array_mut().unwrap() {
        frame.as_object_mut().unwrap().remove("sha256");
        frame.as_object_mut().unwrap().remove("idx_sha256");
    }
    fs::write(
        &manifest_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&manifest).unwrap()
        ),
    )
    .unwrap();
    let frames_before = frames(&tree);

    // First: a frame with no recorded digest is a finding, not a pass.
    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.output().contains("records no sha256"), "{}", run.output());

    let run = xtask(&manifest_dir(), &tree, "backfill");
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("3 digest(s) recorded into 1 manifest(s)"),
        "{}",
        run.output()
    );

    // A backfill that rewrote a frame would destroy the thing it protects.
    assert_eq!(frames(&tree), frames_before, "a frame file changed");

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 0, "{}", run.output());

    // And a second backfill is a no-op: idempotent, byte for byte.
    let before = tree_bytes(&tree);
    let run = xtask(&manifest_dir(), &tree, "backfill");
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.stdout.contains("nothing written"), "{}", run.output());
    assert_eq!(tree_bytes(&tree), before, "the second run changed a file");
}

#[test]
fn a_capture_that_names_no_frames_cannot_be_checked_and_exits_two() {
    // "Nothing to check" is not "checked out": there is no finding here, only
    // an answer this tool cannot give.
    let tree = copied_capture("empty-capture");
    let path = tree.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["frames"] = serde_json::json!([]);
    fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap())).unwrap();

    let run = xtask(&manifest_dir(), &tree, "verify");
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("names no frames"), "{}", run.output());
    assert!(run.output().contains("manifest.json"), "the capture is named: {}", run.output());
}

#[test]
fn backfill_refuses_a_digest_that_disagrees_rather_than_recording_the_file() {
    // Otherwise a backfill run after a frame changed would bless the change,
    // and the record would describe whatever the tree happens to be. The
    // failure is a *finding* — exit 1, the code `verify` gives the same fact —
    // and it names the way out as well as the tool that lists the damage.
    let tree = copied_capture("backfill-disagrees");
    let path = tree.join("manifest.json");
    let json = fs::read_to_string(&path).unwrap().replace(
        "1d64add2a6388367c9bc2d1f1b384b069a6ef382cdaaa89771dd103e28613a25",
        &"0".repeat(64),
    );
    fs::write(&path, json).unwrap();
    let before = tree_bytes(&tree);

    let run = xtask(&manifest_dir(), &tree, "backfill");
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000010.rgb"), "{}", run.output());
    assert!(run.output().contains("nothing was written"), "{}", run.output());
    assert!(
        run.output().contains("reference capture"),
        "the message says how a digest legitimately changes: {}",
        run.output()
    );
    assert_eq!(tree_bytes(&tree), before, "a backfill with a finding wrote something");
}

#[test]
fn backfill_writes_nothing_into_a_manifest_it_would_lose_keys_from_and_exits_two() {
    // The manifest is the only record of what was captured, and it is not the
    // only thing that can be in the file: a field a later phase adds must not
    // be deleted by the one command licensed to write inside `reference/`.
    let tree = copied_capture("backfill-key");
    let path = tree.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    manifest["frames"][0].as_object_mut().unwrap().remove("sha256");
    manifest["future_field"] = serde_json::json!("what a later phase added");
    fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap())).unwrap();

    // It has a digest to record, so it *would* write — and refuses instead.
    let run = xtask(&manifest_dir(), &tree, "backfill");
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("future_field"), "{}", run.output());
    assert!(run.output().contains("Nothing was written"), "{}", run.output());
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    assert!(written.get("future_field").is_some(), "the field was deleted");
    assert!(
        written["frames"][0].get("sha256").is_none(),
        "a digest was written despite the refusal"
    );
}

#[test]
fn backfill_writes_only_a_manifest_that_had_a_digest_to_record() {
    // A manifest whose digests are all recorded is not touched at all, so the
    // documented `reference backfill --tree reference` cannot change a byte of
    // a tree that is already recorded — even one formatted differently.
    let tree = copied_capture("backfill-nothing-to-do");
    let path = tree.join("manifest.json");
    let reformatted = fs::read_to_string(&path).unwrap().replace("  ", "    ");
    fs::write(&path, reformatted).unwrap();
    let before = tree_bytes(&tree);

    let run = xtask(&manifest_dir(), &tree, "backfill");
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(run.output().contains("nothing written"), "{}", run.output());
    assert_eq!(tree_bytes(&tree), before, "a manifest with nothing to record was rewritten");
}

/// The three frame files' bytes, in a fixed order.
fn frames(tree: &Path) -> Vec<Vec<u8>> {
    ["frame_000010.rgb", "frame_000010.idx", "frame_000020.rgb"]
        .iter()
        .map(|name| fs::read(tree.join(name)).unwrap())
        .collect()
}

/// Every file under `tree` with its bytes, so a test can say nothing changed.
fn tree_bytes(tree: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = Vec::new();
    for entry in fs::read_dir(tree).unwrap() {
        let entry = entry.unwrap();
        files.push((
            entry.file_name().to_string_lossy().into_owned(),
            fs::read(entry.path()).unwrap(),
        ));
    }
    files.sort();
    files
}
