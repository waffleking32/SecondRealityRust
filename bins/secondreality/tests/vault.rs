//! GRID, HARD and TWIST through the real binary: the silent, unanchored runs,
//! the ends the parts reach by themselves, and GRID's residual rows. The
//! keyframe counts are rediff's (`grid`, `hard`, `twist`), held by
//! `tests/reference_frames.rs`.
//!
//! **GRID's `K0` = 353** (`reference-indices.md` §24.5): port frame `j` is
//! reference `353 + j` (source iteration 0, reference 352, is drawn in
//! `init`). **HARD's** is 356 (§24.7) and **TWIST's** 3 (§24.6).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The reference index of GRID's composed frame 0.
const GRID_K0: u32 = 353;

/// The measured residual at each filed GRID keyframe, in differing pixels.
///
/// They are the capture catching the newest line mid-draw in its top rows
/// (rows 14–15, where the curve turns); the port composes the finished frame.
/// `docs/fidelity/APPROXIMATIONS.md` A41 carries the per-frame numbers and the
/// candidate cause.
const GRID_RESIDUALS: [(u32, usize); 5] = [(400, 6), (500, 2), (600, 36), (700, 33), (800, 14)];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A `--part` run dumping `indices`, which must exit 0, and its manifest. The
/// caller removes the directory.
fn dump(part: &str, name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-vault-{}-{part}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--part",
            part,
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

/// The reference's own file for a part's frame, from the tree it is committed
/// in.
fn reference_frame(part: &str, index: u32, suffix: &str) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/{part}/frame_{index:06}.{suffix}")),
    )
    .unwrap()
}

/// How many pixels of `rendered` differ from `expected`, and the largest
/// channel delta among them; a short read is refused, not compared.
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

/// One keyframe's measured comparison.
struct KeyframeReport {
    reference: u32,
    /// `(offset, differing pixels, max channel delta)` for the three candidates.
    counts: Vec<(i32, usize, u8)>,
    /// The winning offset (`rendered index − reference index`).
    offset: i32,
    pixels: usize,
    max: u8,
    /// Differing pixels per displayed row at the winning offset.
    rows: BTreeMap<usize, usize>,
}

/// Compare every `keyframe` against `part`'s filed reference at `K0`, over the
/// port-side candidates `R − K0 − 1, R − K0, R − K0 + 1`.
///
/// The minimum is chosen by `(pixels, |offset|, rendered index)`, D18's rule,
/// so a held frame reports offset 0.
fn compare_keyframes(part: &str, k0: u32, keyframes: &[u32]) -> Vec<KeyframeReport> {
    let mut indices = Vec::new();
    for &reference in keyframes {
        for offset in -1i32..=1 {
            let port_index = reference as i64 - i64::from(k0) + i64::from(offset);
            assert!(port_index >= 0, "reference {reference} is before K0");
            indices.push(port_index as u32);
        }
    }
    indices.sort_unstable();
    indices.dedup();
    let list = indices
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let (dir, manifest) = dump(part, "keyframes", &list);

    let mut reports = Vec::new();
    for &reference in keyframes {
        let mut counts = Vec::new();
        // (differing pixels, signed offset, max delta, rendered index).
        let mut best: Option<(usize, i32, u8, u32)> = None;
        for offset in -1i32..=1 {
            let port_index = (reference as i64 - i64::from(k0) + i64::from(offset)) as u32;
            let rendered = dumped_file(&dir, &manifest, port_index, "rgb");
            let expected = reference_frame(part, reference, "rgb");
            let (pixels, max) = differing_pixels(&rendered, &expected);
            counts.push((offset, pixels, max));
            let better = match best {
                None => true,
                Some((fewest, best_offset, _, best_index)) => {
                    (pixels, offset.abs(), port_index) < (fewest, best_offset.abs(), best_index)
                }
            };
            if better {
                best = Some((pixels, offset, max, port_index));
            }
        }
        let (pixels, offset, max, port_index) = best.unwrap();


        // The residual's own rows, so a note's claim about where the pixels
        // differ is printed by the command that measures it (D25).
        let rendered = dumped_file(&dir, &manifest, port_index, "rgb");
        let expected = reference_frame(part, reference, "rgb");
        let mut rows = BTreeMap::new();
        for (pixel, (a, b)) in rendered
            .chunks_exact(3)
            .zip(expected.chunks_exact(3))
            .enumerate()
        {
            if a != b {
                *rows.entry(pixel / 320).or_insert(0usize) += 1;
            }
        }

        reports.push(KeyframeReport {
            reference,
            counts,
            offset,
            pixels,
            max,
            rows,
        });
    }
    let _ = std::fs::remove_dir_all(&dir);
    reports
}

fn print_report(report: &KeyframeReport) {
    println!(
        "reference {}: offsets -1/0/+1 give {:?}; minimum {} pixel(s) at offset {:+} (max \
         channel delta {}); rows {:?}",
        report.reference,
        report
            .counts
            .iter()
            .map(|(offset, pixels, _)| (*offset, *pixels))
            .collect::<Vec<_>>(),
        report.pixels,
        report.offset,
        report.max,
        report.rows
    );
}

