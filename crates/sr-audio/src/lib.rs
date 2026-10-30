//! The music engine for the Second Reality port: STMIK's S3M player and mixer, read out of
//! `MAIN/STMIK.300` (`docs/fidelity/music.md`). Dependency-free, like `sr-engine` (design §4.2).
//!
//! | module | what it holds |
//! |---|---|
//! | [`module`] | the S3M container: patterns, cells, sample headers, the order list, D5's descrambling |
//! | [`player`] | the tracker: the tick clock, the position, the order advance and `A`/`B`/`C` |
//! | [`effects`] | one channel's state and its effects: `D`, `E`, `F`, `G`, `H`, `J`, `K`, `O`, `Q`, `SDx` |
//! | [`mix`] | the mixing engine, its output curve, and the drain the device pulls from |
//!
//! Nothing here reads the clock: the player advances because the engine ticked, one
//! [`Player::poll`] a video frame.

pub mod effects;
pub mod mix;
pub mod module;
pub mod player;

pub use effects::{note_period, period_step, scaled_period, Channel};
pub use mix::{multiplier, step, Curve, Mixer, RING_SAMPLES, VOLUME_SCALE};
pub use module::{
    descramble, Cell, Instrument, Module, ModuleError, Pattern, PatternFault, Song, CHANNELS,
    ROWS, SONGS,
};
pub use player::{MixRate, Player, PlayerOptions, Position, Quality, ENHANCED_RATE};

#[cfg(test)]
mod tests {
    /// The audio crate keeps zero runtime dependencies (design §4.2), as `sr-engine` does. Any
    /// table whose name mentions `dependencies` is an offender except `[dev-…]` and
    /// `[build-dependencies]`; the manifest is read as text so the guard is not itself a
    /// dependency.
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
                // A table header; TOML allows whitespace inside the brackets.
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
            "`[dependencies]` is missing from the audio crate's manifest. This test reads that \
             section, so removing it removes the guard: put it back, empty (design §4.2)"
        );
        assert!(
            offenders.is_empty(),
            "sr-audio must keep zero runtime dependencies (design §4.2); the manifest has {}",
            offenders.join(", ")
        );
    }
}
