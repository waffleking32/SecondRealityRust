//! FOREST's and WATER's `.DAT` position lists, against synthetic streams and the shipped files.

mod common;

use common::asset;
use sr_assets::poslist::PosList;
use sr_assets::{AssetError, Format};

#[test]
fn counts_and_offsets_are_little_endian_words() {
    let bytes = [
        0x03, 0x00, 0x34, 0x12, 0xFF, 0x00, 0x40, 0xB5, // three offsets
        0x02, 0x00, 0x01, 0x00, 0x00, 0x02, // two offsets
    ];
    let list = PosList::parse(&bytes).unwrap();
    assert_eq!(list.columns, vec![vec![0x1234, 0x00FF, 0xB540], vec![0x0001, 0x0200]]);
    assert_eq!(list.column(1), &[0x0001, 0x0200]);
}

/// `or ax,ax / je` skips a zero column's pixels, not its count word (`FOREST/ROUTINES.ASM:23-26`).
#[test]
fn a_zero_count_is_a_kept_empty_column() {
    let bytes = [0x00, 0x00, 0x02, 0x00, 0x10, 0x00, 0x20, 0x00, 0x00, 0x00];
    let list = PosList::parse(&bytes).unwrap();
    assert_eq!(list.columns, vec![vec![], vec![0x0010, 0x0020], vec![]]);
}

#[test]
fn a_truncated_stream_is_refused() {
    // A count of three with two offsets, and a trailing byte that cannot hold a count.
    for (bytes, needed, had) in [
        (&[0x03, 0x00, 0x34, 0x12, 0xFF, 0x00][..], 8, 6),
        (&[0x01, 0x00, 0x34, 0x12, 0x00][..], 6, 5),
    ] {
        let err = PosList::parse(bytes).unwrap_err();
        assert!(
            matches!(err, AssetError::Truncated { format: Format::PosList, needed: n, had: h } if n == needed && h == had),
            "{err}"
        );
    }
}

/// Every file is exactly its counts and offsets, in the column count its blitter's loop fixes
/// (`mov dx,237*31`, `FOREST/ROUTINES.ASM:21`; `mov dx,158*34`, `WATER/ROUTINES.ASM:20`).
#[test]
fn the_shipped_lists_are_the_columns_their_blitters_walk() {
    for (files, columns, positions) in [
        (["FOREST/POS1.DAT", "FOREST/POS2.DAT", "FOREST/POS3.DAT"], 237 * 31, [5_197, 5_185, 5_210]),
        (["WATER/WAT1.DAT", "WATER/WAT2.DAT", "WATER/WAT3.DAT"], 158 * 34, [6_176, 6_145, 8_537]),
    ] {
        for (file, want) in files.iter().zip(positions) {
            let bytes = asset(file);
            let list = PosList::parse(&bytes).unwrap_or_else(|e| panic!("{file}: {e}"));
            let found: usize = list.columns.iter().map(Vec::len).sum();
            assert_eq!(list.columns.len(), columns, "{file}");
            assert_eq!(found, want, "{file}: positions");
            assert_eq!(bytes.len(), 2 * columns + 2 * found, "{file}: every byte is a column");
        }
    }
}
