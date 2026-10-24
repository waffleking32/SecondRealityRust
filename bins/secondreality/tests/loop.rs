//! Loop mode through the real binary: `--loop`'s seam between CRED and pass 2's ALKU,
//! compared with the original's own (`reference/parts/loop`, `reference-indices.md` §25).
//!
//! **Two numberings meet here, and both shifts are measured.**
//!
//! - The loop capture's recipe presses three more keys than the B1 recipe the port's
//!   `--block 5` anchor (404) is in, and its boot runs **115** frames later (§25.1:
//!   ENDLOGO's frame-locked fade-in agrees byte for byte at that offset).
//! - The port's CRED ends **81** frames before the original's, in B1 numbering: the closed
//!   iris — the seam's first frame, `C` — is the port's 8833 and the original's 9029 − 115
//!   = 8914. That is CRED's own chain drift (`cred.md` §7, `endscrl.md` §6.1 — measured
//!   there as 82 from ENDSCRL's start), not this task's, and it is pinned below by the
//!   port's frames themselves: 8832 is still the iris and 8833 is the filed `C`'s bytes.
//!
//! So the seam is compared **from `C`**: the port's frame at `C_port + (n − C)` answers the
//! original's `n`. Every filed frame is a held one (§25.4), and each compares at 0.
//!
//! The capture holds `.rgb` only (§25.4: neither screen's `.idx` survives the read), so the
//! comparison is RGB, byte for byte.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The loop recipe's boot runs this many frames later than the B1 recipe's (§25.1).
const LOOP_RECIPE_SHIFT: u32 = 115;

/// `C` in the loop capture's own numbering: CRED's closed screen (§25.3).
const C: u32 = 9029;

/// The port's `C`: its first closed-iris frame in `--block 5`'s numbering.
const PORT_C: u32 = 8833;

/// The port's CRED ends this much earlier than the original's (B1 numbering).
const CRED_CHAIN_OFFSET: u32 = C - LOOP_RECIPE_SHIFT - PORT_C;

/// `L` in the loop capture's numbering: ALKU's first frame on pass 2 (§25.3).
const L: u32 = 9104;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The port's index answering the loop capture's `index`.
fn port_index(index: u32) -> u32 {
    index - C + PORT_C
}

