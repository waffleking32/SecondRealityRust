//! The two shipped modules, decoded: `assets/MAIN/MUSIC0.S3M` and `MUSIC1.S3M` in their
//! scrambled form, against numbers this file computes itself.
//!
//! Two things here are independent of the loader on purpose:
//!
//! 1. **Its own keystream and walker.** [`Walk`] re-implements D5's formula and the packed-cell
//!    stream from the file's flag bits; every cell the loader produced is compared with the
//!    walker's, and the walker shows that 64 rows consume exactly a block's data.
//! 2. **Its own pointer arithmetic.** The tiling test reads the pattern pointer table out of
//!    the raw bytes and computes the gaps in both readings of the block's length word; the
//!    other reading fails 8 and 11 pairs.

mod common;

use std::collections::BTreeMap;

use common::shipped;
use sr_audio::module::SAMPLE_MAGIC;
use sr_audio::{Cell, Module, CHANNELS, ROWS};

/// MUSIC0's shipped size in bytes, `assets/PROVENANCE.md`'s row for the file.
const MUSIC0_SIZE: usize = 391_312;

/// MUSIC1's shipped size in bytes.
const MUSIC1_SIZE: usize = 600_860;

/// A module's bytes and the loader's reading of them.
struct Loaded {
    name: &'static str,
    bytes: Vec<u8>,
    module: Module,
}

/// `name` is the file's (`MUSIC0.S3M`), which the output and the assertions quote.
fn load(name: &'static str) -> Loaded {
    let bytes = shipped(name.trim_end_matches(".S3M"));
    let module = Module::load(&bytes).unwrap_or_else(|e| panic!("{name} does not load: {e}"));
    Loaded {
        name,
        bytes,
        module,
    }
}

fn music0() -> Loaded {
    load("MUSIC0.S3M")
}

fn music1() -> Loaded {
    load("MUSIC1.S3M")
}

fn both() -> [Loaded; 2] {
    [music0(), music1()]
}

// ---------------------------------------------------------------------------------------
// This test's own reader: D5's keystream, and the packed-cell stream, written again
// ---------------------------------------------------------------------------------------

/// D5's keystream, from the formula as D5 states it, in this file's own words.
fn key(i: usize) -> u8 {
    let n = (i >> 1) + 1;
    ((((n ^ (n >> 2)) << 3) | ((5 * i + 2) & 7)) & 0xFF) as u8
}

/// One cell as this file's walker reads it — no `sr_audio` type anywhere in the path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Raw {
    channel: u8,
    note: u8,
    instrument: u8,
    volume: u8,
    command: u8,
    info: u8,
    /// The flag byte's three "what follows" bits, as the file wrote them.
    present: u8,
}

/// One pattern as this file's walker reads it: 64 rows of the cells the packed stream
/// named, in the order it named them.
struct Walk {
    rows: Vec<Vec<Raw>>,
}

impl Walk {
    /// Walk one pattern's descrambled packed data — the `ln - 2` bytes at `ptr * 16 + 2`.
    fn new(data: &[u8]) -> Result<Walk, String> {
        let mut plain = data.to_vec();
        for (i, byte) in plain.iter_mut().enumerate() {
            *byte ^= key(i);
        }

        let mut rows = Vec::with_capacity(ROWS);
        let mut at = 0usize;
        for row in 0..ROWS {
            let mut cells = Vec::new();
            loop {
                let flag = *plain
                    .get(at)
                    .ok_or_else(|| format!("row {row}: the data ran out"))?;
                at += 1;
                // A row ends on a zero byte. The low five bits are the channel, so
                // `flag & 0x1f == 0` would end the row at every channel-0 cell instead.
                if flag == 0 {
                    break;
                }
                let mut cell = Raw {
                    channel: flag & 0x1F,
                    note: 0,
                    instrument: 0,
                    volume: 0,
                    command: 0,
                    info: 0,
                    present: flag & 0xE0,
                };
                if flag & 0x20 != 0 {
                    cell.note = *plain
                        .get(at)
                        .ok_or_else(|| format!("row {row}: no note byte"))?;
                    cell.instrument = *plain
                        .get(at + 1)
                        .ok_or_else(|| format!("row {row}: no instrument byte"))?;
                    at += 2;
                }
                if flag & 0x40 != 0 {
                    cell.volume = *plain
                        .get(at)
                        .ok_or_else(|| format!("row {row}: no volume byte"))?;
                    at += 1;
                }
                if flag & 0x80 != 0 {
                    cell.command = *plain
                        .get(at)
                        .ok_or_else(|| format!("row {row}: no command byte"))?;
                    cell.info = *plain
                        .get(at + 1)
                        .ok_or_else(|| format!("row {row}: no info byte"))?;
                    at += 2;
                }
                cells.push(cell);
            }
            rows.push(cells);
        }
        if at != plain.len() {
            return Err(format!(
                "64 rows consumed {at} of the block's {} bytes",
                plain.len()
            ));
        }
        Ok(Walk { rows })
    }

    fn cells(&self) -> usize {
        self.rows.iter().map(Vec::len).sum()
    }
}

fn word(bytes: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([bytes[at], bytes[at + 1]])
}

/// The pattern pointer table's location, a **byte** offset: `0x60 + ordNum + insNum * 2`
/// (`STARTEMS.C:333-334`; `module` is `char far *`, and `parains`/`parapat` are `int *`,
/// so `parapat = parains + insNum` adds `insNum × 2` bytes). Each *entry* is a paragraph
/// number — `parapat[a] << 4` gives the file offset (`STARTEMS.C:345-347`).
fn pattern_table(loaded: &Loaded) -> Vec<u16> {
    let orders = usize::from(word(&loaded.bytes, 32));
    let instruments = usize::from(word(&loaded.bytes, 34));
    let patterns = usize::from(word(&loaded.bytes, 36));
    let at = 0x60 + orders + 2 * instruments;
    assert!(
        at + 2 * patterns <= loaded.bytes.len(),
        "the pointer table runs past the file"
    );
    (0..patterns)
        .map(|index| word(&loaded.bytes, at + 2 * index))
        .collect()
}

