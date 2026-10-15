//! The mixing engine: what turns the player's channel state into samples, and the drain
//! the audio device pulls from.
//!
//! **Where this file's facts come from.** Like `player.rs` and `effects.rs`, everything here
//! is read out of `upstream/SecondReality/MAIN/STMIK.300`'s own segment image: every offset
//! below is that object's segment offset, and `crates/sr-audio/tests/player.rs`'s claims
//! table asserts the load-bearing bytes at those offsets. Nothing here is read off Scream
//! Tracker documentation, and nothing here is read off a recording of this port.
//!
//! **What the object's mixer is.** `_zpollme` (`+0x261b`) calls `+0x51c5` once a copper frame
//! (`MAIN/COPPER.ASM:466`), and that routine's DMA loop (`+0x4ff8` falling into `+0x50d0`)
//! mixes a chunk of the buffer, runs one tick, mixes the next chunk, and so on. The per-voice
//! work has two entries: the chunk boundary at `+0x3765`, and the sample loop the table at
//! `+0x38c4` dispatches to (`+0x345d` and its siblings). The arithmetic, in the order the
//! object performs it:
//!
//! 1. **The volume table is built once from a 64-byte scale table.** `+0x551c` walks 64
//!    levels × 256 sample values and stores `(sample × scale[level]) >> 8` (`+0x554c`…`+0x555d`)
//!    — `mul %bx` then `mov %ah` — where `scale` is the object's own table at `+0x9c4`,
//!    `floor(level × 255/63)`: level 0 is silence and level 63 is `(sample × 255) >> 8`. The
//!    table lives in the segment `+0x29a9` allocates (0x5000 bytes:
//!    0x4000 for the table, 0x1000 for the accumulators the flush reads, zeroed at `+0x2943`).
//! 2. **A voice's volume is `m_vol = (avol × master) >> 8`** (`+0x5925`: `mov 0x1a(%si),%al;
//!    mulw 0x913; mov %ah,0x41(%si)`), where `[0x913]` is `min(global volume byte, 64) × 4`
//!    (`+0x55c4`…`+0x55e1`). Both shipped modules' global-volume byte is **64**
//!    (`docs/fidelity/music.md` §5), so the clamp is not reached and the master is 256: `m_vol`
//!    is the channel's own volume and the scale is `level × 4`. (The trace's `_np_mastervol`
//!    reads 192 — the same 64-through-the-clamp arithmetic, whatever wrote the published word;
//!    `min(192, 64) × 4` is 256 too, so the voice volume is the same either way.)
//! 3. **The step through the sample is 16.16 fixed point.** `+0x58b9` divides the object's own
//!    `0x00da7600` by the period, `+0x58c9` divides *that* by the mixing rate `[0x8d9]` —
//!    which `+0x53a0`…`+0x53a9` picks as `_zmixspeed` in mono and `[0x8d7]` in stereo, and
//!    the quotient and remainder become `m_speed`'s whole and fractional halves (`+0x50`).
//!    A period of 0 returns with `m_speed = 0` (`+0x5876`), which is how a note off silences
//!    a voice. `+0x58e3`…`+0x5916` also computes `m_safe`, the frames the voice may mix
//!    before its end must be looked at — a chunking device, not a sample value.
//! 4. **One sample byte per frame, read at the position** (`+0x346e`: `mov %es:(%si),%bl`
//!    with `es` = the sample's segment, `si` = `m_pos`), looked up in the volume table at
//!    `(m_vol << 8) | sample` (`+0x3473`: `mov (%bx),%al`, `bh` still holding `m_vol`), and
//!    accumulated into a 16-bit per-frame word. There is no interpolation in this path, and
//!    neither module can ask for it: the interpolating dispatch entries are `S1x`-gated, the
//!    byte is `[0x919]`, and both files leave it 0 (`effects.rs`'s note on `G`).
//! 5. **The position advances by `m_speed` per frame** (`+0x346c`/`+0x3471`/`+0x347e`:
//!    `add %bp,%cx` for the fraction with `adc %dx,%si` for the carry), and when the position
//!    reaches `m_end` the voice either rewraps by adding `m_loop - m_end` (`+0x3840`…`+0x385a`)
//!    — the overshoot and the loop's phase survive, which is what makes a loop clickless — or,
//!    for a one-shot (`m_loop` = 0xffff), stops dead: `m_speed = 0` (`+0x3845`…`+0x384d`).
//!    The bounds come from the instrument header at the note start (`+0x4872`…`+0x48cb`).
//! 6. **The sums become output bytes through a table built for the run.** `+0x2c89` adds the
//!    object's DC correction (`+0x30b4`: `0x400 - [0x2250]`, the sum of the centre values
//!    `+0x37b0` read at `(m_vol << 8) | 0x80`) to each 16-bit accumulator and indexes the
//!    table `+0x551c`'s second half wrote at `cs:[0x6c]`.
//!
//! **What this file does with each of those.**
//!
//! Points 1–5 are reproduced exactly: [`scale`], [`voice_value`], [`centre`], [`step`],
//! `Mixer::frame` and the rewrap. Point 6 is where the port stops. The object's output table
//! is a per-run gain curve built from `_np_mastermul` (the module header's master-volume byte,
//! offset 0x33, which the reference runs at 0xB0 → 2240, `music.md` §10) and at that value it
//! maps every accumulator below *the centre plus 512* to zero; the port converts the voice
//! sums — each voice less its own centre, which is exactly the quantity the object forms at
//! `+0x2cbe`/`+0x30b4` — straight to `i16` with the object's own *linear* case, the table the
//! same builder produces at the minimum its own clamp allows. That is the whole of the
//! approximation and it is registered in `docs/fidelity/APPROXIMATIONS.md`.
//!
//! The DMA plumbing is not reproduced either, and could not be: there is no 8-bit DAC, no
//! `_zbufsize` ring for the hardware to walk, and no EMS page frame. What replaces it is
//! [`Mixer`]'s bounded queue of the same samples with the same meaning, drained by
//! [`Player::take_samples`](crate::Player::take_samples).

