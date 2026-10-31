//! The run: one engine, one output frame and one counter for a plan's parts, plus the shell's
//! own acts between them (the mid-run restarts, the cue wait, the hold, the exit stage and
//! the loop seam).

use std::io;
use std::time::Duration;

use sr_engine::music::MusicPosition;
use sr_engine::part::{Part, PartStatus};
use sr_engine::view::View;
use sr_engine::{Engine, IndexedFrame, Palette, Tick};

use super::dac::hardware_default_palette;
use super::idle::{CueWait, Idle};
use super::music::{
    music_for_part, ModuleId, MusicSource, MusicStart, LOOP_RESTART, MUSIC_RESTART_AT,
};
use super::plan::{boot_anchor, loop_plan};
use super::{start_view, ShellError, SURFACE_HEIGHT, SURFACE_WIDTH};
use crate::parts;

/// How a [`Sequence`] builds a part from a name; the binary's is [`parts::new_part`], and
/// tests substitute stubs.
pub(crate) type PartSource = fn(&str) -> Option<Box<dyn Part>>;

/// Whether a run is still going.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceStatus {
    Continuing,
    /// A part or a restart's music could not be started (a broken build or a missing asset);
    /// the error is printed and the run ends. A run whose parts have all retired is not
    /// finished: it [holds](Sequence::holding).
    Finished,
}

/// How a run ends once its plan has nothing left to play.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Ending {
    /// Hold the last part's screen for ever: `--part`, the vault and the shell's tests.
    #[default]
    Hold,
    /// `SECOND.EXE`'s way out (`MAIN/U2.ASM:935-994`, D41): [`EXIT_FADE_TICKS`] of music fade
    /// over the last picture, [`EXIT_BLANK_TICKS`] of blank screen, then
    /// [`ended`](Sequence::ended). A run built from a block.
    Exit,
}

/// The original's `looping` byte (`MAIN/U2.ASM:746`, the setup's `ip[3]`, `START.C:103`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Looping {
    /// 0: play once, with the end scroller (`MENU.C:233`).
    #[default]
    Off,
    /// 1: no end scroller (`:914-917`); after CRED the music fades, MUSIC0 restarts at order 0
    /// and the run begins again at ALKU (`:919-933`, D42). The port's `--loop`.
    On,
    /// 2: no end scroller and no loop (`:923-924`); only `/gravis` sets it (`START.C:89-93`).
    NoEndScroller,
}

/// What a run is asked to be, beyond its plan.
#[derive(Clone, Copy, Debug, Default)]
pub struct RunSettings {
    pub ending: Ending,
    /// Changes a run built from a block only: its plan ([`run_plan`](super::run_plan)) and
    /// what happens when the plan runs out.
    pub looping: Looping,
    /// The options every player the run installs is built with, restarts included (D43).
    pub player: sr_audio::PlayerOptions,
}

/// The exit stage's fade: `mov cx,1; call fademusic` (`MAIN/U2.ASM:937-939`) walks the
/// module's global volume from 0x40 to zero a `dis_waitb` at a time (`:333-346`).
pub const EXIT_FADE_TICKS: u32 = 64;

/// The exit stage's blank: `fatalexit` sets text mode (`MAIN/U2.ASM:967-968`) and waits
/// twenty `copperwaitborder`s (`:969-973`) before `printend` (`:974`).
pub const EXIT_BLANK_TICKS: u32 = 20;

/// The ticks between the loop fade's last and pass 2's ALKU, the last part's screen still up:
/// `restartmus`'s reload (`MAIN/U2.ASM:347-396`) and ALKU's load (`ALKU/MAIN.C:158`). The
/// release's measured 9093–9103 (`reference-indices.md` §25.3, D42); the port's residual is
/// A61, pinned in `tests/loop.rs`.
pub const LOOP_RELOAD_TICKS: u32 = 11;

