//! DDSTARS through the real binary: the run's own numbering, the block-U music
//! row, the keyframes' moving neighbours and the music at the mapped tick.
//!
//! The keyframes are rediff's `ddstars` entry. The part has no boot anchor: a
//! run numbers its own frames from zero, and the reference is reached by the
//! mapping boot = 414 + tick (`docs/fidelity/ddstars.md` §3).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/DDSTARS/TEXTS.16` path resolves.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-ddstars-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "ddstars",
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
    std::fs::read(dir.join(format!("ddstars_{index:06}.{extension}"))).unwrap()
}

/// The part tick that draws a reference index.
const REFERENCE_FIRST_INDEX: u32 = 414;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x400() {
    let (dir, manifest) = dump("numbering", "0,1");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "ddstars");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 400);
    // `MAIN/U2.ASM:889-894`: `SECOND U` restarts MUSIC0 at order 70.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 70);
    assert_eq!(manifest["music"]["start_frame"], 0);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|f| f["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1]);

    // Tick 0 presents the page the loop has not written yet: the mode set's
    // cleared picture, with the first fade's `b = 8` making every entry the
    // part uses black. Tick 1 presents tick 0's blit — the first field —
    // under `b = 16`.
    let first = frame_file(&dir, 0, "rgb");
    assert_eq!(first.len(), 320 * 400 * 3);
    assert!(first.iter().all(|byte| *byte == 0), "tick 0 is the cleared page");
    let first_indices = frame_file(&dir, 0, "idx");
    assert_eq!(first_indices.len(), 320 * 400);
    assert!(first_indices.iter().all(|pixel| *pixel == 0));
    let second = frame_file(&dir, 1, "rgb");
    assert!(
        second.iter().any(|byte| *byte != 0),
        "tick 1 shows the first star pass"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_keyframes_neighbours_differ_and_the_music_runs_twenty_two_frames_behind_the_trace() {
    // D19's support: every keyframe moves (`reference-indices.md` §22.3), so
    // `K-1` and `K+1` are different pictures and the mapping is exact.
    let keyframes = [1900u32, 3000, 5000, 8000];
    let mut indices = Vec::new();
    for index in keyframes {
        let tick = tick_of(index);
        indices.extend([tick - 1, tick, tick + 1]);
    }
    let list: Vec<String> = indices.iter().map(|index| index.to_string()).collect();
    let (dir, manifest) = dump("keyframes", &list.join(","));
    for index in keyframes {
        let tick = tick_of(index);
        let key = frame_file(&dir, tick, "rgb");
        for neighbour in [tick - 1, tick + 1] {
            assert_ne!(
                frame_file(&dir, neighbour, "rgb"),
                key,
                "boot {index}: tick {neighbour} is not distinguishable"
            );
        }
    }

    // The part reads no music; this is where the shell put it. The run's
    // clock starts at D27's 408, the mapping's zero is 414 and the hiddenu2
    // session's phase is -16 frames, so the trace runs 22 frames ahead: the
    // port at tick 7586 (boot 8000) reads the trace's 7978, not its 8000.
    let position = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["index"] == u64::from(tick_of(8000)))
        .and_then(|frame| frame["music_position"].as_str())
        .expect("the run has a player");
    assert_eq!(position, "30:51");
    let trace = std::fs::read_to_string(repository_root().join("reference/music/hiddenu2/trace.csv"))
        .unwrap();
    let position_at = |frame: u32| -> String {
        let row = trace
            .lines()
            .find(|line| line.starts_with(&format!("{frame},")))
            .unwrap_or_else(|| panic!("the trace holds frame {frame}"));
        let fields: Vec<&str> = row.split(',').collect();
        format!("{}:{}", fields[1], fields[2])
    };
    assert_eq!(position_at(7978), position);
    assert_eq!(position_at(8000), "30:54");
    let _ = std::fs::remove_dir_all(&dir);
}
