//! U2A through the real binary: the block-1 numbering, the pre-gate hold,
//! the saucer frame and the cleared hold.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/u2a.rs`: the
//! merged palette and the pre-gate hold, the two gates, the first stream frame
//! and frame 16's toggle, the END's tick and the teardown, and the measured
//! stream state behind the reference's 7300. What is left over here is the
//! wiring only a process can answer: the registry name, the asset paths as a
//! run resolves them from the repository root, the shell's **404 anchor** and
//! its block-1 music clock, and the frames the shell composes.
//!
//! **The anchor is 404.** U2A is block 1's second part (`MAIN/U2.ASM:793-799`)
//! and the run begins at the boot's first graphics frame, 404 — ALKU's own
//! anchor, which a `--block 1` run inherits (`reference-indices.md` §1.12).
//! U2A itself has no anchor: a run that *starts* at it would draw from a
//! frame the evidence does not name, which is why this test runs the block.
//!
//! **The reference's three keyframes, and what the port's run shows.** 6500
//! is the 30,000-cycle reference's neon pre-gate frame, which the port does not show (A32):
//! it differs in all 64,000 pixels, while the port's hold compares at 0 against the four
//! `reference/parts/u2a-speed` captures. 7640
//! is the cleared hold after U2A's END; the screen is U2A's, but the part
//! running is PAM, which is in its own sync gate and leaves the predecessor's
//! picture up, so the dump names the frame `pam_007640`. It compares at 0,
//! indices included. **7300 diverges, and the divergence is recorded rather
//! than fitted away**: the port's schedule displays a later page of the same
//! animation, while the reference's picture is stream state **294** (the unit
//! test `the_reference_7300_is_the_measured_stream_state` searches every
//! state and pins it), so the pose itself differs — 1098 pixels, not the 426
//! the reference's own state leaves (that residual is on the Gouraud-shaded
//! faces; `docs/fidelity/u2a.md` §5 carries the diagnosis). Both divergences
//! are registered under D17/D24 as **A28** (the Gouraud residual) and
//! **A29** (this session-phase offset) in `docs/fidelity/APPROXIMATIONS.md`;
//! neither is fitted, and this test records the measured numbers.

use std::path::{Path, PathBuf};
use std::process::Command;

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// `assets/VISU/` paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A headless block-1 run dumping the frames asked for, and the manifest it
/// wrote.
///
/// The run must exit 0: every requested index is produced, and a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str, indices: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-u2a-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args([
            "--block",
            "1",
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

