//! `xtask music`: STMIK's public symbols, the locator that finds the player's
//! segment in the running guest, the per-frame trace of the player's own state,
//! and the measurements the trace is read for.
//!
//! Phase 3 has no audio reference to compare against: the emulator runs silent
//! (D17 requires it — a real audio device makes the animation drift), and the
//! two modules' *timing* is invisible from the outside. What is visible is the
//! player's own state. `MAIN/STMIK.H:125-131` publishes the words STMIK keeps
//! its position in, `MAIN/STMIK.300`'s `PUBDEF` records give their offsets, and
//! `dosbox-automation` can read guest memory — so the oracle this phase builds
//! is a **per-frame trace of the original's own player state**, and it is the
//! ground truth Tasks 3, 6 and 8 are measured against.
//!
//! Three commands, in the order they are used:
//!
//! - [`symbols_command`] prints the symbol map, so every address the trace
//!   reads has a command behind it (`cargo xtask music symbols --object …`).
//! - [`trace_command`] boots the original under the D17 recipe, locates the
//!   STMIK segment, and writes one row of player state per reference frame
//!   into `reference/music/<name>/` beside a manifest.
//! - [`analyse_command`] derives the phase's numbers from a trace — where the
//!   music starts, the tick rate and its residual, the frame-to-frame
//!   quantisation, the machine's card and mixer words, `zplus`/`zinfo`, and
//!   what the player does at the order list's `254`/`255` entries.
//!
//! # What the trace reads, and why every frame
//!
//! The tick quantisation lives in the *sequence* of row transitions: a sparse
//! sample cannot see a one-frame error, so the trace samples every requested
//! frame. Each sample is keyed by the frame counter the guest itself reports
//! (never by a hoped-for index), the read is bracketed by two counter reads,
//! and a sample whose counter moved between them is discarded and re-taken — a
//! sample that arrives late is one frame with a different number, never a wrong
//! value under the right number.
//!
//! Each sample is **one memory request for the whole STMIK segment** rather
//! than one per word: the words this phase compares frame by frame then cannot
//! tear against each other at all, and three requests a frame (counter, memory,
//! counter) is fewer than splitting the words up would cost.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use sr_assets::omf::data_blocks;
use sr_assets::omf::symbols::{self, Symbol, Walk};

use crate::oracle::{sleep_frames, Oracle, OracleError, DEFAULT_WAIT, FRAME_ORIGIN};
use crate::reference::{
    keys_from_script, sha256_hex, validate_recipe, BootRecipe, CaptureError, CaptureOptions,
    MANIFEST_VERSION, REFERENCE_ROOT, SETUP_ANSWER_TIMEOUT, SETUP_SCREEN_TEXT, UPSTREAM_COMMIT,
};

/// The tree the traces live in. `reference/` is input-only, and this is the one
/// directory inside it the phase plan licenses `xtask music trace` to write: a
/// *new* directory under here, never a frame capture's directory, and never
/// `reference/music` itself.
pub const MUSIC_ROOT: &str = "reference/music";

/// The object the symbol map is read from, relative to the repository root.
pub const DEFAULT_OBJECT: &str = "upstream/SecondReality/MAIN/STMIK.300";

/// The manifest's `kind` for a trace, so `reference verify` can tell one from a
/// frame capture — and refuse one it does not know.
pub const TRACE_KIND: &str = "music-trace";

/// The trace's own file name, beside `manifest.json`.
pub const TRACE_FILE: &str = "trace.csv";

/// The columns `trace.csv` writes, in order. `frame` first, because a reader
/// keys on it.
pub const COLUMNS: &[&str] = &[
    "frame", "ord", "row", "pat", "loop", "speed", "tempo", "zinfo", "zplus", "zframe",
];

/// The seven words that are the player's position — the ones one memory request
/// returns together, and the ones Tasks 3 and 8 compare frame by frame.
pub const POSITION_WORDS: &[&str] = &[
    "_np_zinfo", "_np_zplus", "_np_zframe", "_np_row", "_np_pat", "_np_ord", "_np_loop",
];

/// The two words outside the position run: speed and tempo are elsewhere in the
/// segment (`MAIN/STMIK.H:117-120`), and the trace carries both every frame.
pub const CLOCK_WORDS: &[&str] = &["_np_speed", "_np_tempo"];

/// The machine's own words: which card the reference runs, at what mixing rate,
/// and what the shell put in them (Step 4.4). They are read when the segment is
/// located and again at the end of the trace: a word the two readings agree
/// about is recorded once, and a word that **moved** is recorded twice, in
/// [`TraceManifest::machine_moved`]. Nothing here is assumed constant — the
/// mixing rate itself moves later in a run.
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

/// The modules the demo plays, with the tempo `MAIN/STARTMUS.C:257` patches into
/// the first of them. The analysis reads their headers, because the tick rate is
/// a *prediction* of the module's own data and not a fit to the trace.
pub const MODULES: &[(&str, Option<u8>)] = &[
    ("assets/MAIN/MUSIC0.S3M", Some(120)),
    ("assets/MAIN/MUSIC1.S3M", None),
];

/// The video clock the demo's own frame counter runs on
/// (`MAIN/COPPER.ASM:300-320` reprograms the PIT and polls the retrace): every
/// rate in this file is per this frame, and it is the same 70.086 Hz the engine
/// ticks at.
pub const FRAME_RATE_HZ: f64 = 70.086;

/// The frame and the rate `docs/fidelity/reference-indices.md` §1.7 measured the
/// 16-bit word at physical `0x105B0E` at: 371/373/374 at video frame 900, and
/// 0.7424 per frame across three sessions. `music analyse --at 0x105B0E` places
/// that address against the object's symbols and against the trace's own rate.
pub const PHASE1_CROSS_CHECK_FRAME: u64 = 900;
pub const PHASE1_CROSS_CHECK_RATE: f64 = 0.7424;

/// How many frames a sample may fall behind before the guest's own counter is
/// slept off rather than polled. The frame rate is 70.086 Hz, so this is ~0.35 s.
const POLL_WINDOW_FRAMES: u64 = 25;

/// The largest memory a locate scan will walk, in bytes. The emulator's memory
/// size is not in the API; a read past it is refused, and that refusal ends the
/// scan — this is only the bound for a machine that answers forever.
const SCAN_LIMIT: u32 = 128 * 1024 * 1024;

/// The size of one request the locate scan makes: 64 KiB, the largest block a
/// segment:offset pair can address in one go.
const SCAN_CHUNK: usize = 0x10000;

/// How many bases an ambiguous match reports before the scan stops looking:
/// enough to show the shape of the ambiguity, few enough to print.
pub const AMBIGUOUS_LIMIT: usize = 8;

/// The shortest needle that is evidence of a segment base. The rule that picks
/// them is [`needles`]'.
pub const NEEDLE_LEN: usize = 32;

/// How long the segment may take to appear before the locate gives up. The
/// loader has it in memory within the first seconds of the boot; a scan still
/// finding nothing a minute in is not going to.
const LOCATE_TIMEOUT: Duration = Duration::from_secs(60);

/// How long a trace may spend with the guest's counter not moving before it is
/// refused: a guest that has stopped cannot be sampled, and waiting for ever is
/// not a measurement.
const STALL_TIMEOUT: Duration = Duration::from_secs(120);

// -------------------------------------------------------------------------------------
// Errors
// -------------------------------------------------------------------------------------

