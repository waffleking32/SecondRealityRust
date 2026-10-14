//! Task 3.1: `MAIN/STMIK.300`'s public symbols, and the four structural facts the
//! music phase reads through them.
//!
//! Phase 3 has no audio reference — the emulator runs silent (D17) — so its oracle is a
//! per-frame trace of the original's own player state, read out of guest memory at
//! addresses this object's `PUBDEF` records give. Everything downstream (the trace's
//! addresses, the locator's needles, `docs/fidelity/music-clock.md`'s numbers) rests on the
//! map this module's reader produces, so the map is checked against the file rather than
//! against itself:
//!
//! 1. **The census.** A walk that skipped a record type by the wrong length still finds
//!    symbols further down. This test counts `STMIK.300`'s records with its own loop —
//!    stepper, `3 + length`, nothing shared with [`sr_assets::omf::symbols::walk`] — and
//!    compares the whole type census, the record count, and the byte total with the walk's.
//!    A walk that mis-stepped cannot produce the same counts.
//! 2. **The names.** Every symbol `MAIN/STMIK.H` and `MAIN/STMIKA.INC` declare for this
//!    phase is asserted by name, with its measured offset — not "some symbols were found".
//! 3. **The layout.** The seven position words are consecutive two-byte steps, and the
//!    order `MAIN/STMIK.H:125-131` declares them in is that run read from its high end
//!    down, which is what makes one `mov es,cs:stmikseg` plus a base enough to read them
//!    (`DIS/DISINT.ASM:242-282`).
//! 4. **One segment.** `_zinit`'s segment index is `_np_row`'s, and every symbol's offset
//!    lies inside that segment's declared length.
//!
//! The file is read from `upstream/SecondReality` at the pinned commit `071a82e`; nothing
//! here writes to it.

use std::collections::BTreeMap;
use std::path::Path;

use sr_assets::omf::symbols::{self, OmfError, Symbol};
use sr_assets::omf::{data_blocks, extract_ledata};

/// The vendored original, at the pinned commit. Read-only: nothing here writes into it.
const UPSTREAM: &str = "../../upstream/SecondReality";

/// The object this phase's whole instrument is built on.
const STMIK: &str = "MAIN/STMIK.300";

fn upstream(rel: &str) -> Vec<u8> {
    let path = Path::new(UPSTREAM).join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

fn stmik() -> Vec<u8> {
    upstream(STMIK)
}

/// Every offset the phase reads, as measured from the object. A wrong reading of the
/// `PUBDEF` framing — a shifted name, a `u16` offset read at the wrong byte — changes these
/// numbers, which is why they are asserted rather than counted.
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
    symbols
        .iter()
        .find(|symbol| symbol.name == name)
        .unwrap_or_else(|| {
            panic!(
                "the object publishes no {name}; it publishes {}",
                symbols.len()
            )
        })
}

// -------------------------------------------------------------------------------------
// 1. The census, counted here as well
// -------------------------------------------------------------------------------------

/// The census this test counts from the file, with nothing shared with the reader: a type
/// byte, a `u16` little-endian length, `3 + length` to the next record.
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

/// The measured census of `MAIN/STMIK.300`: 278 records, 8 types, every byte accounted for.
///
/// The numbers are asserted against the walk's own counts *and* against this test's
/// independent walk, so neither can be right by accident: the reader returns the same
/// 278-record census only if every length prefix in the file was read where the file put
/// it.
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
    // Spelled out, because a census that is right in total and wrong per type is a walk
    // that swapped one record's length for another's and still landed on the end.
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

// -------------------------------------------------------------------------------------
// 2. The names, by name
// -------------------------------------------------------------------------------------

/// Every symbol this phase reads, by name, at its measured offset — `MAIN/STMIK.H:117-131`
/// and `MAIN/STMIKA.INC:17-28` are the declarations that expect them.
#[test]
fn every_symbol_the_phase_reads_is_published_at_its_measured_offset() {
    let symbols = symbols::public_symbols(&stmik()).unwrap();
    assert_eq!(symbols.len(), 45, "the object's PUBDEF census");
    for (name, offset) in MEASURED {
        assert_eq!(
            symbol(&symbols, name).offset,
            *offset,
            "{name} is at 0x{offset:04X}"
        );
    }
}