/// A run of the demo's parts, numbered in the reference's frame space.
///
/// The counter starts at the run's [`boot_anchor`] and advances one a tick, whoever drew the
/// frame (D21). Nothing is cleared between parts: the original's parts are separate EXEs that
/// leave the screen and the DAC as they found them. When the plan runs out the run holds the
/// last screen (BEGLOGO's 900 is that hold, `reference-indices.md` §3.5); an [`Ending::Exit`]
/// run's hold is the exit stage, and a [`Looping::On`] run goes round through the loop seam.
pub struct Sequence {
    engine: Engine,
    source: PartSource,
    /// The parts this run consists of, in the demo's order.
    plan: Vec<&'static str>,
    /// How far into `plan` the run is.
    next: usize,
    live: Option<Box<dyn Part>>,
    /// The plan part whose screen is up; a retired part keeps its name until the next starts.
    current: &'static str,
    frame: IndexedFrame,
    /// The reference index the next tick draws at.
    index: u32,
    anchor: Option<u32>,
    clock: Tick,
    stage: Stage,
    music: Option<RunMusic>,
    /// How the run builds a player, kept for the restarts; `None` for a silent run.
    music_source: Option<MusicSource>,
    player_options: sr_audio::PlayerOptions,
    /// Whether `panicend` just retired by itself, so the next part is the cue wait
    /// (`MAIN/U2.ASM:826-853`). The exit flag retiring it is the wait's own `bx=2` check.
    cue_pending: bool,
    ending: Ending,
    looping: Looping,
    /// 1, and one more each time a loop seam starts ALKU again.
    pass: u32,
    /// Every plan part started, at the index its first tick drew at, across passes.
    starts: Vec<(u32, &'static str)>,
}

/// Where a run is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stage {
    /// Playing the plan: `live` is a plan part, or nothing between two.
    Plan,
    /// `live` is the post-PANICEND [`CueWait`].
    Cue,
    /// `live` is the loop seam, this many ticks in.
    Seam(u32),
    /// The plan has run out and `live` holds its last screen; the exit stage's ticks so far
    /// ([`Ending::Exit`] only).
    Hold(u32),
    /// A part or a restart's music could not be started; the error is printed and the run
    /// ends.
    Finished,
}

/// The run's music: the player, the module and order it began at, how its clock was started,
/// and the `--audio-dump` it feeds.
struct RunMusic {
    /// Shared with the engine's adapter and the sound card; a restart replaces it.
    player: crate::audio::Shared,
    module: ModuleId,
    order: u8,
    start: MusicStart,
    /// One output stream for the run: a dump or the sound card, never both.
    dump: Option<crate::audio::AudioDump>,
    /// The tick's samples, drained into `dump`.
    scratch: Vec<i16>,
}

impl Sequence {
    /// A run of `plan`'s registered parts on the shell's surface, with the demo's music. The
    /// first part starts here, so a part that cannot start is an error the caller sees.
    pub fn new(plan: Vec<&'static str>, settings: RunSettings) -> Result<Sequence, ShellError> {
        Sequence::with_run(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            start_view(),
            plan,
            parts::new_part,
            Some(crate::audio::load_player),
            settings,
        )
    }

    /// [`Sequence::new`] with the parts from `source` and no music, for tests.
    #[cfg(test)]
    pub(crate) fn with_source(
        surface_width: usize,
        surface_height: usize,
        view: View,
        plan: Vec<&'static str>,
        source: PartSource,
    ) -> Result<Sequence, ShellError> {
        Sequence::with_run(
            surface_width,
            surface_height,
            view,
            plan,
            source,
            None,
            RunSettings::default(),
        )
    }

    /// A run with the parts from `source` and the player from `music`, or no music for `None`.
    ///
    /// The run's music is its entry point's: the plan's first name decides it
    /// ([`music_for_part`]), so `--block 5` and `--part endlogo` both start MUSIC0 at 25.
    pub(crate) fn with_run(
        surface_width: usize,
        surface_height: usize,
        view: View,
        plan: Vec<&'static str>,
        source: PartSource,
        music: Option<MusicSource>,
        settings: RunSettings,
    ) -> Result<Sequence, ShellError> {
        let Some(&first) = plan.first() else {
            return Err(ShellError::EmptyPlan);
        };
        let anchor = boot_anchor(first);
        let frame = IndexedFrame::new(view.width, view.height);
        let mut run = Sequence {
            engine: Engine::new(surface_width, surface_height, view),
            source,
            plan,
            next: 1,
            live: None,
            current: first,
            frame,
            index: anchor.unwrap_or(0),
            anchor,
            clock: Tick::new(),
            stage: Stage::Plan,
            music: None,
            music_source: music,
            player_options: settings.player,
            cue_pending: false,
            ending: settings.ending,
            looping: settings.looping,
            pass: 1,
            starts: Vec::new(),
        };
        *run.engine.palette_mut() = hardware_default_palette();
        if let (Some(loader), Some((module, order))) = (music, music_for_part(first)) {
            let start = MusicStart::for_part(first, anchor);
            run.install_music(module, order, loader, start)?;
        }
        run.start(first)?;
        Ok(run)
    }