/// Why a music command could not run, as opposed to a finding about what it
/// measured.
#[derive(Debug)]
pub enum MusicError {
    /// The options cannot produce a reproducible trace. Nothing was started.
    Options(String),
    /// The object's symbol map cannot carry the phase: a name is missing, the
    /// position words are not the run this phase reads them as, or the object
    /// is not a readable OMF object.
    Symbols(String),
    /// The running guest does not hold the STMIK segment where a unique match
    /// says it should.
    Locate(LocateError),
    /// The trace's own coherence checks failed, so nothing was written: the
    /// numbers would have been a record of a read that did not happen.
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

impl From<symbols::OmfError> for MusicError {
    fn from(error: symbols::OmfError) -> MusicError {
        MusicError::Symbols(error.to_string())
    }
}

/// Why the STMIK segment could not be located in the running guest.
#[derive(Debug)]
pub enum LocateError {
    /// The needle's bytes are not in guest memory at all, or not within
    /// [`SCAN_LIMIT`].
    NotFound { scanned: usize },
    /// More than one place holds the needle, so no base is *the* base: the
    /// locator refuses rather than picking one, and reports every base the
    /// needle's own offset solves (up to [`AMBIGUOUS_LIMIT`] of them) with how
    /// many matches the whole scan saw.
    Ambiguous { bases: Vec<u32>, found: usize },
    /// The oracle could not read anything at all — the first address it asked
    /// for failed — so there was no memory to search.
    Read(String),
    /// The needle is too short to be evidence, or the match is below the offset
    /// it is supposed to sit at.
    BadNeedle(String),
}

impl fmt::Display for LocateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocateError::NotFound { scanned } => write!(
                f,
                "the segment was not found in the last {scanned} bytes of guest memory that were \
                 scanned"
            ),
            LocateError::Ambiguous { bases, found } => write!(
                f,
                "the needle occurs in {found} place(s), so no base is the base: it solves {}",
                
                bases
                    .iter()
                    .map(|base| format!("0x{base:06X}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            LocateError::Read(message) => write!(f, "reading guest memory failed: {message}"),
            LocateError::BadNeedle(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for LocateError {}

// -------------------------------------------------------------------------------------
// Step 2: the locator
// -------------------------------------------------------------------------------------

/// Find the segment whose `target` offset holds `needle`, by scanning guest
/// memory for it.
///
/// `read(segment, offset, len)` is the oracle's memory read
/// (`dosbox-automation`'s `GET /api/v1/memory/{segment}/{offset}/{len}`), whose
/// address is `segment * 16 + offset` with a **32-bit** offset: the segment is a
/// paragraph number, the offset is an address in the emulated memory. A segment
/// above 1 MB therefore cannot be spelled as a `u16` segment and a `u16` offset,
/// which is why the scan walks addresses as `(0, address)` — the one form that
/// reaches all of the emulated memory — and why the returned pair is in that
/// same form.
///
/// **The address space is the guest's, page tables included**: the emulator's
/// `MEM_BlockRead` reads through `mem_readb_inline`
/// (`dosbox-automation`, `hardware/memory.cpp:199`, `cpu/paging.h:293`), which
/// goes through the TLB when paging is on. The addresses here are therefore
/// linear, and a DOS extender that maps one physical page at two linear
/// addresses shows the same bytes twice — which is what this machine does, and
/// why several matches are possible at all.
///
/// The scan reads the guest in 64 KiB requests, so the whole of a 16 MB machine
/// costs 256 requests rather than millions, and a needle that straddles two
/// requests is still found: each request is searched together with the last
/// `NEEDLE_LEN - 1` bytes of the one before it.
///
/// **One match is the answer; several are a refusal.** Two matches mean no base
/// is *the* base — a base one paragraph off lands on the wrong variable, and
/// the trace would be a record of the wrong bytes — so every base the needle's
/// own offset solves is reported, and it is the caller's business to show that
/// the several are views of one memory before reading anything through one of
/// them.
pub fn locate(
    target: u16,
    needle: &[u8],
    mut read: impl FnMut(u16, u32, usize) -> Vec<u8>,
) -> Result<(u16, u32), LocateError> {
    if needle.len() < NEEDLE_LEN {
        return Err(LocateError::BadNeedle(format!(
            "a {}-byte needle is not evidence of a segment base; {NEEDLE_LEN} bytes is the \
             shortest the locator accepts",
            needle.len()
        )));
    }
    let overlap = (needle.len() - 1).min(SCAN_CHUNK - 1);
    let mut matches: Vec<u32> = Vec::new();
    let mut found = 0usize;
    let mut tail: Vec<u8> = Vec::new();
    let mut address: u32 = 0;
    let mut scanned = 0usize;

    while address < SCAN_LIMIT {
        // An empty answer ends the scan: past the end of the emulated memory,
        // which is the end of the search rather than a failure — the emulator
        // refuses an address beyond its own memory, and the scan walks upward.
        let bytes = read(0, address, SCAN_CHUNK);
        if bytes.is_empty() {
            break;
        }
        scanned = bytes.len();

        // Matches entirely inside the previous chunk were counted then; a match
        // that starts in the tail and ends in this chunk was not.
        let mut haystack = tail.clone();
        haystack.extend_from_slice(&bytes);
        if haystack.len() >= needle.len() {
            for start in 0..=haystack.len() - needle.len() {
                if &haystack[start..start + needle.len()] != needle {
                    continue;
                }
                if start + needle.len() <= tail.len() {
                    continue;
                }
                if matches.len() < AMBIGUOUS_LIMIT {
                    matches.push(address - tail.len() as u32 + start as u32);
                }
                found += 1;
            }
        }

        // A chunk shorter than a full request is the end of the emulated
        // memory: nothing above it will answer, and `bytes.len() - overlap`
        // would not even be a valid slice for a chunk shorter than the needle.
        if bytes.len() < SCAN_CHUNK {
            break;
        }
        tail = bytes[bytes.len() - overlap..].to_vec();
        address += bytes.len() as u32;
    }

    match found {
        0 => Err(LocateError::NotFound { scanned }),
        1 => {
            let at = matches[0];
            let base = at.checked_sub(u32::from(target)).ok_or_else(|| {
                LocateError::BadNeedle(format!(
                    "the needle was found at 0x{at:06X}, below the 0x{target:04X} offset it is \
                     supposed to sit at — the match is not this segment"
                ))
            })?;
            Ok((0, base))
        }
        _ => {
            let mut bases: Vec<u32> = matches
                .iter()
                .filter_map(|at| at.checked_sub(u32::from(target)))
                .collect();
            bases.sort_unstable();
            bases.dedup();
            Err(LocateError::Ambiguous { bases, found })
        }
    }
}

/// One needle: where in the object it comes from, what it is, and what it
/// proves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needle {
    /// The offset inside the segment the run sits at — the `target` the locator
    /// solves the base from.
    pub offset: u32,
    /// How many bytes of the object's `LEDATA` data the run is.
    pub len: usize,
    /// The `sha256` of the run's bytes, so the manifest names what was searched
    /// for without carrying 32 bytes of hex, and so a reader can re-derive it.
    pub sha256: String,
    /// The `LEDATA` record's own segment offset, which is where in the object
    /// the run came from.
    pub block: u32,
    /// The rule that chose it, said in the manifest so the same run can be
    /// re-derived from the object.
    pub rule: String,
    /// The physical address the run was found at, once it has been.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found_at: Option<u32>,
}

/// The two runs of the object's own `LEDATA` data the locator searches for.
///
/// The rule is printed because it has to be re-derivable: the first run of
/// [`NEEDLE_LEN`] bytes, in `LEDATA` record order and at every byte offset
/// within each record, with **no byte repeated inside the run** — an all-distinct
/// run is one that a shifted or padded copy of the segment cannot match by
/// accident. The second needle is the first such run from a *different* `LEDATA`
/// record: one match is a coincidence until a second, independently chosen run
/// lands at the same base.
pub fn needles(object: &[u8]) -> Result<[Needle; 2], MusicError> {
    let blocks = data_blocks(object)?;
    let mut chosen: Vec<(usize, usize)> = Vec::new();
    for (index, block) in blocks.iter().enumerate() {
        if let Some(at) = block
            .data
            .windows(NEEDLE_LEN)
            .position(all_distinct)
        {
            if let Some(first) = chosen.first() {
                // The second needle has to come from a different record, so
                // that the two are independent evidence.
                if blocks[first.0].offset == block.offset {
                    continue;
                }
            }
            chosen.push((index, at));
            if chosen.len() == 2 {
                break;
            }
        }
    }
    if chosen.len() < 2 {
        return Err(MusicError::Symbols(format!(
            "the object carries {} run(s) of {NEEDLE_LEN} bytes with no repeated byte inside them, \
             from different records; the locator needs two",
            chosen.len()
        )));
    }
    let mut out = Vec::with_capacity(2);
    for (index, at) in chosen {
        let block = &blocks[index];
        let run = &block.data[at..at + NEEDLE_LEN];
        out.push(Needle {
            offset: block.offset + at as u32,
            len: NEEDLE_LEN,
            sha256: sha256_hex(run),
            block: block.offset,
            rule: format!(
                "the first run of {NEEDLE_LEN} bytes with no byte repeated inside it, in LEDATA \
                 record order and at every byte offset within each record"
            ),
            found_at: None,
        });
    }
    Ok(<[Needle; 2]>::try_from(out).expect("two needles were chosen"))
}

/// Whether every byte of `window` is different from every other.
fn all_distinct(window: &[u8]) -> bool {
    let mut seen = [false; 256];
    for byte in window {
        if seen[usize::from(*byte)] {
            return false;
        }
        seen[usize::from(*byte)] = true;
    }
    true
}

/// A needle's bytes, so a scan searches for the object's own data rather than
/// for the digest the manifest records.
pub fn needle_bytes(object: &[u8], needle: &Needle) -> Result<Vec<u8>, MusicError> {
    for block in data_blocks(object)? {
        if block.offset <= needle.offset && needle.offset < block.offset + block.data.len() as u32 {
            let at = (needle.offset - block.offset) as usize;
            if at + needle.len <= block.data.len() {
                return Ok(block.data[at..at + needle.len].to_vec());
            }
        }
    }
    Err(MusicError::Symbols(format!(
        "the needle at 0x{:04X} is not inside any LEDATA record of the object",
        needle.offset
    )))
}

// -------------------------------------------------------------------------------------
// The symbol map
// -------------------------------------------------------------------------------------

/// The symbols the trace reads, resolved to offsets inside the segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolMap {
    /// The object's path as the command was given it.
    pub object: String,
    /// The object's `sha256`, so the map cannot be read against another build.
    pub sha256: String,
    /// The segment's own name and class, from its `SEGDEF`.
    pub segment_name: String,
    pub segment_class: String,
    /// The segment's declared length: how many bytes one sample reads.
    pub segment_length: u32,
    /// How the map was produced, in the manifest's own words.
    pub route: String,
    /// The record census the walk counted, `0xNN -> count`.
    pub records: BTreeMap<String, usize>,
    /// Every symbol the trace reads, by name, with its offset inside the segment.
    pub words: BTreeMap<String, u32>,
}

impl SymbolMap {
    /// The offset of a word the map was built with.
    pub fn offset(&self, name: &str) -> Result<u32, MusicError> {
        self.words
            .get(name)
            .copied()
            .ok_or_else(|| MusicError::Symbols(format!("the symbol map does not carry {name}")))
    }
}

/// Build the map from the object's own public symbols, checking the three
/// structural facts the trace depends on — each independently falsifiable:
///
/// 1. every name the phase reads is published (this *is* `D8`'s route 1);
/// 2. the seven position words are consecutive two-byte slots, so one request
///    returns them together;
/// 3. `_zinit`'s segment is the position words' — STMIK is one segment, which
///    is why `DIS/DISINT.ASM:242-282` sets `es` once and reads all of them
///    through it.
pub fn symbol_map(object_path: &Path, object: &[u8]) -> Result<SymbolMap, MusicError> {
    let walk = symbols::walk(object)?;
    let wanted: Vec<&str> = POSITION_WORDS
        .iter()
        .chain(CLOCK_WORDS)
        .chain(MACHINE_WORDS)
        .copied()
        .collect();
    let chosen = walk.select(&wanted)?;

    let mut slots: Vec<u32> = POSITION_WORDS
        .iter()
        .map(|name| {
            chosen
                .iter()
                .find(|symbol| symbol.name == *name)
                .expect("select returned every name")
                .offset
        })
        .collect();
    slots.sort_unstable();
    for pair in slots.windows(2) {
        if pair[1] - pair[0] != 2 {
            return Err(MusicError::Symbols(format!(
                "the position words are not consecutive two-byte slots: 0x{:04X} then 0x{:04X}",
                pair[0], pair[1]
            )));
        }
    }

    let mut segments: Vec<_> = walk.segments.iter().collect();
    segments.sort_by_key(|(index, _)| **index);
    let (index, segment) = segments
        .first()
        .ok_or_else(|| MusicError::Symbols("the object defines no segment".into()))?;
    for symbol in &chosen {
        if symbol.segment != **index {
            return Err(MusicError::Symbols(format!(
                "{} is in segment {} and the object's only segment is {index}",
                symbol.name, symbol.segment
            )));
        }
        if symbol.offset >= segment.length {
            return Err(MusicError::Symbols(format!(
                "{} is at {} and the segment is {} bytes",
                symbol.name, symbol.offset, segment.length
            )));
        }
    }
    check_one_segment(&walk, chosen.first().expect("select returned every name"))?;

    let words: BTreeMap<String, u32> = chosen
        .into_iter()
        .map(|symbol| (symbol.name, symbol.offset))
        .collect();
    let records = walk
        .types
        .iter()
        .map(|(kind, count)| (format!("0x{kind:02X}"), *count))
        .collect();
    Ok(SymbolMap {
        object: object_path.display().to_string(),
        sha256: sha256_hex(object),
        segment_name: segment.name.clone(),
        segment_class: segment.class.clone(),
        segment_length: segment.length,
        route: "OMF PUBDEF, resolved against the object's own SEGDEF and LNAMES records \
                (`MAIN/STMIK.H:125-131`, `MAIN/STMIKA.INC:17-28`)"
            .to_string(),
        records,
        words,
    })
}

/// STMIK is one segment: `_zinit` and the position words share it, or one base
/// cannot reach them all.
fn check_one_segment(walk: &Walk, symbol: &Symbol) -> Result<(), MusicError> {
    let zinit = walk.symbol("_zinit").ok_or_else(|| {
        MusicError::Symbols(
            "the object publishes no _zinit, so the segment map cannot be confirmed against the \
             player's own entry point"
                .into(),
        )
    })?;
    if zinit.segment != symbol.segment {
        return Err(MusicError::Symbols(format!(
            "_zinit is in segment {} and the position words are in segment {}: STMIK must be one \
             segment, or one base cannot reach them all",
            zinit.segment, symbol.segment
        )));
    }
    Ok(())
}

// -------------------------------------------------------------------------------------
// Step 3: the trace
// -------------------------------------------------------------------------------------

/// One traced row: the player's own state at one reference frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlayerState {
    /// The frame counter the guest itself reported, counted from the emulator's
    /// first rendered frame (the origin every capture's indices share).
    pub frame: u64,
    pub ord: u8,
    pub row: u8,
    pub pat: u8,
    /// `_np_loop`, which `MAIN/STMIK.H:125` publishes beside the position: the
    /// S3M's own loop counter, and part of what Task 3 compares.
    pub loop_: u8,
    pub speed: u8,
    pub tempo: u8,
    pub zinfo: i16,
    pub zplus: i16,
    pub zframe: i16,
}

impl PlayerState {
    /// The CSV line, in [`COLUMNS`]' order.
    pub fn line(&self) -> String {
        format!(
            "{},{},{},{},{},{},{},{},{},{}",
            self.frame,
            self.ord,
            self.row,
            self.pat,
            self.loop_,
            self.speed,
            self.tempo,
            self.zinfo,
            self.zplus,
            self.zframe
        )
    }

    /// The CSV header.
    pub fn header() -> String {
        COLUMNS.join(",")
    }

    /// Parse one row, refusing anything that is not exactly the columns.
    pub fn parse(line: &str) -> Result<PlayerState, MusicError> {
        let fields: Vec<&str> = line.split(',').collect();
        if fields.len() != COLUMNS.len() {
            return Err(MusicError::Trace(format!(
                "a trace row has {} field(s) where {} columns are declared: {line:?}",
                fields.len(),
                COLUMNS.len()
            )));
        }
        let number = |at: usize| -> Result<i64, MusicError> {
            fields[at].trim().parse::<i64>().map_err(|e| {
                MusicError::Trace(format!("column {} is not a number: {e}", COLUMNS[at]))
            })
        };
        let byte = |at: usize| -> Result<u8, MusicError> {
            u8::try_from(number(at)?)
                .map_err(|_| MusicError::Trace(format!("column {} is not a byte", COLUMNS[at])))
        };
        Ok(PlayerState {
            frame: u64::try_from(number(0)?)
                .map_err(|_| MusicError::Trace("the frame column is negative".into()))?,
            ord: byte(1)?,
            row: byte(2)?,
            pat: byte(3)?,
            loop_: byte(4)?,
            speed: byte(5)?,
            tempo: byte(6)?,
            zinfo: i16::try_from(number(7)?)
                .map_err(|_| MusicError::Trace("zinfo does not fit 16 bits".into()))?,
            zplus: i16::try_from(number(8)?)
                .map_err(|_| MusicError::Trace("zplus does not fit 16 bits".into()))?,
            zframe: i16::try_from(number(9)?)
                .map_err(|_| MusicError::Trace("zframe does not fit 16 bits".into()))?,
        })
    }

    /// Whether any word of the position moved between two samples: what "the
    /// player is advancing" means for the music's start frame.
    fn position_moved_from(&self, earlier: &PlayerState) -> bool {
        self.ord != earlier.ord
            || self.row != earlier.row
            || self.pat != earlier.pat
            || self.loop_ != earlier.loop_
            || self.zframe != earlier.zframe
    }
}

/// What a trace covers: the window asked for, and the frames actually sampled.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Coverage {
    /// The window the command sampled, inclusive at both ends.
    pub window: [u64; 2],
    /// How many frames the window holds.
    pub requested: u64,
    /// The first and last frame sampled.
    pub first: u64,
    pub last: u64,
    /// How many frames were sampled.
    pub samples: u64,
    /// Whether every frame of the window is present.
    pub per_frame: bool,
    /// The runs of frames the window asked for and the trace does not hold, as
    /// inclusive `[first, last]` pairs. Recomputable from the trace's own
    /// frames, and recomputed by `reference verify`.
    pub missing: Vec<[u64; 2]>,
    /// The frames of the window that were already gone when sampling began —
    /// the locate's own cost — or `None` when the window's first frame was
    /// sampled. It is the first run of [`Coverage::missing`] when it is there,
    /// named on its own so the boundary explains itself in the manifest instead
    /// of only in prose.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub before_sampling: Option<[u64; 2]>,
}

impl Coverage {
    /// The coverage a window and a list of sampled frames make. `reference
    /// verify` calls this on a trace file's own frames and compares the answer
    /// with the manifest's claim: the claim is recomputed, not believed.
    ///
    /// Nothing here can panic on a window it is handed, however malformed: the
    /// window comes out of a manifest, and a manifest is what a verifier exists
    /// to disbelieve. A window whose last frame is before its first is an empty
    /// one — zero requested, nothing missing, and **not** per-frame — which
    /// then disagrees with whatever the manifest claimed, and the disagreement
    /// is a finding rather than a crash.
    pub(crate) fn of(window: [u64; 2], frames: &[u64]) -> Coverage {
        // The number of frames the window holds, or `None` when there is no such
        // number: a window whose last frame is before its first, or whose span
        // does not fit a `u64`. Both are windows a manifest can claim and a
        // trace cannot cover, so both come back as an empty coverage — zero
        // requested, nothing missing, **not** per-frame — which then disagrees
        // with what the manifest says and is reported as a finding naming the
        // field. Nothing here may panic on manifest data: a verifier that a
        // crafted window can kill is a verifier that cannot do its job.
        let requested = window[1]
            .checked_sub(window[0])
            .and_then(|span| span.checked_add(1));
        let Some(requested) = requested else {
            return Coverage {
                window,
                requested: 0,
                first: frames.first().copied().unwrap_or(window[0]),
                last: frames.last().copied().unwrap_or(window[1]),
                samples: frames.len() as u64,
                per_frame: false,
                missing: Vec::new(),
                before_sampling: None,
            };
        };
        let mut missing = Vec::new();
        let mut cursor = window[0];
        for frame in frames {
            if *frame > cursor {
                missing.push([cursor, *frame - 1]);
            }
            cursor = frame.saturating_add(1);
        }
        if cursor <= window[1] {
            // `cursor` is one past a sampled frame, so it cannot be past the
            // window's end without the comparison above being false.
            missing.push([cursor, window[1]]);
        }
        // The first run, when it is the one before the first sample, is the
        // boundary rather than a gap: it is named as such.
        let before_sampling = match (missing.first(), frames.first()) {
            (Some([from, to]), Some(first)) if *to < *first && *from == window[0] => {
                Some([*from, *to])
            }
            _ => None,
        };
        Coverage {
            window,
            requested,
            first: frames.first().copied().unwrap_or(window[0]),
            last: frames.last().copied().unwrap_or(window[0]),
            samples: frames.len() as u64,
            per_frame: missing.is_empty(),
            missing,
            before_sampling,
        }
    }

    /// How many frames of the window are missing. Saturating rather than
    /// panicking: the runs come out of a manifest as often as out of a file.
    fn gaps(&self) -> u64 {
        self.missing
            .iter()
            .map(|[first, last]| last.saturating_sub(*first).saturating_add(1))
            .fold(0u64, u64::saturating_add)
    }

    /// Every field of this coverage that disagrees with `recorded`, with both
    /// values, as the sentences a finding is made of.
    ///
    /// `reference verify` recomputes a trace's coverage from the trace file's
    /// own frames and compares it with the manifest's claim through this, so a
    /// disagreement **names the field** rather than printing two sentences that
    /// look alike. `before_sampling` is compared only when the manifest records
    /// one: it is a naming of the first missing run rather than an independent
    /// claim, and a manifest filed before the field existed (D23's additive
    /// rule) is not wrong for lacking it — the recomputed coverage names the
    /// boundary in [`Coverage::describe`] either way.
    pub(crate) fn disagreements(&self, recorded: &Coverage) -> Vec<String> {
        let mut out = Vec::new();
        let mut field = |name: &str, mine: String, theirs: String| {
            if mine != theirs {
                out.push(format!("{name}: the file makes it {mine}, the manifest says {theirs}"));
            }
        };
        field(
            "window",
            format!("{}..{}", self.window[0], self.window[1]),
            format!("{}..{}", recorded.window[0], recorded.window[1]),
        );
        field(
            "requested",
            self.requested.to_string(),
            recorded.requested.to_string(),
        );
        field(
            "first/last",
            format!("{}..{}", self.first, self.last),
            format!("{}..{}", recorded.first, recorded.last),
        );
        field(
            "samples",
            self.samples.to_string(),
            recorded.samples.to_string(),
        );
        field(
            "per_frame",
            self.per_frame.to_string(),
            recorded.per_frame.to_string(),
        );
        field(
            "missing",
            format!("{:?}", self.missing),
            format!("{:?}", recorded.missing),
        );
        if let Some(theirs) = recorded.before_sampling {
            field(
                "before_sampling",
                format!("{:?}", self.before_sampling),
                format!("{:?}", Some(theirs)),
            );
        }
        out
    }

    /// The sentence the tool prints, and the one the note may quote.
    pub fn describe(&self) -> String {
        if self.per_frame {
            format!(
                "frames {}..{} of the requested {}..{}, every one of them ({} sample(s))",
                self.first, self.last, self.window[0], self.window[1], self.samples
            )
        } else {
            let missing: Vec<String> = self
                .missing
                .iter()
                .map(|[first, last]| {
                    if first == last {
                        format!("{first}")
                    } else {
                        format!("{first}..{last}")
                    }
                })
                .collect();
            let boundary = self
                .before_sampling
                .map(|[from, to]| {
                    format!(
                        " (frames {from}..{to} of them are the boundary: the segment was located \
                         after they had gone)"
                    )
                })
                .unwrap_or_default();
            format!(
                "frames {}..{} of the requested {}..{}: {} sample(s), {} frame(s) missing \
                 ({}){boundary} — this trace is NOT per-frame",
                self.first,
                self.last,
                self.window[0],
                self.window[1],
                self.samples,
                self.gaps(),
                missing.join(", ")
            )
        }
    }
}

/// One run of frames the trace could not sample, and what the guest's own
/// counter did around it.
///
/// This is an *observation*, not a claim the verifier can recompute: the
/// coverage it explains is recomputed from the trace file's own frames, while
/// this records what the sampler saw when it lost them. A gap whose counter
/// went from `before` straight to `after` is one the emulator never offered —
/// no sampling method could have had those frames — and a gap of one frame
/// where the counter moved by one is a read that straddled a boundary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Skipped {
    /// The frames that are not in the trace, inclusive.
    pub from: u64,
    pub to: u64,
    /// The guest's frame counter on the two sides of the read that lost them.
    pub counter_before: u64,
    pub counter_after: u64,
    /// What that means, in the manifest's own words.
    pub why: String,
}

/// The trace file, as the manifest records it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceFile {
    pub file: String,
    pub columns: Vec<String>,
    pub rows: u64,
    pub bytes: u64,
    pub sha256: String,
}

/// Where the segment was found, how, and whether it was still there at the end.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocateRecord {
    /// The segment base every sample is read through, as an address in the
    /// guest's own (paged) space: `base .. base + segment_length`.
    pub base: u32,
    /// Every base the needles were found at, sorted. More than one is a view of
    /// one memory, proved by [`LocateRecord::aliasing`]; the trace reads through
    /// the lowest.
    pub views: Vec<u32>,
    /// Why several views are one memory, when there are several: the whole
    /// segment read through each base, byte-identical, and the frame that was
    /// measured at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aliasing: Option<String>,
    /// The pair the oracle's memory read takes for `base`, which is how every
    /// sample was read (`segment * 16 + offset`).
    pub segment: u16,
    pub offset: u32,
    /// The guest's frame when the segment was first found.
    pub frame: u64,
    /// The address of every word the trace reads, `base + offset`.
    pub words: BTreeMap<String, u32>,
    pub needles: Vec<Needle>,
    /// The base the re-locate at the end of the trace solved, the views it
    /// found, and the frame it was solved at. The base must be among them, and
    /// every view must still be byte-identical to it, or the trace was refused.
    pub rerun_base: u32,
    pub rerun_views: Vec<u32>,
    pub rerun_frame: u64,
}

/// The behavioural check the manifest records: the words behave like the
/// counters they are named after, over the whole capture.
///
/// This is what makes the trace a measurement rather than a reading of seven
/// addresses that might be the words beside the right ones.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Behaviour {
    pub row: [u8; 2],
    pub ord: [u8; 2],
    pub pat: [u8; 2],
    /// Every distinct speed the trace saw, in order of first appearance.
    pub speeds: Vec<u8>,
    /// Every distinct tempo the trace saw, in order of first appearance.
    pub tempos: Vec<u8>,
    /// The range `zinfo` covered: one value for a capture that never moved it.
    #[serde(default)]
    pub zinfo_min: i16,
    #[serde(default)]
    pub zinfo_max: i16,
    /// The first frame at which the player is advancing — a row, an order, a
    /// pattern, a loop or `zframe` moved since the previous sample.
    pub first_advance: Option<u64>,
    /// Whether `zframe` moved on, and how much it moved in total.
    pub zframe_moves: u64,
    pub zframe_total: i64,
}

impl Behaviour {
    /// Whether `zinfo` never changed in the trace: a capture that shows one
    /// value does not measure what the others are.
    pub fn z_info_is_constant(&self) -> bool {
        self.zinfo_max == self.zinfo_min
    }
}

