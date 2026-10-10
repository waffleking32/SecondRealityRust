//! The command line: everything the binary can be asked to do.
//!
//! Parsing is pure — no SDL, no filesystem, no clock — and it is where the
//! headless/window split is decided, before anything is initialised. Headless
//! mode therefore reaches a machine with no display at all; see `main`.

use std::path::PathBuf;
use std::str::FromStr;

use sr_engine::present::Aspect;

use crate::parts;

/// What to run, and how to show it.
#[derive(Debug, Clone)]
pub struct Cli {
    /// The part to run, one of [`parts::registry`]'s names.
    pub part: String,
    /// Dump frames to disk instead of opening a window.
    pub headless: bool,
    /// Where the dump goes. Required with `--headless`.
    pub dump_dir: Option<PathBuf>,
    /// Stop after this many logic ticks, dumping every one. Required with
    /// `--headless` unless `run_seconds` is given.
    pub frames: Option<u32>,
    /// Drive from real elapsed time for this long, dumping only the last frame.
    /// Required with `--headless` unless `frames` is given.
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
            part: "testcard".to_string(),
            headless: false,
            dump_dir: None,
            frames: None,
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
                    if !parts::is_known(&name) {
                        return Err(format!(
                            "unknown part {name:?}; known parts: {}",
                            known_parts()
                        ));
                    }
                    cli.part = name;
                }
                "--frames" => cli.frames = Some(number(&mut args, "--frames")?),
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

    /// The rules that span more than one argument.
    fn validate(&self) -> Result<(), String> {
        if self.scale < 1 {
            return Err("--scale is a magnification, so it must be at least 1".to_string());
        }
        if !self.headless {
            // Silently ignoring a dump would be a trap: the run would look
            // right and leave nothing behind to compare.
            if self.frames.is_some() || self.run_seconds.is_some() || self.dump_dir.is_some() {
                return Err(
                    "--frames, --run-seconds and --dump-dir need --headless".to_string()
                );
            }
            return Ok(());
        }
        if self.dump_dir.is_none() {
            return Err("--headless needs --dump-dir".to_string());
        }
        match (self.frames, self.run_seconds) {
            (Some(_), Some(_)) => Err("give --frames or --run-seconds, not both".to_string()),
            (Some(0), None) => Err("--frames must be at least 1".to_string()),
            (Some(_), None) => Ok(()),
            (None, Some(seconds)) if !(seconds.is_finite() && seconds > 0.0) => {
                Err("--run-seconds must be a positive number of seconds".to_string())
            }
            (None, Some(_)) => Ok(()),
            (None, None) => Err("--headless needs --frames or --run-seconds".to_string()),
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

/// The registry's names, for the message an unknown `--part` produces.
fn known_parts() -> String {
    parts::registry()
        .iter()
        .map(|(name, _)| *name)
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
        assert_eq!(c.part, "testcard");
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
    fn the_square_pixel_aspect_is_accepted() {
        assert_eq!(parse(&["--aspect", "square"]).unwrap().aspect, Aspect::SquarePixels);
    }
}
