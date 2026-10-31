//! Repository tooling.
//!
//! - `reference capture` drives the emulated original and files keyframes under `reference/`
//!   (the oracle is behind a trait, D10; its API is D14). `reference verify` checks that tree
//!   against the digests its manifests record (D23).
//! - `compare` diffs frames against a capture; `rediff` re-renders every filed frame and checks
//!   each against its registered count.
//! - `frames sheet` and `assets verify --sheet` write contact sheets to look at; `assets verify`
//!   checks `assets/` against `assets/PROVENANCE.md`.
//! - `music` reads the original's player state out of the emulator (D26).
//!
//! Every checking command exits **0** when everything checked out, **1** when a check failed
//! (a frame differs, a count moved, a digest does not match) and **2** when it could not check
//! at all, so a script can tell "they differ" from "it could not tell". Nothing writes inside
//! `reference/` or `assets/` except `reference capture` (and `music trace`, under
//! `reference/music/`).

mod assets;
mod capture;
mod compare;
mod music;
mod oracle;
mod paths;
mod rediff;
mod reference;
mod sheet;
mod verify;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use oracle::dosbox::DosboxOracle;
use oracle::Oracle;
use reference::{
    capture, keys_from_script, BootRecipe, BootWait, CaptureManifest, CaptureOptions,
};

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
    /// Diff frames against a reference capture and report how far apart they
    /// are. Exit code 0 when every frame is identical, 1 when any differs, 2
    /// when the comparison could not run at all (a geometry mismatch, a
    /// reference index with no rendered frame within one frame of it, a missing
    /// or malformed manifest or frame file). A frame may be paired one index
    /// either side of its reference index, and the offset is reported (D18).
    Compare {
        /// The capture to measure against: a directory holding `manifest.json`,
        /// or a tree holding capture directories at any depth below it. It is
        /// input-only.
        #[arg(long)]
        reference: PathBuf,
        /// The frames to measure, in the same shape. Defaults to `--reference`.
        #[arg(long)]
        rendered: Option<PathBuf>,
        /// Where `report.json` and the diff images go. Defaults to
        /// `out/compare`; a tree writes one report per capture, by name. An
        /// `--out` inside `--reference` is refused.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Read the rendered side's frame indices this much higher (negative
        /// for lower) when pairing them with the reference's: for a run that
        /// numbers its own ticks from zero, at a measured constant (TUNNELI,
        /// `docs/fidelity/tunneli.md` §7.2). It moves the pairing, never the
        /// pixels, and every reported offset is the net one.
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        align: i32,
    },
    /// Re-render every filed reference frame and check each one's differing-pixel
    /// count against the count registered for it.
    ///
    /// One headless run of the release binary per filed capture under
    /// `reference/parts`, with all of that capture's rendered indices in one
    /// `--dump-frames`, at the mapping the part's own test states (no ±1 search).
    /// Exit code 0 when every frame is at its registered count, 1 when any is off
    /// it, 2 when something could not be compared (a missing frame, a geometry
    /// mismatch, a capture with no table entry, an entry with no capture, a failed
    /// run).
    Rediff {
        /// The binary to run. Without it, `cargo build --release -p secondreality`
        /// runs first and its executable is used.
        #[arg(long)]
        binary: Option<PathBuf>,
        /// Only this capture, or the captures below this directory (`loop`,
        /// `techno/troll`). May be given more than once.
        #[arg(long)]
        only: Vec<String>,
        /// Where the dumps, the run logs and `report.json` go. Defaults to
        /// `out/rediff`; a directory inside `reference/` is refused.
        #[arg(long)]
        out: Option<PathBuf>,
        /// The tree the table's capture paths are below. Defaults to
        /// `reference/parts`; for the command's own tests.
        #[arg(long, hide = true)]
        reference: Option<PathBuf>,
        /// A JSON table to use in place of the compiled one; for the command's own
        /// tests.
        #[arg(long, hide = true)]
        table: Option<PathBuf>,
    },
    /// Look at a directory of frames the port or a capture wrote: one PNG of
    /// every frame in it.
    Frames {
        #[command(subcommand)]
        command: FramesCommand,
    },
    /// Work with the curated asset tree in `assets/`.
    Assets {
        #[command(subcommand)]
        command: AssetsCommand,
    },
    /// The music player's own state, read out of the emulated original.
    ///
    /// The oracle is a per-frame trace of the words `MAIN/STMIK.H:125-131`
    /// publishes, located in guest memory through `MAIN/STMIK.300`'s own OMF
    /// public symbols. `symbols` prints that map; `trace` writes one trace under
    /// `reference/music/`; `analyse` derives the music notes' numbers from one.
    Music {
        #[command(subcommand)]
        command: MusicCommand,
    },
}

