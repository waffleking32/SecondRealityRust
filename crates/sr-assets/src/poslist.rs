//! FOREST's and WATER's position lists, `FOREST/POS1-3.DAT` and `WATER/WAT1-3.DAT`, which
//! MNTSCRL and RAYSCRL blit their scrolling text through.
//!
//! One file per frame group: a stream of columns, each a little-endian `u16` count and then that
//! many little-endian `u16` screen byte offsets, with no header. The writer emits the count and,
//! only when it is non-zero, the offsets (`WATER/DATGEN.PAS:73-88`); both blitters walk a column
//! with `lodsw` and skip a zero count's pixels, not its count word (`FOREST/ROUTINES.ASM:20-35`,
//! `WATER/ROUTINES.ASM:20-36`). The column count is the caller's loop (`237*31`,
//! `FOREST/ROUTINES.ASM:21`; `158*34`, `WATER/ROUTINES.ASM:20`), so [`PosList::parse`] reads
//! columns until the bytes run out and the part checks the count.

use crate::{le16, truncated, AssetError, Format};

/// One frame group's columns, in file order. A zero-count column is kept as an empty `Vec`.
#[derive(Debug)]
pub struct PosList {
    pub columns: Vec<Vec<u16>>,
}

impl PosList {
    /// Parse a whole `.DAT` file. A stream that ends inside a count or before a count's offsets
    /// is [`AssetError::Truncated`].
    pub fn parse(bytes: &[u8]) -> Result<PosList, AssetError> {
        let mut columns = Vec::new();
        let mut at = 0usize;
        while at < bytes.len() {
            if bytes.len() - at < 2 {
                return Err(truncated(Format::PosList, at + 2, bytes.len()));
            }
            let count = le16(bytes, at) as usize;
            at += 2;
            let end = at + count * 2;
            if end > bytes.len() {
                return Err(truncated(Format::PosList, end, bytes.len()));
            }
            columns.push((at..end).step_by(2).map(|i| le16(bytes, i)).collect());
            at = end;
        }
        Ok(PosList { columns })
    }

    /// Column `i`. Panics if there is no such column, as the fixed-count blitter loops would
    /// run past a short file.
    pub fn column(&self, i: usize) -> &[u16] {
        &self.columns[i]
    }
}
