//! The run's music on the machine: [`PlayerMusic`], the player as the engine's `Music`
//! (the only crate that can name both sides is this one); [`Device`], the sound card that
//! drains it; and [`AudioDump`], the WAV `--audio-dump` writes.
//!
//! **The device drains, and that is all.** The music's position is the tick's, one
//! `Player::poll` a tick (`MAIN/COPPER.ASM:466`, `call _zpollme`), and the card is behind
//! it by what the ring has not given up. An underrun is silence ([`Drain::drain`]), never a
//! stall, a repeat or a resample; the ring's overflow drops the oldest samples
//! (`Player::take_samples`).

use std::error::Error;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};

use sdl2::audio::{AudioCallback, AudioDevice, AudioSpecDesired};
use sr_audio::{Player, PlayerOptions};
use sr_engine::music::{muscode_from, musplus_from, sync_from, Music, MusicPosition};

use crate::cli::Cli;
use crate::shell::{ModuleId, Sequence};

/// The player, shared by the engine's seam and by whatever drains it — the sound card's
/// callback, or the dump. One player, one ring, one consumer: a second drainer would take
/// samples away from the first.
pub(crate) type Shared = Arc<Mutex<Player>>;

/// The player behind a `Shared`, through a poisoned lock too: one panic elsewhere must not
/// silence the rest of the run.
pub(crate) fn lock(player: &Shared) -> MutexGuard<'_, Player> {
    player.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The cell a [`Drain`] reads its player through, so the card can follow a mid-run restart
/// (`MAIN/U2.ASM:880-885`, `restartmus` `:348-397`; `Sequence::restart_music`, D45) to the
/// run's new player ([`Device::follow`]).
pub(crate) type Swappable = Arc<Mutex<Shared>>;

/// The player in a [`Swappable`], with [`lock`]'s poison policy.
fn cell(cell: &Swappable) -> MutexGuard<'_, Shared> {
    cell.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The run's player as the engine's [`Music`]. The DIS's arithmetic is sr-engine's
/// ([`musplus_from`], [`muscode_from`], [`sync_from`]); this hands it the **published**
/// words (`Player::state`, `Player::zplus`), the ten-poll-old copies a part reads, never the
/// engine's own row (`docs/fidelity/music.md` §8, §11).
pub(crate) struct PlayerMusic {
    player: Shared,
    /// Polls still owed before the music's own clock starts: the frames between the run's
    /// first drawn frame and the music's start ([`crate::shell::MUSIC_START_FRAME`],
    /// `docs/fidelity/music-clock.md` §2). The player answers throughout but does not
    /// advance until then.
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

/// The real [`MusicSource`](crate::shell::music::MusicSource): the module read from
/// `assets/` under the working directory ([`sr_audio::Song::load`]), with the run's player
/// options (D43).
pub(crate) fn load_player(id: ModuleId, options: PlayerOptions) -> Result<Shared, String> {
    player_at(id, Path::new(""), options)
}

/// [`load_player`] with `assets/` under `root`.
pub(crate) fn player_at(
    id: ModuleId,
    root: &Path,
    options: PlayerOptions,
) -> Result<Shared, String> {
    let module = id.song().load(root)?;
    Ok(Arc::new(Mutex::new(Player::new(module, options))))
}

/// The [`MusicSource`](crate::shell::music::MusicSource) unit tests play with: they run in
/// the crate's directory.
#[cfg(test)]
pub(crate) fn test_music(id: ModuleId, options: PlayerOptions) -> Result<Shared, String> {
    player_at(id, Path::new("../.."), options)
}

/// [`test_music`] with the default (authentic) options: the player a part's own test reads.
#[cfg(test)]
pub(crate) fn test_player(id: ModuleId) -> Result<Shared, String> {
    test_music(id, PlayerOptions::default())
}

/// The mixer's stream as a WAV header and the sound card describe it: **frames** a second
/// ([`PlayerOptions::output_rate`]) and channels. The authentic `MixRate::Hz40000` is 40000
/// i16 samples a second, so 20000 stereo frames (`docs/fidelity/music.md` §10.2); the
/// enhanced renderer is 48000 (D43).
pub(crate) fn wav_format(options: &PlayerOptions) -> (u32, u16) {
    let channels = if options.stereo { 2 } else { 1 };
    (options.output_rate(), channels)
}

/// One pass of `fademusic` (`MAIN/U2.ASM:333-346`), whose loop waits `dis_waitb` each
/// step, so the shell calls it once a tick (`mov cx,1`, `:927`, `:938`): a saturating step
/// on the module's own global-volume byte (`ds:[30h]`, the S3M header's offset 48, 0x40),
/// not `_np_mastervol`.
pub fn fade_music(player: &mut Player, step: u8) {
    player.set_global_volume(player.global_volume().saturating_sub(step));
}

/// Whether `fademusic` has nothing left to do.
#[cfg(test)]
pub fn is_faded(player: &Player) -> bool {
    player.global_volume() == 0
}

/// A 16-bit signed PCM WAV of what `Player::take_samples` hands over, unconverted, at the
/// frame rate [`wav_format`] gives.
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

