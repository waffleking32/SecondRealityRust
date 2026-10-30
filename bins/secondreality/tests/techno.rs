//! TECHNO through the real binary: the run's own numbering, the music row,
//! the two screens' geometry, and D19's support around the seven keyframes.
//!
//! The part has no boot anchor (its first rings are a content-probed frame,
//! 4797; `reference-indices.md` §8.5), so a `--part techno` run numbers its
//! own frames from zero. The keyframes sit on three measured mappings
//! (`techno.md` §2); their pixel counts are rediff's `techno/interference`
//! and `techno/troll` entries (`tests/reference_frames.rs`).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-techno-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "techno",
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
    std::fs::read(dir.join(format!("techno_{index:06}.{extension}"))).unwrap()
}

/// The three mappings (`techno.md` §2): the first pass is frame-quantised,
/// **boot = 4797 + tick**; the row-gated sections run on the player's clock,
/// **boot = 4799 + tick**; the troll's mode change costs a frame,
/// **boot = 4800 + tick**.
const FIRST_PASS_ORIGIN: u32 = 4797;
const MUSIC_ORIGIN: u32 = 4799;
const TROLL_ORIGIN: u32 = 4800;

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
    let (dir, manifest) = dump("numbering", "0,1,103,1501,3396,3397,3600");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "techno");
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 18);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1, 103, 1501, 3396, 3397, 3600]);
    // The interference screen is 320×200, the troll 320×400. A dump is sized
    // from the view as the tick starts, so the mode change during frame 3396
    // first shows at 3397.
    for (frame, height) in frames.iter().zip([200, 200, 200, 200, 200, 400, 400]) {
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], height);
    }
    assert_eq!(frame_file(&dir, 3396, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 3397, "rgb").len(), 320 * 400 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D19's support (`reference-indices.md` §9.3): a moving keyframe differs from
/// both port neighbours, a held one does not.
#[test]
fn the_keyframes_move_and_hold_as_the_references_do() {
    let keyframes = [
        (4900u32, 4900 - FIRST_PASS_ORIGIN, true),
        (5000, 5000 - FIRST_PASS_ORIGIN, false),
        (6300, 6300 - MUSIC_ORIGIN, true),
        (8100, 8100 - MUSIC_ORIGIN, true),
        (8200, 8200 - MUSIC_ORIGIN, false),
        (8300, 8300 - TROLL_ORIGIN, true),
        (8400, 8400 - TROLL_ORIGIN, false),
    ];
    let indices = keyframes
        .iter()
        .flat_map(|(_, tick, _)| [tick - 1, *tick, tick + 1])
        .map(|tick| tick.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (dir, _) = dump("support", &indices);
    for (index, tick, moving) in keyframes {
        let key = frame_file(&dir, tick, "rgb");
        for neighbour in [tick - 1, tick + 1] {
            let other = frame_file(&dir, neighbour, "rgb");
            if moving {
                assert_ne!(other, key, "boot {index}: tick {neighbour} is not distinguishable");
            } else {
                assert_eq!(other, key, "boot {index}: tick {neighbour} is the held picture");
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
