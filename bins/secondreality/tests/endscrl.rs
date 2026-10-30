//! ENDSCRL through the real binary: the numbering of a `--block 5` run's keyframes, and the
//! index the chain hands the scroller. The keyframes' counts are rediff's (`endscrl`, A11), held
//! by `tests/reference_frames.rs`.

use std::path::PathBuf;
use std::process::Command;

/// The five indices around the chain's boundary between CRED and ENDSCRL: the
/// port's CRED ends at 8834 (`cred.md` §7), so the scroller's own tick 0 is
/// 8835.
const BOUNDARY: &str = "8834,8835,8836,8837,8838";

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `--block 5` dump of `indices`, which must exit 0, and its manifest.
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
fn the_first_keyframe_dumps_in_the_references_own_numbering() {
    // `SECOND 5` is anchored at ENDLOGO's 404 (`MAIN/U2.ASM:906-908`); 9100 is
    // the first of `reference-indices.md` §21.2's keyframes.
    let (dir, manifest) = dump("keyframes", "9100");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["part"], "endscrl");
    assert_eq!(frames(&manifest), [(9100, "endscrl".to_string(), 640, 400)]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_chain_hands_the_scroller_its_tick_zero_at_the_index_the_note_records() {
    // The dump names the part that drew each frame; the geometry follows the
    // view from the tick after the mode set (tick 1).
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
