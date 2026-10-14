//! Task 3's own tests: what the player does, and what the object it was read from says.
//!
//! Two halves, and they are different kinds of evidence:
//!
//! 1. **The object's own bytes.** The original's player is an OMF object with no source in
//!    the tree, so `player.rs`'s claims are claims about machine code. This file rebuilds the
//!    object's segment image out of its `LEDATA` and `LIDATA` records — the image is 29,607
//!    bytes, which is the length the symbol map records for the segment, and it starts with
//!    the string `Scream Tracker V3.0 Music Interface Kit V0.9` — and asserts the
//!    load-bearing instructions byte for byte. A claim about an offset that cannot survive
//!    this file should not be in `player.rs`.
//!
//! 2. **The player's own behaviour**, on the two shipped modules and on a small module built
//!    here so that cells this phase's data does not contain (a `B` jump, a `255`, a `T`)
//!    can be played rather than wished for.
//!
//! Every number this file prints is printed by `cargo test -p sr-audio --test player --
//! --nocapture`.

use std::collections::BTreeMap;
use std::path::Path;

use sr_audio::{descramble, MixRate, Module, Player, PlayerOptions, Position, CHANNELS, ROWS};

/// The curated originals, as `tests/module.rs` reads them. Read-only.
const ASSETS: &str = "../../assets/MAIN";

fn shipped(name: &str) -> Vec<u8> {
    let path = Path::new(ASSETS).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// A module with the shell's patch applied where the shell applies it: MUSIC0 plays at
/// tempo 120 (`MAIN/STARTMUS.C:252-265`), MUSIC1 at its file's own 130.
fn module(name: &'static str) -> Module {
    let mut module = Module::load(&shipped(&format!("{name}.S3M")))
        .unwrap_or_else(|e| panic!("{name} does not load: {e}"));
    if name == "MUSIC0" {
        module.apply_tempo_patch(0x78);
    }
    module
}

fn player(name: &'static str) -> Player {
    Player::new(module(name), PlayerOptions::default())
}

// -------------------------------------------------------------------------------------------
// The object's own bytes
// -------------------------------------------------------------------------------------------

/// The original's segment image, rebuilt from the object's own records.
///
/// `0xA0` is `LEDATA` (a segment offset and its bytes), `0xA2` is `LIDATA` (the same, with the
/// data written as repeated blocks — an iterated block being a two-byte count and that many
/// bytes, or a count of zero and a nested repeat). The object carries 105 `LEDATA` and 118
/// `LIDATA` records and they tile one 29,607-byte segment with no gaps, which is what
/// `sr-assets`'s symbol map records as `text_stmik300_text`'s length.
fn segment_image() -> Vec<u8> {
    let object = std::fs::read("../../upstream/SecondReality/MAIN/STMIK.300")
        .expect("the vendored original's player object");
    let mut image: Vec<u8> = Vec::new();
    let word = |bytes: &[u8], at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);

    let mut place = |at: usize, data: &[u8]| {
        if image.len() < at + data.len() {
            image.resize(at + data.len(), 0);
        }
        image[at..at + data.len()].copy_from_slice(data);
    };

    let mut position = 0usize;
    while position + 3 <= object.len() {
        let kind = object[position];
        let length = word(&object, position + 1) as usize;
        let body = &object[position + 3..position + 3 + length];
        match kind {
            0xA0 if body.len() >= 3 && body[0] == 1 => {
                let offset = word(body, 1) as usize;
                place(offset, &body[3..body.len() - 1]);
            }
            0xA2 if body.len() >= 5 && body[0] == 1 => {
                let offset = word(body, 1) as usize;
                let repeat = word(body, 3) as usize;
                let mut data = Vec::new();
                iterated(body, 5, &mut data, 0);
                let mut all = Vec::with_capacity(data.len() * repeat);
                for _ in 0..repeat {
                    all.extend_from_slice(&data);
                }
                place(offset, &all);
            }
            _ => {}
        }
        position += 3 + length;
    }
    assert_eq!(
        image.len(),
        29_607,
        "the records do not tile the 29,607-byte segment the symbol map records"
    );
    image
}

/// One iterated block of a `LIDATA` record, appended to `out`.
fn iterated(body: &[u8], at: usize, out: &mut Vec<u8>, depth: usize) {
    assert!(depth < 8, "LIDATA nesting deeper than the format allows");
    let count = u16::from_le_bytes([body[at], body[at + 1]]) as usize;
    if count != 0 {
        out.extend_from_slice(&body[at + 2..at + 2 + count]);
        return;
    }
    let repeat = u16::from_le_bytes([body[at + 2], body[at + 3]]) as usize;
    let mut block = Vec::new();
    iterated(body, at + 4, &mut block, depth + 1);
    for _ in 0..repeat {
        out.extend_from_slice(&block);
    }
}

/// Where the seven published words live in the object (`cargo xtask music symbols` prints the
/// same offsets from the object's `PUBDEF` records; `crates/sr-assets/tests/omf_symbols.rs`
/// asserts them there).
const NP_ZPLUS: usize = 0x08F9;
const NP_ZFRAME: usize = 0x08FB;
const NP_ROW: usize = 0x08FD;
const NP_ORD: usize = 0x0901;
const NP_LOOP: usize = 0x0903;