/// A pattern's block: its length word and its still-scrambled data, read from the raw file.
fn block(loaded: &Loaded, pointer: u16) -> (usize, Vec<u8>) {
    let at = usize::from(pointer) * 16;
    let length = usize::from(word(&loaded.bytes, at));
    (length, loaded.bytes[at + 2..at + length].to_vec())
}

/// Every pattern's walk, in pointer-table order.
fn walks(loaded: &Loaded) -> Vec<Walk> {
    pattern_table(loaded)
        .iter()
        .map(|pointer| {
            let (_, data) = block(loaded, *pointer);
            Walk::new(&data).unwrap_or_else(|e| panic!("{}: {e}", loaded.name))
        })
        .collect()
}

/// How many channel slots the file's packed streams actually name.
fn named_cells(loaded: &Loaded) -> usize {
    walks(loaded).iter().map(Walk::cells).sum()
}

// ---------------------------------------------------------------------------------------
// 1. The files, and the header
// ---------------------------------------------------------------------------------------

#[test]
fn the_shipped_files_are_the_ones_this_test_measured() {
    // Each module names itself in the header's first 28 bytes; the two strings differ, which
    // is one more way to tell the files apart by their own contents.
    fn padded(text: &[u8]) -> [u8; 28] {
        let mut title = [0u8; 28];
        title[..text.len()].copy_from_slice(text);
        title
    }
    for (loaded, size, title) in [
        (
            music0(),
            MUSIC0_SIZE,
            padded(b"UnreaL ][ - The 2ND Reality"),
        ),
        (music1(), MUSIC1_SIZE, padded(b"UnreaL ][ / PM ")),
    ] {
        println!(
            "{}: {} bytes, signature {:?} at 44, marker {} at 28, type {} at 29, title {:?}",
            loaded.name,
            loaded.bytes.len(),
            std::str::from_utf8(&loaded.bytes[44..48]).expect("ascii"),
            loaded.bytes[28],
            loaded.bytes[29],
            String::from_utf8_lossy(&loaded.bytes[..28]),
        );
        assert_eq!(loaded.bytes.len(), size, "{}'s shipped size", loaded.name);
        assert_eq!(&loaded.bytes[44..48], b"SCRM", "the S3M signature");
        assert_eq!(&loaded.bytes[..28], &title[..], "the module's own title");
        assert_eq!(loaded.bytes[28], 26, "the marker byte the format fixes");
        assert_eq!(loaded.bytes[29], 16, "type 16: a module, not a song");
    }
}

#[test]
fn the_header_is_read_field_by_field() {
    // The header's own values, in the order `struct ds_fileheader` declares them
    // (`STARTEMS.C:56-67`). The order list's *length* is checked here; its contents are the
    // next test's.
    let pan: [u8; 32] = [
        0, 8, 1, 9, 2, 10, 3, 11, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
        255, 255, 255, 255, 255, 255, 255, 255, 255, 255, 255,
    ];
    for (loaded, orders, instruments, patterns, speed, tempo) in [
        (music0(), 80usize, 44usize, 77usize, 6u8, 125u8),
        (music1(), 100usize, 54usize, 77usize, 3u8, 130u8),
    ] {
        println!(
            "{}: ordNum {orders} insNum {instruments} patNum {patterns}, global volume {}, \
             initial speed {speed}, initial tempo {tempo}, master volume {} (stereo {}, {} \
             channels enabled), channel table {:?}",
            loaded.name,
            loaded.module.global_volume(),
            loaded.module.master_volume(),
            loaded.module.stereo(),
            loaded
                .module
                .channel_pan()
                .iter()
                .filter(|c| **c != 255)
                .count(),
            loaded.module.channel_pan(),
        );

        assert_eq!(word(&loaded.bytes, 32) as usize, orders, "ordNum at 32");
        assert_eq!(
            word(&loaded.bytes, 34) as usize,
            instruments,
            "insNum at 34"
        );
        assert_eq!(word(&loaded.bytes, 36) as usize, patterns, "patNum at 36");
        assert_eq!(loaded.module.order_count(), orders);
        assert_eq!(loaded.module.instrument_count(), instruments);
        assert_eq!(loaded.module.pattern_count(), patterns);
        assert_eq!(loaded.module.global_volume(), 64, "the byte at 48");
        assert_eq!(loaded.module.initial_speed(), speed, "the byte at 49");
        assert_eq!(loaded.module.initial_tempo(), tempo, "the byte at 50");
        assert_eq!(loaded.module.master_volume(), 176, "the byte at 51");
        assert!(loaded.module.stereo(), "0xB0's stereo bit is set");
        assert_eq!(loaded.module.channel_pan(), &pan, "the channel table");
        assert_eq!(&loaded.bytes[64..96], &pan, "which lives at 64..96");
    }
}

