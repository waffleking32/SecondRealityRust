//! PLZPART through the real binary: the registry, the run's own numbering
//! across its two screens, the plasma's max deltas and the cube's music row.
//!
//! The keyframe counts are rediff's `plzpart/*` entries. The part has no boot
//! anchor: a run numbers its own frames from zero, and the reference is reached
//! by the mapping `boot = 5723 + tick` (`docs/fidelity/plzpart.md` §2).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for; it must exit 0.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-plzpart-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "plzpart",
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
    std::fs::read(dir.join(format!("plzpart_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(screen: &str, index: u32) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/plzpart/{screen}/frame_{index:06}.rgb"
    )))
    .unwrap()
}

/// The part tick that draws a reference index: tick 0 is boot 5723.
const REFERENCE_FIRST_INDEX: u32 = 5723;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

/// The largest channel difference between two RGB frames of one size.
fn max_delta(rendered: &[u8], expected: &[u8]) -> u8 {
    assert_eq!(rendered.len(), expected.len(), "a short read is not a comparison");
    rendered
        .iter()
        .zip(expected)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap()
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_both_screens() {
    let (dir, manifest) = dump("numbering", "0,1,2777");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "plzpart");
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 62);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1, 2777]);

    // Tick 0 is the plasma's gate: `init_plz` sets the 320×400 view inside
    // the frame call, and the engine composes a tick at the size it started
    // with, so tick 0 is still the shell's 320×200 and the plasma's screen
    // begins at tick 1.
    assert_eq!(frames[0]["width"], 320);
    assert_eq!(frames[0]["height"], 200);
    assert_eq!(frames[1]["width"], 320);
    assert_eq!(frames[1]["height"], 400);
    assert_eq!(frame_file(&dir, 1, "rgb").len(), 320 * 400 * 3);
    assert_eq!(frame_file(&dir, 1, "idx").len(), 320 * 400);

    // By 2777 the cube's `tw_opengraph` has set the 320×200 screen.
    assert_eq!(frames[2]["width"], 320);
    assert_eq!(frames[2]["height"], 200);
    assert_eq!(frame_file(&dir, 2777, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 2777, "idx").len(), 320 * 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_plasma_keyframes_are_close_at_the_derived_mapping() {
    // The plasma's tear (A24) leaves 11.209 / 5.848 / 4.052 % of pixels off
    // (rediff's `plzpart/plasma`), each within these channel deltas.
    let (dir, _) = dump("plasma", "277,1277,1877");
    for (boot, expected) in [(6000u32, 16u8), (7000, 21), (7600, 17)] {
        let rendered = frame_file(&dir, tick_of(boot), "rgb");
        assert_eq!(max_delta(&rendered, &reference_frame("plasma", boot)), expected, "{boot}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cube_keyframe_reads_the_traces_68_46() {
    // The cube's gate is pinned to boot 8364 by the eight-poll pre-roll, so at
    // boot 8500 (tick 2777) the published position is the trace's 68:46; nine
    // polls read 68:47 (`docs/fidelity/plzpart.md` §8).
    let (dir, manifest) = dump("cube", "2777");
    assert_eq!(manifest["frames"][0]["index"], 2777);
    assert_eq!(manifest["frames"][0]["music_position"], "68:46");
    let _ = std::fs::remove_dir_all(&dir);
}