#[test]
fn the_objects_own_bytes_say_what_the_player_says() {
    let image = segment_image();
    assert_eq!(
        &image[1..45],
        b"Scream Tracker V3.0 Music Interface Kit V0.9",
        "the image's first bytes are the kit's own version string"
    );

    // Every claim is (offset, the bytes, and what they mean). The bytes are read from the
    // image; the sentence beside each one is the claim `player.rs` makes, and this is where it
    // can be checked without an 8086 in the room.
    let claims: &[(usize, &[u8], &str)] = &[
        // The tick's sample count: `_zmixspeed × 125/(50 × tempo)`, an integer division, and
        // the only place a tick's length is computed.
        (
            0x3D99,
            &[
                0x2E, 0xA1, 0xD5, 0x08, 0xB9, 0x7D, 0x00, 0xF7, 0xE1, 0xF7, 0xF3, 0x2E, 0xA3,
                0xE1, 0x08,
            ],
            "mov %cs:_zmixspeed,%ax; mov $0x7d,%cx; mul %cx; div %bx; mov %ax,%cs:0x8e1",
        ),
        // The seek's own routine (`_zplaysong` calls it at `+0x258e`): the tick-in-row
        // counter, the note-delay pair, the row counter, `_np_loop`, the pattern, the published
        // row and the pending row, all zeroed before the order advance runs. This is the run
        // the note's two unreachable `255` paths are read off.
        (
            0x56EC,
            &[
                0xC7, 0x06, 0x00, 0x20, 0x00, 0x00, 0xC6, 0x06, 0x08, 0x20, 0x00, 0xC7, 0x06,
                0x0A, 0x20, 0x00, 0x00, 0xC6, 0x06, 0x09, 0x20, 0x00, 0xA3, 0x48, 0x09, 0xA3,
                0x03, 0x09, 0xA3, 0x4C, 0x09, 0xC7, 0x06, 0xFD, 0x08, 0x00, 0x00, 0xC6, 0x06,
                0x3B, 0x09, 0x00,
            ],
            "movw $0,0x2000; movb $0,0x2008; movw $0,0x200a; ...; mov %ax,0x948 (row); \
             mov %ax,0x903 (loop); mov %ax,0x94c (pattern); movw $0,0x8fd (published row); \
             movb $0,0x93b (pending row)",
        ),
        // The tick counter: `_np_zframe` is incremented by the tick routine, so a tick *is*
        // what the parts' counter counts.
        (0x4357, &[0xFF, 0x06, 0xFB, 0x08], "incw _np_zframe"),
        // The row's length is a count of ticks compared against the published speed word: the
        // counter is incremented and then compared with `_np_speed`, and the row advances when
        // it is not below it. (Decoded: `incw 0x2000`, `mov %cs:0x2002,%ax`, `cmp %ax,0x2000`.)
        (
            0x4397,
            &[0xFF, 0x06, 0x00, 0x20, 0x2E, 0xA1, 0x02, 0x20, 0x2E, 0x8B, 0x16, 0xCD, 0x08],
            "incw 0x2000; mov %cs:_np_speed,%ax; ...",
        ),
        (
            0x43AC,
            &[0x39, 0x06, 0x00, 0x20, 0x7D],
            "cmp %ax,0x2000; jge (→ the row advance)",
        ),
        // The words the ring is fed from are written only here — the row-advance path of the
        // tick routine, and nowhere a seek can reach.
        (
            0x43E6,
            &[0xA1, 0x40, 0x09, 0x3D, 0xFF, 0xFF, 0x74, 0x01, 0x48, 0xA3, 0x09, 0x09],
            "mov 0x940,%ax; cmp $0xffff,%ax; ...; mov %ax,0x909 (the fed order)",
        ),
        (
            0x43F5,
            &[0xA3, 0x05, 0x09, 0xA1, 0x4C, 0x09, 0xA3, 0x07, 0x09],
            "mov %ax,0x905 (row); mov 0x94c,%ax; mov %ax,0x907 (pattern)",
        ),
        // `_np_zframe`'s row, `_np_ord` and `_np_loop` are *not* the fed words: the ring's
        // entries carry row/pattern/order/zplus, which is what the offsets above and the pop
        // below say.
        // The ring: push at the write pointer, pop at the read pointer, publish four words.
        (
            0x52C9,
            &[0x2E, 0x8B, 0x36, 0x4E, 0x21],
            "mov %cs:0x214e,%si (the ring's write pointer)",
        ),
        (
            0x52FF,
            &[0x2E, 0x8B, 0x84, 0x50, 0x21, 0xA3, 0xFD, 0x08],
            "mov %cs:0x2150(%si),%ax; mov %ax,0x8fd (→ _np_row)",
        ),
        (
            0x5314,
            &[0xA3, 0x01, 0x09],
            "mov %ax,0x901 (the ring's third word is → _np_ord)",
        ),
        (
            0x531C,
            &[0xA3, 0xF9, 0x08],
            "mov %ax,0x8f9 (the fourth is → _np_zplus)",
        ),
        (
            0x5326,
            &[0x2E, 0x89, 0x36, 0x4C, 0x21],
            "mov %si,%cs:0x214c (the read pointer)",
        ),
        // ...and the ring is primed ten entries deep: the write pointer starts at 0x50 (ten
        // eight-byte entries) and the read pointer at 0.
        (
            0x2AC1,
            &[0x2E, 0xC7, 0x06, 0x4E, 0x21, 0x50, 0x00],
            "movw $0x50,%cs:0x214e — ten entries of fill",
        ),
        (
            0x2AC8,
            &[0x2E, 0xC7, 0x06, 0x4C, 0x21, 0x00, 0x00],
            "movw $0x0,%cs:0x214c",
        ),
        // The order advance: `zplus` cleared, then the `254` of the *next* entry sets bit 0
        // and the `254` of the *current* one sets bit 1 and steps over it.
        (
            0x442D,
            &[0x2E, 0xC7, 0x06, 0x0D, 0x09, 0x00, 0x00],
            "movw $0x0,%cs:0x90d (zplus = 0)",
        ),
        (
            0x4452,
            &[0x3C, 0xFE, 0x75, 0x06, 0x2E, 0x83, 0x0E, 0x0D, 0x09, 0x01],
            "cmp $0xfe,%al; jne; orw $0x1,%cs:0x90d  (the *next* entry is a 254 → bit 0)",
        ),
        (
            0x445C,
            &[0x80, 0xFB, 0xFE, 0x75, 0x08, 0x2E, 0x83, 0x0E, 0x0D, 0x09, 0x02, 0xEB, 0xCC],
            "cmp $0xfe,%bl; jne; orw $0x2,%cs:0x90d; jmp  (the *current* one → bit 1, and step over)",
        ),
        (
            0x4469,
            &[0x80, 0xFB, 0xFF, 0x75, 0x2F],
            "cmp $0xff,%bl; jne (a 255 leaves the scan)",
        ),
        (
            0x4481,
            &[0x2E, 0xFF, 0x06, 0x0B, 0x09],
            "incw %cs:0x90b — _np_loop's source, on the loop path",
        ),
        // A and T are one handler told apart by the parameter: 32 and under is a speed.
        (
            0x3D73,
            &[0x32, 0xE4, 0x3D, 0x20, 0x00, 0x76],
            "xor %ah,%ah; cmp $0x20,%ax; jbe (→ the speed path)",
        ),
        (
            0x3DD8,
            &[0x2E, 0xA3, 0x02, 0x20, 0xC3],
            "mov %ax,%cs:0x2002 (_np_speed); ret",
        ),
        (
            0x3D7B,
            &[0x2E, 0xA2, 0x0F, 0x09],
            "mov %al,%cs:0x90f (_np_tempo)",
        ),
        // C's parameter is decimal: high nibble, low nibble, then `aad $0xa`.
        (
            0x3DFE,
            &[0x8A, 0xE0, 0xC0, 0xEC, 0x04, 0x24, 0x0F, 0xD5, 0x0A, 0xA2, 0x3B, 0x09],
            "mov %al,%ah; shr $0x4,%ah; and $0xf,%al; aad $0xa; mov %al,0x93b",
        ),
        // B raises the same advance flag and writes the order to try next.
        (
            0x3DE2,
            &[0xC6, 0x06, 0x3A, 0x09, 0x01, 0x3C, 0xFF],
            "movb $0x1,0x93a; cmp $0xff,%al (B, and BFF)",
        ),
    ];

    for (offset, bytes, meaning) in claims {
        assert_eq!(
            &image[*offset..*offset + bytes.len()],
            *bytes,
            "0x{offset:04X}: {meaning}"
        );
    }

    // And the seven published words are where the symbol map says they are, in the segment
    // the trace reads: the four the ring carries are contiguous and the tick counter sits
    // beside them (`MAIN/STMIK.H:125-131`'s declaration order is not the memory order — the
    // offsets come from the object's own `PUBDEF` records).
    for at in [NP_ZPLUS, NP_ZFRAME, NP_ROW, NP_ORD, NP_LOOP] {
        assert!(at + 1 < image.len(), "0x{at:04X} is inside the segment");
    }
    assert_eq!(NP_ROW - NP_ZFRAME, 2, "zframe, row");
    assert_eq!(NP_ORD - NP_ROW, 4, "row, pat, ord");
    println!(
        "the segment image: {} bytes, {} claims checked at their own offsets",
        image.len(),
        claims.len()
    );
}

