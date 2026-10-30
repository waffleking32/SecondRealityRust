//! The OMF **public symbols**: `SEGDEF` and `PUBDEF`, read over the same record walk as
//! [`extract_ledata`](super::extract_ledata).
//!
//! | type | record | what [`walk`] does with it |
//! |---|---|---|
//! | `0x90` | `PUBDEF` | decoded: name, segment index, offset |
//! | `0x96` | `LNAMES` | decoded: the running list of names the indexes refer to |
//! | `0x98` | `SEGDEF` | decoded: the segment's declared length, name and class |
//! | `0x91`, `0x99` | `PUBDEF32`, `SEGDEF32` | refused: their fields are 32-bit, and reading one as its 16-bit spelling would produce a plausible, wrong map |
//! | everything else | `THEADR`, `COMENT`, `MODEND`, `LEDATA`, `LIDATA`, `FIXUPP`, … | walked past by the length |
//!
//! A walk that mis-steps still "finds symbols", so a malformed object is refused rather than
//! half-read, and [`Walk::types`] carries the record census for a caller to check against the
//! file.
//!
//! The published variables are words (`MAIN/STMIKA.INC:17-28`, `int far` in `MAIN/STMIK.H`): a
//! symbol's offset is a byte offset into its segment. A `SEGDEF`'s length is read as bytes: every
//! single-segment object in the tree declares it so whatever its `ACBP` alignment says
//! (`MAIN/STMIK.300`'s 29,607 is its 21,029 `LEDATA` bytes plus the 8,578 its `LIDATA` expands to),
//! and [`SegmentDef::acbp`] keeps the raw byte.

use std::collections::BTreeMap;

use super::records;
use crate::{le16, unsupported, AssetError, Format};

fn malformed(what: String) -> AssetError {
    unsupported(Format::Omf, what)
}

/// One `PUBDEF` entry: a name, the `SEGDEF` index its offset is relative to,
/// and the offset itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symbol {
    /// The name as the object writes it — `_np_row`, `_zinit`, …
    pub name: String,
    /// The index of the `SEGDEF` record this offset is relative to —
    /// [`Walk::segments`] is keyed by it.
    pub segment: u16,
    /// The byte offset inside that segment.
    pub offset: u32,
}

/// One `SEGDEF` record: what a symbol's segment index resolves to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentDef {
    /// The segment's name, resolved through `LNAMES`
    /// (`MAIN/STMIK.300`'s is `text_stmik300_text`).
    pub name: String,
    /// The segment's class name (`CODE` for `MAIN/STMIK.300`).
    pub class: String,
    /// The segment's declared length. Read as bytes — see the module docs.
    pub length: u32,
    /// The `ACBP` byte as the record wrote it, so the raw alignment /
    /// combination / big / use flags survive the reading above.
    pub acbp: u8,
}

/// What one walk of an object found, with the census that shows the walk stepped correctly.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Walk {
    /// Every public symbol, in the order the `PUBDEF` records list them.
    pub symbols: Vec<Symbol>,
    /// How many records of each type the walk stepped over, by type byte.
    pub types: BTreeMap<u8, usize>,
    /// The sum of `3 + length` over the records: the object's length, for a caller to compare.
    pub bytes: usize,
    /// The `SEGDEF` records, by the index `Symbol::segment` names.
    pub segments: BTreeMap<u16, SegmentDef>,
}

impl Walk {
    /// How many records the walk stepped over.
    pub fn records(&self) -> usize {
        self.types.values().sum()
    }

    /// The symbol with this name, or `None`.
    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|symbol| symbol.name == name)
    }
}

/// Walk `object` and return everything this module reads.
pub fn walk(object: &[u8]) -> Result<Walk, AssetError> {
    let mut walk = Walk::default();
    // `LNAMES` records append to one running list; a `SEGDEF`'s index is 1-based over the
    // `SEGDEF`s met so far; `PUBDEF`s are resolved after the walk, since nothing promises one
    // follows the `SEGDEF` it names.
    let mut names: Vec<String> = Vec::new();
    let mut published: Vec<(u16, String, u32)> = Vec::new();

    for record in records(object)? {
        *walk.types.entry(record.kind).or_insert(0) += 1;
        walk.bytes += 3 + record.body.len();
        match record.kind {
            0x90 => pubdef(record.body, record.pos, &mut published)?,
            0x91 | 0x99 => {
                return Err(unsupported(
                    Format::Omf,
                    format!(
                        "record type {:#04x} (a 32-bit PUBDEF or SEGDEF), which no file in this \
                         tree uses",
                        record.kind
                    ),
                ))
            }
            0x96 => lnames(record.body, record.pos, &mut names)?,
            0x98 => segdef(record.body, record.pos, &names, &mut walk)?,
            _ => {}
        }
    }

    for (index, name, offset) in published {
        if !walk.segments.contains_key(&index) {
            return Err(malformed(format!(
                "the symbol {name:?} names segment {index}, and the object defines {} segment(s)",
                walk.segments.len()
            )));
        }
        walk.symbols.push(Symbol {
            name,
            segment: index,
            offset,
        });
    }
    Ok(walk)
}

