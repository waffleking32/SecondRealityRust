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

use sr_audio::PlayerOptions;
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
/// is the chain's business, not this table's. Two parts have a measured
/// anchor and they are both block entries:
///
/// - `beglogo` and `endlogo`, at **404**. `SECOND 2` sets mode 13h + `inittwk`
///   at `MAIN/U2.ASM:806-808` and `SECOND 5` does the same at `:906-908`, and
///   the boot's first graphics frame is 404 in five runs of five
///   (`reference-indices.md` §1.12). BEGLOGO's own first act is the white
///   fill, which is what the screen holds from there (§3.3 measures 410 and
///   429 byte-identical to 430); ENDLOGO's white is up by 415 and holds (§19).
///
/// Everything else returns `None`, and the three mid-block parts are why:
/// TUNNELI, CRED and ENDSCRL begin after an unported predecessor or a music
/// gate, and their starts are known only as **windows** — CRED's boundary is in
/// (1200, 1240] and ENDSCRL's in (8840, 8940] (§20, §21). A number here would
/// be a guess wearing a citation, and a run that starts at one of them is not
/// reference-comparable, which is what `None` makes it say.
pub fn boot_anchor(part: &str) -> Option<u32> {
    match part {
        "beglogo" | "endlogo" => Some(FIRST_GRAPHICS_FRAME),
        _ => None,
    }
}

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
    /// The module's own name, as the demo's files spell it and as the status line and the
    /// manifest record it.
    pub fn name(self) -> &'static str {
        match self {
            ModuleId::Music0 => "MUSIC0",
            ModuleId::Music1 => "MUSIC1",
        }
    }

    /// The module's own file, at the path a run reads its media from (the working
    /// directory, which for this repository's commands is its root).
    pub fn path(self) -> &'static str {
        match self {
            ModuleId::Music0 => "assets/MAIN/MUSIC0.S3M",
            ModuleId::Music1 => "assets/MAIN/MUSIC1.S3M",
        }
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
    /// own 130. `measure`'s `music-clock.md` §3 has the two headers.
    pub fn tempo_patch(self) -> Option<u8> {
        match self {
            ModuleId::Music0 => Some(0x78),
            ModuleId::Music1 => None,
        }
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

/// How a run reaches the music's own clock.
///
/// The original does one of two things, and which one depends on where the run's own
/// frames begin:
///
/// - A run whose first drawn frame is **at or before** [`MUSIC_START_FRAME`] waits for it:
///   the player is installed and seeked at the run's start, and does not advance until
///   the frame the music's first tick falls on. This is the demo's own beginning — block 2
///   and block 1 both draw from 404, four frames before the music starts — and it is what
///   `second5`'s frames 404…407 hold: a loaded module that has not ticked.
/// - A run whose first drawn frame is **after** it **pre-polls** the frames it missed, at
///   its start, because the music's advance is deterministic and does not depend on a
///   frame being drawn: `Player::poll` is the only thing that moves it, and N polls in a
///   row leave it exactly where N polls spread over N frames would. A `SECOND 3` run is
///   this case — MNTSCRL's first graphics frame is in (415, 420] while the music starts at
///   408 — and so is every run whose entry point is not the demo's own start.
/// - A run whose numbering is **its own** (`index_is_reference` is false — `--part X`,
///   `--part testcard`) has no reference frame to synchronise to: its music starts with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MusicStart {
    /// The frame the music's own clock starts on, in the run's numbering.
    pub frame: u32,
    /// Polls the engine makes before the music advances: the frames between the run's
    /// first drawn frame and the music's start.
    pub waiting: u32,
    /// Polls the run makes at its start, before its first tick, so that the music is
    /// where it would have been had it been polled since its start.
    pub pre: u32,
}

