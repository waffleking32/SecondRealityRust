//! BEGLOGO through the real binary: the registry, the shell's anchor and the
//! numbering a run's manifest states. The keyframes are rediff's (`beglogo`,
//! `beglogo-leadin/*`), held by `tests/reference_frames.rs`.

use std::path::PathBuf;
use std::process::Command;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn the_four_keyframes_dump_in_the_references_own_numbering() {
    let dir = std::env::temp_dir().join(format!("sr-beglogo-{}-numbering", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--headless", "--part", "beglogo", "--dump-frames", "430,470,560,900", "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    // Anchored at the demo's first graphics frame, in the reference's numbering.
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["part"], "beglogo");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [430, 470, 560, 900]);
    for frame in frames {
        assert_eq!(frame["width"], 320, "{}", frame["idx"]);
        assert_eq!(frame["height"], 400, "{}", frame["idx"]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}
