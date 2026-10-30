//! The mixing engine: the player's channel state turned into samples, and the drain the audio
//! device pulls from.
//!
//! Read out of `upstream/SecondReality/MAIN/STMIK.300`'s segment image (every `+0x…` is a
//! segment offset; `tests/player.rs`'s `the_ports_tables_are_the_objects_own` holds the tables
//! to the image). `docs/fidelity/music.md`
//! §10 has the object's mixer in full and the measurements. In the object's order:
//!
//! 1. A voice's level is `m_vol = (avol × master) >> 8` (`+0x5925`), with master
//!    `min(global volume, 64) × 4` (`+0x55c4`); both modules carry 64.
//! 2. Its value is `(sample × scale[m_vol]) >> 8` from the volume table (`+0x551c`, scale at
//!    `+0x9c4`), less the centre value of sample 128 (`+0x37b0`, subtracted at `+0x30b4`).
//! 3. Its step is `0x00da7600 / period × 65536 / rate`, 16.16 (`+0x58b9`, `+0x58c9`), and its
//!    position wraps at `m_end` by `m_loop − m_end` or stops for a one-shot (`+0x3840`).
//! 4. The sums index the output table at `0x400 + value` (the flush, `+0x2c89`/`+0x30ad`):
//!    [`Curve`], which the machine builds with a bug in it (A14). The port clamps the index at
//!    the table's ends where the object reads its own segment (A13).
//!
//! The DMA buffer is replaced by [`Mixer`]'s bounded ring, drained by
//! [`Player::take_samples`](crate::Player::take_samples). The enhanced renderer
//! ([`Quality::Enhanced`], D43, `music.md` §13) is the port's own and off by default: it walks the
//! voices exactly as the authentic path does and renders each frame as 2 or 3 interpolated output
//! frames at 48 kHz.

use crate::effects::{period_step, Channel};
use crate::module::Module;
use crate::player::{MixRate, PlayerOptions, Quality, ENHANCED_RATE};
use crate::CHANNELS;

/// The volume scale table at `+0x9c4`, byte for byte: `floor(level × 255/63)`. It is four a
/// level only up to level 20, and level 63 maps a sample of 255 to 254.
pub const VOLUME_SCALE: [u8; 64] = [
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 85, 89, 93,
    97, 101, 105, 109, 113, 117, 121, 125, 129, 133, 137, 141, 145, 149, 153, 157, 161, 165, 170,
    174, 178, 182, 186, 190, 194, 198, 202, 206, 210, 214, 218, 222, 226, 230, 234, 238, 242, 246,
    250, 255,
];

/// `[0x8d7]`, the stereo divisor (`+0x53a0`…`+0x53a9`): the segment's initial **21111**, which
/// SB Pro never overwrites (its only store, `+0x228f`, is another card's branch). The frame rate
/// is 20000, so stereo plays 5.26 % flat, as the reference does (`bugs.md` §28.5).
const STEREO_DIVISOR: u32 = 0x5277;

/// The one-shot marker for `m_loop` (`+0x48cb`).
const NO_LOOP: u32 = 0xffff_ffff;

/// The flush's base: `0x400 − centre_sum` (`+0x30b4`) plus the accumulator, which the flush
/// zeroes after each read (`+0x2cf1`, `+0x30e8`), so silence lands on entry 1024, the byte 0x80
/// (A14(a); `tests/player.rs`'s `the_flush_clears_the_accumulator_word_it_reads`).
const FLUSH_BASE: i32 = 0x400;

/// The largest position: `m_pos`'s whole bytes are a 16-bit word (`+0x378a`).
const MAX_POS: u32 = 0xffff;

/// One level's scale, clamped to the table (`m_vol` cannot exceed 63).
fn scale(level: u8) -> u8 {
    VOLUME_SCALE[usize::from(level.min(63))]
}

/// The volume table's value for one sample byte at one level: `(sample × scale) >> 8`
/// (`+0x5553`, `+0x5555`).
fn voice_value(sample: u8, level: u8) -> u16 {
    (u16::from(sample) * u16::from(scale(level))) >> 8
}

/// A level's centre: the value of sample 128 (`+0x37ae`…`+0x37b0`).
fn centre(level: u8) -> u16 {
    voice_value(0x80, level)
}

