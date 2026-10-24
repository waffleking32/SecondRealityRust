//! `secondreality` — the port as something you can run.
//!
//! Two ways to run it, chosen before anything is initialised: a window, which
//! needs SDL2 and a display, or `--headless`, which needs neither and must not
//! touch video at all. Both drive the same `Sequence` — a block's ported parts
//! in the demo's own order, or one part — so what a window plays and what a
//! dump holds are the same run. The engine keeps zero runtime dependencies;
//! the window is a client of it like any other.

mod audio;
mod cli;
mod endscreen;
mod headless;
mod parts;
mod shell;
mod vault_menu;
mod window;

use std::error::Error;
use std::io::IsTerminal;
use std::process::ExitCode;

use crate::shell::{Ending, Sequence, SURFACE_HEIGHT, SURFACE_WIDTH};

/// Printed when the command line does not parse.
const USAGE: &str = "\
usage: secondreality [--part <name> | --block <1|2|3|4|5|U>] [--frames <N>]
                     [--dump-frames <A,B,C>] [--dump-dir <DIR>] [--scale <N>]
                     [--aspect 4:3|square] [--no-border] [--headless]
                     [--run-seconds <S>] [--audio-dump <FILE>] [--no-audio]
                     [--vault]
run:   with no --part and no --block, the demo plays from its beginning: no
       argument is SECOND 1, as it is in the original.
keys:  in a window, ESC ends the part that is playing, and Ctrl skips it.
       A block's run ends the way the demo does: once its last part is over
       (one ESC leaves ENDSCRL) the music fades, the screen goes blank, the
       window closes and the demo's end box is printed here; ESC during the
       fade closes the window at once. A --part run holds its last screen
       until ESC closes the window. No other key does anything, and nothing
       waits on a key.
vault: --vault opens the window's menu for the three unreleased effects — GRID,
       HARD and TWIST. Up and Down choose, Enter plays, ESC at a held part
       returns to the menu, and ESC in the menu closes the window. The menu is
       window-only: a headless run names a part with --part instead.
music: a window plays the demo's music on the default sound card; --no-audio runs
       silent, and --audio-dump <FILE> writes it to a WAV instead (the ring has one
       consumer, so a dump is silent to watch). The music runs either way.";

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
    if prints_end_screen(&cli) {
        print_end_screen();
    }
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
    match cli.block {
        Some(block) => Ok(Sequence::new(
            SURFACE_WIDTH,
            SURFACE_HEIGHT,
            shell::start_view(),
            shell::block_plan(block)?,
            cli.run_settings(),
        )?),
        None => new_sequence_for_part(shell::resolve(cli.part_name())?),
    }
}

/// A run of one part, by its registry name — the run `--part <name>` builds,
/// and the run the vault menu starts for the row `Enter` chose.
///
/// The vault's three are silent and unanchored (`shell::music_for_part` says
/// `None`, `shell::boot_anchor` says `None`), so the menu's runs play no music
/// and number their frames from zero; the window presents them through the
/// same loop a normal window run uses.
pub fn new_sequence_for_part(name: &'static str) -> Result<Sequence, Box<dyn Error>> {
    Ok(Sequence::new(
        SURFACE_WIDTH,
        SURFACE_HEIGHT,
        shell::start_view(),
        vec![name],
        shell::RunSettings::default(),
    )?)
}

/// Whether this command line's run ends by printing the demo's end box: a **windowed
/// demo run** — one built from a block, which is also the run with no argument
/// ([`Ending::Exit`]) — however its window closed. Never a `--part` run, the vault, or
/// `--headless`, whose stdout is the dump's console and not a DOS screen.
fn prints_end_screen(cli: &cli::Cli) -> bool {
    !cli.headless && cli.run_settings().ending == Ending::Exit
}

/// Print `U2END.BIN` to stdout — the terminal the run was launched from is the port's
/// stand-in for the text mode `printend` writes it into (D41) — with its colours when
/// stdout is a terminal and as plain text when it is piped.
fn print_end_screen() {
    match std::fs::read(endscreen::END_SCREEN) {
        Ok(screen) => {
            let ansi = std::io::stdout().is_terminal();
            print!("{}", endscreen::render(&screen, ansi));
        }
        // The box is the demo's goodbye, not its result: a missing file is said, and
        // the run's own outcome is what the exit code reports.
        Err(error) => eprintln!("secondreality: {}: {error}", endscreen::END_SCREEN),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> cli::Cli {
        cli::Cli::parse(args.iter().map(|s| s.to_string())).unwrap()
    }

    #[test]
    fn the_end_screen_is_printed_after_a_windowed_demo_run_only() {
        assert!(prints_end_screen(&parse(&[])), "no argument is the demo");
        assert!(prints_end_screen(&parse(&["--block", "5"])));
        assert!(!prints_end_screen(&parse(&["--part", "glenz"])), "a part run holds");
        assert!(!prints_end_screen(&parse(&["--vault"])), "the vault's runs are parts");
        assert!(
            !prints_end_screen(&parse(&["--headless", "--block", "5", "--dump-dir", "d",
                                        "--frames", "1"])),
            "a headless run is a dump"
        );
    }
}
