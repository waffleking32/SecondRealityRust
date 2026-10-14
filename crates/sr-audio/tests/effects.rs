//! Task 4's tests: the channel effects, driven from the shipped modules' own cells.
//!
//! **This is the task's whole evidence.** The per-frame trace comparison (`tests/trace.rs`)
//! cannot judge a channel effect — a wrong `D` or `H` does not move the timeline — so every
//! effect here is asserted tick by tick against **the object's arithmetic**, written out in
//! this file from `MAIN/STMIK.300`'s own bytes: the period table at `+0xa24`, the octave
//! shifts at `+0x97c`, the vibrato sine at `+0xa50` and the `Q` tables at `+0x95c`/`+0x96c`,
//! each of them asserted against the object's segment image by `tests/player.rs`'s claims
//! table. Nothing in this file asks the port what it does.
//!
//! Where a test drives a shipped module it **names the order, pattern, row and channel** and
//! asserts the cell at that position carries the command it means to drive
//! ([`the_cells_the_tests_drive_are_the_modules_own`]), so a change to the data fails here
//! rather than silently testing nothing. Where the module's data does not reach a form (the
//! fine *down* volume slide, the `O` with no instrument, `K`'s shared parameter) the test
//! builds a module of its own — and the note records the absent form's count.

use std::collections::BTreeMap;
use std::path::Path;

use sr_audio::{descramble, Cell, Module, Player, PlayerOptions, ROWS};

const ASSETS: &str = "../../assets/MAIN";

/// The module with the shell's patch applied where the shell applies it: MUSIC0 plays at
/// tempo 120 (`MAIN/STARTMUS.C:252-265`), MUSIC1 at its file's own 130.
fn module(name: &str) -> Module {
    let path = Path::new(ASSETS).join(format!("{name}.S3M"));
    let bytes = std::fs::read(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let mut module = Module::load(&bytes).unwrap_or_else(|error| panic!("{name}: {error}"));
    if name == "MUSIC0" {
        module.apply_tempo_patch(0x78);
    }
    module
}

// -------------------------------------------------------------------------------------------
// The tick harness
// -------------------------------------------------------------------------------------------

/// Advance to the next tick. One poll covers `mix_rate/70.086` samples and a tick is 833 of
/// them at tempo 120, so a poll runs at most one tick — the loop is a formality.
fn tick(player: &mut Player) {
    let before = player.zframe();
    for _ in 0..64 {
        player.poll();
        if player.zframe() != before {
            return;
        }
    }
    panic!("no tick in 64 polls");
}

/// Tick until the engine is playing `row` of `pattern`, **before** that row's first tick:
/// the state is the one the row before it left, which is the input a per-tick test needs.
fn run_to_row(player: &mut Player, pattern: u8, row: u8) {
    for _ in 0..100_000 {
        let position = player.engine_position();
        if position.pattern == pattern && position.row == row {
            return;
        }
        tick(player);
    }
    panic!("pattern {pattern} row {row} was never entered");
}

/// Tick until the engine is playing `row` of `pattern`, then one more tick — the row's own
/// first tick, which is where its cells are read and its commands dispatched.
fn run_to(player: &mut Player, pattern: u8, row: u8) {
    for _ in 0..100_000 {
        let position = player.engine_position();
        if position.pattern == pattern && position.row == row {
            tick(player);
            assert_eq!(
                player.engine_position().row,
                row,
                "the row advanced while stepping onto it"
            );
            return;
        }
        tick(player);
    }
    panic!("pattern {pattern} row {row} was never entered");
}

/// One channel's state, copied out so the borrow ends.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
struct State {
    period: u16,
    org_period: u16,
    target_period: u16,
    volume: u8,
    vibrato: u16,
    retrigger: u8,
    position: u32,
    offset: u32,
    aequ: u8,
    last_info: u8,
    last_effect1: u8,
    info: u8,
    used: bool,
}

fn state(player: &Player, channel: usize) -> State {
    let channel = player.channel(channel).expect("a channel");
    State {
        period: channel.period,
        org_period: channel.org_period,
        target_period: channel.target_period,
        volume: channel.volume,
        vibrato: channel.vibrato_position,
        retrigger: channel.retrigger_count,
        position: channel.position,
        offset: channel.sample_offset,
        aequ: channel.aequ,
        last_info: channel.last_info,
        last_effect1: channel.last_effect1,
        info: channel.info,
        used: channel.used,
    }
}

/// The cell at a position.
fn cell(module: &Module, pattern: usize, row: usize, channel: usize) -> Cell {
    module.pattern(pattern).expect("a pattern").rows[row][channel]
}

/// Assert a position carries the command a test means to drive.
fn assert_cell(module: &Module, pattern: usize, row: usize, channel: usize, command: u8, info: u8) {
    let found = cell(module, pattern, row, channel);
    assert!(
        found.has_command() && found.command == command && found.info == info,
        "pattern {pattern} row {row} channel {channel} should carry command {command:#04x} \
         info {info:#04x}, and carries {:#04x}/{:#04x} (present {:#04x})",
        found.command,
        found.info,
        found.present
    );
}

/// The `c2spd` the player will load for a cell's instrument, as the object reads it (the
/// header's 32-bit field into a word, `checkins`'s clamp to 65535 already applied to the
/// file). Zero when the cell names no instrument.
fn cell_c2spd(module: &Module, cell: &Cell) -> u16 {
    if cell.instrument == 0 {
        return 0;
    }
    module
        .instrument(usize::from(cell.instrument) - 1)
        .map_or(0, |header| header.c2spd.min(0xffff) as u16)
}

// -------------------------------------------------------------------------------------------
// The object's tables and arithmetic, written out from its bytes
// -------------------------------------------------------------------------------------------

/// The periods at the object's `+0xa24`, semitone by semitone, four octaves up.
const PERIODS: [u16; 12] = [
    27392, 25856, 24384, 23040, 21696, 20480, 19328, 18240, 17216, 16256, 15360, 14512,
];

/// The shifts at the object's `+0x97c` — eight real entries and the eight bytes after them,
/// which octaves 8…15 read because the object does not check the index.
const SHIFTS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 5, 11, 17, 27, 32, 42, 47];

/// The vibrato waveform at the object's `+0xa50`: 64 signed words, ±255.
const SINE: [i16; 64] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24, 0, -24, -49, -74, -97, -120, -141,
    -161, -180, -197, -212, -224, -235, -244, -250, -253, -255, -253, -250, -244, -235, -224, -212,
    -197, -180, -161, -141, -120, -97, -74, -49, -24,
];

/// The object's `+0x593a`: a note byte to a period.
fn note_period(note: u8) -> u16 {
    if note == 0xfe {
        return 0;
    }
    PERIODS[usize::from(note & 0x0f)] >> SHIFTS[usize::from(note >> 4)]
}

/// The object's `+0x5813`: `period × 8363/c2spd`, with the word-overflow guard.
fn scaled(period: u16, c2spd: u16) -> u16 {
    let product = u32::from(period) * 8363;
    if product >> 16 >= u32::from(c2spd) {
        return 0x7fff;
    }
    (product / u32::from(c2spd)).min(0x7fff) as u16
}

/// The object's `+0x582b`: its own base frequency `0x00da7600` over the period.
fn period_step(period: u16) -> u32 {
    if period == 0 {
        return 0;
    }
    0x00da_7600 / u32::from(period)
}

/// One tick of the object's `+0x4005` vibrato, from the position before it: the position
/// after it, and the period the tick plays. The level is `value × depth >> 5`, and 5 is the
/// shift because both modules' flags word is 8, so `[0x915]` is 0 (`+0x4043`).
fn vibrato_tick(position: u16, info: u8, org_period: u16) -> (u16, u16) {
    let depth = i32::from(info & 0x0f);
    let speed = u16::from(info >> 4);
    let stale = position & 0x80 != 0;
    let value = if stale {
        0
    } else {
        i32::from(SINE[usize::from(position & 0x7f) / 2])
    };
    let level = (value * depth) >> 5;
    let period = org_period.wrapping_add(level as u16);
    let from = if stale { 0 } else { position & 0xff };
    ((from + speed * 2) & 0x7e, period)
}

