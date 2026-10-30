//! LNS&ZOOM through the real binary: the registry, the run's own numbering,
//! the music row, and what the keyframes show beyond rediff's counts.
//!
//! The counts are rediff's `lnszoom` entry (A25: the tree's face differs from
//! the release's in 2,058 forehead pixels). The part has no boot anchor: a run
//! numbers its own frames from zero, and the reference is reached by the
//! mapping `boot = 2452 + tick` (`docs/fidelity/lnszoom.md` §7). 5650's `.idx`
//! is the capture tool's planar memory read (100 character rows, then zeros),
//! not the displayed picture, so it is pinned as evidence and not compared.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for; it must exit 0.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-lnszoom-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "lnszoom",
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
    std::fs::read(dir.join(format!("lnszoom_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/lnszoom/frame_{index:06}.{extension}"
    )))
    .unwrap()
}

/// The mapping: boot = `REFERENCE_FIRST_INDEX` + tick.
const REFERENCE_FIRST_INDEX: u32 = 2452;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

/// The largest channel difference between two RGB frames of one size.
fn max_delta(rendered: &[u8], expected: &[u8]) -> u8 {
    assert_eq!(rendered.len(), expected.len(), "a short read is not a comparison");
    rendered
        .iter()
        .zip(expected)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap()
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
    let (dir, manifest) = dump("numbering", "0,248");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "lnszoom");
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 45);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 248]);
    // The whole part is 320×200: parts 1–2 are mode 13h, and part 3's planar
    // screen is the emulator's 100 character rows each shown twice.
    for frame in frames {
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
    }
    assert_eq!(frame_file(&dir, 0, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 0, "idx").len(), 320 * 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_five_keyframes_at_the_measured_mapping() {
    // The keyframes plus D19's support: one either side of every keyframe that
    // moves. 2700 is held; 3400, 4200 and 4900 move 6.7–39.8 % of pixels a
    // frame (`reference-indices.md` §12.3); 5650 is the white hold.
    let (dir, _) = dump(
        "keyframes",
        "247,248,249,947,948,949,1747,1748,1749,2447,2448,2449,3195,3197,3198,3199",
    );

    let held = |index: u32| -> Vec<u8> { frame_file(&dir, tick_of(index), "rgb") };

    // The forehead residuals' largest channel differences (A25).
    for (index, expected) in [(2700u32, 53u8), (3400, 48), (4900, 41)] {
        assert_eq!(max_delta(&held(index), &reference_frame(index, "rgb")), expected, "{index}");
    }

    // The rotazoomer's screen is 100 character rows, each on two scanlines
    // (`inittwk`'s maximum scan line 3); the reference's 4200 is row pairs too.
    for picture in [held(4200), reference_frame(4200, "rgb")] {
        for row in 0..100 {
            assert_eq!(
                &picture[2 * row * 960..(2 * row + 1) * 960],
                &picture[(2 * row + 1) * 960..(2 * row + 2) * 960],
                "character row {row} is two scanlines"
            );
        }
    }
    assert!(
        held(5650).chunks_exact(3).all(|pixel| pixel == [255, 255, 255]),
        "the exit is 63 in every DAC entry"
    );

    // 5650's `.idx` is the tool's planar *memory* read, not the displayed
    // frame: 100 character rows and the zeros past them, a different geometry
    // from the doubled picture the RGB is. Pinned as evidence, not compared.
    let plane = reference_frame(5650, "idx");
    assert_eq!(plane.len(), 64_000);
    assert!(
        plane[100 * 320..].iter().all(|byte| *byte == 0),
        "rows 100..199 are the memory past the rotazoomer's 100 character rows"
    );
    assert_eq!(
        plane[..100 * 320].iter().filter(|byte| **byte != 0).count(),
        13_692,
        "the last frame's 100 character rows"
    );
    assert_eq!(
        plane.iter().collect::<std::collections::BTreeSet<_>>().len(),
        63,
        "63 distinct indices under the white palette"
    );
    // The port's own plane is the *displayed* picture: the last drawn
    // rotazoomer frame, unchanged by the white palette.
    assert_eq!(
        frame_file(&dir, tick_of(5650), "idx"),
        frame_file(&dir, 3195, "idx"),
        "the white exit changes the palette, not the picture"
    );

    // D19's support: 2700 held (both neighbours the same bytes); 3400, 4200
    // and 4900 moving (both neighbours different); 5650 held.
    for (index, moving) in [(2700u32, false), (3400, true), (4200, true), (4900, true), (5650, false)] {
        let key = held(index);
        for neighbour in [tick_of(index) - 1, tick_of(index) + 1] {
            let other = frame_file(&dir, neighbour, "rgb");
            if moving {
                assert_ne!(
                    other, key,
                    "boot {index}: tick {neighbour} is not distinguishable"
                );
            } else {
                assert_eq!(
                    other, key,
                    "boot {index}: tick {neighbour} is the held picture"
                );
            }
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}
