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
//!   there as 82 from ENDSCRL's start), not the loop's, and the test reads it off the
//!   port's frames: 8832 is still the iris and 8833 is the filed `C`'s bytes.
//!
//! So the seam is compared **from `C`**: the port's frame at `C_port + (n − C)` answers the
//! original's `n`. The capture holds `.rgb` only (§25.4: neither screen's `.idx` survives the
//! read), so the comparison is RGB, byte for byte.
//!
//! **What the seam does not match, pinned as measured (D24; `APPROXIMATIONS.md` A61 registers
//! it).** The port's CRED shows its closed iris on two ticks before it retires (8833, 8834), so its
//! fade is 8835–8898 — the original's 9031–9094, **2 frames** after the original's
//! 9029–9092 (§25.3). `LOOP_RELOAD_TICKS` is the original's own 11 frames from the fade's
//! end to `L` (9093–9103), so the port's `L` is 2 frames late too: the original's 9104 is
//! the port's 8910, not 8908, and the port's 8908 is still CRED's 320×200 screen.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The loop recipe's boot runs this many frames later than the B1 recipe's (§25.1).
const LOOP_RECIPE_SHIFT: u32 = 115;

/// `C` in the loop capture's own numbering: CRED's closed screen (§25.3).
const C: u32 = 9029;

/// The port's `C`: its first closed-iris frame in `--block 5`'s numbering (asserted on the
/// dumped frames below).
const PORT_C: u32 = 8833;

/// `L` in the loop capture's numbering: ALKU's first frame on pass 2 (§25.3).
const L: u32 = 9104;

/// How late the port's seam is against `C`: CRED's closed iris is on screen for two port
/// ticks before the fade begins.
const CRED_TAIL: u32 = 2;

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

/// The filed seam: (screen, index).
const SEAM: &[(&str, u32)] = &[
    ("cred", 9029),
    ("cred", 9055),
    ("cred", 9081),
    ("alku", 9104),
    ("alku", 9600),
    ("alku", 10419),
    ("alku", 11419),
];

/// What the port's frame at a filed index is against the original's.
#[derive(Debug, PartialEq, Eq)]
enum Count {
    /// The same geometry, and this many pixels differ.
    Pixels(usize),
    /// A different screen: the port's part and geometry at that index.
    Geometry(String, u64, u64),
}

