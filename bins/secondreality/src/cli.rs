//! The command line: everything the binary can be asked to do. Parsing is pure (no SDL,
//! no filesystem, no clock) and decides the headless/window split before anything is
//! initialised. The usage text is `main`'s.

use std::path::PathBuf;
use std::str::FromStr;

use sr_audio::{PlayerOptions, Quality};
use sr_engine::present::{Aspect, PresentOptions};

use crate::parts;
use crate::shell;

/// The part [`Cli::part_name`] falls back to for a hand-built `Cli` naming neither a part
/// nor a block ([`Cli::parse`] makes that [`DEFAULT_BLOCK`]).
const DEFAULT_PART: &str = "testcard";

/// No part and no block is `SECOND 1`, as in the original (`MAIN/U2.ASM:108`'s default
/// `whattorun` 7fh and `partsmask[1]`'s 1fh gate the same parts; `part-sequence.md` §4).
const DEFAULT_BLOCK: char = '1';

/// What to run, and how to show it.
#[derive(Debug, Clone)]
pub struct Cli {
    /// `--part`: one of [`parts::registry`]'s names.
    pub part: Option<&'static str>,
    /// `--block`: a `SECOND n` entry point.
    pub block: Option<char>,
    pub headless: bool,
    pub dump_dir: Option<PathBuf>,
    /// `--frames`: stop after this many ticks, dumping every one.
    pub frames: Option<u32>,
    /// `--dump-frames`: dump exactly these indices, and stop after the largest.
    pub dump_frames: Option<Vec<u32>>,
    /// `--run-seconds`: drive by wall-clock time, dumping only the last frame.
    pub run_seconds: Option<f64>,
    /// The window's integer magnification (D48); a dump is always 1:1.
    pub scale: u32,
    pub aspect: Aspect,
    /// Whether to draw the frame's overscan colour as a border.
    pub border: bool,
    /// `--audio-dump`: the WAV that replaces the sound card (`docs/fidelity/music.md` §10.5).
    pub audio_dump: Option<PathBuf>,
    /// `--no-audio`: no sound card; the player still runs.
    pub no_audio: bool,
    /// `--vault`: the window-only menu over `parts::vault::VAULT` (D40).
    pub vault: bool,
    /// The original's `looping` (`MAIN/U2.ASM:746`): `--loop` is the setup's "Demo
    /// looping: Enabled" (`MENU.C:236`), and `/gravis` is 2 (`START.C:89-93`).
    pub looping: shell::Looping,
    /// Whether a windowed demo run opens on the setup screen (`START.C:89-95`, D44).
    pub setup: bool,
    /// `--positions`: the headless run's timeline (`headless::Positions`).
    pub positions: Option<PathBuf>,
    /// `--sound-quality`: the object's own mixer, or the enhanced renderer (D43).
    pub quality: Quality,
}

