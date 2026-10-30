//! U2A through the real binary: the block-1 numbering, the music positions,
//! the part each dumped frame is named for, and the saucer page's hold.
//!
//! A `--block 1` run inherits ALKU's 404 anchor (`reference-indices.md` §1.12);
//! U2A has no anchor of its own. The keyframes (6500 at A32's 64,000, 7300 at
//! A29's 1,098, 7640 at 0, and the four `u2a-speed` captures at 0) are rediff's
//! entries `u2a` and `u2a-speed/*`, held by `tests/reference_frames.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless block-1 run dumping `indices`, and the manifest it wrote.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-u2a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--block",
            "1",
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

/// The manifest's entry for `index`.
fn entry(manifest: &serde_json::Value, index: u32) -> &serde_json::Value {
    manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["index"].as_u64() == Some(u64::from(index)))
        .unwrap_or_else(|| panic!("the manifest holds no frame {index}"))
}

/// One dumped file, by the manifest's own name for it.
fn dumped_file(dir: &Path, manifest: &serde_json::Value, index: u32, suffix: &str) -> Vec<u8> {
    let name = entry(manifest, index)[suffix].as_str().unwrap();
    std::fs::read(dir.join(name))
        .unwrap_or_else(|error| panic!("{}/{name}: {error}", dir.display()))
}

/// One frame of a filed trace: the published position and zframe at a
/// reference index.
fn trace_row(name: &str, frame: u32) -> (u8, u8, i64) {
    let path = repository_root()
        .join("reference/music")
        .join(name)
        .join("trace.csv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields[0].trim().parse::<u32>().unwrap() == frame {
            return (
                fields[1].trim().parse().unwrap(),
                fields[2].trim().parse().unwrap(),
                fields[9].trim().parse().unwrap(),
            );
        }
    }
    panic!("{name} holds no frame {frame}")
}

/// One frame of a filed trace: the published position at a reference index.
fn trace_position(name: &str, frame: u32) -> (u8, u8) {
    let path = repository_root()
        .join("reference/music")
        .join(name)
        .join("trace.csv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    for line in text.lines().skip(1) {
        let mut fields = line.split(',');
        let number = |fields: &mut std::str::Split<'_, char>| -> u32 {
            fields.next().unwrap().trim().parse().unwrap()
        };
        let index = number(&mut fields);
        let order = number(&mut fields);
        let row = number(&mut fields);
        if index == frame {
            return (order as u8, row as u8);
        }
    }
    panic!("{name} holds no frame {frame}")
}

/// The keyframes, and the saucer's neighbours.
const FRAMES: &str = "6500,7299,7300,7301,7640";

#[test]
fn the_run_is_numbered_in_boot_frames_and_reaches_u2a_after_alku() {
    let (dir, manifest) = dump("numbering", FRAMES);
    assert_eq!(manifest["part"], "u2a");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404, "block 1 draws from the boot's 404");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `MAIN/U2.ASM:766-775`: block 1's `_zplaysong(0)`, pre-polled 400..403.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 404);

    // Each frame is named for the part running: U2A until its END, then PAM
    // holding U2A's cleared page through its gate.
    assert_eq!(entry(&manifest, 6500)["part"], "u2a");
    assert_eq!(entry(&manifest, 7300)["part"], "u2a");
    assert_eq!(entry(&manifest, 7640)["part"], "pam");
    assert_eq!(entry(&manifest, 7640)["rgb"].as_str().unwrap(), "pam_007640.rgb");

    // The music is the trace's own at the gate's row and the saucer's.
    for index in [6500, 7300] {
        let (order, row) = trace_position("second1c", index);
        assert_eq!(entry(&manifest, index)["music_position"], format!("{order}:{row}"), "{index}");
    }

    // One page holds the saucer's pose across 7299-7301 (the reference's
    // neighbour phase is its session's, `reference-indices.md` §5.3).
    let at_7300 = dumped_file(&dir, &manifest, 7300, "idx");
    assert_eq!(dumped_file(&dir, &manifest, 7299, "idx"), at_7300, "7299 holds");
    assert_eq!(dumped_file(&dir, &manifest, 7301, "idx"), at_7300, "7301 holds");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_block_one_clock_matches_second1_at_boot_5900() {
    // A29's mechanism: the block-1 clock follows `second1`, not `second1c`
    // (the session that supplies the 6988 gate).
    let (dir, manifest) = dump("phase", "5900");
    assert_eq!(entry(&manifest, 5900)["music_position"], "9:51");
    let (order, row, zframe) = trace_row("second1", 5900);
    assert_eq!((order, row), (9, 51), "second1 at boot 5900");
    assert_eq!(zframe, 3771, "second1's zframe at boot 5900");
    let (order, row, zframe) = trace_row("second1c", 5900);
    assert_eq!(
        (order, row, zframe),
        (9, 50, 3768),
        "second1c is the other session, four frames behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