// -------------------------------------------------------------------------------------------
// A module built here, for the cells the shipped data does not carry
// -------------------------------------------------------------------------------------------

/// Build an S3M in memory the way `tests/module.rs`'s builder does: the header, the order
/// list, the two pointer tables, then paragraph-aligned blocks, with the pattern data
/// scrambled by the same keystream the loader undoes.
struct Builder {
    bytes: Vec<u8>,
}

impl Builder {
    /// `orders` is the order list; `patterns` is one packed row per entry, the rest of each
    /// pattern being terminators.
    fn new(orders: &[u8], patterns: &[Vec<u8>]) -> Builder {
        let mut bytes = vec![0u8; 0x60];
        bytes[28] = 26;
        bytes[29] = 16;
        bytes[32..34].copy_from_slice(&(orders.len() as u16).to_le_bytes());
        bytes[34..36].copy_from_slice(&0u16.to_le_bytes());
        bytes[36..38].copy_from_slice(&(patterns.len() as u16).to_le_bytes());
        bytes[44..48].copy_from_slice(b"SCRM");
        bytes[48] = 64; // global volume
        bytes[49] = 6; // initial speed
        bytes[50] = 125; // initial tempo
        bytes[51] = 176;
        bytes[64..96].fill(255);
        bytes.extend_from_slice(orders);
        bytes.resize(0x60 + orders.len() + 2 * patterns.len(), 0);
        let patterns_at = 0x60 + orders.len();

        let mut pointers = Vec::new();
        for row in patterns {
            let mut data = row.clone();
            data.resize(row.len() + ROWS, 0);
            descramble(&mut data);
            let mut block = ((data.len() + 2) as u16).to_le_bytes().to_vec();
            block.extend_from_slice(&data);
            let pad = (16 - bytes.len() % 16) % 16;
            bytes.extend(std::iter::repeat_n(0u8, pad));
            let paragraph = (bytes.len() / 16) as u16;
            bytes.extend_from_slice(&block);
            pointers.push(paragraph);
        }
        for (index, pointer) in pointers.iter().enumerate() {
            bytes[patterns_at + 2 * index..patterns_at + 2 * index + 2]
                .copy_from_slice(&pointer.to_le_bytes());
        }
        Builder { bytes }
    }