/// The position's step per output frame, 16.16: `m_speed` (`+0x50`).
///
/// The period is clamped to `0x40..=0x7fff` first, the bounds at `0x95A`/`0x958` (`+0x5863`,
/// `+0x5890`), with the zero test between them (`+0x5876`), so 0 still returns 0. The inner
/// division ([`period_step`]) truncates before the rate divides it (`+0x58c9`…`+0x58d8`). Only
/// volume-0 channels reach the high bound on the shipped modules (`music.md` §10.2).
pub fn step(period: u16, mix_rate: MixRate, stereo: bool) -> u32 {
    if period == 0 {
        return 0;
    }
    let rate = if stereo {
        STEREO_DIVISOR
    } else {
        mix_rate.samples_per_second()
    };
    let rate_32 = period_step(period.clamp(0x40, 0x7fff));
    ((u64::from(rate_32) << 16) / u64::from(rate)) as u32
}

/// Which accumulator a channel's samples go to, from the module's channel table
/// (`Module::channel_pan`; `amixtype`, read at `+0x37b5`): 0…7 left, 8…15 right, 255 unused.
/// Both modules pan channels 0…7 L, R, L, R, L, R, L, R.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Side {
    /// Both: a mono mix, or a channel the module leaves unpositioned.
    Centre,
    Left,
    Right,
}

impl Side {
    /// The side channel `index` plays on: [`Side::Centre`] in mono, else from its pan byte.
    fn of(options: &PlayerOptions, pan: &[u8; CHANNELS], index: usize) -> Side {
        if !options.stereo {
            return Side::Centre;
        }
        match pan[index] {
            255 => Side::Centre,
            pan if pan & 8 == 0 => Side::Left,
            _ => Side::Right,
        }
    }

    /// Add `value` to the accumulators this side feeds.
    fn add(self, value: i32, left: &mut i32, right: &mut i32) {
        match self {
            Side::Left => *left += value,
            Side::Right => *right += value,
            Side::Centre => {
                *left += value;
                *right += value;
            }
        }
    }
}

// -------------------------------------------------------------------------------------------
// The voice
// -------------------------------------------------------------------------------------------

/// One mixer voice: the mixer half of the object's `_zchn` entry (from `m_vol`, `+0x41`).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
struct Voice {
    /// The instrument whose sample the voice plays: `m_base`'s stand-in, `None` until a note
    /// start writes it (`+0x4886`, `+0x488d`).
    instrument: Option<u8>,
    /// `m_end` (`+0x48`), in sample bytes: where the voice stops or rewraps.
    end: u32,
    /// `m_loop` (`+0x4c`), in sample bytes: where a rewrapped voice continues, or [`NO_LOOP`].
    loop_start: u32,
    /// `m_speed`, 16.16 sample bytes an output frame, cached against `cached_period`.
    speed: u32,
    /// The period [`Voice::speed`] was computed for: the object recomputes `m_speed` at every
    /// write to the period (`+0x582b`).
    cached_period: u16,
    /// `m_vol` (`+0x41`), cached against the pair [`Voice::cached_volume`].
    volume: u8,
    /// The `(avol, master)` pair [`Voice::volume`] was computed from.
    cached_volume: Option<(u8, u16)>,
    /// Set when a one-shot ran into `m_end` (`+0x3845`: `m_speed = 0`); only a note start
    /// clears it.
    stopped: bool,
}

impl Voice {
    /// `+0x5925`: `m_vol = (avol × master) >> 8`, recomputed when either input moves.
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

/// The samples the mixer has produced that the device has not taken: the port's stand-in for
/// the DMA buffer, which trails the engine's position (`MAIN/STMIK.H:100-103`).
///
/// When it is full the oldest samples are dropped ([`Mixer::dropped`] counts them): a poll is
/// one frame of the demo's timeline and may never wait for the device.
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

/// The ring's capacity in samples: 8192 i16, about fourteen polls or 0.2 s of stereo output.
/// The port's choice (the original's `_zbufsize` is 4096 bytes), large enough that a device
/// draining once a frame never overflows it.
pub const RING_SAMPLES: usize = 8192;

/// The port's mixing engine: one `Voice` per channel and the ring the device drains. It owns no
/// clock: [`Player::poll`](crate::Player::poll) drives it, once a frame.
pub struct Mixer {
    voices: [Voice; CHANNELS],
    ring: Ring,
    /// The output curve, rebuilt when its multiplier changes, as the header watch does
    /// (`+0x53af`, `+0x5420`).
    curve: Curve,
    /// The samples the ring's overflow policy evicted.
    dropped: u64,
    /// The right half of a stereo frame the last budget split (a byte-stream DMA has the same
    /// seam).
    held_right: Option<i16>,
    /// The enhanced path's output frames owed, in units of `1/authentic rate`: each authentic
    /// frame adds [`ENHANCED_RATE`] and emits one frame per whole rate held, so after `k`
    /// frames exactly `floor(k × 12/5)` have gone out. Always 0 on the authentic path.
    owed: u32,
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

