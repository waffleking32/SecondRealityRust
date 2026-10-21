//! GRID through the real binary: the silent, unanchored run, and the five
//! filed reference keyframes at the measured `K0`.
//!
//! The part's arithmetic is unit-tested beside it (`parts::vault::grid`): the
//! sine tables and the measured `d0`, the eye shift bound, the three palettes
//! and the generated line blit over its whole table. What is left for a
//! process is the wiring only a run can answer: the registry, the numbering,
//! silence, and the frames the shell actually composes.
//!
//! **`K0` = 353** (`docs/fidelity/reference-indices.md` §24.5): the filed
//! capture is route A, whose 353 is byte-identical to route B's 704 — the
//! first byte-reproducible loop frame. The port's composed frame 0 is source
//! iteration 1 (iteration 0 is the start-up transition frame, reference 352,
//! and is drawn in `init`), so **port frame `j` = reference `353 + j`** and the
//! candidate for reference `R` is port `R − K0`, with ±1 searched.
//!
//! All five filed frames are RGB-only: the screen moves during the indexed
//! read, so the oracle keeps the RGB frame (D15/D19). The test compares the
//! `.rgb` and would compare an `.idx` if a later capture ever carried one.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The reference index of the port's composed frame 0.
const K0: u32 = 353;

/// The measured residual at each filed keyframe, in differing pixels.
///
/// They are the capture catching the newest line mid-draw in its top rows
/// (rows 14–15, where the curve turns); the port composes the finished frame.
/// `docs/fidelity/APPROXIMATIONS.md` A41 carries the per-frame numbers and the
/// candidate cause.
const RESIDUALS: [(u32, usize); 5] = [(400, 6), (500, 2), (600, 36), (700, 33), (800, 14)];

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of GRID alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-vault-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            "grid",
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

/// The reference's own file for a GRID frame, from the tree it is committed
/// in.
fn reference_frame(index: u32, suffix: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!("reference/parts/grid/frame_{index:06}.{suffix}")))
        .unwrap()
}

/// How many pixels of `rendered` differ from `expected`, and the largest
/// channel delta among them.
///
/// **The lengths are asserted first**: `zip` stops at the shorter side, so a
/// truncated dump — the one failure a comparison test exists to catch — would
/// compare a prefix and report zero differences as if it had compared the
/// frame.
fn differing_pixels(rendered: &[u8], expected: &[u8]) -> (usize, u8) {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    let mut pixels = 0;
    let mut max = 0u8;
    for (a, b) in rendered.chunks_exact(3).zip(expected.chunks_exact(3)) {
        if a != b {
            pixels += 1;
            for channel in 0..3 {
                max = max.max(a[channel].abs_diff(b[channel]));
            }
        }
    }
    (pixels, max)
}

#[test]
fn a_part_run_of_grid_numbers_itself_from_zero_and_is_silent() {
    let (dir, manifest) = dump("silent", "0,1,10");

    assert_eq!(manifest["part"], "grid");
    // The vault is not in the demo's sequence and carries no measured boot
    // anchor: a `--part grid` run numbers its own frames from zero.
    assert_eq!(manifest["index_origin"], "part");
    assert!(manifest["anchor"].is_null(), "{}", manifest["anchor"]);
    // Silent: no player is installed and `music_for_part("grid")` is `None`.
    assert!(manifest["music"].is_null(), "{}", manifest["music"]);

    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    for (frame, index) in frames.iter().zip([0u32, 1, 10]) {
        assert_eq!(frame["index"].as_u64(), Some(u64::from(index)));
        assert_eq!(frame["part"], "grid");
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
        let rgb = std::fs::read(dir.join(frame["rgb"].as_str().unwrap())).unwrap();
        assert_eq!(rgb.len(), 320 * 200 * 3);
        let idx = std::fs::read(dir.join(frame["idx"].as_str().unwrap())).unwrap();
        assert_eq!(idx.len(), 64_000);
    }
}

#[test]
fn grid_matches_the_reference_at_its_measured_start() {
    // The candidate for every keyframe, and its two neighbours: the anchor is
    // on the measured K0, and D18's ±1 search is reproduced here so the test
    // reports the same three counts the comparison workflow would.
    let mut indices = Vec::new();
    for (reference, _) in RESIDUALS {
        indices.push(reference - K0 - 1);
        indices.push(reference - K0);
        indices.push(reference - K0 + 1);
    }
    let list = indices
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let (dir, manifest) = dump("keyframes", &list);

    for (reference, residual) in RESIDUALS {
        let mut counts = Vec::new();
        let mut best: Option<(usize, i32, u8)> = None;
        for offset in -1i32..=1 {
            let port_index = (reference as i32 - K0 as i32 + offset) as u32;
            let rendered = dumped_file(&dir, &manifest, port_index, "rgb");
            let expected = reference_frame(reference, "rgb");
            let (pixels, max) = differing_pixels(&rendered, &expected);
            counts.push((offset, pixels, max));
            let better = match best {
                None => true,
                Some((fewest, _, _)) => pixels < fewest,
            };
            if better {
                best = Some((pixels, offset, max));
            }
            // GRID's filed capture has no `.idx` (D15/D19: the indexed read
            // cannot reproduce the moving screen). If a later capture carries
            // one, compare it too — indexed is the stricter side.
            let reference_idx =
                repository_root().join(format!("reference/parts/grid/frame_{reference:06}.idx"));
            if reference_idx.is_file() {
                let idx = std::fs::read(&reference_idx).unwrap();
                let rendered_idx = dumped_file(&dir, &manifest, port_index, "idx");
                assert_eq!(
                    rendered_idx, idx,
                    "reference {reference} at offset {offset:+}: indexed plane"
                );
            }
        }
        let (fewest, offset, max) = best.unwrap();
        // The residual's own rows, so the note's "top rows" claim is printed
        // by the command that measures it (D25).
        let port_index = (reference as i32 - K0 as i32 + offset) as u32;
        let rendered = dumped_file(&dir, &manifest, port_index, "rgb");
        let expected = reference_frame(reference, "rgb");
        let mut rows = std::collections::BTreeMap::new();
        for (pixel, (a, b)) in rendered
            .chunks_exact(3)
            .zip(expected.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                *rows.entry(pixel / 320).or_insert(0usize) += 1;
            }
        }
        println!(
            "reference {reference}: offsets -1/0/+1 give {:?}; minimum {fewest} pixel(s) \
             at offset {offset:+} (max channel delta {max}); rows {rows:?}",
            counts
                .iter()
                .map(|(offset, pixels, _)| (*offset, *pixels))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            (fewest, offset),
            (residual, 0),
            "reference {reference}: {counts:?}; the minimum must be the measured residual at the \
             K0 mapping"
        );
    }
}