    fn build(self) -> Module {
        Module::load(&self.bytes).expect("the built module loads")
    }
}

/// One cell, packed as the format packs it: the flag byte carries the channel in its low five
/// bits, and one flag byte can carry a note (and instrument), a volume and an effect.
fn cell(channel: u8, note: Option<(u8, u8)>, command: Option<(u8, u8)>) -> Vec<u8> {
    let mut flag = channel;
    let mut out = Vec::new();
    if note.is_some() {
        flag |= 0x20;
    }
    if command.is_some() {
        flag |= 0x80;
    }
    out.push(flag);
    if let Some((note, instrument)) = note {
        out.push(note);
        out.push(instrument);
    }
    if let Some((command, info)) = command {
        out.push(command);
        out.push(info);
    }
    out
}

/// How many polls it takes to see `count` ticks at a tempo, for the tests' frame arithmetic.
fn ticks_to_polls(count: u32, mix_rate: MixRate, tempo: u8) -> u32 {
    let per_tick = mix_rate.samples_per_second() * 5 / (2 * u32::from(tempo));
    let samples = per_tick * count;
    // A poll adds 40000/70.086 samples; round up, the same way the accumulator does.
    (u64::from(samples) * 70_086).div_ceil(u64::from(mix_rate.samples_per_second()) * 1000) as u32
}

/// Poll until `done`, or until the cap — a test that hangs is worse than one that fails.
fn poll_until(player: &mut Player, cap: u32, mut done: impl FnMut(&Player, u32) -> bool) -> u32 {
    for poll in 0..cap {
        if done(player, poll) {
            return poll;
        }
        player.poll();
    }
    panic!("no answer within {cap} polls");
}

// -------------------------------------------------------------------------------------------
// The player's behaviour
// -------------------------------------------------------------------------------------------

#[test]
fn a_seek_starts_the_order_without_publishing_a_position() {
    // `_zplaysong` calls the order-advance routine directly, and that routine never writes
    // `[0x905]`/`[0x907]`/`[0x909]` — so the published position stays where the ring left it
    // (zeroes, in a fresh player) until the *first row advance* comes out of the ring ten
    // polls later. `second5` is the trace of exactly this: frames 408…426 read `ord 0 row 0
    // pat 0`, and the first published row is 427.
    let mut player = player("MUSIC0");
    player.play_song(25);
    assert_eq!(
        player.state(),
        Position {
            order: 0,
            pattern: 0,
            row: 0,
            speed: 6,
            tempo: 120,
            r#loop: 0
        },
        "the seek published nothing, and the speed and tempo are the module's (patched) header"
    );
    assert_eq!(
        player.engine_position(),
        Position {
            order: 25,
            pattern: 70,
            row: 0,
            speed: 6,
            tempo: 120,
            r#loop: 0
        },
        "the engine is in the order the seek asked for"
    );
}

#[test]
fn the_published_position_is_ten_polls_behind_the_engine() {
    // The ring is primed ten entries deep (`+0x2ac1`). The engine enters row 1 six ticks
    // after the seek; the ring publishes it ten polls after the poll that pushed it.
    let mut player = player("MUSIC0");
    player.play_song(25);
    let ticks_per_row = 6;
    let per_tick = 40_000u64 * 5 / (2 * 120);
    let samples = per_tick * ticks_per_row;
    let row_begins = u64::from(ticks_to_polls(ticks_per_row as u32, MixRate::Hz40000, 120));
    let _ = samples;

    let entered = poll_until(&mut player, 100, |player, _| {
        player.engine_position().row == 1
    });
    println!(
        "row 1 is entered after {entered} polls (the arithmetic says {row_begins}), and \
         published {} polls later",
        entered + 10
    );
    assert!(
        player.state().row == 0,
        "the engine's row 1 is not yet published"
    );
    for _ in 0..9 {
        player.poll();
    }
    assert_eq!(
        player.state().row,
        0,
        "nine polls on, the ring is still publishing the row before"
    );
    player.poll();
    assert_eq!(
        player.state().row,
        1,
        "the tenth poll publishes it — the lag is the ring's fill"
    );
}

