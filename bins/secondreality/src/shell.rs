//! The demo shell: the original's entry points, its part order, and the one
//! frame counter the whole run shares.
//!
//! `sr-engine` drives one part at a time and numbers that part's own frames
//! from zero. The demo does not: its parts are separate EXEs that the shell
//! loads in turn, and every index in `reference/` is a **boot-relative
//! emulator frame counter** (`docs/fidelity/reference-indices.md` §1.12) — the
//! demo's first graphics frame is 404, and BEGLOGO's keyframes are 430, 470,
//! 560 and 900 in that space. Until the port's counter lives in the same space,
//! no `xtask compare` in this phase can run at all, so [`Sequence`] is built
//! around one number in reference-space rather than one per part.
//!
//! Three things live here and nowhere else:
//!
//! - [`BLOCKS`] and [`block_plan`]: `SECOND n`'s `partsmask` byte, and the
//!   parts it selects, transcribed from `MAIN/U2.ASM:110-120`, `:595-630`.
//! - [`boot_anchor`]: where in the reference a part a run *starts* at begins.
//! - [`hardware_default_palette`]: the DAC the machine powers up with, which a
//!   run's first frames are drawn through.

use std::error::Error;
use std::fmt;
use std::io;
use std::time::Duration;

use sr_engine::music::MusicPosition;
use sr_engine::part::{Part, PartCtx, PartError, PartStatus};
use sr_engine::view::View;
use sr_engine::{Engine, EngineError, IndexedFrame, Palette, Tick};

use crate::parts;

// Re-exported for the binary's other modules: the shell is what decides how
// big the demo's screen is.
pub use sr_engine::{MAX_HEIGHT as SURFACE_HEIGHT, MAX_WIDTH as SURFACE_WIDTH};

/// The picture's starting geometry. A part may change it — double-scan makes it
/// 400 lines — and the frame follows it; this is only where a run begins.
pub const VIEW_WIDTH: u16 = 320;
pub const VIEW_HEIGHT: u16 = 200;

/// The demo's first graphics frame under the capture recipe: the shell sets
/// mode 13h and calls `inittwk`, and that is the first frame the emulator's
/// counter sees with the demo's picture on it. Measured 404 in five runs of
/// five (`reference-indices.md` §1.12, D17).
pub const FIRST_GRAPHICS_FRAME: u32 = 404;

/// A run's beginning: the shell's surface and the picture's starting geometry.
pub fn start_view() -> View {
    View::new(VIEW_WIDTH, VIEW_HEIGHT)
}

/// `SECOND n`'s argument to the `partsmask` byte it selects: the binary
/// literals at `MAIN/U2.ASM:111-117`, read by `checkcmdline` (`:595-630`).
///
/// `1` and no argument are equivalent in the original — the default
/// `whattorun` is `7fh` (`:108`) and the runner never tests bits 5 or 6 — so
/// the port keeps `1`'s mask and lets the command line decide whether a run
/// was asked for at all. `U` is the hidden part's block (`:613-616`, `:626-627`
/// accept `U` and `u`). The `partsmask` row for a block the original rejects
/// (`0`, `6`-`9`) is not here, because the port rejects it too.
pub const BLOCKS: &[(char, u8)] = &[
    ('1', 0x1f),
    ('2', 0x1e),
    ('3', 0x1c),
    ('4', 0x18),
    ('5', 0x10),
    ('U', 0x80),
];

/// The reference index of `part`'s first drawn frame, or `None` when the
/// evidence does not give one.
///
/// **This is only where a part begins when a run starts at that part.** A part
/// the demo reaches mid-run begins wherever the parts before it left off, which
/// is the chain's business, not this table's. Four parts have a measured
/// anchor — three block entries at 404, and one mid-block part whose first
/// frame the reference names exactly:
///
/// - `alku`, `beglogo` and `endlogo`, at **404**. `SECOND 1` runs ALKU first
///   (`MAIN/U2.ASM:790-792`), `SECOND 2` sets mode 13h + `inittwk` at
///   `MAIN/U2.ASM:806-808` and `SECOND 5` does the same at `:906-908`, and the
///   boot's first graphics frame is 404 in five runs of five
///   (`reference-indices.md` §1.12). ALKU opens on the black screen its own
///   `tw_setpalette(fade1)` leaves (§4.3's 900 is every pixel `(0,0,0)`);
///   BEGLOGO's first act is the white fill, which is what the screen holds
///   from there (§3.3 measures 410 and 429 byte-identical to 430); ENDLOGO's
///   white is up by 415 and holds (§19).
/// - `pam`, at **[`PAM_ANCHOR`] (7710)**. Block 1's third part
///   (`MAIN/U2.ASM:793-799`) is not an entry point, but its first drawn frame
///   is measured: the flat wash at 7710 is byte-identical in three sessions
///   and is the 63rd palette copy over the opening frame
///   (`reference-indices.md` §6.1, §6.4), while 7700 is still U2A's landscape.
///
/// Everything else returns `None`, and the mid-block parts without an anchor
/// are why: TUNNELI, CRED and ENDSCRL begin after an unported predecessor or a
/// music gate, and their starts are known only as **windows** — CRED's boundary
/// is in (1200, 1240] and ENDSCRL's in (8840, 8940] (§20, §21); MNTSCRL's is
/// the (415, 420] window §11.5 measures. A number here would be a guess wearing
/// a citation, and a run that starts at one of them is not
/// reference-comparable, which is what `None` makes it say.
pub fn boot_anchor(part: &str) -> Option<u32> {
    match part {
        "alku" | "beglogo" | "endlogo" => Some(FIRST_GRAPHICS_FRAME),
        "pam" => Some(PAM_ANCHOR),
        "u2e" => Some(U2E_ANCHOR),
        _ => None,
    }
}

/// U2E's intro start in the reference's numbering: the derived **418**, not
/// the first visible frame.
///
/// On the `SECOND 4` boot the loader's flat black holds through 466 and the
/// intro's `fadeset` draws the white band at **467** — measured in two
/// independent dense scans (`reference-indices.md` §18.7). The source
/// (`U2E.C:285-303`) runs 33 fade-in waits and 16 further waits before the
/// fadeset writes the displayed page, and that write precedes the fadeset's own
/// wait: 49 pre-visible waits, so the part's tick 0 is `467 - 49 = 418`. The
/// candidate 417 counts the fadeset's own wait before the write instead of
/// after it; the unit test `the_fadeset_lands_on_tick_49_and_paints_the_measured_band`
/// calibrates the value against the measured 467.
pub const U2E_ANCHOR: u32 = 418;

/// PAM's first drawn frame in the reference's numbering: the flat
/// `(251,251,251)` wash that is the 63rd palette copy over the opening frame.
///
/// **Measured, not a window.** `reference-indices.md` §6.1 found it by content
/// — 7700 is still U2A's landscape and 7710 is the wash — and §6.4 records it
/// byte-identical in three sessions; the frames either side (7709, 7711) are
/// the same bytes too. PAM is block 1's third part (`MAIN/U2.ASM:793-799`), so
/// its music recipe is the block's own `_zplaysong(0)`, whose clock starts at
/// [`BLOCK_ONE_MUSIC_START_FRAME`]: a run starting here pre-polls the 7,310
/// frames between the two (`MusicStart::for_part`).
pub const PAM_ANCHOR: u32 = 7710;

/// The smallest of `wanted` that a run starting at `start` can never reach.
///
/// A run's indices only ever go up, so an index below the first frame it draws
/// is one it walks away from — and a dump that silently lacks the frame says
/// nothing about why, which is the failure this check exists to prevent.
pub fn first_unreachable(start: u32, wanted: &[u32]) -> Option<u32> {
    wanted.iter().copied().filter(|index| *index < start).min()
}

/// The demo's parts in the order the shell runs them — `part-sequence.md` §1,
/// transcribed from `MAIN/U2.ASM`'s `move si,OFFSET exeN / call partexecute`
/// pairs (:790-918) — each with the `partsmask` bit that gates it (§4).
///
/// The names are the port's own: the original's are 8.3 filenames, and the
/// demo never puts one on screen.
const PART_ORDER: &[(&str, u8)] = &[
    ("alku", 0x01),
    ("u2a", 0x01),
    ("pam", 0x01),
    ("beglogo", 0x02),
    ("glenz", 0x02),
    ("tunneli", 0x02),
    ("techno", 0x02),
    ("panicend", 0x02),
    ("mntscrl", 0x04),
    ("lnszoom", 0x04),
    ("plzpart", 0x04),
    ("minvball", 0x04),
    ("rayscrl", 0x04),
    ("3dsinfld", 0x04),
    ("jplogo", 0x04),
    ("u2e", 0x08),
    ("ddstars", 0x80),
    ("endlogo", 0x10),
    ("cred", 0x10),
    ("endscrl", 0x10),
];

/// How a [`Sequence`] builds a part from a name in its plan.
///
/// The binary's is [`parts::new_part`], and `None` means the port has no
/// implementation of that part yet. The seam exists because the shell's
/// numbering is this task's and the parts it numbers are the phase's next five:
/// without it the one thing this module exists to get right could only be
/// tested after the last of them landed.
pub(crate) type PartSource = fn(&str) -> Option<Box<dyn Part>>;

// -------------------------------------------------------------------------------------------
// The music: which module, at which order, and when its clock starts
// -------------------------------------------------------------------------------------------

/// One of the two modules the demo plays: `MUSIC0.S3M` and `MUSIC1.S3M`.
///
/// The shell decides **which** module a run begins with (`MAIN/U2.ASM:766-775`'s
/// `restartmus(ax, bx, cx)`, where `ax` is this number), so the mapping to a file is
/// the shell's too.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleId {
    /// `ax = 0` — `MUSIC0.S3M`, whose tempo byte the shell patches
    /// (`MAIN/STARTMUS.C:254-258`).
    Music0,
    /// `ax = 1` — `MUSIC1.S3M`.
    Music1,
}

impl ModuleId {
    /// The shared song row: the module's name, its file and the tempo the shell patches.
    fn song(self) -> &'static sr_audio::Song {
        match self {
            ModuleId::Music0 => &sr_audio::SONGS[0],
            ModuleId::Music1 => &sr_audio::SONGS[1],
        }
    }

    /// The module's own name, as the demo's files spell it and as the status line and the
    /// manifest record it.
    pub fn name(self) -> &'static str {
        self.song().name
    }

    /// The module's own file, at the path a run reads its media from (the working
    /// directory, which for this repository's commands is its root).
    pub fn path(self) -> &'static str {
        self.song().file
    }

    /// The number `restartmus`'s `ax` carries for this module (`MAIN/U2.ASM:767`,
    /// `:772`) — the value the restart table's `mov ax,N` carries, which is what the test
    /// beside that table checks it against.
    #[cfg(test)]
    pub fn number(self) -> u8 {
        match self {
            ModuleId::Music0 => 0,
            ModuleId::Music1 => 1,
        }
    }

    /// The initial tempo byte the shell patches into this module's header, or `None`
    /// where it patches nothing.
    ///
    /// `MAIN/STARTMUS.C:252-265`'s `switch(a)`: case 0 loads MUSIC0 and then sets
    /// `module[50] = 0x78` — the S3M header's `initialTempo` byte at offset 50, 125 →
    /// 120 — and case 1 loads MUSIC1 and patches nothing, so MUSIC1 plays at its file's
    /// own 130. `measure`'s `music-clock.md` §3 has the two headers. The value is the
    /// shared [`sr_audio::Song`] row's, so `xtask music analyse` reads the same copy.
    pub fn tempo_patch(self) -> Option<u8> {
        self.song().tempo_patch
    }
}

/// The music's own clock, in the reference's frame space: the frame the first tick falls
/// on.
///
/// **Measured, and stable to a tenth of a frame across four sessions** (Task 1, Step
/// 4.1; `docs/fidelity/music-clock.md` §2): the trace's first row change is at frame 427
/// with `_np_zframe` 13 there, and 13 ticks at the tick clock's measured 1.4622 frames a
/// tick puts the first tick at **408.0**; the same derivation gives 408.0 in the fresh
/// session of the same recipe and 408.1 in each of the other two captures. It is a
/// property of the boot, not of a session, which is why it is one number.
///
/// Everything before it is silence in the player's own terms: `_np_ord`, `_np_row` and
/// `_np_pat` are 0 in every sample from frame 148 to 426 of `second5`, and `_np_zframe`
/// is 0 too. The port's run reaches it by [`MusicStart`], which is where the two kinds of
/// entry point part company.
pub const MUSIC_START_FRAME: u32 = 408;

/// The music's own clock for a **block-1** run: the initial `_zplaysong(0)`,
/// which no block restart precedes (`MAIN/U2.ASM:766-775`; the restart sites
/// are `:856` onwards).
///
/// **Measured, and it is not the restart recipes' 408.** `second1` — a
/// `SECOND 1` session — fits its first row change at frame 420 with
/// `_np_zframe` 14, and 14 ticks at the trace's own 1.4583 frames a tick is
/// 20.4 frames, so the row began at **399.6**; the `zframe` column's direct
/// reading is 405, 5.4 frames later (the same tick, seen at the next whole
/// frame sample; `cargo xtask music analyse --trace reference/music/second1`,
/// `docs/fidelity/music-clock.md` §2). A start frame is an integer in the
/// run's numbering, so the port uses the fitted value's nearest whole frame,
/// **400**. A `SECOND 1` run draws from 404, four frames after it, so the run
/// pre-polls those four frames ([`MusicStart::pre`]) rather than waiting.
pub const BLOCK_ONE_MUSIC_START_FRAME: u32 = 400;

/// The recipe a run's first part begins under: the block-1 start for the three
/// parts before any block restart (`alku`, `u2a`, `pam` — `MAIN/U2.ASM:790-799`,
/// gated on `whattorun & 1`), and the restart recipes' frame for everything
/// else. `part` is the plan's *first* name, which is the entry point.
pub fn music_start_frame_for(part: &str) -> u32 {
    match part {
        "alku" | "u2a" | "pam" => BLOCK_ONE_MUSIC_START_FRAME,
        _ => MUSIC_START_FRAME,
    }
}

/// How a run reaches the music's own clock.
///
/// The original does one of two things, and which one depends on where the run's own
/// frames begin:
///
/// - A run whose first drawn frame is **at or before** [`MUSIC_START_FRAME`] waits for it:
///   the player is installed and seeked at the run's start, and does not advance until
///   the frame the music's first tick falls on. This is the restart recipes' entry — block 2
///   draws from 404, four frames before its restart's 408 — and it is what `second5`'s frames
///   404…407 hold: a loaded module that has not ticked. (Block 1 is the other case: its
///   clock starts at 400 and a run at 404 pre-polls the four frames it missed.)
/// - A run whose first drawn frame is **after** it **pre-polls** the frames it missed, at
///   its start, because the music's advance is deterministic and does not depend on a
///   frame being drawn: `Player::poll` is the only thing that moves it, and N polls in a
///   row leave it exactly where N polls spread over N frames would. A `SECOND 3` run is
///   this case — MNTSCRL's first graphics frame is in (415, 420] while the music starts at
///   408 — and so is every run whose entry point is not the demo's own start. **PAM is
///   the port's measured case of it**: `--part pam` draws from 7710 while block 1's clock
///   starts at 400, so it pre-polls 7,310 frames (`PAM_ANCHOR`,
///   `reference-indices.md` §6.1).
/// - A run whose numbering is **its own** (`index_is_reference` is false — `--part X`,
///   `--part testcard`) has no reference frame to synchronise to: its music starts with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicStart {
    /// The run frame the music is **aligned to**. For a waiting entry
    /// ([`new`](MusicStart::new), [`for_part`](MusicStart::for_part) with an
    /// anchor at or before the start) it is the clock's start; for a
    /// **pre-polled** entry (an anchor after it — block 4 is 418 against the
    /// clock's 408) it is the run's first drawn frame, and the frames in
    /// between are polled at the run's start
    /// ([`pre`](MusicStart::pre)); for a restart
    /// ([`restart`](MusicStart::restart)) it is the boundary, with the clock
    /// beginning [`waiting`](MusicStart::waiting) frames later. The manifest's
    /// `start_frame` is this same word.
    pub frame: u32,
    /// Polls the engine makes before the music advances: the frames between the run's
    /// first drawn frame and the music's start (for a restart, between the boundary it was
    /// installed at and the restarted clock's first tick).
    pub waiting: u32,
    /// Polls the run makes at its start, before its first tick, so that the music is
    /// where it would have been had it been polled since its start.
    pub pre: u32,
}

impl MusicStart {
    /// The music's start for a run whose first drawn frame carries the reference index
    /// `anchor`, or `None` for a run with a numbering of its own — under the **restart
    /// recipes'** frame, [`MUSIC_START_FRAME`]. [`for_part`](MusicStart::for_part) is what
    /// a run uses, because a block-1 entry starts under a different one.
    pub fn new(anchor: Option<u32>) -> MusicStart {
        MusicStart::at(anchor, MUSIC_START_FRAME)
    }

    /// The music's start for a run whose plan begins at `part`: its recipe's frame
    /// ([`music_start_frame_for`]) and the anchor that run draws from.
    pub fn for_part(part: &str, anchor: Option<u32>) -> MusicStart {
        let start_frame = music_start_frame_for(part);
        if start_frame == MUSIC_START_FRAME {
            // The restart recipes' rule is the default `new` states.
            MusicStart::new(anchor)
        } else {
            MusicStart::at(anchor, start_frame)
        }
    }

    /// The start a **mid-run restart** installs at the run's boundary frame `index`
    /// ([`MUSIC_RESTART_AT`], `MAIN/U2.ASM:880-885`).
    ///
    /// The original's `restartmus` loads the new module and seeks it with `_zplaysong`, then
    /// hands the shell to the next part; the player is not polled across the boundary, and
    /// the first tick of the restarted clock lands **four** frames later — the restart
    /// recipes' measured 404→408 relation, `MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME`, which
    /// D34 records as `RunMusic.start.waiting == 4` and verifies through the block-3 chain
    /// (`second3e` is `second4`'s session shifted by 17,441–17,442 frames). Nothing is
    /// pre-polled: a restart is installed at the boundary, not before it.
    ///
    /// `index` is the run's own frame at that boundary — the frame the part being started
    /// draws its first tick at. The player's clock begins `waiting` frames later.
    pub fn restart(index: u32) -> MusicStart {
        MusicStart {
            frame: index,
            waiting: MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME,
            pre: 0,
        }
    }

    /// The measured rule, with the recipe's start frame as its parameter: a run that
    /// begins at or before it waits for it; one that begins after it pre-polls the
    /// frames it missed.
    fn at(anchor: Option<u32>, start_frame: u32) -> MusicStart {
        match anchor {
            Some(anchor) if anchor < start_frame => MusicStart {
                frame: start_frame,
                waiting: start_frame - anchor,
                pre: 0,
            },
            Some(anchor) if anchor > start_frame => MusicStart {
                frame: anchor,
                waiting: 0,
                pre: anchor - start_frame,
            },
            Some(_) => MusicStart {
                frame: start_frame,
                waiting: 0,
                pre: 0,
            },
            None => MusicStart {
                frame: 0,
                waiting: 0,
                pre: 0,
            },
        }
    }
}

/// How a [`Sequence`] builds its player. The binary's is
/// [`audio::load_player`](crate::audio::load_player).
///
/// The seam is [`PartSource`]'s: the modules are read from the working directory, which a
/// unit test does not share with a run, and a test that wants the real player without the
/// real parts (or no player at all) must not have to reach the assets' path to get one.
pub(crate) type MusicSource = fn(ModuleId) -> Result<crate::audio::Shared, String>;

/// One roll of the demo's own restart table: which module a block's run begins on, at which
/// order, and where `MAIN/U2.ASM` says so.
///
/// **The block's mask is the guard**, and `when`/`unless` are the two bits the guard's code
/// tests: `when` must be set in the block's `partsmask` byte, `unless` must be clear. Two of
/// the six rows are what happens when a bit is *not* set (block 2 begins MUSIC1 because
/// `whattorun & 1` is clear), and the middle four are the restart sites D13 transcribes.
///
/// **A row is a block's *starting* music, and three of those four sites are also that block's
/// entry point — but `:880-885` is not.** Its guard is `whattorun & 8` **alone** (`:880`), so
/// it fires for every block whose mask has bit 3: `SECOND 1`, `2`, `3` and `4`. Only block 4
/// reaches it as an entry point (its plan begins at U2E, `:886-887`); blocks 1, 2 and 3 run
/// the tier before it (`exe17`/`exe18`, `:875-878`) and reach it **mid-run**, between
/// JPLOGO and U2E, where the shell restarts MUSIC0 at order 18 under a player that has been
/// playing something else. `SECOND 1` has a second restart of its own (`:813-818`:
/// MUSIC1 at order 0, between BEGLOGO and GLENZ), whose guard is **two** bits — `& 2`
/// (the region `:801-802` opens and `:855` closes) **and** `& 1` (`:813-814`) — so its
/// shape is not `:880`'s; it fires for every block with bit 0, and every such mask also
/// has bit 1.
///
/// **Nothing the port can run today reaches either of them.** `--block 2`'s plan stops at
/// GLENZ, which is not ported, so TECHNO and PANICEND — block 2's fourth and fifth parts —
/// are unreachable through a block until GLENZ lands (Phase 7). `--block 3` now runs its
/// whole tier (MNTSCRL → LNS&ZOOM → PLZPART → MINVBALL → RAYSCRL → 3DSINFLD → JPLOGO) but
/// stops at U2E, which is not ported, so the plan stops before `:880-885` fires. `--block
/// 5`'s mask has bit 3 clear, so `:880` is skipped for it. A run that *does* reach U2E
/// inside blocks 2 or 3 needs a **mid-run** restart at that point in the sequence, and this
/// one-row-per-run table has no place for one yet. That is the Phase 4 dependency the row
/// for block 4 does not, by itself, express.
#[derive(Debug, Clone, Copy)]
// The four fields below are the row's *evidence* — which `whattorun` bits guard it and
// where `MAIN/U2.ASM` says so — and nothing at run time reads them: the test beside the
// table is what does, line by line. They are fields and not comments because that test
// has to be able to check them.
#[allow(dead_code)]
pub struct MusicRow {
    /// The block (`SECOND n`'s argument) this row is the entry point of.
    pub block: char,
    pub module: ModuleId,
    /// The order the entry point starts the module at, or 0 for the demo's own start,
    /// which reaches order 0 through `_zplaysong(0)` at `:786`.
    pub order: u8,
    /// The `whattorun` bit the row requires, or 0 when it requires none.
    pub when: u8,
    /// The `whattorun` bit the row requires to be **clear**.
    pub unless: u8,
    /// The lines of `MAIN/U2.ASM` the row is read from, as `file:first-last`. The test
    /// beside this table reads them and checks `said` against them.
    pub citations: &'static [&'static str],
    /// What those lines carry, in order. The test asserts each string occurs in the cited
    /// lines, in this order, and that the module's own number (`mov ax,N`) and the order
    /// (`mov bx,NN`) are among them.
    pub said: &'static [&'static str],
}

