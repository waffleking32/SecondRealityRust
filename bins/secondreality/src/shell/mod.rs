//! The demo shell, `MAIN/U2.ASM`'s equivalent: which parts a run plays, which music it
//! starts under, and the [`Sequence`] that runs them on one frame counter.
//!
//! Every index under `reference/` is a boot-relative emulator frame
//! (`docs/fidelity/reference-indices.md` §1.12); the demo's first graphics frame is 404. A
//! run numbers its frames in that space from its first part's [`boot_anchor`], whoever
//! draws them, so a rendered frame and a reference keyframe carry the same number (D21).
//!
//! - `plan`: `SECOND n`'s `partsmask` byte and the parts it selects (`MAIN/U2.ASM:110-120`,
//!   `:595-630`, `:790-918`), and where a run that starts at a part begins.
//! - `music`: the module, order and clock a run starts under, and the mid-run restarts
//!   (`:766-786`, `:813-818`, `:856-905`).
//! - `sequence`: the run, its hold, its exit stage (`:935-994`) and loop seam (`:914-933`).
//! - `idle`: the shell's own stretches with a part's screen up: the hold, the loop seam and
//!   the post-PANICEND cue wait (`:826-853`).
//! - `dac`: the DAC a mode set leaves, which a run's first frames are drawn through.
//!
//! The rulings are D21 (numbering), D34 and D45 (restarts), D38 (cue wait), D41 (the end)
//! and D42 (loop mode); `docs/fidelity/part-sequence.md` and `music.md` §12 hold the evidence.

use std::error::Error;
use std::fmt;

use sr_engine::part::PartError;
use sr_engine::view::View;

mod dac;
mod idle;
mod music;
mod plan;
mod sequence;
#[cfg(test)]
mod test_support;

pub(crate) use dac::hardware_default_palette;
pub use music::{music_for_part, ModuleId, MUSIC_START_FRAME};
pub use plan::{
    block_plan, boot_anchor, first_unreachable, resolve, run_plan, BLOCKS, FIRST_GRAPHICS_FRAME,
};
pub use sequence::{Ending, Looping, RunSettings, Sequence, SequenceStatus};
// What the binary's other tests read.
#[cfg(test)]
pub use music::MusicStart;
#[cfg(test)]
pub use plan::PAM_ANCHOR;
#[cfg(test)]
pub use sequence::{EXIT_BLANK_TICKS, EXIT_FADE_TICKS, LOOP_RELOAD_TICKS};

/// The shell's surface: the engine's largest, so every part's mode fits it.
pub use sr_engine::{MAX_HEIGHT as SURFACE_HEIGHT, MAX_WIDTH as SURFACE_WIDTH};

/// The picture's geometry when a run begins. A part may change it (double-scan makes it 400
/// lines) and the frame follows.
pub const VIEW_WIDTH: u16 = 320;
pub const VIEW_HEIGHT: u16 = 200;

/// The view a run begins with.
pub fn start_view() -> View {
    View::new(VIEW_WIDTH, VIEW_HEIGHT)
}

/// Why a run could not be built.
#[derive(Debug)]
pub enum ShellError {
    /// The name is not a registered part, or the block one the original rejects. An empty
    /// `--part` gets this too, and the message quotes the empty name.
    UnknownPart { name: String },
    /// The plan named no parts.
    EmptyPlan,
    /// A part failed to start.
    Engine(PartError),
    /// The run's music could not be built: the module is missing or is not an S3M. The demo
    /// is music-counted (D13), so a run without its music is refused rather than drawn.
    Music { message: String },
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::UnknownPart { name } => write!(f, "unknown part {name:?}"),
            ShellError::EmptyPlan => write!(f, "a run needs at least one part"),
            ShellError::Engine(e) => write!(f, "{e}"),
            ShellError::Music { message } => {
                write!(f, "the run's music cannot be loaded: {message}")
            }
        }
    }
}

impl Error for ShellError {}

impl From<PartError> for ShellError {
    fn from(e: PartError) -> ShellError {
        ShellError::Engine(e)
    }
}

/// rediff's table, for the trace window `JITTER_FRAMES` (A12) that the shell's trace tests
/// share with `tests/fullrun.rs`.
#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../xtask/src/rediff/table.rs"]
mod rediff_table;
