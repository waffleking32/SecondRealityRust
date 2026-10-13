//! `secondreality` — the port as something you can run.
//!
//! Two ways to run it, chosen before anything is initialised: a window, which
//! needs SDL2 and a display, or `--headless`, which needs neither and must not
//! touch video at all. Both drive the same `Sequence` — a block's ported parts
//! in the demo's own order, or one part — so what a window plays and what a
//! dump holds are the same run. The engine keeps zero runtime dependencies;
//! the window is a client of it like any other.

mod cli;
mod headless;
mod parts;
mod shell;
mod window;

use std::error::Error;
use std::process::ExitCode;

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

/// The run both modes drive: the ported parts of a block, in the demo's own
/// order, or the one part `--part` named — numbered from the reference index
/// the run's first part begins at, and drawn on the demo's own surface.
///
/// The view leaves its line stride unset, so the beam reads the surface's own
/// rows whatever the picture's width is: a part that draws through `Surface`
/// and never touches the stride still gets its picture back un-sheared.
///
/// Nothing here touches SDL: the window presents this run's frames and
/// `--headless` dumps them, which is what keeps a machine with no display able
/// to run the demo and answer for it.
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