#[derive(Subcommand)]
enum FramesCommand {
    /// Write one PNG of every frame in a directory (or a tree), in the layout
    /// `reference capture --sheet` and `assets verify --sheet` use, and print
    /// each tile's position and index. Nothing is scaled.
    ///
    /// Exit code 0 when a sheet was written, 2 when it could not be (no
    /// directory, no `manifest.json` anywhere below it, no frames, a frame file
    /// that cannot be read, an `--out` inside the directory being read).
    Sheet {
        /// The directory (or tree) of frames to read, as `compare --rendered`
        /// accepts it.
        #[arg(long)]
        dir: PathBuf,
        /// Where the PNG goes. A path inside `--dir` is refused.
        #[arg(long)]
        out: PathBuf,
    },
}

#[derive(Subcommand)]
enum AssetsCommand {
    /// Check the curated tree against its provenance, decode every asset, and
    /// write a contact sheet per part to look at.
    ///
    /// Exit code 0 when everything asked for checked out, 1 when a check
    /// failed (a digest that does not match `assets/PROVENANCE.md`, a
    /// provenance row with no file, a file with no provenance row, an asset
    /// whose magic says it is a picture and which does not decode), and 2 when
    /// the tool could not run at all (no `assets/`, an unreadable or malformed
    /// provenance, a refused `--sheet`, or nothing asked for).
    Verify {
        /// Re-hash every file in the tree and compare it with the digest
        /// `assets/PROVENANCE.md` records for it.
        #[arg(long)]
        tree: bool,
        /// Decode every asset that has a decoder and report what each one is:
        /// format, dimensions and palette size for a picture, frame count for
        /// an animation, and what a headerless file is not.
        #[arg(long)]
        decode: bool,
        /// Write one PNG per part, each of that part's assets a tile, into this
        /// directory. Nothing is scaled and no text is drawn, so the tile order
        /// is printed as it goes. A directory inside `assets/` is refused: the
        /// tree is input-only.
        #[arg(long)]
        sheet: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum MusicCommand {
    /// Print `MAIN/STMIK.300`'s public symbols: the record census, the segments,
    /// every `PUBDEF` name with its offset inside the segment, the words the
    /// trace reads, and the two runs of the object's own `LEDATA` data the
    /// locator searches guest memory for.
    ///
    /// Every address the trace reads comes from here, so this is the command
    /// behind the numbers in `docs/fidelity/music-clock.md`.
    Symbols {
        /// The OMF object to walk. The default is the tree's own music player,
        /// the object STMIK was linked from.
        #[arg(long, default_value = music::DEFAULT_OBJECT)]
        object: PathBuf,
    },
    /// Trace the emulated original's own music-player state, one row per
    /// reference frame, into `--out`.
    ///
    /// The trace is keyed by the guest's own frame counter, never by a
    /// hoped-for index: each sample is bracketed by two counter reads and
    /// re-taken when the counter moved between them, and the manifest states
    /// the coverage actually achieved. A trace with gaps **says so** and does
    /// not describe itself as per-frame.
    ///
    /// `--out` is refused inside a frame capture's directory, and inside
    /// `reference/` the only place it may point is a new directory under
    /// `reference/music/` — the rest of the tree is the reference, and a file
    /// written among it is a file every later diff is measured against.
    Trace {
        /// The OMF object whose `PUBDEF` records give the words' offsets.
        #[arg(long, default_value = music::DEFAULT_OBJECT)]
        object: PathBuf,
        /// The program to run in the guest.
        #[arg(long, default_value = "reference/original/SECOND.EXE")]
        exe: PathBuf,
        /// Mounted read-only as C: and the guest's working directory;
        /// SECOND.EXE loads REALITY.FC from it and the two cannot be separated.
        #[arg(long, default_value = "reference/original")]
        cwd: PathBuf,
        /// An argument for the program, e.g. the demo block. Repeatable.
        #[arg(long = "arg")]
        arguments: Vec<String>,
        /// CPU cycles per millisecond to pin (50..2000000). Required: an
        /// unpinned trace is not reproducible and is refused.
        #[arg(long)]
        cycles: u32,
        /// The frames to sample: `A..B` (A up to but not including B, the Rust
        /// range), `A..=B` (both ends included), a comma-separated list, or a
        /// list of ranges — `0..1300` and `0..1299,2000` both work. Indices are
        /// boot-relative, the same space every capture's are, and every frame
        /// in the window is sampled.
        #[arg(long)]
        frames: String,
        /// Where `trace.csv` and `manifest.json` are written.
        #[arg(long)]
        out: PathBuf,
        /// Key names sent once the setup screen appears, whitespace separated,
        /// e.g. "KBD_left KBD_enter". Under `--boot autoexec` they are the
        /// buttons AUTOTYPE presses instead. The soundcard these keys select is
        /// part of the measurement: the trace reads the card's own words.
        #[arg(long, default_value = "KBD_left KBD_enter")]
        keys: String,
        /// How the guest reaches the demo, as `reference capture`'s. The
        /// default is D17's recipe, the one every capture's indices are in.
        #[arg(long, value_enum, default_value_t = BootRecipe::Autoexec)]
        boot: BootRecipe,
        /// Replace a trace the output directory already holds.
        #[arg(long)]
        force: bool,
    },
    /// Read a trace back and print the measurements the music notes record:
    /// where the music starts, the tick rate and its residual against the
    /// module's own header, the frame-to-frame quantisation of the row changes,
    /// the guest's soundcard and mixer words, `zplus`/`zinfo`, and what the
    /// player does at the order list's `254`/`255` entries.
    Analyse {
        /// The trace directory: the one `music trace --out` wrote, holding
        /// `trace.csv` and `manifest.json`. The trace file is checked against
        /// the digest its manifest records before anything is read out of it.
        #[arg(long)]
        trace: PathBuf,
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
        /// separated, e.g. "KBD_left KBD_enter". Under --boot autoexec they
        /// are the buttons AUTOTYPE presses instead.
        #[arg(long)]
        keys: Option<String>,
        /// How the guest reaches the demo. `typed`: the host types the mount and the command line over the API and answers
        /// the setup screen there, so a boot index is the same screen only as
        /// precisely as the host's timing repeats (D15). `autoexec` has the
        /// emulator's own shell run the recipe and AUTOTYPE answer the setup
        /// screen on the emulated clock, which makes the boot repeat (D17).
        #[arg(long, value_enum, default_value_t = BootRecipe::Typed)]
        boot: BootRecipe,
        /// What the autoexec boot waits for before the capture starts.
        /// `setup` is the demo's sound-card setup screen. `graphics` waits for
        /// the first graphics frame instead, for a program that never draws
        /// that screen (the vault's DIS + effect batch). Only `--boot autoexec` can choose;
        /// the typed recipe always waits for the setup screen.
        #[arg(long, value_enum, default_value_t = BootWait::Setup)]
        boot_wait: BootWait,
        /// Write one PNG of the frames this capture takes, to look at. The
        /// tiles are the frames, in the order they were captured, and their
        /// indices are printed as they are laid out. A path inside the
        /// reference tree (`reference/`, which holds the mounted original and
        /// every capture) is refused: the reference is input-only, and a sheet
        /// goes beside it, e.g. `out/<name>-sheet.png`.
        #[arg(long)]
        sheet: Option<PathBuf>,
    },
    /// Check the tree every frame diff is measured against, against the
    /// digests its own manifests record.
    ///
    /// Exit code 0 when every capture checked out, 1 when any frame is
    /// missing, is a length its geometry does not imply, is not named by the
    /// manifest beside it, or does not hash as its entry records; 2 when the
    /// tool could not check at all (nothing at the tree path, no manifest
    /// below it, a manifest it cannot read or does not know, a capture whose
    /// manifest names no frames, a file it cannot read). A file that is merely
    /// absent is a finding and not exit 2: the check that failed is "every
    /// file the manifest names exists".
    Verify {
        /// The tree to walk: every directory below it holding a
        /// `manifest.json` is a capture, at any depth. Nothing is written.
        #[arg(long)]
        tree: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            // 2 is "could not tell" (see the module doc).
            ExitCode::from(2)
        }
    }
}

fn run(cli: Cli) -> Result<ExitCode, Box<dyn std::error::Error>> {
    match cli.command {
        Command::Reference {
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
                    boot,
                    boot_wait,
                    sheet,
                },
        } => {
            let options = CaptureOptions {
                exe,
                cwd,
                arguments,
                cycles,
                frames,
                out,
                skip_boot_frames,
                keys: keys.as_deref().map(keys_from_script).unwrap_or_default(),
                boot,
                boot_wait,
                sheet,
            };
            // Refuse what cannot be reproduced before starting anything. The
            // recipe goes into the emulator's config: `[autoexec]` runs at boot.
            let autoexec = options.autoexec_lines()?;
            reference::validate_options(&options)?;

            let mut oracle = DosboxOracle::spawn(options.cycles, &options.cwd, &autoexec)?;
            capture(&mut oracle, &options)?;
            oracle.shutdown()?;

            // Print what was written, not what was in memory.
            let written = CaptureManifest::from_json(&std::fs::read_to_string(
                options.out.join("manifest.json"),
            )?)?;
            println!(
                "captured {} frame(s) of {}x{} at {} cycles/ms into {} (oracle {}, origin {}, \
                 boot {:?}, boot wait {:?})",
                written.frames.len(),
                written.width,
                written.height,
                written.cycles,
                options.out.display(),
                written.oracle,
                written.frame_origin,
                written.boot,
                written.boot_wait,
            );
            for frame in &written.frames {
                match &frame.idx {
                    Some(idx) => println!("  frame {}: {idx}, {}", frame.index, frame.rgb),
                    None => println!("  frame {}: {} (rgb only)", frame.index, frame.rgb),
                }
            }
            if let Some(sheet) = &options.sheet {
                reference::write_sheet(sheet, &options.out, &written)?;
            }
            Ok(ExitCode::SUCCESS)
        }
        Command::Reference {
            command: ReferenceCommand::Verify { tree },
        } => Ok(verify::command(&tree)?),
        Command::Compare {
            reference,
            rendered,
            out,
            align,
        } => Ok(compare::command(
            &reference,
            rendered.as_deref(),
            out.as_deref(),
            align,
        )?),
        Command::Rediff {
            binary,
            only,
            out,
            reference,
            table,
        } => Ok(rediff::command(&rediff::Options {
            binary,
            only,
            out,
            reference,
            table,
        })?),
        Command::Frames {
            command: FramesCommand::Sheet { dir, out },
        } => Ok(sheet::frames_command(&dir, &out)?),
        Command::Assets {
            command:
                AssetsCommand::Verify {
                    tree,
                    decode,
                    sheet,
                },
        } => Ok(assets::command(
            Path::new(assets::DEFAULT_TREE),
            &assets::Options {
                tree,
                decode,
                sheet,
            },
        )?),
        Command::Music {
            command: MusicCommand::Symbols { object },
        } => Ok(music::symbols_command(&object)?),
        Command::Music {
            command:
                MusicCommand::Trace {
                    object,
                    exe,
                    cwd,
                    arguments,
                    cycles,
                    frames,
                    out,
                    keys,
                    boot,
                    force,
                },
        } => {
            let window = music::parse_window(&frames)?;
            let options = music::TraceOptions {
                object,
                exe,
                cwd,
                arguments,
                cycles,
                window,
                out,
                keys: keys_from_script(&keys),
                boot,
                force,
            };
            Ok(music::trace_command(&options)?)
        }
        Command::Music {
            command: MusicCommand::Analyse { trace },
        } => Ok(music::analyse_command(&trace)?),
    }
}
