//! The `Part` trait: what a demo part implements, replacing the Demo
//! Interrupt Server.
//!
//! A part sees one [`PartCtx`] per call, holding everything DIS used to hand
//! out through globals: the surface, the view, the live palette, the frame
//! counter, and the copper slots.

use std::fmt;

use crate::copper::{CopperFn, CopperRegistry};
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
    // timeline and copper are private; reach them through the methods below
    timeline: &'a mut PaletteTimeline,
    copper: &'a mut CopperRegistry,
}

impl PartCtx<'_> {
    /// Construct from disjoint borrows of the engine's fields.
    ///
    /// Deliberately takes the fields one by one rather than `&mut Engine`:
    /// the engine builds one of these inside its band loop while its other
    /// fields are still in use, which only compiles for a disjoint borrow.
    pub fn new<'a>(
        surface: &'a mut Surface,
        view: &'a mut View,
        palette: &'a mut Palette,
        timeline: &'a mut PaletteTimeline,
        copper: &'a mut CopperRegistry,
        frame: u32,
        band: u16,
        bands: u16,
        standalone: bool,
        exit_requested: bool,
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

    /// Ask the shell to quit (`dis_exit()`).
    pub fn request_exit(&mut self) {
        self.exit_requested = true;
    }
}
