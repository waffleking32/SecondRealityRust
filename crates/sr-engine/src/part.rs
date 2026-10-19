//! The `Part` trait: what a demo part implements, replacing the Demo
//! Interrupt Server.
//!
//! A part sees one [`PartCtx`] per call, holding everything DIS used to hand
//! out through globals: the surface, the view, the live palette, the frame
//! counter, the copper slots, and the music — which is a global in the
//! original too (`DIS/DISINT.ASM`'s music entries read the player's segment).

use std::fmt;

use crate::copper::{CopperFn, CopperRegistry};
use crate::music::MusicRef;
use crate::palette::{Palette, PaletteTimeline};
use crate::surface::Surface;
use crate::view::View;

/// A part failed to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartError {
    /// The part's `name()`.
    pub part: &'static str,
    pub message: String,
}

impl fmt::Display for PartError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "part {}: {}", self.part, self.message)
    }
}

impl std::error::Error for PartError {}

/// What a part reports back after drawing one band.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartStatus {
    /// Keep running.
    Continuing,
    /// Retire the part once this tick's bands have all run.
    Finished,
}

/// One demo part.
pub trait Part {
    fn name(&self) -> &'static str;
    fn init(&mut self, ctx: &mut PartCtx) -> Result<(), PartError>;
    /// Draw one band of this frame, top to bottom. Called `band_count()` times
    /// per logic tick. Returning `Finished` from any band retires the part
    /// when this tick's bands have all run.
    fn frame(&mut self, ctx: &mut PartCtx) -> PartStatus;
    /// 1 (whole frame, composed once) or more (beam compose, composed per band).
    fn band_count(&self) -> u16 {
        1
    }

    /// Whether the part keeps running after the exit flag is set, to finish a
    /// teardown of its own.
    ///
    /// The original's `dis_exit()` ends a part's loop; it does not skip the
    /// code after it. A part whose source runs a **timed teardown** after the
    /// loop — U2E's sixteen-retrace DAC fade (`U2E.C:541-555`) — is still
    /// working when it returns, so retiring it on the exit tick would cut the
    /// teardown short: `Sequence::tick` and `Runner::stopped` keep ticking a
    /// part that reports `true` here until it returns
    /// [`PartStatus::Finished`](PartStatus::Finished) itself.
    ///
    /// The default is `false`, and it is right for every part but U2E: U2A's
    /// teardown is synchronous within the tick its gate or loop sees the flag
    /// (`U2A.C:239`, `:259`), and the rest simply return.
    ///
    /// [`Engine::start`](crate::engine::Engine::start) clears the sticky exit
    /// flag, so a drain cannot leak into the part that follows.
    fn drains_after_exit(&self) -> bool {
        false
    }
}

/// A part's handle on the engine for one call.
pub struct PartCtx<'a> {
    pub surface: &'a mut Surface,
    pub view: &'a mut View,
    pub palette: &'a mut Palette, // the live palette; the engine stamps it per band
    /// Logic ticks since this part started.
    pub frame: u32,
    /// Which band of this tick's `bands` this call is drawing.
    pub band: u16,
    /// How many bands this tick draws.
    pub bands: u16,
    /// Running outside the demo shell (`dis_indemo()`).
    pub standalone: bool,
    pub exit_requested: bool,
    // timeline, copper and music are private; reach them through the methods
    // below
    timeline: &'a mut PaletteTimeline,
    copper: &'a mut CopperRegistry,
    music: Option<MusicRef<'a>>,
    /// True only for the init context of the first part started after the
    /// run's music was installed; see [`music_fresh`](PartCtx::music_fresh).
    music_fresh: bool,
}