    /// Write the header into an open writer (the seam a test drives a failing one through).
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

    /// Append samples. The size fields are patched on every write, so an interrupted run
    /// still leaves a valid WAV. The first failure is kept for [`AudioDump::error`].
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

    /// The first write that failed: a dump that stopped half way fails the run.
    pub fn error(&self) -> Option<&io::Error> {
        self.error.as_ref()
    }
}

/// The sound card, opened for the run.
pub struct Device {
    /// Kept for the run's lifetime, resumed (SDL opens it paused).
    device: AudioDevice<Drain>,
    /// The cell the callback's [`Drain`] reads.
    player: Swappable,
}

impl Device {
    /// Open the default playback device at exactly the mixer's rate and channel count, or
    /// `Err`: SDL's `allowed_changes` is zero, so the card never resamples. The callback's
    /// buffer size is SDL's choice, which [`open_for`] prints beside the ring's size.
    pub fn open(sdl: &sdl2::Sdl, player: Shared, rate: u32, channels: u8) -> Result<Device, String> {
        let audio = sdl.audio()?;
        let spec = AudioSpecDesired {
            freq: Some(rate as i32),
            channels: Some(channels),
            samples: None,
        };
        let cell: Swappable = Arc::new(Mutex::new(player));
        let drain = Drain { player: Arc::clone(&cell) };
        let device = audio.open_playback::<Drain, _, _>(None, &spec, |_spec| drain)?;
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

    /// Point the drain at the run's new player after a restart. The old ring's tail is
    /// dropped, as `restartmus` (`MAIN/U2.ASM:348-397`) interrupts the old module.
    pub fn follow(&mut self, player: Shared) {
        *cell(&self.player) = player;
    }
}

/// What a window run plays through: the WAV `--audio-dump` names (and no card, since the
/// ring has one consumer), nothing for `--no-audio` or a run with no music, or the sound
/// card. A machine without a usable card prints a line and runs silent.
pub fn open_for(
    sdl: &sdl2::Sdl,
    run: &mut Sequence,
    cli: &Cli,
) -> Result<Option<Device>, Box<dyn Error>> {
    if let Some(path) = cli.audio_dump.as_deref() {
        run.dump_audio(path)?;
        println!(
            "secondreality: writing the music to {} (16-bit WAV at the mixer's own rate); \
             no sound card is opened",
            path.display()
        );
        return Ok(None);
    }
    if cli.no_audio {
        return Ok(None);
    }
    let Some(player) = run.audio_player() else {
        return Ok(None);
    };
    // A run with a player has a format: both come from the same `Option<RunMusic>`.
    let Some((rate, channels)) = run.audio_format() else {
        return Ok(None);
    };
    match Device::open(sdl, player, rate, channels as u8) {
        Ok(device) => {
            let (opened_rate, opened_channels, opened_samples) = device.spec();
            let ring = lock(&device.player()).ring_capacity();
            println!(
                "secondreality: audio at {opened_rate} Hz, {opened_channels} channel(s), \
                 {opened_samples} frame(s) a callback, a ring of {ring} samples"
            );
            Ok(Some(device))
        }
        Err(message) => {
            println!("secondreality: no audio device ({message}); the run plays silent");
            Ok(None)
        }
    }
}

/// A run's WAV dump that stopped half way is the run's failure, not a shorter recording.
pub fn dump_result(run: &Sequence) -> Result<(), Box<dyn Error>> {
    match run.audio_error() {
        Some(error) => Err(format!("--audio-dump: {error}").into()),
        None => Ok(()),
    }
}

/// The device's callback, on SDL's audio thread: it drains the ring and nothing else, so
/// the music's position stays the tick's.
pub(crate) struct Drain {
    player: Swappable,
}

impl Drain {
    /// Fill `out` from the ring, zeroes for the rest, and say how many samples it gave up.
    /// The cell is locked only to clone the current player, so a callback in flight
    /// finishes against the player it started with.
    pub(crate) fn drain(&mut self, out: &mut [i16]) -> usize {
        let player = cell(&self.player).clone();
        let taken = lock(&player).take_samples(out);
        out[taken..].fill(0);
        taken
    }
}

impl AudioCallback for Drain {
    type Channel = i16;

