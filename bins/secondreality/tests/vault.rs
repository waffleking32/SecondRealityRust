//! GRID and TWIST through the real binary: the silent, unanchored runs, and
//! each part's filed reference keyframes at its measured `K0`.
//!
//! The parts' arithmetic is unit-tested beside them (`parts::vault::{grid,
//! twist}`): the sine tables and the measured `d0`, the eye shift bound, the
//! three palettes and the generated line blit for GRID; the 16-bit row phase,
//! the shipped texture, the two pages, the masked palette and the 4,444-frame
//! cap for TWIST. What is left for a process is the wiring only a run can
//! answer: the registry, the numbering, silence, and the frames the shell
//! actually composes.
//!
//! **GRID's `K0` = 353** (`docs/fidelity/reference-indices.md` §24.5): the
//! filed capture is route A, whose 353 is byte-identical to route B's 704 — the
//! first byte-reproducible loop frame. The port's composed frame 0 is source
//! iteration 1 (iteration 0 is the start-up transition frame, reference 352,
//! and is drawn in `init`), so **port frame `j` = reference `353 + j`**.
//!
//! **TWIST's `K0` = 3 ±1** (`reference-indices.md` §24.6): the two-band
//! fingerprint (rows 0..63 = generated row 0, rows 64..199 = row 199) is boot
//! frame 3, matched from the phase at boot 4/5/6 (effect frames 1/2/3). The
//! six filed keyframes 103..603 are effect frames 100..600.
//!
//! **HARD's `K0` = 356** (`reference-indices.md` §24.7): the first wipe frame,
//! 16 black pixels at row 100. The pre-roll is 119 port ticks — 20 wipe steps,
//! 28 fill iterations, 70 waits and the letterbox — so **port frame `j` =
//! reference `356 + j`**; the letterbox is port 118 (reference 474), the first
//! drawn dots port 156 (reference 512), and mode 4's count 0 port 495
//! (reference 851, a picture identical to mode 3's hold). The two measured
//! boundary displays put part 2's `part2init` at port 599 (reference 955), the
//! first grid frame at 600 (956) and `part3init` at 1100 (1456; A48).
//!
//! The candidate for a reference `R` is port `R − K0`, with the ±1 search
//! reproduced here so the test reports the same three counts the comparison
//! workflow would. Every filed frame of the three parts is RGB-only except
//! HARD's held 840: the screen moves during the indexed read (D15/D19), and
//! for TWIST an odd frame's page is not at plane offset 0 at all. The shared
//! comparison therefore compares the `.rgb` at every candidate, and a
//! reference `.idx` — HARD files one at its held frame 840 — only at the
//! offset the RGB search chose.

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

/// The reference index of TWIST's composed frame 0 (`reference-indices.md`
/// §24.6): the frame-0 two-band picture is boot index 3, measured from the
/// boundary phase at boot 4/5/6.
const TWIST_K0: u32 = 3;

/// The six filed TWIST keyframes: effect frames 100..600, ~100 apart.
const TWIST_KEYFRAMES: [u32; 6] = [103, 203, 303, 403, 503, 603];

/// TWIST is exact: the six keyframes are effect frames 100..600 under
/// `effect = index − 3`, and the port's model reproduces every filed byte.
const TWIST_RESIDUALS: [(u32, usize); 6] =
    [(103, 0), (203, 0), (303, 0), (403, 0), (503, 0), (603, 0)];

/// The reference index of HARD's composed frame 0 (`reference-indices.md`
/// §24.7): the first wipe frame, 16 black pixels at row 100, reference 356.
const HARD_K0: u32 = 356;

/// The nine filed HARD keyframes, one per mode stage: mode 0's letterbox, the
/// four logo phases, mode 3's hold, mode 4's band, part 2's grid and part 3's
/// cube.
const HARD_KEYFRAMES: [u32; 9] = [480, 560, 630, 700, 770, 840, 910, 1050, 1600];

