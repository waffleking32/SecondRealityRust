//! The command line: everything the binary can be asked to do.
//!
//! Parsing is pure — no SDL, no filesystem, no clock — and it is where the
//! headless/window split is decided, before anything is initialised. Headless
//! mode therefore reaches a machine with no display at all; see `main`.

use std::path::PathBuf;
use std::str::FromStr;

use sr_audio::{PlayerOptions, Quality};
use sr_engine::present::Aspect;

use crate::parts;
use crate::shell;

/// The part [`Cli::part_name`] falls back to for a `Cli` that names neither a
/// part nor a block — one built by hand, since the command line never leaves
/// both unset. A run with no argument is the demo: `SECOND` with no argument is
/// `SECOND 1` (`part-sequence.md` §4), so [`Cli::parse`] makes it
/// [`DEFAULT_BLOCK`]. The Phase 0 test card is still a part, and `--part
/// testcard` names it.
const DEFAULT_PART: &str = "testcard";

/// The block a command line that names no part and no block runs: `SECOND 1`,
/// which the original treats exactly as no argument (`MAIN/U2.ASM:108`'s default
/// `whattorun` 7fh and `partsmask[1]`'s 1fh gate the same parts).
const DEFAULT_BLOCK: char = '1';

/// What to run, and how to show it.
#[derive(Debug, Clone)]
pub struct Cli {
    /// The part to run, one of [`parts::registry`]'s names. `None` when the
    /// command line named none.
    pub part: Option<&'static str>,
    /// A `SECOND n` entry point, whose parts are the ported ones in the demo's
    /// own order from that block's first part. Kept separate from [`part`] so
    /// that "neither was given" is a state and not a string comparison.
    ///
    /// [`part`]: Cli::part
    pub block: Option<char>,
    /// Dump frames to disk instead of opening a window.
    pub headless: bool,
    /// Where the dump goes. Required with `--headless`.
    pub dump_dir: Option<PathBuf>,
    /// Stop after this many logic ticks, dumping every one. One of the three
    /// ways a headless run is bounded.
    pub frames: Option<u32>,
    /// Dump exactly these reference indices, and stop after the largest. One of
    /// the three ways a headless run is bounded, and the only one that is
    /// sparse: a part's keyframes are four numbers, and `--frames 900` is 900
    /// frames of 320x400 output to answer them.
    pub dump_frames: Option<Vec<u32>>,
    /// Drive from real elapsed time for this long, dumping only the last frame.
    pub run_seconds: Option<f64>,
    /// Display magnification; 1 or more.
    pub scale: u32,
    /// How the picture is fitted to the output raster.
    pub aspect: Aspect,
    /// Whether to draw the frame's overscan colour as a border.
    pub border: bool,
    /// Write the music to this file as a 16-bit WAV instead of opening a sound card.
    ///
    /// The ring has one consumer, so a dump *replaces* the device rather than joining it:
    /// a run that writes a file is silent to watch. The artifact is the point — a person
    /// listens to it and a test reads its bytes — and no reference audio exists to compare
    /// against (`docs/fidelity/music.md` §10.5).
    pub audio_dump: Option<PathBuf>,
    /// Do not open a sound card. The player still runs: the music's position is the tick's
    /// and this only stops the tick's samples from being played.
    pub no_audio: bool,
    /// Open the vault menu instead of a run: a window-only selection among the
    /// demo's three unreleased effects (`parts::vault::VAULT`).
    ///
    /// The menu starts the parts itself, one at a time, so every flag that
    /// names a run or bounds a dump contradicts it — [`validate`](Cli::validate)
    /// refuses those. The presentation flags (`--scale`, `--aspect`,
    /// `--no-border`, `--no-audio`) apply to it and to the runs it starts.
    pub vault: bool,
    /// The original's `looping` (`MAIN/U2.ASM:746`): `--loop` is the setup menu's
    /// "Demo looping: Enabled" (`MENU.C:236`, 1), and `/gravis` is 2 (`START.C:89-93`).
    /// Only a block's run loops.
    pub looping: shell::Looping,
    /// Whether the run asks for its settings first — the original's setup screen, which
    /// `START.C:89-95` skips for `/gravis` and runs (`menu()`) otherwise.
    pub setup: bool,
    /// Write the run's timeline to this file: one CSV line a tick, `index,pass,part,order,row`
    /// — the frame's index, the loop pass, the part whose picture is up, and the music's
    /// published position. Headless only; what a full-run comparison against the traces
    /// reads (`tests/fullrun.rs`).
    pub positions: Option<PathBuf>,
    /// How the music is mixed (`--sound-quality authentic|enhanced`): the object's own mixer,
    /// or the port's enhanced renderer at 48 kHz (D43). Authentic by default; the clock and
    /// the pictures are the same either way.
    pub quality: Quality,
}

impl Default for Cli {
    /// Nothing named yet, in a window, 1:1 in a 4:3 box with its border.
    /// [`Cli::parse`] turns "nothing named" into the demo ([`DEFAULT_BLOCK`]).
    fn default() -> Cli {
        Cli {
            part: None,
            block: None,
            headless: false,
            dump_dir: None,
            frames: None,
            dump_frames: None,
            run_seconds: None,
            scale: 1,
            aspect: Aspect::FourThree,
            border: true,
            audio_dump: None,
            no_audio: false,
            vault: false,
            looping: shell::Looping::Off,
            setup: true,
            positions: None,
            quality: Quality::Authentic,
        }
    }
}

