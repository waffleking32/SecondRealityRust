//! TECHNO through the real binary: the registry, the run's own numbering, the
//! music row, and the seven reference keyframes across the two screens.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/techno.rs`
//! (the bar palette, the 512-byte `setpalarea`, the first pass's 256-frame
//! fade, the sine table's byte cursor, the clip/fill renderer, the page flips,
//! the troll's RLE and row layout, the entry pre-roll and the section ticks).
//! What is left over here is the wiring only a process can answer: the
//! registry, the asset paths as a run resolves them from the repository root,
//! the real player's position, and the frames the shell actually composes.
//!
//! **This part has no boot anchor.** Its first rings are a content-probed
//! frame, 4797 (sessions vary by one to three frames;
//! `docs/fidelity/reference-indices.md` §8.5), so a `--part techno` run numbers
//! its own frames from zero and its manifest says `index_origin: "part"`,
//! `anchor: null`.
//!
//! **The mapping is piecewise, and every piece is measured at a keyframe.**
//! The first pass is frame-quantised — its iteration 101 is the reference's
//! 4900 — so `boot = 4797 + tick`. The sections that gate on the player's row
//! advance on the player's clock instead: 6300 and 8100 are drawn at ticks
//! 1501 and 3301 (`boot = 4799 + tick`), and the troll's mode change puts
//! 8300 at tick 3500 and the held 8400 at 3600 (`boot = 4800 + tick`). The
//! source's `setmframe(0)` stands at boot 4794 in `second2`'s trace, and the
//! two- and three-frame offsets are the difference between that player clock
//! and the first pass's frame count. They are mappings, not anchors.
//!
//! **All seven keyframes are RGB only.** The 320×200 screen is 16-colour EGA:
//! DOSBox names its depth `16-colour`, and the oracle's indexed read refuses
//! anything that is not one palette index a byte, so the reference holds no
//! `.idx` there. The 320×400 screen's one `.idx` (8200) is 128,000 bytes of
//! index 0 — a flat black frame whose indices are consistent with the RGB and
//! worthless as evidence about the 160-byte stride, which the RGB's held
//! picture does pin. The test says so rather than counting it as a check.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-techno-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "techno",
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
    std::fs::read(dir.join(format!("techno_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(screen: &str, index: u32) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/techno/{screen}/frame_{index:06}.rgb"
    )))
    .unwrap()
}

/// The section mappings, all measured at the reference keyframes.
///
/// The first pass's picture is frame-quantised: iteration 101 is the
/// reference's 4900 and iteration 201 its 5000 (the 256-frame fade's white
/// hold), and its iteration `n` is tick `n + 2`, so **boot = 4797 + tick**.
/// Tick 0 is the `setmframe(0)` gate tick; the content probe puts the first
/// rings at 4797, and sessions vary by one to three frames there.
///
/// The sections that gate on the player's *row* advance on the player's
/// clock: the source's `setmframe(0)` stands at boot 4794 in `second2`'s
/// trace, the reference's 6300 and 8100 are drawn at ticks 1501 and 3301 —
/// **boot = 4799 + tick** — and the troll's mode-change transition puts 8300
/// at tick 3500 and the held 8400 at 3600 — **boot = 4800 + tick**. 8200 is
/// inside the black hold the mode change begins, so both origins land there.
const FIRST_PASS_ORIGIN: u32 = 4797;
const MUSIC_ORIGIN: u32 = 4799;
const TROLL_ORIGIN: u32 = 4800;

fn first_pass_tick(index: u32) -> u32 {
    index - FIRST_PASS_ORIGIN
}

fn music_tick(index: u32) -> u32 {
    index - MUSIC_ORIGIN
}

