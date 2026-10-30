//! GLENZ through the real binary: the block-2 numbering and music positions,
//! the measured loop start, and the A36/A37 analysis of the moving keyframes.
//!
//! The pixel counts rediff registers (1499 at 0 in both planes; 2305 / 3004 /
//! 3404 at 5,208 / 2,289 / 5,941) are `tests/reference_frames.rs`'. What stays
//! here is the evidence for A36's `K + 5` alignment and A37's residual
//! (`glenz.md` §11): the loop start `L_p = 1567`, the 2305 residual being the
//! capture's fade palette alone, the index-structure minima, and the second
//! object's masks. No constant, anchor or tick was adjusted to chase these
//! numbers (D24/D17).

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use sr_assets::load_path;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every frame the tests read, from one headless block-2 run: the loop-start
/// evidence (1115..1117, 1610..1612, 2268/2269), the held 1499, and each
/// moving keyframe's surface minimum with its neighbours.
const FRAMES: &str = "1115,1116,1117,1499,1610,1611,1612,2268,2269,\
                      2304,2305,2306,3003,3004,3005,3403,3404,3405";

/// The block-2 run's dump directory and manifest, made once per test binary.
fn dump() -> &'static (PathBuf, serde_json::Value) {
    static DUMP: OnceLock<(PathBuf, serde_json::Value)> = OnceLock::new();
    DUMP.get_or_init(|| {
        let dir = std::env::temp_dir().join(format!("sr-gl-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
            .current_dir(repository_root())
            .args(["--block", "2", "--headless", "--dump-frames", FRAMES, "--dump-dir"])
            .arg(&dir)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
        let manifest =
            serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap())
                .unwrap();
        (dir, manifest)
    })
}

/// The manifest's entry for `index`.
fn entry(index: u32) -> &'static serde_json::Value {
    dump().1["frames"]
        .as_array()
        .unwrap()
        .iter()
        .find(|frame| frame["index"].as_u64() == Some(u64::from(index)))
        .unwrap_or_else(|| panic!("the manifest holds no frame {index}"))
}

/// One dumped file, by the manifest's own name for it.
fn dumped_file(index: u32, suffix: &str) -> Vec<u8> {
    let dir: &Path = &dump().0;
    let name = entry(index)[suffix].as_str().unwrap();
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

fn widened_entry(rgb: [u8; 3]) -> [u8; 3] {
    rgb.map(widened)
}

/// How many `unit`-byte pixels differ; the lengths are asserted first, so a
/// short read is not a match.
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

#[test]
fn the_block_two_run_is_numbered_in_boot_frames_and_reaches_glenz() {
    let manifest = &dump().1;
    assert_eq!(manifest["part"], "glenz");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404, "block 2 draws from the boot's 404");
    // `MAIN/U2.ASM:766-775`: block 2 starts MUSIC1 at order 0; the clock's
    // own start is 408 (D26).
    assert_eq!(manifest["music"]["module"], "MUSIC1");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 408);
    for frame in manifest["frames"].as_array().unwrap() {
        assert_eq!(frame["part"], "glenz", "{} is GLENZ's screen", frame["index"]);
    }
    for index in [1499u32, 2305, 3004, 3404] {
        assert_eq!(entry(index)["width"], 320);
        assert_eq!(entry(index)["height"], 200);
    }
    // The filed `second2b` trace is one row ahead at 1499 (5:12) and 2306
    // (8:20): the six-tick wait offset A36 registers, seen on the published
    // position.
    assert_eq!(entry(1499)["music_position"], "5:11");
    assert_eq!(entry(2306)["music_position"], "8:19");
}

/// `L_p = 1567` two ways (A36): main frame 44's first object pixels at 1611,
/// and the first faded surface at 2269 (frame 702, b = 63). The reset is
/// 1117, six ticks after `second2b`'s 1111.
#[test]
fn the_loop_start_is_1567() {
    let rgb = |index| dumped_file(index, "rgb");
    assert_eq!(differing(&rgb(1116), &rgb(1115), 3), 0, "1115/1116 are the held picture");
    let first_body = differing(&rgb(1117), &rgb(1116), 3);
    assert_eq!(first_body, 640, "1117 is the zoomer's first body (rows 0 and 399)");

    assert_eq!(above_floor_object(&rgb(1610)).0, 0, "frame 43 draws no object pixel");
    assert_eq!(above_floor_object(&rgb(1611)), (8, (172, 179, 0, 0)), "frame 44 at 1611");
    assert_eq!(above_floor_object(&rgb(1612)), (21, (166, 179, 0, 1)), "frame 45 at 1612");

    // The port uploads `pal` before stepping, so frame 702 at 2269 shows
    // b = 63 and frame 701 at 2268 still b = 64.
    let palette = picture_palette();
    for (boot, b) in [(2268u32, 64i16), (2269, 63)] {
        let (rgb, indices) = (rgb(boot), dumped_file(boot, "idx"));
        let expected = faded(&palette, b);
        let mismatched = (0..320 * 200)
            .filter(|&i| indices[i] < 16)
            .filter(|&i| rgb[i * 3..i * 3 + 3] != widened_entry(expected[indices[i] as usize]))
            .count();
        assert_eq!(mismatched, 0, "boot {boot}: every low pixel is b = {b}");
    }
}

