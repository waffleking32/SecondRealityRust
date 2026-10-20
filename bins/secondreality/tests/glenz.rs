//! GLENZ through the real binary: the block-2 numbering, the loop start, the
//! held opening and the moving keyframes at their measured alignments.
//!
//! The part's arithmetic is unit-tested beside it (`glenz::*`): the tick map,
//! the intro's 47+177, the palette phases, the physics, the projection, the
//! colour allocator and the compositor. What is left for a process is the
//! wiring: the registry name, the block-2 anchor and music row, the frames the
//! shell composes, the loop-start measurement, and the comparison against the
//! filed reference.
//!
//! **The held opening is exact.** Boot 1499 carries the reference's index
//! plane (`reference/parts/glenz/frame_001499.idx`) at **0 differing indices**
//! of 64,000, and the RGB follows from it.
//!
//! **The port's loop start is L_p = 1567** (`docs/fidelity/glenz.md` §11;
//! `APPROXIMATIONS.md` A36). Two independent measurements: the first
//! above-floor object pixels are at boot **1611** (8 px, the reference frame
//! 44's silhouette) → `1611 − 44 = 1567`; and the first surface showing the
//! fade is boot **2269** (`pal` after frame 701, b = 63) after 2268 (b = 64)
//! → `2269 − 702 = 1567`. The block-2 music wait exits at **1117** against
//! `second2b`'s **1111**, so the reset-to-frame-44 span is **494** in the port
//! against the reference's **495**: the port's real player reaches zframe
//! 300/333 one tick earlier relative to the reset than the trace's. The phase
//! machine itself matches the source (the reset and the first zoomer body
//! share a tick, and the scripted-clock unit test pins the rest).
//!
//! **The plan's alignment is therefore `K + 5`.** Main-loop frame `f` is at
//! boot `1567 + f` in the port and `1562 + f` in the reference. The reference
//! capture adds a surface phase: the filed 2300's surface is its linear
//! frame 738 (lag 0), while the filed 3000/3400 surfaces are frames
//! 1437/1837 against counters 1438/1838 (lag 1); its DAC runs one frame ahead
//! of its surface in all three (D17's sample/publish instant). The test pins
//! three layers, each a measurement:
//!
//! - **the plan's `K + 5`** — 2305/3005/3405 against 2300/3000/3400:
//!   **5,208 / 3,114 / 5,261** differing pixels (A36);
//! - **the surface minima** — 2305 (K+5, the 2300 surface), 3004/3404 (K+4,
//!   the one-frame capture lag): **5,208 / 2,289 / 5,941**, the unique minima
//!   of the index-structure metric (their neighbours are 1,859/1,740,
//!   4,486/2,837 and 5,876/4,780 unexplained pixels);
//! - **`K + 6`** — one frame past the plan's alignment, kept to record the
//!   DAC phase: 9,777 / 10,806 / 12,001.
//!
//! At 2305 every difference is a low-palette fade entry: the port's surface
//! frame 738 pairs with `pal` after 737 (b = 27, its upload-before-step
//! pairing), the capture's with the value computed at 738 (b = 26), and the
//! object pixels are byte-identical — the picture evidence for the low-byte
//! `rolcol` reading A37 adopts.
//!
//! No constant, anchor or tick was adjusted to chase these numbers (D24/D17).

use std::path::{Path, PathBuf};
use std::process::Command;

use sr_assets::load_path;

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

/// `FC.UH`'s palette entries 0..15, the picture palette and `backpal`'s source.
fn picture_palette() -> [[u8; 3]; 16] {
    let image = load_path(&repository_root().join("assets/GLENZ/FC.UH"))
        .expect("the committed FC.UH loads");
    std::array::from_fn(|index| image.palette.entry(index as u8))
}

/// The fade's palette: `pal = backpal · b / 64` (`MAIN.C:542`, `:550`), six
/// bits, truncated.
fn faded(palette: &[[u8; 3]; 16], b: i16) -> [[u8; 3]; 16] {
    std::array::from_fn(|index| {
        std::array::from_fn(|channel| ((i16::from(palette[index][channel]) * b) / 64) as u8)
    })
}