/// The demo's own restart table, in the original's own order: the initial start
/// (`MAIN/U2.ASM:766-786`) and then the four restart sites D13 transcribes (`:856-863`,
/// `:880-885`, `:889-894`, `:898-905`) — one row a *block*, giving the module and order that
/// block's run begins at. Three of the four sites are that block's entry point; `:880-885` is
/// entry-point-only for `SECOND 4` and a mid-run restart for `SECOND 1`, `2` and `3` (see
/// [`MusicRow`], and the test below that pins it).
///
/// Every row carries its citation and is checked against it by
/// `tests/shell.rs`'s `every_row_of_the_restart_table_is_the_originals_own`, which reads
/// the cited lines out of `upstream/SecondReality/MAIN/U2.ASM` — the file is not UTF-8,
/// so it is read as bytes.
pub const MUSIC_RESTARTS: &[MusicRow] = &[
    // The demo's own beginning: `SECOND 1` runs block 1, and the shell starts MUSIC0 at
    // order 0 for it (`mov ax,0` / `mov bx,-1` — the -1 is "no seek here", and the play
    // is the `push 0 / call _zplaysong` after the sound is on).
    MusicRow {
        block: '1',
        module: ModuleId::Music0,
        order: 0,
        when: 0x01,
        unless: 0,
        citations: &["MAIN/U2.ASM:766-775", "MAIN/U2.ASM:785-787"],
        said: &["mov ax,0", "mov bx,-1", "call restartmus", "push 0", "call _zplaysong"],
    },
    // Everything else: MUSIC1 at order 0. This is the initial start's other branch, and
    // it is what `SECOND 2` plays — the mid-run restart after BEGLOGO (`:813-818`) needs
    // `whattorun & 2` (the region `:801-802` opens) **and** `& 1`, so a block whose bit 0
    // is clear never reaches it.
    MusicRow {
        block: '2',
        module: ModuleId::Music1,
        order: 0,
        when: 0,
        unless: 0x01,
        citations: &["MAIN/U2.ASM:772-775"],
        said: &["mov ax,1", "mov bx,0", "mov cx,48700", "call restartmus"],
    },
    // `SECOND 3`'s entry-point restart, guarded `whattorun & 4` and not `whattorun & 2`.
    MusicRow {
        block: '3',
        module: ModuleId::Music1,
        order: 42,
        when: 0x04,
        unless: 0x02,
        citations: &["MAIN/U2.ASM:856-863"],
        said: &[
            "test cs:whattorun,4",
            "test cs:whattorun,2",
            "mov ax,1",
            "mov bx,42",
            "call restartmus",
        ],
    },
    // `SECOND 4`'s — and, for `SECOND 1`, `2` and `3`, the mid-run restart before U2E:
    // the guard is bit 3 **alone**, which those masks also carry (`:877-885`, after
    // JPLOGO and before `exe19`). Only block 4's plan starts here.
    MusicRow {
        block: '4',
        module: ModuleId::Music0,
        order: 18,
        when: 0x08,
        unless: 0,
        citations: &["MAIN/U2.ASM:880-885"],
        said: &["test cs:whattorun,8", "mov ax,0", "mov bx,18", "call restartmus"],
    },
    // The hidden part's, guarded `whattorun & 80h` — `SECOND U`.
    MusicRow {
        block: 'U',
        module: ModuleId::Music0,
        order: 70,
        when: 0x80,
        unless: 0,
        citations: &["MAIN/U2.ASM:889-894"],
        said: &["test cs:whattorun,80h", "mov ax,0", "mov bx,70", "call restartmus"],
    },
    // `SECOND 5`'s, guarded `whattorun & 16` and not `whattorun & 8`.
    MusicRow {
        block: '5',
        module: ModuleId::Music0,
        order: 25,
        when: 0x10,
        unless: 0x08,
        citations: &["MAIN/U2.ASM:898-905"],
        said: &[
            "test cs:whattorun,16",
            "test cs:whattorun,8",
            "mov ax,0",
            "mov bx,25",
            "call restartmus",
        ],
    },
];

/// The parts whose start is a **mid-run** `restartmus` site, and the module and order
/// each restarts: a part-keyed transcription of the demo's two mid-run sites.
///
/// **`u2e`** is `MAIN/U2.ASM:880-885`:
///
/// ```text
/// test    cs:whattorun,8
/// jz      @@skipp4
/// mov     ax,0
/// mov     bx,18
/// mov     cx,45000
/// call    restartmus
/// mov     si,OFFSET exe19
/// call    partexecute
/// ```
///
/// Its guard is `whattorun & 8` **alone** — not a block number — and every block whose mask
/// carries bit 3 reaches the site (`SECOND 1` `0x1f`, `2` `0x1e`, `3` `0x1c`, `4` `0x18`).
/// The port therefore keys the restart by the part the site precedes: U2E's presence in a
/// plan implies the guard, so a plan that reaches `u2e` through [`Sequence::take_next`]
/// restarts MUSIC0 at order 18 before it starts. For `SECOND 4` the same call is the run's
/// *entry* install and goes through [`Sequence::with_run`], not this table; blocks 1–3 reach
/// it mid-plan.
///
/// **`glenz`** is `MAIN/U2.ASM:813-818`, between BEGLOGO (`exe3`) and GLENZ (`exe4`):
///
/// ```text
/// test    cs:whattorun,1
/// jz      @@skip2b
/// mov     ax,1
/// mov     bx,0
/// mov     cx,48700
/// call    restartmus
/// @@skip2b:
/// mov     si,OFFSET exe4
/// call    partexecute
/// ```
///
/// Its guard is **two** bits — `whattorun & 2` (the region `:801-802` opens and `:855`
/// closes) **and** `whattorun & 1` (`:813-814`) — which is not `:880`'s shape. Every block
/// whose mask has bit 0 also has bit 1, so the plan reaching GLENZ under block 1 implies
/// the guard; block 2 (`0x1e`) skips the site at its second test and is already playing
/// MUSIC1 at order 0 (`:772-775`), which is what the site would install. The equality check
/// in [`Sequence::restart_music`] is therefore what keeps block 2 from reloading the player
/// it is already playing.
///
/// D34 has the measurement: the new player waits four engine ticks and the block-3 chain's
/// restarted session is `second4`'s shifted by 17,441–17,442 frames. D38 has the two seams'
/// record, including the block-1 restart's measured port timing.
pub const MUSIC_RESTART_AT: &[(&str, ModuleId, u8)] = &[
    ("u2e", ModuleId::Music0, 18),
    ("glenz", ModuleId::Music1, 0),
];

/// The module and order `SECOND block` starts its music at — the restart table's row for
/// the block, or [`ShellError::UnknownPart`] for a block the original rejects.
pub fn music_for(block: char) -> Result<(ModuleId, u8), ShellError> {
    let block = block.to_ascii_uppercase();
    MUSIC_RESTARTS
        .iter()
        .find(|row| row.block == block)
        .map(|row| (row.module, row.order))
        .ok_or(ShellError::UnknownPart {
            name: block.to_string(),
        })
}

/// The music of a run that begins at `part`: the entry point the demo itself starts that
/// part at, or — for a part the demo only *reaches* — the module and order in force when its
/// own sequence gets there.
///
/// A single-part run names a part as its own entry point, so for a part a block plan begins
/// at the question "which module, at which order" is the one the demo answers for that part:
/// `SECOND 5` starts ENDLOGO, `SECOND 2` starts BEGLOGO, and their restarts are the table
/// above. `--part endlogo` and `--part beglogo` take those rows.
///
/// **A part no block starts at is not played from block 1's row.** TUNNELI, CRED and ENDSCRL
/// are reached mid-run, and which module is in force there is the `whattorun` guards' answer,
/// not block 1's. (The demo's first three parts — ALKU, U2A, PAM — are answered too, because a
/// hand-built plan may name one: they run before any restart, under block 1's boot start,
/// MUSIC0 at order 0.)
///
/// - **TUNNELI** runs only where `whattorun & 2` is set (blocks 1 and 2, masks `0x1f`/`0x1e`).
///   Block 1's bit 0 is set, so it takes the mid-run restart at `MAIN/U2.ASM:813-818` —
///   `mov ax,1` / `mov bx,0` — which puts **MUSIC1 at order 0** between BEGLOGO and
///   GLENZ/TUNNELI; block 2's bit 0 is clear, so it skips that site and took the initial
///   start's other branch, also **MUSIC1 at order 0** (`:772-775`). Both sequences therefore
///   reach TUNNELI under MUSIC1. The **order in force** when TUNNELI begins is not in any
///   filed evidence — GLENZ has played since the restart, and no block-1 or block-2 music
///   trace exists — so the port seeks to the restart's own order, **0**, which is the only
///   value the evidence names. That the order is not established is deliberate in the record,
///   not an oversight: a run of `--part tunneli` plays the right module and the right
///   movement of the track, from the top of it.
/// - **CRED** runs in **every block**: every `partsmask` byte carries bit 4 (blocks 1–5 are
///   `0x1f`, `0x1e`, `0x1c`, `0x18`, `0x10`), and bit 3 is what decides which restart precedes
///   it. Block 5 (bit 3 clear) takes the entry restart at `:898-905`, **MUSIC0 at order 25**;
///   blocks 1–4 (bit 3 set) take the mid-run `:880-885` restart, **MUSIC0 at order 18**, and skip
///   `:898-905` (for block 4 that restart is its entry-point one, immediately before U2E). Every
///   sequence is MUSIC0. The filed
///   block-5 traces (`second5`, `second5b`) put the published music at order **26** at CRED's
///   tick 0 (reference index 1212: `26:56` in both), so the port seeks to **26** — measured, not
///   inferred. The other blocks' order at CRED is not measured by any filed trace.
/// - **ENDSCRL** is CRED's successor in those same sequences: blocks 1–4 reach it under
///   `:880-885`'s MUSIC0@18 and block 5 under `:898-905`'s MUSIC0@25 (the sequence the filed
///   traces cover). Those traces end at frame 1299, some 7,600 frames before ENDSCRL begins
///   (~8917). Its order is therefore **derived, not measured**: advancing MUSIC0 from the
///   block-5 restart (order 25 at frame 408) with the port's own player — the one those traces
///   verify at frames 1212 and 1299 — puts the engine at order **43** across the reference's
///   whole ENDSCRL-start window (8841..8938), so 43 is the value the evidence best supports. The
///   derivation is recorded here because it is the only number in this function that is not
///   either a source constant or a trace reading.
/// - **RAYSCRL** is block 3's fifth part, after MINVBALL, so it is not an entry point. The filed
///   block-3 trace (`second3b`, frames 10200–15050) holds **MUSIC1 at order 86** — pattern 60 —
///   from 12704 through 12961, and order 88 (pattern 35) from 12962. The part's start window is
///   `(12900, 13000]` (`reference-indices.md` §15), so the order in force when the sequence
///   reaches it is **86**; that is the value the port seeks to, **measured** from the trace. The
///   seek is also load-bearing beyond the ordinal: order-list entry 87 is a `254` marker, and
///   playing *through* it is what sets `zplus` bit 1 — the state the reference's player carries
///   into order 88 (`zplus == 2`). A seek straight to 88 gives `zplus == 0`, and RAYSCRL's init
///   wait (`dis_musplus() >= 0`, `WATER/DEMO.PAS:98-104`) would never pass. The part's own
///   `init` then pre-polls the frames between order 86 and the published order 88 row 0 its
///   tick 0 stands at (`MUSIC_PRE_ROLL`, `parts/rayscrl.rs`).
///
/// `None` for a name the demo does not have at all — the test card, which is the port's own
/// and not the demo's — because a run of that has no music to install, and `PartCtx::music()`
/// is then `None` rather than a player nobody asked for.
pub fn music_for_part(part: &str) -> Option<(ModuleId, u8)> {
    let entry_point = BLOCKS
        .iter()
        .map(|(block, _)| *block)
        .find(|block| {
            block_plan(*block)
                .ok()
                .and_then(|plan| plan.first().copied())
                == Some(part)
        });
    if let Some(block) = entry_point {
        return music_for(block).ok();
    }
    match part {
        // The demo's first region (block 1's `whattorun & 1` parts, `MAIN/U2.ASM:793-799`)
        // runs before any restart, under the initial start's MUSIC0 at order 0 (`:766-775`,
        // which block 1 takes because its bit 0 is set). The order in force at each part's
        // start is not established (the track has played on since the start), so the start's
        // own order — 0 — is the value the evidence names.
        "alku" | "u2a" | "pam" => Some((ModuleId::Music0, 0)),
        // GLENZ is block 2's second part and block 1's fifth, and neither block
        // plan *starts* at it. Both sequences reach it under **MUSIC1 at order
        // 0**: `SECOND 2` takes the initial start's `mov ax,1 / mov bx,0`
        // (`MAIN/U2.ASM:772-775`), and `SECOND 1` takes the mid-run restart that
        // does the same (`:813-818`, [`MUSIC_RESTART_AT`]) between BEGLOGO and
        // GLENZ. A `--part glenz` run is therefore MUSIC1 from the top, numbered
        // in its own frame space (the part has no measured boot anchor).
        "glenz" => Some((ModuleId::Music1, 0)),
        "tunneli" => Some((ModuleId::Music1, 0)),
        "cred" => Some((ModuleId::Music0, 26)),
        "endscrl" => Some((ModuleId::Music0, 43)),
        // DOTS starts in the middle of block 3, after PLZPART. The trace's order
        // when the part's start window opens (boot 10,300..10,368) is 75; it
        // advances to 77 at 10,380, twelve frames into the part's fade.
        // PLZPART is block 3's third part. `second3a` holds the transition: the
        // published words go from order 60 row 63 at boot 5720 to **order 62 row 0 at
        // 5723**, where the part's own `_dis_setmframe(0)` (`PLZPART/PLZ.C:69`) zeroes
        // `_np_zframe`; the natural advance over the `254` at MUSIC1 order 61 is what
        // lands 62:0 zplus 2. The seek starts at 62, the order in force; the entry run's
        // head start and its two gates are the part's (`parts/plzpart.rs`), because a
        // seek cannot cross the marker that is now in the past.
        "plzpart" => Some((ModuleId::Music1, 62)),
        // LNS&ZOOM is block 3's second part, after MNTSCRL. The filed block-3 trace
        // (`second3`) holds **MUSIC1 at order 45** at the part's start: frame 2452 reads
        // `45:59` pattern 33 with `_np_zframe` 0 — the part's own `_dis_setmframe(0)`
        // (`LENS/MAIN.C:106`) — and 2456 reads `45:60`. The seek starts at the order in
        // force, 45; the part's `init` pre-polls the polls between row 0 and the measured
        // `45:59` (`parts/lnszoom.rs`'s `MUSIC_PRE_ROLL`), which also puts the inter-act
        // gate's `48:44` on the trace's boot 2925.
        "lnszoom" => Some((ModuleId::Music1, 45)),
        "minvball" => Some((ModuleId::Music1, 75)),
        "rayscrl" => Some((ModuleId::Music1, 86)),
        // TECHNO is block 2's fourth part. `second2`'s trace holds MUSIC1 at
        // order **18** across the part's start window — frame 4797 reads
        // `18:60`, pattern 3, the row `KOE.C:214`'s `setmframe(0)` leaves in
        // place — and the order in force when the sequence reaches it is 18.
        // The seek starts at 18; the entry run's pre-roll is the part's
        // (`parts/techno.rs`), which lands the published row at the trace's 60.
        "techno" => Some((ModuleId::Music1, 18)),
        // PANICEND is block 2's fifth part, after TECHNO, and the part that
        // never reads the player. The filed `second2` trace holds MUSIC1 at
        // order **37** across the part's start window (8780, 8790]: 8780 is
        // `37:53` pattern 16, 8785 `37:54` and 8790 `37:55`, tempo 130. The
        // order in force when the sequence reaches it is 37, so that is the
        // row; no entry pre-roll is needed because the part ignores the
        // player.
        "panicend" => Some((ModuleId::Music1, 37)),
        // 3DSINFLD is block 3's sixth part, after RAYSCRL. The filed block-3
        // traces cross MUSIC1's order 91 -> 93 boundary at 15038
        // (`second3d`; `second3b` at 15031) — order 92 is the `254` marker —
        // and the part's start window is (15000, 15100] with no start frame
        // of its own. Its gate (`COMAN/MAIN.C:56`) opens when `musplus` stops
        // being negative, which is the order-93 row-0 crossing, so the order
        // in force when the sequence reaches the part is **91** where the
        // window opens and 93 at the gate. The row is **derived, not
        // measured**: the window spans the crossing and the part marks no
        // frame. 91 is the order the trace holds across the window's start;
        // the part's `init` pre-polls the rest of order 91, the marker at 92
        // and the published lag to the gate (`parts/coman.rs`).
        "3dsinfld" => Some((ModuleId::Music1, 91)),
        // JPLOGO is block 3's seventh and last part, after 3DSINFLD. The
        // filed `second3d` trace holds **MUSIC1 order 96** across the part's
        // whole start window (frames 17000..17100 are rows 50..62 of pattern
        // 71); the counter steps over the `254` at order 97 into order 98 at
        // frame 17109, nine frames *after* the window. The part's `init`
        // pre-polls 483 frames, the count that puts the player's row
        // boundaries on the trace's from its first frame on
        // (`parts/jplogo.rs`).
        "jplogo" => Some((ModuleId::Music1, 96)),
        _ => None,
    }
}

/// Whether a run is still going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceStatus {
    Continuing,
    /// The run has **ended**: a part the port has could not be started, which is
    /// a broken build or a missing asset and not a part the port has yet to
    /// write. A run whose parts have all retired is not finished — it
    /// [`holds`](Sequence::holding) the screen its last part left, because that
    /// is what the original's loader shows between two EXEs, and a requested
    /// reference index past the end of the port's parts is often a frame the
    /// reference holds too (BEGLOGO's 900 is exactly that).
    Finished,
}

/// A run of the demo's parts, numbered in the reference's own frame space.
///
/// One engine, one output frame, one counter. The counter starts at the run's
/// [`boot_anchor`] — 404 for a run that starts at a block entry — and advances
/// by exactly one per tick, whoever drew the frame: that is what makes a
/// keyframe index in `reference/` the same number as the index a rendered
/// frame carries.
///
/// **Nothing is cleared between parts.** The original's parts are separate
/// EXEs that leave the screen and the DAC as they found them, and the port
/// keeps the engine's surface, view and palette across `start`. BEGLOGO's
/// keyframe 900 is that fact in the reference (§3.5: the hold belongs to the
/// loader and the next part's music wait, not to BEGLOGO's loop).
///
/// **The run holds at the end of its parts.** `BEG/BEG.C:87` ends `main()` when
/// the fade has converged, so the run that ported BEGLOGO has nothing left to
/// draw at the fade's last step — and the reference's index 900 is still the
/// same picture, because the loader and the next part's music wait are showing
/// it. A `Sequence` whose plan is exhausted (or whose next part is one the port
/// has not written) therefore keeps ticking and keeps composing the surface it
/// has, through an internal part that draws nothing, rather than stopping. See
/// [`holding`](Sequence::holding).
pub struct Sequence {
    engine: Engine,
    source: PartSource,
    /// The parts this run consists of, in the demo's order.
    plan: Vec<&'static str>,
    /// How far into `plan` the run is.
    next: usize,
    live: Option<Box<dyn Part>>,
    /// The part the run is on. A retired part keeps its name until the next one
    /// starts, so `part()` after the run ends names the one that drew the last
    /// frame.
    current: &'static str,
    frame: IndexedFrame,
    /// The reference index the next tick draws at. The frame `frame()` holds
    /// was drawn at `index() - 1`.
    index: u32,
    /// Where the run's numbering started, when the evidence gives one.
    anchor: Option<u32>,
    clock: Tick,
    /// Whether the run is past its parts and showing the screen they left.
    holding: bool,
    /// The part the hold began at, when the port has not written it. `None`
    /// when the plan itself ran out.
    stopped_at: Option<&'static str>,
    finished: bool,
    /// The run's music, when it has any. See [`RunMusic`].
    music: Option<RunMusic>,
    /// How the run builds a player, kept so a mid-run restart can install a fresh one
    /// ([`MUSIC_RESTART_AT`]): the original's `restartmus` loads the module into a new
    /// player, so the shell needs the loader again at the boundary, not only at the start.
    /// `None` for a run with no music at all.
    music_source: Option<MusicSource>,
    /// Whether the part that just retired was the demo's `panicend`, so the next
    /// [`take_next`](Sequence::take_next) installs the shell's cue wait
    /// (`MAIN/U2.ASM:826-853`). Set only when `panicend` retires by reporting
    /// [`PartStatus::Finished`] itself — the original reaches the wait after
    /// PANICEND's EXE ends — and not when the exit flag retires it, which is the
    /// original's `bx=2` exit check skipping the wait.
    cue_pending: bool,
    /// Whether `live` is the shell's [`CueWait`] rather than a part from the plan.
    /// Its own retirement must not arm `cue_pending` again: the wait's `current`
    /// is still `panicend`, whose picture is up.
    live_cue: bool,
}

