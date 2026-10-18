//! JPLOGO through the real binary: the registry, the run's own numbering, the
//! music row, and the three reference keyframes at the ticks the reference
//! evidence pins.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/jplogo.rs`:
//! the spring tables and their 800-entry interpolation, the entry's 83
//! scroll steps, the `ZOOM.INC` row semantics at every scale, `READP.C`'s RLE
//! against the shipped `ICEKNGDM.UP`, the 512-byte `setpalarea` upload, the
//! view's 80-plane-byte rows, the palimpsest guard, the freeze after 511 and
//! the entry pre-roll against the `second3d` trace. What is left here is the
//! wiring only a process can answer: the registry, the asset path as a run
//! resolves it from the repository root, the real player's position, and the
//! frames the shell actually composes.
//!
//! **This part has no boot anchor.** Its start is the window `(17000, 17100]`
//! and the part marks no start frame, so a `--part jplogo` run numbers its own
//! frames from zero and its manifest says `index_origin: "part"`,
//! `anchor: null`.
//!
//! **The mapping is derived from the reference pictures.** Tick 0 is
//! `scrolly(400)`; ticks 1..83 are the entry's steps; tick 84 is the wait
//! between the entry and `doit`; tick 85 is `doit`'s first iteration, whose
//! counter reads 1. The three filed keyframes are the port's ticks **235,
//! 435 and 635**, and the sweep's 17100 (start address 309) is tick 35 — one
//! origin for every one of them: tick `t` is boot frame **17065 + t**.
//!
//! **All three keyframes are RGB only.** The part writes CRTC 0x0c/0x0d every
//! entry step (`JP.C:30-45`) and the oracle's indexed read of them is racy
//! (`reference-indices.md` §17.4), so no `.idx` was filed from this part.
//!
//! **17700 has a reference caveat.** The filed picture (`91d6ce6b…`) is
//! presented at that index by **none** of the four census sessions, while the
//! state the sessions settle on (`ec3de2bb…`) is held by 2/4
//! (`reference-indices.md` §17.3, §17.5). The port's model reproduces the
//! **settled state** — it is byte-identical to `-sess2`'s 17700 — and its
//! residual against the filed picture is the census's own `-sess2`-against-the-
//! filing difference: **51,445 pixels, 40.1914 %, max channel delta 255**.
//! That number is asserted, not tolerated: the port cannot match a filing its
//! own census does not hold.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/JPLOGO/...` path resolves.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of this part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-jplogo-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--part", "jplogo", "--headless", "--dump-frames", indices, "--dump-dir"])
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
    std::fs::read(dir.join(format!("jplogo_{index:06}.{extension}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/jplogo/frame_{index:06}.rgb")),
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
        let delta = (0..3).map(|channel| a[channel].abs_diff(b[channel])).max().unwrap();
        if delta != 0 {
            differing += 1;
        }
        max_delta = max_delta.max(delta);
    }
    (differing, max_delta)
}

/// The trace's `order:row` at a boot frame, as the manifest spells it.
fn trace_position(frame: u32) -> String {
    let text =
        std::fs::read_to_string(repository_root().join("reference/music/second3d/trace.csv"))
            .unwrap();
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields[0].parse::<u32>().unwrap() == frame {
            return format!("{}:{}", fields[1], fields[2]);
        }
    }
    panic!("the trace has no sample at {frame}");
}

/// The three keyframes and the tick each is drawn on (see the module comment).
const KEYFRAMES: [(u32, u32); 3] = [(17300, 235), (17500, 435), (17700, 635)];

/// The tick the reference's start window's `17100` sweep frame matches: the
/// entry's step 34, start address 309.
const SWEEP_TICK: u32 = 35;

#[test]
fn the_run_is_numbered_in_its_own_frames_and_declares_the_measured_music_row() {
    let (dir, manifest) = dump("numbering", "0,1");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["part"], "jplogo");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 400);
    // The `second3d` trace holds MUSIC1 order 96 across the part's whole
    // start window (17000..17100); the `254` at 97 is crossed at 17109.
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 96);
    assert_eq!(manifest["music"]["start_frame"], 0, "an unanchored run starts its clock");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|frame| frame["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [0, 1]);
    assert_eq!(frame_file(&dir, 0, "rgb").len(), 320 * 400 * 3);
    assert_eq!(manifest["frames"][0]["music_position"], "96:58");
    let _ = std::fs::remove_dir_all(&dir);
}