impl Cli {
    /// Parse the arguments after the program name.
    ///
    /// Every rule that spans more than one argument is checked here, so once a
    /// `Cli` exists the rest of the binary may rely on it: a headless run has a
    /// dump directory and exactly one way of deciding when to stop.
    pub fn parse<I>(args: I) -> Result<Cli, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut cli = Cli::default();
        let mut gravis = false;
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--part" => {
                    let name = value(&mut args, "--part")?;
                    // The shell decides which names exist, because it is the
                    // only place that knows which of the demo's parts are the
                    // port's: a name the demo has and the port has not reached
                    // is a task someone can pick up, and a name the demo never
                    // had is a typo; the two used to say so.
                    cli.part = Some(
                        shell::resolve(&name)
                            .map_err(|e| format!("{e}; ported parts: {}", known_parts()))?,
                    );
                }
                "--block" => {
                    let raw = value(&mut args, "--block")?;
                    let block = exactly_one_char(&raw)?.to_ascii_uppercase();
                    // Known is checked here and *runnable* in `validate`, which
                    // only runs once the whole command line is in: the two
                    // rules are different things, and `--part x --block 2` is a
                    // contradiction whether or not BEGLOGO is ported yet.
                    shell::block_plan(block).map_err(|_| {
                        format!("unknown block {raw:?}; expected {}", known_blocks())
                    })?;
                    cli.block = Some(block);
                }
                "--frames" => cli.frames = Some(number(&mut args, "--frames")?),
                "--dump-frames" => {
                    cli.dump_frames = Some(indices(&value(&mut args, "--dump-frames")?)?)
                }
                "--run-seconds" => cli.run_seconds = Some(number(&mut args, "--run-seconds")?),
                "--dump-dir" => cli.dump_dir = Some(PathBuf::from(value(&mut args, "--dump-dir")?)),
                "--scale" => cli.scale = number(&mut args, "--scale")?,
                "--aspect" => {
                    let raw = value(&mut args, "--aspect")?;
                    cli.aspect = match raw.as_str() {
                        "4:3" => Aspect::FourThree,
                        "square" => Aspect::SquarePixels,
                        other => {
                            return Err(format!(
                                "unknown aspect {other:?}; expected 4:3 or square"
                            ))
                        }
                    };
                }
                "--no-border" => cli.border = false,
                "--headless" => cli.headless = true,
                "--audio-dump" => {
                    cli.audio_dump = Some(PathBuf::from(value(&mut args, "--audio-dump")?))
                }
                "--no-audio" => cli.no_audio = true,
                "--positions" => {
                    cli.positions = Some(PathBuf::from(value(&mut args, "--positions")?))
                }
                "--sound-quality" => {
                    let raw = value(&mut args, "--sound-quality")?;
                    cli.quality = match raw.as_str() {
                        "authentic" => Quality::Authentic,
                        "enhanced" => Quality::Enhanced,
                        other => {
                            return Err(format!(
                                "unknown sound quality {other:?}; expected authentic or enhanced"
                            ))
                        }
                    };
                }
                "--vault" => cli.vault = true,
                "--loop" => cli.looping = shell::Looping::On,
                // The setup screen is on by default for a windowed demo run
                // ([`Cli::shows_setup`]); this is the way to start without it, as
                // `/gravis` does in the original (`START.C:89-95`) — the flags are then
                // the run's settings.
                "--no-setup" => cli.setup = false,
                // `START.C:89`: `memicmp(argv[1],"/gravis",7)` — the first seven
                // characters, any case — is `m_looping=2` and no setup menu. Applied after
                // the loop, so that it wins over `--loop` in either order.
                switch if is_gravis(switch) => gravis = true,
                // `MAIN/U2.ASM:605-606`: `checkcmdline` returns at a `/`, so any other
                // switch is no block and no error. It is ignored here too.
                switch if switch.starts_with('/') => {}
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        // `START.C:89-94`: `/gravis` sets `m_looping=2` and skips `menu()`, the only
        // place `looping` 1 is chosen (`MENU.C:229-238`). `--loop` is that menu's choice,
        // so `/gravis` wins over it wherever the two are written (D42).
        if gravis {
            cli.looping = shell::Looping::NoEndScroller;
            cli.setup = false;
        }
        // No part and no block is the demo's own run, `SECOND 1`. The vault names
        // neither and starts its own runs, so it is left alone.
        if cli.part.is_none() && cli.block.is_none() && !cli.vault {
            cli.block = Some(DEFAULT_BLOCK);
        }
        cli.validate()?;
        Ok(cli)
    }

    /// Whether the run opens on the settings screen ([`crate::settings`]): a **windowed
    /// demo run** — one built from a block, which is also the run with no argument — that
    /// has not skipped it (`--no-setup`, `/gravis`). Never `--part` or `--vault`, whose
    /// runs are not the demo's, and never `--headless`, which opens no window and waits on
    /// no key.
    pub fn shows_setup(&self) -> bool {
        self.setup && !self.headless && !self.vault && self.block.is_some()
    }

    /// The part a one-part run names, whether `--part` gave it or not.
    ///
    /// Only meaningful for a run that is not a block's: a block names a set of
    /// parts and [`block`](Cli::block) is how a caller tells.
    pub fn part_name(&self) -> &'static str {
        self.part.unwrap_or(DEFAULT_PART)
    }

    /// What the run this command line names is to be, beyond its plan.
    ///
    /// A run built from a block — `--block`, or no argument — is the demo's own and
    /// ends the way `SECOND.EXE` does ([`Ending::Exit`](shell::Ending::Exit)); one part
    /// holds where it ends ([`Ending::Hold`](shell::Ending::Hold)), and so do the
    /// vault's, which name no block.
    pub fn run_settings(&self) -> shell::RunSettings {
        shell::RunSettings {
            ending: if self.block.is_some() {
                shell::Ending::Exit
            } else {
                shell::Ending::Hold
            },
            // `validate` refuses looping for anything but a block, so this is the
            // command line's own word.
            looping: self.looping,
            player: PlayerOptions {
                quality: self.quality,
                ..PlayerOptions::default()
            },
        }
    }

    /// The rules that span more than one argument.
    fn validate(&self) -> Result<(), String> {
        if self.scale < 1 {
            return Err("--scale is a magnification, so it must be at least 1".to_string());
        }
        // The vault menu is the window's own surface: it starts runs itself, and
        // a flag that names one (or bounds a dump of it) contradicts it. The
        // check comes before the headless rules so that the headless vault says
        // *why* it cannot exist, rather than asking for a dump directory.
        if self.vault {
            if self.looping != shell::Looping::Off {
                return Err("the vault's parts do not loop; --loop and /gravis are a block's run"
                    .to_string());
            }
            // GRID, HARD and TWIST are silent (`shell::music_for_part` has no row for them), so
            // a mixer setting would be accepted and do nothing.
            if self.quality != Quality::Authentic {
                return Err("the vault's parts are silent; --sound-quality is for a run with music"
                    .to_string());
            }
            if self.headless
                || self.part.is_some()
                || self.block.is_some()
                || self.dump_dir.is_some()
                || self.dump_frames.is_some()
                || self.frames.is_some()
                || self.run_seconds.is_some()
                || self.audio_dump.is_some()
                || self.positions.is_some()
            {
                return Err("the vault menu needs a window; name a part with --part".to_string());
            }
            return Ok(());
        }
        if self.block.is_some() && self.part.is_some() {
            return Err("give --part or --block, not both".to_string());
        }
        // Looping is the demo's sequence going round (`MAIN/U2.ASM:914-933`); one part
        // holds where it ends, and the vault (refused above for its own reasons) starts
        // parts.
        if self.looping != shell::Looping::Off && self.block.is_none() {
            return Err("--loop and /gravis are a block's run; give --block, not --part".to_string());
        }
        // A mixer setting for a run with no player would be accepted and do nothing: the
        // test card and the vault's effects are silent (`music_for_part` has no row for
        // them). The default names the authentic mixer and is harmless, so it is accepted.
        if self.quality != Quality::Authentic && shell::music_for_part(self.first_part()?).is_none()
        {
            return Err(format!(
                "--sound-quality enhanced needs a run with music, and a run of {} has none",
                self.first_part()?
            ));
        }
        // A dump of a run with no music would be a file with a header and nothing in it,
        // which reads as a recording that captured silence. Which runs have music is the
        // shell's answer (`music_for_part`), not this module's: the test card is the port's
        // own and not the demo's, so a run of it has no player at all.
        if self.audio_dump.is_some() && shell::music_for_part(self.first_part()?).is_none() {
            return Err(format!(
                "--audio-dump needs a run with music, and a run of {} has none",
                self.first_part()?
            ));
        }
        // The block's first part has to be one the port can run, in **either**
        // mode: the window drives the same `Sequence` a headless run dumps, so
        // a block it cannot start at is a command-line mistake wherever it is
        // asked for. Checked before the mode split — and here rather than where
        // `--block` is read, so that a command line that contradicts itself
        // says *that* first.
        if let Some(block) = self.block {
            let first = self.first_part()?;
            shell::resolve(first).map_err(|e| format!("--block {block}: {e}"))?;
        }
        if !self.headless {
            // Silently ignoring a dump would be a trap: the run would look
            // right and leave nothing behind to compare.
            if self.frames.is_some()
                || self.dump_frames.is_some()
                || self.run_seconds.is_some()
                || self.dump_dir.is_some()
            {
                return Err(
                    "--frames, --dump-frames, --run-seconds and --dump-dir need --headless"
                        .to_string(),
                );
            }
            if self.positions.is_some() {
                return Err("--positions needs --headless".to_string());
            }
            return Ok(());
        }
        if self.dump_dir.is_none() {
            return Err("--headless needs --dump-dir".to_string());
        }
        let ways_to_bound = [
            self.frames.is_some(),
            self.dump_frames.is_some(),
            self.run_seconds.is_some(),
        ];
        match ways_to_bound.into_iter().filter(|given| *given).count() {
            1 => {}
            0 => {
                return Err(
                    "--headless needs --frames, --dump-frames or --run-seconds".to_string()
                )
            }
            _ => return Err("give one of --frames, --dump-frames, --run-seconds".to_string()),
        }
        if let Some(frames) = self.frames {
            if frames == 0 {
                return Err("--frames must be at least 1".to_string());
            }
        }
        if let Some(seconds) = self.run_seconds {
            if !(seconds.is_finite() && seconds > 0.0) {
                return Err("--run-seconds must be a positive number of seconds".to_string());
            }
        }
        // The timeline is one line a tick, and a wall-clock run takes its ticks in batches.
        if self.positions.is_some() && self.run_seconds.is_some() {
            return Err(
                "--positions records every tick; bound the run with --frames or --dump-frames"
                    .to_string(),
            );
        }
        self.dump_indices_are_reachable()
    }

    /// The part a run of this command line begins at.
    fn first_part(&self) -> Result<&'static str, String> {
        match self.block {
            Some(block) => shell::block_plan(block)
                .map_err(|e| e.to_string())?
                .first()
                .copied()
                .ok_or_else(|| format!("block {block} runs no parts at all")),
            None => Ok(self.part_name()),
        }
    }

    /// `--dump-frames` and a run's own numbering have to agree.
    ///
    /// An index the run begins *after* is a mistake in the command, not an
    /// empty dump: the run walks away from it on its way to the others, and a
    /// dump that silently lacks the frame says nothing about why. The anchor is
    /// named in the message, because it is the number the caller has to know to
    /// see the mistake.
    fn dump_indices_are_reachable(&self) -> Result<(), String> {
        let Some(wanted) = self.dump_frames.as_ref() else {
            return Ok(());
        };
        let first = self.first_part()?;
        let start = shell::boot_anchor(first).unwrap_or(0);
        match shell::first_unreachable(start, wanted) {
            Some(below) => Err(format!(
                "--dump-frames asks for {below}, but a run of {first} begins at {start} \
                 (its boot anchor); the run can never reach it"
            )),
            None => Ok(()),
        }
    }
}