impl Default for Cli {
    /// Nothing named, in a window, at [`PresentOptions::WINDOW`] (D48).
    fn default() -> Cli {
        Cli {
            part: None,
            block: None,
            headless: false,
            dump_dir: None,
            frames: None,
            dump_frames: None,
            run_seconds: None,
            scale: PresentOptions::WINDOW.scale,
            aspect: PresentOptions::WINDOW.aspect,
            border: PresentOptions::WINDOW.border,
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
    /// Parse the arguments after the program name, with every rule that spans more than one
    /// argument checked, so the rest of the binary may rely on the result.
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
                    cli.part = Some(
                        shell::resolve(&name)
                            .map_err(|e| format!("{e}; ported parts: {}", known_parts()))?,
                    );
                }
                "--block" => {
                    let raw = value(&mut args, "--block")?;
                    let block = block_char(&raw)?.to_ascii_uppercase();
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
                "--no-setup" => cli.setup = false,
                switch if is_gravis(switch) => gravis = true,
                // `MAIN/U2.ASM:605-606`: `checkcmdline` returns at any other `/` switch.
                switch if switch.starts_with('/') => {}
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        // `START.C:89-94`: `/gravis` sets `m_looping=2` and skips `menu()`, the only place
        // `looping` 1 is chosen (`MENU.C:229-238`), so it wins over `--loop` (D42).
        if gravis {
            cli.looping = shell::Looping::NoEndScroller;
            cli.setup = false;
        }
        // The vault names neither and starts its own runs.
        if cli.part.is_none() && cli.block.is_none() && !cli.vault {
            cli.block = Some(DEFAULT_BLOCK);
        }
        cli.validate()?;
        Ok(cli)
    }

    /// Whether the run opens on the settings screen ([`crate::menu::settings`]): a windowed
    /// block run that has not skipped it (`--no-setup`, `/gravis`).
    pub fn shows_setup(&self) -> bool {
        self.setup && !self.headless && !self.vault && self.block.is_some()
    }

    /// How the window presents: `--scale`, `--aspect` and `--no-border`.
    pub fn present_options(&self) -> PresentOptions {
        PresentOptions { aspect: self.aspect, scale: self.scale, border: self.border }
    }

    /// The part a one-part run names (meaningless for a block run).
    pub fn part_name(&self) -> &'static str {
        self.part.unwrap_or(DEFAULT_PART)
    }

    /// The run's settings beyond its plan: a block run ends as `SECOND.EXE` does
    /// ([`Ending::Exit`](shell::Ending::Exit)); one part, the vault's included, holds.
    pub fn run_settings(&self) -> shell::RunSettings {
        shell::RunSettings {
            ending: if self.block.is_some() {
                shell::Ending::Exit
            } else {
                shell::Ending::Hold
            },
            looping: self.looping,
            player: PlayerOptions {
                quality: self.quality,
                ..PlayerOptions::default()
            },
        }
    }

