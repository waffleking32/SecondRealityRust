//! PAM through the real binary: the boot numbering, the block-1 music clock,
//! and the reference keyframes with their D19 support frames.
//!
//! A run of the part is anchored at 7710, the reference's first PAM frame
//! (`reference-indices.md` §6.1), so it numbers its frames in boot space.
//!
//! The keyframes are rediff's `pam` entry (`tests/reference_frames.rs`); the
//! support frames, the 7900 index plane (A21) and the music row stay here.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless run dumping `indices`, and the manifest it wrote.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-pam-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "pam",
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

fn frame_file(dir: &Path, suffix: &str, index: u32) -> Vec<u8> {
    std::fs::read(dir.join(format!("pam_{index:06}.{suffix}"))).unwrap()
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

/// D19's support frames around the keyframes (7710's `K-1` is before the
/// anchor): 7760 moves, 7710, 7830 and 7900 hold (`reference-indices.md` §6.3).
const WITH_SUPPORT: &str = "7710,7711,7759,7760,7761,7829,7830,7831,7899,7900,7901";

#[test]
fn the_run_is_numbered_in_boot_frames_and_plays_block_ones_music() {
    let (dir, manifest) = dump("numbering", "7710,7901");
    assert_eq!(manifest["part"], "pam");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 7710);
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `MAIN/U2.ASM:766-775`: block 1's `_zplaysong(0)`, pre-polled to 7710.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 7710);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [7710, 7901]);
    // The trace's row at 7710, which `dis_sync()` codes 10 (`OUTTAA.C:39`).
    let (order, row) = trace_position("second1b", 7710);
    assert_eq!(frames[0]["music_position"], format!("{order}:{row}"));
    assert_eq!(sr_engine::music::sync_from(order, row), 10, "the gate's code");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_support_frames_move_and_hold_as_the_references_do() {
    let (dir, _) = dump("support", WITH_SUPPORT);
    let pixel = |index: u32| frame_file(&dir, "rgb", index);
    assert_eq!(pixel(7711), pixel(7710), "7711 holds 7710's wash");
    assert_ne!(pixel(7759), pixel(7760), "7759 is the frame before");
    assert_eq!(pixel(7761), pixel(7760), "7761 holds 7760");
    assert_eq!(pixel(7829), pixel(7830), "7829 holds 7830");
    assert_eq!(pixel(7831), pixel(7830), "7831 holds 7830");
    assert_eq!(pixel(7899), pixel(7900), "7899 holds 7900");
    assert_eq!(pixel(7901), pixel(7900), "7901 holds 7900");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame() {
    // The reference's 7900 plane is BEGLOGO's white field read late (A21); the
    // port's is the last frame the loop drew, `OUT.ANI`'s 39 (`if(f<=40)`).
    // Its 60,375 differing indices are rediff's `pam` entry.
    let (dir, _) = dump("indexed", "7900");
    let ani = std::fs::read(repository_root().join("assets/PAM/OUT.ANI")).unwrap();
    let frames = sr_assets::load_frames(&ani, sr_assets::Format::Ani).unwrap();
    let frame39 = frames[39].pixels();
    assert_eq!(frame_file(&dir, "idx", 7900), frame39, "the port's plane is animation frame 39");
    // The two figures A21 quotes about that plane.
    let distinct: std::collections::BTreeSet<u8> = frame39.iter().copied().collect();
    assert_eq!(distinct.len(), 64, "the animation frame's distinct indices");
    assert_eq!(frame39.iter().copied().max(), Some(209), "its highest index");
    let _ = std::fs::remove_dir_all(&dir);
}
