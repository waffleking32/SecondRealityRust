//! The enhanced sound option (Phase 9 Task 7): **the port's own, not the original's, and off by
//! default** (`docs/fidelity/music.md` §13, D43).
//!
//! The enhanced path renders the same voices at 48,000 frames a second — 12/5 of the authentic
//! stereo path's 20,000 — interpolating between sample bytes and converting the sum with the
//! authentic curve's small-signal gain. What these tests hold it to is the authentic path, which
//! is the reference:
//!
//! - **the clock**: the tick schedule, and so every published position, is the authentic one;
//! - **the count**: every authentic frame becomes 2 or 3 output frames, by an exact 12/5
//!   accumulator, so the stream never drifts against the picture;
//! - **the pitch**: a sustained note's frequency is the authentic one (the 5.26 %-flat stereo
//!   divisor included, `music.md` §10.2) — interpolation changes the waveform, not the step;
//! - **the level**: the RMS over ten seconds is the authentic one.
//!
//! Every number this file prints is printed by
//! `cargo test -p sr-audio --test enhanced -- --nocapture`.

use std::path::Path;

use sr_audio::{
    note_period, scaled_period, Channel, Mixer, Module, Player, PlayerOptions, Quality, CHANNELS,
};

const ASSETS: &str = "../../assets/MAIN";

/// A module with the shell's patch applied where the shell applies it: MUSIC0 plays at tempo
/// 120 (`MAIN/STARTMUS.C:252-265`), MUSIC1 at its file's own 130 — as `tests/mix.rs`,
/// `tests/player.rs` and `tests/effects.rs` load them.
fn module(name: &'static str) -> Module {
    let path = Path::new(ASSETS).join(format!("{name}.S3M"));
    let bytes =
        std::fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
    let mut module = Module::load(&bytes).unwrap_or_else(|e| panic!("{name} does not load: {e}"));
    if name == "MUSIC0" {
        module.apply_tempo_patch(0x78);
    }
    module
}

fn options(quality: Quality) -> PlayerOptions {
    PlayerOptions {
        quality,
        ..PlayerOptions::default()
    }
}

fn player(name: &'static str, quality: Quality, order: u8) -> Player {
    let mut player = Player::new(module(name), options(quality));
    player.play_song(order);
    player
}

/// Everything the ring holds, as a device would take it.
fn drain(player: &mut Player) -> Vec<i16> {
    let mut out = vec![0i16; player.pending_samples()];
    let taken = player.take_samples(&mut out);
    assert_eq!(taken, out.len(), "the drain takes what the ring holds");
    out
}

const MODULES: [&str; 2] = ["MUSIC0", "MUSIC1"];

/// Every word a part can read of the player, after one poll.
type Words = (sr_audio::Position, sr_audio::Position, i32, i32);

/// `polls` polls of `name` from order 0 at `quality`, draining as a device would so the ring
/// never reaches its overflow policy: the words after each one.
fn schedule(name: &'static str, quality: Quality, polls: usize) -> Vec<Words> {
    let mut player = player(name, quality, 0);
    (0..polls)
        .map(|_| {
            player.poll();
            drain(&mut player);
            (
                player.state(),
                player.engine_position(),
                player.zplus(),
                player.zframe(),
            )
        })
        .collect()
}