    /// Load the module, seek it, and hand the engine the adapter that polls it, in the
    /// original's order (`MAIN/U2.ASM:775`'s `restartmus`, then `:394`'s or `:786`'s
    /// `_zplaysong`).
    ///
    /// A restart is a new player that takes the old one's state over first
    /// (`Player::restart_from`, D45): `_zplaysong` keeps what the song initialiser does not
    /// write. The dump and scratch carry across; they belong to the run's one output stream.
    fn install_music(
        &mut self,
        module: ModuleId,
        order: u8,
        source: MusicSource,
        start: MusicStart,
    ) -> Result<(), ShellError> {
        let player =
            source(module, self.player_options).map_err(|message| ShellError::Music { message })?;
        // Only once the new player exists is the old one taken, so a failed load keeps the dump.
        let (dump, scratch) = match self.music.take() {
            Some(old) => {
                crate::audio::lock(&player).restart_from(&crate::audio::lock(&old.player), order);
                (old.dump, old.scratch)
            }
            None => {
                crate::audio::lock(&player).play_song(order);
                (None, vec![0; 1024])
            }
        };
        // The frames the music missed before the run began (`MusicStart`). A catch-up longer
        // than the ring (8192 samples) drops its oldest samples, as an undrained device does.
        for _ in 0..start.pre {
            crate::audio::lock(&player).poll();
        }
        self.engine.set_music(Box::new(crate::audio::PlayerMusic::new(
            std::sync::Arc::clone(&player),
            start.waiting,
        )));
        self.music = Some(RunMusic {
            player,
            module,
            order,
            start,
            dump,
            scratch,
        });
        Ok(())
    }

    /// One logic tick. `Finished` only when the run has ended.
    pub fn tick(&mut self) -> SequenceStatus {
        if self.stage == Stage::Finished {
            return SequenceStatus::Finished;
        }
        if self.live.is_none() && !self.take_next() {
            return SequenceStatus::Finished;
        }
        let part = self.live.as_mut().expect("a part is live");
        // A draining part (U2E's post-exit fade, `U2E.C:541-555`) retires only by `Finished`.
        let drains = part.drains_after_exit();
        let status = self.engine.tick(&mut **part, &mut self.frame);
        self.index += 1;
        self.exit_stage();
        if status == PartStatus::Finished || (self.engine.exit_requested() && !drains) {
            // PANICEND's own `Finished` is its EXE ending, which reaches `@@zh5`.
            let panicend = self.stage == Stage::Plan && self.current == "panicend";
            if status == PartStatus::Finished && panicend {
                self.cue_pending = true;
            }
            self.live = None;
            if self.stage == Stage::Cue {
                self.stage = Stage::Plan;
            }
        }
        self.music_after_tick();
        // After the fade step, so the restart installed at the fade's end is never faded.
        self.loop_seam();
        SequenceStatus::Continuing
    }

