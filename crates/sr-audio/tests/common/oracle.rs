//! The object's tables and arithmetic, written out from `MAIN/STMIK.300`'s segment image and
//! never computed by the port: the expected values in the tests come from here.
//! `tests/player.rs`'s `the_ports_tables_are_the_objects_own` checks every table against the
//! image.

/// The periods at `+0xa24`, semitone by semitone, four octaves above the S3M table.
pub const PERIODS: [u16; 12] = [
    27392, 25856, 24384, 23040, 21696, 20480, 19328, 18240, 17216, 16256, 15360, 14512,
];

/// The octave shifts at `+0x97c`: eight entries and the eight bytes after them, which octaves
/// 8…15 read because the index is unchecked.
pub const SHIFTS: [u8; 16] = [0, 1, 2, 3, 4, 5, 6, 7, 0, 5, 11, 17, 27, 32, 42, 47];

/// The vibrato sine at `+0xa50`: 64 signed words, ±255.
pub const SINE: [i16; 64] = [
    0, 24, 49, 74, 97, 120, 141, 161, 180, 197, 212, 224, 235, 244, 250, 253, 255, 253, 250, 244,
    235, 224, 212, 197, 180, 161, 141, 120, 97, 74, 49, 24, 0, -24, -49, -74, -97, -120, -141,
    -161, -180, -197, -212, -224, -235, -244, -250, -253, -255, -253, -250, -244, -235, -224, -212,
    -197, -180, -161, -141, -120, -97, -74, -49, -24,
];

/// `Q`'s volume addends at `+0x95c`, signed.
pub const Q_ADD: [i8; 16] = [0, -1, -2, -4, -8, -16, 0, 0, 0, 1, 2, 4, 8, 16, 0, 0];

/// `Q`'s volume multipliers at `+0x96c`, in sixteenths; zero sends a code to the addend.
pub const Q_MULTIPLY: [u8; 16] = [0, 0, 0, 0, 0, 0, 10, 8, 0, 0, 0, 0, 0, 0, 24, 32];

/// The volume scale at `+0x9c4`: `floor(level × 255/63)`.
pub const SCALE: [u8; 64] = [
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 85, 89, 93,
    97, 101, 105, 109, 113, 117, 121, 125, 129, 133, 137, 141, 145, 149, 153, 157, 161, 165, 170,
    174, 178, 182, 186, 190, 194, 198, 202, 206, 210, 214, 218, 222, 226, 230, 234, 238, 242, 246,
    250, 255,
];

/// `+0x58b9`'s base frequency, `0x00da7600` = 14,317,056.
pub const BASE_FREQ: u64 = 0x00da_7600;

/// `+0x5813`'s reference `c2spd`, the immediate `0x20ab`.
pub const C2SPD_REFERENCE: u32 = 8363;

/// `[0x8d7]`, the stereo divisor, the segment's initial 21111.
pub const STEREO_DIVISOR: u64 = 0x5277;

/// The output curve's flush base: a frame's value lands on entry `0x400 + value` (A14(a)).
pub const BASE: i32 = 0x400;

/// `+0x593a`: a note byte to a period.
pub fn note_period(note: u8) -> u16 {
    if note == 0xfe {
        return 0;
    }
    PERIODS[usize::from(note & 0x0f)] >> SHIFTS[usize::from(note >> 4)]
}

/// `+0x5813`: `period × 8363/c2spd`, with the word-overflow guard.
pub fn scaled(period: u16, c2spd: u16) -> u16 {
    let product = u32::from(period) * C2SPD_REFERENCE;
    if product >> 16 >= u32::from(c2spd) {
        return 0x7fff;
    }
    (product / u32::from(c2spd)).min(0x7fff) as u16
}

/// `+0x58b9`, `+0x58c9`…`+0x58d8`: the base frequency over the period, 16 bits of fraction,
/// then over the mixing rate.
pub fn step(period: u16, rate: u64) -> u32 {
    (((BASE_FREQ / u64::from(period)) << 16) / rate) as u32
}

/// One voice's value for one sample byte at one level: `(sample × scale) >> 8` (`+0x5553`)
/// less the same at the centre byte 128 (`+0x37b0`, `+0x30b4`).
pub fn value(sample: u8, level: u8) -> i32 {
    let scale = i32::from(SCALE[usize::from(level)]);
    ((i32::from(sample) * scale) >> 8) - ((128 * scale) >> 8)
}

/// The output curve for multiplier `mul` at index `0x400 + value`, as the card's byte widened
/// to `(byte − 128) × 256`. `+0x5565`…`+0x55c1`: `2 × ⌊(0x800 − K)/4⌋` zero bytes (the two
/// `shr $1,%cx`), `K = 0x8000/mul` ramp entries of `(k × (65536/K)) >> 8`, then a `2 × ⌊rem/2⌋`
/// `0xFF` tail. For mul 64: 768 zeros, the ramp at 768…1279, 0xFF from 1280.
pub fn curve(mul: u32, value: i32) -> i16 {
    let k = (0x8000 / mul) as usize;
    let step = 65536 / k;
    let zeros = ((0x800 - k) / 4) * 2;
    let tail = ((0x800 - (zeros + k)) / 2) * 2;
    let index = (BASE + value).clamp(0, 0x7ff) as usize;
    let byte = if index < zeros {
        0
    } else if index < zeros + k {
        ((((index - zeros) * step) >> 8) & 0xff) as u8
    } else if index < zeros + k + tail {
        0xff
    } else {
        0 // the span's last byte, left alone
    };
    (i16::from(byte) - 128) * 256
}

/// One tick of `+0x4005`'s vibrato from the position before it: the position after it and the
/// period the tick plays. The level is `value × depth >> 5` (both modules' flags word is 8, so
/// `[0x915]` is 0, `+0x4043`); a stale position (bit 7) reads 0 and restarts the phase.
pub fn vibrato_tick(position: u16, info: u8, org_period: u16) -> (u16, u16) {
    let depth = i32::from(info & 0x0f);
    let speed = u16::from(info >> 4);
    let stale = position & 0x80 != 0;
    let value = if stale {
        0
    } else {
        i32::from(SINE[usize::from(position & 0x7f) / 2])
    };
    let period = org_period.wrapping_add(((value * depth) >> 5) as u16);
    let from = if stale { 0 } else { position & 0xff };
    ((from + speed * 2) & 0x7e, period)
}

/// One tick of `+0x41ec`'s retrigger from the counter before it: the counter after it and
/// whether this tick fired. The interval is the low nibble; a fire zeroes the counter before
/// it is incremented.
pub fn retrigger_tick(count: u8, info: u8) -> (u8, bool) {
    let interval = info & 0x0f;
    if interval != 0 && interval <= count {
        return (1, true);
    }
    (count + 1, false)
}

/// The volume a retrigger leaves for volume code `code`: the multiplier if it has one, else
/// the addend, clamped to 0..=63.
pub fn retrigger_volume(volume: u8, code: u8) -> u8 {
    let code = usize::from(code);
    match Q_MULTIPLY[code] {
        0 => (i16::from(volume) + i16::from(Q_ADD[code])).clamp(0, 63) as u8,
        multiply => ((u16::from(volume) * u16::from(multiply)) >> 4).min(63) as u8,
    }
}