#[test]
fn enhanced_keeps_the_authentic_tick_schedule() {
    // The music clock is the picture (the phase's global constraints): `poll`,
    // `samples_per_tick` and `advance` are the authentic ones, so every word a part reads —
    // `state()`, `engine_position()`, `zplus()`, `zframe()` — is the same at every poll.
    // 30,000 polls is 428 s of the demo's 70.086 Hz timeline. The four runs are independent
    // players, so they run on four threads and are compared afterwards.
    const POLLS: usize = 30_000;
    let runs: Vec<_> = std::thread::scope(|scope| {
        let handles: Vec<_> = MODULES
            .iter()
            .flat_map(|&name| {
                [Quality::Authentic, Quality::Enhanced]
                    .map(|quality| scope.spawn(move || schedule(name, quality, POLLS)))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("a run"))
            .collect()
    });
    for (name, pair) in MODULES.iter().zip(runs.chunks(2)) {
        let (authentic, enhanced) = (&pair[0], &pair[1]);
        assert_eq!(authentic.len(), POLLS);
        for (poll, (a, e)) in authentic.iter().zip(enhanced).enumerate() {
            assert_eq!(e.0, a.0, "{name} poll {poll}: state()");
            assert_eq!(e.1, a.1, "{name} poll {poll}: engine_position()");
            assert_eq!(e.2, a.2, "{name} poll {poll}: zplus()");
            assert_eq!(e.3, a.3, "{name} poll {poll}: zframe()");
        }
        let end = authentic[POLLS - 1].0;
        println!(
            "{name}: {POLLS} polls, the same words at every one; both end at {}:{}, zframe {}",
            end.order,
            end.row,
            authentic[POLLS - 1].3
        );
    }
}

#[test]
fn enhanced_emits_twelve_frames_for_every_five() {
    // An authentic stereo frame the poll's budget splits (an odd budget, `Mixer::mix`) has
    // already been mixed when its left half goes out, so the authentic frames are the drained
    // samples rounded up to whole frames; the enhanced path emits that frame's 2 or 3 output
    // frames whole. The identity is exact after every poll, which is what keeps the stream
    // from drifting against the picture.
    for name in MODULES {
        let mut authentic = player(name, Quality::Authentic, 0);
        let mut enhanced = player(name, Quality::Enhanced, 0);
        let (mut authentic_samples, mut enhanced_samples) = (0u64, 0u64);
        for poll in 0..3_000 {
            authentic.poll();
            enhanced.poll();
            authentic_samples += drain(&mut authentic).len() as u64;
            enhanced_samples += drain(&mut enhanced).len() as u64;
            assert_eq!(
                enhanced_samples % 2,
                0,
                "{name} poll {poll}: whole stereo frames"
            );
            let authentic_frames = authentic_samples.div_ceil(2);
            assert_eq!(
                enhanced_samples / 2,
                authentic_frames * 12 / 5,
                "{name} poll {poll}: floor(authentic frames × 12/5)"
            );
        }
        println!(
            "{name}: 3,000 polls, {} authentic frames, {} enhanced frames",
            authentic_samples.div_ceil(2),
            enhanced_samples / 2
        );
        assert_eq!(
            enhanced.dropped_samples(),
            0,
            "the drained ring never overflowed"
        );
    }
}

/// One second of a single voice, mixed by a bare [`Mixer`] at `quality`: one side's samples.
///
/// The note is the song's own cell — its note, its instrument, its volume column or the
/// instrument's volume (`+0x485b`'s clamp to 63) — started on its own channel with every
/// other channel silent, so what is measured is that one voice.
fn one_second_of(
    module: &Module,
    channel: usize,
    note: u8,
    instrument: u8,
    volume: u8,
    quality: Quality,
) -> Vec<i16> {
    let options = options(quality);
    let header = module
        .instrument(usize::from(instrument) - 1)
        .expect("the cell's instrument");
    let mut channels = [Channel::default(); CHANNELS];
    channels[channel].period = scaled_period(note_period(note), header.c2spd.min(0xffff) as u16);
    channels[channel].volume = volume;
    let mut mixer = Mixer::new_with_capacity(1 << 18);
    mixer.start_note(channel, module, instrument);
    // One second of authentic output is 20,000 stereo frames: 40,000 of the object's bytes.
    mixer.mix(
        module,
        &mut channels,
        &options,
        module.global_volume(),
        false,
        40_000,
    );
    let mut out = vec![0i16; mixer.pending()];
    let taken = mixer.take(&mut out);
    assert_eq!(taken, out.len());
    // The module's pan table puts even channels on the left and odd on the right.
    let side = channel % 2;
    out.iter().skip(side).step_by(2).copied().collect()
}

/// The buffer's zero-crossing frequency, in Hz: the upward crossings, each placed between
/// its two samples by linear interpolation, and the frequency the first and last of them span.
///
/// Two things are done to the signal first, the same for both paths, so that what is counted
/// is the note's own period and not either renderer's high-frequency detail:
///
/// - **a band limit**: two one-pole low-passes at 1 kHz, matched in continuous time to the
///   buffer's `rate`. The authentic path is a staircase at 20 kHz — one sample byte held a whole
///   frame — and the enhanced path a line at 48 kHz, so above the note's fundamental the two
///   waveforms differ by design and their raw crossing counts with them (on these two notes,
///   without the band limit, 173 and 172 crossings, and 389 and 373);
/// - **hysteresis**: a crossing counts only after the signal has been below an eighth of its
///   peak, so a slow crossing is counted once.
///
/// When the two counts agree, the two frequencies differ only by where each path's crossings
/// fall within a sample, a few thousandths of a percent.
fn zero_crossing_frequency(samples: &[i16], rate: f64) -> (usize, f64) {
    let alpha = 1.0 - (-2.0 * std::f64::consts::PI * 1000.0 / rate).exp();
    let (mut first, mut second) = (0.0f64, 0.0f64);
    let filtered: Vec<f64> = samples
        .iter()
        .map(|&sample| {
            first += alpha * (f64::from(sample) - first);
            second += alpha * (first - second);
            second
        })
        .collect();
    let peak = filtered
        .iter()
        .fold(0.0f64, |peak, sample| peak.max(sample.abs()));
    let threshold = peak / 8.0;
    let mut below = false;
    let mut times = Vec::new();
    for (at, pair) in filtered.windows(2).enumerate() {
        if pair[1] < -threshold {
            below = true;
        } else if below && pair[0] <= 0.0 && pair[1] > 0.0 {
            below = false;
            times.push(at as f64 + -pair[0] / (pair[1] - pair[0]));
        }
    }
    let count = times.len();
    assert!(count > 2, "a periodic signal crosses zero");
    let span = times[count - 1] - times[0];
    (count, (count - 1) as f64 * rate / span)
}

#[test]
fn enhanced_pitch_is_the_authentic_pitch() {
    // Two sustained notes, one per module, each sounding unchanged in the song for well over
    // a second: no command on the row, and no cell on the channel for the rows after it.
    //
    // - MUSIC0, order 6 (pattern 3), row 0, channel 5: note 0x30 (C-3), instrument 9 (looping
    //   100..5945), no volume column; the channel's next cell is row 32 — 32 rows at speed 6,
    //   tempo 120, which is 4 s.
    // - MUSIC1, order 90 (pattern 37), row 32, channel 3: note 0x49, instrument 19 (looping
    //   11666..20033), no volume column; the channel is empty to the pattern's end, 31 rows.
    let notes: [(&'static str, u8, usize, usize); 2] = [("MUSIC0", 6, 0, 5), ("MUSIC1", 90, 32, 3)];
    for (name, order, row, channel) in notes {
        let module = module(name);
        let pattern = module
            .pattern(usize::from(module.orders()[usize::from(order)]))
            .expect("the order's pattern");
        let cell = pattern.rows[row][channel];
        assert!(
            cell.has_note() && !cell.has_command(),
            "{name}: a plain note"
        );
        for later in row + 1..row + 9 {
            assert!(
                pattern.rows[later][channel].is_empty(),
                "{name}: the channel is left alone for a second after the note"
            );
        }
        let header = module.instrument(usize::from(cell.instrument) - 1).unwrap();
        assert!(header.loops(), "{name}: a looping sample sustains");
        let volume = if cell.has_volume() {
            cell.volume
        } else {
            header.volume
        }
        .min(0x3f);

        let second = |quality| {
            one_second_of(
                &module,
                channel,
                cell.note,
                cell.instrument,
                volume,
                quality,
            )
        };
        let (authentic, enhanced) = (second(Quality::Authentic), second(Quality::Enhanced));
        assert_eq!(authentic.len(), 20_000, "one second at the authentic rate");
        assert_eq!(enhanced.len(), 48_000, "one second at the enhanced rate");
        let (na, fa) = zero_crossing_frequency(&authentic, 20_000.0);
        let (ne, fe) = zero_crossing_frequency(&enhanced, 48_000.0);
        let difference = (fe - fa) / fa;
        let raw = |samples: &[i16]| samples.windows(2).filter(|p| p[0] < 0 && p[1] >= 0).count();
        println!(
            "{name} order {order} row {row} channel {channel}: without the band limit, {} and \
             {} upward crossings",
            raw(&authentic),
            raw(&enhanced)
        );
        println!(
            "{name} order {order} row {row} channel {channel}: authentic {fa:.3} Hz ({na} \
             crossings), enhanced {fe:.3} Hz ({ne} crossings), {:+.4} %",
            difference * 100.0
        );
        assert!(
            difference.abs() <= 0.005,
            "{name}: the pitch agrees within 0.5 % (authentic {fa:.3} Hz, enhanced {fe:.3} Hz)"
        );
    }
}

fn rms(samples: &[i16]) -> f64 {
    let sum: f64 = samples.iter().map(|&s| f64::from(s) * f64::from(s)).sum();
    (sum / samples.len() as f64).sqrt()
}

#[test]
fn enhanced_level_is_the_authentic_level() {
    // Ten seconds from order 0 of each module: 701 polls of the 70.086 Hz timeline.
    for name in MODULES {
        let mut authentic = player(name, Quality::Authentic, 0);
        let mut enhanced = player(name, Quality::Enhanced, 0);
        let (mut a, mut e) = (Vec::new(), Vec::new());
        for _ in 0..701 {
            authentic.poll();
            enhanced.poll();
            a.extend(drain(&mut authentic));
            e.extend(drain(&mut enhanced));
        }
        let (ra, re) = (rms(&a), rms(&e));
        let db = 20.0 * (re / ra).log10();
        println!("{name}: ten seconds from order 0, RMS authentic {ra:.1}, enhanced {re:.1}, {db:+.3} dB");
        assert!(ra > 100.0, "{name}: the window is not silent");
        assert!(
            db.abs() <= 1.0,
            "{name}: the level agrees within 1 dB ({db:+.3} dB)"
        );
    }
}

#[test]
fn the_output_rate_is_the_streams_frames_a_second() {
    // What the device and the WAV dump are opened at (Task 8 reads it for the settings
    // screen): the authentic stereo stream is 40,000 bytes of two, and the enhanced one 48,000
    // frames.
    assert_eq!(options(Quality::Authentic).output_rate(), 20_000);
    assert_eq!(options(Quality::Enhanced).output_rate(), 48_000);
}