/// The `PUBDEF` body: a base group index, a base segment index, then one entry
/// per symbol — a length-prefixed name, the offset, and the public type.
///
/// The two index bytes are plain bytes (`MAIN/STMIK.300`'s first `PUBDEF` opens `00 01`; the
/// standard's two-byte-index reading would make its group 267). Segment 0, an absolute symbol,
/// is refused: no file here publishes one.
fn pubdef(body: &[u8], record: usize, out: &mut Vec<(u16, String, u32)>) -> Result<(), AssetError> {
    if body.len() < 3 {
        return Err(malformed(format!(
            "the PUBDEF at {record} is {} bytes; it needs at least two index bytes and its \
             checksum",
            body.len()
        )));
    }
    // The body ends in a checksum byte, which is not an entry.
    let entries = body.len() - 1;
    let segment = u16::from(body[1]);
    if segment == 0 {
        return Err(malformed(format!(
            "the PUBDEF at {record} publishes a symbol in segment 0, an absolute symbol"
        )));
    }
    let mut at = 2usize;
    while at < entries {
        let len = usize::from(body[at]);
        if len == 0 {
            return Err(malformed(format!(
                "the PUBDEF at {record} has a zero-length name at offset {at} of its body, so its \
                 entries no longer frame"
            )));
        }
        let name_start = at + 1;
        let name_end = name_start + len;
        let offset_end = name_end + 2;
        // name, two offset bytes, one public-type byte — and all of it inside
        // the body's entries (the trailing byte is the record's checksum).
        if offset_end + 1 > entries {
            return Err(malformed(format!(
                "the PUBDEF at {record} has an entry at body offset {at} that needs {} bytes and \
                 only {} are left before the record's checksum",
                offset_end + 1 - at,
                entries - at
            )));
        }
        let name = String::from_utf8_lossy(&body[name_start..name_end]).into_owned();
        let offset = u32::from(le16(body, name_end));
        out.push((segment, name, offset));
        at = offset_end + 1;
    }
    Ok(())
}

/// The `LNAMES` body: length-prefixed names, appended to the object's running
/// list. The trailing byte is the record's checksum.
fn lnames(body: &[u8], record: usize, names: &mut Vec<String>) -> Result<(), AssetError> {
    let end = body.len().checked_sub(1).ok_or_else(|| {
        malformed(format!("the LNAMES at {record} is empty"))
    })?;
    let mut at = 0usize;
    while at < end {
        let len = usize::from(body[at]);
        let name_end = at + 1 + len;
        if name_end > end {
            return Err(malformed(format!(
                "the LNAMES at {record} has a name at body offset {at} that needs {len} bytes and \
                 runs past the record's checksum"
            )));
        }
        names.push(String::from_utf8_lossy(&body[at + 1..name_end]).into_owned());
        at = name_end;
    }
    Ok(())
}

/// The `SEGDEF` body: `ACBP`, the length, and the `LNAMES` indexes of the
/// segment's name, its class and its overlay.
fn segdef(
    body: &[u8],
    record: usize,
    names: &[String],
    walk: &mut Walk,
) -> Result<(), AssetError> {
    if body.len() < 6 {
        return Err(malformed(format!(
            "the SEGDEF at {record} is {} bytes; six are the least that can hold its header",
            body.len()
        )));
    }
    let acbp = body[0];
    let length = u32::from(le16(body, 1));
    let index = u16::try_from(walk.segments.len() + 1).expect("an object cannot hold 65,535 segments");
    let name = resolve(names, body[3], record, "name")?;
    let class = resolve(names, body[4], record, "class")?;
    let _overlay = resolve(names, body[5], record, "overlay")?;
    walk.segments.insert(
        index,
        SegmentDef {
            name,
            class,
            length,
            acbp,
        },
    );
    Ok(())
}

/// An `LNAMES` index (1-based) as a name. Index 0 is "no name", which the
/// segment records use for the overlay they do not have.
fn resolve(names: &[String], index: u8, record: usize, what: &str) -> Result<String, AssetError> {
    if index == 0 {
        return Ok(String::new());
    }
    names
        .get(usize::from(index) - 1)
        .cloned()
        .ok_or_else(|| {
            malformed(format!(
                "the SEGDEF at {record} names {what} index {index}, and the object's LNAMES \
                 records define {} name(s)",
                names.len()
            ))
        })
}
