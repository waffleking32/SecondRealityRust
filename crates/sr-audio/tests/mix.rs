//! Task 5's own tests: what the mixer produces, computed by hand from the module's bytes.
//!
//! The evidence in this file is of two kinds, and they are different:
//!
//! 1. **The arithmetic, hand-computed.** Every expected sample value below is worked out in
//!    this file from the `S3M` file's own sample bytes, the channel's own volume and period,
//!    and the object's own arithmetic — the volume scale table at `+0x9c4`, the step at
//!    `+0x58b9`, the 16.16 position, the loop's rewrap. The tables are pasted here as the
//!    object's bytes, and `tests/player.rs`'s claims assert those same bytes in the object's
//!    image, so a wrong table fails in two places.
//!
//!    **Nothing here is compared against a recording of this port**, and nothing could be: the
//!    emulator runs silent (`docs/decisions.md` D17), so Phase 3 has no audio reference at all
//!    (`docs/fidelity/music.md` §10). What the object *would* have played is the object's own
//!    bytes and its own arithmetic, and that is what these tests compute from.
//!
//! 2. **The properties of the stream.** Determinism, non-silence, the samples a window
//!    carries, the drain's exactness and the ring's overflow policy — asserted on the shipped
//!    modules through [`Player`], because they are properties of the engine rather than of one
//!    voice.
//!
//! Every number this file prints is printed by `cargo test -p sr-audio --test mix -- --nocapture`.

use std::path::Path;

use sr_audio::{Channel, Curve, MixRate, Mixer, Module, Player, PlayerOptions, CHANNELS};

const ASSETS: &str = "../../assets/MAIN";