#[test]
fn a_part_run_of_each_vault_effect_numbers_itself_from_zero_and_is_silent() {
    // The vault is not in the demo's sequence and has no boot anchor, and no
    // player is installed (`music_for_part` is `None`).
    for part in ["grid", "hard", "twist"] {
        let (dir, manifest) = dump(part, "silent", "0,1,10");
        assert_eq!(manifest["part"], part);
        assert_eq!(manifest["index_origin"], "part", "{part}");
        assert!(manifest["anchor"].is_null(), "{part}: {}", manifest["anchor"]);
        assert!(manifest["music"].is_null(), "{part}: {}", manifest["music"]);
        let frames = manifest["frames"].as_array().unwrap();
        let listed: Vec<(u64, &str, u64, u64)> = frames
            .iter()
            .map(|f| {
                let field = |name: &str| f[name].as_u64().unwrap();
                (field("index"), f["part"].as_str().unwrap(), field("width"), field("height"))
            })
            .collect();
        assert_eq!(listed, [0, 1, 10].map(|index| (index, part, 320, 200)), "{part}");
        let _ = std::fs::remove_dir_all(&dir);
    }
}

#[test]
fn grid_matches_the_reference_at_its_measured_start() {
    let keyframes: Vec<u32> = GRID_RESIDUALS.iter().map(|(frame, _)| *frame).collect();
    let reports = compare_keyframes("grid", GRID_K0, &keyframes);
    for (report, (reference, residual)) in reports.iter().zip(GRID_RESIDUALS) {
        assert_eq!(report.reference, reference);
        print_report(report);
        // The note's claim is asserted, not only printed: every residual pixel
        // is on rows 14–15, the topmost rows the curve reaches.
        assert!(
            report.rows.keys().all(|&row| row == 14 || row == 15),
            "reference {reference}: the residual leaves rows 14-15: {:?}",
            report.rows
        );
        assert_eq!(
            report.rows.values().sum::<usize>(),
            report.pixels,
            "reference {reference}: the row histogram does not add up"
        );
        assert_eq!(
            (report.pixels, report.offset),
            (residual, 0),
            "reference {reference}: {:?}; the minimum must be the measured residual at the K0 \
             mapping",
            report.counts
        );
    }
}

/// TWIST's six keyframes, effect frames 100..600 at `K0` = 3: exact at offset
/// 0. The run prints the ±1 neighbours `twist.md`'s table quotes.
#[test]
fn twist_matches_the_reference_at_its_measured_start() {
    let reports = compare_keyframes("twist", 3, &[103, 203, 303, 403, 503, 603]);
    for report in &reports {
        print_report(report);
        assert_eq!((report.pixels, report.offset), (0, 0), "reference {}: {:?}", report.reference, report.counts);
    }
}

#[test]
fn hard_draws_the_whole_effect_and_holds() {
    // Part 3's last drawn frame is its count 800 (port 1900, reference 2256).
    // Mode 7's one iteration and the `default` arm draw nothing (1901, 1902),
    // and the shell holds the same picture after the part retires; the
    // source's `if(!dis_indemo())` text-mode restore (`B.C:325-329`) is the
    // window's close, not a frame.
    let (dir, manifest) = dump("hard", "hold", "1898,1899,1900,1901,1902,1903,1920");
    for index in [1898u32, 1899, 1900, 1901, 1902, 1903, 1920] {
        assert_eq!(entry(&manifest, index)["part"], "hard", "frame {index}");
    }
    let last = dumped_file(&dir, &manifest, 1900, "rgb");
    assert_ne!(
        dumped_file(&dir, &manifest, 1899, "rgb"),
        last,
        "the cube still moves on count 799"
    );
    for hold in [1901u32, 1902, 1903, 1920] {
        assert_eq!(
            dumped_file(&dir, &manifest, hold, "rgb"),
            last,
            "frame {hold} is the hold after the last drawn cube"
        );
        assert_eq!(
            dumped_file(&dir, &manifest, hold, "idx"),
            dumped_file(&dir, &manifest, 1900, "idx"),
            "frame {hold} holds the indexed frame too"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn twist_draws_exactly_4444_frames_and_holds() {
    // 4,444 iterations (`while(!kbhit() && frame<4444)`): 4,442, 4,443 and the
    // held frames after the part retires. The 4,445th check returns `Finished`
    // before drawing, so 4,444 and 4,500 are the picture 4,443 left.
    let (dir, manifest) = dump("twist", "cap", "4442,4443,4444,4500");
    for index in [4442u32, 4443, 4444, 4500] {
        let frame = entry(&manifest, index);
        assert_eq!(frame["part"], "twist", "frame {index}");
        assert_eq!(frame["width"], 320, "frame {index}");
        assert_eq!(frame["height"], 200, "frame {index}");
    }

    let previous = dumped_file(&dir, &manifest, 4442, "rgb");
    let last = dumped_file(&dir, &manifest, 4443, "rgb");
    assert_ne!(
        previous, last,
        "frame 4443 is the last drawn frame and is a new picture"
    );

    for hold in [4444u32, 4500] {
        assert_eq!(
            dumped_file(&dir, &manifest, hold, "rgb"),
            last,
            "frame {hold} is the hold, not a 4,445th drawn frame"
        );
        assert_eq!(
            dumped_file(&dir, &manifest, hold, "idx"),
            dumped_file(&dir, &manifest, 4443, "idx"),
            "frame {hold} holds the indexed frame too"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
