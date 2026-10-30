//! The machine a part draws on: what a [`PartCtx`] borrows, less the music.

use crate::copper::CopperRegistry;
use crate::music::MusicRef;
use crate::palette::Palette;
use crate::part::PartCtx;
use crate::surface::Surface;
use crate::view::View;

/// The surface, the view onto it, the live palette and the copper slots.
pub struct Machine {
    pub surface: Surface,
    pub view: View,
    pub palette: Palette,
    pub copper: CopperRegistry,
}

impl Machine {
    /// A zeroed surface, a black palette and no copper callbacks.
    pub fn new(surface_width: usize, surface_height: usize, view: View) -> Machine {
        Machine {
            surface: Surface::new(surface_width, surface_height),
            view,
            palette: Palette::BLACK,
            copper: CopperRegistry::new(),
        }
    }

    /// A context for one call.
    pub fn ctx<'a>(
        &'a mut self,
        music: Option<MusicRef<'a>>,
        frame: u32,
        band: u16,
        bands: u16,
        exit_requested: bool,
        music_fresh: bool,
    ) -> PartCtx<'a> {
        PartCtx::new(
            &mut self.surface,
            &mut self.view,
            &mut self.palette,
            &mut self.copper,
            music,
            frame,
            band,
            bands,
            exit_requested,
            music_fresh,
        )
    }
}