use crate::effects::Channel;
use crate::module::Module;
use crate::player::{MixRate, PlayerOptions};
use crate::CHANNELS;

/// The volume scale table the object carries at `+0x9c4`, byte for byte: 64 entries, the
/// object's own values, whose formula is `floor(level × 255/63)` for every level — verified
/// against the image for all 64.
///
/// Reading the table rather than the formula is the point: `level × 4` is what it looks like
/// from the bottom (levels 0…20 are exactly that) and it stops being true at level 21, where
/// `88` would be four a level and the object holds `85`. The top entry is 255, so a sample of
/// 255 at level 63 maps to `(255 × 255) >> 8 = 254` and not 255 — a hair below full scale, and
/// a hair the object's arithmetic keeps.
pub const VOLUME_SCALE: [u8; 64] = [
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 85, 89, 93,
    97, 101, 105, 109, 113, 117, 121, 125, 129, 133, 137, 141, 145, 149, 153, 157, 161, 165, 170,
    174, 178, 182, 186, 190, 194, 198, 202, 206, 210, 214, 218, 222, 226, 230, 234, 238, 242, 246,
    250, 255,
];

/// `+0x58b9`'s base frequency as the 16.16 value the object divides by the period:
/// `mov $0xda,%dx; mov $0x7600,%ax; div period`, i.e. `0x00da7600` = 14,317,056.
///
/// It is 400 short of `8363 × 1712`, so C-4 at the format's reference `c2spd` is stepped at
/// 8362 (`effects.rs`'s `period_step` measures the same constant from the other side, §9.11).
const BASE_FREQ: u32 = 0x00da_7600;

/// `[0x8d7]`, the divisor `+0x53a0`…`+0x53a9` picks in stereo mode, holding the segment's own
/// initial data: **21111**.
///
/// It is read on every path the reference takes and written on none of them — the only store
/// to it is `+0x228f`, inside `_zoutputmode == 2`'s branch, which `MAIN/U2.ASM:734` labels
/// **Gravis** (the value it writes, 0x96bb = 38587, is 44100 × 7/8 — a GUS rate) and which the
/// reference's SB Pro run never reaches (`_zoutputmode` is 5, `MAIN/U2.ASM:759`). The shell doubles `_zmixspeed` to 40000
/// for that mode and leaves this word alone, so the reference's stereo voices step
/// `14317056/period/21111` per frame while the frame rate is 20000 a second: **the reference's
/// stereo playback runs **5.26 % flat** — one semitone is 5.95 % — because `21111/20000` is the
/// reciprocal of the divisor's being 5.56 % too large, and a mono run is exact. The port keeps
/// the object's word, so it is flat in the same proportion, and `music.md` §10 records it.
const STEREO_DIVISOR: u32 = 0x5277;

/// The object's one-shot marker for `m_loop` (`+0x48cb`): a voice whose loop word is this
/// stops at `m_end` rather than rewrapping.
const NO_LOOP: u32 = 0xffff_ffff;

/// The bytes in the object's first mixed chunk: the DMA loop's countdown starts at 1
/// (`+0x50ab`) and is rounded up to a multiple of 64 before the mixer sees it (`+0x50e6`).
const FIRST_CHUNK: usize = 64;

/// The object's table base offset index: silence is the 1024th of the curve's 2048 entries and
/// the flush's `0x400 - centre_sum` puts a frame's own `value` on top of it, marked
/// (`0x501 = 0x400 + 0x101`) or not.
const INDEX_BASE_MARKED: i32 = 0x501;

/// The largest position the object can hold: `m_pos`'s whole bytes are a 16-bit word
/// (`+0x378a`: `mov 0x44(%bp),%si`), so nothing past this is reachable, and the bounds the
/// note start writes are clamped to it here for the same reason.
const MAX_POS: u32 = 0xffff;

/// One level's scale, clamped the way the table is: `m_vol` comes from
/// `(avol × master) >> 8` with `avol` ≤ 63 and `master` ≤ 256, so it cannot exceed 63, and a
/// level out of range must not panic.
pub fn scale(level: u8) -> u8 {
    VOLUME_SCALE[usize::from(level.min(63))]
}

/// The volume table value for one sample byte at one volume level: `+0x5553`'s `mul` and
/// `+0x5555`'s `mov %ah`, so `(sample × scale[level]) >> 8`, truncated.
///
/// The sample byte is the S3M file's own unsigned value — 128 is the centre, which is what
/// [`centre`] is for.
pub fn voice_value(sample: u8, level: u8) -> u16 {
    (u16::from(sample) * u16::from(scale(level))) >> 8
}

/// The value a voice at `level` contributes when its sample is at the centre: the byte
/// `+0x37ae`…`+0x37b0` reads (`mov $0x80,%bl; mov %fs:(%bx),%al`, `bh` still holding `m_vol`)
/// and `+0x37d0`/`+0x37df` accumulate before the flush subtracts the sum (`+0x30b4`).
pub fn centre(level: u8) -> u16 {
    voice_value(0x80, level)
}

/// The position's step per output frame, in 16.16: `m_speed` (`+0x50`).
///
/// `+0x58b9`'s division is the object's own: `0x00da7600 / period` as a 32-bit integer (the
/// quotient's halves go into `addherz`, `+0x58c3`/`+0x58c6`, whose split path below 218 is the
/// same value computed in two 16-bit steps), and then `+0x58c9`…`+0x58d8` divides *that*
/// 32-bit value by the mixing rate — `divw` twice, the quotient into the step's high word and
/// the remainder's next 16 bits into its low word. So the step is `(14317056/period) ×
/// 65536 / rate`, and the inner division's truncation is the object's, not a rounding the
/// port chose. A period of 0 returns 0 without dividing (`+0x5876`).
pub fn step(period: u16, mix_rate: MixRate, stereo: bool) -> u32 {
    if period == 0 {
        return 0;
    }
    let rate = if stereo {
        STEREO_DIVISOR
    } else {
        mix_rate.samples_per_second()
    };
    let rate_32 = BASE_FREQ / u32::from(period);
    (((u64::from(rate_32)) << 16) / u64::from(rate)) as u32
}

