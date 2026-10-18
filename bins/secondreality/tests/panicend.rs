//! PANICEND through the real binary: the registry, the run's own numbering,
//! the music row, and the one reference keyframe that is self-contained.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/panicend.rs`:
//! `getpal`'s read index, the fade tables' entry-0 black, `copyline`'s
//! 80-word latch copy, the line/dot coordinates, the shell-owned retirement,
//! and — the comparison this process test cannot make — **the handoff**: the
//! same unit test drives TECHNO's port to `Done` and then this part on the
//! same `Surface` and `Palette`, and matches
//! `reference/parts/panicend/frame_008792.rgb` (the collapse) and
//! `frame_008800.rgb` (the line) byte for byte.
//!
//! **8792 and 8800 are handoff evidence, not standalone evidence.** Both
//! frames are made of TECHNO's inherited picture and drawn through a DAC
//! built from TECHNO's inherited palette; a `--part panicend` run starts on a
//! blank surface and the hardware's default DAC, so it cannot reproduce them.
//! The test below asserts exactly that (they differ) rather than hiding it,
//! and compares the frame that *is* self-contained: **9000**, the held dot.
//! Its screen is index 0 everywhere except one index-1 pixel at (160, 100),
//! and the palette's entry 0 is black and entry 1 `(1, 1, 1)` after the
//! pulse, so the inherited picture cannot reach it.
//!
//! **This part has no boot anchor.** The reference's 320×400 picture ends at
//! 8780 and the collapse is visible from the (8780, 8790] window
//! (`reference-indices.md` §10). The handoff test's exact matches put the
//! a = 7 collapse pass at tick 10 and the line at tick 18, so the reference
//! numbering runs `boot = 8782 + tick` — a mapping derived from the
//! reference, not an anchor (the window's own `0d`-to-`0dh` reading is in the
//! note). 9000 is then tick 218, deep inside the loader's hold after the
//! part's `sleep(1)` (the part retires at tick 121; the shell holds its
//! screen), and a run numbers its own frames from zero: `index_origin:
//! "part"`, `anchor: null`.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-panicend-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "panicend",
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
    std::fs::read(dir.join(format!("panicend_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/panicend/frame_{index:06}.{extension}"
    )))
    .unwrap()
}

/// How many bytes of `rendered` differ from `expected`.
///
/// **The lengths are asserted first**: `zip` stops at the shorter side, so a
/// truncated dump — the one failure a comparison test exists to catch — would
/// compare a prefix and report zero differences as if it had compared the
/// frame.
fn differing_bytes(rendered: &[u8], expected: &[u8]) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    rendered
        .iter()
        .zip(expected)
        .filter(|(a, b)| a != b)
        .count()
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
    let (dir, manifest) = dump("numbering", "0,10,18,121,218");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "panicend");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `second2`'s trace holds MUSIC1 at order 37 across the part's start
    // window: rows 53..55 at 8780..8790, pattern 16, tempo 130.
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 37);
    assert_eq!(manifest["music"]["start_frame"], 0, "an unanchored run starts its clock");

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 10, 18, 121, 218]);
    for frame in frames {
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
    }
    // The part retires at 121 (`sleep(1)`); the run holds the screen after
    // it, so a later index is still a frame (and still 320×200).
    assert_eq!(frame_file(&dir, 218, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 218, "idx").len(), 320 * 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_standalone_run_matches_the_self_contained_dot_9000_exactly() {
    let (dir, _) = dump("keyframe", "218");
    // The reference's `.idx` is one index 1 at (160, 100) against 63,999
    // zeros; its RGB is that dot through DAC entries 0 and 1, which the
    // pulse leaves black and (1, 1, 1).
    let mine = frame_file(&dir, 218, "rgb");
    let theirs = reference_frame(9000, "rgb");
    assert_eq!(
        differing_bytes(&mine, &theirs),
        0,
        "9000 is the port's frame at tick 218"
    );
    let mine_idx = frame_file(&dir, 218, "idx");
    let theirs_idx = reference_frame(9000, "idx");
    assert_eq!(differing_bytes(&mine_idx, &theirs_idx), 0, "9000's indices");
    let visible: Vec<usize> = mine_idx
        .iter()
        .enumerate()
        .filter(|(_, index)| **index != 0)
        .map(|(at, _)| at)
        .collect();
    assert_eq!(visible, vec![100 * 320 + 160], "the dot at (160, 100)");
    assert_eq!(mine_idx[100 * 320 + 160], 1, "the dot's index");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_standalone_run_cannot_answer_the_handoffs_8792_and_8800_and_says_so() {
    // The two frames that depend on TECHNO's inherited screen and DAC are
    // the handoff unit test's evidence (`src/parts/panicend.rs`). A run that
    // starts here has a blank surface and the machine's default DAC, so its
    // ticks 10 and 18 — the mapping's own numbers for 8792 and 8800 — are
    // different frames. Recording the difference here is the honest half of
    // the evidence split: a zero would mean the comparison had not run.
    let (dir, _) = dump("handoff-limitation", "10,18");
    for (tick, index) in [(10u32, 8792u32), (18, 8800)] {
        let differing = differing_bytes(
            &frame_file(&dir, tick, "rgb"),
            &reference_frame(index, "rgb"),
        );
        println!(
            "tick {tick} vs {index}: {differing} of {} bytes differ",
            320 * 200 * 3
        );
        assert!(
            differing > 0,
            "tick {tick} is meant to be the standalone run's own screen, not {index}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