#[test]
fn the_loop_seam_is_the_originals() {
    let mut indices: Vec<u32> = (PORT_C - 8..=PORT_C + 8).collect();
    indices.extend(SEAM.iter().map(|(_, index)| port_index(*index)));
    // The seam's ALKU start, the restarted song's first published row, and ALKU's pass-2
    // fade-in (below).
    indices.extend(port_index(L) - 2..=port_index(L) + 16);
    indices.extend(10_040..=10_047);
    indices.extend(10_100..=10_107);
    let (dir, manifest) = dump("seam", &["--loop"], &indices);

    // `C` is where the port's frames put it: the first of 8825..=8841 that is the filed
    // `C`'s own bytes (the iris is still closing before it). CRED's chain drift is then
    // 9029 − 115 − that index.
    let c_bytes = reference_frame("cred", C);
    let measured_c = (PORT_C - 8..=PORT_C + 8)
        .find(|index| differing_pixels(&rendered(&dir, &manifest, *index).0, &c_bytes) == 0)
        .expect("the port's CRED closes its iris near 8833");
    assert_eq!(measured_c, PORT_C, "the port's C");
    assert_eq!(C - LOOP_RECIPE_SHIFT - measured_c, 81, "CRED's chain drift");

    // Each filed frame at the port's `C_port + (n − C)`.
    let mut counts = Vec::new();
    for &(screen, index) in SEAM {
        let at = port_index(index);
        let (bytes, part, width, height) = rendered(&dir, &manifest, at);
        let expected = reference_frame(screen, index);
        counts.push((
            index,
            if bytes.len() == expected.len() && part == screen {
                Count::Pixels(differing_pixels(&bytes, &expected))
            } else {
                Count::Geometry(part, width, height)
            },
        ));
    }
    assert_eq!(
        counts,
        [
            (9029, Count::Pixels(0)),
            (9055, Count::Pixels(0)),
            (9081, Count::Pixels(0)),
            // `L` is 2 frames late: the port still shows CRED's closed iris here.
            (9104, Count::Geometry("cred".to_string(), 320, 200)),
            (9600, Count::Pixels(0)),
            (10419, Count::Pixels(0)),
            (11419, Count::Pixels(0)),
        ],
        "the seam's held frames, and its one late frame"
    );

    // The port's `L`: its first ALKU frame is `CRED_TAIL` after the original's, and it is
    // the original's `L` byte for byte. `L − C` is 75 in the original and 77 in the port.
    let port_l = (port_index(L) - 2..=port_index(L) + 16)
        .find(|index| entry(&manifest, *index)["part"] == "alku")
        .expect("ALKU starts within 16 frames of L");
    assert_eq!(port_l, port_index(L) + CRED_TAIL);
    assert_eq!(port_l, 8910);
    let (_, part, width, height) = rendered(&dir, &manifest, port_l - 1);
    assert_eq!((part.as_str(), width, height), ("cred", 320, 200), "L − 1 is CRED's screen");
    let (bytes, ..) = rendered(&dir, &manifest, port_l);
    assert_eq!(differing_pixels(&bytes, &reference_frame("alku", L)), 0, "the port's L is L");
    assert_eq!((L - C, port_l - PORT_C), (75, 77));

    // **Pass 2's music: 5 frames late against `C`.** The seam trace has the restarted song
    // at (0,1) at 9112 (§25.3) — the port's 8916 — 20 frames after the original's fade
    // ends (9092). The port's fade ends at 8898 (CRED's 2-tick tail) and the restart,
    // installed there under `MusicStart::restart`, publishes (0,1) 23 frames later, at
    // 8921: 2 frames of tail and 3 of the restart's recipe.
    let first_row = (port_index(L) - 2..=port_index(L) + 16)
        .find(|index| entry(&manifest, *index)["music_position"] == "0:1")
        .expect("the restarted song publishes order 0, row 1 within 16 frames of L");
    assert_eq!(first_row, 8921);
    assert_eq!(first_row - port_index(9112), 5);
    let port_fade_end = PORT_C + CRED_TAIL + 63;
    assert_eq!((port_fade_end, first_row - port_fade_end, 9112 - 9092), (8898, 23, 20));

    // **ALKU's pass-2 fade-in follows the music: 3 to 5 frames late, and its hold 4 to 6.**
    // The original's first lit frame is in (10236, 10238] and its "A Future Crew
    // Production" hold starts in (10296, 10298] (two-frame sampling, §25.3): the port's
    // (10040, 10042] and (10100, 10102]. The port's are 10045 and 10106. The held frames
    // above sit inside their holds either way.
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

/// **A55's counterfactual, measured: a restart that carried the old player's position ring
/// would hand pass 2's ALKU the old song's position.** The port's restart does not carry the
/// ring (`Player::restart_from`, D45, A55). Had it carried it, the new player would publish, until
/// its own first row came through, exactly what the old player publishes if it is stopped and
/// polled on (`crates/sr-audio/src/player.rs`'s
/// `a_carried_ring_publishes_what_the_stopped_previous_player_would` proves the equality). So this
/// test plays block 5's pass-1 MUSIC0 itself, checks it frame for frame against the run's own
/// `--positions`, stops it at the loop's restart and reads what ALKU's first frames would see.
///
/// The run publishes `0:0` there, whose `dis_sync` is the word before the table (negative), so
/// ALKU's first gate, `dis_sync() < 1` (`ALKU/MAIN.C:59`), waits for the new song — the filed
/// capture's behaviour (D42's majority). The carried ring would publish the old song's `43:34`
/// over every one of ALKU's frames before the new song's first row, whose `dis_sync` is 10: past
/// all four of ALKU's gates at once, which is D42's minority session (there the stale `(43,43)`,
/// sync 10, which leaves at once).
#[test]
fn a_carried_ring_would_hand_pass_two_the_old_songs_position() {
    use sr_audio::{Player, PlayerOptions};
    use sr_engine::music::sync_from;

    // The run: `--block 5 --loop`, its timeline to just past the new song's first published row.
    let dir = std::env::temp_dir().join(format!("sr-loop-{}-ring", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let positions = dir.join("positions.csv");
    std::fs::create_dir_all(&dir).unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_secondreality"))
        .current_dir(repository_root())
        .args(["--block", "5", "--loop", "--headless", "--dump-frames", "8925", "--dump-dir"])
        .arg(dir.join("frames"))
        .arg("--positions")
        .arg(&positions)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let timeline: std::collections::BTreeMap<u32, (String, u8, u8)> = std::fs::read_to_string(&positions)
        .unwrap()
        .lines()
        .skip(1)
        .map(|line| {
            let fields: Vec<&str> = line.split(',').collect();
            (fields[0].parse().unwrap(), (fields[2].to_string(), fields[3].parse().unwrap(), fields[4].parse().unwrap()))
        })
        .collect();
    let _ = std::fs::remove_dir_all(&dir);
    let position = |frame: u32| (timeline[&frame].1, timeline[&frame].2);

    // The restart: the fade's end is 8898 (`the_loop_seam_is_the_originals`), whose tick installs
    // the new player — its row already reads the new player's `0:0` — and ALKU starts at 8910.
    const RESTART: u32 = 8898;
    const ALKU: u32 = 8910;
    assert_eq!(position(RESTART - 1), (43, 33), "pass 1's last published row");
    assert_eq!(position(RESTART), (0, 0), "the new player, installed on this tick");
    assert_eq!(timeline[&ALKU].0, "alku");
    assert_eq!(timeline[&(ALKU - 1)].0, "cred");

    // Pass 1's music, played here: block 5's MUSIC0 at order 25 with the shell's tempo patch,
    // its clock waiting the restart recipes' four frames from the run's 404 (polled from 408),
    // one poll a frame through the restart's own tick. It must be the run's, frame for frame.
    let module = sr_audio::SONGS[0].load(&repository_root()).unwrap();
    let mut old = Player::new(module, PlayerOptions::default());
    old.play_song(25);
    for frame in 404..RESTART {
        if frame >= 408 {
            old.poll();
        }
        let state = old.state();
        assert_eq!((state.order, state.row), position(frame), "pass 1's music at {frame}");
    }
    old.poll(); // the restart tick's own poll, before the install

    // The counterfactual: the old player stopped and polled on wherever the new one is polled —
    // from the end of `MusicStart::restart`'s four frames of waiting — up to the new song's
    // first published row, 8921 (`the_loop_seam_is_the_originals`).
    assert_eq!(position(8921), (0, 1), "the new song's first published row");
    old.stop();
    let mut carried = std::collections::BTreeMap::new();
    for frame in RESTART + 5..8921 {
        old.poll();
        let state = old.state();
        carried.insert(frame, (state.order, state.row));
    }
    for frame in ALKU..8921 {
        let fresh = position(frame);
        let stale = carried[&frame];
        assert_eq!(fresh, (0, 0), "{frame}: the run publishes the new player's 0:0");
        assert!(sync_from(fresh.0, fresh.1) < 1, "{frame}: ALKU's first gate waits");
        assert_eq!(stale.0, 43, "{frame}: a carried ring publishes the old song's order");
        assert_eq!(sync_from(stale.0, stale.1), 10, "{frame}: past all four of ALKU's gates");
    }
    println!(
        "a carried ring at ALKU's first frame, {ALKU}: {:?} (sync {}); at 8920: {:?}; the run: 0:0 (sync {})",
        carried[&ALKU],
        sync_from(carried[&ALKU].0, carried[&ALKU].1),
        carried[&8920],
        sync_from(0, 0)
    );
    // The ring has drained to the old song's last fed row by ALKU's first frame.
    assert_eq!((carried[&ALKU], carried[&8920]), ((43, 34), (43, 34)));
}
