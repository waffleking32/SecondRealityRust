//! CRED through the real binary, reached through ENDLOGO by `--block 5`: the
//! chain's numbering, the holds around the keyframes, and the wipes between
//! them. The keyframes themselves are rediff's (`cred`), held by
//! `tests/reference_frames.rs`.
//!
//! CRED's tick 0 is reference index 1212 (`docs/fidelity/cred.md` §2), so
//! screen 1's open loop is 1214-1315, its hold 1316-1515 and its close
//! 1516-1574; a screen is 363 ticks.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The holds of screens 1, 6, 12, 18 and 21 (`reference-indices.md` §20.1).
const KEYFRAMES: [u32; 5] = [1440, 3240, 5440, 7640, 8740];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `--block 5` dump of `indices`, which must exit 0, and its manifest.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-cred-{}-{name}", std::process::id()));
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

fn frame_file(dir: &Path, index: u32) -> Vec<u8> {
    std::fs::read(dir.join(format!("cred_{index:06}.rgb"))).unwrap()
}

#[test]
fn the_five_keyframes_dump_in_the_references_own_numbering() {
    let (dir, manifest) = dump("numbering", "1440,3240,5440,7640,8740");
    // `SECOND 5` is anchored at the demo's first graphics frame
    // (`MAIN/U2.ASM:906-908`); the manifest names the part of its first frame.
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["part"], "cred");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, KEYFRAMES.map(u64::from));
    for frame in frames {
        assert_eq!(frame["width"], 320, "{}", frame["idx"]);
        assert_eq!(frame["height"], 200, "{}", frame["idx"]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_five_keyframes_are_inside_the_chains_own_holds() {
    // The frame either side of each keyframe is the same (D19's "held"): the
    // port's holds, at 1316 + 363·k, contain all five, screen 21's with 34
    // frames to spare.
    let indices: Vec<String> =
        KEYFRAMES.iter().flat_map(|k| [k - 1, *k, k + 1]).map(|i| i.to_string()).collect();
    let (dir, _) = dump("holds", &indices.join(","));
    for key in KEYFRAMES {
        let at = frame_file(&dir, key);
        assert_eq!(frame_file(&dir, key - 1), at, "{key} moves from {}", key - 1);
        assert_eq!(at, frame_file(&dir, key + 1), "{key} moves to {}", key + 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_wipes_move_the_display_between_the_holds() {
    // What the reference cannot check (its keyframes are all held): the
    // registers each step writes reach the dumped frame.
    let (dir, _) = dump("wipe", "1213,1214,1215,1220,1230,1315,1316,1525,1526");
    // Ticks 1-3 are blank: the start is still in the even logical line 400,
    // and the picture is in the odd lines.
    assert_eq!(frame_file(&dir, 1213), frame_file(&dir, 1214));
    assert_eq!(frame_file(&dir, 1214), frame_file(&dir, 1215));
    // The open wipe moves.
    assert_ne!(frame_file(&dir, 1220), frame_file(&dir, 1215), "open step 6");
    assert_ne!(frame_file(&dir, 1230), frame_file(&dir, 1220), "open step 16");
    // The open loop's last step writes the hold's state.
    assert_eq!(frame_file(&dir, 1315), frame_file(&dir, 1316));
    // The close wipe moves.
    assert_ne!(frame_file(&dir, 1526), frame_file(&dir, 1525), "close steps 9 and 10");
    let _ = std::fs::remove_dir_all(&dir);
}
