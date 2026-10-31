//! Each part's `--part` run through the real binary: the numbering and music its manifest
//! states, and what the frames do that `cargo xtask rediff` cannot see (D19's holds and moves
//! around the keyframes, the music at a keyframe, a part's first ticks, its end and the hold
//! after it). The keyframes' pixel counts are rediff's, run as `tests/reference_frames.rs`.
//!
//! One module a part; each runs its part once, dumping the union of the frames its tests read.

mod common;

use common::{differing, dump, reference, trace_position, Dump};

/// The manifest's numbering for a run of `part`: boot-numbered from `anchor`, or its own frames
/// from zero; its music `(module, start order, start frame)`, or none; and, when one geometry
/// covers the run, every frame's.
fn numbered(
    dump: &Dump,
    part: &str,
    anchor: Option<u32>,
    music: Option<(&str, u32, u32)>,
    size: Option<(u64, u64)>,
) {
    let manifest = &dump.manifest;
    assert_eq!(manifest["part"], part);
    assert_eq!(manifest["index_origin"], if anchor.is_some() { "boot" } else { "part" });
    assert_eq!(manifest["anchor"], serde_json::json!(anchor));
    match music {
        Some((module, order, frame)) => {
            assert_eq!(manifest["music"]["module"], module);
            assert_eq!(manifest["music"]["start_order"], order);
            assert_eq!(manifest["music"]["start_frame"], frame);
        }
        None => assert!(manifest["music"].is_null(), "{part}: {}", manifest["music"]),
    }
    if let Some((width, height)) = size {
        for (index, frame_part, w, h) in dump.listing() {
            assert_eq!((frame_part, w, h), (part, width, height), "frame {index}");
        }
    }
}

/// `K - 1, K, K + 1` for each tick.
fn with_neighbours(ticks: &[u32]) -> Vec<u32> {
    ticks.iter().flat_map(|tick| [tick - 1, *tick, tick + 1]).collect()
}

/// D19's support: a moving keyframe differs from both neighbours, a held one from neither.
fn moves_or_holds(dump: &Dump, label: u32, tick: u32, moving: bool) {
    for neighbour in [tick - 1, tick + 1] {
        if moving {
            assert_ne!(dump.rgb(neighbour), dump.rgb(tick), "{label}: tick {neighbour} is the same picture");
        } else {
            assert_eq!(dump.rgb(neighbour), dump.rgb(tick), "{label}: tick {neighbour} is not the held picture");
        }
    }
}

fn black(bytes: &[u8]) -> bool {
    bytes.iter().all(|byte| *byte == 0)
}

/// ALKU: anchored at the boot's 404 (`reference-indices.md` §1.12, D17) under block 1's
/// `_zplaysong(0)` (`MAIN/U2.ASM:766-775`), fitted at 400 and pre-polled to 404.
mod alku {
    use super::*;

    const FRAMES: &[u32] = &[899, 900, 901, 1718, 1719, 1720, 2718, 2719, 2720, 5858, 5859, 5860];

    fn run() -> std::sync::Arc<Dump> {
        dump(&["--part", "alku"], FRAMES)
    }

    #[test]
    fn the_run_is_numbered_in_boot_frames_and_draws_320x400() {
        numbered(&run(), "alku", Some(404), Some(("MUSIC0", 0, 404)), Some((320, 400)));
    }

    #[test]
    fn the_support_frames_hold_as_the_references_do() {
        // §4.3: all four keyframes are held; 5859's window moves every nine frames and, with
        // the fifth batch's text gone (A20), nothing else does.
        let run = run();
        for index in [900, 1719, 2719, 5859] {
            moves_or_holds(&run, index, index, false);
        }
    }

    #[test]
    fn the_runs_music_is_the_block_one_recipes_at_the_keyframes() {
        // The pre-polls leave the published row 2-4 frames behind the trace's row changes, so
        // a keyframe may sit one row either side; the order must agree.
        let run = run();
        for index in [900, 1719, 2719, 5859] {
            let port = run.position(index);
            let traced = trace_position("second1", index);
            let (port_order, port_row) = port.split_once(':').unwrap();
            let (order, row) = traced.split_once(':').unwrap();
            assert_eq!(port_order, order, "{index}: the port reads {port}, the trace {traced}");
            let row_gap = port_row.parse::<i32>().unwrap() - row.parse::<i32>().unwrap();
            assert!(row_gap.abs() <= 1, "{index}: the port reads {port}, the trace {traced}");
        }
    }
}

