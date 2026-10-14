//! The OMF **public symbols**: `SEGDEF` and `PUBDEF`, read by the same record
//! framing [`extract_ledata`](super::extract_ledata) walks.
//!
//! `extract_ledata` answers "what bytes did the original's linker receive";
//! this module answers "what did it call them". The two are the same walk: a
//! flat sequence of `[type: u8][length: u16 LE][body]` records with no
//! alignment and no padding, so a record is found only by stepping exactly
//! `3 + length` from the previous one.
//!
//! # Why a walk that fails loudly
//!
//! A walk that skips the record types it does not understand is correct only if
//! the length prefix is read correctly at *every* step, and a walk that
//! mis-steps still "finds symbols" further down the file — it reports a map for
//! a record framing that is not the file's. So [`walk`] refuses
//! ([`OmfError::Malformed`]) rather than returning what it happened to find:
//!
//! - a record header that does not fit in the bytes left, or a body whose
//!   length runs past the end of the object;
//! - a walk that does not land exactly on the object's last byte;
//! - a `PUBDEF` entry that does not fit between its record's header and its
//!   trailing checksum byte, or a name index that no `LNAMES` record defines.
//!
//! And [`Walk::types`] carries the census — every record type and how many
//! times it was seen — so a caller can check the walk against the file it
//! walked rather than against how many symbols came out ([`Walk::records`],
//! [`Walk::bytes`]).
//!
//! # The records it decodes
//!
//! | type | record | what this module does with it |
//! |---|---|---|
//! | `0x90` | `PUBDEF` | decoded: name, segment index, offset |
//! | `0x96` | `LNAMES` | decoded: the running list of names the indexes refer to |
//! | `0x98` | `SEGDEF` | decoded: the segment's declared length, name and class |
//! | `0x91`, `0x99` | `PUBDEF32`, `SEGDEF32` | refused: their offsets are 32-bit, and decoding one as its 16-bit spelling would produce a wrong map |
//! | everything else | `THEADR`, `COMENT`, `MODEND`, `LEDATA`, `LIDATA`, `LINNUM`, `GRPDEF`, `FIXUPP`, … | walked past by the length |
//!
//! # The published variables are two-byte words
//!
//! `MAIN/STMIKA.INC:17-28` declares the `np_*` and `_z*` variables this phase
//! reads as `word`s, and `MAIN/STMIK.H` declares them `int far`. A symbol's
//! offset is a byte offset into its segment; a reader adds it to the segment's
//! base and reads two bytes, little-endian.
//!
//! # The one measured detail this module does not derive
//!
//! A `SEGDEF`'s declared length is nominally in bytes when the segment is
//! byte-aligned and in paragraphs otherwise (`ACBP`'s alignment field). **Every
//! single-segment object in this tree declares it in bytes**, whatever its
//! alignment field says — `MAIN/STMIK.300`'s 29,607 is the 21,029 bytes of its
//! `LEDATA` data plus the 8,578 its `LIDATA` records expand to, and all 71
//! single-segment `.OBJ`/`.OBK` files in the tree agree the same way. The
//! number is therefore taken verbatim and used as bytes; [`SegmentDef::acbp`]
//! keeps the byte the record wrote, so nothing is hidden by the choice.

use std::collections::BTreeMap;
use std::fmt;

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

/// What one walk of an object found.
///
/// The census travels with the symbols because the symbols alone cannot say
/// whether the walk that produced them stepped correctly: a mis-stepped walk
/// finds *some* symbols, and only the record counts (and the record count times
/// three bytes plus the summed lengths versus the file's own size) show it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Walk {
    /// Every public symbol, in the order the `PUBDEF` records list them.
    pub symbols: Vec<Symbol>,
    /// How many records of each type the walk stepped over, by type byte.
    pub types: BTreeMap<u8, usize>,
    /// Every byte the walk's records cover: the sum of `3 + length` over them,
    /// which is the object's own length for any object [`walk`] accepted — it
    /// refuses one its records do not cover exactly. Carried as a number so a
    /// caller can compare the two rather than take the walk's word for it.
    pub bytes: usize,
    /// The `SEGDEF` records, by the index `Symbol::segment` names.
    pub segments: BTreeMap<u16, SegmentDef>,
}

impl Walk {
    /// How many records the walk stepped over.
    pub fn records(&self) -> usize {
        self.types.values().sum()
    }

