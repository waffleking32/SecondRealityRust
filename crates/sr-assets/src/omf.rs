//! The OMF object reader: the one decoder in this crate that reads a *container* rather
//! than an image.
//!
//! The original's assets travelled into `SECOND.EXE` and the part EXEs inside Borland OMF
//! objects (`.OBK`, `.OBJ`), linked as data segments. This module is not a runtime
//! loader — no `.OBK` ever reaches `sr-parts` (spec §4.4) — it exists to prove what the
//! inventory's §D measured: that every asset a part links has a standalone file in the
//! tree, byte for byte. §9 is the clause that makes the one that did not a recoverable
//! asset; `assets/PROVENANCE.md` records the file this phase recovered.
//!
//! # The record framing
//!
//! An OMF object is a flat sequence of records: a type byte, a `u16` little-endian length,
//! then that many bytes of body — no alignment and no padding, so records are found only
//! by walking them. `LEDATA` is type **`0xA0`** here, and it is the only record type that
//! carries segment data. Measured over every `.OBK`/`.OBJ` in the tree (151 files):
//! **3,975 `LEDATA` records, and not one of type `0xA1`.**
//!
//! The distinction is pinned because a walk that mapped the two types to one name could not
//! tell them apart: in the standard Intel OMF, `0xA1` is also `LEDATA`, with a 32-bit offset
//! and so a different body. No file in this tree uses that spelling, and [`extract_ledata`]
//! refuses it rather than guess at a framing that never occurs here.
//!
//! A `LEDATA` body is
//!
//! ```text
//! [segment index: u8][data offset: u16 LE][data: 1..=1024 bytes][checksum: u8]
//! ```
//!
//! so three bytes of header and one of trailer, and the data is `body[3..len - 1]`. The
//! offset is explicit, not implied by a record's position in the file: a code object may
//! place several blocks in one segment with gaps between them (`ALKU/COPPER.OBJ` puts data
//! at offset 0 and again at 1,715), while the tools that built the asset blobs emit
//! 1,024-byte chunks at 0, 1024, 2048, … Two measurements over the whole tree back the
//! framing, both taken while writing this module:
//!
//! - **The trailing byte is the OMF checksum.** For every one of the 3,975 records it is
//!   either `0x00` — the "no checksum" escape, 2,113 records — or exactly the value that
//!   makes the record's bytes sum to 0 modulo 256 (the other 1,862). A walk that had
//!   slipped a byte could not produce 3,975 self-consistent records.
//! - **Both the header and the trailer have to go.** `GLENZ/_FC.OBK` reconstructs 64,784
//!   bytes that equal `GLENZ/FC.UH`; reading the header as a segment and a `u16` offset
//!   (data from `body[4]`) shifts every record's data one byte and takes its checksum
//!   instead of the byte it dropped, which yields the same 64,784 bytes with different
//!   contents, and keeping the checksum yields 64,848. Only byte equality catches the
//!   first of those.
//!
//! The inventory §D describes the same three-and-one bytes as a `u16` segment followed by
//! a repeated offset byte. The byte positions are identical under either reading, and they
//! are what the extraction uses; the checksum measurement above is what identifies the
//! trailer, which is why this module states it that way.

use crate::{AssetError, Format};

pub mod symbols;

/// One `LEDATA` record's data, at the segment offset the record gives it.
///
/// The offset is the record's own field, not its position in the file: a code object may
/// place several blocks in one segment with gaps between them, so the blocks are *not*
/// contiguous in the segment and the concatenation [`extract_ledata`] returns is not a
/// segment image. A caller that needs the data where it will be in memory — the phase 3
/// trace's needles are located that way — reads it from here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBlock {
    /// The byte offset inside the segment the record declares.
    pub offset: u32,
    /// The data, without the record's header or its checksum byte.
    pub data: Vec<u8>,
}

