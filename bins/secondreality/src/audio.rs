//! The run's music, on the machine: the player as the engine's `Music`, the sound card
//! that drains it, and the WAV file a dump writes.
//!
//! Three things live here, and they are the three the *binary* owns rather than either
//! crate:
//!
//! - **[`PlayerMusic`], the adapter.** `sr-audio` does not depend on `sr-engine` and
//!   `sr-engine` has no runtime dependencies at all, so neither crate can name the other's
//!   types and the only crate that can is the one that depends on both
//!   (`crates/sr-engine/src/music.rs`'s module comment). It translates nothing: the three
//!   `dis_*` translations are the engine's ([`musplus_from`], [`muscode_from`],
//!   [`sync_from`]) and this hands them the player's published words.
//! - **[`Device`], the sound card**, and its callback. SDL lives in the binary and only in
//!   the binary (`CLAUDE.md`, and the manifest guard in `crates/sr-audio/src/lib.rs`).
//! - **[`AudioDump`], the WAV file** `--audio-dump` writes: 16-bit signed PCM at the
//!   mixer's own rate and channel count, because no audio reference exists to compare
//!   against (the oracle runs silent by D17) and this is what a person listens to and what
//!   a test can inspect byte for byte.
//!
//! **The device's policy, in one sentence: it drains, and that is all.** The music's
//! position is the *tick's* — one `Player::poll` per logic tick, from
//! `MAIN/COPPER.ASM:466`'s `call _zpollme` — and the card is behind it by whatever the
//! ring has not given up. A callback that asks for more samples than the ring holds gets
//! silence for the rest ([`Drain::callback`]): never a stall, never a repeat, never a
//! resample; and the ring's own overflow policy drops the *oldest* samples rather than
//! making a poll wait (`Player::take_samples`, `Mixer::dropped`).

use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sr_audio::{Player, PlayerOptions};
use sr_engine::music::{muscode_from, musplus_from, sync_from, Music, MusicPosition};

use crate::shell::ModuleId;

/// The player, shared by the engine's seam and by whatever drains it — the sound card's
/// callback, or the dump. One player, one ring, one consumer: a second drainer would take
/// samples away from the first.
pub(crate) type Shared = Arc<Mutex<Player>>;

/// The player behind a `Shared`, whatever state the lock is in.
///
/// A poisoned lock means some other thread panicked while holding it, and the device is
/// not the place to unwind: the player's own state is still readable through the poison,
/// and the alternative — silence for the rest of the run — would turn one panic into a
/// silent demo.
pub(crate) fn lock(player: &Shared) -> MutexGuard<'_, Player> {
    player.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The cell a [`Drain`] reads its player through, so the card can **follow** the run to a
/// new player.
///
/// A run's player can be replaced mid-run: `MAIN/U2.ASM:880-885`'s restart at the U2E
/// boundary stops the module, loads the next and seeks it (`restartmus`, `:348-397`), and
/// the port installs that as a new [`Shared`] (`Sequence::restart_music`), which takes the
/// old player's state over first (D45). The sound
/// card is opened once, on the run's entry player, and outlives it; this cell is what lets
/// the window hand it the run's current one ([`Device::follow`]).
pub(crate) type Swappable = Arc<Mutex<Shared>>;

/// The player in a [`Swappable`], whatever state the cell's lock is in. The same poison
/// policy as [`lock`].
fn cell(cell: &Swappable) -> MutexGuard<'_, Shared> {
    cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

// -------------------------------------------------------------------------------------------
// The adapter: `Player` as the engine's `Music`
// -------------------------------------------------------------------------------------------

/// The run's player, as the engine's [`Music`] — the eight methods, forwarded.
///
/// Forwarded and not translated: `musplus`, `muscode` and `sync` are the DIS's own
/// arithmetic, which lives in `sr-engine` ([`musplus_from`], [`muscode_from`],
/// [`sync_from`]) because it is the parts' side of the seam; this passes them the
/// **published** words (`Player::state`, `Player::zplus`) — the ten-poll-old ring copies a
/// real part reads — and never the engine's own row (`docs/fidelity/music.md` §8, §11).
pub(crate) struct PlayerMusic {
    player: Shared,
    /// Polls still owed before the music's own clock starts: the frames between the run's
    /// first drawn frame and the music's boot anchor
    /// ([`crate::shell::MUSIC_START_FRAME`]). The player exists throughout — a part may
    /// ask it anything, and `PartCtx::music()` is `Some` from the run's first tick — and
    /// does not advance until the frame the original's own first tick falls on.
    ///
    /// The alternative, polling from the run's start, publishes the music's first row
    /// change five frames early against `reference/music/second5`: the measurement is in
    /// `docs/fidelity/music-clock.md` §2 and the shell's `MusicStart`.
    waiting: u32,
}

