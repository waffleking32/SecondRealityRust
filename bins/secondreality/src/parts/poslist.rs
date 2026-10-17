//! The per-column position lists MNTSCRL and RAYSCRL share.
//!
//! `FOREST/POS1-3.DAT` and `WATER/WAT1-3.DAT` are the same format, one file
//! per frame group: a stream of columns, each a little-endian `u16` count and
//! then that many little-endian `u16` screen byte offsets. Both blitters walk
//! a column with `lodsw` and skip a zero count outright — the original's
//! hidden/transparent column (`FOREST/ROUTINES.ASM:20-35`,
//! `WATER/ROUTINES.ASM:20-36`) — and the writer emits exactly that shape:
//! `blockwrite(f,tc,2)` followed, only when `tc > 0`, by the offsets as words
//! (`WATER/DATGEN.PAS:73-88`). The column count is fixed by the caller's loop
//! — `237*31` for FOREST (`FOREST/ROUTINES.ASM:20`), `158*34` for WATER
//! (`WATER/ROUTINES.ASM:20`) — so the parser reads columns until the bytes
//! run out and the part checks the count.
//!
//! The two blitters stay in their own parts: FOREST adds the background
//! pixel and the font byte with a +128 bias, WATER replays the background
//! where the font byte is zero. Only the byte layout is shared.

/// One frame group's columns: each a count and then that many destination
/// offsets, in the order the file carries them.
// The decoder lands before its two consumers: MNTSCRL and RAYSCRL are the
// parts that construct it, and they arrive in their own tasks. Until then the
// binary's non-test build has no caller, and without this the build warns.
#[allow(dead_code)]
#[derive(Debug)]
pub(crate) struct PosList {
    /// The columns, in file order. A zero-count column is kept as an empty
    /// `Vec`, not dropped: its two count bytes are part of the stream and the
    /// caller's loop advances over it.
    pub columns: Vec<Vec<u16>>,
}

// Same reason as the struct's: no non-test caller until the parts land.
#[allow(dead_code)]
impl PosList {
    /// Parse a whole `.DAT` file. Every byte belongs to a column, so a stream
    /// that ends before a declared count's offsets is an error; an empty
    /// `columns` means the input held no columns at all.
    ///
    /// A zero count consumes its two bytes and yields an empty column: the
    /// blitters' `or ax,ax / je` skips its pixels, not its count word
    /// (`FOREST/ROUTINES.ASM:23-26`, `WATER/ROUTINES.ASM:23-26`).
    pub(crate) fn parse(bytes: &[u8]) -> Result<PosList, String> {
        let mut columns = Vec::new();
        let mut at = 0usize;
        while at < bytes.len() {
            if bytes.len() - at < 2 {
                return Err(format!(
                    "truncated position list at byte {at}: a column count needs 2 bytes, \
                     but the stream has {} left",
                    bytes.len() - at
                ));
            }
            let count = u16::from_le_bytes([bytes[at], bytes[at + 1]]) as usize;
            at += 2;
            let end = at + count * 2;
            if end > bytes.len() {
                return Err(format!(
                    "truncated position list at byte {at}: column {} declares {count} \
                     offsets ({count} words = {} bytes), but the stream has {} left",
                    columns.len(),
                    count * 2,
                    bytes.len() - at
                ));
            }
            let offsets = bytes[at..end]
                .chunks_exact(2)
                .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
                .collect();
            at = end;
            columns.push(offsets);
        }
        Ok(PosList { columns })
    }

