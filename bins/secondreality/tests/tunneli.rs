//! TUNNELI through the real binary: the registry, the run's own numbering, and
//! the screen the part leaves. A `--part tunneli` run has no boot anchor, so no
//! filed reference index is one it draws (`docs/fidelity/tunneli.md` §7).

use std::path::{Path, PathBuf};
use std::process::Command;

/// `TUN10.PAS:3` `const veke = 1060` — the part's last tick.
const VEKE: u32 = 1060;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping `indices`.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-tunneli-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--headless", "--part", "tunneli", "--dump-frames", indices, "--dump-dir"])
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
    std::fs::read(dir.join(format!("tunneli_{index:06}.{extension}"))).unwrap()
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
    // `shell::boot_anchor` has none for TUNNELI: its indices are its ticks.
    let (dir, manifest) = dump("numbering", "0,10,200");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "tunneli");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [0, 10, 200]);
    for frame in frames {
        // `:110-115` mode 13h: 320x200, one byte per pixel.
        assert_eq!(frame["width"], 320, "{}", frame["idx"]);
        assert_eq!(frame["height"], 200, "{}", frame["idx"]);
    }
    // Tick 0 draws nothing: every ring's colour is `:130-135`'s zero (`:161`).
    let first = frame_file(&dir, 0, "idx");
    assert_eq!(first.len(), 320 * 200);
    assert!(first.iter().all(|index| *index == 0), "tick 0 is black");
    // By tick 200 the tunnel is up.
    let later = frame_file(&dir, 200, "idx");
    let ink = later.iter().filter(|index| **index != 0).count();
    assert!(ink > 3_000, "tick 200 holds {ink} dotted pixels");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_part_retires_at_veke_and_the_run_holds_its_last_black_screen() {
    // `:221`: the tick at `veke` is the last drawn, and black (`:220`); 1,061
    // and 1,200 are the run holding that screen.
    let (dir, manifest) = dump("retire", &format!("{},{},{},1200", VEKE - 20, VEKE, VEKE + 1));
    let last = frame_file(&dir, VEKE, "idx");
    assert!(last.iter().all(|index| *index == 0), "the last drawn frame is black");
    assert!(
        frame_file(&dir, VEKE - 20, "idx").iter().any(|index| *index != 0),
        "and it was not always: tick 1,040 still has dots"
    );
    for held in [VEKE + 1, 1200] {
        assert_eq!(
            frame_file(&dir, held, "idx"),
            last,
            "the held frame {held} is not the screen the part left"
        );
        assert_eq!(frame_file(&dir, held, "rgb"), frame_file(&dir, VEKE, "rgb"));
    }
    assert_eq!(manifest["index_origin"], "part");
    let _ = std::fs::remove_dir_all(&dir);
}