impl PlayerMusic {
    /// The adapter around `player`, `waiting` polls short of the music's own start.
    pub(crate) fn new(player: Shared, waiting: u32) -> PlayerMusic {
        PlayerMusic { player, waiting }
    }
}

impl Music for PlayerMusic {
    fn poll(&mut self) {
        if self.waiting > 0 {
            self.waiting -= 1;
            return;
        }
        lock(&self.player).poll();
    }

    fn position(&self) -> MusicPosition {
        let player = lock(&self.player);
        let state = player.state();
        MusicPosition {
            order: state.order,
            pattern: state.pattern,
            row: state.row,
            speed: state.speed,
            tempo: state.tempo,
        }
    }

    fn musplus(&mut self) -> i32 {
        let player = lock(&self.player);
        musplus_from(player.zplus(), i32::from(player.state().row))
    }

    fn musrow(&mut self) -> i32 {
        i32::from(lock(&self.player).state().row)
    }

    fn muscode(&mut self, wanted: i32) -> i32 {
        muscode_from(self.position(), wanted)
    }

    fn sync(&mut self) -> i32 {
        let player = lock(&self.player);
        let state = player.state();
        sync_from(state.order, state.row)
    }

    fn getmframe(&mut self) -> i32 {
        lock(&self.player).zframe()
    }

    fn setmframe(&mut self, frame: i32) {
        lock(&self.player).set_zframe(frame);
    }
}

// -------------------------------------------------------------------------------------------
// Loading the modules
// -------------------------------------------------------------------------------------------

/// The real [`MusicSource`](crate::shell::MusicSource): the demo's own module, read from
/// `assets/` under the working directory and patched as the shell patches it
/// ([`sr_audio::Song::load`]).
///
/// The player is built with the run's `options`
/// ([`RunSettings::player`](crate::shell::RunSettings::player)): authentic by default, the
/// enhanced renderer when the run asks for it (D43).
pub(crate) fn load_player(id: ModuleId, options: PlayerOptions) -> Result<Shared, String> {
    player_at(id, Path::new(""), options)
}

/// [`load_player`] with the module path rooted somewhere else.
///
/// A *run* reads `assets/` from its working directory (the repository root, for this
/// repository's commands); a unit test runs in the crate's own directory and reads the same
/// files through `../..`. One loader with the root as a parameter is what keeps those from
/// being two transcriptions of the same three steps.
pub(crate) fn player_at(
    id: ModuleId,
    root: &Path,
    options: PlayerOptions,
) -> Result<Shared, String> {
    let module = id.song().load(root)?;
    Ok(Arc::new(Mutex::new(Player::new(module, options))))
}

/// The [`MusicSource`](crate::shell::MusicSource) the *tests* play with: the real modules,
/// read at the path a unit test reads them by (the crate's own directory, not the repository
/// root a run starts in — the same `../../` the parts' tests read their media by), built with
/// the run's options.
#[cfg(test)]
pub(crate) fn test_music(id: ModuleId, options: PlayerOptions) -> Result<Shared, String> {
    player_at(id, Path::new("../.."), options)
}

/// [`test_music`] with the default (authentic) options: the player a part's own test reads.
#[cfg(test)]
pub(crate) fn test_player(id: ModuleId) -> Result<Shared, String> {
    test_music(id, PlayerOptions::default())
}

/// The mixer's stream as a WAV header (and the sound card) describes it: **frames a second**
/// and channels — [`PlayerOptions::output_rate`].
///
/// The authentic mixer produces `MixRate::samples_per_second()` **i16 samples** a second,
/// interleaved as the output mode says (`Player::take_samples`), so in stereo the *frame* rate
/// is half of it — `MixRate::Hz40000` gives 20000 frames a second, which is the rate the port's
/// own playback runs at: "the frame rate is 20000 a second in stereo (40000 bytes of two)"
/// (`docs/fidelity/music.md` §10.2, where the same 20000 is what the reference's 5.26 %-flat
/// stereo divides against). Writing 40000 into a stereo WAV header would play the music twice
/// as fast and an octave up. The enhanced renderer (D43) is 48,000 frames a second.
pub(crate) fn wav_format(options: &PlayerOptions) -> (u32, u16) {
    let channels = if options.stereo { 2 } else { 1 };
    (options.output_rate(), channels)
}

