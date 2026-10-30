//! 3DSINFLD (COMAN) through the real binary: the registry, the run's own
//! numbering, the music row and the keyframes' two-tick holds.
//!
//! The keyframes are rediff's `3dsinfld` entry. The part has no boot anchor: a
//! run numbers its own frames from zero, and each keyframe's tick is measured,
//! because the implied origins drift (`docs/fidelity/3dsinfld.md` §6, A26).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/COMAN/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-coman-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "3dsinfld",
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
    std::fs::read(dir.join(format!("3dsinfld_{index:06}.{extension}"))).unwrap()
}

/// The four keyframes and the tick each is drawn on.
const KEYFRAMES: [(u32, u32); 4] = [(15500, 464), (16000, 966), (16500, 1470), (16800, 1772)];

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_derived_music_row() {
    let (dir, manifest) = dump("numbering", "0,1");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "3dsinfld");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // Order 91 is derived, not measured: the start window (15000, 15100]
    // spans the traces' 91 -> 93 crossing (`3dsinfld.md` §5).
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 91);
    assert_eq!(manifest["music"]["start_frame"], 0, "an unanchored run starts its clock");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1]);
    assert_eq!(frame_file(&dir, 0, "rgb").len(), 320 * 200 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// D19's support, made from the rendered side: each picture is drawn on two
/// consecutive ticks, so `K+1` holds it and `K-1`/`K+2` are the two-frame
/// step the reference's sessions show (`reference-indices.md` §16.3).
#[test]
fn each_keyframe_holds_for_two_ticks_and_its_neighbours_differ() {
    let ticks: Vec<u32> = KEYFRAMES
        .iter()
        .flat_map(|(_, tick)| [tick - 1, *tick, tick + 1, tick + 2])
        .collect();
    let indices = ticks
        .iter()
        .map(|tick| tick.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (dir, _) = dump("keyframes", &indices);
    for (index, tick) in KEYFRAMES {
        let key = frame_file(&dir, tick, "rgb");
        assert_eq!(frame_file(&dir, tick + 1, "rgb"), key, "{index}: the held tick");
        assert_ne!(frame_file(&dir, tick - 1, "rgb"), key, "{index}: the tick before");
        assert_ne!(frame_file(&dir, tick + 2, "rgb"), key, "{index}: the step after the hold");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
