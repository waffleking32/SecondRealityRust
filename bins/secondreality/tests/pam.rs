//! PAM through the real binary: the boot numbering, the block-1 music clock,
//! and the reference keyframes with their D19 support frames.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/pam.rs` — the
//! `dis_sync()<10` gate, the four-tick cadence, the 40-of-42 frames, the
//! `wfade` indexing, the two-page flip, and the four keyframes composed from
//! the engine's own pieces. What is left over here is the wiring only a
//! process can answer: the registry name, the asset paths as a run resolves
//! them from the repository root, the shell's **7710 anchor** and its 7,310
//! pre-polls of the block-1 music, and the frames the shell composes.
//!
//! **The anchor is 7710.** PAM is block 1's third part (`MAIN/U2.ASM:793-799`)
//! and its first drawn frame is the flat `(251,251,251)` wash the reference
//! holds at 7710 (`reference-indices.md` §6.1, byte-identical in three
//! sessions, §6.4), so a run of this part numbers its frames in the reference's
//! own space and its manifest says `index_origin: "boot"`, `anchor: 7710`.
//!
//! **D19's support shape, with one index missing.** A moving keyframe is
//! answered from `K-1` and `K+1`; 7710's `K-1` is 7709, which the anchor makes
//! unreachable — a run that starts at 7710 walks away from it — so the dump
//! carries 7711 only. 7830 and 7900 are held (`reference-indices.md` §6.3) and
//! their neighbours are included to show the hold.
//!
//! **Only 7900 carries `.idx` in the reference, and it is not PAM's screen.**
//! The plane is 64,000 bytes of index 15, which is BEGLOGO's white field
//! (`BEG/BEG.C:18-21`, `WHITE_FIELD = 15`) read while PAM's own 63rd palette
//! copy is still in the DAC — the D15 late-read case, on a collapsed palette
//! that the coherence check cannot see through (`reference-indices.md` §1.6).
//! The port's 7900 RGB is byte-identical, and its index plane is the last
//! animation frame it drew; `docs/fidelity/pam.md` §6 records the divergence
//! with the measurement below.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/PAM/` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless run dumping the frames asked for, and the manifest it wrote.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-pam-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "pam",
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

fn frame_file(dir: &Path, suffix: &str, index: u32) -> Vec<u8> {
    std::fs::read(dir.join(format!("pam_{index:06}.{suffix}"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32, suffix: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/pam/frame_{index:06}.{suffix}"
    )))
    .unwrap()
}

/// One frame of a filed trace: the published position at a reference index.
fn trace_position(name: &str, frame: u32) -> (u8, u8) {
    let path = repository_root()
        .join("reference/music")
        .join(name)
        .join("trace.csv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    for line in text.lines().skip(1) {
        let mut fields = line.split(',');
        let number = |fields: &mut std::str::Split<'_, char>| -> u32 {
            fields.next().unwrap().trim().parse().unwrap()
        };
        let index = number(&mut fields);
        let order = number(&mut fields);
        let row = number(&mut fields);
        if index == frame {
            return (order as u8, row as u8);
        }
    }
    panic!("{name} holds no frame {frame}")
}

/// How many pixels of `rendered` differ from `expected`.
///
/// **The lengths are asserted first**: a truncated dump — the one failure a
/// comparison test exists to catch — would otherwise compare a prefix and
/// report zero differences as if it had compared the frame.
fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    rendered
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| a != b)
        .count()
}

/// The keyframes plus D19's support frames: `K-1, K, K+1` around the two
/// moving keyframes (7710's `K-1` is unreachable — see the module docs) and
/// the held triples around 7830 and 7900, which the record's own scan measured
/// as one hash each (`reference-indices.md` §6.3).
const WITH_SUPPORT: &str = "7710,7711,7759,7760,7761,7829,7830,7831,7899,7900,7901";

