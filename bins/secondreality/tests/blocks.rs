//! `--block` runs through the real binary: each block's parts in the demo's order on the one
//! reference numbering, the indices where they hand over, the music the shell installs and
//! restarts against the traces, and the parts only a block run reaches (U2A, GLENZ's block-2
//! keyframes, U2E, CRED, ENDSCRL). The keyframes' pixel counts are `cargo xtask rediff`'s, run as
//! `tests/reference_frames.rs`; the whole of block 1 is `tests/fullrun.rs`.
//!
//! Each block's tests share one run that dumps the union of the frames they read.

mod common;

use common::{differing, dump, reference, trace, trace_position, Dump};

/// `(index, part, width, height)` for each index, from the dump's own manifest.
fn assert_listing(dump: &Dump, expected: &[(u32, &str, u64, u64)]) {
    for &(index, part, width, height) in expected {
        let frame = dump.frame(index);
        assert_eq!((frame.part.as_str(), frame.width, frame.height), (part, width, height), "frame {index}");
    }
}

/// The position the first of `traces` that covers `frame` holds there. Sessions that overlap
/// are different sessions whose rows may differ by one at the overlap, so a frame takes the
/// first, rather than having to agree across them.
fn first_covering(traces: &[&str], frame: u32) -> String {
    traces
        .iter()
        .find(|name| trace(name).contains_key(&frame))
        .map(|name| trace_position(name, frame))
        .unwrap_or_else(|| panic!("no trace of {traces:?} holds boot frame {frame}"))
}

/// `SECOND 1`'s traces: `second1` covers 148..5999, `second1c` 5900..7599 (U2A's gates) and
/// `second1b` 7600..7959 (PAM's opening).
const BLOCK1_TRACES: &[&str] = &["second1", "second1c", "second1b"];

/// `SECOND 3`'s traces, which tile the block's boot timeline: `second3` 144..2899, `second3a`
/// 2900..10199, `second3b` 10200..15049, `second3d` 15000..17799 and `second3e` 18000..21999
/// (the mid-run restart and U2E's opening, D34). `second3b` and `second3d` overlap at
/// 15000..15049 a row apart.
const BLOCK3_TRACES: &[&str] = &["second3", "second3a", "second3b", "second3d", "second3e"];

/// Block 1 stopped at PAM, before the GLENZ restart replaces the entry's player: ALKU's four
/// keyframes, U2A's gate and saucer, and PAM holding U2A's cleared page.
const BLOCK1_ENTRY: &[u32] = &[900, 1719, 2719, 5859, 5900, 6500, 7299, 7300, 7301, 7640];

/// Block 1 through ALKU, U2A, PAM and BEGLOGO into GLENZ's first zoomer body.
const BLOCK1_CHAIN: &[u32] = &[
    900, 5955, 5956, 6500, 6988, 7300, 7507, 7508, 7618, 7709, 7710, 7712, 7884, 7885, 8055, 8056,
    8057, 8086, 8087, 8523, 8524, 8525, 8526, 8527, 8528,
];

#[test]
fn a_block_one_run_starts_at_alku_and_is_the_same_run() {
    // `SECOND 1`'s plan begins at ALKU (`MAIN/U2.ASM:790-792`): the block run and the `--part`
    // run share the anchor, the recipe and the bytes.
    let keyframes = [900, 1719, 2719, 5859];
    let part = dump(&["--part", "alku"], &keyframes);
    let block = dump(&["--block", "1"], BLOCK1_ENTRY);
    assert_eq!(block.manifest["part"], "alku");
    assert_eq!(block.manifest["index_origin"], "boot");
    assert_eq!(block.manifest["anchor"], 404);
    assert_eq!(block.manifest["music"], part.manifest["music"]);
    for index in keyframes {
        assert_eq!(block.rgb(index), part.rgb(index), "boot {index}: the block run and the part run differ");
    }
}

#[test]
fn the_run_is_numbered_in_boot_frames_and_reaches_u2a_after_alku() {
    // U2A has no anchor of its own; block 1 draws from ALKU's 404 under `_zplaysong(0)`
    // (`MAIN/U2.ASM:766-775`), pre-polled 400..403.
    let run = dump(&["--block", "1"], BLOCK1_ENTRY);
    assert_eq!(run.manifest["music"]["module"], "MUSIC0");
    assert_eq!(run.manifest["music"]["start_order"], 0);
    assert_eq!(run.manifest["music"]["start_frame"], 404);
    // Each frame is named for the part running: U2A until its END, then PAM holding U2A's
    // cleared page through its gate.
    assert_listing(&run, &[(6500, "u2a", 320, 200), (7300, "u2a", 320, 200), (7640, "pam", 320, 200)]);
    for index in [6500, 7300] {
        assert_eq!(run.position(index), trace_position("second1c", index), "{index}");
    }
    // One page holds the saucer's pose across 7299-7301 (the reference's neighbour phase is
    // its session's, `reference-indices.md` §5.3).
    assert_eq!(run.idx(7299), run.idx(7300), "7299 holds");
    assert_eq!(run.idx(7301), run.idx(7300), "7301 holds");
}

