//! The OMF object reader: the `.OBK`/`.OBJ` container the original's linker put assets in.
//!
//! Not a runtime loader: it proves that every asset a part links has a standalone file in the
//! tree (`asset-inventory.md` §D, `tests/omf.rs`), recovers the one that has none
//! (`assets/TECHNO/CIRCLE2.EGA`, `tests/provenance.rs`), and gives `xtask music` the
//! player's symbols ([`symbols`]).
//!
//! An object is a flat sequence of records, `[type: u8][length: u16 LE][body]`, with no
//! alignment, so records are found only by walking them. `LEDATA` is **`0xA0`**,
//! the only type that carries segment data: 3,975 records over the tree's 151 objects, none of
//! type `0xA1` (Intel's 32-bit-offset `LEDATA`, refused rather than guessed at). A `LEDATA` body
//! is
//!
//! ```text
//! [segment index: u8][data offset: u16 LE][data: 1..=1024 bytes][checksum: u8]
//! ```
//!
//! The trailing byte is the checksum: it is `0x00` or the value that makes the record sum to 0
//! modulo 256 for all 3,975 records. `GLENZ/_FC.OBK` reconstructs `GLENZ/FC.UH` byte for byte;
//! reading the header one byte longer yields the same length with different bytes.

use crate::{le16, truncated, unsupported, AssetError, Format};

pub mod symbols;

/// One record: where it starts in the object, its type byte, and its body (checksum included).
pub(crate) struct Record<'a> {
    pub pos: usize,
    pub kind: u8,
    pub body: &'a [u8],
}

/// Every record of `bytes`, in order. A record header or body that runs past the end of the
/// object is [`AssetError::Truncated`]: a walk that mis-stepped would still find records further
/// on, so nothing is returned from a walk that does not land exactly on the object's last byte.
/// Framing is checked over the whole object before any record is decoded, so a torn record
/// anywhere wins over a refused record type earlier on.
pub(crate) fn records(bytes: &[u8]) -> Result<Vec<Record<'_>>, AssetError> {
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < bytes.len() {
        if bytes.len() - pos < 3 {
            return Err(truncated(Format::Omf, pos + 3, bytes.len()));
        }
        let end = pos + 3 + le16(bytes, pos + 1) as usize;
        if end > bytes.len() {
            return Err(truncated(Format::Omf, end, bytes.len()));
        }
        out.push(Record {
            pos,
            kind: bytes[pos],
            body: &bytes[pos + 3..end],
        });
        pos = end;
    }
    Ok(out)
}

/// One `LEDATA` record's data, at the segment offset the record gives it. A code object may
/// leave gaps between its blocks (`ALKU/COPPER.OBJ` puts data at 0 and again at 1,715), so the
/// blocks are not a segment image when concatenated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataBlock {
    /// The byte offset inside the segment the record declares.
    pub offset: u32,
    /// The data, without the record's header or its checksum byte.
    pub data: Vec<u8>,
}

/// Every `LEDATA` record's data with its segment offset, in record order. An object with no
/// `LEDATA` (a code object) is an empty list.
pub fn data_blocks(bytes: &[u8]) -> Result<Vec<DataBlock>, AssetError> {
    let mut out = Vec::new();
    for record in records(bytes)? {
        let body = record.body;
        match record.kind {
            0xA0 => {
                if body.len() < 4 {
                    return Err(truncated(Format::Omf, record.pos + 3 + 4, record.pos + 3 + body.len()));
                }
                out.push(DataBlock {
                    offset: u32::from(le16(body, 1)),
                    data: body[3..body.len() - 1].to_vec(),
                });
            }
            0xA1 => {
                return Err(unsupported(
                    Format::Omf,
                    "record type 0xA1 (LEDATA with a 32-bit offset), which no file in this tree uses",
                ))
            }
            _ => {}
        }
    }
    Ok(out)
}

/// The concatenated data of every `LEDATA` record in `bytes`: for 35 of the tree's blobs,
/// exactly the standalone file the blob shipped inside (§D).
///
/// [`AssetError::UnknownFormat`] when there is no `LEDATA` at all (a code object such as
/// `VISU/AMAIN.OBJ`, or an empty input).
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