/// The music a run was built with: the player, which module and order it began at, how its
/// clock was started, and the dump it feeds.
struct RunMusic {
    /// The player, shared with the engine's adapter and with whatever drains it. A
    /// mid-run restart ([`MUSIC_RESTART_AT`]) replaces it with a fresh one.
    player: crate::audio::Shared,
    /// The module and order the player began at: the entry point's ([`MUSIC_RESTARTS`]) or
    /// the restart site's.
    module: ModuleId,
    order: u8,
    /// How this player's clock was started in the run's numbering ([`MusicStart`]): the
    /// entry recipe or the boundary a mid-run restart installed it at.
    start: MusicStart,
    /// `--audio-dump`'s file, when one was asked for: every tick's samples, in order.
    ///
    /// The dump and the sound card are two consumers of one ring, so a run has one or the
    /// other and never both (`window::run` opens no device when a dump is asked for).
    dump: Option<crate::audio::AudioDump>,
    /// The samples this tick produced, drained into `dump`.
    scratch: Vec<i16>,
}

/// The part a run holds with: it draws nothing, and touches nothing of the
/// surface, the palette or the view, so every tick composes them exactly as the
/// last real part left them. That is what the original's loader does between two
/// EXEs — BEGLOGO exits and GLENZ has not started, while the VGA keeps showing
/// what is in video memory — and it is why a reference index past the end of the
/// port's parts (BEGLOGO's 900) is answerable at all.
///
/// The **copper** it does touch, indirectly: installing it goes through
/// [`Engine::start`], which clears every copper slot the way each part start
/// does. So a callback the retiring part registered stops cycling during the
/// hold. The hold is a static screen and none of the port's parts keeps a
/// callback running past its own end, so this costs nothing here; it is written
/// down because it is the one thing the hold does not leave alone.
struct Holding {
    /// The part whose screen is being held, for the error message a `Part`
    /// name ends up in. The run's own `part()` still names that part.
    of: &'static str,
}

impl Part for Holding {
    fn name(&self) -> &'static str {
        self.of
    }

    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        Ok(())
    }

    fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus {
        PartStatus::Continuing
    }
}

/// The shell's wait between PANICEND and the next part of the plan —
/// `MAIN/U2.ASM:826-853`'s `@@zh5`/`@@zh4`, entered after `exe7` (PANICEND)
/// returns and skipped only by Ctrl or the exit flag:
///
/// ```text
/// @@zh5: mov bx,1
///        int 0fch              ; dis_waitb — one tick, and it checks the keys
///        mov bx,2
///        int 0fch              ; dis_exit
///        or ax,ax
///        jnz @@skip5
///        mov bx,6
///        int 0fch              ; dis_musplus
///        cmp dx,0
///        jge @@zh5             ; while musplus >= 0
/// @@zh4: mov bx,1
///        int 0fch
///        mov bx,2
///        int 0fch
///        or ax,ax
///        jnz @@skip5
///        mov bx,6
///        int 0fch
///        cmp dx,0
///        jg @@skip5            ; leave on the first positive
///        jmp @@zh4
/// ```
///
/// So the wait is **two-phase**: it waits while `musplus` is non-negative, then,
/// once a negative has been seen, while it is non-positive, and it continues the
/// moment one comes back positive. That is the measured 8903 (musplus 19, still
/// waiting) → 8953 (−32, the first negative) → 9219 (1, the exit) of
/// `reference/music/second2` (`docs/decisions.md` D38).
///
/// It draws nothing and touches nothing — PANICEND's screen is what is up while
/// the music reaches the cue — so a tick's frame is the surface the retiring part
/// left. One call is one `dis_waitb`.
struct CueWait {
    /// Whether the first phase has seen a negative yet. Only after it has does
    /// `@@zh4`'s "wait until positive" apply; before it, a non-negative reading
    /// is the first phase's own condition to keep waiting.
    saw_negative: bool,
}

impl Part for CueWait {
    fn name(&self) -> &'static str {
        // The picture on screen is PANICEND's, and this wait belongs to the
        // shell rather than to a part of the plan; `Sequence::part` still names
        // `panicend`.
        "panicend"
    }

    fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
        Ok(())
    }

    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
        // `mov bx,2; int 0fch; or ax,ax; jnz @@skip5`: the exit flag skips the
        // wait and the run goes on to the next part (it does not end here).
        if ctx.exit_requested {
            return PartStatus::Finished;
        }
        // There is no cue to wait for in a run with no player. The shell only
        // arms this wait after PANICEND's own tick, and PANICEND needs no music
        // to reach it; a hand-built music-less run must not wait for ever on a
        // player that does not exist.
        let Some(music) = ctx.music() else {
            return PartStatus::Finished;
        };
        let plus = music.musplus();
        if !self.saw_negative {
            if plus >= 0 {
                return PartStatus::Continuing;
            }
            self.saw_negative = true;
        }
        if plus > 0 {
            PartStatus::Finished
        } else {
            PartStatus::Continuing
        }
    }
}

impl Sequence {
    /// A run of `plan`'s parts, in that order, on a surface of
    /// `surface_width` × `surface_height` with the picture starting at `view`.
    ///
    /// The first part is started here rather than on the first tick, so a part
    /// that cannot be started fails as a [`ShellError`] the caller can act on
    /// instead of on a tick that has nowhere to report it.
    pub fn new(
        surface_width: usize,
        surface_height: usize,
        view: View,
        plan: Vec<&'static str>,
    ) -> Result<Sequence, ShellError> {
        Sequence::with_run(
            surface_width,
            surface_height,
            view,
            plan,
            parts::new_part,
            Some(crate::audio::load_player),
        )
    }

    /// [`Sequence::new`] with the parts coming from somewhere else, and no music.
    /// Crate-wide rather than private so `headless`'s own tests can drive a run of stub
    /// parts and see what a dump does with it — which is all it is for, now that a run
    /// with music goes through [`with_run`](Sequence::with_run).
    #[cfg(test)]
    pub(crate) fn with_source(
        surface_width: usize,
        surface_height: usize,
        view: View,
        plan: Vec<&'static str>,
        source: PartSource,
    ) -> Result<Sequence, ShellError> {
        Sequence::with_run(
            surface_width,
            surface_height,
            view,
            plan,
            source,
            None,
        )
    }

    /// A run of `plan`'s parts, with the parts coming from `source` and the player from
    /// `music` — or no music at all, for `None`.
    ///
    /// **The run's music is the entry point's**, decided from the plan's first name
    /// ([`music_for_part`]) because that name *is* the entry point: `--block 5` and
    /// `--part endlogo` both begin at ENDLOGO, and the demo starts ENDLOGO at MUSIC0 order
    /// 25 either way. The module is loaded and seeked here, at the run's start; when its
    /// polls begin is [`MusicStart`]'s business.
    ///
    /// `None` is a run with no player rather than a loader that fails: it is what the
    /// shell's own tests want when what they are watching is the numbering, the keys or the
    /// dump, and what a part the demo does not have (the test card) gets.
    pub(crate) fn with_run(
        surface_width: usize,
        surface_height: usize,
        view: View,
        plan: Vec<&'static str>,
        source: PartSource,
        music: Option<MusicSource>,
    ) -> Result<Sequence, ShellError> {
        // A run of nothing has no part to name and no frame to draw; the
        // binary never builds one, because every block has at least one part
        // and every `--part` names one.
        let Some(&first) = plan.first() else {
            return Err(ShellError::EmptyPlan);
        };
        let anchor = boot_anchor(first);
        let frame = IndexedFrame::new(view.width, view.height);
        let mut run = Sequence {
            engine: Engine::new(surface_width, surface_height, view),
            source,
            plan,
            next: 1,
            live: None,
            current: first,
            frame,
            // The run's numbering: the anchor when the evidence gives one, and
            // its own frames otherwise — and then it says so.
            index: anchor.unwrap_or(0),
            anchor,
            clock: Tick::new(),
            holding: false,
            stopped_at: None,
            finished: false,
            music: None,
            music_source: music,
            cue_pending: false,
            live_cue: false,
        };
        // The DAC the machine already had, installed before the first part
        // starts and never reset: see `hardware_default_palette`.
        *run.engine.palette_mut() = hardware_default_palette();
        if let (Some(loader), Some((module, order))) = (music, music_for_part(first)) {
            // The plan's first name is the entry point, and it decides the recipe: a
            // block-1 part starts under the initial `_zplaysong(0)` (frame 400), every
            // other entry under its restart's frame (408).
            let start = MusicStart::for_part(first, anchor);
            run.install_music(module, order, loader, start)?;
        }
        run.start(first)?;
        Ok(run)
    }

    /// Load the module, seek it, and hand the engine the adapter that polls it.
    ///
    /// The order here is the original's: the module is loaded (`MAIN/U2.ASM:775`'s
    /// `restartmus` runs `exemus`), seeked (`:394`'s `call _zplaysong`, or `:786`'s for the
    /// demo's own start), and only then does anything poll it — the engine's `set_music`
    /// makes it the run's player from the next tick on, and [`MusicStart::waiting`] holds
    /// its clock back to the frame the music's own first tick falls on.
    ///
    /// `start` is the entry path's [`MusicStart::for_part`] or the restart's
    /// [`MusicStart::restart`]; the two differ in when the player's clock begins. A
    /// restart is a **new player** (`restartmus` loads the module again), so the run's
    /// [`AudioDump`](crate::audio::AudioDump) and sample scratch are carried across: they
    /// belong to the run and its one output stream, not to whichever player mixes it.
    fn install_music(
        &mut self,
        module: ModuleId,
        order: u8,
        source: MusicSource,
        start: MusicStart,
    ) -> Result<(), ShellError> {
        let player = source(module).map_err(|message| ShellError::Music { message })?;
        // Only now that the new player exists is the old one's state taken: a load that
        // fails must not cost the run the dump it has already written.
        let (dump, scratch) = match self.music.take() {
            Some(old) => (old.dump, old.scratch),
            None => (None, vec![0; 1024]),
        };
        crate::audio::lock(&player).play_song(order);
        // **The frames the music missed before this run began.** A run whose first drawn
        // frame is *after* the music's own start pre-polls the frames in between, because
        // the music's advance is deterministic and does not depend on a frame being drawn:
        // `Player::poll` is the only thing that moves it, and `pre` polls in a row leave it
        // exactly where `pre` polls spread over `pre` frames would. A run that begins
        // *before* the music's start has nothing to catch up on — its player waits
        // (`MusicStart::waiting`) rather than running early.
        //
        // **`pre` for the entries the port has today**: ALKU's anchor is 404 and its
        // block-1 recipe starts at 400, so a `SECOND 1` run pre-polls four frames (the
        // player had ticked through 400-403 before the run's first drawn frame); the
        // restart recipes' entries — BEGLOGO and ENDLOGO at 404 against 408 — wait four
        // frames instead. **PAM is the long catch-up**: `boot_anchor("pam")` is the
        // measured 7710 against block 1's 400, so a `--part pam` run pre-polls 7,310
        // frames before its first tick.
        //
        // Two things about a catch-up are worth knowing before it is debugged: one longer
        // than the ring (8192 samples) has its oldest samples dropped by the ring's own
        // overflow policy — the same policy an undrained device meets — so a pre-polled
        // run that also dumped its audio would begin slightly clipped (PAM's 7,310 polls
        // are such a run, and the headless dump sets no audio device); and the pre-poll is
        // `poll` after `poll`, not a jump, because the music's advance is what
        // `Player::poll` does and nothing else moves it.
        for _ in 0..start.pre {
            crate::audio::lock(&player).poll();
        }
        self.engine.set_music(Box::new(crate::audio::PlayerMusic::new(
            std::sync::Arc::clone(&player),
            start.waiting,
        )));
        self.music = Some(RunMusic {
            player,
            module,
            order,
            start,
            dump,
            scratch,
        });
        Ok(())
    }

    /// One logic tick. Returns `Finished` only when the run has ended — see
    /// [`SequenceStatus::Finished`], which is not where a run with no parts
    /// left goes.
    pub fn tick(&mut self) -> SequenceStatus {
        if self.finished {
            return SequenceStatus::Finished;
        }
        if self.live.is_none() && !self.take_next() {
            return SequenceStatus::Finished;
        }
        let part = self.live.as_mut().expect("a part is live");
        // Whether the exit flag ends this part's *work* as well as its loop. U2E's
        // source fades the DAC over sixteen retraces after `dis_exit`
        // (`U2E.C:541-555`), so its `Finished` is the only retirement signal; every
        // other part is done the moment the flag reaches it.
        let drains = part.drains_after_exit();
        let status = self.engine.tick(&mut **part, &mut self.frame);
        self.index += 1;
        // Finished, or asked to quit: either way this part's loop has ended and
        // the next tick hands the screen to the next part. The engine keeps the
        // surface and the DAC, which is what the original's separate EXEs do.
        if status == PartStatus::Finished
            || (self.engine.exit_requested() && !drains)
        {
            // PANICEND's own `Finished` is what reaches `@@zh5` in the
            // original: its EXE ends and the shell's wait begins. An exit flag
            // retiring it is the wait's own `bx=2` check, which skips the wait.
            // The cue wait itself also reports `Finished`; `live_cue` keeps it
            // from arming another one.
            if status == PartStatus::Finished && !self.live_cue && self.current == "panicend" {
                self.cue_pending = true;
            }
            self.live = None;
            self.live_cue = false;
        }
        // The music, after the part has drawn: the fade while there is nothing left to
        // play, and the tick's own samples to the dump. Both are the *shell's* acts — a
        // part never restarts, fades or drains the player — and neither can change what
        // the tick drew.
        self.music_after_tick();
        SequenceStatus::Continuing
    }

    /// What the shell does to the music once a tick has drawn.
    fn music_after_tick(&mut self) {
        let Some(music) = self.music.as_mut() else {
            return;
        };
        // `@@partsdone` (`MAIN/U2.ASM:935-942`): with nothing left to play, `mov cx,1;
        // call fademusic` walks the module's own global-volume byte down, one step a frame
        // (that is `fademusic`'s own `dis_waitb`), and the demo deinits and returns to DOS.
        // The port's hold *is* that moment — the plan has run out, or the next part is one
        // the port has not written — so the fade runs here, once per tick, until the byte
        // reaches zero.
        if self.holding {
            crate::audio::fade_music(&mut crate::audio::lock(&music.player), 1);
        }
        // The dump's samples are the tick's: `take_samples` is the ring's only consumer,
        // and a tick's worth is what the poll inside `engine.tick` mixed.
        if let Some(dump) = music.dump.as_mut() {
            // One poll's worth is a few hundred samples and the scratch holds more, so a
            // steady run takes this branch once a tick; the loop is here because how much
            // the ring holds is not this code's to bound (a pre-polled run starts with a
            // catch-up's worth in it).
            loop {
                let n = crate::audio::lock(&music.player).take_samples(&mut music.scratch);
                if n == 0 {
                    break;
                }
                dump.write(&music.scratch[..n]);
                if n < music.scratch.len() {
                    break;
                }
            }
        }
    }

    /// Put a part in `live` for this tick: the next one in the plan, or the
    /// [`hold`](Sequence::holding) once there is none the port can run.
    /// Returns false only when the run has ended.
    fn take_next(&mut self) -> bool {
        if self.holding {
            return true;
        }
        // `@@zh5`/`@@zh4` (`MAIN/U2.ASM:826-853`): the wait comes between
        // PANICEND and whatever the plan does next — the part after it, or the
        // hold when PANICEND was the plan's last. Consumed here, before either.
        if self.cue_pending {
            self.cue_pending = false;
            // `@@zh5`'s first `dis_waitb`/`dis_exit` pair runs before it reads
            // the music, and `jnz @@skip5` leaves the wait when the flag is
            // already set. The race is real: PANICEND does not poll the flag
            // (`SHUTDOWN.C` has no `dis_exit`), so ESC on its own final tick
            // leaves the part reporting `Finished` *and* the engine's flag
            // set — and `start_cue_wait`'s `engine.start` would clear that
            // flag before the cue could see it. Skip the wait instead and go
            // on with the plan; the next part's `start` clears the flag, as
            // `dis_partstart`'s version call does in the original.
            if !self.engine.exit_requested() {
                return self.start_cue_wait();
            }
        }
        let Some(&name) = self.plan.get(self.next) else {
            return self.hold(None);
        };
        self.next += 1;
        // `MAIN/U2.ASM:880-885` runs **before** `partexecute`: a restart site
        // replaces the player the incoming part will read, so it must be
        // installed before the part's `init` (U2E's `music_fresh` and its
        // `dis_sync` gate both see it).
        if let Err(error) = self.restart_music(name) {
            eprintln!("secondreality: {error}; the run ends here");
            self.finished = true;
            return false;
        }
        match self.start(name) {
            Ok(()) => true,
            // A part the port *has* but cannot start is a different thing from
            // one it has not reached, and `tick` has no channel to report
            // either on — so the loud one says so, and a run that ends here
            // does not look like one that is holding.
            Err(ShellError::Engine(error)) => {
                eprintln!("secondreality: {error}; the run ends here");
                self.finished = true;
                false
            }
            // The port has not written this part: the run holds where it
            // stands rather than skipping over it, and its frames keep the
            // numbering the block gave them.
            Err(_) => self.hold(Some(name)),
        }
    }

    /// Install the restart site's player when the part the run is about to start
    /// is one ([`MUSIC_RESTART_AT`], `MAIN/U2.ASM:880-885` and `:813-818`).
    ///
    /// The original's `restartmus` stops and unloads the player, loads the new module and
    /// seeks it, every time the site runs. The port installs a fresh player too — the
    /// modules are different files and `Player` owns its module — but only when the row's
    /// module/order **differs** from the player the run already has: a plan that reaches a
    /// restart site under the very same module and order would otherwise throw away a
    /// player's position for nothing. That is what keeps block 2 from reloading at GLENZ:
    /// its `:813-818` guard is clear (`whattorun & 1` is 0), and its initial start already
    /// sought MUSIC1 to order 0 (`:772-775`), the row's own module and order. The first
    /// part of a plan never comes through here (it is [`with_run`](Sequence::with_run)'s
    /// entry install).
    fn restart_music(&mut self, name: &'static str) -> Result<(), ShellError> {
        let Some(&(_, module, order)) = MUSIC_RESTART_AT
            .iter()
            .find(|(part, _, _)| *part == name)
        else {
            return Ok(());
        };
        let Some(source) = self.music_source else {
            return Ok(());
        };
        let already = self
            .music
            .as_ref()
            .is_some_and(|music| music.module == module && music.order == order);
        if already {
            return Ok(());
        }
        // The boundary is the frame the incoming part's first tick draws at; the
        // restarted clock begins `waiting` frames later (D34).
        self.install_music(module, order, source, MusicStart::restart(self.index))
    }

    /// Install the shell's post-PANICEND cue wait (`MAIN/U2.ASM:826-853`).
    ///
    /// `current` stays `panicend`: the picture on screen is PANICEND's, and the
    /// wait is the shell's, not a part of the plan. The wait consumes one tick a
    /// call and does not advance the plan; [`take_next`](Sequence::take_next)
    /// starts the next plan part — or the hold — when it retires.
    fn start_cue_wait(&mut self) -> bool {
        let mut part = Box::new(CueWait { saw_negative: false });
        if let Err(error) = self.engine.start(&mut *part) {
            eprintln!("secondreality: {error}; the run ends here");
            self.finished = true;
            return false;
        }
        self.live = Some(part);
        self.live_cue = true;
        true
    }

    /// Start holding the screen the run's last part left. `stopped_at` is the
    /// plan name the run stopped at, when that is why the hold began.
    fn hold(&mut self, stopped_at: Option<&'static str>) -> bool {
        let mut part = Box::new(Holding { of: self.current });
        if let Err(error) = self.engine.start(&mut *part) {
            eprintln!("secondreality: {error}; the run ends here");
            self.finished = true;
            return false;
        }
        self.live = Some(part);
        self.live_cue = false;
        self.holding = true;
        self.stopped_at = stopped_at;
        true
    }