#[test]
fn the_order_list_is_preserved_byte_for_byte() {
    // The order list as the file wrote it: no normalisation, no filtering, 254 and 255 kept
    // as the bytes they are (the player's handling is `tests/player.rs`'s).
    let music0_orders: &[u8] = &[
        0, 1, 9, 10, 19, 2, 3, 4, 5, 6, 7, 8, 11, 13, 31, 31, 255, 255, 50, 51, 49, 48, 52, 72, 47,
        70, 71, 254, 14, 15, 16, 17, 28, 21, 24, 30, 23, 20, 20, 35, 35, 29, 36, 39, 40, 41, 42,
        43, 44, 55, 56, 57, 58, 59, 60, 45, 61, 68, 20, 20, 33, 33, 24, 69, 255, 25, 26, 53, 54,
        255, 63, 64, 65, 66, 67, 45, 61, 62, 255, 255,
    ];
    let music1_orders: &[u8] = &[
        73, 39, 50, 254, 26, 28, 0, 27, 29, 27, 29, 1, 65, 5, 254, 2, 2, 34, 3, 254, 9, 17, 4, 7,
        8, 254, 13, 254, 10, 10, 11, 10, 12, 14, 47, 46, 15, 16, 254, 18, 41, 254, 30, 31, 32, 33,
        254, 51, 52, 254, 19, 24, 21, 20, 25, 22, 38, 6, 23, 54, 55, 254, 43, 44, 48, 49, 66, 254,
        56, 67, 40, 40, 53, 64, 58, 62, 254, 57, 42, 61, 61, 59, 45, 61, 61, 60, 60, 254, 35, 36,
        37, 37, 254, 68, 69, 70, 71, 254, 72, 255,
    ];

    for (loaded, orders) in [(music0(), music0_orders), (music1(), music1_orders)] {
        let at = |byte: u8| -> Vec<usize> {
            orders
                .iter()
                .enumerate()
                .filter(|(_, order)| **order == byte)
                .map(|(index, _)| index)
                .collect()
        };
        println!(
            "{}: {} orders; 255 at {:?}; 254 at {:?}",
            loaded.name,
            orders.len(),
            at(255),
            at(254),
        );

        assert_eq!(
            loaded.module.orders(),
            orders,
            "{}'s order list",
            loaded.name
        );
        assert_eq!(
            &loaded.bytes[0x60..0x60 + orders.len()],
            orders,
            "and it is the file's own bytes from 0x60"
        );
        assert_eq!(
            loaded.module.orders().len(),
            word(&loaded.bytes, 32) as usize,
            "ordNum is its length"
        );
    }

    // The 254 and 255 positions.
    let at = |orders: &[u8], byte: u8| -> Vec<usize> {
        orders
            .iter()
            .enumerate()
            .filter(|(_, order)| **order == byte)
            .map(|(index, _)| index)
            .collect()
    };
    assert_eq!(
        at(music0_orders, 255),
        vec![16, 17, 64, 69, 78, 79],
        "MUSIC0's six"
    );
    assert_eq!(at(music0_orders, 254), vec![27], "MUSIC0's one");
    assert_eq!(at(music1_orders, 254).len(), 15, "MUSIC1's fifteen");
    assert_eq!(at(music1_orders, 255), vec![99], "MUSIC1's one, last");
}

// ---------------------------------------------------------------------------------------
// 2. The patterns: the pointer table, the tiling, and the two decoders' agreement
// ---------------------------------------------------------------------------------------

#[test]
fn the_pattern_blocks_tile_on_paragraph_boundaries() {
    let mut failures: Vec<(&str, usize)> = Vec::new();
    for loaded in both() {
        let pointers = pattern_table(&loaded);
        // Where the two tables are: `ordNum` bytes of order list from 0x60, then `insNum`
        // instrument pointers, then `patNum` pattern pointers (`STARTEMS.C:333-334`). The
        // blocks all come after them, which is what makes the pointers' paragraph numbers
        // readable as file offsets at all (`<< 4`).
        let orders = usize::from(word(&loaded.bytes, 32));
        let instruments = usize::from(word(&loaded.bytes, 34));
        let patterns_at = 0x60 + orders + 2 * instruments;
        println!(
            "{}: {} orders at 0x60, {instruments} instrument pointers at {}, {} pattern \
             pointers at {}..{}, the first pattern block at byte {}",
            loaded.name,
            orders,
            0x60 + orders,
            pointers.len(),
            patterns_at,
            patterns_at + 2 * pointers.len(),
            usize::from(pointers[0]) * 16,
        );
        assert_eq!(
            word(&loaded.bytes, patterns_at),
            pointers[0],
            "{}: the pattern table starts where the header says",
            loaded.name
        );
        assert!(
            usize::from(pointers[0]) * 16 >= patterns_at + 2 * pointers.len(),
            "{}: the blocks come after the tables, not over them",
            loaded.name
        );

        let mut padding: BTreeMap<i64, usize> = BTreeMap::new();
        let mut other_reading_fails = 0usize;
        let mut last_end = 0usize;

        for pair in pointers.windows(2) {
            let at = usize::from(pair[0]) * 16;
            let length = usize::from(word(&loaded.bytes, at));
            let end = at + length;
            // The reading the loader uses: `ln` includes its own two bytes, so the data is
            // `ln - 2` bytes and the block ends here.
            *padding.entry(pair[1] as i64 * 16 - end as i64).or_insert(0) += 1;
            // The other reading: `ln` taken as the data's own length, which puts the block's
            // end two bytes further on and stops these pairs tiling.
            if !(0..=15).contains(&(pair[1] as i64 * 16 - (end as i64 + 2))) {
                other_reading_fails += 1;
            }
            last_end = end;
        }

        let present: Vec<i64> = (0..16).filter(|p| padding.contains_key(p)).collect();
        println!(
            "{}: {} patterns, {} consecutive pairs; the next block begins 0..15 bytes after \
             ptr*16 + ln, present sizes {present:?}, every gap {:?}",
            loaded.name,
            pointers.len(),
            pointers.len() - 1,
            padding,
        );
        println!(
            "{}: reading `ln` as the data's length instead breaks {other_reading_fails} of \
             those {} pairs",
            loaded.name,
            pointers.len() - 1,
        );

        assert_eq!(pointers.len(), 77, "{} patterns", loaded.name);
        assert_eq!(
            present.len(),
            16,
            "{}: every padding size 0..=15 occurs, and these do not: {padding:?}",
            loaded.name
        );
        assert!(
            padding.keys().all(|gap| (0..=15).contains(gap)),
            "{}: every pair tiles within 0..=15 bytes: {padding:?}",
            loaded.name
        );
        assert!(
            last_end < loaded.bytes.len(),
            "{}: the last pattern ends inside the file",
            loaded.name
        );
        failures.push((loaded.name, other_reading_fails));
    }

    // The counter-claim, in numbers: the other reading is not merely "also possible", it
    // fails this many times in each module.
    assert_eq!(failures, vec![("MUSIC0.S3M", 8), ("MUSIC1.S3M", 11)]);
}