/// A six-bit channel widened the way the DAC and the presenter widen it.
fn widened(value: u8) -> u8 {
    ((u16::from(value) * 259 + 33) >> 6) as u8
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

/// The differing RGB pixels split by the port's own palette: `(low, high)`,
/// where `low` counts pixels whose `.idx` is 0..15.
///
/// The low entries are the ones the main loop uploads (`MAIN.C:414-415`); the
/// group entries at 16..255 are the faces' DAC groups.
fn differing_by_palette(rendered: &[u8], expected: &[u8], indices: &[u8]) -> (usize, usize) {
    assert_eq!(rendered.len(), expected.len(), "the frame lengths differ");
    assert_eq!(indices.len(), rendered.len() / 3, "one index per pixel");
    let (mut low, mut high) = (0, 0);
    for (pixel, &index) in indices.iter().enumerate() {
        if rendered[pixel * 3..pixel * 3 + 3] != expected[pixel * 3..pixel * 3 + 3] {
            if index < 16 {
                low += 1;
            } else {
                high += 1;
            }
        }
    }
    (low, high)
}

/// The port's index structure against the reference: the number of pixels
/// whose reference colour is not their port index's most common reference
/// colour, `(unexplained, low-index, high-index, object pixels)`.
///
/// A moving screen's index is the XOR of the faces covering its pixel. When
/// the port and the reference draw the same faces, every index maps to one
/// reference colour; a slid facet boundary shows up here first.
fn unexplained_pixels(
    rendered: &[u8],
    expected: &[u8],
    indices: &[u8],
) -> (usize, usize, usize, usize) {
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
    let (mut unexplained, mut low, mut high, mut total) = (0, 0, 0, 0);
    for (&index, by_colour) in &counts {
        let here: usize = by_colour.values().sum();
        let top = by_colour.values().copied().max().unwrap_or(0);
        total += here;
        unexplained += here - top;
        if index < 16 {
            low += here - top;
        } else {
            high += here - top;
        }
    }
    (unexplained, low, high, total)
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

/// The second (red) object's mask on both sides: `(port, reference,
/// differing)`. The reference has only RGB, so membership is its colour in
/// the rebuilt palette's entries 1..7; the port's is an index below 8
/// (`Glz2`'s `(word >> 1) & 1` and the hidden faces' low bytes 2/4, XORed).
fn red_mask(rendered: &[u8], expected: &[u8], indices: &[u8]) -> (usize, usize, usize) {
    let reds: Vec<[u8; 3]> = rebuilt_reds()[1..]
        .iter()
        .map(|rgb| [widened(rgb[0]), widened(rgb[1]), widened(rgb[2])])
        .collect();
    let is_red = |pixel: &[u8]| reds.contains(&[pixel[0], pixel[1], pixel[2]]);
    assert_eq!(rendered.len(), expected.len(), "the frame lengths differ");
    assert_eq!(indices.len(), rendered.len() / 3, "one index per pixel");
    let (mut port, mut reference, mut diff) = (0, 0, 0);
    for (i, &index) in indices.iter().enumerate() {
        let port_red = index != 0 && index < 8;
        let reference_red = is_red(&expected[i * 3..i * 3 + 3]);
        if port_red {
            port += 1;
        }
        if reference_red {
            reference += 1;
        }
        if port_red != reference_red {
            diff += 1;
        }
    }
    (port, reference, diff)
}

/// The above-floor object pixels of a frame, against its own sky colour:
/// `(count, (min x, max x, min y, max y))`. Rows 0..149 are above the floor
/// (`reference-indices.md` §7.6).
fn above_floor_object(rgb: &[u8]) -> (usize, (usize, usize, usize, usize)) {
    let width = 320;
    let sky = &rgb[0..3];
    let mut points = Vec::new();
    for i in 0..150 * width {
        if &rgb[i * 3..i * 3 + 3] != sky {
            points.push((i % width, i / width));
        }
    }
    let count = points.len();
    let boxed = (
        points.iter().map(|p| p.0).min().unwrap_or(0),
        points.iter().map(|p| p.0).max().unwrap_or(0),
        points.iter().map(|p| p.1).min().unwrap_or(0),
        points.iter().map(|p| p.1).max().unwrap_or(0),
    );
    (count, boxed)
}

/// The object box, count and centroid in the port's own index plane, against
/// the 1499 background: `(min x, max x, min y, max y, count, cx, cy)`.
fn object_box(indices: &[u8], background: &[u8]) -> (usize, usize, usize, usize, usize, f64, f64) {
    let width = 320;
    let points: Vec<(usize, usize)> = (0..150 * width)
        .filter(|&i| indices[i] != background[i])
        .map(|i| (i % width, i / width))
        .collect();
    let count = points.len();
    let xs: Vec<usize> = points.iter().map(|p| p.0).collect();
    let ys: Vec<usize> = points.iter().map(|p| p.1).collect();
    (
        *xs.iter().min().unwrap(),
        *xs.iter().max().unwrap(),
        *ys.iter().min().unwrap(),
        *ys.iter().max().unwrap(),
        count,
        xs.iter().sum::<usize>() as f64 / count as f64,
        ys.iter().sum::<usize>() as f64 / count as f64,
    )
}

/// The images the keyframe tests read: the held opening and the moving
/// indices at the plan's alignment (`K + 5`), one past it (`K + 6`) and the
/// capture's surface minima (`K + 5` at 2300, `K + 4` at 3000/3400).
const FRAMES: &str = "1499,2304,2305,2306,3003,3004,3005,3006,3403,3404,3405,3406";

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

    for &index in &[
        1499u32, 2304, 2305, 2306, 3003, 3004, 3005, 3006, 3403, 3404, 3405, 3406,
    ] {
        let frame = entry(&manifest, index);
        assert_eq!(frame["part"], "glenz", "boot {index} is GLENZ's screen");
        assert_eq!(frame["width"], 320);
        assert_eq!(frame["height"], 200);
        assert!(
            dir.join(frame["idx"].as_str().unwrap()).is_file(),
            "boot {index} wrote the .idx it names"
        );
    }

    // The port's music position at the filed indices. The filed `second2b`
    // trace is one row ahead at all four (1499 5:12, 2306 8:20, 3006 11:1,
    // 3406 12:36) — the six-tick wait offset A36 registers, seen on the
    // published position.
    assert_eq!(entry(&manifest, 1499)["music_position"], "5:11");
    assert_eq!(entry(&manifest, 2306)["music_position"], "8:19");
    assert_eq!(entry(&manifest, 3006)["music_position"], "11:0");
    assert_eq!(entry(&manifest, 3406)["music_position"], "12:35");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_loop_start_is_1567() {
    // The reset is boot 1117: 1115 and 1116 are still BEGLOGO's held
    // 320x400 picture (the zoomer inherits it), and 1117 is the zoomer's
    // first body — row 0 and row 399 (640 px). The block-2 wait's exit is
    // six ticks after `second2b`'s 1111.
    let (dir, manifest) = dump("phase", "1115,1116,1117,1610,1611,1612,2268,2269");
    let at_1115 = dumped_file(&dir, &manifest, 1115, "rgb");
    let at_1116 = dumped_file(&dir, &manifest, 1116, "rgb");
    let at_1117 = dumped_file(&dir, &manifest, 1117, "rgb");
    assert_eq!(
        differing(&at_1116, &at_1115, 3),
        0,
        "1115/1116 are the held picture"
    );
    assert_eq!(
        differing(&at_1117, &at_1116, 3),
        640,
        "1117 is the zoomer's first body (rows 0 and 399)"
    );

    // The first object pixels: main-loop frame 44's 8-pixel silhouette at
    // boot 1611, none at 1610, 21 px at 1612 (frames 45).
    let (none, _) = above_floor_object(&dumped_file(&dir, &manifest, 1610, "rgb"));
    assert_eq!(none, 0, "frame 43 draws no object pixel");
    let (eight, boxed) = above_floor_object(&dumped_file(&dir, &manifest, 1611, "rgb"));
    assert_eq!((eight, boxed), (8, (172, 179, 0, 0)), "frame 44 at 1611");
    let (twenty_one, boxed) = above_floor_object(&dumped_file(&dir, &manifest, 1612, "rgb"));
    assert_eq!(
        (twenty_one, boxed),
        (21, (166, 179, 0, 1)),
        "frame 45 at 1612"
    );

    // The first faded surface: the fade begins at main frame 701 and the
    // port uploads `pal` before stepping, so frame 702 at boot 2269 shows
    // b = 63; 2268 (frame 701) still shows b = 64.
    let palette = picture_palette();
    for &(boot, b) in &[(2268u32, 64i16), (2269, 63)] {
        let rgb = dumped_file(&dir, &manifest, boot, "rgb");
        let indices = dumped_file(&dir, &manifest, boot, "idx");
        let expected = faded(&palette, b);
        let mismatched = (0..320 * 200)
            .filter(|&i| indices[i] < 16)
            .filter(|&i| {
                let pixel = &rgb[i * 3..i * 3 + 3];
                let entry = &expected[indices[i] as usize];
                pixel != [widened(entry[0]), widened(entry[1]), widened(entry[2])]
            })
            .count();
        assert_eq!(mismatched, 0, "boot {boot}: every low pixel is b = {b}");
    }

    // Both measurements give the same loop start.
    assert_eq!(1611 - 44, 1567, "first object: 1611 - frame 44");
    assert_eq!(2269 - 702, 1567, "first fade: 2269 - frame 702");
    // The reset-to-frame-44 span: the port's 494 against the reference's
    // 495 (1606 - 1111; `reference-indices.md` §7.6). The one-tick difference
    // is the port's real player reaching zframe 300/333 one tick earlier
    // relative to the reset than `second2b`'s; the phase machine's own
    // relative ticks match the source.
    assert_eq!(1611 - 1117, 494);
    assert_eq!(1606 - 1111, 495);

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
fn the_moving_keyframes_record_the_registered_alignments_and_residuals() {
    let (dir, manifest) = dump("keyframes", FRAMES);
    let reference = |index: u32| reference_frame(index, "rgb");
    let frame = |index: u32| dumped_file(&dir, &manifest, index, "rgb");
    let indices = |index: u32| dumped_file(&dir, &manifest, index, "idx");

    // A36: the plan's `K + 5` — L_p 1567 against L_r 1562. The residual is
    // the capture's DAC phase; at 2305 it is the low fade palette alone.
    for &(port, filed, expected, unexplained) in &[
        (2305u32, 2300u32, 5208usize, 0usize),
        (3005, 3000, 3114, 2837),
        (3405, 3400, 5261, 4780),
    ] {
        let count = differing(&frame(port), &reference(filed), 3);
        let (not_explained, _, _, _) =
            unexplained_pixels(&frame(port), &reference(filed), &indices(port));
        println!("boot {port} vs {filed} (K+5): {count} px, {not_explained} unexplained");
        assert_eq!(count, expected, "A36's K+5 residual at {filed}");
        assert_eq!(
            not_explained, unexplained,
            "A36's K+5 index structure at {filed}"
        );
    }

    // A37: the capture's surface minima. 2305 is the 2300 surface (lag 0);
    // 3004/3404 are the 3000/3400 surfaces (the capture's one-frame lag), and
    // the unique minima of the index-structure metric — neighbours 1,859 /
    // 1,740, 4,486 / 2,837 and 5,876 / 4,780.
    for &(port, filed, expected, unexplained) in &[
        (2305u32, 2300u32, 5208usize, 0usize),
        (3004, 3000, 2289, 711),
        (3404, 3400, 5941, 839),
    ] {
        let count = differing(&frame(port), &reference(filed), 3);
        let (not_explained, _, _, _) =
            unexplained_pixels(&frame(port), &reference(filed), &indices(port));
        println!("boot {port} vs {filed} (surface): {count} px, {not_explained} unexplained");
        assert_eq!(count, expected, "A37's surface residual at {filed}");
        assert_eq!(
            not_explained, unexplained,
            "A37's surface index structure at {filed}"
        );
    }

    // One frame past the plan's alignment: the capture's DAC phase.
    for &(port, filed, expected, unexplained) in &[
        (2306u32, 2300u32, 9777usize, 1740usize),
        (3006, 3000, 10806, 5762),
        (3406, 3400, 12001, 7974),
    ] {
        let count = differing(&frame(port), &reference(filed), 3);
        let (not_explained, _, _, _) =
            unexplained_pixels(&frame(port), &reference(filed), &indices(port));
        println!("boot {port} vs {filed} (K+6): {count} px, {not_explained} unexplained");
        assert_eq!(count, expected, "the K+6 (one past) residual at {filed}");
        assert_eq!(
            not_explained, unexplained,
            "the K+6 index structure at {filed}"
        );
    }

    // At 2305 (main frame 738) the port pairs the frame with `pal` after 737
    // (b = 27); the capture pairs the same surface with the value computed
    // at 738 (b = 26). Every low-palette pixel differs for that reason and
    // every object pixel is identical.
    let rgb_2305 = frame(2305);
    let reference_2300 = reference(2300);
    let idx_2305 = indices(2305);
    assert_eq!(
        differing_by_palette(&rgb_2305, &reference_2300, &idx_2305),
        (5208, 0),
        "the 2300 residual is the capture's fade palette"
    );
    let palette = picture_palette();
    let port_b27 = faded(&palette, 27);
    let capture_b26 = faded(&palette, 26);
    let (mut port_bad, mut capture_bad) = (0, 0);
    for i in 0..320 * 200 {
        if idx_2305[i] >= 16 {
            continue;
        }
        let entry = idx_2305[i] as usize;
        let expected = [
            widened(port_b27[entry][0]),
            widened(port_b27[entry][1]),
            widened(port_b27[entry][2]),
        ];
        if rgb_2305[i * 3..i * 3 + 3] != expected {
            port_bad += 1;
        }
        if reference_2300[i * 3..i * 3 + 3] != rgb_2305[i * 3..i * 3 + 3] {
            let expected = [
                widened(capture_b26[entry][0]),
                widened(capture_b26[entry][1]),
                widened(capture_b26[entry][2]),
            ];
            if reference_2300[i * 3..i * 3 + 3] != expected {
                capture_bad += 1;
            }
        }
    }
    assert_eq!(port_bad, 0, "the port's frame 738 renders b = 27");
    assert_eq!(capture_bad, 0, "the capture's 2300 renders b = 26");

    // The 2305 object is the filed 2300's: box and centroid from the port's
    // own index plane against the 1499 background.
    let background = dumped_file(&dir, &manifest, 1499, "idx");
    let (x0, x1, y0, y1, count, cx, cy) = object_box(&idx_2305, &background);
    println!("boot 2305 object: x {x0}..{x1}, y {y0}..{y1}, {count} px, ({cx:.1}, {cy:.1})");
    assert_eq!((x0, x1, y0, y1, count), (97, 217, 38, 149, 9989));
    assert!((cx - 156.8).abs() < 0.05 && (cy - 94.1).abs() < 0.05);

    // The next displayed frame (739 at boot 2306) is the 9,927-px box the
    // partner sessions present at 2300 (and Task 1's `out/p7-stability`).
    let (x0, x1, y0, y1, count, cx, cy) = object_box(&indices(2306), &background);
    println!("boot 2306 object: x {x0}..{x1}, y {y0}..{y1}, {count} px, ({cx:.1}, {cy:.1})");
    assert_eq!((x0, x1, y0, y1, count), (99, 217, 37, 149, 9927));
    assert!((cx - 157.6).abs() < 0.05 && (cy - 94.2).abs() < 0.05);

    // The chosen surface frames are the unique minima of the index-structure
    // metric; these are their neighbours (the same comparison one frame each
    // side).
    for &(port, filed, unexplained) in &[
        (2304u32, 2300u32, 1859usize),
        (3003, 3000, 4486),
        (3403, 3400, 5876),
    ] {
        let (not_explained, _, _, _) =
            unexplained_pixels(&frame(port), &reference(filed), &indices(port));
        println!("neighbour {port} vs {filed}: {not_explained} unexplained");
        assert_eq!(
            not_explained, unexplained,
            "the surface minimum's neighbour at {filed}"
        );
    }

    // The surface minima's remaining residual: the second object's interior
    // XOR boundaries. 3004: 710 of 711 at indices < 16 (one at >=16); 3404:
    // all 839 at indices < 16. Its outer red silhouette matches within
    // 118/180 px, the port's red masks holding 27,057/33,312 px.
    for &(port, filed, split, red) in &[
        (
            3004u32,
            3000u32,
            (710usize, 1usize, 35_896usize),
            (27_057usize, 27_143usize, 118usize),
        ),
        (3404, 3400, (839, 0, 43_938), (33_312, 33_416, 180)),
    ] {
        let (not_explained, low, high, object) =
            unexplained_pixels(&frame(port), &reference(filed), &indices(port));
        assert_eq!(
            (low, high, object),
            split,
            "A37's unexplained split and object total at {filed}"
        );
        assert_eq!(low + high, not_explained);
        let (port_red, reference_red, red_diff) =
            red_mask(&frame(port), &reference(filed), &indices(port));
        assert_eq!(
            (port_red, reference_red, red_diff),
            red,
            "A37's red-object masks at {filed}"
        );
        let (low_differing, high_differing) =
            differing_by_palette(&frame(port), &reference(filed), &indices(port));
        println!(
            "boot {port} vs {filed}: {not_explained} unexplained ({low} low / {high} high) of \
             {object}; red masks {port_red}/{reference_red} differ {red_diff}; \
             {low_differing} low-palette / {high_differing} group px differ"
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