/// Whether `arg` is `/gravis` as `START.C:89` reads it: `memicmp(argv[1], "/gravis", 7)`,
/// its first seven characters in any case — so `/GRAVIS` and `/gravisultrasound` too.
fn is_gravis(arg: &str) -> bool {
    arg.len() >= 7 && arg.as_bytes()[..7].eq_ignore_ascii_case(b"/gravis")
}

/// The argument after `flag`, which the flag requires.
fn value(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<String, String> {
    args.next().ok_or_else(|| format!("{flag} needs a value"))
}

/// The argument after `flag`, read as a number.
fn number<T: FromStr>(args: &mut impl Iterator<Item = String>, flag: &str) -> Result<T, String> {
    let raw = value(args, flag)?;
    raw.parse()
        .map_err(|_| format!("{flag} expects a number, got {raw:?}"))
}

/// `flag`'s argument, which is one character and no more.
fn exactly_one_char(raw: &str) -> Result<char, String> {
    let mut chars = raw.chars();
    match (chars.next(), chars.next()) {
        (Some(block), None) => Ok(block),
        _ => Err(format!("--block expects one character, got {raw:?}")),
    }
}

/// A comma-separated list of reference indices: `430,470,560,900`.
///
/// An empty item is refused rather than skipped — `430,,470` is a typo, and a
/// skipped item would dump a different set from the one the command names.
///
/// **A repeated index is dropped, keeping the order the list was written in.**
/// `--dump-frames` names a *set* of frames, so this is where it is normalised
/// into one: a command line that says `5,5` twice asks for frame 5 twice, which
/// is frame 5. Not a sort — the run visits indices in its own order whatever
/// the command says, so only the set matters — and first occurrence wins so the
/// list stays as it was written.
fn indices(raw: &str) -> Result<Vec<u32>, String> {
    let items: Vec<&str> = raw.split(',').collect();
    if raw.is_empty() || items.iter().any(|item| item.is_empty()) {
        return Err(format!(
            "--dump-frames expects a list like 430,470,560,900, got {raw:?}"
        ));
    }
    let mut wanted: Vec<u32> = Vec::with_capacity(items.len());
    for item in items {
        let index = item
            .parse()
            .map_err(|_| format!("--dump-frames expects a number, got {item:?}"))?;
        if !wanted.contains(&index) {
            wanted.push(index);
        }
    }
    Ok(wanted)
}

/// The registry's names, for the message an unknown `--part` produces.
fn known_parts() -> String {
    parts::registry()
        .iter()
        .map(|(name, _)| *name)
        .collect::<Vec<_>>()
        .join(", ")
}

/// The blocks `--block` accepts, for the message an unknown one produces.
fn known_blocks() -> String {
    shell::BLOCKS
        .iter()
        .map(|(block, _)| block.to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shell::{Ending, Looping};

    fn parse(args: &[&str]) -> Result<Cli, String> { Cli::parse(args.iter().map(|s| s.to_string())) }

    #[test]
    fn a_run_with_no_arguments_is_the_demo() {
        // `SECOND` with no argument is `SECOND 1` (`part-sequence.md` §4: the
        // default `whattorun` 7fh and `partsmask[1]` 1fh run the same parts), so
        // the port's no-argument run is block 1 — in a window, 1:1 in a 4:3 box
        // with its border, as the default run has always been presented.
        let c = parse(&[]).unwrap();
        assert_eq!(c.block, Some('1'));
        assert_eq!(c.part, None);
        assert!(!c.headless);
        assert_eq!(c.scale, 1);
        assert_eq!(c.aspect, Aspect::FourThree);
        assert!(c.border);
    }

    #[test]
    fn the_test_card_is_still_a_part() {
        // The Phase 0 test card is no longer the default run, but it is still
        // the port's own part and `--part` still names it.
        let c = parse(&["--part", "testcard"]).unwrap();
        assert_eq!(c.part, Some("testcard"));
        assert_eq!(c.block, None, "a named part is not a block run");
        assert_eq!(c.part_name(), "testcard");
    }

    #[test]
    fn a_part_run_holds_and_a_block_run_exits() {
        // A run built from a block ends the way `SECOND.EXE` does
        // (`MAIN/U2.ASM:935-994`: the fade, text mode, the end box); one part
        // holds where it ends, as it always has.
        assert_eq!(parse(&["--part", "glenz"]).unwrap().run_settings().ending, Ending::Hold);
        assert_eq!(parse(&["--block", "3"]).unwrap().run_settings().ending, Ending::Exit);
        assert_eq!(parse(&[]).unwrap().run_settings().ending, Ending::Exit);
        // The vault's runs are one-part runs the menu starts: they hold.
        assert_eq!(parse(&["--vault"]).unwrap().run_settings().ending, Ending::Hold);
    }

    #[test]
    fn the_sound_quality_flag_picks_the_renderer_and_authentic_is_the_default() {
        // The enhanced renderer is the port's own, off by default (D43): a command line that
        // does not ask for it gets the object's mixer, and the run's player is built with it.
        assert_eq!(Cli::default().quality, Quality::Authentic);
        let c = parse(&[]).unwrap();
        assert_eq!(c.quality, Quality::Authentic);
        assert_eq!(c.run_settings().player, PlayerOptions::default());
        let c = parse(&["--sound-quality", "enhanced"]).unwrap();
        assert_eq!(c.quality, Quality::Enhanced);
        assert_eq!(c.run_settings().player.quality, Quality::Enhanced);
        assert_eq!(c.run_settings().player.output_rate(), 48_000);
        let c = parse(&["--sound-quality", "authentic", "--part", "glenz"]).unwrap();
        assert_eq!(c.run_settings().player.quality, Quality::Authentic);
        // A part's run takes it as well as a block's.
        let c = parse(&["--part", "u2e", "--sound-quality", "enhanced"]).unwrap();
        assert_eq!(c.run_settings().player.quality, Quality::Enhanced);
        assert!(parse(&["--sound-quality", "high"]).is_err(), "only the two names");
        // The vault's parts have no music, so the setting is refused there rather than
        // ignored; naming the default is harmless and accepted.
        let error = parse(&["--vault", "--sound-quality", "enhanced"]).unwrap_err();
        assert!(error.contains("silent"), "{error}");
        assert!(parse(&["--vault", "--sound-quality", "authentic"]).is_ok());
        assert!(parse(&["--sound-quality"]).is_err(), "and a name is needed");
    }

    #[test]
    fn enhanced_sound_is_refused_for_a_run_with_no_music() {
        // The test card and the vault's three effects have no player
        // (`shell::music_for_part` is `None`), so a mixer setting would be accepted and do
        // nothing. It is refused, as `--audio-dump` and `--vault` refuse it.
        for part in ["testcard", "grid", "hard", "twist"] {
            assert!(shell::music_for_part(part).is_none(), "{part} is silent");
            let error = parse(&["--part", part, "--sound-quality", "enhanced"]).unwrap_err();
            assert_eq!(
                error,
                format!("--sound-quality enhanced needs a run with music, and a run of {part} has none")
            );
            let error = parse(&["--sound-quality", "enhanced", "--headless", "--dump-dir", "d",
                                "--frames", "1", "--part", part]).unwrap_err();
            assert!(error.starts_with("--sound-quality enhanced needs"), "{part}: {error}");
        }
        // The default is accepted everywhere: every part, every block, and the vault.
        for (name, _) in parts::registry() {
            assert!(parse(&["--part", name, "--sound-quality", "authentic"]).is_ok(), "{name}");
            let music = shell::music_for_part(name).is_some();
            assert_eq!(
                parse(&["--part", name, "--sound-quality", "enhanced"]).is_ok(),
                music,
                "{name}: enhanced is accepted exactly when the part has music"
            );
        }
        for (block, _) in shell::BLOCKS {
            let block = block.to_string();
            for quality in ["authentic", "enhanced"] {
                assert!(
                    parse(&["--block", &block, "--sound-quality", quality]).is_ok(),
                    "--block {block} --sound-quality {quality}"
                );
            }
        }
        assert!(parse(&["--vault", "--sound-quality", "authentic"]).is_ok());
    }

    #[test]
    fn the_loop_flag_is_looping_on() {
        // The setup menu's looping row (`MENU.C:229-238`) is `looping` 1, which
        // `MAIN/U2.ASM:920-933` turns into a run that plays for ever.
        let c = parse(&["--loop"]).unwrap();
        assert_eq!(c.looping, Looping::On);
        assert_eq!(c.block, Some('1'), "with no block it is still the demo");
        assert_eq!(c.run_settings().looping, Looping::On);
        assert_eq!(c.run_settings().ending, Ending::Exit);
        assert!(c.setup, "--loop says nothing about the setup screen");
        let c = parse(&["--block", "5", "--loop"]).unwrap();
        assert_eq!((c.block, c.looping), (Some('5'), Looping::On));
        assert_eq!(parse(&[]).unwrap().looping, Looping::Off);
        assert_eq!(Cli::default().looping, Looping::Off);
        // Looping is the demo's sequence: a part holds where it ends, and the vault
        // starts parts.
        assert!(parse(&["--part", "glenz", "--loop"]).is_err());
        assert!(parse(&["--vault", "--loop"]).is_err());
    }

    #[test]
    fn gravis_is_looping_two() {
        // `START.C:89-93`: `if(!memicmp(argv[1],"/gravis",7)) { m_looping=2;
        // m_soundcard=3; } else menu();` — no setup screen, and ENDSCRL skipped
        // without looping (`U2.ASM:914-925`).
        for spelling in ["/gravis", "/GRAVIS", "/Gravis", "/gravisultrasound"] {
            let c = parse(&[spelling]).unwrap();
            assert_eq!(c.looping, Looping::NoEndScroller, "{spelling}");
            assert!(!c.setup, "{spelling}: the setup screen is skipped");
            assert_eq!(c.block, Some('1'), "{spelling}: `checkcmdline` stops at the slash");
            assert_eq!(c.run_settings().looping, Looping::NoEndScroller);
        }
        assert!(parse(&[]).unwrap().setup, "a plain run asks its settings");
        assert!(Cli::default().setup);
        assert!(parse(&["--part", "glenz", "/gravis"]).is_err());
    }

    #[test]
    fn gravis_wins_over_loop_in_either_order() {
        // `START.C:89-94`: `/gravis` sets `m_looping=2` and skips `menu()`, and the menu
        // is the only place `looping` 1 is chosen (`MENU.C:229-238`), so the setup's
        // looping choice never meets `/gravis`. `--loop` is that choice, and the order the
        // two are written in does not matter.
        for args in [["--loop", "/gravis"], ["/gravis", "--loop"]] {
            let c = parse(&args).unwrap();
            assert_eq!(c.looping, Looping::NoEndScroller, "{args:?}");
            assert!(!c.setup, "{args:?}: /gravis skips the setup screen");
            assert_eq!(c.run_settings().looping, Looping::NoEndScroller, "{args:?}");
        }
        for args in [["--block", "5", "--loop", "/gravis"], ["/gravis", "--block", "5", "--loop"]]
            .iter()
            .map(|a| a.as_slice())
        {
            let c = parse(args).unwrap();
            assert_eq!((c.block, c.looping), (Some('5'), Looping::NoEndScroller), "{args:?}");
        }
    }

    #[test]
    fn unknown_slash_switches_are_ignored() {
        // `MAIN/U2.ASM:605-606`: `checkcmdline` returns at a `/` with `whattorun`
        // left at its 7fh — a switch is never a block and never an error. Only
        // `/gravis` (its first seven characters, any case, `START.C:89`) means
        // anything.
        for switch in ["/x", "/grav", "/gravi", "/?", "/nosound"] {
            let c = parse(&[switch]).unwrap();
            assert_eq!(c.block, Some('1'), "{switch}");
            assert_eq!(c.looping, Looping::Off, "{switch}");
            assert!(c.setup, "{switch}");
        }
        let c = parse(&["--block", "5", "/x"]).unwrap();
        assert_eq!((c.block, c.looping), (Some('5'), Looping::Off));
    }

    #[test]
    fn positions_names_a_file_for_a_headless_run() {
        let cli = parse(&["--headless", "--dump-dir", "d", "--dump-frames", "500",
                          "--positions", "p.csv"]).unwrap();
        assert_eq!(cli.positions, Some(PathBuf::from("p.csv")));
        assert_eq!(parse(&[]).unwrap().positions, None, "--positions is opt-in");
        assert!(parse(&["--headless", "--positions"]).is_err(), "it needs a file");
        assert!(
            parse(&["--headless", "--dump-dir", "d", "--run-seconds", "1", "--positions", "p"])
                .unwrap_err()
                .contains("--frames or --dump-frames"),
            "a wall-clock run ticks in batches"
        );
    }

    #[test]
    fn positions_needs_a_headless_run() {
        // The same rule as the dump flags: a window run that silently wrote nothing would
        // look like a run that recorded its timeline.
        assert_eq!(
            parse(&["--positions", "p.csv"]).unwrap_err(),
            "--positions needs --headless"
        );
        assert_eq!(
            parse(&["--block", "1", "--positions", "p.csv"]).unwrap_err(),
            "--positions needs --headless"
        );
        assert_eq!(
            parse(&["--vault", "--positions", "p.csv"]).unwrap_err(),
            VAULT_REFUSAL,
            "the vault says why it cannot record first"
        );
    }

    #[test]
    fn headless_requires_a_dump_directory() {
        assert!(parse(&["--headless", "--frames", "1"]).is_err());
    }

    #[test]
    fn headless_requires_a_frame_budget_or_a_duration() {
        assert!(parse(&["--headless", "--dump-dir", "/tmp/x"]).is_err());
    }

    #[test]
    fn an_unknown_part_is_rejected() {
        assert!(parse(&["--part", "nope"]).is_err());
    }

    #[test]
    fn an_empty_part_name_is_reported_as_the_empty_name_it_is() {
        // `--part "$A"` with `A` unset. The message has to point at that
        // argument, not at the run's plan: they are different mistakes.
        let message = parse(&["--part", ""]).unwrap_err();
        assert!(message.starts_with("unknown part \"\""), "{message}");
    }

    #[test]
    fn a_part_the_port_has_is_accepted_by_name() {
        // GLENZ was the last name the demo's order had and the port had not;
        // Task 6 gives it its music row, so `--part glenz` parses. (The
        // unknown-name side is `an_unknown_part_is_rejected`.)
        //
        // `shell::resolve`'s `NotPorted` arm is consequently unreachable from
        // any `--part` string — every demo name is registered — but it is
        // retained for the vault (the next demo part a later phase ports):
        // `resolve` still distinguishes a demo name the port lacks from a
        // typo, and the error path is exercised through the shell's stub
        // source, whose `unbuildable("techno")` test pins
        // "techno is not ported yet" (`a_run_that_stopped_says_which_part_it_stopped_at`).
        let cli = parse(&["--part", "glenz"]).unwrap();
        assert_eq!(cli.part, Some("glenz"));
        assert_eq!(cli.part_name(), "glenz");
    }

    #[test]
    fn dump_frames_parses_a_sparse_list_and_refuses_a_bad_one() {
        let cli = Cli::parse(
            ["--headless", "--dump-dir", "d", "--dump-frames", "430,470,560,900"]
                .map(String::from),
        )
        .unwrap();
        assert_eq!(cli.dump_frames, Some(vec![430, 470, 560, 900]));
        assert!(Cli::parse(
            ["--headless", "--dump-dir", "d", "--dump-frames", "430,,470"].map(String::from)
        )
        .is_err());
        assert!(Cli::parse(["--headless", "--dump-dir", "d", "--dump-frames", ""].map(String::from))
            .is_err());
        assert!(Cli::parse(
            ["--headless", "--dump-dir", "d", "--dump-frames", "430,x"].map(String::from)
        )
        .is_err());
    }

    #[test]
    fn a_repeated_dump_index_is_dropped_and_the_order_is_kept() {
        // `--part testcard`: the no-argument run is the demo now, which begins
        // at 404, and index 5 is below it.
        let cli = parse(&["--headless", "--part", "testcard", "--dump-dir", "d",
                          "--dump-frames", "5,5"]).unwrap();
        assert_eq!(cli.dump_frames, Some(vec![5]));
        // First occurrence wins, and the list is not reordered: the run walks
        // its indices in its own order, so the list is a set, not a schedule.
        let cli = parse(&["--headless", "--dump-dir", "d", "--dump-frames", "900,430,900"]).unwrap();
        assert_eq!(cli.dump_frames, Some(vec![900, 430]));
    }

    #[test]
    fn dump_frames_is_a_way_to_bound_a_headless_run_on_its_own() {
        // No --frames and no --run-seconds: the largest requested index is the
        // end of the run. (Of a part numbered from zero: the no-argument run is
        // the demo, which begins at 404.)
        let cli = parse(&["--headless", "--part", "testcard", "--dump-dir", "d",
                          "--dump-frames", "7"]).unwrap();
        assert!(cli.headless);
        assert_eq!(cli.frames, None);
    }

    #[test]
    fn frames_and_dump_frames_are_mutually_exclusive() {
        assert!(parse(&["--headless", "--dump-dir", "d", "--frames", "2", "--dump-frames", "7"])
            .is_err());
        assert!(parse(&[
            "--headless",
            "--dump-dir",
            "d",
            "--run-seconds",
            "1",
            "--dump-frames",
            "7"
        ])
        .is_err());
    }

    #[test]
    fn dump_frames_needs_a_headless_run() {
        assert!(parse(&["--dump-frames", "7"]).is_err());
    }

    #[test]
    fn a_block_and_a_part_are_two_ways_to_say_what_runs() {
        let message = parse(&[
            "--headless",
            "--dump-dir",
            "d",
            "--dump-frames",
            "1",
            "--block",
            "2",
            "--part",
            "testcard",
        ])
        .unwrap_err();
        // The contradiction, not the block's own state: `--part` and `--block`
        // name two different runs, which is a different problem from either
        // one's parts.
        assert!(message.contains("not both"), "{message}");
    }

    #[test]
    fn a_run_that_starts_at_zero_reaches_every_index_asked_of_it() {
        // The refusal rule itself is `shell::first_unreachable`, tested there
        // against an anchor. (`--part alku` carries one now, so a block or part
        // run asking for an index below 404 is refused by `--block 5`'s case
        // above.)
        assert!(parse(&["--headless", "--part", "testcard", "--dump-dir", "d",
                        "--dump-frames", "0"]).is_ok());
        assert!(parse(&["--headless", "--part", "testcard", "--dump-dir", "d",
                        "--dump-frames", "0,9000"])
            .is_ok());
        // The no-argument run is `SECOND 1`, which begins at ALKU's 404: index 0
        // is below it, and the same rule refuses it.
        let message = parse(&["--headless", "--dump-dir", "d", "--dump-frames", "0"]).unwrap_err();
        assert!(message.contains("begins at 404"), "{message}");
    }

    #[test]
    fn a_block_is_one_of_the_originals_entry_points() {
        // Every block the code accepts is *recognised* — whether the port can
        // run it yet is a different answer, and one that changes as the parts
        // land, so this asserts the recognition and nothing about the porting.
        for block in ["1", "2", "3", "4", "5", "U", "u"] {
            let args = ["--headless", "--dump-dir", "d", "--dump-frames", "1", "--block", block];
            match parse(&args) {
                Ok(cli) => assert!(cli.block.is_some(), "block {block} selected nothing"),
                Err(message) => assert!(
                    !message.contains("unknown block"),
                    "block {block} was called unknown: {message}"
                ),
            }
        }
        for rejected in ["0", "6", "7", "x", "12", ""] {
            assert!(
                parse(&["--headless", "--dump-dir", "d", "--dump-frames", "1", "--block", rejected])
                    .is_err(),
                "block {rejected:?} was accepted"
            );
        }
        let unknown = parse(&["--headless", "--dump-dir", "d", "--dump-frames", "1", "--block", "7"])
            .unwrap_err();
        assert!(unknown.contains("unknown block"), "{unknown}");
    }

    #[test]
    fn a_block_is_a_run_the_window_drives_as_well_as_a_dump() {
        // The window drives the same `Sequence` a headless run dumps, so a
        // block is no longer refused outside `--headless`. Every block the
        // demo accepts now starts at a part the port has: `SECOND 4` begins at
        // U2E, which landed with the vector engine's second part.
        let cli = parse(&["--block", "5"]).unwrap();
        assert_eq!(cli.block, Some('5'));
        assert!(!cli.headless, "the window is the default mode");
        assert!(parse(&["--block", "2"]).is_ok());
        assert!(parse(&["--block", "3"]).is_ok(), "MNTSCRL is ported");
        assert!(parse(&["--block", "1"]).is_ok(), "ALKU is ported");

        let cli = parse(&["--block", "4"]).unwrap();
        assert_eq!(cli.block, Some('4'), "SECOND 4 begins at U2E, which is ported");
        assert!(!cli.headless);
    }

    #[test]
    fn the_square_pixel_aspect_is_accepted() {
        assert_eq!(parse(&["--aspect", "square"]).unwrap().aspect, Aspect::SquarePixels);
    }

    const VAULT_REFUSAL: &str = "the vault menu needs a window; name a part with --part";

    #[test]
    fn the_vault_flag_is_parsed() {
        let cli = parse(&["--vault"]).unwrap();
        assert!(cli.vault);
        assert!(!cli.headless, "the vault is a window surface");
        assert_eq!(cli.part, None, "the menu chooses the part");
        assert_eq!(cli.block, None);
        assert!(!parse(&[]).unwrap().vault, "--vault is opt-in");
    }

    #[test]
    fn a_vault_run_refuses_headless() {
        assert_eq!(
            parse(&["--vault", "--headless"]).unwrap_err(),
            VAULT_REFUSAL
        );
        // Even with everything else a headless run would need: the reason is
        // the mode, not the missing dump.
        assert_eq!(
            parse(&["--vault", "--headless", "--dump-dir", "d", "--frames", "1"]).unwrap_err(),
            VAULT_REFUSAL
        );
    }

    #[test]
    fn a_vault_run_refuses_a_part_or_block() {
        let cases: [&[&str]; 3] = [
            &["--vault", "--part", "grid"],
            &["--vault", "--block", "1"],
            &["--vault", "--part", "grid", "--block", "1"],
        ];
        for args in cases {
            assert_eq!(parse(args).unwrap_err(), VAULT_REFUSAL, "{args:?}");
        }
    }

    #[test]
    fn a_vault_run_refuses_a_dump_or_audio_flag() {
        let flags: [&[&str]; 5] = [
            &["--dump-dir", "d"],
            &["--dump-frames", "1"],
            &["--frames", "1"],
            &["--run-seconds", "1"],
            &["--audio-dump", "music.wav"],
        ];
        for flag in flags {
            let mut args = vec!["--vault"];
            args.extend_from_slice(flag);
            assert_eq!(parse(&args).unwrap_err(), VAULT_REFUSAL, "{args:?}");
        }
    }

    #[test]
    fn a_vault_run_accepts_the_presentation_flags() {
        let cli = parse(&[
            "--vault",
            "--scale",
            "2",
            "--aspect",
            "square",
            "--no-border",
            "--no-audio",
        ])
        .unwrap();
        assert!(cli.vault);
        assert_eq!(cli.scale, 2);
        assert_eq!(cli.aspect, Aspect::SquarePixels);
        assert!(!cli.border);
        assert!(cli.no_audio);
    }

    #[test]
    fn the_screen_shows_only_for_a_windowed_demo_run() {
        // The demo's own run, in a window: no argument, a named block, a looping run.
        for args in [&[][..], &["--block", "3"], &["--block", "U"], &["--loop"],
                     &["--no-audio", "--aspect", "square"], &["--audio-dump", "a.wav"]] {
            assert!(parse(args).unwrap().shows_setup(), "{args:?} opens the settings screen");
        }
        // Everything else starts as it always did.
        assert!(!parse(&["--part", "glenz"]).unwrap().shows_setup(), "--part");
        assert!(!parse(&["--vault"]).unwrap().shows_setup(), "--vault");
        assert!(!parse(&["--no-setup"]).unwrap().shows_setup(), "--no-setup");
        assert!(!parse(&["--no-setup", "--block", "2"]).unwrap().shows_setup());
        assert!(!parse(&["/gravis"]).unwrap().shows_setup(), "/gravis skips it (START.C:89)");
        assert!(!parse(&["/GRAVIS", "--block", "2"]).unwrap().shows_setup());
        assert!(
            !parse(&["--headless", "--frames", "1", "--dump-dir", "d"]).unwrap().shows_setup(),
            "a headless run opens no window"
        );
        // `--no-setup` is a flag and changes nothing else.
        let plain = parse(&[]).unwrap();
        let skipped = parse(&["--no-setup"]).unwrap();
        assert!(plain.setup && !skipped.setup);
        assert_eq!(skipped.block, plain.block);
        assert_eq!(skipped.looping, plain.looping);
    }
}
