//! ENDSCRL through the real binary: the four reference keyframes, and the
//! index the chain hands the scroller.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/endscrl.rs`.
//! What is left over here is the wiring only a process can answer: the
//! registry, the shell's block anchor and numbering, the asset read at the path
//! a run really uses, and where in the chain the scroller begins.
//!
//! **Two runs, not four.** A run of `--block 5` reaches index 14000 in 13,596
//! ticks and takes over two minutes in a debug build, so the checks are grouped
//! by the dump they need: one far run for the keyframes and everything the
//! four frames themselves answer, and one five-index run at the chain's
//! boundary.

use std::path::PathBuf;
use std::process::Command;

/// The keyframes the reference holds for this part (`reference-indices.md`
/// §21.2), and the four its own run must reach.
const KEYFRAMES: &str = "9100,10000,12000,14000";

/// The five indices around the chain's boundary between CRED and ENDSCRL: the
/// port's CRED ends at 8834 (`cred.md` §7), so the scroller's own tick 0 is
/// 8835.
const BOUNDARY: &str = "8834,8835,8836,8837,8838";

/// The frames the reference holds, by the same names `reference/` files them
/// under, and the part every one of them belongs to.
const REFERENCE_FRAMES: [(u32, &str); 4] = [
    (9100, "frame_009100.rgb"),
    (10000, "frame_010000.rgb"),
    (12000, "frame_012000.rgb"),
    (14000, "frame_014000.rgb"),
];

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
    // A run of `SECOND 5` begins at ENDLOGO — the demo's first graphics frame,
    // 404 (`MAIN/U2.ASM:906-908`, `reference-indices.md` §1.12) — and the
    // manifest has to say its indices are that space: a part numbering would
    // make every index below incomparable with `reference/`.
    let reference_before = std::fs::read_dir(repository_root().join("reference/parts/endscrl"))
        .unwrap()
        .count();

    let (dir, manifest) = dump("keyframes", KEYFRAMES);
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

    // Each frame is dumped under the part that owns it, which is what makes the
    // acceptance's dump files `endscrl_*.rgb`; and the four are the reference's
    // own frames, same geometry. The pixel comparison itself is `xtask compare`'s,
    // and it is **not** zero (the unit tests measure the reference's build), so
    // what this test asserts is the wiring **and that each dump is a real,
    // distinct scroller frame** — a uniform or truncated buffer would make the
    // tool's report about a buffer rather than about pixels.
    let mut dumps: Vec<(u32, Vec<u8>)> = Vec::new();
    for (index, name) in REFERENCE_FRAMES {
        let dumped = std::fs::read(dir.join(format!("endscrl_{index:06}.rgb"))).unwrap();
        let expected =
            std::fs::read(repository_root().join("reference/parts/endscrl").join(name)).unwrap();
        assert_eq!(dumped.len(), expected.len(), "{index}");
        assert_eq!(dumped.len(), 640 * 400 * 3, "{index}");
        assert!(
            dumped.iter().any(|&byte| byte != dumped[0]),
            "{index}: the dumped frame is one uniform byte, not a picture"
        );
        dumps.push((index, dumped));
    }
    for (at, (index, frame)) in dumps.iter().enumerate() {
        for (other_index, other) in &dumps[at + 1..] {
            assert_ne!(
                frame, other,
                "{index} and {other_index}: the scroller's two frames are identical"
            );
        }
    }

    // `reference/` is input-only: a run that wrote into it would be the failure
    // the `--out` guard exists for.
    let reference_after = std::fs::read_dir(repository_root().join("reference/parts/endscrl"))
        .unwrap()
        .count();
    assert_eq!(reference_before, reference_after, "the run touched the reference tree");
    assert_eq!(reference_after, 5, "the reference tree holds its four frames and its manifest");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_chain_hands_the_scroller_its_tick_zero_at_the_index_the_note_records() {
    // Where this part begins in the chain, measured rather than reasoned: the
    // dump names the part that drew each frame, and the geometry changes on the
    // tick after the mode set (`src/parts/endscrl.rs`'s tick map — the frame
    // size follows the view from the tick after the one that sets it).
    let (dir, manifest) = dump("boundary", BOUNDARY);
    assert_eq!(
        frames(&manifest),
        [
            (8834, "cred".to_string(), 320, 200),
            (8835, "endscrl".to_string(), 320, 200),
            (8836, "endscrl".to_string(), 320, 200),
            (8837, "endscrl".to_string(), 640, 400),
            (8838, "endscrl".to_string(), 640, 400),
        ],
        "the port's CRED→ENDSCRL boundary is not where `cred.md` §7 and the unit tests put it"
    );
    assert!(dir.join("endscrl_008835.rgb").exists());
    assert!(dir.join("cred_008834.rgb").exists());
    let _ = std::fs::remove_dir_all(&dir);
}