#[test]
fn every_pattern_decodes_to_exactly_sixty_four_rows() {
    // The loader refuses a pattern that does not, so this file's own walker does the
    // counting — and every cell the loader produced is compared with the walker's, slot by
    // slot, which is what makes the two readers one statement.
    for loaded in both() {
        let patterns = walks(&loaded);
        let mut cells = 0usize;

        for (index, walk) in patterns.iter().enumerate() {
            let (length, data) = block(&loaded, pattern_table(&loaded)[index]);
            let pattern = loaded
                .module
                .pattern(index)
                .unwrap_or_else(|| panic!("{} has no pattern {index}", loaded.name));
            assert_eq!(pattern.rows.len(), ROWS, "{} pattern {index}", loaded.name);
            assert_eq!(data.len(), length - 2, "the data is ln - 2 bytes");

            let mut highest = None;
            for (row, named) in walk.rows.iter().enumerate() {
                let mut row_cells = 0usize;
                for cell in named {
                    let slot = usize::from(cell.channel);
                    assert_eq!(
                        pattern.rows[row][slot],
                        Cell {
                            note: cell.note,
                            instrument: cell.instrument,
                            volume: cell.volume,
                            command: cell.command,
                            info: cell.info,
                            present: cell.present,
                        },
                        "{} pattern {index} row {row} channel {slot}",
                        loaded.name
                    );
                    highest = Some(highest.map_or(cell.channel, |h: u8| h.max(cell.channel)));
                    row_cells += 1;
                }
                assert_eq!(
                    pattern.rows[row]
                        .iter()
                        .filter(|cell| !cell.is_empty())
                        .count(),
                    row_cells,
                    "{} pattern {index} row {row}: the row names exactly the cells the file does",
                    loaded.name
                );
                for (slot, cell) in pattern.rows[row].iter().enumerate() {
                    if !named.iter().any(|raw| usize::from(raw.channel) == slot) {
                        assert_eq!(*cell, Cell::EMPTY, "{} pattern {index} row {row}", loaded.name);
                    }
                }
            }
            assert_eq!(
                pattern.channel_count,
                highest.map_or(0, |h| h + 1),
                "{} pattern {index}: the count is one past the highest channel named",
                loaded.name
            );
            cells += walk.cells();
        }

        println!(
            "{}: {} of {} patterns decode as exactly 64 rows consuming exactly their data; \
             {cells} cells in all",
            loaded.name,
            patterns.len(),
            loaded.module.pattern_count(),
        );
        assert_eq!(patterns.len(), 77);
        assert_eq!(cells, named_cells(&loaded));
    }
}

#[test]
fn every_row_names_each_channel_at_most_once_and_in_order() {
    // A row is 32 slots and the file names each cell's channel in its flag byte, so placing
    // cells by that number is lossless exactly when no row names a channel twice — the case
    // the loader refuses. This is the measurement that says it is not hiding in either
    // module, and that the channels come in ascending order, so "the stream's order" and
    // "the channel order" are the same order.
    // The highest channel each module names: MUSIC0 stays inside its eight declared
    // channels, MUSIC1 reaches channel 14 — the off-channel cells D8 records. The count of
    // channel-0 cells is here too, because it is the number of cells a decoder that ended a
    // row on `flag & 0x1f == 0` would swallow as terminators.
    for (loaded, expected) in [(music0(), 7u8), (music1(), 14u8)] {
        let patterns = walks(&loaded);
        let mut rows = 0usize;
        let mut widest = 0usize;
        let mut highest = 0u8;
        let mut channel_zero = 0usize;

        for walk in &patterns {
            for named in &walk.rows {
                rows += 1;
                widest = widest.max(named.len());
                let channels: Vec<u8> = named.iter().map(|cell| cell.channel).collect();
                assert!(
                    channels.windows(2).all(|pair| pair[0] < pair[1]),
                    "{}: a row names its channels twice or out of order: {channels:?}",
                    loaded.name
                );
                channel_zero += channels.iter().filter(|channel| **channel == 0).count();
                if let Some(last) = channels.last() {
                    highest = highest.max(*last);
                }
            }
        }

        println!(
            "{}: {rows} rows, {widest} cells in the widest row, highest channel {highest}, \
             {channel_zero} cells on channel 0, no row names a channel twice",
            loaded.name
        );
        assert_eq!(rows, 77 * ROWS);
        assert!(widest <= CHANNELS, "a row cannot hold more than 32 cells");
        assert_eq!(highest, expected, "{}'s highest channel", loaded.name);
        assert!(
            channel_zero > 0,
            "{}: channel-0 cells exist, so `flag & 0x1f == 0` is not the row terminator",
            loaded.name
        );
    }
}

// ---------------------------------------------------------------------------------------
// 3. The cells: the effect alphabet, the histogram, and the notes
// ---------------------------------------------------------------------------------------

/// The ST3 numeric alphabet (design §7.1): 1..=26 is A..Z, 0x30..=0x39 is the digits, 255 is
/// "none", and 0 is the second spelling of "no command" that D5 records.
fn alphabet(command: u8) -> String {
    match command {
        0 => "0(none)".to_string(),
        1..=26 => char::from(b'A' + command - 1).to_string(),
        0x30..=0x39 => char::from(command).to_string(),
        255 => "255(none)".to_string(),
        other => format!("{other}(invalid)"),
    }
}

/// How many of a module's decoded cells carry a given effect letter.
fn count(histogram: &BTreeMap<u8, usize>, letter: char) -> usize {
    histogram
        .get(&(letter as u8 - b'A' + 1))
        .copied()
        .unwrap_or(0)
}