    /// The loop seam (`MAIN/U2.ASM:919-933`), on a tick it has composed: the fade for
    /// [`EXIT_FADE_TICKS`] (`:926-928`, taken by `music_after_tick`), then [`LOOP_RESTART`]
    /// under [`MusicStart::restart`] (`:929-932`), then [`LOOP_RELOAD_TICKS`] more, and
    /// `jmp @@partsagain` (`:933`) to block 1's plan.
    fn loop_seam(&mut self) {
        let Stage::Seam(ticks) = &mut self.stage else {
            return;
        };
        *ticks += 1;
        let ticks = *ticks;
        if ticks == EXIT_FADE_TICKS {
            if let Some(source) = self.music_source.filter(|_| self.music.is_some()) {
                let (module, order) = LOOP_RESTART;
                let start = MusicStart::restart(self.index);
                if let Err(error) = self.install_music(module, order, source, start) {
                    self.end_run(error);
                    return;
                }
            }
        }
        if ticks == EXIT_FADE_TICKS + LOOP_RELOAD_TICKS {
            self.stage = Stage::Plan;
            self.live = None;
            // The pass is counted when its first part starts.
            self.plan = loop_plan();
            self.next = 0;
        }
    }

    /// The exit stage (`MAIN/U2.ASM:935-994`), on a tick the hold has composed: the first
    /// [`EXIT_FADE_TICKS`] are the hold's frames under the music's fade, then `fatalexit`'s
    /// text mode (`:967-973`) composes every pixel as index 0 through a black palette for
    /// [`EXIT_BLANK_TICKS`]. The border is left as the part set it, so the window keeps its
    /// size.
    fn exit_stage(&mut self) {
        let Stage::Hold(ticks) = &mut self.stage else {
            return;
        };
        if self.ending != Ending::Exit {
            return;
        }
        *ticks = ticks.saturating_add(1);
        if *ticks > EXIT_FADE_TICKS {
            self.frame.pixels.fill(0);
            self.frame.palettes.reset(Palette::BLACK);
        }
    }

    /// The shell's music after a tick has drawn: `fademusic`'s step a tick while the run
    /// holds (`@@partsdone`, `MAIN/U2.ASM:935-942`) or a loop seam fades (`:926-928`), and
    /// the tick's samples to the dump.
    fn music_after_tick(&mut self) {
        let Some(music) = self.music.as_mut() else {
            return;
        };
        if matches!(self.stage, Stage::Hold(_))
            || matches!(self.stage, Stage::Seam(ticks) if ticks < EXIT_FADE_TICKS)
        {
            crate::audio::fade_music(&mut crate::audio::lock(&music.player), 1);
        }
        if let Some(dump) = music.dump.as_mut() {
            // A pre-polled run starts with more in the ring than one scratch holds.
            loop {
                let n = crate::audio::lock(&music.player).take_samples(&mut music.scratch);
                if n == 0 {
                    break;
                }
                dump.write(&music.scratch[..n]);
                if n < music.scratch.len() {
                    break;
                }
            }
        }
    }

    /// Put a part in `live` for this tick: the cue wait when one is due, the plan's next part,
    /// the loop seam or the hold. False only when the run has ended.
    fn take_next(&mut self) -> bool {
        if matches!(self.stage, Stage::Hold(_)) {
            return true;
        }
        if self.cue_pending {
            self.cue_pending = false;
            // `@@zh5` checks `dis_exit` before the music and skips the wait. PANICEND never
            // polls the flag (`SHUTDOWN.C`), so ESC on its last tick leaves the flag set, and
            // the cue's `engine.start` would clear it before the cue saw it.
            if !self.engine.exit_requested() {
                return self.start_idle(Box::new(CueWait::default()), Stage::Cue);
            }
        }
        let Some(&name) = self.plan.get(self.next) else {
            // `:920-925`: looping 1 goes round; 0 and 2 reach `@@partsdone`.
            let (drains, stage) = match self.looping {
                Looping::On => (true, Stage::Seam(0)),
                _ => (false, Stage::Hold(0)),
            };
            return self.start_idle(Box::new(Idle { of: self.current, drains }), stage);
        };
        // Only a loop seam rewinds `next` to 0.
        if self.next == 0 {
            self.pass += 1;
        }
        self.next += 1;
        // A restart site runs before `partexecute`, so the part's `init` sees the new player.
        if let Err(error) = self.restart_music(name).and_then(|()| self.start(name)) {
            return self.end_run(error);
        }
        true
    }

