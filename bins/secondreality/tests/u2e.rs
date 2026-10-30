//! U2E through the real binary: block 4's numbering from the derived 418
//! anchor, its music against the `second4` trace, and the keyframes' session
//! phase and silhouettes.
//!
//! The `K - 4` residuals are rediff's `u2e` entry (A30). The filed keyframes
//! show the stream state the port shows four ticks earlier (A31), so at `K` the
//! port differs by a whole state; at `K - 4` the silhouettes are exact and only
//! the Gouraud shading differs (`docs/fidelity/u2e.md` §5).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/VISU/` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless block-4 run dumping the frames asked for. It must exit 0: a hole
/// in a dump would read as a frame that matched.
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

/// How many RGB pixels of `rendered` differ from `expected`; a short read is
/// refused rather than compared as a prefix.
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

#[test]
fn the_run_is_numbered_from_the_intro_anchor_and_plays_the_block_four_music() {
    let (dir, manifest) = dump("numbering", "1000,1600,2600,3600");
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
    for index in [1000u32, 1600, 2600, 3600] {
        assert_eq!(entry(&manifest, index)["part"], "u2e");
    }
    // The keyframes' music is the trace's, except at 1600, where the row's
    // 8.76-frame boundary falls within a frame of the index (D17's class).
    for index in [1000u32, 2600, 3600] {
        let (order, row) = trace_position(index);
        assert_eq!(
            entry(&manifest, index)["music_position"],
            format!("{order}:{row}"),
            "the trace's own position at {index}"
        );
    }
    assert_eq!(entry(&manifest, 1600)["music_position"], "20:39");
    assert_eq!(trace_position(1600), (20, 38));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_block_four_clock_reaches_order_19_at_698() {
    // One frame ahead of `second4` at the gate: the trace reads 18:31 at 698.
    let (dir, manifest) = dump("gate", "698,699");
    assert_eq!(entry(&manifest, 698)["music_position"], "19:0");
    assert_eq!(entry(&manifest, 699)["music_position"], "19:0");
    assert_eq!(trace_position(698), (18, 31));
    assert_eq!(trace_position(699), (19, 0));
    let _ = std::fs::remove_dir_all(&dir);
}

/// For each filed index `K`: the port's `K - 4` (the reference's own stream
/// state) and its hold `K - 3`, and the keyframe's `K - 1`, `K`, `K + 1`.
const FRAMES: &str = "996,997,999,1000,1001,\
                      1596,1597,1599,1600,1601,\
                      2596,2597,2599,2600,2601,\
                      3596,3597,3599,3600,3601";

#[test]
fn the_keyframes_record_the_measured_session_phase_and_gouraud_residual() {
    let (dir, manifest) = dump("keyframes", FRAMES);
    // The divergence at `K` itself: the previous stream state (A31).
    let exact: &[(u32, usize)] = &[(1000, 11006), (1600, 11805), (2600, 17278), (3600, 16234)];
    for &(index, exact) in exact {
        let reference = reference_frame(index);
        let rgb = |index: u32| dumped_file(&dir, &manifest, index, "rgb");
        assert_eq!(differing(&rgb(index), &reference), exact, "boot {index} at K");
        // At the reference's own state the pose is exact: only colours inside
        // the objects differ.
        assert_eq!(
            silhouette_differing(&rgb(index - 4), &reference),
            0,
            "boot {index}: the aligned silhouette is the reference's"
        );
        // Each state holds two ticks: K-4 through K-3 and K through K+1, while
        // K-1 is the previous state (D19's neighbours make that visible).
        assert_eq!(rgb(index - 4), rgb(index - 3), "boot {index}: K-4 holds through K-3");
        assert_eq!(rgb(index), rgb(index + 1), "boot {index}: K holds through K+1");
        assert_ne!(rgb(index - 1), rgb(index), "boot {index}: K-1 is the previous state");
    }
    let _ = std::fs::remove_dir_all(&dir);
}
