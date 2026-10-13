//! ENDLOGO through the real binary: the four reference keyframes, dumped from
//! the repository root, where the part's own asset path resolves.
//!
//! The part's arithmetic is unit-tested beside it in `src/parts/endlogo.rs`.
//! What is left over here is the wiring only a process can answer: the
//! registry, the shell's block anchor and numbering, the asset read at the
//! path a run really uses, and the DAC and surface the shell hands the part.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The part's own asset, by the same path `src/parts/endlogo.rs` reads it by.
const PICTURE: &str = "assets/END/PIC.UH";

/// The keyframes the reference holds for this part (`reference-indices.md`
/// §19.2): its opening white, its fade-in, its held picture and its fadeout.
const KEYFRAMES: &str = "430,500,900,1180";

/// Where a `cargo run` of the acceptance command starts, and where `PICTURE`
/// resolves: the part reads a path relative to the working directory.
fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A dump directory of this test's own, and the manifest the run wrote there.
///
/// The run must exit 0: every requested index is produced, and the frames are
/// the part's. A missing keyframe is a failure with a code, because a hole in a
/// dump reads as a keyframe that matched.
fn dump(name: &str) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-endlogo-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--headless", "--part", "endlogo", "--dump-frames", KEYFRAMES, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let manifest =
        serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    (dir, manifest)
}

fn frame_file(dir: &Path, index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(dir.join(format!("endlogo_{index:06}.{extension}"))).unwrap()
}

/// How many bytes of `rendered` differ from `expected`.
///
/// **The lengths are asserted first, and they have to be.** `iter().zip()`
/// stops at the shorter side, so a *truncated* dump — the one failure a
/// comparison test exists to catch — would compare a prefix, count zero
/// differences and pass, while the message printed `expected.len()` as if it
/// were what had been compared. A check that cannot fail is worth less than no
/// check, so the length is what makes this count a comparison.
fn differing_bytes(rendered: &[u8], expected: &[u8]) -> usize {
    assert_eq!(
        rendered.len(),
        expected.len(),
        "the frame is {} bytes where the reference's is {}; a short read is not a comparison",
        rendered.len(),
        expected.len()
    );
    rendered.iter().zip(expected).filter(|(a, b)| a != b).count()
}

/// The reference's own frame, read from the tree it is committed in.
///
/// `reference/` is input-only, and reading it from a test is established
/// practice here (`task 4`'s integration tests, and the part's own derivation
/// test): it is what lets the suite, and not only `xtask compare`, say that a
/// frame is the reference's.
fn reference_frame(index: u32, extension: &str) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/endlogo/frame_{index:06}.{extension}")),
    )
    .unwrap()
}

#[test]
fn the_four_keyframes_dump_in_the_references_own_numbering() {
    let (dir, manifest) = dump("numbering");
    // A run that starts at ENDLOGO is anchored at the demo's first graphics
    // frame — the frame `SECOND 5`'s mode 13h and `inittwk` produce, which is
    // the same 404 `SECOND 2` gives BEGLOGO (`MAIN/U2.ASM:906-908`,
    // `reference-indices.md` §1.12) — and the manifest has to say its indices
    // are that space: a `part` numbering would make every index below
    // incomparable with `reference/`.
    assert_eq!(manifest["index_origin"], "boot");
    assert_eq!(manifest["anchor"], 404);
    assert_eq!(manifest["part"], "endlogo");
    let frames = manifest["frames"].as_array().unwrap();
    let indices: Vec<u64> = frames.iter().map(|f| f["index"].as_u64().unwrap()).collect();
    assert_eq!(indices, [430, 500, 900, 1180]);
    for frame in frames {
        assert_eq!(frame["width"], 320, "{}", frame["idx"]);
        assert_eq!(frame["height"], 400, "{}", frame["idx"]);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_opening_white_is_index_zero_and_the_references_own_bytes() {
    // The part has no `memset`: what makes 430 white is `:62-63`'s 765 bytes of
    // 63 written into a DAC whose entries 0-254 were last the mode set's, over
    // video memory `inittwk` cleared to index 0. That is the whole frame: one
    // index, and the reference files its `.idx` for this frame, so `xtask
    // compare` takes the **indexed** path — a port that blitted the picture
    // before 430 would fail here while its RGB stayed white.
    let (dir, _) = dump("white");
    let indices = frame_file(&dir, 430, "idx");
    assert_eq!(indices.len(), 320 * 400);
    assert!(indices.iter().all(|index| *index == 0), "430 is not index 0 everywhere");
    assert_eq!(
        differing_bytes(&indices, &reference_frame(430, "idx")),
        0,
        "430's indices are not the reference's"
    );
    let rgb = frame_file(&dir, 430, "rgb");
    assert!(rgb.iter().all(|channel| *channel == 255), "430 is not white");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_two_fade_frames_are_the_references_own_bytes() {
    // 500 and 1180 are RGB-only in the reference (`reference-indices.md`
    // §19.3: an indexed read is 20+ fade steps late, so D15's coherence rule
    // refuses the pair), so this is the comparison `xtask compare` makes of
    // them — the DAC's 6→8-bit widening included, which is why the pair is
    // where a widening that differed would show. The part's own derivation test
    // matches the same two frames against the same bytes through its
    // arithmetic; what this adds is the whole binary: the registry, the anchor,
    // the asset read from the repository root, and the `.rgb` the dump wrote to
    // disk. The two are the fade *in* and the fade *out*, so a diff at one and
    // not the other separates a ramp error from a hold-length error.
    let (dir, _) = dump("fade");
    for index in [500, 1180] {
        let dumped = frame_file(&dir, index, "rgb");
        let expected = reference_frame(index, "rgb");
        assert_eq!(
            differing_bytes(&dumped, &expected),
            0,
            "frame {index} differs from the reference"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

/// The guard above can fail, which is the whole point of it: a dump cut short
/// is exactly the case a bare `zip` would report as identical.
#[test]
#[should_panic(expected = "a short read is not a comparison")]
fn a_truncated_frame_is_refused_rather_than_counted() {
    differing_bytes(&[1, 2, 3], &[1, 2, 3, 4]);
}

#[test]
fn the_held_frame_is_the_picture_the_part_read() {
    // 900 is inside the hold, so the screen is `END/PIC.UH` through the
    // palette the fade-in ended on — the picture's own. What is on it is the
    // container as `sr_assets` decodes it, which the part's derivation test
    // finds byte-identical to `reference/parts/endlogo/frame_000900.idx`; this
    // says the same of the frame the binary dumped. 900 is the one keyframe
    // `xtask compare` takes the indexed path on, so this is the comparison.
    let (dir, _) = dump("held");
    let image = sr_assets::load_path(&repository_root().join(PICTURE)).unwrap();
    let dumped = frame_file(&dir, 900, "idx");
    let differing = differing_bytes(&dumped, image.pixels());
    assert_eq!(
        differing, 0,
        "the held frame differs from the picture in {differing} of {} indices",
        dumped.len()
    );
    assert_eq!(differing_bytes(&dumped, &reference_frame(900, "idx")), 0);
    let _ = std::fs::remove_dir_all(&dir);
}
