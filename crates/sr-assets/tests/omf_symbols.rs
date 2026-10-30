//! `MAIN/STMIK.300`'s public symbols, which `xtask music` reads the original player's state
//! through (`music-clock.md`). The map is checked against the file: the record census by an
//! independent walk, every symbol by name at its measured offset, the seven position words as
//! one run of slots, and the one segment.

mod common;

use std::collections::BTreeMap;

use common::upstream;
use sr_assets::omf::symbols::{self, Symbol};
use sr_assets::omf::{data_blocks, extract_ledata};
use sr_assets::{AssetError, Format};

fn stmik() -> Vec<u8> {
    upstream("MAIN/STMIK.300")
}

/// Every offset the music tooling reads, as measured from the object.
const MEASURED: &[(&str, u32)] = &[
    ("_np_loop", 0x0903),
    ("_np_ord", 0x0901),
    ("_np_pat", 0x08FF),
    ("_np_row", 0x08FD),
    ("_np_zinfo", 0x08F7),
    ("_np_zplus", 0x08F9),
    ("_np_zframe", 0x08FB),
    ("_np_speed", 0x2002),
    ("_np_tempo", 0x090F),
    ("_np_mastermul", 0x0916),
    ("_zoutputmode", 0x0882),
    ("_zmixspeed", 0x08D5),
    ("_zpollmix", 0x08C0),
    ("_zbufsize", 0x08A7),
    ("_zspeedadjust", 0x08CD),
    ("_zspeedadjust2", 0x08D1),
    ("_zinit", 0x2324),
    ("_zpollme", 0x261B),
];

fn symbol<'a>(symbols: &'a [Symbol], name: &str) -> &'a Symbol {
    symbols.iter().find(|symbol| symbol.name == name).unwrap_or_else(|| panic!("no {name}"))
}

/// The census counted here, sharing nothing with the reader: `3 + length` to the next record.
fn census(object: &[u8]) -> (BTreeMap<u8, usize>, usize, usize) {
    let mut types: BTreeMap<u8, usize> = BTreeMap::new();
    let mut pos = 0usize;
    while pos < object.len() {
        assert!(object.len() - pos >= 3, "a torn record header at {pos}");
        *types.entry(object[pos]).or_insert(0) += 1;
        let len = usize::from(u16::from_le_bytes([object[pos + 1], object[pos + 2]]));
        pos += 3 + len;
        assert!(pos <= object.len(), "a record overran the object");
    }
    (types, pos, object.len())
}

/// `MAIN/STMIK.300`'s census: 278 records of 8 types, every byte accounted for, by the reader
/// and by the walk above.
#[test]
fn the_record_census_is_counted_from_the_file() {
    let object = stmik();
    let (types, ended, length) = census(&object);
    assert_eq!(length, 24_849, "the object's size");
    assert_eq!(ended, 24_849, "the walk by 3 + length ends on the last byte");

    let walk = symbols::walk(&object).unwrap();
    assert_eq!(walk.types, types, "the reader's census is the file's");
    assert_eq!(walk.records(), 278, "the measured record count");
    assert_eq!(walk.bytes, 24_849, "3 + length over every record is the file");
    assert_eq!(types.get(&0x80), Some(&1), "THEADR");
    assert_eq!(types.get(&0x88), Some(&5), "COMENT");
    assert_eq!(types.get(&0x8A), Some(&1), "MODEND");
    assert_eq!(types.get(&0x90), Some(&45), "PUBDEF");
    assert_eq!(types.get(&0x96), Some(&2), "LNAMES");
    assert_eq!(types.get(&0x98), Some(&1), "SEGDEF");
    assert_eq!(types.get(&0xA0), Some(&105), "LEDATA");
    assert_eq!(types.get(&0xA2), Some(&118), "LIDATA");
    assert_eq!(types.len(), 8, "the object holds eight record types, no more");
}