    /// The rules that span more than one argument. The order decides which refusal a
    /// command line that breaks several rules gets.
    fn validate(&self) -> Result<(), String> {
        if self.scale < 1 {
            return Err("--scale is a magnification, so it must be at least 1".to_string());
        }
        // Before the headless rules, so a headless vault says why it cannot exist.
        if self.vault {
            if self.looping != shell::Looping::Off {
                return Err("the vault's parts do not loop; --loop and /gravis are a block's run"
                    .to_string());
            }
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
        // Looping is the demo's sequence going round (`MAIN/U2.ASM:914-933`).
        if self.looping != shell::Looping::Off && self.block.is_none() {
            return Err("--loop and /gravis are a block's run; give --block, not --part".to_string());
        }
        // A mixer setting or a WAV for a run with no player would do nothing. The default
        // quality names the authentic mixer and is harmless.
        let first = self.first_part()?;
        for (given, flag) in [
            (self.quality != Quality::Authentic, "--sound-quality enhanced"),
            (self.audio_dump.is_some(), "--audio-dump"),
        ] {
            if given && shell::music_for_part(first).is_none() {
                return Err(format!("{flag} needs a run with music, and a run of {first} has none"));
            }
        }
        // In either mode: the window drives the same `Sequence` a dump does.
        if let Some(block) = self.block {
            shell::resolve(first).map_err(|e| format!("--block {block}: {e}"))?;
        }
        if !self.headless {
            // A dump flag silently ignored would leave nothing behind to compare.
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
        self.dump_indices_are_reachable(first)
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

    /// An index before the run's start (its boot anchor) is a mistake in the command, not
    /// an empty dump; the message names the anchor.
    fn dump_indices_are_reachable(&self, first: &str) -> Result<(), String> {
        let Some(wanted) = self.dump_frames.as_ref() else {
            return Ok(());
        };
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
/// its first seven characters in any case, so `/GRAVIS` and `/gravisultrasound` too.
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

/// `--block`'s argument, which is one character and no more.
fn block_char(raw: &str) -> Result<char, String> {
    let mut chars = raw.chars();
    match (chars.next(), chars.next()) {
        (Some(block), None) => Ok(block),
        _ => Err(format!("--block expects one character, got {raw:?}")),
    }
}

/// A comma-separated list of indices: `430,470,560,900`. An empty item is refused; a
/// repeated index is dropped, keeping the first, and the order is kept.
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

    fn parse(args: &[&str]) -> Result<Cli, String> {
        Cli::parse(args.iter().map(|s| s.to_string()))
    }

    const VAULT_REFUSAL: &str = "the vault menu needs a window; name a part with --part";
    const HEADLESS: [&str; 4] = ["--headless", "--dump-dir", "d", "--dump-frames"];

    /// A headless run of `rest`, bounded by `--dump-frames frames`.
    fn headless(frames: &str, rest: &[&str]) -> Result<Cli, String> {
        let mut args = HEADLESS.to_vec();
        args.push(frames);
        args.extend_from_slice(rest);
        parse(&args)
    }

    #[test]
    fn a_run_with_no_arguments_is_the_demo_in_a_window() {
        let c = parse(&[]).unwrap();
        assert_eq!((c.block, c.part), (Some('1'), None));
        assert!(!c.headless && !c.vault && c.setup && c.border);
        assert_eq!((c.scale, c.aspect), (2, Aspect::FourThree));
        assert_eq!((c.looping, c.quality, &c.positions), (Looping::Off, Quality::Authentic, &None));
        assert_eq!(c.run_settings().ending, Ending::Exit);
        assert_eq!(c.run_settings().player, PlayerOptions::default());
    }

    #[test]
    fn the_flags_set_their_fields() {
        let c = parse(&["--aspect", "4:3", "--scale", "1", "--no-border", "--no-audio"]).unwrap();
        assert_eq!((c.aspect, c.scale, c.border, c.no_audio), (Aspect::FourThree, 1, false, true));
        assert_eq!(parse(&["--aspect", "square"]).unwrap().aspect, Aspect::SquarePixels);

        let c = parse(&["--part", "testcard"]).unwrap();
        assert_eq!((c.part, c.block, c.part_name()), (Some("testcard"), None, "testcard"));
        assert_eq!(c.run_settings().ending, Ending::Hold, "a part run holds");
        assert_eq!(parse(&["--block", "u"]).unwrap().block, Some('U'));

        let c = parse(&["--part", "u2e", "--sound-quality", "enhanced"]).unwrap();
        assert_eq!(c.run_settings().player.quality, Quality::Enhanced);
        assert_eq!(c.run_settings().player.output_rate(), 48_000);

        let c = parse(&["--vault", "--scale", "3", "--aspect", "4:3", "--no-border", "--no-audio",
                        "--sound-quality", "authentic"]).unwrap();
        assert!(c.vault && !c.headless && !c.border && c.no_audio);
        assert_eq!((c.part, c.block, c.scale), (None, None, 3));
        assert_eq!(c.run_settings().ending, Ending::Hold, "the vault's runs are parts");

        let c = headless("500", &["--positions", "p.csv"]).unwrap();
        assert_eq!(c.positions, Some(PathBuf::from("p.csv")));
        let c = headless("7", &["--part", "testcard"]).unwrap();
        assert_eq!((c.dump_frames, c.frames), (Some(vec![7]), None));
    }

    #[test]
    fn dump_frames_is_a_set_in_the_order_written() {
        let list = |raw: &str| headless(raw, &["--part", "testcard"]).map(|c| c.dump_frames);
        assert_eq!(list("430,470,560,900"), Ok(Some(vec![430, 470, 560, 900])));
        assert_eq!(list("5,5"), Ok(Some(vec![5])));
        assert_eq!(list("900,430,900"), Ok(Some(vec![900, 430])));
        for bad in ["430,,470", "", "430,x"] {
            assert!(list(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn loop_and_gravis_are_the_originals_looping() {
        // `--loop` is `looping` 1 (`MENU.C:229-238`); `/gravis` is 2 and no setup
        // (`START.C:89-93`), matched on its first seven characters in any case, winning over
        // `--loop` in either order. Other `/` switches are ignored (`MAIN/U2.ASM:605-606`).
        let cases: [(&[&str], char, Looping, bool); 10] = [
            (&["--loop"], '1', Looping::On, true),
            (&["--block", "5", "--loop"], '5', Looping::On, true),
            (&["/gravis"], '1', Looping::NoEndScroller, false),
            (&["/GRAVIS"], '1', Looping::NoEndScroller, false),
            (&["/gravisultrasound"], '1', Looping::NoEndScroller, false),
            (&["--loop", "/gravis"], '1', Looping::NoEndScroller, false),
            (&["/Gravis", "--block", "5", "--loop"], '5', Looping::NoEndScroller, false),
            (&["/gravi"], '1', Looping::Off, true),
            (&["/nosound"], '1', Looping::Off, true),
            (&["--block", "5", "/x"], '5', Looping::Off, true),
        ];
        for (args, block, looping, setup) in cases {
            let c = parse(args).unwrap();
            assert_eq!((c.block, c.looping, c.setup), (Some(block), looping, setup), "{args:?}");
            assert_eq!(c.run_settings().looping, looping, "{args:?}");
        }
    }

    #[test]
    fn the_settings_screen_shows_only_for_a_windowed_demo_run() {
        for args in [&[][..], &["--block", "3"], &["--block", "U"], &["--loop"],
                     &["--no-audio", "--aspect", "square"], &["--audio-dump", "a.wav"]] {
            assert!(parse(args).unwrap().shows_setup(), "{args:?}");
        }
        for args in [&["--part", "glenz"][..], &["--vault"], &["--no-setup"],
                     &["--no-setup", "--block", "2"], &["/gravis"], &["/GRAVIS", "--block", "2"],
                     &["--headless", "--frames", "1", "--dump-dir", "d"]] {
            assert!(!parse(args).unwrap().shows_setup(), "{args:?}");
        }
    }

    #[test]
    fn every_block_is_recognised_and_runnable_in_either_mode() {
        for (block, _) in shell::BLOCKS {
            let block = block.to_string();
            assert!(parse(&["--block", &block]).is_ok(), "--block {block}");
            for quality in ["authentic", "enhanced"] {
                assert!(parse(&["--block", &block, "--sound-quality", quality]).is_ok());
            }
        }
        for rejected in ["0", "6", "7", "x", "12", ""] {
            assert!(parse(&["--block", rejected]).is_err(), "{rejected:?}");
        }
        assert!(parse(&["--block", "7"]).unwrap_err().contains("unknown block"));
    }

    #[test]
    fn enhanced_sound_is_accepted_exactly_for_a_run_with_music() {
        for (name, _) in parts::registry() {
            assert!(parse(&["--part", name, "--sound-quality", "authentic"]).is_ok(), "{name}");
            let enhanced = parse(&["--part", name, "--sound-quality", "enhanced"]);
            match shell::music_for_part(name) {
                Some(_) => assert!(enhanced.is_ok(), "{name}"),
                None => assert_eq!(
                    enhanced.unwrap_err(),
                    format!("--sound-quality enhanced needs a run with music, and a run of {name} has none")
                ),
            }
        }
    }

    #[test]
    fn a_dump_index_before_the_runs_start_is_refused() {
        assert!(headless("0", &["--part", "testcard"]).is_ok());
        assert!(headless("0,9000", &["--part", "testcard"]).is_ok());
        let message = headless("0", &[]).unwrap_err();
        assert!(message.contains("begins at 404"), "{message}");
    }

    #[test]
    fn contradictory_command_lines_are_refused() {
        // (args, the refusal, or a fragment of it).
        let refused: &[(&[&str], &str)] = &[
            (&["--scale", "0"], "--scale is a magnification"),
            (&["--sound-quality", "high"], "unknown sound quality"),
            (&["--sound-quality"], "--sound-quality needs a value"),
            (&["--aspect", "wide"], "unknown aspect"),
            (&["--part", "nope"], "unknown part"),
            (&["--part", ""], "unknown part \"\""),
            (&["--block", "55"], "--block expects one character"),
            (&["--frames", "x"], "--frames expects a number"),
            (&["--what"], "unknown argument"),
            (&["--part", "glenz", "--loop"], "--loop and /gravis are a block's run"),
            (&["--part", "glenz", "/gravis"], "--loop and /gravis are a block's run"),
            (&["--part", "testcard", "--audio-dump", "a.wav"], "--audio-dump needs a run with music"),
            (&["--headless", "--dump-dir", "d", "--frames", "1", "--sound-quality", "enhanced",
               "--part", "grid"], "--sound-quality enhanced needs"),
            (&["--headless", "--dump-dir", "d", "--dump-frames", "1", "--block", "2", "--part",
               "testcard"], "give --part or --block, not both"),
            (&["--dump-frames", "7"], "need --headless"),
            (&["--frames", "7"], "need --headless"),
            (&["--positions", "p.csv"], "--positions needs --headless"),
            (&["--block", "1", "--positions", "p.csv"], "--positions needs --headless"),
            (&["--headless", "--frames", "1"], "--headless needs --dump-dir"),
            (&["--headless", "--dump-dir", "d"], "--headless needs --frames"),
            (&["--headless", "--dump-dir", "d", "--frames", "2", "--dump-frames", "7"],
             "give one of"),
            (&["--headless", "--dump-dir", "d", "--run-seconds", "1", "--dump-frames", "7"],
             "give one of"),
            (&["--headless", "--dump-dir", "d", "--frames", "0"], "--frames must be at least 1"),
            (&["--headless", "--dump-dir", "d", "--run-seconds", "-1"], "--run-seconds must be"),
            (&["--headless", "--dump-dir", "d", "--run-seconds", "1", "--positions", "p"],
             "bound the run with --frames or --dump-frames"),
            (&["--vault", "--loop"], "the vault's parts do not loop"),
            (&["--vault", "--sound-quality", "enhanced"], "the vault's parts are silent"),
            (&["--vault", "--headless"], VAULT_REFUSAL),
            (&["--vault", "--headless", "--dump-dir", "d", "--frames", "1"], VAULT_REFUSAL),
            (&["--vault", "--part", "grid"], VAULT_REFUSAL),
            (&["--vault", "--block", "1"], VAULT_REFUSAL),
            (&["--vault", "--dump-dir", "d"], VAULT_REFUSAL),
            (&["--vault", "--dump-frames", "1"], VAULT_REFUSAL),
            (&["--vault", "--frames", "1"], VAULT_REFUSAL),
            (&["--vault", "--run-seconds", "1"], VAULT_REFUSAL),
            (&["--vault", "--audio-dump", "music.wav"], VAULT_REFUSAL),
            (&["--vault", "--positions", "p.csv"], VAULT_REFUSAL),
        ];
        for (args, expected) in refused {
            let message = parse(args).expect_err(&format!("{args:?} was accepted"));
            assert!(message.contains(expected), "{args:?}: {message}");
        }
        // The exact messages other tests and users see.
        assert_eq!(parse(&["--vault", "--part", "grid"]).unwrap_err(), VAULT_REFUSAL);
        assert_eq!(parse(&["--positions", "p.csv"]).unwrap_err(), "--positions needs --headless");
        assert_eq!(
            parse(&["--part", "testcard", "--audio-dump", "a.wav"]).unwrap_err(),
            "--audio-dump needs a run with music, and a run of testcard has none"
        );
    }
}