#[test]
fn the_block_one_clock_matches_second1_at_boot_5900() {
    // A29's mechanism: the block-1 clock follows `second1`, not `second1c` (the session that
    // supplies the 6988 gate, four frames behind it here).
    let run = dump(&["--block", "1"], BLOCK1_ENTRY);
    assert_eq!(run.position(5900), "9:51");
    assert_eq!(run.position(5900), trace_position("second1", 5900));
    assert_eq!(trace_position("second1c", 5900), "9:50");
}

#[test]
fn a_block_one_run_is_alku_then_u2a_then_pam_in_the_boots_numbering() {
    // `SECOND 1` runs ALKU, U2A and PAM (`MAIN/U2.ASM:790-799`), then BEGLOGO and GLENZ.
    //
    // **The hand-overs are measured, not fitted.** The parts hand over with no loader gap
    // (D31 §3): ALKU's last frame is 5,955, the release's closing 320×200 page (A20), so U2A
    // starts at 5,956, 5 frames before the reference's first 320×200 frame (§5: 5,960/5,961).
    // U2A's stream ends before the reference's second gate (D33), so PAM starts at 7,508. PAM's
    // 45 four-tick steps end on 7,884; BEGLOGO's 172 ticks end on 8,056; GLENZ starts on 8,057
    // under the `:813-818` restart's player.
    let run = dump(&["--block", "1"], BLOCK1_CHAIN);
    assert_eq!(run.manifest["part"], "alku");
    assert_eq!(run.manifest["index_origin"], "boot");
    assert_eq!(run.manifest["anchor"], 404);
    // The manifest's music is the player last installed: the `:813-818` restart's MUSIC1 at
    // order 0, sought at the boundary 8,057.
    assert_eq!(run.manifest["music"]["module"], "MUSIC1");
    assert_eq!(run.manifest["music"]["start_order"], 0);
    assert_eq!(run.manifest["music"]["start_frame"], 8_057);

    let mut expected = vec![(900, "alku", 320, 400), (5955, "alku", 320, 200)];
    expected.extend([5956, 6500, 6988, 7300, 7507].map(|index| (index, "u2a", 320, 200)));
    expected.extend([7508, 7618, 7709, 7710, 7712, 7884].map(|index| (index, "pam", 320, 200)));
    expected.extend([7885, 8055, 8056].map(|index| (index, "beglogo", 320, 400)));
    expected.extend([8057, 8086, 8087, 8523, 8524, 8525, 8526, 8527, 8528].map(|index| (index, "glenz", 320, 400)));
    assert_listing(&run, &expected);

    // The music is the traces' at the same boot index: at the hand-overs, U2A's first gate
    // (6,988, `U2A.C:238`: order > 10 and row > 46, `second1c`'s 11:47), its second (7,618,
    // `second1b`'s 12:55, though the port's U2A has already retired) and PAM's first drawn
    // frame 7,710 (§6.1, §6.4).
    for index in [900, 5956, 6988, 7508, 7618, 7710, 7884, 7885] {
        assert_eq!(run.position(index), first_covering(BLOCK1_TRACES, index), "chain frame {index}");
    }
    assert_eq!(first_covering(BLOCK1_TRACES, 6_988), "11:47", "U2A's first gate");
    assert_eq!(first_covering(BLOCK1_TRACES, 7_618), "12:55", "U2A's second gate");

    // **The GLENZ boundary's music** (D38, D45). The restart continues from MUSIC0's channel
    // state: channel 2 is still at 63, so order 0's row-3 `C00` is taken and order 0 lasts 4
    // rows, publishing `1:0` 30 ticks after the boundary (D34's four of waiting, then four rows
    // of speed 3 and the ring's ten). A fresh player would publish `0:4` there.
    assert_eq!(run.position(8_086), "0:3", "order 0's last row");
    assert_eq!(run.position(8_087), "1:0", "order 0 is 4 rows long");
    // The wait is `while (dis_musplus() < -19)` (`GLENZ/MAIN.C:290`); `musplus` is −19 at 2:45,
    // where the reference's wait exits (8,521 in `second1d`). The port reaches it at 8,527,
    // its first zoomer body: 6 frames after 8,521, 5 of them the restarted clock's lag (A38)
    // and 1 D17's (the picture first differs at 8,522), so `FULLRUN_SEAMS`' GLENZ row is
    // 8,522 → 8,527.
    for index in [8_523, 8_524, 8_525, 8_526] {
        assert_eq!(run.rgb(index), run.rgb(8_057), "{index} still holds BEGLOGO's last picture");
    }
    assert_ne!(run.rgb(8_527), run.rgb(8_057), "the zoomer's first body draws over it");
    assert_eq!(run.position(8_526), "2:44", "the wait's last holding tick");
    assert_eq!(run.position(8_527), "2:45", "musplus −19: the wait's exit");
    assert_eq!(trace_position("second1d", 8_521), "2:45", "the reference's block-1 wait exit");
}

/// Block 2: BEGLOGO's last frames, GLENZ's wait and its exit, and the frames GLENZ's analysis
/// reads (the loop-start evidence, the held 1499, each moving keyframe's surface minimum with
/// its neighbours).
const BLOCK2: &[u32] = &[
    574, 575, 576, 577, 1100, 1115, 1116, 1117, 1499, 1610, 1611, 1612, 2268, 2269, 2304, 2305,
    2306, 3003, 3004, 3005, 3403, 3404, 3405,
];