/// The command histogram of a loaded module, counted from its decoded cells.
fn histogram_of(loaded: &Loaded) -> BTreeMap<u8, usize> {
    let mut histogram = BTreeMap::new();
    for index in 0..loaded.module.pattern_count() {
        for cell in loaded
            .module
            .pattern(index)
            .expect("a pattern")
            .rows
            .iter()
            .flatten()
        {
            if cell.has_command() {
                *histogram.entry(cell.command).or_insert(0) += 1;
            }
        }
    }
    histogram
}

#[test]
fn the_effect_histogram_is_the_modules_own() {
    // An effect the demo never plays is measured absent, not assumed absent. Counted here from the loader's decoded cells and
    // cross-checked against this file's own walker.
    for loaded in both() {
        let mut histogram: BTreeMap<u8, usize> = BTreeMap::new();
        let mut from_walker: BTreeMap<u8, usize> = BTreeMap::new();
        let mut command_zero: Vec<(usize, usize, u8)> = Vec::new();
        let mut command_255 = 0usize;
        let mut invalid: Vec<(usize, usize, u8, u8)> = Vec::new();
        let mut no_command = 0usize;

        for walk in walks(&loaded) {
            for cell in walk.rows.iter().flatten() {
                if cell.present & 0x80 != 0 {
                    *from_walker.entry(cell.command).or_insert(0) += 1;
                }
            }
        }

        for index in 0..loaded.module.pattern_count() {
            let pattern = loaded.module.pattern(index).expect("a pattern");
            for (row, cells) in pattern.rows.iter().enumerate() {
                for (channel, cell) in cells.iter().enumerate() {
                    if !cell.has_command() {
                        no_command += 1;
                        continue;
                    }
                    *histogram.entry(cell.command).or_insert(0) += 1;
                    match cell.command {
                        0 => command_zero.push((index, row, channel as u8)),
                        255 => command_255 += 1,
                        1..=26 | 0x30..=0x39 => {}
                        other => invalid.push((index, row, channel as u8, other)),
                    }
                }
            }
        }

        let letters: Vec<String> = histogram
            .iter()
            .map(|(command, count)| format!("{}:{count}", alphabet(*command)))
            .collect();
        println!(
            "{}: {} channel slots, {} of them carrying no command byte; the rest: {}",
            loaded.name,
            ROWS * CHANNELS * loaded.module.pattern_count(),
            no_command,
            letters.join(", "),
        );
        println!(
            "{}: a command byte of 0 on {command_zero:?}; of 255 on {command_255}; invalid \
             commands: {}",
            loaded.name,
            invalid.len()
        );

        assert_eq!(
            histogram, from_walker,
            "the two readers count the same cells"
        );
        assert!(invalid.is_empty(), "no invalid command byte: {invalid:?}");
        assert_eq!(command_255, 0, "no cell carries the 255 spelling");
        assert_eq!(
            no_command + histogram.values().sum::<usize>(),
            ROWS * CHANNELS * loaded.module.pattern_count(),
            "every slot is either a cell with a command, a cell without one, or unnamed"
        );
    }

    // MUSIC0's own counts, letter by letter. D5's six — D:7348, F:1051, E:827, G:802,
    // O:755, H:212 — are this module's; the whole list is asserted.
    let music0 = music0();
    let histogram = histogram_of(&music0);
    let measured: Vec<(&str, usize)> = vec![
        ("A", 63),
        ("B", 3),
        ("C", 8),
        ("D", 7348),
        ("E", 827),
        ("F", 1051),
        ("G", 802),
        ("H", 212),
        ("J", 137),
        ("K", 105),
        ("O", 755),
        ("Q", 84),
        ("S", 140),
    ];
    for (letter, want) in &measured {
        let letter = letter.chars().next().expect("a letter");
        assert_eq!(count(&histogram, letter), *want, "MUSIC0's {letter} count");
    }
    println!(
        "MUSIC0's histogram: {} cells carry a command over {} effect letters, all of them \
         A..Z",
        histogram.values().sum::<usize>(),
        histogram.len(),
    );
    assert_eq!(
        histogram.values().sum::<usize>(),
        measured.iter().map(|(_, count)| count).sum::<usize>()
    );
    assert_eq!(
        histogram.len(),
        measured.len(),
        "and no other letter occurs"
    );

    // MUSIC1's, plus the one cell whose command byte is 0 — D5 records that the cell
    // exists and that 0 means no command; where it is, is this test's find.
    let music1 = music1();
    let histogram = histogram_of(&music1);
    let measured: Vec<(&str, usize)> = vec![
        ("A", 54),
        ("C", 4),
        ("D", 3340),
        ("E", 217),
        ("F", 180),
        ("G", 2607),
        ("H", 365),
        ("K", 62),
        ("O", 2417),
        ("Q", 38),
        ("S", 48),
    ];
    for (letter, want) in &measured {
        let letter = letter.chars().next().expect("a letter");
        assert_eq!(count(&histogram, letter), *want, "MUSIC1's {letter} count");
    }
    println!(
        "MUSIC1's histogram: {} cells over {} effect letters, plus one whose command byte is \
         0 (pattern 13 row 19 channel 5)",
        histogram.values().sum::<usize>() - 1,
        histogram.len() - 1,
    );
    assert_eq!(
        histogram.len(),
        measured.len() + 1,
        "the eleven letters plus the command-0 cell"
    );
    assert_eq!(
        histogram.get(&0).copied(),
        Some(1),
        "one command-0 cell, at pattern 13 row 19 channel 5"
    );
    assert_eq!(
        histogram.values().sum::<usize>(),
        measured.iter().map(|(_, count)| count).sum::<usize>() + 1
    );

    // The cell itself: a real cell whose command field was carried with a byte of 0, not an
    // error and not an unnamed slot.
    let cell = music1.module.pattern(13).expect("pattern 13").rows[19][5];
    println!(
        "MUSIC1 pattern 13 row 19 channel 5: present {:#04x} (a command field follows), command \
         {}, info {}",
        cell.present, cell.command, cell.info
    );
    assert!(cell.has_command(), "the file carried the command field");
    assert_eq!(cell.command, 0, "with a byte of zero");
    assert_eq!(
        cell.info, 0x40,
        "and a real info byte, so the cell is not empty"
    );
    assert!(!cell.is_empty());

    // Its flag byte, from this file's own walker: bit 7 (command and info) with the low five
    // bits naming channel 5 — 0x85, the cell D5 records as MUSIC1's exception and that this
    // test locates.
    let walk = walks(&music1);
    let raw = walk[13].rows[19]
        .iter()
        .find(|raw| raw.channel == 5)
        .expect("the cell on channel 5");
    assert_eq!(
        raw.present, 0x80,
        "the flag byte is bit 7 alone, with channel 5 in the low bits"
    );
    assert_eq!(
        music1.module.pattern_count(),
        77,
        "and it does not stop any pattern from decoding"
    );
}