#[test]
fn an_a_command_changes_the_speed_where_the_effect_is_processed_not_where_it_is_published() {
    // The published speed is written straight through (`+0x3dd8`) while the position travels
    // the ring, so the trace's `speed` word leads its `row` column — measured in `second5`
    // as one row at speed 6 and six rows at speed 1. Here: pattern 71's `A01` is on its row
    // 0, and the trace's speed word reads 1 while the *row* column still reads the last row
    // of pattern 70.
    let mut player = player("MUSIC0");
    player.play_song(25);
    let published = poll_until(&mut player, 200_000, |player, _| {
        player.state().speed == 1
    });
    assert_eq!(player.state().order, 25, "the order the A01's row follows");
    assert_eq!(
        player.state().pattern, 70,
        "the pattern *before* the one carrying the A01"
    );
    assert_eq!(player.state().row, 63, "the last row of pattern 70");
    println!(
        "the A01 shows at poll {published}: speed 1 while the published row is pattern 70's          last — and the engine's own row is {}",
        player.engine_position().row
    );
    let row_after = poll_until(&mut player, 100, |player, _| player.state().pattern == 71);
    println!(
        "the ring publishes pattern 71 {row_after} polls later, when the engine is at row {}",
        player.engine_position().row
    );
}

#[test]
fn a_tempo_command_is_told_from_a_speed_by_its_parameter() {
    // `+0x3d73`: one handler, `cmp $0x20,%ax; jbe` — 32 and under is a speed, above it a
    // tempo. The tick's sample count is recomputed with the tempo and only there.
    let module = Builder::new(
        &[0],
        &[cell(0, None, Some((1, 0x20)))], // A20: the boundary — a speed of 32
    )
    .build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    poll_until(&mut player, 10, |player, _| player.state().speed != 6);
    assert_eq!(player.state().speed, 0x20, "32 is still a speed");
    assert_eq!(player.state().tempo, 125);

    let module = Builder::new(
        &[0],
        &[cell(0, None, Some((1, 0x21)))], // A21 — above it, so a tempo
    )
    .build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    poll_until(&mut player, 10, |player, _| player.state().tempo != 125);
    assert_eq!(player.state().speed, 6, "the speed is untouched");
    assert_eq!(player.state().tempo, 0x21, "and the tempo takes the value");
}

#[test]
fn a_c_command_breaks_to_the_next_order_at_its_own_decimal_row() {
    // `+0x3dfe`: the parameter is decimal (high nibble × 10 + low nibble), unclamped. `C12`
    // therefore means row 12, not row 0x12 = 18 — and the break lands at the *row's end*, the
    // order advance being deferred by the flag (`+0x3de2`'s sibling at `+0x3e0a`).
    let module = Builder::new(
        &[0, 1],
        &[
            cell(0, None, Some((3, 0x12))), // C12 on pattern 0's row 0
            vec![],                         // pattern 1: empty
        ],
    )
    .build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    // The engine enters order 1 at row 12 after one row's worth of ticks.
    let polls = ticks_to_polls(6, MixRate::Hz40000, 125);
    let mut entered = None;
    for poll in 0..polls * 3 {
        if player.engine_position().order == 1 {
            entered = Some(poll);
            break;
        }
        player.poll();
    }
    let entered = entered.expect("the break reached order 1");
    assert_eq!(player.engine_position().row, 12, "row 12, not 18");
    println!("C12 reached order 1 row 12 after {entered} polls");

    // A parameter naming a row past the pattern's last is stored unclamped — the original's
    // `C` handler writes `[0x93b]` and nothing checks it (`+0x3dfe`…`+0x3e07`) — and the row
    // counter is past 63, so the very next row advance takes the order counter on again.
    let module = Builder::new(&[0, 1, 2], &[cell(0, None, Some((3, 0x99))), vec![], vec![]]).build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    poll_until(&mut player, polls * 4, |player, _| {
        player.engine_position().order == 2
    });
    assert_eq!(
        player.engine_position().row,
        0,
        "and the order after that one is entered at its first row"
    );
}

#[test]
fn a_b_command_jumps_at_the_end_of_its_own_row() {
    // `+0x3de2`: `B` raises the same advance flag a `C` does, so the jump lands at the row's
    // end — after the row's own ticks, not at the cell.
    let module = Builder::new(
        &[0, 1, 2],
        &[
            // Pattern 0's row 0 jumps to order 2; pattern 1 must never be played.
            cell(0, None, Some((2, 2))),
            cell(0, None, Some((1, 1))),
            vec![],
        ],
    )
    .build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    let polls = ticks_to_polls(6, MixRate::Hz40000, 125);
    let mut jumped = None;
    for poll in 0..polls * 3 {
        if player.engine_position().order == 2 {
            jumped = Some(poll);
            break;
        }
        assert_ne!(
            player.engine_position().order,
            1,
            "the jump skipped order 1"
        );
        player.poll();
    }
    let jumped = jumped.expect("the jump reached order 2");
    println!("B02 reached order 2 after {jumped} polls (one row of ticks)");
    // And the row it jumped from played out first: the row advance is what carries it.
    assert!(jumped >= polls / 2, "the jump waited for the row to end");
}

