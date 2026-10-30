//! What `cargo xtask rediff` re-renders and what it expects back: one entry per filed
//! capture under `reference/parts`.
//!
//! **Every mapping and every count here is copied from the test or record that asserts
//! it, never fitted** (Phase 9 Task 5). Each entry names its source. Where a part's test
//! compares a frame, the row is that test's pairing — its run arguments, its
//! reference→rendered rule, its plane — and that test's asserted count. Where no test
//! asserts a pixel count (CRED's bands, PANICEND's standalone frames, ENDSCRL, TUNNELI,
//! PLZPART's percentages and its fit corpus, the lead-in windows), the count is the one
//! this command measured, and `register` names the A-entry whose figure it reproduces (A5, A7
//! and A23 were restated at these counts, and PANICEND's handoff registered as A63, in Phase 9's
//! register pass).
//!
//! This file is self-contained (no imports) because `xtask/tests/rediff.rs` includes it by
//! path to check it against the tree: `xtask` is a binary crate.

/// Which plane a frame is compared in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Plane {
    /// The `.idx` planes, one palette index a pixel — the stricter comparison, used where
    /// the test compares indices (the reference must file a `.idx` for the frame).
    Indexed,
    /// The `.rgb` frames, any channel differing counting the pixel once.
    Rgb,
}

/// One filed capture and the run that answers it.
#[derive(Debug, Clone, Copy)]
pub struct Entry {
    /// The capture's directory below `reference/parts`, `/`-separated: `"loop/cred"`.
    pub capture: &'static str,
    /// The binary's arguments before `--headless --dump-frames … --dump-dir …`:
    /// `["--part", "mntscrl"]`.
    pub args: &'static [&'static str],
    /// Every filed frame of the capture, at least once; a frame both of whose planes a
    /// test compares is listed once per plane.
    pub frames: &'static [Frame],
}

/// One comparison: the capture's frame `reference` against the run's frame `rendered`.
#[derive(Debug, Clone, Copy)]
pub struct Frame {
    pub reference: u32,
    pub rendered: u32,
    pub plane: Plane,
    /// The differing-pixel count the comparison must produce, exactly.
    pub registered: u32,
    /// `"exact"` for a frame that matches because nothing stands between it and the
    /// reference; otherwise the entry that registers the difference or the pairing —
    /// `"A41"`, or a decision such as `"D42"` (whose pairing is what makes a count 0).
    pub register: &'static str,
}

/// Filed captures whose mapping a later task measures. `rediff` names each capture under
/// one of these in its report as not compared. Empty since Phase 9 Task 6 measured the
/// last of them (`fullrun`); the mechanism stays, for the next capture filed before its
/// mapping is.
pub const PENDING: &[&str] = &[];

/// How far, in frames, a published music position may be from the frame a filed trace
/// publishes it at. **Measured, not chosen**: the two filed sessions of one recipe
/// (`second5`, `second5b`) disagree with each other about the frame some of their row changes
/// land on, the port, whose clock is exact, sits between them, and its worst displacement
/// against `second5` is 3 frames (A12). The shell's `second5` test and `fullrun.rs` both hold
/// the port to it; it lives here because both include this file.
#[allow(dead_code)]
pub const JITTER_FRAMES: u32 = 3;

const fn rgb(reference: u32, rendered: u32, registered: u32, register: &'static str) -> Frame {
    Frame {
        reference,
        rendered,
        plane: Plane::Rgb,
        registered,
        register,
    }
}

const fn idx(reference: u32, rendered: u32, registered: u32, register: &'static str) -> Frame {
    Frame {
        reference,
        rendered,
        plane: Plane::Indexed,
        registered,
        register,
    }
}

/// An RGB frame the test asserts is byte-identical.
const fn rgb0(reference: u32, rendered: u32) -> Frame {
    rgb(reference, rendered, 0, "exact")
}

/// An index plane the test asserts is byte-identical.
const fn idx0(reference: u32, rendered: u32) -> Frame {
    idx(reference, rendered, 0, "exact")
}

/// One part of block 1's run, as the seam table of `reference-indices.md` §26.8 holds it: the
/// original's first frame by §26.3's definition (in the session the part's `fullrun` capture was
/// filed in), the port's first frame by the same definition, and the index the port starts the
/// part at (`Sequence::starts`). **This is the one source of the full run's start differences**:
/// the `fullrun/*` entries above derive every rendered index from it, and
/// `bins/secondreality/tests/fullrun.rs` includes this file and asserts both port columns
/// against the run's own frames and timeline.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)] // `part` is read by the tests that include this file
pub struct Seam {
    pub part: &'static str,
    pub original: u32,
    pub port: u32,
    pub start: u32,
}

