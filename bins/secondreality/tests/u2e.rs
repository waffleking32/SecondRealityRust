//! U2E through the real binary: block 4's numbering, its intro anchor, and the
//! four filed keyframes.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/u2e.rs`: the
//! 100-wait intro, the fadeset's pixels, the measured fade-out at boot 490 and
//! 500, the main palette and its letterbox hold, the gate, the first-draw skip,
//! the two-tick cadence and the teardown's rotated DAC write. What is left over
//! here is the wiring only a process can answer: the registry name, the asset
//! paths as a run resolves them from the repository root, the shell's **418
//! anchor** and its block-4 music clock, and the frames the shell composes.
//!
//! **The intro is byte-exact.** A `--block 4` run's 460, 467, 470, 480, 490,
//! 500, 530, 590 and 660 compare at **0 differing** against the reference
//! sessions' captures (`out/u2e-dense-a`, `out/u2e-start`,
//! `out/b4b-u2e-sweep2`); the commands are in `docs/fidelity/u2e.md` §3. That
//! pins the anchor: the fadeset's white band is boot 467 (two independent dense
//! scans, `reference-indices.md` §18.7) and the source draws it after 49
//! pre-visible waits, so the part's tick 0 is 418.
//!
//! **The keyframes diverge, and the divergence is measured rather than
//! fitted.** The port's displayed stream state at boot `t` is `(t - 698)/2`;
//! the filed keyframes are `(t - 702)/2` — the same slope to the frame, a
//! four-tick intercept (two stream states). The port's music clock reaches
//! order 19 at 698 where the filed `second4` trace reads it at 699, and the
//! keyframe capture is another session: the offset is D17's session phase, the
//! same class A29 registers for U2A's 7300. At the reference's **own** state
//! (the port's `K - 4`) the silhouettes are exact — **0** pixels differ between
//! "background" and "object" — and the remaining colour residual is **189,
//! 7118, 231 and 1177** pixels on the Gouraud faces of `s01`, `Tree01g` and
//! `Car02`, the release-vs-tree Gouraud difference A28 registers. The ignored
//! unit test `search_reference_states` reproduces both the states and the
//! attribution. No constant, anchor or timing was adjusted to chase a number
//! (D24/D17).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/VISU/` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless block-4 run dumping the frames asked for, and the manifest it
/// wrote.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-u2e-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--block",
            "4",
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

/// The manifest's entry for `index`.
fn entry(manifest: &serde_json::Value, index: u32) -> &serde_json::Value {
    manifest["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["index"].as_u64() == Some(u64::from(index)))
        .unwrap_or_else(|| panic!("the manifest holds no frame {index}"))
}

/// One dumped file, by the manifest's own name for it.
fn dumped_file(dir: &Path, manifest: &serde_json::Value, index: u32, suffix: &str) -> Vec<u8> {
    let name = entry(manifest, index)[suffix].as_str().unwrap();
    std::fs::read(dir.join(name))
        .unwrap_or_else(|error| panic!("{}/{name}: {error}", dir.display()))
}

/// The reference's own file for a U2E frame, from the tree it is committed in.
fn reference_frame(index: u32) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/u2e/frame_{index:06}.rgb"
    )))
    .unwrap()
}

/// How many RGB pixels of `rendered` differ from `expected`.
///
/// **The lengths are asserted first**: a truncated dump — the one failure a
/// comparison test exists to catch — would otherwise compare a prefix and
/// report zero differences as if it had compared the frame.
fn differing(rendered: &[u8], expected: &[u8]) -> usize {
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

/// How many pixels disagree about being background. U2E clears every page with
/// `vid_clear255()` and patches entry 255 black, so "background" is exactly
/// `(0,0,0)`.
fn silhouette_differing(rendered: &[u8], expected: &[u8]) -> usize {
    fn background(pixel: &[u8]) -> bool {
        pixel == [0, 0, 0]
    }
    assert_eq!(rendered.len(), expected.len(), "the frame lengths differ");
    rendered
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| background(a) != background(b))
        .count()
}

/// One frame of the filed `second4` trace: the published position at a
/// reference index.
fn trace_position(frame: u32) -> (u8, u8) {
    let path = repository_root().join("reference/music/second4/trace.csv");
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
    panic!("second4 holds no frame {frame}")
}

/// The keyframes and the port's aligned state with its supports: for each
/// filed index `K`, the port's frame at `K - 4` (the reference's own stream
/// state), its two-tick hold `K - 3`, and the keyframe's `K - 1`, `K`, `K + 1`.
/// Every index here is read by an assertion below.
const FRAMES: &str = "996,997,999,1000,1001,\
                      1596,1597,1599,1600,1601,\
                      2596,2597,2599,2600,2601,\
                      3596,3597,3599,3600,3601";