// -------------------------------------------------------------------------------------------
// `fademusic`
// -------------------------------------------------------------------------------------------

/// `fademusic` (`MAIN/U2.ASM:333-346`): walk the **module's own global-volume byte**
/// (`ds:[30h]` — the S3M header's offset 48) towards zero by `step`.
///
/// The source's routine is a loop:
///
/// ```text
/// @@cont: mov al,ds:[30h]
///         cmp al,0
///         jle @@done            ; already silent: nothing to fade
///         push cx
///         mov bx,1
///         int 0fch              ; dis_waitb() — one video frame per step
///         pop cx
///         sub byte ptr ds:[30h],cl
///         jnc @@cont            ; no borrow: the byte is still above zero
/// @@done: mov byte ptr ds:[30h],0
/// ```
///
/// so **the loop takes one step a frame** and reaches zero because it is entered again on
/// the next one: the `int 0fch` inside it is `dis_waitb()`, the vertical blank. The port's
/// tick *is* that frame, so this function is one pass of the body — one subtraction,
/// saturating at zero, which is the loop's two exits (`jle @@done`, and `@@done`'s own
/// `mov ...,0`) folded together — and the shell calls it once per tick with the step the
/// demo passes (`mov cx,1` at `:927` and `:938`).
///
/// Once per tick is `@@partsdone`'s own timing: the demo's fade runs from the moment there
/// is nothing left to play until the byte reaches zero — **64** frames at the shipped
/// modules' shared `0x40` (the S3M header's offset 48, not `_np_mastervol`'s 0xC0: see the
/// test below, which asserts the byte it walks).
pub fn fade_music(player: &mut Player, step: u8) {
    player.set_global_volume(player.global_volume().saturating_sub(step));
}

/// Whether `fademusic` has nothing left to do: the module's byte is at zero, which is
/// where the demo's fade stops. The shell's fade calls [`fade_music`] either way — the
/// source's own loop re-enters and finds `al` at zero — so this is what a test reads.
#[cfg(test)]
pub fn is_faded(player: &Player) -> bool {
    player.global_volume() == 0
}

// -------------------------------------------------------------------------------------------
// The WAV dump
// -------------------------------------------------------------------------------------------

/// A 16-bit signed PCM WAV: `--audio-dump`'s file.
///
/// The mixer's own rate and channel count, because the samples are the mixer's — this writes
/// what `Player::take_samples` hands over and converts nothing. `rate` is the **frame** rate a
/// WAV header carries, which is what [`wav_format`] computes from the player's own options
/// (`PlayerOptions::default()` is `MixRate::Hz40000` stereo, i.e. 20000 frames a second).
pub struct AudioDump<W: Write + Seek = File> {
    file: W,
    /// The data chunk's length so far, in bytes.
    data_bytes: u32,
    error: Option<io::Error>,
}

impl AudioDump<File> {
    /// Create `path` and write the header for `rate` Hz, `channels` channels.
    pub fn create(path: &Path, rate: u32, channels: u16) -> io::Result<AudioDump> {
        let file = File::create(path)?;
        AudioDump::over(file, rate, channels)
    }
}

impl<W: Write + Seek> AudioDump<W> {
    /// The header's length: the two size fields are patched in place as the file grows.
    pub const HEADER_BYTES: u32 = 44;

    /// Write the header into a writer that is already open — the seam [`AudioDump::create`]
    /// is one call of, and the one a test drives a failing writer through.
    pub fn over(mut file: W, rate: u32, channels: u16) -> io::Result<AudioDump<W>> {
        let block_align = channels * 2;
        let mut header = Vec::with_capacity(Self::HEADER_BYTES as usize);
        header.extend_from_slice(b"RIFF");
        header.extend_from_slice(&0u32.to_le_bytes()); // patched: 36 + the data length
        header.extend_from_slice(b"WAVE");
        header.extend_from_slice(b"fmt ");
        header.extend_from_slice(&16u32.to_le_bytes()); // the PCM `fmt ` chunk's length
        header.extend_from_slice(&1u16.to_le_bytes()); // format 1: PCM
        header.extend_from_slice(&channels.to_le_bytes());
        header.extend_from_slice(&rate.to_le_bytes());
        header.extend_from_slice(&(rate * u32::from(block_align)).to_le_bytes());
        header.extend_from_slice(&block_align.to_le_bytes());
        header.extend_from_slice(&16u16.to_le_bytes()); // bits a sample
        header.extend_from_slice(b"data");
        header.extend_from_slice(&0u32.to_le_bytes()); // patched: the data length
        file.write_all(&header)?;
        Ok(AudioDump {
            file,
            data_bytes: 0,
            error: None,
        })
    }

