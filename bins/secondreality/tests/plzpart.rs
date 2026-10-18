//! PLZPART through the real binary: the registry, the run's own numbering, the
//! music row, and the five reference keyframes at the derived mapping.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/plzpart.rs`
//! (the `.INC` sizes and edges, the assembler's index expression, the
//! timetable, `dtau`, the split/pan walk, the spline, the projection and the
//! extent/clear pair). What is left over here is the wiring only a process can
//! answer: the registry, the asset paths as a run resolves them from the
//! repository root, the real player's position, and the frames the shell
//! actually composes.
//!
//! **This part has no boot anchor.** Its first graphics frame is a measured
//! window, `(5720, 5730]` (`docs/fidelity/reference-indices.md` §13), so a
//! `--part plzpart` run numbers its own frames from zero and its manifest says
//! `index_origin: "part"`, `anchor: null`. The comparison below uses the
//! mapping the plasma's own parameter phase pins — part tick 0 is boot frame
//! 5723, from `_np_zframe` reading zero there and the setmframe(0) at
//! `PLZ.C:69` — and calls it a mapping, not an anchor.
//!
//! **The plasma and the cube are one run, two screens.** At tick 277 (boot
//! 6000) the part is drawing the 320×400 plasma; at tick 2777 (boot 8500) it
//! is drawing the 320×200 cube. The manifest carries each frame's own size.
//!
//! **The cube's two keyframes are D24 measurements.** The release's cube
//! projection was measured from fourteen captured frames and adopted
//! (y scale 213, origin 100; the tree's build compiles 142/66); with it the
//! far keyframe 8500 aligns geometrically to 0.6 % of silhouette and the RGB
//! residual is 32.1 % of pixels, most of them one to three DAC steps. 9500 is
//! a close-up whose camera translation the tree's data do not carry (D24,
//! APPROXIMATIONS A23), so it stays 94.7 %. The test records both verdicts.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/PLZPART/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-plzpart-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "plzpart",
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
    std::fs::read(dir.join(format!("plzpart_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(screen: &str, index: u32) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/plzpart/{screen}/frame_{index:06}.rgb"
    )))
    .unwrap()
}

/// The part tick that draws a reference index, under the plasma's own
/// timeline: tick 0 is boot 5723 (`src/parts/plzpart.rs`'s
/// `REFERENCE_FIRST_INDEX`). A mapping, not an anchor.
const REFERENCE_FIRST_INDEX: u32 = 5723;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

/// A frame comparison: (differing pixels, max channel delta), as percentages
/// of the frame's pixels.
///
/// **The lengths are asserted first**: `zip` stops at the shorter side, so a
/// truncated dump — the one failure a comparison test exists to catch — would
/// compare a prefix and report no differences as if it had compared the frame.
fn compare(rendered: &[u8], expected: &[u8]) -> (f64, u8) {
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
    let pixels = rendered.len() / 3;
    (100.0 * differing as f64 / pixels as f64, max_delta)
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_both_screens() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices below are reached by the stated mapping, not by
    // claiming an anchor this part does not have.
    assert_eq!(tick_of(6000), 277);
    assert_eq!(tick_of(8500), 2777);
    let (dir, manifest) = dump("numbering", "0,1,2777");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "plzpart");
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 62);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1, 2777]);

    // Tick 0 is the plasma's gate: `init_plz` sets the 320×400 view inside
    // the frame call, and the engine composes a tick at the size it started
    // with, so tick 0 is still the shell's 320×200 and the plasma's screen
    // begins at tick 1.
    assert_eq!(frames[0]["width"], 320);
    assert_eq!(frames[0]["height"], 200);
    assert_eq!(frames[1]["width"], 320);
    assert_eq!(frames[1]["height"], 400);
    assert_eq!(frame_file(&dir, 1, "rgb").len(), 320 * 400 * 3);
    assert_eq!(frame_file(&dir, 1, "idx").len(), 320 * 400);

    // By 2777 the cube's `tw_opengraph` has set the 320×200 screen.
    assert_eq!(frames[2]["width"], 320);
    assert_eq!(frames[2]["height"], 200);
    assert_eq!(frame_file(&dir, 2777, "rgb").len(), 320 * 200 * 3);
    assert_eq!(frame_file(&dir, 2777, "idx").len(), 320 * 200);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_plasma_keyframes_are_close_at_the_derived_mapping() {
    // boot 6000/7000/7600 are the reference's plasma keyframes; the mapping
    // makes them ticks 277/1277/1877. The plasma's own sub-frame retrace
    // spread is why these are bounds and not byte equality: the reference
    // frame is a tear across two parameter phases (the note's §4), so no
    // fixed-phase port can be byte-identical. The bounds sit above the
    // measured values (11.2 %, 5.8 %, 4.1 %; max channel delta 16/21/17).
    let (dir, _) = dump("plasma", "277,1277,1877");
    for (boot, bound) in [(6000u32, 15.0f64), (7000, 10.0), (7600, 8.0)] {
        let tick = tick_of(boot);
        let rendered = frame_file(&dir, tick, "rgb");
        let expected = reference_frame("plasma", boot);
        let (percent, max_delta) = compare(&rendered, &expected);
        println!("plasma {boot} (tick {tick}): {percent:.3}% pixels, max delta {max_delta}");
        assert!(
            percent < bound,
            "plasma boot {boot} differs in {percent:.3}% of pixels (bound {bound}%)"
        );
        assert!(
            max_delta <= 32,
            "plasma boot {boot} has a channel delta of {max_delta}"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cube_keyframes_are_measured_and_recorded() {
    // boot 8500/9500 are the reference's cube keyframes: ticks 2777/3777.
    // The cube's gate is pinned to boot 8364 by the music (the published
    // order-68 row 13; `the_music_pre_roll_lands_the_cube_gate_on_the_reference_tick`
    // measures nine pre-polls), so the animation's frame count is the
    // reference's. With the release's measured projection (A23) the far
    // frame 8500 aligns geometrically, but its RGB still differs in 32.1 % of
    // pixels (one to three DAC steps each); 9500's close-up camera
    // translation is a release-side difference, 94.7 %. This test records the
    // verdicts and does not assert they are small; `docs/fidelity/plzpart.md`
    // §7 carries the evidence.
    let (dir, _) = dump("cube", "2777,3777");
    let mut verdicts = Vec::new();
    for (boot, recorded) in [(8500u32, 32.11f64), (9500, 94.71)] {
        let tick = tick_of(boot);
        let rendered = frame_file(&dir, tick, "rgb");
        let expected = reference_frame("cube", boot);
        let (percent, max_delta) = compare(&rendered, &expected);
        println!("cube {boot} (tick {tick}): {percent:.3}% pixels, max delta {max_delta}");
        // Pin the measured value so a change is visible and has to update the
        // record. A percentage this large is a divergence, not a tolerance.
        assert!(
            (percent - recorded).abs() < 2.0,
            "cube boot {boot} now differs in {percent:.3}% of pixels; the recorded \
             value is {recorded:.2}%"
        );
        verdicts.push((boot, percent, max_delta));
    }
    assert_eq!(verdicts.len(), 2);
    let _ = std::fs::remove_dir_all(&dir);
}