impl MusicStart {
    /// The music's start for a run whose first drawn frame carries the reference index
    /// `anchor`, or `None` for a run with a numbering of its own.
    pub fn new(anchor: Option<u32>) -> MusicStart {
        match anchor {
            Some(anchor) if anchor < MUSIC_START_FRAME => MusicStart {
                frame: MUSIC_START_FRAME,
                waiting: MUSIC_START_FRAME - anchor,
                pre: 0,
            },
            Some(anchor) if anchor > MUSIC_START_FRAME => MusicStart {
                frame: anchor,
                waiting: 0,
                pre: anchor - MUSIC_START_FRAME,
            },
            Some(_) => MusicStart {
                frame: MUSIC_START_FRAME,
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
/// **Nothing the port can run today reaches either of them** — `--block 2`'s plan stops at
/// GLENZ, which is not ported; `--block 3` cannot start (MNTSCRL is not ported); `--block 5`'s
/// mask has bit 3 clear, so `:880` is skipped for it — but a run that *does* reach U2E inside
/// blocks 2 or 3 needs a **mid-run** restart at that point in the sequence, and this
/// one-row-per-run table has no place for one yet. That is the Phase 4 dependency the row for
/// block 4 does not, by itself, express.
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
/// part at.
///
/// A single-part run names a part as its own entry point, so the question "which module,
/// at which order" is the one the demo answers for that part: `SECOND 5` starts ENDLOGO,
/// `SECOND 2` starts BEGLOGO, and their restarts are the table above. A demo part **no**
/// block starts at — TUNNELI, CRED, ENDSCRL: reached by a run, never started by one — is
/// played from the demo's own beginning, which is block 1's row (`MAIN/U2.ASM:766-775`,
/// `MOV ax,0` / `_zplaysong(0)`): the demo's own run reaches those parts under exactly that
/// module, whatever order it has got to by then.
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
    match entry_point {
        Some(block) => music_for(block).ok(),
        // No entry point starts where this run does. A demo part is then played from the
        // demo's own beginning — block 1's row, the module the demo's own run reaches those
        // parts under — and a name the demo does not have at all has no music.
        None => demo_part(part).and_then(|_| music_for('1').ok()),
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
}

/// The music a run was built with: the player, which module and order it began at, where
/// its own clock starts, and the dump it feeds.
struct RunMusic {
    /// The player, shared with the engine's adapter and with whatever drains it.
    player: crate::audio::Shared,
    /// The module and order the run's entry point starts at ([`MUSIC_RESTARTS`]).
    module: ModuleId,
    order: u8,
    /// Where the music's own clock starts, in the run's numbering ([`MusicStart`]).
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
        };
        // The DAC the machine already had, installed before the first part
        // starts and never reset: see `hardware_default_palette`.
        *run.engine.palette_mut() = hardware_default_palette();
        if let (Some(loader), Some((module, order))) = (music, music_for_part(first)) {
            run.install_music(module, order, loader)?;
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
    fn install_music(
        &mut self,
        module: ModuleId,
        order: u8,
        source: MusicSource,
    ) -> Result<(), ShellError> {
        let player = source(module).map_err(|message| ShellError::Music { message })?;
        let start = MusicStart::new(self.anchor);
        crate::audio::lock(&player).play_song(order);
        // **The frames the music missed before this run began.** A run whose first drawn
        // frame is *after* the music's own start pre-polls the frames in between, because
        // the music's advance is deterministic and does not depend on a frame being drawn:
        // `Player::poll` is the only thing that moves it, and `pre` polls in a row leave it
        // exactly where `pre` polls spread over `pre` frames would. A run that begins
        // *before* the music's start has nothing to catch up on — its player waits
        // (`MusicStart::waiting`) rather than running early.
        //
        // **`pre` is zero for every run the port can make today**, and that is arithmetic
        // rather than a hope: `pre` is `anchor - MUSIC_START_FRAME` and exists only for an
        // anchor *after* that frame, `boot_anchor` answers `Some(404)` (which waits, since
        // 404 < 408) or `None` (`pre` 0), and no part carries an anchor past 408. A
        // catch-up needs an entry point whose first frame is later than 408 — MNTSCRL's is
        // in (415, 420], which is `SECOND 3`'s, and that block cannot start yet (MNTSCRL is
        // not ported).
        //
        // If one ever runs, two things about it are worth knowing before it is debugged: a
        // catch-up longer than the ring (8192 samples) has its oldest samples dropped by the
        // ring's own overflow policy — the same policy an undrained device meets — so a
        // pre-polled run that also dumped its audio would begin slightly clipped; and the
        // pre-poll is `poll` after `poll`, not a jump, because the music's advance is what
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
            dump: None,
            scratch: vec![0; 1024],
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
        let status = self.engine.tick(&mut **part, &mut self.frame);
        self.index += 1;
        // Finished, or asked to quit: either way this part's loop has ended and
        // the next tick hands the screen to the next part. The engine keeps the
        // surface and the DAC, which is what the original's separate EXEs do.
        if status == PartStatus::Finished || self.engine.exit_requested() {
            self.live = None;
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
        let Some(&name) = self.plan.get(self.next) else {
            return self.hold(None);
        };
        self.next += 1;
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

    /// Where the music's own clock starts, in the run's numbering, or `None` for a run with
    /// no player.
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
    pub fn audio_format(&self) -> Option<(u32, u16)> {
        self.music.as_ref().map(|_| crate::audio::wav_format(&PlayerOptions::default()))
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
    use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

    #[test]
    fn a_block_entry_anchor_is_the_boots_first_graphics_frame() {
        // Both block entries the port can start from are anchored at 404:
        // the demo's first graphics frame under the D17 recipe.
        assert_eq!(boot_anchor("beglogo"), Some(404));
        assert_eq!(boot_anchor("endlogo"), Some(404));
    }

    #[test]
    fn a_mid_block_part_has_no_anchor_and_says_so() {
        // CRED starts in (1200, 1240] and ENDSCRL in (8840, 8940] — windows,
        // not frames. A number here would be a guess wearing a citation.
        assert_eq!(boot_anchor("cred"), None);
        assert_eq!(boot_anchor("endscrl"), None);
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
        // must therefore be the row its own plan finds — and the two blocks the port can run
        // are the two this matters for today.
        for row in MUSIC_RESTARTS {
            let first = block_plan(row.block).unwrap()[0];
            assert_eq!(
                music_for_part(first),
                Some((row.module, row.order)),
                "{first} starts {}'s plan and must start its music",
                row.block
            );
        }
        // ENDLOGO is block 5's entry point and BEGLOGO is block 2's: the two runs the port
        // can make today, and the two the trace at `reference/music/second5` covers.
        assert_eq!(music_for_part("endlogo"), Some((ModuleId::Music0, 25)));
        assert_eq!(music_for_part("beglogo"), Some((ModuleId::Music1, 0)));
        // A part no block *starts* at is played from the demo's own beginning — the module
        // the demo's own run reaches those parts under.
        assert_eq!(music_for_part("cred"), Some((ModuleId::Music0, 0)));
        // And a name the demo does not have at all has no music: the test card is the
        // port's own, so a run of it installs no player and `PartCtx::music()` is `None`.
        assert_eq!(music_for_part("testcard"), None);
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
        text.lines()
            .skip(1)
            .map(|line| {
                let number = |index: usize| -> u32 {
                    line.split(',')
                        .nth(index)
                        .unwrap_or_else(|| panic!("{line}: no field {index}"))
                        .trim()
                        .parse()
                        .unwrap_or_else(|error| panic!("{line}: field {index}: {error}"))
                };
                (number(0), (number(1) as u8, number(2) as u8), number(9) as i32)
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
    fn resolve_separates_a_part_the_port_has_from_one_it_has_not_reached() {
        assert_eq!(resolve("testcard").unwrap(), "testcard");
        assert!(matches!(resolve("glenz"), Err(ShellError::NotPorted { name: "glenz" })));
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
            // A demo part that retires at once, so a run of it reaches the hold and its
            // music's fade. `pam` is the demo's own part and no block *starts* at it, which
            // is why it is played from the demo's beginning (MUSIC0 at order 0).
            "pam" => Some(Box::new(Stub { name: "pam", fill: None, retires: Some(0) })),
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

        // And with the block's own plan, where the next part is a name the
        // source cannot build rather than one the helper left out.
        let mut run = sequence_of(vec!["a", "glenz"]).unwrap();
        run_out(&mut run, 8);
        assert_eq!(run.part(), "a", "the unported part must not be skipped to");
        assert!(!run.finished(), "the port ran out of parts; the run has not ended");
        assert_eq!(run.index(), 8, "and it holds rather than stopping: the index keeps counting");
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
        // `SECOND 2` after BEGLOGO runs GLENZ, which the port has not written:
        // the run holds BEGLOGO's screen and can name the part it could not go
        // on to, which is what a window's status line has to say out loud.
        let mut run = sequence_of(vec!["a", "glenz"]).unwrap();
        assert_eq!(run.stopped_at(), None, "nothing has stopped yet");
        run_out(&mut run, 8);
        assert!(run.holding());
        assert_eq!(run.stopped_at(), Some("glenz"));
        assert_eq!(run.part(), "a", "and the held screen is still a's");
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