/// One tick of the object's `+0x41ec` retrigger, from the counter before it: the counter
/// after it and whether this tick fired. The interval is the low nibble, the volume code the
/// high one, and the counter goes to zero on a fire before it is incremented.
fn retrigger_tick(count: u8, info: u8) -> (u8, bool) {
    let interval = info & 0x0f;
    if interval != 0 && interval <= count {
        return (1, true);
    }
    (count + 1, false)
}

// -------------------------------------------------------------------------------------------
// The scope, measured: which cells the tests drive, and the letters that are absent
// -------------------------------------------------------------------------------------------

/// The two modules' effect letters and their counts — this task's scope evidence — and the
/// cells the tests below name. An effect the tests drive that the histogram shows zero of
/// would be a test of nothing.
#[test]
fn the_cells_the_tests_drive_are_the_modules_own() {
    let mut counts: BTreeMap<&str, BTreeMap<u8, usize>> = BTreeMap::new();
    for name in ["MUSIC0", "MUSIC1"] {
        let module = module(name);
        let mut histogram: BTreeMap<u8, usize> = BTreeMap::new();
        for index in 0..module.pattern_count() {
            for cells in &module.pattern(index).expect("a pattern").rows {
                for cell in cells {
                    if cell.has_command() {
                        *histogram.entry(cell.command).or_default() += 1;
                    }
                }
            }
        }
        let named: Vec<String> = histogram
            .iter()
            .map(|(&command, &count)| {
                let letter = if (1..=26).contains(&command) {
                    char::from(b'A' + command - 1).to_string()
                } else {
                    format!("{command:#04x}")
                };
                format!("{letter}:{count}")
            })
            .collect();
        println!("  {name}: {}", named.join(" "));
        counts.insert(name, histogram);
    }

    // The letters the scope names, each present in at least one module.
    for (name, command) in [
        ("MUSIC0", 4u8),
        ("MUSIC0", 5),
        ("MUSIC0", 6),
        ("MUSIC0", 7),
        ("MUSIC0", 8),
        ("MUSIC0", 10),
        ("MUSIC0", 11),
        ("MUSIC0", 15),
        ("MUSIC0", 17),
        ("MUSIC0", 19),
    ] {
        assert!(
            counts[name].get(&command).copied().unwrap_or(0) > 0,
            "{name} carries no {}",
            char::from(b'A' + command - 1)
        );
    }
    // ...and the three the object's tables implement but no cell reaches.
    for (command, letter) in [(9u8, 'I'), (12, 'L'), (18, 'R')] {
        for name in ["MUSIC0", "MUSIC1"] {
            assert_eq!(
                counts[name].get(&command).copied().unwrap_or(0),
                0,
                "{name} carries an {letter} — this task's note records it as absent"
            );
        }
    }

    // `S` is `SDx` and nothing else: subcommand `d`, delays 2, 3 and 6.
    let delays = |name: &str| {
        let module = module(name);
        let mut subcommands: BTreeMap<u8, usize> = BTreeMap::new();
        let mut delays: BTreeMap<u8, usize> = BTreeMap::new();
        for index in 0..module.pattern_count() {
            for cells in &module.pattern(index).expect("a pattern").rows {
                for cell in cells {
                    if cell.has_command() && cell.command == 19 {
                        *subcommands.entry(cell.info >> 4).or_default() += 1;
                        *delays.entry(cell.info & 0x0f).or_default() += 1;
                    }
                }
            }
        }
        println!("  {name}: S subcommands {subcommands:?}, delays {delays:?}");
        assert_eq!(
            subcommands.keys().copied().collect::<Vec<u8>>(),
            vec![0x0d],
            "{name}'s S cells are not all SDx"
        );
        delays
    };
    assert_eq!(delays("MUSIC0"), BTreeMap::from([(3u8, 77usize), (6, 63)]));
    assert_eq!(delays("MUSIC1"), BTreeMap::from([(2u8, 48usize)]));

    // The named cells, checked against the data the tests below drive them through.
    let m0 = module("MUSIC0");
    assert_cell(&m0, 0, 0, 0, 4, 0x10); // D10
    assert_cell(&m0, 0, 1, 0, 4, 0x00); // D00
    assert_cell(&m0, 0, 0, 3, 4, 0x20); // the note the E/F rows walk from
    assert_cell(&m0, 0, 11, 3, 5, 0x02); // E02
    assert_cell(&m0, 0, 12, 3, 5, 0x00); // E00
    assert_cell(&m0, 0, 13, 3, 6, 0x02); // F02
    assert_cell(&m0, 9, 60, 7, 4, 0x0f); // D0F
    assert_cell(&m0, 8, 16, 1, 7, 0x01); // G01
    assert_cell(&m0, 8, 32, 3, 6, 0xf1); // F extra-fine, 1
    assert_cell(&m0, 41, 47, 0, 6, 0xf4); // F extra-fine, 4
    assert_cell(&m0, 47, 0, 6, 1, 0x06); // A06 — the row H vibratos from
    assert_cell(&m0, 47, 6, 6, 8, 0x81); // H81
    assert_cell(&m0, 47, 56, 2, 17, 0xf4); // Q F4
    assert_cell(&m0, 42, 8, 7, 10, 0x7c); // J7C
    assert_cell(&m0, 48, 0, 6, 11, 0x01); // K01
    assert_cell(&m0, 1, 0, 2, 15, 0x05); // O05
    assert_cell(&m0, 11, 12, 0, 15, 0x08); // O08 — a multiple of 256
    assert_cell(&m0, 23, 58, 7, 17, 0xc4); // Q C4
    assert_cell(&m0, 41, 0, 5, 19, 0xd3); // SD3
    assert_cell(&m0, 40, 1, 5, 19, 0xd6); // SD6
    let m1 = module("MUSIC1");
    assert_cell(&m1, 30, 0, 3, 1, 0x06); // A06 — the speed the SD2 row runs at
    assert_cell(&m1, 30, 2, 3, 19, 0xd2); // SD2
    assert_cell(&m1, 2, 0, 6, 5, 0x02); // E02 on a row that plays a note
}

// -------------------------------------------------------------------------------------------
// D — the volume slide
// -------------------------------------------------------------------------------------------

#[test]
fn d_slides_the_volume_a_nibble_a_tick() {
    // MUSIC0, order 0 (pattern 0), channel 0: row 0 is `D10` with a note and a volume of 1,
    // and row 1 is `D00`.
    //
    // `D10` is `volume += 1` a tick — `+0x3f2c` sends a zero low nibble to the addend path at
    // `+0x3f51` — and it runs on the row's **first** tick too, because `D` is in both
    // dispatch tables. So row 0's six ticks give 2, 3, 4, 5, 6, 7.
    //
    // `D00` is the shared-memory case: the parameter is zero, so `+0x3f11` takes the
    // channel's `alastnfo` — which row 0 wrote — and writes it back into the cell's info
    // byte. Row 1 keeps sliding up by 1, and the channel's info field reads 0x10.
    let m0 = module("MUSIC0");
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(0);
    run_to(&mut player, 0, 0);
    assert_eq!(
        player.state().speed,
        6,
        "MUSIC0's header speed, and no A yet"
    );

    let mut volumes = Vec::new();
    for _ in 0..6 {
        volumes.push(state(&player, 0).volume);
        tick(&mut player);
    }
    assert_eq!(volumes, vec![2, 3, 4, 5, 6, 7], "D10's own row");
    assert_eq!(
        state(&player, 0).last_info,
        0x10,
        "the first-tick pass remembers every non-zero parameter"
    );

    // The tick that ended row 0 advanced the player into row 1, whose first tick was that
    // same tick — so the state now is row 1's tick 0.
    assert_eq!(player.engine_position().row, 1);
    assert_eq!(state(&player, 0).info, 0x10, "D00 reused row 0's parameter");
    let mut volumes = Vec::new();
    for _ in 0..6 {
        volumes.push(state(&player, 0).volume);
        tick(&mut player);
    }
    assert_eq!(
        volumes,
        vec![8, 9, 10, 11, 12, 13],
        "D00 slides by the parameter it did not carry"
    );
}