    /// A mixer whose ring holds `capacity` samples (`tests/mix.rs` reaches the overflow with it).
    pub fn new_with_capacity(capacity: usize) -> Mixer {
        Mixer {
            voices: [Voice::default(); CHANNELS],
            ring: Ring::new(capacity),
            // The reference's own (0xB0, stereo); `mix` rebuilds it from the module.
            curve: Curve::new(0xb0, true),
            dropped: 0,
            held_right: None,
            owed: 0,
        }
    }

    /// A mixer whose ring holds the same 0.2 s of `options`' stream: [`RING_SAMPLES`], or
    /// 19,660 for the enhanced path.
    pub fn for_options(options: &PlayerOptions) -> Mixer {
        let capacity = match options.quality {
            Quality::Authentic => RING_SAMPLES,
            // Rounded down to whole stereo frames, so an eviction never splits one.
            Quality::Enhanced => {
                (RING_SAMPLES * ENHANCED_RATE as usize / options.authentic_rate() as usize) & !1
            }
        };
        Mixer::new_with_capacity(capacity)
    }

    /// The samples the overflow policy has dropped.
    pub fn dropped(&self) -> u64 {
        self.dropped
    }

    /// How many samples the ring holds before it drops the oldest ([`Mixer::for_options`]).
    pub fn capacity(&self) -> usize {
        self.ring.buffer.len()
    }

    /// The samples waiting for the device.
    pub fn pending(&self) -> usize {
        self.ring.len()
    }

    /// Every voice silent and the ring empty: `_zplaysong`'s channel clear (`+0x5614`, whose
    /// `+0x5641`/`+0x5644` zero `m_base`). The curve is the card's and stays.
    pub fn reset(&mut self) {
        self.voices = [Voice::default(); CHANNELS];
        self.ring.reset();
        self.dropped = 0;
        self.held_right = None;
        self.owed = 0;
    }

