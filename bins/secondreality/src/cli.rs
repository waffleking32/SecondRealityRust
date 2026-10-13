//! The command line: everything the binary can be asked to do.
//!
//! Parsing is pure — no SDL, no filesystem, no clock — and it is where the
//! headless/window split is decided, before anything is initialised. Headless
//! mode therefore reaches a machine with no display at all; see `main`.

use std::path::PathBuf;
use std::str::FromStr;

use sr_engine::present::Aspect;

use crate::parts;
use crate::shell;

/// The part a run names when the command line names none. The demo's own
/// default is `SECOND 1`, which needs parts the port does not have; a run with
/// no argument is the test card, as it has been since Phase 0.
const DEFAULT_PART: &str = "testcard";

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
}

impl Default for Cli {
    /// The test card in a window, 1:1 in a 4:3 box with its border.
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
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--part" => {
                    let name = value(&mut args, "--part")?;
                    // The shell decides which names exist, because it is the
                    // only place that knows which of the demo's parts are the
                    // port's: `--part glenz` is a task someone can pick up,
                    // `--part glenzz` is a typo, and the two say so.
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
                other => return Err(format!("unknown argument {other:?}")),
            }
        }
        cli.validate()?;
        Ok(cli)
    }

    /// The part a one-part run names, whether `--part` gave it or not.
    ///
    /// Only meaningful for a run that is not a block's: a block names a set of
    /// parts and [`block`](Cli::block) is how a caller tells.
    pub fn part_name(&self) -> &'static str {
        self.part.unwrap_or(DEFAULT_PART)
    }

    /// The rules that span more than one argument.
    fn validate(&self) -> Result<(), String> {
        if self.scale < 1 {
            return Err("--scale is a magnification, so it must be at least 1".to_string());
        }
        if self.block.is_some() && self.part.is_some() {
            return Err("give --part or --block, not both".to_string());
        }
        if self.block.is_some() && !self.headless {
            // A block is a run of several parts, and the window still drives
            // one; the phase's window task is what changes that.
            return Err("--block needs --headless".to_string());
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
        // The block's first part has to be one the port can run. Checked here
        // rather than where `--block` is read, so that a command line that
        // contradicts itself says *that* first.
        if let Some(block) = self.block {
            let first = self.first_part()?;
            shell::resolve(first).map_err(|e| format!("--block {block}: {e}"))?;
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
fn indices(raw: &str) -> Result<Vec<u32>, String> {
    let items: Vec<&str> = raw.split(',').collect();
    if raw.is_empty() || items.iter().any(|item| item.is_empty()) {
        return Err(format!(
            "--dump-frames expects a list like 430,470,560,900, got {raw:?}"
        ));
    }
    items
        .iter()
        .map(|item| {
            item.parse()
                .map_err(|_| format!("--dump-frames expects a number, got {item:?}"))
        })
        .collect()
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

    fn parse(args: &[&str]) -> Result<Cli, String> { Cli::parse(args.iter().map(|s| s.to_string())) }

    #[test]
    fn defaults_to_the_test_card_in_a_window() {
        let c = parse(&[]).unwrap();
        assert_eq!(c.part_name(), "testcard");
        assert_eq!(c.block, None);
        assert!(!c.headless);
        assert_eq!(c.scale, 1);
        assert_eq!(c.aspect, Aspect::FourThree);
        assert!(c.border);
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
    fn a_part_the_port_has_not_reached_is_rejected_by_name() {
        // Not "unknown part": the demo has GLENZ, the port does not yet.
        let message = parse(&["--part", "glenz"]).unwrap_err();
        assert!(message.contains("glenz is not ported yet"), "{message}");
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
    fn dump_frames_is_a_way_to_bound_a_headless_run_on_its_own() {
        // No --frames and no --run-seconds: the largest requested index is the
        // end of the run.
        let cli = parse(&["--headless", "--dump-dir", "d", "--dump-frames", "7"]).unwrap();
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
        // The contradiction, not the block's own state: the block's first part
        // is not ported yet either, and the two are different problems.
        assert!(message.contains("not both"), "{message}");
    }

    #[test]
    fn a_run_that_starts_at_zero_reaches_every_index_asked_of_it() {
        // The refusal rule itself is `shell::first_unreachable`, tested there
        // against an anchor: no part that carries one can be run yet, so there
        // is no command line here that reaches it.
        assert!(parse(&["--headless", "--dump-dir", "d", "--dump-frames", "0"]).is_ok());
        assert!(parse(&["--headless", "--part", "testcard", "--dump-dir", "d",
                        "--dump-frames", "0,9000"])
            .is_ok());
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
    fn the_square_pixel_aspect_is_accepted() {
        assert_eq!(parse(&["--aspect", "square"]).unwrap().aspect, Aspect::SquarePixels);
    }
}