#[test]
fn d0f_slides_fifteen_a_tick() {
    // The degenerate fine form: `D0F`'s low nibble is 15, which reaches `+0x3f73` with a zero
    // high nibble and falls back to the ordinary slide — **15 a tick, every tick**. MUSIC0
    // carries three of them; this is pattern 9 (order 2) row 60 on channel 7, with its `D00`
    // at row 61.
    //
    // The volume the row begins with is the effect's *input* — the test reads it from the
    // channel — and the expected values are the object's arithmetic on it.
    let m0 = module("MUSIC0");
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(2);
    run_to_row(&mut player, 9, 60);
    let speed = usize::from(player.state().speed);
    let before = i32::from(state(&player, 7).volume);
    println!("  the volume before D0F is {before}, and the row runs {speed} ticks");

    // `D` is in both dispatch tables, so the slide starts on the row's first tick: the state
    // after tick `t` is the volume less 15 × (t + 1).
    let mut volumes = Vec::new();
    for _ in 0..speed {
        tick(&mut player);
        volumes.push(i32::from(state(&player, 7).volume));
    }
    let expected: Vec<i32> = (1..=speed as i32)
        .map(|tick| (before - 15 * tick).clamp(0, 63))
        .collect();
    assert_eq!(volumes, expected, "D0F: fifteen a tick, clamped at 0");

    // And `D00` after it keeps the parameter — D always does. Its own first tick slides too,
    // and `alastnfo` is 0x0f, not the cell's own zero.
    assert_eq!(player.engine_position().row, 61);
    assert_eq!(state(&player, 7).info, 0x0f);
    let before = i32::from(state(&player, 7).volume);
    let mut volumes = Vec::new();
    for _ in 0..speed {
        tick(&mut player);
        volumes.push(i32::from(state(&player, 7).volume));
    }
    let expected: Vec<i32> = (1..=speed as i32)
        .map(|tick| (before - 15 * tick).clamp(0, 63))
        .collect();
    assert_eq!(volumes, expected, "D00 continues it");
}

#[test]
fn the_fine_volume_slides_move_once_on_the_first_tick() {
    // Both fine forms, from the modules' own cells. Which nibble is the marker is the
    // surprise: `+0x3f22` sends a **low** nibble of `0xf` to `+0x3f73`, which adds the **high**
    // nibble once — `D1F` is a fine *up* by 1 — and `+0x3f27` sends a **high** nibble of `0xf`
    // to `+0x3f81`, which subtracts the **low** nibble once — `DF1` is a fine *down* by 1. The
    // degenerate `D0F` has a zero in the other nibble and falls back to the ordinary slide
    // (`d0f_slides_fifteen_a_tick`).
    //
    // The counts, so the shapes' presence is a measurement and not a hope: MUSIC0's `D` infos
    // hold 27 cells whose low nibble is `f` with a non-zero high nibble (the fine up), 144
    // whose high nibble is `f` (the fine down), and 3 that are `D0F`.
    let m0 = module("MUSIC0");
    let mut fine_up = 0usize;
    let mut fine_down = 0usize;
    for index in 0..m0.pattern_count() {
        for cells in &m0.pattern(index).expect("a pattern").rows {
            for cell in cells {
                if cell.has_command() && cell.command == 4 {
                    if cell.info & 0x0f == 0x0f && cell.info & 0xf0 == 0xf0 {
                        // 0xff: both nibbles the marker; the high one wins (`+0x3f22` first).
                        fine_down += 1;
                    } else if cell.info & 0x0f == 0x0f && cell.info & 0xf0 != 0 {
                        fine_up += 1;
                    } else if cell.info & 0xf0 == 0xf0 {
                        fine_down += 1;
                    }
                }
            }
        }
    }
    println!("  MUSIC0: {fine_up} fine-up (0x?f) and {fine_down} fine-down (0xf?) D cells");
    assert_eq!(fine_up, 27, "the fine up: 0x1f, 0x2f, 0x4f and 0x6f");
    assert_eq!(fine_down, 144, "the fine down: 0xf1…0xf4");

    // MUSIC0, order 1 (pattern 1), channel 5: row 8 is `D1F` (fine up by 1) and row 15 is
    // `DF1` (fine down by 1). The volume the row starts with is the effect's input — read
    // before the row's own first tick — and the expected state is one step on that tick, and
    // no step on any tick after it.
    for (row, delta, what) in [(8u8, 1i32, "D1F"), (15, -1, "DF1")] {
        let m0 = module("MUSIC0");
        let cell = cell(&m0, 1, usize::from(row), 5);
        let mut player = Player::new(m0, PlayerOptions::default());
        player.play_song(1);
        run_to_row(&mut player, 1, row);
        let speed = usize::from(player.state().speed);
        // The row's own note, when it has one, is what sets the volume the slide starts
        // from — its column is the cell's. Without one, the volume is whatever the channel
        // carried in, which is the effect's input and not this test's business.
        let base = if cell.has_note() {
            i32::from(cell.volume)
        } else {
            i32::from(state(&player, 5).volume)
        };
        let expected = (base + delta).clamp(0, 63) as u8;
        tick(&mut player);
        assert_eq!(
            state(&player, 5).volume,
            expected,
            "{what}: the first tick moves the volume once"
        );
        for _ in 1..speed {
            tick(&mut player);
            assert_eq!(
                state(&player, 5).volume,
                expected,
                "{what}: and no tick after it does"
            );
        }
    }

    // And the form's shape at a known volume, from a module written here, so the direction is
    // visible against a number and not only a delta: 30 + 3 and 30 - 3.
    for (info, expected, what) in [
        (0x3fu8, 33u8, "D3F: the high nibble 3, once"),
        (0xf3, 27, "DF3: the low nibble 3, once"),
    ] {
        let mut song = Song::new();
        song.instrument(8363, 64);
        song.row(&[Note::new(0, 0x40, 1, 30)]);
        song.row(&[Note::command(0, 4, info)]);
        let mut player = Player::new(song.build(), PlayerOptions::default());
        player.play_song(0);
        run_to(&mut player, 0, 1);
        assert_eq!(
            state(&player, 0).volume,
            expected,
            "{what}: on the first tick"
        );
    }
}

// -------------------------------------------------------------------------------------------
// E and F — the portamentos
// -------------------------------------------------------------------------------------------

#[test]
fn e_and_f_walk_the_period_by_four_times_the_parameter() {
    // MUSIC0, order 0 (pattern 0), channel 3. Row 0 plays note 0x20 with instrument 3; row
    // 11 is `E02`, row 12 `E00`, row 13 `F02`.
    //
    // The base period is the object's: note 0x20 (octave 2, C) is `27392 >> 2` = 6848, and
    // instrument 3's `c2spd` is 16000 — the header's own field — so `+0x5813` gives
    // `6848 × 8363/16000` = 3579. On every tick after the first the step is the whole info
    // byte times four (`+0x3e6c`) — 8 for `E02` — and it moves both `aspd` and `aorgspd`.
    let m0 = module("MUSIC0");
    assert_eq!(m0.instrument(2).expect("instrument 3").c2spd, 16000);
    let base = scaled(note_period(0x20), 16000);
    assert_eq!(
        base, 3579,
        "the object's arithmetic on the file's own numbers"
    );

    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(0);
    run_to_row(&mut player, 0, 11);
    let speed = u16::from(player.state().speed);
    let before = state(&player, 3);
    assert_eq!(
        before.period, base,
        "the note's own period, before E touches it"
    );
    assert_eq!(before.org_period, base);
    assert!(before.used);

    // The row's first tick: an info of 0x02 is at or below 0xe0, which is the *fine* path of
    // `+0x3e3b` — so it does nothing at all, and the slide runs only on the ticks after it.
    tick(&mut player);
    assert_eq!(
        state(&player, 3).period,
        base,
        "the first tick is the fine path"
    );
    for step in 1..speed {
        tick(&mut player);
        assert_eq!(
            state(&player, 3).period,
            base + 8 * step,
            "E02, tick {step}: the period grows by 4 × the parameter"
        );
        assert_eq!(
            state(&player, 3).org_period,
            base + 8 * step,
            "and the note's period moves with it"
        );
    }

    // Row 12 is `E00`: the parameter is remembered, so the slide continues — and its own
    // first tick is the fine path again, so it starts where row 11 ended.
    assert_eq!(player.engine_position().row, 12);
    assert_eq!(
        state(&player, 3).info,
        0x02,
        "E00 reused row 11's parameter"
    );
    let mut expected = base + 8 * (speed - 1);
    assert_eq!(
        state(&player, 3).period,
        expected,
        "the row starts where the last one left it"
    );
    tick(&mut player);
    assert_eq!(
        state(&player, 3).period,
        expected,
        "E00's own first tick is the fine path too"
    );
    for _ in 1..speed {
        tick(&mut player);
        expected += 8;
        assert_eq!(state(&player, 3).period, expected, "E00 keeps sliding");
    }

    // Row 13 is `F02`, the same step the other way.
    assert_eq!(player.engine_position().row, 13);
    let before = state(&player, 3).period;
    assert_eq!(
        before, expected,
        "the row starts where the last one left it"
    );
    tick(&mut player);
    assert_eq!(
        state(&player, 3).period,
        before,
        "F02's first tick is the fine path too"
    );
    for step in 1..speed {
        tick(&mut player);
        assert_eq!(
            state(&player, 3).period,
            before - 8 * step,
            "F02, tick {step}: the period shrinks by 4 × the parameter"
        );
    }
}