    /// Give a channel a fresh sample (`+0x4872`…`+0x48cb`): `m_loop` takes `lbeg` (`+0x48a6`)
    /// and `m_end` takes `lend` (`+0x48b9`), and a zero `lend` is the one-shot path (`+0x489c`).
    /// With `checkins` clearing the loop flag when `lend <= lbeg` (`STARTEMS.C:145`), a sample
    /// loops when its flag is set and `lend > lbeg`, including the full-sample loops from 0
    /// (`the_modules_loops_and_the_objects_zero_loop_end_rule`).
    ///
    /// The object reduces a `lend` at or below 500 by whole loop lengths (`+0x48ab`…`+0x48b7`),
    /// which keeps the phase; no shipped sample takes that path, so the port keeps `lend`.
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
            // `+0x48be`: the object's `m_end` is the length plus 0x20, into the padding of 0x80
            // `checkins` writes behind a one-shot (`STARTEMS.C:167-172`); that slack is silence.
            voice.end = header.length.min(MAX_POS);
        }
    }

    /// Mix `samples` of the object's bytes (one i16 a frame in mono, two in stereo) from the
    /// player's channel state into the ring, advancing the channels' `m_pos`/`m_poslow`. An odd
    /// stereo budget holds the frame's right half for the next call.
    pub fn mix(
        &mut self,
        module: &Module,
        channels: &mut [Channel; CHANNELS],
        options: &PlayerOptions,
        global_volume: u8,
        samples: usize,
    ) {
        // `+0x55c4`: `[0x913] = min(global volume byte, 64) × 4`.
        let master = u16::from(global_volume.min(64)) * 4;
        // The header watch rebuilds the curve when its byte moves (`+0x5397`…`+0x53af`).
        let mul = multiplier(module.master_volume(), options.stereo);
        if self.curve.mul() != mul {
            self.curve = Curve::build(mul);
        }
        let width = if options.stereo { 2 } else { 1 };
        let mut budget = samples;

        // `samples` is the authentic budget whatever the quality. The enhanced path emits whole
        // frames, so a split frame's right half has already gone out and is consumed unpushed.
        let enhanced = options.quality == Quality::Enhanced;

        if let Some(right) = self.held_right.take() {
            if budget == 0 {
                self.held_right = Some(right);
                return;
            }
            if !enhanced {
                self.push(right);
            }
            budget -= 1;
        }

        while budget >= width {
            if enhanced {
                self.enhanced_frame(module, channels, options, master);
            } else {
                let (left, right) = self.frame(module, channels, options, master);
                self.push(left);
                if options.stereo {
                    self.push(right);
                }
            }
            budget -= width;
        }

        if budget == 1 {
            if enhanced {
                let right = self.enhanced_frame(module, channels, options, master).1;
                self.held_right = Some(right);
            } else {
                let (left, right) = self.frame(module, channels, options, master);
                self.push(left);
                self.held_right = Some(right);
            }
        }
    }

    /// The device's drain: up to `out.len()` samples, oldest first. It moves no engine state.
    pub fn take(&mut self, out: &mut [i16]) -> usize {
        self.ring.take(out)
    }

    /// Push one sample into the ring, counting an eviction.
    fn push(&mut self, sample: i16) {
        self.dropped += u64::from(self.ring.push(sample));
    }

    /// One output frame: every voice's sample read, scaled, accumulated and converted. The port
    /// tests the ends per frame where the object tests them per chunk (`+0x3826`…`+0x387d`);
    /// the rewrap keeps the phase either way, so the samples are the same.
    fn frame(
        &mut self,
        module: &Module,
        channels: &mut [Channel; CHANNELS],
        options: &PlayerOptions,
        master: u16,
    ) -> (i16, i16) {
        let pan = module.channel_pan();
        let mut left: i32 = 0;
        let mut right: i32 = 0;

        for (index, (channel, voice)) in channels.iter_mut().zip(&mut self.voices).enumerate() {
            voice.refresh_volume(channel.volume, master);
            voice.refresh_speed(channel.period, options);
            let Some(contribution) = voice_contribution(module, voice, channel) else {
                continue;
            };
            Side::of(options, pan, index).add(contribution, &mut left, &mut right);
            advance(voice, channel);
        }

        // `+0x30b4`/`+0x2cb4`: indexed at the flush's base. A mono mix writes only the left.
        let left = self.emit(left);
        let right = if options.stereo { self.emit(right) } else { 0 };
        (left, right)
    }

    /// One authentic frame rendered by the enhanced path ([`Quality::Enhanced`], D43).
    ///
    /// The voices are walked exactly as [`Mixer::frame`] walks them, so the clock and every
    /// position are the authentic ones. The frame becomes `n` output frames (2 or 3); frame `j`
    /// reads each voice at its position plus `j/n` of its step, interpolated
    /// ([`enhanced_value`]), and each side's sum goes through the curve's small-signal gain
    /// ([`Mixer::enhanced_gain`]) rather than its table.
    ///
    /// Returns the last output frame, which a split frame holds its right half from.
    fn enhanced_frame(
        &mut self,
        module: &Module,
        channels: &mut [Channel; CHANNELS],
        options: &PlayerOptions,
        master: u16,
    ) -> (i16, i16) {
        self.owed += ENHANCED_RATE;
        let rate = options.authentic_rate();
        let count = self.owed / rate;
        self.owed %= rate;

        // Each sounding voice as it is before this frame's advance.
        let pan = module.channel_pan();
        let mut voices: [Option<EnhancedVoice>; CHANNELS] = [None; CHANNELS];
        for index in 0..CHANNELS {
            let channel = &mut channels[index];
            let voice = &mut self.voices[index];
            voice.refresh_volume(channel.volume, master);
            voice.refresh_speed(channel.period, options);
            if voice_contribution(module, voice, channel).is_none() {
                continue;
            }
            let Some(bytes) = voice
                .instrument
                .and_then(|instrument| module.sample_bytes(usize::from(instrument.wrapping_sub(1))))
            else {
                continue;
            };
            let side = Side::of(options, pan, index);
            voices[index] = Some(EnhancedVoice {
                bytes,
                position: (u64::from(channel.position) << 16) | u64::from(channel.position_low),
                speed: u64::from(voice.speed),
                end: voice.end,
                loop_start: voice.loop_start,
                scale: i32::from(scale(voice.volume)),
                side,
            });
            advance(voice, channel);
        }

        let gain = self.enhanced_gain();
        let mut last = (0, 0);
        for j in 0..count {
            let mut left: i32 = 0;
            let mut right: i32 = 0;
            for voice in voices.iter().flatten() {
                let at = voice.position + voice.speed * u64::from(j) / u64::from(count);
                voice.side.add(enhanced_value(voice, at), &mut left, &mut right);
            }
            last = (enhanced_sample(left, gain), enhanced_sample(right, gain));
            self.push(last.0);
            if options.stereo {
                self.push(last.1);
            }
        }
        last
    }

    /// The curve's small-signal gain, in `i16` per two units of a frame's value: the entries one
    /// either side of the base (256 at the reference's `mul` 64).
    fn enhanced_gain(&self) -> i64 {
        let above = i64::from(self.curve.sample(FLUSH_BASE + 1));
        let below = i64::from(self.curve.sample(FLUSH_BASE - 1));
        above - below
    }

    /// One output sample: the curve's entry for a frame's value at [`FLUSH_BASE`].
    fn emit(&self, value: i32) -> i16 {
        self.curve.sample(FLUSH_BASE + value)
    }
}

