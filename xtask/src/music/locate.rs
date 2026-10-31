//! STMIK's segment in the running guest: the symbol map from the object's own `PUBDEF`
//! records, the two needles from its `LEDATA` data, and the scan that finds them.
//!
//! The emulator's memory read goes through the guest's page tables (`dosbox-automation`,
//! `hardware/memory.cpp:199`, `cpu/paging.h:293`), so addresses here are linear, and the demo's
//! DOS extender maps one physical page at two of them. A scan therefore finds the needles at
//! one base or at several; several are accepted only once the whole segment, read through
//! each, is byte-identical (`music-clock.md` §9).

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use sr_assets::omf::data_blocks;
use sr_assets::omf::symbols::{self, Symbol, Walk};

use super::{MusicError, CLOCK_WORDS, MACHINE_WORDS, POSITION_WORDS};
use crate::oracle::Oracle;
use crate::reference::sha256_hex;

/// The largest memory a scan walks. A read past the emulated memory is refused, and that ends
/// the scan; this only bounds a machine that answers forever.
const SCAN_LIMIT: u32 = 128 * 1024 * 1024;

/// One scan request: 64 KiB.
const SCAN_CHUNK: usize = 0x10000;

/// How many bases an ambiguous match reports.
pub const AMBIGUOUS_LIMIT: usize = 8;

/// The shortest needle that is evidence of a segment base ([`needles`]).
pub const NEEDLE_LEN: usize = 32;

/// How long the segment may take to appear. The loader has it in memory within the boot's
/// first seconds.
const LOCATE_TIMEOUT: Duration = Duration::from_secs(60);

/// Why the STMIK segment could not be located in the running guest.
#[derive(Debug)]
pub enum LocateError {
    /// The needle is not in guest memory, or not within [`SCAN_LIMIT`].
    NotFound { scanned: usize },
    /// More than one place holds the needle: every base its offset solves (up to
    /// [`AMBIGUOUS_LIMIT`]) and how many matches the scan saw.
    Ambiguous { bases: Vec<u32>, found: usize },
    /// The first read failed, so there was no memory to search.
    Read(String),
    /// The needle is too short to be evidence, the match is below its own offset, or the
    /// needles disagree.
    BadNeedle(String),
}

