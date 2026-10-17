//! RAYSCRL through the real binary: the registry, the run's own numbering, the
//! block-3 music position, and the reference keyframes at the mapping the
//! evidence gives.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/rayscrl.rs`,
//! including the exact reference comparisons that fix the release's seven-tick
//! gate confirmation, the music pre-roll and the fade's buggy palette ramps.
//! What is left over here is the wiring only a process can answer: the
//! registry, the asset paths as a run resolves them from the repository root,
//! the real player's position, and the frames the shell actually composes.
//!
//! **This part has no boot anchor.** Its first drawn frame is a measured
//! window, `(12900, 13000]` (`docs/fidelity/reference-indices.md` §15), so a
//! `--part rayscrl` run numbers its own frames from zero and its manifest says
//! `index_origin: "part"`, `anchor: null`. The reference comparison below uses
//! the mapping the reference itself pins — **boot = 12962 + tick** — and calls
//! it a mapping, not an anchor. The number is the frame the reference's
//! published order 88 row 0 first stands at; the blit lands on tick 7 (boot
//! 12969), the main loop's first draw on tick 654 (boot 13616) and the fade's
//! first upload on tick 1988 (boot 14950).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/WATER/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-rayscrl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "rayscrl",
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
    std::fs::read(dir.join(format!("rayscrl_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/rayscrl/frame_{index:06}.{extension}"
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
    rendered
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| a != b)
        .count()
}

/// The part tick that draws a reference index: the reference's published
/// order 88 row 0 stands at boot 12962 and the port's tick 0 is that frame
/// (`src/parts/rayscrl.rs`'s `REFERENCE_FIRST_INDEX`). A mapping, not an
/// anchor.
const REFERENCE_FIRST_INDEX: u32 = 12_962;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices below are reached by the stated mapping.
    assert_eq!(tick_of(13_000), 38);
    assert_eq!(tick_of(14_500), 1_538);
    let (dir, manifest) = dump("numbering", "0,7");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "rayscrl");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 86);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|f| f["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 7]);

    // Tick 0 is the init wait's first poll: nothing is drawn, but the music is
    // already at the reference's blit-time state (order 88 row 0).
    assert_eq!(frames[0]["music_position"], "88:0");
    // Tick 7 is `:111-113`: the 64,000-byte picture with the DAC black.
    let first_indices = frame_file(&dir, 7, "idx");
    assert_eq!(first_indices.len(), 320 * 200);
    let first_rgb = frame_file(&dir, 7, "rgb");
    assert!(
        first_rgb.iter().all(|byte| *byte == 0),
        "the blit's DAC is black"
    );
    // The picture is BKG.CLX's own pixels, rebased.
    let bkg = std::fs::read(repository_root().join("assets/WATER/BKG.CLX")).unwrap();
    assert_eq!(first_indices, bkg[778..778 + 64_000]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_music_is_the_block_three_position_at_every_keyframe() {
    // The pre-roll walks the `254` at order-list entry 87 into order 88 with
    // `zplus` 2, so the published position at each mapped tick is the trace's
    // (`reference/music/second3b/trace.csv` frames 13000, 13500, 14000,
    // 14500). Without it the init's `musplus >= 0` wait would never pass.
    let (dir, manifest) = dump("music", "38,538,1038,1538");
    let positions: Vec<&str> = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["music_position"].as_str().unwrap())
        .collect();
    assert_eq!(positions, ["88:4", "89:2", "90:0", "90:62"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_four_keyframes_match_the_reference_at_the_mapping() {
    // The mapping: boot = 12962 + tick. 13000 is the fade's hold (`:120-131`,
    // compared RGB-only — the reference files no `.idx`); 13500 is the held
    // background, the reference's one indexed keyframe; 14000 and 14500 are
    // RGB-only. D19's support: `K-1, K, K+1` around every moving keyframe, and
    // the held 13500's neighbours are the same picture on both sides.
    let mut indices = vec![
        tick_of(13_000),
        tick_of(13_500),
        tick_of(14_000),
        tick_of(14_500),
    ];
    for tick in [
        tick_of(13_000),
        tick_of(13_500),
        tick_of(14_000),
        tick_of(14_500),
    ] {
        indices.extend([tick - 1, tick + 1]);
    }
    indices.sort_unstable();
    indices.dedup();
    let list: Vec<String> = indices.iter().map(|index| index.to_string()).collect();
    let (dir, manifest) = dump("keyframes", &list.join(","));
    assert_eq!(manifest["index_origin"], "part");

    // 13000: mid fade-in.
    assert_eq!(
        differing_bytes(&frame_file(&dir, tick_of(13_000), "rgb"), &reference_frame(13_000, "rgb")),
        0,
        "boot 13000 (tick {})",
        tick_of(13_000)
    );
    // Boot 13000 is the *hold* of the k=14 upload: `:120-131` uploads on odd
    // `y` only, so tick 37 (y=29) and tick 38 (y=30) are one picture. The
    // reference's 12999→13000 differs by 64 px only because the capture caught
    // the palette upload mid-loop (`reference-indices.md` §15.3); the port's
    // upload is one tick, so the pair is byte-identical here. The forward
    // neighbour (y=31, the k=15 upload) is the different picture.
    assert_eq!(
        frame_file(&dir, tick_of(13_000) - 1, "rgb"),
        frame_file(&dir, tick_of(13_000), "rgb"),
        "boot 13000: the upload's hold"
    );
    assert_ne!(
        frame_file(&dir, tick_of(13_000) + 1, "rgb"),
        frame_file(&dir, tick_of(13_000), "rgb"),
        "boot 13000: the next upload"
    );

    // 13500: the stage-2 hold, in both planes. Its neighbours are the same
    // bytes — the reference's held keyframe.
    let held = frame_file(&dir, tick_of(13_500), "idx");
    assert_eq!(
        differing_bytes(&held, &reference_frame(13_500, "idx")),
        0,
        "boot 13500's index plane"
    );
    assert_eq!(
        differing_bytes(&frame_file(&dir, tick_of(13_500), "rgb"), &reference_frame(13_500, "rgb")),
        0,
        "boot 13500's RGB"
    );
    for neighbour in [tick_of(13_500) - 1, tick_of(13_500) + 1] {
        assert_eq!(
            frame_file(&dir, neighbour, "idx"),
            held,
            "boot 13500: tick {neighbour} is the same held picture"
        );
    }

    // 14000 and 14500: the main loop. The residual is the release's embedded
    // MIEKKA differing from the tree's asset at the handful of bytes these two
    // frames expose (`docs/fidelity/rayscrl.md` §6); the neighbours are
    // different pictures, which is what makes the phase exact.
    for (index, expected) in [(14_000u32, 3usize), (14_500, 42)] {
        let key = frame_file(&dir, tick_of(index), "rgb");
        assert_eq!(
            differing_pixels(&key, &reference_frame(index, "rgb")),
            expected,
            "boot {index} (tick {})",
            tick_of(index)
        );
        for neighbour in [tick_of(index) - 1, tick_of(index) + 1] {
            assert_ne!(
                frame_file(&dir, neighbour, "rgb"),
                key,
                "boot {index}: tick {neighbour} is not distinguishable"
            );
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The guard above can fail, which is the point of it: a dump cut short is
/// exactly the case a bare `zip` would report as identical.
#[test]
#[should_panic(expected = "a short read is not a comparison")]
fn a_truncated_frame_is_refused_rather_than_counted() {
    differing_bytes(&[0; 6], &[0; 9]);
}
