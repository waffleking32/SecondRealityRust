//! LNS&ZOOM through the real binary: the registry, the run's own numbering,
//! the music row, and the five reference keyframes at the derived mapping.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/lnszoom.rs`
//! (the three acts' lengths and gates, the lens tables' bounds and the
//! 1,536-byte overrun, `rotate`'s aspect ordering and 90° swap, the entry
//! pre-roll and the acts' ticks with the real player). What is left over here
//! is the wiring only a process can answer: the registry, the asset paths as a
//! run resolves them from the repository root, the real player's position, and
//! the frames the shell actually composes.
//!
//! **This part has no boot anchor.** Its first drawn face frame is a measured
//! window, `(2452, 2456]` (`docs/fidelity/reference-indices.md` §12.5), so a
//! `--part lnszoom` run numbers its own frames from zero and its manifest says
//! `index_origin: "part"`, `anchor: null`. The comparison below uses the
//! mapping the music and the two later acts pin — **boot = 2452 + tick**, from
//! the trace's `45:59` at 2452, the lens's frame 474 at 3400 and the
//! rotazoomer's frame 552 at 4200 — and calls it a mapping, not an anchor.
//!
//! **The residual is one asset difference (D24, APPROXIMATIONS A25).** The
//! tree's `LENS.EXB` picture differs from the release's in 2,058 pixels of the
//! forehead (rows 33..91, columns 129..211): the port's four visible frames
//! differ by exactly those pixels, and patching the tree's picture with the
//! reference's 2700 index plane makes 3400, 4200 and 4900 byte-identical
//! (`docs/fidelity/lnszoom.md` §7). The pinned numbers below are that
//! divergence measured: 3.2156 %, 3.0234 %, 0.0000 %, 2.18125 % and 0.0000 %.
//!
//! **5650's `.idx` is not a picture.** The reference's plane is the tool's
//! 320×200 planar read of the rotazoomer's last frame under a white palette:
//! its rows 0..99 are the 100 character rows and its rows 100..199 are the
//! memory past them (all zero). The displayed frame — the one the RGB file is
//! — has each character row twice, as 100 of the emulator's 200 scanline
//! pairs, so the port's `.idx` is the displayed (doubled) plane and the two
//! are different geometries. The white keyframe is compared as RGB and the
//! `.idx`'s structure is pinned as evidence of what it is.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/LENS/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
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

/// The mapping: boot = [`REFERENCE_FIRST_INDEX`] + tick. It is pinned by the
/// trace's `45:59` at 2452 (the part's tick 0), the lens's frame 474 at 3400
/// (`src/parts/lnszoom.rs`'s notes), and the rotazoomer's frame 552 at 4200
/// and frame 1252 at 4900. A mapping, not an anchor.
const REFERENCE_FIRST_INDEX: u32 = 2452;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

/// A frame comparison: (differing pixels, max channel delta).
///
/// **The lengths are asserted first**: `zip` stops at the shorter side, so a
/// truncated dump — the one failure a comparison test exists to catch — would
/// compare a prefix and report no differences as if it had compared the frame.
fn compare(rendered: &[u8], expected: &[u8]) -> (usize, u8) {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    assert_eq!(rendered.len() % 3, 0, "RGB8 frames are three bytes a pixel");
    let mut differing = 0usize;
    let mut max_delta = 0u8;
    for (a, b) in rendered.chunks_exact(3).zip(expected.chunks_exact(3)) {
        let delta = (0..3)
            .map(|channel| a[channel].abs_diff(b[channel]))
            .max()
            .unwrap();
        if delta != 0 {
            differing += 1;
        }
        max_delta = max_delta.max(delta);
    }
    (differing, max_delta)
}

fn differing_bytes(rendered: &[u8], expected: &[u8]) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the index plane is {} bytes where the reference's is {}",
        rendered.len(),
        expected.len()
    );
    rendered
        .iter()
        .zip(expected)
        .filter(|(a, b)| a != b)
        .count()
}

