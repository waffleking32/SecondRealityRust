//! `xtask reference capture`: drive the emulated original, write keyframes
//! under `reference/`, and record what produced them.
//!
//! Task 13 diffs the port against whatever this writes, so two things matter
//! more than anything else here: the manifest says exactly which frame each
//! image is (what origin the index counts from, which arguments, which key
//! sequence, which CPU cycles), and a capture that cannot be reproduced is
//! refused instead of written.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::oracle::{CapturedFrame, Oracle, OracleError, DEFAULT_WAIT, FRAME_ORIGIN};

/// The pinned commit of `upstream/SecondReality` the port follows
/// (`.gitmodules`; `docs/decisions.md` D13).
pub const UPSTREAM_COMMIT: &str = "071a82e";

/// The tree the reference lives in, where the CLI's defaults put the mounted
/// original (`reference/original`) and where every capture goes
/// (`reference/parts/<part>/` in Part B). It is **input-only**: `compare`
/// refuses an `--out` inside one of its captures, and this tool refuses a
/// `--sheet` inside it, for the same reason — a file written among the frames
/// is a file every later diff is measured against.
pub const REFERENCE_ROOT: &str = "reference";

/// The original's sound card setup screen. `SECOND.EXE` shows it whatever the
/// block argument, and the default choice fails on this emulator, so the
/// capture waits for this text before sending `--keys` (D14).
pub const SETUP_SCREEN_TEXT: &str = "Soundcard";

/// How long `AUTOTYPE` waits before its first key, in seconds.
///
/// Every step it waits for is on the emulated clock, so this only has to be
/// after the setup screen is up — the screen then waits for an answer, and a
/// longer wait costs wall-clock time rather than determinism. Measured under
/// `--boot autoexec`: the screen's text is up at boot frame 16 (0.23 s), so
/// five seconds leaves about 4.8 s of margin and is what D17's captures were
/// taken with.
pub const AUTOTYPE_WAIT_SECONDS: f64 = 5.0;

const MANIFEST_VERSION: u32 = 1;

#[derive(Debug)]
pub enum CaptureError {
    /// The options cannot produce a reproducible capture. Nothing was started.
    Options(String),
    Oracle(OracleError),
    Io {
        path: PathBuf,
        message: String,
    },
    /// The emulator changed display geometry between frames, so one manifest
    /// cannot honestly describe them all: capture one screen at a time.
    GeometryChanged {
        index: u64,
        expected: (u16, u16),
        found: (u16, u16),
    },
}

impl fmt::Display for CaptureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CaptureError::Options(message) => write!(f, "{message}"),
            CaptureError::Oracle(e) => write!(f, "{e}"),
            CaptureError::Io { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
            CaptureError::GeometryChanged {
                index,
                expected,
                found,
            } => write!(
                f,
                "the display changed geometry between frames: frame {index} is {}x{} where the \
                 capture started at {}x{}; capture one screen at a time",
                found.0, found.1, expected.0, expected.1
            ),
        }
    }
}

impl std::error::Error for CaptureError {}

impl From<OracleError> for CaptureError {
    fn from(error: OracleError) -> CaptureError {
        CaptureError::Oracle(error)
    }
}

/// How the guest is taken from the emulator's boot to the demo.
///
/// A boot index means a screen only as precisely as the boot repeats, so this
/// is part of the capture recipe exactly like the cycles are, and the manifest
/// records it (D17).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum BootRecipe {
    /// Phase 0's recipe: the host types `MOUNT`, `C:` and the program line
    /// over the API and answers the setup screen with a frame-scheduled key
    /// sequence anchored to when the host posted it. Two runs put the same
    /// screen 43-45 frames apart, because every step waits on the host (D15).
    #[default]
    Typed,
    /// The emulator's own shell runs the recipe from its `[autoexec]` section
    /// and `AUTOTYPE` answers the setup screen from the PIC clock, so the
    /// whole boot is a function of the emulated clock the demo animates on
    /// and the same boot frame is the same screen in every run (D17).
    Autoexec,
}

/// Everything a capture needs, and everything the manifest records about it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOptions {
    /// The program to run in the guest. Bare `SECOND 2` defaults keep the two
    /// reference files together, because `SECOND.EXE` loads `REALITY.FC` from
    /// its working directory.
    pub exe: PathBuf,
    pub cwd: PathBuf,
    pub arguments: Vec<String>,
    pub cycles: u32,
    /// Boot-relative frame indices: counted from the emulator's first rendered
    /// frame at process start (D14).
    pub frames: Vec<u64>,
    pub out: PathBuf,
    /// Frames skipped from that same origin before the capture begins.
    pub skip_boot_frames: u64,
    /// `KBD_*` key names sent in order once the setup screen appears. Under
    /// [`BootRecipe::Autoexec`] they become `AUTOTYPE` buttons instead, named
    /// the way the mapper names them (`KBD_left` -> `left`).
    pub keys: Vec<String>,
    /// How the guest reaches the demo. The default is what every capture
    /// written before this field existed used, so their indices keep meaning
    /// what they meant when they were taken (D17).
    pub boot: BootRecipe,
    /// Where to write one PNG of the frames this capture takes — the sheet a
    /// person looks at to see *what* was captured. `None` writes none.
    ///
    /// It is an output of the capture but not part of what the capture *is*:
    /// the frames and the manifest are the reference, and the sheet is how a
    /// person reads them (see [`write_sheet`]).
    pub sheet: Option<PathBuf>,
}