#[test]
fn the_run_is_numbered_from_the_intro_anchor_and_plays_the_block_four_music() {
    let (dir, manifest) = dump("numbering", FRAMES);
    assert_eq!(manifest["part"], "u2e");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 418, "the fadeset at 467 minus 49 waits");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `MAIN/U2.ASM:880-885`: block 4 restarts MUSIC0 at order 18, whose clock
    // starts at 408; the run begins at 418 and pre-polls ten frames.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 18);
    assert_eq!(manifest["music"]["start_frame"], 418);
    // The block's plan is U2E first; every dumped frame is its screen.
    for index in [1000u32, 1600, 2600, 3600] {
        assert_eq!(entry(&manifest, index)["part"], "u2e");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_block_four_clock_reaches_order_19_at_698() {
    // The port's player is one frame ahead of `second4` at the gate: order 19
    // is published at 698, where the trace reads 18:31 (699 is 19:0 in both).
    // This is the fact docs/fidelity/u2e.md §5's session-phase intercept
    // rests on.
    let (dir, manifest) = dump("gate", "698,699");
    assert_eq!(entry(&manifest, 698)["music_position"], "19:0");
    assert_eq!(entry(&manifest, 699)["music_position"], "19:0");
    assert_eq!(trace_position(698), (18, 31));
    assert_eq!(trace_position(699), (19, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_keyframe_music_is_the_second4_trace_at_three_of_four_indices() {
    let (dir, manifest) = dump("music", FRAMES);
    for index in [1000u32, 2600, 3600] {
        let (order, row) = trace_position(index);
        assert_eq!(
            entry(&manifest, index)["music_position"],
            format!("{order}:{row}"),
            "the trace's own position at {index}"
        );
    }
    // 1600 is one row boundary away from `second4`: the port publishes 20:39
    // and the trace 20:38. The row's 8.76-frame cadence puts the boundary
    // within a frame of 1600, so this is the same session-phase class as the
    // keyframes themselves (D17), not a different clock.
    assert_eq!(entry(&manifest, 1600)["music_position"], "20:39");
    assert_eq!(trace_position(1600), (20, 38));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_keyframes_record_the_measured_session_phase_and_gouraud_residual() {
    let (dir, manifest) = dump("keyframes", FRAMES);
    // The port's displayed state at boot `t` is `(t - 698)/2`; the filed
    // keyframes are `(t - 702)/2`. The exact-keyframe counts are the
    // consequence — two stream states of D17 session phase — and the aligned
    // counts are the release's Gouraud difference (A28) at the reference's own
    // state.
    let measured: &[(u32, usize, usize)] = &[
        (1000, 11006, 189),
        (1600, 11805, 7118),
        (2600, 17278, 231),
        (3600, 16234, 1177),
    ];
    for &(index, exact, aligned) in measured {
        let reference = reference_frame(index);
        let at_key = dumped_file(&dir, &manifest, index, "rgb");
        let count = differing(&at_key, &reference);
        println!("boot {index}: {count}/64000 px differ at K, {aligned} at K-4");
        assert_eq!(count, exact, "the measured exact-keyframe divergence at {index}");
        let at_state = dumped_file(&dir, &manifest, index - 4, "rgb");
        assert_eq!(
            differing(&at_state, &reference),
            aligned,
            "the measured residual at the reference's own state, boot {index}"
        );
        // The pose is exact: only colours inside the objects differ.
        assert_eq!(
            silhouette_differing(&at_state, &reference),
            0,
            "boot {index}: the aligned silhouette is the reference's"
        );
        // The aligned state is held for two ticks too, which is what `K - 3`
        // is dumped for.
        assert_eq!(
            dumped_file(&dir, &manifest, index - 4, "rgb"),
            dumped_file(&dir, &manifest, index - 3, "rgb"),
            "boot {index}: the aligned state holds through K-3"
        );
        // The supports move: the port holds each state for two ticks, so K and
        // K+1 are the same page and K-1 is the previous stream state. D19's
        // neighbour capture is what makes that visible.
        assert_eq!(
            dumped_file(&dir, &manifest, index, "rgb"),
            dumped_file(&dir, &manifest, index + 1, "rgb"),
            "boot {index}: the two-retrace page holds through K+1"
        );
        assert_ne!(
            dumped_file(&dir, &manifest, index - 1, "rgb"),
            dumped_file(&dir, &manifest, index, "rgb"),
            "boot {index}: K-1 is the previous stream state"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