#[test]
fn the_notes_are_st3_numeric_and_reach_past_ninety_six() {
    // `(octave << 4) | semitone` (design §7.1), valid up to 0x9B. A decoder that caps at 96
    // (0x60) rejects MUSIC1's high material.
    let mut measured: Vec<(&str, u8, u8, usize, usize, usize)> = Vec::new();
    for loaded in both() {
        let mut lowest = u8::MAX;
        let mut highest = 0u8;
        let mut note_off = 0usize;
        let mut no_note = 0usize;
        let mut played = 0usize;

        for index in 0..loaded.module.pattern_count() {
            for cells in loaded.module.pattern(index).expect("a pattern").rows.iter() {
                for cell in cells {
                    if !cell.has_note() {
                        continue;
                    }
                    match cell.note {
                        0xFE => note_off += 1,
                        0xFF => no_note += 1,
                        note => {
                            played += 1;
                            lowest = lowest.min(note);
                            highest = highest.max(note);
                        }
                    }
                }
            }
        }

        println!(
            "{}: {played} notes {lowest:#04x}..{highest:#04x}, {note_off} note-offs (0xFE), \
             {no_note} no-notes (0xFF); the format's ceiling is 0x9B",
            loaded.name
        );
        assert!(highest < 0x9B, "no note byte is past the format's ceiling");
        assert!(lowest >= 0x10, "and none is below the first octave");
        measured.push((loaded.name, lowest, highest, played, note_off, no_note));
    }

    // The highest note in each module, and the format's ceiling above it.
    let music0 = measured
        .iter()
        .find(|row| row.0 == "MUSIC0.S3M")
        .expect("MUSIC0");
    let music1 = measured
        .iter()
        .find(|row| row.0 == "MUSIC1.S3M")
        .expect("MUSIC1");
    assert_eq!(music0.2, 0x64, "MUSIC0's highest note");
    assert_eq!(music1.2, 0x80, "MUSIC1's highest note");
    assert!(
        music0.2 < 0x9B && music1.2 < 0x9B,
        "the format's ceiling is above both"
    );
    assert!(
        music1.2 > 0x60,
        "so a decoder capped at 96 would reject MUSIC1's high material"
    );
}

// ---------------------------------------------------------------------------------------
// 4. The instruments and their samples
// ---------------------------------------------------------------------------------------

#[test]
fn the_instrument_census_is_the_files_own() {
    // 0 = empty, 1 = PCM, 2..=7 = adlib (`STARTEMS.C:132`, `:175-176`). The adlib count is
    // scope evidence: neither module has one, so the port models no OPL.
    for (loaded, pcm, empty) in [(music0(), 22usize, 22usize), (music1(), 41usize, 13usize)] {
        let mut census: BTreeMap<u8, usize> = BTreeMap::new();
        let mut looped = 0usize;
        let mut sample_bytes = 0usize;
        let mut pcm_without_a_pointer = 0usize;
        for index in 0..loaded.module.instrument_count() {
            let instrument = loaded.module.instrument(index).expect("an instrument");
            *census.entry(instrument.sample_type).or_insert(0) += 1;
            if instrument.loops() {
                looped += 1;
            }
            if instrument.is_pcm() && instrument.memseg == 0 {
                pcm_without_a_pointer += 1;
            }
            sample_bytes += loaded.module.sample_bytes(index).map_or(0, <[u8]>::len);
        }
        println!(
            "{}: {} instruments, type census {census:?}; {looped} of them loop; {sample_bytes} \
             sample bytes in all; {pcm_without_a_pointer} PCM headers with a zero paragraph \
             pointer",
            loaded.name,
            loaded.module.instrument_count(),
        );

        assert_eq!(census.get(&1).copied(), Some(pcm), "{} PCM", loaded.name);
        assert_eq!(
            census.get(&0).copied(),
            Some(empty),
            "{} empty",
            loaded.name
        );
        assert!(
            census.keys().all(|kind| *kind <= 1),
            "no type-2 (adlib) instrument in {}: {census:?}",
            loaded.name
        );
        assert_eq!(
            census.values().sum::<usize>(),
            loaded.module.instrument_count(),
            "and no other type occurs"
        );
        // `STARTEMS.C:367` loads a sample only when `type == 1 && memseg`; the loader here
        // slices when the type is PCM and the length is non-zero. The conditions differ only
        // for a PCM header with a zero paragraph pointer, and there is no such header in
        // either file — which is what makes the difference inert rather than a divergence.
        assert_eq!(
            pcm_without_a_pointer, 0,
            "{}: every PCM header has a paragraph pointer, so the two conditions agree",
            loaded.name
        );
    }
}

