//! The engine for the Second Reality port.
//!
//! Deliberately dependency-free: the demo's hardware is modelled here, not
//! borrowed from a graphics stack.

pub mod compose;
pub mod copper;
pub mod engine;
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
pub use engine::{Engine, EngineError, Runner};
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
    /// The engine keeps zero runtime dependencies (spec §4.2, D16 item 1): the
    /// demo's hardware is modelled here, not borrowed from a graphics stack,
    /// which is what keeps the engine usable, buildable and testable without
    /// SDL. D16 recorded that a test asserted this; this is that test.
    ///
    /// The manifest is read as text, not through a TOML crate, because the
    /// guard must not itself be the first dependency it forbids.
    ///
    /// **Any table whose name mentions `dependencies` is an offender** except
    /// `[dev-dependencies]` and `[build-dependencies]`. The earlier shape —
    /// `[dependencies]` plus `[dependencies.x]` — let two spellings through:
    /// `[target.'cfg(unix)'.dependencies]`, which is still a runtime
    /// dependency, and `[ dependencies ]`, which TOML allows and the old
    /// equality test missed. Both are caught now, in this crate and in
    /// `sr-audio`, which holds the same test.
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
             section, so removing it removes the guard: put it back, empty (spec §4.2, D16)"
        );
        assert!(
            offenders.is_empty(),
            "the engine must keep zero runtime dependencies (spec §4.2, D16); the manifest has {}",
            offenders.join(", ")
        );
    }
}