    /// Feed wall-clock time: tick once per due frame, dropping the stale ones.
    /// Returns how many ticks it took, and none once the run has finished.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        if self.finished {
            return 0;
        }
        let due = self.clock.advance(elapsed);
        let mut steps = 0;
        while steps < due {
            if self.tick() == SequenceStatus::Finished {
                break;
            }
            steps += 1;
        }
        steps
    }

    /// Start `name` as the part this run is on.
    fn start(&mut self, name: &'static str) -> Result<(), ShellError> {
        let mut part = (self.source)(name).ok_or_else(|| self.unbuildable(name))?;
        self.engine.start(&mut *part)?;
        self.live = Some(part);
        self.current = name;
        self.live_cue = false;
        Ok(())
    }

    /// Why a name in the plan has no part: the port has not reached it, or it
    /// is not the demo's at all.
    fn unbuildable(&self, name: &'static str) -> ShellError {
        match demo_part(name) {
            Some(part) => ShellError::NotPorted { name: part },
            None => ShellError::UnknownPart { name: name.to_string() },
        }
    }

    /// The last presented frame.
    pub fn frame(&self) -> &IndexedFrame {
        &self.frame
    }

    /// The reference index the next tick draws at; the frame [`frame`] holds
    /// was drawn at `index() - 1`.
    ///
    /// [`frame`]: Sequence::frame
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The part the run is on — after a tick, the one that drew the frame.
    pub fn part(&self) -> &'static str {
        self.current
    }

    /// Whether this run's indices are the reference's own. False when the run
    /// began somewhere the evidence does not anchor: its frames are then
    /// numbered from zero and no capture answers for them.
    pub fn index_is_reference(&self) -> bool {
        self.anchor.is_some()
    }

    /// The reference index the run began at, or `None` for an unanchored run.
    pub fn anchor(&self) -> Option<u32> {
        self.anchor
    }

    /// Whether the run has ended: a part the port has could not be started.
    /// False while the run holds.
    pub fn finished(&self) -> bool {
        self.finished
    }

    /// Whether the run has nothing left to play and is holding the screen its
    /// last part left — every part in the plan has retired, or the next one is
    /// a part the port has not written.
    ///
    /// This is the original's `@@partsdone` (`MAIN/U2.ASM:935-968`): the shell
    /// fades the music out, deinits the demo and returns to DOS. The port's
    /// window takes it as the moment there is nothing left to watch.
    pub fn holding(&self) -> bool {
        self.holding
    }

    /// The part the run stopped at: the plan's next name, when the hold began
    /// because the port has not written it. `None` while a part is running, and
    /// `None` when the run's own plan ran out.
    pub fn stopped_at(&self) -> Option<&'static str> {
        self.stopped_at
    }

    // --- the music ---------------------------------------------------------

    /// The music's published position — `_np_ord`, `_np_pat`, `_np_row` and the two words
    /// written straight through — or `None` for a run with no player.
    ///
    /// This is what a person debugging a sync problem needs (the window's status line
    /// shows `order:row`), and it is the *published* position: the ring's, ten polls behind
    /// the player's own (`docs/fidelity/music.md` §8), which is the one the parts read.
    pub fn music_position(&self) -> Option<MusicPosition> {
        self.engine.music().map(|music| music.position())
    }

    /// Which module the run is playing and the order it was seeked to, or `None` for a run
    /// with no player.
    pub fn music_module(&self) -> Option<(ModuleId, u8)> {
        self.music.as_ref().map(|music| (music.module, music.order))
    }

    /// The frame the run's music was started at, in the run's numbering — where the
    /// clock's start is for an entry install, or the boundary a mid-run restart was
    /// installed at — or `None` for a run with no player. See [`MusicStart::frame`].
    pub fn music_start_frame(&self) -> Option<u32> {
        self.music.as_ref().map(|music| music.start.frame)
    }

    /// The module's own global-volume byte, or `None` for a run with no player.
    ///
    /// This is the byte `fademusic` (`MAIN/U2.ASM:333-346`) walks — the S3M header's offset
    /// 48, 0x40 in both files — and it is what the shell's fade reads and writes. A run that
    /// is holding walks it to zero, one step a tick.
    #[cfg(test)]
    pub fn music_volume(&self) -> Option<u8> {
        self.music
            .as_ref()
            .map(|music| crate::audio::lock(&music.player).global_volume())
    }

    /// The player, for a caller that drains it — the sound card's callback, which holds its
    /// own share of the same ring.
    pub fn audio_player(&self) -> Option<crate::audio::Shared> {
        self.music.as_ref().map(|music| std::sync::Arc::clone(&music.player))
    }

    /// The mixer's stream as a WAV header describes it — frames a second and channels — or
    /// `None` for a run with no player.
    ///
    /// The rate is the **player's own** `PlayerOptions`, not a fresh `Default`: the samples
    /// being written are the mixer's, so a header built from a second copy of the options
    /// would silently describe a stream the mixer is not producing if the loader's ever
    /// changed (a different `MixRate`, mono).
    pub fn audio_format(&self) -> Option<(u32, u16)> {
        self.music.as_ref().map(|music| {
            crate::audio::wav_format(&crate::audio::lock(&music.player).options())
        })
    }

    /// Send every tick's samples to `path` as a 16-bit WAV at the mixer's rate and channel
    /// count (`--audio-dump`).
    ///
    /// **The dump replaces the sound card rather than joining it**: the ring has one
    /// consumer, so a run with a dump writes the file and opens no device — which is what
    /// `window::run` does, and why a dump is silent to watch.
    pub fn dump_audio(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        // A run with no player has nothing to dump, and a file with a header and nothing
        // in it reads as a recording that captured silence. The command line refuses this
        // combination (`Cli::validate`); this is the same rule for a `Cli` built by hand.
        let (rate, channels) = self.audio_format().ok_or_else(|| {
            io::Error::other("this run has no music to dump")
        })?;
        let dump = crate::audio::AudioDump::create(path, rate, channels)?;
        let music = self.music.as_mut().expect("audio_format answered, so there is music");
        music.dump = Some(dump);
        Ok(())
    }

    /// The first write `--audio-dump` could not make, if any: asked at the end of a run,
    /// because a dump that stopped half way is a failure and not a shorter recording.
    pub fn audio_error(&self) -> Option<String> {
        self.music
            .as_ref()
            .and_then(|music| music.dump.as_ref())
            .and_then(|dump| dump.error())
            .map(|error| error.to_string())
    }

    /// The reference index of the frame [`frame`](Sequence::frame) holds, or
    /// `None` before the run has drawn one: `new` leaves the cursor at the
    /// run's start, so the frame beside it is an empty one that no index names.
    pub fn frame_index(&self) -> Option<u32> {
        if self.index > self.anchor.unwrap_or(0) {
            Some(self.index - 1)
        } else {
            None
        }
    }

    /// The exit flag, from the keyboard: ask the running part to quit
    /// (`DIS/DISINT.ASM:151-153`).
    ///
    /// `checkkeys` sets `exitflag` on ESC and `exit_2` (`:186-190`, "returns 1
    /// if part should exit") hands it to the part's own loop, so the part
    /// returns *itself* and the shell goes on to the next part. The port's part
    /// sees the flag in the next [`PartCtx`] it is handed, and the run retires
    /// it at the end of that tick.
    ///
    /// Returns whether there was a part to ask: false when none is live —
    /// between two parts, or with the plan exhausted and the screen being held,
    /// which is where the original's shell has already reached `@@partsdone`
    /// (`MAIN/U2.ASM:935`).
    pub fn request_exit(&mut self) -> bool {
        if self.holding || self.live.is_none() {
            return false;
        }
        self.engine.request_exit();
        true
    }

    /// Ctrl: skip the running part (`MAIN/U2.ASM:205-207` — `partexecute` calls
    /// `ctrldown` and returns before `execute` when the key is down, so the part
    /// is not run at all).
    ///
    /// The port is in the middle of a part rather than in front of one, so "not
    /// run" is "not run again": the part is dropped where it stands, and the
    /// next tick starts the next part of the plan. The numbering is the run's,
    /// so it keeps counting across the seam — the next part's first frame is the
    /// index the skipped one would have drawn at.
    ///
    /// **One part, not all of them.** The original's `ctrldown` (`:185-203`)
    /// holds its carry from the first time it sees the key down, so *every*
    /// later `partexecute` skips its part as well and the demo runs to its end
    /// in a single jump. The port skips the part the key interrupted and goes on
    /// with the next, which is what the window's Ctrl is for; a run-wide skip is
    /// the window's close button's job.
    ///
    /// Returns whether there was a part to skip.
    pub fn skip_part(&mut self) -> bool {
        if self.holding || self.live.is_none() {
            return false;
        }
        self.live = None;
        true
    }
}

/// The DAC a VGA mode set leaves, which is what a run's first frames are drawn
/// through.
///
/// This is not the port's invention and not a "standard" default from a
/// datasheet: it is the table the port's oracle writes at the mode set, read
/// out of the emulator's own source — `INT10_SetVideoMode`'s `M_VGA` case hands
/// `palette.vga` to `write_palette_dac_data` (`ints/int10_modes.cpp:1977-1987`,
/// `:1152-1159`), and entries 0-15 of that table are the CGA colours
/// `cga_colors_default` (`:404-412`) because `cga_colors` is unset (`default`,
/// `gui/render/render.cpp:1917`).
///
/// Two details that are the emulator's and not a guess:
///
/// - **The table has 248 entries, not 256.** The mode set writes 248 triples
///   from DAC index 0 and stops; the last eight are whatever the DAC held at
///   power-up, which is black. `PALETTE` therefore leaves them out and
///   [`hardware_default_palette`] leaves them black.
/// - **It is written once per mode set.** The original's shell sets mode 13h at
///   each block entry (`MAIN/U2.ASM:806-808`, `:906-908`), and a run of the
///   port is one block entry, so the shell installs it once and the parts
///   inherit whatever the part before them left — which is exactly what makes
///   BEGLOGO's 430 *index 15 over an inherited DAC*: `BEG/BEG.C:20-21` fills
///   A-space with 15 and only writes the DAC at `:25-34`, after the 32 vertical
///   blanks. A shell that began from a black palette would render 430 black and
///   fail a keyframe before the part did anything.
fn hardware_default_palette() -> Palette {
    let mut dac = Palette::new();
    for (index, rgb) in VGA_DEFAULT_PALETTE.iter().enumerate() {
        dac.set_entry(index as u8, *rgb);
    }
    dac
}

/// `palette.vga` (`ints/int10_modes.cpp:580-645`), entry for entry: the first
/// sixteen are `cga_colors_default`, the rest the table's own literals. Six
/// bits a channel, which is what the emulator's `Rgb666` holds as well.
///
/// Transcribed rather than derived: the first sixteen are a doubling of the
/// first eight with the CGA's own palette registers, but the table past them is
/// a set of four 16-hue cubes at four intensities, and regenerating it from
/// that description would be a re-derivation with nothing to check it against.
#[rustfmt::skip]
const VGA_DEFAULT_PALETTE: [[u8; 3]; 248] = [
    [ 0,  0,  0], [ 0,  0, 42], [ 0, 42,  0], [ 0, 42, 42], [42,  0,  0], [42,  0, 42], [42, 21,  0], [42, 42, 42],
    [21, 21, 21], [21, 21, 63], [21, 63, 21], [21, 63, 63], [63, 21, 21], [63, 21, 63], [63, 63, 21], [63, 63, 63],
    [ 0,  0,  0], [ 5,  5,  5], [ 8,  8,  8], [11, 11, 11], [14, 14, 14], [17, 17, 17], [20, 20, 20], [24, 24, 24],
    [28, 28, 28], [32, 32, 32], [36, 36, 36], [40, 40, 40], [45, 45, 45], [50, 50, 50], [56, 56, 56], [63, 63, 63],
    [ 0,  0, 63], [16,  0, 63], [31,  0, 63], [47,  0, 63], [63,  0, 63], [63,  0, 47], [63,  0, 31], [63,  0, 16],
    [63,  0,  0], [63, 16,  0], [63, 31,  0], [63, 47,  0], [63, 63,  0], [47, 63,  0], [31, 63,  0], [16, 63,  0],
    [ 0, 63,  0], [ 0, 63, 16], [ 0, 63, 31], [ 0, 63, 47], [ 0, 63, 63], [ 0, 47, 63], [ 0, 31, 63], [ 0, 16, 63],
    [31, 31, 63], [39, 31, 63], [47, 31, 63], [55, 31, 63], [63, 31, 63], [63, 31, 55], [63, 31, 47], [63, 31, 39],
    [63, 31, 31], [63, 39, 31], [63, 47, 31], [63, 55, 31], [63, 63, 31], [55, 63, 31], [47, 63, 31], [39, 63, 31],
    [31, 63, 31], [31, 63, 39], [31, 63, 47], [31, 63, 55], [31, 63, 63], [31, 55, 63], [31, 47, 63], [31, 39, 63],
    [45, 45, 63], [49, 45, 63], [54, 45, 63], [58, 45, 63], [63, 45, 63], [63, 45, 58], [63, 45, 54], [63, 45, 49],
    [63, 45, 45], [63, 49, 45], [63, 54, 45], [63, 58, 45], [63, 63, 45], [58, 63, 45], [54, 63, 45], [49, 63, 45],
    [45, 63, 45], [45, 63, 49], [45, 63, 54], [45, 63, 58], [45, 63, 63], [45, 58, 63], [45, 54, 63], [45, 49, 63],
    [ 0,  0, 28], [ 7,  0, 28], [14,  0, 28], [21,  0, 28], [28,  0, 28], [28,  0, 21], [28,  0, 14], [28,  0,  7],
    [28,  0,  0], [28,  7,  0], [28, 14,  0], [28, 21,  0], [28, 28,  0], [21, 28,  0], [14, 28,  0], [ 7, 28,  0],
    [ 0, 28,  0], [ 0, 28,  7], [ 0, 28, 14], [ 0, 28, 21], [ 0, 28, 28], [ 0, 21, 28], [ 0, 14, 28], [ 0,  7, 28],
    [14, 14, 28], [17, 14, 28], [21, 14, 28], [24, 14, 28], [28, 14, 28], [28, 14, 24], [28, 14, 21], [28, 14, 17],
    [28, 14, 14], [28, 17, 14], [28, 21, 14], [28, 24, 14], [28, 28, 14], [24, 28, 14], [21, 28, 14], [17, 28, 14],
    [14, 28, 14], [14, 28, 17], [14, 28, 21], [14, 28, 24], [14, 28, 28], [14, 24, 28], [14, 21, 28], [14, 17, 28],
    [20, 20, 28], [22, 20, 28], [24, 20, 28], [26, 20, 28], [28, 20, 28], [28, 20, 26], [28, 20, 24], [28, 20, 22],
    [28, 20, 20], [28, 22, 20], [28, 24, 20], [28, 26, 20], [28, 28, 20], [26, 28, 20], [24, 28, 20], [22, 28, 20],
    [20, 28, 20], [20, 28, 22], [20, 28, 24], [20, 28, 26], [20, 28, 28], [20, 26, 28], [20, 24, 28], [20, 22, 28],
    [ 0,  0, 16], [ 4,  0, 16], [ 8,  0, 16], [12,  0, 16], [16,  0, 16], [16,  0, 12], [16,  0,  8], [16,  0,  4],
    [16,  0,  0], [16,  4,  0], [16,  8,  0], [16, 12,  0], [16, 16,  0], [12, 16,  0], [ 8, 16,  0], [ 4, 16,  0],
    [ 0, 16,  0], [ 0, 16,  4], [ 0, 16,  8], [ 0, 16, 12], [ 0, 16, 16], [ 0, 12, 16], [ 0,  8, 16], [ 0,  4, 16],
    [ 8,  8, 16], [10,  8, 16], [12,  8, 16], [14,  8, 16], [16,  8, 16], [16,  8, 14], [16,  8, 12], [16,  8, 10],
    [16,  8,  8], [16, 10,  8], [16, 12,  8], [16, 14,  8], [16, 16,  8], [14, 16,  8], [12, 16,  8], [10, 16,  8],
    [ 8, 16,  8], [ 8, 16, 10], [ 8, 16, 12], [ 8, 16, 14], [ 8, 16, 16], [ 8, 14, 16], [ 8, 12, 16], [ 8, 10, 16],
    [11, 11, 16], [12, 11, 16], [13, 11, 16], [15, 11, 16], [16, 11, 16], [16, 11, 15], [16, 11, 13], [16, 11, 12],
    [16, 11, 11], [16, 12, 11], [16, 13, 11], [16, 15, 11], [16, 16, 11], [15, 16, 11], [13, 16, 11], [12, 16, 11],
    [11, 16, 11], [11, 16, 12], [11, 16, 13], [11, 16, 15], [11, 16, 16], [11, 15, 16], [11, 13, 16], [11, 12, 16],
];

/// Why a run could not be built.
#[derive(Debug)]
pub enum ShellError {
    /// The name is not the demo's at all. An empty name gets this too: an
    /// empty `--part` is a name that matched nothing, and the message echoes it
    /// so the caller can see which argument was empty.
    UnknownPart { name: String },
    /// The name is a demo part the port has not reached yet.
    NotPorted { name: &'static str },
    /// The plan named no parts. Its own variant rather than an empty
    /// [`UnknownPart`](ShellError::UnknownPart): a run with nothing in it has
    /// no name to quote, and quoting one anyway is how `--part ""` came to be
    /// told it had named no parts.
    EmptyPlan,
    /// A part failed to start.
    Engine(EngineError),
    /// The run's music could not be built: the module's file is missing or is not an S3M.
    ///
    /// **A failure, not a silent run.** The demo is music-counted — a part that begins at
    /// the wrong music position drifts against every later part (D13) — so a run whose
    /// module cannot be read is a run that is not the demo, and it says so instead of
    /// drawing frames that nothing answers for. `--no-audio` is how a caller asks for no
    /// *device*; the player still runs.
    Music { message: String },
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::UnknownPart { name } => write!(f, "unknown part {name:?}"),
            ShellError::EmptyPlan => write!(f, "a run needs at least one part"),
            ShellError::NotPorted { name } => write!(f, "{name} is not ported yet"),
            ShellError::Engine(e) => write!(f, "{e}"),
            ShellError::Music { message } => write!(f, "the run's music cannot be loaded: {message}"),
        }
    }
}

impl Error for ShellError {}

impl From<EngineError> for ShellError {
    fn from(e: EngineError) -> ShellError {
        ShellError::Engine(e)
    }
}

/// The parts `block` runs, in the demo's own order (`part-sequence.md` §1).
///
/// Not truncated to the parts the port has: where the run stops is the
/// `Sequence`'s to discover and report, and a plan quietly cut short is how a
/// part gets skipped without anyone saying so.
pub fn block_plan(block: char) -> Result<Vec<&'static str>, ShellError> {
    let mask = block_mask(block).ok_or_else(|| ShellError::UnknownPart {
        name: block.to_string(),
    })?;
    Ok(PART_ORDER
        .iter()
        .filter(|(_, gate)| gate & mask != 0)
        .map(|(name, _)| *name)
        .collect())
}

/// The `partsmask` byte `SECOND block` selects, or `None` for a block the
/// original rejects.
fn block_mask(block: char) -> Option<u8> {
    // The code accepts `U` and `u` alike and both select `partsmask[8]`.
    let block = block.to_ascii_uppercase();
    BLOCKS.iter().find(|(name, _)| *name == block).map(|(_, mask)| *mask)
}

/// The demo part `name` is, as the run order spells it, or `None`.
fn demo_part(name: &str) -> Option<&'static str> {
    PART_ORDER.iter().find(|(part, _)| *part == name).map(|(part, _)| *part)
}