/// All nine are exact at offset 0: every filed pixel reproduces, and 840's
/// `.idx` reproduces byte for byte (`hard_matches_the_reference_at_its_keyframes`
/// prints the three candidates).
const HARD_RESIDUALS: [(u32, usize, i32); 9] = [
    (480, 0, 0),
    (560, 0, 0),
    (630, 0, 0),
    (700, 0, 0),
    (770, 0, 0),
    (840, 0, 0),
    (910, 0, 0),
    (1050, 0, 0),
    (1600, 0, 0),
];

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own `assets/...` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A run of one part alone, dumping the frames asked for.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched. The directory carries the part's
/// name as well as the test's, so two parts' runs of the same test name cannot
/// share a dump.
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
/// The minimum is chosen by `(pixels, |offset|, rendered index)`: D18's rule
/// (fewest differing pixels, then the smaller offset), so a held or exactly
/// repeated frame reports offset 0 rather than a coin flip. **The indexed
/// comparison follows the RGB choice** — a reference that carries an `.idx`
/// (HARD's held 840) is compared only at the offset whose RGB picture won; a
/// `±1` loop that compared every candidate's `.idx` would fail on the wrong
/// picture.
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

        // Indexed is the stricter side, where the reference files one. It is
        // compared at the winner only.
        let reference_idx =
            repository_root().join(format!("reference/parts/{part}/frame_{reference:06}.idx"));
        if reference_idx.is_file() {
            let idx = std::fs::read(&reference_idx).unwrap();
            let rendered_idx = dumped_file(&dir, &manifest, port_index, "idx");
            assert_eq!(
                rendered_idx, idx,
                "reference {reference} at offset {offset:+}: indexed plane"
            );
        }

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
fn a_part_run_of_grid_numbers_itself_from_zero_and_is_silent() {
    let (dir, manifest) = dump("grid", "silent", "0,1,10");

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
fn a_part_run_of_twist_is_silent_and_numbers_itself_from_zero() {
    let (dir, manifest) = dump("twist", "silent", "0,1,10");

    assert_eq!(manifest["part"], "twist");
    // The vault is not in the demo's sequence and carries no measured boot
    // anchor: a `--part twist` run numbers its own frames from zero.
    assert_eq!(manifest["index_origin"], "part");
    assert!(manifest["anchor"].is_null(), "{}", manifest["anchor"]);
    // Silent: TWIST calls no music service and `music_for_part("twist")` is
    // `None`; a stray key would have ended the original, and none is sent.
    assert!(manifest["music"].is_null(), "{}", manifest["music"]);

    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    for (frame, index) in frames.iter().zip([0u32, 1, 10]) {
        assert_eq!(frame["index"].as_u64(), Some(u64::from(index)));
        assert_eq!(frame["part"], "twist");
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
    let keyframes: Vec<u32> = GRID_RESIDUALS.iter().map(|(frame, _)| *frame).collect();
    let reports = compare_keyframes("grid", GRID_K0, &keyframes);
    for (report, (reference, residual)) in reports.iter().zip(GRID_RESIDUALS) {
        assert_eq!(report.reference, reference);
        print_report(report);
        assert_eq!(
            (report.pixels, report.offset),
            (residual, 0),
            "reference {reference}: {:?}; the minimum must be the measured residual at the K0 \
             mapping",
            report.counts
        );
    }
}

#[test]
fn twist_matches_the_reference_at_its_measured_start() {
    // All six filed frames are RGB-only: the moving screen fails the indexed
    // read's coherence check, and an odd TWIST frame draws page 2, which the
    // oracle's plane read (always from plane offset 0) would see as page 1.
    for reference in TWIST_KEYFRAMES {
        let idx = repository_root().join(format!("reference/parts/twist/frame_{reference:06}.idx"));
        assert!(
            !idx.is_file(),
            "TWIST's filed capture is RGB-only (reference {reference})"
        );
    }

    let reports = compare_keyframes("twist", TWIST_K0, &TWIST_KEYFRAMES);
    for (report, (reference, residual)) in reports.iter().zip(TWIST_RESIDUALS) {
        assert_eq!(report.reference, reference);
        print_report(report);
        assert_eq!(
            (report.pixels, report.offset),
            (residual, 0),
            "reference {reference}: {:?}; effect frame {} must match at the K0 mapping",
            report.counts,
            reference - TWIST_K0
        );
    }
}

#[test]
fn a_part_run_of_hard_is_silent_and_numbers_itself_from_zero() {
    let (dir, manifest) = dump("hard", "silent", "0,1,10");

    assert_eq!(manifest["part"], "hard");
    // The vault is not in the demo's sequence and carries no measured boot
    // anchor: a `--part hard` run numbers its own frames from zero.
    assert_eq!(manifest["index_origin"], "part");
    assert!(manifest["anchor"].is_null(), "{}", manifest["anchor"]);
    // Silent: HARD's only music call is the start gate and the 0xff cut; no
    // player is installed and `music_for_part("hard")` is `None`.
    assert!(manifest["music"].is_null(), "{}", manifest["music"]);

    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 3);
    for (frame, index) in frames.iter().zip([0u32, 1, 10]) {
        assert_eq!(frame["index"].as_u64(), Some(u64::from(index)));
        assert_eq!(frame["part"], "hard");
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
        let rgb = std::fs::read(dir.join(frame["rgb"].as_str().unwrap())).unwrap();
        assert_eq!(rgb.len(), 320 * 200 * 3);
        let idx = std::fs::read(dir.join(frame["idx"].as_str().unwrap())).unwrap();
        assert_eq!(idx.len(), 64_000);
    }
}

#[test]
fn hard_matches_the_reference_at_its_keyframes() {
    let reports = compare_keyframes("hard", HARD_K0, &HARD_KEYFRAMES);
    for report in &reports {
        print_report(report);
    }
    for (report, (reference, residual, offset)) in reports.iter().zip(HARD_RESIDUALS) {
        assert_eq!(report.reference, reference);
        assert_eq!(
            (report.pixels, report.offset),
            (residual, offset),
            "reference {reference}: {:?}; the minimum must be the measured residual at the K0 \
             mapping",
            report.counts
        );
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
}
