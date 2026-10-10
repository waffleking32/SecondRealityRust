//! The engine for the Second Reality port.
//!
//! Deliberately dependency-free: the demo's hardware is modelled here, not
//! borrowed from a graphics stack.

pub mod compose;
pub mod palette;
pub mod surface;
pub mod tick;
pub mod view;

pub use compose::{band_range, compose_band, IndexedFrame};
pub use palette::{dac_expand, Palette, PaletteTimeline};
pub use surface::{Surface, MAX_HEIGHT, MAX_WIDTH};
pub use tick::Tick;
pub use view::{Split, View};