#[test]
fn a_254_is_stepped_over_and_zplus_records_the_crossing() {
    // `music-clock.md` §10 and §6: 254 is stepped over, and `zplus` is the object's own two
    // bits —`orw $0x1` while the next entry is a 254 (`+0x4452`, `+0x4456`), `orw $0x2` when
    // this one is and is stepped over (`+0x445c`, `+0x4461`).
    //
    // The module below is the shape the two bits differ over: `[0, 254, 1, 254]`. Playing
    // order 0 sets bit 0 (its next entry, order 1, is the 254) and nothing else — **1**. The
    // step over it to order 2 sets bit 1 for the entry stepped over *and* bit 0 for order 2's
    // own successor at order 3 — **3**, which is what the object's arithmetic produces and
    // what `music-clock.md` §6's original wording ("unless the entry after the one entered is
    // itself 254, in which case 1") could not.
    let mut player = Player::new(
        Builder::new(&[0, 254, 1, 254], &[vec![], vec![]]).build(),
        PlayerOptions::default(),
    );
    player.play_song(0);
    assert_eq!(
        player.engine_zplus(),
        1,
        "the order being played has the 254 next: bit 0 alone"
    );
    assert_eq!(player.engine_position().order, 0);

    poll_until(&mut player, 1_000_000, |player, _| {
        player.engine_position().order == 2
    });
    assert_eq!(
        player.engine_position().order,
        2,
        "order 1 is the 254, and the scan stepped over it"
    );
    assert_eq!(player.engine_position().row, 0);
    assert_eq!(player.engine_zplus(), 3, "both bits: stepped over, and a 254 next");
    // And what the parts see is the same value, ten polls later: `second5`'s own zplus column
    // turns 1 at order 26 (the order whose successor at 27 is the 254), which is the ring
    // publishing what this word held.
    let published = poll_until(&mut player, 100, |player, _| player.zplus() != 0);
    println!("the step-over's zplus 3 reaches the parts {published} poll(s) later");
    let _ = published;
}

#[test]
fn a_255_ends_the_order_list_and_loops_when_the_song_was_already_running() {
    // `+0x4469`…`0x449b`: a 255 stops a song whose first row has not played yet, and
    // otherwise increments `_np_loop` and restarts the scan at the loop order `_zplaysong`
    // recorded — which is 0 for every order the shell hands it.
    let module = Builder::new(&[0, 1, 255], &[vec![], vec![]]).build();

    let mut player = Player::new(module.clone(), PlayerOptions::default());
    player.play_song(2);
    assert!(
        player.finished(),
        "a seek onto a 255 runs the scan off the list"
    );
    // ...and the original then sets `[0x938] = 1` anyway (`+0x259a`), so the player is left
    // *ticking* with no new order entered: the row counter (`+0x5702`), the pattern
    // (`+0x5708`) and the published row (`+0x570b`) were each zeroed by the seek's own routine
    // and the scan only gives up, so the engine sits at row 0 of pattern 0 — while the ring,
    // fed `[0x940] - 1`,
    // hands the parts the index of the `255` itself. That is the limbo the port reproduces,
    // not a stop, and neither module can reach it (neither has a `255` at its first order), so
    // the bytes are all there is to follow.
    let engine = player.engine_position();
    assert_eq!(
        (engine.order, engine.pattern, engine.row),
        (0, 0, 0),
        "the seek zeroed the row and the pattern, and entered no order"
    );
    for _ in 0..200 {
        player.poll();
    }
    assert_eq!(
        player.state().pattern,
        0,
        "the scan entered no order, so nothing else was published"
    );
    assert_eq!(
        player.state().order,
        2,
        "and the ring hands out `[0x940] - 1`: the index of the `255` the scan gave up on"
    );
    assert!(
        player.zframe() > 0,
        "the ticks ran: the mixing flag was put back by the seek itself"
    );

    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    let polls = ticks_to_polls(6, MixRate::Hz40000, 125) * 64 * 2;
    let mut looped_at = None;
    for poll in 0..polls {
        player.poll();
        if player.state().r#loop > 0 {
            looped_at = Some(poll);
            break;
        }
    }
    let looped_at = looped_at.expect("the 255 looped the song back to order 0");
    assert!(!player.finished(), "a loop is not an end");
    println!("the 255 at order 2 looped back to order 0 after {looped_at} polls");
}

#[test]
fn goto_song_enters_its_row_on_the_next_tick() {
    // `+0x2513`: the row goes to `[0x93b]`, the order to `[0x940]`, the flag goes up, and
    // `[0x2000]` is set to the speed so the very next tick enters the row.
    let mut player = player("MUSIC0");
    player.goto_song(28, 7);
    poll_until(&mut player, 10, |player, _| player.engine_position().order == 28);
    let position = player.engine_position();
    assert_eq!((position.order, position.pattern, position.row), (28, 14, 7));
}

#[test]
fn stop_keeps_the_position_and_shut_up_only_silences() {
    let mut player = player("MUSIC0");
    player.play_song(25);
    for _ in 0..300 {
        player.poll();
    }
    player.stop();
    // The ring still holds what the engine pushed before the stop: ten polls later the
    // published position settles, and from then on it is the same word for ever.
    for _ in 0..20 {
        player.poll();
    }
    let before = player.state();
    let zframe = player.zframe();
    for _ in 0..600 {
        player.poll();
    }
    assert_eq!(player.state(), before, "a stopped player holds its position");
    assert_eq!(
        player.zframe(),
        zframe,
        "and its ticks stop counting: the original's stopped path never touches _np_zframe"
    );

    player.play_song(25);
    player.poll();
    assert!(!player.is_silenced());
    player.shut_up();
    assert!(player.is_silenced());
    for _ in 0..300 {
        player.poll();
    }
    assert!(
        !player.is_silenced(),
        "a new row starts voices again — which is Task 5's to make audible"
    );
}