    fn callback(&mut self, out: &mut [i16]) {
        self.drain(out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sr_audio::Quality;
    use std::path::PathBuf;

    fn shared(id: ModuleId) -> Shared {
        test_player(id).expect("the module loads")
    }

    fn drain_of(player: &Shared) -> Drain {
        Drain { player: Arc::new(Mutex::new(Arc::clone(player))) }
    }

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("sr-wav-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("dump.wav")
    }

    fn word(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    fn half(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    #[test]
    fn the_adapter_hands_the_engine_the_players_published_words() {
        // The published row and `zplus`, never the engine's own, ten polls ahead.
        let player = shared(ModuleId::Music0);
        let mut music = PlayerMusic::new(Arc::clone(&player), 0);
        music.poll();
        let published = lock(&player).state();
        assert_eq!(music.musrow(), i32::from(published.row));
        assert_eq!(
            music.musplus(),
            musplus_from(lock(&player).zplus(), i32::from(published.row))
        );
        assert_eq!(music.sync(), sync_from(published.order, published.row));
        music.setmframe(1516);
        assert_eq!(music.getmframe(), 1516, "`dis_setmframe` writes it through");
    }

    #[test]
    fn the_adapter_waits_out_the_frames_before_the_music_starts() {
        // A run anchored at 404 reaches the music's clock at 408: the trace's `_np_zframe`
        // is 0 at 404..=407.
        let player = shared(ModuleId::Music0);
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
    fn the_wav_format_is_the_players_frame_rate() {
        let enhanced = PlayerOptions { quality: Quality::Enhanced, ..PlayerOptions::default() };
        let player = test_music(ModuleId::Music0, enhanced).expect("MUSIC0 loads");
        assert_eq!(lock(&player).options(), enhanced, "the loader builds what it was asked for");
        assert_eq!(wav_format(&lock(&player).options()), (48_000, 2));
        assert_eq!(wav_format(&PlayerOptions::default()), (20_000, 2));
    }

    #[test]
    fn the_header_is_true_at_every_tick_not_only_at_the_end() {
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

    /// A file that takes the header and then refuses everything: a disk that filled up.
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
        let header = AudioDump::<File>::HEADER_BYTES as usize;
        let mut dump =
            AudioDump::over(FullAfterTheHeader { written: Vec::new(), room: header }, 20_000, 2)
                .expect("the header fits");
        assert_eq!(dump.samples(), 0);
        dump.write(&[1, 2, 3, 4]);
        assert!(dump.error().is_some(), "the write past the room failed");
        assert_eq!(dump.samples(), 0, "and nothing was counted as written");
        // The header's own write fails before the run.
        let error = AudioDump::over(FullAfterTheHeader { written: Vec::new(), room: 0 }, 20_000, 2)
            .err()
            .expect("a writer with no room cannot take a header");
        assert_eq!(error.kind(), io::ErrorKind::StorageFull);
    }

    #[test]
    fn the_fade_walks_the_modules_own_byte_down_to_zero_and_stops() {
        // `fademusic` (`MAIN/U2.ASM:333-346`): one step a call, saturating at zero
        // (`@@done`'s `mov byte ptr ds:[30h],0`), from the module's own 0x40.
        let shared = shared(ModuleId::Music0);
        let mut player = lock(&shared);
        let start = player.global_volume();
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

    #[test]
    fn an_empty_ring_under_runs_into_silence() {
        // A callback asking for more than a tick's samples gets them and zeroes.
        let player = shared(ModuleId::Music0);
        lock(&player).play_song(25);
        lock(&player).poll();
        let produced = lock(&player).samples_this_poll() as usize;
        assert!(produced > 0 && produced < 4096, "one tick is a few hundred samples");

        let mut drain = drain_of(&player);
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

        // With the ring empty a drain is silence, and does not tick the player.
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
        // `restartmus` at the U2E boundary (`MAIN/U2.ASM:880-885`): after the swap the
        // next drain takes the new ring's samples and none of the old player's.
        let old = shared(ModuleId::Music1);
        lock(&old).play_song(0);
        lock(&old).poll();
        lock(&old).poll();
        let old_pending = lock(&old).pending_samples();
        assert!(old_pending > 0, "the old player has samples waiting");

        let new = shared(ModuleId::Music0);
        lock(&new).play_song(18);
        lock(&new).poll();
        let new_samples = lock(&new).samples_this_poll() as usize;
        assert!(new_samples > 0, "and the new player mixes a poll's worth");
        assert_ne!(old_pending, new_samples, "the two rings are distinguishable by size");

        let mut drain = drain_of(&old);
        *cell(&drain.player) = Arc::clone(&new);
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
        let mut empty = vec![i16::MIN; 64];
        assert_eq!(drain.drain(&mut empty), 0);
        assert!(empty.iter().all(|sample| *sample == 0));
    }

    #[test]
    fn the_device_drains_at_the_mixers_own_rate_and_never_resamples() {
        // Every sample the mixer makes comes out exactly once.
        let player = shared(ModuleId::Music0);
        lock(&player).play_song(25);
        let mut drain = drain_of(&player);
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
        assert_eq!(lock(&player).dropped_samples(), 0);
        println!("40 polls: {produced} samples produced, {total} drained, none dropped");
    }
}