/// Which side of a stereo mix a channel's samples go to, from the module's own 32-byte
/// channel table (`Module::channel_pan`, the file's offset 0x40).
///
/// The S3M table's values are pan positions: 0…7 are the left ones, 8…15 the right, and 255
/// means the channel is not used. Both modules carry `0, 8, 1, 9, 2, 10, 3, 11` for channels
/// 0…7 and `255` for the rest (`module.rs`'s `channel_pan`), so the eight channels the music
/// uses are hard-panned L, R, L, R, L, R, L, R. The object reads the same byte as `amixtype`
/// (`+0x1b`, at `+0x37b5`) and sends it to one of two accumulators.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Side {
    /// Both sides: a mono mix, or a channel the module leaves unpositioned.
    Centre,
    /// The left accumulator.
    Left,
    /// The right accumulator.
    Right,
}

/// The side `pan` plays on, per [`Side`]'s reading of the module's pan table.
pub fn side_of(pan: u8) -> Side {
    match pan {
        255 => Side::Centre,
        pan if pan & 8 == 0 => Side::Left,
        _ => Side::Right,
    }
}

// -------------------------------------------------------------------------------------------
// The voice
// -------------------------------------------------------------------------------------------

/// One mixer voice: the object's `m_*` words, kept between polls.
///
/// The object holds them in the same 0x60-byte channel struct the effects write (`_zchn` at
/// `+0x0da0`, whose mixer half starts at `m_vol`/`+0x41`). The port keeps them here because
/// they are the mixer's: which sample the voice plays, its bounds, and the two cached words
/// `+0x582b` and `+0x5925` compute.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Voice {
    /// The instrument whose sample the voice plays — `m_base`'s stand-in, `None` for a voice
    /// that has never been given one. The object's `m_base` is zero until a note start writes
    /// it (`+0x4886`, `+0x488d`) and `m_speed` is 0 until then, so the two mean the same.
    instrument: Option<u8>,
    /// `m_end` (`+0x48`), in sample bytes: where the voice stops or rewraps. The note start
    /// puts the header's **`lend`** here.
    end: u32,
    /// `m_loop` (`+0x4c`), in sample bytes: the header's **`lbeg`** — where a rewrapped voice
    /// continues — or [`NO_LOOP`].
    loop_start: u32,
    /// `m_speed`, 16.16 sample bytes an output frame, cached against `cached_period`.
    speed: u32,
    /// The period [`Voice::speed`] was computed for. The object recomputes `m_speed` at every
    /// write to the channel's period (`+0x582b`, called from `D`, `E`, `F`, `G`, `H`, `J` and
    /// the note start), so a changed period is exactly when the port must recompute too.
    cached_period: u16,
    /// `m_vol` (`+0x41`), cached against the pair [`Voice::cached_volume`].
    volume: u8,
    /// The `(avol, master)` pair [`Voice::volume`] was computed from.
    cached_volume: Option<(u8, u16)>,
    /// Set when the voice ran into `m_end` with no loop (`+0x3845`: `m_speed = 0`). Only a
    /// note start clears it, which is the object's state too: nothing else writes `m_loop`.
    stopped: bool,
}

/// The object's `+0x5925`: `m_vol = (avol × master) >> 8`, recomputed when either input moves.
impl Voice {
    fn refresh_volume(&mut self, avol: u8, master: u16) {
        if self.cached_volume == Some((avol, master)) {
            return;
        }
        self.cached_volume = Some((avol, master));
        self.volume = ((u16::from(avol) * master) >> 8) as u8;
    }

    /// `+0x582b`'s tail: the step for the current period, or 0 while the voice is stopped.
    fn refresh_speed(&mut self, period: u16, options: &PlayerOptions) {
        if self.cached_period == period {
            return;
        }
        self.cached_period = period;
        if self.stopped {
            return;
        }
        self.speed = step(period, options.mix_rate, options.stereo);
    }
}

// -------------------------------------------------------------------------------------------
// The ring
// -------------------------------------------------------------------------------------------

/// The samples the mixer has produced that the device has not taken, in a fixed-size buffer.
///
/// This is the port's stand-in for the object's DMA buffer. The original's buffer is the same
/// kind of thing seen from the other end: `MAIN/STMIK.H:100-103` says the `np_*` words describe
/// the mixing engine, "which is a bit ahead of the actual music", so the position the parts
/// read belongs to the engine and the bytes the card plays trail it by whatever the buffer
/// still holds. The port's parts read the same engine position, and this ring holds what the
/// device has not drained.
///
/// **The overflow policy is to drop the oldest samples.** The engine may not wait: a poll is
/// one 70.086 Hz frame of the demo's timeline (`crates/sr-engine/src/tick.rs`), so a stall in
/// the mixer is a wrong frame everywhere else. A device that has fallen further behind than
/// the ring is long is a device whose backlog cannot be played, and the samples worth keeping
/// are the ones nearest the engine's position — so the oldest go, and the count is exposed as
/// [`Mixer::dropped`] so a caller can see it happen.
#[derive(Clone, Debug)]
struct Ring {
    buffer: Vec<i16>,
    read: usize,
    len: usize,
}

impl Ring {
    /// `capacity` samples — one i16 each, so a stereo frame is two of them.
    fn new(capacity: usize) -> Ring {
        assert!(capacity > 0, "the ring needs somewhere to put a sample");
        Ring {
            buffer: vec![0; capacity],
            read: 0,
            len: 0,
        }
    }

    fn len(&self) -> usize {
        self.len
    }

    fn reset(&mut self) {
        self.read = 0;
        self.len = 0;
        self.buffer.iter_mut().for_each(|sample| *sample = 0);
    }

    /// Append one sample, evicting the oldest when the ring is full. True when it evicted.
    fn push(&mut self, sample: i16) -> bool {
        let capacity = self.buffer.len();
        if self.len == capacity {
            self.buffer[self.read] = sample;
            self.read = (self.read + 1) % capacity;
            return true;
        }
        self.buffer[(self.read + self.len) % capacity] = sample;
        self.len += 1;
        false
    }

