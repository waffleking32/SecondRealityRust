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
        // row and the pending row, all zeroed before the order advance runs. **This run is the
        // zeroing half of the note's unreachable `255` paths and not the whole of them**: it is
        // why a seek onto a `255` leaves the engine at row 0 of pattern 0 with the published
        // row cleared, while the other half — that the scan *gives up* rather than stopping,
        // `+0x4499`'s `je` landing past `+0x4476`'s stop — is read at that offset and not here.
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
        // **`A` and `T` are two handlers, not one told apart by the parameter** — the
        // reading Task 4 corrected. `T` is the first-tick table's entry 20 and its body is
        // this entry: `cmp $0x20,%ax; jbe 0x3dce` sends 32 and under to the *driver* call,
        // never to the speed, so a `T` with a small parameter changes nothing.
        (
            0x3D73,
            &[0x32, 0xE4, 0x3D, 0x20, 0x00, 0x76],
            "T: xor %ah,%ah; cmp $0x20,%ax; jbe (+0x3dce, the driver call) — the split that is this handler's alone",
        ),
        // `A` is the first-tick table's entry 1 and its whole body is this: **no upper
        // bound**, so `A7F` sets a speed of 127 (MUSIC1 has such a cell). The correction in
        // `player.rs` and the note's §9.13 rest on these eleven bytes.
        (
            0x3DD2,
            &[0x3C, 0x00, 0x74, 0xF8, 0x32, 0xE4, 0x2E, 0xA3, 0x02, 0x20, 0xC3],
            "A: cmp $0x0,%al; je (+0x3dce); xor %ah,%ah; mov %ax,%cs:0x2002 (_np_speed); ret",
        ),
        // ...and the object's own *header watch* is the second witness: it reads the module
        // header's tempo byte (offset 50, `0x32` — the two `mov %al,ds:[..]` below are the
        // read) and calls `+0x3d73` with it, then reads the speed byte (offset 49, `0x31`)
        // and calls `+0x3dd2`. Four sites, and each of the four is its own claim.
        (
            0x53B9,
            &[0xA0, 0x32, 0x00],
            "the header watch reads the header's *tempo* byte (offset 50) into %al",
        ),
        (
            0x53C7,
            &[0xE8, 0xA9, 0xE9],
            "and calls +0x3d73 with it — the tempo handler, the only one that gets this byte",
        ),
        (
            0x53CA,
            &[0xA0, 0x31, 0x00],
            "then reads the header's *speed* byte (offset 49) into %al",
        ),
        (
            0x53D8,
            &[0xE8, 0xF7, 0xE9],
            "and calls +0x3dd2 with it — the speed handler, which takes every value it is given",
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

        // ----------------------------------------------------------------------------------
        // Task 4's: the channel effects, and the two tick passes that reach them.
        //
        // These are the bytes `crates/sr-audio/src/effects.rs` and the second half of
        // `player.rs` were read out of — the handlers, the two dispatch tables, the S
        // subcommand tables, the four waveform entries, and the tables every one of them
        // is measured in (the note table, the shifts, the sine, the two `Q` tables).
        // ----------------------------------------------------------------------------------
        (
            0x3BD8,
            &[0x72, 0x3D, 0xD2, 0x3D, 0xDD, 0x3D, 0xFE, 0x3D, 0x08, 0x3F, 0x4F, 0x3E, 0x10, 0x3E, 0x72, 0x3D, 0x72, 0x3D, 0x48, 0x41, 0x90, 0x41, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0xEC, 0x41, 0x72, 0x3D, 0x4E, 0x42, 0x73, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x4A, 0x42, 0x72, 0x3D],
            "the first-tick dispatch table (`+0x3bd8`), entry by entry: 0 = the bare ret at 0x3d72, 1 = 0x3dd2 (A), 2 = 0x3ddd (B), 3 = 0x3dfe (C), 4 = 0x3f08 (D), 5 = 0x3e4f (E), 6 = 0x3e10 (F), 7 and 8 = the bare ret, 9 = 0x4148 (I), 10 = 0x4190 (J), 11..16 = the bare ret, 17 = 0x41ec (Q), 18 = the bare ret, 19 = 0x424e (S), 20 = 0x3d73 (T), 21..25 = the bare ret, 26 = 0x424a (Z), 27 = the bare ret",
        ),
        (
            0x3C0E,
            &[0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x08, 0x3F, 0x4F, 0x3E, 0x10, 0x3E, 0x94, 0x3F, 0x05, 0x40, 0x48, 0x41, 0x90, 0x41, 0x00, 0x3F, 0x05, 0x3F, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0xEC, 0x41, 0xD0, 0x40, 0x61, 0x42, 0x72, 0x3D, 0x72, 0x3D, 0xC8, 0x42, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D],
            "the later-tick dispatch table (`+0x3c0e`), entry by entry: 1, 2 and 3 are the bare ret where the first table has A, B and C; 4 = 0x3f08 (D), 5 = 0x3e4f (E), 6 = 0x3e10 (F), 7 = 0x3f94 (G), 8 = 0x4005 (H), 9 = 0x4148 (I), 10 = 0x4190 (J), 11 = 0x3f00 (K), 12 = 0x3f05 (L), 17 = 0x41ec (Q), 18 = 0x40d0 (R), 19 = 0x4261 (S), 22 = 0x42c8 (V) — I, R and V have handlers here, so the letters this task records as absent are absent from the *data*, not from the object",
        ),
        (
            0x3B98,
            &[0xA9, 0x42, 0x4E, 0x43, 0xD0, 0x42, 0xF1, 0x42, 0x03, 0x43, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x15, 0x43, 0x1E, 0x43, 0x7F, 0x42, 0x96, 0x42, 0x74, 0x42, 0x72, 0x3D],
            "the S subcommand table on the row's first tick: 12 = 0x427f, 13 = 0x4296 (SDx), 14 = 0x4274, 3 and 4 the waveform setters",
        ),
        (
            0x3BB8,
            &[0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x72, 0x3D, 0x83, 0x42, 0x9A, 0x42, 0x72, 0x3D, 0x72, 0x3D],
            "the S subcommand table on the later ticks: only 12 = 0x4283 and 13 = 0x429a have handlers",
        ),
        (
            0x4068,
            &[0x7B, 0x40, 0x8E, 0x40, 0xA1, 0x40, 0xB4, 0x40, 0x78, 0x40, 0x8B, 0x40, 0x9E, 0x40, 0xB1, 0x40],
            "the vibrato waveform table: 0 = 0x407b (the sine), 2 = 0x408e, 4 = 0x40a1, 6 = 0x40b4, then the four no-restart variants",
        ),
        (
            0x0A24,
            &[0x00, 0x6B, 0x00, 0x65, 0x40, 0x5F, 0x00, 0x5A, 0xC0, 0x54, 0x00, 0x50, 0x80, 0x4B, 0x40, 0x47, 0x40, 0x43, 0x80, 0x3F, 0x00, 0x3C, 0xB0, 0x38],
            "the note table at `+0xa24`, four octaves up: C is 27392, so octave 4's C is the format's 1712",
        ),
        (
            0x097C,
            &[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x00, 0x05, 0x0B, 0x11, 0x1B, 0x20, 0x2A, 0x2F],
            "the octave shifts at `+0x97c`: eight real entries and the eight bytes after them, which octaves 8…15 read",
        ),
        (
            0x0A50,
            &[0x00, 0x00, 0x18, 0x00, 0x31, 0x00, 0x4A, 0x00, 0x61, 0x00, 0x78, 0x00, 0x8D, 0x00, 0xA1, 0x00],
            "the sine's first eight words at `+0xa50`: 0, 24, 49, 74, 97, 120, 141, 161",
        ),
        (
            0x095C,
            &[0x00, 0xFF, 0xFE, 0xFC, 0xF8, 0xF0, 0x00, 0x00, 0x00, 0x01, 0x02, 0x04, 0x08, 0x10, 0x01, 0x01],
            "the Q volume addends at `+0x95c`",
        ),
        (
            0x096C,
            &[0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x0A, 0x08, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x18, 0x20],
            "the Q volume multipliers at `+0x96c` — `0x0101` filler but for 6, 7, 14 and 15",
        ),
        (
            0x1FA0,
            &[0xA0, 0x0D, 0x00, 0x0E, 0x60, 0x0E, 0xC0, 0x0E],
            "the S3M channel to `_zchn` map is the identity",
        ),
        (
            0x5448,
            &[0xA1, 0x26, 0x00, 0xA9, 0x01, 0x00, 0x74, 0x06, 0x2E, 0xC6],
            "the amiga byte: mov 0x26,%ax; test $0x1,%ax; je; movb $0x1,%cs:0x915",
        ),
        (
            0x5401,
            &[0x2E, 0xA3, 0x17, 0x09, 0xB9],
            "masterflags: mov %ax,%cs:0x917 — the module's own flags word",
        ),
        (
            0x3F08,
            &[0x2E, 0xC6, 0x44, 0x3C, 0x01],
            "D: movb $0x1,%cs:0x3c(%si) — the slide begins (and `K`/`L` enter here)",
        ),
        (
            0x3F22,
            &[0x80, 0xFA, 0x0F, 0x74, 0x4C, 0x80],
            "D: cmp $0xf,%dl; je (+0x3f73) — a low nibble of f is the fine marker",
        ),
        (
            0x3F27,
            &[0x80, 0xFE, 0x0F, 0x74, 0x55, 0x80],
            "D: cmp $0xf,%dh; je (+0x3f81) — a high nibble of f is the other one",
        ),
        (
            0x3F31,
            &[0x32, 0xF6, 0x28, 0x54, 0x1A],
            "D: xor %dh,%dh; sub %dl,0x1a(%si) — slide down by the low nibble",
        ),
        (
            0x3F51,
            &[0x8A, 0xC6, 0x32, 0xE4, 0x00, 0x44, 0x1A],
            "D: add %al,0x1a(%si) — slide up by the high nibble",
        ),
        (
            0x3F5E,
            &[0xC6, 0x44, 0x1A, 0x3F],
            "D: movb $0x3f,0x1a(%si) — the volume clamps at 63, not the file's 64",
        ),
        (
            0x3F73,
            &[0x80, 0xFE, 0x00, 0x74, 0xB9, 0x83, 0x3E, 0x00, 0x20, 0x00, 0x75],
            "D: the fine-up path — a zero high nibble falls back to the ordinary slide",
        ),
        (
            0x3F81,
            &[0x80, 0xFA, 0x00, 0x74, 0xCB, 0x83],
            "D: and the fine-down path's own test: cmp $0x0,%dl; je (+0x3f51)",
        ),
        (
            0x3E4F,
            &[0x83, 0x7C, 0x24, 0x00, 0x74, 0xE5],
            "E: cmpw $0x0,0x24(%si); je (ret) — nothing without a note's period",
        ),
        (
            0x3E59,
            &[0x8A, 0x44, 0x39, 0x88, 0x44, 0x36, 0x32],
            "E: mov 0x39(%si),%al; mov %al,0x36(%si) — a zero info reuses alastnfo",
        ),
        (
            0x3E68,
            &[0x3C, 0xE0, 0x73, 0xCE, 0xD1, 0xE0],
            "E: cmp $0xe0,%al; jae (ret) — the ordinary slide stops at 0xe0",
        ),
        (
            0x3E6C,
            &[0xD1, 0xE0, 0xD1, 0xE0, 0x01, 0x44, 0x12, 0x01, 0x44, 0x24],
            "E: shl $1,%ax twice; add %ax,0x12(%si); add %ax,0x24(%si) — four times the parameter, a tick, on both periods",
        ),
        (
            0x3E10,
            &[0x83, 0x7C, 0x24, 0x00, 0x74, 0x24],
            "F: the same handler with the other sign, cmpw $0x0,0x24(%si)",
        ),
        (
            0x3E2D,
            &[0xD1, 0xE0, 0xD1, 0xE0, 0x29, 0x44, 0x12, 0x29, 0x44, 0x24],
            "F: shl $1,%ax twice; sub %ax,0x12(%si); sub %ax,0x24(%si)",
        ),
        (
            0x3E3B,
            &[0x3C, 0xE0, 0x76, 0xFB, 0x3C, 0xF0],
            "E/F's first-tick split: cmp $0xe0,%al; jbe (ret)",
        ),
        (
            0x3E43,
            &[0x24, 0x0F, 0xD1, 0xE0, 0xD1, 0xE0, 0xEB, 0xE6],
            "E/F: above 0xf0 is the extra-fine form — the low nibble times four",
        ),
        (
            0x3F94,
            &[0x83, 0x7C, 0x24, 0x00, 0x74, 0x17],
            "G: cmpw $0x0,0x24(%si); je (ret)",
        ),
        (
            0x3FA1,
            &[0x88, 0x44, 0x2C, 0x88, 0x44, 0x36],
            "G: mov %al,0x2c(%si); mov %al,0x36(%si) — alasteff2 and the cell's info",
        ),
        (
            0x3FA9,
            &[0x8B, 0x44, 0x12, 0x3B, 0x44, 0x20],
            "G: mov 0x12(%si),%ax; cmp 0x20(%si),%ax — the live period against asldspd",
        ),
        (
            0x3FB6,
            &[0xD1, 0xE2, 0xD1, 0xE2, 0x03, 0xC2, 0x3B, 0x44, 0x20],
            "G: shl $1,%dx twice; add %dx,%ax — four times the parameter, a tick",
        ),
        (
            0x3FC4,
            &[0x89, 0x44, 0x24],
            "G: mov %ax,0x24(%si) — the glide's base moves with it",
        ),
        (
            0x3FD2,
            &[0x89, 0x44, 0x12],
            "G: mov %ax,0x12(%si) — and then the live period",
        ),
        (
            0x4005,
            &[0x3C, 0x00, 0x75, 0x03, 0x8A, 0x44, 0x2B, 0xA8],
            "H: cmp $0x0,%al; jne; mov 0x2b(%si),%al — the parameter is alasteff1",
        ),
        (
            0x4010,
            &[0x8A, 0x64, 0x2B, 0x24, 0x0F, 0x80, 0xE4, 0xF0, 0x0A],
            "H: the merge — a zero high nibble keeps the previous parameter's",
        ),
        (
            0x402D,
            &[0x8B, 0x44, 0x26, 0x8B, 0x5C, 0x3D],
            "H: mov 0x26(%si),%ax; mov 0x3d(%si),%bx — the position and vibtretype",
        ),
        (
            0x4036,
            &[0x2E, 0xFF, 0x97, 0x68, 0x40],
            "H: call *%cs:0x4068(%bx) — the waveform table",
        ),
        (
            0x404D,
            &[0xD3, 0xF8, 0x03, 0x44, 0x24],
            "H: sar %cl,%ax; add 0x24(%si),%ax — the level, and the note's own period",
        ),
        (
            0x4060,
            &[0x80, 0xE3, 0x7E, 0x89, 0x5C, 0x26],
            "H: and $0x7e,%bl; mov %bx,0x26(%si) — the phase, masked to the even low seven bits",
        ),
        (
            0x407B,
            &[0xA9, 0x80, 0x00, 0x74, 0x03, 0xB8, 0x00, 0x00, 0x8B, 0xD8, 0x2E, 0x8B, 0x87, 0x50, 0x0A, 0xC3],
            "the sine waveform: test $0x80,%ax; mov %ax,%bx; mov %cs:0xa50(%bx),%ax; ret",
        ),
        (
            0x4190,
            &[0x3C, 0x00, 0x75, 0x06, 0x8A, 0x44],
            "J: cmp $0x0,%al; jne; mov 0x39(%si),%al — alastnfo",
        ),
        (
            0x419A,
            &[0xA1, 0x00, 0x20, 0xB1, 0x03, 0xF6, 0xF1, 0x32],
            "J: mov 0x2000,%ax; mov $0x3,%cl; div %cl — the engine's tick, modulo three",
        ),
        (
            0x41C8,
            &[0x80, 0xFF, 0x0C, 0x72, 0x06, 0x80, 0xEF],
            "J: cmp $0xc,%bh; jb — the semitone carries at B",
        ),
        (
            0x3F00,
            &[0xBD, 0x02, 0x00, 0xEB, 0x03, 0xBD],
            "K and L share D's body: mov $0x2,%bp; jmp 0x3f08",
        ),
        (
            0x3F05,
            &[0xBD, 0x01, 0x00, 0x2E, 0xC6, 0x44],
            "L: mov $0x1,%bp; jmp 0x3f08",
        ),
        (
            0x4000,
            &[0x8A, 0x44, 0x2B, 0xEB],
            "K's vibrato half: mov 0x2b(%si),%al — H's byte, not the slide's",
        ),
        (
            0x41EC,
            &[0x3C, 0x00, 0x75, 0x06, 0x8A, 0x44],
            "Q: cmp $0x0,%al; jne; mov 0x39(%si),%al",
        ),
        (
            0x41F6,
            &[0x8A, 0xD8, 0xB1, 0x04, 0xD2, 0xEB, 0x83, 0xE3, 0x0F, 0x24],
            "Q: the volume code is the high nibble and the interval the low one",
        ),
        (
            0x4205,
            &[0x3A, 0x44, 0x2E, 0x77, 0x3C, 0x33],
            "Q: cmp 0x2e(%si),%al; ja — it fires when the interval is reached",
        ),
        (
            0x420F,
            &[0x2E, 0x89, 0x44, 0x44, 0x66, 0x2E],
            "Q: mov %ax,%cs:0x44(%si) — m_pos = 0, the retrigger itself",
        ),
        (
            0x421F,
            &[0x8A, 0xA7, 0x6C, 0x09, 0x80, 0xFC, 0x00, 0x74],
            "Q: mov 0x96c(%bx),%ah; cmp $0x0,%ah; je — the multiplier table",
        ),
        (
            0x4228,
            &[0xF6, 0xE4, 0xB1, 0x04, 0xD3, 0xE8, 0xEB, 0x04],
            "Q: mul %ah; mov $0x4,%cl; shr %cl,%ax — the multiply path",
        ),
        (
            0x4230,
            &[0x02, 0x87, 0x5C, 0x09, 0x3C, 0x00, 0x7D],
            "Q: add 0x95c(%bx),%al — the addend path",
        ),
        (
            0x4246,
            &[0xFE, 0x44, 0x2E],
            "Q and `SDx`: incb 0x2e(%si) — one counter for both",
        ),
        (
            0x425C,
            &[0xFF, 0x97, 0x98, 0x3B, 0xC3],
            "S: the first-tick table's call: call *0x3b98(%bx)",
        ),
        (
            0x426F,
            &[0xFF, 0x97, 0xB8, 0x3B, 0xC3],
            "S: the later-tick table's: call *0x3bb8(%bx)",
        ),
        (
            0x4296,
            &[0x88, 0x44, 0x2E, 0xC3, 0x80],
            "SDx on the first tick: mov %al,0x2e(%si); ret",
        ),
        (
            0x42A5,
            &[0xE8, 0xB6, 0x04],
            "SDx later: call 0x475e — where the deferred note begins",
        ),
        (
            0x4751,
            &[0x3C, 0x13, 0x75, 0x0D, 0x80, 0xE4, 0xF0, 0x80, 0xFC, 0xD0, 0x75, 0x05, 0xC3],
            "the row parse's SDx gate: cmd == S and info & 0xf0 == 0xd0 returns early",
        ),
        (
            0x475E,
            &[0xC6, 0x44, 0x01, 0x81],
            "the note the delay starts: movb $0x81,0x1(%si)",
        ),
        (
            0x4762,
            &[0x8B, 0x4C, 0x32, 0x8B, 0x54, 0x34, 0x80, 0xFD, 0x65, 0x76],
            "the row parse's clamps: mov 0x32(%si),%cx; mov 0x34(%si),%dx; cmp $0x65,%ch",
        ),
        (
            0x477B,
            &[0xB2, 0x3F],
            "and mov $0x3f,%dl — a volume column of 64 becomes 63",
        ),
        (
            0x46A4,
            &[0xB8, 0x00, 0xFF, 0xBE, 0xA0, 0x0D, 0xB9, 0x30, 0x00],
            "the row reset: mov $0xff00,%ax; ...; mov $0x30,%cx — note 0xff, volume 0xff, the rest zero",
        ),
        (
            0x490E,
            &[0x83, 0x7C, 0x24, 0x00, 0x74, 0x0A, 0x80],
            "the note start: cmpw $0x0,0x24(%si); je (+0x491e)",
        ),
        (
            0x491A,
            &[0xC6, 0x44, 0x02, 0xFF],
            "aequ = 0xff, on the not-fresh path only",
        ),
        (
            0x4922,
            &[0x66, 0xC7, 0x44, 0x5C, 0x01, 0x01, 0x01, 0x01, 0x89, 0x44, 0x44, 0x33],
            "movl $0x1010101,0x5c(%si); mov %ax,0x44(%si) — m_pos takes the O offset",
        ),
        (
            0x4938,
            &[0xE8, 0xFF, 0x0F, 0xE8, 0xD5, 0x0E],
            "call 0x593a; call 0x5813 — the note to a period, scaled by the instrument's c2spd",
        ),
        (
            0x4944,
            &[0x80, 0x7C, 0x35, 0x07, 0x74, 0x0A, 0x89],
            "cmpb $0x7,0x35(%si); je (+0x4954) — a note under a G does not set aspd",
        ),
        (
            0x4954,
            &[0x89, 0x44, 0x24, 0x89, 0x44, 0x20, 0x8A, 0x44],
            "mov %ax,0x24(%si); mov %ax,0x20(%si) — aorgspd and asldspd both take it",
        ),
        (
            0x496A,
            &[0x80, 0x7C, 0x02, 0xFF, 0x75, 0x08, 0x8A, 0x44, 0x1A, 0x04, 0x10, 0x88],
            "aequ: from 0xff to avol + 0x10",
        ),
        (
            0x48D6,
            &[0x80, 0x7C, 0x35, 0x0F, 0x75, 0x09, 0x8A, 0x64, 0x36, 0x32, 0xC0, 0x2E],
            "O: cmpb $0xf,0x35(%si); mov 0x36(%si),%ah; xor %al,%al; mov %ax,%cs:0x6(%si) — info << 8",
        ),
        (
            0x5813,
            &[0xB9, 0xAB, 0x20, 0xF7, 0xE1, 0x8B, 0x4C, 0x22, 0x3B, 0xD1, 0x72, 0x04],
            "the c2spd scale: mov $0x20ab,%cx; mul %cx; mov 0x22(%si),%cx; cmp %cx,%dx",
        ),
        (
            0x5823,
            &[0xF7, 0xF1, 0x3D, 0xFF, 0x7F, 0x77, 0xF5, 0xC3],
            "div %cx; cmp $0x7fff,%ax; ja — the word-overflow guard",
        ),
        (
            0x58B9,
            &[0xBA, 0xDA, 0x00, 0xB8, 0x00, 0x76, 0xF7, 0xF1],
            "the step's base frequency: mov $0xda,%dx; mov $0x7600,%ax; div %cx — 0x00da7600",
        ),
        (
            0x593A,
            &[0x3C, 0xFE, 0x75, 0x04, 0xB8, 0x00],
            "the note to a period: cmp $0xfe,%al; jne; mov $0x0,%ax — the note off is 0",
        ),
        (
            0x594A,
            &[0x8A, 0x8F, 0x7C, 0x09, 0x8A, 0xD8, 0x80, 0xE3],
            "mov 0x97c(%bx),%cl — the octave's shift, with no bound on the index",
        ),
        (
            0x5955,
            &[0x8B, 0x87, 0x24, 0x0A, 0xD3, 0xE8, 0xC3],
            "mov 0xa24(%bx),%ax; shr %cl,%ax; ret — the table, shifted",
        ),
        (
            0x3D0C,
            &[0x33, 0xC0, 0x88, 0x44, 0x2E, 0x8B],
            "the first-tick pass, a channel with no command: xor %ax,%ax; mov %al,0x2e(%si)",
        ),
        (
            0x3D24,
            &[0x88, 0x44, 0x2E, 0x8B, 0x44],
            "and a D row: mov %al,0x2e(%si); mov 0x24(%si),%ax",
        ),
        (
            0x3C97,
            &[0xFE, 0x4C, 0x31, 0x75],
            "the volume-0 optimisation: decb 0x31(%si); jne",
        ),
        (
            0x3CA9,
            &[0xC6, 0x44, 0x01, 0x00],
            "and movb $0x0,0x1(%si) — three silent rows free the channel",
        ),
        (
            0x3CB9,
            &[0x3C, 0x00, 0x74, 0x03, 0x88, 0x44],
            "alastnfo: cmp $0x0,%al; je; mov %al,0x39(%si)",
        ),
        (
            0x3CEA,
            &[0x80, 0x4C, 0x26, 0x80],
            "the stale vibrato bit: orb $0x80,0x26(%si)",
        ),
        (
            0x3D72,
            &[0xC3],
            "the bare ret every absent command shares",
        ),
        (
            0x46C2,
            &[0x8C, 0xC8, 0x8E, 0xD8],
            "the row parse: mov %cs,%ax; mov %ax,%ds",
        ),
        (
            0x4371,
            &[0x83, 0x3E, 0x00, 0x20, 0x00, 0x75, 0x1C, 0x80],
            "cmpw $0x0,0x2000; jne (+0x4394) — the first tick's own test",
        ),
        (
            0x437F,
            &[0xE8, 0x40, 0x03],
            "and the row parse it calls: call 0x46c2",
        ),
        (
            0x4382,
            &[0xE8, 0xBF, 0xF8],
            "then the first-tick dispatch: call 0x3c44",
        ),
        (
            0x4394,
            &[0xE8, 0xA1, 0xF9],
            "and every later tick's: call 0x3d38",
        ),
        // B raises the same advance flag and writes the order to try next.
        (
            0x3DE2,
            &[0xC6, 0x06, 0x3A, 0x09, 0x01, 0x3C, 0xFF],
            "movb $0x1,0x93a; cmp $0xff,%al (B, and BFF)",
        ),
        // The channel initialiser and the mixer's shadow of the position: the offsets
        // `effects.rs`'s `Channel::reset` doc names, asserted rather than described. Two
        // labels in that doc were wrong for a round (`+0x5625` called a zero, `+0x5630` called
        // `m_base`) and nothing here would have caught them; these twelve entries are that gap.
        (
            0x5625,
            &[0x88, 0x5C, 0x03],
            "the initialiser: mov %bl,0x3(%si) — `channelnum` takes the loop's own channel index, not a zero",
        ),
        (
            0x5628,
            &[0x66, 0xC7, 0x44, 0x44, 0x01, 0x01, 0x01, 0x01],
            "movl $0x1010101,0x44(%si) — the marker goes into `m_pos`",
        ),
        (
            0x5630,
            &[0x66, 0xC7, 0x44, 0x5C, 0x01, 0x01, 0x01, 0x01],
            "and into +0x5c — the mixer's shadow, not `m_base`",
        ),
        (
            0x5641,
            &[0x89, 0x44, 0x58],
            "mov %ax,0x58(%si) with %ax zero (`xor %ax,%ax`, +0x561b) — `m_base`'s low word",
        ),
        (
            0x5644,
            &[0x89, 0x44, 0x5A],
            "and its high word at +0x5a",
        ),
        (
            0x5681,
            &[0xC6, 0x44, 0x3A, 0x65],
            "movb $0x65,0x3a(%si) — `lastadlins`, the loop's last store",
        ),
        (
            0x56A6,
            &[0xC6, 0x44, 0x04, 0xFF],
            "the silence pass the initialiser ends with: movb $0xff,0x4(%si)",
        ),
        (
            0x56AA,
            &[0xC6, 0x44, 0x3F, 0xFF],
            "and 0xff at +0x3f",
        ),
        (
            0x56AE,
            &[0xC6, 0x44, 0x41, 0x00],
            "and 0 at +0x41",
        ),
        (
            0x6FB8,
            &[0x66, 0x8B, 0x44, 0x5C, 0x66, 0x8B, 0x5C, 0x44, 0x66, 0x3B, 0xD8],
            "the driver's voice update: mov 0x5c(%si),%eax; mov 0x44(%si),%ebx; cmp %eax,%ebx \
             — +0x5c is the position's shadow and +0x44 the position, and the `je` past the \
             rewrite at +0x6FC7 makes the comparison's direction unobservable",
        ),
        (
            0x6FC7,
            &[0x66, 0x89, 0x5C, 0x5C],
            "and mov %ebx,0x5c(%si) rewrites it when the position has moved",
        ),
        (
            0x708C,
            &[0x8B, 0x4C, 0x58, 0x66, 0xC1, 0xE1, 0x0E],
            "mov 0x58(%si),%cx; shl $0xe,%ecx — +0x58 is the sample base, `m_base`",
        ),
        // ---------------------------------------------------------------------------------
        // Task 5 — the mixing engine. The voice, its arithmetic, and the bounds it plays within.
        // ---------------------------------------------------------------------------------
        // The per-frame loop's body (`+0x345d`'s inner block): the sample byte is read at
        // `es:(si)` with `si` = `m_pos`, looked up in the volume table at `(m_vol << 8) |
        // sample` (`bh` is `m_vol`, `bl` the byte), and the two frames' values are added into
        // the accumulators as one 32-bit `add`. `di` advances four bytes for two frames.
        (
            0x346C,
            &[
                0x03, 0xCD, 0x26, 0x8A, 0x1C, 0x13, 0xF2, 0x8A, 0x07, 0x66, 0xC1, 0xE0, 0x10, 0x03,
                0xCD, 0x26, 0x8A, 0x1C, 0x13, 0xF2, 0x8A, 0x07, 0x66, 0x01, 0x05, 0x83, 0xC7, 0x04,
            ],
            "add %bp,%cx; mov %es:(%si),%bl; adc %dx,%si; mov (%bx),%al; shl $0x10,%eax; \
             …twice…; add %eax,(%di); add $0x4,%di — the sample fetch, the volume lookup and \
             the 16.16 position advance, two frames at a time",
        ),
        // The volume table's arithmetic: `mul %bx` with `bx` = (level << 8) | sample value,
        // then `mov %ah` — an entry is `(sample × scale[level]) >> 8`, truncated.
        (
            0x5546,
            &[
                0xB9, 0x00, 0x01, 0xB8, 0x00, 0x00, 0x8A, 0xDF, 0x32, 0xFF, 0x8A, 0x18, 0x50, 0xF7,
                0xE3, 0x26, 0x88, 0x25, 0x47, 0x58, 0x40, 0xE2, 0xF5,
            ],
            "mov $0x100,%cx; mov $0x0,%ax; mov %bh,%bl; xor %bh,%bh; mov (%bx,%si),%bl; \
             push %ax; mul %bx; mov %ah,%es:(%di); inc %di; pop %ax; inc %ax; loop — 256 \
             entries of `(sample × scale) >> 8` per level",
        ),
        // The scale table itself, at `+0x9c4`: `floor(level × 255/63)`, which is four a level
        // from the bottom and a hair above it from level 21.
        (0x9C4, &[0x00, 0x04, 0x08, 0x0C], "the scale table: level 0 is 0, level 1 is 4, …"),
        (
            0x9FF,
            &[238],
            "… sixty entries later: level 59's scale is 238 = floor(59 × 255/63), where four a \
             level would be 236",
        ),
        (0xA03, &[255], "… and level 63's is 255, not 252"),
        // The centre a voice at a level contributes when its sample is at 128: the table is
        // read with `bl` = 0x80 and `bh` = `m_vol`, then shifted by the pan tables.
        (
            0x37AE,
            &[
                0xB3, 0x80, 0x64, 0x8A, 0x07, 0x32, 0xE4, 0x3E, 0x8B, 0x5E, 0x1B, 0x83, 0xE3, 0x07,
                0x80, 0xFB, 0x06, 0x73, 0x04, 0x32, 0x1E, 0x1E, 0x21, 0x03, 0x1E, 0x1C, 0x21,
            ],
            "mov $0x80,%bl; mov %fs:(%bx),%al; xor %ah,%ah; mov %ds:0x1b(%bp),%bx; and $0x7,%bx; \
             cmp $0x6,%bl; jae; xor 0x211e,%bl; add 0x211c,%bx — the centre read at \
             `(m_vol << 8) | 0x80`, and the pan index = `(amixtype & 7) ^ pass`",
        ),
        // The two shifts the pan tables give the centre before it is accumulated: `+0x38e4`
        // and `+0x38f4`, 16 bytes each, 0x10 meaning "not this side" (a word shifted by 16).
        (0x38E4, &[0x00, 0x10, 0x01, 0x10], "shift table for [0x2250]/[0x2252]"),
        (0x38F4, &[0x10, 0x00, 0x10, 0x01], "shift table for [0x2254]"),
        // `m_speed`: the object's own base frequency over the period, then over the mixing rate.
        (
            0x58B9,
            &[0xBA, 0xDA, 0x00, 0xB8, 0x00, 0x76, 0xF7, 0xF1],
            "mov $0xda,%dx; mov $0x7600,%ax; div %cx — 0x00da7600 over the period",
        ),
        (
            0x58C9,
            &[
                0xF7, 0x36, 0xD9, 0x08, 0x8B, 0xC8, 0x33, 0xC0, 0xF7, 0x36, 0xD9, 0x08, 0x89, 0x44,
                0x50, 0x89, 0x4C, 0x52,
            ],
            "divw 0x8d9; mov %ax,%cx; xor %ax,%ax; divw 0x8d9; mov %ax,0x50(%si); mov \
             %cx,0x52(%si) — the quotient and the remainder as m_speed's whole and fraction",
        ),
        // The stereo divisor's choice: the low word of `_zmixspeed` in mono, `[0x8d7]` in
        // stereo, stored as the effective rate the step is divided by.
        (
            0x5397,
            &[
                0x2E, 0xA2, 0x16, 0x09, 0x2E, 0x8B, 0x16, 0xD5, 0x08, 0xA8, 0x80, 0x74, 0x05, 0x2E,
                0x8B, 0x16, 0xD7, 0x08, 0x2E, 0x89, 0x16, 0xD9, 0x08,
            ],
            "mov %al,%cs:0x916 (np_mastermul); mov %cs:0x8d5,%dx; test $0x80,%al; je; mov \
             %cs:0x8d7,%dx; mov %dx,%cs:0x8d9 — the master multiplier byte and the rate it \
             selects, and [0x8d7] holds the segment's initial 21111 in stereo",
        ),
        // The two end paths: a one-shot stops (`m_speed = 0`) and a loop rewraps the position by
        // `m_loop - m_end`, keeping the overshoot.
        (
            0x383C,
            &[
                0x2E, 0x8B, 0x46, 0x4C, 0x3D, 0xFF, 0xFF, 0x75, 0x11, 0x66, 0x33, 0xC0, 0x66, 0x2E,
                0x89, 0x46, 0x50, 0x2E, 0xC7, 0x06, 0x4A, 0x21, 0x00, 0x80, 0xEB, 0x27, 0x2E, 0x2B,
                0x46, 0x48, 0x03, 0xF0,
            ],
            "mov 0x4c(%bp),%ax (m_loop); cmp $0xffff,%ax; jne; xor %eax,%eax; mov \
             %eax,0x50(%bp) (m_speed = 0); movw $0x8000,0x214a; jmp; sub 0x48(%bp),%ax; add \
             %ax,%si — the one-shot's stop and the loop's rewrap",
        ),
        // The note start's bounds: `m_loop` = the header's loop end, `m_end` reduced by whole
        // loop lengths until it is small, and the one-shot's `m_end` = length + 0x20 with the
        // 0xffff marker.
        (
            0x489C,
            &[
                0x3D, 0x00, 0x00, 0x74, 0x1D, 0x26, 0x8B, 0x16, 0x14, 0x00, 0x89, 0x54, 0x4C, 0x2B,
                0xD0, 0x3D, 0xF4, 0x01, 0x77, 0x09, 0x2B, 0xC2, 0x3D, 0xF4, 0x01, 0x72, 0xF9, 0x03,
                0xC2, 0x89, 0x44, 0x48,
            ],
            "cmp $0x0,%ax; je; mov %es:0x14,%dx; mov %dx,0x4c(%si) (m_loop = the loop end); \
             sub %ax,%dx; cmp $0x1f4,%ax; ja; sub %dx,%ax; cmp $0x1f4,%ax; jb; add %dx,%ax; \
             mov %ax,0x48(%si) — m_end reduced by loop lengths until it is below 500",
        ),
        (
            0x48BE,
            &[0x26, 0xA1, 0x10, 0x00, 0x05, 0x20, 0x00, 0x89, 0x44, 0x48, 0xB8, 0xFF, 0xFF, 0x89, 0x44, 0x4C],
            "mov %es:0x10,%ax; add $0x20,%ax; mov %ax,0x48(%si); mov $0xffff,%ax; mov \
             %ax,0x4c(%si) — the one-shot's bounds, length plus the padding, m_loop = 0xffff",
        ),
        // The output conversion's other half: the accumulator less the centre sums, indexed
        // into the table the builder wrote at `cs:[0x6c]`.
        (
            0x30AD,
            &[0x2E, 0xA1, 0x6C, 0x00, 0x05, 0x00, 0x04, 0x2E, 0x2B, 0x06, 0x50, 0x22],
            "mov %cs:0x6c,%ax; add $0x400,%ax; sub %cs:0x2250,%ax — the table's base plus \
             0x400 less the mono centre sum",
        ),
        (
            0x2CAF,
            &[0x8B, 0xCA, 0xB8, 0x00, 0x04, 0x2E, 0x2B, 0x06, 0x54, 0x22],
            "mov %dx,%cx; mov $0x400,%ax; sub %cs:0x2254,%ax — the stereo right side's",
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
fn a_speed_takes_its_parameter_whatever_it_is_and_a_tempo_is_split_at_32() {
    // **Which command reached the handler, not which parameter it carries, is what picks
    // the arithmetic** — and Task 4 read the tables and corrected this test, which had it
    // the other way round.
    //
    // The first-tick table at `+0x3bd8` sends command 1 (`A`) to `+0x3dd2`, whose whole body
    // is `cmp $0x0,%al; je; xor %ah,%ah; mov %ax,%cs:0x2002` — **there is no upper bound**,
    // so an `A` with a parameter of 0x21 sets a speed of 33. Command 20 (`T`) goes to
    // `+0x3d73` instead, whose `cmp $0x20,%ax; jbe 0x3dce` sends 32 and under to the driver
    // call and never to the speed.
    //
    // This is not a corner: MUSIC1's `A` parameters are `{3: 30, 6: 23, **127: 1**}`, so one
    // of its cells plays a row at speed 127 — see `tests/effects.rs` and the note.
    let module = Builder::new(&[0], &[cell(0, None, Some((1, 0x20)))]).build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    poll_until(&mut player, 10, |player, _| player.state().speed != 6);
    assert_eq!(player.state().speed, 0x20, "A20 is a speed of 32");
    assert_eq!(player.state().tempo, 125);

    let module = Builder::new(&[0], &[cell(0, None, Some((1, 0x21)))]).build();
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    poll_until(&mut player, 10, |player, _| player.state().speed != 6);
    assert_eq!(
        player.state().speed,
        0x21,
        "A21 is a speed of 33, not a tempo"
    );
    assert_eq!(player.state().tempo, 125, "and the tempo is untouched");

    // The `T` handler's own split, through its own table entry: 32 and under does nothing.
    for (parameter, expected) in [(0x20, 125), (0x21, 0x21)] {
        let module = Builder::new(&[0], &[cell(0, None, Some((20, parameter)))]).build();
        let mut player = Player::new(module, PlayerOptions::default());
        player.play_song(0);
        for _ in 0..20 {
            player.poll();
        }
        assert_eq!(player.state().tempo, expected, "T{parameter:02X}");
        assert_eq!(player.state().speed, 6, "and the speed is the header's");
    }
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
                // The range is the *parameter's*, 1..=255, not the ST3 documentation's
                // "1..32": the `A` handler is `mov %ax,0x2002` with no bound at all
                // (`+0x3dd6`), and MUSIC1 carries one `A7F` cell (order 98, row 48), so a
                // row legitimately spends 127 ticks — which is what this row-timer check
                // exists to catch the *shape* of, not the size.
                assert!(
                    (1..=255).contains(&ticks_this_row),
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
            // A speed is the `A` handler's parameter, 1..=255 — the speed is *not* bounds
            // checked, and MUSIC1's `A7F` cell (order 98, row 48) publishes 127. Zero is the
            // only value the handler refuses (`+0x3dd2`).
            assert!(position.speed != 0, "{name}: poll {poll} published speed 0");
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

