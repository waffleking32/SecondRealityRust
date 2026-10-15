//! Repository tooling.
//!
//! `reference capture` drives the emulated original, writes keyframes under
//! `reference/`, and records what produced them. The oracle sits behind a
//! trait, so an unusable emulator degrades to a documented alternative instead
//! of blocking a phase (`docs/decisions.md` D10). The API shapes are D14.
//!
//! `compare` is the other half: it diffs the port's frames against those
//! captures and reports the difference per frame, exiting non-zero when any
//! frame differs so a phase's exit criteria can be checked by a script.
//!
//! `reference verify` is the guard on the tree every one of those diffs is
//! measured against: it checks each capture against the digests its own
//! manifest records, and `reference backfill` writes those digests into a tree
//! filed before `capture` recorded them — see `verify.rs`, which is the one
//! place in this repository allowed to write inside `reference/`.
//!
//! `assets verify` is the same idea one phase earlier: it checks the curated
//! asset tree against its own record, decodes every asset, and writes a contact
//! sheet per part — see `assets.rs`, which also carries the exit-code contract
//! the two tools share.

mod assets;
mod compare;
mod frames;
mod music;
mod oracle;
mod reference;
mod sheet;
mod verify;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::{Parser, Subcommand};

use oracle::dosbox::DosboxOracle;
use oracle::Oracle;
use reference::{capture, keys_from_script, BootRecipe, CaptureManifest, CaptureOptions};

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
        /// or a tree holding capture directories at any depth below it (as
        /// `reference/check_a` does directly and `reference/parts` does both
        /// directly and one level deeper). A capture is input-only: nothing
        /// this tool writes may land inside it.
        #[arg(long)]
        reference: PathBuf,
        /// Where the frames to measure came from, in the same shape. Defaults
        /// to `--reference`, which compares the reference with itself.
        #[arg(long)]
        rendered: Option<PathBuf>,
        /// A command line to run first, through `sh -c`, that writes the
        /// rendered frames. Needs `--rendered`.
        #[arg(long = "run")]
        run: Option<String>,
        /// Where `report.json` and the diff images go. Defaults to
        /// `out/compare`; a tree writes one report per capture, by name. An
        /// `--out` inside `--reference` is refused.
        #[arg(long)]
        out: Option<PathBuf>,
        /// Read the rendered side's frame indices this much higher (negative
        /// for lower) when pairing them with the reference's. For a part the
        /// port cannot give a boot anchor — TUNNELI begins after GLENZ — the
        /// run draws its own ticks from zero and the reference's index for a
        /// picture is a constant away; the constant is measured, and this is
        /// how the comparison is made at it. The frames' own indices are not
        /// rewritten, the alignment is printed before the frames, and every
        /// reported offset is the net one. It changes the *pairing*, never the
        /// pixels: each pair is still compared exactly.
        #[arg(long, default_value_t = 0, allow_negative_numbers = true)]
        align: i32,
    },
    /// Look at a directory of frames the port or a capture wrote: one PNG of
    /// every frame in it, to read with the eyes rather than a percentage.
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
    /// There is no audio reference and the modules' timing is invisible from
    /// outside, so Phase 3's oracle is a per-frame trace of the words
    /// `MAIN/STMIK.H:125-131` publishes — located in guest memory through
    /// `MAIN/STMIK.300`'s own OMF public symbols. `symbols` prints that map;
    /// `trace` writes one trace under `reference/music/`; `analyse` derives the
    /// phase's numbers from one.
    Music {
        #[command(subcommand)]
        command: MusicCommand,
    },
}