/// The name a run's plan may carry for `name`.
///
/// The distinction is the point: a name the demo has but the port has not
/// reached is a task someone can pick up, and a name the demo never had is a
/// typo.
pub fn resolve(name: &str) -> Result<&'static str, ShellError> {
    if let Some(registered) = parts::registered(name) {
        return Ok(registered);
    }
    match demo_part(name) {
        Some(part) => Err(ShellError::NotPorted { name: part }),
        None => Err(ShellError::UnknownPart {
            name: name.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_engine::copper::CopperRegistry;
    use sr_engine::music::Music;
    use sr_engine::palette::PaletteTimeline;
    use sr_engine::surface::Surface;
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    #[test]
    fn a_block_entry_anchor_is_the_boots_first_graphics_frame() {
        // The block entries that start at the boot's first graphics frame are
        // anchored at 404 under the D17 recipe.
        assert_eq!(boot_anchor("alku"), Some(404));
        assert_eq!(boot_anchor("beglogo"), Some(404));
        assert_eq!(boot_anchor("endlogo"), Some(404));
    }

    #[test]
    fn u2e_runs_are_numbered_from_their_derived_intro_anchor() {
        // The reference's first visible U2E frame is 467 (two dense sessions),
        // and the fadeset that draws it is the source's 49th pre-visible wait;
        // 467 - 49 = 418. The part's unit test calibrates the fadeset onto
        // tick 49.
        assert_eq!(boot_anchor("u2e"), Some(418));
        assert_eq!(U2E_ANCHOR, 467 - 49);
        // Block 4's plan begins at U2E (`SECOND 4`'s partsmask `0x18` selects
        // U2E's bit and nothing before it), so a `--block 4` run's first index
        // is the part's derived anchor.
        assert_eq!(block_plan('4').unwrap()[0], "u2e");
        assert_eq!(boot_anchor(block_plan('4').unwrap()[0]), Some(U2E_ANCHOR));
        // Block 4's recipe restarts MUSIC0 at 408, so a run from 418 pre-polls
        // the ten frames between them.
        assert_eq!(
            MusicStart::for_part("u2e", Some(U2E_ANCHOR)),
            MusicStart {
                frame: U2E_ANCHOR,
                waiting: 0,
                pre: 10
            }
        );
    }

    #[test]
    fn a_mid_block_part_whose_first_frame_is_measured_has_an_anchor() {
        // PAM is block 1's third part and not an entry point, but its first
        // drawn frame is the reference's own 7710 — a measured frame, not a
        // window (`reference-indices.md` §6.1, §6.4).
        assert_eq!(boot_anchor("pam"), Some(7710));
        assert_eq!(PAM_ANCHOR, 7710);
    }

    #[test]
    fn a_mid_block_part_has_no_anchor_and_says_so() {
        // CRED starts in (1200, 1240] and ENDSCRL in (8840, 8940] — windows,
        // not frames. A number here would be a guess wearing a citation.
        // MNTSCRL's first graphics frame is a *measured* window too — in
        // (415, 420], `reference-indices.md` §11.5 — and the 419 its fade
        // pins is the part's tick-0 index for the reference comparison, not
        // an anchor a run starts at.
        assert_eq!(boot_anchor("cred"), None);
        assert_eq!(boot_anchor("endscrl"), None);
        assert_eq!(boot_anchor("mntscrl"), None);
        // Block 3's other ported parts are mid-block too: MINVBALL and RAYSCRL
        // begin inside their block's run, and DDSTARS — block U's only part —
        // has a derived boot mapping from its own filed keyframes, not a
        // measured first drawn frame. Their runs number their own frames from
        // zero and the dump manifest says `index_origin: "part"`, `anchor: null`.
        assert_eq!(boot_anchor("minvball"), None);
        assert_eq!(boot_anchor("rayscrl"), None);
        assert_eq!(boot_anchor("ddstars"), None);
        // Phase 5's six parts are all windows or mappings too, never a measured
        // first drawn frame: LNS&ZOOM's first frame is in (2452, 2456],
        // PLZPART's in (5720, 5730], TECHNO's first rings are the
        // content-probed 4797 with its preamble spanning 4794..4800,
        // PANICEND's first frame is in (8780, 8790], 3DSINFLD's in
        // (15000, 15100] and JPLOGO's in (17000, 17100].
        assert_eq!(boot_anchor("lnszoom"), None);
        assert_eq!(boot_anchor("plzpart"), None);
        assert_eq!(boot_anchor("techno"), None);
        assert_eq!(boot_anchor("panicend"), None);
        assert_eq!(boot_anchor("3dsinfld"), None);
        assert_eq!(boot_anchor("jplogo"), None);
    }

    // --- the restart table --------------------------------------------------

    /// The demo's own source, the file every row of [`MUSIC_RESTARTS`] cites, read as
    /// **bytes**: it is not UTF-8 (it is a 1993 DOS source with box-drawing characters in
    /// it), and a reader that decoded it as text would misplace every line after the first
    /// non-ASCII byte. The same trap the phase has hit twice with `grep`.
    fn upstream_lines() -> Vec<String> {
        let path = "../../upstream/SecondReality/MAIN/U2.ASM";
        let bytes = std::fs::read(path).unwrap_or_else(|error| {
            panic!("{path}: {error} — the upstream tree is a submodule; run `git submodule update --init`")
        });
        // One line per `\n`, each with its bytes made into a `char` one for one, and every
        // run of spaces and tabs — the source aligns its operands with tabs — collapsed to
        // one space: what a `said` string has to match is the instruction, not the column
        // it starts in.
        bytes
            .split(|byte| *byte == b'\n')
            .map(|line| {
                let text: String = line.iter().map(|byte| char::from(*byte)).collect();
                text.split_whitespace().collect::<Vec<_>>().join(" ")
            })
            .collect()
    }

    /// The lines a citation names: `MAIN/U2.ASM:898-905`.
    fn cited(lines: &[String], citation: &str) -> Vec<String> {
        let (file, range) = citation
            .split_once(':')
            .unwrap_or_else(|| panic!("{citation:?} is not `file:first-last`"));
        assert_eq!(file, "MAIN/U2.ASM", "every row cites the shell's own source");
        let (first, last) = range
            .split_once('-')
            .unwrap_or_else(|| panic!("{citation:?} is not a line range"));
        let first: usize = first.parse().unwrap();
        let last: usize = last.parse().unwrap();
        assert!(first <= last, "{citation}: back to front");
        (first..=last)
            .map(|number| {
                lines
                    .get(number - 1)
                    .unwrap_or_else(|| panic!("{citation}: line {number} is past the file's end"))
                    .clone()
            })
            .collect()
    }

    #[test]
    fn every_row_of_the_restart_table_is_the_originals_own() {
        // **The table against the file it cites**, line by line: each row's `said` strings
        // must occur in the cited lines, in order, and the block's own `partsmask` bit must
        // be the guard the code tests. A row that carries a number the source does not is
        // the failure this test exists to catch — the phase has had two false claims from
        // reading this file with tools that called it binary.
        let lines = upstream_lines();
        let mut checked = 0;
        for row in MUSIC_RESTARTS {
            let text: Vec<String> = row
                .citations
                .iter()
                .flat_map(|citation| cited(&lines, citation))
                .collect();
            let joined = text.join("\n");
            let mut at = 0usize;
            for said in row.said {
                let found = joined[at..].find(said).unwrap_or_else(|| {
                    panic!(
                        "{} cites {:?}, and {said:?} is not in it after byte {at}:\n{}",
                        row.block,
                        row.citations,
                        joined
                    )
                });
                at += found + said.len();
            }
            // The numbers are the ones the instructions carry, not a second transcription:
            // the module is `mov ax,N` and the order is `mov bx,NN`, both already checked
            // above — this says so out loud.
            assert!(
                row.said.contains(&"call restartmus"),
                "{}'s row must be a `restartmus` call",
                row.block
            );
            let module_number = format!("mov ax,{}", row.module.number());
            assert!(
                row.said.contains(&module_number.as_str()),
                "{} runs {}, whose number is {module_number:?}",
                row.block,
                row.module.name()
            );
            // The order, where the row's own `mov bx,NN` carries it — the initial start's
            // block-1 row reaches order 0 through `push 0 / call _zplaysong` instead, and
            // its citation carries that.
            if row.block == '1' {
                assert!(row.order == 0 && row.said.contains(&"mov bx,-1"), "{}", row.block);
            } else {
                let order = format!("mov bx,{}", row.order);
                assert!(row.said.contains(&order.as_str()), "{}: {order}", row.block);
            }
            // The guard: the code's own two tests, against the block's `partsmask` byte.
            let mask = BLOCKS
                .iter()
                .find(|(name, _)| *name == row.block)
                .unwrap_or_else(|| panic!("{} is not a block", row.block))
                .1;
            assert_eq!(
                mask & row.when,
                row.when,
                "{}'s row is guarded by bit {:#04x}, which its mask lacks",
                row.block,
                row.when
            );
            assert_eq!(
                mask & row.unless,
                0,
                "{}'s mask has the bit ({:#04x}) the row requires to be clear",
                row.block,
                row.unless
            );
            // And the row is what the function hands out for that block.
            assert_eq!(
                music_for(row.block).ok(),
                Some((row.module, row.order)),
                "{}'s row is not the one `music_for` answers with",
                row.block
            );
            checked += 1;
        }
        assert_eq!(checked, 6, "the six entry points D13 records");
        assert_eq!(MUSIC_RESTARTS.len(), BLOCKS.len(), "one row a block");
    }

    #[test]
    fn one_row_of_the_table_is_not_entry_point_only_and_the_table_says_which() {
        // **The characterization the table has to get right.** Three of the four restart
        // sites fire only when the block they belong to is the one whose plan reaches them
        // first — `:856`'s guard is `& 4` *and not* `& 2`, `:898`'s is `& 16` and not `& 8`,
        // and `:889`'s is `& 80h` alone but only `SECOND U`'s mask has bit 7. The fourth,
        // `:880-885`, is guarded by `whattorun & 8` **alone**, and every block whose mask has
        // bit 3 reaches it: `SECOND 1` (0x1f), `2` (0x1e), `3` (0x1c) and `4` (0x18).
        //
        // For `SECOND 4` that is its entry point (its plan begins at U2E, `:886-887`); for
        // the other three it is a **mid-run** restart between JPLOGO (`exe18`, `:877-878`)
        // and U2E, under a player that has been playing something else — which the port's
        // one-row-per-run table has no place for. Nothing the port can run today reaches it,
        // and this test is what makes that a stated dependency rather than a hidden one.
        let row = MUSIC_RESTARTS
            .iter()
            .find(|row| row.block == '4')
            .expect("the block whose start the row gives");
        assert_eq!(
            (row.when, row.unless),
            (0x08, 0),
            "the guard is bit 3 alone — an `unless` would be what made it entry-point-only"
        );
        for block in ['1', '2', '3', '4'] {
            let mask = BLOCKS
                .iter()
                .find(|(name, _)| *name == block)
                .unwrap_or_else(|| panic!("{block} is not a block"))
                .1;
            assert!(
                mask & 0x08 != 0,
                "{block}'s mask has bit 3, so its run reaches :880 too — the restart is \
                 mid-run for it unless its plan begins at U2E"
            );
        }
        // Two of the three remaining sites carry an `unless` bit that keeps them
        // entry-point-only (`:856`'s `& 2` and `:898`'s `& 8`); the third, `:889-894`, has
        // none and is kept entry-point-only by the masks — asserted below, not assumed.
        for (block, unless) in [('3', 0x02), ('5', 0x08)] {
            let row = MUSIC_RESTARTS.iter().find(|row| row.block == block).unwrap();
            assert_eq!(row.unless, unless, "{block}'s restart fires only without the bit");
        }
        // `:889-894` is the fourth site and the odd one out: it has **one** gating test too
        // (`:889`'s `& 80h`), and what keeps it entry-point-only is the masks rather than the
        // guard — only `SECOND U`'s carries bit 7. Asserted here rather than left in prose,
        // because this test's own comment used to claim it without checking it.
        let hidden = MUSIC_RESTARTS.iter().find(|row| row.block == 'U').unwrap();
        assert_eq!((hidden.when, hidden.unless), (0x80, 0));
        for (block, mask) in BLOCKS {
            if *block == 'U' {
                continue;
            }
            assert_eq!(
                mask & 0x80,
                0,
                "{block}'s mask carries bit 7, so `SECOND U` is not the only block reaching \
                 :889 and the site is not entry-point-only"
            );
        }
    }

    #[test]
    fn the_second_mid_run_restart_is_two_guards_and_the_masks_carry_both() {
        // **The other site no row in this table is about**, and the one nothing else in the
        // suite reads: `:813-818`'s restart. Its guard is **two** bits — `whattorun & 2`
        // (the region `:801-802` opens with `jz @@skipp2`, closed at `:855`) **and**
        // `whattorun & 1` (`:813-814`) — which is not `:880-885`'s single-test shape, so the
        // two are worth telling apart. The conclusion survives the second guard: the only
        // mask with bit 0 is `SECOND 1`'s, and it carries bit 1 as well, so the site fires
        // for exactly the block whose run reaches it (after BEGLOGO, `exe3`, and before
        // GLENZ, `exe4`).
        let lines = upstream_lines();
        let region_opening = cited(&lines, "MAIN/U2.ASM:801-802").join("\n");
        assert!(
            region_opening.contains("test cs:whattorun,2")
                && region_opening.contains("jz @@skipp2"),
            "the restart is inside the region `& 2` opens: {region_opening}"
        );
        // ... and the region's close, so "the site is inside `& 2`'s region" is asserted
        // rather than resting on the line numbers' order: `:855` carries the label `:802`
        // jumps to.
        let region_close = cited(&lines, "MAIN/U2.ASM:855-855").join("\n");
        assert!(
            region_close.contains("@@skipp2:"),
            ":855 is the label `:802`'s `jz` lands on: {region_close}"
        );
        let site = cited(&lines, "MAIN/U2.ASM:813-818").join("\n");
        let mut at = 0usize;
        for said in [
            "test cs:whattorun,1",
            "jz @@skip2b",
            "mov ax,1",
            "mov bx,0",
            "mov cx,48700",
            "call restartmus",
        ] {
            let found = site[at..]
                .find(said)
                .unwrap_or_else(|| panic!("{:?} is not in :813-818 after byte {at}: {site}", said));
            at += found + said.len();
        }
        // The guard as a whole, then: bit 0 *and* bit 1, and every mask that has bit 0 has
        // bit 1 — so "it fires for `SECOND 1`'s run only" is true of the pair, not of `& 1`
        // alone.
        for (block, mask) in BLOCKS {
            if mask & 0x01 != 0 {
                assert_eq!(
                    *block, '1',
                    "{block}'s mask has bit 0, so the assumption that `SECOND 1` is the \
                     only block whose run reaches this restart is wrong"
                );
                assert!(
                    mask & 0x02 != 0,
                    "{block}'s mask has bit 1 too, so `& 2` does not narrow the site"
                );
            }
        }
    }

    #[test]
    fn a_blocks_first_part_is_the_entry_point_its_row_is_about() {
        // The table's rows are keyed by block; a *plan* is keyed by its first part, and the
        // two meet in `music_for_part`, which is what a run actually uses. Every block's row
        // must therefore be the row its own plan finds.
        for row in MUSIC_RESTARTS {
            let first = block_plan(row.block).unwrap()[0];
            assert_eq!(
                music_for_part(first),
                Some((row.module, row.order)),
                "{first} starts {}'s plan and must start its music",
                row.block
            );
        }
        // And every name `--part` accepts is in the table below: the entry points by their
        // rows, the three parts no block starts at by the module and order in force when the
        // demo's own sequence reaches them (TUNNELI's and ENDSCRL's order are the two values
        // `music_for_part`'s doc says are unestablished and derived; CRED's is the block-5
        // traces' `26:56` at reference index 1212), and the test card with no music at all.
        let expected: &[(&str, Option<(ModuleId, u8)>)] = &[
            ("testcard", None),
            // `SECOND 1`'s first parts run before any restart, under the initial
            // `_zplaysong(0)` (`MAIN/U2.ASM:766-775`).
            ("alku", Some((ModuleId::Music0, 0))),
            ("u2a", Some((ModuleId::Music0, 0))),
            ("pam", Some((ModuleId::Music0, 0))),
            ("beglogo", Some((ModuleId::Music1, 0))),
            // Block 2's initial start (`:772-775`) and block 1's `:813-818`
            // restart both put MUSIC1 at order 0 under GLENZ.
            ("glenz", Some((ModuleId::Music1, 0))),
            ("tunneli", Some((ModuleId::Music1, 0))),
            ("mntscrl", Some((ModuleId::Music1, 42))),
            // The order in force at the part's start window (2452, 2456]:
            // `second3` frame 2452 is `45:59`, the part's `_dis_setmframe(0)`.
            ("lnszoom", Some((ModuleId::Music1, 45))),
            // The order in force at the part's start window (5720, 5730]; the
            // `_dis_setmframe(0)` reset at 5723 is the part's tick 0.
            ("plzpart", Some((ModuleId::Music1, 62))),
            ("minvball", Some((ModuleId::Music1, 75))),
            ("rayscrl", Some((ModuleId::Music1, 86))),
            // `second2`'s `18:60` at 4797, the part's start window.
            ("techno", Some((ModuleId::Music1, 18))),
            // `second2`'s `37:53..37:55` across the part's start window
            // (8780, 8790]; the part never reads the player.
            ("panicend", Some((ModuleId::Music1, 37))),
            // The order the block-3 trace holds across the part's start
            // window (15000, 15100]; derived, because the window spans the
            // 91 -> 93 crossing and the part marks no start frame.
            ("3dsinfld", Some((ModuleId::Music1, 91))),
            // `second3d` holds order 96 across JPLOGO's whole start window
            // (17000, 17100]; the `254` at 97 is crossed at 17109.
            ("jplogo", Some((ModuleId::Music1, 96))),
            // `MAIN/U2.ASM:880-885` restarts MUSIC0 at order 18 for block 4
            // (`SECOND 4`) and for the mid-run restart the other blocks take
            // before U2E.
            ("u2e", Some((ModuleId::Music0, 18))),
            // The hidden part: `SECOND U` restarts MUSIC0 at order 70
            // (`MAIN/U2.ASM:889-894`), and block U's plan is this part alone.
            ("ddstars", Some((ModuleId::Music0, 70))),
            ("endlogo", Some((ModuleId::Music0, 25))),
            ("cred", Some((ModuleId::Music0, 26))),
            ("endscrl", Some((ModuleId::Music0, 43))),
        ];
        for (name, music) in expected {
            assert_eq!(
                music_for_part(name),
                *music,
                "the music the demo's own sequence reaches {name} under"
            );
        }
        assert_eq!(
            expected.len(),
            crate::parts::registry().len(),
            "every `--part` name the port accepts has a row above"
        );
        for (name, _) in crate::parts::registry() {
            assert!(
                expected.iter().any(|(at, _)| at == name),
                "{name} is accepted by `--part` but has no expected music"
            );
        }
    }

    #[test]
    fn a_run_with_nothing_left_to_play_fades_the_music_out_a_step_a_tick() {
        // `@@partsdone` (`MAIN/U2.ASM:935-942`) is `mov cx,1; call fademusic` — and the
        // port's hold *is* that moment, so the shell walks the module's own global-volume
        // byte down, one step a tick, from the tick the run has nothing left to play. The
        // demo's step is `mov cx,1` (`:927`/`:938`) and 0x40 is the byte both modules carry,
        // so the fade is 64 ticks long and stops at zero (`fademusic`'s `@@done`).
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["pam"],
            stub_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        let start = run.music_volume().expect("pam's run plays MUSIC0");
        assert_eq!(start, 0x40, "the module's own global-volume byte");

        // The part retires on its first tick, so the hold begins with the second.
        run.tick();
        assert!(!run.holding(), "the part drew its one frame");
        assert_eq!(run.music_volume(), Some(start), "and nothing has faded yet");
        for expected in (0..start).rev() {
            run.tick();
            assert!(run.holding(), "the run has nothing left to play");
            assert_eq!(
                run.music_volume(),
                Some(expected),
                "the fade is one step a tick, and {} steps are left",
                expected
            );
        }
        // And it stops at zero rather than wrapping: further ticks leave it there.
        run.tick();
        run.tick();
        assert_eq!(run.music_volume(), Some(0), "the byte saturates at zero");
    }

    // --- the run's music against the filed trace -----------------------------

    /// One row of a filed trace: the frame, and the published position at it.
    ///
    /// `reference/music/<name>/trace.csv` holds exactly the frames the trace *has* — the
    /// emulator's own gaps are missing rows and not zeroes — so walking the file is
    /// walking the frames the trace holds, which is what the brief's "read the trace's own
    /// gaps from its manifest" is for, read the other way round.
    ///
    /// **That the two readings agree is checked, not assumed**: `crates/sr-audio`'s
    /// `tests/trace.rs` recomputes each trace's coverage from the same CSV and asserts it is
    /// the manifest's window minus its gaps, field by field, for every filed trace
    /// (`Trace::check_coverage`) — so this file may walk the rows without opening the
    /// manifest.
    fn trace(name: &str) -> Vec<(u32, (u8, u8), i32)> {
        let path = format!("../../reference/music/{name}/trace.csv");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("{path}: {error}"));
        let mut lines = text.lines();
        let header: Vec<&str> = lines
            .next()
            .unwrap_or_else(|| panic!("{path}: no header row"))
            .split(',')
            .map(str::trim)
            .collect();
        // The columns are bound by the header's own names, not by position: this is the
        // format's third reader (`xtask/src/music.rs`'s `PlayerState::parse` and
        // `crates/sr-audio/tests/trace.rs`'s `parse_row` are the others), and a column
        // inserted before `zframe` must not silently re-bind this one.
        let column = |wanted: &str| -> usize {
            header
                .iter()
                .position(|field| *field == wanted)
                .unwrap_or_else(|| panic!("{path}: no `{wanted}` column in {header:?}"))
        };
        let (frame_at, ord_at, row_at, zframe_at) = (
            column("frame"),
            column("ord"),
            column("row"),
            column("zframe"),
        );
        lines
            .map(|line| {
                let number = |index: usize| -> u32 {
                    line.split(',')
                        .nth(index)
                        .unwrap_or_else(|| panic!("{line}: no field {index}"))
                        .trim()
                        .parse()
                        .unwrap_or_else(|error| panic!("{line}: field {index}: {error}"))
                };
                (
                    number(frame_at),
                    (number(ord_at) as u8, number(row_at) as u8),
                    number(zframe_at) as i32,
                )
            })
            .collect()
    }

    /// How far a published position may be from the frame the trace publishes it at.
    ///
    /// **Measured, not chosen**, and the same measurement `crates/sr-audio/tests/trace.rs`
    /// uses: the two filed sessions of one recipe (`second5` and `second5b` — the same
    /// command, the same emulator, the same cycles) disagree with *each other* about which
    /// frame eight of their row changes land on, and the port — whose clock is exact —
    /// sits between the sessions. The worst displacement against any filed trace is 3
    /// frames (`docs/fidelity/APPROXIMATIONS.md` A12, whose evidence is that test's own
    /// per-trace counts).
    const JITTER_FRAMES: u32 = 3;

    /// How far the port's `_np_zframe` may sit from the trace's own column, in ticks, once
    /// the sample instant's one-frame shift is taken out.
    ///
    /// **Measured, not chosen.** The trace's sample lands within a fifth of a frame of the
    /// poll it is about, and the two clocks' rate residuals over this window are a fraction
    /// of a tick (`crates/sr-audio/tests/trace.rs`'s per-trace counts; A12) — so one tick is
    /// the quantisation and two would be a drift. Measured at the commit that added this
    /// check: 753 of 891 frames exact, worst displacement **1** tick.
    const ZFRAME_SLOP: i32 = 1;

    /// The distinct values of a sequence, in order.
    fn distinct<T: PartialEq + Copy>(items: impl Iterator<Item = T>) -> Vec<T> {
        let mut out: Vec<T> = Vec::new();
        for item in items {
            if out.last() != Some(&item) {
                out.push(item);
            }
        }
        out
    }

    #[test]
    fn the_runs_music_is_where_the_trace_holds_it_at_every_frame_the_trace_has() {
        // **The phase's own test of the run's music start.** A `SECOND 5` entry point —
        // `--block 5`'s plan, whose first part *is* the entry point — driven in process
        // with the real player and stub parts: what is under test is the shell's music, and
        // the music does not depend on what the parts draw. Every frame of
        // `reference/music/second5` from the run's own start on is compared, because a
        // music start four frames off moves every row change in the run and only a
        // frame-by-frame comparison sees it.
        let plan = block_plan('5').unwrap();
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            plan,
            stub_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        assert_eq!(run.anchor(), Some(404), "block 5 draws from the boot's 404");
        assert_eq!(
            run.music_start_frame(),
            Some(MUSIC_START_FRAME),
            "and the music's own clock starts at 408"
        );

        let trace = trace("second5");
        let last = trace.iter().map(|(frame, _, _)| *frame).max().unwrap();
        let start = run.index();
        let wanted: std::collections::BTreeMap<u32, (u8, u8)> = trace
            .iter()
            .filter(|(frame, _, _)| *frame >= start)
            .map(|(frame, position, _)| (*frame, *position))
            .collect();
        let wanted_ticks: std::collections::BTreeMap<u32, i32> = trace
            .iter()
            .filter(|(frame, _, _)| *frame >= start)
            .map(|(frame, _, zframe)| (*frame, *zframe))
            .collect();

        // The run's own published position, one sample a frame, for every frame the trace
        // holds.
        let mut port: std::collections::BTreeMap<u32, (u8, u8)> =
            std::collections::BTreeMap::new();
        // The port's own tick counter, `_np_zframe`, frame by frame: the second support for
        // "the port's player is the trace's player", and the one the docs quote.
        let mut port_ticks: std::collections::BTreeMap<u32, i32> =
            std::collections::BTreeMap::new();
        while run.frame_index().is_none_or(|index| index < last) {
            if run.tick() == SequenceStatus::Finished {
                break;
            }
            let index = run.index() - 1;
            if wanted.contains_key(&index) {
                let position = run.music_position().expect("block 5 has a player");
                port.insert(index, (position.order, position.row));
                let player = run.audio_player().expect("block 5 has a player");
                port_ticks.insert(index, crate::audio::lock(&player).zframe());
            }
        }
        assert_eq!(
            port.len(),
            wanted.len(),
            "every frame the trace holds was visited"
        );

        // 1. **Placement.** Every position the port publishes is one the trace publishes at
        //    that frame or within the jitter the two filed sessions of the recipe measure on
        //    each other. This is the check that pins the *anchor*: a music start four frames
        //    out moves every row change in the run.
        let nearest = |frame: u32, position: (u8, u8)| -> Option<u32> {
            wanted
                .iter()
                .filter(|(_, value)| **value == position)
                .map(|(at, _)| at.abs_diff(frame))
                .min()
        };
        let mut exact = 0usize;
        let mut worst = 0u32;
        let mut unknown: Vec<(u32, (u8, u8))> = Vec::new();
        for (frame, position) in &port {
            match nearest(*frame, *position) {
                Some(0) => exact += 1,
                Some(distance) => worst = worst.max(distance),
                None => unknown.push((*frame, *position)),
            }
        }
        println!(
            "second5: {exact} of {} frames agree exactly ({:.3} %); worst displacement \
             {worst} frame(s)",
            port.len(),
            100.0 * exact as f64 / port.len() as f64
        );
        // A position the trace publishes nowhere in its window is a run whose music is
        // *ahead* of the trace's — the tail of a run started early — and it is a different
        // fault from a displacement, so it is named as one.
        assert!(
            unknown.is_empty(),
            "the run published {unknown:?}, which the trace publishes nowhere in frames \
             {}..={last} — its music is ahead of the trace's",
            start,
        );
        assert!(
            worst <= JITTER_FRAMES,
            "the worst displacement is {worst} frames, more than the {JITTER_FRAMES} the \
             two filed sessions measure on each other (A12)"
        );

        //    **The position-change distribution**, which is what says the −1 is a sub-sample
        //    phase effect rather than a shifted clock: for each position the port and the
        //    trace both publish, the frame each first publishes it on. The port's ticks land
        //    at the frame's *start* and the original's poll a fifth of a frame in, so the
        //    port is one frame early whenever the change falls in that fifth — and exactly on
        //    time when it does not. `endlogo.md` §5.2 and D28 quote this printout.
        let first_frame = |pairs: &std::collections::BTreeMap<u32, (u8, u8)>| {
            let mut out: Vec<((u8, u8), u32)> = Vec::new();
            let mut last: Option<(u8, u8)> = None;
            for (frame, position) in pairs {
                if last != Some(*position) {
                    out.push((*position, *frame));
                    last = Some(*position);
                }
            }
            out
        };
        let port_changes = first_frame(&port);
        let trace_changes: std::collections::BTreeMap<(u8, u8), u32> =
            first_frame(&wanted).into_iter().collect();
        let mut by_delta: std::collections::BTreeMap<i64, usize> = std::collections::BTreeMap::new();
        let mut compared_changes = 0usize;
        for (position, frame) in &port_changes {
            // The trace's own first frame for this position, when it has one in the window
            // *and* the port's is not the window's first sample.
            if let Some(at) = trace_changes.get(position) {
                if *frame > start {
                    by_delta
                        .entry(i64::from(*frame) - i64::from(*at))
                        .and_modify(|count| *count += 1)
                        .or_insert(1);
                    compared_changes += 1;
                }
            }
        }
        println!(
            "second5: {compared_changes} position change(s) compared; the port's frame minus \
             the trace's: {:?}",
            by_delta
        );
        assert!(
            by_delta.keys().all(|delta| (-2..=0).contains(delta)),
            "a position change more than two frames from the trace's is a shifted clock, not \
             the sample instant: {by_delta:?}"
        );

        // 2. **The tick counter.** `_np_zframe` is the other half of the position the parts
        //    read (D26: it is the clock, not the ring), and the port's is compared against
        //    the trace's own column frame by frame. **The one-index shift the sample instant
        //    makes.** The
        // trace's sample for a frame is taken at that frame's *start*, before the copper
        // polls the player for it (D17: the emulator publishes a frame's pixels and counts
        // it immediately afterwards), so the port's tick `N` is the trace's frame `N + 1` —
        // the same shift the published positions have. `ZFRAME_SLOP` is what is left: the
        // sample is within a fifth of a frame of the poll and the two clocks' residuals over
        // this window are a fraction of a tick (A12), so a tick either way is the measurement
        // and not a tolerance of convenience.
        let mut off_ticks: Vec<(u32, i32)> = Vec::new();
        let mut compared = 0usize;
        for (frame, ticks) in &port_ticks {
            let want = u32::try_from(i64::from(*frame) + 1)
                .ok()
                .and_then(|at| wanted_ticks.get(&at).copied());
            if let Some(want) = want {
                compared += 1;
                if *ticks != want {
                    off_ticks.push((*frame, ticks - want));
                }
            }
        }
        let worst_ticks = off_ticks.iter().map(|(_, d)| d.abs()).max().unwrap_or(0);
        println!(
            "second5: {} of {compared} frames agree on `zframe` exactly (port index N vs \
             trace frame N+1); worst displacement {worst_ticks} tick(s)",
            compared - off_ticks.len()
        );
        assert!(compared > 800, "the walk covered {compared} frames");
        assert!(
            worst_ticks <= ZFRAME_SLOP,
            "the worst `zframe` displacement is {worst_ticks} ticks, more than the \
             {ZFRAME_SLOP} the sample instant and the two clocks' residuals allow (A12)"
        );

        //    **And the strict same-index reading, printed beside the shifted one.** A reader
        //    who has not met the sample instant compares port index `N` against the trace's
        //    *same* frame `N`, and that reading is what falsifies the shift: if the one-index
        //    offset were an artefact of a drifting clock rather than of where in the frame the
        //    sample is taken, the strict reading's displacement would grow with the window
        //    instead of sitting at the shift itself. The bound is `ZFRAME_SLOP + 1` — the
        //    shift, plus the tick either way the shifted reading already allows — so "exactly
        //    the shift, not a drift" is a measurement rather than a phrase.
        let mut strict_off: Vec<i32> = Vec::new();
        let mut strict_compared = 0usize;
        for (frame, ticks) in &port_ticks {
            if let Some(want) = wanted_ticks.get(frame) {
                strict_compared += 1;
                if *ticks != *want {
                    strict_off.push(ticks - want);
                }
            }
        }
        let strict_worst = strict_off.iter().map(|d| d.abs()).max().unwrap_or(0);
        println!(
            "second5: {} of {strict_compared} frames agree on `zframe` at the strict same \
             index (no shift); worst displacement {strict_worst} tick(s)",
            strict_compared - strict_off.len()
        );
        assert!(strict_compared > 800, "the strict walk covered {strict_compared} frames");
        assert!(
            strict_worst <= ZFRAME_SLOP + 1,
            "the strict same-index reading is {strict_worst} ticks out, more than the shift's \
             own one plus the {ZFRAME_SLOP} the shifted reading allows — which is a drifting \
             clock rather than a sample instant (A12)"
        );

        // And then **content**: the sequence of positions the port publishes is the
        // trace's, so a dropped row, an invented row or an order published for the wrong
        // pattern shows up however the frames line up. Placement first on purpose: a music
        // start in the wrong frame leaves the *values* almost right (only the tail moves)
        // and the displacement is the diagnosis a reader wants.
        assert_eq!(
            distinct(port.values().copied()),
            distinct(wanted.values().copied()),
            "the run's positions, in order, are not the trace's"
        );
    }

    #[test]
    fn a_block_one_run_starts_under_the_initial_play_song_and_matches_second1() {
        // **The block-1 recipe's measurement, against the trace that fits it.** A
        // `SECOND 1` plan begins at ALKU at 404; the music's own clock starts at the
        // initial `_zplaysong(0)`, measured at 399.6 and used as 400 — four frames
        // before the run draws — so the run pre-polls four frames. The consequence is
        // measured, not assumed: every published position `second1` holds is compared.
        let plan = block_plan('1').unwrap();
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            plan,
            stub_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        assert_eq!(run.anchor(), Some(404), "block 1 draws from the boot's 404");
        // The fitted 400 is before the run's first frame, so the clock is caught up to
        // 404 by four pre-polls; `frame` is where it stands in the run's numbering.
        assert_eq!(
            run.music_start_frame(),
            Some(404),
            "the run's own start, with the four 400-403 frames pre-polled"
        );

        let trace = trace("second1");
        let last = trace.iter().map(|(frame, _, _)| *frame).max().unwrap();
        let start = run.index();
        let wanted: std::collections::BTreeMap<u32, (u8, u8)> = trace
            .iter()
            .filter(|(frame, _, _)| *frame >= start)
            .map(|(frame, position, _)| (*frame, *position))
            .collect();
        let mut port: std::collections::BTreeMap<u32, (u8, u8)> =
            std::collections::BTreeMap::new();
        while run.frame_index().is_none_or(|index| index < last) {
            if run.tick() == SequenceStatus::Finished {
                break;
            }
            let index = run.index() - 1;
            if wanted.contains_key(&index) {
                let position = run.music_position().expect("block 1 has a player");
                port.insert(index, (position.order, position.row));
            }
        }
        assert_eq!(port.len(), wanted.len(), "every frame the trace holds was visited");

        // The *change frames*: where the port's position first differs from its previous
        // one, against the trace's own. The four pre-polls leave the port's music a few
        // frames behind the trace's direct reading — its change frames are 2-4 later, as
        // the printed distribution shows — so the measured displacement is printed and
        // bounded rather than assumed zero.
        let changes = |values: &std::collections::BTreeMap<u32, (u8, u8)>| {
            let mut out: std::collections::BTreeMap<(u8, u8), u32> =
                std::collections::BTreeMap::new();
            let mut previous = None;
            for (frame, position) in values {
                if previous.is_some() && previous != Some(*position) {
                    out.entry(*position).or_insert(*frame);
                }
                previous = Some(*position);
            }
            out
        };
        let port_changes = changes(&port);
        let trace_changes = changes(&wanted);
        let mut by_delta: std::collections::BTreeMap<i64, usize> =
            std::collections::BTreeMap::new();
        for (position, frame) in &port_changes {
            if let Some(at) = trace_changes.get(position) {
                if *frame > start && *at > start {
                    *by_delta
                        .entry(i64::from(*frame) - i64::from(*at))
                        .or_insert(0) += 1;
                }
            }
        }
        println!(
            "second1: {} position change(s) compared; the port's frame minus the trace's:              {:?}",
            by_delta.values().sum::<usize>(),
            by_delta
        );
        // The code transitions are what ALKU's gates read; compare them directly.
        let codes = |values: &std::collections::BTreeMap<u32, (u8, u8)>| {
            let mut out: std::collections::BTreeMap<i32, u32> =
                std::collections::BTreeMap::new();
            for (frame, (order, row)) in values {
                out.entry(sr_engine::music::sync_from(*order, *row)).or_insert(*frame);
            }
            out
        };
        let port_codes = codes(&port);
        let trace_codes = codes(&wanted);
        let mut code_delta: std::collections::BTreeMap<i64, i64> =
            std::collections::BTreeMap::new();
        for (code, frame) in &port_codes {
            if let Some(at) = trace_codes.get(code) {
                if *frame > start && *at > start {
                    code_delta.insert(i64::from(*code), i64::from(*frame) - i64::from(*at));
                }
            }
        }
        println!("second1: trace code transitions {trace_codes:?}");
        println!("second1: code transition deltas (port minus trace): {code_delta:?}");
        assert!(
            by_delta.keys().all(|delta| (-3..=9).contains(delta)),
            "a position change outside the pre-poll's measured displacement is a shifted              clock, not the fitted start: {by_delta:?}"
        );
        assert!(
            code_delta.values().all(|delta| (-3..=9).contains(delta)),
            "a start code outside the pre-poll's measured displacement: {code_delta:?}"
        );
        // And the values, in order, are the trace's.
        assert_eq!(
            distinct(port.values().copied()),
            distinct(wanted.values().copied()),
            "the run's positions, in order, are not the trace's"
        );
    }

    // --- the music's own start ----------------------------------------------

    #[test]
    fn the_music_waits_for_its_anchor_and_catches_up_past_it() {
        // The anchor is 408 in the reference's frame space, and a run's own start decides
        // which side of it the run is on.
        assert_eq!(MUSIC_START_FRAME, 408);
        // A block entry: the run draws from 404, four frames before the music starts. The
        // player is installed and seeked at the run's start, and waits.
        assert_eq!(
            MusicStart::new(Some(404)),
            MusicStart { frame: 408, waiting: 4, pre: 0 }
        );
        // A run that begins exactly on the anchor waits for nothing.
        assert_eq!(
            MusicStart::new(Some(408)),
            MusicStart { frame: 408, waiting: 0, pre: 0 }
        );
        // A run that begins after it catches up: `SECOND 3`'s MNTSCRL draws from about 417
        // (the part's first graphics frame is in (415, 420], `reference-indices.md`), and
        // the music would have ticked through the frames the run never drew.
        assert_eq!(
            MusicStart::new(Some(417)),
            MusicStart { frame: 417, waiting: 0, pre: 9 }
        );
        assert!(MusicStart::new(Some(417)).pre > 0);
        // A run with a numbering of its own has no reference frame to synchronise to: its
        // music starts with it, on its frame 0.
        assert_eq!(
            MusicStart::new(None),
            MusicStart { frame: 0, waiting: 0, pre: 0 }
        );
        // A run that waits for the anchor is a run whose music is *late*: the player is
        // there, and nothing polls it until the frame the original's first tick falls on.
        assert_eq!(MusicStart::new(Some(404)).waiting, 4);
        assert_eq!(MusicStart::new(Some(404)).pre, 0);

        // **The block-1 recipe**: `SECOND 1` never takes a block restart, so its
        // player's clock starts at the initial `_zplaysong(0)`, measured at
        // 399.6 and used as 400. A run anchored at 404 is four frames *after*
        // it, and pre-polls them.
        assert_eq!(BLOCK_ONE_MUSIC_START_FRAME, 400);
        // `frame` is where the clock stands *in the run's numbering*: four
        // pre-polls put it at the run's own 404.
        assert_eq!(
            MusicStart::for_part("alku", Some(404)),
            MusicStart { frame: 404, waiting: 0, pre: 4 }
        );
        assert_eq!(
            MusicStart::for_part("u2a", Some(404)),
            MusicStart { frame: 404, waiting: 0, pre: 4 }
        );
        assert_eq!(
            MusicStart::for_part("pam", Some(404)),
            MusicStart { frame: 404, waiting: 0, pre: 4 }
        );
        // **The catch-up run the port now has**: a `--part pam` run starts at
        // the reference's 7710, 7,310 frames after the block-1 clock's start,
        // and pre-polls all of them.
        assert_eq!(
            MusicStart::for_part("pam", Some(PAM_ANCHOR)),
            MusicStart { frame: PAM_ANCHOR, waiting: 0, pre: 7310 }
        );
        // The restart recipes keep 408: BEGLOGO and ENDLOGO wait four frames.
        assert_eq!(
            MusicStart::for_part("beglogo", Some(404)),
            MusicStart { frame: 408, waiting: 4, pre: 0 }
        );
        assert_eq!(
            MusicStart::for_part("endlogo", Some(404)),
            MusicStart { frame: 408, waiting: 4, pre: 0 }
        );
    }

    #[test]
    fn the_mid_run_restart_waits_the_restart_recipes_four_ticks() {
        // **The restart sites.** `MAIN/U2.ASM:880-885`'s
        // `restartmus(ax=0, bx=18, cx=45000)` is the demo's `SECOND 4` entry and the
        // mid-plan restart every block-1-to-3 run takes before U2E: its guard is
        // `whattorun & 8` alone (`:880`), and every block whose plan reaches U2E
        // carries bit 3 — so the site is keyed by the part's presence rather than by a
        // block. `:813-818`'s `restartmus(ax=1, bx=0, cx=48700)` is block 1's own,
        // between BEGLOGO and GLENZ (`MUSIC_RESTART_AT`'s second row).
        assert_eq!(
            MUSIC_RESTART_AT,
            &[
                ("u2e", ModuleId::Music0, 18),
                ("glenz", ModuleId::Music1, 0),
            ],
            "each part, its module and the order the restart seeks"
        );
        assert_eq!(
            music_for('4').unwrap(),
            (MUSIC_RESTART_AT[0].1, MUSIC_RESTART_AT[0].2),
            "the site's row is block 4's `restartmus`, the same call"
        );
        // And each is the music its part is reached under, by the entry table's own answer.
        assert_eq!(music_for_part("u2e"), Some((ModuleId::Music0, 18)));
        assert_eq!(music_for_part("glenz"), Some((ModuleId::Music1, 0)));

        // **D34's measured wait.** A restarted player is seeked at the boundary and its
        // clock starts four engine ticks later — `MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME`,
        // the restart recipes' measured 404→408 relation — with nothing pre-polled, because
        // the run is already at the boundary when the player is installed.
        assert_eq!(MUSIC_START_FRAME - FIRST_GRAPHICS_FRAME, 4);
        assert_eq!(
            MusicStart::restart(17_418),
            MusicStart { frame: 17_418, waiting: 4, pre: 0 }
        );
        assert_eq!(
            MusicStart::restart(0),
            MusicStart { frame: 0, waiting: 4, pre: 0 },
            "an unanchored run's boundary is its own frame 0"
        );
    }

    #[test]
    fn every_part_of_the_plan_starts_under_its_blocks_music_recipe() {
        // `MAIN/U2.ASM:790-799` runs ALKU, U2A and PAM before any restart; every
        // other entry point is one of the restart sites.
        for part in ["alku", "u2a", "pam"] {
            assert_eq!(music_start_frame_for(part), BLOCK_ONE_MUSIC_START_FRAME, "{part}");
        }
        for part in ["beglogo", "glenz", "tunneli", "techno", "panicend", "mntscrl",
                     "lnszoom", "plzpart", "minvball", "rayscrl", "3dsinfld", "jplogo",
                     "u2e", "ddstars", "endlogo", "cred", "endscrl", "testcard"] {
            assert_eq!(music_start_frame_for(part), MUSIC_START_FRAME, "{part}");
        }
        // The table covers every part in the demo's order.
        for (part, _) in PART_ORDER {
            assert!(music_start_frame_for(part) == BLOCK_ONE_MUSIC_START_FRAME
                || music_start_frame_for(part) == MUSIC_START_FRAME);
        }
    }

    #[test]
    fn a_blocks_parts_are_the_demos_own_order_from_that_blocks_first_part() {
        // SECOND 5 is partsmask[5] = 0x10, bit 4 only: the three scrollers, in
        // the order the runner calls them.
        assert_eq!(block_plan('5').unwrap(), ["endlogo", "cred", "endscrl"]);
        // SECOND 2 is 0x1e: BEGLOGO onwards, and none of the "start" block that
        // precedes it.
        let plan = block_plan('2').unwrap();
        assert_eq!(plan.first(), Some(&"beglogo"));
        assert_eq!(plan.last(), Some(&"endscrl"));
        assert!(!plan.contains(&"alku"));
    }

    #[test]
    fn the_hidden_parts_block_runs_the_hidden_part_alone() {
        // SECOND U is partsmask[8] = 0x80, and bit 7 gates DDSTARS only.
        assert_eq!(block_plan('U').unwrap(), ["ddstars"]);
        // The code accepts either case (`MAIN/U2.ASM:613-616`, `:626-627`).
        assert_eq!(block_plan('u').unwrap(), ["ddstars"]);
    }

    #[test]
    fn a_block_the_original_rejects_is_not_a_block() {
        // `checkcmdline` sets errorcode 2 for '0' and '6'-'9' and runs nothing.
        for rejected in ['0', '6', '7', '8', '9', 'x'] {
            assert!(block_plan(rejected).is_err(), "{rejected} is not a block");
        }
    }

    #[test]
    fn every_part_in_the_run_order_is_in_some_blocks_plan() {
        // The order and the blocks are two transcriptions of one file, and a
        // part that no block selects would be unreachable in the port while
        // the original runs it.
        for (part, gate) in PART_ORDER {
            let mut reached = false;
            for (block, mask) in BLOCKS {
                let plan = block_plan(*block).unwrap();
                if plan.contains(part) {
                    reached = true;
                    // A block that runs a part must have the part's own bit
                    // set: the plan is the mask's business, not a second list.
                    assert!(gate & mask != 0, "{block} runs {part} without its bit");
                }
            }
            assert!(reached, "{part} is in no block and the port can never run it");
        }
    }

    #[test]
    fn glenz_resolves_under_part() {
        // GLENZ is registered (Task 5) and Task 6 gives it its music row: a
        // `--part glenz` run is the part's own, numbered from zero, under the
        // module and order both block sequences reach it at (`:772-775` for
        // block 2, the `:813-818` restart for block 1).
        assert_eq!(resolve("glenz").unwrap(), "glenz");
        assert_eq!(crate::parts::registered("glenz"), Some("glenz"));
        assert_eq!(music_for_part("glenz"), Some((ModuleId::Music1, 0)));
        assert_eq!(boot_anchor("glenz"), None, "no measured first drawn frame");
        let run = sequence_of(vec!["glenz"]).unwrap();
        assert_eq!(run.index(), 0);
        assert!(!run.index_is_reference(), "its own numbering, and it says so");
    }

    #[test]
    fn resolve_rejects_a_name_the_demo_never_had() {
        // Every name in the demo's run order is registered now (GLENZ was the
        // last), so `resolve` can no longer show the `NotPorted` arm: the only
        // name left for it to answer is one that is not a part at all. The
        // unported arm is still exercised through a source that declines a demo
        // name (`a_run_that_stopped_says_which_part_it_stopped_at`), which is
        // where a run meets it.
        assert_eq!(resolve("testcard").unwrap(), "testcard");
        assert!(matches!(resolve("nope"), Err(ShellError::UnknownPart { name }) if name == "nope"));
        // An empty name is a name that matched nothing, and the message has to
        // say *which* name was empty: `--part ""` is a shell variable that was
        // never set, and "a run needs at least one part" would send its author
        // looking at the wrong argument.
        assert_eq!(resolve("").unwrap_err().to_string(), "unknown part \"\"");
    }

    // --- the sequence -----------------------------------------------------

    /// The parts these tests can run now. The shell's numbering is this task's
    /// and the parts it numbers are Tasks 4-9's, so the run is driven through
    /// [`Sequence::with_source`] with stubs that stand in for them: what is
    /// being tested is the counter, not the picture.
    const TEST_SURFACE: usize = 8;

    fn stub_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            // Stands in for BEGLOGO: an anchored block entry. It fills the
            // screen, the way BEGLOGO's `memset(vram,15,...)` does.
            "beglogo" => Some(Box::new(Stub { name: "beglogo", fill: Some(7), retires: None })),
            "a" => Some(Box::new(Stub { name: "a", fill: Some(7), retires: Some(1) })),
            // Draws nothing at all: whatever the screen holds when it starts is
            // what its frames show, which is how the handover is tested.
            "b" => Some(Box::new(Stub { name: "b", fill: None, retires: Some(0) })),
            // Never retires, and does not drain: the shell must retire it on
            // the exit tick.
            "holds" => Some(Box::new(Stub { name: "holds", fill: None, retires: None })),
            // A demo part that retires at once, so a run of it reaches the hold and its
            // music's fade. `pam` is the demo's own part and no block *starts* at it, which
            // is why it is played from the demo's beginning (MUSIC0 at order 0).
            "pam" => Some(Box::new(Stub { name: "pam", fill: None, retires: Some(0) })),
            // The part Task 6 gives its music row: it never retires, so a run that
            // reaches it stays there for the music assertions to read.
            "glenz" => Some(Box::new(Stub { name: "glenz", fill: None, retires: None })),
            // PANICEND: it fills the surface and retires at once, so a run reaches the
            // shell's cue wait with a picture to hold.
            "panicend" => Some(Box::new(Stub {
                name: "panicend",
                fill: Some(9),
                retires: Some(0),
            })),
            // `--block 1`'s first part, for the test that watches the *block-1* music:
            // it draws nothing and never retires, so the run ticks to the trace's last
            // frame with the numbering its anchor gave it.
            "alku" => Some(Box::new(Stub { name: "alku", fill: None, retires: None })),
            // `--block 5`'s plan, for the test that watches the *music*: the three
            // scrollers draw nothing and never retire, so the run ticks to the trace's
            // last frame with the numbering the block gave it.
            "endlogo" => Some(Box::new(Stub { name: "endlogo", fill: None, retires: None })),
            "cred" => Some(Box::new(Stub { name: "cred", fill: None, retires: None })),
            "endscrl" => Some(Box::new(Stub { name: "endscrl", fill: None, retires: None })),
            _ => None,
        }
    }

    /// Fills the surface with `fill` every frame — or draws nothing, for
    /// `None` — and reports `Finished` from frame `retires` on, so a stub of
    /// `Some(1)` draws two frames.
    struct Stub {
        name: &'static str,
        fill: Option<u8>,
        retires: Option<u32>,
    }

    impl Part for Stub {
        fn name(&self) -> &'static str { self.name }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            if let Some(fill) = self.fill {
                ctx.surface.clear(fill);
            }
            match self.retires {
                Some(from) if ctx.frame >= from => PartStatus::Finished,
                _ => PartStatus::Continuing,
            }
        }
    }

    fn sequence_of(plan: Vec<&'static str>) -> Result<Sequence, ShellError> {
        Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            plan,
            stub_source,
        )
    }

    fn for_part(name: &'static str) -> Result<Sequence, ShellError> {
        sequence_of(vec![name])
    }

    fn for_block(block: char, ported: &[&str]) -> Result<Sequence, ShellError> {
        let mut plan = block_plan(block)?;
        let stops = plan.iter().position(|part| !ported.contains(part));
        plan.truncate(stops.unwrap_or(plan.len()));
        sequence_of(plan)
    }

    /// Ticks to the end of the run, or to `limit` ticks, whichever comes first.
    fn run_out(run: &mut Sequence, limit: u32) -> u32 {
        let mut ticks = 0;
        while ticks < limit {
            ticks += 1;
            if run.tick() == SequenceStatus::Finished {
                break;
            }
        }
        ticks
    }

    #[test]
    fn the_index_starts_at_the_anchor_and_advances_one_per_tick() {
        let mut run = for_block('2', &["beglogo"]).unwrap();
        assert_eq!(run.index(), 404);
        run.tick();
        assert_eq!(run.index(), 405);
    }

    #[test]
    fn a_part_with_no_anchor_numbers_its_own_frames_and_says_they_are_not_the_references() {
        let run = sequence_of(vec!["a"]).unwrap();
        assert_eq!(run.index(), 0);
        assert!(!run.index_is_reference());
        assert_eq!(run.anchor(), None);
    }

    #[test]
    fn a_run_that_began_at_an_anchored_part_says_its_numbering_is_the_references() {
        let run = for_part("beglogo").unwrap();
        assert!(run.index_is_reference());
        assert_eq!(run.anchor(), Some(404));
    }

    #[test]
    fn the_frame_a_tick_produces_is_one_behind_the_index() {
        // The index is the run's cursor: `new` puts it at the anchor, so the
        // first tick draws the anchored frame and the cursor moves past it.
        let mut run = for_part("beglogo").unwrap();
        run.tick();
        assert_eq!(run.frame().pixel(0, 0), 7, "the first tick drew a frame");
        assert_eq!(run.index(), 405, "and the frame it drew was the one at 404");
    }

    #[test]
    fn a_retired_part_hands_the_screen_to_the_next_without_clearing_it() {
        // The original's parts are separate EXEs that leave the screen as it
        // is; BEGLOGO's 900 is that fact, and it belongs to the loader and the
        // next part's wait, not to BEGLOGO's own loop (§3.5).
        let mut run = sequence_of(vec!["a", "b"]).unwrap();
        run_out(&mut run, 8);
        assert_eq!(run.part(), "b");
        assert!(
            run.frame().pixels.iter().all(|pixel| *pixel == 7),
            "the second part cleared the screen the first one handed it"
        );
    }

    #[test]
    fn a_block_run_stops_at_the_first_part_the_port_cannot_run() {
        // `SECOND 2` runs BEGLOGO then GLENZ; GLENZ is not ported, so the run
        // stops at BEGLOGO rather than silently skipping to TUNNELI and
        // producing frames in a numbering nothing supports.
        let mut run = for_part("beglogo").unwrap();
        run_out(&mut run, 8);
        assert_eq!(run.part(), "beglogo");

        // And with a demo name the source declines (TECHNO is registered now, but
        // this stub does not build it): the run holds where it stands rather than
        // skipping over the part.
        let mut run = sequence_of(vec!["a", "techno"]).unwrap();
        run_out(&mut run, 8);
        assert_eq!(run.part(), "a", "the part the source lacks must not be skipped to");
        assert!(!run.finished(), "the port ran out of parts; the run has not ended");
        assert_eq!(run.index(), 8, "and it holds rather than stopping: the index keeps counting");
    }

    /// A one-frame stub for every name the registry builds, and nothing for the demo's
    /// names the port has not reached: the registry, not a list in the test below, is
    /// what decides which is which.
    fn registered_stub_source(name: &str) -> Option<Box<dyn Part>> {
        crate::parts::registered(name)
            .map(|name| Box::new(Stub { name, fill: None, retires: Some(0) }) as Box<dyn Part>)
    }

    #[test]
    fn every_blocks_plan_stops_at_the_first_part_the_registry_lacks() {
        // The plan is the demo's own sequence and is not truncated (`block_plan`); where
        // a run stops is `take_next`'s answer when the part source refuses the next name.
        // The expected stop is the first plan name `parts.rs` does not register, so a
        // part that lands moves the stop with it — and the run says which name it could
        // not go on to (`stopped_at`, which the window's status line prints as
        // "holding; <name> is not ported yet").
        //
        // This is the block-level claim the end-to-end runs rest on: **every block's plan
        // is now fully ported** — GLENZ was the last name any of them lacked (Phase 7) — so
        // each plan runs to its own end and stops nowhere. Block 1 runs ALKU → … → GLENZ →
        // … → ENDSCRL, block 2 BEGLOGO → GLENZ → … → ENDSCRL, block 3 its whole tier and
        // the end sequence (U2E's bit 8 is in 0x1c), block 4 U2E and the scrollers, block 5
        // the scrollers and block U the hidden part alone.
        let expected: &[(char, Option<&str>)] = &[
            ('1', None),
            ('2', None),
            ('3', None),
            ('4', None),
            ('5', None),
            ('U', None),
        ];
        for (block, stop) in expected {
            let plan = block_plan(*block).unwrap();
            let first_missing = plan
                .iter()
                .copied()
                .find(|name| crate::parts::registered(name).is_none());
            assert_eq!(first_missing, *stop, "{block}: the first part the port lacks");

            let prefix = plan
                .iter()
                .position(|name| Some(*name) == *stop)
                .unwrap_or(plan.len());
            if prefix == 0 {
                // A block whose *first* part is unported has no run at all: the command
                // line refuses it (`resolve`) and a hand-built sequence names the part it
                // could not start.
                match Sequence::with_source(
                    TEST_SURFACE,
                    TEST_SURFACE,
                    View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
                    plan,
                    registered_stub_source,
                ) {
                    Ok(_) => panic!("{block} begins at a part the port lacks"),
                    Err(error) => assert_eq!(
                        error.to_string(),
                        format!("{} is not ported yet", stop.unwrap()),
                        "{block}'s run does not start"
                    ),
                }
                continue;
            }

            // Every prefix part builds, and the first missing name is where the run
            // holds — with the part before it still on the screen.
            let held = plan[prefix - 1];
            let mut run = Sequence::with_source(
                TEST_SURFACE,
                TEST_SURFACE,
                View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
                plan,
                registered_stub_source,
            )
            .unwrap();
            run_out(&mut run, 64);
            assert!(run.holding(), "{block}: the run holds where the port stopped");
            assert_eq!(run.stopped_at(), *stop, "{block}: the part it could not go on to");
            assert_eq!(run.part(), held, "{block}: the screen it holds");
        }
    }

    #[test]
    fn a_run_past_its_last_part_holds_the_screen_that_part_left() {
        // `BEG/BEG.C:87` ends BEGLOGO's `main()` when the fade has converged,
        // and the reference's 900 is the same picture: the loader and the next
        // part's music wait are showing it. So one part and a long tick budget
        // is a run, not a run that has gone nowhere.
        let mut run = for_part("a").unwrap();
        let ticks = run_out(&mut run, 8);
        assert_eq!(ticks, 8, "the run keeps ticking after a has retired");
        assert!(!run.finished());
        assert_eq!(run.part(), "a", "the held frames are a's screen, and say so");
        assert_eq!(run.index(), 8, "and the index keeps counting");
        assert!(
            run.frame().pixels.iter().all(|pixel| *pixel == 7),
            "the hold drew over the screen it was holding"
        );
    }

    #[test]
    fn the_index_is_continuous_across_a_part_boundary() {
        // One numbering for the run, not one per part: a part that started its
        // own count would put the second part's frames back at zero, and every
        // mid-block keyframe in `reference/` is behind one.
        let mut run = sequence_of(vec!["a", "b"]).unwrap();
        run_out(&mut run, 5);
        assert_eq!(run.part(), "b");
        assert_eq!(run.index(), 5, "two frames from a, one from b, two held");
    }

    #[test]
    fn a_run_that_ended_keeps_reporting_itself_finished() {
        let mut run = failing_run();
        run_out(&mut run, 8);
        assert!(run.finished());
        assert_eq!(run.tick(), SequenceStatus::Finished);
        assert_eq!(run.tick(), SequenceStatus::Finished);
    }

    #[test]
    fn a_part_that_asks_to_quit_hands_the_screen_to_the_next() {
        // `dis_exit()` ends the part's own loop; the shell's next act is the
        // next part, which is what ESC in a window will do.
        let mut run = Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(8, 8),
            vec!["quits", "b"],
            |name| match name {
                "quits" => Some(Box::new(Quits)),
                other => stub_source(other),
            },
        )
        .unwrap();
        run_out(&mut run, 8);
        assert_eq!(run.part(), "b", "the quitter's request moved the run on");
    }

    struct Quits;

    impl Part for Quits {
        fn name(&self) -> &'static str { "quits" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            ctx.request_exit();
            PartStatus::Continuing
        }
    }

    // --- the exit drain (`U2E.C:541-555`) ------------------------------------

    /// A part with a timed teardown: it keeps drawing after the exit flag and
    /// reports `Finished` itself on its fourth tick. Its two post-exit ticks are
    /// what say the shell did not cut it short.
    struct Draining;

    impl Part for Draining {
        fn name(&self) -> &'static str { "draining" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            if ctx.frame >= 3 {
                PartStatus::Finished
            } else {
                PartStatus::Continuing
            }
        }
        fn drains_after_exit(&self) -> bool { true }
    }

    fn draining_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            "draining" => Some(Box::new(Draining)),
            other => stub_source(other),
        }
    }

    #[test]
    fn a_part_that_drains_after_exit_keeps_ticking_until_it_finishes() {
        // The exit flag is sticky in the engine, so `Sequence::tick` retires a
        // part on the exit tick unless the part opts out. U2E's source runs its
        // sixteen-retrace DAC fade *after* `dis_exit` (`U2E.C:541-555`), and
        // `drains_after_exit` is what buys it those ticks.
        let mut run = Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["draining", "b"],
            draining_source,
        )
        .unwrap();
        run.tick();
        run.tick();
        assert_eq!(run.part(), "draining");
        assert!(run.request_exit(), "a part is live to ask");
        run.tick();
        assert_eq!(run.part(), "draining", "the exit flag does not retire a draining part");
        assert_eq!(run.frame_index(), Some(2), "and the tick drew");
        run.tick();
        assert_eq!(
            run.part(),
            "draining",
            "the part's own Finished tick still drew; it retires after it"
        );
        run.tick();
        assert_eq!(run.part(), "b", "and only then does the run go on");
    }

    #[test]
    fn a_part_that_does_not_drain_retires_on_the_exit_tick() {
        // The default, and every part but U2E: a part that never polls the flag
        // is retired by the shell at the end of the tick the flag reaches it.
        let mut run = sequence_of(vec!["holds", "b"]).unwrap();
        run.tick();
        run.tick();
        assert_eq!(run.part(), "holds");
        assert!(run.request_exit());
        run.tick();
        assert_eq!(run.part(), "holds", "the exit tick still drew the part once");
        run.tick();
        assert_eq!(run.part(), "b", "the next tick has retired it");
    }

    // --- the mid-run restart (`MAIN/U2.ASM:880-885`) -------------------------

    /// What U2E's `init` saw of `music_fresh` when it was reached through the
    /// restart site. A `static` because [`PartSource`] is a plain `fn`.
    static FRESH_AT_RESTART: AtomicBool = AtomicBool::new(false);

    /// Stands in for U2E: its `init` is the context the engine's fresh flag
    /// belongs to, and it never retires.
    struct FreshProbe;

    impl Part for FreshProbe {
        fn name(&self) -> &'static str { "u2e" }
        fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
            FRESH_AT_RESTART.store(ctx.music_fresh(), Ordering::SeqCst);
            Ok(())
        }
        fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
    }

    fn restart_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            // Block 3's first ported part, retiring at once so the plan reaches
            // the boundary on the second tick.
            "mntscrl" => Some(Box::new(Stub {
                name: "mntscrl",
                fill: None,
                retires: Some(0),
            })),
            "u2e" => Some(Box::new(FreshProbe)),
            other => stub_source(other),
        }
    }

    #[test]
    fn a_restart_installs_the_next_modules_order_at_the_u2e_boundary() {
        FRESH_AT_RESTART.store(false, Ordering::SeqCst);
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["mntscrl", "u2e"],
            restart_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        // The entry install is the plan's first part's own row: MNTSCRL runs
        // under MUSIC1 at order 42 and, having no anchor, its clock starts with
        // the run.
        assert_eq!(run.music_module(), Some((ModuleId::Music1, 42)));
        assert_eq!(
            run.music.as_ref().unwrap().start,
            MusicStart { frame: 0, waiting: 0, pre: 0 }
        );
        // The shell's dump and scratch belong to the run, not to one player: a
        // restart installs a fresh player and carries both across. The marker
        // is past one poll's samples (~570) so the dump's drain before the
        // boundary cannot overwrite it — what is under test is the vector's
        // identity, not its contents.
        run.music.as_mut().unwrap().scratch[1000] = 700;
        let dir = std::env::temp_dir().join(format!("sr-shell-restart-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        run.dump_audio(&dir.join("music.wav")).unwrap();
        assert!(run.music.as_ref().unwrap().dump.is_some());
        let entry_player = std::sync::Arc::as_ptr(&run.music.as_ref().unwrap().player);

        // Tick 0: MNTSCRL draws and retires. Tick 1: the boundary — the
        // restart site installs MUSIC0@18 before U2E's `init`.
        run.tick();
        assert_eq!(run.part(), "mntscrl");
        run.tick();
        assert_eq!(run.part(), "u2e", "the boundary part started");
        assert_eq!(
            run.music_module(),
            Some((ModuleId::Music0, 18)),
            "`MAIN/U2.ASM:880-885`'s restartmus(0, 18)"
        );
        let music = run.music.as_ref().expect("the restart installed a player");
        assert_eq!(music.start.waiting, 4, "D34's measured wait");
        assert_eq!(
            music.start,
            MusicStart { frame: 1, waiting: 4, pre: 0 },
            "the boundary's own run index, four ticks of waiting, no pre-poll"
        );
        assert_eq!(music.scratch[1000], 700, "the scratch is the run's");
        assert!(music.dump.is_some(), "and so is the dump");
        assert_ne!(
            std::sync::Arc::as_ptr(&music.player),
            entry_player,
            "a fresh player, as `restartmus` loads the module again"
        );
        assert!(
            FRESH_AT_RESTART.load(Ordering::SeqCst),
            "set_music re-armed the fresh flag, and U2E's init consumed it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The number of `init` contexts the probe has seen, and how many of those
    /// `music_fresh` reported `true` to. The total is what proves the second
    /// U2E's init actually ran.
    static SAME_ORDER_INITS: AtomicU32 = AtomicU32::new(0);
    static SAME_ORDER_FRESH_INITS: AtomicU32 = AtomicU32::new(0);

    struct SameOrderProbe;

    impl Part for SameOrderProbe {
        fn name(&self) -> &'static str { "u2e" }
        fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError> {
            SAME_ORDER_INITS.fetch_add(1, Ordering::SeqCst);
            if ctx.music_fresh() {
                SAME_ORDER_FRESH_INITS.fetch_add(1, Ordering::SeqCst);
            }
            Ok(())
        }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            if ctx.frame >= 1 {
                PartStatus::Finished
            } else {
                PartStatus::Continuing
            }
        }
    }

    fn same_order_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            "u2e" => Some(Box::new(SameOrderProbe)),
            other => stub_source(other),
        }
    }

    #[test]
    fn a_run_already_on_the_restarts_module_does_not_reinstall_it() {
        // The restart fires only when the row's module/order differs from the
        // current `RunMusic`: a plan that reaches U2E with MUSIC0@18 already
        // playing keeps the one player (and its clock's position).
        //
        // The probe draws frame 0 and finishes on frame 1, so the boundary is
        // the third tick: tick 0 draws the first U2E's frame 0, tick 1 draws
        // its frame 1 and retires it, and tick 2 starts the second U2E — whose
        // init is the context the skip branch must leave un-rearmed. Ticking
        // only twice would stop on the first part and test nothing.
        SAME_ORDER_INITS.store(0, Ordering::SeqCst);
        SAME_ORDER_FRESH_INITS.store(0, Ordering::SeqCst);
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["u2e", "u2e"],
            same_order_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        assert_eq!(run.music_module(), Some((ModuleId::Music0, 18)));
        let player = std::sync::Arc::as_ptr(&run.music.as_ref().unwrap().player);
        run.tick();
        assert_eq!(run.part(), "u2e");
        run.tick();
        assert_eq!(run.part(), "u2e", "the first U2E's last frame");
        run.tick();
        assert_eq!(run.part(), "u2e", "the second U2E started");
        assert_eq!(
            SAME_ORDER_INITS.load(Ordering::SeqCst),
            2,
            "the second U2E's init ran, so the restart site was consulted"
        );
        assert_eq!(
            std::sync::Arc::as_ptr(&run.music.as_ref().unwrap().player),
            player,
            "the same player: the row's module/order did not differ"
        );
        assert_eq!(
            SAME_ORDER_FRESH_INITS.load(Ordering::SeqCst),
            1,
            "only the entry install armed the fresh flag"
        );
    }

    /// Block 1's first part and GLENZ: the block's own `_zplaysong(0)` runs the
    /// first, and the `:813-818` restart precedes the second.
    fn block_one_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            "alku" => Some(Box::new(Stub { name: "alku", fill: None, retires: Some(0) })),
            other => stub_source(other),
        }
    }

    #[test]
    fn the_block_one_run_restarts_music1_at_glenz() {
        // Block 1 runs under the initial `_zplaysong(0)` — MUSIC0 at order 0 with
        // the block-1 clock's 400 (`MAIN/U2.ASM:766-775`, `:790-799`) — until the
        // `:813-818` site between BEGLOGO and GLENZ restarts `restartmus(1, 0)`
        // under a fresh player. A plan of ALKU then GLENZ is that sequence's
        // shortest form.
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["alku", "glenz"],
            block_one_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        assert_eq!(
            run.music_module(),
            Some((ModuleId::Music0, 0)),
            "block 1's entry is the initial start, not a restart"
        );
        assert_eq!(
            run.music.as_ref().unwrap().start,
            MusicStart { frame: 404, waiting: 0, pre: 4 },
            "the block-1 clock starts at 400 and the run's 404 pre-polls the four frames"
        );
        let entry_player = std::sync::Arc::as_ptr(&run.music.as_ref().unwrap().player);

        // Tick 0: ALKU draws and retires. Tick 1: the boundary — the restart
        // installs MUSIC1@0 before GLENZ's `init`, and the part starts.
        run.tick();
        assert_eq!(run.part(), "alku");
        run.tick();
        assert_eq!(run.part(), "glenz", "the boundary part started");
        assert_eq!(
            run.music_module(),
            Some((ModuleId::Music1, 0)),
            "`MAIN/U2.ASM:813-818`'s restartmus(1, 0)"
        );
        let music = run.music.as_ref().expect("the restart installed a player");
        assert_eq!(music.start.waiting, 4, "D38: the restart's four-tick wait");
        assert_eq!(
            music.start,
            MusicStart { frame: 405, waiting: 4, pre: 0 },
            "the boundary's own run index, four ticks of waiting, no pre-poll"
        );
        assert_ne!(
            std::sync::Arc::as_ptr(&music.player),
            entry_player,
            "a fresh player, as `restartmus` loads the module again"
        );
    }

    /// Block 2's first part and GLENZ: the block already plays MUSIC1@0.
    fn block_two_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            "beglogo" => Some(Box::new(Stub { name: "beglogo", fill: None, retires: Some(0) })),
            other => stub_source(other),
        }
    }

    #[test]
    fn the_block_two_run_does_not_reload_at_glenz() {
        // `SECOND 2`'s mask (`0x1e`) has bit 1 but not bit 0, so `:813-814`'s
        // `test whattorun,1 / jz @@skip2b` skips the restart — and the module the
        // site would install is the one its initial start already sought
        // (`:772-775`: MUSIC1 at order 0). The equality check must keep the
        // player, its clock and its position.
        let mut run = Sequence::with_run(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["beglogo", "glenz"],
            block_two_source,
            Some(crate::audio::test_player),
        )
        .unwrap();
        assert_eq!(run.music_module(), Some((ModuleId::Music1, 0)));
        let entry = run.music.as_ref().unwrap().start;
        assert_eq!(entry, MusicStart { frame: 408, waiting: 4, pre: 0 });
        let entry_player = std::sync::Arc::as_ptr(&run.music.as_ref().unwrap().player);

        run.tick();
        assert_eq!(run.part(), "beglogo");
        run.tick();
        assert_eq!(run.part(), "glenz", "the boundary part started");
        assert_eq!(run.music_module(), Some((ModuleId::Music1, 0)), "still the entry's module");
        let music = run.music.as_ref().unwrap();
        assert_eq!(
            std::sync::Arc::as_ptr(&music.player),
            entry_player,
            "the same player: the run already plays the row's module and order"
        );
        assert_eq!(music.start, entry, "and the same clock, not a fresh four-tick wait");
    }

    // --- the post-PANICEND cue wait (`MAIN/U2.ASM:826-853`) -------------------

    /// A scripted [`Music`] for the cue wait: `musplus` hands out the next
    /// scripted answer a call, which is what makes the two phases' decisions
    /// readable one tick at a time.
    struct ScriptedMusic {
        plus: std::collections::VecDeque<i32>,
    }

    impl ScriptedMusic {
        fn new(plus: impl IntoIterator<Item = i32>) -> ScriptedMusic {
            ScriptedMusic { plus: plus.into_iter().collect() }
        }
    }

    impl Music for ScriptedMusic {
        fn poll(&mut self) {}
        fn position(&self) -> MusicPosition { MusicPosition::default() }
        fn musplus(&mut self) -> i32 { self.plus.pop_front().unwrap_or(-32) }
        fn musrow(&mut self) -> i32 { 0 }
        fn muscode(&mut self, wanted: i32) -> i32 {
            sr_engine::muscode_from(MusicPosition::default(), wanted)
        }
        fn sync(&mut self) -> i32 { 0 }
        fn getmframe(&mut self) -> i32 { 0 }
        fn setmframe(&mut self, _frame: i32) {}
    }

    /// The borrowed pieces a [`PartCtx`] needs, so one cue can be driven tick
    /// by tick without a `Sequence`'s engine in the way.
    struct CueRig {
        surface: Surface,
        view: View,
        palette: Palette,
        timeline: PaletteTimeline,
        copper: CopperRegistry,
    }

    impl CueRig {
        fn new() -> CueRig {
            CueRig {
                surface: Surface::new(TEST_SURFACE, TEST_SURFACE),
                view: View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
                palette: Palette::new(),
                timeline: PaletteTimeline::new(Palette::new()),
                copper: CopperRegistry::new(),
            }
        }

        fn tick(&mut self, cue: &mut CueWait, music: &mut ScriptedMusic, exit: bool) -> PartStatus {
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                Some(music),
                0,
                0,
                1,
                true,
                exit,
                false,
            );
            cue.frame(&mut ctx)
        }

        fn tick_without_music(&mut self, cue: &mut CueWait) -> PartStatus {
            let mut ctx = PartCtx::new(
                &mut self.surface,
                &mut self.view,
                &mut self.palette,
                &mut self.timeline,
                &mut self.copper,
                None,
                0,
                0,
                1,
                true,
                false,
                false,
            );
            cue.frame(&mut ctx)
        }
    }

    #[test]
    fn the_post_panicend_wait_ends_on_a_positive_musplus() {
        // `@@zh5` waits while `musplus >= 0`; `@@zh4`, entered once a negative
        // has been seen, waits while it is `<= 0` and leaves on `> 0`. The
        // scripted sequence has negatives from the first call, a zero that still
        // waits, and the positive that finishes — five calls, the last one the
        // first positive.
        let mut cue = CueWait { saw_negative: false };
        let mut rig = CueRig::new();
        let mut music = ScriptedMusic::new([-3, -1, -32, 0, 1]);
        let statuses: Vec<PartStatus> =
            (0..5).map(|_| rig.tick(&mut cue, &mut music, false)).collect();
        assert_eq!(
            statuses,
            [
                PartStatus::Continuing,
                PartStatus::Continuing,
                PartStatus::Continuing,
                PartStatus::Continuing,
                PartStatus::Finished,
            ],
            "the fifth call is the first positive"
        );

        // **The measured calibration** (`reference/music/second2`, D38): PANICEND
        // retires at boot 8903 with musplus 19 — non-negative, so `@@zh5` keeps
        // waiting rather than ending there — 8953 is the first negative (−32 at
        // row 32, zplus 2), and 9219 is the first positive (order 42 row 1 → 1).
        assert_eq!(sr_engine::musplus_from(2, 19), 19);
        assert_eq!(sr_engine::musplus_from(2, 32), -32);
        assert_eq!(sr_engine::musplus_from(2, 1), 1);
        // The same wait with the measured 8903 value first: the non-negative
        // reading is the *first* phase's condition, not a reason to finish.
        let mut cue = CueWait { saw_negative: false };
        let mut music = ScriptedMusic::new([19, 19, -32, 0, 1]);
        let statuses: Vec<PartStatus> =
            (0..5).map(|_| rig.tick(&mut cue, &mut music, false)).collect();
        assert_eq!(
            statuses,
            [
                PartStatus::Continuing,
                PartStatus::Continuing,
                PartStatus::Continuing,
                PartStatus::Continuing,
                PartStatus::Finished,
            ],
            "the first phase waits through a positive reading"
        );

        // The exit flag skips the wait on the tick it is seen (`mov bx,2;
        // int 0fch; or ax,ax; jnz @@skip5`), and the run goes on.
        let mut cue = CueWait { saw_negative: false };
        let mut music = ScriptedMusic::new([-3]);
        assert_eq!(rig.tick(&mut cue, &mut music, true), PartStatus::Finished);
        // And a run with no player has no cue to wait for.
        let mut cue = CueWait { saw_negative: false };
        assert_eq!(rig.tick_without_music(&mut cue), PartStatus::Finished);
    }

    /// A run of PANICEND and then GLENZ, with PANICEND drawing a picture and
    /// retiring at once so the shell reaches its wait.
    fn cue_source(name: &str) -> Option<Box<dyn Part>> {
        match name {
            "panicend" => Some(Box::new(Stub { name: "panicend", fill: Some(9), retires: Some(0) })),
            other => stub_source(other),
        }
    }

    #[test]
    fn the_cue_wait_holds_the_screen() {
        let mut run = Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["panicend", "glenz"],
            cue_source,
        )
        .unwrap();
        run.engine.set_music(Box::new(ScriptedMusic::new([-32; 8])));
        run.tick();
        assert_eq!(run.part(), "panicend");
        let held = run.frame().pixels.clone();
        assert!(held.iter().all(|pixel| *pixel == 9), "PANICEND's picture is up");
        for tick in 0..4 {
            run.tick();
            assert_eq!(run.part(), "panicend", "the wait is still the shell's");
            assert!(!run.holding(), "the hold has not begun");
            assert_eq!(
                run.frame().pixels, held,
                "tick {tick} of the wait drew nothing over the surface"
            );
        }
    }

    #[test]
    fn the_post_panicend_wait_is_between_the_part_and_the_next_one() {
        // The wait is installed by `take_next` in place of the next plan part and
        // advances the plan only when it finishes: PANICEND draws on tick 1, the
        // five scripted cue ticks are 2-6, and GLENZ starts on tick 7.
        let mut run = Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["panicend", "glenz"],
            cue_source,
        )
        .unwrap();
        run.engine.set_music(Box::new(ScriptedMusic::new([-3, -1, -32, 0, 1])));
        let start = run.index();
        run.tick();
        assert_eq!(run.part(), "panicend");
        for tick in 0..5 {
            run.tick();
            assert_eq!(run.part(), "panicend", "cue tick {tick} holds PANICEND's picture");
            assert!(!run.holding());
        }
        assert_eq!(run.index(), start + 6, "one index a tick: the part, then five cue ticks");
        run.tick();
        assert_eq!(run.part(), "glenz", "the plan goes on when the cue turns positive");
        assert_eq!(run.index(), start + 7, "and the next part keeps the run's numbering");
    }

    #[test]
    fn a_plan_that_ends_at_panicend_waits_and_then_holds() {
        // The wait is installed even when PANICEND is the plan's last part; the
        // hold begins after it, as the original's `@@zh5` runs before the next
        // region rather than inside it.
        let mut run = Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["panicend"],
            cue_source,
        )
        .unwrap();
        run.engine.set_music(Box::new(ScriptedMusic::new([-32, -32, 1])));
        run.tick(); // PANICEND's one frame
        assert_eq!(run.part(), "panicend");
        run.tick(); // cue: -32
        assert_eq!(run.part(), "panicend");
        assert!(!run.holding(), "the wait comes before the hold");
        run.tick(); // cue: -32
        run.tick(); // cue: 1, the exit
        assert!(!run.holding(), "the wait retires at the end of its exit tick");
        run.tick(); // the hold
        assert!(run.holding(), "and then the run holds PANICEND's screen");
        assert_eq!(run.stopped_at(), None, "the plan ran out; no part is to blame");
        assert_eq!(run.part(), "panicend");
    }

    #[test]
    fn an_exit_on_panicends_last_tick_skips_the_cue_wait() {
        // `SHUTDOWN.C` never polls `dis_exit`, so ESC on PANICEND's final tick
        // leaves the part reporting `Finished` *and* the engine's flag set. The
        // original's `@@zh5` reads `dis_exit` before the music and jumps to
        // `@@skip5`: the wait is skipped and the sequence goes on. The port
        // must skip it too — installing the `CueWait` would call
        // `engine.start`, which clears the flag before the cue could see it.
        let mut run = Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec!["panicend", "mntscrl"],
            |name| match name {
                "panicend" => Some(Box::new(Stub {
                    name: "panicend",
                    fill: Some(9),
                    retires: Some(0),
                })),
                "mntscrl" => Some(Box::new(Stub {
                    name: "mntscrl",
                    fill: None,
                    retires: None,
                })),
                other => stub_source(other),
            },
        )
        .unwrap();
        run.engine.set_music(Box::new(ScriptedMusic::new([-32, -32, -32, -32])));
        assert!(run.request_exit(), "the running PANICEND is asked");
        run.tick();
        assert_eq!(run.part(), "panicend", "its final tick still reports Finished");
        run.tick();
        assert_eq!(run.part(), "mntscrl", "the exit skipped the cue; the plan goes on");
        assert_eq!(run.index(), 2, "two ticks, and no cue tick between them");
        assert!(!run.holding());
    }

    #[test]
    fn blocks_one_and_two_have_no_stop() {
        // Block 1 and block 2 are the two plans that reach GLENZ, and with the
        // part registered they run their whole sequence to ENDSCRL — neither
        // stops at a part, and the plan's end is the hold, not a stopped-at name.
        for block in ['1', '2'] {
            let plan = block_plan(block).unwrap();
            assert!(plan.contains(&"glenz"), "{block} reaches GLENZ");
            assert_eq!(
                plan.iter().copied().find(|name| crate::parts::registered(name).is_none()),
                None,
                "{block}: every part in the plan is registered"
            );
            let mut run = Sequence::with_source(
                TEST_SURFACE,
                TEST_SURFACE,
                View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
                plan,
                registered_stub_source,
            )
            .unwrap();
            run_out(&mut run, 64);
            assert!(run.holding(), "{block}: the plan ran out");
            assert_eq!(run.stopped_at(), None, "{block}: no part stopped it");
            assert_eq!(run.part(), "endscrl", "{block}: the last part's screen is held");
        }
    }

    // --- the keyboard (`DIS/DISINT.ASM:112-156`, `MAIN/U2.ASM:205-207`) ------

    /// What a watching part has recorded: the frames it has drawn, and whether
    /// any of them was handed the exit flag in its `PartCtx`.
    struct Watched {
        frames: AtomicU32,
        asked_to_quit: AtomicBool,
    }

    // One per part. A `static` because [`PartSource`] is a plain `fn` — the
    // parts it builds cannot capture anything — and one *each* because these
    // tests run in parallel: a shared one would let a test's key press answer
    // another's assertion.
    static BY_ESCAPE: Watched =
        Watched { frames: AtomicU32::new(0), asked_to_quit: AtomicBool::new(false) };
    static BY_CTRL: Watched =
        Watched { frames: AtomicU32::new(0), asked_to_quit: AtomicBool::new(false) };

    /// Draws nothing, never retires, and says what it was asked — the part's
    /// own view of the two keys, rather than the shell's.
    struct Keyed {
        watched: &'static Watched,
    }

    impl Part for Keyed {
        fn name(&self) -> &'static str { "keyed" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> { Ok(()) }
        fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus {
            self.watched.frames.fetch_add(1, Ordering::SeqCst);
            if ctx.exit_requested {
                self.watched.asked_to_quit.store(true, Ordering::SeqCst);
            }
            PartStatus::Continuing
        }
    }

    /// A run of `first` — a part that watches and never retires — and then a
    /// stub that does, so that what a key does to the run *after* the part it
    /// was pressed in is visible.
    fn keyed_run(first: &'static str) -> Sequence {
        Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(TEST_SURFACE as u16, TEST_SURFACE as u16),
            vec![first, "b"],
            |name| match name {
                "asked" => Some(Box::new(Keyed { watched: &BY_ESCAPE })),
                "unasked" => Some(Box::new(Keyed { watched: &BY_CTRL })),
                other => stub_source(other),
            },
        )
        .unwrap()
    }

    #[test]
    fn the_exit_flag_asks_the_running_part_to_quit_and_the_run_goes_on() {
        // `DIS/DISINT.ASM:151-153`: ESC sets `exitflag`; `exit_2` (`:186-190`,
        // "returns 1 if part should exit") hands it to the part's own loop, the
        // part returns, and the shell runs the *next* part. ESC ends a part; it
        // does not end the demo.
        let mut run = keyed_run("asked");
        run_out(&mut run, 2);
        assert_eq!(run.part(), "asked");
        let frames = || BY_ESCAPE.frames.load(Ordering::SeqCst);
        assert_eq!(frames(), 2, "the part has drawn two frames");
        assert!(!BY_ESCAPE.asked_to_quit.load(Ordering::SeqCst), "and been asked nothing yet");

        assert!(run.request_exit(), "there is a running part to ask");
        run.tick();
        assert_eq!(frames(), 3, "the part runs once more...");
        assert!(
            BY_ESCAPE.asked_to_quit.load(Ordering::SeqCst),
            "...and is handed the flag as it does"
        );
        run.tick();
        assert_eq!(run.part(), "b", "and the run goes on to the next part");
        assert_eq!(run.index(), 4, "on the run's own numbering, one tick at a time");
        assert_eq!(frames(), 3, "the retired part is not run again");
    }

    #[test]
    fn ctrl_skips_the_running_part_without_running_it_again() {
        // `MAIN/U2.ASM:205-207`: `partexecute` calls `ctrldown` and returns
        // before `execute` when the key is down — the part is not run. The port
        // is inside the part rather than in front of it, so the part it
        // interrupted gets no further frame at all, and the next part starts at
        // the index the skipped one would have drawn: the numbering is the
        // run's and a key does not re-anchor it. That is the difference from
        // ESC, whose part draws once more and is handed the flag.
        let mut run = keyed_run("unasked");
        run_out(&mut run, 3);
        assert_eq!(run.index(), 3);
        let frames = || BY_CTRL.frames.load(Ordering::SeqCst);
        assert_eq!(frames(), 3);

        assert!(run.skip_part(), "there is a running part to skip");
        run.tick();
        assert_eq!(frames(), 3, "a skipped part draws nothing more");
        assert_eq!(run.part(), "b", "the next part starts where the skipped one would have");
        assert_eq!(
            run.frame_index(),
            Some(3),
            "and its first frame is the index the skipped part would have drawn"
        );
    }

    #[test]
    fn neither_key_has_anything_to_act_on_once_the_run_is_holding() {
        // `BEG/BEG.C:87` ends BEGLOGO's `main()`, and the run holds that screen
        // (Task 1's hold, which is what answers BEGLOGO's 900). There is no part
        // left to ask or to skip: the original's shell is at `@@partsdone`
        // (`MAIN/U2.ASM:935`), and ESC is the window's way out from there.
        let mut run = for_part("a").unwrap();
        run_out(&mut run, 8);
        assert!(run.holding());
        assert!(!run.request_exit(), "no part is running to ask");
        assert!(!run.skip_part(), "and none to skip");
        assert_eq!(run.part(), "a", "the held screen is still a's");
    }

    #[test]
    fn a_run_that_stopped_says_which_part_it_stopped_at() {
        // Every part in the demo's order is registered now, so the hold-with-a-name
        // path is reached through a source that declines a demo name — which is what
        // a broken build or a missing part would look like. The run holds the screen
        // it has and can name the part it could not go on to, which is what a
        // window's status line has to say out loud.
        let mut run = sequence_of(vec!["a", "techno"]).unwrap();
        assert_eq!(run.stopped_at(), None, "nothing has stopped yet");
        run_out(&mut run, 8);
        assert!(run.holding());
        assert_eq!(run.stopped_at(), Some("techno"));
        assert_eq!(run.part(), "a", "and the held screen is still a's");
        // `unbuildable` answers `NotPorted` for a demo name: TECHNO is the demo's,
        // the source merely did not build it.
        assert_eq!(
            run.unbuildable("techno").to_string(),
            "techno is not ported yet",
            "a demo name the port has not written, not a typo"
        );
    }

    #[test]
    fn a_run_that_exhausted_its_plan_stopped_at_no_part_in_particular() {
        let mut run = for_part("a").unwrap();
        run_out(&mut run, 8);
        assert!(run.holding());
        assert_eq!(run.stopped_at(), None, "the plan ran out; no part is to blame");
    }

    #[test]
    fn the_frame_index_is_the_cursor_minus_one_and_none_before_the_first_tick() {
        // The status line shows this number, and off by one is the whole
        // difference between a reference index and a part's own.
        let mut run = for_part("beglogo").unwrap();
        assert_eq!(run.index(), 404);
        assert_eq!(run.frame_index(), None, "no frame has been drawn yet");
        run.tick();
        assert_eq!(run.frame_index(), Some(404), "the anchored frame is the 404 the reference holds");

        let mut run = for_part("a").unwrap();
        assert_eq!(run.frame_index(), None);
        run.tick();
        assert_eq!(run.frame_index(), Some(0), "an unanchored run counts from zero");
    }

    #[test]
    fn a_run_of_no_parts_is_refused_rather_than_ticked() {
        let error = match sequence_of(Vec::new()) {
            Err(error) => error,
            Ok(_) => panic!("a run with nothing in it has no part to name"),
        };
        assert!(matches!(error, ShellError::EmptyPlan), "{error:?}");
        assert_eq!(error.to_string(), "a run needs at least one part");
    }

    #[test]
    fn an_index_before_the_runs_start_can_never_be_reached() {
        // A run anchored at 404 walks away from 100 and answers 430 and 900;
        // asking for it is a mistake in the command, and the smallest one is
        // what the refusal names.
        assert_eq!(first_unreachable(404, &[430, 100, 900]), Some(100));
        assert_eq!(first_unreachable(404, &[404, 430, 900]), None);
        assert_eq!(first_unreachable(404, &[]), None);
        // An unanchored run starts at zero, and nothing is below zero.
        assert_eq!(first_unreachable(0, &[0, 1]), None);
    }

    #[test]
    fn a_part_whose_init_fails_is_reported_as_an_engine_error() {
        let err = match Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(8, 8),
            vec!["fails"],
            |_name| Some(Box::new(Fails)),
        ) {
            Err(err) => err,
            Ok(_) => panic!("a part whose init failed must not produce a run"),
        };
        assert!(matches!(err, ShellError::Engine(_)), "{err:?}");
    }

    /// A run of one stub and then a part the port has that cannot start: the
    /// one way a run *ends* rather than holding.
    fn failing_run() -> Sequence {
        Sequence::with_source(
            TEST_SURFACE,
            TEST_SURFACE,
            View::new(8, 8),
            vec!["a", "fails"],
            |name| match name {
                "a" => stub_source("a"),
                "fails" => Some(Box::new(Fails)),
                _ => None,
            },
        )
        .unwrap()
    }

    #[test]
    fn a_part_that_fails_to_start_mid_run_ends_it_rather_than_being_skipped() {
        let mut run = failing_run();
        assert_eq!(run_out(&mut run, 8), 3, "a's two frames, then the failing start");
        assert!(run.finished(), "a part the port has that cannot start ends the run");
        assert_eq!(run.part(), "a", "the run ends where the part failed");
    }

    struct Fails;

    impl Part for Fails {
        fn name(&self) -> &'static str { "fails" }
        fn init(&mut self, _ctx: &mut PartCtx) -> Result<(), PartError> {
            Err(PartError { part: "fails", message: "no".to_string() })
        }
        fn frame(&mut self, _ctx: &mut PartCtx) -> PartStatus { PartStatus::Continuing }
    }

    /// The test card through the shell's own source: the one part the port has,
    /// at the demo's real geometry.
    fn testcard_run() -> Sequence {
        Sequence::new(SURFACE_WIDTH, SURFACE_HEIGHT, start_view(), vec!["testcard"]).unwrap()
    }

    #[test]
    fn the_shell_starts_from_the_hardwares_default_palette() {
        let mut run = testcard_run();
        run.tick();
        // A frame's palette is the live DAC stamped for that frame, so this is
        // the table the run's frames are drawn through: entry 15 is white, and
        // that is what BEGLOGO's frame 430 shows before the part has written a
        // single DAC entry of its own.
        let dac = run.frame().palette_for(0);
        assert_eq!(dac.entry(15), [63, 63, 63]);
        // The test card loads the first sixteen entries itself — with this very
        // table, as it happens — so the proof that the *shell* installed one is
        // an entry the part never writes.
        assert_eq!(dac.entry(247), [11, 12, 16]);
        assert_eq!(dac.entry(248), [0, 0, 0], "the emulator's table stops at 248");
    }

    #[test]
    fn the_dac_the_shell_installs_is_the_one_a_mode_set_leaves() {
        // The table itself, against the emulator's — the oracle for what the
        // hardware powers up with. Two entries fixed by hand at each end and
        // one in the middle: a transcription slip anywhere else would have to
        // move one of them.
        let dac = hardware_default_palette();
        assert_eq!(dac.entry(0), [0, 0, 0]);
        assert_eq!(dac.entry(7), [42, 42, 42], "light grey, the CGA default's 7");
        assert_eq!(dac.entry(128), [14, 14, 28]);
        assert_eq!(dac.entry(255), [0, 0, 0]);
    }
}
