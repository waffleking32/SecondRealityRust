//! The engine for the Second Reality port.
//!
//! Deliberately dependency-free: the demo's hardware is modelled here, not
//! borrowed from a graphics stack.

pub mod compose;
pub mod copper;
pub mod engine;
pub mod palette;
pub mod part;
pub mod present;
pub mod surface;
pub mod testcard;
pub mod tick;
pub mod view;

pub use compose::{band_range, compose_band, IndexedFrame};
pub use copper::{CopperFn, CopperRegistry, COPPER_SLOTS};
pub use engine::{Engine, EngineError, Runner};
pub use palette::{dac_expand, Palette, PaletteTimeline};
pub use part::{Part, PartCtx, PartError, PartStatus};
pub use surface::{Surface, MAX_HEIGHT, MAX_WIDTH};
pub use tick::Tick;
pub use view::{Split, View};

#[cfg(test)]
mod tests {
    /// The engine keeps zero runtime dependencies (spec §4.2, D16 item 1): the
    /// demo's hardware is modelled here, not borrowed from a graphics stack,
    /// which is what keeps the engine usable, buildable and testable without
    /// SDL. D16 recorded that a test asserted this; this is that test.
    ///
    /// The manifest is read as text, not through a TOML crate, because the
    /// guard must not itself be the first dependency it forbids — and because
    /// it has to catch a dependency *table*, `[dependencies.foo]`, which is an
    /// offender whatever it holds.
    #[test]
    fn the_engine_has_no_runtime_dependencies() {
        const MANIFEST: &str = include_str!("../Cargo.toml");

        let mut in_dependencies = false;
        let mut found_the_section = false;
        let mut offenders: Vec<String> = Vec::new();

        for (index, line) in MANIFEST.lines().enumerate() {
            let line_number = index + 1;
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                // A section header. `[dev-dependencies]` and
                // `[build-dependencies]` do not start with `[dependencies`.
                in_dependencies = trimmed == "[dependencies]";
                found_the_section |= in_dependencies;
                if !in_dependencies && trimmed.starts_with("[dependencies") {
                    offenders.push(format!("line {line_number}: {trimmed}"));
                }
                continue;
            }
            if in_dependencies && !trimmed.is_empty() && !trimmed.starts_with('#') {
                offenders.push(format!("line {line_number}: {trimmed}"));
            }
        }

        assert!(
            found_the_section,
            "`[dependencies]` is missing from the engine's manifest. This test reads that \
             section, so removing it removes the guard: put it back, empty (spec §4.2, D16)"
        );
        assert!(
            offenders.is_empty(),
            "the engine must keep zero runtime dependencies (spec §4.2, D16); the manifest has {}",
            offenders.join(", ")
        );
    }
}