#[test]
fn the_run_is_numbered_in_boot_frames_and_plays_block_ones_music() {
    let (dir, manifest) = dump("numbering", "7710,7901");
    assert_eq!(manifest["part"], "pam");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 7710);
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `MAIN/U2.ASM:766-775`: block 1 takes the initial `_zplaysong(0)`, so the
    // module is MUSIC0 at order 0, and the run reaches the part's own frame by
    // pre-polling the block-1 clock's 400..7709.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 7710);

    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [7710, 7901]);

    // The gate's evidence: the published position at 7710 is the trace's own
    // row, and `dis_sync()` codes it 10 — the value `OUTTAA.C:39` waits for.
    let (order, row) = trace_position("second1b", 7710);
    assert_eq!((order, row), (13, 1));
    assert_eq!(frames[0]["music_position"], "13:1");
    assert_eq!(sr_engine::music::sync_from(order, row), 10, "the gate's code");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_four_keyframes_match_the_reference_with_their_support_frames() {
    let (dir, manifest) = dump("keyframes", WITH_SUPPORT);
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 7710);

    for index in [7710u32, 7760, 7830, 7900] {
        let differing = differing_pixels(
            &frame_file(&dir, "rgb", index),
            &reference_frame(index, "rgb"),
        );
        println!(
            "boot {index}: {differing}/64000 px differ ({:.4} %)",
            100.0 * differing as f64 / 64_000.0
        );
        assert_eq!(differing, 0, "boot {index}: the frame is not the reference's");
    }

    // D19's support reading, and the record's own (§6.3): 7710's wash holds
    // one frame either side, 7760 moves, 7830 and 7900 hold.
    let pixel = |index: u32| frame_file(&dir, "rgb", index);
    assert_eq!(pixel(7711), pixel(7710), "7711 holds 7710's wash");
    assert_ne!(pixel(7759), pixel(7760), "7759 is the frame before");
    assert_eq!(pixel(7761), pixel(7760), "7761 holds 7760");
    assert_eq!(pixel(7829), pixel(7830), "7829 holds 7830");
    assert_eq!(pixel(7831), pixel(7830), "7831 holds 7830");
    assert_eq!(pixel(7899), pixel(7900), "7899 holds 7900");
    assert_eq!(pixel(7901), pixel(7900), "7901 holds 7900");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame() {
    // The reference's 7900 `.idx` is 64,000 bytes of index 15; its manifest's
    // palette entry 15 is [62,62,62], which widens to the frame's uniform
    // 251. That is BEGLOGO's `WHITE_FIELD` (`BEG/BEG.C:18-21`: `memset` with
    // 15) under PAM's still-loaded 63rd copy, read late (D15) — not the last
    // animation frame PAM drew. The RGB agrees because both screens are white.
    let reference_idx = reference_frame(7900, "idx");
    assert_eq!(reference_idx.len(), 64_000);
    assert!(
        reference_idx.iter().all(|index| *index == 15),
        "the reference's plane is BEGLOGO's white field"
    );
    let reference_rgb = reference_frame(7900, "rgb");
    assert!(
        reference_rgb
            .chunks_exact(3)
            .all(|rgb| rgb == [251, 251, 251]),
        "and it displays as the flat wash"
    );

    let (dir, _) = dump("indexed", "7900");
    let rendered_rgb = frame_file(&dir, "rgb", 7900);
    assert_eq!(
        differing_pixels(&rendered_rgb, &reference_rgb),
        0,
        "the displayed frame is byte-identical"
    );
    // The port's plane is the animation's last drawn frame (0-based 39): the
    // loop's `if(f<=40)` draws 40 frames and stops.
    let ani = std::fs::read(repository_root().join("assets/PAM/OUT.ANI")).unwrap();
    let frames = sr_assets::load_frames(&ani, sr_assets::Format::Ani).unwrap();
    assert_eq!(frames.len(), 42);
    assert_eq!(
        frame_file(&dir, "idx", 7900),
        frames[39].pixels(),
        "the port's plane is animation frame 39"
    );

    // The measured divergence, as `cargo xtask compare` reports it: indices
    // differ where the animation frame is not index 15, and the widest index
    // delta is the frame's highest index (209) less 15.
    let port_idx = frame_file(&dir, "idx", 7900);
    let differing = port_idx.iter().zip(&reference_idx).filter(|(a, b)| a != b).count();
    let max_delta = port_idx
        .iter()
        .zip(&reference_idx)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap();
    println!("7900 index plane: {differing}/64000 px differ, max delta {max_delta}");
    assert_eq!(differing, 60_375, "the measured indexed divergence");
    assert_eq!(max_delta, 194);

    let _ = std::fs::remove_dir_all(&dir);
}