    /// Install [`MUSIC_RESTART_AT`]'s player when `name` is a restart site, unless the run
    /// already plays that module and order (block 2 at GLENZ, whose `:813-818` guard is
    /// clear and whose initial start is the same MUSIC1@0).
    fn restart_music(&mut self, name: &'static str) -> Result<(), ShellError> {
        let Some(&(_, module, order)) = MUSIC_RESTART_AT
            .iter()
            .find(|(part, _, _)| *part == name)
        else {
            return Ok(());
        };
        let Some(source) = self.music_source else {
            return Ok(());
        };
        let already = self
            .music
            .as_ref()
            .is_some_and(|music| music.module == module && music.order == order);
        if already {
            return Ok(());
        }
        self.install_music(module, order, source, MusicStart::restart(self.index))
    }

    /// Start one of the shell's own parts as `live`, at `stage`: the cue wait, the loop seam
    /// ([`loop_seam`](Sequence::loop_seam)) or the hold. `current` stays the part whose
    /// screen is up.
    fn start_idle(&mut self, mut part: Box<dyn Part>, stage: Stage) -> bool {
        if let Err(error) = self.engine.start(&mut *part) {
            return self.end_run(error);
        }
        self.live = Some(part);
        self.stage = stage;
        true
    }

    /// End the run on `error`, which `tick` has no channel to return. Returns false.
    fn end_run(&mut self, error: impl std::fmt::Display) -> bool {
        eprintln!("secondreality: {error}; the run ends here");
        self.stage = Stage::Finished;
        false
    }

    /// Feed wall-clock time: tick once per due frame, dropping the stale ones. Returns the
    /// ticks taken, none once the run has finished.
    pub fn advance(&mut self, elapsed: Duration) -> u32 {
        if self.stage == Stage::Finished {
            return 0;
        }
        let due = self.clock.advance(elapsed);
        let mut steps = 0;
        while steps < due {
            if self.tick() == SequenceStatus::Finished {
                break;
            }
            steps += 1;
        }
        steps
    }

    /// Start `name` as the part this run is on.
    fn start(&mut self, name: &'static str) -> Result<(), ShellError> {
        let mut part = (self.source)(name).ok_or_else(|| ShellError::UnknownPart {
            name: name.to_string(),
        })?;
        self.engine.start(&mut *part)?;
        self.live = Some(part);
        self.current = name;
        self.stage = Stage::Plan;
        self.starts.push((self.index, name));
        Ok(())
    }

    /// The last presented frame, drawn at `index() - 1`.
    pub fn frame(&self) -> &IndexedFrame {
        &self.frame
    }

    /// The reference index the next tick draws at.
    pub fn index(&self) -> u32 {
        self.index
    }