/// The reference's own file for a U2A frame, from the tree it is committed in.
fn reference_frame(index: u32, suffix: &str) -> Vec<u8> {
    std::fs::read(repository_root().join(format!(
        "reference/parts/u2a/frame_{index:06}.{suffix}"
    )))
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

/// One frame of a filed trace: the published position and zframe at a
/// reference index.
fn trace_row(name: &str, frame: u32) -> (u8, u8, i64) {
    let path = repository_root()
        .join("reference/music")
        .join(name)
        .join("trace.csv");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    for line in text.lines().skip(1) {
        let fields: Vec<&str> = line.split(',').collect();
        if fields[0].trim().parse::<u32>().unwrap() == frame {
            return (
                fields[1].trim().parse().unwrap(),
                fields[2].trim().parse().unwrap(),
                fields[9].trim().parse().unwrap(),
            );
        }
    }
    panic!("{name} holds no frame {frame}")
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

/// 6500 is held (the pre-gate hold), 7300 moves, and 7640 is held; the dump carries
/// the moving keyframe's neighbours so the port's own page phase is visible.
const NEON_6500: usize = 64_000;
const FRAMES: &str = "6500,7299,7300,7301,7640";

#[test]
fn the_run_is_numbered_in_boot_frames_and_reaches_u2a_after_alku() {
    let (dir, manifest) = dump("numbering", FRAMES);
    assert_eq!(manifest["part"], "u2a");
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404, "block 1 draws from the boot's 404");
    assert_eq!(manifest["width"], 320);
    assert_eq!(manifest["height"], 200);
    // `MAIN/U2.ASM:766-775`: block 1 takes the initial `_zplaysong(0)`, so the
    // module is MUSIC0 at order 0, and the run pre-polls 400..403.
    assert_eq!(manifest["music"]["module"], "MUSIC0");
    assert_eq!(manifest["music"]["start_order"], 0);
    assert_eq!(manifest["music"]["start_frame"], 404);

    // Each dumped frame is named for the part whose screen it is: U2A's own
    // until its END, then PAM holding U2A's cleared page through its gate.
    assert_eq!(entry(&manifest, 6500)["part"], "u2a");
    assert_eq!(entry(&manifest, 7300)["part"], "u2a");
    assert_eq!(entry(&manifest, 7640)["part"], "pam");
    assert_eq!(
        entry(&manifest, 7640)["rgb"].as_str().unwrap(),
        "pam_007640.rgb",
        "the hold is named for the part that is running, not the one that drew it"
    );

    // The music is the trace's own at the two indices it holds: the gate's
    // row and the saucer's. (The 7300 divergence below is the animation
    // schedule's, not a music-position one.)
    let trace = trace_position("second1c", 6500);
    assert_eq!(entry(&manifest, 6500)["music_position"], "10:55");
    assert_eq!(trace, (10, 55));
    let trace = trace_position("second1c", 7300);
    assert_eq!(entry(&manifest, 7300)["music_position"], "12:18");
    assert_eq!(trace, (12, 18));

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_block_one_clock_matches_second1_at_boot_5900() {
    // A29's mechanism: the port's block-1 clock follows `second1`, not
    // `second1c` (the trace that supplies the 6988 gate measurement). At
    // boot 5900 `second1` holds 9:51 with zframe 3771; `second1c` holds
    // 9:50 with zframe 3768. The 7300 session-phase offset follows from the
    // clock the port is actually faithful to.
    let (dir, manifest) = dump("phase", "5900");
    assert_eq!(entry(&manifest, 5900)["music_position"], "9:51");
    let (order, row, zframe) = trace_row("second1", 5900);
    assert_eq!((order, row), (9, 51), "second1 at boot 5900");
    assert_eq!(zframe, 3771, "second1's zframe at boot 5900");
    let (order, row, zframe) = trace_row("second1c", 5900);
    assert_eq!(
        (order, row, zframe),
        (9, 50, 3768),
        "second1c is the other session, four frames behind"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_pre_gate_hold_compares_at_zero_with_the_four_speed_captures() {
    let (dir, manifest) = dump("pad", FRAMES);
    let rendered = dumped_file(&dir, &manifest, 6500, "rgb");
    for speed in ["c15000", "c25000", "c35000", "c60000"] {
        for index in [6400, 6600] {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../reference/parts/u2a-speed")
                .join(speed)
                .join(format!("frame_{index:06}.rgb"));
            let expected = std::fs::read(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let count = differing(&rendered, &expected, 3);
            println!("{speed} boot {index}: {count}/64000 px differ");
            assert_eq!(count, 0, "{speed} {index}");
        }
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_30000_cycle_neon_6500_differs_by_the_registered_count() {
    let (dir, manifest) = dump("neon", FRAMES);
    let rendered = dumped_file(&dir, &manifest, 6500, "rgb");
    let expected = reference_frame(6500, "rgb");
    let count = differing(&rendered, &expected, 3);
    println!("boot 6500: {count}/64000 px differ");
    assert_eq!(count, NEON_6500, "A32's count");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_cleared_hold_reference_7640_compares_at_zero_in_both_planes() {
    let (dir, manifest) = dump("hold", FRAMES);
    // The screen is U2A's cleared page; PAM is the part running (its gate is
    // still closed), so the manifest's names are PAM's.
    let rendered = dumped_file(&dir, &manifest, 7640, "idx");
    let expected = reference_frame(7640, "idx");
    assert_eq!(differing(&rendered, &expected, 1), 0, "the hold's indices");
    let rendered = dumped_file(&dir, &manifest, 7640, "rgb");
    let expected = reference_frame(7640, "rgb");
    let count = differing(&rendered, &expected, 3);
    println!("boot 7640: {count}/64000 px differ");
    assert_eq!(count, 0, "the hold's picture");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_saucer_frame_records_its_measured_divergence() {
    let (dir, manifest) = dump("saucer", FRAMES);
    let rendered = dumped_file(&dir, &manifest, 7300, "idx");
    let expected = reference_frame(7300, "idx");
    let count = differing(&rendered, &expected, 1);
    println!("boot 7300: {count}/64000 indices differ");
    // The measured divergence (`docs/fidelity/u2a.md` §5), registered as
    // A29 in `docs/fidelity/APPROXIMATIONS.md`: the port's schedule displays
    // stream state 297 here and the reference's picture is state 294, so the
    // pose differs. The reference's own state renders 426 pixels differently
    // (the unit test pins it); the 297-vs-294 pose is 1098. This is not
    // fitted: the assertion records what the faithful port draws (D17/D24).
    assert_eq!(count, 1098, "the measured 7300 divergence (A29)");
    let rendered = dumped_file(&dir, &manifest, 7300, "rgb");
    let expected = reference_frame(7300, "rgb");
    assert_eq!(differing(&rendered, &expected, 3), 1098, "the RGB agrees");

    // The port's own page phase: one page holds this pose across 7299-7301.
    // (The reference's partner session had 7299 differ and 7301 hold; the
    // neighbour phase is the session's, `reference-indices.md` §5.3.)
    let at_7300 = frame_bytes(&dir, &manifest, 7300);
    assert_eq!(frame_bytes(&dir, &manifest, 7299), at_7300, "7299 holds");
    assert_eq!(frame_bytes(&dir, &manifest, 7301), at_7300, "7301 holds");

    let _ = std::fs::remove_dir_all(&dir);
}

fn frame_bytes(dir: &Path, manifest: &serde_json::Value, index: u32) -> Vec<u8> {
    dumped_file(dir, manifest, index, "idx")
}