#[test]
fn f_s_extra_fine_form_moves_the_period_once_by_four_times_the_low_nibble() {
    // MUSIC0, order 11 (pattern 8), channel 3, row 32: `F` with info 0xf1. `+0x3e3f` sends a
    // parameter above 0xf0 to the extra-fine path — the low nibble times **four** — and then
    // jumps into the ordinary step, so it happens on the first tick and on no other.
    let m0 = module("MUSIC0");
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(11);
    run_to_row(&mut player, 8, 32);
    let speed = usize::from(player.state().speed);
    let before = state(&player, 3).period;
    println!("  pattern 8 row 32: entering with {before}, speed {speed}");
    assert!(
        before > 0,
        "the channel has a note from earlier in the pattern"
    );
    tick(&mut player);
    assert_eq!(
        state(&player, 3).period,
        before - 4,
        "extra-fine 1: the low nibble 1, times four, subtracted once"
    );
    for _ in 1..speed {
        tick(&mut player);
        assert_eq!(
            state(&player, 3).period,
            before - 4,
            "and never again in the row"
        );
    }

    // Row 33 is `F00`: the remembered parameter is 0xf1 again, and its own first tick — the
    // one before which this assertion reads — has not run yet.
    assert_eq!(player.engine_position().row, 33);
    assert_eq!(state(&player, 3).info, 0xf1);
    assert_eq!(state(&player, 3).period, before - 4, "unchanged so far");
    tick(&mut player);
    assert_eq!(
        state(&player, 3).period,
        before - 8,
        "and one more step on its first tick"
    );

    // And the low nibble 4 at pattern 41 (order 45) row 47 channel 0: 4 × 4 = 16.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(45);
    run_to_row(&mut player, 41, 47);
    let speed = usize::from(player.state().speed);
    let before = state(&player, 0).period;
    println!("  pattern 41 row 47: entering with {before}, speed {speed}");
    assert!(before > 0);
    tick(&mut player);
    assert_eq!(state(&player, 0).period, before - 16, "extra-fine 4: 4 × 4");
    for _ in 1..speed {
        tick(&mut player);
        assert_eq!(state(&player, 0).period, before - 16);
    }
}

// -------------------------------------------------------------------------------------------
// G — the tone portamento
// -------------------------------------------------------------------------------------------

#[test]
fn g_walks_the_period_to_the_notes_target_four_a_tick() {
    // MUSIC0, order 11 (pattern 8), channel 1, row 16: a note 0x30 with instrument 7, a `G01`
    // and a volume column of 0x40.
    //
    // The note with a `G` does **not** start: `+0x4944` skips the assignment of `aspd`, so the
    // period stays where it was and only `asldspd` — the target — takes the note's period. The
    // glide then moves both the period and `aorgspd` by `info × 4` a tick (`+0x3fb6`) and
    // clamps exactly onto the target.
    //
    // The volume column 0x40 is the file's 64, which the row parse clamps to **63**
    // (`+0x4779`).
    let m0 = module("MUSIC0");
    assert_eq!(m0.instrument(6).expect("instrument 7").c2spd, 11000);
    let target = scaled(note_period(0x30), 11000);
    assert_eq!(target, 2603, "note 0x30 at instrument 7's c2spd");

    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(11);
    run_to(&mut player, 8, 16);
    let after = state(&player, 1);
    assert_eq!(after.target_period, target, "the note set the target");
    assert_eq!(
        after.volume, 0x3f,
        "the volume column's 64 is clamped to 63"
    );
    // `+0x4944`: the note with a `G` does **not** touch `aspd` — that is the whole of the
    // tone porta. It does move `aorgspd` (`+0x4954`), because that is the base the glide
    // walks *and* writes back to.
    assert_ne!(
        after.period, target,
        "the note did not move the live period"
    );
    assert_eq!(
        after.org_period, target,
        "but the glide's base is the target"
    );

    let start = u32::from(after.period);
    let speed = u32::from(player.state().speed);
    for step in 1..speed {
        tick(&mut player);
        let expected = if start < u32::from(target) {
            (start + 4 * step).min(u32::from(target))
        } else {
            start.saturating_sub(4 * step).max(u32::from(target))
        };
        assert_eq!(
            u32::from(state(&player, 1).period),
            expected,
            "G01, tick {step}: four a tick towards {target}, clamped onto it"
        );
        assert_eq!(
            u32::from(state(&player, 1).org_period),
            expected,
            "and the glide's base moves with it"
        );
    }
    println!("  G01 walked {start} towards {target} at four a tick");
}

// -------------------------------------------------------------------------------------------
// H — the vibrato
// -------------------------------------------------------------------------------------------

#[test]
fn h_vibratos_on_the_objects_sine_table() {
    // MUSIC0, order 24 (pattern 47), channel 6: row 0 plays note 0x40 with instrument 5
    // (`c2spd` 14650), and row 6 is `H81` — speed 8, depth 1.
    //
    // The note's period is `1712 × 8363/14650` = 977. Each vibrato tick is
    // `aorgspd + (sine(position) × depth) >> 5`, and then the position advances by
    // `speed × 2`, masked to the even low seven bits — with the shift of 5 because the
    // module's flags word (8) has bit 0 clear, so `[0x915]` is 0 (`+0x4043`).
    //
    // The first vibrato tick finds the position at 0 — the note start zeroed it (`+0x4950`),
    // no command has marked it stale, and the `A06` that played the note is not one of the
    // commands that would — so the sine's first entry is 0 and the period does not move; the
    // position then steps to 16 (index 8, the sine's 180) and so on.
    let m0 = module("MUSIC0");
    assert_eq!(
        m0.flags(),
        8,
        "the module's flags word, which sets the shift"
    );
    let base = scaled(note_period(0x40), 14650);
    assert_eq!(base, 977);

    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(24);
    run_to(&mut player, 47, 6);
    let after = state(&player, 6);
    assert_eq!(after.period, base, "the first tick of an H row is quiet");
    // The A06 on row 0 is what set the stale bit: it is not one of the four commands the
    // first-tick pass exempts (`+0x3cdb`, `+0x3ce0`, `+0x3ce5`), and nothing between cleared
    // it — rows 1…5 have no command at all, which leaves the position alone.
    assert_eq!(after.vibrato, 0x80, "the position the A row left behind");
    assert_eq!(
        after.retrigger, 0,
        "the A row's first tick cleared the counter"
    );

    let mut position = after.vibrato;
    let mut periods = Vec::new();
    for step in 1..=5 {
        let (next, period) = vibrato_tick(position, 0x81, base);
        tick(&mut player);
        assert_eq!(
            state(&player, 6).vibrato,
            next,
            "H81, tick {step}: the position advances by 8 × 2"
        );
        assert_eq!(
            state(&player, 6).period,
            period,
            "H81, tick {step}: the period, from the object's sine"
        );
        position = next;
        periods.push(period);
    }
    // The sine's own values make the shape visible: the stale first tick returns 0, index 8
    // → 180 → +5, index 16 → 255 → +7, index 24 → 180 → +5 (the sine is symmetric about the
    // quarter point), index 32 → 0.
    assert_eq!(periods, vec![977, 982, 984, 982, 977]);
}