/// BEGLOGO: anchored at 404 under block 2's MUSIC1@0, whose clock starts at 408.
mod beglogo {
    use super::*;

    #[test]
    fn the_four_keyframes_dump_in_the_references_own_numbering() {
        let run = dump(&["--part", "beglogo"], &[430, 470, 560, 900]);
        numbered(&run, "beglogo", Some(404), Some(("MUSIC1", 0, 408)), Some((320, 400)));
    }
}

/// ENDLOGO: anchored at 404, the frame `SECOND 5`'s mode set and `inittwk` produce
/// (`MAIN/U2.ASM:906-908`), under MUSIC0@25.
mod endlogo {
    use super::*;

    #[test]
    fn the_four_keyframes_dump_in_the_references_own_numbering() {
        let run = dump(&["--part", "endlogo"], &[430, 500, 900, 1180]);
        numbered(&run, "endlogo", Some(404), Some(("MUSIC0", 25, 408)), Some((320, 400)));
    }
}

/// 3DSINFLD (COMAN): no anchor, and each keyframe's tick is measured because the implied
/// origins drift (`3dsinfld.md` §6, A26).
mod coman {
    use super::*;

    /// The four keyframes and the tick each is drawn on.
    const KEYFRAMES: [(u32, u32); 4] = [(15500, 464), (16000, 966), (16500, 1470), (16800, 1772)];

    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 1];
        frames.extend(KEYFRAMES.iter().flat_map(|(_, tick)| [tick - 1, *tick, tick + 1, tick + 2]));
        dump(&["--part", "3dsinfld"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_declares_the_derived_music_row() {
        // Order 91 is derived: the start window (15000, 15100] spans the traces' 91 -> 93
        // crossing (`3dsinfld.md` §5).
        numbered(&run(), "3dsinfld", None, Some(("MUSIC1", 91, 0)), Some((320, 200)));
    }

    /// Each picture is drawn on two consecutive ticks: `K+1` holds it, and `K-1`/`K+2` are the
    /// two-frame step the reference's sessions show (`reference-indices.md` §16.3).
    #[test]
    fn each_keyframe_holds_for_two_ticks_and_its_neighbours_differ() {
        let run = run();
        for (index, tick) in KEYFRAMES {
            assert_eq!(run.rgb(tick + 1), run.rgb(tick), "{index}: the held tick");
            assert_ne!(run.rgb(tick - 1), run.rgb(tick), "{index}: the tick before");
            assert_ne!(run.rgb(tick + 2), run.rgb(tick), "{index}: the step after the hold");
        }
    }
}

/// DDSTARS: no anchor; boot = 414 + tick (`ddstars.md` §3). `SECOND U` restarts MUSIC0 at
/// order 70 (`MAIN/U2.ASM:889-894`).
mod ddstars {
    use super::*;

    const ORIGIN: u32 = 414;
    const KEYFRAMES: [u32; 4] = [1900, 3000, 5000, 8000];

    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 1];
        frames.extend(with_neighbours(&KEYFRAMES.map(|index| index - ORIGIN)));
        dump(&["--part", "ddstars"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_draws_320x400() {
        let run = run();
        numbered(&run, "ddstars", None, Some(("MUSIC0", 70, 0)), Some((320, 400)));
        // Tick 0 presents the page the loop has not written: the mode set's cleared picture,
        // under the first fade's `b = 8`. Tick 1 presents tick 0's first star pass.
        assert!(black(run.rgb(0)) && black(run.idx(0)), "tick 0 is the cleared page");
        assert!(!black(run.rgb(1)), "tick 1 shows the first star pass");
    }

    #[test]
    fn the_keyframes_neighbours_differ_and_the_music_runs_twenty_two_frames_behind_the_trace() {
        // §22.3: every keyframe moves.
        let run = run();
        for index in KEYFRAMES {
            moves_or_holds(&run, index, index - ORIGIN, true);
        }
        // The part reads no music; this is where the shell put it. The clock starts at D27's
        // 408, the mapping's zero is 414 and the hiddenu2 session's phase is -16 frames, so the
        // port at boot 8000 reads the trace's 7978.
        assert_eq!(run.position(8000 - ORIGIN), trace_position("hiddenu2", 7978));
    }
}

/// GLENZ: no anchor of its own (block 2's is BEGLOGO's), so a part run numbers itself from
/// zero; its first ticks are the music wait (`MAIN.C:290`), on the inherited Mode X screen.
mod glenz {
    use super::*;

    #[test]
    fn a_part_run_of_glenz_numbers_itself_from_zero() {
        let run = dump(&["--part", "glenz"], &[0, 1]);
        numbered(&run, "glenz", None, Some(("MUSIC1", 0, 0)), Some((320, 400)));
    }
}

/// JPLOGO: no anchor; boot = 17065 + tick (`jplogo.md` §3). The `second3d` trace holds MUSIC1
/// order 96 across the start window (17000..17100); the `254` at 97 is crossed at 17109.
mod jplogo {
    use super::*;

    /// The three keyframes and the tick each is drawn on.
    const KEYFRAMES: [(u32, u32); 3] = [(17300, 235), (17500, 435), (17700, 635)];

    /// The tick the start window's `17100` sweep frame matches: entry step 34, address 309.
    const SWEEP_TICK: u32 = 35;

    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 1, SWEEP_TICK];
        frames.extend(with_neighbours(&KEYFRAMES.map(|(_, tick)| tick)));
        dump(&["--part", "jplogo"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_declares_the_measured_music_row() {
        let run = run();
        numbered(&run, "jplogo", None, Some(("MUSIC1", 96, 0)), Some((320, 400)));
        assert_eq!(run.position(0), "96:58");
    }

    #[test]
    fn the_sweeps_17100_is_entry_step_thirty_four() {
        assert_eq!(run().position(SWEEP_TICK), trace_position("second3d", 17100));
    }

    /// 17300 and 17500 hold on the next tick after a different state; 17700's settled state is
    /// static. The published position at each keyframe is the trace's.
    #[test]
    fn the_keyframes_hold_as_the_reference_does_and_publish_the_traces_music() {
        let run = run();
        for (index, tick) in [(17300, 235), (17500, 435)] {
            assert_eq!(run.rgb(tick + 1), run.rgb(tick), "{index}: the held tick");
            assert_ne!(run.rgb(tick - 1), run.rgb(tick), "{index}: the tick before");
        }
        moves_or_holds(&run, 17700, 635, false);
        for (index, tick) in KEYFRAMES {
            assert_eq!(run.position(tick), trace_position("second3d", index), "{index} (tick {tick})");
        }
    }
}

/// LNS&ZOOM: no anchor; boot = 2452 + tick (`lnszoom.md` §7). The whole part is 320×200:
/// parts 1–2 are mode 13h, and part 3's planar screen is 100 character rows each shown twice.
mod lnszoom {
    use super::*;

    const ORIGIN: u32 = 2452;

    fn run() -> std::sync::Arc<Dump> {
        let frames = [
            0, 247, 248, 249, 947, 948, 949, 1747, 1748, 1749, 2447, 2448, 2449, 3195, 3197, 3198, 3199,
        ];
        dump(&["--part", "lnszoom"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
        numbered(&run(), "lnszoom", None, Some(("MUSIC1", 45, 0)), Some((320, 200)));
    }

    #[test]
    fn the_five_keyframes_at_the_measured_mapping() {
        let run = run();
        let held = |index: u32| run.rgb(index - ORIGIN);

        // The rotazoomer's screen is 100 character rows on two scanlines each (`inittwk`'s
        // maximum scan line 3); the reference's 4200 is row pairs too.
        for picture in [held(4200), &reference("lnszoom", 4200, "rgb")] {
            for row in 0..100 {
                assert_eq!(
                    &picture[2 * row * 960..(2 * row + 1) * 960],
                    &picture[(2 * row + 1) * 960..(2 * row + 2) * 960],
                    "character row {row} is two scanlines"
                );
            }
        }
        assert!(held(5650).chunks_exact(3).all(|pixel| pixel == [255, 255, 255]), "the white exit");

        // 5650's filed `.idx` is the capture tool's planar memory read, not the displayed frame:
        // 100 character rows and zeros past them. Pinned as evidence, not compared.
        let plane = reference("lnszoom", 5650, "idx");
        assert_eq!(plane.len(), 64_000);
        assert!(black(&plane[100 * 320..]), "rows 100..199 are the memory past the 100 rows");
        assert_eq!(plane[..100 * 320].iter().filter(|byte| **byte != 0).count(), 13_692);
        assert_eq!(plane.iter().collect::<std::collections::BTreeSet<_>>().len(), 63);
        // The port's plane is the displayed picture: the last rotazoomer frame, unchanged by
        // the white palette.
        assert_eq!(run.idx(5650 - ORIGIN), run.idx(3195), "the white exit changes the palette only");

        // D19: 2700 and 5650 held, 3400, 4200 and 4900 moving.
        for (index, moving) in [(2700, false), (3400, true), (4200, true), (4900, true), (5650, false)] {
            moves_or_holds(&run, index, index - ORIGIN, moving);
        }
    }
}

/// MINVBALL: no anchor; boot = 10,368 + tick (`minvball.md` §3).
mod minvball {
    use super::*;

    const ORIGIN: u32 = 10_368;
    /// `reference-indices.md` §14.2.
    const KEYFRAMES: [u32; 4] = [10_900, 11_200, 11_800, 12_400];

    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 130, 2579, 2580];
        frames.extend(with_neighbours(&KEYFRAMES.map(|index| index - ORIGIN)));
        dump(&["--part", "minvball"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
        let run = run();
        numbered(&run, "minvball", None, Some(("MUSIC1", 75, 0)), Some((320, 200)));
        // The 256-poll pre-roll: the tick-0 poll publishes the trace's reading at boot 10,368.
        assert_eq!(run.position(0), "75:61");

        // Tick 0 is the first fade wait: the floor ramp under a black DAC (`MAIN.C:151-155`).
        let first = run.idx(0);
        for a in 0..100usize {
            let row = 100 + a;
            assert!(
                first[row * 320..(row + 1) * 320].iter().all(|p| *p == (a + 64) as u8),
                "tick 0: row {row} carries index {}",
                a + 64
            );
        }
        assert!(black(&first[..100 * 320]));
        assert!(black(run.rgb(0)), "tick 0's DAC is black");
        // Tick 130 is the main loop's first draw: dots on the ramp, under the fade's last
        // upload (`pal`).
        let row = 158;
        let ramp = run.idx(130)[row * 320..(row + 1) * 320].iter().filter(|p| **p == (row - 100 + 64) as u8);
        assert!(ramp.count() > 300, "row {row} is still mostly the ramp");
        assert!(!black(run.rgb(130)), "the main loop's first draw is not black");
    }

    #[test]
    fn each_keyframes_neighbours_differ_from_it() {
        let run = run();
        for index in KEYFRAMES {
            moves_or_holds(&run, index, index - ORIGIN, true);
        }
    }

    #[test]
    fn the_run_reaches_the_frame_cap_and_retires() {
        // `MAIN.C:185` `frame<2450`: the last draw is tick 130 + 2449 = 2579 (boot 12,947), and
        // the run holds it. The last upload before the cap is `pal2` at frame 2449, whose
        // c = 63-2a has long reached zero (`MAIN.C:258-269`), so the picture is black.
        let run = run();
        assert_eq!(run.rgb(2580), run.rgb(2579), "the retired part holds its last frame");
        assert!(black(run.rgb(2579)), "the frame cap leaves the fade-out's black DAC");
    }
}

/// MNTSCRL: no anchor; the fade-in pins tick 0 at boot 419 (`mntscrl.md` §2). The block-3
/// restart starts MUSIC1@42 on the clock's 408 (`MAIN/U2.ASM:856-863`).
mod mntscrl {
    use super::*;

    const ORIGIN: u32 = 419;

    fn run() -> std::sync::Arc<Dump> {
        let frames = [
            0, 81, 134, 135, 136, 680, 681, 682, 1280, 1281, 1282, 1880, 1881, 1882, 1960, 1979, 1980,
            2040, 2050, 2100,
        ];
        dump(&["--part", "mntscrl"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
        let run = run();
        numbered(&run, "mntscrl", None, Some(("MUSIC1", 42, 0)), Some((320, 200)));
        // `:121-123`: tick 0 blits the picture and sets the DAC black; the hold that follows
        // does not draw.
        assert!(black(run.rgb(0)), "tick 0's DAC is black");
        assert_eq!(run.idx(0), run.idx(81));
    }

    #[test]
    fn the_music_is_eleven_polls_in_at_the_parts_first_tick() {
        // The part's tick 0 is boot 419, 11 polls after the clock's 408; `Mntscrl::init`
        // pre-polls them, so tick 81 (boot 500) publishes `second3`'s 42:10 there (without the
        // pre-roll it would read 42:8).
        let run = run();
        assert_eq!((run.position(0), run.position(81)), ("0:0", "42:10"));
        assert_eq!(run.position(81), trace_position("second3", 500));
    }

    #[test]
    fn each_scroll_keyframes_neighbours_differ_from_it() {
        // The scroller moves 2.4-3.1 % of its pixels a frame.
        let run = run();
        for index in [1100, 1700, 2300] {
            moves_or_holds(&run, index, index - ORIGIN, true);
        }
    }

    #[test]
    fn the_releases_first_wait_is_seventy_one_ticks() {
        // The reference's last held frame is boot 554 and its pre-roll's first `scr` 555: ticks
        // 135 and 136. The music shows no boundary there (42:16 at both 134 and 135), which is
        // why the release's wait is a constant, not a threshold (`mntscrl.md` §6).
        let run = run();
        assert_eq!(run.rgb(134), run.rgb(81), "tick 134 still holds");
        assert_eq!(run.rgb(135), run.rgb(81), "the wait's last tick draws nothing");
        assert_ne!(run.rgb(136), run.rgb(81), "the pre-roll's first scr");
        let positions = [81, 134, 135, 136].map(|index| run.position(index));
        assert_eq!(positions, ["42:10", "42:16", "42:16", "42:17"]);
    }

    #[test]
    fn the_real_players_cue_fades_the_part_out_and_the_run_holds_black() {
        // The `:196-203` cue is `dis_musplus() == -11`: with the release's wait the loop starts
        // at tick 263 and the first -11 after it is 45:53, read at 1,979 (the fade's
        // full-palette upload), one step down at 1,980. The 65-tick fade (`:207-214`) ends by
        // 2,050; the part retires and the run holds black.
        let run = run();
        let positions = [1960, 1979, 1980, 2040, 2050, 2100].map(|index| run.position(index));
        assert_eq!(positions, ["45:50", "45:53", "45:53", "45:60", "45:61", "47:8"]);
        let ink = |index: u32| run.rgb(index).iter().filter(|byte| **byte != 0).count();
        assert!(ink(1960) > 180_000, "1,960 is still the full scroller");
        assert!(ink(1979) > 180_000, "1,979 uploads the full palette");
        assert!(ink(1980) < ink(1979), "the fade's first step is visible");
        assert!(ink(2040) < 5_000, "2,040 is the fade's tail");
        assert!(black(run.rgb(2050)) && black(run.rgb(2100)), "the held screen is black");
    }
}

/// PAM: anchored at 7710, the reference's first PAM frame (`reference-indices.md` §6.1), under
/// block 1's `_zplaysong(0)` pre-polled to it (`MAIN/U2.ASM:766-775`).
mod pam {
    use super::*;

    /// §6.3's support: 7760 moves; 7710, 7830 and 7900 hold (7710's `K-1` is before the anchor).
    fn run() -> std::sync::Arc<Dump> {
        let frames = [7710, 7711, 7759, 7760, 7761, 7829, 7830, 7831, 7899, 7900, 7901];
        dump(&["--part", "pam"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_boot_frames_and_plays_block_ones_music() {
        let run = run();
        numbered(&run, "pam", Some(7710), Some(("MUSIC0", 0, 7710)), Some((320, 200)));
        // The trace's row at 7710, which `dis_sync()` codes 10 (`OUTTAA.C:39`).
        let position = trace_position("second1b", 7710);
        assert_eq!(run.position(7710), position);
        let (order, row) = position.split_once(':').unwrap();
        assert_eq!(sr_engine::music::sync_from(order.parse().unwrap(), row.parse().unwrap()), 10);
    }

    #[test]
    fn the_support_frames_move_and_hold_as_the_references_do() {
        let run = run();
        assert_eq!(run.rgb(7711), run.rgb(7710), "7711 holds 7710's wash");
        assert_ne!(run.rgb(7759), run.rgb(7760), "7759 is the frame before");
        assert_eq!(run.rgb(7761), run.rgb(7760), "7761 holds 7760");
        moves_or_holds(&run, 7830, 7830, false);
        moves_or_holds(&run, 7900, 7900, false);
    }

    #[test]
    fn the_7900_index_plane_is_the_next_parts_white_field_not_pams_frame() {
        // The reference's 7900 plane is BEGLOGO's white field read late (A21); the port's is the
        // last frame the loop drew, `OUT.ANI`'s 39 (`if(f<=40)`). The two figures A21 quotes.
        let ani = std::fs::read(common::root().join("assets/PAM/OUT.ANI")).unwrap();
        let frames = sr_assets::load_frames(&ani, sr_assets::Format::Ani).unwrap();
        let frame39 = frames[39].pixels();
        assert_eq!(run().idx(7900), frame39, "the port's plane is animation frame 39");
        let distinct: std::collections::BTreeSet<u8> = frame39.iter().copied().collect();
        assert_eq!(distinct.len(), 64, "the animation frame's distinct indices");
        assert_eq!(frame39.iter().copied().max(), Some(209), "its highest index");
    }
}

/// PANICEND: no anchor; the handoff puts the reference at boot = 8782 + tick (`panicend.md`).
/// `second2` holds MUSIC1 order 37 across the start window. The part retires at 121 (`sleep(1)`)
/// and the run holds its screen, so 218 (boot 9000) is still a frame.
mod panicend {
    use super::*;

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
        let run = dump(&["--part", "panicend"], &[0, 10, 18, 121, 218]);
        numbered(&run, "panicend", None, Some(("MUSIC1", 37, 0)), Some((320, 200)));
    }
}

/// PLZPART: no anchor; boot = 5723 + tick (`plzpart.md` §2).
mod plzpart {
    use super::*;

    fn run() -> std::sync::Arc<Dump> {
        dump(&["--part", "plzpart"], &[0, 1, 2777])
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_declares_both_screens() {
        let run = run();
        numbered(&run, "plzpart", None, Some(("MUSIC1", 62, 0)), None);
        // `init_plz` sets the 320×400 view inside tick 0, and a tick is composed at the size
        // it started with, so the plasma's screen begins at tick 1. By 2777 the cube's
        // `tw_opengraph` has set 320×200.
        let sizes: Vec<(u64, u64)> = run.listing().iter().map(|frame| (frame.2, frame.3)).collect();
        assert_eq!(sizes, [(320, 200), (320, 400), (320, 200)]);
    }

    #[test]
    fn the_cube_keyframe_reads_the_traces_68_46() {
        // The cube's gate is pinned to boot 8364 by the eight-poll pre-roll, so at boot 8500
        // (tick 2777) the published position is the trace's 68:46; nine polls would read 68:47
        // (`plzpart.md` §8).
        assert_eq!(run().position(2777), "68:46");
    }
}

/// RAYSCRL: no anchor; boot = 12962 + tick (`rayscrl.md` §2).
mod rayscrl {
    use super::*;

    const ORIGIN: u32 = 12_962;
    const KEYFRAMES: [u32; 4] = [13_000, 13_500, 14_000, 14_500];

    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 7];
        frames.extend(with_neighbours(&KEYFRAMES.map(|index| index - ORIGIN)));
        dump(&["--part", "rayscrl"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
        let run = run();
        numbered(&run, "rayscrl", None, Some(("MUSIC1", 86, 0)), Some((320, 200)));
        // Tick 0 is the init wait's first poll, with the music already at the reference's
        // blit-time 88:0. Tick 7 is `:111-113`: BKG.CLX's pixels, rebased, under a black DAC.
        assert_eq!(run.position(0), "88:0");
        assert!(black(run.rgb(7)), "the blit's DAC is black");
        let bkg = std::fs::read(common::root().join("assets/WATER/BKG.CLX")).unwrap();
        assert_eq!(run.idx(7), &bkg[778..778 + 64_000]);
    }

    #[test]
    fn the_music_is_the_block_three_position_at_every_keyframe() {
        // The pre-roll walks the `254` at order-list entry 87 into order 88 with `zplus` 2, so
        // each mapped tick publishes `second3b`'s position (without it the init's
        // `musplus >= 0` wait would never pass).
        let run = run();
        for index in KEYFRAMES {
            assert_eq!(run.position(index - ORIGIN), trace_position("second3b", index), "{index}");
        }
    }

    #[test]
    fn each_keyframes_neighbours_hold_or_move_as_the_reference_does() {
        let run = run();
        let [fade, held, first, second] = KEYFRAMES.map(|index| index - ORIGIN);
        // 13000 is the hold of the k=14 upload: `:120-131` uploads on odd `y` only, so ticks 37
        // and 38 are one picture (the reference's 64-px 12999/13000 difference is the capture
        // catching the upload mid-loop, §15.3), and the next upload is another.
        assert_eq!(run.rgb(fade - 1), run.rgb(fade), "boot 13000: the upload's hold");
        assert_ne!(run.rgb(fade + 1), run.rgb(fade), "boot 13000: the next upload");
        // 13500 is the stage-2 hold; 14000 and 14500 are the main loop.
        moves_or_holds(&run, 13_500, held, false);
        moves_or_holds(&run, 14_000, first, true);
        moves_or_holds(&run, 14_500, second, true);
    }
}

/// TECHNO: no anchor (its first rings are a content-probed frame, 4797, §8.5). The keyframes
/// sit on three mappings (`techno.md` §2): the frame-quantised first pass at 4797 + tick, the
/// row-gated sections at 4799 + tick, and the troll, whose mode change costs a frame, at
/// 4800 + tick.
mod techno {
    use super::*;

    const FIRST_PASS_ORIGIN: u32 = 4797;
    const MUSIC_ORIGIN: u32 = 4799;
    const TROLL_ORIGIN: u32 = 4800;

    /// `(boot, tick, moving)`.
    const KEYFRAMES: [(u32, u32, bool); 7] = [
        (4900, 4900 - FIRST_PASS_ORIGIN, true),
        (5000, 5000 - FIRST_PASS_ORIGIN, false),
        (6300, 6300 - MUSIC_ORIGIN, true),
        (8100, 8100 - MUSIC_ORIGIN, true),
        (8200, 8200 - MUSIC_ORIGIN, false),
        (8300, 8300 - TROLL_ORIGIN, true),
        (8400, 8400 - TROLL_ORIGIN, false),
    ];

    fn run() -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 1, 103, 1501, 3396, 3397, 3600];
        frames.extend(with_neighbours(&KEYFRAMES.map(|(_, tick, _)| tick)));
        dump(&["--part", "techno"], &frames)
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_declares_the_music_row() {
        let run = run();
        numbered(&run, "techno", None, Some(("MUSIC1", 18, 0)), None);
        // The interference screen is 320×200 and the troll 320×400; a dump is sized from the
        // view as the tick starts, so the mode change during 3396 first shows at 3397.
        for (index, height) in [(0, 200), (1, 200), (103, 200), (1501, 200), (3396, 200), (3397, 400), (3600, 400)] {
            assert_eq!((run.frame(index).width, run.frame(index).height), (320, height), "{index}");
        }
    }

    /// `reference-indices.md` §9.3.
    #[test]
    fn the_keyframes_move_and_hold_as_the_references_do() {
        let run = run();
        for (index, tick, moving) in KEYFRAMES {
            moves_or_holds(&run, index, tick, moving);
        }
    }
}

/// TUNNELI: no anchor, so no filed reference index is one a `--part` run draws (`tunneli.md`
/// §7).
mod tunneli {
    use super::*;

    /// `TUN10.PAS:3` `const veke = 1060`: the part's last tick.
    const VEKE: u32 = 1060;

    fn run() -> std::sync::Arc<Dump> {
        dump(&["--part", "tunneli"], &[0, 10, 200, VEKE - 20, VEKE, VEKE + 1, 1200])
    }

    #[test]
    fn the_run_is_numbered_in_its_own_frames_and_draws_320x200() {
        // `:110-115` mode 13h.
        let run = run();
        numbered(&run, "tunneli", None, Some(("MUSIC1", 0, 0)), Some((320, 200)));
        // Tick 0 draws nothing: every ring's colour is `:130-135`'s zero (`:161`). By tick 200
        // the tunnel is up.
        assert!(black(run.idx(0)), "tick 0 is black");
        let ink = run.idx(200).iter().filter(|index| **index != 0).count();
        assert!(ink > 3_000, "tick 200 holds {ink} dotted pixels");
    }

    #[test]
    fn the_part_retires_at_veke_and_the_run_holds_its_last_black_screen() {
        // `:221`: the tick at `veke` is the last drawn, and black (`:220`).
        let run = run();
        assert!(black(run.idx(VEKE)), "the last drawn frame is black");
        assert!(!black(run.idx(VEKE - 20)), "tick 1,040 still has dots");
        for held in [VEKE + 1, 1200] {
            assert_eq!(run.idx(held), run.idx(VEKE), "{held} is not the screen the part left");
            assert_eq!(run.rgb(held), run.rgb(VEKE));
        }
    }
}

/// GRID, HARD and TWIST: not in the demo's sequence, so a run has no anchor and no player.
/// Port frame `j` is reference `K0 + j`: GRID's `K0` is 353 (source iteration 0, reference 352,
/// is drawn in `init`), HARD's 356 and TWIST's 3 (`reference-indices.md` §24.5-§24.7).
mod vault {
    use super::*;

    const GRID_K0: u32 = 353;
    const GRID_KEYFRAMES: [u32; 5] = [400, 500, 600, 700, 800];
    const TWIST_K0: u32 = 3;
    const TWIST_KEYFRAMES: [u32; 6] = [103, 203, 303, 403, 503, 603];
    const HARD_HOLD: [u32; 7] = [1898, 1899, 1900, 1901, 1902, 1903, 1920];
    const TWIST_CAP: [u32; 4] = [4442, 4443, 4444, 4500];

    fn run(part: &str) -> std::sync::Arc<Dump> {
        let mut frames = vec![0, 1, 10];
        match part {
            "grid" => frames.extend(with_neighbours(&GRID_KEYFRAMES.map(|index| index - GRID_K0))),
            "hard" => frames.extend(HARD_HOLD),
            "twist" => {
                frames.extend(with_neighbours(&TWIST_KEYFRAMES.map(|index| index - TWIST_K0)));
                frames.extend(TWIST_CAP);
            }
            _ => unreachable!(),
        }
        dump(&["--part", part], &frames)
    }

    #[test]
    fn a_part_run_of_each_vault_effect_numbers_itself_from_zero_and_is_silent() {
        // `music_for_part` is `None` for the vault.
        for part in ["grid", "hard", "twist"] {
            let run = run(part);
            numbered(&run, part, None, None, Some((320, 200)));
            assert!([0, 1, 10].iter().all(|index| run.frame(*index).music_position.is_none()));
        }
    }

    /// The differing pixels of the port's `R − K0 − 1`, `R − K0`, `R − K0 + 1` against the
    /// filed `R`, printed (the notes' ±1 columns) and minimal at offset 0 by D18's rule
    /// `(pixels, |offset|)`. Returns the offset-0 frame's differing pixels per row.
    fn the_minimum_is_at_offset_zero(part: &str, k0: u32, keyframe: u32) -> Vec<usize> {
        let run = run(part);
        let expected = reference(part, keyframe, "rgb");
        let tick = keyframe - k0;
        let counts: Vec<(i32, usize)> = [-1, 0, 1]
            .into_iter()
            .map(|offset| (offset, differing(run.rgb(tick.wrapping_add_signed(offset)), &expected, 3)))
            .collect();
        println!("{part} {keyframe}: offsets -1/0/+1 give {counts:?}");
        let best = counts.iter().min_by_key(|(offset, pixels)| (*pixels, offset.abs())).unwrap();
        assert_eq!(best.0, 0, "{part} {keyframe}: {counts:?}");
        let rendered = run.rgb(tick);
        (0..200)
            .map(|row| differing(&rendered[row * 960..(row + 1) * 960], &expected[row * 960..(row + 1) * 960], 3))
            .collect()
    }

    #[test]
    fn grid_matches_the_reference_at_its_measured_start() {
        // A41: every residual pixel is on rows 14–15, the topmost rows the curve reaches.
        for keyframe in GRID_KEYFRAMES {
            let rows = the_minimum_is_at_offset_zero("grid", GRID_K0, keyframe);
            let off: Vec<usize> = (0..200).filter(|row| rows[*row] != 0).collect();
            assert!(off.iter().all(|row| *row == 14 || *row == 15), "{keyframe}: rows {off:?}");
        }
    }

    #[test]
    fn twist_matches_the_reference_at_its_measured_start() {
        for keyframe in TWIST_KEYFRAMES {
            the_minimum_is_at_offset_zero("twist", TWIST_K0, keyframe);
        }
    }

    #[test]
    fn hard_draws_the_whole_effect_and_holds() {
        // Part 3's last drawn frame is its count 800 (port 1900, reference 2256). Mode 7's one
        // iteration and the `default` arm draw nothing (1901, 1902), and the run holds the
        // picture; `B.C:325-329`'s text-mode restore is the window's close, not a frame.
        let run = run("hard");
        assert!(HARD_HOLD.iter().all(|index| run.part(*index) == "hard"));
        assert_ne!(run.rgb(1899), run.rgb(1900), "the cube still moves on count 799");
        for hold in [1901, 1902, 1903, 1920] {
            assert_eq!(run.rgb(hold), run.rgb(1900), "{hold} is the hold after the last cube");
            assert_eq!(run.idx(hold), run.idx(1900), "{hold} holds the indexed frame too");
        }
    }

    #[test]
    fn twist_draws_exactly_4444_frames_and_holds() {
        // `while(!kbhit() && frame<4444)`: the 4,445th check returns `Finished` before drawing,
        // so 4,444 and 4,500 are the picture 4,443 left.
        let run = run("twist");
        assert!(TWIST_CAP.iter().all(|index| run.part(*index) == "twist"));
        assert_ne!(run.rgb(4442), run.rgb(4443), "4443 is the last drawn frame, a new picture");
        for hold in [4444, 4500] {
            assert_eq!(run.rgb(hold), run.rgb(4443), "{hold} is the hold, not a 4,445th frame");
            assert_eq!(run.idx(hold), run.idx(4443), "{hold} holds the indexed frame too");
        }
    }
}
