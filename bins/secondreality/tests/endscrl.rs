//! ENDSCRL through the real binary: the four reference keyframes, and the
//! index the chain hands the scroller.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/endscrl.rs`.
//! What is left over here is the wiring only a process can answer: the
//! registry, the shell's block anchor and numbering, the asset read at the path
//! a run really uses, and where in the chain the scroller begins.

use std::path::PathBuf;
use std::process::Command;

/// The keyframes the reference holds for this part (`reference-indices.md`
/// §21.2), and the four its own run must reach.
const KEYFRAMES: &str = "9100,10000,12000,14000";

/// The five indices around the chain's boundary between CRED and ENDSCRL: the
/// port's CRED ends at 8834 (`cred.md` §7), so the scroller's own tick 0 is
/// 8835.
const BOUNDARY: &str = "8834,8835,8836,8837,8838";

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own asset paths resolve: it reads them relative to the working directory.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A dump directory of this test's own, and the manifest the run wrote there.
///
/// The run must exit 0: every requested index is produced. A missing keyframe is
/// a failure with a code, because a hole in a dump reads as a keyframe that
/// matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-endscrl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--block", "5", "--headless", "--dump-frames", indices, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

/// The manifest's frames, as (index, part, width, height).
fn frames(manifest: &serde_json::Value) -> Vec<(u64, String, u64, u64)> {
    manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| {
            (
                frame["index"].as_u64().unwrap(),
                frame["part"].as_str().unwrap().to_string(),
                frame["width"].as_u64().unwrap(),
                frame["height"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_four_keyframes_dump_in_the_references_own_numbering() {
    let (dir, manifest) = dump("numbering", KEYFRAMES);
    // A run of `SECOND 5` begins at ENDLOGO — the demo's first graphics frame,
    // 404 (`MAIN/U2.ASM:906-908`, `reference-indices.md` §1.12) — and the
    // manifest has to say its indices are that space: a part numbering would
    // make every index below incomparable with `reference/`.
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["part"], "endscrl");
    let listed = frames(&manifest);
    assert_eq!(
        listed.iter().map(|(index, ..)| *index).collect::<Vec<_>>(),
        [9100, 10000, 12000, 14000]
    );
    for (index, part, width, height) in &listed {
        assert_eq!(part, "endscrl", "{index}");
        assert_eq!((*width, *height), (640, 400), "{index}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_chain_hands_the_scroller_its_tick_zero_at_the_index_the_note_records() {
    // Where this part begins in the chain, measured rather than reasoned: the
    // dump names the part that drew each frame, and the geometry changes on the
    // tick after the mode set (`src/parts/endscrl.rs`'s tick map — the frame
    // size follows the view from the tick after the one that sets it).
    let (dir, manifest) = dump("boundary", BOUNDARY);
    let listed = frames(&manifest);
    assert_eq!(
        listed,
        [
            (8834, "cred".to_string(), 320, 200),
            (8835, "endscrl".to_string(), 320, 200),
            (8836, "endscrl".to_string(), 320, 200),
            (8837, "endscrl".to_string(), 640, 400),
            (8838, "endscrl".to_string(), 640, 400),
        ],
        "the port's CRED→ENDSCRL boundary is not where `cred.md` §7 and the unit tests put it"
    );
    // And the frames are named for the part that owns them, which is what makes
    // the acceptance's dump files `endscrl_*.rgb`.
    for index in [8835u32, 8837] {
        assert!(
            dir.join(format!("endscrl_{index:06}.rgb")).exists(),
            "{index} was not dumped under the scroller's own name"
        );
    }
    assert!(dir.join("cred_008834.rgb").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

/// The part the reference's own frames belong to is the one the run's dump
/// names, at the index the chain gives it: a reader who finds a mismatch here
/// has the port's chain and the reference's, side by side.
#[test]
fn the_references_own_frames_are_the_parts_the_reference_holds_them_for() {
    let (dir, manifest) = dump("reference", "9100");
    assert_eq!(frames(&manifest), [(9100, "endscrl".to_string(), 640, 400)]);
    let dumped = std::fs::read(dir.join("endscrl_009100.rgb")).unwrap();
    let expected = std::fs::read(
        repository_root().join("reference/parts/endscrl/frame_009100.rgb"),
    )
    .unwrap();
    // The comparison itself is `xtask compare`'s, and it is **not** zero: the
    // reference's own build differs from the tree this part is ported from (the
    // unit tests measure the 25-row pitch and the raster). What this asserts is
    // only that both sides are the same frame of the same part, 640×400, so
    // that the tool's report is about the pixels and not about the wiring.
    assert_eq!(dumped.len(), expected.len());
    assert_eq!(dumped.len(), 640 * 400 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// `reference/` is input-only, and this test reads it: a run that wrote into it
/// would be the failure the `--out` guard exists for.
#[test]
fn the_reference_directory_is_not_written_to() {
    let before = std::fs::read_dir(repository_root().join("reference/parts/endscrl"))
        .unwrap()
        .count();
    let (dir, _) = dump("readonly", "9100");
    let after = std::fs::read_dir(repository_root().join("reference/parts/endscrl"))
        .unwrap()
        .count();
    assert_eq!(before, after, "the run touched the reference tree");
    assert_eq!(after, 5, "the reference tree holds its four frames and its manifest");
    let _ = std::fs::remove_dir_all(&dir);
}