/// The entry's place in the reference's numbering: `17100` is the sweep's
/// start address 309, which the model puts at tick 35, and the trace's
/// position there is `96:62`. (The sweep's own capture is under `out/`, not
/// committed, so the picture itself is pinned by the unit tests; what this
/// dump proves is that the run reaches the sweep's tick with the trace's
/// music.)
#[test]
fn the_sweeps_17100_is_entry_step_thirty_four() {
    let (dir, manifest) = dump("sweep", &SWEEP_TICK.to_string());
    assert_eq!(manifest["frames"][0]["index"], SWEEP_TICK);
    assert_eq!(manifest["frames"][0]["part"], "jplogo");
    assert_eq!(manifest["frames"][0]["music_position"], trace_position(17100));
    assert_eq!(manifest["frames"][0]["music_position"], "96:62");
    assert_eq!(frame_file(&dir, SWEEP_TICK, "rgb").len(), 320 * 400 * 3);
    let _ = std::fs::remove_dir_all(&dir);
}

/// The three keyframes, with D19's support. 17300 is held in pairs in the
/// reference (17300 and 17301 are one `sha256`); 17500's picture holds at
/// 17501; 17700's settled state holds from the settle on. Tick `K-1` is the
/// previous state in each case.
#[test]
fn the_three_keyframes_at_the_measured_ticks() {
    let ticks: Vec<u32> = KEYFRAMES
        .iter()
        .flat_map(|(_, tick)| [tick - 1, *tick, tick + 1])
        .collect();
    let indices = ticks.iter().map(|tick| tick.to_string()).collect::<Vec<_>>().join(",");
    let (dir, manifest) = dump("keyframes", &indices);

    let check = |tick: u32, index: u32| {
        let mine = frame_file(&dir, tick, "rgb");
        let theirs = reference_frame(index);
        let (differing, max_delta) = compare(&mine, &theirs);
        println!(
            "{index} (tick {tick}): {differing} px {:.4} %, max delta {max_delta}",
            100.0 * differing as f64 / (320.0 * 400.0)
        );
        (differing, max_delta)
    };

    // 17300 and 17500 are the reference's own settled pictures.
    for (index, tick) in [(17300u32, 235u32), (17500, 435)] {
        let (differing, max_delta) = check(tick, index);
        assert_eq!(differing, 0, "{index} is not the port's frame at tick {tick}");
        assert_eq!(max_delta, 0, "{index}'s max channel delta");
        // The held second tick of the pair.
        let (differing, max_delta) = check(tick + 1, index);
        assert_eq!(differing, 0, "{index} is not held at tick {}", tick + 1);
        assert_eq!(max_delta, 0);
        // And the tick before is a different state (the reference's 17299 /
        // 17499 differ from the keyframe too).
        let before = frame_file(&dir, tick - 1, "rgb");
        assert_ne!(
            before,
            frame_file(&dir, tick, "rgb"),
            "{index}: the tick before the keyframe is the same picture"
        );
    }

    // 17700: the port reproduces the 2/4 settled state, not the 0/4 filing.
    // The residual is exactly the census's `-sess2` against the filing.
    let (differing, max_delta) = check(635, 17700);
    assert_eq!(differing, 51_445, "the census's -sess2 vs the filed picture");
    assert_eq!(max_delta, 255);
    assert_ne!(differing, 0, "the filing is not a picture the census holds");
    // The settled state is static: the settle's own support frames agree.
    let settled = frame_file(&dir, 635, "rgb");
    assert_eq!(frame_file(&dir, 636, "rgb"), settled);
    assert_eq!(frame_file(&dir, 634, "rgb"), settled);

    // The manifest's published position at each keyframe is the trace's.
    for (index, tick) in KEYFRAMES {
        let entry = manifest["frames"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["index"].as_u64() == Some(tick as u64))
            .unwrap();
        assert_eq!(
            entry["music_position"],
            serde_json::Value::String(trace_position(index)),
            "frame {index} (tick {tick}): the trace's position"
        );
    }

    // No indexed reference was filed: this part writes CRTC 0x0c/0x0d during
    // the entry and the oracle's read of it is racy (§17.4). The port's dump
    // writes one anyway; it is not compared.
    for (index, _) in KEYFRAMES {
        assert!(
            !repository_root()
                .join(format!("reference/parts/jplogo/frame_{index:06}.idx"))
                .exists(),
            "{index} has an indexed reference this test does not know how to read"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
