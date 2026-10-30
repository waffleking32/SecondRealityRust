//! The player: the object it is read from, and what it does.
//!
//! 1. **The object.** `MAIN/STMIK.300` is an OMF object with no source in the tree.
//!    [`segment_image`] rebuilds its segment from the `LEDATA`/`LIDATA` records, and the tests
//!    hold the port's tables, its song initialiser and its boot state to those bytes. The `+0x…`
//!    offsets the src docs cite are offsets into this image (`objdump -b binary -m i8086 -D`).
//! 2. **The behaviour**, on the shipped modules and on modules built here for the cells the
//!    shipped data does not carry (a `B`, a `255`, a `T`).

mod common;

use std::collections::BTreeMap;

use common::oracle;
use common::{first_rows, module, player, Note};
use sr_audio::{MixRate, Player, PlayerOptions, Position, CHANNELS, ROWS};

// -------------------------------------------------------------------------------------------
// The object
// -------------------------------------------------------------------------------------------

/// The original's segment image, rebuilt from the object's records.
///
/// `0xA0` is `LEDATA` (a segment offset and its bytes), `0xA2` is `LIDATA` (the same, as
/// iterated blocks: a two-byte repeat count, a two-byte block count and then, for a block count
/// of zero, a one-byte length and that many bytes, else that many nested blocks). The object's
/// 105 `LEDATA` and 118 `LIDATA` records tile one 29,607-byte segment, the length `sr-assets`'s
/// symbol map records for `text_stmik300_text`.
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
            0xA2 if body.len() >= 7 && body[0] == 1 => {
                let offset = word(body, 1) as usize;
                // The blocks run to the checksum byte that ends the record.
                let mut data = Vec::new();
                let mut at = 3;
                while at < body.len() - 1 {
                    at = iterated(body, at, &mut data, 0);
                }
                assert_eq!(at, body.len() - 1, "LIDATA at 0x{offset:04X}: the blocks overrun");
                place(offset, &data);
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

/// One iterated block of a `LIDATA` record at `at`, appended to `out`; returns the offset just
/// past it. The layout is the OMF one `segment_image`'s comment gives.
fn iterated(body: &[u8], at: usize, out: &mut Vec<u8>, depth: usize) -> usize {
    assert!(depth < 8, "LIDATA nesting deeper than the format allows");
    let word = |at: usize| u16::from_le_bytes([body[at], body[at + 1]]) as usize;
    let (repeat, blocks) = (word(at), word(at + 2));
    let mut block = Vec::new();
    let mut next = at + 4;
    if blocks == 0 {
        let length = usize::from(body[next]);
        block.extend_from_slice(&body[next + 1..next + 1 + length]);
        next += 1 + length;
    } else {
        for _ in 0..blocks {
            next = iterated(body, next, &mut block, depth + 1);
        }
    }
    for _ in 0..repeat {
        out.extend_from_slice(&block);
    }
    next
}

/// The image's little-endian word at `at`.
fn word(image: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([image[at], image[at + 1]])
}

/// **The port's tables, and the oracle's, are the object's.** The port's public tables and
/// arithmetic are read against the image directly; the oracle's (`common::oracle`), which the
/// effect and mixer tests hold the port to, are read against it here.
#[test]
fn the_ports_tables_are_the_objects_own() {
    let image = segment_image();
    assert_eq!(&image[1..45], b"Scream Tracker V3.0 Music Interface Kit V0.9");

    // The volume scale at `+0x9c4`.
    assert_eq!(&image[0x9C4..0xA04], &sr_audio::VOLUME_SCALE, "the port's VOLUME_SCALE");
    assert_eq!(&image[0x9C4..0xA04], &oracle::SCALE);

    // `+0x593a`: the periods at `+0xa24` shifted by the byte `+0x97c + octave`, unchecked, so
    // octaves 8…15 read the bytes after the eight shifts. From octave 11 the shift is 16 or more
    // and a word shift overflows; no module reaches past octave 8.
    for octave in 0..=10u8 {
        for semitone in 0..12u8 {
            let note = (octave << 4) | semitone;
            let period = word(&image, 0xA24 + 2 * usize::from(semitone));
            let expected = period >> image[0x97C + usize::from(octave)];
            assert_eq!(sr_audio::note_period(note), expected, "the port's note {note:#04x}");
            assert_eq!(oracle::note_period(note), expected, "the oracle's note {note:#04x}");
        }
    }
    let periods: Vec<u16> = (0..12).map(|i| word(&image, 0xA24 + 2 * i)).collect();
    assert_eq!(periods, oracle::PERIODS);
    assert_eq!(&image[0x97C..0x98C], &oracle::SHIFTS);
    assert_eq!(sr_audio::note_period(0xfe), 0, "`+0x593a`'s note-off test");

    // The vibrato sine at `+0xa50`, and `Q`'s addends and multipliers at `+0x95c`/`+0x96c`.
    let sine: Vec<i16> = (0..64).map(|i| word(&image, 0xA50 + 2 * i) as i16).collect();
    assert_eq!(sine, oracle::SINE);
    let addends: Vec<i8> = image[0x95C..0x96C].iter().map(|&byte| byte as i8).collect();
    assert_eq!(addends, oracle::Q_ADD);
    assert_eq!(&image[0x96C..0x97C], &oracle::Q_MULTIPLY);

    // `+0x58b9`: `mov $0xda,%dx; mov $0x7600,%ax; div %cx`, the step's base frequency.
    assert_eq!((image[0x58B9], image[0x58BC]), (0xBA, 0xB8), "mov %dx, mov %ax");
    let base = u32::from(word(&image, 0x58BA)) << 16 | u32::from(word(&image, 0x58BD));
    assert_eq!(u64::from(base), oracle::BASE_FREQ);
    for period in [1u16, 428, 1712, 27392] {
        assert_eq!(sr_audio::period_step(period), base / u32::from(period), "period {period}");
    }
    assert_eq!(sr_audio::period_step(1712), 8362, "400 short of 8363 × 1712");
    assert_eq!(sr_audio::period_step(0), 0);

    // `+0x5813`'s `mov $0x20ab,%cx`: the c2spd reference.
    assert_eq!(image[0x5813], 0xB9, "mov %cx");
    assert_eq!(u32::from(word(&image, 0x5814)), oracle::C2SPD_REFERENCE);
    assert_eq!(sr_audio::scaled_period(1712, 8363), 1712);

    // The stereo divisor `[0x8d7]`, and the period bounds `[0x958]`/`[0x95a]` the step clamps to.
    let divisor = word(&image, 0x8D7);
    assert_eq!(u64::from(divisor), oracle::STEREO_DIVISOR);
    let stereo = |period| sr_audio::step(period, MixRate::Hz40000, true);
    assert_eq!(stereo(1712), oracle::step(1712, u64::from(divisor)));
    let (high, low) = (word(&image, 0x958), word(&image, 0x95A));
    assert_eq!((high, low), (0x7FFF, 0x40));
    assert_eq!(stereo(0xFFFF), stereo(high), "the high bound");
    assert_eq!(stereo(1), stereo(low), "the low bound");
}

/// The song initialiser's stores, decoded from the object: the loop body `+0x5625`…`+0x5684`,
/// the silence pass `+0x56a6`…`+0x56b1` that `+0x56bc` calls after it, and `+0x5781`'s
/// `andb $0x7f,0x1(%si)` (`+0x5791`), which the initialiser itself ends with. Each byte of the
/// 0x60-byte channel struct that a store reaches maps to the value it leaves there.
fn initialiser_stores() -> BTreeMap<u8, u8> {
    let image = segment_image();
    let mut stored: BTreeMap<u8, u8> = BTreeMap::new();
    let mut decode = |from: usize, to: usize| {
        let mut at = from;
        while at < to {
            // `ax` is zero through the loop (`xor %ax,%ax` at `+0x561b`), so a register store is
            // a zero; `bl`, `channelnum`'s source, is the channel's own index — set apart here.
            let si = |modrm: u8| modrm & 0xC7 == 0x44;
            let (displacement, length, values) = match image[at..at + 3] {
                [0x88, 0x5C, d] => (d, 3, vec![0xEE]),
                [0x88, modrm, d] if si(modrm) => (d, 3, vec![0]),
                [0x89, modrm, d] if si(modrm) => (d, 3, vec![0, 0]),
                [0xC6, 0x44, d] => (d, 4, vec![image[at + 3]]),
                [0x66, 0xC7, 0x44] => (image[at + 3], 8, image[at + 4..at + 8].to_vec()),
                _ => panic!("0x{at:04X}: not one of the initialiser's store encodings"),
            };
            for (byte, value) in values.into_iter().enumerate() {
                stored.insert(displacement + byte as u8, value);
            }
            at += length;
        }
        assert_eq!(at, to, "the stores decode to their end");
    };
    // The routines are reached, not merely present: a `call rel16` lands at its own end plus the
    // displacement. `_zplaysong` calls `+0x56bc` (`+0x258e`); `+0x56bc` calls the initialiser
    // (`+0x5716`) and then the silence pass (`+0x5719`); the initialiser ends by calling
    // `+0x4aa0` (`+0x5693`) and `+0x5781` (`+0x5696`).
    let calls = |at: usize| -> usize {
        assert_eq!(image[at], 0xE8, "0x{at:04X} is a near call");
        let displacement = i16::from_le_bytes([image[at + 1], image[at + 2]]);
        (at as i32 + 3 + i32::from(displacement)) as usize
    };
    for (at, target) in [
        (0x258E, 0x56BC),
        (0x5716, 0x5614),
        (0x5719, 0x569A),
        (0x5693, 0x4AA0),
        (0x5696, 0x5781),
    ] {
        assert_eq!(calls(at), target, "0x{at:04X} calls 0x{target:04X}");
    }
    // And none of their other callees stores to `_zchn` on the measured card (output mode 5, SB
    // Pro, at `+0x882`): `+0x4aa0` fills the AdLib register shadow at `+0x200c` and writes the
    // AdLib ports (`+0x4a25`), globals only; `+0x5781`'s per-channel `+0x2be7` and its
    // `+0x2bf8` return at once while `[0x89e]` is 0, which `+0x2a94` sets to 1 only for modes 7
    // and 2; `+0x569a`'s `+0x2c7a` calls the driver's function `0xf`, and the SB driver
    // `+0x5a60` returns for any function at or past `0xb`; and `+0x5781`'s `+0x4d88` runs only
    // when `[0x8a3]` is set, which `+0x4aa0` clears (`+0x4ade`) just before.
    assert_eq!(&image[0x2BE7..0x2BEF], &[0x2E, 0x80, 0x3E, 0x9E, 0x08, 0x00, 0x74, 0x08]);
    assert_eq!(&image[0x2BF8..0x2C00], &[0x2E, 0x80, 0x3E, 0x9E, 0x08, 0x00, 0x74, 0x0A]);
    assert_eq!(&image[0x2A94..0x2A9A], &[0x2E, 0xC6, 0x06, 0x9E, 0x08, 0x00], "[0x89e] = 0 …");
    assert_eq!(&image[0x2AA4..0x2AAA], &[0x2E, 0xC6, 0x06, 0x9E, 0x08, 0x01], "… 1 for 7 or 2");
    assert_eq!(&image[0x0882..0x0884], &[0x05, 0x00], "the object's output mode, SB Pro");
    let driver = |mode: usize| u16::from_le_bytes([image[0x2A6F + 2 * mode], image[0x2A70 + 2 * mode]]);
    assert_eq!(driver(5), 0x5A60, "mode 5's driver");
    assert_eq!(
        &image[0x5A60..0x5A6D],
        &[0xD1, 0xE3, 0x83, 0xFB, 0x16, 0x73, 0x05, 0x2E, 0xFF, 0x97, 0xB6, 0x5A, 0xCB],
        "shl %bx; cmp $0x16,%bx; jae lret — function 0xf (0x1e) is past the table"
    );
    assert_eq!(&image[0x2C7D..0x2C80], &[0xBB, 0x0F, 0x00], "+0x2c7a asks for function 0xf");
    assert_eq!(&image[0x4ADE..0x4AE4], &[0x2E, 0xC6, 0x06, 0xA3, 0x08, 0x00], "[0x8a3] = 0");
    assert_eq!(&image[0x57A4..0x57AA], &[0x2E, 0x80, 0x3E, 0xA3, 0x08, 0x00], "+0x4d88's gate");

    decode(0x5625, 0x5685);
    assert_eq!(&image[0x56A2..0x56A6], &[0xBE, 0xA0, 0x0D, 0xFA], "the silence pass walks `_zchn`");
    decode(0x56A6, 0x56B2);
    assert_eq!(
        &image[0x578B..0x5795],
        &[0xF6, 0x44, 0x01, 0x80, 0x74, 0x0B, 0x80, 0x64, 0x01, 0x7F],
        "`+0x5781`: testb $0x80,0x1(%si); je; andb $0x7f,0x1(%si)"
    );
    let used = stored.get_mut(&0x01).expect("achannelused is stored");
    *used &= 0x7F;
    stored
}

/// **A restart writes exactly what the song initialiser writes, and keeps the rest** (D45):
/// `Channel::initialise` against the stores the object's own bytes make. The object's fields the
/// port does not keep (`aequ`, `aorgvol`, `atreon`, `atremor`, `vibtretype`, `a0clearcnt`) have no row.
///
/// Every field of the port's channel is given a value no store leaves, the initialiser is run,
/// and each field must then hold the store's value if any of its bytes is stored to, and its own
/// value if none is. A field the port keeps that the object writes — or the other way round —
/// fails here by name.
#[test]
fn the_song_initialiser_writes_exactly_what_initialise_writes() {
    use sr_audio::Channel;
    let stored = initialiser_stores();
    println!("the initialiser stores to {:x?}", stored.keys().collect::<Vec<_>>());

    let before = Channel {
        used: true,
        volume: 0x2a,
        period: 0x1414,
        target_period: 0x1515,
        c2spd: 0x1616,
        org_period: 0x1717,
        vibrato_position: 0x1818,
        last_effect1: 0x19,
        last_effect2: 0x1a,
        retrigger_count: 0x1b,
        zero_volume_count: 0x1d,
        note: 0x1e,
        instrument: 0x1f,
        cell_volume: 0x20,
        command: 0x21,
        info: 0x22,
        last_instrument: 0x23,
        last_note: 0x24,
        last_info: 0x25,
        sample_offset: 0x2828,
        position: 0x2929_2929,
        position_low: 0x3030,
    };
    let mut after = before;
    after.initialise();

    // (field, offset, width in the object's struct, the port's value as a number)
    type Read = fn(&Channel) -> u64;
    let fields: [(&str, u8, u8, Read); 22] = [
        ("used (achannelused)", 0x01, 1, |c| u64::from(c.used)),
        // `sample_offset` *is* the word at `+0x06` (`Oxy` writes `info << 8` there), not its
        // high half: the projection is the field itself.
        ("sample_offset (aadr's high word)", 0x06, 2, |c| u64::from(c.sample_offset)),
        ("period (aspd)", 0x12, 2, |c| u64::from(c.period)),
        ("volume (avol)", 0x1a, 1, |c| u64::from(c.volume)),
        ("target_period (asldspd)", 0x20, 2, |c| u64::from(c.target_period)),
        ("c2spd (ac2spd)", 0x22, 2, |c| u64::from(c.c2spd)),
        ("org_period (aorgspd)", 0x24, 2, |c| u64::from(c.org_period)),
        ("vibrato_position (avibcnt)", 0x26, 2, |c| u64::from(c.vibrato_position)),
        ("last_effect1 (alasteff1)", 0x2b, 1, |c| u64::from(c.last_effect1)),
        ("last_effect2 (alasteff2)", 0x2c, 1, |c| u64::from(c.last_effect2)),
        ("retrigger_count (atrigcnt)", 0x2e, 1, |c| u64::from(c.retrigger_count)),
        ("zero_volume_count (a0volcut)", 0x31, 1, |c| u64::from(c.zero_volume_count)),
        ("note", 0x32, 1, |c| u64::from(c.note)),
        ("instrument (ins)", 0x33, 1, |c| u64::from(c.instrument)),
        ("cell_volume (vol)", 0x34, 1, |c| u64::from(c.cell_volume)),
        ("command (cmd)", 0x35, 1, |c| u64::from(c.command)),
        ("info", 0x36, 1, |c| u64::from(c.info)),
        ("last_instrument (lastins)", 0x37, 1, |c| u64::from(c.last_instrument)),
        ("last_note (lastnote)", 0x38, 1, |c| u64::from(c.last_note)),
        ("last_info (alastnfo)", 0x39, 1, |c| u64::from(c.last_info)),
        ("position_low (m_poslow)", 0x42, 2, |c| u64::from(c.position_low)),
        ("position (m_pos)", 0x44, 4, |c| u64::from(c.position)),
    ];
    let mut kept = Vec::new();
    for (name, offset, width, read) in fields {
        let bytes: Vec<Option<u8>> = (offset..offset + width).map(|b| stored.get(&b).copied()).collect();
        if bytes.iter().all(Option::is_none) {
            assert_eq!(read(&after), read(&before), "{name} (+0x{offset:02x}): no store, so it is kept");
            kept.push(name);
        } else {
            let value = bytes
                .iter()
                .rev()
                .fold(0u64, |acc, byte| (acc << 8) | u64::from(byte.expect("a whole field is stored")));
            assert_eq!(read(&after), value, "{name} (+0x{offset:02x}): the store's value");
        }
    }
    println!("kept across a restart: {kept:?}");
    assert_eq!(
        kept,
        [
            "sample_offset (aadr's high word)",
            "volume (avol)",
            "c2spd (ac2spd)",
            "retrigger_count (atrigcnt)",
            "zero_volume_count (a0volcut)",
            "position_low (m_poslow)",
        ]
    );
}

/// **A restart keeps the previous module's channel volumes and clears `lastins`** (D45): block
/// 1's GLENZ restart, `restartmus(1, 0)` at the port's boundary 8,057 (`MAIN/U2.ASM:813-818`),
/// on block 1's MUSIC0 from its anchor 400 (`second1`'s, the port's
/// `BLOCK_ONE_MUSIC_START_FRAME`). Channel 2 is at 63 when MUSIC0 hands over, and the restarted
/// MUSIC1 still has it — where a fresh player, a boot, has 0.
#[test]
fn a_restart_keeps_the_previous_songs_volume_and_clears_lastins() {
    let mut music0 = player("MUSIC0");
    music0.play_song(0);
    for _ in 400..8_057 {
        music0.poll();
    }
    let handed = *music0.channel(2).unwrap();
    println!("MUSIC0's channel 2 at 8,057: {handed:?}, zframe {}", music0.zframe());
    assert_eq!(handed.volume, 63, "MUSIC0's channel 2 is sounding");
    assert_ne!(handed.last_instrument, 0, "and has loaded an instrument");

    let mut music1 = player("MUSIC1");
    music1.restart_from(&music0, 0);
    let restarted = *music1.channel(2).unwrap();
    assert_eq!(restarted.volume, 63, "`avol` is kept");
    assert_eq!(restarted.zero_volume_count, handed.zero_volume_count, "and `a0volcut`");
    assert_eq!(restarted.last_instrument, 0, "`lastins` is cleared");
    assert!(!restarted.used, "`achannelused` is 0 after `+0x5781`");
    assert_eq!(music1.zframe(), music0.zframe(), "`_np_zframe` is kept");
    for index in 0..CHANNELS {
        let (old, new) = (music0.channel(index).unwrap(), music1.channel(index).unwrap());
        let mut expected = *old;
        expected.initialise();
        assert_eq!(new, &expected, "channel {index}: the old channel, initialised");
    }

    let mut boot = player("MUSIC1");
    boot.play_song(0);
    assert_eq!(boot.channel(2).unwrap().volume, 0, "a boot's channel 2 is silent");
}

/// **A boot is the first `_zplaysong`, onto the object's initial state** — and that state is
/// zero wherever the song initialiser does not write (D45).
///
/// Every boot path reaches its first `_zplaysong` with no tick in between: `_zinit`
/// (`MAIN/U2.ASM:747`), the boot's `restartmus` and its `_zinitmodule`, then `_zsoundon` and
/// `_zplaysong(0)` (`:776-786`) — `zpollmix` is not set until `:783`, so nothing has mixed or
/// ticked. What a boot's channels hold, then, is the object's own image wherever
/// `+0x5614` does not store. The image's `_zchn` (`+0x0da0`, 48 × 0x60 bytes), the loop
/// counter `[0x90b]`, `_np_zframe` (`+0x8fb`), the ring (`+0x2150`, 256 bytes), its two
/// pointers (`+0x214c`, `+0x214e`) and the fed words (`+0x905`…`+0x90e`) are all zero, and so
/// is `Player::new`'s state: a boot is a fresh player.
#[test]
fn a_boot_starts_on_the_objects_zeroed_channels() {
    let image = segment_image();
    let zeroed = |from: usize, length: usize| image[from..from + length].iter().all(|&b| b == 0);
    assert!(zeroed(0x0da0, 48 * 0x60), "`_zchn`, every byte of the 48 channels");
    assert!(zeroed(0x090b, 2), "the loop counter [0x90b]");
    assert!(zeroed(0x08fb, 2), "`_np_zframe`");
    assert!(zeroed(0x2150, 0x100), "the ring");
    assert!(zeroed(0x214c, 4), "its read and write pointers, before `_zinit` primes them");
    assert!(zeroed(0x0905, 10), "the words a row advance feeds it");

    let fresh = player("MUSIC0");
    assert_eq!(fresh.zframe(), 0);
    for index in 0..CHANNELS {
        assert_eq!(fresh.channel(index), Some(&sr_audio::Channel::default()), "channel {index}");
    }
}

// -------------------------------------------------------------------------------------------
// Helpers
// -------------------------------------------------------------------------------------------

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
fn a_seek_mid_song_clears_the_tick_counter_so_the_new_rows_cells_are_read() {
    // `_zplaysong`'s routine zeroes `[0x2000]` — its own first instruction, `+0x56ec`'s
    // `movw $0x0,0x2000` — and the shell's restarts reach it **mid-song**: `restartmus`
    // (`MAIN/U2.ASM:348-397`) is called with the player part-way through a row at `:775`,
    // `:818`, `:863`, `:885`, `:894` and `:905`. A stale counter begins the new order's row 0
    // on a *later* tick, because `+0x4371`'s `cmpw $0x0,0x2000` is then false: the row's cells
    // are never read, and its notes and commands never start.
    //
    // The observable is an `A` command on the row the seek lands on — the row's own first act,
    // and the one thing a row that is never processed cannot do.
    let module = first_rows(
        &[0, 1],
        &[
            &[Note::command(0, 1, 0x09)], // pattern 0 row 0: A09
            &[Note::command(0, 1, 0x0c)], // pattern 1 row 0: A0C, what the seek must read
        ],
    );
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    // One tick in: pattern 0's row 0 has been processed, so the counter is part-way through
    // the row — exactly the state the shell's restart finds the player in.
    poll_until(&mut player, 10, |player, _| player.state().speed == 9);

    player.play_song(1);
    let at = poll_until(&mut player, 6, |player, _| player.state().speed == 0x0c);
    assert_eq!(player.engine_position().order, 1);
    assert_eq!(
        player.engine_position().row,
        0,
        "the seek's row 0 is the row whose cells were read"
    );
    println!("the seek's own row 0 was processed {at} poll(s) after it");
}

#[test]
fn the_published_position_is_ten_polls_behind_the_engine() {
    // The ring is primed ten entries deep (`+0x2ac1`). The engine enters row 1 six ticks
    // after the seek; the ring publishes it ten polls after the poll that pushed it.
    let mut player = player("MUSIC0");
    player.play_song(25);
    let row_begins = ticks_to_polls(6, MixRate::Hz40000, 120);

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
        "the A01 shows at poll {published}: speed 1 while the published row is pattern 70's \
         last, and the engine's own row is {}",
        player.engine_position().row
    );
}