#[test]
fn h_restarts_a_stale_phase() {
    // The stale bit: the first-tick pass marks the vibrato position `| 0x80` for any channel
    // whose command is not `D`, `H`, `K`, `I` or `R` (`+0x3cea`), and each waveform function
    // tests it (`+0x407b`): a stale position returns **0** for the value and restarts the
    // phase at `speed × 2` — it does not continue from where it was.
    //
    // No shipped `H` run has a non-`H` command on the row before it on the same channel, so
    // the module is written here: a note, an `F10` row (any command that is not `H`), then
    // `H82`.
    let mut song = Song::new();
    song.instrument(8363, 64);
    song.row(&[Note::new(0, 0x40, 1, 40)]);
    song.row(&[Note::command(0, 5, 0x00)]); // E00 — a command, and one that moves nothing:
                                            // the parameter is its own zero, so the step is 0, but the first-tick pass still marks
                                            // the vibrato position stale (`+0x3cea`).
    song.row(&[Note::command(0, 8, 0x82)]); // H82 — speed 8, depth 2
    let mut player = Player::new(song.build(), PlayerOptions::default());
    player.play_song(0);
    let base = scaled(note_period(0x40), 8363);
    assert_eq!(base, 1712);

    // The `H` row's first tick: `H` has no first-tick handler at all (both tables' entry 8 is
    // the bare `ret`), so the period is unchanged and the stale bit is still set.
    run_to(&mut player, 0, 2);
    assert_eq!(state(&player, 0).period, base);
    assert_eq!(state(&player, 0).vibrato, 0x80, "the bit the F row set");

    // The first vibrato tick: stale, so the value is 0 and the phase restarts at 8 × 2 = 16.
    tick(&mut player);
    assert_eq!(
        state(&player, 0).period,
        base,
        "a stale position has no value"
    );
    assert_eq!(state(&player, 0).vibrato, 16, "and the phase restarts");
    // The second: index 8 → 180, depth 2 → `180 × 2 >> 5` = 11.
    tick(&mut player);
    assert_eq!(state(&player, 0).period, base + 11);
    assert_eq!(state(&player, 0).vibrato, 32);
}

// -------------------------------------------------------------------------------------------
// J — the arpeggio
// -------------------------------------------------------------------------------------------

#[test]
fn j_transposes_the_note_by_the_tick_modulo_three() {
    // MUSIC0, order 46 (pattern 42), channel 7, row 8: note 0x40 with instrument 4 — the
    // file's one instrument at the 8363 reference — and `J7C`.
    //
    // `+0x419a` divides the engine's tick counter by three and uses the remainder: the first
    // tick is the note itself, the second adds the high nibble (7 semitones), the third the
    // low nibble (12). The semitone carries at B (`+0x41c8`), so +12 from C is the next
    // octave's C. At the 8363 reference the periods are the table's own: 1712, 1140, 856.
    let m0 = module("MUSIC0");
    assert_eq!(m0.instrument(3).expect("instrument 4").c2spd, 8363);
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(46);
    run_to(&mut player, 42, 8);

    let expected = [
        scaled(note_period(0x40), 8363), // the note
        scaled(note_period(0x47), 8363), // + 7
        scaled(note_period(0x50), 8363), // + 12, wrapped into the next octave
        scaled(note_period(0x40), 8363),
        scaled(note_period(0x47), 8363),
        scaled(note_period(0x50), 8363),
    ];
    assert_eq!(expected, [1712, 1140, 856, 1712, 1140, 856]);

    let speed = usize::from(player.state().speed);
    println!("  J7C's row runs {speed} ticks");
    assert!(
        speed >= 3,
        "an arpeggio needs three ticks to show its cycle"
    );
    for step in 0..speed {
        assert_eq!(
            state(&player, 7).period,
            expected[step % 3],
            "J7C, tick {step}"
        );
        tick(&mut player);
    }
    // The row ended on the tick above — the loop's last `tick` was the one that advanced it
    // — and the next row is row 9, whose own first tick is the `J` at tick 0: the note's own
    // period, not the third of the cycle.
    assert_eq!(player.engine_position().row, 9);
    assert_eq!(state(&player, 7).period, expected[0]);
}

// -------------------------------------------------------------------------------------------
// K — the vibrato and the volume slide, on two different remembered bytes
// -------------------------------------------------------------------------------------------

#[test]
fn k_reads_h_s_parameter_for_the_vibrato_and_alastnfo_for_the_slide() {
    // The classic trap, and the reason it is one here: `K`'s volume half is the *same body*
    // `D` uses (`+0x3f08`) and falls back on `alastnfo` (`+0x39`) — but `K`'s vibrato half
    // (`+0x4000`) reads `alasteff1` (`+0x2b`), the byte **`H`** keeps. A `D` row between an
    // `H` and a `K` therefore changes what the slide remembers and *not* what the vibrato
    // uses.
    //
    // The module below makes the difference observable: `H81` (speed 8, depth 1) sets
    // `alasteff1`, a `D10` row overwrites `alastnfo` with 0x10, and the `K01` row then slides
    // the volume down 1 a tick while vibrating with **0x81**. A port that used `alastnfo` for
    // the vibrato would vibrate with 0x10 — depth 0 — and the period would never move.
    let mut song = Song::new();
    song.instrument(8363, 64);
    song.row(&[Note::new(0, 0x40, 1, 40)]);
    song.row(&[Note::command(0, 8, 0x81)]); // H81
    song.row(&[Note::command(0, 4, 0x10)]); // D10 — alastnfo becomes 0x10
    song.row(&[Note::command(0, 11, 0x01)]); // K01
    song.row(&[Note::command(0, 11, 0x00)]); // K00 — both defaults
    let mut player = Player::new(song.build(), PlayerOptions::default());
    player.play_song(0);

    // The H row. `H` has no first-tick handler at all, so `alasteff1` is only written on the
    // row's *second* tick — the first vibrato tick.
    run_to(&mut player, 0, 1);
    assert_eq!(
        state(&player, 0).last_effect1,
        0,
        "the row's first tick runs no H"
    );
    assert_eq!(
        state(&player, 0).vibrato,
        0,
        "and row 0's own command was a note, which does not mark the phase stale"
    );
    tick(&mut player);
    assert_eq!(
        state(&player, 0).last_effect1,
        0x81,
        "the first H tick wrote it"
    );
    // The D row: alastnfo, and nothing else.
    run_to(&mut player, 0, 2);
    assert_eq!(state(&player, 0).last_info, 0x10, "D wrote alastnfo");
    assert_eq!(
        state(&player, 0).last_effect1,
        0x81,
        "and left H's byte alone"
    );
    assert_eq!(
        state(&player, 0).volume,
        41,
        "D10's own first tick slid it once"
    );
    for _ in 1..6 {
        tick(&mut player);
    }
    let after_d = state(&player, 0).volume;
    assert_eq!(after_d, 46, "and the five after it made six in all");

    // The K row: the slide goes down 1 a tick, and the vibrato is H's 0x81.
    run_to(&mut player, 0, 3);
    let base = state(&player, 0).org_period;
    assert_eq!(base, 1712, "the note's own period at the 8363 reference");
    let start = state(&player, 0);
    assert_eq!(
        start.volume, after_d,
        "K01's first tick runs nothing: `K` is in the second table only"
    );
    assert_eq!(start.period, base, "and its vibrato half did not run yet");

    let mut position = start.vibrato;
    let mut volume = start.volume;
    let mut walked = Vec::new();
    for step in 1..=5 {
        let (next, period) = vibrato_tick(position, 0x81, base);
        volume -= 1;
        tick(&mut player);
        assert_eq!(
            state(&player, 0).volume,
            volume,
            "K01, tick {step}: the slide"
        );
        assert_eq!(
            state(&player, 0).period,
            period,
            "K01, tick {step}: the vibrato used H's 0x81"
        );
        assert_eq!(state(&player, 0).vibrato, next);
        position = next;
        walked.push(period);
    }
    println!("  K01 walked the period {walked:?} about {base}");
    assert!(
        walked.iter().any(|&period| period != base),
        "the vibrato moved the period — which the slide's byte could never have done"
    );

    // The `K00` row takes the slide's default from `alastnfo` (0x01, written by the K row's
    // own first-tick pass) and the vibrato's from `alasteff1`.
    run_to(&mut player, 0, 4);
    assert_eq!(
        state(&player, 0).info,
        0,
        "the K00 row's own first tick runs nothing"
    );
    tick(&mut player);
    assert_eq!(
        state(&player, 0).info,
        0x01,
        "and the slide's default comes from alastnfo — written back into the cell"
    );
    assert_eq!(
        state(&player, 0).last_effect1,
        0x81,
        "while the vibrato's is H's"
    );
}