    /// Column `i`. Panics if there is no such column, as the fixed-count
    /// blitter loops do when a file is short.
    pub(crate) fn column(&self, i: usize) -> &[u16] {
        &self.columns[i]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Two columns written byte for byte: a three-offset column and a
    /// two-offset one. The offsets mix byte orders that cannot be confused
    /// with their byte-swapped readings in the assertions below.
    const TWO_COLUMNS: [u8; 14] = [
        0x03, 0x00, // column 0 count: 3
        0x34, 0x12, // 0x1234
        0xFF, 0x00, // 0x00FF
        0x40, 0xB5, // 0xB540
        0x02, 0x00, // column 1 count: 2
        0x01, 0x00, // 0x0001
        0x00, 0x02, // 0x0200
    ];

    #[test]
    fn parses_counts_and_little_endian_offsets() {
        let list = PosList::parse(&TWO_COLUMNS).expect("the two columns parse");
        assert_eq!(
            list.columns,
            vec![vec![0x1234, 0x00FF, 0xB540], vec![0x0001, 0x0200]]
        );
        assert_eq!(list.column(0), &[0x1234, 0x00FF, 0xB540]);
        assert_eq!(list.column(1), &[0x0001, 0x0200]);
    }

    #[test]
    fn a_zero_count_is_a_kept_empty_column_not_an_error() {
        // `or ax,ax / je` skips the column's pixels without consuming more of
        // the stream (`FOREST/ROUTINES.ASM:23-26`); the two count bytes are
        // still there, so the column exists and is empty.
        let bytes = [
            0x00, 0x00, // 0x0000
            0x02, 0x00, 0x10, 0x00, 0x20, 0x00, // two offsets
            0x00, 0x00, // 0x0000
        ];
        let list = PosList::parse(&bytes).expect("hidden columns are ordinary columns");
        assert_eq!(list.columns.len(), 3);
        assert_eq!(list.column(0), &[] as &[u16]);
        assert_eq!(list.column(1), &[0x0010, 0x0020]);
        assert_eq!(list.column(2), &[] as &[u16]);
    }

    #[test]
    fn a_truncated_stream_is_an_error_not_a_panic() {
        // A count of three, but only two offsets follow.
        let bytes = [0x03, 0x00, 0x34, 0x12, 0xFF, 0x00];
        let error = PosList::parse(&bytes).expect_err("the stream ends inside a column");
        assert!(
            error.contains("truncated"),
            "the error names the fault: {error}"
        );

        // A trailing byte cannot hold the next column's count.
        let bytes = [0x01, 0x00, 0x34, 0x12, 0x00];
        let error = PosList::parse(&bytes).expect_err("the stream ends inside a count");
        assert!(
            error.contains("truncated"),
            "the error names the fault: {error}"
        );
    }

    /// The real lists by the path that resolves from this crate's directory —
    /// a unit test's working directory is not the repository root, which is
    /// where the parts' own `assets/...` paths resolve (the `TUNNEL_LOCAL`
    /// precedent in `parts/tunneli.rs`).
    const FOREST_LISTS: [&str; 3] = [
        "../../assets/FOREST/POS1.DAT",
        "../../assets/FOREST/POS2.DAT",
        "../../assets/FOREST/POS3.DAT",
    ];
    const WATER_LISTS: [&str; 3] = [
        "../../assets/WATER/WAT1.DAT",
        "../../assets/WATER/WAT2.DAT",
        "../../assets/WATER/WAT3.DAT",
    ];

    /// The columns both blitters' loops fix: the product is the font's shape,
    /// not a two-dimensional layout of the file (`FOREST/ROUTINES.ASM:20` is
    /// `mov dx,237*31`; `WATER/ROUTINES.ASM:20` is `mov dx,158*34`).
    const FOREST_COLUMNS: usize = 237 * 31; // 7,347
    const WATER_COLUMNS: usize = 158 * 34; // 5,372

    struct Measured {
        columns: usize,
        positions: usize,
    }

    /// Parse one file and prove the parse accounts for every byte: a complete
    /// stream is exactly the counts and their offsets, so its length is
    /// `2*columns + 2*positions`.
    fn measure(path: &str) -> Measured {
        let bytes = std::fs::read(path).unwrap_or_else(|error| panic!("{path}: {error}"));
        let list = PosList::parse(&bytes).unwrap_or_else(|error| panic!("{path}: {error}"));
        let positions: usize = list.columns.iter().map(Vec::len).sum();
        assert_eq!(
            bytes.len(),
            2 * list.columns.len() + 2 * positions,
            "{path}: the parse did not consume the file exactly \
             ({} bytes, {} columns, {} positions)",
            bytes.len(),
            list.columns.len(),
            positions
        );
        Measured {
            columns: list.columns.len(),
            positions,
        }
    }

    #[test]
    fn the_forest_lists_measure_as_the_survey_says() {
        let measured: Vec<Measured> = FOREST_LISTS.iter().map(|path| measure(path)).collect();
        for (path, found) in FOREST_LISTS.iter().zip(&measured) {
            assert_eq!(
                found.columns, FOREST_COLUMNS,
                "{path}: the parser measured {} columns; the blitter's loop reads \
                 {FOREST_COLUMNS} (237*31, FOREST/ROUTINES.ASM:20)",
                found.columns
            );
        }
        let positions: usize = measured.iter().map(|found| found.positions).sum();
        let per_file: Vec<usize> = measured.iter().map(|found| found.positions).collect();
        assert_eq!(
            positions, 15_592,
            "MNTSCRL: the parser measured {positions} positions across POS1-3 {per_file:?}; \
             the survey says 15,592"
        );
    }

    #[test]
    fn the_water_lists_measure_as_the_survey_says() {
        // The survey's per-file position counts, in WAT1/2/3 order.
        const SURVEY: [usize; 3] = [6_176, 6_145, 8_537];
        let measured: Vec<Measured> = WATER_LISTS.iter().map(|path| measure(path)).collect();
        for ((path, found), expected) in WATER_LISTS.iter().zip(&measured).zip(SURVEY) {
            assert_eq!(
                found.columns, WATER_COLUMNS,
                "{path}: the parser measured {} columns; the blitter's loop reads \
                 {WATER_COLUMNS} (158*34, WATER/ROUTINES.ASM:20)",
                found.columns
            );
            assert_eq!(
                found.positions, expected,
                "{path}: the parser measured {} positions; the survey says {expected}",
                found.positions
            );
        }
        let positions: usize = measured.iter().map(|found| found.positions).sum();
        let per_file: Vec<usize> = measured.iter().map(|found| found.positions).collect();
        assert_eq!(
            positions, 20_858,
            "RAYSCRL: the parser measured {positions} positions across WAT1-3 {per_file:?}; \
             the survey says 20,858 (6,176 + 6,145 + 8,537)"
        );
    }
}