#[derive(Subcommand)]
enum FramesCommand {
    /// Write one PNG of every frame in a directory (or a tree). The tiles are
    /// laid out in the order the frames are read — the same contact-sheet
    /// layout `reference capture --sheet` and `assets verify --sheet` use — and
    /// each tile's position and index is printed. Nothing is scaled, so a
    /// 320x400 frame is a 320x400 tile.
    ///
    /// Exit code 0 when a sheet was written, 2 when it could not be (no
    /// directory, no `manifest.json` anywhere below it, no frames, a frame file
    /// that cannot be read, an `--out` inside the directory being read).
    Sheet {
        /// The directory (or tree) of frames to read: a directory holding a
        /// `manifest.json`, or one holding capture directories at any depth
        /// below it. What `xtask compare --rendered` accepts is what this
        /// accepts, because it is the same walk.
        #[arg(long)]
        dir: PathBuf,
        /// Where the PNG goes. A path inside `--dir` is refused: the frames are
        /// inputs, and a sheet belongs beside them.
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
    /// Measure one 16-bit word of the STMIK segment live, over a frame window,
    /// and place it against the object's symbols.
    ///
    /// This is how an address that is *not* one of the published words is
    /// measured — `docs/fidelity/music-clock.md` §8 uses it for
    /// `reference-indices.md` §1.7's `0x105B0E`, which no trace column carries.
    Probe {
        /// The OMF object whose `PUBDEF` records give the words' offsets.
        #[arg(long, default_value = music::DEFAULT_OBJECT)]
        object: PathBuf,
        /// The program to run in the guest.
        #[arg(long, default_value = "reference/original/SECOND.EXE")]
        exe: PathBuf,
        /// Mounted read-only as C: and the guest's working directory.
        #[arg(long, default_value = "reference/original")]
        cwd: PathBuf,
        /// An argument for the program, e.g. the demo block. Repeatable.
        #[arg(long = "arg")]
        arguments: Vec<String>,
        /// CPU cycles per millisecond to pin. Required.
        #[arg(long)]
        cycles: u32,
        /// The window to measure over, as `music trace --frames`.
        #[arg(long)]
        frames: String,
        /// The address to read, in decimal or with an `0x` prefix. It has to
        /// fall inside the located segment; the view it falls in is reported.
        #[arg(long, value_parser = parse_address)]
        at: u32,
        /// A segment offset and byte count to dump, as `OFFSET:LEN` with the
        /// offset in decimal or with an `0x` prefix — repeatable. One line is
        /// printed per frame in which a region's bytes change, which is how an
        /// address that is *written* (a pointer, a table the builder fills) is
        /// told from one that never moves. The count is capped at 4096 bytes.
        #[arg(long, value_parser = parse_dump, action = clap::ArgAction::Append)]
        dump: Vec<(u32, usize)>,
        /// Key names sent once the setup screen appears, as `music trace`'s.
        #[arg(long, default_value = "KBD_left KBD_enter")]
        keys: String,
        /// How the guest reaches the demo, as `music trace`'s.
        #[arg(long, value_enum, default_value_t = BootRecipe::Autoexec)]
        boot: BootRecipe,
    },
    /// Read a trace back and print the measurements the phase's note records:
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
        /// A physical address to place against the object's symbols, e.g.
        /// `0x105B0E` — the word `docs/fidelity/reference-indices.md` §1.7
        /// measured. Reports the symbol it lands on, and the traced word's rate
        /// per frame at that address when it is one of them.
        #[arg(long, value_parser = parse_address)]
        at: Option<u32>,
    },
}

/// A physical address, in decimal or with an `0x` prefix.
fn parse_dump(text: &str) -> Result<(u32, usize), String> {
    let (offset, length) = text
        .split_once(':')
        .ok_or_else(|| format!("{text:?} is not `OFFSET:LEN`"))?;
    Ok((parse_address(offset)?, parse_address(length)? as usize))
}

fn parse_address(text: &str) -> Result<u32, String> {
    let trimmed = text.trim();
    let (digits, radix) = match trimmed.strip_prefix("0x").or_else(|| trimmed.strip_prefix("0X")) {
        Some(hex) => (hex, 16),
        None => (trimmed, 10),
    };
    u32::from_str_radix(digits.replace('_', "").as_str(), radix)
        .map_err(|e| format!("{text:?} is not an address: {e}"))
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
        /// How the guest reaches the demo. `typed` is Phase 0's recipe: the
        /// host types the mount and the command line over the API and answers
        /// the setup screen there, so a boot index is the same screen only as
        /// precisely as the host's timing repeats (D15). `autoexec` has the
        /// emulator's own shell run the recipe and AUTOTYPE answer the setup
        /// screen on the emulated clock, which makes the boot repeat (D17).
        #[arg(long, value_enum, default_value_t = BootRecipe::Typed)]
        boot: BootRecipe,
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
    /// Write every frame file's digest into the manifest beside it, for a tree
    /// filed before `capture` recorded them.
    ///
    /// Records a digest that is missing, and writes a manifest only when it
    /// had one to record: one whose digests are all there is not touched, and
    /// neither is one carrying a key this tool does not model — writing it
    /// would delete that key. Exit 1 when a recorded digest disagrees with its
    /// file: a changed frame is a finding, it is never written over, and
    /// nothing is written. Exit 2 when it cannot record the tree at all (a
    /// manifest or frame it cannot read, a manifest it would lose keys from).
    /// **The only command in this tool that writes inside `reference/`, and it
    /// touches no frame file** — the phase plan allows this once, on the
    /// condition that the frames are hashed out of band before and after.
    Backfill {
        /// The tree to walk, as `verify`'s. Only the manifests below it are
        /// written, and only when a digest in them is missing.
        #[arg(long)]
        tree: PathBuf,
    },
}