impl PartCtx<'_> {
    /// Construct from disjoint borrows of the engine's fields.
    ///
    /// Deliberately takes the fields one by one rather than `&mut Engine`:
    /// the engine builds one of these inside its band loop while its other
    /// fields are still in use, which only compiles for a disjoint borrow.
    ///
    /// The arity is the point, so clippy's `too_many_arguments` is allowed
    /// rather than worked around: bundling these into a struct would have to
    /// hold the same `&mut` fields and be built at the same call site, which
    /// changes nothing about the borrow (and a `&mut Engine` parameter — the
    /// shape the lint is steering towards — is exactly what does not compile
    /// here). Six of the twelve are the engine's own fields in the order
    /// `Engine::tick` splits them.
    #[allow(clippy::too_many_arguments)]
    pub fn new<'a>(
        surface: &'a mut Surface,
        view: &'a mut View,
        palette: &'a mut Palette,
        timeline: &'a mut PaletteTimeline,
        copper: &'a mut CopperRegistry,
        music: Option<MusicRef<'a>>,
        frame: u32,
        band: u16,
        bands: u16,
        standalone: bool,
        exit_requested: bool,
        music_fresh: bool,
    ) -> PartCtx<'a> {
        PartCtx {
            surface,
            view,
            palette,
            frame,
            band,
            bands,
            standalone,
            exit_requested,
            timeline,
            copper,
            music,
            music_fresh,
        }
    }

    /// Replace the live palette. The engine stamps it at the next band's first
    /// scanline, so a change made while drawing band `n` shows from band `n`.
    pub fn set_palette(&mut self, pal: Palette) {
        self.palette.copy_from(&pal);
    }

    /// Change one entry of the live palette, for the same stamping as
    /// [`set_palette`](PartCtx::set_palette).
    pub fn set_palette_entry(&mut self, index: u8, rgb: [u8; 3]) {
        self.palette.set_entry(index, rgb);
    }

    /// Register a palette change at an explicit scanline (raster bars).
    ///
    /// The timeline is rebuilt every tick, so this lasts for the rest of the
    /// tick it was made in — register it in copper slot 0 or in the band it
    /// applies to. Each band then stamps the live palette at that band's first
    /// scanline, replacing an explicit change made at exactly that scanline.
    pub fn set_palette_at(&mut self, scanline: u16, pal: Palette) {
        self.timeline.push(scanline, pal);
    }

    /// Install `f` in copper slot `slot`, one of [`COPPER_SLOTS`].
    ///
    /// [`COPPER_SLOTS`]: crate::copper::COPPER_SLOTS
    pub fn set_copper(&mut self, slot: usize, f: CopperFn) {
        self.copper.set(slot, f);
    }

    /// Whether copper slot `slot` already holds a callback.
    pub fn copper_is_set(&self, slot: usize) -> bool {
        self.copper.is_set(slot)
    }

    /// The music, when the run has any — `None` when it does not.
    ///
    /// This is the whole of a part's access to the player, the way `DIS/DIS.H:37-59` is the whole
    /// of the original's: the six calls on [`Music`], the position, and nothing else. A part that
    /// ignores music never calls this and is unaffected by a run having none; a part that needs a
    /// cue is written against `Some` and says so, and the shell refuses to start it when the run
    /// has no music (Task 7) rather than letting it wait for ever.
    ///
    /// [`Music`]: crate::music::Music
    pub fn music(&mut self) -> Option<MusicRef<'_>> {
        self.music.as_deref_mut()
    }

    /// Whether the run's music was installed for this part alone — true only
    /// for the init context of the part the **last `set_music` was for**, false
    /// for every later part in the chain.
    ///
    /// [`Engine::set_music`](crate::engine::Engine::set_music) marks the
    /// player fresh; the next [`Engine::start`](crate::engine::Engine::start)
    /// hands `true` to that part's init context and clears it. So the first
    /// part a `--part` or block run starts — the one the shell starts after
    /// installing the music — is fresh, and every part the chain reaches
    /// afterwards is not: its stretch of the player's timeline has already
    /// run. A **mid-run restart** (`MAIN/U2.ASM:880-885`, at U2E's boundary)
    /// loads a new player and re-arms the flag, so the part the restart
    /// precedes is fresh too; the part after it is not.
    ///
    /// A part that makes the player catch up to a boot frame (MINVBALL's
    /// 256 polls, RAYSCRL's 268, MNTSCRL's 11) or anchors a counter to one
    /// (PAM's `frame_count = 2`) does so only when this is `true`; a part
    /// reached mid-chain polls nothing and starts its counter at zero.
    ///
    /// Band and copper contexts are never fresh: the flag belongs to the
    /// `init` that follows `set_music` and to nothing else.
    pub fn music_fresh(&self) -> bool {
        self.music_fresh
    }

    /// Ask the engine to quit (`dis_exit()`).
    ///
    /// The request outlives this call: the engine reads it back out of the
    /// context when the call returns, reports it through
    /// [`Engine::exit_requested`](crate::engine::Engine::exit_requested), and
    /// stops ticking this part.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }
}
