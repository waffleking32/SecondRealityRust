//! RAYSCRL through the real binary: the registry, the run's own numbering,
//! the blit, the music at the keyframes and D19's neighbours.
//!
//! The keyframes are rediff's `rayscrl` entry. The part has no boot anchor: a
//! run numbers its own frames from zero, and the reference is reached by the
//! mapping boot = 12962 + tick (`docs/fidelity/rayscrl.md` §2).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/WATER/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-rayscrl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "rayscrl",
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
    std::fs::read(dir.join(format!("rayscrl_{index:06}.{extension}"))).unwrap()
}

/// The part tick that draws a reference index (`src/parts/rayscrl.rs`'s
/// module doc).
const REFERENCE_FIRST_INDEX: u32 = 12_962;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
    let (dir, manifest) = dump("numbering", "0,7");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "rayscrl");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 86);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|f| f["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 7]);

    // Tick 0 is the init wait's first poll: nothing is drawn, but the music is
    // already at the reference's blit-time state (order 88 row 0).
    assert_eq!(frames[0]["music_position"], "88:0");
    // Tick 7 is `:111-113`: the 64,000-byte picture with the DAC black.
    let first_indices = frame_file(&dir, 7, "idx");
    assert_eq!(first_indices.len(), 320 * 200);
    let first_rgb = frame_file(&dir, 7, "rgb");
    assert!(
        first_rgb.iter().all(|byte| *byte == 0),
        "the blit's DAC is black"
    );
    // The picture is BKG.CLX's own pixels, rebased.
    let bkg = std::fs::read(repository_root().join("assets/WATER/BKG.CLX")).unwrap();
    assert_eq!(first_indices, bkg[778..778 + 64_000]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_music_is_the_block_three_position_at_every_keyframe() {
    // The pre-roll walks the `254` at order-list entry 87 into order 88 with
    // `zplus` 2, so the published position at each mapped tick is the trace's
    // (`reference/music/second3b/trace.csv` frames 13000, 13500, 14000,
    // 14500). Without it the init's `musplus >= 0` wait would never pass.
    let (dir, manifest) = dump("music", "38,538,1038,1538");
    let positions: Vec<&str> = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["music_position"].as_str().unwrap())
        .collect();
    assert_eq!(positions, ["88:4", "89:2", "90:0", "90:62"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn each_keyframes_neighbours_hold_or_move_as_the_reference_does() {
    // D19's support: `K-1, K, K+1` around every keyframe.
    let keyframes = [13_000, 13_500, 14_000, 14_500].map(tick_of);
    let mut indices = Vec::new();
    for tick in keyframes {
        indices.extend([tick - 1, tick, tick + 1]);
    }
    let list: Vec<String> = indices.iter().map(|index| index.to_string()).collect();
    let (dir, _) = dump("keyframes", &list.join(","));
    let frame = |tick: u32| frame_file(&dir, tick, "rgb");

    // 13000 is the hold of the k=14 upload: `:120-131` uploads on odd `y`
    // only, so ticks 37 and 38 are one picture (the reference's 64-px
    // 12999/13000 difference is the capture catching the upload mid-loop,
    // `reference-indices.md` §15.3); the next upload is a different one.
    let [fade, held, first, second] = keyframes;
    assert_eq!(frame(fade - 1), frame(fade), "boot 13000: the upload's hold");
    assert_ne!(frame(fade + 1), frame(fade), "boot 13000: the next upload");
    // 13500 is the stage-2 hold: its neighbours are the same picture.
    for neighbour in [held - 1, held + 1] {
        assert_eq!(frame(neighbour), frame(held), "boot 13500: tick {neighbour}");
    }
    // 14000 and 14500 are the main loop: the neighbours are different
    // pictures, which is what makes the phase exact.
    for tick in [first, second] {
        for neighbour in [tick - 1, tick + 1] {
            assert_ne!(frame(neighbour), frame(tick), "tick {tick}: tick {neighbour} is the same");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