/// Every symbol `MAIN/STMIK.H:117-131` and `MAIN/STMIKA.INC:17-28` declare, at its offset.
#[test]
fn every_symbol_the_music_tooling_reads_is_published_at_its_measured_offset() {
    let symbols = symbols::walk(&stmik()).unwrap().symbols;
    assert_eq!(symbols.len(), 45, "the object's PUBDEF census");
    for (name, offset) in MEASURED {
        assert_eq!(
            symbol(&symbols, name).offset,
            *offset,
            "{name} is at 0x{offset:04X}"
        );
    }
}

/// The seven position words are one run of two-byte slots from 0x08F7, in an order that is not
/// `MAIN/STMIK.H:125-131`'s declaration order, so each slot is pinned by name.
#[test]
fn the_seven_position_words_are_one_contiguous_run_of_two_byte_slots() {
    let symbols = symbols::walk(&stmik()).unwrap().symbols;
    const IN_MEMORY: &[&str] = &[
        "_np_zinfo", "_np_zplus", "_np_zframe", "_np_row", "_np_pat", "_np_ord", "_np_loop",
    ];
    for (slot, name) in IN_MEMORY.iter().enumerate() {
        assert_eq!(
            symbol(&symbols, name).offset,
            0x08F7 + 2 * slot as u32,
            "{name} is slot {slot} of the run"
        );
    }
}

/// STMIK is one segment, which is why `DIS/DISINT.ASM:242-282` sets `es` once and reads every
/// word through it.
#[test]
fn every_symbol_lives_in_the_one_segment_and_inside_its_declared_length() {
    let object = stmik();
    let walk = symbols::walk(&object).unwrap();
    assert_eq!(walk.segments.len(), 1, "the object defines one segment");
    let (index, segment) = walk.segments.iter().next().unwrap();
    assert_eq!(*index, 1, "the one SEGDEF is index 1, the index PUBDEFs write");
    assert_eq!(segment.name, "text_stmik300_text");
    assert_eq!(segment.class, "CODE");
    assert_eq!(segment.length, 29_607, "the measured declared length");
    assert_eq!(segment.acbp, 0x68);

    assert_eq!(
        symbol(&walk.symbols, "_zinit").segment,
        symbol(&walk.symbols, "_np_row").segment,
        "_zinit and _np_row are in one segment"
    );
    for entry in &walk.symbols {
        assert_eq!(entry.segment, *index, "{} is in the one segment", entry.name);
        assert!(
            entry.offset < segment.length,
            "{} is at {} and the segment is {} bytes",
            entry.name,
            entry.offset,
            segment.length
        );
    }
    let highest = walk.symbols.iter().map(|s| s.offset).max().unwrap();
    assert_eq!(highest, 29_504, "the highest offset in the segment");
}

/// A record whose length runs past the object, or a torn header, refuses the whole walk: a
/// mis-stepped walk would still find symbols.
#[test]
fn a_record_that_runs_past_the_end_is_refused() {
    // Type 0x90, length 0x0010, and four bytes of body.
    let object = [0x90, 0x10, 0x00, 0x00, 0x01, 0x0B, 0x5F];
    assert!(matches!(
        symbols::walk(&object),
        Err(AssetError::Truncated { format: Format::Omf, needed: 19, had: 7 })
    ));
    assert!(matches!(symbols::walk(&[0x90, 0x00]), Err(AssetError::Truncated { needed: 3, had: 2, .. })));
}