    /// The symbols whose names are `names`, in `names`' order. A name the
    /// object does not carry is an error naming it: a caller that asks for
    /// `_np_row` and gets nothing back must not go on to read an address it
    /// guessed.
    pub fn select(&self, names: &[&str]) -> Result<Vec<Symbol>, OmfError> {
        let mut chosen = Vec::with_capacity(names.len());
        for name in names {
            match self.symbols.iter().find(|symbol| symbol.name == *name) {
                Some(symbol) => chosen.push(symbol.clone()),
                None => {
                    return Err(OmfError::Malformed(format!(
                        "the object publishes no symbol named {name:?}; it publishes {}",
                        self.symbols.len()
                    )))
                }
            }
        }
        Ok(chosen)
    }

    /// The symbol with this name, or `None`.
    pub fn symbol(&self, name: &str) -> Option<&Symbol> {
        self.symbols.iter().find(|symbol| symbol.name == name)
    }
}

/// Why an object could not be walked, or a walk's result could not be used.
#[derive(Debug)]
pub enum OmfError {
    /// The records do not frame: a body that runs past the end of the object, a
    /// walk that does not end on the last byte, a `PUBDEF` entry that does not
    /// fit between its record's header and its checksum, or a name index no
    /// `LNAMES` record defines. Every one of these means the walk cannot be
    /// trusted, so no symbols are returned.
    Malformed(String),
    /// A record this reader will not decode, where decoding it as something
    /// else would invent an answer. `0x91`/`0x99` are `PUBDEF`/`SEGDEF` with
    /// 32-bit offsets and lengths; `0xA1` is `LEDATA` with a 32-bit offset,
    /// which [`extract_ledata`](super::extract_ledata) refuses for the same
    /// reason.
    Unsupported { what: String },
}

impl fmt::Display for OmfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OmfError::Malformed(message) => write!(f, "malformed OMF object: {message}"),
            OmfError::Unsupported { what } => write!(f, "unsupported OMF record: {what}"),
        }
    }
}

impl std::error::Error for OmfError {}