#[test]
fn a_block_two_run_reaches_glenz_without_reloading_music_one() {
    // `SECOND 2`'s mask is `0x1e`: BEGLOGO (`MAIN/U2.ASM:801-811`), then GLENZ (`:812-815`).
    // BEGLOGO is 172 ticks from 404 (32 vertical blanks, the mode set's tick, A5's nine
    // unmodelled frames, 129 fade steps and `BEG/BEG.C:86`'s closing write), so GLENZ's first
    // frame is 576. `whattorun & 1` is clear, so `:813-814` skips the restart: the music is
    // still the entry install at 408, not a reload at 576.
    let run = dump(&["--block", "2"], BLOCK2);
    assert_eq!(run.manifest["part"], "beglogo");
    assert_eq!(run.manifest["index_origin"], "boot");
    assert_eq!(run.manifest["anchor"], 404);
    assert_eq!(run.manifest["music"]["module"], "MUSIC1");
    assert_eq!(run.manifest["music"]["start_order"], 0);
    assert_eq!(run.manifest["music"]["start_frame"], 408);
    let mut expected = vec![(574, "beglogo", 320, 400), (575, "beglogo", 320, 400)];
    expected.extend([576, 577, 1100, 1116, 1117].map(|index| (index, "glenz", 320, 400)));
    assert_listing(&run, &expected);

    // The wait's exit: `second2b` holds 2:44 at 1,110 and exits at 1,111 (2:45, `musplus` −19);
    // the port holds 2:44 through 1,116 and draws its first zoomer body at 2:45 on 1,117. The
    // six ticks are a session-phase difference of A12's class, registered (A36) rather than
    // absorbed, and GLENZ's moving keyframes are aligned by it.
    assert_eq!(run.rgb(1_116), run.rgb(576), "1,116 still holds the picture");
    assert_ne!(run.rgb(1_117), run.rgb(576), "the zoomer's first body draws at 1,117");
    assert_eq!(run.position(1_116), "2:44");
    assert_eq!(run.position(1_117), "2:45", "the wait's exit condition");
    assert_eq!(trace_position("second2b", 1_110), "2:44", "the reference's last holding tick");
    assert_eq!(trace_position("second2b", 1_111), "2:45", "the reference's exit");
}

/// GLENZ in block 2: the measured loop start and the A36/A37 analysis of the moving keyframes
/// (`glenz.md` §11). No constant, anchor or tick was adjusted to chase these numbers (D24/D17).
mod glenz {
    use super::*;
    use sr_assets::load_path;

    fn run() -> std::sync::Arc<Dump> {
        dump(&["--block", "2"], BLOCK2)
    }

    /// `FC.UH`'s palette entries 0..15, the picture palette and `backpal`'s source.
    fn picture_palette() -> [[u8; 3]; 16] {
        let image = load_path(&common::root().join("assets/GLENZ/FC.UH")).expect("the committed FC.UH loads");
        std::array::from_fn(|index| image.palette.entry(index as u8))
    }

    /// The fade's palette: `pal = backpal · b / 64` (`MAIN.C:542`, `:550`), six bits, truncated.
    fn faded(palette: &[[u8; 3]; 16], b: i16) -> [[u8; 3]; 16] {
        std::array::from_fn(|index| {
            std::array::from_fn(|channel| ((i16::from(palette[index][channel]) * b) / 64) as u8)
        })
    }

    /// A six-bit channel widened the way the DAC and the presenter widen it.
    fn widened(rgb: [u8; 3]) -> [u8; 3] {
        rgb.map(|value| ((u16::from(value) * 259 + 33) >> 6) as u8)
    }