    /// Copy out up to `out.len()` samples, oldest first, as the device would take them.
    fn take(&mut self, out: &mut [i16]) -> usize {
        let capacity = self.buffer.len();
        let count = out.len().min(self.len);
        for slot in out.iter_mut().take(count) {
            *slot = self.buffer[self.read];
            self.read = (self.read + 1) % capacity;
        }
        self.len -= count;
        count
    }
}

// -------------------------------------------------------------------------------------------
// The mixer
// -------------------------------------------------------------------------------------------

/// The ring's capacity in samples: 8192 i16, about fourteen polls' worth at the reference's
/// 570 samples a poll, or 0.2 s of stereo output. The size is the port's choice and not the
/// object's — the original's `_zbufsize` is 4096 **bytes**, which is 2048 stereo frames — and
/// it is picked to be a good deal larger than a poll so that a device draining once a frame
/// never sees the overflow policy at all.
pub const RING_SAMPLES: usize = 8192;

/// The port's mixing engine: one `Voice` per channel and the ring the device drains.
///
/// The mixer owns no clock. It is driven by [`Player::poll`](crate::Player::poll), which is the
/// engine's tick — one call per copper frame, exactly as `MAIN/COPPER.ASM:466` calls `zpollme`
/// once per copper frame while `_zpollmix` is set.
pub struct Mixer {
    voices: [Voice; CHANNELS],
    ring: Ring,
    /// The output curve, rebuilt when the module's master-volume byte or the output mode
    /// changes — which is when the object's header watch rebuilds it (`+0x53af`, `+0x5420`).
    curve: Curve,
    /// How many of the next output samples are read against the object's *unmarked*
    /// accumulator planes.
    ///
    /// `+0x2943` zeroes the planes and every flush re-marks the words it read with `0x0101`
    /// (`+0x2ce5`/`+0x2cf1`, `+0x30d0`/`+0x30e8`), so a frame's index is
    /// `0x400 − centre sum + accumulator` while the planes are the zeroed ones and
    /// `0x501 − centre sum + accumulator` — the marker's `0x101` — afterwards. Only the first
    /// chunk after the planes are zeroed is unmarked, and that chunk is 64 bytes: the DMA
    /// loop's countdown starts at 1 (`+0x50ab`) and is rounded up to a multiple of 64 before
    /// mixing (`+0x50e6`'s `add $0x41,%ax; and $0xffc0,%ax`).
    unmarked: usize,
    /// The samples the ring's overflow policy evicted.
    dropped: u64,
    /// A stereo frame the last budget split: the left sample went out, this is its right.
    /// The object has the same seam — its DMA is a stream of bytes, and a chunk boundary can
    /// fall between the two bytes of a frame.
    held_right: Option<i16>,
    /// The last frame mixed, for a reader and for the tests.
    last_frame: (i16, i16),
}

impl Default for Mixer {
    fn default() -> Mixer {
        Mixer::new()
    }
}

impl Mixer {
    /// A mixer with silent voices and an empty ring.
    pub fn new() -> Mixer {
        Mixer::new_with_capacity(RING_SAMPLES)
    }

    /// A mixer whose ring holds `capacity` samples — the seam [`Mixer::new`] does not need,
    /// and `tests/mix.rs` uses to reach the overflow policy in a few polls.
    pub fn new_with_capacity(capacity: usize) -> Mixer {
        Mixer {
            voices: [Voice::default(); CHANNELS],
            ring: Ring::new(capacity),
            // The reference's own: the modules' master volume byte is 0xB0 and its output mode
            // is SB Pro's stereo, and the curve is rebuilt from the module at the first mix.
            curve: Curve::new(0xb0, true),
            unmarked: FIRST_CHUNK,
            dropped: 0,
            held_right: None,
            last_frame: (0, 0),
        }
    }

    /// The whole samples the overflow policy has dropped. Zero on any run whose device keeps
    /// up, which is every run that drains a poll's worth between polls.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// The samples waiting for the device.
    pub fn pending(&self) -> usize {
        self.ring.len()
    }

    /// The last frame the mixer produced — what `frame` is asserted against.
    pub fn last_frame(&self) -> (i16, i16) {
        self.last_frame
    }

    /// Every voice silent and the ring empty: what `_zplaysong`'s channel clear does to the
    /// mixer's half of the channel struct (`+0x5614`, called at `+0x5716`, whose
    /// `+0x5641`/`+0x5644` zero `m_base`).
    pub fn reset(&mut self) {
        self.voices = [Voice::default(); CHANNELS];
        self.ring.reset();
        self.dropped = 0;
        self.held_right = None;
        self.last_frame = (0, 0);
        // The curve and the plane marker are the *card's*, not the song's: `_zplaysong` clears
        // the channels (`+0x5614`) and touches neither the table nor the accumulator planes.
    }

