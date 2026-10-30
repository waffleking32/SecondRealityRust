//! The copper: the original's three per-frame callbacks, preserved.
//!
//! `dis_setcopper()` registers a routine the Demo Interrupt Server calls at a
//! fixed point of every frame: at the top of the screen, just before retrace,
//! and during retrace. The order is [`Engine::tick`](crate::engine::Engine::tick)'s.

use crate::part::PartCtx;

/// How many copper callbacks the original registered, and in what order they
/// run: 0 at the top of the screen, 1 just before retrace, 2 in retrace.
pub const COPPER_SLOTS: usize = 3;

/// A registered copper callback. It is called once per tick with the engine's
/// live state, exactly as `dis_setcopper`'s routine was called on the hardware.
pub type CopperFn = Box<dyn for<'c> FnMut(&mut PartCtx<'c>)>;

/// The three copper slots, in tick order.
///
/// Slots are addressed by index; an index at or beyond [`COPPER_SLOTS`] is a
/// programming error and panics, like an out-of-range surface row.
pub struct CopperRegistry {
    slots: [Option<CopperFn>; COPPER_SLOTS],
}

impl Default for CopperRegistry {
    fn default() -> Self {
        CopperRegistry::new()
    }
}

impl CopperRegistry {
    /// All three slots empty.
    pub fn new() -> CopperRegistry {
        CopperRegistry {
            slots: [None, None, None],
        }
    }

    /// Install `f` in `slot`, replacing any callback already there.
    pub fn set(&mut self, slot: usize, f: CopperFn) {
        self.slots[slot] = Some(f);
    }

    /// Remove whatever `slot` holds.
    pub fn clear(&mut self, slot: usize) {
        self.slots[slot] = None;
    }

    /// Whether `slot` holds a callback.
    pub fn is_set(&self, slot: usize) -> bool {
        self.slots[slot].is_some()
    }

    /// Remove the slot's function, so it can run while the registry is
    /// borrowed by its context; [`set`](CopperRegistry::set) puts it back.
    pub fn take(&mut self, slot: usize) -> Option<CopperFn> {
        self.slots[slot].take()
    }
}