/// Walk `object` and return everything this module knows how to read.
///
/// See the module docs for what is refused and why. The walk is the framing
/// [`extract_ledata`](super::extract_ledata) uses — a type byte, a `u16`
/// little-endian length, that many bytes of body — and it ends exactly at the
/// object's last byte or the object is refused.
pub fn walk(object: &[u8]) -> Result<Walk, OmfError> {
    let mut walk = Walk::default();
    // `LNAMES` records *append* to one running list (`OMF`'s name indexes are
    // global to the object), so the list is built as the records are met.
    let mut names: Vec<String> = Vec::new();
    // `SEGDEF`s in the order they were met: the segment index a `PUBDEF` writes
    // is 1-based over that order.
    let mut segment_order: Vec<u16> = Vec::new();
    // `PUBDEF` records are resolved after the walk, because nothing in the
    // format promises a `PUBDEF` follows the `SEGDEF` it names.
    let mut published: Vec<(u16, String, u32)> = Vec::new();

    let mut pos = 0usize;
    while pos < object.len() {
        if object.len() - pos < 3 {
            return Err(OmfError::Malformed(format!(
                "a record header needs three bytes and the object ends {} byte(s) after offset \
                 {pos} (of {})",
                object.len() - pos,
                object.len()
            )));
        }
        let kind = object[pos];
        let len = u16::from_le_bytes([object[pos + 1], object[pos + 2]]) as usize;
        let body_start = pos + 3;
        let end = body_start + len;
        if end > object.len() {
            return Err(OmfError::Malformed(format!(
                "the record of type 0x{kind:02X} at offset {pos} says its body is {len} bytes, \
                 which ends at {end} — past the object's {} bytes",
                object.len()
            )));
        }
        let body = &object[body_start..end];
        *walk.types.entry(kind).or_insert(0) += 1;
        walk.bytes += 3 + len;

        match kind {
            0x90 => pubdef(body, pos, &mut published)?,
            // The 32-bit spellings of the two records this module decodes. They
            // are refused rather than walked past: the fields are at different
            // positions, so reading one as its 16-bit spelling would produce a
            // symbol map that looks fine and is not the object's.
            0x91 => {
                return Err(OmfError::Unsupported {
                    what: "record type 0x91 (PUBDEF with 32-bit offsets), which no file in this \
                           tree uses and whose fields this reader will not read as 0x90's"
                        .to_string(),
                })
            }
            0x99 => {
                return Err(OmfError::Unsupported {
                    what: "record type 0x99 (SEGDEF with a 32-bit length), which no file in this \
                           tree uses and whose fields this reader will not read as 0x98's"
                        .to_string(),
                })
            }
            0x96 => lnames(body, pos, &mut names)?,
            0x98 => segdef(body, pos, &names, &mut segment_order, &mut walk)?,
            // Everything else — THEADR, COMENT, MODEND, LINNUM, GRPDEF,
            // FIXUPP, LEDATA, LIDATA — is a record this module does not want,
            // and walking it by its length is what walking an OMF object means.
            _ => {}
        }
        pos = end;
    }

    if pos != object.len() {
        return Err(OmfError::Malformed(format!(
            "the walk ended at byte {pos} of {} — a record's length did not land on the end of \
             the object, so the framing it read is not the object's",
            object.len()
        )));
    }

    for (index, name, offset) in published {
        if !walk.segments.contains_key(&index) {
            return Err(OmfError::Malformed(format!(
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

/// [`walk`]'s symbols alone, for a caller with no use for the census.
pub fn public_symbols(object: &[u8]) -> Result<Vec<Symbol>, OmfError> {
    Ok(walk(object)?.symbols)
}

/// The `PUBDEF` body: a base group index, a base segment index, then one entry
/// per symbol — a length-prefixed name, the offset, and the public type.
///
/// The two index bytes are read as plain bytes: `MAIN/STMIK.300`'s first
/// `PUBDEF` opens `00 01`, which under the standard's "a zero index is followed
/// by a two-byte index" reading would make the group index 267 — a value no
/// object with no `GRPDEF` record can mean. Measured across the tree's objects,
/// the segment index is the 1-based position of the `SEGDEF` the entry names,
/// and that is what the check against [`Walk::segments`] confirms.
///
/// A segment index of 0 is a symbol whose "offset" is an absolute address
/// rather than a segment offset. No file in this tree publishes one, and
/// reading it as an offset would put a variable at the wrong address, so it is
/// refused.
fn pubdef(body: &[u8], record: usize, out: &mut Vec<(u16, String, u32)>) -> Result<(), OmfError> {
    if body.len() < 3 {
        return Err(OmfError::Malformed(format!(
            "the PUBDEF at {record} is {} bytes; it needs at least two index bytes and its \
             checksum",
            body.len()
        )));
    }
    // The body ends in a checksum byte, which is not an entry.
    let entries = body.len() - 1;
    let segment = u16::from(body[1]);
    if segment == 0 {
        return Err(OmfError::Unsupported {
            what: format!(
                "the PUBDEF at {record} publishes a symbol in segment 0 — an absolute symbol, \
                 whose offset is an address and not a segment offset"
            ),
        });
    }
    let mut at = 2usize;
    while at < entries {
        let len = usize::from(body[at]);
        if len == 0 {
            return Err(OmfError::Malformed(format!(
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
            return Err(OmfError::Malformed(format!(
                "the PUBDEF at {record} has an entry at body offset {at} that needs {} bytes and \
                 only {} are left before the record's checksum",
                offset_end + 1 - at,
                entries - at
            )));
        }
        let name = String::from_utf8_lossy(&body[name_start..name_end]).into_owned();
        let offset = u32::from(u16::from_le_bytes([body[name_end], body[name_end + 1]]));
        out.push((segment, name, offset));
        at = offset_end + 1;
    }
    Ok(())
}

/// The `LNAMES` body: length-prefixed names, appended to the object's running
/// list. The trailing byte is the record's checksum.
fn lnames(body: &[u8], record: usize, names: &mut Vec<String>) -> Result<(), OmfError> {
    let end = body.len().checked_sub(1).ok_or_else(|| {
        OmfError::Malformed(format!("the LNAMES at {record} is empty"))
    })?;
    let mut at = 0usize;
    while at < end {
        let len = usize::from(body[at]);
        let name_end = at + 1 + len;
        if name_end > end {
            return Err(OmfError::Malformed(format!(
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
    order: &mut Vec<u16>,
    walk: &mut Walk,
) -> Result<(), OmfError> {
    if body.len() < 6 {
        return Err(OmfError::Malformed(format!(
            "the SEGDEF at {record} is {} bytes; six are the least that can hold its header",
            body.len()
        )));
    }
    let acbp = body[0];
    let length = u32::from(u16::from_le_bytes([body[1], body[2]]));
    let index = u16::try_from(order.len() + 1).expect("an object cannot hold 65,535 segments");
    let name = resolve(names, body[3], record, "name")?;
    let class = resolve(names, body[4], record, "class")?;
    let _overlay = resolve(names, body[5], record, "overlay")?;
    order.push(index);
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
fn resolve(names: &[String], index: u8, record: usize, what: &str) -> Result<String, OmfError> {
    if index == 0 {
        return Ok(String::new());
    }
    names
        .get(usize::from(index) - 1)
        .cloned()
        .ok_or_else(|| {
            OmfError::Malformed(format!(
                "the SEGDEF at {record} names {what} index {index}, and the object's LNAMES \
                 records define {} name(s)",
                names.len()
            ))
        })
}
