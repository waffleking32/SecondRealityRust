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
//! read), so the comparison is RGB, byte for byte; the seam frames' counts are rediff's
//! `loop/*` entries (`tests/reference_frames.rs`), and this file measures what they rest on.
//!
//! **What the seam does not match, pinned as measured (D24; `APPROXIMATIONS.md` A61 registers
//! it).** The port's CRED shows its closed iris on two ticks before it retires (8833, 8834), so its
//! fade is 8835–8898 — the original's 9031–9094, **2 frames** after the original's
//! 9029–9092 (§25.3). `LOOP_RELOAD_TICKS` is the original's own 11 frames from the fade's
//! end to `L` (9093–9103), so the port's `L` is 2 frames late too: the original's 9104 is
//! the port's 8910, not 8908, and the port's 8908 is still CRED's 320×200 screen.

mod common;

use common::{differing, dump, reference, Dump};

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

/// The port's index answering the loop capture's `index`.
fn port_index(index: u32) -> u32 {
    index - C + PORT_C
}

fn filed(screen: &str, index: u32) -> Vec<u8> {
    reference(&format!("loop/{screen}"), index, "rgb")
}

/// Whether the port's frame at `index` is `expected` byte for byte.
fn is(run: &Dump, index: u32, expected: &[u8]) -> bool {
    differing(run.rgb(index), expected, 3) == 0
}

#[test]
fn the_loop_seam_is_the_originals() {
    // The seam's frames, held at their counts by rediff's `loop/*` entries; this reads what
    // the counts rest on.
    let mut indices: Vec<u32> = (PORT_C - 8..=PORT_C + 8).collect();
    // The seam's ALKU start, the restarted song's first published row, and ALKU's pass-2
    // fade-in (below).
    indices.extend(port_index(L) - 2..=port_index(L) + 16);
    indices.extend(10_040..=10_047);
    indices.extend(10_100..=10_107);
    let run = dump(&["--block", "5", "--loop"], &indices);

    // `C` is the first of 8825..=8841 that is the filed `C`'s bytes (the iris is still closing
    // before it). CRED's chain drift is then 9029 − 115 − that index.
    let c_bytes = filed("cred", C);
    let measured_c = (PORT_C - 8..=PORT_C + 8)
        .find(|index| is(&run, *index, &c_bytes))
        .expect("the port's CRED closes its iris near 8833");
    assert_eq!(measured_c, PORT_C, "the port's C");
    assert_eq!(C - LOOP_RECIPE_SHIFT - measured_c, 81, "CRED's chain drift");

    // `L` is 2 frames late: at the port's `C_port + (L − C)` it still shows CRED's closed iris,
    // and its first ALKU frame is `CRED_TAIL` later. `L − C` is 75 in the original and 77 in
    // the port.
    let frame = run.frame(port_index(L));
    assert_eq!((frame.part.as_str(), frame.width, frame.height), ("cred", 320, 200), "L − 1 is CRED's screen");
    let port_l = (port_index(L) - 2..=port_index(L) + 16)
        .find(|index| run.part(*index) == "alku")
        .expect("ALKU starts within 16 frames of L");
    assert_eq!(port_l, port_index(L) + CRED_TAIL);
    assert_eq!(port_l, 8910);
    assert_eq!((L - C, port_l - PORT_C), (75, 77));

    // **Pass 2's music: 5 frames late against `C`.** The seam trace has the restarted song at
    // (0,1) at 9112 (§25.3), the port's 8916, 20 frames after the original's fade ends (9092).
    // The port's fade ends at 8898 (CRED's 2-tick tail) and the restart, installed there under
    // `MusicStart::restart`, publishes (0,1) 23 frames later, at 8921: 2 frames of tail and 3
    // of the restart's recipe.
    let first_row = (port_index(L) - 2..=port_index(L) + 16)
        .find(|index| run.position(*index) == "0:1")
        .expect("the restarted song publishes order 0, row 1 within 16 frames of L");
    assert_eq!(first_row, 8921);
    assert_eq!(first_row - port_index(9112), 5);
    let port_fade_end = PORT_C + CRED_TAIL + 63;
    assert_eq!((port_fade_end, first_row - port_fade_end, 9112 - 9092), (8898, 23, 20));

    // **ALKU's pass-2 fade-in follows the music: 3 to 5 frames late, and its hold 4 to 6.**
    // The original's first lit frame is in (10236, 10238] and its "A Future Crew Production"
    // hold starts in (10296, 10298] (two-frame sampling, §25.3): the port's (10040, 10042] and
    // (10100, 10102]. The port's are 10045 and 10106.
    let lit = (10_040..=10_047)
        .find(|index| run.rgb(*index).iter().any(|byte| *byte != 0))
        .expect("ALKU's fade-in has begun by 10047");
    assert_eq!(lit, 10_045);
    let hold = filed("alku", 10419);
    let held = (10_100..=10_107).find(|index| is(&run, *index, &hold)).expect("the fade-in holds by 10107");
    assert_eq!(held, 10_106);
}

#[test]
fn looping_leaves_pass_one_alone() {
    // Up to the seam, `--loop`'s frames and published music are the plain run's, byte for byte.
    let indices = [404, 1212, 1440, 5440, 8740, PORT_C - 1, PORT_C, PORT_C + 1];
    let plain = dump(&["--block", "5"], &indices);
    let looping = dump(&["--block", "5", "--loop"], &indices);
    assert_eq!(plain.manifest["music"], looping.manifest["music"], "the same entry install");
    for index in indices {
        let (a, b) = (plain.frame(index), looping.frame(index));
        assert_eq!(a.music_position, b.music_position, "{index}: the music");
        assert_eq!(a.part, b.part, "{index}");
        assert!(a.rgb == b.rgb, "{index}: the picture");
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
    let run = common::Dump::with_positions(&["--block", "5", "--loop"], &[8925]);
    let timeline = &run.timeline;
    let position = |frame: u32| timeline[&frame].position.unwrap();

    // The restart: the fade's end is 8898 (`the_loop_seam_is_the_originals`), whose tick installs
    // the new player — its row already reads the new player's `0:0` — and ALKU starts at 8910.
    const RESTART: u32 = 8898;
    const ALKU: u32 = 8910;
    assert_eq!(position(RESTART - 1), (43, 33), "pass 1's last published row");
    assert_eq!(position(RESTART), (0, 0), "the new player, installed on this tick");
    assert_eq!(timeline[&ALKU].part, "alku");
    assert_eq!(timeline[&(ALKU - 1)].part, "cred");

    // Pass 1's music, played here: block 5's MUSIC0 at order 25 with the shell's tempo patch,
    // its clock waiting the restart recipes' four frames from the run's 404 (polled from 408),
    // one poll a frame through the restart's own tick. It must be the run's, frame for frame.
    let module = sr_audio::SONGS[0].load(&common::root()).unwrap();
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