/// The percentage of differing pixels, to four decimal places, as the note
/// states it.
fn percent(differing: usize) -> f64 {
    100.0 * differing as f64 / 64_000.0
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices are reached by the stated mapping, not by claiming an
    // anchor this part does not have.
    assert_eq!(tick_of(2700), 248);
    assert_eq!(tick_of(3400), 948);
    assert_eq!(tick_of(4200), 1748);
    assert_eq!(tick_of(4900), 2448);
    assert_eq!(tick_of(5650), 3198);
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

    // 2700: the held face. The port's picture is the tree's LENS.EXB; the
    // reference's is the release's, whose forehead differs (A25). Both the RGB
    // and the index plane differ in exactly those 2,058 pixels.
    let mine = held(2700);
    let theirs = reference_frame(2700, "rgb");
    let (differing, max_delta) = compare(&mine, &theirs);
    println!(
        "2700 (tick {}): {:.4} % pixels, max delta {max_delta}",
        tick_of(2700),
        percent(differing)
    );
    assert_eq!(differing, 2058, "the D24 forehead divergence, RGB");
    assert_eq!(max_delta, 53);
    assert_eq!(
        differing_bytes(
            &frame_file(&dir, tick_of(2700), "idx"),
            &reference_frame(2700, "idx")
        ),
        2058,
        "the same 2,058 pixels in the index plane"
    );
    assert!(
        (percent(differing) - 3.2156).abs() < 0.0001,
        "3.2156 %"
    );

    // 3400: the lens over both eyes. RGB only in the reference (its palette
    // moves while the read would run); the residual is the forehead behind the
    // bubble.
    let (differing, max_delta) = compare(&held(3400), &reference_frame(3400, "rgb"));
    println!(
        "3400 (tick {}): {:.4} % pixels, max delta {max_delta}",
        tick_of(3400),
        percent(differing)
    );
    assert_eq!(differing, 1935);
    assert_eq!(max_delta, 48);
    assert!((percent(differing) - 3.0234).abs() < 0.0001, "3.0234 %");

    // 4200: the rotazoomer's early phase. Byte-identical — the sampled region
    // does not reach the forehead.
    let (differing, max_delta) = compare(&held(4200), &reference_frame(4200, "rgb"));
    println!(
        "4200 (tick {}): {differing} pixels, max delta {max_delta}",
        tick_of(4200)
    );
    assert_eq!(differing, 0, "the rotazoomer at 4200");
    assert_eq!(max_delta, 0);
    // The rotazoomer's screen is 100 character rows, each on two scanlines
    // (`inittwk`'s maximum-scan-line 3 with mode 13h's 400-line total); both
    // the port's and the reference's 4200 are row pairs.
    for picture in [held(4200), reference_frame(4200, "rgb")] {
        for row in 0..100 {
            assert_eq!(
                &picture[2 * row * 960..(2 * row + 1) * 960],
                &picture[(2 * row + 1) * 960..(2 * row + 2) * 960],
                "character row {row} is two scanlines"
            );
        }
    }

    // 4900: the rotazoomer's late phase. The residual is the forehead's
    // 2,058 picture bytes seen through the zoom, 1,396 of them.
    let (differing, max_delta) = compare(&held(4900), &reference_frame(4900, "rgb"));
    println!(
        "4900 (tick {}): {:.4} % pixels, max delta {max_delta}",
        tick_of(4900),
        percent(differing)
    );
    assert_eq!(differing, 1396);
    assert_eq!(max_delta, 41);
    assert!((percent(differing) - 2.18125).abs() < 0.0001, "2.18125 %");

    // 5650: the flat white screen the part exits on.
    let mine = held(5650);
    let (differing, max_delta) = compare(&mine, &reference_frame(5650, "rgb"));
    println!("5650 (tick {}): {differing} pixels", tick_of(5650));
    assert_eq!(differing, 0);
    assert_eq!(max_delta, 0);
    assert!(
        mine.chunks_exact(3).all(|pixel| pixel == [255, 255, 255]),
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
