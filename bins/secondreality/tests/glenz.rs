//! GLENZ through the real binary: the block-2 numbering, the held opening and
//! the moving keyframes at their measured phase.
//!
//! The part's arithmetic is unit-tested beside it (`glenz::*`): the tick map,
//! the intro's 47+177, the palette phases, the physics, the projection, the
//! colour allocator and the compositor. What is left for a process is the
//! wiring: the registry name, the block-2 anchor and music row, the frames the
//! shell composes, and the comparison against the filed reference.
//!
//! **The held opening is exact.** Boot 1499 carries the reference's index
//! plane (`reference/parts/glenz/frame_001499.idx`) at **0 differing indices**
//! of 64,000, and the RGB follows from it.
//!
//! **The moving keyframes are compared at the measured phase, not fitted**
//! (`docs/fidelity/glenz.md` §11; `APPROXIMATIONS.md` A36/A37). The block-2
//! music wait exits at **1117** in the port against the reference `second2b`'s
//! **1111**, and the intro's ticks from the reset to the main loop's first
//! wait are fixed at 451, so the port's loop start is `L_p = 1568` against
//! Task 1's `L_r = 1562`. Main-loop frame `f` is therefore the picture at boot
//! `1568 + f` while the reference's is at `1562 + f`, and the plan's aligned
//! index is `K + 6` (2306/3006/3406 for 2300/3000/3400).
//!
//! The reference's *captured* moving frames pair a surface with a DAC that
//! runs one to two main frames ahead of it (D17's sample/publish instant):
//! the filed 2300 surface is the partner sessions' 2299 and its low palette is
//! the fade computed at main frame 738, while the port at its own main frame
//! 737 (boot 2305) draws that surface. So the test pins two alignments:
//!
//! - **the plan's `K + 6`** — the music-clock alignment, whose residual is
//!   dominated by the reference capture's surface lag: 9,777 / 10,806 /
//!   12,001 pixels at 2306/3006/3406 (A36);
//! - **the surface-aligned frame** — 2305 at 2300, and 3004/3404 where the
//!   second object's larger faces expose the capture's two-frame DAC lead —
//!   where the model itself is pinned: **5,208 / 2,289 / 5,941** differing
//!   pixels, and at 2305 **every** differing pixel is a low-palette fade entry
//!   (the reference shows `b = 26`, the port `b = 27`; the object pixels are
//!   byte-identical, which is the picture evidence for the shipped low-byte
//!   `rolcol` key A37 adopts).
//!
//! No constant, anchor or tick was adjusted to chase these numbers (D24/D17).

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/GLENZ/` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless run dumping the frames asked for, and the manifest it wrote.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn run(name: &str, mode: &[&str], indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-gl-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(mode)
        .args(["--headless", "--dump-frames", indices, "--dump-dir"])
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

/// A headless block-2 run: BEGLOGO, then GLENZ, in the reference's own
/// numbering.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    run(name, &["--block", "2"], indices)
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

/// The reference's own file for a GLENZ frame, from the tree it is committed
/// in.
fn reference_frame(index: u32, suffix: &str) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/glenz/frame_{index:06}.{suffix}")),
    )
    .unwrap()
}

/// How many pixels of `rendered` differ from `expected`, with the unit taken
/// from the suffix's element size.
///
/// **The lengths are asserted first**: a truncated dump — the one failure a
/// comparison test exists to catch — would otherwise compare a prefix and
/// report zero differences as if it had compared the frame.
fn differing(rendered: &[u8], expected: &[u8], unit: usize) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    rendered
        .chunks_exact(unit)
        .zip(expected.chunks_exact(unit))
        .filter(|(a, b)| a != b)
        .count()
}

/// The differing RGB pixels split by the port's own background palette:
/// `(low, high)`, where `low` counts pixels whose `.idx` is 0..15.
///
/// The low entries are the ones the main loop uploads (`MAIN.C:414-415`); the
/// group entries at 16..255 are the faces' DAC groups. The split is what
/// attributes the 2305 residual to the reference capture's fade palette.
fn differing_by_palette(rendered: &[u8], expected: &[u8], indices: &[u8]) -> (usize, usize) {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    assert_eq!(indices.len(), rendered.len() / 3, "one index per pixel");
    let (mut low, mut high) = (0, 0);
    for (pixel, &index) in indices.iter().enumerate() {
        let (a, b) = (
            &rendered[pixel * 3..pixel * 3 + 3],
            &expected[pixel * 3..pixel * 3 + 3],
        );
        if a != b {
            if index < 16 {
                low += 1;
            } else {
                high += 1;
            }
        }
    }
    (low, high)
}

/// How many pixels the port's index structure fails to explain: for each
/// index the port paints, the reference's colour at those pixels is counted,
/// and every pixel whose colour is not that index's most common reference
/// colour is returned.
///
/// A moving screen's index structure is the XOR of the faces covering each
/// pixel. When the port and the reference draw the same faces, every index
/// maps to one reference colour; a slid facet boundary shows up here first.
fn unexplained_pixels(rendered: &[u8], expected: &[u8], indices: &[u8]) -> usize {
    use std::collections::HashMap;
    assert_eq!(rendered.len(), expected.len(), "the frame lengths differ");
    assert_eq!(indices.len(), rendered.len() / 3, "one index per pixel");
    let pixel = |bytes: &[u8], i: usize| bytes[i * 3..i * 3 + 3].to_vec();
    let mut counts: HashMap<u8, HashMap<Vec<u8>, usize>> = HashMap::new();
    for (i, &index) in indices.iter().enumerate() {
        let (port, reference) = (pixel(rendered, i), pixel(expected, i));
        if port == [0, 0, 0] || reference == [0, 0, 0] {
            continue;
        }
        *counts
            .entry(index)
            .or_default()
            .entry(reference)
            .or_default() += 1;
    }
    let mut unexplained = 0;
    for by_colour in counts.values() {
        let total: usize = by_colour.values().sum();
        let top = by_colour.values().copied().max().unwrap_or(0);
        unexplained += total - top;
    }
    unexplained
}

/// The frame's eight reds after the frame-786 `backpal` rebuild (`MAIN.C:557-584`).
fn rebuilt_reds() -> [[u8; 3]; 8] {
    std::array::from_fn(|a| {
        let a = a as u16;
        let r = 10 * (a & 1) + 30 * ((a >> 1) & 1) + 20 * ((a >> 2) & 1) + 16 * ((a >> 3) & 1);
        let gb = 16 * ((a >> 3) & 1);
        [r.min(63) as u8, gb as u8, gb as u8]
    })
}

/// A six-bit channel widened the way the DAC and the presenter widen it.
fn widened(value: u8) -> u8 {
    ((u16::from(value) * 259 + 33) >> 6) as u8
}

/// How many pixels disagree about belonging to the second (red) object. The
/// reference has only RGB, so membership is its colour in the rebuilt
/// palette's entries 1..7; the port's is an index below 8 (`Glz2`'s
/// `(word >> 1) & 1` and the hidden faces' low bytes 2/4, all XORed).
fn red_mask_differing(rendered: &[u8], expected: &[u8], indices: &[u8]) -> usize {
    let reds: Vec<[u8; 3]> = rebuilt_reds()[1..]
        .iter()
        .map(|rgb| [widened(rgb[0]), widened(rgb[1]), widened(rgb[2])])
        .collect();
    let is_red = |pixel: &[u8]| {
        let pixel = [pixel[0], pixel[1], pixel[2]];
        reds.contains(&pixel)
    };
    assert_eq!(rendered.len(), expected.len(), "the frame lengths differ");
    assert_eq!(indices.len(), rendered.len() / 3, "one index per pixel");
    indices
        .iter()
        .enumerate()
        .filter(|&(i, &index)| {
            let port = index != 0 && index < 8;
            let reference = is_red(&expected[i * 3..i * 3 + 3]);
            port != reference
        })
        .count()
}

/// The images every test below reads: the held opening, the plan's aligned
/// moving indices (`K + 6`) and the surface-aligned frames the capture's DAC
/// lead requires (A36/A37).
const FRAMES: &str = "1499,2305,2306,3004,3006,3404,3406";

#[test]
fn the_block_two_run_is_numbered_in_boot_frames_and_reaches_glenz() {
    let (dir, manifest) = dump("numbering", FRAMES);
    assert_eq!(manifest["part"], "glenz");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404, "block 2 draws from the boot's 404");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `MAIN/U2.ASM:766-775`: block 2 starts MUSIC1 at order 0 against the
    // boot's 404; the clock's own start is 408 (D26's measured start).
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 408);

    for &index in &[1499u32, 2305, 2306, 3004, 3006, 3404, 3406] {
        let frame = entry(&manifest, index);
        assert_eq!(frame["part"], "glenz", "boot {index} is GLENZ's screen");
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
        assert!(
            dir.join(frame["idx"].as_str().unwrap()).is_file(),
            "boot {index} wrote the .idx it names"
        );
    }

    // The port's music position at each index. The filed `second2b` trace is
    // one row ahead at all four (1499 5:12, 2306 8:20, 3006 11:1, 3406
    // 12:36) — the six-tick wait offset A36 registers, seen on the published
    // position.
    assert_eq!(entry(&manifest, 1499)["music_position"], "5:11");
    assert_eq!(entry(&manifest, 2306)["music_position"], "8:19");
    assert_eq!(entry(&manifest, 3006)["music_position"], "11:0");
    assert_eq!(entry(&manifest, 3406)["music_position"], "12:35");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_held_opening_1499_is_the_reference_index_plane() {
    let (dir, manifest) = dump("hold", FRAMES);
    let rendered = dumped_file(&dir, &manifest, 1499, "idx");
    let expected = reference_frame(1499, "idx");
    let count = differing(&rendered, &expected, 1);
    println!("boot 1499: {count}/64000 indices differ");
    assert_eq!(count, 0, "the held opening's index plane (Task 5's intro)");
    // The RGB follows from the plane; assert it too so a palette slip cannot
    // hide behind an index match.
    let rendered = dumped_file(&dir, &manifest, 1499, "rgb");
    let expected = reference_frame(1499, "rgb");
    assert_eq!(differing(&rendered, &expected, 3), 0, "the held picture");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_moving_keyframes_record_the_registered_phase_and_residuals() {
    let (dir, manifest) = dump("keyframes", FRAMES);

    // A36: the plan's `K + 6` — the music-clock alignment. The residual is
    // the reference capture's surface lag (the filed picture is the partner
    // sessions' `K - 1`), not a port frame error: the port's K+6 surface is
    // the partner sessions' K+1 surface, an independent capture's picture.
    let aligned: &[(u32, u32, usize)] =
        &[(2306, 2300, 9777), (3006, 3000, 10806), (3406, 3400, 12001)];
    for &(port, reference, expected) in aligned {
        let rendered = dumped_file(&dir, &manifest, port, "rgb");
        let expected_frame = reference_frame(reference, "rgb");
        let count = differing(&rendered, &expected_frame, 3);
        println!("boot {port} vs reference {reference} (K+6): {count}/64000 px differ");
        assert_eq!(
            count, expected,
            "A36's measured K+6 residual at {reference}"
        );
    }

    // A37: the surface-aligned frame. At 2300 the reference's own main frame
    // is 737 (the partner sessions' 2299) — the port's boot 2305. The only
    // difference is the capture's low palette, which is the fade computed at
    // main frame 738 (b = 26) where the port's displayed frame 737 computes
    // b = 27; every object pixel is byte-identical.
    let surface_aligned: &[(u32, u32, usize)] =
        &[(2305, 2300, 5208), (3004, 3000, 2289), (3404, 3400, 5941)];
    for &(port, reference, expected) in surface_aligned {
        let rendered = dumped_file(&dir, &manifest, port, "rgb");
        let expected_frame = reference_frame(reference, "rgb");
        let count = differing(&rendered, &expected_frame, 3);
        println!("boot {port} vs reference {reference} (surface): {count}/64000 px differ");
        assert_eq!(
            count, expected,
            "A37's measured surface residual at {reference}"
        );
    }

    let rendered = dumped_file(&dir, &manifest, 2305, "rgb");
    let expected = reference_frame(2300, "rgb");
    let indices = dumped_file(&dir, &manifest, 2305, "idx");
    let (low, high) = differing_by_palette(&rendered, &expected, &indices);
    println!("boot 2305 vs 2300: {low} low-palette, {high} group-entry pixels differ");
    assert_eq!(
        (low, high),
        (5208, 0),
        "the 2300 residual is the capture's fade palette: the object entries match"
    );
    assert_eq!(
        unexplained_pixels(&rendered, &expected, &indices),
        0,
        "every index at 2305 maps to one reference colour (the filed 2300's face coverage)"
    );

    // At 3000/3400 the second object's interior facet boundaries differ on a
    // small count of XOR pixels while its outer silhouette agrees to a pixel;
    // these are A37's measured residual, not a constant a port could fit.
    for &(port, reference, unexplained, red_mask) in &[
        (3004u32, 3000u32, 711usize, 118usize),
        (3404, 3400, 839, 180),
    ] {
        let rendered = dumped_file(&dir, &manifest, port, "rgb");
        let expected = reference_frame(reference, "rgb");
        let indices = dumped_file(&dir, &manifest, port, "idx");
        let count = unexplained_pixels(&rendered, &expected, &indices);
        let red = red_mask_differing(&rendered, &expected, &indices);
        println!("boot {port} vs {reference}: {count} unexplained indices, {red} red-mask px");
        assert_eq!(
            count, unexplained,
            "A37's unexplained pixels at {reference}"
        );
        assert_eq!(
            red, red_mask,
            "A37's second-object silhouette at {reference}"
        );
    }

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_part_run_of_glenz_numbers_itself_from_zero() {
    // A standalone run has no measured boot anchor, so its frames are its own:
    // the manifest says `part`, the anchor is null, and the MUSIC1 row starts
    // at the run's frame 0. Its first frames are inside the music wait
    // (`MAIN.C:290`), so the inherited 320x400 view holds.
    let (dir, manifest) = run("part", &["--part", "glenz"], "0,1");
    assert_eq!(manifest["part"], "glenz");
    assert_eq!(manifest["index_origin"], "part");
    assert_eq!(manifest["anchor"], serde_json::Value::Null);
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(
        manifest["music"]["start_frame"], 0,
        "the run's own numbering"
    );
    let frames = manifest["frames"].as_array().unwrap();
    assert_eq!(frames.len(), 2, "both asked-for frames were dumped");
    for (frame, index) in frames.iter().zip([0u32, 1]) {
        assert_eq!(frame["index"], index);
        assert_eq!(frame["part"], "glenz");
        assert_eq!(frame["width"], 320);
        assert_eq!(
            frame["height"], 400,
            "the inherited Mode X screen, waiting on the music"
        );
        assert!(
            dir.join(frame["idx"].as_str().unwrap()).is_file(),
            "frame {index} wrote the .idx it names"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}