/// `--keys "KBD_left KBD_enter"`: whitespace-separated key names, in order.
pub fn keys_from_script(script: &str) -> Vec<String> {
    script.split_whitespace().map(str::to_owned).collect()
}

impl CaptureOptions {
    /// The `[autoexec]` lines the guest's own shell has to run for this
    /// capture, or an empty list for the typed recipe.
    ///
    /// The mount names the absolute directory for the same reason `spawn`
    /// absolutises the allow-list: the guest resolves a relative host path
    /// against the emulator's own working directory, not ours.
    pub fn autoexec_lines(&self) -> Result<Vec<String>, CaptureError> {
        if self.boot != BootRecipe::Autoexec {
            return Ok(Vec::new());
        }
        let cwd = self.cwd.canonicalize().map_err(|e| CaptureError::Io {
            path: self.cwd.clone(),
            message: e.to_string(),
        })?;
        autoexec_lines(&cwd, &self.exe, &self.arguments, &self.keys)
    }
}

/// The recipe the guest runs itself, as the shell will read it: mount the
/// reference read-only, then `AUTOTYPE` the setup-screen answer, then run the
/// program (D17).
///
/// `AUTOTYPE` comes before the program line because the program does not
/// return: it schedules its key presses on the PIC clock and exits, and the
/// shell runs the demo underneath them. The wait it is given is
/// [`AUTOTYPE_WAIT_SECONDS`].
pub fn autoexec_lines(
    cwd: &Path,
    exe: &Path,
    arguments: &[String],
    keys: &[String],
) -> Result<Vec<String>, CaptureError> {
    let program = crate::oracle::guest_command_line(exe, arguments).ok_or_else(|| {
        CaptureError::Options(format!("--exe {} has no file name to run", exe.display()))
    })?;
    let mut lines = vec![format!("MOUNT C -RO {}", cwd.display()), "C:".to_owned()];
    if !keys.is_empty() {
        let buttons: Vec<&str> = keys.iter().map(|key| autotype_name(key)).collect();
        lines.push(format!(
            "AUTOTYPE -w {AUTOTYPE_WAIT_SECONDS} {}",
            buttons.join(" ")
        ));
    }
    lines.push(program);
    Ok(lines)
}

/// `AUTOTYPE` names buttons the way the mapper does: `left`, `enter`, `kp_8`
/// (`gui/mapper.cpp`), where `input/sequence` names the same keys by their
/// `KBD_KEYS` enumerators. The capture's surface stays the `KBD_*` spelling
/// `reference/check_a`'s manifest records, so the prefix is all that differs.
fn autotype_name(key: &str) -> &str {
    key.strip_prefix("KBD_").unwrap_or(key)
}

/// `frame_000100` — six digits, so a directory listing sorts by frame.
pub fn frame_file_name(index: u64) -> String {
    format!("frame_{index:06}")
}

/// One captured frame on disk.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrameRef {
    pub index: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idx: Option<String>,
    pub rgb: String,
    /// The palette entries this frame's indices use, six bits per component,
    /// keyed by index and recorded so the indexed frame can be widened back to
    /// colour without a second oracle run.
    ///
    /// Only the entries the frame uses are recorded: a running demo rewrites
    /// the rest of the DAC continuously, so a snapshot of all 256 is a snapshot
    /// of a race and does not repeat between runs, while the entries a still
    /// screen shows are stable (and are what the coherence check verified
    /// against the RGB frame).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<BTreeMap<u8, [u8; 3]>>,
}

/// `<DIR>/manifest.json`: what was captured, and what produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaptureManifest {
    pub version: u32,
    /// Emulator and version, e.g. `dosbox-automation 0.85.1 (6d100)`.
    pub oracle: String,
    pub upstream_commit: String,
    /// The program as the guest ran it.
    pub exe: String,
    pub arguments: Vec<String>,
    /// CPU cycles per millisecond, always pinned (D14).
    pub cycles: u32,
    pub width: u16,
    pub height: u16,
    /// What `index` counts from. `"boot"` is the emulator's first rendered
    /// frame at process start; the counter and every index are counted from
    /// there, and `skip_boot_frames` says how much of the boot was skipped.
    pub frame_origin: String,
    pub skip_boot_frames: u64,
    /// How the guest was taken to the demo. A manifest written before D17 does
    /// not carry it and is the typed recipe, which is what `default` means —
    /// and it has to be in the manifest at all because the same boot index is
    /// a different screen under a different recipe.
    #[serde(default)]
    pub boot: BootRecipe,
    pub keys: Vec<String>,
    pub frames: Vec<FrameRef>,
}