#[test]
fn a_sample_header_is_parsed_at_the_files_own_offsets() {
    // Two headers, field by field. `flags` at +0x1F and `c2spd` at +0x20..0x23 are the
    // trap: read four bytes early and both come out as plausible-looking numbers that
    // belong to other fields.
    let music0 = music0();
    let first = music0.module.instrument(0).expect("instrument 0");
    println!(
        "MUSIC0 instrument 0: type {} filename {:?} memseg {:#06x}:{} length {} loop {}..{} \
         volume {} disk {} pack {} flags {:#04x} c2spd {} name {:?}",
        first.sample_type,
        String::from_utf8_lossy(&first.filename),
        first.memseg,
        first.memseg2,
        first.length,
        first.loop_begin,
        first.loop_end,
        first.volume,
        first.disk,
        first.pack,
        first.flags,
        first.c2spd,
        String::from_utf8_lossy(&first.name),
    );
    assert_eq!(
        &first.filename, b"HISTRING\0\0\0\0",
        "the 12-byte filename field, at +0x01"
    );
    assert_eq!(first.sample_type, 1);
    assert!(first.is_pcm() && !first.is_empty());
    assert_eq!(first.length, 30_208);
    assert_eq!(first.loop_begin, 5_720);
    assert_eq!(first.loop_end, 30_014);
    assert_eq!(first.volume, 64);
    assert_eq!(first.disk, 0);
    assert_eq!(first.pack, 0);
    assert_eq!(first.flags, 0x01, "flags at +0x1F");
    assert!(first.loops());
    assert_eq!(
        first.c2spd, 27_700,
        "c2spd at +0x20, not the 0x0100_0040 of +0x1C"
    );
    assert_eq!(first.memseg, 3_873);
    assert_eq!(first.memseg2, 0);
    assert_eq!(&first.name[..25], b"HiStrings.HiQual(Marcato)");

    let music1 = music1();
    let first = music1.module.instrument(0).expect("instrument 0");
    println!(
        "MUSIC1 instrument 0: type {} filename {:02X?} memseg {:#06x}:{} length {} loop {}..{} \
         volume {} disk {} pack {} flags {:#04x} c2spd {} name {:?}",
        first.sample_type,
        first.filename,
        first.memseg,
        first.memseg2,
        first.length,
        first.loop_begin,
        first.loop_end,
        first.volume,
        first.disk,
        first.pack,
        first.flags,
        first.c2spd,
        String::from_utf8_lossy(&first.name),
    );
    assert_eq!(
        first.filename, [0u8; 12],
        "MUSIC1's instruments carry no filename"
    );
    assert_eq!(first.length, 40_066);
    assert_eq!(first.loop_begin, 0);
    assert_eq!(first.loop_end, 0);
    assert_eq!(first.volume, 64);
    assert_eq!(first.pack, 0);
    assert_eq!(
        first.flags, 0x00,
        "flags at +0x1F, and this one does not loop"
    );
    assert!(!first.loops());
    assert_eq!(first.c2spd, 8_630);
    assert_eq!(first.memseg, 5_012);
    assert_eq!(&first.name[..19], b"By Purple Motion of");
}

#[test]
fn an_empty_instrument_is_a_normal_case() {
    // 22 of MUSIC0's 44 and 13 of MUSIC1's 54 are type 0 (`STARTEMS.C:132`: only type 1 is
    // a sample). Not an error, and not empty bytes: the header still carries the file's own
    // values.
    let music0 = music0();
    let empties: Vec<usize> = (0..music0.module.instrument_count())
        .filter(|index| {
            music0
                .module
                .instrument(*index)
                .expect("an instrument")
                .is_empty()
        })
        .collect();
    let first = music0.module.instrument(empties[0]).expect("an instrument");
    println!(
        "MUSIC0's {} empty instruments: instrument {} carries length {} memseg {:#06x} volume \
         {} flags {:#04x} c2spd {}",
        empties.len(),
        empties[0],
        first.length,
        first.memseg,
        first.volume,
        first.flags,
        first.c2spd,
    );
    assert_eq!(empties.len(), 22);
    assert_eq!(first.length, 0);
    assert_eq!(first.memseg, 0);
    assert_eq!(first.volume, 64);
    assert_eq!(first.flags, 0x00);
    assert_eq!(first.c2spd, 8_363);
    assert_eq!(music0.module.sample_bytes(empties[0]), None);

    let music1 = music1();
    let empties = (0..music1.module.instrument_count())
        .filter(|index| {
            music1
                .module
                .instrument(*index)
                .expect("an instrument")
                .is_empty()
        })
        .count();
    println!("MUSIC1's {empties} empty instruments");
    assert_eq!(empties, 13);
}