    /// The differing RGB pixels split by the port's palette: `(low, high)`, where `low` counts
    /// pixels whose `.idx` is 0..15, the entries the main loop uploads (`MAIN.C:414-415`).
    fn differing_by_palette(rendered: &[u8], expected: &[u8], indices: &[u8]) -> (usize, usize) {
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

    /// The port's index structure against the reference: the pixels whose reference colour is
    /// not their port index's most common reference colour, `(unexplained, low-index,
    /// high-index, object pixels)`. A moving screen's index is the XOR of the faces covering
    /// it, so when both sides draw the same faces every index maps to one colour.
    fn unexplained_pixels(rendered: &[u8], expected: &[u8], indices: &[u8]) -> (usize, usize, usize, usize) {
        use std::collections::HashMap;
        let pixel = |bytes: &[u8], i: usize| bytes[i * 3..i * 3 + 3].to_vec();
        let mut counts: HashMap<u8, HashMap<Vec<u8>, usize>> = HashMap::new();
        for (i, &index) in indices.iter().enumerate() {
            let (port, reference) = (pixel(rendered, i), pixel(expected, i));
            if port == [0, 0, 0] || reference == [0, 0, 0] {
                continue;
            }
            *counts.entry(index).or_default().entry(reference).or_default() += 1;
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

    /// The second (red) object's mask on both sides: `(port, reference, differing)`. The
    /// reference's membership is its colour in the frame-786 `backpal` rebuild's reds 1..7
    /// (`MAIN.C:557-584`); the port's is an index below 8 (`Glz2`'s `(word >> 1) & 1` and the
    /// hidden faces' low bytes 2/4, XORed).
    fn red_mask(expected: &[u8], indices: &[u8]) -> (usize, usize, usize) {
        let reds: Vec<[u8; 3]> = (1..8u16)
            .map(|a| {
                let r = 10 * (a & 1) + 30 * ((a >> 1) & 1) + 20 * ((a >> 2) & 1);
                widened([r.min(63) as u8, 0, 0])
            })
            .collect();
        let (mut port, mut reference, mut diff) = (0, 0, 0);
        for (i, &index) in indices.iter().enumerate() {
            let port_red = index != 0 && index < 8;
            let reference_red = reds.contains(&[expected[i * 3], expected[i * 3 + 1], expected[i * 3 + 2]]);
            port += usize::from(port_red);
            reference += usize::from(reference_red);
            diff += usize::from(port_red != reference_red);
        }
        (port, reference, diff)
    }

    /// The above-floor object pixels (rows 0..149, `reference-indices.md` §7.6) against the
    /// frame's own sky colour: `(count, (min x, max x, min y, max y))`.
    fn above_floor_object(rgb: &[u8]) -> (usize, (usize, usize, usize, usize)) {
        let sky = &rgb[0..3];
        let points: Vec<(usize, usize)> =
            (0..150 * 320).filter(|i| &rgb[i * 3..i * 3 + 3] != sky).map(|i| (i % 320, i / 320)).collect();
        let extent = |pick: fn(&(usize, usize)) -> usize| {
            (points.iter().map(pick).min().unwrap_or(0), points.iter().map(pick).max().unwrap_or(0))
        };
        let ((x0, x1), (y0, y1)) = (extent(|p| p.0), extent(|p| p.1));
        (points.len(), (x0, x1, y0, y1))
    }

    /// The object box, count and centroid in the port's index plane against the 1499
    /// background: `(min x, max x, min y, max y, count, cx, cy)`.
    fn object_box(indices: &[u8], background: &[u8]) -> (usize, usize, usize, usize, usize, f64, f64) {
        let points: Vec<(usize, usize)> =
            (0..150 * 320).filter(|&i| indices[i] != background[i]).map(|i| (i % 320, i / 320)).collect();
        let xs: Vec<usize> = points.iter().map(|p| p.0).collect();
        let ys: Vec<usize> = points.iter().map(|p| p.1).collect();
        let count = points.len();
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
        let run = run();
        for index in [1499, 2305, 3004, 3404] {
            assert_eq!(run.part(index), "glenz");
            assert_eq!((run.frame(index).width, run.frame(index).height), (320, 200), "{index}");
        }
        // `second2b` is one row ahead at 1499 (5:12) and 2306 (8:20): A36's six-tick wait
        // offset, seen on the published position.
        assert_eq!(run.position(1499), "5:11");
        assert_eq!(run.position(2306), "8:19");
    }

    /// `L_p = 1567` two ways (A36): main frame 44's first object pixels at 1611, and the first
    /// faded surface at 2269 (frame 702, b = 63). The reset is 1117, six ticks after
    /// `second2b`'s 1111.
    #[test]
    fn the_loop_start_is_1567() {
        let run = run();
        assert_eq!(differing(run.rgb(1116), run.rgb(1115), 3), 0, "1115/1116 are the held picture");
        assert_eq!(differing(run.rgb(1117), run.rgb(1116), 3), 640, "1117 is the zoomer's first body (rows 0 and 399)");
        assert_eq!(above_floor_object(run.rgb(1610)).0, 0, "frame 43 draws no object pixel");
        assert_eq!(above_floor_object(run.rgb(1611)), (8, (172, 179, 0, 0)), "frame 44 at 1611");
        assert_eq!(above_floor_object(run.rgb(1612)), (21, (166, 179, 0, 1)), "frame 45 at 1612");

        // The port uploads `pal` before stepping, so frame 702 at 2269 shows b = 63 and frame
        // 701 at 2268 still b = 64.
        let palette = picture_palette();
        for (boot, b) in [(2268, 64), (2269, 63)] {
            let (rgb, indices, expected) = (run.rgb(boot), run.idx(boot), faded(&palette, b));
            let mismatched = (0..320 * 200)
                .filter(|&i| indices[i] < 16)
                .filter(|&i| rgb[i * 3..i * 3 + 3] != widened(expected[indices[i] as usize]))
                .count();
            assert_eq!(mismatched, 0, "boot {boot}: every low pixel is b = {b}");
        }
    }

    #[test]
    fn the_moving_keyframes_record_the_registered_alignments_and_residuals() {
        let run = run();
        let filed = |index: u32| reference("glenz", index, "rgb");
        let unexplained = |port: u32, at: u32| unexplained_pixels(run.rgb(port), &filed(at), run.idx(port));

        // A36: the plan's `K + 5` at 3000/3400.
        for (port, at, expected) in [(3005, 3000, 3114), (3405, 3400, 5261)] {
            assert_eq!(differing(run.rgb(port), &filed(at), 3), expected, "K+5 at {at}");
        }

        // A37: the registered surfaces are the unique minima of the index-structure metric
        // against their neighbours one frame each side.
        for (at, [before, minimum, after]) in [
            (2300, [(2304, 1859), (2305, 0), (2306, 1740)]),
            (3000, [(3003, 4486), (3004, 711), (3005, 2837)]),
            (3400, [(3403, 5876), (3404, 839), (3405, 4780)]),
        ] {
            for (port, count) in [before, minimum, after] {
                assert_eq!(unexplained(port, at).0, count, "unexplained at {port} against {at}");
            }
        }

        // At 2305 (main frame 738) every differing pixel is a low-palette entry: the port pairs
        // the frame with `pal` after 737 (b = 27), the capture with the value computed at 738
        // (b = 26).
        let (rgb_2305, reference_2300, idx_2305) = (run.rgb(2305), filed(2300), run.idx(2305));
        assert_eq!(differing_by_palette(rgb_2305, &reference_2300, idx_2305), (5208, 0));
        let palette = picture_palette();
        let (port_b27, capture_b26) = (faded(&palette, 27), faded(&palette, 26));
        for i in (0..320 * 200).filter(|&i| idx_2305[i] < 16) {
            let entry = idx_2305[i] as usize;
            let (port, capture) = (&rgb_2305[i * 3..i * 3 + 3], &reference_2300[i * 3..i * 3 + 3]);
            assert_eq!(port, widened(port_b27[entry]), "the port's frame 738 renders b = 27");
            if capture != port {
                assert_eq!(capture, widened(capture_b26[entry]), "the capture's 2300 renders b = 26");
            }
        }

        // The object boxes against the 1499 background: 2305's is the filed 2300's; 2306's is
        // the low-byte model's row in `glenz.md` §11.2.
        let background = run.idx(1499);
        let (x0, x1, y0, y1, count, cx, cy) = object_box(idx_2305, background);
        assert_eq!((x0, x1, y0, y1, count), (97, 217, 38, 149, 9989));
        assert!((cx - 156.8).abs() < 0.05 && (cy - 94.1).abs() < 0.05);
        let (x0, x1, y0, y1, count, cx, cy) = object_box(run.idx(2306), background);
        assert_eq!((x0, x1, y0, y1, count), (99, 217, 37, 149, 9927));
        assert!((cx - 157.6).abs() < 0.05 && (cy - 94.2).abs() < 0.05);

        // The surface minima's residual is the second object's interior XOR boundaries: the
        // (low, high) unexplained of the object total, and its red silhouette (port,
        // reference, differing).
        for (port, at, split, red) in [
            (3004, 3000, (710, 1, 35_896), (27_057, 27_143, 118)),
            (3404, 3400, (839, 0, 43_938), (33_312, 33_416, 180)),
        ] {
            let (_, low, high, object) = unexplained(port, at);
            assert_eq!((low, high, object), split, "A37's unexplained split at {at}");
            assert_eq!(red_mask(&filed(at), run.idx(port)), red, "A37's red masks at {at}");
        }
    }
}

/// MNTSCRL's tick 0 draws at boot 419 (`mntscrl.md` §2), and `--block 3` begins there, so chain
/// tick `t` names the reference's boot `419 + t`.
const MNTSCRL_CHAIN_ORIGIN: u32 = 419;

#[test]
fn a_block_three_run_starts_at_mntscrl_with_its_own_numbering() {
    // `SECOND 3`'s entry point is MNTSCRL (`MAIN/U2.ASM:865-866`, guarded `& 4`) under MUSIC1 at
    // order 42 (`:856-863`). Its first graphics frame is a measured window, not a frame
    // (`reference-indices.md` §11.5), so the run numbers its own frames.
    let run = dump(&["--block", "3"], &[81, 1100]);
    assert_eq!(run.manifest["part"], "mntscrl");
    assert_eq!(run.manifest["index_origin"], "part");
    assert_eq!(run.manifest["anchor"], serde_json::Value::Null);
    assert_eq!(run.manifest["music"]["module"], "MUSIC1");
    assert_eq!(run.manifest["music"]["start_order"], 42);
    assert_eq!(run.manifest["music"]["start_frame"], 0, "part-numbered, so the music starts with it");
    assert_eq!(run.part(1100), "mntscrl");
    // The block's entry path draws the `--part` run's pixels: its tick 81 is the reference's 500.
    assert_eq!(differing(run.idx(81), &reference("mntscrl", 500, "idx"), 1), 0, "block 3's tick 81 is the reference's 500");
}

#[test]
fn a_block_three_run_is_the_whole_chain_and_its_mid_chain_entries_do_not_pre_roll() {
    // A part's filed keyframe at boot `K` is chain tick `K - 419`:
    //
    //   part        keyframe  chain   the note's mapping
    //   MNTSCRL        1,100    681   boot =   419 + tick  (mntscrl.md §2)
    //   LNS&ZOOM       2,700  2,281   boot = 2,452 + tick  (lnszoom.md §7.1)
    //   PLZPART        6,000  5,581   boot = 5,723 + tick  (plzpart.md §2; plasma)
    //   PLZPART        8,500  8,081   boot = 5,723 + tick  (the vect/cube screen)
    //   MINVBALL      10,900 10,481   boot = 10,368 + tick (minvball.md §3)
    //   RAYSCRL       13,000 12,581   boot = 12,962 + tick (rayscrl.md §2)
    //   3DSINFLD      15,500 15,081   boot = 15,036 + tick (3dsinfld.md §6, derived)
    //   JPLOGO        17,300 16,881   boot = 17,065 + tick (jplogo.md §3)
    //
    // PLZPART 8,082, MINVBALL 10,482 and JPLOGO 16,884 are asked for one to three ticks later:
    // the parts hand over with no loader gap, so the chain's player is a frame or two off the
    // reference's boot counter, and the shifted index is the first whose position the trace
    // holds at `index + 419`.
    //
    // **The seam.** MINVBALL's and RAYSCRL's `init` pre-rolls (256 and 268 polls) are for
    // `--part` runs; reached mid-chain they must not fire.
    let run = dump(
        &["--block", "3"],
        &[
            681, 2281, 5581, 8082, 9942, 9943, 9944, 9946, 10482, 12537, 12546, 12581, 15081, 16884,
            17417, 17418, 17431, 17440, 17581, 17600, 17721,
        ],
    );
    // The manifest's music is the player last installed: the U2E boundary's restart
    // (`MAIN/U2.ASM:880-885`) replaces the entry's MUSIC1@42 with MUSIC0@18.
    assert_eq!(run.manifest["part"], "mntscrl");
    assert_eq!(run.manifest["index_origin"], "part");
    assert_eq!(run.manifest["anchor"], serde_json::Value::Null);
    assert_eq!(run.manifest["music"]["module"], "MUSIC0");
    assert_eq!(run.manifest["music"]["start_order"], 18);
    assert_eq!(run.manifest["music"]["start_frame"], 17_418, "the boundary the restart was installed at");

    // Every part of block 3's plan, at the chain index above, in its note's geometry. JPLOGO
    // hands over at 17,417/17,418, nine ticks before D34's reference-derived chain 17,427.
    assert_listing(
        &run,
        &[
            (681, "mntscrl", 320, 200),
            (2281, "lnszoom", 320, 200),
            (5581, "plzpart", 320, 400),
            (8082, "plzpart", 320, 200),
            (9942, "plzpart", 320, 200),
            (9943, "minvball", 320, 200),
            (9944, "minvball", 320, 200),
            (9946, "minvball", 320, 200),
            (10482, "minvball", 320, 200),
            (12537, "rayscrl", 320, 200),
            (12546, "rayscrl", 320, 200),
            (12581, "rayscrl", 320, 200),
            (15081, "3dsinfld", 320, 200),
            (16884, "jplogo", 320, 400),
            (17417, "jplogo", 320, 400),
            (17418, "u2e", 320, 200),
            (17431, "u2e", 320, 200),
            (17440, "u2e", 320, 200),
            (17581, "u2e", 320, 200),
            (17600, "u2e", 320, 200),
            (17721, "u2e", 320, 200),
        ],
    );

    // Each mapped index publishes the trace's position at the boot frame it names.
    for index in [681, 2281, 5581, 8082, 9944, 10482, 12537, 12581, 15081, 16884] {
        let boot = index + MNTSCRL_CHAIN_ORIGIN;
        assert_eq!(run.position(index), first_covering(BLOCK3_TRACES, boot), "chain frame {index} (boot {boot})");
    }

    // **MINVBALL's mid-chain entry.** The player ran on through PLZPART: its last frame and
    // MINVBALL's first both publish 75:60, and MINVBALL reaches the reference's start position
    // (boot 10,368's 75:61) three ticks in, its 130-tick fade covering the gapless handoff.
    assert_eq!(run.position(9942), "75:60", "PLZPART's last frame");
    assert_eq!(run.position(9943), "75:60", "MINVBALL's first frame");
    assert_eq!(first_covering(BLOCK3_TRACES, 10_368), "75:61");
    assert_eq!(run.position(9946), "75:61", "MINVBALL reaches the trace's start position");

    // **RAYSCRL's mid-chain entry.** Inside its init gate at 12,537 the player is at 86:62,
    // the trace's six frames before the reference's start (boot 12,956), and the part's own
    // `musplus >= 0` gate (seven confirming reads) opens at 12,546 on 88:0, boot 12,962's. A
    // 268-poll pre-roll on top of 86:62 could not publish 86:62 at the gate.
    assert_eq!(run.position(12537), "86:62", "the seek order, not past it");
    assert_eq!(first_covering(BLOCK3_TRACES, 12_962), "88:0");
    assert_eq!(run.position(12546), "88:0", "RAYSCRL's gate opens on the trace's start position");

    // **The mid-run restart at the U2E boundary** (`MAIN/U2.ASM:880-885`, D34). The fresh
    // player is sought to order 18 at 17,418, waits D34's four ticks and publishes nothing while
    // its ten-deep ring primes: 0:0 at the boundary and at D34's first-tick chain index 17,431,
    // and its first row 18:1 at 17,440.
    assert_eq!(run.position(17_418), "0:0", "the restarted player's ring primes");
    assert_eq!(run.position(17_431), "0:0", "still priming at D34's first tick");
    assert_eq!(run.position(17_440), "18:1", "the restarted session's first row");

    // **Against `second3e`**, `second4`'s session shifted by 17,441–17,442 frames (D34). The
    // port's nine-tick lead is under one MUSIC0 row (8.76 frames), so the order is exact and
    // the row within one (D25: `u2e.md` §5 quotes these rows).
    for (index, port, traced) in [(17_581, "18:17", "18:16"), (17_600, "18:19", "18:18"), (17_721, "19:1", "19:0")] {
        let boot = index + MNTSCRL_CHAIN_ORIGIN;
        assert_eq!(run.position(index), port, "chain frame {index}: the port's measured row");
        assert_eq!(first_covering(BLOCK3_TRACES, boot), traced, "the trace at boot {boot}");
    }
}

/// U2E through block 4, numbered from the derived 418 anchor. The filed keyframes show the
/// stream state the port shows four ticks earlier (A31), so at `K` the port differs by a whole
/// state; at `K - 4` the silhouettes are exact and only the Gouraud shading differs (rediff's
/// `u2e` entry, A30; `u2e.md` §5).
mod u2e {
    use super::*;

    const KEYFRAMES: [u32; 4] = [1000, 1600, 2600, 3600];

    /// The gate at 698/699 and, for each keyframe `K`: `K - 4` (the reference's own stream
    /// state), its hold `K - 3`, and `K - 1`, `K`, `K + 1`.
    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![698, 699];
        frames.extend(KEYFRAMES.iter().flat_map(|k| [k - 4, k - 3, k - 1, *k, k + 1]));
        dump(&["--block", "4"], &frames)
    }

    #[test]
    fn the_run_is_numbered_from_the_intro_anchor_and_plays_the_block_four_music() {
        let run = run();
        assert_eq!(run.manifest["part"], "u2e");
        assert_eq!(run.manifest["index_origin"], "boot");
        assert_eq!(run.manifest["anchor"], 418, "the fadeset at 467 minus 49 waits");
        // `MAIN/U2.ASM:880-885`: block 4 restarts MUSIC0 at order 18, whose clock starts at
        // 408; the run begins at 418 and pre-polls ten frames.
        assert_eq!(run.manifest["music"]["module"], "MUSIC0");
        assert_eq!(run.manifest["music"]["start_order"], 18);
        assert_eq!(run.manifest["music"]["start_frame"], 418);
        assert_listing(&run, &KEYFRAMES.map(|index| (index, "u2e", 320, 200)));
        // The trace's own position, except at 1600, where the row's 8.76-frame boundary falls
        // within a frame of the index (D17's class).
        for index in [1000, 2600, 3600] {
            assert_eq!(run.position(index), trace_position("second4", index), "{index}");
        }
        assert_eq!(run.position(1600), "20:39");
        assert_eq!(trace_position("second4", 1600), "20:38");
    }

    #[test]
    fn the_block_four_clock_reaches_order_19_at_698() {
        // One frame ahead of `second4` at the gate: the trace reads 18:31 at 698.
        let run = run();
        assert_eq!((run.position(698), run.position(699)), ("19:0", "19:0"));
        assert_eq!(trace_position("second4", 698), "18:31");
        assert_eq!(trace_position("second4", 699), "19:0");
    }

    #[test]
    fn the_keyframes_record_the_measured_session_phase_and_gouraud_residual() {
        let run = run();
        // U2E clears every page with `vid_clear255()` and patches entry 255 black, so
        // background is exactly (0,0,0).
        let silhouette_differing = |rendered: &[u8], expected: &[u8]| {
            let background = |pixel: &[u8]| pixel == [0, 0, 0];
            rendered.chunks_exact(3).zip(expected.chunks_exact(3)).filter(|(a, b)| background(a) != background(b)).count()
        };
        // The divergence at `K` itself: the previous stream state (A31).
        for (index, at_k) in [(1000, 11006), (1600, 11805), (2600, 17278), (3600, 16234)] {
            let filed = reference("u2e", index, "rgb");
            assert_eq!(differing(run.rgb(index), &filed, 3), at_k, "boot {index} at K");
            assert_eq!(silhouette_differing(run.rgb(index - 4), &filed), 0, "boot {index}: the aligned silhouette");
            // Each state holds two ticks: K-4 through K-3 and K through K+1, while K-1 is the
            // previous state.
            assert_eq!(run.rgb(index - 4), run.rgb(index - 3), "boot {index}: K-4 holds through K-3");
            assert_eq!(run.rgb(index), run.rgb(index + 1), "boot {index}: K holds through K+1");
            assert_ne!(run.rgb(index - 1), run.rgb(index), "boot {index}: K-1 is the previous state");
        }
    }
}

/// CRED's screens 1, 6, 12, 18 and 21 (`reference-indices.md` §20.1). CRED's tick 0 is boot
/// 1212 (`cred.md` §2), so screen 1's open loop is 1214-1315, its hold 1316-1515 and its close
/// 1516-1574; a screen is 363 ticks.
const CRED_KEYFRAMES: [u32; 5] = [1440, 3240, 5440, 7640, 8740];

/// Block 5: ENDLOGO's keyframes, the two hand-overs, CRED's holds and first wipes, and ENDSCRL's
/// first ticks and keyframe.
fn block5() -> std::sync::Arc<Dump> {
    let mut frames = vec![404, 430, 500, 900, 1180, 1211, 1212];
    frames.extend([1213, 1214, 1215, 1220, 1230, 1315, 1316, 1525, 1526]);
    frames.extend(CRED_KEYFRAMES.iter().flat_map(|k| [k - 1, *k, k + 1]));
    frames.extend([8834, 8835, 8836, 8837, 8838, 9100]);
    dump(&["--block", "5"], &frames)
}

#[test]
fn a_block_five_run_plays_music_one_at_the_order_its_entry_point_names() {
    // `SECOND 5`'s restart is `restartmus(0, 25, ...)` (`MAIN/U2.ASM:898-905`): MUSIC0 at
    // order 25, its clock from 408. Every published position is `second5`'s at the same index,
    // including the anchor 404, four frames before the clock starts, where the seek has
    // published nothing and the position is the ring's priming 0:0.
    let run = block5();
    assert_eq!(run.manifest["music"]["module"], "MUSIC0");
    assert_eq!(run.manifest["music"]["start_order"], 25);
    assert_eq!(run.manifest["music"]["start_frame"], 408);
    for index in [404, 430, 500, 900, 1180] {
        assert_eq!(run.position(index), trace_position("second5", index), "frame {index}");
    }
    assert_eq!(run.position(404), "0:0");
}

#[test]
fn a_block_run_plays_its_parts_in_order_on_one_reference_numbering() {
    // ENDLOGO, CRED (`MAIN/U2.ASM:910-917`), ENDSCRL, on one counter: a block run ticks one
    // index per tick, so both sides of each hand-over are produced only if they are adjacent.
    let run = block5();
    assert_eq!(run.manifest["index_origin"], "boot");
    assert_eq!(run.manifest["anchor"], 404);
    assert_eq!(run.manifest["part"], "endlogo", "the run's first frame is ENDLOGO's");
    assert_listing(
        &run,
        &[(1211, "endlogo", 320, 400), (1212, "cred", 320, 200), (8834, "cred", 320, 200), (8835, "endscrl", 320, 200)],
    );
}

#[test]
fn the_five_keyframes_dump_in_the_references_own_numbering() {
    let run = block5();
    assert_listing(&run, &CRED_KEYFRAMES.map(|index| (index, "cred", 320, 200)));
}

#[test]
fn the_five_keyframes_are_inside_the_chains_own_holds() {
    // The port's holds, at 1316 + 363·k, contain all five, screen 21's ending 35 frames after
    // its keyframe (`cred.md` §7).
    let run = block5();
    for key in CRED_KEYFRAMES {
        assert_eq!(run.rgb(key - 1), run.rgb(key), "{key} moves from {}", key - 1);
        assert_eq!(run.rgb(key + 1), run.rgb(key), "{key} moves to {}", key + 1);
    }
}

#[test]
fn the_wipes_move_the_display_between_the_holds() {
    // What the reference cannot check, its keyframes all being held: the registers each step
    // writes reach the dumped frame.
    let run = block5();
    // Ticks 1-3 are blank: the start is still in the even logical line 400, the picture in the
    // odd lines.
    assert_eq!(run.rgb(1213), run.rgb(1214));
    assert_eq!(run.rgb(1214), run.rgb(1215));
    assert_ne!(run.rgb(1220), run.rgb(1215), "open step 6");
    assert_ne!(run.rgb(1230), run.rgb(1220), "open step 16");
    assert_eq!(run.rgb(1315), run.rgb(1316), "the open loop's last step writes the hold's state");
    assert_ne!(run.rgb(1526), run.rgb(1525), "close steps 9 and 10");
}

#[test]
fn the_chain_hands_the_scroller_its_tick_zero_at_the_index_the_note_records() {
    // The port's CRED ends at 8834 (`cred.md` §7), so ENDSCRL's tick 0 is 8835; its view's
    // geometry follows from the tick after the mode set. 9100 is the first of
    // `reference-indices.md` §21.2's keyframes.
    let run = block5();
    assert_listing(
        &run,
        &[
            (8834, "cred", 320, 200),
            (8835, "endscrl", 320, 200),
            (8836, "endscrl", 320, 200),
            (8837, "endscrl", 640, 400),
            (8838, "endscrl", 640, 400),
            (9100, "endscrl", 640, 400),
        ],
    );
}

#[test]
fn a_block_u_run_starts_at_ddstars_with_its_own_numbering() {
    // `SECOND U` is DDSTARS alone (`MAIN/U2.ASM:889-896`) under MUSIC0 at order 70; DDSTARS's
    // mapping is derived from its keyframes, not a first drawn frame, so the run is
    // part-numbered.
    let run = dump(&["--block", "U"], &[1900]);
    assert_eq!(run.manifest["part"], "ddstars");
    assert_eq!(run.manifest["index_origin"], "part");
    assert_eq!(run.manifest["anchor"], serde_json::Value::Null);
    assert_eq!(run.manifest["music"]["module"], "MUSIC0");
    assert_eq!(run.manifest["music"]["start_order"], 70);
    assert_eq!(run.manifest["music"]["start_frame"], 0);
    assert_eq!(run.part(1900), "ddstars");
}