#[test]
fn k_leaves_a_depth_zero_period_alone() {
    // The other half of the share: when no `H` has ever run, `alasteff1` is 0 — so `K01`'s
    // vibrato half has a depth of 0 and moves nothing, while its slide half still slides. The
    // module is written here because a *seek* clears every channel (`+0x5614`): MUSIC0's
    // shipped `K` rows sit in patterns whose channel state comes from an earlier order, and
    // this test reaches its order with `play_song`.
    let mut song = Song::new();
    song.instrument(8363, 64);
    song.row(&[Note::new(0, 0x40, 1, 40)]);
    song.row(&[Note::command(0, 11, 0x01)]); // K01, with no H before it
    let mut player = Player::new(song.build(), PlayerOptions::default());
    player.play_song(0);
    run_to(&mut player, 0, 1);
    let before = state(&player, 0);
    assert_eq!(before.last_effect1, 0, "no H has ever run on this channel");
    assert_eq!(before.org_period, 1712, "a note from row 0 has");
    assert_eq!(before.period, before.org_period);

    let mut volume = before.volume;
    let speed = player.state().speed;
    // The first tick runs nothing at all: `K` has no first-tick handler in either table.
    assert_eq!(state(&player, 0).volume, volume, "K01's own first tick");
    for step in 1..speed {
        volume = volume.saturating_sub(1);
        tick(&mut player);
        assert_eq!(state(&player, 0).volume, volume, "K01 slides, tick {step}");
        assert_eq!(
            state(&player, 0).period,
            before.org_period,
            "and a depth-0 vibrato leaves the period"
        );
    }
}

// -------------------------------------------------------------------------------------------
// O — the sample offset
// -------------------------------------------------------------------------------------------

#[test]
fn o_starts_the_next_note_at_the_offset_in_bytes() {
    // `Oxy` (`+0x48d6`) writes `info << 8` into the channel's address field, and the next note
    // to start takes it as its sample position (`+0x491e`). MUSIC0 order 1 (pattern 1) row 0
    // channel 2 is `O05` with note 0x30 and instrument 4 — so the note starts at byte
    // 5 × 256 = **1280** of the sample (an 8-bit sample: samples).
    let m0 = module("MUSIC0");
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(1);
    run_to(&mut player, 1, 0);
    let now = state(&player, 2);
    assert_eq!(now.offset, 5 * 256, "the offset the O cell set");
    assert_eq!(
        now.position,
        5 * 256,
        "and the position the note starts from"
    );
    assert_eq!(now.period, 3424, "note 0x30 at the 8363 reference");
    // `aequ` is *not* set by a note on a channel that was not playing: `+0x491a` is on the
    // other side of `+0x490e`'s test, so a fresh channel keeps whatever it had — zero here.
    // Only a note that restarts a playing sample sets the 0xff the tail turns into
    // `avol + 0x10` (`+0x496a`).
    assert_eq!(now.aequ, 0, "a first note leaves aequ alone");

    // The 256-multiple form, from the module's own data: MUSIC0 pattern 11 (order 12) row 12
    // channel 0 is `O08` → 2048. It has no note of its own, so the offset waits for the next
    // one; the test asserts the field the command wrote.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(12);
    run_to(&mut player, 11, 12);
    assert_eq!(state(&player, 0).offset, 8 * 256);
}

#[test]
fn o_without_an_instrument_leaves_the_last_offset_in_place() {
    // `+0x48ce`: the note-and-offset block is behind `lastins != 0`, and only a cell that
    // names an instrument clears the offset (`+0x481d`, inside the instrument load). So an
    // `O` on a cell that names no instrument leaves the previous offset for the next note.
    // Neither module has such a cell — the count below is over every `O` cell of both files —
    // so the module is written here.
    let mut song = Song::new();
    song.instrument(8363, 64);
    song.row(&[Note::new(0, 0x40, 1, 40)]);
    song.row(&[Note::command(0, 15, 0x03)]); // O03 with no instrument: 768
    song.row(&[Note::new(0, 0x41, 0, 50)]); // the next note, no instrument of its own
    let mut player = Player::new(song.build(), PlayerOptions::default());
    player.play_song(0);
    run_to(&mut player, 0, 1);
    assert_eq!(state(&player, 0).offset, 3 * 256);
    run_to(&mut player, 0, 2);
    assert_eq!(
        state(&player, 0).offset,
        3 * 256,
        "the offset was not cleared"
    );
    assert_eq!(
        state(&player, 0).position,
        3 * 256,
        "and the note started from it"
    );

    // The count the claim above rests on.
    for name in ["MUSIC0", "MUSIC1"] {
        let module = module(name);
        let mut without = 0;
        let mut total = 0;
        for index in 0..module.pattern_count() {
            for cells in &module.pattern(index).expect("a pattern").rows {
                for cell in cells {
                    if cell.has_command() && cell.command == 15 {
                        total += 1;
                        if cell.instrument == 0 {
                            without += 1;
                        }
                    }
                }
            }
        }
        println!("  {name}: {total} O cell(s), {without} without an instrument");
        assert_eq!(without, 0, "{name}");
    }
}

// -------------------------------------------------------------------------------------------
// Q — the retrigger
// -------------------------------------------------------------------------------------------

#[test]
fn q_retriggers_when_its_interval_is_reached() {
    // MUSIC0, order 36 (pattern 23), channel 7, row 58: note 0x42 with instrument 6, a volume
    // of 1 and `QC4` — the interval is the **low** nibble (4) and the volume code the **high**
    // one (12, the addend table's +8).
    //
    // `+0x4205` fires when the interval has been reached, and the counter is incremented at
    // the end of every call (`+0x4246`): starting from 0 — row 57 has no command, and a
    // channel with no command has its counter cleared by the first-tick pass at `+0x3d0e` —
    // the fires land on the ticks congruent to 0 mod 4. Only a `D` row or a row with no
    // command clears that counter, so the phase runs across rows.
    let m0 = module("MUSIC0");
    assert_eq!(m0.instrument(5).expect("instrument 6").c2spd, 14600);
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(36);
    run_to_row(&mut player, 23, 58);
    let speed = player.state().speed;
    assert_eq!(speed, 6, "the rows in this pattern");
    let before = state(&player, 7);
    assert_eq!(before.retrigger, 0, "the counter row 57's pass cleared");
    assert_ne!(
        before.period,
        scaled(note_period(0x42), 14600),
        "the row's note has not started yet"
    );

    // Each tick, in order: the note's period and volume on the first, then the retrigger's.
    let mut count = before.retrigger;
    let mut volume = 0u8;
    let mut volumes = Vec::new();
    for step in 0..speed {
        if step == 0 {
            volume = 1; // the cell's own volume column, applied by the note start
        }
        let (next, fired) = retrigger_tick(count, 0xc4);
        if fired {
            volume += 8;
        }
        count = next;
        tick(&mut player);
        assert_eq!(state(&player, 7).volume, volume, "tick {step}: the volume");
        assert_eq!(
            state(&player, 7).retrigger,
            count,
            "tick {step}: the counter"
        );
        if step == 0 {
            assert_eq!(
                state(&player, 7).period,
                scaled(note_period(0x42), 14600),
                "the note started on the row's first tick"
            );
        }
        volumes.push(volume);
    }
    assert_eq!(volumes, vec![1, 1, 1, 1, 9, 9], "QC4's own row");

    // The next three rows are `Q00`, so the parameter is remembered (0xc4) and the fires keep
    // coming every fourth tick — two a row, from wherever the counter was.
    let mut volumes = Vec::new();
    for row in 0..3u8 {
        assert_eq!(player.engine_position().row, 59 + row);
        for step in 0..speed {
            let (next, fired) = retrigger_tick(count, 0xc4);
            if fired {
                volume += 8;
            }
            count = next;
            tick(&mut player);
            assert_eq!(
                state(&player, 7).volume,
                volume,
                "row {} tick {step}",
                59 + row
            );
            assert_eq!(state(&player, 7).retrigger, count);
            volumes.push(volume);
        }
    }
    assert_eq!(
        volumes,
        vec![
            9, 9, 17, 17, 17, 17, //
            25, 25, 25, 25, 33, 33, //
            33, 33, 41, 41, 41, 41,
        ],
        "the fires every fourth tick, over the row boundaries"
    );
}