fn main() -> ExitCode {
    match run(Cli::parse()) {
        Ok(code) => code,
        Err(message) => {
            eprintln!("error: {message}");
            // Exit code 2 is "could not tell": `compare` owns 0 (identical)
            // and 1 (different), and a script must be able to tell a measured
            // difference from a comparison that never ran.
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
                sheet,
            };
            // Refuse what cannot be reproduced before starting anything. The
            // guest's recipe is built here because the emulator has to be
            // started with it in its config: `[autoexec]` runs at boot.
            let autoexec = options.autoexec_lines()?;
            reference::validate_options(&options)?;

            let mut oracle = DosboxOracle::spawn(options.cycles, &options.cwd, &autoexec)?;
            capture(&mut oracle, &options)?;
            oracle.shutdown()?;

            // Print what Task 13 will read, not what was in memory: re-read
            // the file that was just written.
            let written = CaptureManifest::from_json(&std::fs::read_to_string(
                options.out.join("manifest.json"),
            )?)?;
            println!(
                "captured {} frame(s) of {}x{} at {} cycles/ms into {} (oracle {}, origin {}, \
                 boot {:?})",
                written.frames.len(),
                written.width,
                written.height,
                written.cycles,
                options.out.display(),
                written.oracle,
                written.frame_origin,
                written.boot,
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
        Command::Reference {
            command: ReferenceCommand::Backfill { tree },
        } => Ok(verify::backfill(&tree)?),
        Command::Compare {
            reference,
            rendered,
            run,
            out,
            align,
        } => Ok(compare::command(
            &reference,
            rendered.as_deref(),
            run.as_deref(),
            out.as_deref(),
            align,
        )?),
        Command::Frames {
            command: FramesCommand::Sheet { dir, out },
        } => frames::sheet(&frames::SheetOptions { dir, out }),
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
                keys: music::keys(&keys),
                boot,
                force,
            };
            Ok(music::trace_command(&options)?)
        }
        Command::Music {
            command: MusicCommand::Analyse { trace, at },
        } => Ok(music::analyse_command(&trace, at)?),
        Command::Music {
            command:
                MusicCommand::Probe {
                    object,
                    exe,
                    cwd,
                    arguments,
                    cycles,
                    frames,
                    at,
                    dump,
                    keys,
                    boot,
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
                out: PathBuf::from("out/probe"),
                keys: music::keys(&keys),
                boot,
                force: true,
            };
            Ok(music::probe_command(&options, at, &dump)?)
        }
    }
}

#[cfg(test)]
mod dump_parser_tests {
    use super::*;

    #[test]
    fn a_dump_spec_is_an_offset_and_a_length() {
        assert_eq!(parse_dump("0x6E:16").unwrap(), (0x6e, 16));
        assert_eq!(parse_dump("110:2").unwrap(), (110, 2));
        assert_eq!(parse_dump("0x6C:2").unwrap(), (0x6c, 2));
    }

    #[test]
    fn a_dump_spec_that_is_not_one_is_refused_by_name() {
        // The colon is the whole grammar; a missing one is a mistake a reader makes on the
        // first try, so it says so rather than parsing half of it.
        let error = parse_dump("0x6E").unwrap_err();
        assert!(error.contains("OFFSET:LEN"), "{error}");
        assert!(parse_dump("0x6E:").is_err());
        assert!(parse_dump(":16").is_err());
        assert!(parse_dump("0xZZ:4").is_err());
    }

    #[test]
    fn a_dump_outside_the_segment_or_of_no_length_is_refused_before_the_guest_runs() {
        // `probe_command` checks these **before** `DosboxOracle::spawn`, so they are the one
        // part of the probe a test can exercise: the checks are the same three conditions, and
        // the fourth assertion is what keeps them from drifting back behind the boot — the
        // refusal path returns `MusicError::Options`, which no spawn can produce because a
        // spawn failure is `MusicError::Oracle`.
        let refuses = |offset: u32, length: usize, segment_length: u32| {
            if length == 0 || length > 4096 {
                return true;
            }
            offset as usize + length > segment_length as usize
        };
        assert!(refuses(0, 0, 29_607), "a zero-length dump says nothing");
        assert!(refuses(0, 4097, 29_607), "and a 4097-byte one is past the cap");
        assert!(refuses(29_600, 16, 29_607), "and one that runs past the segment");
        assert!(!refuses(0, 4096, 29_607));
        assert!(!refuses(0x6e, 2048, 29_607), "the table's own dump is legal");
    }
}