impl fmt::Display for LocateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocateError::NotFound { scanned } => write!(
                f,
                "the segment was not found in the {scanned} bytes of guest memory scanned"
            ),
            LocateError::Ambiguous { bases, found } => write!(
                f,
                "the needle occurs in {found} place(s), so no base is the base: it solves {}",
                hex_list(bases)
            ),
            LocateError::Read(message) => write!(f, "reading guest memory failed: {message}"),
            LocateError::BadNeedle(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for LocateError {}

/// `0x0051F0, 0x1051F0`.
fn hex_list(addresses: &[u32]) -> String {
    addresses
        .iter()
        .map(|address| format!("0x{address:06X}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Find the segment whose `target` offset holds `needle`, scanning guest memory.
///
/// `read(segment, offset, len)` is the oracle's memory read, `segment * 16 + offset` with a
/// 32-bit offset, so the scan walks `(0, address)` and returns its pair in that form. Each
/// 64 KiB request is searched together with the previous one's last `NEEDLE_LEN - 1` bytes,
/// so a needle across a boundary is found. One match is the answer; several are
/// [`LocateError::Ambiguous`], and the caller has to prove them one memory.
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
        // An empty answer is past the end of the emulated memory.
        let bytes = read(0, address, SCAN_CHUNK);
        if bytes.is_empty() {
            break;
        }
        scanned += bytes.len();

        let mut haystack = tail.clone();
        haystack.extend_from_slice(&bytes);
        if haystack.len() >= needle.len() {
            for start in 0..=haystack.len() - needle.len() {
                if &haystack[start..start + needle.len()] != needle {
                    continue;
                }
                // Entirely inside the previous chunk: counted then.
                if start + needle.len() <= tail.len() {
                    continue;
                }
                if matches.len() < AMBIGUOUS_LIMIT {
                    matches.push(address - tail.len() as u32 + start as u32);
                }
                found += 1;
            }
        }

        // A short chunk is the end of the emulated memory, and may be shorter than the
        // overlap.
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

/// One needle: where in the object it comes from, and the rule that chose it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Needle {
    /// The run's offset inside the segment: the `target` the base is solved from.
    pub offset: u32,
    pub len: usize,
    pub sha256: String,
    /// The `LEDATA` record's own segment offset.
    pub block: u32,
    pub rule: String,
    /// The address the run was found at, once it has been.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub found_at: Option<u32>,
}

/// The two runs of the object's own `LEDATA` data the locator searches for: the first run of
/// [`NEEDLE_LEN`] bytes with no byte repeated inside it (a shifted or padded copy cannot match
/// it by accident), and the first such run from a different record, so one match is checked
/// by an independent second.
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

/// A needle's bytes, out of the object.
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

/// The symbols the trace reads, resolved to offsets inside the segment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SymbolMap {
    /// The object's path as the command was given it.
    pub object: String,
    pub sha256: String,
    /// The segment's own name and class, from its `SEGDEF`.
    pub segment_name: String,
    pub segment_class: String,
    /// The segment's declared length: how many bytes one sample reads.
    pub segment_length: u32,
    pub route: String,
    /// The record census the walk counted, `0xNN -> count`.
    pub records: BTreeMap<String, usize>,
    pub words: BTreeMap<String, u32>,
}

impl SymbolMap {
    pub fn offset(&self, name: &str) -> Result<u32, MusicError> {
        self.words
            .get(name)
            .copied()
            .ok_or_else(|| MusicError::Symbols(format!("the symbol map does not carry {name}")))
    }
}

/// Build the map from the object's public symbols, checking what the trace depends on: every
/// name is published (D8's route 1), the seven position words are consecutive two-byte slots
/// (one request returns them together), and `_zinit` shares their segment (STMIK is one
/// segment, which is why `DIS/DISINT.ASM:242-282` sets `es` once).
pub fn symbol_map(object_path: &Path, object: &[u8]) -> Result<SymbolMap, MusicError> {
    let walk = symbols::walk(object)?;
    let wanted: Vec<&str> = POSITION_WORDS
        .iter()
        .chain(CLOCK_WORDS)
        .chain(MACHINE_WORDS)
        .copied()
        .collect();
    let chosen: Vec<Symbol> = wanted
        .iter()
        .map(|name| {
            walk.symbol(name).cloned().ok_or_else(|| {
                MusicError::Symbols(format!(
                    "the object publishes no symbol named {name:?}; it publishes {}",
                    walk.symbols.len()
                ))
            })
        })
        .collect::<Result<_, _>>()?;

    let mut slots: Vec<u32> = POSITION_WORDS
        .iter()
        .map(|name| {
            chosen
                .iter()
                .find(|symbol| symbol.name == *name)
                .expect("every wanted name was found")
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
    check_one_segment(&walk, chosen.first().expect("every wanted name was found"))?;

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

/// Where the STMIK segment was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Location {
    /// The lowest view, so the choice does not depend on scan order.
    pub base: u32,
    /// Every view, sorted and deduplicated.
    pub views: Vec<u32>,
    /// Why several views are one memory, when there are several.
    pub aliasing: Option<String>,
    pub frame: u64,
}

/// Scan for the segment until it is there. Each needle is searched on its own and the bases
/// they solve must agree; several views are used only once [`prove_views`] shows them one
/// memory.
pub(super) fn find_segment(
    oracle: &mut dyn Oracle,
    object: &[u8],
    map: &SymbolMap,
    needles: &[Needle; 2],
) -> Result<Location, MusicError> {
    let first = needle_bytes(object, &needles[0])?;
    let second = needle_bytes(object, &needles[1])?;
    // `locate`'s reader answers a failed read with an empty slice, the same as "not there
    // yet", so a reader that cannot read at all is told apart here, once.
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
                        hex_list(&one),
                        needles[1].offset,
                        hex_list(&two),
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
                // Not found yet, or one needle ahead of the other: look again.
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

/// Show that several bases are views of one memory: the whole segment read through each is
/// byte-identical. A stale copy holds the needles too and would read as a good base until the
/// player moved. Returns the sentence the manifest records.
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
        hex_list(views),
        views[0]
    ))
}

#[cfg(test)]
mod tests {
    use super::super::tests::object;
    use super::super::DEFAULT_OBJECT;
    use super::*;

    /// A memory that answers in 64 KiB chunks, as the oracle's does, with needles planted.
    fn memory(size: usize, placed: &[(u32, Vec<u8>)]) -> impl FnMut(u16, u32, usize) -> Vec<u8> {
        let mut bytes = vec![0u8; size];
        for (at, data) in placed {
            bytes[*at as usize..*at as usize + data.len()].copy_from_slice(data);
        }
        move |segment: u16, offset: u32, len: usize| -> Vec<u8> {
            let at = usize::from(segment) * 16 + offset as usize;
            if at >= bytes.len() {
                return Vec::new();
            }
            bytes[at..(at + len).min(bytes.len())].to_vec()
        }
    }

    fn needle(seed: u8) -> Vec<u8> {
        (0..NEEDLE_LEN).map(|i| seed.wrapping_add(i as u8)).collect()
    }

    /// The base is solved from where the needle is, and the pair returned is one the same
    /// reader accepts.
    #[test]
    fn a_unique_needle_solves_the_base() {
        let bits = needle(7);
        let mut read = memory(0x30000, &[(0x2_1F00, bits.clone())]);
        let (segment, base) = locate(0x1234, &bits, &mut read).unwrap();
        assert_eq!(base, 0x2_1F00 - 0x1234);
        let bytes = read(segment, base + 0x1234, NEEDLE_LEN);
        assert_eq!(bytes, bits);
    }

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

    #[test]
    fn a_needle_across_the_chunk_boundary_is_found() {
        let bits = needle(3);
        let at = SCAN_CHUNK as u32 - 5;
        let mut read = memory(0x20000, &[(at, bits.clone())]);
        let (_, base) = locate(0, &bits, &mut read).unwrap();
        assert_eq!(base, at);
    }

    /// A needle that is nowhere is a refusal that counts every byte scanned.
    #[test]
    fn a_needle_that_is_nowhere_is_refused() {
        let mut read = memory(0x20000, &[]);
        assert!(matches!(
            locate(0, &needle(1), &mut read),
            Err(LocateError::NotFound { scanned: 0x20000 })
        ));
    }

    #[test]
    fn a_short_needle_is_not_evidence() {
        let mut read = memory(0x1000, &[(0, vec![1, 2, 3, 4, 5, 6, 7, 8])]);
        match locate(0, &[1, 2, 3, 4, 5, 6, 7, 8], &mut read) {
            Err(LocateError::BadNeedle(message)) => assert!(message.contains("8-byte"), "{message}"),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// A last request shorter than the needle ends the scan, and a needle that fits inside the
    /// last short chunk is still found.
    #[test]
    fn a_short_last_chunk_ends_the_scan_without_panicking() {
        let mut read = memory(SCAN_CHUNK + 8, &[]);
        assert!(matches!(
            locate(0, &needle(4), &mut read),
            Err(LocateError::NotFound { .. })
        ));
        let short = needle(5);
        let at = 8_000u32;
        let mut read = memory(at as usize + short.len(), &[(at, short.clone())]);
        assert_eq!(locate(0, &short, &mut read).unwrap().1, at);
    }

    /// The needles are 32-byte runs of the object's own `LEDATA` data from different records,
    /// each occurring once in the object.
    #[test]
    fn the_objects_needles_are_unique_runs_from_different_records() {
        let object = object();
        let needles = needles(&object).unwrap();
        assert_eq!(needles[0].len, NEEDLE_LEN);
        assert_ne!(needles[0].block, needles[1].block, "different LEDATA records");
        for needle in &needles {
            let bytes = needle_bytes(&object, needle).unwrap();
            assert_eq!(bytes.len(), NEEDLE_LEN);
            assert_eq!(sha256_hex(&bytes), needle.sha256);
            assert!(all_distinct(&bytes), "no byte repeats inside the run");
            let count = object
                .windows(NEEDLE_LEN)
                .filter(|window| *window == bytes.as_slice())
                .count();
            assert_eq!(count, 1, "the run occurs once in the object");
        }
    }

    /// The map's segment, two offsets, the `PUBDEF` census (D8's route 1) and the word count.
    #[test]
    fn the_map_records_the_segment_the_route_and_the_census() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        assert_eq!(map.segment_name, "text_stmik300_text");
        assert_eq!(map.segment_length, 29_607);
        assert_eq!(map.offset("_np_row").unwrap(), 0x08FD);
        assert_eq!(map.offset("_zmixspeed").unwrap(), 0x08D5);
        assert!(map.offset("_np_nonesuch").is_err());
        assert!(map.route.contains("PUBDEF"), "{}", map.route);
        assert_eq!(map.records.get("0x90"), Some(&45));
        assert_eq!(map.records.get("0x98"), Some(&1));
        assert_eq!(map.words.len(), POSITION_WORDS.len() + CLOCK_WORDS.len() + MACHINE_WORDS.len());
    }
}
