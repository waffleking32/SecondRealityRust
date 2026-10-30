//! MINVBALL through the real binary: the registry, the run's own numbering,
//! the music's head start, the keyframes' moving neighbours and the frame cap.
//!
//! The keyframes are rediff's `minvball` entry. The part has no boot anchor: a
//! run numbers its own frames from zero, and the reference is reached by the
//! mapping boot = 10,368 + tick (`docs/fidelity/minvball.md` §3).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-minvball-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "minvball",
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
    std::fs::read(dir.join(format!("minvball_{index:06}.{extension}"))).unwrap()
}

/// The part tick that draws a reference index (`src/parts/minvball.rs`'s
/// module doc).
const REFERENCE_FIRST_INDEX: u32 = 10_368;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
    let (dir, manifest) = dump("numbering", "0,130");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "minvball");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 75);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|f| f["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 130]);
    // The 256-poll pre-roll's witness through the real binary: the engine's
    // tick-0 poll publishes the trace's own reading at boot 10,368.
    assert_eq!(frames[0]["music_position"], "75:61");

    // Tick 0 is the first fade wait: the picture is the floor ramp with the
    // DAC black (`MAIN.C:151-155`). Tick 130 is the main loop's first draw,
    // with the fade's last upload (`pal`) in the DAC.
    for frame in frames {
        let index = frame["index"].as_u64().unwrap() as u32;
        let pixels = frame_file(&dir, index, "idx");
        assert_eq!(pixels.len(), 320 * 200);
        if index == 0 {
            // Tick 0 is before any drawing: the whole picture is the ramp.
            for a in 0..100usize {
                let row = 100 + a;
                assert!(
                    pixels[row * 320..(row + 1) * 320]
                        .iter()
                        .all(|p| *p == (a + 64) as u8),
                    "tick {index}: row {row} carries index {}",
                    a + 64
                );
            }
            assert!(pixels[..100 * 320].iter().all(|p| *p == 0));
        } else {
            // The main loop's first draw has put dots on the floor, but the
            // rows are still mostly the ramp.
            let ramp = |a: usize| (a + 64) as u8;
            let row = 158;
            let a = row - 100;
            let matching = pixels[row * 320..(row + 1) * 320]
                .iter()
                .filter(|p| **p == ramp(a))
                .count();
            assert!(matching > 300, "row {row}: {matching} ramp pixels");
        }
        let rgb = frame_file(&dir, index, "rgb");
        if index == 0 {
            assert!(rgb.iter().all(|byte| *byte == 0), "tick 0's DAC is black");
        } else {
            assert!(
                rgb.iter().any(|byte| *byte != 0),
                "the main loop's first draw is not black: the fade's last upload is `pal`"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The four keyframes (`reference-indices.md` §14.2).
const KEYFRAMES: [u32; 4] = [10_900, 11_200, 11_800, 12_400];

#[test]
fn each_keyframes_neighbours_differ_from_it() {
    // D19's support: the dots move every frame, so `K-1` and `K+1` are
    // different pictures and the mapping is exact.
    let mut indices = Vec::new();
    for index in KEYFRAMES {
        let tick = tick_of(index);
        indices.extend([tick - 1, tick, tick + 1]);
    }
    let list: Vec<String> = indices.iter().map(|index| index.to_string()).collect();
    let (dir, _) = dump("keyframes", &list.join(","));
    for index in KEYFRAMES {
        let tick = tick_of(index);
        let key = frame_file(&dir, tick, "rgb");
        for neighbour in [tick - 1, tick + 1] {
            assert_ne!(
                frame_file(&dir, neighbour, "rgb"),
                key,
                "boot {index}: tick {neighbour} is not distinguishable"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_run_reaches_the_frame_cap_and_retires() {
    // `MAIN.C:185` `frame<2450`: the loop's last draw is frame 2450 at tick
    // 130 + 2449 = 2579 (boot 12,947), and the run holds it after.
    let (dir, manifest) = dump("cap", "2579,2580");
    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2);
    // The last drawn frame and the held one are the same picture.
    let last = frame_file(&dir, 2_579, "rgb");
    let held = frame_file(&dir, 2_580, "rgb");
    assert_eq!(last, held, "the retired part holds its last frame");
    // And the palette at the cap is not the still-running fade's: the last
    // upload before the cap is `pal2` at frame 2449, whose c = 63-2a has
    // long reached zero (`MAIN.C:258-269`), so the picture is black.
    assert!(
        last.iter().all(|byte| *byte == 0),
        "the frame cap leaves the fade-out's black DAC"
    );
    let _ = std::fs::remove_dir_all(&dir);
}
