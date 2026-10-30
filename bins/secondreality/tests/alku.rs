//! ALKU through the real binary: the registry, the block-1 numbering, the
//! music's recipe, and the reference keyframes.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/alku.rs` —
//! the scan's stages, the 9-tick scroll, the pan arithmetic, the font's
//! one-byte shift, the palette alias and the four keyframes themselves. What
//! is left over here is the wiring only a process can answer: the registry
//! name, the asset paths as a run resolves them from the repository root, the
//! shell's boot anchor and music recipe, and the frames the shell composes.
//!
//! **The anchor is 404.** `SECOND 1` runs ALKU first (`MAIN/U2.ASM:790-792`),
//! and the boot's first graphics frame is 404 in five runs of five
//! (`reference-indices.md` §1.12, D17), so a run of this part numbers its
//! frames in the reference's own space and its manifest says `index_origin:
//! "boot"`, `anchor: 404`. The music's clock is block 1's: the initial
//! `_zplaysong(0)`, fitted at 399.6 and used as 400 — four frames *before* the
//! run's first, which the run pre-polls — so a `--part alku` and a `--block 1`
//! run are the same run.
//!
//! **The reference holds four `.rgb` frames and no `.idx`** (§4.3, §1.6), so
//! every comparison here is RGB. 900, 1719 and 2719 are held screens and
//! match byte for byte; 5859 is the moving picture, and matches byte for byte
//! too since the port runs the release's loop (the fifth batch stops the text,
//! A20; `docs/fidelity/alku.md` §8).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/ALKU/` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless run dumping the frames asked for, and the manifest it wrote.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
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

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/alku/frame_{index:06}.rgb"
    )))
    .unwrap()
}

/// How many pixels of `rendered` differ from `expected`.
///
/// **The lengths are asserted first**: a truncated dump — the one failure a
/// comparison test exists to catch — would otherwise compare a prefix and
/// report zero differences as if it had compared the frame.
fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    rendered
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| a != b)
        .count()
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

    // 900 is the black screen every pixel of the reference's is.
    let black = frame_file(&dir, 900);
    assert!(black.iter().all(|byte| *byte == 0), "900 is black");
    assert_eq!(differing_pixels(&black, &reference_frame(900)), 0);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_four_keyframes_against_the_reference() {
    // D19's support shape: K-1, K, K+1 around the moving keyframe; the held
    // ones are supported for the same reason they are cheap (their neighbours
    // are the same bytes).
    let (dir, manifest) = dump(
        "keyframes",
        &["--part", "alku"],
        "899,900,901,1718,1719,1720,2718,2719,2720,5858,5859,5860",
    );

    for index in [900u32, 1719, 2719] {
        let frame = frame_file(&dir, index);
        assert_eq!(
            differing_pixels(&frame, &reference_frame(index)),
            0,
            "boot {index}: the held frame is not the reference's"
        );
        // A held screen: the support frames are the same picture.
        assert_eq!(frame_file(&dir, index - 1), frame, "boot {index}: K-1 holds");
        assert_eq!(frame_file(&dir, index + 1), frame, "boot {index}: K+1 holds");
    }

    // The moving keyframe: bare, as the release's loop leaves it once the fifth batch has
    // cleared the text and the window stops carrying it (A20).
    let key = frame_file(&dir, 5859);
    let expected = reference_frame(5859);
    assert_eq!(differing_pixels(&key, &expected), 0, "boot 5859");
    // The run's 5858 and 5860 are the same step, as the reference's session holds 5859,
    // 5860 and 5861 (`reference-indices.md` §4.3): the window steps every nine frames
    // and, with the text gone, nothing else moves.
    assert_eq!(frame_file(&dir, 5858), key, "5858 is the same step");
    assert_eq!(frame_file(&dir, 5860), key, "5860 is the same step");

    assert_eq!(manifest["index_origin"], "boot");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_one_run_starts_at_alku_and_is_the_same_run() {
    // `SECOND 1`'s plan begins at ALKU (`MAIN/U2.ASM:790-792`), so the block
    // run and the `--part` run are one run: same anchor, same recipe, same
    // bytes. The plan then reaches U2A, which is not ported, and the shell
    // holds ALKU's screen — which is why the indices above the part's end are
    // still answerable for a block run.
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
    // The manifest's published position is the ring's, one sample a frame, and
    // the trace's is the same word from a `SECOND 1` session. The port's four
    // pre-polls leave it 2-4 frames behind the trace's row changes (the shell's
    // `a_block_one_run_starts_under_the_initial_play_song_and_matches_second1`
    // compares all 638 of them), so a keyframe may sit one row either side;
    // the order must agree.
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