/// `<DIR>/manifest.json` for a trace: what was traced, from what, and what the
/// numbers came out as.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceManifest {
    pub version: u32,
    /// [`TRACE_KIND`]. A manifest without this key is a frame capture (D23's
    /// additive field, and `reference verify` tells the two apart by it).
    pub kind: String,
    pub oracle: String,
    pub upstream_commit: String,
    pub exe: String,
    pub arguments: Vec<String>,
    pub cycles: u32,
    pub frame_origin: String,
    pub skip_boot_frames: u64,
    pub boot: BootRecipe,
    pub keys: Vec<String>,
    pub coverage: Coverage,
    pub trace: TraceFile,
    pub object: SymbolMap,
    pub locate: LocateRecord,
    /// The machine's own words, read from the guest when the segment was found.
    /// A word the machine held all run appears here once; a word that moved has
    /// its other reading in [`TraceManifest::machine_moved`].
    pub machine: BTreeMap<String, i64>,
    /// The words whose reading at the end of the trace differs from the one
    /// above. Which words move is not assumed: the mixing rate moves
    /// (`_zmixspeed` 20000 → 40000 in `second5`'s session), `_zpollmix` moves
    /// (0 → 1, the demo switching to the polled mode), the player's volume words
    /// move under `fademusic` (`MAIN/U2.ASM:333-346`), and a word that did not
    /// move is simply absent here.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub machine_moved: BTreeMap<String, i64>,
    pub check: Behaviour,
    /// The gaps, with what the guest's counter did around each: the sampler's
    /// own observation of why they are not in the file.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<Skipped>,
}

impl TraceManifest {
    pub fn from_json(json: &str) -> Result<TraceManifest, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Everything `music trace` needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceOptions {
    pub object: PathBuf,
    pub exe: PathBuf,
    pub cwd: PathBuf,
    pub arguments: Vec<String>,
    pub cycles: u32,
    /// The window to sample, inclusive at both ends.
    pub window: [u64; 2],
    pub out: PathBuf,
    pub keys: Vec<String>,
    pub boot: BootRecipe,
    /// Replace a trace the directory already holds.
    pub force: bool,
}

impl TraceOptions {
    /// The `[autoexec]` lines the guest's own shell has to run for this trace,
    /// through the same builder `reference capture` uses — so the mount, the
    /// `AUTOTYPE` buttons and the program line cannot come to differ between a
    /// capture and a trace of the same boot (D17).
    pub fn autoexec_lines(&self) -> Result<Vec<String>, MusicError> {
        let options = CaptureOptions {
            exe: self.exe.clone(),
            cwd: self.cwd.clone(),
            arguments: self.arguments.clone(),
            cycles: self.cycles,
            frames: vec![self.window[0]],
            out: self.out.clone(),
            skip_boot_frames: 0,
            keys: self.keys.clone(),
            boot: self.boot,
            sheet: None,
        };
        Ok(options.autoexec_lines()?)
    }
}

/// Refuse a trace that cannot be reproduced, before touching the emulator.
///
/// The recipe half is [`validate_recipe`], the same checks `reference capture`
/// makes, because a trace's indices mean what a capture's indices mean only
/// under the same recipe (D17). The rest is this command's own rule:
/// `reference/` is input-only, and the one place inside it a trace may be
/// written is **a new directory under `reference/music/`**.
pub fn validate_trace_options(options: &TraceOptions) -> Result<(), MusicError> {
    validate_recipe(
        &options.exe,
        &options.cwd,
        options.cycles,
        &options.keys,
        options.boot,
    )?;
    if options.window[1] < options.window[0] {
        return Err(MusicError::Options(format!(
            "the frame window {}..{} is empty: its last frame is before its first",
            options.window[0], options.window[1]
        )));
    }
    if !options.object.is_file() {
        return Err(MusicError::Options(format!(
            "--object {} is not a file; the symbol map has to come from the object the demo was \
             linked from",
            options.object.display()
        )));
    }
    let music_root = Path::new(MUSIC_ROOT);
    if options.out.starts_with(Path::new(REFERENCE_ROOT)) {
        // Inside `reference/` exactly one shape is allowed: a new directory
        // under `reference/music/`. Everything else is refused by name — and so
        // is `reference/music` itself, because a trace belongs in a directory
        // of its own, named for the capture it is.
        if !options.out.starts_with(music_root) || options.out == music_root {
            return Err(MusicError::Options(format!(
                "--out {} is inside {REFERENCE_ROOT}/: that tree holds the mounted original and \
                 every capture, and a file written among them is a file every later diff is \
                 measured against. The only place inside it a trace may be written is a new \
                 directory under {MUSIC_ROOT}/, e.g. {MUSIC_ROOT}/<name>",
                options.out.display()
            )));
        }
    }
    // A frame capture's own directory is a reference like any other: writing
    // into it would put a file among the frames every diff is measured against.
    if let Some(capture) = frame_capture_holding(&options.out) {
        return Err(MusicError::Options(format!(
            "--out {} is inside the frame capture {}: the frames and the manifest in it are what \
             every later diff reads. A trace goes beside it, e.g. {MUSIC_ROOT}/<name>",
            options.out.display(),
            capture.display()
        )));
    }
    // A directory that already holds a trace is not written into without
    // `--force` — checked here, before the emulator is started, so a refused
    // run costs nothing. Only a *trace* counts: a frame capture's manifest is
    // refused above, by what it is.
    if !options.force && holds_a_trace(&options.out) {
        return Err(MusicError::Options(format!(
            "{} already holds a trace ({}); pass --force to replace it",
            options.out.display(),
            TRACE_FILE
        )));
    }
    Ok(())
}

/// Whether a directory holds a trace already: the trace file itself, or a
/// manifest that declares [`TRACE_KIND`].
fn holds_a_trace(dir: &Path) -> bool {
    if dir.join(TRACE_FILE).is_file() {
        return true;
    }
    std::fs::read_to_string(dir.join("manifest.json"))
        .map(|text| {
            serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|value| value.get("kind").cloned())
                .and_then(|kind| kind.as_str().map(str::to_owned))
                .as_deref()
                == Some(TRACE_KIND)
        })
        .unwrap_or(false)
}