/// Block 1's nineteen parts, in the demo's order (`MAIN/U2.ASM:789-912`).
pub const FULLRUN_SEAMS: [Seam; 19] = [
    Seam { part: "alku", original: 404, port: 404, start: 404 },
    Seam { part: "u2a", original: 5961, port: 5956, start: 5956 },
    Seam { part: "pam", original: 7707, port: 7704, start: 7508 },
    Seam { part: "beglogo", original: 7922, port: 7885, start: 7885 },
    Seam { part: "glenz", original: 8522, port: 8527, start: 8057 },
    Seam { part: "tunneli", original: 11158, port: 11151, start: 11131 },
    Seam { part: "techno", original: 12214, port: 12216, start: 12192 },
    Seam { part: "panicend", original: 16193, port: 16181, start: 16181 },
    Seam { part: "mntscrl", original: 16637, port: 16633, start: 16633 },
    Seam { part: "lnszoom", original: 18656, port: 18670, start: 18669 },
    Seam { part: "plzpart", original: 21927, port: 21928, start: 21853 },
    Seam { part: "minvball", original: 26673, port: 26670, start: 26569 },
    Seam { part: "rayscrl", original: 29174, port: 29180, start: 29150 },
    Seam { part: "3dsinfld", original: 31272, port: 31288, start: 31224 },
    Seam { part: "jplogo", original: 33263, port: 33255, start: 33255 },
    Seam { part: "u2e", original: 34120, port: 34075, start: 34043 },
    Seam { part: "endlogo", original: 37967, port: 37956, start: 37956 },
    Seam { part: "cred", original: 38583, port: 38564, start: 38564 },
    Seam { part: "endscrl", original: 46277, port: 46189, start: 46187 },
];

/// An index into [`FULLRUN_SEAMS`], so a row names its part rather than a number.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Part {
    Alku,
    U2a,
    Pam,
    Beglogo,
    Glenz,
    Tunneli,
    Techno,
    Panicend,
    Mntscrl,
    Lnszoom,
    Plzpart,
    Minvball,
    Rayscrl,
    /// 3DSINFLD (`coman.rs`).
    Coman,
    Jplogo,
    U2e,
    Endlogo,
    Cred,
    Endscrl,
}

/// The port's index answering the original's `reference` in `part`: plus the part's start
/// difference.
pub const fn at(reference: u32, part: Part) -> u32 {
    let seam = FULLRUN_SEAMS[part as usize];
    reference + seam.port - seam.original
}

/// The port's last frame before `next` starts: where a frame the original shows during the
/// loader stretch before `next` is answered, the port having no such stretch.
pub const fn before(next: Part) -> u32 {
    FULLRUN_SEAMS[next as usize].start - 1
}

/// The frames the original shows during a loader stretch the port does not have, and the part
/// that starts after it: PAM's white under BEGLOGO's load, the dot after the cue wait's exit,
/// JPLOGO's white under the U2E reload (`reference-indices.md` §26.8).
#[allow(dead_code)]
pub const FULLRUN_LOADER_FRAMES: [(u32, Part); 3] =
    [(7920, Part::Beglogo), (16636, Part::Mntscrl), (34110, Part::U2e)];

/// Frames paired at their own part's difference whose rendered index the port has already
/// handed to the next part: BEGLOGO's held picture (8300; §26.3 gives BEGLOGO's last frame as
/// 8520), which the port holds under GLENZ's wait from 8057 on.
#[allow(dead_code)]
pub const FULLRUN_ACROSS_HANDOVER: [u32; 1] = [8300];

const fn rgb_at(reference: u32, part: Part, registered: u32, register: &'static str) -> Frame {
    rgb(reference, at(reference, part), registered, register)
}

const fn idx_at(reference: u32, part: Part, registered: u32, register: &'static str) -> Frame {
    idx(reference, at(reference, part), registered, register)
}

const fn rgb_before(reference: u32, next: Part, registered: u32, register: &'static str) -> Frame {
    rgb(reference, before(next), registered, register)
}

const fn idx_before(reference: u32, next: Part, registered: u32, register: &'static str) -> Frame {
    idx(reference, before(next), registered, register)
}