#[test]
fn a_speed_takes_its_parameter_whatever_it_is_and_a_tempo_is_split_at_32() {
    // **Which command reached the handler, not which parameter it carries, is what picks
    // the arithmetic.**
    //
    // The first-tick table at `+0x3bd8` sends command 1 (`A`) to `+0x3dd2`, whose whole body
    // is `cmp $0x0,%al; je; xor %ah,%ah; mov %ax,%cs:0x2002` — **there is no upper bound**,
    // so an `A` with a parameter of 0x21 sets a speed of 33. Command 20 (`T`) goes to
    // `+0x3d73` instead, whose `cmp $0x20,%ax; jbe 0x3dce` sends 32 and under to the driver
    // call and never to the speed.
    //
    // This is not a corner: MUSIC1's `A` parameters are `{3: 30, 6: 23, **127: 1**}`, so one
    // of its cells plays a row at speed 127 — see `tests/effects.rs` and the note.
    let module = first_rows(&[0], &[&[Note::command(0, 1, 0x20)]]);
    let mut player = Player::new(module, PlayerOptions::default());
    player.play_song(0);
    poll_until(&mut player, 10, |player, _| player.state().speed != 6);
    assert_eq!(player.state().speed, 0x20, "A20 is a speed of 32");
    assert_eq!(player.state().tempo, 125);

    let module = first_rows(&[0], &[&[Note::command(0, 1, 0x21)]]);
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
        let module = first_rows(&[0], &[&[Note::command(0, 20, parameter)]]);
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
    let module = first_rows(&[0, 1], &[&[Note::command(0, 3, 0x12)], &[]]); // C12, then empty
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
    let module = first_rows(&[0, 1, 2], &[&[Note::command(0, 3, 0x99)], &[], &[]]);
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
    // Pattern 0's row 0 jumps to order 2; pattern 1 must never be played.
    let module = first_rows(
        &[0, 1, 2],
        &[&[Note::command(0, 2, 2)], &[Note::command(0, 1, 1)], &[]],
    );
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
    // own successor at order 3 — **3**.
    let mut player = Player::new(
        first_rows(&[0, 254, 1, 254], &[&[], &[]]),
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
}

#[test]
fn a_255_ends_the_order_list_and_loops_when_the_song_was_already_running() {
    // `+0x4469`…`0x449b`: a 255 stops a song whose first row has not played yet, and
    // otherwise increments `_np_loop` and restarts the scan at the loop order `_zplaysong`
    // recorded — which is 0 for every order the shell hands it.
    let module = first_rows(&[0, 1, 255], &[&[], &[]]);

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
fn a_stopped_player_keeps_its_position() {
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

/// Each module from the order the demo's boot starts it at (`MAIN/U2.ASM:786` pushes 0), for
/// 100,000 polls (about 24 minutes): every published word stays inside the module's tables,
/// every row the engine enters gets 1..=255 ticks (`A` is unbounded, `+0x3dd6`, and MUSIC1's
/// `A7F` row takes 127), and the song loops rather than ending. The first loop's polls, ticks
/// and rows are printed.
#[test]
fn each_module_plays_to_its_loop_inside_its_own_tables() {
    for name in ["MUSIC0", "MUSIC1"] {
        let module = module(name);
        let mut player = player(name);
        player.play_song(0);
        let (mut ticks, mut rows, mut ticks_this_row) = (0i64, 0u32, 0u32);
        let mut last_row = None;
        let (mut started, mut looped, mut wraps, mut previous) = (false, false, 0u32, 0u8);
        for poll in 0..100_000u32 {
            let before = player.zframe();
            let engine = player.engine_position();
            player.poll();
            let position = player.state();
            if !looped {
                let spent = u32::try_from(player.zframe() - before).expect("a few ticks a poll");
                ticks += i64::from(spent);
                if last_row != Some((engine.order, engine.row)) {
                    if rows > 0 {
                        assert!(
                            (1..=255).contains(&ticks_this_row),
                            "{name}: the row at {last_row:?} was given {ticks_this_row} ticks"
                        );
                    }
                    rows += 1;
                    ticks_this_row = 0;
                    last_row = Some((engine.order, engine.row));
                }
                ticks_this_row += spent;
            }

            assert!(
                usize::from(position.order) < module.order_count(),
                "{name}: poll {poll} published order {}",
                position.order
            );
            assert!(
                usize::from(position.pattern) < module.pattern_count(),
                "{name}: poll {poll} published pattern {}",
                position.pattern
            );
            assert!(position.row < ROWS as u8, "{name}: poll {poll} published row {}", position.row);
            assert!(
                (25..=255).contains(&position.tempo),
                "{name}: poll {poll} published tempo {}",
                position.tempo
            );
            assert!(position.speed != 0, "{name}: poll {poll} published speed 0");
            assert!(!player.finished(), "{name}: a song played from order 0 loops");

            started |= position.order != 0;
            if started && position.order < previous {
                wraps += 1;
                if !looped {
                    looped = true;
                    println!(
                        "  {name}: wrapped after {poll} polls ({:.1} seconds of logic ticks) at \
                         order {} pattern {} row {}, loop counter {}",
                        f64::from(poll) / 70.086,
                        position.order,
                        position.pattern,
                        position.row,
                        position.r#loop,
                    );
                    println!("  {name}: {poll} poll(s), {ticks} tick(s) over {rows} row(s)");
                    assert!(position.r#loop > 0, "{name}: the wrap is a loop");
                }
            }
            previous = position.order;
        }
        assert!(looped, "{name}: no loop in 100,000 polls");
        println!("  {name}: {wraps} wrap(s) in 100,000 polls, none past the order list");
    }
}

/// **A14(a), settled: the flush clears the accumulator word it reads, so nothing marks the plane.**
///
/// Both flushes load a zero into a 32-bit register before the loop and store that register
/// back over every accumulator dword they have just read: the stereo flush's `mov $0x0,%esp`
/// (`+0x2ce5`, with `%esp` saved at `+0x2cde` and interrupts off at `+0x2ce4`) and its
/// `mov %esp,(%si)` (`+0x2cf1`), and the mono flush's `mov $0x0,%ecx` (`+0x30d0`) and
/// `mov %ecx,(%si)` (`+0x30e8`); the four zero bytes of each immediate are a `LIDATA` run. So
/// the plane a flush leaves behind is zero and a frame's value lands on the flush's own
/// `0x400 + value`, the base the port's mixer indexes with (`mix.rs`'s `FLUSH_BASE`).
#[test]
fn the_flush_clears_the_accumulator_word_it_reads() {
    let image = segment_image();
    let claims: &[(usize, &[u8], &str)] = &[
        (0x2CDE, &[0x66, 0x2E, 0x89, 0x26, 0x0C, 0x21], "mov %esp,%cs:0x210c (saved)"),
        (0x2CE4, &[0xFA], "cli"),
        (0x2CE5, &[0x66, 0xBC, 0x00, 0x00, 0x00, 0x00], "mov $0x0,%esp"),
        (0x2CEB, &[0x66, 0x8B, 0x1C], "mov (%si),%ebx (read the accumulator)"),
        (0x2CEE, &[0x66, 0x03, 0xD9], "add %ecx,%ebx"),
        (0x2CF1, &[0x66, 0x89, 0x24], "mov %esp,(%si) (store the zero back)"),
        (0x30D0, &[0x66, 0xB9, 0x00, 0x00, 0x00, 0x00], "mov $0x0,%ecx"),
        (0x30E2, &[0x66, 0x8B, 0x1C], "mov (%si),%ebx (read the accumulator)"),
        (0x30E5, &[0x66, 0x03, 0xDA], "add %edx,%ebx"),
        (0x30E8, &[0x66, 0x89, 0x0C], "mov %ecx,(%si) (store the zero back)"),
    ];
    for (offset, bytes, meaning) in claims {
        assert_eq!(
            &image[*offset..*offset + bytes.len()],
            *bytes,
            "+0x{offset:04x}: {meaning}"
        );
    }
}

/// `_zspeedadjust` (`0x8cd`) and `_zspeedadjust2` (`0x8d1`, `crates/sr-assets/tests/omf_symbols.rs`)
/// start at zero in the object's image: both are `LIDATA` runs of `0x00`. The live words read 0 in
/// the reference's run too (`music-clock.md` §5), and the shell writes `_zspeedadjust2` only for
/// output modes 7 and 2 (`MAIN/U2.ASM:384-389`), which the reference's Sound Blaster (5) is not —
/// D46. The row timer's fractional adder (`+0x439f`) adds `_zspeedadjust`, so it adds nothing.
#[test]
fn the_speed_adjusters_are_zero_in_the_image() {
    let image = segment_image();
    assert_eq!(&image[0x08CD..0x08D5], &[0u8; 8], "_zspeedadjust and _zspeedadjust2");
    assert_eq!(
        &image[0x439F..0x43A9],
        &[0x2E, 0x8B, 0x16, 0xCD, 0x08, 0x2E, 0x01, 0x16, 0xCF, 0x08],
        "mov %cs:0x8cd,%dx; add %dx,%cs:0x8cf — the row timer adds _zspeedadjust"
    );
}
