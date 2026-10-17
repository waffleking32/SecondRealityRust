//! DDSTARS through the real binary: the registry, the run's own numbering,
//! the block-U music row, and the reference keyframes at the mapping the
//! evidence gives.
//!
//! The part's arithmetic is unit-tested beside it in
//! `src/parts/ddstars.rs`: the LCG's first values, the projection tables and
//! their bounds, the ring's 64-frame wrap and its never-cleared persistence,
//! the text strip's two planes, the 33-tick palette fade, the exit, and the
//! four keyframes themselves byte for byte. What is left over here is the
//! wiring only a process can answer: the registry, the asset path as a run
//! resolves it from the repository root, the shell's music row and the frames
//! the shell actually composes.
//!
//! **This part has no boot anchor.** The reference's earliest evidence is a
//! probe at 415 with the DDSTARS screen already up
//! (`docs/fidelity/reference-indices.md` §22) — a window, and the reference
//! files no frame at the part's start — so a `--part ddstars` run numbers its
//! own frames from zero and its manifest says `index_origin: "part"`,
//! `anchor: null`. The comparison below uses the mapping the reference itself
//! pins — **boot = 414 + tick** — and calls it a mapping, not an anchor: the
//! sweep's 500-8000 frames and the four keyframes reproduce at that offset,
//! and tick 1 (boot 415) is the first displayed field. The reference's
//! `.rgb` files are the only planes there are (the emulator refuses the
//! indexed read, §22.5), so the comparison is RGB only.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/DDSTARS/TEXTS.16` path resolves.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-ddstars-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "ddstars",
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
    std::fs::read(dir.join(format!("ddstars_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/ddstars/frame_{index:06}.{extension}"
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

/// The part tick that draws a reference index: **boot = 414 + tick**. A
/// mapping, not an anchor; the unit tests pin the number and
/// `src/parts/ddstars.rs`'s module docs carry the derivation.
const REFERENCE_FIRST_INDEX: u32 = 414;

fn tick_of(index: u32) -> u32 {
    index - REFERENCE_FIRST_INDEX
}

#[test]
fn the_run_is_numbered_in_its_own_frames_and_draws_320x400() {
    // No anchor: the manifest says the indices are the run's own, and the
    // reference indices below are reached by the stated mapping.
    assert_eq!(tick_of(1900), 1486);
    assert_eq!(tick_of(8000), 7586);
    let (dir, manifest) = dump("numbering", "0,1");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "ddstars");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 400);
    // `MAIN/U2.ASM:889-894`: `SECOND U` restarts MUSIC0 at order 70. The
    // part never reads the player; this is the shell's row.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 70);
    assert_eq!(manifest["music"]["start_frame"], 0);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|f| f["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1]);

    // Tick 0 presents the page the loop has not written yet: the mode set's
    // cleared picture, with the first fade's `b = 8` making every entry the
    // part uses black. Tick 1 presents tick 0's blit — the first field —
    // under `b = 16`.
    let first = frame_file(&dir, 0, "rgb");
    assert_eq!(first.len(), 320 * 400 * 3);
    assert!(first.iter().all(|byte| *byte == 0), "tick 0 is the cleared page");
    let first_indices = frame_file(&dir, 0, "idx");
    assert_eq!(first_indices.len(), 320 * 400);
    assert!(first_indices.iter().all(|pixel| *pixel == 0));
    let second = frame_file(&dir, 1, "rgb");
    assert!(
        second.iter().any(|byte| *byte != 0),
        "tick 1 shows the first star pass"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_four_keyframes_and_their_support_frames_match_at_the_mapping() {
    // 1900 / 3000 / 5000 / 8000 are RGB-only and all moving
    // (`reference-indices.md` §22.2-22.3). D19's support: `K-1, K, K+1`
    // around every keyframe, so the mapping is exact and not a search's
    // nearest fit.
    let keyframes = [1900u32, 3000, 5000, 8000];
    let mut indices = Vec::new();
    for index in keyframes {
        let tick = tick_of(index);
        indices.extend([tick - 1, tick, tick + 1]);
    }
    indices.sort_unstable();
    indices.dedup();
    let list: Vec<String> = indices.iter().map(|index| index.to_string()).collect();
    let (dir, manifest) = dump("keyframes", &list.join(","));
    assert_eq!(manifest["index_origin"], "part");

    for index in keyframes {
        let tick = tick_of(index);
        let key = frame_file(&dir, tick, "rgb");
        let theirs = reference_frame(index, "rgb");
        assert_eq!(
            differing_bytes(&key, &theirs),
            0,
            "boot {index} (tick {tick})"
        );

        // D19: the neighbours are different pictures. The reference's own
        // K-1/K+1 hashes are `reference-indices.md` §22.3's: the port's
        // neighbours are those pictures, so they cannot equal the keyframe.
        for neighbour in [tick - 1, tick + 1] {
            assert_ne!(
                frame_file(&dir, neighbour, "rgb"),
                key,
                "boot {index}: tick {neighbour} is not distinguishable"
            );
        }
    }

    // The music is the shell's only. The filed `hiddenu2` trace is
    // `SECOND U`'s own boot-relative music at frame 8000; the port's player
    // at the mapped tick is three rows behind it. The arithmetic: the run's
    // music clock starts at D27's frame 408, the tick mapping's zero is boot
    // 414 (six frames later), and the hiddenu2 session's phase is **-16
    // frames from the boot's** (`cargo test -p sr-audio --test trace --
    // --nocapture`; docs/fidelity/music.md), so the trace's published state
    // runs 414 - (408 - 16) = 22 frames ahead of the port's. Trace frame
    // 8000 - 22 = 7978 reads 30:51, the port's position at tick 7586. The
    // part reads none of it; this records where the shell put it.
    let position = manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["index"] == u64::from(tick_of(8000)))
        .and_then(|frame| frame["music_position"].as_str())
        .expect("the run has a player");
    assert_eq!(position, "30:51");
    let trace = std::fs::read_to_string(repository_root().join("reference/music/hiddenu2/trace.csv"))
        .unwrap();
    let position_at = |frame: u32| -> (String, String) {
        let row = trace
            .lines()
            .find(|line| line.starts_with(&format!("{frame},")))
            .unwrap_or_else(|| panic!("the trace holds frame {frame}"));
        let fields: Vec<&str> = row.split(',').collect();
        (fields[1].to_string(), fields[2].to_string())
    };
    // The mapping's 8000, and the frame 22 earlier that the port's clock
    // answers for.
    assert_eq!(
        position_at(8000),
        ("30".to_string(), "54".to_string()),
        "the trace's boot-8000 position"
    );
    assert_eq!(
        position_at(7978),
        ("30".to_string(), "51".to_string()),
        "the port's position at the mapped tick"
    );
    assert_eq!(8000 - 7978, 22, "6 frames of clock start + 16 of session phase");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The guard above can fail, which is the point of it: a dump cut short is
/// exactly the case a bare `zip` would report as identical.
#[test]
#[should_panic(expected = "a short read is not a comparison")]
fn a_truncated_frame_is_refused_rather_than_counted() {
    differing_bytes(&[0; 6], &[0; 9]);
}