    /// Give a channel a fresh sample: the note start's half of the mixer's state.
    ///
    /// `+0x4872`…`+0x48cb` is where the object writes `m_base`, `m_end` and `m_loop` from the
    /// instrument header it just loaded, and the two bounds are the header's own two loop
    /// fields: **`m_loop` takes `lbeg`** (`+0x4890` reads `es:0x18` into `ax` — `lend`, the
    /// loader's layout `STARTEMS.C:28-48` — `+0x48a1` reads `es:0x14` — `lbeg` — into `dx`,
    /// and `+0x48a6` stores *that* into `0x4c(%si)`) and **`m_end` takes `lend`** (`+0x48b9`).
    /// Reading those two the other way round gets the *bounds* right by symmetry and the
    /// *rule* wrong, which is what this function did before the review: the test at `+0x489c`
    /// is `cmp $0x0,%ax; je` on `ax` = `lend`, so what sends a sample down the object's
    /// one-shot path is a **zero loop end**, not a loop that begins at 0.
    ///
    /// Combined with the loader's own rule — `checkins` clears the loop flag when
    /// `lend <= lbeg` (`STARTEMS.C:145`) — a sample loops exactly when its flag is set and its
    /// loop end is past its loop begin. That is not a formality: **five of MUSIC0's twelve
    /// looping samples and one of MUSIC1's nine have `lbeg` 0 and `lend` = the length** — a
    /// full-sample loop — and the object wraps them at the sample's end back to 0, where the
    /// earlier reading stopped them once.
    ///
    /// The object's `m_end` is not always `lend`: when `lend` is at or below 500 it is reduced
    /// by whole loop lengths until it is small (`+0x48ab`…`+0x48b7`), so that the mixer's
    /// over-read stays inside the 512 bytes `checkins` copies in behind the loop
    /// (`STARTEMS.C:157-161`). That value is congruent to `lend` modulo the loop's length, and
    /// the rewrap's displacement `m_loop - m_end` is a whole number of loop lengths, so the
    /// phase is the same either way — **and on these two modules the reduction never runs at
    /// all**: every looping sample has a loop end well past 500 (`tests/mix.rs` prints the
    /// census). The port keeps `lend` itself and reads no padding.
    pub fn start_note(&mut self, channel: usize, module: &Module, instrument: u8) {
        let Some(voice) = self.voices.get_mut(channel) else {
            return;
        };
        let Some(header) = module
            .instrument(usize::from(instrument.wrapping_sub(1)))
            .filter(|header| header.is_pcm())
        else {
            *voice = Voice {
                stopped: true,
                ..Voice::default()
            };
            return;
        };
        voice.instrument = Some(instrument);
        voice.stopped = false;
        voice.speed = 0;
        voice.cached_period = 0;
        if header.loops() && header.loop_end > header.loop_begin {
            voice.loop_start = header.loop_begin.min(MAX_POS);
            voice.end = header.loop_end.min(MAX_POS);
        } else {
            voice.loop_start = NO_LOOP;
            // `+0x48be`: `m_end` = the length plus 0x20, inside the 512 bytes of 0x80 `checkins`
            // writes behind a one-shot (`STARTEMS.C:167-172`) — so the object's slack holds no
            // sample data and the port's `end` = the length is the same signal, not a shorter
            // one.
            voice.end = header.length.min(MAX_POS);
        }
    }

    /// Mix `samples` output samples from the player's channel state into the ring.
    ///
    /// `samples` counts the object's *bytes*: one i16 a frame in mono and two in stereo, so a
    /// stereo budget is twice the frames. An odd budget splits a frame across the call, which
    /// is what a byte-stream DMA does at a buffer boundary; the held sample goes out first
    /// next time.
    ///
    /// `channels` is the player's own channel state, which the mixer *advances*: `m_pos` and
    /// `m_poslow` are the fields the note start writes (`+0x491e`, `+0x492a`), so a later note
    /// inherits the position the mixer left.
    #[allow(clippy::too_many_arguments)]
    pub fn mix(
        &mut self,
        module: &Module,
        channels: &mut [Channel; CHANNELS],
        options: &PlayerOptions,
        global_volume: u8,
        silenced: bool,
        samples: usize,
    ) {
        // `+0x55c4`: `[0x913] = min(global volume byte, 64) × 4`.
        let master = u16::from(global_volume.min(64)) * 4;
        // The header watch rebuilds the curve whenever the byte it watches moves
        // (`+0x5397`…`+0x53af`); the port rebuilds it when the multiplier it derives moves.
        let mul = multiplier(module.master_volume(), options.stereo);
        if self.curve.mul() != mul {
            self.curve = Curve::build(mul);
        }
        let width = if options.stereo { 2 } else { 1 };
        let mut budget = samples;

        if let Some(right) = self.held_right.take() {
            if budget == 0 {
                self.held_right = Some(right);
                return;
            }
            let overflow = self.ring.push(right);
            self.dropped += u64::from(overflow);
            budget -= 1;
        }

        while budget >= width {
            let (left, right) = self.frame(module, channels, options, master, silenced);
            let overflow = self.ring.push(left);
            self.dropped += u64::from(overflow);
            if options.stereo {
                let overflow = self.ring.push(right);
                self.dropped += u64::from(overflow);
            }
            budget -= width;
        }

        if budget == 1 {
            let (left, right) = self.frame(module, channels, options, master, silenced);
            let overflow = self.ring.push(left);
            self.dropped += u64::from(overflow);
            self.held_right = Some(right);
        }
    }

    /// The device's drain: up to `out.len()` samples, oldest first, and nothing else — the
    /// engine's position is the player's and this does not move it.
    pub fn take(&mut self, out: &mut [i16]) -> usize {
        self.ring.take(out)
    }

    /// One output frame: every voice's sample read, scaled, accumulated and converted.
    ///
    /// The object's chunking is not reproduced as such. It mixes as many frames as the DMA
    /// buffer has room for and as `m_safe` allows, then looks at the ends (`+0x3826`…`+0x387d`);
    /// the port mixes a frame at a time over the same arithmetic with the end test per frame,
    /// which produces the same sample values: the rewrap's displacement is congruent to the
    /// loop's length either way, so the *phase* — and therefore every sample's value — is the
    /// same, and a one-shot stops at its end in both.
    fn frame(
        &mut self,
        module: &Module,
        channels: &mut [Channel; CHANNELS],
        options: &PlayerOptions,
        master: u16,
        silenced: bool,
    ) -> (i16, i16) {
        let pan = module.channel_pan();
        let mut left: i32 = 0;
        let mut right: i32 = 0;

        for index in 0..CHANNELS {
            let channel = &mut channels[index];
            let voice = &mut self.voices[index];
            // `+0x5925` runs for every channel the mixer walks, and `_zshutup` is modelled as
            // zeroing the volumes it reads.
            voice.refresh_volume(if silenced { 0 } else { channel.volume }, master);
            voice.refresh_speed(channel.period, options);
            let Some(contribution) = voice_contribution(module, voice, channel) else {
                continue;
            };
            // A mono mix has one accumulator and no pan: the object's mono path walks all
            // sixteen channels into the single buffer (`+0x39cd`), where the stereo path
            // sends each to one of two by its `amixtype`.
            let side = if options.stereo {
                side_of(pan[index])
            } else {
                Side::Centre
            };
            match side {
                Side::Left => left += contribution,
                Side::Right => right += contribution,
                Side::Centre => {
                    left += contribution;
                    right += contribution;
                }
            }
            advance(voice, channel);
        }

        // `+0x30b4`/`+0x2cb4`: the accumulator less the centre sums, indexed into the curve at
        // the base the planes' marker sets.
        let left = self.emit(left);
        // A mono mix writes one byte a frame, so only the left accumulator is an output sample;
        // the right's value is still converted for `last_frame` but consumes no marker unit.
        let right = if options.stereo {
            self.emit(right)
        } else {
            self.peek(right)
        };
        self.last_frame = (left, right);
        (left, right)
    }