    /// Append samples, and keep the file's two size fields true.
    ///
    /// The sizes are patched on **every** write rather than once at the end, so the file
    /// is a valid WAV at every moment of the run: a run that is interrupted — a failing
    /// part, a closed window — leaves a file a player can read rather than one whose
    /// header claims zero samples.
    pub fn write(&mut self, samples: &[i16]) {
        if let Err(error) = self.append(samples) {
            if self.error.is_none() {
                self.error = Some(error);
            }
        }
    }

    fn append(&mut self, samples: &[i16]) -> io::Result<()> {
        let mut bytes = Vec::with_capacity(samples.len() * 2);
        for sample in samples {
            bytes.extend_from_slice(&sample.to_le_bytes());
        }
        self.file.write_all(&bytes)?;
        self.data_bytes = self.data_bytes.saturating_add(bytes.len() as u32);
        let riff = Self::HEADER_BYTES - 8 + self.data_bytes;
        self.file.seek(SeekFrom::Start(4))?;
        self.file.write_all(&riff.to_le_bytes())?;
        self.file.seek(SeekFrom::Start(40))?;
        self.file.write_all(&self.data_bytes.to_le_bytes())?;
        self.file.seek(SeekFrom::End(0))?;
        Ok(())
    }

    /// The samples written so far, as the header says.
    #[cfg(test)]
    pub fn samples(&self) -> u32 {
        self.data_bytes / 2
    }

    /// The first write that failed, if any — asked at the end of a run, because a dump
    /// that stopped half way is a failure and not a shorter recording.
    pub fn error(&self) -> Option<&io::Error> {
        self.error.as_ref()
    }
}

// -------------------------------------------------------------------------------------------
// The sound card
// -------------------------------------------------------------------------------------------

/// The sound card, opened for the run.
pub struct Device {
    /// Kept for the run's lifetime; [`Device::open`] has already resumed it, because SDL
    /// opens a playback device paused.
    device: AudioDevice<Drain>,
    /// The same cell the callback's [`Drain`] reads: swapping the `Shared` here is what
    /// makes the card follow a mid-run restart ([`Device::follow`]).
    player: Swappable,
}

impl Device {
    /// Open the default playback device at the mixer's own rate and channel count.
    ///
    /// `allowed_changes` is SDL's zero (`sdl2`'s `AudioDevice::open` passes it), so the
    /// device opens at exactly this **format** or not at all: samples in a frequency,
    /// channel count or sample format the caller did not ask for would be a **resample**,
    /// which is the one thing this device must never do. A machine that cannot give that
    /// format — no card, no audio subsystem, a card the driver will not open — gets `Err`,
    /// and the caller prints a line and runs silent rather than failing (`window::run`).
    ///
    /// **`samples` is not part of that contract.** The desired spec asks for `None` and SDL
    /// adjusts the buffer size to the driver: measured against the system SDL2 (2.30.0) on
    /// the `dummy`, `alsa`, `pulseaudio` and `pipewire` drivers, 20000 Hz stereo gives
    /// **1024 frames** a callback (2048 i16, 0.051 s) and a request of 4096 is honoured —
    /// so the buffer is SDL's, and [`Device::spec`] reports what it gave.
    ///
    /// The margin is the player's own ring ([`Player::ring_capacity`]) over a callback's
    /// samples. Authentic: 1024 frames is a **4×** margin under
    /// [`RING_SAMPLES`](sr_audio::RING_SAMPLES) = 8192 i16. Enhanced (D43): the same four
    /// drivers give **4096 frames** a callback at 48000 Hz stereo (8192 i16, 0.085 s, measured
    /// by `SDL_VIDEODRIVER=dummy SDL_AUDIODRIVER=<driver> secondreality --part endlogo
    /// --sound-quality enhanced`, which prints the line `window::run` writes), a **2.4×** margin
    /// under its 19,660-sample ring. A driver that returned a callback as large as the ring
    /// would take all of it at once, which is why the size is printed with the rate and the ring
    /// rather than assumed.
    pub fn open(sdl: &sdl2::Sdl, player: Shared, rate: u32, channels: u8) -> Result<Device, String> {
        let audio = sdl.audio()?;
        let spec = AudioSpecDesired {
            freq: Some(rate as i32),
            channels: Some(channels),
            samples: None,
        };
        let cell: Swappable = Arc::new(Mutex::new(player));
        let callback = Arc::clone(&cell);
        let device =
            audio.open_playback::<Drain, _, _>(None, &spec, |_spec| Drain { player: callback })?;
        device.resume();
        Ok(Device { device, player: cell })
    }