impl CaptureManifest {
    pub fn from_json(json: &str) -> Result<CaptureManifest, serde_json::Error> {
        serde_json::from_str(json)
    }
}

/// Refuse a capture that cannot be reproduced, before touching the emulator.
pub fn validate_options(options: &CaptureOptions) -> Result<(), CaptureError> {
    if options.cycles == 0 {
        return Err(CaptureError::Oracle(OracleError::UnpinnedCycles));
    }
    if options.frames.is_empty() {
        return Err(CaptureError::Options(
            "at least one frame index is required (--frames I,J,K)".into(),
        ));
    }
    for pair in options.frames.windows(2) {
        if pair[1] <= pair[0] {
            return Err(CaptureError::Options(format!(
                "frame indices must increase: {} then {}",
                pair[0], pair[1]
            )));
        }
    }
    if let Some(&first) = options.frames.first() {
        if first < options.skip_boot_frames {
            return Err(CaptureError::Options(format!(
                "frame {first} is inside the boot the capture skips (--skip-boot-frames {}); \
                 indices count from the emulator's start",
                options.skip_boot_frames
            )));
        }
    }
    if !options.cwd.is_dir() {
        return Err(CaptureError::Options(format!(
            "{} is not a directory to mount",
            options.cwd.display()
        )));
    }
    let exe_parent = options.exe.parent();
    if exe_parent != Some(options.cwd.as_path()) {
        return Err(CaptureError::Options(format!(
            "--exe {} must live in --cwd {}: the guest runs it from the mounted directory, \
             because it loads its data files from the working directory",
            options.exe.display(),
            options.cwd.display()
        )));
    }
    // `reference/` is input-only: the mounted distribution and every capture
    // live there, `compare` refuses an `--out` inside one of them, and this
    // tool refuses a `--sheet` inside one for the same reason — the reference
    // is read, never written, by anything but a capture.
    if let Some(sheet) = &options.sheet {
        crate::compare::refuse_output_inside(
            sheet,
            &options.cwd,
            "--sheet",
            "that directory is the mounted original (SECOND.EXE and REALITY.FC), and every frame \
             diff in the project is measured against those exact bytes",
            "Pass --sheet outside the reference, e.g. out/<name>-sheet.png",
        )
        .map_err(CaptureError::Options)?;
        crate::compare::refuse_output_inside(
            sheet,
            Path::new(REFERENCE_ROOT),
            "--sheet",
            "the reference tree holds the mounted original and every capture, and a file written \
             among them is an extra file in the tree every later diff is measured against",
            &format!("Pass --sheet outside {REFERENCE_ROOT}/, e.g. out/<name>-sheet.png"),
        )
        .map_err(CaptureError::Options)?;
        // The capture it just wrote is a reference tree too, and the same rule
        // applies to it: the frames and the manifest are the capture, and a
        // sheet is a rendering of them, not part of them.
        crate::compare::refuse_output_inside(
            sheet,
            &options.out,
            "--sheet",
            "that is this capture's own directory, and the frames and manifest in it are what every \
             later diff reads — a sheet belongs beside it, not inside it",
            "Pass --sheet outside the capture directory, e.g. out/<name>-sheet.png",
        )
        .map_err(CaptureError::Options)?;
    }
    Ok(())
}