/// One voice's contribution to the frame: its table value less its centre (the object sums
/// the two separately and subtracts at `+0x30b4`, which is the same), or `None` when it is
/// skipped: no sample yet, `m_speed` 0 (`+0x3783`), or `m_pos` at `0xffff` (`+0x378e`), where
/// the "volume 0" path parks a freed channel ([`Channel::row_start`]).
fn voice_contribution(module: &Module, voice: &Voice, channel: &Channel) -> Option<i32> {
    let instrument = voice.instrument?;
    if voice.stopped || voice.speed == 0 {
        return None;
    }
    // `+0x378a` reads `m_pos` as a 16-bit word, so the compare is on the word the object holds.
    if (channel.position & 0xffff) == MAX_POS {
        return None;
    }
    let bytes = module.sample_bytes(usize::from(instrument.wrapping_sub(1)))?;
    // Past the end the object reads `checkins`'s padding (`STARTEMS.C:129-177`); the port reads
    // the last byte. Only an `O` longer than the sample gets there.
    let at = usize::try_from(channel.position).ok()?;
    let sample = bytes.get(at).or_else(|| bytes.last()).copied()?;
    Some(i32::from(voice_value(sample, voice.volume)) - i32::from(centre(voice.volume)))
}

/// What [`Mixer::enhanced_frame`] reads of a sounding voice before it is advanced.
#[derive(Clone, Copy, Debug)]
struct EnhancedVoice<'a> {
    /// The sample bytes the voice plays.
    bytes: &'a [u8],
    /// The 16.16 position before this frame's advance: `m_pos` and `m_poslow`.
    position: u64,
    /// `m_speed`, 16.16.
    speed: u64,
    /// `m_end` and `m_loop`, as [`Voice`] holds them.
    end: u32,
    loop_start: u32,
    /// The volume scale for `m_vol` (`+0x9c4`).
    scale: i32,
    side: Side,
}

/// The fractional bits the enhanced path keeps of an interpolated sample byte.
const ENHANCED_FRACTION: u32 = 8;

/// One voice's enhanced value at the 16.16 position `at`: the sample interpolated between the
/// byte at `at` and the next, less the centre, times the scale, without the authentic `>> 8`
/// and with [`ENHANCED_FRACTION`] bits of fraction kept.
///
/// A position that crosses the end inside this frame's step is placed as [`advance`] will place
/// it (rewrapped, or held on a one-shot's last byte); one already past the end reads the last
/// byte, as the authentic path does.
fn enhanced_value(voice: &EnhancedVoice, at: u64) -> i32 {
    let bytes = voice.bytes;
    let end = u64::from(voice.end) << 16;
    let looped = voice.loop_start != NO_LOOP;
    let at = if at >= end && voice.position < end {
        if looped {
            at - end + (u64::from(voice.loop_start) << 16)
        } else {
            end.saturating_sub(1 << 16)
        }
    } else {
        at
    };
    let whole = (at >> 16) as usize;
    let fraction = (at & 0xffff) as i32;
    let last = bytes.last().copied().unwrap_or(0x80);
    let first = bytes.get(whole).copied().unwrap_or(last);
    let second = if whole + 1 < voice.end as usize {
        bytes.get(whole + 1).copied().unwrap_or(last)
    } else if looped {
        bytes.get(voice.loop_start as usize).copied().unwrap_or(last)
    } else {
        last
    };
    let (first, second) = (i32::from(first), i32::from(second));
    let sample = (first << ENHANCED_FRACTION)
        + (((second - first) * fraction) >> (16 - ENHANCED_FRACTION));
    (sample - (0x80 << ENHANCED_FRACTION)) * voice.scale
}