/// The 32-bit spellings are refused rather than read as their 16-bit namesakes.
#[test]
fn the_32_bit_spellings_are_refused() {
    // A well-formed 0x99 (SEGDEF with a 32-bit length) is still refused.
    let segdef32 = [0x99u8, 0x07, 0x00, 0x68, 0x00, 0x00, 0x01, 0x01, 0x01, 0x00];
    match symbols::walk(&segdef32) {
        Err(AssetError::Unsupported { what, .. }) => assert!(what.contains("0x99"), "{what}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    let pubdef32 = [0x91u8, 0x09, 0x00, 0x00, 0x01, 0x01, 0x5F, 0x00, 0x00, 0x00, 0x00, 0x00];
    match symbols::walk(&pubdef32) {
        Err(AssetError::Unsupported { what, .. }) => assert!(what.contains("0x91"), "{what}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A `PUBDEF` entry that does not fit before its record's checksum is refused.
#[test]
fn a_pubdef_entry_that_does_not_fit_is_refused() {
    // Announced name length 11, three bytes left before the record's checksum.
    let object = [0x90, 0x06, 0x00, 0x00, 0x01, 0x0B, 0x5F, 0x7A, 0x00];
    match symbols::walk(&object) {
        Err(AssetError::Unsupported { what: message, .. }) => {
            assert!(message.contains("before the record's checksum"), "{message}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A symbol naming a segment the object does not define is refused; with the defined index, the
/// same object yields the symbol.
#[test]
fn a_symbol_in_a_segment_the_object_does_not_define_is_refused() {
    let mut object = Vec::new();
    // LNAMES: one name, "CODE" (index 1).
    object.extend_from_slice(&[0x96, 0x06, 0x00, 0x04, b'C', b'O', b'D', b'E', 0x00]);
    // SEGDEF: ACBP 0x68, length 0x0010, name/class/overlay index 1 — the only segment.
    object.extend_from_slice(&[0x98, 0x07, 0x00, 0x68, 0x10, 0x00, 0x01, 0x01, 0x01, 0x00]);
    // PUBDEF: group 0, segment 2 (which the object does not define), one symbol.
    object.extend_from_slice(&[0x90, 0x08, 0x00, 0x00, 0x02, 0x01, 0x5F, 0x5F, 0x00, 0x00, 0x00]);
    match symbols::walk(&object) {
        Err(AssetError::Unsupported { what: message, .. }) => {
            assert!(message.contains("segment 2"), "{message}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    // The PUBDEF starts at 9 + 10 = 19; its segment index is body byte 1, at 19 + 4.
    object[19 + 4] = 0x01;
    let walk = symbols::walk(&object).unwrap();
    assert_eq!(walk.symbols.len(), 1);
    assert_eq!(walk.symbols[0].name, "_");
    assert_eq!(walk.symbols[0].offset, 0x005F);
    assert_eq!(walk.segments[&1].class, "CODE");
}

/// The static values `music-clock.md` §2 and §5 name, read out of the object's own `LEDATA` image
/// at its symbols' offsets: the default tempo (125), master volume (192) and speed (6).
#[test]
fn the_objects_own_initialisers_are_the_notes_values() {
    let object = stmik();
    let symbols = symbols::walk(&object).unwrap().symbols;
    let mut image: Vec<Option<u8>> = vec![None; 29_607];
    for block in data_blocks(&object).unwrap() {
        for (at, byte) in block.data.iter().enumerate() {
            image[block.offset as usize + at] = Some(*byte);
        }
    }
    let byte_at = |name: &str| {
        let at = symbol(&symbols, name).offset as usize;
        image[at].unwrap_or_else(|| panic!("{name} at 0x{at:04X} is not in any LEDATA record"))
    };
    assert_eq!(byte_at("_np_tempo"), 0x7D, "the player's default tempo, 125");
    assert_eq!(byte_at("_np_mastervol"), 0xC0, "the player's default master volume, 192");
    assert_eq!(byte_at("_np_speed"), 6, "the player's default speed");
}

/// `data_blocks` keeps each `LEDATA` record's offset: `STMIK.300`'s first block is at 0 and the
/// next at 108, a gap the concatenation loses.
#[test]
fn data_blocks_keep_the_offsets_the_record_declares() {
    let object = stmik();
    let blocks = data_blocks(&object).unwrap();
    assert_eq!(blocks.len(), 105, "the LEDATA census");
    assert_eq!(blocks[0].offset, 0);
    assert_eq!(blocks[0].data.len(), 104);
    assert_eq!(blocks[1].offset, 108);
    assert_eq!(blocks.iter().map(|b| b.data.len()).sum::<usize>(), 21_029);
    assert!(data_blocks(&upstream("VISU/AMAIN.OBJ")).unwrap().is_empty());

    let concatenated: Vec<u8> = blocks
        .iter()
        .flat_map(|block| block.data.iter().copied())
        .collect();
    assert_eq!(concatenated, extract_ledata(&object).unwrap());
}