    /// The rate, channel count and callback buffer size SDL actually gave us, in frames.
    pub fn spec(&self) -> (i32, u8, u16) {
        let spec = self.device.spec();
        (spec.freq, spec.channels, spec.samples)
    }

    /// The player the card is draining now.
    pub fn player(&self) -> Shared {
        cell(&self.player).clone()
    }

    /// Point the drain at `player` — the run's current one after a mid-run restart.
    ///
    /// The old player's ring keeps its tail, but nothing drains it: `restartmus`
    /// (`MAIN/U2.ASM:348-397`) interrupts the module it was playing too, and the samples
    /// of a module the demo has stopped are not the demo's music. The callback's next
    /// `drain` takes the new player's ring (the cell is swapped under its lock, so a
    /// callback in flight finishes against the player it started with).
    pub fn follow(&mut self, player: Shared) {
        *cell(&self.player) = player;
    }
}

/// The device's callback: drain the ring, and nothing else.
///
/// `Send` because SDL runs it on its own audio thread, which is why the player is behind
/// a mutex: the *tick* polls and the callback drains, and neither does the other's job. A
/// callback that mixed, ticked or waited would make the music's position depend on the
/// card's clock instead of the demo's.
pub(crate) struct Drain {
    /// The run's current player, behind a cell so [`Device::follow`] can swap it while
    /// the callback is live.
    player: Swappable,
}

impl Drain {
    /// [`Drain`] around `player`, for a caller that wants the callback's own behaviour
    /// without a sound card (this module's tests, and the underrun policy's in particular).
    #[cfg(test)]
    pub(crate) fn new(player: Shared) -> Drain {
        Drain { player: Arc::new(Mutex::new(player)) }
    }

    /// Point the drain at `player`; the next [`drain`](Drain::drain) reads its ring.
    #[cfg(test)]
    pub(crate) fn follow(&self, player: Shared) {
        *cell(&self.player) = player;
    }

    /// Fill `out` the way the callback does, and say how many samples the ring gave up.
    ///
    /// The cell is locked only long enough to clone the current `Shared`; the take then
    /// holds that player's own ring lock. A `follow` between two drains is therefore
    /// safe, and one that lands while a drain is running cannot split the take across two
    /// players — the callback finishes against the player it started with.
    pub(crate) fn drain(&mut self, out: &mut [i16]) -> usize {
        let player = cell(&self.player).clone();
        let taken = lock(&player).take_samples(out);
        out[taken..].fill(0);
        taken
    }
}

impl AudioCallback for Drain {
    type Channel = i16;

