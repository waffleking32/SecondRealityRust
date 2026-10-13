//! `secondreality` — the port as something you can run.
//!
//! Two ways to run it, chosen before anything is initialised: a window, which
//! needs SDL2 and a display, or `--headless`, which needs neither and must not
//! touch video at all. The engine keeps zero runtime dependencies; the window
//! is a client of it like any other.

mod cli;
mod headless;
mod parts;
mod shell;
mod window;

use std::error::Error;
use std::process::ExitCode;

use sr_engine::Runner;

use crate::shell::{Sequence, SURFACE_HEIGHT, SURFACE_WIDTH};

/// Printed when the command line does not parse.
const USAGE: &str = "\
usage: secondreality [--part <name> | --block <1|2|3|4|5|U>] [--frames <N>]
                     [--dump-frames <A,B,C>] [--dump-dir <DIR>] [--scale <N>]
                     [--aspect 4:3|square] [--no-border] [--headless]
                     [--run-seconds <S>]";

fn main() -> ExitCode {
    let cli = match cli::Cli::parse(std::env::args().skip(1)) {
        Ok(cli) => cli,
        Err(message) => {
            eprintln!("secondreality: {message}\n{USAGE}");
            return ExitCode::from(2);
        }
    };
    // Dispatched on the parsed command line: a headless run must reach a
    // machine with no display, so it never initialises SDL video.
    let result = if cli.headless {
        headless::run(&cli)
    } else {
        window::run(&cli)
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("secondreality: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The runner the window drives: the demo's surface, the picture's starting
/// geometry, and the one part the command line named.
///
/// The view leaves its line stride unset, so the beam reads the surface's own
/// rows whatever the picture's width is: a part that draws through `Surface`
/// and never touches the stride still gets its picture back un-sheared.
fn new_runner(part_name: &str) -> Result<Runner, Box<dyn Error>> {
    let part =
        parts::new_part(part_name).ok_or_else(|| format!("unknown part {part_name:?}"))?;
    Runner::new(SURFACE_WIDTH, SURFACE_HEIGHT, shell::start_view(), part).map_err(Into::into)
}

/// The sequence the headless mode drives: the ported parts of a block, in the
/// demo's own order, or the one part `--part` named — numbered from the
/// reference index the run's first part begins at.
fn new_sequence(cli: &cli::Cli) -> Result<Sequence, Box<dyn Error>> {
    let plan = match cli.block {
        Some(block) => shell::block_plan(block)?,
        None => vec![shell::resolve(cli.part_name())?],
    };
    Ok(Sequence::new(
        SURFACE_WIDTH,
        SURFACE_HEIGHT,
        shell::start_view(),
        plan,
    )?)
}