fn troll_tick(index: u32) -> u32 {
    index - TROLL_ORIGIN
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

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices below are reached by the measured mappings, not by
    // claiming an anchor this part does not have.
    assert_eq!(first_pass_tick(4900), 103);
    assert_eq!(music_tick(6300), 1501);
    assert_eq!(troll_tick(8200), 3400);
    assert_eq!(troll_tick(8400), 3600);
    let (dir, manifest) = dump("numbering", "0,1,103,1501,3396,3397,3600");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "techno");
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 18);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1, 103, 1501, 3396, 3397, 3600]);
    // The interference screen is 320×200; the troll is 320×400. The engine
    // sizes a dumped frame from the view as the tick starts, so the part's
    // mode change during frame 3396 first produces a 320×400 dump at 3397.
    for (frame, height) in frames.iter().zip([200, 200, 200, 200, 200, 400, 400]) {
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], height);
    }
    assert_eq!(frame_file(&dir, 103, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 3396, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 3397, "rgb").len(), 320 * 400 * 3);
    assert_eq!(frame_file(&dir, 3600, "rgb").len(), 320 * 400 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The seven keyframes, with D19's support: `K-1, K, K+1` around every
/// keyframe that moves, and `K` alone for one that holds
/// (`reference-indices.md` §9.3). Each is compared at the tick its mapping
/// names, and each is byte-identical.
#[test]
fn the_seven_keyframes_at_the_measured_mappings() {
    let first = first_pass_tick(4900);
    let white = first_pass_tick(5000);
    let bars = music_tick(6300);
    let cross = music_tick(8100);
    let black = music_tick(8200);
    let fade = troll_tick(8300);
    let picture = troll_tick(8400);
    assert_eq!(
        (first, white, bars, cross, black, fade, picture),
        (103, 203, 1501, 3301, 3401, 3500, 3600)
    );

    let ticks: Vec<u32> = [first, white, bars, cross, black, fade, picture]
        .iter()
        .flat_map(|tick| [tick - 1, *tick, tick + 1])
        .collect();
    let indices = ticks
        .iter()
        .map(|tick| tick.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (dir, _) = dump("keyframes", &indices);

    let check = |tick: u32, screen: &str, index: u32, pixels: usize| {
        let mine = frame_file(&dir, tick, "rgb");
        let theirs = reference_frame(screen, index);
        let (differing, max_delta) = compare(&mine, &theirs);
        println!(
            "{index} (tick {tick}): {differing} px {:.4} %, max delta {max_delta}",
            100.0 * differing as f64 / pixels as f64
        );
        assert_eq!(differing, 0, "{index} is not the port's frame at tick {tick}");
        assert_eq!(max_delta, 0, "{index}'s max channel delta");
    };

    // --- the 320×200 interference screen ------------------------------------
    // 4900: the first pass's static picture and its fading palette.
    check(first, "interference", 4900, 64_000);
    // 5000: the 256-frame fade's white hold.
    let white_frame = frame_file(&dir, white, "rgb");
    assert!(
        white_frame.chunks_exact(3).all(|pixel| pixel == [255, 255, 255]),
        "the fade ends on 63 in every one of the 16 DAC entries"
    );
    check(white, "interference", 5000, 64_000);
    // 6300: the doit1 bars/interference screen.
    check(bars, "interference", 6300, 64_000);
    // 8100: doit3's crosshatch.
    check(cross, "interference", 8100, 64_000);

    // --- the 320×400 troll screen -------------------------------------------
    // 8200: the flat black the mode change leaves.
    check(black, "troll", 8200, 128_000);
    // The reference's `.idx` is 128,000 bytes of index 0. It is consistent
    // with the black RGB and it is worthless as evidence about the 160-byte
    // row stride — recorded, not counted as a check.
    let plane = std::fs::read(
        repository_root().join("reference/parts/techno/troll/frame_008200.idx"),
    )
    .unwrap();
    assert_eq!(plane.len(), 320 * 400);
    assert!(
        plane.iter().all(|index| *index == 0),
        "8200's idx is all index 0"
    );
    // 8300: the fade-in; 8400: the held picture.
    check(fade, "troll", 8300, 128_000);
    check(picture, "troll", 8400, 128_000);

    // D19's support: a held keyframe's neighbours are the same bytes; a
    // moving one's are distinguishable one tick either side.
    for (index, tick, moving) in [
        (4900u32, first, true),
        (5000, white, false),
        (6300, bars, true),
        (8100, cross, true),
        (8200, black, false),
        (8300, fade, true),
        (8400, picture, false),
    ] {
        let key = frame_file(&dir, tick, "rgb");
        for neighbour in [tick - 1, tick + 1] {
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