fn shipped(name: &str) -> Vec<u8> {
    let path = Path::new(ASSETS).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The module as the shell loads it: `Song::load`, MUSIC0 patched to tempo 120.
fn module(name: &'static str) -> Module {
    sr_audio::Song::named(name)
        .unwrap_or_else(|| panic!("{name} is not a song"))
        .load(Path::new("../.."))
        .unwrap_or_else(|error| panic!("{error}"))
}

// -------------------------------------------------------------------------------------------
// The object's own tables, as the object's own bytes
// -------------------------------------------------------------------------------------------

/// The volume scale table at `+0x9c4`: 64 entries, `floor(level × 255/63)`.
/// (`mix.rs`'s `VOLUME_SCALE` is the same 64 bytes; this copy is what the arithmetic below is
/// worked out from, and `tests/player.rs` asserts the image's bytes.)
const SCALE: [u8; 64] = [
    0, 4, 8, 12, 16, 20, 24, 28, 32, 36, 40, 44, 48, 52, 56, 60, 64, 68, 72, 76, 80, 85, 89, 93,
    97, 101, 105, 109, 113, 117, 121, 125, 129, 133, 137, 141, 145, 149, 153, 157, 161, 165, 170,
    174, 178, 182, 186, 190, 194, 198, 202, 206, 210, 214, 218, 222, 226, 230, 234, 238, 242, 246,
    250, 255,
];

/// `+0x58b9`'s constant: `0x00da7600` = 14,317,056, the object's own base frequency.
const BASE_FREQ: u64 = 0x00da_7600;

/// `[0x8d7]`, the divisor the object picks in stereo (`+0x53a4`), holding the segment's
/// initial data. The reference's SB Pro run never writes it (`music.md` §10).
const STEREO_DIVISOR: u64 = 0x5277;

/// The object's two divisions, written out (`+0x58b9`, `+0x58c9`…`+0x58d8`): the base
/// frequency over the period, 16 bits of fraction, then over the mixing rate.
fn step(period: u16, rate: u64) -> u32 {
    (((BASE_FREQ / u64::from(period)) << 16) / rate) as u32
}

/// One voice's contribution for one sample byte at one level: `+0x5553`'s `mul` with
/// `+0x5555`'s `mov %ah` is `(sample × scale) >> 8`, and `+0x37b0`'s centre is the same at the
/// sample byte 128 — the difference is what `+0x30b4`'s correction leaves.
fn value(sample: u8, level: u8) -> i32 {
    let scale = i32::from(SCALE[usize::from(level)]);
    ((i32::from(sample) * scale) >> 8) - ((128 * scale) >> 8)
}

/// The base the port indexes the curve with: the flush's own `0x400 − centre_sum`
/// (`+0x30b4`, `+0x2cbe`), so a frame's value lands on entry `0x400 + value` and silence is the
/// curve's 1024th entry — the byte 0x80 the audible DMA buffer is centred on. **Every** sample
/// is read at this base, the first chunk's included, because the flush stores zero over every
/// accumulator word it reads (`+0x2ce5`/`+0x2cf1`, `+0x30d0`/`+0x30e8`;
/// `tests/player.rs`'s `the_flush_clears_the_accumulator_word_it_reads`), so no chunk's index
/// moves — A14(a), settled.
const BASE: i32 = 0x400;

/// The number of the object's first mixed chunk: the DMA loop's countdown starts at 1
/// (`+0x50ab`) and is rounded up to 64 (`+0x50e6`).
const FIRST_CHUNK: usize = 64;

/// The offset a **misreading** of the flush put on a later chunk's index: a segment builder that
/// decoded the `LIDATA` zeros of `+0x2ce5`'s and `+0x30d0`'s immediates as `0x01` saw the flush
/// write `0x0101` into each accumulator word, which would have read at `0x501 + value`. The
/// flush writes zero (A14(a)), so neither the object nor the port ever reads there; the
/// constant exists only for the counterfactual assertions below, which keep the numbers the
/// misreading predicted (`music.md` §10.4 quotes two of them) so that the difference stays
/// checkable.
const MISREAD_MARKER_OFFSET: i32 = 0x101;

/// The object's output curve, worked out by hand: the machine's own counts of `+0x5565`…
/// `+0x55c1` — `2 × ⌊(0x800 − K)/4⌋` zero entries (the two `shr $1,%cx` halve the count twice),
/// `K = 0x8000/mul` ramp entries of `(k × (65536/K)) >> 8`, then `+0x55bd`'s `2 × ⌊rem/2⌋`-byte
/// `0xFF` tail — indexed at the base the flush's correction gives a frame: `0x400 + value`,
/// every sample of the window. The card plays unsigned 8-bit, so the port's `i16` is
/// `(byte − 128) × 256`.
///
/// **This is a second copy of `Curve::build`'s arithmetic, and deliberately so**: the expected
/// sample values below must not be computed by the code they are checking, so the oracle is
/// written out from the object's instructions instead of called. What it catches is a
/// transcription slip in either copy (`the_files_hand_arithmetic_is_the_builders_at_every_entry`
/// also pins the two together, and M1's mutation of the builder fails five tests through it);
/// what it cannot catch is a *shared* misreading of `+0x559d`…`+0x55bd`. That one is guarded
/// from outside — the probe's 768/1279/1280 measurement (A14) and `mix::tests::
/// the_curve_is_the_objects_own_table`'s literal boundary entries — not by this copy.
fn curve_with(mul: u32, value: i32) -> i16 {
    curve_at(mul, BASE, value)
}
fn curve_at(mul: u32, base: i32, value: i32) -> i16 {
    let k = (0x8000 / mul) as usize;
    let step = 65536 / k;
    // The machine's arithmetic, step for step: the two `shr $1,%cx` at `+0x5592`/`+0x559b` leave
    // `2 × floor((0x800 − K)/4)` zero bytes, then the ramp's `K` entries, then `+0x55b5`'s
    // shortfall as `2 × floor(rem/2)` bytes of `0xFFFF`. For mul 64 that is 768 zeros, the ramp
    // at 768…1279 and 0xFF from 1280 — the boundaries the probe read; for mul 48 — the port's
    // mono multiplier — the zero fill is 682, not the 683 a plain halving would write.
    let span = 0x800 - k;
    let zeros = (span / 4) * 2;
    let rest = 0x800 - (zeros + k);
    let tail = (rest / 2) * 2;
    let index = (base + value).clamp(0, 0x7ff) as usize;
    let byte = if index < zeros {
        0
    } else if index < zeros + k {
        ((((index - zeros) * step) >> 8) & 0xff) as u8
    } else if index < zeros + k + tail {
        0xff
    } else {
        0 // the span's last byte, the object's own 0x01 filler, left alone
    };
    (i16::from(byte) - 128) * 256
}

/// The reference's own curve: the modules' master volume byte is 0xB0, whose
/// `+0x5362`…`+0x5391` derivation gives a multiplier of **64**, so `K` is 512 — 768 zeros, then
/// 512 ramp entries of `k/2`, then the `0xFF` tail — with silence at the ramp's centre, the
/// byte 0x80, which is the 0 every window here counts as its own silence.
fn curve(value: i32) -> i16 {
    curve_with(64, value)
}

/// MUSIC0 with one header byte replaced. The master volume byte is 0x33, and `0x82` is
/// stereo | 2, whose derivation `((2 × 11) >> 3) − 2` underflows into the builder's own floor
/// of 0x10 — the *linear* curve, the one the object builds when nothing else asks for more, and
/// the one the voice arithmetic below is visible through.
fn module_with_master_volume(byte: u8) -> Module {
    let mut bytes = shipped("MUSIC0.S3M");
    bytes[0x33] = byte;
    let mut module = Module::load(&bytes).expect("MUSIC0 with one byte patched");
    module.apply_tempo_patch(0x78);
    module
}

/// The samples one voice plays, worked out by hand from the module's bytes: the table value at
/// the position, the position advanced by the object's 16.16 step, and the loop's rewrap —
/// `+0x3840`…`+0x385a` adds `m_loop - m_end` and keeps the overshoot, and a one-shot voice
/// stops at its end (`+0x3845`).
fn expected_values(
    bytes: &[u8],
    level: u8,
    step: u32,
    start: u32,
    end: u32,
    loop_start: Option<u32>,
    frames: usize,
) -> Vec<i32> {
    let mut position = start;
    let mut low: u16 = 0;
    let mut out = Vec::with_capacity(frames);
    for _ in 0..frames {
        if position >= end && loop_start.is_none() {
            out.push(0);
            continue;
        }
        let byte = bytes[position as usize];
        out.push(value(byte, level));
        let (next_low, carry) = low.overflowing_add((step & 0xffff) as u16);
        low = next_low;
        position += (step >> 16) + u32::from(carry);
        if position >= end {
            match loop_start {
                Some(loop_start) => position = loop_start + (position - end),
                None => position = end,
            }
        }
    }
    out
}

/// [`expected_values`] through a curve, as a **stereo** window: a frame's left sample is the
/// output sample `2 × frame`, and every frame — the first 32 frames' 64 samples included — is
/// read at [`BASE`], because the flush leaves no marker (A14(a)).
#[allow(clippy::too_many_arguments)]
fn expected_voice(
    mul: u32,
    bytes: &[u8],
    level: u8,
    step: u32,
    start: u32,
    end: u32,
    loop_start: Option<u32>,
    frames: usize,
) -> Vec<i16> {
    expected_values(bytes, level, step, start, end, loop_start, frames)
        .into_iter()
        .map(|value| curve_at(mul, BASE, value))
        .collect()
}

/// The 16.16 walk the mixer performs, to find the first frame that reads inside the loop
/// again: the frame after the one whose advance reached `m_end`, and the position the rewrap
/// left — `+0x3856`'s `m_loop - m_end` keeps the overshoot, so it need not land on `lbeg`.
fn wrap_frame(step: u32, end: u32, loop_start: u32) -> (usize, u32) {
    let mut position = 0u32;
    let mut low = 0u16;
    for frame in 0..1_000_000 {
        let (next_low, carry) = low.overflowing_add((step & 0xffff) as u16);
        low = next_low;
        position += (step >> 16) + u32::from(carry);
        if position >= end {
            position = loop_start + (position - end);
            return (frame + 1, position);
        }
    }
    panic!("the voice never reached its end");
}

/// A channel holding nothing but the three fields the mixer reads.
fn channel(period: u16, volume: u8, position: u32) -> Channel {
    Channel {
        period,
        volume,
        position,
        ..Channel::default()
    }
}

// -------------------------------------------------------------------------------------------
// 1. The arithmetic, hand-computed from the module's own bytes
// -------------------------------------------------------------------------------------------

#[test]
fn one_voice_is_the_modules_own_sample_bytes() {
    // MUSIC0 with its master-volume byte asking for the *linear* curve
    // (`module_with_master_volume`: 0x82 is stereo | 2, whose derivation underflows into the
    // builder's own floor of 0x10), its eleventh sample — a one-shot whose first bytes swing
    // widely, 115, 176, 162, 147, … — at level 63, playing C-4 at the format's reference period
    // 1712, mixed **mono** so that the drain is one i16 a frame and the whole mix is one voice.
    let module = module_with_master_volume(0x82);
    let bytes = module.sample_bytes(10).expect("MUSIC0's eleventh sample");
    let header = module.instrument(10).expect("its header");
    assert!(header.is_pcm() && !header.loops() && header.length > 512);
    assert_eq!(&bytes[..4], &[115, 176, 162, 147]);

    let level = 63u8;
    let period = 1712u16;
    let options = PlayerOptions {
        stereo: false,
        ..PlayerOptions::default()
    };
    let voice_step = step(period, 40000);

    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(period, level, 0);
    let mut mixer = Mixer::new_with_capacity(4096);
    mixer.start_note(0, &module, 11);

    let frames = 256;
    let mut out = vec![0i16; frames];
    mixer.mix(&module, &mut channels, &options, 64, out.len());
    assert_eq!(mixer.take(&mut out), out.len());

    let values = expected_values(
        bytes,
        level,
        voice_step,
        0,
        header.length,
        None,
        frames,
    );
    // One base for the whole mono window: the port's index does not move at the first chunk's
    // end (the flush leaves no marker; A14(a)), so this is the *same* curve for all 256 frames.
    let expected: Vec<i16> = values.iter().map(|value| curve_with(16, *value)).collect();
    let positions: Vec<usize> = {
        let mut position = 0u32;
        let mut low = 0u16;
        (0..frames)
            .map(|_| {
                let here = position as usize;
                let (next_low, carry) = low.overflowing_add((voice_step & 0xffff) as u16);
                low = next_low;
                position += (voice_step >> 16) + u32::from(carry);
                here
            })
            .collect()
    };
    for frame in 0..frames {
        assert_eq!(
            out[frame], expected[frame],
            "frame {frame}: byte {} at position {}",
            bytes[positions[frame]], positions[frame]
        );
    }

    // And the numbers written out, so the arithmetic can be checked without running anything.
    // At level 63 the scale is 255 and the centre is (128 × 255) >> 8 = 127, so the sample's
    // 115 is −13 and its 176 is +48; with the *linear* curve (`mul` 16 → `K` 2048 → no zero
    // fill and a step of 32) and the port's base, the byte is `(0x400 + value) / 8`: 1011 / 8 =
    // 126 and 1072 / 8 = 134, i.e. −512 and 1536 once the card's 128-centred byte is widened.
    println!(
        "MUSIC0 instrument 11, channel 0, level 63, period 1712, step {voice_step} (16.16), \
         mono, linear curve:\n  sample bytes {:?}\n  the first eight samples {:?}",
        positions[..8]
            .iter()
            .map(|p| bytes[*p])
            .collect::<Vec<u8>>(),
        &out[..8]
    );
    assert_eq!((115 * 255) >> 8, 114);
    assert_eq!((128 * 255) >> 8, 127);
    assert_eq!(value(115, 63), -13);
    assert_eq!(value(176, 63), 48);
    assert_eq!(curve_with(16, 48), 1536, "index 0x400 + 48 = 1072, the byte 134");
    assert_eq!(curve_with(16, -13), -512, "index 1011, the byte 126");
    // The misread marker's arithmetic, which neither the object nor the port has (A14(a)):
    // `0x501 = 1281`, so the same two values would read 1268 and 1329 — the linear curve's
    // bytes 158 and 166, i.e. 7680 and 9728. It is pinned here because `music.md` §10.4
    // quotes the two numbers; nothing the mixer produces is read at that index.
    assert_eq!(curve_at(16, BASE + MISREAD_MARKER_OFFSET, 48), 9728);
    assert_eq!(curve_at(16, BASE + MISREAD_MARKER_OFFSET, -13), 7680);
    // The sample's 115 is the value −13 → index 1011 → the byte 126 → −512; its 176 is +48 →
    // index 1072 → the byte 134 → 1536. The positions are 0 five times and then 1, so the first
    // five samples are the first and the next three the second.
    assert_eq!(
        &out[..8],
        &[-512, -512, -512, -512, -512, 1536, 1536, 1536]
    );
    assert_eq!(
        out[FIRST_CHUNK],
        curve_with(16, values[FIRST_CHUNK]),
        "and the sample after the first chunk is read at the same base as the first"
    );

    // The step is 14317056/1712 × 65536 / 40000 = 13700 in 16.16 — 0.209 of a sample byte a
    // frame — so 256 frames read 53 bytes of the sample: positions 0 four times, then 1.
    assert_eq!(voice_step, 13_700);
    assert_eq!(positions[..8], [0, 0, 0, 0, 0, 1, 1, 1]);
    assert_eq!(positions[255], 53);
}

#[test]
fn the_files_hand_arithmetic_is_the_builders_at_every_entry() {
    // `curve_at` is a second copy of `Curve::build`'s arithmetic — deliberately, so the
    // expected values are not computed by the code they check — and this is what keeps the
    // two copies from drifting apart silently. It cannot make the copy independent of a
    // *shared* misreading of `+0x559d`…`+0x55bd`; the probe's 768/1279/1280 measurement and
    // `mix::tests::the_curve_is_the_objects_own_table`'s literal entries are that guard.
    for mul in [16u32, 48, 64, 127] {
        let builder = Curve::build(mul as u8);
        for index in 0..0x800i32 {
            assert_eq!(
                curve_at(mul, BASE, index - BASE),
                builder.sample(index),
                "mul {mul}, table entry {index}: the hand copy and the builder"
            );
        }
    }
}

#[test]
fn a_solo_voice_is_audible_through_the_table_the_machine_holds() {
    // The same voice on the modules' *own* master-volume byte, 0xB0 → `mul` 64, whose table is
    // the one the machine holds: 768 zero entries, the ramp at 768…1279 and `0xFF` above. A
    // solo voice's value is at most ±127 — `(255 × 255) >> 8 = 254` against the centre's 127 —
    // and the port indexes at `0x400 + value`, so the voice lands inside the ramp and is
    // audible: the assertions below pin both ends of that swing.
    let module = module("MUSIC0");
    assert_eq!(module.master_volume(), 0xb0);
    assert_eq!(sr_audio::multiplier(module.master_volume(), true), 64);

    let bytes = module.sample_bytes(10).expect("MUSIC0's eleventh sample");
    let options = PlayerOptions::default();
    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(1712, 63, 0);
    let mut mixer = Mixer::new_with_capacity(1024);
    mixer.start_note(0, &module, 11);
    let frames = 256;
    let mut out = vec![0i16; frames * 2];
    mixer.mix(&module, &mut channels, &options, 64, out.len());
    assert_eq!(mixer.take(&mut out), out.len());

    // The machine's table at `mul` 64 is the halved-zero fill the builder's `rep stosw` bug
    // leaves: 768 zero entries, the ramp at 768…1279 (one level per two entries) and 0xFF
    // above. Silence is the ramp's centre — the byte 0x80, which is what the DMA buffer the
    // probe read is centred on — and a solo voice's ±127 of value moves it by ±63: audible,
    // neither gated nor clipped.
    assert_eq!(curve(0), 0, "the byte 128, the card's centre");
    assert_eq!(curve(127) / 256, 63, "the byte 191");
    assert_eq!(curve(-127) / 256, -64, "the byte 64, one level per two of value");
    let left: Vec<i16> = out.iter().step_by(2).copied().collect();
    let right: Vec<i16> = out.iter().skip(1).step_by(2).copied().collect();
    assert!(
        left.iter().any(|sample| *sample > 0) && left.iter().any(|sample| *sample < 0),
        "the voice swings both ways through the machine's ramp, on the left pan"
    );
    assert!(
        right.iter().all(|sample| *sample == 0),
        "and the right accumulator is empty, so its samples are the ramp's centre"
    );
    // The loudest byte the sample carries, at the top level, is still inside the ramp's own
    // range: a solo voice's value tops out at 127, well short of the +256 where the `0xFF`
    // tail starts.
    let loudest = bytes.iter().copied().max().unwrap();
    assert_eq!(value(loudest, 63), (i32::from(loudest) * 255 / 256) - 127);
    assert!(value(loudest, 63) < 255);
}

#[test]
fn a_mono_frame_is_the_whole_mix_in_one_i16() {
    // The object's mono path is one accumulator over all sixteen channels (`+0x39cd`), so a
    // hard-right channel is *not* lost — the pan is the stereo path's. Channel 1 of MUSIC0 is
    // the module's rightmost pan (its table entry is 8).
    let module = module_with_master_volume(0x82);
    let bytes = module.sample_bytes(0).expect("MUSIC0's first sample");
    let header = module.instrument(0).expect("MUSIC0's first header");
    let options = PlayerOptions {
        stereo: false,
        ..PlayerOptions::default()
    };
    let voice_step = step(1712, 40000);

    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(1712, 32, 0);
    channels[1] = channel(1712, 16, 0);
    let mut mixer = Mixer::new_with_capacity(4096);
    mixer.start_note(0, &module, 1);
    mixer.start_note(1, &module, 1);

    let mut out = vec![0i16; 256];
    mixer.mix(&module, &mut channels, &options, 64, out.len());
    assert_eq!(mixer.take(&mut out), out.len());

    let first = expected_values(
        bytes,
        32,
        voice_step,
        0,
        header.loop_end,
        Some(header.loop_begin),
        256,
    );
    let second = expected_values(
        bytes,
        16,
        voice_step,
        0,
        header.loop_end,
        Some(header.loop_begin),
        256,
    );
    println!(
        "mono: first {:?} second {:?} sums {:?} out {:?}",
        &first[..6],
        &second[..6],
        (0..6).map(|f| first[f] + second[f]).collect::<Vec<i32>>(),
        &out[..6]
    );
    for frame in 0..256 {
        assert_eq!(
            out[frame],
            curve_with(16, first[frame] + second[frame]),
            "frame {frame}: both channels through one curve, at the port's one base"
        );
    }
}

#[test]
fn a_looping_sample_rewraps_at_its_loop_and_a_one_shot_stops() {
    // The two end paths of `+0x3834`…`+0x385a`, on the modules' own instruments: MUSIC0's
    // first sample loops (its header's flags bit 0 is set and its loop begins at 5720), and a
    // sample that does not loop stops dead at its length (`+0x48be`'s `m_loop` = 0xffff, then
    // `+0x3845`'s `m_speed = 0`).
    // The module whose master-volume byte asks for the linear curve: the modules' own byte
    // gives `mul` 64, whose one-level-per-two table hides a value's low bit (the test above),
    // and this test is about the *bounds*, so the values have to be visible.
    let module = module_with_master_volume(0x82);
    let options = PlayerOptions::default();
    let rate = STEREO_DIVISOR;

    let looping = module.instrument(0).expect("MUSIC0's first header").clone();
    assert!(looping.loops() && looping.loop_begin > 0);

    // The case the review caught: a full-sample loop, `lbeg` 0 with `lend` = the length. The
    // object wraps these at the sample's end back to 0 (`m_loop` = the header's `lbeg`, which
    // is 0) — the reading that tested `lbeg != 0` played them once and stopped.
    let full_loop_index = (0..32)
        .find(|index| {
            module.instrument(*index).is_some_and(|header| {
                header.is_pcm()
                    && header.loops()
                    && header.loop_begin == 0
                    && header.loop_end > header.loop_begin
            })
        })
        .expect("MUSIC0 carries full-sample loops");
    let full_loop = module
        .instrument(full_loop_index)
        .expect("the header")
        .clone();
    assert_eq!(full_loop.loop_begin, 0);
    assert_eq!(full_loop.loop_end, full_loop.length);

    let one_shot_index = (0..32)
        .find(|index| {
            module
                .instrument(*index)
                .is_some_and(|header| header.is_pcm() && !header.loops())
        })
        .expect("MUSIC0 carries one-shot samples");
    let one_shot = module
        .instrument(one_shot_index)
        .expect("the header")
        .clone();
    assert!(!one_shot.loops());

    // A period low enough that a frame's step is a whole byte: 14317056/period over 21111 is
    // at least 1 only for periods below about 678.
    let period = 428u16;
    let voice_step = step(period, rate);
    assert_eq!(voice_step >> 16, 1);

    for (index, header, loop_start) in [
        (1u8, looping.clone(), Some(looping.loop_begin)),
        (
            full_loop_index as u8 + 1,
            full_loop.clone(),
            Some(full_loop.loop_begin),
        ),
        (one_shot_index as u8 + 1, one_shot.clone(), None::<u32>),
    ] {
        let bytes = module
            .sample_bytes(usize::from(index) - 1)
            .expect("the sample");
        let mut channels = [Channel::default(); CHANNELS];
        channels[0] = channel(period, 63, 0);
        let mut mixer = Mixer::new_with_capacity(1 << 17);
        mixer.start_note(0, &module, index);
        // A window long enough to pass the sample's end either way, so both end paths are
        // reached inside it: `end` bytes of sample at one byte a frame, plus the rewrap.
        let end = if loop_start.is_some() {
            header.loop_end
        } else {
            header.length
        };
        let frames = end as usize + 16;
        mixer.mix(&module, &mut channels, &options, 64, frames * 2);
        let mut out = vec![0i16; frames * 2];
        assert_eq!(mixer.take(&mut out), frames * 2);

        let expected = expected_voice(
            16,
            bytes,
            63,
            voice_step,
            0,
            end,
            loop_start.map(|l| l.min(0xffff)),
            frames,
        );
        for frame in 0..frames {
            assert_eq!(
                out[frame * 2],
                expected[frame],
                "frame {frame}, instrument {index}"
            );
        }
        if loop_start.is_none() {
            // Past its length the voice is stopped and stays stopped: `+0x384d`'s `m_speed = 0`.
            // The frame it stops on is the by-hand walk's, not an assumption about the step.
            // The stop is the last frame the by-hand model *plays*: the curve's own zero entry
            // is not enough to find it, because a playing frame whose value is small maps to
            // the same entry.
            let zero = curve_with(16, 0);
            let stop = expected
                .iter()
                .rposition(|sample| *sample != zero)
                .map(|last| last + 1)
                .unwrap_or_else(|| {
                    panic!("instrument {index}: the one-shot stopped inside the window")
                });
            assert!(stop < frames, "instrument {index}: it stopped inside the window");
            assert!(
                expected[stop..].iter().all(|sample| *sample == zero),
                "the by-hand model never plays again"
            );
            assert!(
                out[stop * 2..].iter().all(|sample| *sample == zero),
                "and every frame the port produced after the end is the curve's zero entry"
            );
        } else {
            // A looping voice never stops: the loop rewraps it while the window lasts, and for
            // the full-sample loop the frame after the sample's end is the sample's own first
            // byte again — `m_loop` is 0, so the rewrap goes back to the start.
            // The loop never stops: the by-hand sequence above covers frames past the sample's
            // end, so the equality already pins the rewrap rather than a stop. What is left is
            // to say *where* the wrap lands: the position reaches `m_end` on an advance, and
            // `+0x3856` adds `m_loop - m_end` to it, overshoot and all.
            let (after, position) = wrap_frame(voice_step, end, header.loop_begin);
            assert!(
                out[..].iter().any(|sample| *sample != 0),
                "the loop kept producing samples"
            );
            assert!(
                (header.loop_begin..end).contains(&position),
                "instrument {index}: the rewrapped position {position} is inside the loop"
            );
            assert_eq!(
                out[after * 2],
                curve_with(16, value(bytes[position as usize], 63)),
                "instrument {index}: the first frame after the rewrap reads the loop"
            );
        }
    }
}

// -------------------------------------------------------------------------------------------
// 2. The stream's properties, through the player
// -------------------------------------------------------------------------------------------

/// One poll's worth of the drain, as the engine would hand it to a device.
fn poll_and_drain(player: &mut Player) -> Vec<i16> {
    player.poll();
    let mut out = vec![0i16; player.pending_samples()];
    let count = player.take_samples(&mut out);
    assert_eq!(count, out.len(), "the drain takes what the poll produced");
    out
}

#[test]
fn a_poll_produces_exactly_the_samples_it_says_it_does() {
    // Task 3's seam: `samples_this_poll` is what the mixer must produce, no more and no less —
    // the five counts Task 3 pinned, 570/571/571/570/571. A stereo budget that is odd splits
    // one frame across two polls, which is what a byte-stream DMA does at a buffer boundary:
    // the held sample goes out at the head of the next poll, so every poll's drain is exactly
    // its own count.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(0);
    let mut counts = Vec::new();
    for _ in 0..5 {
        player.poll();
        let before = player.samples_this_poll() as usize;
        assert_eq!(
            player.pending_samples(),
            before,
            "the poll mixed its own count"
        );
        let mut out = vec![0i16; before];
        assert_eq!(player.take_samples(&mut out), before);
        counts.push(before);
    }
    println!("the first five polls' sample counts: {counts:?}");
    assert_eq!(counts, vec![570, 571, 571, 570, 571]);
}

#[test]
fn the_samples_before_a_tick_are_mixed_before_it_runs() {
    // **`Player::poll`'s documented order**, the object's `zpollme` (`player.rs:457-461`):
    // the samples before a tick are mixed before that tick runs. A row's first tick is where
    // its cells load and its note starts, so on a seek the samples *before* the first tick
    // must still be the silence the reset left, and the note must first sound in the samples
    // after it. Mixing the chunk after the tick it ends on would put the note one chunk
    // early — this test's name and its failure messages are both about that order.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(1); // order 1: pattern 1 row 0 carries `O05` + note 0x30 on channel 2
    let mut samples: Vec<i16> = Vec::new();
    let mut counts: Vec<usize> = Vec::new();
    let mut before_tick = None;
    while player.zframe() < 3 {
        player.poll();
        let count = player.samples_this_poll() as usize;
        counts.push(count);
        let mut out = vec![0i16; player.pending_samples()];
        let taken = player.take_samples(&mut out);
        assert_eq!(taken, out.len(), "the drain takes what the poll produced");
        samples.extend_from_slice(&out[..taken]);
        if player.zframe() > 0 && before_tick.is_none() {
            // The first tick ran inside this poll. No poll here spans a whole tick — the
            // counts asserted below are all under the 833 samples a tick holds at tempo
            // 120 — so every sample this poll mixed came before the tick.
            before_tick = Some(samples.len());
        }
    }
    assert_eq!(&counts[..2], &[570, 571], "the fixture's first two polls");
    assert!(
        counts.iter().all(|count| *count < 833),
        "no poll spans a whole tick, so the boundary below is exact: {counts:?}"
    );
    let before_tick = before_tick.expect("the first tick ran in the window");
    assert_eq!(
        before_tick, 1141,
        "the first tick's sample boundary in this fixture (570 + 571)"
    );
    assert!(
        samples[..before_tick].iter().all(|&sample| sample == 0),
        "the samples before the first tick are silent: they are mixed before the row's cells \
         load, which is `Player::poll`'s documented order (the samples before a tick are mixed \
         before that tick runs)"
    );
    assert!(
        samples[before_tick..].iter().any(|&sample| sample != 0),
        "the note the row starts is audible only in the samples after that tick"
    );
}

#[test]
fn the_drain_moves_no_engine_state() {
    // `take_samples` is the device's drain and nothing more: the position the parts read is
    // the engine's (`MAIN/STMIK.H:100-103`), so draining does not touch it.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(0);
    for _ in 0..40 {
        player.poll();
    }
    let position = player.engine_position();
    let published = player.state();
    let zframe = player.zframe();
    let pending = player.pending_samples();
    assert!(pending > 0, "there is something to drain");

    let mut out = vec![0i16; pending];
    assert_eq!(player.take_samples(&mut out), pending);
    assert_eq!(player.pending_samples(), 0);
    assert_eq!(player.engine_position(), position);
    assert_eq!(player.state(), published);
    assert_eq!(player.zframe(), zframe);
    // And a second drain has nothing to give.
    assert_eq!(player.take_samples(&mut out), 0);
}

#[test]
fn two_runs_of_a_window_are_byte_identical() {
    // Determinism, asserted as its own property: the same module, the same window of polls,
    // the same bytes. Nothing in the mixer reads a clock, a random source or the machine.
    let window = |order: u8| {
        let mut player = Player::new(module("MUSIC1"), PlayerOptions::default());
        player.play_song(order);
        let mut samples = Vec::new();
        for _ in 0..600 {
            samples.extend(poll_and_drain(&mut player));
        }
        samples
    };
    let first = window(0);
    let second = window(0);
    assert_eq!(first.len(), second.len(), "the same number of samples");
    println!("a 600-poll window of MUSIC1: {} samples", first.len());
    assert_eq!(
        first, second,
        "two runs of the same window are the same bytes"
    );
    // And a different window is a different stream, so the equality is not vacuous.
    let other = window(1);
    assert_ne!(first, other, "another order is another stream");
}

#[test]
fn the_window_is_not_silent() {
    // A mixer that produces the curve's own silence for every sample passes every "does it
    // run" test, so this one asks for signal: how many samples are *not* the silence entry,
    // how many distinct levels the window reaches, and what its extremes are. Two things are
    // stated rather than assumed: the table the machine holds is not a gate — a value of ±128
    // moves the byte ±64 — and the silence entry is the ramp's centre, the byte 0x80, i.e. 0,
    // because the port indexes at `0x400` for every sample.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(0);
    let mut samples = Vec::new();
    for _ in 0..120 {
        samples.extend(poll_and_drain(&mut player));
    }
    let silence = curve(0);
    let silent = samples.iter().filter(|sample| **sample == silence).count();
    let loudest = samples.iter().copied().max().expect("a window");
    let quietest = samples.iter().copied().min().expect("a window");
    let distinct = samples
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<i16>>()
        .len();
    println!(
        "MUSIC0's first 120 polls: {} samples, {silent} of them the curve's silence ({}), \
         {distinct} distinct values, quietest {quietest}, loudest {loudest}",
        samples.len(),
        silence
    );
    assert_eq!(
        samples[..64],
        [curve_with(64, 0); 64],
        "the first chunk is the ramp's centre too: no marker moves the base"
    );

    // The window is extended past the intro because MUSIC0 starts quietly; what the
    // assertion needs is that the mixer is not producing a constant, and over the longer
    // window it plainly is not.
    for _ in 0..4000 {
        samples.extend(poll_and_drain(&mut player));
    }
    let above = samples.iter().filter(|sample| **sample > silence).count();
    let peak = samples.iter().copied().max().expect("a window");
    let distinct = samples
        .iter()
        .copied()
        .collect::<std::collections::BTreeSet<i16>>()
        .len();
    println!(
        "MUSIC0's first 4120 polls ({} samples): {above} above the silence entry, peak {peak}, \
         {distinct} distinct values",
        samples.len()
    );
    assert!(distinct > 16, "the ramp is reached: {distinct} distinct values");
    assert!(
        samples.iter().any(|sample| *sample > silence),
        "and the window reaches above the ramp's centre"
    );
    assert!(
        samples.iter().any(|sample| *sample < silence),
        "and below it"
    );
}

#[test]
fn a_window_of_rows_carries_its_samples() {
    // The audio-side counterpart of Task 3's position test: over `rows` rows the mixed sample
    // count is `rows × speed × mix_rate × 2.5 / tempo`, within one tick's worth. A mixer that
    // drops or duplicates a tick's samples is what this sees.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(0);
    let samples_per_tick = u64::from(MixRate::Hz40000.samples_per_second()) * 5
        / (2 * u64::from(player.state().tempo));
    assert_eq!(
        samples_per_tick, 833,
        "a tick at tempo 120 and 40000 a second"
    );

    let mut samples = 0u64;
    let mut rows = 0u64;
    let mut last = player.engine_position().row;
    for _ in 0..4000 {
        samples += poll_and_drain(&mut player).len() as u64;
        let position = player.engine_position();
        if position.row != last {
            last = position.row;
            rows += 1;
            if rows == 64 {
                break;
            }
        }
    }
    let speed = u64::from(player.state().speed);
    let predicted = rows * speed * samples_per_tick;
    let difference = samples.abs_diff(predicted);
    println!(
        "{rows} rows at speed {speed}: {samples} samples, predicted {predicted}, difference \
         {difference} ({:.4} %), one tick is {samples_per_tick}",
        100.0 * difference as f64 / predicted as f64
    );
    assert!(
        difference < samples_per_tick,
        "{samples} samples over {rows} rows is more than a tick from {predicted}"
    );
}

#[test]
fn a_device_that_stops_draining_loses_the_oldest_samples() {
    // The overflow policy, through the player: the engine never waits (a poll is one 70.086 Hz
    // frame of the demo's timeline), so a device that falls behind loses the *oldest* samples.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(0);
    assert_eq!(player.dropped_samples(), 0);

    let mut polls = 0;
    while player.dropped_samples() == 0 {
        player.poll();
        polls += 1;
        assert!(polls < 1000, "the ring never filled");
    }
    let dropped = player.dropped_samples();
    println!(
        "after {polls} undrained polls the ring holds {} samples and has dropped {dropped}",
        player.pending_samples()
    );
    assert_eq!(player.pending_samples(), sr_audio::RING_SAMPLES);
    assert!(dropped > 0);

    // What is left is the *newest* samples: the ring holds the last `RING_SAMPLES` the engine
    // produced, and the samples the first polls made are among the dropped ones.
    let mut out = vec![0i16; 4];
    assert_eq!(player.take_samples(&mut out), 4);
    let engine = player.engine_position();
    let dropped_before = player.dropped_samples();
    player.poll();
    assert_eq!(player.engine_position(), engine, "a poll advances nothing here");
    assert!(
        player.dropped_samples() > dropped_before,
        "the mixer is still producing samples while the device is behind"
    );
    assert_eq!(player.pending_samples(), sr_audio::RING_SAMPLES);

    // And a drained ring is empty: the policy costs samples, never the drain's accounting.
    let mut out = vec![0i16; sr_audio::RING_SAMPLES];
    let count = player.take_samples(&mut out);
    assert_eq!(count, sr_audio::RING_SAMPLES);
    assert_eq!(player.pending_samples(), 0);
}

#[test]
fn a_note_off_leaves_the_voice_silent() {
    // `0xfe` parks the position at 1 and the volume at 0 (`effects.rs`'s note start), and the
    // mixer then plays the sample at the centre — silence at level 0 whatever the byte is. The
    // master-volume byte is the one that asks for the linear curve, so a level of 0 reads the
    // curve's own entry for 0 — the byte 128, i.e. 0 — and not the misread marker's byte 160
    // (8192), which the flush never writes (A14(a)); the counterfactual below pins that.
    let module = module_with_master_volume(0x82);
    let options = PlayerOptions::default();
    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(1712, 0, 1);
    let mut mixer = Mixer::new_with_capacity(1024);
    mixer.start_note(0, &module, 1);
    let mut out = vec![0i16; 128];
    mixer.mix(&module, &mut channels, &options, 64, out.len());
    assert_eq!(mixer.take(&mut out), 128);
    // A level of 0 makes every sample's value 0, so the samples are the table's own silence
    // entry — the byte 128, i.e. 0 — and never the note. (A misread marker's `0x501` would
    // read the byte 160, i.e. 8192; the flush writes zero: A14(a).)
    assert_eq!(curve_with(16, 0), 0, "the curve's centre, the byte 128");
    assert_eq!(
        curve_at(16, BASE + MISREAD_MARKER_OFFSET, 0),
        8192,
        "the misread marker's byte 160"
    );
    assert!(out.iter().all(|sample| *sample == 0), "a level of 0 is the byte 128");
}

#[test]
fn the_modules_loops_and_the_objects_zero_loop_end_rule() {
    // `+0x4890` reads the header's **`lend`** (`es:0x18`) into `ax` and `+0x489c` sends it to
    // the one-shot path when it is **0**; `+0x48a6` puts **`lbeg`** (`es:0x14`) into `m_loop`.
    // With the loader's own rule that clears the loop flag when `lend <= lbeg`
    // (`STARTEMS.C:145`), a sample loops exactly when its flag is set and its loop end is past
    // its loop begin — and the case that separates that rule from "the loop must not begin at
    // 0" is not hypothetical: **five of MUSIC0's twelve looping samples and one of MUSIC1's
    // nine have `lbeg` 0 with `lend` = the length**, a full-sample loop the object wraps at the
    // sample's end back to 0.
    for (name, looping, zero_begin, one_shot) in
        [("MUSIC0", 12, 5, 10), ("MUSIC1", 9, 1, 32)]
    {
        let module = module(name);
        let mut loops = 0;
        let mut begins_at_zero = 0;
        let mut one_shots = 0;
        let mut zero_ends = 0;
        for index in 0..module.instrument_count() {
            let Some(header) = module.instrument(index) else {
                continue;
            };
            if !header.is_pcm() {
                continue;
            }
            if header.loops() && header.loop_end > header.loop_begin {
                loops += 1;
                if header.loop_begin == 0 {
                    begins_at_zero += 1;
                    assert_eq!(
                        header.loop_end, header.length,
                        "{name} instrument {}: a full-sample loop",
                        index + 1
                    );
                }
            } else {
                one_shots += 1;
                if header.loops() {
                    zero_ends += 1;
                }
            }
        }
        println!(
            "{name}: {loops} samples loop, {begins_at_zero} of them from 0 to the sample's end; \
             {one_shots} do not loop, {zero_ends} of them with the flag set but no loop"
        );
        assert_eq!((loops, begins_at_zero, one_shots), (looping, zero_begin, one_shot));
        // No sample in either file has the flag set with a zero loop *end*, so `+0x489c`'s test
        // is a rule the data does not reach — it is the loader's `lend <= lbeg` clearing that
        // the shipped files exercise.
        assert_eq!(zero_ends, 0, "{name}: no zero-lend sample, so +0x489c is unreached");
    }
}

/// FNV-1a over the stream's little-endian bytes: the golden's digest. Written out here so the
/// pin needs no crate (`sr-audio` has no dependencies, dev ones included).
fn fnv1a(hash: u64, samples: &[i16]) -> u64 {
    samples.iter().flat_map(|sample| sample.to_le_bytes()).fold(hash, |hash, byte| {
        (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

#[test]
fn the_authentic_stream_is_pinned_byte_for_byte() {
    // **The authentic path's whole stream, not one frame of it.** The hand-computed tests above
    // pin individual frames; this pins every sample of 12,000 polls (171 s) from four entry
    // orders, so an edit to `Mixer::mix`'s shared branches that moves the authentic output
    // fails here even when every frame test still passes.
    //
    // The four digests are the stream **before the enhanced renderer existed**: recomputed by
    // this same loop on a clean extract of commit 8004905 (D43). The default options are the
    // authentic ones, so this is also what every run that does not ask for `--sound-quality
    // enhanced` plays. The four runs are independent, so they run on four threads.
    const POLLS: usize = 12_000;
    let runs = [
        ("MUSIC0", 0u8, 0xc864_588f_c763_f410u64),
        ("MUSIC0", 25, 0xcc33_2ccc_6fed_136a),
        ("MUSIC1", 0, 0x4872_e144_c499_48cf),
        ("MUSIC1", 42, 0x49ed_b4f0_c2f1_89e2),
    ];
    let digests: Vec<(u64, u64)> = std::thread::scope(|scope| {
        let handles: Vec<_> = runs
            .iter()
            .map(|&(name, order, _)| {
                scope.spawn(move || {
                    let mut player = Player::new(module(name), PlayerOptions::default());
                    player.play_song(order);
                    let (mut hash, mut samples) = (0xcbf2_9ce4_8422_2325u64, 0u64);
                    let mut out = vec![0i16; 4096];
                    for _ in 0..POLLS {
                        player.poll();
                        let taken = player.take_samples(&mut out);
                        hash = fnv1a(hash, &out[..taken]);
                        samples += taken as u64;
                    }
                    (samples, hash)
                })
            })
            .collect();
        handles.into_iter().map(|handle| handle.join().expect("a run")).collect()
    });
    for (&(name, order, golden), &(samples, hash)) in runs.iter().zip(&digests) {
        println!("{name}@{order}: {POLLS} polls, {samples} samples, FNV-1a {hash:016x}");
        assert_eq!(samples, 6_848_728, "{name}@{order}: 12,000 polls of 40000/70.086 samples");
        assert_eq!(hash, golden, "{name}@{order}: the authentic stream moved");
    }
}