    /// **Underrun is silence, never a stall and never a resample.** The ring gives up
    /// whatever it holds — the samples the last tick mixed — and the rest of the buffer is
    /// zeroes. Nothing is repeated to fill the gap and nothing is resampled to fit it,
    /// because the gap is the truth: the tick has not produced those samples yet, and the
    /// music's position is the tick's. The alternative — waiting for the ring to fill —
    /// would stall SDL's audio thread and pull the picture's clock into the card's.
    fn callback(&mut self, out: &mut [i16]) {
        self.drain(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_audio::{Module, Quality};
    use std::path::PathBuf;

    /// The module this binary plays, at the path a *unit test* reads it by: the crate's
    /// own directory, not the repository root a run starts in (the same `../../` the
    /// parts' tests use).
    fn module(name: &str) -> Module {
        sr_audio::Song::named(name)
            .unwrap_or_else(|| panic!("{name} is not a song"))
            .load(Path::new("../.."))
            .unwrap_or_else(|error| panic!("{error}"))
    }

    fn player(name: &str) -> Player {
        Player::new(module(name), PlayerOptions::default())
    }

    fn shared(name: &str) -> Shared {
        Arc::new(Mutex::new(player(name)))
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sr-wav-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("dump.wav")
    }

    /// The header's own words, as a WAV reader reads them: little-endian, and the sizes
    /// are the file's.
    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    fn half(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    // --- the adapter -------------------------------------------------------

    #[test]
    fn the_adapter_hands_the_engine_the_players_published_words() {
        // Every method is a forwarder, so what is asserted is *which* word each one reads:
        // the published position (the ring's, ten polls old), the published row, the
        // published `zplus` — never the engine's own, which is ten polls ahead.
        let player = shared("MUSIC0");
        let mut music = PlayerMusic::new(Arc::clone(&player), 0);
        music.poll();
        let published = lock(&player).state();
        assert_eq!(music.position().order, published.order);
        assert_eq!(music.position().row, published.row);
        assert_eq!(music.position().speed, published.speed);
        assert_eq!(music.position().tempo, published.tempo);
        assert_eq!(music.musrow(), i32::from(published.row));
        assert_eq!(
            music.musplus(),
            musplus_from(lock(&player).zplus(), i32::from(published.row)),
            "the DIS's arithmetic over the *published* zplus and row"
        );
        assert_eq!(music.muscode(2), 0, "`_np_zinfo` is 0 (D8)");
        assert_eq!(music.sync(), sync_from(published.order, published.row));

        // The parts' own tick counter: written and read through the same word.
        assert_eq!(music.getmframe(), 0, "a fresh player has not ticked");
        music.setmframe(1516);
        assert_eq!(music.getmframe(), 1516, "`dis_setmframe` writes it through");

        // `MUSIC0`'s global-volume byte is 0x40 — the S3M header's own offset 48, which is
        // what `fademusic` walks, not `_np_mastervol`'s 0xC0 initialiser: the music is
        // audible, and the fade is what will take it down.
        assert!(!is_faded(&lock(&player)));
    }

    #[test]
    fn the_adapter_waits_out_the_frames_before_the_music_starts() {
        // A run anchored at 404 reaches the music's own clock at 408, so the first four
        // polls must not advance the player: the position a part reads is the fresh one,
        // and `_np_zframe` is still 0 — which is what the trace holds at frames 404…407.
        let player = shared("MUSIC0");
        lock(&player).play_song(25);
        let mut music = PlayerMusic::new(Arc::clone(&player), 4);
        for _ in 0..4 {
            music.poll();
        }
        assert_eq!(music.getmframe(), 0, "the music has not started yet");
        assert_eq!(music.position().row, 0);
        // The fifth poll is the music's own first: from here it ticks like any other.
        let before = music.getmframe();
        for _ in 0..8 {
            music.poll();
        }
        assert!(
            music.getmframe() > before,
            "the polls after the wait advance the music"
        );
    }

    // --- the dump ----------------------------------------------------------

    #[test]
    fn the_dump_is_a_wav_at_the_mixers_own_rate_and_width() {
        let path = scratch("header");
        let mut dump = AudioDump::create(&path, 20_000, 2).unwrap();
        assert_eq!(dump.samples(), 0);
        dump.write(&[0, 1, 2, 3, -1, -2]);
        assert_eq!(dump.samples(), 6);
        assert!(dump.error().is_none());
        drop(dump);

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(bytes.len(), 44 + 12, "a 44-byte header and six i16 samples");
        assert_eq!(&bytes[0..4], b"RIFF");
        assert_eq!(word(&bytes, 4), (bytes.len() - 8) as u32, "the RIFF size");
        assert_eq!(&bytes[8..12], b"WAVE");
        assert_eq!(&bytes[12..16], b"fmt ");
        assert_eq!(word(&bytes, 16), 16, "a PCM `fmt ` chunk");
        assert_eq!(half(&bytes, 20), 1, "format 1 is PCM");
        assert_eq!(half(&bytes, 22), 2, "stereo, as the mixer's options say");
        // 20000 frames a second, stereo: the mixer's `MixRate::Hz40000` is 40000 i16
        // samples a second, so a stereo stream is half that in frames (see `wav_format`).
        assert_eq!(word(&bytes, 24), 20_000, "the mixer's frame rate");
        assert_eq!(word(&bytes, 28), 20_000 * 4, "the byte rate");
        assert_eq!(half(&bytes, 32), 4, "four bytes a frame");
        assert_eq!(half(&bytes, 34), 16, "16-bit samples");
        assert_eq!(&bytes[36..40], b"data");
        assert_eq!(word(&bytes, 40), 12, "the data chunk's length");
        // The samples themselves, signed and little-endian, in the order written.
        assert_eq!(&bytes[44..48], &[0, 0, 1, 0]);
        assert_eq!(&bytes[52..56], &[0xff, 0xff, 0xfe, 0xff], "-1 then -2");

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_audio_dump_header_is_the_output_rate() {
        // The header describes the stream the player's own options produce
        // (`PlayerOptions::output_rate`): 20,000 frames a second for the authentic stereo
        // `Hz40000`, and 48,000 for the enhanced renderer (D43), stereo both.
        let enhanced = PlayerOptions { quality: Quality::Enhanced, ..PlayerOptions::default() };
        let player = test_music(ModuleId::Music0, enhanced).expect("MUSIC0 loads");
        assert_eq!(lock(&player).options(), enhanced, "the loader builds what it was asked for");
        assert_eq!(wav_format(&lock(&player).options()), (48_000, 2));
        assert_eq!(wav_format(&PlayerOptions::default()), (20_000, 2));

        let path = scratch("enhanced");
        let (rate, channels) = wav_format(&lock(&player).options());
        let mut dump = AudioDump::create(&path, rate, channels).unwrap();
        dump.write(&[1, 2]);
        drop(dump);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(half(&bytes, 22), 2, "stereo");
        assert_eq!(word(&bytes, 24), 48_000, "the enhanced frame rate");
        assert_eq!(word(&bytes, 28), 48_000 * 4, "the byte rate");
        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    #[test]
    fn the_header_is_true_at_every_tick_not_only_at_the_end() {
        // A run interrupted between two ticks leaves a file a player can read: the sizes
        // are patched on every write, so the header never claims samples that are not
        // there and never denies the ones that are.
        let path = scratch("patched");
        let mut dump = AudioDump::create(&path, 20_000, 2).unwrap();
        dump.write(&[7, 7, 7, 7]);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(word(&bytes, 4), 44, "36 + 8 data bytes");
        assert_eq!(word(&bytes, 40), 8);
        dump.write(&[9, 9]);
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(word(&bytes, 4), 48);
        assert_eq!(word(&bytes, 40), 12);
        assert_eq!(bytes.len(), 44 + 12);

        let _ = std::fs::remove_dir_all(path.parent().unwrap());
    }

    /// A file that takes the header and then refuses everything: a disk that filled up
    /// between the two, which is the failure a dump's own report exists for.
    struct FullAfterTheHeader {
        written: Vec<u8>,
        room: usize,
    }

    impl Write for FullAfterTheHeader {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.written.len() + bytes.len() > self.room {
                return Err(io::Error::new(io::ErrorKind::StorageFull, "no room"));
            }
            self.written.extend_from_slice(bytes);
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl Seek for FullAfterTheHeader {
        fn seek(&mut self, _to: SeekFrom) -> io::Result<u64> {
            Ok(self.written.len() as u64)
        }
    }

    #[test]
    fn a_write_that_cannot_be_made_is_kept_rather_than_lost() {
        // A dump that stopped half way is a failure with a reason, not a shorter
        // recording: the first error is kept, the run carries on drawing, and the caller
        // asks at the end (`Sequence::audio_error`, which `headless::run` turns into a
        // failed run).
        let header = AudioDump::<File>::HEADER_BYTES as usize;
        let mut dump =
            AudioDump::over(FullAfterTheHeader { written: Vec::new(), room: header }, 20_000, 2)
                .expect("the header fits");
        assert_eq!(dump.samples(), 0);
        dump.write(&[1, 2, 3, 4]);
        assert!(dump.error().is_some(), "the write past the room failed");
        assert_eq!(dump.samples(), 0, "and nothing was counted as written");
        // The header's own write is a failure too, and it is the *caller's*, not a run's:
        // `--audio-dump` into a directory that cannot be written says so before the run.
        let error = AudioDump::over(FullAfterTheHeader { written: Vec::new(), room: 0 }, 20_000, 2)
            .err()
            .expect("a writer with no room cannot take a header");
        assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    }

    // --- the fade ----------------------------------------------------------

    #[test]
    fn the_fade_walks_the_modules_own_byte_down_to_zero_and_stops() {
        // `fademusic` (`MAIN/U2.ASM:333-346`): one step a call, and the byte never goes
        // below zero (`@@done`'s `mov byte ptr ds:[30h],0`).
        let mut player = player("MUSIC0");
        let start = player.global_volume();
        // `ds:[30h]` is the module image's own global-volume byte — the S3M header's offset
        // 48, which both modules carry as 0x40 (`Module::global_volume`;
        // `crates/sr-audio/tests/module.rs` asserts it as "the byte at 48") — and *not*
        // `_np_mastervol`, whose
        // initialiser is 0xC0: the shell fades the module's byte
        // (`Player::set_global_volume`'s doc).
        assert_eq!(start, 0x40, "the module's own global-volume byte");
        let mut steps = 0;
        while !is_faded(&player) {
            fade_music(&mut player, 1);
            steps += 1;
            assert!(steps <= 300, "the fade did not reach zero");
        }
        assert_eq!(steps, usize::from(start), "one step a call, 0x40 of them");
        fade_music(&mut player, 5);
        assert_eq!(player.global_volume(), 0, "and it stops at zero");
    }

    // --- the device's policy -----------------------------------------------

    #[test]
    fn an_empty_ring_under_runs_into_silence() {
        // **The underrun policy, as behaviour.** A freshly polled player has a
        // tick's samples at most, so a callback asking for far more gets that much music
        // and zeroes for the rest — never a stall, never a repeat, never a resample.
        let player = shared("MUSIC0");
        lock(&player).play_song(25);
        lock(&player).poll();
        let produced = lock(&player).samples_this_poll() as usize;
        assert!(produced > 0 && produced < 4096, "one tick is a few hundred samples");

        let mut drain = Drain::new(Arc::clone(&player));
        let mut out = vec![i16::MIN; 4096];
        let taken = drain.drain(&mut out);
        assert_eq!(
            taken, produced,
            "the ring gave up exactly what the poll mixed — no more, no fewer"
        );
        assert!(
            out[taken..].iter().all(|sample| *sample == 0),
            "the rest of the buffer is silence, not a repeat of the last sample"
        );

        // And a second drain of the same buffer, with the ring now empty, is silence all
        // the way through: the device does not pull the music forward for being asked.
        let ticks = lock(&player).zframe();
        let mut empty = vec![i16::MIN; 64];
        let taken = drain.drain(&mut empty);
        assert_eq!(taken, 0);
        assert!(empty.iter().all(|sample| *sample == 0));
        assert_eq!(
            lock(&player).zframe(),
            ticks,
            "and asking for samples did not tick the player"
        );
        assert_eq!(
            lock(&player).pending_samples(),
            0,
            "the drain is the only consumer, and it took everything there was"
        );
    }

    #[test]
    fn the_drain_follows_a_restart_to_the_runs_new_player() {
        // A run's player is replaced mid-run at the U2E boundary (`MAIN/U2.ASM:880-885`'s
        // `restartmus`), and the card is opened once, on the entry player. The drain's
        // cell is what lets it follow: after the swap the next callback takes the new
        // ring's samples and none of the old player's, which are the stopped module's.
        let old = shared("MUSIC1");
        lock(&old).play_song(0);
        lock(&old).poll();
        lock(&old).poll();
        let old_pending = lock(&old).pending_samples();
        assert!(old_pending > 0, "the old player has samples waiting");

        let new = shared("MUSIC0");
        lock(&new).play_song(18);
        lock(&new).poll();
        let new_samples = lock(&new).samples_this_poll() as usize;
        assert!(new_samples > 0, "and the new player mixes a poll's worth");
        assert_ne!(old_pending, new_samples, "the two rings are distinguishable by size");

        let mut drain = Drain::new(Arc::clone(&old));
        drain.follow(Arc::clone(&new));
        let mut out = vec![0i16; 4096];
        assert_eq!(
            drain.drain(&mut out),
            new_samples,
            "the next drain takes the new player's ring, not the old player's \
             {old_pending} samples"
        );
        assert_eq!(lock(&new).pending_samples(), 0, "the new ring is what was drained");
        assert_eq!(
            lock(&old).pending_samples(),
            old_pending,
            "none of the old player's samples were taken"
        );
        // And the new ring is now empty, so the next drain is silence: the swap did not
        // duplicate anything either.
        let mut empty = vec![i16::MIN; 64];
        assert_eq!(drain.drain(&mut empty), 0);
        assert!(empty.iter().all(|sample| *sample == 0));
    }

    #[test]
    fn the_device_drains_at_the_mixers_own_rate_and_never_resamples() {
        // What the card gets is the mixer's own stream: `MixRate::Hz40000` interleaved
        // stereo (20000 frames a second, see `wav_format`), a poll's samples a tick, in the
        // order the mixer produced them. Draining across a run of polls
        // returns every sample exactly once — the count the mixer made is the count that
        // comes out, which is what "no resample" means.
        let player = shared("MUSIC0");
        lock(&player).play_song(25);
        let mut drain = Drain::new(Arc::clone(&player));
        let mut out = vec![0i16; 8192];
        let mut total = 0usize;
        let mut produced = 0u32;
        for _ in 0..40 {
            lock(&player).poll();
            produced += lock(&player).samples_this_poll();
            total += drain.drain(&mut out);
        }
        assert_eq!(
            total as u32, produced,
            "every sample the mixer produced came out once"
        );
        // 40 frames at 40000/70.086 samples each is 22,828 samples: the ring is 8,192, so
        // the drain has to keep up — and when it does, nothing is dropped.
        assert_eq!(lock(&player).dropped_samples(), 0);
        println!("40 polls: {produced} samples produced, {total} drained, none dropped");
    }
}