#[test]
fn the_moving_keyframes_record_the_registered_alignments_and_residuals() {
    let reference = |index: u32| reference_frame(index, "rgb");
    let frame = |index: u32| dumped_file(index, "rgb");
    let indices = |index: u32| dumped_file(index, "idx");
    let unexplained =
        |port: u32, filed: u32| unexplained_pixels(&frame(port), &reference(filed), &indices(port));

    // A36: the plan's `K + 5` at 3000/3400 (2305 is rediff's).
    for (port, filed, expected) in [(3005u32, 3000u32, 3114usize), (3405, 3400, 5261)] {
        assert_eq!(differing(&frame(port), &reference(filed), 3), expected, "K+5 at {filed}");
    }

    // A37: the registered surfaces are the unique minima of the
    // index-structure metric, against their neighbours one frame each side.
    for (filed, [before, minimum, after]) in [
        (2300u32, [(2304u32, 1859usize), (2305, 0), (2306, 1740)]),
        (3000, [(3003, 4486), (3004, 711), (3005, 2837)]),
        (3400, [(3403, 5876), (3404, 839), (3405, 4780)]),
    ] {
        for (port, count) in [before, minimum, after] {
            assert_eq!(unexplained(port, filed).0, count, "unexplained at {port} against {filed}");
        }
    }

    // At 2305 (main frame 738) every differing pixel is a low-palette entry:
    // the port pairs the frame with `pal` after 737 (b = 27), the capture
    // with the value computed at 738 (b = 26).
    let (rgb_2305, reference_2300, idx_2305) = (frame(2305), reference(2300), indices(2305));
    assert_eq!(differing_by_palette(&rgb_2305, &reference_2300, &idx_2305), (5208, 0));
    let palette = picture_palette();
    let (port_b27, capture_b26) = (faded(&palette, 27), faded(&palette, 26));
    for i in (0..320 * 200).filter(|&i| idx_2305[i] < 16) {
        let entry = idx_2305[i] as usize;
        let (port, capture) = (&rgb_2305[i * 3..i * 3 + 3], &reference_2300[i * 3..i * 3 + 3]);
        assert_eq!(port, widened_entry(port_b27[entry]), "the port's frame 738 renders b = 27");
        if capture != port {
            let expected = widened_entry(capture_b26[entry]);
            assert_eq!(capture, expected, "the capture's 2300 renders b = 26");
        }
    }

    // The object boxes against the 1499 background: 2305's is the filed
    // 2300's; 2306's is the low-byte model's row in `glenz.md` §11.2.
    let background = indices(1499);
    let (x0, x1, y0, y1, count, cx, cy) = object_box(&idx_2305, &background);
    assert_eq!((x0, x1, y0, y1, count), (97, 217, 38, 149, 9989));
    assert!((cx - 156.8).abs() < 0.05 && (cy - 94.1).abs() < 0.05);
    let (x0, x1, y0, y1, count, cx, cy) = object_box(&indices(2306), &background);
    assert_eq!((x0, x1, y0, y1, count), (99, 217, 37, 149, 9927));
    assert!((cx - 157.6).abs() < 0.05 && (cy - 94.2).abs() < 0.05);

    // The surface minima's residual is the second object's interior XOR
    // boundaries: (low, high) unexplained of the object total, and its red
    // silhouette (port, reference, differing).
    for (port, filed, split, red) in [
        (3004u32, 3000u32, (710usize, 1usize, 35_896usize), (27_057usize, 27_143usize, 118usize)),
        (3404, 3400, (839, 0, 43_938), (33_312, 33_416, 180)),
    ] {
        let (_, low, high, object) = unexplained(port, filed);
        assert_eq!((low, high, object), split, "A37's unexplained split at {filed}");
        let masks = red_mask(&frame(port), &reference(filed), &indices(port));
        assert_eq!(masks, red, "A37's red masks at {filed}");
    }
}