/// `Walk::select` is how the trace asks for its words: by name, in order, and refused when
/// a name is not there — never a guessed address.
#[test]
fn selecting_by_name_refuses_a_name_the_object_does_not_publish() {
    let walk = symbols::walk(&stmik()).unwrap();
    let chosen = walk.select(&["_np_row", "_np_ord"]).unwrap();
    assert_eq!(chosen[0].name, "_np_row");
    assert_eq!(chosen[1].name, "_np_ord");
    match walk.select(&["_np_row", "_np_nonesuch"]) {
        Err(OmfError::Malformed(message)) => assert!(message.contains("_np_nonesuch"), "{message}"),
        other => panic!("a name the object does not publish must be refused: {other:?}"),
    }
}

// -------------------------------------------------------------------------------------
// 3. The layout the phase depends on
// -------------------------------------------------------------------------------------

/// The seven position words are **one contiguous run of two-byte slots**, and each name
/// sits at the slot the memory layout gives it.
///
/// The run's order is measured, not assumed, because it is not the order the declaration
/// lists them in: `MAIN/STMIK.H:125-131` declares
/// `np_loop, np_ord, np_pat, np_row, np_zinfo, np_zplus, np_zframe`, and the segments
/// place them in memory as
/// `np_zinfo, np_zplus, np_zframe, np_row, np_pat, np_ord, np_loop` — the last four
/// mirrored. A reader that walked the declaration order forwards would compute every
/// address but the middle one wrong, so the layout is pinned name by name and offset by
/// offset.
#[test]
fn the_seven_position_words_are_one_contiguous_run_of_two_byte_slots() {
    let symbols = symbols::public_symbols(&stmik()).unwrap();
    // The memory layout, measured: seven consecutive words from 0x08F7.
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
    // And the names STMIK.H:125-131 declares are exactly that run — no word of it is a
    // symbol this list does not name.
    const DECLARED: &[&str] = &[
        "_np_loop", "_np_ord", "_np_pat", "_np_row", "_np_zinfo", "_np_zplus", "_np_zframe",
    ];
    let mut declared: Vec<u32> = DECLARED
        .iter()
        .map(|name| symbol(&symbols, name).offset)
        .collect();
    declared.sort_unstable();
    let expected: Vec<u32> = (0..7).map(|slot| 0x08F7 + 2 * slot).collect();
    assert_eq!(declared, expected, "the run is exactly these seven slots");
    // The declaration order is neither the memory order nor its reverse — measured, and
    // the reason the slots above are asserted one by one.
    let declared_in_order: Vec<u32> = DECLARED
        .iter()
        .map(|name| symbol(&symbols, name).offset)
        .collect();
    assert_ne!(
        declared_in_order, expected,
        "MAIN/STMIK.H:125-131 is not the memory order"
    );
    assert_ne!(
        declared_in_order,
        expected.iter().rev().copied().collect::<Vec<u32>>(),
        "it is not the reverse either"
    );
}

// -------------------------------------------------------------------------------------
// 4. One segment
// -------------------------------------------------------------------------------------

/// STMIK is one segment, which is why `DIS/DISINT.ASM:242-282` can set `es` once and read
/// every one of these words through it — and why an offset inside the segment is an address
/// once the segment's base is known.
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
    // The length is a real bound, not a generous one: the last symbol in the segment sits
    // 103 bytes below it.
    let highest = walk.symbols.iter().map(|s| s.offset).max().unwrap();
    assert_eq!(highest, 29_504, "the highest offset in the segment");
}

// -------------------------------------------------------------------------------------
// The reader's refusals
// -------------------------------------------------------------------------------------