pub const TABLE: &[Entry] = &[
    // tests/coman.rs:129 KEYFRAMES (reference, tick), RGB, 0 px and delta 0 (:184-185).
    Entry {
        capture: "3dsinfld",
        args: &["--part", "3dsinfld"],
        frames: &[
            rgb0(15500, 464),
            rgb0(16000, 966),
            rgb0(16500, 1470),
            rgb0(16800, 1772),
        ],
    },
    // tests/alku.rs: boot numbering (anchor 404), 900/1719/2719 at 0 RGB px, and 5859 at 0
    // since the port runs the release's loop (the fifth batch stops the text, A20).
    Entry {
        capture: "alku",
        args: &["--part", "alku"],
        frames: &[
            rgb0(900, 900),
            rgb0(1719, 1719),
            rgb0(2719, 2719),
            rgb0(5859, 5859),
        ],
    },
    // tests/beglogo.rs:16 KEYFRAMES at their own indices: 430 all index 15 and all white
    // (:107-111, the reference's 430 is the same field, A5), 470/560 RGB at 0 (:126-133),
    // 900 the picture's indices (:154-157, byte-identical to the filed `.idx`, :151).
    Entry {
        capture: "beglogo",
        args: &["--part", "beglogo"],
        frames: &[
            idx0(430, 430),
            rgb0(430, 430),
            rgb0(470, 470),
            rgb0(560, 560),
            idx0(900, 900),
        ],
    },
    // The lead-in window (reference-indices.md §25.6, captured `--arg 2`) at BEGLOGO's own
    // boot numbering (tests/beglogo.rs:34). No test asserts these; the counts are this
    // command's, and every one reproduces A5's "zero on every frame the reference holds".
    Entry {
        capture: "beglogo-leadin/even",
        args: &["--part", "beglogo"],
        frames: &[
            rgb(430, 430, 0, "A5"),
            rgb(432, 432, 0, "A5"),
            rgb(434, 434, 0, "A5"),
            rgb(436, 436, 0, "A5"),
            rgb(438, 438, 0, "A5"),
            rgb(440, 440, 0, "A5"),
            rgb(442, 442, 0, "A5"),
            rgb(444, 444, 0, "A5"),
            rgb(446, 446, 0, "A5"),
            rgb(448, 448, 0, "A5"),
            rgb(450, 450, 0, "A5"),
        ],
    },
    Entry {
        capture: "beglogo-leadin/odd",
        args: &["--part", "beglogo"],
        frames: &[
            rgb(431, 431, 0, "A5"),
            rgb(433, 433, 0, "A5"),
            rgb(435, 435, 0, "A5"),
            rgb(437, 437, 0, "A5"),
            rgb(439, 439, 0, "A5"),
            rgb(441, 441, 0, "A5"),
            rgb(443, 443, 0, "A5"),
            rgb(445, 445, 0, "A5"),
            rgb(447, 447, 0, "A5"),
            rgb(449, 449, 0, "A5"),
        ],
    },
    // §25.6: the field (index 15) at the port's own blit index, 436 (A5: the port blits at
    // tick 32), filed unbatched for its index plane. The RGB is white on both sides; the
    // port's plane already holds the picture (all but 2 of its 128,000 indices are not
    // 15), which A5 predicted and now registers as its restated index-plane figure.
    Entry {
        capture: "beglogo-leadin/plane-436",
        args: &["--part", "beglogo"],
        frames: &[
            idx(436, 436, 127998, "A5"),
            rgb(436, 436, 0, "A5"),
        ],
    },
    // tests/cred.rs:22 KEYFRAMES through the block-5 chain (:40), every one at 0 bytes in
    // the band, the caption and the frame (`the_five_keyframes_are_the_references_byte_for_byte`)
    // since the screens are read from the shipped CRED.EXE (A10, retired; they were 7,204 /
    // 6,088 / 3,741 at 3240 / 5440 / 8740 from the tree's data).
    Entry {
        capture: "cred",
        args: &["--block", "5"],
        frames: &[
            rgb0(1440, 1440),
            rgb0(3240, 3240),
            rgb0(5440, 5440),
            rgb0(7640, 7640),
            rgb0(8740, 8740),
        ],
    },
    // tests/ddstars.rs:102 boot = 414 + tick; every keyframe 0 bytes (:169-177).
    Entry {
        capture: "ddstars",
        args: &["--part", "ddstars"],
        frames: &[
            rgb0(1900, 1486),
            rgb0(3000, 2586),
            rgb0(5000, 4586),
            rgb0(8000, 7586),
        ],
    },
    // tests/endlogo.rs:17 KEYFRAMES at their own indices: 430's indices (:115-119),
    // 500/1180 RGB (:138-146), 900's indices (:175).
    Entry {
        capture: "endlogo",
        args: &["--part", "endlogo"],
        frames: &[
            idx0(430, 430),
            rgb0(500, 500),
            idx0(900, 900),
            rgb0(1180, 1180),
        ],
    },
    // The lead-in window (§25.6, captured `--arg 5`) at ENDLOGO's own boot numbering
    // (tests/endlogo.rs:35); the counts are this command's, and every one reproduces A7's
    // "zero on every frame the reference holds".
    Entry {
        capture: "endlogo-leadin/even",
        args: &["--part", "endlogo"],
        frames: &[
            rgb(436, 436, 0, "A7"),
            rgb(438, 438, 0, "A7"),
            rgb(440, 440, 0, "A7"),
            rgb(442, 442, 0, "A7"),
            rgb(444, 444, 0, "A7"),
            rgb(446, 446, 0, "A7"),
            rgb(448, 448, 0, "A7"),
            rgb(450, 450, 0, "A7"),
            rgb(452, 452, 0, "A7"),
            rgb(454, 454, 0, "A7"),
        ],
    },
    Entry {
        capture: "endlogo-leadin/odd",
        args: &["--part", "endlogo"],
        frames: &[
            rgb(437, 437, 0, "A7"),
            rgb(439, 439, 0, "A7"),
            rgb(441, 441, 0, "A7"),
            rgb(443, 443, 0, "A7"),
            rgb(445, 445, 0, "A7"),
            rgb(447, 447, 0, "A7"),
            rgb(449, 449, 0, "A7"),
            rgb(451, 451, 0, "A7"),
            rgb(453, 453, 0, "A7"),
            rgb(455, 455, 0, "A7"),
        ],
    },
    // §25.6: the field (index 0) at the port's blit index, 440 (A7: tick 36). The RGB is
    // white on both sides; the port's plane holds the picture, 55,499 of whose indices are
    // not 0 — A7's prediction, and its restated index-plane figure.
    Entry {
        capture: "endlogo-leadin/plane-440",
        args: &["--part", "endlogo"],
        frames: &[
            idx(440, 440, 55499, "A7"),
            rgb(440, 440, 0, "A7"),
        ],
    },
    // tests/endscrl.rs:20 KEYFRAMES through `--block 5` (:52), at their own indices. Since the
    // part reads the release's image (D49) every count is the chain's 82-frame offset alone:
    // `parts::endscrl`'s `the_port_draws_the_references_frames_at_the_references_own_ticks`
    // renders all four at 0 from the release's own start (8917) and these four from the
    // port's (8835) — A11's residual.
    Entry {
        capture: "endscrl",
        args: &["--block", "5"],
        frames: &[
            rgb(9100, 9100, 7217, "A11"),
            rgb(10000, 10000, 32508, "A11"),
            rgb(12000, 12000, 25118, "A11"),
            rgb(14000, 14000, 17613, "A11"),
        ],
    },
    // The original's whole `SECOND 1` run (reference-indices.md §26) against the port's
    // `--block 1`. Every rendered index is derived, never written: `rgb_at`/`idx_at` put a
    // frame at its reference index plus **its part's start difference** (FULLRUN_SEAMS,
    // below the table — the one source tests/fullrun.rs asserts against the run's frames),
    // and `rgb_before`/`idx_before` put the three frames the original shows during a loader
    // stretch the port does not have (FULLRUN_LOADER_FRAMES) on the port's last frame
    // before the next part starts. xtask/tests/rediff.rs checks every fullrun row against
    // that rule. Held frames compare at 0 but for the two that carry their part's own
    // registered count (19000 is A25's 2,058; 40600 was A10's 7,204 until CRED's screens
    // were read from its image, and is 0); every other count is
    // this command's and is A53's, which also names every pairing at a non-zero difference.
    Entry {
        capture: "fullrun/01-alku",
        args: &["--block", "1"],
        frames: &[
            rgb_at(1719, Part::Alku, 0, "exact"),
        ],
    },
    // 5961 is the original's first 320×200 frame — ALKU's closing page, or U2A's identical
    // background — under the 30,000-cycle session's DAC; the port's U2A starts 5 frames
    // earlier on the same picture under the merged palette (A20, A32, A53). 7880's
    // index plane is PAM's picture under a white DAC; 7920 is PAM's white under BEGLOGO's
    // load.
    Entry {
        capture: "fullrun/02-u2a-pam",
        args: &["--block", "1"],
        frames: &[
            rgb_at(5961, Part::U2a, 60160, "A53"),
            rgb_at(6500, Part::U2a, 64000, "A53"),
            rgb_at(7707, Part::Pam, 0, "A53"),
            idx_at(7707, Part::Pam, 0, "A53"),
            rgb_at(7880, Part::Pam, 0, "A53"),
            idx_at(7880, Part::Pam, 48786, "A53"),
            rgb_before(7920, Part::Beglogo, 0, "A53"),
        ],
    },
    // BEGLOGO's −37 is A5's lead-in; its ramp is frame-locked — the port's 7930 is the
    // original's byte for byte (tests/fullrun.rs) — so at −37 it is 81,273 px off. 8300 is
    // BEGLOGO's held picture by §26.3 (its last frame is 8520), paired at BEGLOGO's
    // difference, and the port shows that hold under GLENZ's wait: FULLRUN_ACROSS_HANDOVER.
    Entry {
        capture: "fullrun/03-beglogo",
        args: &["--block", "1"],
        frames: &[
            rgb_at(7930, Part::Beglogo, 81273, "A53"),
            rgb_at(8300, Part::Beglogo, 0, "A53"),
            idx_at(8300, Part::Beglogo, 0, "A53"),
            rgb_at(8522, Part::Glenz, 0, "A53"),
        ],
    },
    Entry {
        capture: "fullrun/04-glenz-techno",
        args: &["--block", "1"],
        frames: &[
            rgb_at(8572, Part::Glenz, 0, "A53"),
            idx_at(8572, Part::Glenz, 0, "A53"),
            rgb_at(8900, Part::Glenz, 0, "A53"),
            idx_at(8900, Part::Glenz, 0, "A53"),
            rgb_at(11159, Part::Tunneli, 192, "A53"),
            rgb_at(11900, Part::Tunneli, 4336, "A53"),
            rgb_at(12214, Part::Techno, 0, "A53"),
            rgb_at(12400, Part::Techno, 0, "A53"),
        ],
    },
    Entry {
        capture: "fullrun/05-troll-panicend",
        args: &["--block", "1"],
        frames: &[
            rgb_at(16000, Part::Techno, 0, "A53"),
            rgb_at(16193, Part::Panicend, 108923, "A53"),
        ],
    },
    // 16636 is the dot after the cue's exit; LNS&ZOOM's hold carries A25's 2,058 in both
    // planes.
    Entry {
        capture: "fullrun/06-panicend-lnszoom",
        args: &["--block", "1"],
        frames: &[
            rgb_at(16500, Part::Panicend, 0, "A53"),
            idx_at(16500, Part::Panicend, 0, "A53"),
            rgb_before(16636, Part::Mntscrl, 0, "A53"),
            rgb_at(16637, Part::Mntscrl, 0, "A53"),
            rgb_at(16690, Part::Mntscrl, 0, "A53"),
            rgb_at(18656, Part::Lnszoom, 129, "A53"),
            rgb_at(19000, Part::Lnszoom, 2058, "A25"),
            idx_at(19000, Part::Lnszoom, 2058, "A25"),
        ],
    },
    Entry {
        capture: "fullrun/07-plasma",
        args: &["--block", "1"],
        frames: &[
            rgb_at(21927, Part::Plzpart, 0, "A53"),
            rgb_at(24500, Part::Plzpart, 45808, "A53"),
        ],
    },
    Entry {
        capture: "fullrun/08-cube-3dsinfld",
        args: &["--block", "1"],
        frames: &[
            rgb_at(24540, Part::Plzpart, 0, "A53"),
            idx_at(24540, Part::Plzpart, 48384, "A53"),
            rgb_at(26673, Part::Minvball, 0, "A53"),
            rgb_at(26700, Part::Minvball, 0, "A53"),
            rgb_at(29174, Part::Rayscrl, 875, "A53"),
            rgb_at(29800, Part::Rayscrl, 0, "A53"),
            idx_at(29800, Part::Rayscrl, 0, "A53"),
            rgb_at(31272, Part::Coman, 4, "A53"),
            rgb_at(31901, Part::Coman, 33554, "A53"),
            idx_at(31901, Part::Coman, 34036, "A53"),
        ],
    },
    // 34110 is JPLOGO's white under the U2E reload: the same indices as the hold (the white is
    // the DAC's), not the same colours.
    Entry {
        capture: "fullrun/09-jplogo",
        args: &["--block", "1"],
        frames: &[
            rgb_at(33263, Part::Jplogo, 0, "A53"),
            idx_at(33263, Part::Jplogo, 0, "A53"),
            rgb_at(33900, Part::Jplogo, 0, "A53"),
            idx_at(33900, Part::Jplogo, 0, "A53"),
            rgb_before(34110, Part::U2e, 72363, "A53"),
            idx_before(34110, Part::U2e, 0, "A53"),
        ],
    },
    Entry {
        capture: "fullrun/10-u2e",
        args: &["--block", "1"],
        frames: &[
            rgb_at(34120, Part::U2e, 0, "A53"),
            rgb_at(34250, Part::U2e, 0, "A53"),
            idx_at(34250, Part::U2e, 0, "A53"),
        ],
    },
    // 37970's index plane is the video memory U2E left, under ENDLOGO's white DAC (`END.C` has
    // no memset).
    Entry {
        capture: "fullrun/11-endlogo",
        args: &["--block", "1"],
        frames: &[
            rgb_at(37970, Part::Endlogo, 0, "A53"),
            idx_at(37970, Part::Endlogo, 46997, "A53"),
            rgb_at(38300, Part::Endlogo, 0, "A53"),
            idx_at(38300, Part::Endlogo, 0, "A53"),
        ],
    },
    // Its 40600 hold is `cred`'s 3240 (screen 6), at 0 since the screens are the image's.
    Entry {
        capture: "fullrun/12-cred",
        args: &["--block", "1"],
        frames: &[
            rgb_at(38583, Part::Cred, 0, "A53"),
            rgb_at(40600, Part::Cred, 0, "A53"),
        ],
    },
    Entry {
        capture: "fullrun/13-endscrl",
        args: &["--block", "1"],
        frames: &[
            rgb_at(46280, Part::Endscrl, 0, "A53"),
            rgb_at(46283, Part::Endscrl, 0, "A53"),
            rgb_at(46286, Part::Endscrl, 0, "A53"),
        ],
    },
    // tests/glenz.rs, block 2 (:90): 1499 at 0 in both planes (:420-431); the moving
    // keyframes at the capture's surface minima, K+5 for 2300 and K+4 for 3000/3400
    // (:461-479: 5208 / 2289 / 5941, A37).
    Entry {
        capture: "glenz",
        args: &["--block", "2"],
        frames: &[
            idx0(1499, 1499),
            rgb0(1499, 1499),
            rgb(2300, 2305, 5208, "A37"),
            rgb(3000, 3004, 2289, "A37"),
            rgb(3400, 3404, 5941, "A37"),
        ],
    },
    // tests/vault.rs:46 K0 = 353, :54 GRID_RESIDUALS at offset 0 (A41).
    Entry {
        capture: "grid",
        args: &["--part", "grid"],
        frames: &[
            rgb(400, 47, 6, "A41"),
            rgb(500, 147, 2, "A41"),
            rgb(600, 247, 36, "A41"),
            rgb(700, 347, 33, "A41"),
            rgb(800, 447, 14, "A41"),
        ],
    },
    // tests/vault.rs:71 K0 = 356, :81 HARD_RESIDUALS all 0 at offset 0, and 840's filed
    // `.idx` byte for byte (:250-260).
    Entry {
        capture: "hard",
        args: &["--part", "hard"],
        frames: &[
            rgb0(480, 124),
            rgb0(560, 204),
            rgb0(630, 274),
            rgb0(700, 344),
            rgb0(770, 414),
            rgb0(840, 484),
            idx0(840, 484),
            rgb0(910, 554),
            rgb0(1050, 694),
            rgb0(1600, 1244),
        ],
    },
    // tests/jplogo.rs:127 KEYFRAMES (reference, tick): 17300/17500 at 0 (:196-199), 17700
    // at 51,445 against the filing no census session holds (:217, A27).
    Entry {
        capture: "jplogo",
        args: &["--part", "jplogo"],
        frames: &[
            rgb0(17300, 235),
            rgb0(17500, 435),
            rgb(17700, 635, 51445, "A27"),
        ],
    },
    // tests/lnszoom.rs:96 boot = 2452 + tick: 2700 at 2,058 in both planes (:209, :211-217),
    // 3400 at 1,935 (:233), 4200 at 0 (:244), 4900 at 1,396 (:267), 5650 RGB at 0 (:275) —
    // its `.idx` is pinned as a memory read, not compared (:282-284). A25.
    Entry {
        capture: "lnszoom",
        args: &["--part", "lnszoom"],
        frames: &[
            rgb(2700, 248, 2058, "A25"),
            idx(2700, 248, 2058, "A25"),
            rgb(3400, 948, 1935, "A25"),
            rgb0(4200, 1748),
            rgb(4900, 2448, 1396, "A25"),
            rgb0(5650, 3198),
        ],
    },
    // tests/loop.rs: `--block 5 --loop` (:62), the port's `C_port + (n − C)` = n − 196
    // (:33, :37, :51), every seam frame at 0 (:171-183). The original's `L` = 9104 lands
    // on the port's 8908, still CRED's 320×200 screen (:177) — a geometry mismatch, never a
    // registered outcome — so it is paired with the port's own `L`, 8910, which the test
    // asserts is 9104 byte for byte (:189-195): A61's 2-tick CRED tail (D42).
    Entry {
        capture: "loop/alku",
        args: &["--block", "5", "--loop"],
        frames: &[
            rgb(9104, 8910, 0, "A61"),
            rgb0(9600, 9404),
            rgb0(10419, 10223),
            rgb0(11419, 11223),
        ],
    },
    Entry {
        capture: "loop/cred",
        args: &["--block", "5", "--loop"],
        frames: &[rgb0(9029, 8833), rgb0(9055, 8859), rgb0(9081, 8885)],
    },
    // tests/minvball.rs boot = 10,368 + tick; KEYFRAMES' differing pixels, all four 0 since the
    // z rotation stopped borrowing from the previous dot's `bx` (bugs.md §18.1, A19).
    Entry {
        capture: "minvball",
        args: &["--part", "minvball"],
        frames: &[
            rgb0(10900, 532),
            rgb0(11200, 832),
            rgb0(11800, 1432),
            rgb0(12400, 2032),
        ],
    },
    // tests/mntscrl.rs:96 tick = index − 419: 500 at 0 in both planes (:200-209), the three
    // scroll keyframes RGB at 0 (:213-219).
    Entry {
        capture: "mntscrl",
        args: &["--part", "mntscrl"],
        frames: &[
            idx0(500, 81),
            rgb0(500, 81),
            rgb0(1100, 681),
            rgb0(1700, 1281),
            rgb0(2300, 1881),
        ],
    },
    // tests/pam.rs: anchor 7710, the four keyframes RGB at 0 (:169-178); 7900's index plane
    // is BEGLOGO's white field, 60,375 indices off PAM's last frame (:253, A21).
    Entry {
        capture: "pam",
        args: &["--part", "pam"],
        frames: &[
            rgb0(7710, 7710),
            rgb0(7760, 7760),
            rgb0(7830, 7830),
            rgb0(7900, 7900),
            idx(7900, 7900, 60375, "A21"),
        ],
    },
    // tests/panicend.rs: boot = 8782 + tick (module docs), 9000 at tick 218 in both planes
    // (:138-152). 8792/8800 are the handoff's frames, which a standalone run cannot
    // reproduce; the test pins them at ticks 10/18 as 4,914 and 702 *bytes* (:178), and
    // the counts here are this command's — A63, the standalone run's limitation.
    Entry {
        capture: "panicend",
        args: &["--part", "panicend"],
        frames: &[
            rgb(8792, 10, 1638, "A63"),
            rgb(8800, 18, 234, "A63"),
            rgb0(9000, 218),
            idx0(9000, 218),
        ],
    },
    // The fit corpus (reference-indices.md §1.13) at the part's mapping, tick = index −
    // 5723 (tests/plzpart.rs:93). It is not a keyframe set; the counts are this command's, and
    // A23 registers them. With the release's camera path and sine table read from its image,
    // 8500 and 8600 are exact; the four others differ in both-set pixels of one face at a
    // channel delta of at most 9, where the reference shows one index in two colours.
    Entry {
        capture: "plzpart/band",
        args: &["--part", "plzpart"],
        frames: &[
            rgb(8400, 2677, 340, "A23"),
            rgb(8450, 2727, 24, "A23"),
            rgb0(8500, 2777),
            rgb0(8600, 2877),
            rgb(8700, 2977, 148, "A23"),
            rgb(8800, 3077, 4, "A23"),
        ],
    },
    // tests/plzpart.rs `the_cube_keyframes_match_the_reference`: both keyframes are the
    // reference's byte for byte since the release's camera path is read from its image (A23).
    Entry {
        capture: "plzpart/cube",
        args: &["--part", "plzpart"],
        frames: &[rgb0(8500, 2777), rgb0(9500, 3777)],
    },
    // tests/plzpart.rs:305-328: 11.209 / 5.848 / 4.052 % of 128,000 to 0.001 points (A24);
    // the counts are this command's — 11.2094 / 5.8484 / 4.0516 % — inside that window.
    Entry {
        capture: "plzpart/plasma",
        args: &["--part", "plzpart"],
        frames: &[
            rgb(6000, 277, 14348, "A24"),
            rgb(7000, 1277, 7486, "A24"),
            rgb(7600, 1877, 5186, "A24"),
        ],
    },
    // tests/rayscrl.rs:108 boot = 12962 + tick: 13000 at 0 (:198-205), 13500 at 0 in both
    // planes (:226-238), 14000/14500 at 3/42 pixels (:252-258, A18).
    Entry {
        capture: "rayscrl",
        args: &["--part", "rayscrl"],
        frames: &[
            rgb0(13000, 38),
            idx0(13500, 538),
            rgb0(13500, 538),
            rgb(14000, 1038, 3, "A18"),
            rgb(14500, 1538, 42, "A18"),
        ],
    },
    // tests/techno.rs:102-104 the three origins, :188-256 every keyframe at 0: 4900/5000
    // on the first pass's 4797, 6300/8100/8200 on the music's 4799, 8300/8400 on the
    // troll's 4800.
    Entry {
        capture: "techno/interference",
        args: &["--part", "techno"],
        frames: &[
            rgb0(4900, 103),
            rgb0(5000, 203),
            rgb0(6300, 1501),
            rgb0(8100, 3301),
        ],
    },
    Entry {
        capture: "techno/troll",
        args: &["--part", "techno"],
        frames: &[rgb0(8200, 3401), rgb0(8300, 3500), rgb0(8400, 3600)],
    },
    // docs/fidelity/tunneli.md:30 and A6: offset 3,722 (port ticks 78/478/778/878). No test
    // compares these frames; the counts are this command's, against A6's 179 / 373 / 321 /
    // 433.
    Entry {
        capture: "tunneli",
        args: &["--part", "tunneli"],
        frames: &[
            rgb(3800, 78, 179, "A6"),
            rgb(4200, 478, 373, "A6"),
            rgb(4500, 778, 321, "A6"),
            rgb(4600, 878, 433, "A6"),
        ],
    },
    // tests/vault.rs:59 K0 = 3, :66 TWIST_RESIDUALS all 0 at offset 0.
    Entry {
        capture: "twist",
        args: &["--part", "twist"],
        frames: &[
            rgb0(103, 100),
            rgb0(203, 200),
            rgb0(303, 300),
            rgb0(403, 400),
            rgb0(503, 500),
            rgb0(603, 600),
        ],
    },
    // tests/u2a.rs, block 1 (:55): 6500 RGB at 64,000 (the 30,000-cycle neon frame, A32),
    // 7300 at 1,098 in both planes (A29), 7640 at 0 in both planes.
    Entry {
        capture: "u2a",
        args: &["--block", "1"],
        frames: &[
            rgb(6500, 6500, 64000, "A32"),
            idx(7300, 7300, 1098, "A29"),
            rgb(7300, 7300, 1098, "A29"),
            idx0(7640, 7640),
            rgb0(7640, 7640),
        ],
    },
    // The original at 15000 cycles (reference-indices.md §27): U2A's pre-gate hold is the
    // background under the merged palette, which the port shows (A32; tests/u2a.rs).
    Entry {
        capture: "u2a-speed/c15000",
        args: &["--block", "1"],
        frames: &[rgb0(6400, 6400), rgb0(6600, 6600)],
    },
    // The original at 25000 cycles (reference-indices.md §27): U2A's pre-gate hold is the
    // background under the merged palette, which the port shows (A32; tests/u2a.rs).
    Entry {
        capture: "u2a-speed/c25000",
        args: &["--block", "1"],
        frames: &[rgb0(6400, 6400), rgb0(6600, 6600)],
    },
    // The original at 35000 cycles (reference-indices.md §27): U2A's pre-gate hold is the
    // background under the merged palette, which the port shows (A32; tests/u2a.rs).
    Entry {
        capture: "u2a-speed/c35000",
        args: &["--block", "1"],
        frames: &[rgb0(6400, 6400), rgb0(6600, 6600)],
    },
    // The original at 60000 cycles (reference-indices.md §27): U2A's pre-gate hold is the
    // background under the merged palette, which the port shows (A32; tests/u2a.rs).
    Entry {
        capture: "u2a-speed/c60000",
        args: &["--block", "1"],
        frames: &[rgb0(6400, 6400), rgb0(6600, 6600)],
    },
    // tests/u2e.rs, block 4 (:55): the test's own index rule, the reference's state at the
    // port's K − 4 (:221-251), where the silhouettes are exact and the residual is the
    // release's Gouraud shading: 189 / 7,118 / 231 / 1,177 (:229-232, A30).
    Entry {
        capture: "u2e",
        args: &["--block", "4"],
        frames: &[
            rgb(1000, 996, 189, "A30"),
            rgb(1600, 1596, 7118, "A30"),
            rgb(2600, 2596, 231, "A30"),
            rgb(3600, 3596, 1177, "A30"),
        ],
    },
];