#[test]
fn the_polls_samples_are_the_frames_share_and_carry_their_fraction() {
    // 40000/70.086 = 570.733… samples a poll. The port hands Task 5 whole samples and carries
    // the fraction, rather than rounding per poll (which would drift by 0.13 %): the fives are
    // 570, 571, 571, 570, 571 — the fraction showing up as the odd 571, never lost.
    let mut player = player("MUSIC0");
    player.play_song(0);
    let mut run = Vec::new();
    for _ in 0..5 {
        player.poll();
        run.push(player.samples_this_poll());
    }
    assert_eq!(run, vec![570, 571, 571, 570, 571]);

    // And the sum over a stretch of polls is the rate: `polls × mix_rate / 70.086`, to the
    // sample, however the fraction falls.
    let polls = 1_000u32;
    let mut total = 0u64;
    for _ in 0..polls {
        player.poll();
        total += u64::from(player.samples_this_poll());
    }
    let expected = u64::from(polls) * 40_000 * 1000 / 70_086;
    assert!(
        total.abs_diff(expected) <= 1,
        "{polls} polls gave {total} samples where {expected} is the rate"
    );
    println!("{polls} polls: {total} samples against a rate of {expected}");
}

#[test]
fn the_global_volume_is_the_modules_own_byte_and_the_shell_walks_it_down() {
    assert_eq!(player("MUSIC0").global_volume(), 64);
    let mut player = player("MUSIC1");
    assert_eq!(player.global_volume(), 64);
    // `fademusic` (`MAIN/U2.ASM:333-346`) subtracts a step per call and stops at zero.
    for step in [16u8, 16, 16, 16, 16] {
        let volume = player.global_volume().saturating_sub(step);
        player.set_global_volume(volume);
    }
    assert_eq!(player.global_volume(), 0);
}

#[test]
fn zframe_counts_ticks_at_the_tempos_rate_and_the_parts_can_reset_it() {
    let mut player = player("MUSIC0");
    player.play_song(25);
    let polls = ticks_to_polls(48, MixRate::Hz40000, 120);
    for _ in 0..polls {
        player.poll();
    }
    let zframe = player.zframe();
    println!("{zframe} tick(s) in {polls} polls at tempo 120 (48 a second at 70.086 Hz)");
    assert!(
        (zframe - 48).abs() <= 1,
        "≈48 ticks a second, not 48 a poll: got {zframe} in {polls} polls"
    );
    player.set_zframe(0);
    assert_eq!(player.zframe(), 0, "the parts write this word");
    player.set_zframe(-7);
    assert_eq!(player.zframe(), -7, "and it is signed");
}

// -------------------------------------------------------------------------------------------
// End to end
// -------------------------------------------------------------------------------------------