#[test]
fn q_multiplies_when_its_code_has_a_multiplier() {
    // MUSIC0, order 24 (pattern 47), channel 2, row 56: note 0x35, instrument 6, volume 1 and
    // `QF4` — the interval 4 with the volume code 15, whose multiplier is ×32/16.
    //
    // The multiplier table at `+0x96c` holds real bytes only at 6, 7, 14 and 15 (10, 8, 24
    // and 32); everywhere else the object image carries the `0x0101` filler, which at runtime
    // is zero and sends those codes down the *addend* path. So this cell doubles the volume on
    // every retrigger, and the multiply clamps at 63 the way the addend does.
    let m0 = module("MUSIC0");
    let mut player = Player::new(m0, PlayerOptions::default());
    player.play_song(24);
    run_to_row(&mut player, 47, 56);
    let speed = player.state().speed;
    assert_eq!(speed, 8, "pattern 47's rows run eight ticks");
    let before = state(&player, 2);
    assert_ne!(
        before.volume, 1,
        "the note's own volume column has not applied yet"
    );
    assert_eq!(before.position, 0, "and the note's position has not either");

    let mut count = before.retrigger;
    let mut volume = 0i32;
    let mut volumes = Vec::new();
    for step in 0..speed {
        if step == 0 {
            volume = 1;
        }
        let (next, fired) = retrigger_tick(count, 0xf4);
        if fired {
            volume = ((volume * 32) >> 4).min(63);
        }
        count = next;
        tick(&mut player);
        assert_eq!(state(&player, 2).volume, volume as u8, "tick {step}");
        volumes.push(volume as u8);
    }
    assert_eq!(
        volumes,
        vec![1, 1, 1, 1, 2, 2, 2, 2],
        "×2 on the tick the interval reaches, and nothing on the three after it"
    );

    // Four more rows of `Q00`: 4, 8, 16, 32 and then the clamp.
    let mut volumes = Vec::new();
    for row in 0..4u8 {
        assert_eq!(player.engine_position().row, 57 + row);
        for step in 0..speed {
            let (next, fired) = retrigger_tick(count, 0xf4);
            if fired {
                volume = ((volume * 32) >> 4).min(63);
            }
            count = next;
            tick(&mut player);
            assert_eq!(
                state(&player, 2).volume,
                volume as u8,
                "row {} tick {step}",
                57 + row
            );
            volumes.push(volume as u8);
        }
    }
    assert_eq!(
        volumes,
        vec![
            4, 4, 4, 4, 8, 8, 8, 8, //
            16, 16, 16, 16, 32, 32, 32, 32, //
            63, 63, 63, 63, 63, 63, 63, 63, //
            63, 63, 63, 63, 63, 63, 63, 63,
        ],
        "the doubling, four ticks apart, and the clamp at 63"
    );
}

// -------------------------------------------------------------------------------------------
// S — the note delay
// -------------------------------------------------------------------------------------------

