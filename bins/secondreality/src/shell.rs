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
use std::time::Duration;

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
    finished: bool,
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
        Sequence::with_source(surface_width, surface_height, view, plan, parts::new_part)
    }

    /// [`Sequence::new`] with the parts coming from somewhere else. Crate-wide
    /// rather than private so `headless`'s own tests can drive a run of stub
    /// parts and see what a dump does with it.
    pub(crate) fn with_source(
        surface_width: usize,
        surface_height: usize,
        view: View,
        plan: Vec<&'static str>,
        source: PartSource,
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
            finished: false,
        };
        // The DAC the machine already had, installed before the first part
        // starts and never reset: see `hardware_default_palette`.
        *run.engine.palette_mut() = hardware_default_palette();
        run.start(first)?;
        Ok(run)
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
        SequenceStatus::Continuing
    }

    /// Put a part in `live` for this tick: the next one in the plan, or the
    /// [`hold`](Sequence::holding) once there is none the port can run.
    /// Returns false only when the run has ended.
    fn take_next(&mut self) -> bool {
        if self.holding {
            return true;
        }
        let Some(&name) = self.plan.get(self.next) else {
            return self.hold();
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
            Err(_) => self.hold(),
        }
    }

    /// Start holding the screen the run's last part left.
    fn hold(&mut self) -> bool {
        let mut part = Box::new(Holding { of: self.current });
        if let Err(error) = self.engine.start(&mut *part) {
            eprintln!("secondreality: {error}; the run ends here");
            self.finished = true;
            return false;
        }
        self.live = Some(part);
        self.holding = true;
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
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::UnknownPart { name } => write!(f, "unknown part {name:?}"),
            ShellError::EmptyPlan => write!(f, "a run needs at least one part"),
            ShellError::NotPorted { name } => write!(f, "{name} is not ported yet"),
            ShellError::Engine(e) => write!(f, "{e}"),
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