    /// The base the planes are on: `0x400` while the first chunk's zeroed accumulators are
    /// still unmarked, `0x501` once a flush has written the `0x0101` marker into them.
    fn base(&self) -> i32 {
        if self.unmarked > 0 {
            INDEX_BASE_MARKED - 0x101
        } else {
            INDEX_BASE_MARKED
        }
    }

    /// One output sample: the curve's entry for a frame's value, and the marker's consumption —
    /// every *byte* the object writes consumes one of the first chunk's 64, so a stereo frame
    /// consumes two and a mono frame one.
    fn emit(&mut self, value: i32) -> i16 {
        let base = self.base();
        if self.unmarked > 0 {
            self.unmarked -= 1;
        }
        self.curve.sample(base + value)
    }

    /// The same conversion without consuming a marker unit.
    fn peek(&self, value: i32) -> i16 {
        self.curve.sample(self.base() + value)
    }
}

/// One voice's contribution to the frame: its sample's table value less its own centre, or
/// `None` when the voice has nothing to say.
///
/// The object forms the same difference the other way round — the sums of the table values in
/// the accumulators and the sum of the centres in `[0x2250]`/`[0x2252]`/`[0x2254]`, subtracted
/// at `+0x30b4` — and the two agree because the sum of the differences is the difference of the
/// sums. The *set* of voices agrees too: the object's voice routine tests `m_speed` at `+0x3783`
/// and jumps to its return at `+0x3786` **before** the centre is read at `+0x37b0`, and
/// `+0x3791` gates on `m_pos == 0xffff` the same way, so a voice that has stopped is in neither
/// sum. The port's three conditions are those two tests plus the sample the voice must have.
fn voice_contribution(module: &Module, voice: &Voice, channel: &Channel) -> Option<i32> {
    let instrument = voice.instrument?;
    if voice.stopped || voice.speed == 0 {
        return None;
    }
    let bytes = module.sample_bytes(usize::from(instrument.wrapping_sub(1)))?;
    // Past the sample's end the object reads the padding `checkins` wrote behind it
    // (`STARTEMS.C:129-177`); the port has no padding, so a position past the last byte reads
    // the last byte. Reachable only through an `O` offset longer than the sample.
    let at = usize::try_from(channel.position).ok()?;
    let sample = bytes.get(at).or_else(|| bytes.last()).copied()?;
    Some(i32::from(voice_value(sample, voice.volume)) - i32::from(centre(voice.volume)))
}

/// Advance one voice's position by `m_speed` and honour its end.
///
/// The step is the 16.16 pair at `+0x50`: the fraction is `m_poslow` and the whole bytes go
/// into `m_pos` with the carry (`+0x346c`…`+0x347e`). The end is `+0x3840`…`+0x385a`: a voice
/// whose frame crossed `m_end` rewraps by `m_loop - m_end`, keeping the overshoot, and a
/// one-shot stops with `m_speed = 0` (`+0x3845`…`+0x384d`).
fn advance(voice: &mut Voice, channel: &mut Channel) {
    if voice.speed == 0 {
        return;
    }
    let (low, carry) = channel
        .position_low
        .overflowing_add((voice.speed & 0xffff) as u16);
    channel.position_low = low;
    channel.position = channel
        .position
        .wrapping_add((voice.speed >> 16) + u32::from(carry));

    if channel.position < voice.end {
        return;
    }
    if voice.loop_start == NO_LOOP {
        channel.position = voice.end;
        voice.speed = 0;
        voice.stopped = true;
        return;
    }
    let overshoot = channel.position - voice.end;
    channel.position = voice.loop_start + overshoot;
}

// -------------------------------------------------------------------------------------------
// The output curve
// -------------------------------------------------------------------------------------------

/// The object's output curve: the table `+0x551c`'s second half builds at `cs:[0x6c]` and the
/// flush at `+0x2c89` applies.
///
/// **The multiplier.** The header watch (`+0x535e`…`+0x5391`) turns the module's master-volume
/// byte into the byte at `[0x916]`, whose low seven bits `+0x5569` reads. See [`multiplier`].
///
/// **The table** (`+0x5565`…`+0x55c1`): `mul` is floored at `0x10` (`+0x5570`…`+0x5575`),
/// `K = 0x8000 / mul` (`+0x557d`), and the buffer at `cs:[0x6c]` gets `0x800 - K` zero bytes
/// (`+0x559d`/`+0x55a1`) followed by `K` entries of a ramp whose step is `65536 / K`
/// (`+0x5588`) stored as the **high** byte of a 16-bit accumulator (`+0x55a7`: `mov %ah,(%di)`).
/// So entry `0x800 - K + k` is `(k × (65536/K)) >> 8`.
///
/// At the reference's own byte — the modules' master volume is 176, `0xB0` — the multiplier is
/// **64**, `K` is **512**, and the curve is 1536 zeros followed by 512 ramp entries: `value × 2`
/// is the entry's index into 256 output levels. That is what the note's §10.3 is about.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curve {
    table: Vec<u8>,
    mul: u8,
}