/// Boot the original under the oracle, capture the requested frames, and write
/// the frames and their manifest. The oracle is left running; shut it down.
pub fn capture(
    oracle: &mut dyn Oracle,
    options: &CaptureOptions,
) -> Result<CaptureManifest, CaptureError> {
    validate_options(options)?;

    let oracle_name = oracle.describe()?;
    // Pinned before the first frame. The emulator was already started with
    // `cpu_cycles`; this is the runtime pin the API documents, and it is what
    // makes a capture reproducible (D14).
    oracle.set_cycles(options.cycles)?;

    oracle.boot(&options.exe, &options.cwd, &options.arguments)?;
    if options.boot == BootRecipe::Typed && !options.keys.is_empty() {
        // The setup screen has to be answered before the demo draws anything.
        // Under the `Autoexec` recipe the guest answers it itself, from the
        // `[autoexec]` section this run was started with (D17).
        oracle.wait_for_text(SETUP_SCREEN_TEXT, DEFAULT_WAIT)?;
        oracle.send_keys(&options.keys.join(" "))?;
    }

    let now = oracle.frame_counter()?;
    if now < options.skip_boot_frames {
        oracle.step_frames((options.skip_boot_frames - now).min(u32::MAX as u64) as u32)?;
    }

    std::fs::create_dir_all(&options.out).map_err(|e| CaptureError::Io {
        path: options.out.clone(),
        message: e.to_string(),
    })?;

    let mut geometry: Option<(u16, u16)> = None;
    let mut frames = Vec::with_capacity(options.frames.len());
    for &index in &options.frames {
        // Wait for this frame. The oracle refuses a capture the emulator has
        // clearly run past (see `Oracle::wait_for_frame` for how close it can
        // come, and D15), because a frame taken later would be written under
        // this one's name.
        oracle.wait_for_frame(index)?;
        let frame = oracle.frame()?;

        let found = (frame.width, frame.height);
        match geometry {
            None => geometry = Some(found),
            Some(expected) if expected != found => {
                return Err(CaptureError::GeometryChanged {
                    index,
                    expected,
                    found,
                })
            }
            Some(_) => {}
        }
        frames.push(write_frame(&options.out, index, &frame)?);
    }

    let (width, height) = geometry.expect("validate_options refuses an empty frame list");
    let manifest = CaptureManifest {
        version: MANIFEST_VERSION,
        oracle: oracle_name,
        upstream_commit: UPSTREAM_COMMIT.to_owned(),
        exe: options
            .exe
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        arguments: options.arguments.clone(),
        cycles: options.cycles,
        width,
        height,
        frame_origin: FRAME_ORIGIN.to_owned(),
        skip_boot_frames: options.skip_boot_frames,
        boot: options.boot,
        keys: options.keys.clone(),
        frames,
    };
    let manifest_path = options.out.join("manifest.json");
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| CaptureError::Options(format!("cannot serialise the manifest: {e}")))?;
    std::fs::write(&manifest_path, format!("{json}\n")).map_err(|e| CaptureError::Io {
        path: manifest_path,
        message: e.to_string(),
    })?;
    Ok(manifest)
}

fn write_frame(
    out: &Path,
    index: u64,
    frame: &CapturedFrame,
) -> Result<FrameRef, CaptureError> {
    let name = frame_file_name(index);
    let rgb = format!("{name}.rgb");
    std::fs::write(out.join(&rgb), &frame.rgb).map_err(|e| CaptureError::Io {
        path: out.join(&rgb),
        message: e.to_string(),
    })?;

    let idx = match (&frame.indexed, &frame.palette) {
        (Some(indexed), Some(palette)) => {
            let file = format!("{name}.idx");
            std::fs::write(out.join(&file), indexed).map_err(|e| CaptureError::Io {
                path: out.join(&file),
                message: e.to_string(),
            })?;
            Some((file, used_palette(indexed, palette)))
        }
        _ => None,
    };

    Ok(FrameRef {
        index,
        idx: idx.as_ref().map(|(file, _)| file.clone()),
        rgb,
        palette: idx.map(|(_, palette)| palette),
    })
}

/// One PNG of a capture's frames, for a person to look at.
///
/// Written from what the capture left on disk rather than from the pixels in
/// memory, for the same reason `main` re-reads the manifest: a sheet drawn from
/// anything else could show something the capture did not write. The frames are
/// tiles in the order they were captured — the layout is [`crate::sheet`]'s, the
/// same one `assets verify --sheet` uses — and because nothing can draw text in
/// a PNG, the tile positions are printed against the frame indices.
pub fn write_sheet(
    sheet: &Path,
    out: &Path,
    manifest: &CaptureManifest,
) -> Result<(), CaptureError> {
    let mut tiles = Vec::with_capacity(manifest.frames.len());
    for frame in &manifest.frames {
        let file = out.join(&frame.rgb);
        let rgb = std::fs::read(&file).map_err(|e| CaptureError::Io {
            path: file,
            message: e.to_string(),
        })?;
        tiles.push(crate::sheet::Tile {
            width: u32::from(manifest.width),
            height: u32::from(manifest.height),
            rgb,
            // The frame's own name for its file, and the index it was captured
            // at — the two things a person needs to tell two frames apart.
            label: format!("frame {} ({})", frame.index, frame.rgb),
        });
    }
    let columns = crate::sheet::columns_for(tiles.len());
    let (width, height, rgb) = crate::sheet::contact_sheet(&tiles, columns);
    crate::sheet::write_png(sheet, width, height, &rgb).map_err(|e| CaptureError::Io {
        path: sheet.to_path_buf(),
        message: e.to_string(),
    })?;
    println!(
        "sheet {}: {} tile(s), {columns} column(s), {width}x{height}",
        sheet.display(),
        tiles.len()
    );
    for (index, tile) in tiles.iter().enumerate() {
        let (row, column) = crate::sheet::position(index, columns);
        println!("  r{row}c{column} {}", tile.label);
    }
    Ok(())
}

