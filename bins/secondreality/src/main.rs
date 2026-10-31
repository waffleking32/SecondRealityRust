//! `secondreality` — the port as something you can run.
//!
//! Two ways to run it, chosen before anything is initialised: a window, which
//! needs SDL2 and a display, or `--headless`, which needs neither and must not
//! touch video at all. Both drive the same `Sequence` — a block's parts in the
//! demo's own order, or one part — so what a window plays and what a dump holds
//! are the same run.

mod assets;
mod audio;
mod cli;
mod endscreen;
mod headless;
mod menu;
mod parts;
mod shell;
mod window;

use std::error::Error;
use std::process::ExitCode;

use crate::shell::Sequence;

/// Printed when the command line does not parse.
const USAGE: &str = "\
usage: secondreality [--part <name> | --block <1|2|3|4|5|U>] [--frames <N>]
                     [--dump-frames <A,B,C>] [--dump-dir <DIR>] [--scale <N>]
                     [--aspect square|4:3] [--no-border] [--headless]
                     [--run-seconds <S>] [--audio-dump <FILE>] [--no-audio]
                     [--vault] [--loop] [--no-setup] [/gravis]
                     [--sound-quality authentic|enhanced] [--positions <FILE>]
run:   with no --part and no --block, the demo plays from its beginning: no
       argument is SECOND 1, as it is in the original.
setup: a windowed demo run opens on a settings screen first, as the original asked its
       questions every time: Sound, Sound quality, Demo looping, Aspect and Border.
       Up and Down choose a row, Left and Right change it, Enter starts the run and
       ESC leaves (the demo's end box is printed). The flags give the initial values and
       nothing is saved. --no-setup and /gravis skip it, and --part, --vault and
       --headless never show it.
display: the window shows the picture at 2x by duplicating pixels, with no
       interpolation: each 320x200 pixel is a 2x2 block, so the screen is
       640x400, and the 320x400 and 640x400 parts are the same 640x400.
       --scale <N> magnifies by any whole N instead (each 320x200 pixel N x N).
       --aspect square is the default; --aspect 4:3 stretches the picture into
       a 4:3 box as a 1993 monitor did, which resamples 200 lines into 288 and
       so is not pure duplication. --no-border leaves out the overscan border,
       a flat pad of 8 x N pixels a side. A --headless dump is always 1:1.
loop:  --loop plays a block's run for ever, as the setup screen's looping row
       does: no end scroller, and after CRED the music fades and the demo
       begins again at ALKU (close the window to leave). /gravis plays once
       without the end scroller. Any other /switch is ignored, as SECOND.EXE
       ignores it.
keys:  in a window, ESC ends the part that is playing, and Ctrl skips it.
       A block's run ends the way the demo does: once its last part is over
       (ENDSCRL ends by itself at the end of its text, and one ESC leaves it
       early) the music fades, the screen goes blank, the window closes and
       the demo's end box is printed here; ESC during the fade closes the
       window at once. A --part run holds its last screen
       until ESC closes the window. No other key does anything, and nothing
       waits on a key but the setup screen.
vault: --vault opens the window's menu for the three unreleased effects — GRID,
       HARD and TWIST. Up and Down choose, Enter plays, ESC at a held part
       returns to the menu, and ESC in the menu closes the window. The menu is
       window-only: a headless run names a part with --part instead.
music: a window plays the demo's music on the default sound card; --no-audio runs
       silent, and --audio-dump <FILE> writes it to a WAV instead (the ring has one
       consumer, so a dump is silent to watch). The music runs either way.
       --sound-quality enhanced mixes it at 48 kHz, interpolated: the port's own
       renderer, with the same clock and pictures; authentic, the default, is the
       original's mixer at its own 20 kHz. A run with no music (the test card, GRID,
       HARD, TWIST) refuses enhanced, as it refuses --audio-dump.
dump:  --headless runs with no window and no sound card and needs --dump-dir and one of
       --frames, --dump-frames or --run-seconds. --positions <FILE> is headless only and
       writes the run's timeline as CSV: a header, then one line a tick,
       index,pass,part,order,row (the frame's index, the loop pass, the part whose
       picture is up, and the music's published order and row, empty for a run with no
       music). It needs --frames or --dump-frames, because a --run-seconds run ticks in
       batches.";

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
    // `fatalexit` prints the end box on every exit, the error path's included, and
    // before the error itself (`MAIN/U2.ASM:974`, `printerror` after it).
    if endscreen::follows(&cli) {
        endscreen::print();
    }
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("secondreality: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The run both modes drive: a block's plan, or the one part `--part` named. Nothing here
/// touches SDL, so `--headless` runs on a machine with no display.
fn new_sequence(cli: &cli::Cli) -> Result<Sequence, Box<dyn Error>> {
    let settings = cli.run_settings();
    let plan = match cli.block {
        Some(block) => shell::run_plan(block, settings.looping)?,
        None => vec![shell::resolve(cli.part_name())?],
    };
    Ok(Sequence::new(plan, settings)?)
}
