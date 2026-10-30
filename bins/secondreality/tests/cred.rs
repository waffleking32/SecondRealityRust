//! CRED through the real binary: the five reference keyframes, dumped from the
//! repository root, where the part's own asset paths resolve.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/cred.rs`. What
//! is left over here is the wiring only a process can answer — the registry,
//! the block-5 chain's numbering, the assets read at the paths a run really
//! uses — and the comparison with `reference/parts/cred` broken down by region:
//! all five keyframes are the reference's, byte for byte, now that the screens
//! are read from the shipped `CRED.EXE`.
//!
//! **The chain is part of the test.** These indices are reached through
//! ENDLOGO: 1440 is only answerable if ENDLOGO's duration was right, and the
//! run says `--block 5` for that reason, not `--part cred`. `reference-indices`
//! §20.2 measured every keyframe as held, in every session, so the port's own
//! holds have to contain them — and they do, with 34 frames to spare at the
//! tightest (screen 21).

use std::path::{Path, PathBuf};
use std::process::Command;

/// The keyframes the reference holds for this part (`reference-indices.md`
/// §20.1): the holds of screens 1, 6, 12, 18 and 21.
const KEYFRAMES: &str = "1440,3240,5440,7640,8740";

/// Where a `cargo run` of the acceptance command starts, and where the part's
/// own asset paths resolve.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The block-5 chain's dump of the five keyframes, and the manifest it wrote.
///
/// The run must exit 0: every requested index is produced, and the frames are
/// the chain's. A missing keyframe is a failure with a code, because a hole in
/// a dump reads as a keyframe that matched.
fn dump(name: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-cred-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--block", "5", "--headless", "--dump-frames", KEYFRAMES, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

fn frame_file(dir: &Path, index: u32) -> Vec<u8> {
    std::fs::read(dir.join(format!("cred_{index:06}.rgb"))).unwrap()
}

/// The reference's own frame, from the tree it is committed in.
fn reference_frame(index: u32) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/cred/frame_{index:06}.rgb")),
    )
    .unwrap()
}

/// How many bytes of `rendered` differ from `expected` over `rows`, or over all
/// 200 rows when `rows` is `None`.
///
/// **The lengths are asserted first, and they have to be.** `iter().zip()`
/// stops at the shorter side, so a truncated dump — the one failure a
/// comparison test exists to catch — would compare a prefix and report zero
/// differences as if it had compared the frame.
fn differing_bytes(rendered: &[u8], expected: &[u8], rows: Option<(usize, usize)>) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    assert_eq!(rendered.len(), 320 * 200 * 3, "the display is 320x200");
    let (first, last) = rows.unwrap_or((0, 200));
    (first..last)
        .flat_map(|y| (0..320 * 3).map(move |at| y * 320 * 3 + at))
        .filter(|at| rendered[*at] != expected[*at])
        .count()
}

/// The keyframes and the frame either side of each, which is what "held" means
/// (`reference-indices.md` §1.4): the comparison the record made of the
/// reference, made of the port.
const WITH_SUPPORT: &str = "1439,1440,1441,3239,3240,3241,5439,5440,5441,\
                             7639,7640,7641,8739,8740,8741";

/// The picture band: displayed rows 0-99, which the view reads from the odd
/// logical lines the picture is drawn into.
const BAND: (usize, usize) = (0, 100);
/// The caption: displayed rows 100-199, below the split, read from the caption
/// page at origin 0.
const CAPTION: (usize, usize) = (100, 200);