/// A side's enhanced sum as an `i16`: `gain` ([`Mixer::enhanced_gain`]) applied to a sum that
/// is `2^(8 + ENHANCED_FRACTION)` times the authentic value, clamped.
fn enhanced_sample(sum: i32, gain: i64) -> i16 {
    let scaled = i64::from(sum) * gain / (2 << (8 + ENHANCED_FRACTION));
    scaled.clamp(i64::from(i16::MIN), i64::from(i16::MAX)) as i16
}

/// Advance one voice's position by `m_speed` (`+0x346c`…`+0x347e`) and honour its end
/// (`+0x3840`…`+0x385a`): rewrap by `m_loop − m_end` keeping the overshoot, or stop a one-shot.
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

/// The output curve: the table `+0x551c`'s second half builds at `cs:[0x6c]` (`+0x5565`…`+0x55c1`)
/// and the flush applies, from the multiplier the header watch derives ([`multiplier`]). At the
/// reference's `mul` 64 it is 768 zeros, the ramp at 768…1279 and `0xFF` from 1280 (`music.md`
/// §10.3).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Curve {
    table: Vec<u8>,
    mul: u8,
}

/// The curve's multiplier from the header's master-volume byte (`Module::master_volume`): the
/// header watch (`+0x535e`…`+0x5391`), then the builder's floor of 0x10 (`+0x5575`).
///
/// With bit 7 (stereo) set on a stereo output it is `min((v × 11) >> 3 − 2, 0x7f)`, so the
/// modules' 0xB0 gives 64 (`music-clock.md` §5's 2240). The 16-bit `sub $0x2` can wrap, and the
/// `cmp $0x7f; jbe` then resets it to 0x7f. Otherwise it is `v & 0x7f`.
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
    /// The curve for a master-volume byte and an output mode (`+0x53af`, `+0x5420`).
    pub fn new(master_volume: u8, stereo: bool) -> Curve {
        Curve::build(multiplier(master_volume, stereo))
    }

    /// The curve for a multiplier, with the builder's bug in it, because the machine's memory
    /// holds the bug (`bugs.md` §28.4, A14).
    ///
    /// `K = 0x8000 / mul` ramp entries of `(k × (65536/K)) >> 8` (`+0x5588`, `+0x55a7`). The
    /// zero fill is meant to be `0x800 − K` bytes, but `+0x559d`'s `rep stosw` leaves `CX` = 0,
    /// so `+0x55a1`'s `rep stosb` writes nothing: the fill is `2 × ⌊(0x800 − K)/4⌋` (the two
    /// `shr $1,%cx`), and `+0x55bd` fills the shortfall after the ramp with `0xFFFF` in
    /// `2 × ⌊rem/2⌋` bytes. `+0x55c1`'s `rep stosb` writes nothing either, so an odd last byte
    /// keeps what the segment held, on a first build the object's zero (`cs:[0x6c]` is 0x6E, and
    /// the object's 0x800 bytes there are zero); the port leaves it zero. `rem` is odd only for
    /// an odd `K`, never at the modules' `mul` of 64 or 48. `tests/common/oracle.rs`'s `curve` writes the same
    /// arithmetic out independently.
    pub fn build(mul: u8) -> Curve {
        let mul = mul.max(0x10);
        let ramp = 0x8000 / usize::from(mul);
        let step = 65536 / ramp;
        let span = 0x800 - ramp;
        let zeros = (span / 4) * 2;
        let mut table = vec![0u8; 0x800];
        for entry in 0..ramp {
            table[zeros + entry] = ((entry * step) >> 8) as u8;
        }
        let rest = 0x800 - (zeros + ramp);
        let tail = (rest / 2) * 2;
        for byte in table[zeros + ramp..zeros + ramp + tail].iter_mut() {
            *byte = 0xff;
        }
        Curve { table, mul }
    }

    /// The multiplier the table was built with.
    pub fn mul(&self) -> u8 {
        self.mul
    }

    /// One entry, widened as the card plays it: unsigned 8-bit, so `(byte − 128) × 256`.
    ///
    /// The index is clamped to the 2048 entries; the object's 16-bit index reads past the table
    /// into its own segment (A13).
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
        for level in 0..64u16 {
            assert_eq!(u16::from(VOLUME_SCALE[usize::from(level)]), level * 255 / 63);
        }
        assert_eq!(scale(64), 255, "a level past the table is clamped, not a panic");
    }

    #[test]
    fn a_voice_value_is_the_high_byte_of_sample_times_scale() {
        // +0x5553's mul and +0x5555's `mov %ah`: an integer division by 256, truncated.
        assert_eq!(voice_value(255, 63), 254);
        assert_eq!(voice_value(255, 0), 0);
        assert_eq!(voice_value(200, 32), 100);
        // Less the centre (+0x30b4): ±127 at full scale, and the truncation can leave a
        // level's centre one below half its scale, so one above the centre reads one.
        assert_eq!(centre(63), 127);
        assert_eq!(voice_value(0, 63) as i32 - centre(63) as i32, -127);
        assert_eq!(voice_value(128, 32) as i32 - centre(32) as i32, 0);
        assert_eq!(voice_value(129, 32) as i32 - centre(32) as i32, 1);
    }

    #[test]
    fn a_step_is_the_objects_two_divisions() {
        // 0x00da7600 / 1712 = 8362, then × 65536 / rate.
        assert_eq!(step(1712, MixRate::Hz20000, false), 27_400);
        assert_eq!(step(1712, MixRate::Hz40000, false), 13_700);
        // The inner division truncates before the fraction is applied: 14317056 / 428 is
        // 33451 with 148 left over, and the remainder is discarded (`+0x58c1`).
        assert_eq!(step(428, MixRate::Hz40000, false), 33_451 * 65_536 / 40_000);
        // Stereo divides by the object's own [0x8d7], not by the frame rate.
        assert_eq!(step(1712, MixRate::Hz40000, true), 8_362 * 65_536 / 21_111);
        assert_eq!(step(0, MixRate::Hz40000, false), 0);
    }

    #[test]
    fn a_step_clamps_the_period_the_way_the_object_does() {
        // [0x958] = 0x7FFF and [0x95A] = 0x40, with the zero test between them.
        for period in [0x8000u16, 0xffff] {
            assert_eq!(
                step(period, MixRate::Hz40000, false),
                step(0x7fff, MixRate::Hz40000, false),
                "period {period:#06x} is above the object's high bound"
            );
        }
        for period in [0x3fu16, 1] {
            assert_eq!(
                step(period, MixRate::Hz40000, false),
                step(0x40, MixRate::Hz40000, false),
                "period {period:#06x} is below the object's low bound"
            );
        }
        assert_eq!(step(0, MixRate::Hz40000, false), 0, "zero is not clamped up");
        // 14317056 / 0x7FFF is 436, and 436 × 65536 / 40000 is 714.
        assert_eq!(step(0x8000, MixRate::Hz40000, false), 714);
    }

    #[test]
    fn a_voice_freed_at_position_ffff_is_not_advanced() {
        // `+0x378e`/`+0x3791`: a channel the "volume 0" path parked at 0xffff is in neither the
        // sums nor the position walk. Without the gate its position moves.
        let bytes = std::fs::read("../../assets/MAIN/MUSIC0.S3M")
            .expect("the shipped module; a missing asset is not this test's subject");
        let module = Module::load(&bytes).expect("MUSIC0 loads");
        let instrument = (1..=module.instrument_count())
            .find(|&index| {
                module
                    .instrument(index - 1)
                    .is_some_and(crate::module::Instrument::is_pcm)
            })
            .expect("MUSIC0 has a PCM instrument") as u8;

        let mut mixer = Mixer::new();
        let mut channels = [Channel::default(); CHANNELS];
        mixer.start_note(0, &module, instrument);
        channels[0].position = MAX_POS;
        // The low bound: about eleven bytes a frame, so a missing gate shows in one frame.
        channels[0].period = 0x40;
        channels[0].volume = 0;
        let options = PlayerOptions::default();
        for frame in 0..4 {
            mixer.frame(&module, &mut channels, &options, 64);
            assert_eq!(
                channels[0].position, MAX_POS,
                "frame {frame}: a voice at m_pos 0xffff is skipped, not walked"
            );
        }

        // The control: one byte below, the same voice is walked (or rewrapped).
        channels[0].position = MAX_POS - 1;
        mixer.frame(&module, &mut channels, &options, 64);
        assert_ne!(channels[0].position, MAX_POS - 1);
    }

    #[test]
    fn the_multiplier_is_the_header_watchs_arithmetic() {
        // +0x5362…+0x5391 with the modules' 0xB0: ((48 × 11) >> 3) − 2 = 64 in stereo.
        assert_eq!(multiplier(0xb0, true), 64);
        assert_eq!(multiplier(0xb0, false), 48);
        // 0x83: ((3 × 11) >> 3) − 2 = 2, floored to 0x10 (+0x5570…+0x5575).
        assert_eq!(multiplier(0x83, true), 0x10);
        // 0x80: ((0 × 11) >> 3) − 2 wraps, and `cmp $0x7f,%ax; jbe` sends it to 0x7f.
        assert_eq!(multiplier(0x80, true), 0x7f);
    }

    #[test]
    fn the_curve_is_the_objects_own_table() {
        // mul 64: K = 512, the ramp is k/2, and the halved zero fill puts it at 768…1279 with
        // 0xFF from 1280. Every boundary here is one the probe read (A14).
        let curve = Curve::build(64);
        assert_eq!(curve.mul(), 64);
        for entry in [0usize, 1, 766, 767, 768, 769] {
            assert_eq!(curve.table[entry], 0, "entry {entry}");
        }
        assert_eq!(curve.table[770], 1);
        assert_eq!(curve.table[771], 1);
        assert_eq!(curve.table[1024], 128, "the ramp's centre");
        assert_eq!(curve.table[1248], 240);
        assert_eq!(curve.table[1279], 255);
        for entry in [1280usize, 1300, 2046, 2047] {
            assert_eq!(curve.table[entry], 0xff, "entry {entry} is the 0xFFFF tail");
        }
        // The floor: a multiplier below 0x10 is built as 0x10.
        assert_eq!(Curve::build(1).table, Curve::build(16).table);
        // mul 48, the mono multiplier: K = 682 and the zero fill is 2 × ⌊1366/4⌋ = 682, where a
        // plain halving would write 683.
        let mono = Curve::build(48);
        assert_eq!(mono.table[684], 0, "(k × 96) >> 8 at k = 2");
        assert_eq!(mono.table[685], 1, "(k × 96) >> 8 = 1 at k = 3");
    }

    #[test]
    fn a_curve_entry_becomes_the_cards_unsigned_byte() {
        // The linear case (mul 16): K = 2048, no zero fill, the ramp covers the whole span.
        let curve = Curve::build(16);
        assert_eq!(curve.sample(1024), 0, "index 1024 is the byte 128, the card's centre");
        assert_eq!(curve.sample(0), -32768);
        assert_eq!(curve.sample(2047), 127 * 256);
        // The reference's curve at the flush's base: silence is the ramp's centre.
        let curve = Curve::build(64);
        assert_eq!(curve.sample(FLUSH_BASE), 0);
        assert_eq!(curve.sample(FLUSH_BASE + 80), 40 * 256, "±80 of value is ±40 of byte");
        assert_eq!(curve.sample(FLUSH_BASE - 80), -40 * 256);
        assert_eq!(curve.sample(FLUSH_BASE + 512), 32512, "the 0xFF tail");
        // Past the table the port clamps (A13).
        assert_eq!(curve.sample(FLUSH_BASE + 100_000), 32512);
        assert_eq!(curve.sample(-100_000), -32768);
    }

    #[test]
    fn the_ring_drops_the_oldest_when_it_is_full() {
        let mut ring = Ring::new(4);
        for sample in [1i16, 2, 3, 4] {
            assert!(!ring.push(sample), "the ring is not full yet");
        }
        assert!(ring.push(5), "the fifth sample evicts the first");
        assert_eq!(ring.len(), 4);
        let mut out = [0i16; 4];
        assert_eq!(ring.take(&mut out), 4);
        assert_eq!(out, [2, 3, 4, 5], "the oldest four survive, in order");
        assert_eq!(ring.take(&mut out), 0);
    }
}
