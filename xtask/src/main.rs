//! Repository tooling.
//!
//! The first subcommand is `reference capture`: drive the emulated original,
//! write keyframes under `reference/`, and record what produced them. The
//! oracle sits behind a trait, so an unusable emulator degrades to a
//! documented alternative instead of blocking a phase (`docs/decisions.md`
//! D10). The API shapes are D14.

mod oracle;
mod reference;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use oracle::dosbox::DosboxOracle;
use oracle::Oracle;
use reference::{capture, keys_from_script, CaptureManifest, CaptureOptions};

#[derive(Parser)]
#[command(name = "xtask", about = "Repository tooling")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Work with the reference the port is measured against.
    Reference {
        #[command(subcommand)]
        command: ReferenceCommand,
    },
}

#[derive(Subcommand)]
enum ReferenceCommand {
    /// Capture frames from the emulated original into --out.
    Capture {
        /// The program to run in the guest.
        #[arg(long, default_value = "reference/original/SECOND.EXE")]
        exe: PathBuf,
        /// Mounted read-only as C: and the guest's working directory;
        /// SECOND.EXE loads REALITY.FC from it and the two cannot be
        /// separated.
        #[arg(long, default_value = "reference/original")]
        cwd: PathBuf,
        /// An argument for the program, e.g. the demo block. Repeatable.
        #[arg(long = "arg")]
        arguments: Vec<String>,
        /// CPU cycles per millisecond to pin (50..2000000). Required: an
        /// unpinned capture is not reproducible and is refused.
        #[arg(long)]
        cycles: u32,
        /// Boot-relative frame indices to capture, comma separated. They
        /// count from the emulator's first rendered frame at process start.
        #[arg(long, value_delimiter = ',')]
        frames: Vec<u64>,
        /// Where the frames and manifest.json are written.
        #[arg(long)]
        out: PathBuf,
        /// Boot frames skipped before the capture begins.
        #[arg(long, default_value_t = 0)]
        skip_boot_frames: u64,
        /// Key names sent once the setup screen appears, whitespace
        /// separated, e.g. "KBD_left KBD_enter".
        #[arg(long)]
        keys: Option<String>,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("error: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    let Command::Reference {
        command:
            ReferenceCommand::Capture {
                exe,
                cwd,
                arguments,
                cycles,
                frames,
                out,
                skip_boot_frames,
                keys,
            },
    } = cli.command;
    let options = CaptureOptions {
        exe,
        cwd,
        arguments,
        cycles,
        frames,
        out,
        skip_boot_frames,
        keys: keys.as_deref().map(keys_from_script).unwrap_or_default(),
    };
    // Refuse what cannot be reproduced before starting anything.
    reference::validate_options(&options)?;

    let mut oracle = DosboxOracle::spawn(options.cycles, &options.cwd)?;
    capture(&mut oracle, &options)?;
    oracle.shutdown()?;

    // Print what Task 13 will read, not what was in memory: re-read the file
    // that was just written.
    let written =
        CaptureManifest::from_json(&std::fs::read_to_string(options.out.join("manifest.json"))?)?;
    println!(
        "captured {} frame(s) of {}x{} at {} cycles/ms into {} (oracle {}, origin {})",
        written.frames.len(),
        written.width,
        written.height,
        written.cycles,
        options.out.display(),
        written.oracle,
        written.frame_origin,
    );
    for frame in &written.frames {
        match &frame.idx {
            Some(idx) => println!("  frame {}: {idx}, {}", frame.index, frame.rgb),
            None => println!("  frame {}: {} (rgb only)", frame.index, frame.rgb),
        }
    }
    Ok(())
}