#[test]
fn the_five_keyframes_dump_in_the_references_own_numbering() {
    let (dir, manifest) = dump("numbering");
    // A run of `SECOND 5` is anchored at the demo's first graphics frame — the
    // frame the block entry's mode 13h and `inittwk` produce
    // (`MAIN/U2.ASM:906-908`, `reference-indices.md` §1.12) — and the manifest
    // has to say its indices are in that space: a `part` numbering would make
    // every index below incomparable with `reference/`.
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    // The manifest names the part *of the first dumped frame*, not of the run:
    // the chain passes through ENDLOGO, and 1440 is CRED's.
    assert_eq!(manifest["part"], "cred");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [1440, 3240, 5440, 7640, 8740]);
    for frame in frames {
        assert_eq!(frame["width"], 320, "{}", frame["idx"]);
        assert_eq!(frame["height"], 200, "{}", frame["idx"]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_five_keyframes_are_the_references_byte_for_byte() {
    // Screens 1, 6, 12, 18 and 21, read from the shipped image, and the port's
    // frames are the reference's, byte for byte — 0 of 192,000 bytes each. That
    // is the whole machinery in one number a frame: the tweak mode's display
    // geometry, the palette repair, the ramp, the font's metrics, the caption,
    // the `+16` picture offset, and the split. Three of the five (3240, 5440,
    // 8740) are screens the release changed from the tree, which the port drew
    // from the tree's data until the image was read (A10, retired): the band
    // and the caption are asserted separately so a regression says which.
    let (dir, _) = dump("agreed");
    for index in [1440, 3240, 5440, 7640, 8740] {
        let dumped = frame_file(&dir, index);
        let expected = reference_frame(index);
        assert_eq!(
            differing_bytes(&dumped, &expected, Some(BAND)),
            0,
            "frame {index}: the picture band"
        );
        assert_eq!(
            differing_bytes(&dumped, &expected, Some(CAPTION)),
            0,
            "frame {index}: the caption"
        );
        assert_eq!(differing_bytes(&dumped, &expected, None), 0, "frame {index}");
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_five_keyframes_are_inside_the_chains_own_holds() {
    // **The numbering lands.** A keyframe is answerable only if the port's tick
    // for it falls inside that screen's hold, and the chain is what decides
    // where the ticks are: CRED's tick 0 is reference index **1212** —
    // `--dump-frames 1211,1212` writes `endlogo_001211.rgb` and
    // `cred_001212.rgb`, and the dump names the part that drew each frame — and
    // a screen is 363 ticks, so the five holds are at 1315-1514, 3130-3329,
    // 5308-5507, 7486-7685 and 8575-8774 — each containing its keyframe, the
    // tightest (screen 21) with 34 frames to spare.
    //
    // This is the port's own version of §20.2's measurement: three frames
    // around each keyframe, byte-identical. It says both things at once — the
    // keyframe is inside a hold, and the screen there is the one the reference
    // holds — without needing the reference at all.
    let dir = std::env::temp_dir().join(format!("sr-cred-{}-holds", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--block", "5", "--headless", "--dump-frames", WITH_SUPPORT, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    for key in [1440u32, 3240, 5440, 7640, 8740] {
        let before = frame_file(&dir, key - 1);
        let at = frame_file(&dir, key);
        let after = frame_file(&dir, key + 1);
        assert_eq!(before, at, "{key} moves between {0} and {key}", key - 1);
        assert_eq!(at, after, "{key} moves between {key} and {}", key + 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The indices this part's *motion* lives at, through the real binary: two
/// blank early frames, two mid-wipe steps that are not, the open loop's last
/// step beside the hold's first frame, and two close steps.
const WIPE: &str = "1213,1214,1215,1220,1230,1315,1316,1525,1526";

#[test]
fn the_wipes_move_the_display_between_the_holds() {
    // The part's whole visible motion, and the one thing `reference/parts/cred`
    // cannot check: all five of its keyframes are *held* screens
    // (`reference-indices.md` §20.2). CRED's tick 0 is reference index **1212**
    // (`--dump-frames 1211,1212` writes `endlogo_001211.rgb` and
    // `cred_001212.rgb`), so tick `t` of screen 1 is index `1212 + t`, and
    // `MAIN.C:176-213`'s three parts fall at
    //
    //   1214..1315   the open loop's 102 steps, one a tick (`:176-192`)
    //   1316..1515   the hold: `:194`'s 200 `dis_waitb`s, writing nothing
    //   1516..1574   the close loop's 59 steps (`:196-212`)
    //
    // with ticks 0 and 1 (1212 and 1213) the parked state and the tick the
    // palette and the picture land on. The state each step writes is
    // `src/parts/cred.rs`'s
    // `the_three_registers_move_once_a_tick_through_both_wipes`; what this test
    // adds is that the three registers reach the dumped frame — that the shell
    // composes what the part wrote — and the two one-frame continuities the
    // dump can see.
    let dir = std::env::temp_dir().join(format!("sr-cred-{}-wipe", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--block", "5", "--headless", "--dump-frames", WIPE, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));

    // The screen's first four frames are **blank** — ticks 0 to 3, indices
    // 1212 to 1215, and this dump holds the last three — and that is the
    // geometry, not a stalled part: the display start is still inside the
    // *even* logical line 400 (the picture is drawn into the odd lines,
    // `MAIN.C:174`) and the row's other half is the blank sliver of line 401
    // before the picture's column 80. The ink arrives at 1216 (tick 4, open
    // step 2): 550 non-black pixels, rows 193-199 — the caption's first line
    // coming in at the bottom right — and at 1220 there are 4,972.
    assert_eq!(frame_file(&dir, 1214), frame_file(&dir, 1215));
    assert_eq!(
        frame_file(&dir, 1213),
        frame_file(&dir, 1214),
        "the parked tick, the picture's tick and the first two open steps are all blank"
    );
    // And the wipe is drawn: consecutive steps of the same loop show different
    // frames.
    assert_eq!(
        differing_bytes(&frame_file(&dir, 1220), &frame_file(&dir, 1215), None),
        10_635,
        "open step 6 is not the blank frame of step 1"
    );
    assert_eq!(
        differing_bytes(&frame_file(&dir, 1230), &frame_file(&dir, 1220), None),
        29_659,
        "the open wipe is still moving at step 16"
    );
    // The open loop's **last** step (1315, tick 103) and the hold's first frame
    // (1316, tick 104) are the same frame, because the last step writes the
    // hold's state exactly and the hold writes nothing. What that shows is the
    // *seam*: the display the wipe ends on is the display every hold frame
    // shows. It does **not** pin the ramp's length — its last four `y` values
    // (4, 3, 2, 1) all give `yy = 320`, so a ramp ending a few steps early would
    // end on the same state. The length is pinned by the unit test
    // (`open.last() == 1` and 102 steps).
    assert_eq!(
        frame_file(&dir, 1315),
        frame_file(&dir, 1316),
        "the open loop's last step is not the hold's state"
    );
    // And the close loop moves the display back out: 1525 is its step 9 and
    // 1526 its step 10 — the state the record's own `frame_001540.rgb` shows.
    assert_eq!(
        differing_bytes(&frame_file(&dir, 1526), &frame_file(&dir, 1525), None),
        13_708,
        "the close wipe is not moving between steps 9 and 10"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// The guard above can fail, which is the point of it: a dump cut short is
/// exactly the case a bare `zip` would report as identical.
#[test]
#[should_panic(expected = "a short read is not a comparison")]
fn a_truncated_frame_is_refused_rather_than_counted() {
    differing_bytes(&[0; 6], &[0; 9], None);
}