/// A headless dump of `--block 5` (with `extra` flags) at `indices`, and its manifest.
fn dump(name: &str, extra: &[&str], indices: &[u32]) -> (PathBuf, serde_json::Value) {
    let dir = std::env::temp_dir().join(format!("sr-loop-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let list = indices.iter().map(u32::to_string).collect::<Vec<_>>().join(",");
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--block", "5", "--headless"])
        .args(extra)
        .args(["--dump-frames", &list, "--dump-dir"])
        .arg(&dir)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
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
        .find(|frame| frame["index"] == index)
        .unwrap_or_else(|| panic!("the dump has no frame {index}"))
}

/// The dumped `.rgb` of `index`, and the part and geometry the manifest gives it.
fn rendered(dir: &Path, manifest: &serde_json::Value, index: u32) -> (Vec<u8>, String, u64, u64) {
    let frame = entry(manifest, index);
    let bytes = std::fs::read(dir.join(frame["rgb"].as_str().unwrap())).unwrap();
    (
        bytes,
        frame["part"].as_str().unwrap().to_string(),
        frame["width"].as_u64().unwrap(),
        frame["height"].as_u64().unwrap(),
    )
}

fn reference_frame(screen: &str, index: u32) -> Vec<u8> {
    std::fs::read(
        repository_root().join(format!("reference/parts/loop/{screen}/frame_{index:06}.rgb")),
    )
    .unwrap()
}

/// How many pixels differ. The lengths are asserted first: a short read is not a
/// comparison.
fn differing_pixels(rendered: &[u8], expected: &[u8]) -> usize {
    assert_eq!(rendered.len(), expected.len(), "a short read is not a comparison");
    rendered
        .chunks_exact(3)
        .zip(expected.chunks_exact(3))
        .filter(|(a, b)| a != b)
        .count()
}

/// The filed seam: (screen, index, width, height, part).
const SEAM: &[(&str, u32, u64, u64, &str)] = &[
    ("cred", 9029, 320, 200, "cred"),
    ("cred", 9055, 320, 200, "cred"),
    ("cred", 9081, 320, 200, "cred"),
    ("alku", 9104, 320, 400, "alku"),
    ("alku", 9600, 320, 400, "alku"),
    ("alku", 10419, 320, 400, "alku"),
    ("alku", 11419, 320, 400, "alku"),
];

#[test]
fn the_loop_seam_is_the_originals() {
    assert_eq!(CRED_CHAIN_OFFSET, 81, "CRED's chain drift, measured from C");
    let mut indices = vec![PORT_C - 1, PORT_C, port_index(L) - 1];
    indices.extend(SEAM.iter().map(|(_, index, ..)| port_index(*index)));
    // The restarted song's first published row, and ALKU's pass-2 fade-in (below).
    indices.extend(port_index(L)..=port_index(L) + 16);
    indices.extend(10_040..=10_047);
    indices.extend(10_100..=10_107);
    let (dir, manifest) = dump("seam", &["--loop"], &indices);

    // `C` is where the port's frames say it is: the iris is still closing at 8832, and
    // 8833 is the filed `C`'s own bytes.
    let c_bytes = reference_frame("cred", C);
    let (before, ..) = rendered(&dir, &manifest, PORT_C - 1);
    assert_ne!(differing_pixels(&before, &c_bytes), 0, "8832 is the iris's last step");
    let (at, ..) = rendered(&dir, &manifest, PORT_C);
    assert_eq!(differing_pixels(&at, &c_bytes), 0, "8833 is C");

    // `L − C` is the original's 75: the frame before `L` is still CRED's 320×200 screen
    // (the original's 9103, §25.3), and `L` is ALKU's first 320×400 frame.
    let (_, part, width, height) = rendered(&dir, &manifest, port_index(L) - 1);
    assert_eq!((part.as_str(), width, height), ("cred", 320, 200), "L − 1 is CRED's screen");
    assert_eq!(L - C, 75);

    let mut counts = Vec::new();
    for &(screen, index, width, height, part) in SEAM {
        let at = port_index(index);
        let (bytes, drawn_by, w, h) = rendered(&dir, &manifest, at);
        assert_eq!((w, h), (width, height), "{screen} {index}: the port's {at} geometry");
        assert_eq!(drawn_by, part, "{screen} {index}: the port's {at} is {part}'s");
        counts.push((index, differing_pixels(&bytes, &reference_frame(screen, index))));
    }
    assert_eq!(
        counts,
        SEAM.iter().map(|(_, index, ..)| (*index, 0)).collect::<Vec<_>>(),
        "every held frame of the seam is the original's"
    );

    // **Measured, not fitted (D24): pass 2's music runs 3 to 5 frames later than the
    // original's against `C`.** The seam trace has the restarted song at (0,1) at 9112
    // (§25.3), port 8916; the port publishes it at 8921 — `MusicStart::restart`'s four
    // waiting polls from the fade's end. ALKU's pass-2 fade-in follows the music: the
    // original's first non-black frame is in (10236, 10238] and its "A Future Crew
    // Production" hold starts in (10296, 10298] (two-frame sampling, §25.3), port
    // (10040, 10042] and (10100, 10102]; the port's are 10045 and 10106. The held frames
    // above sit inside their holds either way. Task 10 registers the difference.
    let first_row = (port_index(L)..=port_index(L) + 16)
        .find(|index| entry(&manifest, *index)["music_position"] == "0:1")
        .expect("the restarted song publishes order 0, row 1 within 16 frames of L");
    assert_eq!(first_row, 8921);
    assert_eq!(first_row - port_index(9112), 5);
    let lit = (10_040..=10_047)
        .find(|index| rendered(&dir, &manifest, *index).0.iter().any(|byte| *byte != 0))
        .expect("ALKU's fade-in has begun by 10047");
    assert_eq!(lit, 10_045);
    let hold = reference_frame("alku", 10419);
    let held = (10_100..=10_107)
        .find(|index| differing_pixels(&rendered(&dir, &manifest, *index).0, &hold) == 0)
        .expect("the fade-in has reached its hold by 10107");
    assert_eq!(held, 10_106);
}

#[test]
fn looping_leaves_pass_one_alone() {
    // The music clock is the picture: up to the seam, `--loop`'s frames and published
    // music are the plain run's, byte for byte.
    let indices = [404, 1212, 1440, 5440, 8740, PORT_C - 1, PORT_C, PORT_C + 1];
    let (plain_dir, plain) = dump("pass1-plain", &[], &indices);
    let (loop_dir, looping) = dump("pass1-loop", &["--loop"], &indices);
    assert_eq!(plain["music"], looping["music"], "the same entry install");
    for index in indices {
        let a = entry(&plain, index);
        let b = entry(&looping, index);
        assert_eq!(a["music_position"], b["music_position"], "{index}: the music");
        assert_eq!(a["part"], b["part"], "{index}");
        let (a, ..) = rendered(&plain_dir, &plain, index);
        let (b, ..) = rendered(&loop_dir, &looping, index);
        assert_eq!(differing_pixels(&a, &b), 0, "{index}: the picture");
    }
}
