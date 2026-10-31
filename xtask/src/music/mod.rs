//! `xtask music`: the original's music-player state, read out of the emulated guest (D26).
//!
//! The emulator runs silent (D17), so the oracle for the music is the player's own state:
//! `MAIN/STMIK.H:125-131` publishes the words STMIK keeps its position in, and
//! `MAIN/STMIK.300`'s `PUBDEF` records give their offsets.
//!
//! - `symbols` prints that map ([`locate`] builds it).
//! - `trace` locates the segment in the guest and writes one row of player state per frame
//!   under `reference/music/` ([`trace`]; the files are [`format`]'s).
//! - `analyse` prints the numbers `docs/fidelity/music-clock.md` quotes from a trace
//!   ([`analyse`]).

mod analyse;
mod format;
mod locate;
mod trace;

use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use sr_assets::omf::symbols::{self, Symbol};

use crate::oracle::OracleError;
use crate::reference::{sha256_hex, CaptureError};

pub use analyse::analyse_command;
pub use format::{Coverage, PlayerState, TraceManifest, TRACE_KIND};
pub use locate::LocateError;
pub use trace::{parse_window, trace_command, TraceOptions};

use locate::{needles, symbol_map};

/// The one directory inside `reference/` a trace may be written under, as a new directory of
/// its own.
pub const MUSIC_ROOT: &str = "reference/music";

/// The object the symbol map is read from, relative to the repository root.
pub const DEFAULT_OBJECT: &str = "upstream/SecondReality/MAIN/STMIK.300";

/// The seven words of the player's position: consecutive slots one memory request returns
/// together.
pub const POSITION_WORDS: &[&str] = &[
    "_np_zinfo",
    "_np_zplus",
    "_np_zframe",
    "_np_row",
    "_np_pat",
    "_np_ord",
    "_np_loop",
];

/// Speed and tempo, elsewhere in the segment (`MAIN/STMIK.H:117-120`).
pub const CLOCK_WORDS: &[&str] = &["_np_speed", "_np_tempo"];

/// The machine's own words: the card, the mixing rate and what the shell put in them. Read at
/// the locate and again at the end of the trace ([`TraceManifest::machine_moved`]).
pub const MACHINE_WORDS: &[&str] = &[
    "_zoutputmode",
    "_zmixspeed",
    "_zpollmix",
    "_zbufsize",
    "_zspeedadjust",
    "_zspeedadjust2",
    "_np_mastermul",
    "_np_mastervol",
];

/// The video clock the demo's frame counter runs on (`MAIN/COPPER.ASM:300-320`), the engine's
/// 70.086 Hz. The modules and their tempo patches are `sr_audio::SONGS`.
pub const FRAME_RATE_HZ: f64 = 70.086;

/// Why a music command could not run, as opposed to a finding about what it measured.
#[derive(Debug)]
pub enum MusicError {
    /// The options cannot produce a reproducible trace. Nothing was started.
    Options(String),
    /// The object is not a readable OMF object, or its symbols are not the layout the trace
    /// reads.
    Symbols(String),
    Locate(LocateError),
    /// The trace's coherence checks failed, so nothing was written.
    Coherence(String),
    /// A trace on disk is not one this tool can read.
    Trace(String),
    Oracle(OracleError),
    Io {
        path: PathBuf,
        message: String,
    },
}

impl fmt::Display for MusicError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MusicError::Options(message)
            | MusicError::Symbols(message)
            | MusicError::Coherence(message)
            | MusicError::Trace(message) => f.write_str(message),
            MusicError::Locate(error) => write!(f, "{error}"),
            MusicError::Oracle(error) => write!(f, "{error}"),
            MusicError::Io { path, message } => write!(f, "{}: {message}", path.display()),
        }
    }
}

impl std::error::Error for MusicError {}

impl From<OracleError> for MusicError {
    fn from(error: OracleError) -> MusicError {
        MusicError::Oracle(error)
    }
}

impl From<CaptureError> for MusicError {
    fn from(error: CaptureError) -> MusicError {
        MusicError::Options(error.to_string())
    }
}

impl From<LocateError> for MusicError {
    fn from(error: LocateError) -> MusicError {
        MusicError::Locate(error)
    }
}

impl From<sr_assets::AssetError> for MusicError {
    fn from(error: sr_assets::AssetError) -> MusicError {
        MusicError::Symbols(error.to_string())
    }
}

/// `cargo xtask music symbols --object …`: the map every address the trace reads comes from.
pub fn symbols_command(object_path: &Path) -> Result<ExitCode, MusicError> {
    let object = std::fs::read(object_path).map_err(|e| MusicError::Io {
        path: object_path.to_path_buf(),
        message: e.to_string(),
    })?;
    let walk = symbols::walk(&object)
        .map_err(|error| MusicError::Symbols(format!("{}: {error}", object_path.display())))?;
    println!(
        "object: {} — {} bytes, {} record(s), sha256 {}",
        object_path.display(),
        object.len(),
        walk.records(),
        sha256_hex(&object)
    );
    println!(
        "  every record the walk covers is 3 + length bytes: {} in all",
        walk.bytes
    );
    for (kind, count) in &walk.types {
        println!("  {:<14} {} record(s)", record_name(*kind), count);
    }
    for (index, segment) in &walk.segments {
        println!(
            "segment {index}: {:?} class {:?}, {} bytes, acbp 0x{:02X}",
            segment.name, segment.class, segment.length, segment.acbp
        );
    }
    println!("symbols ({}), by offset:", walk.symbols.len());
    let mut sorted: Vec<&Symbol> = walk.symbols.iter().collect();
    sorted.sort_by_key(|symbol| symbol.offset);
    for symbol in sorted {
        println!(
            "  0x{:04X} {:6} {} (segment {})",
            symbol.offset, symbol.offset, symbol.name, symbol.segment
        );
    }
    let map = symbol_map(object_path, &object)
        .map_err(|error| MusicError::Symbols(format!("{}: {error}", object_path.display())))?;
    println!("traced words ({}), read at base + offset:", map.words.len());
    for (name, offset) in &map.words {
        println!("  {name:<16} 0x{offset:04X} {:6}", offset);
    }
    let needles = needles(&object)?;
    println!("locator needles:");
    for needle in &needles {
        println!(
            "  0x{:04X} {} bytes, from the LEDATA at 0x{:04X}, sha256 {}",
            needle.offset, needle.len, needle.block, needle.sha256
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// The name a record type is known by, for the census's own report.
fn record_name(kind: u8) -> String {
    let name = match kind {
        0x80 => "THEADR",
        0x88 => "COMENT",
        0x8A => "MODEND",
        0x90 => "PUBDEF",
        0x91 => "PUBDEF32",
        0x94 => "LINNUM",
        0x96 => "LNAMES",
        0x98 => "SEGDEF",
        0x99 => "SEGDEF32",
        0x9C => "FIXUPP",
        0xA0 => "LEDATA",
        0xA1 => "LEDATA32",
        0xA2 => "LIDATA",
        0xA3 => "LIDATA32",
        0xB0 => "COMDEF",
        0xBC => "LCOMDEF",
        _ => "",
    };
    format!("0x{kind:02X} {name}")
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The vendored STMIK object.
    pub(crate) fn object() -> Vec<u8> {
        std::fs::read(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../")
                .join(DEFAULT_OBJECT),
        )
        .expect("the vendored STMIK object")
    }
}