/// The nearest directory at or above `path` that holds a frame capture's
/// manifest — a `manifest.json` that is not a trace's.
///
/// A trace's own directory is not refused here: `--force` re-writing a trace
/// this tool wrote is the one case that must not trip the guard.
fn frame_capture_holding(path: &Path) -> Option<PathBuf> {
    let mut current = Some(path);
    while let Some(directory) = current {
        if directory.join("manifest.json").is_file() && !holds_a_trace(directory) {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

/// Boot the original, locate STMIK, sample the window frame by frame, and write
/// the trace and its manifest.
///
/// The oracle is left running; the caller shuts it down.
pub fn trace(oracle: &mut dyn Oracle, options: &TraceOptions) -> Result<TraceManifest, MusicError> {
    validate_trace_options(options)?;
    let object = std::fs::read(&options.object).map_err(|e| MusicError::Io {
        path: options.object.clone(),
        message: e.to_string(),
    })?;
    let map = symbol_map(&options.object, &object)?;
    let mut needles = needles(&object)?;

    let oracle_name = oracle.describe()?;
    oracle.set_cycles(options.cycles)?;
    oracle.boot(&options.exe, &options.cwd, &options.arguments)?;

    let answering = !options.keys.is_empty();
    if options.boot == BootRecipe::Typed && answering {
        // The setup screen has to be answered before the demo draws anything.
        // Under the `Autoexec` recipe the guest answers it itself, from the
        // `[autoexec]` section this run was started with (D17).
        oracle.wait_for_text(SETUP_SCREEN_TEXT, DEFAULT_WAIT)?;
        oracle.send_keys(&options.keys.join(" "))?;
    }

    // ---------------------------------------------------------------------
    // Find the segment. The loader has it in memory before the demo draws
    // anything, so the scan starts as early as the guest answers: the earliest
    // frames of the window are the boot's, and a locate that waited for the
    // first graphics frame would throw them away — and the music's start is
    // the number this trace exists to measure.
    // ---------------------------------------------------------------------
    let location = find_segment(oracle, &object, &map, &needles)?;
    let base = location.base;
    let located_at = location.frame;
    for needle in needles.iter_mut() {
        // Each needle is recorded where the base it solves sits: the trace's
        // reads are through `base`, so the run's own address is this.
        needle.found_at = Some(base + needle.offset);
    }
    let mut locate_record = LocateRecord {
        base,
        views: location.views.clone(),
        aliasing: location.aliasing.clone(),
        segment: 0,
        offset: base,
        frame: located_at,
        words: map
            .words
            .iter()
            .map(|(name, offset)| (name.clone(), base + *offset))
            .collect(),
        needles: needles.to_vec(),
        rerun_base: base,
        rerun_views: location.views.clone(),
        rerun_frame: located_at,
    };
    let machine_at_start = read_words(oracle, base, &map, MACHINE_WORDS)?;

    // ---------------------------------------------------------------------
    // Sample. Every frame from the first the guest can give us; each sample is
    // one memory request for the whole segment, bracketed by two reads of the
    // guest's own frame counter.
    // ---------------------------------------------------------------------
    let window = [options.window[0].max(located_at), options.window[1]];
    if window[0] > window[1] {
        return Err(MusicError::Coherence(format!(
            "the segment was located at frame {located_at}, after the last frame asked for \
             ({}): there is nothing to trace",
            options.window[1]
        )));
    }
    let mut samples: Vec<PlayerState> = Vec::new();
    // Start at the frame the guest is actually on: locating the segment costs
    // frames, and a frame that has already gone is not one this trace can have.
    // The coverage records where sampling began.
    // One frame past where the guest is: the frame it is on when the sampler
    // starts is already half gone, and aiming at it would only waste a read.
    let mut next = window[0].max(oracle.frame_counter()? + 1);
    let mut stalled_at = u64::MAX;
    let mut stalled_since = Instant::now();
    let mut skipped: Vec<Skipped> = Vec::new();
    let mut seen = next.saturating_sub(1);
    // One request is 0.4 ms and a frame is 14.3 ms, so the loop can poll the
    // guest's own counter and still take a sample inside the frame it found:
    // distance is slept off, the last stretch is polled.
    while next <= window[1] {
        let now = oracle.frame_counter()?;
        if now < next {
            if now + POLL_WINDOW_FRAMES < next {
                sleep_frames(next - now, POLL_WINDOW_FRAMES);
            }
            if now != stalled_at {
                stalled_at = now;
                stalled_since = Instant::now();
            } else if stalled_since.elapsed() > STALL_TIMEOUT {
                return Err(MusicError::Coherence(format!(
                    "the guest's frame counter has been {now} for {}s while the trace waited for \
                     {next}: the guest has stopped, and nothing past it was sampled",
                    stalled_since.elapsed().as_secs()
                )));
            }
            continue;
        }
        // The sample: one read of the whole segment, bracketed by two reads of
        // the guest's own counter. The counter is read *immediately* before the
        // memory read, so the bracket is the memory read and little else.
        if now > next {
            // The guest's own counter was seen past these frames: it went from
            // `seen` to `now` between two reads of it, and nothing in between
            // was ever the current frame. The frames are named rather than
            // filled in with the one that follows them.
            skipped.push(Skipped {
                from: next,
                to: now - 1,
                counter_before: seen,
                counter_after: now,
                why: format!(
                    "the guest's own frame counter was read as {seen} and then as {now} with no \
                     reading of the frames between, so they were never the current frame and no \
                     sample could be taken at them"
                ),
            });
            next = now;
        }
        seen = now;
        let before = oracle.frame_counter()?;
        let bytes = oracle.read_memory(0, base, map.segment_length as usize)?;
        let after = oracle.frame_counter()?;
        if before == after {
            samples.push(parse_sample(&bytes, before, &map, base)?);
            next = before + 1;
        } else {
            // The counter moved across the read: the bytes are not a value for
            // either frame with certainty, so they are dropped. The sample is
            // re-taken at the frame the guest now reports — which is why a
            // sample that arrives late is one frame with a different number,
            // never a wrong value under the right number.
            if after > next {
                skipped.push(Skipped {
                    from: next,
                    to: after - 1,
                    counter_before: before,
                    counter_after: after,
                    why: if after > before + 1 {
                        format!(
                            "the guest's frame counter went from {before} to {after} across one \
                             read — more than one frame at once — so the frames between were never \
                             the current frame"
                        )
                    } else {
                        format!(
                            "the read that was opened on frame {before} was still open when the \
                             guest moved to {after}, so the bytes belong to neither frame with \
                             certainty and were dropped"
                        )
                    },
                });
            }
            next = after;
        }
    }

    // The guest is the judge of whether the keys answered the setup screen, not
    // the key names (D17): a demo that never reached graphics is a trace of the
    // menu, and every number in it would be about nothing.
    if answering {
        if let Err(error) = oracle.wait_for_graphics(SETUP_ANSWER_TIMEOUT) {
            let detail = match error {
                OracleError::Timeout(message) => message,
                other => other.to_string(),
            };
            return Err(MusicError::Coherence(format!(
                "the keys {:?} did not start the demo: {detail}",
                options.keys.join(" ")
            )));
        }
    }

    // The segment must not have moved: a moved segment would make every later
    // sample a different variable, which is the failure that would otherwise
    // look like a music anomaly.
    let rerun = find_segment(oracle, &object, &map, &needles)?;
    if !rerun.views.contains(&base) {
        return Err(MusicError::Coherence(format!(
            "the STMIK segment moved during the trace: the trace read 0x{base:06X} and the \
             needles are now at {}; every sample after the move would be a different variable, \
             so nothing was written",
            rerun
                .views
                .iter()
                .map(|view| format!("0x{view:06X}"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    locate_record.rerun_base = rerun.base;
    locate_record.rerun_views = rerun.views.clone();
    locate_record.rerun_frame = rerun.frame;
    let machine_at_end = read_words(oracle, base, &map, MACHINE_WORDS)?;
    if rerun.views != location.views {
        // The views the second scan found are recorded either way; what matters
        // is that the base the trace read is among them and that they are one
        // memory, which `prove_views` checked inside the second scan.
        locate_record.aliasing = match (&locate_record.aliasing, &rerun.aliasing) {
            (Some(first), Some(second)) => Some(format!("{first} At the end of the trace: {second}")),
            (Some(first), None) => Some(first.clone()),
            (None, Some(second)) => Some(second.clone()),
            (None, None) => None,
        };
    }

    // ---------------------------------------------------------------------
    // The coherence checks. A trace whose numbers fail these is not written:
    // the failure this exists for is a data file that looks like a measurement
    // and is a record of the wrong bytes.
    // ---------------------------------------------------------------------
    check_samples(&samples, &map)?;
    let check = behaviour(&samples);

    let text = render_trace(&samples);
    let trace_file = TraceFile {
        file: TRACE_FILE.to_string(),
        columns: COLUMNS.iter().map(|column| (*column).to_string()).collect(),
        rows: samples.len() as u64,
        bytes: text.len() as u64,
        sha256: sha256_hex(text.as_bytes()),
    };
    let (machine, machine_moved) = merge_machine_words(machine_at_start, machine_at_end);

    let manifest = TraceManifest {
        version: MANIFEST_VERSION,
        kind: TRACE_KIND.to_string(),
        oracle: oracle_name,
        upstream_commit: UPSTREAM_COMMIT.to_string(),
        exe: options
            .exe
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        arguments: options.arguments.clone(),
        cycles: options.cycles,
        frame_origin: FRAME_ORIGIN.to_string(),
        skip_boot_frames: 0,
        boot: options.boot,
        keys: options.keys.clone(),
        coverage: Coverage::of(window, &frames_in(&samples)),
        trace: trace_file,
        object: map,
        locate: locate_record,
        machine,
        machine_moved,
        check,
        skipped,
    };

    write_trace(&options.out, &text, &manifest, options.force)?;
    Ok(manifest)
}

/// Where the STMIK segment was found: the base the trace reads through, every
/// view of it the needles were found at, and the frame it was found on.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Location {
    /// The base every sample is read through: the lowest view, so the choice is
    /// a function of the addresses and not of the order a scan happened to
    /// visit them in.
    base: u32,
    /// Every view, sorted and deduplicated. More than one is not an error here
    /// — see [`find_segment`] — but it is a fact the manifest records.
    views: Vec<u32>,
    /// The sentence that says why several views are one memory, when there are
    /// several.
    aliasing: Option<String>,
    frame: u64,
}

/// Scan for the segment until it is there, and say which frame it was found on
/// and where each needle was.
///
/// **The needles must agree.** Each needle is searched on its own, and the bases
/// they solve must be the same set — one needle's unique match is a coincidence
/// until a second, independently chosen run lands at the same place.
///
/// **Several views are accepted only as views of one memory.** The demo's DOS
/// extender maps the same physical page at more than one linear address (the
/// emulator's memory read goes through the page tables, so both addresses answer
/// with the same bytes), and the needles are then found at both. Before a
/// several-view location is used, the whole segment is read through every view
/// and required to be **byte-identical** to the lowest one — a genuinely stale
/// second copy would differ, and a trace read through it would be a record of
/// dead bytes. What was compared, and at which frame, goes into the manifest.
fn find_segment(
    oracle: &mut dyn Oracle,
    object: &[u8],
    map: &SymbolMap,
    needles: &[Needle; 2],
) -> Result<Location, MusicError> {
    let first = needle_bytes(object, &needles[0])?;
    let second = needle_bytes(object, &needles[1])?;
    // A reader that cannot read at all is a failure, not a segment that is not
    // there yet: the locator's own `read` answers an empty slice for both, so
    // the difference is measured here, once.
    oracle
        .read_memory(0, 0, 16)
        .map_err(|error| MusicError::Locate(LocateError::Read(error.to_string())))?;
    let deadline = Instant::now() + LOCATE_TIMEOUT;
    loop {
        let found = {
            let mut read = |segment: u16, offset: u32, len: usize| -> Vec<u8> {
                oracle.read_memory(segment, offset, len).unwrap_or_default()
            };
            let one = locate(needles[0].offset as u16, &first, &mut read);
            let two = locate(needles[1].offset as u16, &second, &mut read);
            (one, two)
        };
        match found {
            (Ok((_, one)), Ok((_, two))) => {
                if one != two {
                    return Err(MusicError::Locate(LocateError::BadNeedle(format!(
                        "the two needles disagree: the run at 0x{:04X} solves base 0x{one:06X} and \
                         the run at 0x{:04X} solves base 0x{two:06X} — one match is a coincidence \
                         until a second, independent run lands at the same base",
                        needles[0].offset, needles[1].offset
                    ))));
                }
                let frame = oracle.frame_counter()?;
                return Ok(Location {
                    base: one,
                    views: vec![one],
                    aliasing: None,
                    frame,
                });
            }
            (
                Err(LocateError::Ambiguous { bases: one, .. }),
                Err(LocateError::Ambiguous { bases: two, .. }),
            ) => {
                if one != two {
                    return Err(MusicError::Locate(LocateError::BadNeedle(format!(
                        "the two needles do not agree about where the segment is: the run at \
                         0x{:04X} solves {}, the run at 0x{:04X} solves {}",
                        needles[0].offset,
                        one.iter().map(|b| format!("0x{b:06X}")).collect::<Vec<_>>().join(", "),
                        needles[1].offset,
                        two.iter().map(|b| format!("0x{b:06X}")).collect::<Vec<_>>().join(", "),
                    ))));
                }
                let frame = oracle.frame_counter()?;
                let aliasing = prove_views(oracle, map, &one, frame)?;
                return Ok(Location {
                    base: one[0],
                    views: one,
                    aliasing: Some(aliasing),
                    frame,
                });
            }
            (Err(LocateError::Read(message)), _) | (_, Err(LocateError::Read(message))) => {
                return Err(MusicError::Locate(LocateError::Read(message)))
            }
            (one, two) => {
                // Anything else — not found yet, or one needle matching in
                // several views while the other has not caught up — is "look
                // again in a few frames".
                if Instant::now() >= deadline {
                    let error = one
                        .err()
                        .or_else(|| two.err())
                        .unwrap_or(LocateError::NotFound { scanned: 0 });
                    return Err(MusicError::Locate(error));
                }
            }
        }
        oracle.step_frames(8)?;
    }
}

/// Show that several bases are views of one memory: the whole segment, read
/// through each of them, is byte-identical.
///
/// Returns the sentence the manifest records, or refuses. A stale second copy —
/// one the guest is not writing any more — has the needles in it (they are the
/// object's own data) and would read as a perfectly good segment base until the
/// player moved; this is the check that tells the two apart *before* a trace is
/// taken through either.
fn prove_views(
    oracle: &mut dyn Oracle,
    map: &SymbolMap,
    views: &[u32],
    frame: u64,
) -> Result<String, MusicError> {
    let canonical = oracle.read_memory(0, views[0], map.segment_length as usize)?;
    for view in &views[1..] {
        let other = oracle.read_memory(0, *view, map.segment_length as usize)?;
        if other != canonical {
            let at = canonical
                .iter()
                .zip(&other)
                .position(|(a, b)| a != b)
                .unwrap_or(0);
            return Err(MusicError::Locate(LocateError::BadNeedle(format!(
                "the needle is in {} places and the bytes around them are NOT one memory: the \
                 copy at 0x{:06X} differs from the one at 0x{:06X} at offset 0x{at:04X} \
                 ({} against {}), so one of them is a stale copy and a trace read through it \
                 would be a record of dead bytes",
                views.len(),
                views[1],
                views[0],
                canonical[at],
                other[at]
            ))));
        }
    }
    Ok(format!(
        "the needle's bytes are in {} places and they are one memory: the whole {} bytes of the \
         segment read through each base ({}) are byte-identical at frame {frame} — the demo's \
         DOS extender maps one physical page at several linear addresses, and the emulator's \
         memory read goes through the page tables. The trace reads through the lowest, \
         0x{:06X}.",
        views.len(),
        map.segment_length,
        views
            .iter()
            .map(|view| format!("0x{view:06X}"))
            .collect::<Vec<_>>()
            .join(", "),
        views[0]
    ))
}

/// Read the named words out of the guest, at the base the locator solved.
///
/// The values are read as the 16-bit words `MAIN/STMIK.H` declares them to be,
/// and no range is imposed here: `_zmixspeed` is a mixing rate in Hz (this
/// machine's is 20000, Step 4.4) and `_zbufsize` is a buffer size in bytes, so a
/// "byte-valued word" check belongs on the words that *are* byte-valued — the
/// position and clock words, which [`sample`] and [`check_samples`] check.
fn read_words(
    oracle: &mut dyn Oracle,
    base: u32,
    map: &SymbolMap,
    names: &[&str],
) -> Result<BTreeMap<String, i64>, MusicError> {
    let mut out = BTreeMap::new();
    for name in names {
        let offset = map.offset(name)?;
        let bytes = oracle.read_memory(0, base + offset, 2)?;
        if bytes.len() != 2 {
            return Err(MusicError::Coherence(format!(
                "reading {name} at 0x{:06X} returned {} byte(s)",
                base + offset,
                bytes.len()
            )));
        }
        out.insert(
            name.to_string(),
            i64::from(i16::from_le_bytes([bytes[0], bytes[1]])),
        );
    }
    Ok(out)
}

/// The machine's words as the manifest records them: the reading taken when the
/// segment was located, and the reading taken at the end.
///
/// A word the two readings agree about is recorded once; a word that moved is
/// recorded **twice**, in the second map, and never merged: silently keeping one
/// of two disagreeing readings is how a record comes to describe a machine that
/// was never in one state. Nothing here is assumed constant — the mixing rate
/// and `_zpollmix` both move during a run (see [`TraceManifest::machine_moved`]).
fn merge_machine_words(
    start: BTreeMap<String, i64>,
    end: BTreeMap<String, i64>,
) -> (BTreeMap<String, i64>, BTreeMap<String, i64>) {
    let mut machine = start;
    let mut moved = BTreeMap::new();
    for (name, value) in end {
        match machine.get(&name) {
            // A word whose two readings differ is **recorded twice**, not
            // merged and not refused: the manifest says what it read at the
            // locate and what it read at the end, and a reader can see that the
            // machine changed. Which words change is not a rule this tool gets
            // to assume — `_np_mastermul` moves because the module is loaded
            // and `fademusic` walks the volume (`MAIN/U2.ASM:333-346`), and
            // `_zmixspeed` moves because the mixing rate is set again later in
            // the run (20000 → 40000 in `second5`'s session) — so nothing is
            // assumed constant.
            Some(first) if *first != value => {
                moved.insert(name, value);
            }
            Some(_) => {}
            None => {
                machine.insert(name, value);
            }
        }
    }
    (machine, moved)
}

/// The player's words, taken out of one read of the whole segment.
///
/// `frame` is the frame the guest reported on **both** sides of the read — the
/// caller only calls this for a sample whose two counter reads agreed, so the
/// bytes belong to that frame and to no other.
fn parse_sample(
    bytes: &[u8],
    frame: u64,
    map: &SymbolMap,
    base: u32,
) -> Result<PlayerState, MusicError> {
    let word = |name: &str| -> Result<i16, MusicError> {
        let offset = map.offset(name)? as usize;
        let raw = bytes.get(offset..offset + 2).ok_or_else(|| {
            MusicError::Coherence(format!(
                "{name} at offset {offset} is past the {} bytes the segment read returned",
                bytes.len()
            ))
        })?;
        Ok(i16::from_le_bytes([raw[0], raw[1]]))
    };
    let byte = |name: &str| -> Result<u8, MusicError> {
        let value = word(name)?;
        u8::try_from(value).map_err(|_| {
            MusicError::Coherence(format!(
                "{name} reads {value}, which is not a byte value: the base 0x{base:06X} is not \
                 this segment's"
            ))
        })
    };
    Ok(PlayerState {
        frame,
        ord: byte("_np_ord")?,
        row: byte("_np_row")?,
        pat: byte("_np_pat")?,
        loop_: byte("_np_loop")?,
        speed: byte("_np_speed")?,
        tempo: byte("_np_tempo")?,
        zinfo: word("_np_zinfo")?,
        zplus: word("_np_zplus")?,
        zframe: word("_np_zframe")?,
    })
}

/// The checks a trace has to pass before it is written at all.
fn check_samples(samples: &[PlayerState], map: &SymbolMap) -> Result<(), MusicError> {
    if samples.is_empty() {
        return Err(MusicError::Coherence(
            "no frame was sampled: there is nothing to write".into(),
        ));
    }
    for pair in samples.windows(2) {
        if pair[1].frame <= pair[0].frame {
            return Err(MusicError::Coherence(format!(
                "the frame counter is not strictly increasing: {} then {}",
                pair[0].frame, pair[1].frame
            )));
        }
    }
    for state in samples {
        if state.row > 63 {
            return Err(MusicError::Coherence(format!(
                "at frame {} _np_row reads {}: an S3M row is 0..=63, so this is not the word the \
                 map put at offset 0x{:04X}",
                state.frame, state.row, map.words["_np_row"]
            )));
        }
        if state.speed == 0 || state.tempo == 0 {
            return Err(MusicError::Coherence(format!(
                "at frame {} the player reports speed {} tempo {}: both are 1..=255, so this is \
                 not the player's clock",
                state.frame, state.speed, state.tempo
            )));
        }
    }
    Ok(())
}

/// The behavioural check: what the words did over the whole capture.
fn behaviour(samples: &[PlayerState]) -> Behaviour {
    let first = samples.first().expect("checked not empty");
    let mut row = [first.row, first.row];
    let mut ord = [first.ord, first.ord];
    let mut pat = [first.pat, first.pat];
    let mut speeds: Vec<u8> = Vec::new();
    let mut tempos: Vec<u8> = Vec::new();
    let mut zinfo_min = first.zinfo;
    let mut zinfo_max = first.zinfo;
    let mut first_advance = None;
    let mut previous: Option<&PlayerState> = None;
    let mut zframe_moves = 0u64;
    let mut zframe_total = 0i64;
    for state in samples {
        row = [row[0].min(state.row), row[1].max(state.row)];
        ord = [ord[0].min(state.ord), ord[1].max(state.ord)];
        pat = [pat[0].min(state.pat), pat[1].max(state.pat)];
        if !speeds.contains(&state.speed) {
            speeds.push(state.speed);
        }
        if !tempos.contains(&state.tempo) {
            tempos.push(state.tempo);
        }
        zinfo_min = zinfo_min.min(state.zinfo);
        zinfo_max = zinfo_max.max(state.zinfo);
        if let Some(earlier) = previous {
            if first_advance.is_none() && state.position_moved_from(earlier) {
                first_advance = Some(state.frame);
            }
            if earlier.zframe != state.zframe {
                zframe_moves += 1;
                zframe_total += i64::from(state.zframe) - i64::from(earlier.zframe);
            }
        }
        previous = Some(state);
    }
    Behaviour {
        row,
        ord,
        pat,
        speeds,
        tempos,
        zinfo_min,
        zinfo_max,
        first_advance,
        zframe_moves,
        zframe_total,
    }
}

/// The trace as it is written: the header, then one line per sample.
fn render_trace(samples: &[PlayerState]) -> String {
    let mut text = String::with_capacity(samples.len() * 40 + 64);
    text.push_str(&PlayerState::header());
    text.push('\n');
    for state in samples {
        text.push_str(&state.line());
        text.push('\n');
    }
    text
}

/// Write the trace and the manifest, trace first: a directory half written has
/// no manifest, and a manifest is what makes a directory a capture.
fn write_trace(
    out: &Path,
    text: &str,
    manifest: &TraceManifest,
    force: bool,
) -> Result<(), MusicError> {
    let trace_path = out.join(TRACE_FILE);
    let manifest_path = out.join("manifest.json");
    if !force && (trace_path.exists() || manifest_path.exists()) {
        return Err(MusicError::Options(format!(
            "{} already holds a trace ({}); pass --force to replace it",
            out.display(),
            if trace_path.exists() {
                TRACE_FILE
            } else {
                "manifest.json"
            }
        )));
    }
    std::fs::create_dir_all(out).map_err(|e| MusicError::Io {
        path: out.to_path_buf(),
        message: e.to_string(),
    })?;
    std::fs::write(&trace_path, text).map_err(|e| MusicError::Io {
        path: trace_path.clone(),
        message: e.to_string(),
    })?;
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| MusicError::Options(format!("cannot serialise the manifest: {e}")))?;
    std::fs::write(&manifest_path, format!("{json}\n")).map_err(|e| MusicError::Io {
        path: manifest_path,
        message: e.to_string(),
    })?;
    Ok(())
}

// -------------------------------------------------------------------------------------
// The commands
// -------------------------------------------------------------------------------------

/// `cargo xtask music symbols --object …`: print the map, so every address the
/// trace reads has a command behind it.
pub fn symbols_command(object_path: &Path) -> Result<ExitCode, MusicError> {
    let object = std::fs::read(object_path).map_err(|e| MusicError::Io {
        path: object_path.to_path_buf(),
        message: e.to_string(),
    })?;
    let walk = symbols::walk(&object).map_err(|error| {
        MusicError::Symbols(format!("{}: {error}", object_path.display()))
    })?;
    println!(
        "object: {} — {} bytes, {} record(s), sha256 {}",
        object_path.display(),
        object.len(),
        walk.records(),
        sha256_hex(&object)
    );
    println!("  every record the walk covers is 3 + length bytes: {} in all", walk.bytes);
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
    let map = symbol_map(object_path, &object).map_err(|error| {
        MusicError::Symbols(format!("{}: {error}", object_path.display()))
    })?;
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

/// `cargo xtask music probe`: measure one 16-bit word of the STMIK segment over
/// a frame window, and place it against the object's symbols.
///
/// This is the command behind this note's cross-check of
/// `docs/fidelity/reference-indices.md` §1.7 — a word at an address that is not
/// one of the published symbols cannot be read out of a *trace*, so it is
/// measured live, at the same recipe the traces use.
pub fn probe_command(options: &TraceOptions, at: u32) -> Result<ExitCode, MusicError> {
    validate_trace_options(options)?;
    let autoexec = options.autoexec_lines()?;
    let mut oracle =
        crate::oracle::dosbox::DosboxOracle::spawn(options.cycles, &options.cwd, &autoexec)?;
    let object = std::fs::read(&options.object).map_err(|e| MusicError::Io {
        path: options.object.clone(),
        message: e.to_string(),
    })?;
    let map = symbol_map(&options.object, &object)?;
    let needles = needles(&object)?;
    oracle.set_cycles(options.cycles)?;
    oracle.boot(&options.exe, &options.cwd, &options.arguments)?;
    let answering = !options.keys.is_empty();
    if options.boot == BootRecipe::Typed && answering {
        oracle.wait_for_text(SETUP_SCREEN_TEXT, DEFAULT_WAIT)?;
        oracle.send_keys(&options.keys.join(" "))?;
    }
    let location = find_segment(&mut oracle, &object, &map, &needles)?;
    let views: Vec<u32> = location
        .views
        .iter()
        .copied()
        .filter(|view| at >= *view && at < *view + map.segment_length)
        .collect();
    println!(
        "address 0x{at:06X}: the segment's base(s) are {} (of {} byte(s))",
        location
            .views
            .iter()
            .map(|view| format!("0x{view:06X}"))
            .collect::<Vec<_>>()
            .join(", "),
        map.segment_length
    );
    if views.is_empty() {
        println!("  it is not inside this segment in any view, so nothing was read");
        oracle.shutdown()?;
        return Ok(ExitCode::SUCCESS);
    }
    let view = views[0];
    let offset = at - view;
    println!("  in the view at 0x{view:06X} it is that base + 0x{offset:04X}");
    let walk = symbols::walk(&object)?;
    let mut sorted: Vec<&Symbol> = walk.symbols.iter().collect();
    sorted.sort_by_key(|symbol| symbol.offset);
    match sorted.iter().rfind(|symbol| symbol.offset <= offset) {
        Some(symbol) => println!(
            "  the last published symbol at or below it is {} (base + 0x{:04X}, {} byte(s) \
             below); the next above is {}",
            symbol.name,
            symbol.offset,
            offset - symbol.offset,
            sorted
                .iter()
                .find(|next| next.offset > offset)
                .map(|next| format!("{} at base + 0x{:04X}", next.name, next.offset))
                .unwrap_or_else(|| "nothing".into())
        ),
        None => println!("  no published symbol is at or below it"),
    }

    // Read the word at the window's first and last frames, and say the rate.
    // The word asked for **and** `_np_zframe`, read in the same bracketed
    // sample: the two are only comparable if they are measured over the same
    // frames of the same session, and that is what tells a counter running at
    // the playing tempo's tick rate from one running at something else.
    let zframe_at = view + map.offset("_np_zframe")?;
    let mut samples: Vec<(u64, i16, i16)> = Vec::new();
    // From the window's own first frame, not from wherever the locate left the
    // guest: a rate measured from the segment's arrival would include the boot.
    let mut next = oracle.frame_counter()?.max(options.window[0]) + 1;
    while next <= options.window[1] {
        let now = oracle.frame_counter()?;
        if now < next {
            if now + POLL_WINDOW_FRAMES < next {
                sleep_frames(next - now, POLL_WINDOW_FRAMES);
            }
            continue;
        }
        let bytes = oracle.read_memory(0, at, 2)?;
        let zframe = oracle.read_memory(0, zframe_at, 2)?;
        let after = oracle.frame_counter()?;
        if now == after && after >= next {
            samples.push((
                after,
                i16::from_le_bytes([bytes[0], bytes[1]]),
                i16::from_le_bytes([zframe[0], zframe[1]]),
            ));
            next = after + 1;
        } else {
            next = after;
        }
    }
    if let (Some((first_frame, first, _)), Some((last_frame, last, _))) =
        (samples.first(), samples.last())
    {
        let rate = (f64::from(*last) - f64::from(*first)) / (*last_frame - *first_frame) as f64;
        // The words in this segment are a mix of `int` and `unsigned`
        // (`MAIN/STMIK.H`), so a value above 32767 is printed both ways: 40000
        // as a signed 16-bit word is -25536, and a reader who sees only that
        // would think the mixing rate had gone negative.
        let show = |value: i16| -> String {
            if value < 0 {
                format!("{value} ({} unsigned)", u16::from_le_bytes(value.to_le_bytes()))
            } else {
                value.to_string()
            }
        };
        println!(
            "  frame {first_frame}: {}; frame {last_frame}: {} — {rate:+.4} per frame, {:+.2} per \
             second at {FRAME_RATE_HZ} Hz",
            show(*first),
            show(*last),
            rate * FRAME_RATE_HZ
        );
        println!("  {} sample(s) between them", samples.len());
        // The same measure for `_np_zframe`, split at its resets when it has
        // any: the two counters are what §8 of the note is about.
        let resets = samples
            .windows(2)
            .filter(|pair| pair[1].2 < pair[0].2)
            .count();
        let longest = samples
            .split(|sample| sample.2 == 0)
            .max_by_key(|run| run.len())
            .filter(|run| run.len() > 1);
        if let Some(run) = longest {
            let (first_frame, _, zfirst) = run[0];
            let (last_frame, _, zlast) = run[run.len() - 1];
            let zrate =
                (f64::from(zlast) - f64::from(zfirst)) / (last_frame - first_frame) as f64;
            println!(
                "  _np_zframe over the same window: {zfirst} at frame {first_frame} to {zlast} at \
                 frame {last_frame} — {zrate:+.4} per frame, {:+.2} per second ({resets} \
                 reset(s))",
                zrate * FRAME_RATE_HZ
            );
        }
        // The player's own clock at the same moment, so the measured word can
        // be compared with the tempo that is playing: a word running at the
        // tick rate of a tempo the demo is not playing is a different counter,
        // and that is only sayable with both numbers in one output.
        let clock = read_words(
            &mut oracle,
            view,
            &map,
            &["_np_tempo", "_np_speed", "_np_zframe", "_np_ord", "_np_row"],
        )?;
        println!(
            "  the player's own words at frame {last_frame}: tempo {}, speed {}, zframe {}, ord \
             {}, row {}",
            clock["_np_tempo"], clock["_np_speed"], clock["_np_zframe"], clock["_np_ord"],
            clock["_np_row"]
        );
    } else {
        println!("  no frame in the window was sampled");
    }
    oracle.shutdown()?;
    Ok(ExitCode::SUCCESS)
}

/// `cargo xtask music trace`: boot the original, trace the player, write it.
pub fn trace_command(options: &TraceOptions) -> Result<ExitCode, MusicError> {
    validate_trace_options(options)?;
    let autoexec = options.autoexec_lines()?;
    let mut oracle = crate::oracle::dosbox::DosboxOracle::spawn(options.cycles, &options.cwd, &autoexec)?;
    let manifest = trace(&mut oracle, options)?;
    oracle.shutdown()?;
    println!(
        "traced {} frame(s) into {} (window {}..{}, {}, oracle {}, base 0x{:06X})",
        manifest.trace.rows,
        options.out.display(),
        manifest.coverage.window[0],
        manifest.coverage.window[1],
        manifest.coverage.describe(),
        manifest.oracle,
        manifest.locate.base,
    );
    for skip in &manifest.skipped {
        println!(
            "  not sampled: frames {}..{} (the counter went {} -> {}) — {}",
            skip.from, skip.to, skip.counter_before, skip.counter_after, skip.why
        );
    }
    println!(
        "  music advances first at frame {}; row {}..{}, ord {}..{}, pat {}..{}, speed {:?}, \
         tempo {:?}",
        manifest
            .check
            .first_advance
            .map(|frame| frame.to_string())
            .unwrap_or_else(|| "never".into()),
        manifest.check.row[0],
        manifest.check.row[1],
        manifest.check.ord[0],
        manifest.check.ord[1],
        manifest.check.pat[0],
        manifest.check.pat[1],
        manifest.check.speeds,
        manifest.check.tempos,
    );
    for needle in &manifest.locate.needles {
        println!(
            "  needle at 0x{:04X} found at 0x{:06X} (sha256 {})",
            needle.offset,
            needle.found_at.unwrap_or(0),
            needle.sha256
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// `keys_from_script` for the CLI, so `--keys` means the same thing here as it
/// does for `reference capture`.
pub fn keys(script: &str) -> Vec<String> {
    keys_from_script(script)
}

/// `--frames`: a list, a range, or a list of ranges, as one window.
///
/// `A..B` is the Rust range — A up to but **not** including B — and `A..=B`
/// includes B, because a command that silently includes or excludes its last
/// frame is a command whose coverage nobody can predict. `0..1300` is 1300
/// frames, 0 through 1299. Everything given has to be one contiguous run: the
/// trace's coverage is reported as one window, and a window with a hole in it
/// the user asked for would be reported as a defect of the trace.
pub fn parse_window(spec: &str) -> Result<[u64; 2], MusicError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(MusicError::Options(
            "--frames is empty: give a range (0..1300), an inclusive range (0..=1299), or a \
             comma-separated list of frames"
                .into(),
        ));
    }
    let mut frames: Vec<u64> = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(MusicError::Options(format!(
                "--frames {spec:?} has an empty entry"
            )));
        }
        if let Some((start, end)) = part.split_once("..=") {
            let start = parse_frame(start, spec)?;
            let end = parse_frame(end, spec)?;
            if end < start {
                return Err(MusicError::Options(format!(
                    "--frames {part:?}: {} is before {}",
                    end, start
                )));
            }
            frames.extend(start..=end);
        } else if let Some((start, end)) = part.split_once("..") {
            let start = parse_frame(start, spec)?;
            let end = parse_frame(end, spec)?;
            if end <= start {
                return Err(MusicError::Options(format!(
                    "--frames {part:?} is empty: A..B is A up to but not including B, so B must be \
                     after A (use A..=B to include B)"
                )));
            }
            frames.extend(start..end);
        } else {
            frames.push(parse_frame(part, spec)?);
        }
    }
    frames.sort_unstable();
    for pair in frames.windows(2) {
        if pair[1] == pair[0] {
            return Err(MusicError::Options(format!(
                "--frames {spec:?} names frame {} twice",
                pair[0]
            )));
        }
    }
    let first = frames[0];
    let last = frames[frames.len() - 1];
    if last - first + 1 != frames.len() as u64 {
        return Err(MusicError::Options(format!(
            "--frames {spec:?} is not one contiguous run: {} frame(s) from {first} to {last} \
             would need {}, so there is a hole in the window",
            frames.len(),
            last - first + 1
        )));
    }
    Ok([first, last])
}

fn parse_frame(text: &str, spec: &str) -> Result<u64, MusicError> {
    text.trim().parse::<u64>().map_err(|e| {
        MusicError::Options(format!("--frames {spec:?}: {} is not a frame index: {e}", text.trim()))
    })
}

// -------------------------------------------------------------------------------------
// Step 4: the analysis
// -------------------------------------------------------------------------------------

/// `cargo xtask music analyse --trace …`: everything the note's numbers are
/// read from.
pub fn analyse_command(trace_dir: &Path, at: Option<u32>) -> Result<ExitCode, MusicError> {
    let analysis = Analysis::read(trace_dir)?;
    analysis.print(at)?;
    Ok(ExitCode::SUCCESS)
}

/// A trace on disk, read back, with the module its clock belongs to.
pub struct Analysis {
    pub dir: PathBuf,
    pub manifest: TraceManifest,
    pub samples: Vec<PlayerState>,
    /// The module whose header speed matches the trace's, when one does.
    pub module: Option<Module>,
    /// The coverage **recomputed from the trace file's own frames**, which is
    /// what §7 of the note is about: the manifest's claim is checked against it
    /// by `reference verify`, and printing the recomputed one means the
    /// pre-sampling boundary is named even for a trace filed before the
    /// manifest carried `before_sampling`.
    pub coverage: Coverage,
}

/// How many samples the `zplus` rule was checked against, and how many it got
/// wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZplusCheck {
    pub samples: usize,
    pub mismatches: usize,
    /// The first few disagreements: frame, order, row, measured, expected.
    pub examples: Vec<(u64, u8, u8, i16, i16)>,
    /// The orders the trace held, in the order the counter first held them.
    pub held: Vec<u8>,
}

/// **The port's `zplus` rule**, re-derived over a trace's own order sequence.
///
/// `zplus` is what `muscode_6` (`DIS/DISINT.ASM:242-282`) turns into a row
/// offset for `dis_muscode`, so the port has to compute it rather than copy it.
/// The rule below is the one that reproduces the three filed traces over every
/// sample of all of them, and the note states it as **the port's rule, verified
/// over the filed traces** — not as a reading of the original's source:
///
/// - **1** while the counter is playing an order whose *next* entry in the
///   module's order list is `254` — the marker is coming;
/// - **2** on a marker crossing: the counter moved from `o` to `o + 2` with
///   `orders[o + 1] == 254`, **unless** the entry after the one being entered is
///   itself `254`, in which case it is **1**;
/// - **0** otherwise;
/// - and a seek or a restart credits **no** marker behind it: a crossing needs
///   the order the counter came from to be the one before the skipped entry,
///   which is why `SECOND 3`'s restart at order 42 — whose list predecessor is
///   a `254` the player never played — reads 0.
///
/// What the traces do **not** show, and the note says so: no trace holds an
/// order two steps past a marker (`MUSIC0` order 29 onwards, `MUSIC1` order 49
/// onwards), so `2` returning to `0` on the next natural advance is
/// extrapolation, and so is anything about a `255`.
pub fn zplus_rule(orders: &[u8], held: &[u8]) -> Vec<i16> {
    let mut expected = Vec::with_capacity(held.len());
    for (position, order) in held.iter().enumerate() {
        let order = usize::from(*order);
        let next_is_marker = orders.get(order + 1) == Some(&254);
        let crossing = position
            .checked_sub(1)
            .filter(|_| order > 0)
            .map(|earlier| {
                usize::from(held[earlier]) + 2 == order && orders.get(order - 1) == Some(&254)
            })
            .unwrap_or(false);
        expected.push(if next_is_marker {
            1
        } else if crossing {
            2
        } else {
            0
        });
    }
    expected
}

/// One run of constant `(speed, tempo)` in which rows advanced: what the tick
/// rate is measured over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateRun {
    pub speed: u8,
    pub tempo: u8,
    /// The run's first and last row change, which is what the measurement runs
    /// between; `None` while no row has advanced yet.
    pub first_change: Option<u64>,
    pub last_change: Option<u64>,
    /// The row changes inside the run: `rows - 1` row intervals.
    pub rows: usize,
}

impl RateRun {
    /// The frames the measurement runs over: from the first row change to the
    /// last.
    pub fn frames(&self) -> u64 {
        match (self.first_change, self.last_change) {
            (Some(first), Some(last)) => last - first,
            _ => 0,
        }
    }

    /// The row intervals between the first and the last change.
    pub fn intervals(&self) -> usize {
        self.rows.saturating_sub(1)
    }

    /// How many frames the module's own rule predicts for those intervals:
    /// `intervals x speed` ticks, a tick being `2.5/tempo` seconds, at the
    /// demo's own frame rate. A **prediction**, not a fit.
    pub fn predicted_frames(&self) -> f64 {
        self.intervals() as f64 * f64::from(self.speed) * (2.5 / f64::from(self.tempo)) * FRAME_RATE_HZ
    }

    /// One row's worth of frames: the resolution the prediction can be checked
    /// to, because a row boundary lands on a whole frame.
    pub fn row_frames(&self) -> f64 {
        f64::from(self.speed) * (2.5 / f64::from(self.tempo)) * FRAME_RATE_HZ
    }

    /// The residual: the frames measured minus the frames predicted.
    pub fn residual(&self) -> f64 {
        self.frames() as f64 - self.predicted_frames()
    }
}

/// An S3M header, as much of it as the tick rate needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub path: String,
    /// `ordnum`, `patnum` and the order list, from the header.
    pub orders: Vec<u8>,
    pub patterns: u16,
    /// The file's own speed and tempo bytes (offsets 0x31 and 0x32).
    pub speed: u8,
    pub tempo: u8,
    /// The tempo the demo patches into the module before playing it
    /// (`MAIN/STARTMUS.C:257`), when it patches one.
    pub patched_tempo: Option<u8>,
}

impl Module {
    /// The tempo the player will see: the patch, or the file's own byte.
    pub fn playing_tempo(&self) -> u8 {
        self.patched_tempo.unwrap_or(self.tempo)
    }
}

impl Analysis {
    /// Read a trace directory: the manifest, then the file the manifest names.
    pub fn read(dir: &Path) -> Result<Analysis, MusicError> {
        let manifest_path = dir.join("manifest.json");
        let text = std::fs::read_to_string(&manifest_path).map_err(|e| MusicError::Io {
            path: manifest_path.clone(),
            message: e.to_string(),
        })?;
        let manifest: TraceManifest =
            TraceManifest::from_json(&text).map_err(|e| MusicError::Trace(format!(
                "{}: {e}",
                manifest_path.display()
            )))?;
        if manifest.kind != TRACE_KIND {
            return Err(MusicError::Trace(format!(
                "{} is a {:?} capture; this command reads a {TRACE_KIND}",
                manifest_path.display(),
                manifest.kind
            )));
        }
        let trace_path = dir.join(&manifest.trace.file);
        let text = std::fs::read_to_string(&trace_path).map_err(|e| MusicError::Io {
            path: trace_path.clone(),
            message: e.to_string(),
        })?;
        let digest = sha256_hex(text.as_bytes());
        if digest != manifest.trace.sha256 {
            return Err(MusicError::Trace(format!(
                "{} records sha256 {} for {} and the file is {digest}: this is not the trace the \
                 manifest describes",
                manifest_path.display(),
                manifest.trace.sha256,
                manifest.trace.file
            )));
        }
        let mut samples = Vec::new();
        for (number, line) in text.lines().enumerate() {
            if number == 0 {
                let header = line.trim_start_matches('\u{feff}');
                if header != PlayerState::header() {
                    return Err(MusicError::Trace(format!(
                        "{}'s first line is {header:?} where the columns are {:?}",
                        trace_path.display(),
                        PlayerState::header()
                    )));
                }
                continue;
            }
            if line.trim().is_empty() {
                continue;
            }
            samples.push(PlayerState::parse(line)?);
        }
        if samples.len() as u64 != manifest.trace.rows {
            return Err(MusicError::Trace(format!(
                "{} holds {} row(s) and the manifest says {}",
                trace_path.display(),
                samples.len(),
                manifest.trace.rows
            )));
        }
        // Which module is playing is decided by the trace's own `(ord, pat)`
        // pairs against each module's order list — every order the counter held
        // has to name the pattern the trace shows — and not by the speed and
        // tempo words, which do not always agree with the file's header (the
        // block 3 trace plays MUSIC1's patterns while its speed word reads 6,
        // where that file says 3).
        let coverage = Coverage::of(manifest.coverage.window, &frames_in(&samples));
        let module = MODULES
            .iter()
            .filter_map(|(path, patched)| read_module(path, *patched).ok())
            .find(|module| {
                let mut pairs = 0usize;
                for state in &samples {
                    if state.ord == 0 && state.pat == 0 {
                        continue;
                    }
                    let Some(pattern) = module.orders.get(usize::from(state.ord)) else {
                        return false;
                    };
                    if *pattern != state.pat {
                        return false;
                    }
                    pairs += 1;
                }
                pairs > 0
            });
        Ok(Analysis {
            dir: dir.to_path_buf(),
            manifest,
            samples,
            module,
            coverage,
        })
    }

    /// The frames at which the row advanced, in order.
    fn row_changes(&self) -> Vec<u64> {
        self.samples
            .windows(2)
            .filter(|pair| pair[1].row != pair[0].row || pair[1].ord != pair[0].ord)
            .map(|pair| pair[1].frame)
            .collect()
    }

    /// The runs of constant `(speed, tempo)` in which a row advanced, measured
    /// **between the run's first and last row change**.
    ///
    /// The rate is measured per run and not over the whole trace because the
    /// module changes its own speed and tempo (`Axx`/`Txx` cells) — this trace
    /// shows speed 6 at one point and speed 1 at another — and a single average
    /// over both would be a fit to nothing.
    ///
    /// The measurement starts and ends *on a row change* rather than at the
    /// run's own first and last frames: a run begins and ends in the middle of
    /// a row, and counting a part-row as a whole one would put a whole row of
    /// error (8.8 frames at speed 6) into the residual, which is the size of
    /// the thing being measured. Between two row changes the intervals
    /// telescope, so what is left in the residual is the frame-quantisation of
    /// the tick clock and not the boundaries.
    fn rate_runs(&self) -> Vec<RateRun> {
        let mut runs: Vec<RateRun> = Vec::new();
        let mut previous: Option<&PlayerState> = None;
        for state in &self.samples {
            let moved = previous.map(|earlier| earlier.row != state.row);
            match runs.last_mut() {
                Some(run) if run.speed == state.speed && run.tempo == state.tempo => {
                    if moved == Some(true) {
                        if run.first_change.is_none() {
                            run.first_change = Some(state.frame);
                        }
                        run.last_change = Some(state.frame);
                        run.rows += 1;
                    }
                }
                _ => runs.push(RateRun {
                    speed: state.speed,
                    tempo: state.tempo,
                    first_change: None,
                    last_change: None,
                    rows: 0,
                }),
            }
            previous = Some(state);
        }
        // Only the runs a row advanced in are measurements: a run before the
        // module was started says nothing about the tick rate.
        let first = runs
            .iter()
            .position(|run| run.rows > 1)
            .unwrap_or(runs.len());
        runs.drain(..first);
        runs.retain(|run| run.rows > 1);
        runs
    }

    /// A machine word as it is printed: 16-bit, and both ways when it is above
    /// 32767. `MAIN/STMIK.H` declares some of these `unsigned` (`_zmixspeed`,
    /// `_zbufsize`) and some `int`, so 40000 — the mixing rate the music runs at
    /// — would otherwise print as -25536 and read as a negative rate.
    fn show_word(value: i64) -> String {
        match i16::try_from(value) {
            Ok(signed) if signed < 0 => format!(
                "{signed} ({} unsigned)",
                u16::from_le_bytes(signed.to_le_bytes())
            ),
            _ => value.to_string(),
        }
    }

    /// The first row change in the trace: its frame, the pattern's row after it,
    /// and the `zframe` value there. `None` when no row ever advanced.
    fn first_row_change(&self) -> Option<(u64, u8, i16)> {
        self.samples
            .windows(2)
            .find(|pair| pair[1].row != pair[0].row)
            .map(|pair| (pair[1].frame, pair[1].row, pair[1].zframe))
    }

    /// The runs of `_np_zframe` between its resets, as
    /// `(first frame, last frame, first value, last value)`.
    ///
    /// The word is written by two hands — the player counts ticks into it and
    /// `_dis_setmframe` (`DIS/DISC.ASM:138-148`) puts a value in it — and it is
    /// reset when the song is restarted, so a rate measured end to end over a
    /// trace with a restart in it says nothing about the tick rate.
    pub fn zframe_runs(&self) -> Vec<(u64, u64, i16, i16)> {
        let mut runs: Vec<(u64, u64, i16, i16)> = Vec::new();
        for state in &self.samples {
            // The leading zeros, before the music starts, are not a run: the
            // first run begins where the word first moves.
            if runs.is_empty() && state.zframe == 0 {
                continue;
            }
            match runs.last_mut() {
                Some(run) if state.zframe >= run.3 => {
                    run.1 = state.frame;
                    run.3 = state.zframe;
                }
                Some(_) | None => {
                    runs.push((state.frame, state.frame, state.zframe, state.zframe))
                }
            }
        }
        runs.retain(|run| run.1 > run.0);
        runs
    }

    /// The `zplus` rule ([`zplus_rule`]) re-derived over this trace's own order
    /// sequence, with every sample compared against it.
    ///
    /// The sequence of orders the counter held is built from the samples
    /// themselves, and each order's expected value is computed when the counter
    /// first holds it — from the transition that entered it. That is what makes
    /// "*a seek or a restart credits no marker behind it*" fall out of the
    /// trace rather than out of this function having to know which transition
    /// was a restart: a crossing is only a crossing when the counter came from
    /// the order before the skipped entry.
    pub fn zplus_check(&self) -> Option<ZplusCheck> {
        let module = self.module.as_ref()?;
        let mut held: Vec<u8> = Vec::new();
        for state in &self.samples {
            if held.last() != Some(&state.ord) {
                held.push(state.ord);
            }
        }
        let expected = zplus_rule(&module.orders, &held);
        let value_of = |order: u8| -> i16 {
            held.iter()
                .position(|held| *held == order)
                .map(|at| expected[at])
                .unwrap_or(0)
        };
        let mut check = ZplusCheck {
            samples: self.samples.len(),
            mismatches: 0,
            examples: Vec::new(),
            held: held.clone(),
        };
        for state in &self.samples {
            let want = value_of(state.ord);
            if state.zplus != want {
                check.mismatches += 1;
                if check.examples.len() < 6 {
                    check.examples
                        .push((state.frame, state.ord, state.row, state.zplus, want));
                }
            }
        }
        Some(check)
    }

    /// The frames at which the order advanced, in order.
    fn order_changes(&self) -> Vec<(u64, u8, u8)> {
        self.samples
            .windows(2)
            .filter(|pair| pair[1].ord != pair[0].ord)
            .map(|pair| (pair[1].frame, pair[1].ord, pair[1].row))
            .collect()
    }

    fn print(&self, at: Option<u32>) -> Result<(), MusicError> {
        println!("trace: {}", self.dir.display());
        println!(
            "  {} (recomputed from the trace file's own frames)",
            self.coverage.describe()
        );
        println!(
            "  oracle {}, boot {:?}, cycles {}, keys {:?}",
            self.manifest.oracle,
            self.manifest.boot,
            self.manifest.cycles,
            self.manifest.keys.join(" ")
        );
        println!(
            "  segment base 0x{:06X} (found at frame {}, still there at frame {})",
            self.manifest.locate.base,
            self.manifest.locate.frame,
            self.manifest.locate.rerun_frame
        );
        self.print_start();
        self.print_rate()?;
        self.print_quantisation();
        self.print_machine();
        self.print_zplus_zinfo();
        self.print_order_list()?;
        self.print_cross_check(at)?;
        Ok(())
    }

    /// 1. Where the music starts.
    fn print_start(&self) {
        println!("\n1. where the music starts");
        let advance = self.manifest.check.first_advance;
        match advance {
            None => println!("  the player never advanced in this trace"),
            Some(frame) => {
                let at = self
                    .samples
                    .iter()
                    .position(|state| state.frame >= frame)
                    .expect("the advancing frame is a sample");
                println!("  first advancing frame: {frame}");
                let from = at.saturating_sub(2);
                let to = (at + 3).min(self.samples.len());
                for state in &self.samples[from..to] {
                    let earlier = self
                        .samples
                        .iter()
                        .find(|other| other.frame + 1 == state.frame);
                    let moved = earlier
                        .map(|earlier| {
                            let mut moved = Vec::new();
                            if earlier.row != state.row {
                                moved.push(format!("row {} -> {}", earlier.row, state.row));
                            }
                            if earlier.ord != state.ord {
                                moved.push(format!("ord {} -> {}", earlier.ord, state.ord));
                            }
                            if earlier.pat != state.pat {
                                moved.push(format!("pat {} -> {}", earlier.pat, state.pat));
                            }
                            if earlier.loop_ != state.loop_ {
                                moved.push(format!("loop {} -> {}", earlier.loop_, state.loop_));
                            }
                            if earlier.zframe != state.zframe {
                                moved.push(format!(
                                    "zframe {} -> {}",
                                    earlier.zframe, state.zframe
                                ));
                            }
                            if moved.is_empty() {
                                String::new()
                            } else {
                                format!("   <- {}", moved.join(", "))
                            }
                        })
                        .unwrap_or_default();
                    println!(
                        "    frame {:5}  ord {:3} row {:3} pat {:3}  zframe {:6}{moved}",
                        state.frame, state.ord, state.row, state.pat, state.zframe
                    );
                }
            }
        }
    }

    /// 2. The tick rate, and the rule behind it.
    ///
    /// The rate is a **prediction** of the trace, not a fit to it: it comes from
    /// the module's own header (speed and tempo, with the shell's patch,
    /// `MAIN/STARTMUS.C:257`) through the original's mixing rule — a tick is
    /// `2.5/tempo` seconds, a row one `speed` ticks — and the trace is checked
    /// against it, per run of constant speed and tempo, because the module
    /// changes both as it plays.
    fn print_rate(&self) -> Result<(), MusicError> {
        println!("\n2. the tick rate");
        let Some(first) = self.samples.first() else {
            println!("  no samples");
            return Ok(());
        };
        let last = self.samples.last().expect("checked");
        println!(
            "  trace covers frames {}..{}: {} frames, {:.3} s at {FRAME_RATE_HZ} Hz",
            first.frame,
            last.frame,
            last.frame - first.frame,
            (last.frame - first.frame) as f64 / FRAME_RATE_HZ
        );
        match &self.module {
            Some(module) => println!(
                "  module {}: file speed {} tempo {}{}, {} order(s), {} pattern(s) \n  a tick is \
                 then 2.5/{} s = {:.4} s",
                module.path,
                module.speed,
                module.tempo,
                match module.patched_tempo {
                    Some(tempo) => format!(" (the demo patches it to {tempo})"),
                    None => String::new(),
                },
                module.orders.len(),
                module.patterns,
                module.playing_tempo(),
                2.5 / f64::from(module.playing_tempo()),
            ),
            None => println!(
                "  no module's own speed matches the trace's (speeds seen {:?})",
                self.manifest.check.speeds
            ),
        }
        let runs = self.rate_runs();
        if runs.is_empty() {
            println!("  the player never advanced a row in this trace");
            return Ok(());
        }
        println!(
            "  the rate is measured over each run of constant speed and tempo (the module's own \
             Axx/Txx cells change both):"
        );
        let mut total_frames = 0i64;
        let mut total_predicted = 0.0f64;
        let mut total_rows = 0usize;
        let longest = runs
            .iter()
            .max_by_key(|run| run.frames())
            .expect("checked not empty");
        for run in &runs {
            println!(
                "    frames {}..{} at speed {} tempo {}: {} row interval(s) in {} frame(s) — \
                 predicted {:.1} frame(s), residual {:+.1}, one row is {:.2} frame(s)",
                run.first_change.expect("a run with rows has a change"),
                run.last_change.expect("a run with rows has a change"),
                run.speed,
                run.tempo,
                run.intervals(),
                run.frames(),
                run.predicted_frames(),
                run.residual(),
                run.row_frames()
            );
            total_frames += run.frames() as i64;
            total_predicted += run.predicted_frames();
            total_rows += run.intervals();
        }
        println!(
            "  over all runs: {total_rows} row interval(s) in {total_frames} frame(s), predicted \
             {total_predicted:.1} — residual {:+.1} frame(s), {:.2}% of the measured frames",
            total_frames as f64 - total_predicted,
            100.0 * (total_frames as f64 - total_predicted) / total_frames as f64
        );
        println!(
            "  longest run: frames {}..{} at speed {} tempo {}, residual {:+.1} frame(s) over \
             {:.1} predicted — the prediction is the module's own header, not a fit",
            longest.first_change.expect("rows"),
            longest.last_change.expect("rows"),
            longest.speed,
            longest.tempo,
            longest.residual(),
            longest.predicted_frames(),
        );
        println!(
            "  the player's own words: speed {:?}, tempo {:?} (both straight out of the trace)",
            self.manifest.check.speeds, self.manifest.check.tempos
        );
        // `_np_zframe`'s own rate over its longest run without a reset, which
        // the anchor's derivation needs as well as the print below.
        let runs = self.zframe_runs();
        let longest_zframe_run = runs
            .iter()
            .map(|run| (run.1 - run.0, i64::from(run.3) - i64::from(run.2)))
            .filter(|(frames, ticks)| *frames > 0 && *ticks > 0)
            .max_by_key(|(frames, _)| *frames);

        // The anchor the *rows* imply, from two measured numbers and no
        // simulation: the first row change and the trace's own `zframe` value
        // there, at the tick clock's measured frames a tick. A row is `speed`
        // ticks, so the row change is seen on a frame that is within one tick of
        // the boundary — which is the resolution of both readings.
        if let (Some(first_change), Some(longest)) = (self.first_row_change(), longest_zframe_run)
        {
            let frames_per_tick = longest.0 as f64 / longest.1 as f64;
            let ticks = f64::from(first_change.2);
            let implied = first_change.0 as f64 - ticks * frames_per_tick;
            let direct = self
                .samples
                .iter()
                .find(|state| state.zframe != 0)
                .map(|state| state.frame);
            println!(
                "  the anchor the rows imply: the first row change is at frame {} with \\
                 `_np_zframe` {}, and {} ticks at the measured {frames_per_tick:.4} frames a tick \\
                 is {:.1} frames — so that row began at frame {:.1} (± the tick the row \
                 boundary is within and the frame the change is seen on)",
                first_change.0,
                first_change.2,
                first_change.2,
                ticks * frames_per_tick,
                implied
            );
            if let Some(direct) = direct {
                let apart = (direct as f64 - implied).abs();
                println!(
                    "  the anchor the `zframe` column shows: frame {direct} is the first frame \
                     whose `zframe` is not 0 — {apart:.1} frame(s), or {:.1} tick(s), from the \
                     rows' reading: the same tick, seen at the next whole-frame sample. Both \
                     readings are quantised to a whole frame, so the gap is one frame plus the \
                     tick the row change is seen within; it is not a claim that they agree to \
                     within a tick, and the two numbers above say how big it is.",
                    apart / frames_per_tick
                );
            }
        }

        // `_np_zframe` is the word the parts wait on (`DIS/DISINT.ASM:412-425`):
        // it counts ticks, so its own rate is the tick rate said a second way.
        // It is **reset** — `_dis_setmframe(0)` is one writer and a song restart
        // is another — so the rate is measured over the longest run of samples
        // in which it never went backwards, and the resets are counted rather
        // than averaged across.
        if !runs.is_empty() {
            let resets: usize = runs.len() - 1;
            let longest = runs
                .iter()
                .max_by_key(|run| run.1 - run.0)
                .expect("checked not empty");
            let (first_frame, last_frame, first, last) = *longest;
            let ticks = i64::from(last) - i64::from(first);
            let frames = last_frame - first_frame;
            if frames > 0 {
                println!(
                    "  _np_zframe: {} at frame {} to {} at frame {} — {ticks} tick(s) in {frames} \
                     frames = {:.4} per frame = {:.2} per second, over the longest run without a \
                     reset ({resets} reset(s) in the trace)",
                    first,
                    first_frame,
                    last,
                    last_frame,
                    ticks as f64 / frames as f64,
                    ticks as f64 / frames as f64 * FRAME_RATE_HZ
                );
            }
        }
        Ok(())
    }

    /// 3. The quantisation: which frames the rows land on.
    ///
    /// The sequence of row-change frames is what Task 3's player has to
    /// reproduce tick for tick, so the gaps are reported per run of constant
    /// speed and tempo — the two regimes have different tick rates and a single
    /// histogram over both would describe neither — and the run's own
    /// quantisation (how many distinct gap sizes) is the number that says
    /// whether the row rate is periodic on the frame clock.
    fn print_quantisation(&self) {
        println!("\n3. the quantisation");
        let changes = self.row_changes();
        if changes.len() < 2 {
            println!("  fewer than two row changes: nothing to measure");
            return;
        }
        println!(
            "  {} row change(s) in the trace; the frames themselves, first 32: {:?}",
            changes.len(),
            &changes[..changes.len().min(32)]
        );
        for run in self.rate_runs() {
            let (Some(first), Some(last)) = (run.first_change, run.last_change) else {
                continue;
            };
            let inside: Vec<u64> = changes
                .iter()
                .copied()
                .filter(|frame| first <= *frame && *frame <= last)
                .collect();
            if inside.len() < 2 {
                continue;
            }
            let mut gaps: BTreeMap<u64, usize> = BTreeMap::new();
            for pair in inside.windows(2) {
                *gaps.entry(pair[1] - pair[0]).or_insert(0) += 1;
            }
            let histogram: Vec<String> = gaps
                .iter()
                .map(|(gap, count)| format!("{gap} frame(s) x{count}"))
                .collect();
            println!(
                "  frames {}..{} at speed {} tempo {}: {} gap(s) — {}",
                first,
                last,
                run.speed,
                run.tempo,
                inside.len() - 1,
                histogram.join(", ")
            );
            println!(
                "    distinct gap sizes: {} (one row is {:.1} frame(s) at this speed/tempo)",
                gaps.len(),
                run.row_frames()
            );
            // The gaps outside the two dominant sizes, **where they are**: a
            // reader (and Task 3) needs to know that they are scattered through
            // the run rather than sitting at its edges, because "only the ends
            // are special" is exactly the lesson this data does not teach.
            // The gaps that are neither the commonest size nor one less than
            // it — the two sizes a rule that lands a whole number of frames per
            // row would produce — with **where they are** in the run. A reader
            // (and Task 3) needs that: a gap the rule does not predict sitting
            // at a run's edge is a boundary, and the same gap in the middle is a
            // fact about the player that is not explained here.
            // The two sizes the run's own row length brackets — `floor` and
            // `ceil` of `row_frames()` — are what a row that lasts a whole
            // number of frames must produce. Everything else is a gap the
            // prediction does not make, and its *place* in the run is what tells
            // a boundary apart from a fact about the player.
            let exact = run.row_frames();
            let (below, above) = (exact.floor() as u64, exact.ceil() as u64);
            let outliers: Vec<String> = inside
                .windows(2)
                .enumerate()
                .filter(|(_, pair)| {
                    let gap = pair[1] - pair[0];
                    gap < below || gap > above
                })
                .map(|(at, pair)| {
                    format!("{} of {} ({} -> {}, {})", at + 1, inside.len() - 1, pair[0], pair[1], pair[1] - pair[0])
                })
                .collect();
            if !outliers.is_empty() {
                let shown: Vec<&String> = outliers.iter().take(8).collect();
                println!(
                    "    the {} gap(s) outside the {below}..{above} the run's own row length \
                     ({exact:.2} frames) brackets, by their position in the run: {}{}",
                    outliers.len(),
                    shown
                        .iter()
                        .map(|entry| entry.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                    if outliers.len() > shown.len() {
                        format!(", and {} more", outliers.len() - shown.len())
                    } else {
                        String::new()
                    }
                );
            }
        }
    }

    /// 4. The machine's card and mixer words.
    fn print_machine(&self) {
        println!("\n4. the soundcard and mixer, from the guest");
        for (name, value) in &self.manifest.machine {
            let meaning = match (name.as_str(), *value) {
                ("_zoutputmode", 0) => " (no sound)",
                ("_zoutputmode", 1) => " (Sound Blaster)",
                ("_zoutputmode", 2) => " (Gravis Ultrasound)",
                ("_zoutputmode", 7) => " (no sound)",
                ("_zpollmix", 0) => " (the player mixes from the 70 Hz timer interrupt)",
                ("_zpollmix", 1) => " (the caller polls the player: `_zpollme` once a frame)",
                ("_znoborders", 1) => " (no border colours)",
                _ => "",
            };
            println!("  {name:<16} {}{meaning}", Self::show_word(*value));
        }
        for (name, value) in &self.manifest.machine_moved {
            println!(
                "  {name:<16} moved during the trace: {} at the locate, {} at the end — a word \
                 whose two readings differ is recorded as both, not merged",
                Self::show_word(self.manifest.machine.get(name).copied().unwrap_or(0)),
                Self::show_word(*value)
            );
        }
        println!(
            "  addresses: {}",
            self.manifest
                .locate
                .words
                .iter()
                .map(|(name, address)| format!("{name} 0x{address:06X}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }

    /// 5. `zplus` and `zinfo`.
    fn print_zplus_zinfo(&self) {
        println!("\n5. zplus and zinfo");
        let mut zplus_values: Vec<i16> = Vec::new();
        let mut zinfo_values: Vec<i16> = Vec::new();
        for state in &self.samples {
            if !zplus_values.contains(&state.zplus) {
                zplus_values.push(state.zplus);
            }
            if !zinfo_values.contains(&state.zinfo) {
                zinfo_values.push(state.zinfo);
            }
        }
        println!("  zplus values: {zplus_values:?}");
        println!("  zinfo values: {zinfo_values:?}");

        // Is zplus a function of (ord, row)? Every frame sharing a position must
        // share the value, or it is a function of something else.
        let mut by_position: BTreeMap<(u8, u8), Vec<i16>> = BTreeMap::new();
        for state in &self.samples {
            let values = by_position.entry((state.ord, state.row)).or_default();
            if !values.contains(&state.zplus) {
                values.push(state.zplus);
            }
        }
        let conflicting: Vec<_> = by_position
            .iter()
            .filter(|(_, values)| values.len() > 1)
            .collect();
        println!(
            "  zplus against (ord, row): {} distinct position(s), {} of them carrying more than \
             one zplus value",
            by_position.len(),
            conflicting.len()
        );
        for ((ord, row), values) in conflicting.iter().take(8) {
            println!("    ord {ord} row {row}: zplus {values:?}");
        }
        // The transitions: the rows at which zplus changes within one order.
        let transitions: Vec<(u8, u8, i16, i16)> = self
            .samples
            .windows(2)
            .filter(|pair| pair[1].zplus != pair[0].zplus)
            .map(|pair| {
                (
                    pair[1].ord,
                    pair[1].row,
                    pair[0].zplus,
                    pair[1].zplus,
                )
            })
            .collect();
        println!(
            "  zplus transitions: {} (first 12: {:?})",
            transitions.len(),
            &transitions[..transitions.len().min(12)]
        );

        // The port's rule, re-derived from this trace's own order sequence and
        // checked against every sample of it: a rule that says what the trace
        // does, with the count of the samples it had to be right about.
        match self.zplus_check() {
            Some(check) => {
                println!(
                    "  the port's zplus rule, re-derived from this trace's own order sequence: {} \
                     sample(s) checked, {} mismatch(es)",
                    check.samples, check.mismatches
                );
                println!(
                    "    rule: 1 while playing an order whose next entry in the module's order \
                     list is 254; 2 on a crossing (the counter moved from o to o+2 with \
                     orders[o+1] == 254) unless the entry after the one entered is itself 254, in \
                     which case 1; 0 otherwise; a seek or a restart credits no marker behind it"
                );
                println!("    the orders the counter held, in order: {:?}", check.held);
                for (frame, order, row, measured, expected) in &check.examples {
                    println!(
                        "    frame {frame}: ord {order} row {row} reads {measured}, the rule says \
                         {expected}"
                    );
                }
                if check.mismatches == 0 {
                    println!(
                        "    (what the traces cannot show: none of them holds an order two steps \
                         past a marker, so \"2 goes back to 0 on the next natural advance\" is \
                         extrapolation — and so is anything about a 255)"
                    );
                }
            }
            None => println!(
                "  no module matched the trace's order sequence, so the zplus rule cannot be \
                 re-derived from it"
            ),
        }

        // zinfo: the frames each value covers, as runs.
        let mut runs: Vec<(i16, u64, u64)> = Vec::new();
        for state in &self.samples {
            match runs.last_mut() {
                Some(run) if run.0 == state.zinfo => run.2 = state.frame,
                _ => runs.push((state.zinfo, state.frame, state.frame)),
            }
        }
        println!("  zinfo runs: {}", runs.len());
        for (value, first, last) in runs.iter().take(12) {
            let at = self.samples.iter().find(|state| state.frame == *last);
            println!(
                "    {value:6}  frames {first}..{last} ({}){}",
                last - first + 1,
                at.map(|state| format!(", ending at ord {} row {}", state.ord, state.row))
                    .unwrap_or_default()
            );
        }
        if runs.len() > 12 {
            println!("    … and {} more run(s)", runs.len() - 12);
        }
        // The correlation that matters: does zinfo move with the position at
        // all, and does it move with zplus?
        let with_position: Vec<_> = self
            .samples
            .windows(2)
            .filter(|pair| pair[1].zinfo != pair[0].zinfo)
            .collect();
        let same_row: usize = with_position
            .iter()
            .filter(|pair| pair[1].row == pair[0].row)
            .count();
        println!(
            "  zinfo changes on {} frame(s); on {same_row} of them the row did not change, and on \
             {} the row changed too",
            with_position.len(),
            with_position.len() - same_row
        );
        if self.manifest.check.z_info_is_constant() {
            println!(
                "  zinfo holds {} for the whole trace: this capture does NOT measure what its \
                 other values are — the record says so rather than reading a rule off the \
                 modules' cells (D8)",
                zinfo_values.first().copied().unwrap_or(0)
            );
        }
        let zplus_and_zinfo: usize = self
            .samples
            .windows(2)
            .filter(|pair| pair[1].zinfo != pair[0].zinfo && pair[1].zplus != pair[0].zplus)
            .count();
        println!("  zinfo and zplus change together on {zplus_and_zinfo} frame(s)");
    }

    /// 6. What the player does at the order list's `254` and `255` entries.
    ///
    /// Read off the order counter's own sequence and nothing else: for every
    /// such entry the trace says whether the counter ever held that value, what
    /// it did instead, and — when the entry was never reached — says so rather
    /// than reading the rule off stock ST3 semantics.
    fn print_order_list(&self) -> Result<(), MusicError> {
        println!("\n6. the order list, and the 254/255 entries");
        let changes = self.order_changes();
        let held: BTreeSet<u8> = self.samples.iter().map(|state| state.ord).collect();
        println!(
            "  the order counter held {} distinct value(s); its changes ({}):",
            held.len(),
            changes.len()
        );
        let shown: Vec<String> = changes
            .iter()
            .take(24)
            .map(|(frame, ord, row)| format!("{frame}: ord {ord} row {row}"))
            .collect();
        println!("    {}", shown.join(" | "));
        if changes.len() > 24 {
            println!("    … and {} more", changes.len() - 24);
        }
        match &self.module {
            Some(module) => {
                let entries: Vec<(usize, u8)> = module
                    .orders
                    .iter()
                    .enumerate()
                    .filter(|(_, entry)| **entry == 254 || **entry == 255)
                    .map(|(index, entry)| (index, *entry))
                    .collect();
                println!(
                    "  {}'s order list: {} order(s), {} 254/255 entry/entries at {}",
                    module.path,
                    module.orders.len(),
                    entries.len(),
                    entries
                        .iter()
                        .map(|(index, entry)| format!("{index}={entry}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                );
                for (index, entry) in &entries {
                    let value = *index as u8;
                    match self.samples.iter().find(|state| state.ord == value) {
                        Some(state) => {
                            // The counter rested on the entry: what came next?
                            let after = self
                                .samples
                                .iter()
                                .find(|later| later.frame > state.frame && later.ord != value);
                            println!(
                                "    order {index} ({entry}): the counter held it at frame {} \
                                 (row {}) and then went to {}",
                                state.frame,
                                state.row,
                                after
                                    .map(|later| format!("ord {} row {}", later.ord, later.row))
                                    .unwrap_or_else(|| "the end of the trace".into())
                            );
                        }
                        None => {
                            let before = held.range(..value).next_back();
                            let after = held.range(value + 1..).next();
                            match (before, after) {
                                // The counter's neighbours either side are the
                                // entry's own neighbours in the list: the player
                                // *played past* it, which is a rule.
                                (Some(before), Some(after))
                                    if *before + 1 == value && value + 1 == *after =>
                                {
                                    println!(
                                        "    order {index} ({entry}): never held — the counter \
                                         went from ord {before} to ord {after} without resting on \
                                         {value}, having played each of them, so the player steps \
                                         over this entry"
                                    )
                                }
                                (Some(before), None) => println!(
                                    "    order {index} ({entry}): never reached — the counter \
                                     stopped at ord {before} in this trace, so what the player \
                                     does at this entry is NOT measured by it"
                                ),
                                // Something jumped: the song was restarted
                                // somewhere else, and this entry was never
                                // played, so the trace does not measure it.
                                _ => println!(
                                    "    order {index} ({entry}): never held — the counter went \
                                     from {} to {}, a jump rather than a step, so this entry was \
                                     not played in this trace and what the player does at it is \
                                     NOT measured by it",
                                    before
                                        .map(|ord| format!("ord {ord}"))
                                        .unwrap_or_else(|| "nothing".into()),
                                    after
                                        .map(|ord| format!("ord {ord}"))
                                        .unwrap_or_else(|| "nothing".into())
                                ),
                            }
                        }
                    }
                }
            }
            None => println!("  no module matched the trace's speed, so its orders cannot be read"),
        }
        Ok(())
    }

    /// The cross-check the phase owes `reference-indices.md` §1.7: a 16-bit
    /// word at the physical address `0x105B0E` advanced 0.7424 times a frame
    /// there and read 371/373/374 at video frame 900. The located base turns
    /// that into a claim about a named word — or says which word it is, when it
    /// is not the one the phase's own trace reads.
    ///
    /// Every **view** of the segment is tried, not only the one the trace reads
    /// through: the demo's DOS extender maps the same memory at more than one
    /// linear address, and the phase's address was measured in the higher one.
    fn print_cross_check(&self, at: Option<u32>) -> Result<(), MusicError> {
        let Some(address) = at else {
            return Ok(());
        };
        println!("\n7. the address 0x{address:06X} (reference-indices.md §1.7)");
        let object = std::fs::read(&self.manifest.object.object).map_err(|e| MusicError::Io {
            path: PathBuf::from(&self.manifest.object.object),
            message: e.to_string(),
        })?;
        let walk = symbols::walk(&object)?;
        let length = self.manifest.object.segment_length;
        let mut inside = Vec::new();
        for view in self
            .manifest
            .locate
            .views
            .iter()
            .chain(self.manifest.locate.rerun_views.iter())
        {
            if address >= *view && address < *view + length {
                inside.push(*view);
            }
        }
        inside.sort_unstable();
        inside.dedup();
        if inside.is_empty() {
            println!(
                "  the segment's views are {} and this address is in none of them \
                 ({length} byte(s) each), so it is not inside this segment at all",
                self.manifest
                    .locate
                    .views
                    .iter()
                    .map(|view| format!("0x{view:06X}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
            return Ok(());
        }
        let mut sorted: Vec<&Symbol> = walk.symbols.iter().collect();
        sorted.sort_by_key(|symbol| symbol.offset);
        for view in &inside {
            let offset = address - view;
            println!(
                "  in the view at 0x{view:06X} it is that base + 0x{offset:04X} \
                 (the trace reads through 0x{:06X})",
                self.manifest.locate.base
            );
            let below = sorted.iter().rfind(|symbol| symbol.offset <= offset);
            let above = sorted.iter().find(|symbol| symbol.offset > offset);
            match below {
                Some(symbol) => println!(
                    "    the last published symbol at or below it is {} (base + 0x{:04X}, {} \
                     byte(s) below){}",
                    symbol.name,
                    symbol.offset,
                    offset - symbol.offset,
                    above
                        .map(|next| format!(
                            "; the next published symbol above is {} at base + 0x{:04X}",
                            next.name, next.offset
                        ))
                        .unwrap_or_default()
                ),
                None => println!("    no published symbol of the object is at or below it"),
            }
            match self
                .manifest
                .locate
                .words
                .iter()
                .find(|(_, word)| **word == address)
            {
                Some((name, _)) => {
                    println!("    it is exactly {} — one of the traced words", name);
                    let values: Vec<(u64, i16)> = match name.as_str() {
                        "_np_zframe" => self
                            .samples
                            .iter()
                            .map(|state| (state.frame, state.zframe))
                            .collect(),
                        "_np_zplus" => self
                            .samples
                            .iter()
                            .map(|state| (state.frame, state.zplus))
                            .collect(),
                        "_np_zinfo" => self
                            .samples
                            .iter()
                            .map(|state| (state.frame, state.zinfo))
                            .collect(),
                        _ => Vec::new(),
                    };
                    if let Some((first, last)) = values.first().zip(values.last()) {
                        let rate =
                            (f64::from(last.1) - f64::from(first.1)) / (last.0 - first.0) as f64;
                        println!(
                            "    measured here: {} at frame {}, {} at frame {} — {rate:+.4} per \
                             frame (Phase 1 measured {PHASE1_CROSS_CHECK_RATE:+.4})",
                            first.1, first.0, last.1, last.0
                        );
                        if let Some((frame, value)) = values
                            .iter()
                            .find(|(frame, _)| *frame == PHASE1_CROSS_CHECK_FRAME)
                        {
                            println!(
                                "    at frame {frame} it reads {value} (Phase 1 measured \
                                 371/373/374 there)"
                            );
                        }
                    }
                }
                None => println!(
                    "    it is not one of the traced words, so this trace does not carry its \
                     values"
                ),
            }
        }
        Ok(())
    }
}

/// Read an S3M's header, as much of it as the tick rate needs: the order list,
/// the pattern count and the speed and tempo bytes (`0x31`, `0x32`).
/// The frames of a sample list, for a coverage recomputed from them.
fn frames_in(samples: &[PlayerState]) -> Vec<u64> {
    samples.iter().map(|state| state.frame).collect()
}

fn read_module(path: &str, patched_tempo: Option<u8>) -> Result<Module, MusicError> {
    let bytes = std::fs::read(path).map_err(|e| MusicError::Io {
        path: PathBuf::from(path),
        message: e.to_string(),
    })?;
    if bytes.len() < 0x60 || &bytes[0x2C..0x30] != b"SCRM" {
        return Err(MusicError::Trace(format!("{path} is not an S3M")));
    }
    let orders = usize::from(u16::from_le_bytes([bytes[0x20], bytes[0x21]]));
    let patterns = u16::from_le_bytes([bytes[0x24], bytes[0x25]]);
    let order_list = bytes
        .get(0x60..0x60 + orders)
        .ok_or_else(|| MusicError::Trace(format!("{path} is shorter than its order list")))?
        .to_vec();
    Ok(Module {
        path: path.to_string(),
        orders: order_list,
        patterns,
        speed: bytes[0x31],
        tempo: bytes[0x32],
        patched_tempo,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn object() -> Vec<u8> {
        std::fs::read(Path::new(env!("CARGO_MANIFEST_DIR")).join("../").join(DEFAULT_OBJECT))
            .expect("the vendored STMIK object")
    }

    // ---------------------------------------------------------------------------------
    // The window
    // ---------------------------------------------------------------------------------

    /// `A..B` is the Rust range — A up to but not including B — and `A..=B`
    /// includes B. A command whose last frame is silently included or excluded
    /// is one whose coverage nobody can predict.
    #[test]
    fn the_frame_window_is_half_open_and_checked_for_holes() {
        assert_eq!(parse_window("0..1300").unwrap(), [0, 1299]);
        assert_eq!(parse_window("0..=1299").unwrap(), [0, 1299]);
        assert_eq!(parse_window("5").unwrap(), [5, 5]);
        assert_eq!(parse_window("1,2,3").unwrap(), [1, 3]);
        assert_eq!(parse_window(" 0..5 , 5..9 ").unwrap(), [0, 8]);
        for bad in ["", "10..10", "10..0", "0,2", "0..5,9..12", "a..b", "0..=0..=3"] {
            assert!(
                parse_window(bad).is_err(),
                "--frames {bad:?} must be refused"
            );
        }
    }

    // ---------------------------------------------------------------------------------
    // The locator
    // ---------------------------------------------------------------------------------

    /// A memory that answers in 64 KiB chunks, as the oracle's does, with the
    /// needle planted where the test says.
    fn memory(size: usize, placed: &[(u32, Vec<u8>)]) -> impl FnMut(u16, u32, usize) -> Vec<u8> {
        let mut bytes = vec![0u8; size];
        for (at, data) in placed {
            bytes[*at as usize..*at as usize + data.len()].copy_from_slice(data);
        }
        move |segment: u16, offset: u32, len: usize| -> Vec<u8> {
            let at = usize::from(segment) * 16 + offset as usize;
            if at >= bytes.len() {
                // Past the end of the emulated memory: the API refuses, and the
                // scan treats it as the end.
                return Vec::new();
            }
            bytes[at..(at + len).min(bytes.len())].to_vec()
        }
    }

    fn needle(seed: u8) -> Vec<u8> {
        (0..NEEDLE_LEN).map(|i| seed.wrapping_add(i as u8)).collect()
    }

    /// The base is solved from where the needle is, and the pair returned is
    /// one the same reader accepts — that is what every later sample reads
    /// through.
    #[test]
    fn a_unique_needle_solves_the_base() {
        let bits = needle(7);
        let mut read = memory(0x30000, &[(0x2_1F00, bits.clone())]);
        let (segment, base) = locate(0x1234, &bits, &mut read).unwrap();
        assert_eq!(base, 0x2_1F00 - 0x1234);
        // The returned pair addresses the base itself.
        let bytes = read(segment, base, NEEDLE_LEN);
        assert_eq!(bytes[..NEEDLE_LEN - 0x1234.min(NEEDLE_LEN)], bits[..NEEDLE_LEN - 0x1234.min(NEEDLE_LEN)]);
    }

    /// Two matches mean no base is *the* base, and the refusal names every base
    /// the needle's own offset solves.
    #[test]
    fn two_matches_are_refused_with_both_bases() {
        let bits = needle(9);
        let mut read = memory(0x200000, &[(0x51F0, bits.clone()), (0x10_51F0, bits.clone())]);
        match locate(0x982, &bits, &mut read) {
            Err(LocateError::Ambiguous { bases, found }) => {
                assert_eq!(bases, vec![0x51F0 - 0x982, 0x10_51F0 - 0x982]);
                assert_eq!(found, 2);
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// A needle that straddles two 64 KiB requests is still found: the scan
    /// searches each request together with the tail of the one before it.
    #[test]
    fn a_needle_across_the_chunk_boundary_is_found() {
        let bits = needle(3);
        let at = SCAN_CHUNK as u32 - 5;
        let mut read = memory(0x20000, &[(at, bits.clone())]);
        let (_, base) = locate(0, &bits, &mut read).unwrap();
        assert_eq!(base, at);
    }

    /// A needle that is nowhere is a refusal, not a base.
    #[test]
    fn a_needle_that_is_nowhere_is_refused() {
        let mut read = memory(0x20000, &[]);
        assert!(matches!(
            locate(0, &needle(1), &mut read),
            Err(LocateError::NotFound { .. })
        ));
    }

    /// A needle shorter than the evidence rule is refused: 8 bytes is a
    /// coincidence waiting to happen.
    #[test]
    fn a_short_needle_is_not_evidence() {
        let mut read = memory(0x1000, &[(0, vec![1, 2, 3, 4, 5, 6, 7, 8])]);
        match locate(0, &[1, 2, 3, 4, 5, 6, 7, 8], &mut read) {
            Err(LocateError::BadNeedle(message)) => assert!(message.contains("8-byte"), "{message}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// A scan whose last request comes back shorter than the needle **ends**:
    /// nothing above the emulated memory will answer, and a chunk shorter than
    /// the needle has no tail to carry. It used to underflow and panic — a
    /// verifier that panics on a well-formed guest is worse than one that
    /// refuses.
    #[test]
    fn a_short_last_chunk_ends_the_scan_without_panicking() {
        // Eight bytes past a full chunk: the second request is short, and
        // shorter than the 32-byte needle.
        let mut read = memory(SCAN_CHUNK + 8, &[]);
        assert!(matches!(
            locate(0, &needle(4), &mut read),
            Err(LocateError::NotFound { .. })
        ));
        // And a needle that lives entirely inside the last short chunk is still
        // found when it fits.
        let short = needle(5);
        let at = 8_000u32;
        let mut read = memory(at as usize + short.len(), &[(at, short.clone())]);
        assert_eq!(locate(0, &short, &mut read).unwrap().1, at);
    }

    /// The needles come from the object's own `LEDATA` data, are 32 bytes, come
    /// from different records, and each occurs exactly once in the object — a
    /// run that is in the object twice would match the guest twice for reasons
    /// that have nothing to do with the segment.
    #[test]
    fn the_objects_needles_are_unique_runs_from_different_records() {
        let object = object();
        let needles = needles(&object).unwrap();
        assert_eq!(needles[0].len, NEEDLE_LEN);
        assert_ne!(needles[0].block, needles[1].block, "different LEDATA records");
        assert_ne!(needles[0].offset, needles[1].offset);
        for needle in &needles {
            let bytes = needle_bytes(&object, needle).unwrap();
            assert_eq!(bytes.len(), NEEDLE_LEN);
            assert_eq!(sha256_hex(&bytes), needle.sha256);
            assert!(
                all_distinct(&bytes),
                "no byte repeats inside the run"
            );
            let count = object
                .windows(NEEDLE_LEN)
                .filter(|window| *window == bytes.as_slice())
                .count();
            assert_eq!(count, 1, "the run occurs once in the object");
        }
    }

    // ---------------------------------------------------------------------------------
    // The rows
    // ---------------------------------------------------------------------------------

    /// The CSV is the record: a row survives the round trip, and a row that is
    /// not the columns is refused rather than half-read.
    #[test]
    fn a_row_round_trips_and_a_malformed_row_is_refused() {
        let state = PlayerState {
            frame: 1234,
            ord: 25,
            row: 63,
            pat: 70,
            loop_: 1,
            speed: 6,
            tempo: 120,
            zinfo: 0,
            zplus: -1,
            zframe: 385,
        };
        let line = state.line();
        assert_eq!(PlayerState::parse(&line).unwrap(), state);
        assert_eq!(line, "1234,25,63,70,1,6,120,0,-1,385");
        assert!(PlayerState::parse("1,2,3").is_err());
        assert!(PlayerState::parse("1,2,3,4,5,6,7,8,9,70000").is_err(), "not a 16-bit word");
        assert!(PlayerState::parse("1,300,3,4,5,6,7,8,9,10").is_err(), "not a byte");
        assert!(PlayerState::parse("1,2,3,4,5,6,7,8,9,x").is_err());
    }

    /// The coverage is recomputed from the frames, so a trace that quietly
    /// sampled every other frame cannot describe itself as per-frame.
    #[test]
    fn a_trace_with_a_hole_is_not_per_frame() {
        let every = Coverage::of([10, 19], &(10..=19).collect::<Vec<u64>>());
        assert!(every.per_frame, "{}", every.describe());
        assert!(every.missing.is_empty());

        let holed = Coverage::of([10, 19], &[10, 11, 13, 14, 19]);
        assert!(!holed.per_frame);
        assert_eq!(holed.missing, vec![[12, 12], [15, 18]]);
        assert!(holed.describe().contains("NOT per-frame"), "{}", holed.describe());
        assert_eq!(holed.gaps(), 5);

        // The frames before sampling could begin are missing too, and said so —
        // and the run of them is named on its own, so the first gap explains
        // itself in the manifest rather than only in prose.
        let late = Coverage::of([0, 19], &[10, 11, 12]);
        assert!(!late.per_frame);
        assert_eq!(late.missing, vec![[0, 9], [13, 19]]);
        assert_eq!(late.before_sampling, Some([0, 9]));
        assert!(late.describe().contains("the boundary"), "{}", late.describe());
        // A trace that begins at its window's first frame has no boundary.
        assert_eq!(every.before_sampling, None);
        // And a hole in the middle is not a boundary.
        assert_eq!(holed.before_sampling, None);
    }

    /// Two readings of a machine word that disagree are refused rather than
    /// silently merged: the manifest carries one number per word, and keeping
    /// the first of two is how a record describes a machine that never was.
    #[test]
    fn machine_readings_that_disagree_are_both_recorded() {
        let start = BTreeMap::from([
            ("_zoutputmode".to_string(), 5),
            ("_zmixspeed".to_string(), 20000),
            ("_zbufsize".to_string(), 4096),
        ]);
        let same = BTreeMap::from([
            ("_zoutputmode".to_string(), 5),
            ("_zmixspeed".to_string(), 20000),
            ("_zbufsize".to_string(), 4096),
        ]);
        assert_eq!(
            merge_machine_words(start.clone(), same.clone()).0,
            start
        );
        // A word only the second reading has is added, and the words the two
        // readings agree on keep their value; a word that moved is not merged.
        let grown = BTreeMap::from([
            ("_zoutputmode".to_string(), 5),
            ("_zspeedadjust".to_string(), 0),
        ]);
        let (merged, moved) = merge_machine_words(start.clone(), grown);
        assert_eq!(merged.get("_zoutputmode"), Some(&5));
        assert_eq!(merged.get("_zspeedadjust"), Some(&0));
        assert!(moved.is_empty());
        assert_eq!(merge_machine_words(start.clone(), same), (start.clone(), BTreeMap::new()));

        // A word that moved is recorded **twice**, and a word that did not is
        // recorded once: `_zmixspeed` moves in a real session (20000 → 40000),
        // `_np_mastermul` is 0 before the module is loaded and 2240 (stereo,
        // multiplier 192) once it is, and neither is a word this tool may
        // assume constant.
        let (machine, moved) = merge_machine_words(
            BTreeMap::from([("_zmixspeed".to_string(), 20000)]),
            BTreeMap::from([("_zmixspeed".to_string(), 40000)]),
        );
        assert_eq!(machine.get("_zmixspeed"), Some(&20000));
        assert_eq!(moved.get("_zmixspeed"), Some(&40000));
        let (machine, moved) = merge_machine_words(
            BTreeMap::from([("_np_mastermul".to_string(), 0), ("_np_mastervol".to_string(), 192)]),
            BTreeMap::from([
                ("_np_mastermul".to_string(), 2240),
                ("_np_mastervol".to_string(), 192),
            ]),
        );
        assert_eq!(machine.get("_np_mastermul"), Some(&0), "the reading at the locate");
        assert_eq!(moved.get("_np_mastermul"), Some(&2240), "and the reading at the end");
        assert!(!moved.contains_key("_np_mastervol"), "it did not move");
    }

    /// The `zplus` rule against both modules' order lists: the marker-coming
    /// case, the crossing case, the crossed-into-another-marker case, and the
    /// restart case `SECOND 3` measures.
    #[test]
    fn the_zplus_rule_reproduces_the_traces_own_order_sequences() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let music0 = read_module(
            root.join("assets/MAIN/MUSIC0.S3M").to_str().unwrap(),
            Some(120),
        )
        .unwrap();
        let music1 = read_module(root.join("assets/MAIN/MUSIC1.S3M").to_str().unwrap(), None)
            .unwrap();

        // MUSIC0: order 27 is the 254. Order 26 has it next (1), order 28 is a
        // crossing into it (2) — unless the entry after 28 were itself a 254.
        assert_eq!(zplus_rule(&music0.orders, &[0, 25, 26, 28]), vec![0, 0, 1, 2]);
        // MUSIC1: order 46 is the 254. Same shape, and order 48 has the 254 at
        // 49 next to it, so it reads 1 rather than 0 — the 2 → 1 the trace
        // shows.
        assert_eq!(zplus_rule(&music1.orders, &[42, 43, 44, 45, 47]), vec![0, 0, 0, 1, 2]);
        assert_eq!(zplus_rule(&music1.orders, &[42, 43, 44, 45, 47, 48]), vec![0, 0, 0, 1, 2, 1]);
        // A restart credits no marker behind it: entered at 42, whose list
        // predecessor is a 254 the player never played.
        assert_eq!(zplus_rule(&music1.orders, &[0, 42]), vec![0, 0]);
        // And the marker-coming case holds for a single order with no history.
        assert_eq!(zplus_rule(&music0.orders, &[26]), vec![1]);
    }

    /// A window out of a manifest is data a verifier has to disbelieve, so
    /// `Coverage::of` cannot panic on one: a window whose last frame is before
    /// its first is an empty one, which then disagrees with whatever the
    /// manifest claimed and is reported as a finding.
    #[test]
    fn a_window_that_is_back_to_front_is_not_a_panic() {
        let inverted = Coverage::of([100, 10], &[50, 51]);
        assert_eq!(inverted.requested, 0);
        assert!(!inverted.per_frame);
        assert!(inverted.missing.is_empty());
        assert_eq!(inverted.before_sampling, None);
        // And the frames are still reported as what the file holds.
        assert_eq!(inverted.first, 50);
        assert_eq!(inverted.samples, 2);
    }

    /// An **absurd** window is the same case and must not be a crash either: a
    /// span that does not fit a `u64` has no frame count, so the coverage is
    /// the empty one and the manifest's claim is a finding. (This panicked —
    /// `attempt to add with overflow` — until the arithmetic was made checked;
    /// a manifest is data the verifier exists to disbelieve.)
    #[test]
    fn a_window_whose_span_overflows_is_not_a_panic() {
        // A span that does not fit a `u64`: there is no frame count to give.
        let overflowing = Coverage::of([0, u64::MAX], &[10, 11, 12]);
        assert_eq!(overflowing.requested, 0);
        assert!(overflowing.missing.is_empty());
        let _ = overflowing.describe();
        assert_eq!(overflowing.gaps(), 0);
        // The same window moved one frame in **does** fit (its span is
        // `u64::MAX - 1`), so it is an absurd claim rather than an impossible
        // one — and it must not panic either.
        let absurd = Coverage::of([1, u64::MAX], &[10, 11, 12]);
        assert_eq!(absurd.requested, u64::MAX);
        let _ = absurd.describe();
        assert_eq!(absurd.gaps(), u64::MAX - 3, "two runs: 1..9 and 13..u64::MAX");
        for window in [[u64::MAX, u64::MAX - 1], [u64::MAX, 0], [7, 6]] {
            let coverage = Coverage::of(window, &[10, 11, 12]);
            assert_eq!(coverage.requested, 0, "{window:?}");
            assert!(!coverage.per_frame, "{window:?}");
            assert!(coverage.missing.is_empty(), "{window:?}");
            let _ = coverage.describe();
            assert_eq!(coverage.gaps(), 0, "{window:?}");
        }
        // And `gaps()` itself is saturating on a run that came out of a
        // manifest, not a file.
        let absurd = Coverage {
            window: [0, u64::MAX],
            requested: u64::MAX,
            first: 0,
            last: u64::MAX,
            samples: 3,
            per_frame: false,
            missing: vec![[0, u64::MAX]],
            before_sampling: Some([0, u64::MAX]),
        };
        assert_eq!(absurd.gaps(), u64::MAX);
        let _ = absurd.describe();
    }

    /// `parse_sample` reads the words out of one whole-segment read, and refuses
    /// a segment whose base is wrong: a `_np_row` outside 0..=63 is not a row.
    #[test]
    fn a_sample_reads_the_words_and_refuses_a_wrong_base() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        let mut bytes = vec![0u8; map.segment_length as usize];
        fn put(bytes: &mut [u8], map: &SymbolMap, name: &str, value: i16) {
            let at = map.offset(name).unwrap() as usize;
            bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        for (name, value) in [
            ("_np_ord", 25),
            ("_np_row", 7),
            ("_np_pat", 70),
            ("_np_loop", 1),
            ("_np_speed", 6),
            ("_np_tempo", 120),
            ("_np_zinfo", 0),
            ("_np_zplus", 2),
            ("_np_zframe", 385),
        ] {
            put(&mut bytes, &map, name, value);
        }
        let state = parse_sample(&bytes, 900, &map, 0x1051F0).unwrap();
        assert_eq!(state.ord, 25);
        assert_eq!(state.row, 7);
        assert_eq!(state.pat, 70);
        assert_eq!(state.loop_, 1);
        assert_eq!(state.speed, 6);
        assert_eq!(state.tempo, 120);
        assert_eq!(state.zplus, 2);
        assert_eq!(state.zframe, 385);
        assert_eq!(state.frame, 900);

        // A row of 300 is not a row: the base that produced it is not the
        // segment's, and the sample says so instead of recording it.
        put(&mut bytes, &map, "_np_row", 300);
        match parse_sample(&bytes, 900, &map, 0x1051F0) {
            Err(MusicError::Coherence(message)) => assert!(message.contains("0x1051F0"), "{message}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// The checks a trace passes before it is written: rows inside 0..=63, a
    /// speed and tempo that are 1..=255, and frames that strictly increase.
    #[test]
    fn the_coherence_checks_refuse_what_is_not_the_player() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        let good = PlayerState {
            frame: 900,
            ord: 25,
            row: 7,
            pat: 70,
            loop_: 0,
            speed: 6,
            tempo: 120,
            zinfo: 0,
            zplus: 0,
            zframe: 0,
        };
        assert!(check_samples(&[good], &map).is_ok());
        assert!(check_samples(&[], &map).is_err(), "nothing to write");
        let mut rowed = good;
        rowed.row = 64;
        assert!(check_samples(&[rowed], &map).is_err());
        let mut stopped = good;
        stopped.speed = 0;
        assert!(check_samples(&[stopped], &map).is_err());
        let mut equal = good;
        equal.frame = good.frame;
        assert!(check_samples(&[good, equal], &map).is_err(), "frames must increase");
    }

    /// The machine's words are read at the base the locator solved, and a value
    /// that is not a byte is only refused for the words that *are* bytes: the
    /// reference's mixing rate is 20000, which is not one.
    #[test]
    fn the_machine_words_are_read_as_their_own_widths() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        assert_eq!(map.segment_name, "text_stmik300_text");
        assert_eq!(map.segment_length, 29_607);
        assert_eq!(map.offset("_np_row").unwrap(), 0x08FD);
        assert_eq!(map.offset("_zmixspeed").unwrap(), 0x08D5);
        assert!(map.offset("_np_nonesuch").is_err());
    }

    /// The record census is in the map, so a reader can see which route produced
    /// the addresses: `D8`'s route 1 is the `PUBDEF` walk, and route 2 (the
    /// modules' own cells) is not reached.
    #[test]
    fn the_map_records_the_route_and_the_census() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        assert!(map.route.contains("PUBDEF"), "{}", map.route);
        assert_eq!(map.records.get("0x90"), Some(&45));
        assert_eq!(map.records.get("0x98"), Some(&1));
        assert_eq!(map.words.len(), POSITION_WORDS.len() + CLOCK_WORDS.len() + MACHINE_WORDS.len());
    }
}
