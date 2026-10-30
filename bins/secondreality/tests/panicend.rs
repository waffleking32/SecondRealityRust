//! PANICEND through the real binary: the registry, the run's own numbering and
//! the music row.
//!
//! The keyframes are rediff's `panicend` entry (9000 exact; 8792 and 8800 need
//! TECHNO's screen and DAC, A63), and the handoff unit test in
//! `src/parts/panicend.rs` matches those two. The part has no boot anchor: the
//! handoff puts the reference at `boot = 8782 + tick` (`docs/fidelity/panicend.md`),
//! so a run numbers its own frames from zero and 9000 is tick 218, inside the
//! shell's hold after the part retires at 121.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for; it must exit 0.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-panicend-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "panicend",
            "--headless",
            "--dump-frames",
            indices,
            "--dump-dir",
        ])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

fn frame_file(dir: &Path, index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(dir.join(format!("panicend_{index:06}.{extension}"))).unwrap()
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
    let (dir, manifest) = dump("numbering", "0,10,18,121,218");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "panicend");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `second2`'s trace holds MUSIC1 at order 37 across the part's start
    // window: rows 53..55 at 8780..8790, pattern 16, tempo 130.
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 37);
    assert_eq!(manifest["music"]["start_frame"], 0, "an unanchored run starts its clock");

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 10, 18, 121, 218]);
    for frame in frames {
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
    }
    // The part retires at 121 (`sleep(1)`); the run holds the screen after
    // it, so a later index is still a frame (and still 320×200).
    assert_eq!(frame_file(&dir, 218, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 218, "idx").len(), 320 * 200);
    let _ = std::fs::remove_dir_all(&dir);
}
