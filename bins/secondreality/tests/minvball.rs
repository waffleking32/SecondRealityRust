//! MINVBALL through the real binary: the registry, the run's own numbering,
//! the block-3 music position, and the reference keyframes at the mapping the
//! evidence gives.
//!
//! The part's arithmetic is unit-tested beside it in
//! `src/parts/minvball.rs`, including the Microsoft C `rand()` sequence, the
//! deterministic setup, the cue window, the stage boundary and the two bugs.
//! What is left over here is the wiring only a process can answer: the
//! registry, the display geometry as a run resolves it from the repository
//! root, the music the shell installs, and the frames the shell actually
//! composes.
//!
//! **This part has no boot anchor.** Its first drawn frame is a measured
//! window, `(10,300, 10,400]` (`docs/fidelity/reference-indices.md` §14.5), so
//! a `--part minvball` run numbers its own frames from zero and its manifest
//! says `index_origin: "part"`, `anchor: null`. The reference comparison below
//! uses the mapping the reference itself pins — **boot = 10,368 + tick**, the
//! fade's 130 ticks ending at the main loop's first draw at boot 10,498 — and
//! calls it a mapping, not an anchor. The evidence is the four filed
//! keyframes: each is reproduced at exactly one frame counter (403, 703, 1303
//! and 1903), and the survey's 12,900 flat-grey frame is that palette curve's
//! frame counter 2403.
//!
//! The per-keyframe residues are **measured, not tolerated**: all four
//! keyframes are byte-identical to the reference and the shadow layer agrees
//! pixel for pixel (asserted below). The counts are asserted exactly; a change
//! that moves them fails this test.

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

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/minvball/frame_{index:06}.{extension}"
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

fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
    differing_bytes(rendered, expected);
    assert_eq!(rendered.len() % 3, 0);
    rendered
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| a != b)
        .count()
}

/// The part tick that draws a reference index: boot = 10,368 + tick, with
/// tick 0 the fade's first wait and the main loop starting at tick 130. A
/// mapping, not an anchor.
const REFERENCE_FIRST_INDEX: u32 = 10_368;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices below are reached by the stated mapping.
    assert_eq!(tick_of(10_900), 532);
    assert_eq!(tick_of(12_400), 2_032);
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

/// The four keyframes' residues, in pixels, measured from the shipped
/// reference and asserted exactly: all four are byte-identical. (They were 67 /
/// 63 / 37 / 25 pixels while the port's z rotation subtracted a borrow from
/// the previous dot's offset, which the original's `sub dx,cx` does not do:
/// `bugs.md` §18.1, A19.)
const KEYFRAMES: [(u32, usize, usize, usize); 4] = [
    // (reference index, differing pixels, reference shadow pixels, shared shadow pixels)
    (10_900, 0, 1860, 1860),
    (11_200, 0, 2159, 2159),
    (11_800, 0, 2187, 2187),
    (12_400, 0, 2170, 2170),
];

#[test]
fn the_four_keyframes_match_the_reference_at_the_mapping() {
    // 10,900 / 11,200 / 11,800 / 12,400 are RGB-only (the dots move every
    // frame, so the reference files no `.idx`). D19's support: `K-1, K, K+1`
    // around every keyframe, so the mapping is exact rather than a search's
    // nearest fit.
    let mut indices = Vec::new();
    for (index, _, _, _) in KEYFRAMES {
        let tick = tick_of(index);
        indices.extend([tick - 1, tick, tick + 1]);
    }
    indices.sort_unstable();
    indices.dedup();
    let list: Vec<String> = indices.iter().map(|index| index.to_string()).collect();
    let (dir, manifest) = dump("keyframes", &list.join(","));
    assert_eq!(manifest["index_origin"], "part");

    for (index, expected, reference_shadow, shared_expected) in KEYFRAMES {
        let tick = tick_of(index);
        let key = frame_file(&dir, tick, "rgb");
        let theirs = reference_frame(index, "rgb");
        let differing = differing_pixels(&key, &theirs);
        println!("boot {index} (tick {tick}): {differing} px differ");
        assert_eq!(differing, expected, "boot {index} (tick {tick})");
        // Every keyframe is a moving screen (`reference-indices.md` §14.3):
        // the two neighbours are different pictures, which is what makes the
        // mapping exact.
        for neighbour in [tick - 1, tick + 1] {
            assert_ne!(
                frame_file(&dir, neighbour, "rgb"),
                key,
                "boot {index}: tick {neighbour} is not distinguishable"
            );
        }
        // The shadow is colour 87, whose grey ramp value is 11, widened by the
        // DAC to 45: every reference shadow pixel is at the same place in the
        // port's frame.
        let shadow = [45u8, 45, 45];
        let count = |frame: &[u8]| {
            (0..64_000)
                .filter(|at| frame[at * 3..at * 3 + 3] == shadow)
                .count()
        };
        assert_eq!(count(&theirs), reference_shadow, "boot {index}");
        println!("boot {index}: reference shadow pixels {}", count(&theirs));
        let rendered = frame_file(&dir, tick, "rgb");
        let shared = (0..64_000)
            .filter(|at| {
                theirs[at * 3..at * 3 + 3] == shadow
                    && rendered[at * 3..at * 3 + 3] == shadow
            })
            .count();
        println!("boot {index}: shared shadow pixels {shared}");
        assert_eq!(
            shared, shared_expected,
            "boot {index}: shadow pixels shared with the reference"
        );
        assert!(
            shared * 1000 >= reference_shadow * 990,
            "boot {index}: {shared} of {reference_shadow} shadow pixels are shared"
        );

        // The frame counter the mapping predicts: boot = 10,497 + frame.
        assert_eq!(index as i32 - 10_497, {
            // tick 130 is frame 1, so frame = tick - 129.
            (tick as i32) - 129
        });
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_run_reaches_the_frame_cap_and_retires() {
    // `MAIN.C:185` `frame<2450`: the loop's last draw is frame 2450 at tick
    // 130 + 2449 = 2579, boot 12,947. The two ticks after it are still
    // dumped (the run holds the last picture), and the cue the trace reads at
    // boot 12,950 never arrives in time — the cap wins in both.
    assert_eq!(tick_of(12_947), 2_579);
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

/// The guard above can fail, which is the point of it: a dump cut short is
/// exactly the case a bare `zip` would report as identical.
#[test]
#[should_panic(expected = "a short read is not a comparison")]
fn a_truncated_frame_is_refused_rather_than_counted() {
    differing_bytes(&[0; 6], &[0; 9]);
}