/// The multiplier byte the curve is built from, from the module header's master-volume byte
/// (`Module::master_volume`, the file's offset 0x33) — the header watch's arithmetic at
/// `+0x535e`…`+0x5391` follows by the builder's own floor at `+0x5575`.
///
/// Bit 7 of the byte is the *stereo* request. On a stereo output (the reference's mode 5,
/// `MAIN/U2.ASM:759`) the byte becomes `min((v × 11) >> 3 − 2, 0x7f) | 0x80`, and the modules'
/// 0xB0 gives `((48 × 11) >> 3) − 2 = 64` — the `mul` of 64 `music-clock.md` §5 measures
/// through the published 2240. The subtraction is a 16-bit `sub $0x2,%ax` and its underflow
/// meets `cmp $0x7f,%ax; jbe`, which *fails* for a wrapped value and resets to 0x7f — so a
/// byte of `0x80` (volume 0, stereo asked for) gives a multiplier of 127, not 0. Without
/// stereo the byte is `v & 0x7f` and the curve is the linear map.
pub fn multiplier(master_volume: u8, stereo: bool) -> u8 {
    let volume = master_volume & 0x7f;
    let byte = if stereo && master_volume & 0x80 != 0 {
        let scaled = (u16::from(volume) * 11) >> 3;
        let clamped = match scaled.wrapping_sub(2) {
            value if value > 0x7f => 0x7f,
            value => value as u8,
        };
        clamped | 0x80
    } else {
        volume
    };
    (byte & 0x7f).max(0x10)
}

impl Curve {
    /// The curve for a module's master-volume byte and an output mode, the way the object
    /// builds one at `+0x53af`/`+0x5420` and at the sound init.
    pub fn new(master_volume: u8, stereo: bool) -> Curve {
        Curve::build(multiplier(master_volume, stereo))
    }

    /// The curve for a multiplier, without the header watch in front of it.
    pub fn build(mul: u8) -> Curve {
        let mul = mul.max(0x10);
        let ramp = 0x8000 / usize::from(mul);
        let step = 65536 / ramp;
        let mut table = vec![0u8; 0x800];
        for entry in 0..ramp {
            table[0x800 - ramp + entry] = ((entry * step) >> 8) as u8;
        }
        Curve { table, mul }
    }

    /// The multiplier the table was built with — what [`Mixer::mix`] re-checks so that a
    /// module or an output mode change rebuilds the curve the way `+0x53af` does.
    pub fn mul(&self) -> u8 {
        self.mul
    }

