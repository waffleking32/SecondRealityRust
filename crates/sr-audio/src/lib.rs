//! The music engine for the Second Reality port.
//!
//! Deliberately dependency-free, like `sr-engine` and for the same reason: the port's
//! player is a deterministic function of the engine's ticks, and a crate added for
//! convenience is how that stops being true (spec §4.2, the phase plan's Global
//! Constraints).
//!
//! The pieces, and which task of Phase 3 owns each:
//!
//! | module | what it holds |
//! |---|---|
//! | [`module`] | the S3M container: patterns, cells, sample headers, the order list, D5's descrambling |
//! | `player` | the tracker: position, order advance, `A`/`B`/`C`, effects (Task 3) |
//! | the mixer | the mixing engine and its output (Task 5) |
//!
//! Nothing here reads the clock, sleeps, or looks at how fast the machine is; the player
//! advances because the engine ticked.

pub mod module;

pub use module::{
    descramble, Cell, Instrument, Module, ModuleError, Pattern, PatternFault, CHANNELS, ROWS,
};

#[cfg(test)]
mod tests {
    /// The audio crate keeps zero runtime dependencies, the same rule `sr-engine` is held
    /// to (`crates/sr-engine/src/lib.rs`): an S3M player needs no crate, and a dependency
    /// added for convenience is how a player stops being reproducible.
    ///
    /// The manifest is read as text, not through a TOML crate, because the guard must not
    /// itself be the first dependency it forbids — and because it has to catch a dependency
    /// *table*, `[dependencies.foo]`, which is an offender whatever it holds.
    #[test]
    fn the_player_has_no_runtime_dependencies() {
        const MANIFEST: &str = include_str!("../Cargo.toml");

        let mut in_dependencies = false;
        let mut found_the_section = false;
        let mut offenders: Vec<String> = Vec::new();

        for (index, line) in MANIFEST.lines().enumerate() {
            let line_number = index + 1;
            let trimmed = line.trim();
            if trimmed.starts_with('[') {
                // A section header. `[dev-dependencies]` and `[build-dependencies]` do not
                // start with `[dependencies`.
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
            "`[dependencies]` is missing from the audio crate's manifest. This test reads that \
             section, so removing it removes the guard: put it back, empty (the phase plan's \
             Global Constraints)"
        );
        assert!(
            offenders.is_empty(),
            "sr-audio must keep zero runtime dependencies (the phase plan's Global \
             Constraints); the manifest has {}",
            offenders.join(", ")
        );
    }
}