#[test]
fn the_samples_are_sliced_by_paragraph_pointer() {
    // `STARTEMS.C:385-388`: the sample sits at `memseg << 4 | memseg2 << 20` in the file and
    // is `length` bytes long. Both ends are checked — a slice that ran past the file would
    // read garbage, and the modules are laid out tightly enough that the last sample's end
    // *is* the file's end.
    for (loaded, samples) in [(music0(), 22usize), (music1(), 41usize)] {
        let mut present = 0usize;
        let mut first_at = usize::MAX;
        let mut last_end = 0usize;
        for index in 0..loaded.module.instrument_count() {
            let instrument = loaded.module.instrument(index).expect("an instrument");
            let Some(sample) = loaded.module.sample_bytes(index) else {
                assert!(
                    !instrument.is_pcm() || instrument.length == 0,
                    "{} instrument {index} is PCM with a length but has no sample",
                    loaded.name
                );
                continue;
            };
            present += 1;
            let at = instrument.sample_offset();
            let end = at + sample.len();
            assert!(end <= loaded.bytes.len(), "the sample is inside the file");
            assert_eq!(
                sample,
                &loaded.bytes[at..end],
                "{} instrument {index}: the slice is the file's own bytes at the pointer",
                loaded.name
            );
            assert_eq!(sample.len(), instrument.length as usize);
            first_at = first_at.min(at);
            last_end = last_end.max(end);
        }
        println!(
            "{}: {present} samples sliced, the first at byte {first_at} and the last ending at \
             byte {last_end} of {}",
            loaded.name,
            loaded.bytes.len()
        );
        assert_eq!(present, samples, "{}'s sample count", loaded.name);
        assert_eq!(
            last_end,
            loaded.bytes.len(),
            "{}'s last sample ends on the file's own last byte",
            loaded.name
        );
    }

    // The container is one layout, so the samples start where the patterns end: the last
    // pattern block's end to the first sample is filler and nothing else.
    for loaded in both() {
        let last = *pattern_table(&loaded).last().expect("patterns");
        let end = usize::from(last) * 16 + usize::from(word(&loaded.bytes, usize::from(last) * 16));
        let first_sample = (0..loaded.module.instrument_count())
            .filter_map(|index| {
                let instrument = loaded.module.instrument(index).expect("an instrument");
                loaded
                    .module
                    .sample_bytes(index)
                    .map(|_| instrument.sample_offset())
            })
            .min()
            .expect("samples");
        let between = &loaded.bytes[end..first_sample];
        println!(
            "{}: the patterns end at byte {end}, the first sample begins at {first_sample}, and \
             the {} bytes between them are {:02X?}",
            loaded.name,
            between.len(),
            between,
        );
        assert!(
            between.iter().all(|byte| *byte == 0x80),
            "{}: the gap between the patterns and the samples is 0x80 filler",
            loaded.name
        );
    }
}

#[test]
fn the_magic_is_where_the_format_puts_it() {
    // `SCRS` at +0x4C of `struct ds_smp` (`STARTEMS.C:47`); `SCRM` is the header test's.
    for loaded in both() {
        let orders = usize::from(word(&loaded.bytes, 32));
        let mut with_magic = 0usize;
        let mut without = 0usize;
        for index in 0..loaded.module.instrument_count() {
            let pointer = word(&loaded.bytes, 0x60 + orders + 2 * index);
            let at = usize::from(pointer) * 16;
            let magic = &loaded.bytes[at + 0x4C..at + 0x50];
            if loaded
                .module
                .instrument(index)
                .expect("an instrument")
                .is_pcm()
            {
                assert_eq!(magic, SAMPLE_MAGIC, "{} instrument {index}", loaded.name);
                with_magic += 1;
            } else {
                assert_eq!(magic, [0u8; 4], "{} instrument {index}", loaded.name);
                without += 1;
            }
        }
        println!(
            "{}: SCRS at +0x4C of {with_magic} sample headers, and four zero bytes at \
             +0x4C of {without}",
            loaded.name
        );

        // The loader does not *require* SCRS, and that is deliberate: `checkins` writes it
        // at load time (`STARTEMS.C:134`) rather than checking it, so a header without it is
        // a file the original would still have played.
        let mut header = loaded.bytes.clone();
        let first = usize::from(word(&header, 0x60 + orders)) * 16;
        header[first + 0x4C..first + 0x50].copy_from_slice(b"SCRI");
        assert!(
            Module::load(&header).is_ok(),
            "a sample header whose magic reads SCI stays loadable"
        );
    }
}

#[test]
fn the_cell_type_keeps_the_files_own_byte_and_its_own_presence() {
    // Two readings the loader deliberately does not "fix":
    //
    // 1. `Cell::instrument` is the file's byte: 0 means "no instrument change" and the rest
    //    are 1-based. MUSIC1's highest instrument byte is 54 and its `insNum` is 54 — a
    //    0-based reading would name a 55th instrument the file does not have.
    // 2. A cell whose only content is `volume = 0` is a cell. Counting them is how the
    //    loader's `present` mask earns its place: without it they are indistinguishable from
    //    an unnamed channel slot, and a player would leave the volume where it was instead
    //    of taking it to zero.
    let mut counts: Vec<(&str, u8, usize, usize, usize)> = Vec::new();
    for loaded in both() {
        let mut highest_instrument = 0u8;
        let mut instrument_zero = 0usize;
        let mut volume_only_zero = 0usize;
        let mut all_fields_zero = 0usize;

        for index in 0..loaded.module.pattern_count() {
            for cells in loaded.module.pattern(index).expect("a pattern").rows.iter() {
                for cell in cells {
                    if cell.has_note() {
                        highest_instrument = highest_instrument.max(cell.instrument);
                        if cell.instrument == 0 {
                            instrument_zero += 1;
                        }
                    }
                    if cell.present == Cell::VOLUME && cell.volume == 0 {
                        volume_only_zero += 1;
                        if cell.note == 0
                            && cell.instrument == 0
                            && cell.command == 0
                            && cell.info == 0
                        {
                            all_fields_zero += 1;
                        }
                    }
                }
            }
        }

        println!(
            "{}: highest instrument byte {highest_instrument} of {} instruments; \
             {instrument_zero} cells name instrument 0; {volume_only_zero} cells carry nothing \
             but a volume of 0 ({all_fields_zero} of which have all five fields zero)",
            loaded.name,
            loaded.module.instrument_count(),
        );
        assert!(
            usize::from(highest_instrument) <= loaded.module.instrument_count(),
            "{}: no instrument byte names an instrument the file does not have",
            loaded.name
        );
        assert!(
            instrument_zero > 0,
            "the 0 spelling occurs in {}",
            loaded.name
        );
        assert_eq!(all_fields_zero, volume_only_zero);
        counts.push((
            loaded.name,
            highest_instrument,
            loaded.module.instrument_count(),
            instrument_zero,
            volume_only_zero,
        ));
    }

    // The measured values: MUSIC1's byte 54 against its 54 instruments is the one-based
    // reading's evidence, and the 3 and 189 cells are the `present` mask's.
    assert_eq!(
        counts,
        vec![
            ("MUSIC0.S3M", 29, 44, 74, 3),
            ("MUSIC1.S3M", 54, 54, 35, 189),
        ]
    );
}
