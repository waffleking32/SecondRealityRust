//! `secondreality` — the port as something you can run.
//!
//! Two ways to run it, chosen before anything is initialised: a window, which
//! needs SDL2 and a display, or `--headless`, which needs neither and must not
//! touch video at all. The engine keeps zero runtime dependencies; the window
//! is a client of it like any other.

mod cli;
mod headless;
mod parts;
mod window;

use std::error::Error;
use std::process::ExitCode;

use sr_engine::view::View;
use sr_engine::{Runner, MAX_HEIGHT, MAX_WIDTH};

/// The surface every part draws into: the engine's full geometry. The test
/// card's line stride is `MAX_WIDTH`, so the surface has to be that wide.
const SURFACE_WIDTH: usize = MAX_WIDTH;
const SURFACE_HEIGHT: usize = MAX_HEIGHT;

/// The picture's starting geometry. A part may change it — double-scan makes
/// it 400 lines — and the frame follows it; this is only where a run begins.
const VIEW_WIDTH: u16 = 320;
const VIEW_HEIGHT: u16 = 200;

/// Printed when the command line does not parse.
const USAGE: &str = "\
usage: secondreality [--part <name>] [--frames <N>] [--dump-dir <DIR>]
                     [--scale <N>] [--aspect 4:3|square] [--no-border]
                     [--headless] [--run-seconds <S>]";

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

/// The runner both modes drive: the demo's surface, the picture's starting
/// geometry, and the part the command line named.
///
/// The view leaves its line stride unset, so the beam reads the surface's own
/// rows whatever `VIEW_WIDTH` is: a part that draws through `Surface` and never
/// touches the stride still gets its picture back un-sheared.
fn new_runner(part_name: &str) -> Result<Runner, Box<dyn Error>> {
    let part =
        parts::new_part(part_name).ok_or_else(|| format!("unknown part {part_name:?}"))?;
    Runner::new(
        SURFACE_WIDTH,
        SURFACE_HEIGHT,
        View::new(VIEW_WIDTH, VIEW_HEIGHT),
        part,
    )
    .map_err(Into::into)
}