    /// The part whose screen is up: after a tick, the one that drew the frame.
    pub fn part(&self) -> &'static str {
        self.current
    }

    /// Whether the run's indices are the reference's own; false for a run numbered from zero.
    pub fn index_is_reference(&self) -> bool {
        self.anchor.is_some()
    }

    /// The reference index the run began at, or `None`.
    pub fn anchor(&self) -> Option<u32> {
        self.anchor
    }

    /// Whether a part or the music could not be started and the run has ended.
    pub fn finished(&self) -> bool {
        self.stage == Stage::Finished
    }

    /// Whether the exit stage has run its fade and blank ([`Ending::Exit`] only). The run
    /// still ticks, composing the blank.
    pub fn ended(&self) -> bool {
        matches!(self.stage, Stage::Hold(ticks) if ticks >= EXIT_FADE_TICKS + EXIT_BLANK_TICKS)
    }

    /// Whether the plan has run out and the run holds the last screen (`@@partsdone`,
    /// `MAIN/U2.ASM:935-968`): an [`Ending::Exit`] run's exit stage, or an [`Ending::Hold`]
    /// run's hold until the window closes.
    pub fn holding(&self) -> bool {
        matches!(self.stage, Stage::Hold(_))
    }

    /// The demo's pass: 1, and one more each time a loop seam starts ALKU again
    /// (`MAIN/U2.ASM:933`).
    pub fn pass(&self) -> u32 {
        self.pass
    }

    /// Every plan part the run has started, as `(index of its first tick, name)`, across
    /// passes. The cue wait, the seam and the hold are the shell's, not starts; a part
    /// skipped with Ctrl is. The port's column of the full run's seam table (`FULLRUN_SEAMS`).
    pub fn starts(&self) -> &[(u32, &'static str)] {
        &self.starts
    }

    /// The music's published position (the ring's, ten polls behind the player's own,
    /// `music.md` §8), or `None` for a silent run.
    pub fn music_position(&self) -> Option<MusicPosition> {
        self.engine.music().map(|music| music.position())
    }

    /// The module the run plays and the order it was seeked to.
    pub fn music_module(&self) -> Option<(ModuleId, u8)> {
        self.music.as_ref().map(|music| (music.module, music.order))
    }

    /// The current player's [`MusicStart::frame`].
    pub fn music_start_frame(&self) -> Option<u32> {
        self.music.as_ref().map(|music| music.start.frame)
    }

    /// The module's global-volume byte, which `fademusic` (`MAIN/U2.ASM:333-346`) walks.
    #[cfg(test)]
    pub fn music_volume(&self) -> Option<u8> {
        self.music
            .as_ref()
            .map(|music| crate::audio::lock(&music.player).global_volume())
    }

    /// The player, for the sound card's callback.
    pub fn audio_player(&self) -> Option<crate::audio::Shared> {
        self.music.as_ref().map(|music| std::sync::Arc::clone(&music.player))
    }

    /// The mixer's stream as a WAV header describes it, `(frames a second, channels)`, from
    /// the player's own options.
    pub fn audio_format(&self) -> Option<(u32, u16)> {
        self.music.as_ref().map(|music| {
            crate::audio::wav_format(&crate::audio::lock(&music.player).options())
        })
    }

    /// Send every tick's samples to `path` as a 16-bit WAV (`--audio-dump`), in place of the
    /// sound card: the ring has one consumer.
    pub fn dump_audio(&mut self, path: &std::path::Path) -> std::io::Result<()> {
        // `Cli::validate` refuses this for a silent run; this is the same rule.
        let (rate, channels) = self.audio_format().ok_or_else(|| {
            io::Error::other("this run has no music to dump")
        })?;
        let dump = crate::audio::AudioDump::create(path, rate, channels)?;
        let music = self.music.as_mut().expect("audio_format answered, so there is music");
        music.dump = Some(dump);
        Ok(())
    }

    /// The first write `--audio-dump` could not make: a dump that stopped half way is a
    /// failure.
    pub fn audio_error(&self) -> Option<String> {
        self.music
            .as_ref()
            .and_then(|music| music.dump.as_ref())
            .and_then(|dump| dump.error())
            .map(|error| error.to_string())
    }

    /// The reference index of the frame [`frame`](Sequence::frame) holds, or `None` before
    /// the first tick.
    pub fn frame_index(&self) -> Option<u32> {
        if self.index > self.anchor.unwrap_or(0) {
            Some(self.index - 1)
        } else {
            None
        }
    }

    /// ESC: set the exit flag for the running part (`DIS/DISINT.ASM:151-153`), which sees it
    /// in its next tick and retires at the end of it (`exit_2`, `:186-190`). False when no
    /// part is live or the run holds (`@@partsdone`, `MAIN/U2.ASM:935`). In a loop seam the
    /// press is taken and does nothing: the seam drains, and pass 2's start clears the flag.
    pub fn request_exit(&mut self) -> bool {
        if matches!(self.stage, Stage::Hold(_)) || self.live.is_none() {
            return false;
        }
        self.engine.request_exit();
        true
    }

    /// Ctrl: drop the running part where it stands, and the next tick starts the next one
    /// at the index it would have drawn (`MAIN/U2.ASM:205-207`). The original's `ctrldown`
    /// (`:185-203`) keeps skipping every later part; the port skips one (D21, A58). False
    /// when there is no part: the hold, or a loop seam.
    pub fn skip_part(&mut self) -> bool {
        let idle = matches!(self.stage, Stage::Hold(_) | Stage::Seam(_) | Stage::Finished);
        if idle || self.live.is_none() {
            return false;
        }
        self.live = None;
        true
    }
}

#[cfg(test)]
mod tests;
