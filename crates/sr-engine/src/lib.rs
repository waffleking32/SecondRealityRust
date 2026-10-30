//! The engine for the Second Reality port.
//!
//! Deliberately dependency-free: the demo's hardware is modelled here, not
//! borrowed from a graphics stack.

pub mod compose;
pub mod copper;
pub mod engine;
pub mod machine;
pub mod music;
pub mod palette;
pub mod part;
pub mod present;
pub mod surface;
pub mod testcard;
pub mod tick;
pub mod view;

pub use compose::{band_range, compose_band, IndexedFrame};
pub use copper::{CopperFn, CopperRegistry, COPPER_SLOTS};
pub use engine::Engine;
pub use machine::Machine;
pub use music::{
    muscode_from, musplus_from, sync_from, Music, MusicPosition, MusicRef, NoMusic, ORDERSYNC1,
};
pub use palette::{dac_expand, Palette, PaletteTimeline};
pub use part::{Part, PartCtx, PartError, PartStatus};
pub use surface::{Surface, MAX_HEIGHT, MAX_WIDTH};
pub use tick::Tick;
pub use view::{Split, View};

#[cfg(test)]
mod tests {
    /// The engine keeps zero runtime dependencies (design §4.2, D16). Any table
    /// whose name mentions `dependencies` is an offender except `[dev-…]` and
    /// `[build-dependencies]`; the manifest is read as text so the guard is not
    /// itself a dependency.
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
                // A table header. TOML allows whitespace inside the brackets,
                // so the name is what is between them, trimmed.
                let name = trimmed.trim_start_matches('[').trim_end_matches(']').trim();
                in_dependencies = name == "dependencies";
                found_the_section |= in_dependencies;
                if name.contains("dependencies")
                    && !in_dependencies
                    && name != "dev-dependencies"
                    && name != "build-dependencies"
                {
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
             section, so removing it removes the guard: put it back, empty (design §4.2, D16)"
        );
        assert!(
            offenders.is_empty(),
            "the engine must keep zero runtime dependencies (design §4.2, D16); the manifest has {}",
            offenders.join(", ")
        );
    }
}