/// Every `LEDATA` record's data with its segment offset, in record order.
///
/// The framing, the refusals and the checksum byte are [`extract_ledata`]'s; this is the
/// same walk keeping the offsets it discards.
///
/// # Errors
///
/// The same as [`extract_ledata`], except that an object with no `LEDATA` record is not an
/// error here: an empty list is the answer, because "which runs of data does this object
/// carry" has the answer "none" for a code object.
pub fn data_blocks(bytes: &[u8]) -> Result<Vec<DataBlock>, AssetError> {
    let format = Format::Omf;
    let mut out = Vec::new();
    let mut pos = 0usize;

    while pos < bytes.len() {
        // A record is a type byte and a `u16` length; anything shorter is a torn record.
        if bytes.len() - pos < 3 {
            return Err(AssetError::Truncated {
                format,
                needed: pos + 3,
                had: bytes.len(),
            });
        }
        let kind = bytes[pos];
        let len = u16::from_le_bytes([bytes[pos + 1], bytes[pos + 2]]) as usize;
        let body_start = pos + 3;
        // `len` is bounded by `u16::MAX`, so this cannot overflow.
        let end = body_start + len;
        if end > bytes.len() {
            return Err(AssetError::Truncated {
                format,
                needed: end,
                had: bytes.len(),
            });
        }
        let body = &bytes[body_start..end];

        match kind {
            0xA0 => {
                // Segment index, 16-bit offset, data, checksum: four bytes at minimum,
                // and the data is everything between the header and the checksum.
                if body.len() < 4 {
                    return Err(AssetError::Truncated {
                        format,
                        needed: body_start + 4,
                        had: end,
                    });
                }
                out.push(DataBlock {
                    offset: u32::from(u16::from_le_bytes([body[1], body[2]])),
                    data: body[3..body.len() - 1].to_vec(),
                });
            }
            0xA1 => {
                return Err(AssetError::Unsupported {
                    format,
                    what: "record type 0xA1, which no file in this tree uses and whose body \
                           this reader will not guess at"
                        .to_string(),
                })
            }
            // Every other type is a record this reader does not want: SEGDEF, FIXUPP,
            // symbols, comments. Their bodies are skipped by the length, which is what
            // walking an OMF object means.
            _ => {}
        }
        pos = end;
    }

    Ok(out)
}

/// The concatenated data of every `LEDATA` record in `bytes`, in record order — the
/// segment as the original's linker received it.
///
/// A `.OBK`/`.OBJ` is a build container, not an asset format: for 35 of the tree's blobs the
/// returned bytes are exactly the standalone file the blob shipped inside (the inventory's
/// §D), and callers compare them with a file in the tree (the tests do) or write them out
/// once, as `assets/TECHNO/CIRCLE2.EGA` was written.
///
/// The concatenation is **not** a segment image: the blocks carry their own offsets and a
/// code object may leave gaps between them, which is why [`data_blocks`] keeps them.
///
/// # Errors
///
/// - [`AssetError::Truncated`] — a record header or body runs past the end of the input,
///   or a `LEDATA` body is too short to hold its own header. The fields are file offsets
///   into `bytes`, as [`AssetError`] documents.
/// - [`AssetError::Unsupported`] — a `0xA1` record, which this tree never contains and
///   whose body this reader will not guess at (see the module docs).
/// - [`AssetError::UnknownFormat`] — no `LEDATA` record at all, so nothing is being
///   extracted: an object of code (`VISU/AMAIN.OBJ` is one in this tree), or an empty
///   input.
pub fn extract_ledata(bytes: &[u8]) -> Result<Vec<u8>, AssetError> {
    let blocks = data_blocks(bytes)?;
    if blocks.is_empty() {
        return Err(AssetError::UnknownFormat);
    }
    let mut out = Vec::with_capacity(blocks.iter().map(|block| block.data.len()).sum());
    for block in blocks {
        out.extend_from_slice(&block.data);
    }
    Ok(out)
}