/// The palette entries an indexed frame uses, in index order.
fn used_palette(indexed: &[u8], palette: &[[u8; 3]; 256]) -> BTreeMap<u8, [u8; 3]> {
    let mut used = BTreeMap::new();
    for &index in indexed {
        used.entry(index).or_insert(palette[index as usize]);
    }
    used
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::oracle::dosbox::decode_frame;
    use std::collections::VecDeque;
    use std::time::Duration;

    const FRAME_BIN: &[u8] = include_bytes!("../tests/fixtures/frame.bin");
    const FB_BIN: &[u8] = include_bytes!("../tests/fixtures/fb.bin");
    const DAC_BIN: &[u8] = include_bytes!("../tests/fixtures/dac.bin");
    const MODEX_FRAME_BIN: &[u8] = include_bytes!("../tests/fixtures/modex_title_frame.bin");

    fn recorded_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        for (i, entry) in palette.iter_mut().enumerate() {
            entry.copy_from_slice(&DAC_BIN[i * 3..i * 3 + 3]);
        }
        palette
    }

    /// A frame as the recorded fixtures held it: 320×200 with indices.
    fn indexed_frame() -> CapturedFrame {
        let mut frame = decode_frame(FRAME_BIN).unwrap();
        frame.palette = Some(recorded_palette());
        frame.indexed = Some(FB_BIN.to_vec());
        frame
    }

    fn options(frames: Vec<u64>, out: &Path) -> CaptureOptions {
        // The real reference directory: validation refuses a --cwd that is
        // not there, and this is the one that exists.
        let cwd = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/original");
        CaptureOptions {
            exe: cwd.join("SECOND.EXE"),
            cwd,
            arguments: vec!["2".into()],
            cycles: 25000,
            frames,
            out: out.to_path_buf(),
            skip_boot_frames: 5000,
            keys: keys_from_script("KBD_left KBD_enter"),
            boot: BootRecipe::default(),
            sheet: None,
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("xtask-capture-test-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    #[test]
    fn the_manifest_round_trips() {
        let m = CaptureManifest {
            version: 1,
            oracle: "dosbox-automation 0.85.1 (6d100)".into(),
            upstream_commit: "071a82e".into(),
            exe: "SECOND.EXE".into(),
            arguments: vec!["2".into()],
            cycles: 25000,
            width: 320,
            height: 200,
            frame_origin: "boot".into(),
            skip_boot_frames: 5000,
            boot: BootRecipe::Autoexec,
            keys: vec!["KBD_left".into(), "KBD_enter".into()],
            frames: vec![FrameRef {
                index: 100,
                idx: Some("frame_000100.idx".into()),
                rgb: "frame_000100.rgb".into(),
                palette: Some(BTreeMap::from([(1u8, [255, 255, 255]), (56, [243, 247, 255])])),
            }],
        };
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(CaptureManifest::from_json(&json).unwrap(), m);
    }

    #[test]
    fn frame_file_names_are_zero_padded_to_six_digits() {
        assert_eq!(frame_file_name(7), "frame_000007");
        assert_eq!(frame_file_name(123456), "frame_123456");
    }

    #[test]
    fn the_manifest_records_the_capture_recipe() {
        let json = serde_json::to_string(&CaptureManifest {
            version: 1,
            oracle: "dosbox-automation 0.85.1 (6d100)".into(),
            upstream_commit: "071a82e".into(),
            exe: "SECOND.EXE".into(),
            arguments: vec!["2".into()],
            cycles: 25000,
            width: 320,
            height: 400,
            frame_origin: "boot".into(),
            skip_boot_frames: 5000,
            boot: BootRecipe::Autoexec,
            keys: vec!["KBD_left".into(), "KBD_enter".into()],
            frames: vec![FrameRef {
                index: 100,
                idx: None,
                rgb: "frame_000100.rgb".into(),
                palette: None,
            }],
        })
        .unwrap();
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        // A capture is only reproducible with the cycles it was pinned to, the
        // boot recipe that reached the demo, and the key sequence that
        // answered the setup screen (D14, D17).
        assert_eq!(value["cycles"], 25000);
        assert_eq!(value["frame_origin"], "boot");
        assert_eq!(value["skip_boot_frames"], 5000);
        assert_eq!(value["boot"], "autoexec");
        assert_eq!(value["keys"][1], "KBD_enter");
        assert_eq!(value["width"], 320);
        assert_eq!(value["height"], 400);
    }

    #[test]
    fn a_manifest_written_before_the_boot_recipe_existed_means_the_typed_one() {
        // reference/check_a was captured over the API-typed boot, and its
        // manifest has no `boot` field; reading it must not invent a recipe it
        // was not taken with.
        let json = r#"{
            "version": 1,
            "oracle": "dosbox-automation 0.85.1 (6d100)",
            "upstream_commit": "071a82e",
            "exe": "SECOND.EXE",
            "arguments": ["2"],
            "cycles": 25000,
            "width": 320,
            "height": 400,
            "frame_origin": "boot",
            "skip_boot_frames": 300,
            "keys": ["KBD_left", "KBD_enter"],
            "frames": [{"index": 400, "idx": "frame_000400.idx", "rgb": "frame_000400.rgb"}]
        }"#;
        let manifest = CaptureManifest::from_json(json).unwrap();
        assert_eq!(manifest.boot, BootRecipe::Typed);
    }

    #[test]
    fn the_autoexec_recipe_runs_the_whole_boot_in_the_guest() {
        // D17's route: every line is a guest command, and AUTOTYPE must come
        // before the program because the program does not return — it is the
        // only line that can answer the setup screen the program draws.
        let cwd = Path::new("/reference/original");
        let exe = cwd.join("SECOND.EXE");
        let lines = autoexec_lines(
            cwd,
            &exe,
            &["2".to_owned()],
            &keys_from_script("KBD_left KBD_enter"),
        )
        .unwrap();
        assert_eq!(
            lines,
            vec![
                "MOUNT C -RO /reference/original".to_owned(),
                "C:".to_owned(),
                format!("AUTOTYPE -w {AUTOTYPE_WAIT_SECONDS} left enter"),
                "SECOND.EXE 2".to_owned(),
            ]
        );
    }

    #[test]
    fn a_recipe_with_no_keys_leaves_the_setup_screen_alone() {
        // AUTOTYPE with an empty button list is an error, and a capture that
        // means to show the setup screen must not have it pressed through.
        let lines = autoexec_lines(
            Path::new("/reference/original"),
            Path::new("/reference/original/SECOND.EXE"),
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            lines,
            vec![
                "MOUNT C -RO /reference/original".to_owned(),
                "C:".to_owned(),
                "SECOND.EXE".to_owned(),
            ]
        );
    }

    #[test]
    fn autotype_names_are_the_kbd_names_without_the_prefix() {
        assert_eq!(autotype_name("KBD_left"), "left");
        assert_eq!(autotype_name("KBD_kp_8"), "kp_8");
        // A name the mapper already spells is passed through rather than
        // mangled: the two namespaces only differ by the prefix.
        assert_eq!(autotype_name("left"), "left");
    }

    #[test]
    fn the_typed_recipe_gives_the_guest_nothing_to_run() {
        let opts = options(vec![5100], Path::new("/tmp/never-written"));
        assert_eq!(opts.boot, BootRecipe::Typed);
        assert!(opts.autoexec_lines().unwrap().is_empty());

        // And the autoexec recipe resolves the mount to the same directory the
        // emulator's allow-list does, which takes an absolute path.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.boot = BootRecipe::Autoexec;
        let lines = opts.autoexec_lines().unwrap();
        assert_eq!(lines[0], format!("MOUNT C -RO {}", opts.cwd.canonicalize().unwrap().display()));
        assert_eq!(lines.last().unwrap(), "SECOND.EXE 2");
    }

    #[test]
    fn the_guest_answers_the_setup_screen_under_the_autoexec_recipe() {
        // The keys travel in the emulator's config under this recipe, not over
        // the API: the capture must not wait for the screen and press them
        // again, because the guest has already answered it on the emulated
        // clock by the time boot returns (D17).
        let out = scratch_dir("autoexec-keys");
        let mut opts = options(vec![5100], &out);
        opts.boot = BootRecipe::Autoexec;
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();
        assert_eq!(manifest.boot, BootRecipe::Autoexec);
        assert!(oracle.waited_for_text.is_empty());
        assert_eq!(oracle.keys, None);
        std::fs::remove_dir_all(&out).unwrap();

        // The typed recipe still does both, which is what the contrast the
        // decision in D17 rests on actually is.
        let out = scratch_dir("typed-keys");
        let opts = options(vec![5100], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        capture(&mut oracle, &opts).unwrap();
        assert_eq!(oracle.waited_for_text, vec![SETUP_SCREEN_TEXT.to_owned()]);
        assert_eq!(oracle.keys.as_deref(), Some("KBD_left KBD_enter"));
        std::fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn a_capture_without_pinned_cycles_is_refused() {
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.cycles = 0;
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Oracle(OracleError::UnpinnedCycles))
        ));
        // And the emulator refuses to be told to pin nothing, too.
        let mut oracle = FakeOracle::new(vec![]);
        assert!(matches!(
            oracle.set_cycles(0),
            Err(OracleError::UnpinnedCycles)
        ));
    }

    #[test]
    fn indices_that_break_the_capture_contract_are_refused() {
        let opts = options(vec![], Path::new("/tmp/never-written"));
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));

        let opts = options(vec![5200, 5100], Path::new("/tmp/never-written"));
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));

        // An index inside the skipped boot means two different origins at once.
        let opts = options(vec![4999], Path::new("/tmp/never-written"));
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));
    }

    #[test]
    fn the_program_must_live_in_the_directory_that_is_mounted() {
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.exe = PathBuf::from("elsewhere/SECOND.EXE");
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));
    }

    #[test]
    fn a_trailing_slash_on_the_working_directory_still_matches() {
        // The brief's default is written `reference/original/`, so a user
        // passing that must not be told the exe lives somewhere else.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        let cwd = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/original/");
        opts.exe = cwd.join("SECOND.EXE");
        opts.cwd = cwd;
        assert!(validate_options(&opts).is_ok());
    }

    #[test]
    fn capture_writes_frames_and_a_manifest_that_names_them() {
        let out = scratch_dir("writes-frames");
        let opts = options(vec![5100, 5200], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame(), indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();

        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.oracle, "dosbox-automation test build");
        assert_eq!((manifest.width, manifest.height), (320, 200));
        assert_eq!(manifest.cycles, 25000);
        assert_eq!(manifest.exe, "SECOND.EXE");
        assert_eq!(manifest.arguments, vec!["2".to_string()]);
        assert_eq!(manifest.frame_origin, "boot");
        assert_eq!(manifest.skip_boot_frames, 5000);
        assert_eq!(manifest.keys, vec!["KBD_left", "KBD_enter"]);
        assert_eq!(manifest.frames.len(), 2);

        // The indices are boot-relative: the fake counter was stepped to the
        // skipped boot and then to each requested frame.
        assert_eq!(oracle.counter, 5200);
        assert_eq!(oracle.stepped_frames, vec![5100, 5200]);

        let on_disk = CaptureManifest::from_json(
            &std::fs::read_to_string(out.join("manifest.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(on_disk, manifest);

        for (index, frame) in manifest.frames.iter().enumerate() {
            assert_eq!(frame.index, [5100, 5200][index]);
            assert_eq!(frame.rgb, format!("{}.rgb", frame_file_name(frame.index)));
            assert_eq!(
                std::fs::read(out.join(&frame.rgb)).unwrap(),
                decode_frame(FRAME_BIN).unwrap().rgb
            );
            let idx = frame.idx.as_ref().unwrap();
            assert_eq!(std::fs::read(out.join(idx)).unwrap(), FB_BIN);
            // Only the entries the frame uses are recorded: the rest of the
            // DAC does not hold still while a demo runs.
            let palette = frame.palette.as_ref().unwrap();
            assert_eq!(palette[&1], [11, 48, 63]);
            let distinct: std::collections::BTreeSet<u8> = FB_BIN.iter().copied().collect();
            assert_eq!(palette.len(), distinct.len());
            assert!(palette.len() < 256);
        }
    }

    #[test]
    fn a_capture_is_written_as_one_sheet_a_person_can_look_at() {
        // The point of the flag: after a capture, one PNG holding what was
        // taken, in the order it was taken — B2–B5 find a part's frames by
        // looking at it, so the tiles have to be the frames themselves.
        let out = scratch_dir("sheet");
        let opts = options(vec![5100, 5200], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame(), indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();
        let sheet = out.join("sheet.png");

        write_sheet(&sheet, &out, &manifest).unwrap();

        let (width, height, rgb) = read_png(&sheet);
        // Two 320x200 tiles, one column of cells each: 2 + 2 * (320 + 2*1 + 2)
        // wide and 2 + (200 + 2*1 + 2) tall.
        assert_eq!((width, height), (650, 206));
        let frame_rgb = decode_frame(FRAME_BIN).unwrap().rgb;
        // The tiles are the frames, drawn from the files the capture wrote.
        let at = |x: u32, y: u32| {
            let at = ((y * width + x) * 3) as usize;
            [rgb[at], rgb[at + 1], rgb[at + 2]]
        };
        assert_eq!(at(3, 3), [frame_rgb[0], frame_rgb[1], frame_rgb[2]]);
        assert_eq!(at(327, 3), [frame_rgb[0], frame_rgb[1], frame_rgb[2]]);
        // And the gutter between the two cells is the sheet's background, not
        // frame pixels: two frames side by side are still two frames.
        assert_eq!(at(324, 3), [40, 40, 40]);
        std::fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn a_sheet_inside_the_mounted_directory_is_refused() {
        // reference/original holds the 1993 distribution and is read-only:
        // every frame diff in the project is measured against those bytes.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.sheet = Some(opts.cwd.join("sheet.png"));
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));

        // Beside the capture directory is where it belongs.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.sheet = Some(PathBuf::from("out/capture/sheet.png"));
        assert!(validate_options(&opts).is_ok());
    }

    #[test]
    fn a_sheet_anywhere_in_the_reference_tree_is_refused() {
        // Not only the mounted directory: `reference/` holds the original and
        // every capture, and it is input-only — `compare` refuses an `--out`
        // inside one of its captures, and a sheet written among the frames is
        // the same kind of extra file in the same kind of tree.
        for sheet in [
            "reference/sheet.png",
            "reference/check_a/sheet.png",
            "reference/parts/beglogo/sheet.png",
        ] {
            let mut opts = options(vec![5100], Path::new("out/capture"));
            opts.sheet = Some(PathBuf::from(sheet));
            assert!(
                matches!(validate_options(&opts), Err(CaptureError::Options(_))),
                "{sheet} must be refused"
            );
        }

        // And not inside the capture this run writes, for the same reason: the
        // frames and the manifest are the capture.
        let mut opts = options(vec![5100], Path::new("out/capture"));
        opts.sheet = Some(PathBuf::from("out/capture/sheet.png"));
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));

        // Outside both is where it goes.
        let mut opts = options(vec![5100], Path::new("out/capture"));
        opts.sheet = Some(PathBuf::from("out/beglogo-sheet.png"));
        assert!(validate_options(&opts).is_ok());
    }

    /// Read a PNG back: `(width, height, RGB8 pixels)`.
    fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
        let decoder = png::Decoder::new(std::fs::File::open(path).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        buffer.truncate(info.buffer_size());
        (info.width, info.height, buffer)
    }

    #[test]
    fn a_screen_that_changes_geometry_mid_capture_is_refused() {
        let out = scratch_dir("geometry-change");
        let opts = options(vec![5100, 5200], &out);
        let mut oracle = FakeOracle::new(vec![
            indexed_frame(),
            decode_frame(MODEX_FRAME_BIN).unwrap(),
        ]);
        assert!(matches!(
            capture(&mut oracle, &opts),
            Err(CaptureError::GeometryChanged {
                index: 5200,
                expected: (320, 200),
                found: (320, 400),
            })
        ));
    }

    /// An oracle that answers from memory, so the capture's own behaviour —
    /// ordering, refusals, what lands on disk — is tested without an emulator.
    struct FakeOracle {
        frames: VecDeque<CapturedFrame>,
        counter: u64,
        stepped_frames: Vec<u64>,
        keys: Option<String>,
        waited_for_text: Vec<String>,
        cycles: Option<u32>,
    }

    impl FakeOracle {
        fn new(frames: Vec<CapturedFrame>) -> FakeOracle {
            FakeOracle {
                frames: frames.into(),
                counter: 0,
                stepped_frames: Vec::new(),
                keys: None,
                waited_for_text: Vec::new(),
                cycles: None,
            }
        }
    }

    impl Oracle for FakeOracle {
        fn describe(&self) -> Result<String, OracleError> {
            Ok("dosbox-automation test build".into())
        }
        fn boot(&mut self, _exe: &Path, _cwd: &Path, _args: &[String]) -> Result<(), OracleError> {
            Ok(())
        }
        fn set_cycles(&mut self, cycles: u32) -> Result<(), OracleError> {
            if cycles == 0 {
                return Err(OracleError::UnpinnedCycles);
            }
            self.cycles = Some(cycles);
            Ok(())
        }
        fn step_frames(&mut self, frames: u32) -> Result<(), OracleError> {
            self.counter += frames as u64;
            Ok(())
        }
        fn frame(&mut self) -> Result<CapturedFrame, OracleError> {
            self.frames
                .pop_front()
                .ok_or_else(|| OracleError::Decode("no frame scripted".into()))
        }
        fn send_keys(&mut self, script: &str) -> Result<(), OracleError> {
            self.keys = Some(script.to_owned());
            Ok(())
        }
        fn shutdown(&mut self) -> Result<(), OracleError> {
            Ok(())
        }
        fn wait_for_text(&mut self, needle: &str, _timeout: Duration) -> Result<(), OracleError> {
            self.waited_for_text.push(needle.to_owned());
            Ok(())
        }
        fn frame_counter(&mut self) -> Result<u64, OracleError> {
            Ok(self.counter)
        }
        fn wait_for_frame(&mut self, index: u64) -> Result<(), OracleError> {
            if self.counter > index {
                return Err(OracleError::FrameMissed {
                    index,
                    observed: self.counter,
                });
            }
            self.stepped_frames.push(index);
            self.counter = index;
            Ok(())
        }
    }
}
