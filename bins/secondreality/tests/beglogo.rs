//! BEGLOGO through the real binary: the four reference keyframes, dumped from
//! the repository root, where the part's own asset path resolves.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/beglogo.rs`.
//! What is left over here is the wiring only a process can answer: the
//! registry, the shell's block anchor and numbering, the asset read at the
//! path a run really uses, and the DAC the shell hands the part.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The part's own asset, by the same path `src/parts/beglogo.rs` reads it by.
const PICTURE: &str = "assets/BEG/SRTITLE.UP";

/// The keyframes the reference holds for this part (`reference-indices.md` §3).
const KEYFRAMES: &str = "430,470,560,900";

/// Where a `cargo run` of the acceptance command starts, and where `PICTURE`
/// resolves: the part reads a path relative to the working directory.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A dump directory of this test's own, and the manifest the run wrote there.
///
/// The run must exit 0: every requested index is produced, and the frames are
/// the part's. A missing keyframe is a failure with a code, because a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-beglogo-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--headless", "--part", "beglogo", "--dump-frames", KEYFRAMES, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

fn frame_file(dir: &Path, index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(dir.join(format!("beglogo_{index:06}.{extension}"))).unwrap()
}

#[test]
fn the_four_keyframes_dump_in_the_references_own_numbering() {
    let (dir, manifest) = dump("numbering");
    // A run that starts at BEGLOGO is anchored at the demo's first graphics
    // frame, and the manifest has to say its indices are that space: a `part`
    // numbering would make every index below incomparable with `reference/`.
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

#[test]
fn the_opening_white_is_index_15_through_the_dac_the_shell_installed() {
    // The reference's 430 is 128,000 bytes of 15 and one palette entry,
    // (63,63,63) — and the part has written no DAC entry at that tick: `:25-34`
    // is 32 vertical blanks later. What makes the frame white is entry 15 of
    // the table a mode set leaves, so a part that cleared the DAC would render
    // this black and fail a keyframe before doing anything.
    let (dir, _) = dump("white");
    let indices = frame_file(&dir, 430, "idx");
    assert_eq!(indices.len(), 320 * 400);
    assert!(indices.iter().all(|index| *index == 15), "430 is not the white field");
    let rgb = frame_file(&dir, 430, "rgb");
    assert!(rgb.iter().all(|channel| *channel == 255), "430 is not white");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_held_frame_is_the_picture_the_part_read() {
    // 900 is past the end of the fade, so the run is holding the screen the
    // part left (§3.5). What is on it is `BEG/SRTITLE.UP` as `sr_assets`
    // decodes it — the same decoding the acceptance comparison finds
    // byte-identical to `reference/parts/beglogo/frame_000900.idx`.
    let (dir, _) = dump("held");
    let image = sr_assets::load_path(&repository_root().join(PICTURE)).unwrap();
    let dumped = frame_file(&dir, 900, "idx");
    let differing = dumped.iter().zip(image.pixels()).filter(|(a, b)| a != b).count();
    assert_eq!(differing, 0, "the held frame differs from the picture in {differing} of {} pixels", dumped.len());
    let _ = std::fs::remove_dir_all(&dir);
}
