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

use sr_audio::{Channel, MixRate, Mixer, Module, Player, PlayerOptions, CHANNELS};

const ASSETS: &str = "../../assets/MAIN";

fn shipped(name: &str) -> Vec<u8> {
    let path = Path::new(ASSETS).join(name);
    std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// A module with the shell's patch applied where the shell applies it: MUSIC0 plays at
/// tempo 120 (`MAIN/STARTMUS.C:252-265`), MUSIC1 at its file's own 130 — as `tests/player.rs`
/// and `tests/effects.rs` load them.
fn module(name: &'static str) -> Module {
    let mut module = Module::load(&shipped(&format!("{name}.S3M")))
        .unwrap_or_else(|e| panic!("{name} does not load: {e}"));
    if name == "MUSIC0" {
        module.apply_tempo_patch(0x78);
    }
    module
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

/// The port's conversion of that difference to `i16` (`mix.rs`'s `widen`): the object's own
/// full scale is ±1024 and the shift takes it to ±32768.
fn widen(value: i32) -> i16 {
    (value * 32).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

/// The samples one voice plays, worked out by hand from the module's bytes: the table value at
/// the position, the position advanced by the object's 16.16 step, and the loop's rewrap —
/// `+0x3840`…`+0x385a` adds `m_loop - m_end` and keeps the overshoot, and a one-shot voice
/// stops at its end (`+0x3845`).
fn expected_voice(
    bytes: &[u8],
    level: u8,
    step: u32,
    start: u32,
    end: u32,
    loop_start: Option<u32>,
    frames: usize,
) -> Vec<i16> {
    let mut position = start;
    let mut low: u16 = 0;
    let mut out = Vec::with_capacity(frames);
    for _ in 0..frames {
        if position >= end && loop_start.is_none() {
            out.push(0);
            continue;
        }
        let byte = bytes[position as usize];
        out.push(widen(value(byte, level)));
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
    // MUSIC0, instrument 1 (the file's first PCM header), channel 0 — whose pan entry is 0,
    // the module's own table (`channel_pan`), so its samples land in the left i16.
    //
    // The note is C-4 at the format's reference: period 1712, which is what a note at
    // `c2spd` 8363 leaves (`effects.rs`'s `scaled_period`). The level is 32 — the middle of
    // the range, scale 129, so a silent sample's centre is 64 and the table's values run 0…129.
    let module = module("MUSIC0");
    let bytes = module.sample_bytes(0).expect("MUSIC0's first sample");
    let header = module.instrument(0).expect("MUSIC0's first header");
    assert!(header.is_pcm() && header.loops(), "the first sample loops");

    let level = 32u8;
    let period = 1712u16;
    let options = PlayerOptions::default();
    let voice_step = step(period, STEREO_DIVISOR);

    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(period, level, 0);
    let mut mixer = Mixer::new_with_capacity(4096);
    mixer.start_note(0, &module, 1);

    // One tick's worth at tempo 120: 833 samples, i.e. 833 i16 = 416 stereo frames and a half.
    let mut out = vec![0i16; 833];
    mixer.mix(&module, &mut channels, &options, 64, false, out.len());
    assert_eq!(mixer.take(&mut out), out.len());

    let expected = expected_voice(
        bytes,
        level,
        voice_step,
        0,
        header.loop_end,
        Some(header.loop_begin),
        417,
    );
    for frame in 0..416 {
        assert_eq!(
            out[frame * 2],
            expected[frame],
            "frame {frame}, left: byte {} at the position the 16.16 step reaches",
            bytes[((frame as u64 * u64::from(voice_step)) >> 16) as usize]
        );
        assert_eq!(
            out[frame * 2 + 1],
            0,
            "frame {frame}, right: channel 0 is the module's leftmost pan"
        );
    }

    // And the numbers themselves, printed so the note can quote them. The module's first four
    // sample bytes are 127, 128, 128, 128 and the positions 0, 0, 0, 1: at level 32 the scale
    // is 129 and the centre is (128 × 129) >> 8 = 64, so a byte of 127 is one below it and a
    // byte of 128 is exactly it — the first three frames are −1 × 32 and the fourth is 0.
    let bytes_here: Vec<u8> = (0..8)
        .map(|frame| bytes[((frame as u64 * u64::from(voice_step)) >> 16) as usize])
        .collect();
    println!(
        "MUSIC0 instrument 1, channel 0, level 32, period 1712, step {voice_step} (16.16):\n  \
         sample bytes {:?}\n  the first eight left samples {:?}",
        bytes_here,
        &expected[..8]
    );
    assert_eq!(
        &bytes[..4],
        &[127, 128, 128, 128],
        "MUSIC0's first sample bytes"
    );
    assert_eq!(SCALE[32], 129);
    assert_eq!((128 * 129) >> 8, 64, "the level's centre");
    assert_eq!(&bytes_here[..8], &[127, 127, 127, 128, 128, 128, 128, 128]);
    assert_eq!(
        &expected[..4],
        &[-32, -32, -32, 0],
        "the hand-computed frames"
    );
    for (frame, byte) in bytes_here.iter().enumerate() {
        assert_eq!(
            expected[frame],
            widen(value(*byte, level)),
            "frame {frame}: byte {byte} at level {level}"
        );
    }

    // The step is 8362 × 65536 / 21111 = 25958 in 16.16: 0.396 of a sample byte a frame, so
    // 416 frames read 164 bytes of the sample — the same bytes the object would read.
    assert_eq!(voice_step, 25958);
    assert_eq!((416 * u64::from(voice_step)) >> 16, 164);
}

#[test]
fn a_mono_frame_is_the_whole_mix_in_one_i16() {
    // The object's mono path is one accumulator over all sixteen channels (`+0x39cd`), so a
    // hard-right channel is *not* lost — the pan is the stereo path's. Channel 1 of MUSIC0 is
    // the module's rightmost pan (its table entry is 8).
    let module = module("MUSIC0");
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
    mixer.mix(&module, &mut channels, &options, 64, false, out.len());
    assert_eq!(mixer.take(&mut out), out.len());

    let first = expected_voice(
        bytes,
        32,
        voice_step,
        0,
        header.loop_end,
        Some(header.loop_begin),
        256,
    );
    let second = expected_voice(
        bytes,
        16,
        voice_step,
        0,
        header.loop_end,
        Some(header.loop_begin),
        256,
    );
    for frame in 0..256 {
        assert_eq!(
            out[frame],
            widen((i32::from(first[frame]) + i32::from(second[frame])) / 32),
            "frame {frame}: both channels, one i16"
        );
    }
}

#[test]
fn a_looping_sample_rewraps_at_its_loop_and_a_one_shot_stops() {
    // The two end paths of `+0x3834`…`+0x385a`, on the modules' own instruments: MUSIC0's
    // first sample loops (its header's flags bit 0 is set and its loop begins at 5720), and a
    // sample that does not loop stops dead at its length (`+0x48be`'s `m_loop` = 0xffff, then
    // `+0x3845`'s `m_speed = 0`).
    let module = module("MUSIC0");
    let options = PlayerOptions::default();
    let rate = STEREO_DIVISOR;

    let looping = module.instrument(0).expect("MUSIC0's first header").clone();
    assert!(looping.loops() && looping.loop_begin > 0);

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
        mixer.mix(&module, &mut channels, &options, 64, false, frames * 2);
        let mut out = vec![0i16; frames * 2];
        assert_eq!(mixer.take(&mut out), frames * 2);

        let expected = expected_voice(
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
            let stop = header.length as usize / (voice_step >> 16) as usize + 1;
            assert!(stop < frames, "the one-shot stopped inside the window");
            assert!(
                out[stop * 2..].iter().all(|sample| *sample == 0),
                "every frame after the end is silent"
            );
        } else {
            // A looping voice never stops: the loop rewraps it while the window lasts.
            assert!(
                out[..].iter().any(|sample| *sample != 0),
                "the loop kept producing samples"
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
    // A mixer that produces zeros passes every "does it run" test, so this one asks for
    // signal: how many samples are not zero, how far from zero the furthest is, and that the
    // first note's attack is audible.
    let mut player = Player::new(module("MUSIC0"), PlayerOptions::default());
    player.play_song(0);
    let mut samples = Vec::new();
    for _ in 0..120 {
        samples.extend(poll_and_drain(&mut player));
    }
    let nonzero = samples.iter().filter(|sample| **sample != 0).count();
    let peak = samples
        .iter()
        .map(|sample| i32::from(*sample).abs())
        .max()
        .expect("a window");
    println!(
        "MUSIC0's first 120 polls: {} samples, {nonzero} of them not zero, peak {peak}",
        samples.len()
    );
    assert!(
        nonzero > samples.len() / 4,
        "{nonzero} of {} are not zero",
        samples.len()
    );
    assert!(peak > 1000, "the peak is {peak}");
    assert!(
        samples.iter().all(|sample| *sample != i16::MIN),
        "nothing pinned to the bottom of the range"
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
    let frame_before = player.last_mixed_frame();
    player.poll();
    assert_eq!(player.engine_position(), engine);
    assert_ne!(
        player.last_mixed_frame(),
        frame_before,
        "the mixer is still producing frames while the device is behind"
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
    // mixer then plays the sample at the centre — silence at level 0 whatever the byte is.
    let module = module("MUSIC0");
    let options = PlayerOptions::default();
    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(1712, 0, 1);
    let mut mixer = Mixer::new_with_capacity(1024);
    mixer.start_note(0, &module, 1);
    let mut out = vec![0i16; 128];
    mixer.mix(&module, &mut channels, &options, 64, false, out.len());
    assert_eq!(mixer.take(&mut out), 128);
    assert!(
        out.iter().all(|sample| *sample == 0),
        "a level of 0 is silence for every sample byte"
    );
}

#[test]
fn shutting_up_silences_the_voices_and_a_row_starts_them_again() {
    // `_zshutup` (`MAIN/STMIK.H:39`) and the "volume 0" path's channel-freeing: the player's
    // `silenced` flag is what the mixer reads, and a row's first tick clears it.
    let module = module("MUSIC0");
    let options = PlayerOptions::default();
    let mut channels = [Channel::default(); CHANNELS];
    channels[0] = channel(1712, 63, 0);
    let mut mixer = Mixer::new_with_capacity(1024);
    mixer.start_note(0, &module, 1);

    let mut loud = vec![0i16; 256];
    mixer.mix(&module, &mut channels, &options, 64, false, loud.len());
    assert_eq!(mixer.take(&mut loud), loud.len());
    assert!(loud.iter().any(|sample| *sample != 0), "it was playing");

    let mut quiet = vec![0i16; 256];
    mixer.mix(&module, &mut channels, &options, 64, true, quiet.len());
    assert_eq!(mixer.take(&mut quiet), quiet.len());
    assert!(
        quiet.iter().all(|sample| *sample == 0),
        "shut up is silent at any volume"
    );
}

#[test]
fn the_modules_looping_samples_and_the_objects_zero_loop_rule() {
    // `+0x489c`: a sample whose loop **begins at 0** takes the one-shot path even when its
    // header's loop flag is set — `cmp $0x0,%ax; je` sends it to `+0x48be` — so the object
    // plays it once and stops. The port's `start_note` keeps that rule, and this counts what
    // it means for the two shipped files.
    for name in ["MUSIC0", "MUSIC1"] {
        let module = module(name);
        let mut looping = 0;
        let mut zero_begin = 0;
        let mut one_shot = 0;
        for index in 0..module.instrument_count() {
            let Some(header) = module.instrument(index) else {
                continue;
            };
            if !header.is_pcm() {
                continue;
            }
            if !header.loops() {
                one_shot += 1;
            } else if header.loop_begin == 0 {
                zero_begin += 1;
            } else {
                looping += 1;
            }
        }
        println!(
            "{name}: {looping} samples loop from a nonzero offset, {zero_begin} loop from 0 \
             (played as one-shots), {one_shot} do not loop"
        );
        assert_eq!(
            (looping, zero_begin, one_shot),
            if name == "MUSIC0" {
                (7, 5, 10)
            } else {
                (8, 1, 32)
            },
            "{name}'s sample headers"
        );
        // Which is Task 2's own census, split: 7 + 5 = 12 looping samples in MUSIC0 and
        // 8 + 1 = 9 in MUSIC1 (`tests/module.rs`'s flag counts), of which the zero-begin ones
        // the object plays as one-shots.
        assert_eq!(looping + zero_begin, if name == "MUSIC0" { 12 } else { 9 });
    }
}
