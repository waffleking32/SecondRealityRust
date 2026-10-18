//! 3DSINFLD (COMAN) through the real binary: the registry, the run's own
//! numbering, the music row, and the four reference keyframes at the ticks the
//! reference evidence pins.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/coman.rs`: the
//! swapped 16.16 `wflip`, `SIN1024.INC` and the generator's `zwave` constants,
//! the ray march against a **direct interpretation of the shipped
//! `THELOOP.INC`**, `docopy`'s 18-row clear and 160→320 duplication, the
//! palette's formula and its unreachable overrun, the two-page flip, the
//! tick model and the entry pre-roll. What is left here is the wiring only a
//! process can answer: the registry, the asset paths as a run resolves them
//! from the repository root, the real player's position, and the frames the
//! shell actually composes.
//!
//! **This part has no boot anchor.** Its start is the window `(15000, 15100]`
//! and the part has no start frame of its own, so a `--part 3dsinfld` run
//! numbers its own frames from zero and its manifest says `index_origin:
//! "part"`, `anchor: null`.
//!
//! **The mapping is measured, and it drifts.** The picture is the
//! reference's at four ticks:
//!
//! ```text
//! reference  rendered tick  implied origin (boot - tick)
//! 15500      464, 465       15036
//! 16000      966, 967       15034
//! 16500      1470, 1471     15030
//! 16800      1772, 1773     15028
//! ```
//!
//! The picture is drawn every second tick (`dis_waitb` returns two copper
//! frames a render, `MAIN.C:105`/`:111`), so each keyframe is shown on two
//! consecutive ticks; a tick either side differs. The origins are not one
//! number: the ticks are **502, 504 and 302 apart for 500, 500 and 300 boot
//! frames**, because the reference's `dis_waitb` occasionally returns one
//! frame while the port's model is a fixed two. The four origins are
//! therefore stated per keyframe rather than as an anchor.
//!
//! **All four keyframes are RGB only.** The part writes CRTC `0x0c` every
//! frame (`MAIN.C:101-110`), and `0x80` read back as the offset register is a
//! 256-byte plane row the oracle's indexed read accepts — the read is racy —
//! so no `.idx` was filed from this part (`reference-indices.md` §16.2). The
//! test asserts the directory holds none rather than implying a comparison it
//! cannot make.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/COMAN/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-coman-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "3dsinfld",
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
    std::fs::read(dir.join(format!("3dsinfld_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/3dsinfld/frame_{index:06}.rgb")),
    )
    .unwrap()
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

/// The four keyframes and the tick each is drawn on, measured against the
/// reference (see the module comment).
const KEYFRAMES: [(u32, u32); 4] = [(15500, 464), (16000, 966), (16500, 1470), (16800, 1772)];

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_derived_music_row() {
    let (dir, manifest) = dump("numbering", "0,1");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "3dsinfld");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // The block-3 traces cross MUSIC1's order 91 -> 93 boundary at 15038
    // (`second3d`; `second3b` at 15031) and the part's start window is
    // (15000, 15100] with no start frame. The order the trace holds across
    // the window's start is 91 — **derived, not measured**, because the
    // window spans the crossing. The part's `init` pre-polls 527 frames,
    // which puts the published row at 63 and the engine's first tick on the
    // 528th poll: order 93 row 0, the gate's crossing.
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 91);
    assert_eq!(manifest["music"]["start_frame"], 0, "an unanchored run starts its clock");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames
        .iter()
        .map(|frame| frame["index"].as_u64().unwrap())
        .collect();
    assert_eq!(indices, [0, 1]);
    assert_eq!(frame_file(&dir, 0, "rgb").len(), 320 * 200 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The four keyframes, with D19's support: each picture is drawn on two
/// consecutive ticks, so `K+1` is the held picture and `K-1`/`K+2` are the
/// two-frame step the reference's sessions show (`reference-indices.md`
/// §16.3). The reference itself is keyframes only and holds no `.idx`.
#[test]
fn the_four_keyframes_at_the_measured_ticks() {
    let ticks: Vec<u32> = KEYFRAMES
        .iter()
        .flat_map(|(_, tick)| [tick - 1, *tick, tick + 1, tick + 2])
        .collect();
    let indices = ticks
        .iter()
        .map(|tick| tick.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (dir, _) = dump("keyframes", &indices);

    let check = |tick: u32, index: u32| {
        let mine = frame_file(&dir, tick, "rgb");
        let theirs = reference_frame(index);
        let (differing, max_delta) = compare(&mine, &theirs);
        println!(
            "{index} (tick {tick}): {differing} px {:.4} %, max delta {max_delta}",
            100.0 * differing as f64 / 64_000.0
        );
        assert_eq!(differing, 0, "{index} is not the port's frame at tick {tick}");
        assert_eq!(max_delta, 0, "{index}'s max channel delta");
    };
    for (index, tick) in KEYFRAMES {
        check(tick, index);
        // The picture holds for the second tick of the two-frame step.
        check(tick + 1, index);
        let key = frame_file(&dir, tick, "rgb");
        assert_ne!(
            frame_file(&dir, tick - 1, "rgb"),
            key,
            "{index}: the tick before the keyframe is the same picture"
        );
        assert_ne!(
            frame_file(&dir, tick + 2, "rgb"),
            key,
            "{index}: the step after the held pair is the same picture"
        );
    }

    // No indexed reference was filed: this part writes CRTC 0x0c every frame
    // and the oracle's read of it is racy (§16.2). The port's dump writes one
    // anyway; it is not compared.
    for (index, _) in KEYFRAMES {
        assert!(
            !repository_root()
                .join(format!("reference/parts/3dsinfld/frame_{index:06}.idx"))
                .exists(),
            "{index} has an indexed reference this test does not know how to read"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