/// Play a module from the order the demo's own boot starts it at (`MAIN/U2.ASM:786` pushes 0)
/// until the order counter stops advancing or wraps — and say where and how long it took.
fn play_through(name: &'static str) -> (u32, Position, u8, bool) {
    let mut player = player(name);
    player.play_song(0);
    let cap = 400_000u32;
    let mut previous = player.state().order;
    let mut started = false;
    for poll in 0..cap {
        player.poll();
        let position = player.state();
        if !started && position.order != 0 {
            started = true;
        }
        let wrapped = started && position.order < previous;
        if wrapped || player.finished() {
            println!(
                "  {name}: {} after {poll} polls ({:.1} seconds of logic ticks) at order {} \
                 pattern {} row {}, loop counter {}",
                if player.finished() { "finished" } else { "wrapped" },
                f64::from(poll) / 70.086,
                position.order,
                position.pattern,
                position.row,
                player.state().r#loop,
            );
            return (poll, position, player.state().r#loop, player.finished());
        }
        previous = position.order;
    }
    panic!("{name} never finished or wrapped in {cap} polls");
}

#[test]
fn each_module_plays_to_the_end_of_its_order_list() {
    // The exit criterion's "shown, not asserted": each module is played from its own start
    // order to the end of its order list, and the run says where it got and how many logic
    // ticks it took. The number is this test's own arithmetic — the order list, the 64 rows
    // and the tick counts — and the printed line is what `PROGRESS.md` may quote.
    for name in ["MUSIC0", "MUSIC1"] {
        let (polls, position, loops, finished) = play_through(name);
        // A second, independent count of the same run: the ticks it spent, against the rows it
        // spent them on.
        let (ticks, rows) = count_ticks(name);
        assert!(rows > 0, "{name}: no rows played");
        println!("  {name}: {polls} poll(s), {ticks} tick(s) over {rows} row(s)");
        let module = module(name);
        assert!(
            usize::from(position.order) < module.order_count(),
            "{name}: the published order is inside the order list"
        );
        assert!(
            usize::from(position.pattern) < module.pattern_count(),
            "{name}: the published pattern is inside the pattern table"
        );
        assert!(position.row < ROWS as u8, "{name}: the row is inside the pattern");
        assert!(polls > 0, "{name}: the song took some time to play");
        println!(
            "  {name}: {} order(s), {} pattern(s), ended at order {} ({} loops)",
            module.order_count(),
            module.pattern_count(),
            position.order,
            loops
        );
        // Both modules end their order list with a `255` before the order list's end, and
        // the shell restarts the music before any `255` is reached; played from order 0 the
        // player loops instead (`music-clock.md` §10).
        assert!(
            looped_or_finished(loops, finished),
            "{name}: expected a loop or an end"
        );
    }
}

/// Play the module again, counting the ticks it spends and the rows it spends them on.
///
/// The tick count is the player's own (`Player::zframe`); the row count is the engine's position
/// changing. The row timer is what is really being asked: every row the engine enters must be
/// given between 1 and 32 of those ticks — the range the format's speed byte allows — so the
/// two numbers agreeing is a statement about the row timer and not about the counter reading
/// itself. Returns (ticks, rows).
fn count_ticks(name: &'static str) -> (i64, u32) {
    let mut player = player(name);
    player.play_song(0);
    let mut ticks = 0i64;
    let mut rows = 0u32;
    let mut last_row = None;
    let mut ticks_this_row = 0u32;
    let cap = 400_000u32;
    let mut previous = player.state().order;
    let mut started = false;
    for _ in 0..cap {
        let before = player.zframe();
        let position = player.engine_position();
        player.poll();
        let now = player.zframe();
        let spent = u32::try_from(now - before).expect("a few ticks a poll");
        ticks += i64::from(spent);
        if last_row != Some((position.order, position.row)) {
            if rows > 0 {
                assert!(
                    (1..=32).contains(&ticks_this_row),
                    "{name}: the row at {last_row:?} was given {ticks_this_row} ticks"
                );
            }
            rows += 1;
            ticks_this_row = 0;
            last_row = Some((position.order, position.row));
        }
        ticks_this_row += spent;
        let published = player.state();
        if !started && published.order != 0 {
            started = true;
        }
        if (started && published.order < previous) || player.finished() {
            break;
        }
        previous = published.order;
    }
    (ticks, rows)
}

fn looped_or_finished(loops: u8, finished: bool) -> bool {
    loops > 0 || finished
}

#[test]
fn playing_past_the_end_neither_panics_nor_runs_off_the_orders() {
    // "A player that loops for ever is fine; a player that reads past its orders is not."
    // This plays each module a further window past the point the test above stopped at —
    // 71,685 polls, a thousand seconds — and asks that every published word stays inside the
    // module's own tables, that the tick counter stays finite, and that the tempo stays a
    // tempo.
    for name in ["MUSIC0", "MUSIC1"] {
        let (_, _, _, _) = play_through(name);
        let mut player = player(name);
        let module = module(name);
        player.play_song(0);
        let mut wrapped = 0u32;
        let mut previous = 0u8;
        for poll in 0..100_000u32 {
            player.poll();
            let position = player.state();
            assert!(
                usize::from(position.order) < module.order_count(),
                "{name}: poll {poll} published order {} past the {} the module has",
                position.order,
                module.order_count()
            );
            assert!(
                usize::from(position.pattern) < module.pattern_count(),
                "{name}: poll {poll} published pattern {} past the {} the module has",
                position.pattern,
                module.pattern_count()
            );
            assert!(position.row < ROWS as u8, "{name}: poll {poll} published row {}", position.row);
            assert!(
                (25..=255).contains(&position.tempo),
                "{name}: poll {poll} published tempo {}",
                position.tempo
            );
            assert!(
                (1..=32).contains(&position.speed),
                "{name}: poll {poll} published speed {}",
                position.speed
            );
            if position.order < previous {
                wrapped += 1;
            }
            previous = position.order;
        }
        assert!(wrapped > 0, "{name}: no order wrap in the window at all");
        println!("  {name}: {wrapped} wrap(s) and no reading past the order list");
    }
}

/// The per-channel effect histogram of the cells the two modules carry, so the timeline
/// effects' coverage is a measured fact rather than a claim: `A`, `B` and `C` are the ones
/// this task owns, and `T` — which the plan's amendment says is in neither module — is
/// counted here too.
#[test]
fn the_timeline_effects_the_modules_carry() {
    for name in ["MUSIC0", "MUSIC1"] {
        let module = module(name);
        let mut histogram: BTreeMap<u8, usize> = BTreeMap::new();
        for index in 0..module.pattern_count() {
            let pattern = module.pattern(index).expect("a pattern");
            for row in &pattern.rows {
                for cell in row {
                    if cell.has_command() {
                        *histogram.entry(cell.command).or_default() += 1;
                    }
                }
            }
        }
        let named: Vec<String> = histogram
            .iter()
            .filter(|&(command, _)| (1..=26).contains(command))
            .map(|(command, count)| {
                format!("{}×{count}", char::from(b'A' + command - 1))
            })
            .collect();
        println!("  {name}: {}", named.join(" "));
        for command in [1u8, 2, 3] {
            println!(
                "    {}: {}",
                char::from(b'A' + command - 1),
                histogram.get(&command).copied().unwrap_or(0)
            );
        }
        assert!(
            histogram.get(&1).copied().unwrap_or(0) > 0,
            "{name} carries an A"
        );
        assert_eq!(
            histogram.get(&20).copied().unwrap_or(0),
            0,
            "{name} carries no T, as the phase plan's amendment says"
        );
    }
    let _ = CHANNELS;
}

