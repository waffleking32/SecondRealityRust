//! The `Part` trait: what a demo part implements, in place of the Demo
//! Interrupt Server (`DIS/DIS.H`).
//!
//! A part sees one [`PartCtx`] per call, holding what DIS handed out through
//! globals: the surface, the view, the live palette, the frame counter, the
//! copper slots and the music.

use std::fmt;

use crate::copper::{CopperFn, CopperRegistry};
use crate::music::{MusicRef, NO_PLUS};
use crate::palette::Palette;
use crate::surface::Surface;
use crate::view::View;

/// A part failed to start.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartError {
    /// The part's `name()`.
    pub part: &'static str,
    pub message: String,
}

impl PartError {
    pub fn new(part: &'static str, message: impl Into<String>) -> PartError {
        PartError { part, message: message.into() }
    }
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
    /// `dis_exit()` ends a part's loop, not the code after it. U2E runs a
    /// sixteen-retrace DAC fade after its loop (`U2E.C:541-555`), so the shell's
    /// `Sequence::tick` keeps ticking a part that reports `true` until it
    /// returns [`PartStatus::Finished`] itself. Every other part returns at once
    /// (U2A's teardown is synchronous, `U2A.C:239`, `:259`).
    fn drains_after_exit(&self) -> bool {
        false
    }
}

/// A part's handle on the engine for one call.
pub struct PartCtx<'a> {
    pub surface: &'a mut Surface,
    pub view: &'a mut View,
    /// The live palette; the engine stamps it at each band's first scanline.
    pub palette: &'a mut Palette,
    /// Logic ticks since this part started.
    pub frame: u32,
    /// Which band of this tick's `bands` this call is drawing.
    pub band: u16,
    /// How many bands this tick draws.
    pub bands: u16,
    /// The sticky exit flag (`dis_exit()`): set by [`request_exit`](PartCtx::request_exit)
    /// or from outside by [`Engine::request_exit`](crate::engine::Engine::request_exit).
    pub exit_requested: bool,
    copper: &'a mut CopperRegistry,
    music: Option<MusicRef<'a>>,
    music_fresh: bool,
}

impl PartCtx<'_> {
    /// Construct from disjoint borrows; [`Machine::ctx`](crate::machine::Machine::ctx)
    /// is the usual way to build one.
    #[allow(clippy::too_many_arguments)]
    pub fn new<'a>(
        surface: &'a mut Surface,
        view: &'a mut View,
        palette: &'a mut Palette,
        copper: &'a mut CopperRegistry,
        music: Option<MusicRef<'a>>,
        frame: u32,
        band: u16,
        bands: u16,
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
            exit_requested,
            copper,
            music,
            music_fresh,
        }
    }

    /// Replace the live palette. The engine stamps it at the next band's first
    /// scanline, so a change made while drawing band `n` shows from band `n`.
    pub fn set_palette(&mut self, pal: Palette) {
        *self.palette = pal;
    }

    /// Change one entry of the live palette, stamped like
    /// [`set_palette`](PartCtx::set_palette).
    pub fn set_palette_entry(&mut self, index: u8, rgb: [u8; 3]) {
        self.palette.set_entry(index, rgb);
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

    /// The music, or `None` when the run has none. This and the readers below
    /// are a part's whole access to the player, as `DIS/DIS.H:37-59` is the
    /// original's.
    pub fn music(&mut self) -> Option<MusicRef<'_>> {
        self.music.as_deref_mut()
    }

    /// `Err` naming `why` when the run has no music. A part that waits on a cue
    /// refuses to start without one rather than wait for ever.
    pub fn require_music(&self, part: &'static str, why: &str) -> Result<(), PartError> {
        match self.music {
            Some(_) => Ok(()),
            None => Err(PartError::new(part, why)),
        }
    }

    /// `dis_musplus()`, or the no-plus sentinel `-32` (`DIS/DISINT.ASM:252`)
    /// when the run has no music.
    pub fn musplus(&mut self) -> i32 {
        self.music().map_or(NO_PLUS, |music| music.musplus())
    }

    /// `dis_musrow()`, or 0 when the run has no music.
    pub fn musrow(&mut self) -> i32 {
        self.music().map_or(0, |music| music.musrow())
    }

    /// `dis_getmframe()`, or 0 when the run has no music.
    pub fn getmframe(&mut self) -> i32 {
        self.music().map_or(0, |music| music.getmframe())
    }

    /// True only in the `init` of the first part started after
    /// [`Engine::set_music`](crate::engine::Engine::set_music): the run's entry
    /// part, or the part a mid-run restart installs a new player for
    /// (`MAIN/U2.ASM:880-885`). A part catches the player up to its boot frame
    /// only then; reached mid-chain, the player is already there. Band and
    /// copper contexts are never fresh.
    pub fn music_fresh(&self) -> bool {
        self.music_fresh
    }

    /// Ask the engine to quit (`dis_exit()`). The flag stays set for the rest
    /// of the part's run, in every later context.
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::machine::Machine;
    use crate::music::{Music, NoMusic};

    #[test]
    fn the_music_readers_answer_the_null_readings_without_a_player() {
        let mut machine = Machine::new(8, 4, View::new(8, 4));
        let mut ctx = machine.ctx(None, 0, 0, 1, false, false);
        assert_eq!((ctx.musplus(), ctx.musrow(), ctx.getmframe()), (-32, 0, 0));
        assert_eq!(ctx.require_music("p", "why"), Err(PartError::new("p", "why")));

        let mut music = NoMusic;
        let expected = (music.musplus(), music.musrow(), music.getmframe());
        let mut ctx = machine.ctx(Some(&mut music), 0, 0, 1, false, false);
        assert_eq!((ctx.musplus(), ctx.musrow(), ctx.getmframe()), expected);
        assert_eq!(ctx.require_music("p", "why"), Ok(()));
    }
}
