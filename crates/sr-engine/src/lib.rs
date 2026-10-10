//! The engine for the Second Reality port.
//!
//! Deliberately dependency-free: the demo's hardware is modelled here, not
//! borrowed from a graphics stack.

pub mod palette;
pub mod surface;
pub mod view;

pub use palette::{dac_expand, Palette};
pub use surface::{Surface, MAX_HEIGHT, MAX_WIDTH};
pub use view::{Split, View};