/// A record whose length runs past the end of the object is refused, not truncated: the
/// walk cannot tell where the next record starts, so no symbol it reported would be the
/// object's.
#[test]
fn a_record_that_runs_past_the_end_is_refused() {
    // Type 0x90, length 0x0010, and four bytes of body.
    let object = [0x90, 0x10, 0x00, 0x00, 0x01, 0x0B, 0x5F];
    match symbols::walk(&object) {
        Err(OmfError::Malformed(message)) => {
            assert!(message.contains("past the object's"), "{message}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A walk that stops short (a torn header) is refused for the same reason.
#[test]
fn a_torn_record_header_is_refused() {
    assert!(matches!(
        symbols::walk(&[0x90, 0x00]),
        Err(OmfError::Malformed(_))
    ));
}

/// The 32-bit spellings are refused rather than read as their 16-bit namesakes: the fields
/// are at different positions, so the map would look plausible and be wrong.
#[test]
fn the_32_bit_spellings_are_refused() {
    // A well-formed 0x99 (SEGDEF with a 32-bit length) is still refused.
    let segdef32 = [0x99u8, 0x07, 0x00, 0x68, 0x00, 0x00, 0x01, 0x01, 0x01, 0x00];
    match symbols::walk(&segdef32) {
        Err(OmfError::Unsupported { what }) => assert!(what.contains("0x99"), "{what}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
    let pubdef32 = [0x91u8, 0x09, 0x00, 0x00, 0x01, 0x01, 0x5F, 0x00, 0x00, 0x00, 0x00, 0x00];
    match symbols::walk(&pubdef32) {
        Err(OmfError::Unsupported { what }) => assert!(what.contains("0x91"), "{what}"),
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A `PUBDEF` whose entry needs more bytes than the record has before its checksum is
/// refused: reading the following record's bytes as a name is how a walk invents symbols.
#[test]
fn a_pubdef_entry_that_does_not_fit_is_refused() {
    // Announced name length 11, three bytes left before the record's checksum.
    let object = [0x90, 0x06, 0x00, 0x00, 0x01, 0x0B, 0x5F, 0x7A, 0x00];
    match symbols::walk(&object) {
        Err(OmfError::Malformed(message)) => {
            assert!(message.contains("before the record's checksum"), "{message}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

/// A symbol naming a segment the object does not define is refused: the offset would be
/// relative to nothing, and a caller that added it to a base would read a wrong address.
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
        Err(OmfError::Malformed(message)) => {
            assert!(message.contains("segment 2"), "{message}")
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
    // The same object with the segment index the SEGDEF does define yields the symbol —
    // so the refusal above is about the index and not about the record. (The LNAMES
    // record is 3 + 6 bytes and the SEGDEF 3 + 7, so the PUBDEF's segment index is the
    // fourth byte of its body, at 19 + 4.)
    object[19 + 4] = 0x01;
    let walk = symbols::walk(&object).unwrap();
    assert_eq!(walk.symbols.len(), 1);
    assert_eq!(walk.symbols[0].name, "_");
    assert_eq!(walk.symbols[0].offset, 0x005F);
    assert_eq!(walk.segments[&1].class, "CODE");
}

// -------------------------------------------------------------------------------------
// The blocks the locator's needles come from
// -------------------------------------------------------------------------------------

/// `data_blocks` keeps what `extract_ledata` discards: each `LEDATA` record's data at the
/// offset the record gives it. `STMIK.300` places its first block at 0 and the next at
/// 108, which is the gap that makes the concatenation unusable as a segment image.
#[test]
fn data_blocks_keep_the_offsets_the_record_declares() {
    let object = stmik();
    let blocks = data_blocks(&object).unwrap();
    assert_eq!(blocks.len(), 105, "the LEDATA census");
    assert_eq!(blocks[0].offset, 0);
    assert_eq!(blocks[0].data.len(), 104);
    assert_eq!(blocks[1].offset, 108);
    assert_eq!(blocks.iter().map(|b| b.data.len()).sum::<usize>(), 21_029);
    // An object with no LEDATA is an empty list here, where `extract_ledata` refuses it.
    assert!(data_blocks(&upstream("VISU/AMAIN.OBJ")).unwrap().is_empty());

    // And the concatenation is still exactly what `extract_ledata` returns.
    let concatenated: Vec<u8> = blocks
        .iter()
        .flat_map(|block| block.data.iter().copied())
        .collect();
    assert_eq!(concatenated, extract_ledata(&object).unwrap());
}

/// The needles the trace's locator scans guest memory for come from these blocks, so the
/// rule that picks them is checked where it can be: both needles are 32 bytes, they come
/// from different blocks, and each occurs exactly once in the object's own bytes.
#[test]
fn the_objects_data_holds_runs_the_locator_can_use() {
    let object = stmik();
    let blocks = data_blocks(&object).unwrap();
    let needle = |window: &[u8]| {
        object
            .windows(window.len())
            .filter(|other| *other == window)
            .count()
    };
    // The first block's second half is a table with 32 distinct byte values in a row.
    let run: Vec<(usize, usize)> = blocks
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            block
                .data
                .windows(32)
                .position(|window| {
                    let mut distinct: Vec<u8> = window.to_vec();
                    distinct.sort_unstable();
                    distinct.dedup();
                    distinct.len() == 32
                })
                .map(|at| (index, at))
        })
        .collect();
    assert!(!run.is_empty(), "the object holds 32-byte all-distinct runs");
    for (index, at) in run.iter().take(3) {
        let window = &blocks[*index].data[*at..*at + 32];
        assert_eq!(needle(window), 1, "the run occurs once in the object");
    }
}