#[test]
fn sdx_delays_the_note_to_its_own_tick() {
    // The three delays the two modules carry, driven from their own cells. `SDx` puts the low
    // nibble into the channel's `atrigcnt` on the row's first tick (`+0x4296`), counts it down
    // on every later tick (`+0x429a`) and starts the deferred note when it reaches zero — so
    // the note begins on tick `x`, and a row with not enough ticks never starts it at all.
    //
    // MUSIC0's `SD6` rows are that case: pattern 40 (order 44) is `SD6` and MUSIC0 runs at
    // speed 6, so the ticks are 0…5 and tick 6 does not exist — **the note never starts**, and
    // the channel keeps the period it had.
    for (name, order, pattern, row, channel, delay) in [
        ("MUSIC0", 45u8, 41usize, 0usize, 5usize, 3u8),
        ("MUSIC0", 44, 40, 1, 5, 6),
        ("MUSIC1", 42, 30, 2, 3, 2),
    ] {
        let module = module(name);
        let note = cell(&module, pattern, row, channel);
        let c2spd = cell_c2spd(&module, &note);
        let wanted = scaled(note_period(note.note), c2spd);
        let mut player = Player::new(module, PlayerOptions::default());
        player.play_song(order);
        run_to(&mut player, pattern as u8, row as u8);
        let speed = u16::from(player.state().speed);
        let before = state(&player, channel);
        println!(
            "  {name} order {order}: SD{delay} on pattern {pattern} row {row} channel {channel}, \
             note {:#04x} wants {wanted}, speed {speed}",
            note.note
        );
        assert_ne!(before.period, wanted, "the deferred note has not started");
        assert_eq!(before.retrigger, delay, "the countdown is the low nibble");

        let mut fired = None;
        for tick_number in 0..speed {
            if tick_number > 0 {
                tick(&mut player);
            }
            if state(&player, channel).period == wanted {
                fired = Some(tick_number);
                break;
            }
        }
        match fired {
            Some(tick_number) => {
                assert_eq!(tick_number, u16::from(delay), "{name}: the note's tick");
                assert_eq!(
                    state(&player, channel).volume,
                    note.volume,
                    "{name}: and the note brought the cell's own volume column"
                );
                println!("    it fired on tick {tick_number}");
            }
            None => {
                assert!(
                    u16::from(delay) >= speed,
                    "{name}: SD{delay} should have fired inside a {speed}-tick row"
                );
                assert_eq!(
                    state(&player, channel).volume,
                    before.volume,
                    "{name}: the deferred note's volume was never applied either"
                );
                println!("    it never fired: the row is {speed} ticks long");
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// The periods, one octave at a time
// -------------------------------------------------------------------------------------------

#[test]
fn every_octave_of_the_note_table_is_the_objects_own() {
    // The object's table at `+0xa24` is four octaves above the S3M table the format documents
    // and `+0x594a` shifts it right by one byte per octave from `+0x97c`. This is the whole
    // table, one octave a line, against the port's own note→period.
    let octave4: [u16; 12] = [
        1712, 1616, 1524, 1440, 1356, 1280, 1208, 1140, 1076, 1016, 960, 907,
    ];
    for octave in 0..8u8 {
        let mut row = Vec::new();
        for semitone in 0..12u8 {
            let note = (octave << 4) | semitone;
            let expected = PERIODS[usize::from(semitone)] >> SHIFTS[usize::from(octave)];
            assert_eq!(
                note_period(note),
                expected,
                "the test's own arithmetic, note {note:#04x}"
            );
            assert_eq!(
                sr_audio::note_period(note),
                expected,
                "the port's note {note:#04x}"
            );
            // And the S3M table itself, shifted the way the format documents it: octave 4 is
            // the table, each octave *below* doubles the period and each octave above halves
            // it (`+0x594a`'s shift).
            let table = octave4[usize::from(semitone)];
            assert_eq!(
                expected,
                if octave <= 4 {
                    table << (4 - octave)
                } else {
                    table >> (octave - 4)
                },
                "note {note:#04x} against the S3M table"
            );
            row.push(expected);
        }
        println!("  octave {octave}: {row:?}");
    }

    // The top of the range is not a refusal: octave 8 reads the byte after the shift table,
    // which is 0, so MUSIC1's note 0x80 plays octave 0's period.
    assert_eq!(sr_audio::note_period(0x80), 27392);
    assert_eq!(sr_audio::note_period(0xfe), 0, "ST3's note off");
}

#[test]
fn a_period_s_step_is_the_base_frequency_over_it() {
    // `+0x582b`'s own constant, `0x00da7600` = 14,317,056, over the table's periods — one
    // value an octave, the step the mixer will divide by its rate (Task 5's).
    for (octave, period) in [(0u8, 27392u16), (1, 13696), (2, 6848), (3, 3424), (4, 1712)] {
        assert_eq!(
            sr_audio::period_step(period),
            period_step(period),
            "octave {octave}: {period} to a step"
        );
        println!(
            "  octave {octave}: period {period} steps {}",
            period_step(period)
        );
    }
    assert_eq!(sr_audio::period_step(0), 0);
    // C-4 at the 8363 reference steps at 8362: the object's constant is 400 below
    // 8363 × 1712 = 14,317,456, so the format's two reference numbers do not multiply out.
    assert_eq!(period_step(1712), 8362);
}

#[test]
fn a_notes_period_is_scaled_by_its_instruments_c2spd() {
    // `+0x5813`: `period × 8363/c2spd`. The shipped modules carry instruments on both sides of
    // the reference — MUSIC0's instrument 4 is the reference itself — which is what makes the
    // scaling visible: the same note at two instruments is two periods.
    let m0 = module("MUSIC0");
    assert_eq!(m0.instrument(3).expect("instrument 4").c2spd, 8363);
    assert_eq!(
        sr_audio::scaled_period(1712, 8363),
        1712,
        "the reference is the identity"
    );
    for (note, instrument) in [(0x30u8, 6usize), (0x40, 4), (0x42, 5)] {
        let c2spd = m0
            .instrument(instrument)
            .expect("an instrument")
            .c2spd
            .min(0xffff) as u16;
        assert_eq!(
            sr_audio::scaled_period(note_period(note), c2spd),
            scaled(note_period(note), c2spd),
            "note {note:#04x} at instrument {}",
            instrument + 1
        );
    }
    // The guards at `+0x581f` and `+0x5825`: a product whose high word reaches the divisor
    // answers 0x7fff rather than overflowing a word.
    assert_eq!(sr_audio::scaled_period(0x7fff, 1), 0x7fff);
    assert_eq!(scaled(0x7fff, 1), 0x7fff);

    // The range the scaling actually works over, over *every* PCM instrument of both files:
    // the two files' `c2spd` fields run from 8163 to 27700, so the same note is a period in
    // one channel and nearly three and a half times it in another.
    let mut lowest = u32::MAX;
    let mut highest = 0u32;
    let mut counted = 0usize;
    for name in ["MUSIC0", "MUSIC1"] {
        let module = module(name);
        for index in 0..module.instrument_count() {
            let instrument = module.instrument(index).expect("an instrument");
            if instrument.is_pcm() {
                lowest = lowest.min(instrument.c2spd);
                highest = highest.max(instrument.c2spd);
                counted += 1;
            }
        }
    }
    println!("  {counted} PCM instrument(s): c2spd {lowest}..{highest}");
    assert_eq!((lowest, highest, counted), (8163, 27700, 63));
}

// -------------------------------------------------------------------------------------------
// A module of the test's own, for the forms the shipped data does not reach
// -------------------------------------------------------------------------------------------

/// One cell of a test module's row.
#[derive(Clone, Copy)]
struct Note {
    channel: u8,
    note: u8,
    instrument: u8,
    volume: u8,
    command: u8,
    info: u8,
    flags: u8,
}

impl Note {
    /// A note with an instrument and a volume column.
    fn new(channel: u8, note: u8, instrument: u8, volume: u8) -> Note {
        Note {
            channel,
            note,
            instrument,
            volume,
            command: 0,
            info: 0,
            flags: Cell::NOTE | Cell::VOLUME,
        }
    }

    /// A command with no note and no volume column.
    fn command(channel: u8, command: u8, info: u8) -> Note {
        Note {
            channel,
            note: 0xff,
            instrument: 0,
            volume: 0xff,
            command,
            info,
            flags: Cell::COMMAND,
        }
    }
}

/// A one-pattern, one-order module with its own instruments, built the way `tests/player.rs`'s
/// builder builds one: a header, the tables and paragraph-aligned blocks, with the pattern data
/// scrambled by the same keystream the loader undoes.
struct Song {
    rows: Vec<Vec<Note>>,
    instruments: Vec<(u32, u8)>,
}

impl Song {
    fn new() -> Song {
        Song {
            rows: Vec::new(),
            instruments: Vec::new(),
        }
    }

    fn instrument(&mut self, c2spd: u32, volume: u8) {
        self.instruments.push((c2spd, volume));
    }

    /// Append a row. The rows are appended in order, so the call order *is* the row order.
    fn row(&mut self, notes: &[Note]) {
        self.rows.push(notes.to_vec());
    }

    fn build(self) -> Module {
        let mut bytes = vec![0u8; 0x60];
        bytes[28] = 26;
        bytes[29] = 16;
        bytes[32..34].copy_from_slice(&1u16.to_le_bytes()); // one order
        bytes[34..36].copy_from_slice(&(self.instruments.len() as u16).to_le_bytes());
        bytes[36..38].copy_from_slice(&1u16.to_le_bytes()); // one pattern
        bytes[44..48].copy_from_slice(b"SCRM");
        bytes[48] = 64;
        bytes[49] = 6;
        bytes[50] = 125;
        bytes[51] = 176;
        bytes[64..96].fill(255);
        bytes.push(0); // the order list

        let instruments_at = bytes.len();
        bytes.resize(instruments_at + 2 * self.instruments.len(), 0);
        let patterns_at = bytes.len();
        bytes.resize(patterns_at + 2, 0);

        // The instrument headers: PCM, length 0 (no sample bytes to slice), the volume and
        // c2spd asked for. The player's instrument load is what reads them.
        let mut placed = Vec::new();
        for (c2spd, volume) in &self.instruments {
            let mut header = [0u8; 0x50];
            header[0] = 1;
            header[0x1C] = *volume;
            header[0x20..0x24].copy_from_slice(&c2spd.to_le_bytes());
            header[0x4C..0x50].copy_from_slice(b"SCRS");
            let pad = (16 - bytes.len() % 16) % 16;
            bytes.extend(std::iter::repeat_n(0u8, pad));
            let pointer = (bytes.len() / 16) as u16;
            bytes.extend_from_slice(&header);
            placed.push(pointer);
        }

        let mut data = Vec::new();
        for row in &self.rows {
            for note in row {
                let mut flag = note.channel;
                if note.flags & Cell::NOTE != 0 {
                    flag |= 0x20;
                }
                if note.flags & Cell::VOLUME != 0 {
                    flag |= 0x40;
                }
                if note.flags & Cell::COMMAND != 0 {
                    flag |= 0x80;
                }
                data.push(flag);
                if note.flags & Cell::NOTE != 0 {
                    data.push(note.note);
                    data.push(note.instrument);
                }
                if note.flags & Cell::VOLUME != 0 {
                    data.push(note.volume);
                }
                if note.flags & Cell::COMMAND != 0 {
                    data.push(note.command);
                    data.push(note.info);
                }
            }
            data.push(0); // the row's terminator
        }
        data.resize(data.len() + ROWS - self.rows.len(), 0);
        descramble(&mut data);
        let mut block = ((data.len() + 2) as u16).to_le_bytes().to_vec();
        block.extend_from_slice(&data);
        let pad = (16 - bytes.len() % 16) % 16;
        bytes.extend(std::iter::repeat_n(0u8, pad));
        let pattern_pointer = (bytes.len() / 16) as u16;
        bytes.extend_from_slice(&block);

        for (index, pointer) in placed.iter().enumerate() {
            bytes[instruments_at + 2 * index..instruments_at + 2 * index + 2]
                .copy_from_slice(&pointer.to_le_bytes());
        }
        bytes[patterns_at..patterns_at + 2].copy_from_slice(&pattern_pointer.to_le_bytes());
        Module::load(&bytes).expect("the built module loads")
    }
}
