//! MNTSCRL through the real binary: the registry, the run's own numbering, the
//! music's head start, and the reference keyframes at the mapping the evidence
//! gives.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/mntscrl.rs`,
//! including the exact reference comparisons that fix the release's 71-tick
//! first wait and its 640 `scp` bound. What is left over here is the wiring
//! only a process can answer: the registry, the asset paths as a run resolves
//! them from the repository root, the real player's position, and the frames
//! the shell actually composes.
//!
//! **This part has no boot anchor.** Its first graphics frame is a measured
//! window, `(415, 420]` (`docs/fidelity/reference-indices.md` §11.5), so a
//! `--part mntscrl` run numbers its own frames from zero and its manifest says
//! `index_origin: "part"`, `anchor: null`. The reference comparison below uses
//! the mapping the reference itself pins — part tick 0 is boot frame 419, from
//! the fade-in's recorded frame means — and calls it a mapping, not an anchor.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/FOREST/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-mntscrl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "mntscrl",
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
    std::fs::read(dir.join(format!("mntscrl_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/mntscrl/frame_{index:06}.{extension}"
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

/// The part tick that draws a reference index, under the release's own
/// timeline: tick 0 is boot 419, the number the fade-in's recorded means pin
/// (`src/parts/mntscrl.rs`'s `REFERENCE_FIRST_INDEX`). A mapping, not an
/// anchor.
const REFERENCE_FIRST_INDEX: u32 = 419;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices below are reached by the stated mapping, not by
    // claiming an anchor this part does not have.
    assert_eq!(tick_of(500), 81);
    assert_eq!(tick_of(1100), 681);
    let (dir, manifest) = dump("numbering", "0,81");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "mntscrl");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 42);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|f| f["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 81]);

    // `:121-123`: tick 0 blits the 64,000-byte picture and sets the DAC
    // black, so the frame is black RGB over the picture's own indices.
    let first_indices = frame_file(&dir, 0, "idx");
    assert_eq!(first_indices.len(), 320 * 200);
    let first_rgb = frame_file(&dir, 0, "rgb");
    assert!(
        first_rgb.iter().all(|byte| *byte == 0),
        "tick 0's DAC is black"
    );
    // The hold that follows does not draw: its index plane is tick 0's.
    let held = frame_file(&dir, 81, "idx");
    assert_eq!(first_indices, held);
    // And by tick 81 (boot 500) the hold's DAC is up: the reference's frame.
    assert_eq!(held, reference_frame(500, "idx"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_music_is_eleven_polls_in_at_the_parts_first_tick() {
    // The block-3 restart starts MUSIC1@42 on the music clock's frame 408
    // (`MAIN/U2.ASM:856-863`, `shell::MUSIC_START_FRAME`), and this part's
    // tick 0 is boot 419 — so the reference's part saw **11 polls** of it.
    // An unanchored run starts the player with the part; `Mntscrl::init`
    // pre-polls those 11, and this is the observable consequence: at tick 81
    // (boot 500) the published position is `42:10`, which is what
    // `reference/music/second3/trace.csv` holds at frame 500. Without the
    // pre-roll it would read `42:8` (82 polls, trace frame 490).
    let (dir, manifest) = dump("music", "0,81");
    let positions: Vec<&str> = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["music_position"].as_str().unwrap())
        .collect();
    assert_eq!(positions, ["0:0", "42:10"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_held_opening_is_the_references_frame_500_in_both_planes() {
    // §11.2: 500 is the music wait's hold, and its `.idx` is the only one the
    // reference files for this part. Both planes are compared; the indexed
    // one is the stricter (it makes the DAC-widening rule irrelevant).
    let (dir, _) = dump("held", "81");
    let indices = frame_file(&dir, 81, "idx");
    assert_eq!(
        differing_bytes(&indices, &reference_frame(500, "idx")),
        0,
        "500's index plane"
    );
    let rgb = frame_file(&dir, 81, "rgb");
    assert_eq!(
        differing_bytes(&rgb, &reference_frame(500, "rgb")),
        0,
        "500's RGB"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_four_keyframes_match_the_reference() {
    // The part's own timeline is the release's: tick 0 is boot 419, the first
    // wait is 71 ticks, the pre-roll 127, the main loop from tick 263. The
    // reference's frame at boot `K` is the run's tick `K - 419`, and all four
    // keyframes compare byte for byte through the real binary. 500 is the held
    // opening, compared indexed; the three scroll keyframes are RGB-only in
    // the reference.
    //
    // Each keyframe is dumped with D19's `K-1, K, K+1` support. The scroller
    // moves 2.4-3.1 % of its pixels a frame, so the neighbours are different
    // pictures — which is what makes the mapping exact and not a search's
    // loosest fit.
    let (dir, manifest) = dump(
        "keyframes",
        "80,81,82,680,681,682,1280,1281,1282,1880,1881,1882",
    );

    assert_eq!(
        differing_bytes(&frame_file(&dir, 81, "idx"), &reference_frame(500, "idx")),
        0,
        "500's index plane"
    );
    assert_eq!(
        differing_bytes(&frame_file(&dir, 81, "rgb"), &reference_frame(500, "rgb")),
        0,
        "500's RGB"
    );

    for index in [1100u32, 1700, 2300] {
        let tick = tick_of(index);
        let key = frame_file(&dir, tick, "rgb");
        assert_eq!(
            differing_bytes(&key, &reference_frame(index, "rgb")),
            0,
            "boot {index} (tick {tick})"
        );
        for neighbour in [tick - 1, tick + 1] {
            assert_ne!(
                frame_file(&dir, neighbour, "rgb"),
                key,
                "boot {index}: tick {neighbour} is not distinguishable"
            );
        }
    }
    assert_eq!(manifest["index_origin"], "part");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_block_three_run_starts_here_and_is_numbered_the_same_way() {
    // `SECOND 3` begins at MNTSCRL (`MAIN/U2.ASM:856-866`), so the block run
    // now starts instead of refusing. It carries no anchor either — the part
    // is a block entry whose first frame is a window, not a measured index —
    // and its music is the same MUSIC1@42.
    let dir = std::env::temp_dir().join(format!("sr-mntscrl-{}-block3", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--block",
            "3",
            "--headless",
            "--dump-frames",
            "0,81",
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
    let manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["part"], "mntscrl");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["music"]["start_order"], 42);
    assert_eq!(frame_file(&dir, 81, "idx"), reference_frame(500, "idx"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_releases_first_wait_is_seventy_one_ticks() {
    // The reference's last held frame is boot 554 and its pre-roll's first
    // `scr` is boot 555 — ticks 135 and 136 with tick 0 at 419. The wait has
    // no visible boundary of its own in the music: the published position is
    // order 42 row 16 at both 134 and 135 (`zplus` 0, so `musplus` is -32 at
    // both), which is why the release's duration is a constant rather than a
    // threshold (`docs/fidelity/mntscrl.md` §6).
    let (dir, manifest) = dump("wait", "81,134,135,136");
    let held = frame_file(&dir, 81, "rgb");
    assert_eq!(frame_file(&dir, 134, "rgb"), held, "tick 134 still holds");
    assert_eq!(
        frame_file(&dir, 135, "rgb"),
        held,
        "the wait's last tick draws nothing"
    );
    assert_ne!(
        frame_file(&dir, 136, "rgb"),
        held,
        "the pre-roll's first scr"
    );
    let positions: Vec<&str> = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["music_position"].as_str().unwrap())
        .collect();
    assert_eq!(positions, ["42:10", "42:16", "42:16", "42:17"]);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_real_players_cue_fades_the_part_out_and_the_run_holds_black() {
    // The tree's `:196-203` cue is `dis_musplus() == -11` exactly. With the
    // release's wait the main loop starts at tick 263, and the first -11
    // after it is order 45 row 53: the dump reads `45:53` at 1,979 (the
    // fade's full-palette upload) and the frame is one step down at 1,980.
    // The 65-tick fade (`:207-214`) ends by 2,050: 2,040 is its tail, and
    // 2,050 and 2,100 are black — the part has retired and the run holds its
    // screen.
    let (dir, manifest) = dump("fade", "1960,1979,1980,2040,2050,2100");
    let positions: Vec<&str> = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .map(|frame| frame["music_position"].as_str().unwrap())
        .collect();
    assert_eq!(
        positions,
        ["45:50", "45:53", "45:53", "45:60", "45:61", "47:8"]
    );

    let ink = |index: u32| -> usize {
        frame_file(&dir, index, "rgb")
            .iter()
            .filter(|byte| **byte != 0)
            .count()
    };
    assert!(ink(1960) > 180_000, "1,960 is still the full scroller");
    assert!(ink(1979) > 180_000, "1,979 uploads the full palette");
    assert!(ink(1980) < ink(1979), "the fade's first step is visible");
    assert!(ink(2040) < 5_000, "2,040 is the fade's tail");
    for index in [2050u32, 2100] {
        let rgb = frame_file(&dir, index, "rgb");
        assert!(
            rgb.iter().all(|byte| *byte == 0),
            "tick {index} is not black"
        );
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
