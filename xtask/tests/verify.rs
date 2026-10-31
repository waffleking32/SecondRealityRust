//! `cargo xtask reference verify` end to end: **0** every capture hashes as its manifest
//! records, **1** a check failed, **2** the tool could not check. The fixture
//! (`tests/fixtures/verify-capture/`) is a two-frame capture with the digests `capture` writes;
//! a test that changes it changes a copy. The music-trace checks are `tests/music.rs`'s.

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::{copy_tree, fixture, repo_root, scratch, Run};

/// `reference verify --tree <tree>`.
fn verify(tree: &Path) -> Run {
    common::xtask(&["reference", "verify", "--tree", tree.to_str().unwrap()])
}

/// The fixture capture copied somewhere writable.
fn copied_capture(name: &str) -> PathBuf {
    let tree = scratch(name);
    copy_tree(&fixture("verify-capture"), &tree);
    tree
}

/// Rewrite the copy's manifest through `edit`.
fn edit_manifest(tree: &Path, edit: impl FnOnce(&mut serde_json::Value)) {
    let path = tree.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    edit(&mut manifest);
    fs::write(&path, format!("{}\n", serde_json::to_string_pretty(&manifest).unwrap())).unwrap();
}

#[test]
fn a_capture_that_hashes_as_recorded_checks_out() {
    let run = verify(&fixture("verify-capture"));
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("verdict: ok — 1 capture(s), 3 file(s)"),
        "{}",
        run.output()
    );
}

#[test]
fn a_byte_changed_in_a_frame_is_reported_and_exits_one() {
    // A file changed in place, which comparing the tree with itself cannot see.
    let tree = copied_capture("flip-rgb");
    let frame = tree.join("frame_000010.rgb");
    let mut bytes = fs::read(&frame).unwrap();
    bytes[5] ^= 1;
    fs::write(&frame, &bytes).unwrap();

    let run = verify(&tree);
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000010.rgb"), "{}", run.output());
    assert!(run.stdout.contains("verdict: 1 finding(s)"), "{}", run.output());
}

#[test]
fn a_byte_changed_in_an_index_plane_is_reported_and_exits_one() {
    let tree = copied_capture("flip-idx");
    let frame = tree.join("frame_000010.idx");
    let mut bytes = fs::read(&frame).unwrap();
    bytes[0] ^= 0x80;
    fs::write(&frame, &bytes).unwrap();

    let run = verify(&tree);
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000010.idx"), "{}", run.output());
}

#[test]
fn a_frame_the_manifest_does_not_name_is_reported_and_exits_one() {
    let tree = copied_capture("loose");
    fs::write(tree.join("frame_000999.rgb"), [0u8; 24]).unwrap();

    let run = verify(&tree);
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

    let run = verify(&tree);
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000020.rgb"), "{}", run.output());
}

#[test]
fn a_tree_with_no_manifest_in_it_cannot_be_checked_and_exits_two() {
    let tree = scratch("empty");

    let run = verify(&tree);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("holds no manifest.json"), "{}", run.output());
}

#[test]
fn a_manifest_this_tool_does_not_read_cannot_be_checked_and_exits_two() {
    let tree = copied_capture("version");
    let path = tree.join("manifest.json");
    let json = fs::read_to_string(&path).unwrap().replace("\"version\": 1", "\"version\": 2");
    fs::write(&path, json).unwrap();

    let run = verify(&tree);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("version 2"), "{}", run.output());
}

#[test]
fn a_capture_that_names_no_frames_cannot_be_checked_and_exits_two() {
    let tree = copied_capture("empty-capture");
    edit_manifest(&tree, |manifest| manifest["frames"] = serde_json::json!([]));

    let run = verify(&tree);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("names no frames"), "{}", run.output());
    assert!(run.output().contains("manifest.json"), "the capture is named: {}", run.output());
}

#[test]
fn a_frame_whose_length_is_not_its_geometry_is_reported_and_exits_one() {
    // Two checks fail on a truncated file, and both are said: the length and the digest.
    let tree = copied_capture("truncated");
    let frame = tree.join("frame_000010.rgb");
    let bytes = fs::read(&frame).unwrap();
    fs::write(&frame, &bytes[..8]).unwrap();

    let run = verify(&tree);
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("verdict: 2 finding(s)"), "{}", run.output());
    assert!(run.stdout.contains("8 bytes") && run.stdout.contains("4x2"), "{}", run.output());
    assert!(run.stdout.contains("records sha256"), "{}", run.output());
}

#[test]
fn a_frame_with_no_recorded_digest_is_reported_and_exits_one() {
    // Not a pass: nothing would check those bytes.
    let tree = copied_capture("no-digest");
    edit_manifest(&tree, |manifest| {
        manifest["frames"][1].as_object_mut().unwrap().remove("sha256");
    });

    let run = verify(&tree);
    assert_eq!(run.code, 1, "{}", run.output());
    assert!(run.stdout.contains("FAIL frame_000020.rgb"), "{}", run.output());
    assert!(run.output().contains("records no sha256"), "{}", run.output());
    assert!(run.output().contains("reference capture"), "{}", run.output());
}

#[test]
fn the_reference_tree_the_repository_ships_checks_out() {
    // Every capture and trace under `reference/` (`reference-indices.md` §25–§27 list the
    // captures past the per-part keyframes), each music trace checked as a trace.
    let reference = repo_root().join("reference");
    let run = verify(&reference);

    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout.contains("verdict: ok — 71 capture(s), 270 file(s)"),
        "{}",
        run.output()
    );
    for entry in fs::read_dir(reference.join("music")).unwrap() {
        let name = entry.unwrap().file_name().to_string_lossy().into_owned();
        assert!(
            run.stdout.contains(&format!("ok   music/{name}: music trace")),
            "music/{name} is checked as a trace:\n{}",
            run.output()
        );
    }
}
