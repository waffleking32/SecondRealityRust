//! TUNNELI through the real binary: the registry, the run's own numbering, and
//! the screen the part leaves.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/tunneli.rs`,
//! including the two things this task's evidence rests on — the chain copy and
//! the unchecked read of the data segment past `rows`. What is left over here
//! is the wiring only a process can answer: the registry, the asset paths as a
//! run resolves them from the repository root, the geometry a mode 13h part
//! asks for, and the numbering — which for this part is **not** the
//! reference's, and is the reason `xtask compare` cannot answer for it
//! (`docs/fidelity/tunneli.md` §7).

use std::path::{Path, PathBuf};
use std::process::Command;

/// `TUN10.PAS:3` `const veke = 1060` — the part's last tick.
const VEKE: u32 = 1060;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/TUNNELI/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a dump
/// reads as a keyframe that matched.
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
    // A run that starts at TUNNELI has no boot anchor: the part begins after
    // GLENZ, which the port has not written, and `shell::boot_anchor` returns
    // `None` for it — so its indices are its own ticks and the manifest has to
    // say so. That is the fact behind the comparison this part cannot have:
    // nothing in `reference/parts/tunneli/` is at an index a `--part tunneli`
    // run ever draws (`tunneli.md` §7).
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
    // The first tick draws nothing at all: every ring's colour is still the
    // zero `:130-135` left, and `:161` skips a ring whose `bbc` is below 64.
    let first = frame_file(&dir, 0, "idx");
    assert_eq!(first.len(), 320 * 200);
    assert!(first.iter().all(|index| *index == 0), "tick 0 is black");
    // And by tick 200 the tunnel is up: the ellipses are drawn, and the dots
    // cover a twentieth of the screen.
    let later = frame_file(&dir, 200, "idx");
    let ink = later.iter().filter(|index| **index != 0).count();
    assert!(ink > 3_000, "tick 200 holds {ink} dotted pixels");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_part_retires_at_veke_and_the_run_holds_its_last_black_screen() {
    // `:221` `if frame = veke then quit := true`. The tick at `veke` draws and
    // syncs before the loop ends, so the part draws 1,061 frames and the last
    // of them is black: from `:220`'s tick 958 every ring's colour is 0, `:161`
    // skips them all, and the erases of the rings that were still moving take
    // the rest (`src/parts/tunneli.rs`'s test measures tick 1,052). 1,061 and
    // 1,200 are the run *holding* that screen — `Sequence`'s hold, which is
    // what the loader shows between the original's EXEs.
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
