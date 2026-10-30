//! ALKU through the real binary: the registry, the block-1 numbering, the
//! music's recipe, and the support frames around the reference keyframes.
//!
//! `SECOND 1` runs ALKU first (`MAIN/U2.ASM:790-792`) and the boot's first
//! graphics frame is 404 (`reference-indices.md` §1.12, D17), so a run numbers
//! its frames in the reference's own space. The music's clock is block 1's
//! `_zplaysong(0)`, fitted at 400 and pre-polled to 404, so a `--part alku` and
//! a `--block 1` run are the same run. The four keyframes are rediff's `alku`
//! entry (`tests/reference_frames.rs`); their support frames' holds are here.

use std::path::{Path, PathBuf};
use std::process::Command;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless run dumping `indices`, and the manifest it wrote.
fn dump(name: &str, mode: &[&str], indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-alku-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let mut args = mode.to_vec();
    args.extend_from_slice(&[
        "--headless",
        "--dump-frames",
        indices,
        "--dump-dir",
    ]);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(&args)
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

fn frame_file(dir: &Path, index: u32) -> Vec<u8> {
    std::fs::read(dir.join(format!("alku_{index:06}.rgb"))).unwrap()
}

/// One frame of a filed trace: the published position at a reference index.
fn trace_position(name: &str, frame: u32) -> (u8, u8) {
    let path = repository_root().join("reference/music").join(name).join("trace.csv");
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

#[test]
fn the_run_is_numbered_in_boot_frames_and_draws_320x400() {
    let (dir, manifest) = dump("numbering", &["--part", "alku"], "900,1719");
    assert_eq!(manifest["part"], "alku");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 400);
    // `MAIN/U2.ASM:766-775`: block 1 takes the initial `_zplaysong(0)`, so the
    // module is MUSIC0 at order 0.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 0);
    // The fitted 400 is before the run's first frame, so its four frames are
    // pre-polled and the clock stands at the run's own 404.
    assert_eq!(manifest["music"]["start_frame"], 404);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [900, 1719]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_support_frames_hold_as_the_references_do() {
    // D19's support shape around the four keyframes (`reference-indices.md`
    // §4.3): the held screens' neighbours are the same bytes, and 5859's are
    // the same step, since the window moves every nine frames and, with the
    // fifth batch's text gone (A20), nothing else does.
    let (dir, _) = dump(
        "support",
        &["--part", "alku"],
        "899,900,901,1718,1719,1720,2718,2719,2720,5858,5859,5860",
    );
    for index in [900u32, 1719, 2719, 5859] {
        let frame = frame_file(&dir, index);
        assert_eq!(frame_file(&dir, index - 1), frame, "boot {index}: K-1 holds");
        assert_eq!(frame_file(&dir, index + 1), frame, "boot {index}: K+1 holds");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_one_run_starts_at_alku_and_is_the_same_run() {
    // `SECOND 1`'s plan begins at ALKU (`MAIN/U2.ASM:790-792`): the block run
    // and the `--part` run share the anchor, the recipe and the bytes.
    let (part_dir, part_manifest) =
        dump("part", &["--part", "alku"], "900,1719,2719,5859");
    let (block_dir, block_manifest) =
        dump("block", &["--block", "1"], "900,1719,2719,5859");
    assert_eq!(block_manifest["part"], "alku");
    assert_eq!(block_manifest["index_origin"], "boot");
    assert_eq!(block_manifest["anchor"], 404);
    assert_eq!(block_manifest["music"], part_manifest["music"]);
    for index in [900u32, 1719, 2719, 5859] {
        assert_eq!(
            frame_file(&block_dir, index),
            frame_file(&part_dir, index),
            "boot {index}: the block run and the part run differ"
        );
    }
    let _ = std::fs::remove_dir_all(&part_dir);
    let _ = std::fs::remove_dir_all(&block_dir);
}

#[test]
fn the_runs_music_is_the_block_one_recipes_at_the_keyframes() {
    // The port's pre-polls leave the published row 2-4 frames behind the
    // trace's row changes (`shell`'s
    // `a_block_one_run_starts_under_the_initial_play_song_and_matches_second1`),
    // so a keyframe may sit one row either side; the order must agree.
    let (dir, manifest) = dump("music", &["--part", "alku"], "900,1719,2719,5859");
    let mut measured = Vec::new();
    for entry in manifest["frames"].as_array().unwrap() {
        let index = entry["index"].as_u64().unwrap() as u32;
        let position = entry["music_position"].as_str().unwrap();
        let (order, row) = trace_position("second1", index);
        measured.push((index, position.to_string(), format!("{order}:{row}")));
        let (got_order, got_row) = position.split_once(':').unwrap();
        assert_eq!(got_order.parse::<u8>().unwrap(), order, "frame {index}: the order");
        let got_row: i32 = got_row.parse().unwrap();
        assert!(
            (got_row - i32::from(row)).abs() <= 1,
            "frame {index}: the port reads {position}, the trace {order}:{row}"
        );
    }
    println!("second1 at the keyframes: {measured:?}");
    let _ = std::fs::remove_dir_all(&dir);
}