    /// One entry, and the widening the DMA's byte gets when it becomes a sample: the card
    /// plays unsigned 8-bit, so 128 is its centre and the port's `i16` is `(byte − 128) × 256`.
    ///
    /// The index is clamped to the table's own 2048 entries. The object does not clamp: its
    /// index is a 16-bit `bx` added to the base and read out of its own code segment, so a
    /// signal further from silence than the table is wide reads whatever the linker put after
    /// it — the note tables at `+0x97c` are 0x10E bytes past the table's end at `cs:0x86e`. The port has no
    /// such neighbours to read and says so rather than inventing a value.
    pub fn sample(&self, index: i32) -> i16 {
        let entry = index.clamp(0, 0x7ff) as usize;
        (i16::from(self.table[entry]) - 128) * 256
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_volume_scale_is_the_objects_own_table() {
        // +0x9c4: floor(level × 255/63), which is four a level only up to level 20.
        assert_eq!(scale(0), 0);
        assert_eq!(scale(1), 4);
        assert_eq!(scale(16), 64);
        assert_eq!(scale(32), 129);
        assert_eq!(scale(20), 80, "the last level that is exactly four a level");
        assert_eq!(scale(21), 85, "level × 4 would be 84");
        assert_eq!(scale(62), 250);
        assert_eq!(scale(63), 255);
    }

    #[test]
    fn a_voice_value_is_the_high_byte_of_sample_times_scale() {
        // +0x5553's mul and +0x5555's `mov %ah`: an integer division by 256, truncated.
        assert_eq!(voice_value(0, 63), 0);
        assert_eq!(voice_value(128, 63), (128 * 255) >> 8);
        assert_eq!(voice_value(255, 63), (255 * 255) >> 8);
        assert_eq!(voice_value(255, 0), 0);
        assert_eq!(voice_value(200, 32), (200 * 129) >> 8);
        assert_eq!(voice_value(200, 32), 100);
    }

    #[test]
    fn a_step_is_the_objects_two_divisions() {
        // 0x00da7600 / 1712 = 8362, then × 65536 / rate: at 20000 frames a second the step is
        // 8362 × 65536 / 20000 = 27400 in 16.16, i.e. 0.418 of a byte a frame — 8362 a second.
        assert_eq!(step(1712, MixRate::Hz20000, false), 27_400);
        assert_eq!(step(1712, MixRate::Hz40000, false), 13_700);
        // 13700/65536 × 40000 = 8361.9: the 16.16 truncation costs the last hundredth of a
        // hertz, which is the object's own loss (the fraction it discards at `+0x58c1`).
        assert_eq!(step(1712, MixRate::Hz40000, false) * 40_000 / 65_536, 8_361);
        // The inner division truncates *before* the fraction is applied: 14317056 / 428 is
        // 33451 with 148 left over, and the remainder is discarded (`+0x58c1`: `xor %dx,%dx`).
        assert_eq!(step(428, MixRate::Hz40000, false), 33_451 * 65_536 / 40_000);
        // Stereo divides by the object's own [0x8d7] rather than by half the frame rate.
        assert_eq!(step(1712, MixRate::Hz40000, true), 8_362 * 65_536 / 21_111);
        assert_eq!(step(0, MixRate::Hz40000, false), 0);
    }

    #[test]
    fn the_centre_of_a_level_is_the_value_of_sample_128() {
        assert_eq!(centre(0), 0);
        assert_eq!(centre(63), (128 * 255) >> 8);
        assert_eq!(centre(32), (128 * 129) >> 8);
    }

    #[test]
    fn a_voice_carries_the_sample_less_its_centre() {
        // One voice, one sample: the table's value less the table's centre is the signed
        // sample the object indexes its output table with (+0x30b4).
        assert_eq!(voice_value(255, 63) as i32 - centre(63) as i32, 127);
        assert_eq!(voice_value(0, 63) as i32 - centre(63) as i32, -127);
        assert_eq!(voice_value(128, 63) as i32 - centre(63) as i32, 127 - 127);
        // A centre sample is not always exactly zero: the truncation in `>> 8` can leave a
        // level's centre one below half its scale, and a sample one above the centre then
        // reads one. Written out at level 32: 128 × 129 >> 8 = 64, 129 × 129 >> 8 = 65.
        assert_eq!(voice_value(128, 32) as i32 - centre(32) as i32, 0);
        assert_eq!(voice_value(129, 32) as i32 - centre(32) as i32, 1);
    }

    #[test]
    fn the_multiplier_is_the_header_watchs_arithmetic() {
        // +0x5362…+0x5391 with the modules' own master volume byte, 0xB0: 176 has bit 7 set
        // (stereo asked for) and 48 in its low seven bits, so with a stereo output the byte
        // becomes ((48 × 11) >> 3) − 2 = 64 with the stereo bit set — the 2240 the trace reads
        // through the overlapping `_np_masterflags` byte, and a `mul` of 64.
        assert_eq!(multiplier(0xb0, true), 64);
        assert_eq!(multiplier(0xb0, false), 48);
        // The floor the builder applies (+0x5570…+0x5575): 0x83's low bits are 3, whose
        // `((3 × 11) >> 3) − 2` is 2, and a multiplier below 0x10 is built as 0x10.
        assert_eq!(multiplier(0x83, true), 0x10);
        // The underflow: 0x80's low bits are 0, so ((0 × 11) >> 3) − 2 wraps and the
        // `cmp $0x7f,%ax; jbe` sends it to 0x7f.
        assert_eq!(multiplier(0x80, true), 0x7f);
    }

    #[test]
    fn the_curve_is_the_objects_own_table() {
        // K = 0x8000/mul entries of ramp, the high byte of k × (65536/K), after 0x800 − K
        // zeros. At the reference's mul of 64: K = 512, 1536 zeros, then (k × 128) >> 8 =
        // k/2 — 0, 0, 1, 1, … 255, 255.
        let curve = Curve::build(64);
        assert_eq!(curve.mul(), 64);
        for entry in [0usize, 1, 1024, 1535] {
            assert_eq!(curve.table[entry], 0, "entry {entry} is in the zero region");
        }
        assert_eq!(curve.table[1536], 0);
        assert_eq!(curve.table[1537], 0);
        assert_eq!(curve.table[1538], 1);
        assert_eq!(curve.table[1539], 1);
        assert_eq!(curve.table[2046], 255);
        assert_eq!(curve.table[2047], 255);
        // The linear case, mul = 16: K = 2048, no zeros, and the ramp is k/8.
        let linear = Curve::build(16);
        assert_eq!(linear.table[0], 0);
        assert_eq!(linear.table[8], 1);
        assert_eq!(linear.table[1024], 128);
        assert_eq!(linear.table[2047], 255);
        // And the floor: a multiplier below 0x10 is built as 0x10.
        assert_eq!(Curve::build(1).mul(), 0x10);
        assert_eq!(Curve::build(1).table, linear.table);
    }

    #[test]
    fn a_curve_entry_becomes_the_cards_unsigned_byte() {
        // The card plays unsigned 8-bit, so the port's i16 is (byte − 128) × 256.
        let curve = Curve::build(16);
        assert_eq!(curve.sample(1024), 0, "index 1024 is the byte 128, the card's centre");
        assert_eq!(curve.sample(0), -32768);
        assert_eq!(curve.sample(2047), 127 * 256);
        // The reference's own curve, at the marker's base: silence is index 0x501 = 1281, in
        // the zero region, so it reads the byte 0 — the bottom of the card's range.
        let curve = Curve::build(64);
        assert_eq!(curve.sample(0x501), -32768, "the value 0 is silence");
        assert_eq!(curve.sample(0x501 + 255), -32768, "index 1536, the ramp's first entry");
        assert_eq!(curve.sample(0x501 + 256), -32768, "index 1537, still the byte 0");
        assert_eq!(curve.sample(0x501 + 257), -32768 + 256, "index 1538, the byte 1");
        assert_eq!(curve.sample(0x501 + 766), 32512, "index 2047, the byte 255");
        // Past the table's end the port clamps where the object's 16-bit index would read its
        // own segment's neighbours.
        assert_eq!(curve.sample(0x501 + 100_000), 32512);
        assert_eq!(curve.sample(-100_000), -32768);
    }

    #[test]
    fn the_pan_table_is_the_modules_own() {
        // The file's 32-byte table, as module.rs reads it: channels 0…7 alternate L, R, L, R…
        assert_eq!(side_of(0), Side::Left);
        assert_eq!(side_of(8), Side::Right);
        assert_eq!(side_of(1), Side::Left);
        assert_eq!(side_of(9), Side::Right);
        assert_eq!(side_of(3), Side::Left);
        assert_eq!(side_of(11), Side::Right);
        assert_eq!(side_of(255), Side::Centre);
    }

    #[test]
    fn the_ring_drops_the_oldest_when_it_is_full() {
        let mut ring = Ring::new(4);
        for sample in [1i16, 2, 3, 4] {
            assert!(!ring.push(sample), "the ring is not full yet");
        }
        assert_eq!(ring.len(), 4);
        assert!(ring.push(5), "the fifth sample evicts the first");
        assert_eq!(ring.len(), 4);
        let mut out = [0i16; 4];
        assert_eq!(ring.take(&mut out), 4);
        assert_eq!(out, [2, 3, 4, 5], "the oldest four survive, in order");
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.take(&mut out), 0);
    }

    #[test]
    fn the_interpolation_setting_is_the_objects_own_and_off() {
        // `PlayerOptions::interpolation` is the object's `[0x919]`, the byte `S1x` writes and
        // only the interpolating dispatch entries read. Neither module carries an `S1x`
        // (`music.md` §9.12), so the flag is 0 and this mixer has no path that reads it.
        let options = PlayerOptions::default();
        assert!(!options.interpolation);
        assert!(PlayerOptions::default().stereo);
    }
}
