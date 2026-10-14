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
use sha2::{Digest, Sha256};

use std::time::Duration;

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

/// How long the setup screen has to disappear after the capture's keys were
/// sent or run. It is answered within a second of the keys landing (the demo's
/// first graphics frame follows 0.26 s after Enter, D14), and the keys
/// themselves are placed `AUTOTYPE_WAIT_SECONDS` into the boot, so this only
/// has to outlast the boot plus a little — it is the failure case that waits.
pub const SETUP_ANSWER_TIMEOUT: Duration = Duration::from_secs(15);

/// What a manifest this tool writes may say it is; `compare` refuses to read
/// any other version, and so does `reference verify`. `sha256` was added to
/// the frame entries without changing it: the field is additive and `serde`
/// defaults it, so a reader that predates it still reads the manifest.
pub(crate) const MANIFEST_VERSION: u32 = 1;

/// How long one frame's indexed read costs, in rendered frames — the distance
/// within which two requested indices are taken before either of them is read.
///
/// Measured (B1b fix round 1, `dosbox-automation 0.85.1`, 30000 cycles): one
/// read is 270–326 ms, of which the 768 DAC port reads plus 256 DAC index
/// writes are 260–325 ms, the framebuffer read 1.0 ms in chain-4 and 10 ms in
/// Mode X, and the identified RGB grab around 1 ms. At 70.086 Hz that is
/// 19–23 frames, so a read between two grabs loses every index within that
/// distance of the one it was read for. This is the measured cost rounded up,
/// not a budget: an index further away than this is waited for first, exactly
/// as it always was, so a capture whose frames are far apart is unchanged.
const INDEXED_READ_FRAMES: u64 = 25;

#[derive(Debug)]
pub enum CaptureError {
    /// The options cannot produce a reproducible capture. Nothing was started.
    Options(String),
    Oracle(OracleError),
    Io {
        path: PathBuf,
        message: String,
    },
    /// The emulator changed display geometry between two of the frames this
    /// capture took, so one manifest cannot honestly describe them all:
    /// capture one screen at a time. What is compared is the frames taken *in
    /// this run* — the first read sets the geometry the rest must match — so a
    /// change before the first requested index, or between two runs, is not
    /// seen here and is the caller's to avoid (D15: one capture, one screen).
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
        // Validation has already refused anything without a button; this is
        // the lookup that has to be right, because AUTOTYPE presses nothing
        // and says nothing for a name it does not know.
        let buttons: Vec<&str> = keys
            .iter()
            .map(|key| autotype_button(key).ok_or_else(|| unknown_key(key, true)))
            .collect::<Result<_, _>>()?;
        lines.push(format!(
            "AUTOTYPE -w {AUTOTYPE_WAIT_SECONDS} {}",
            buttons.join(" ")
        ));
    }
    lines.push(program);
    Ok(lines)
}

/// The refusal a key gets when it is not one the capture can press. The
/// offending name is always named, because the whole failure this guards
/// against is a silently unpressed key.
fn unknown_key(key: &str, autotype: bool) -> CaptureError {
    let known = if autotype {
        "a key the emulator's mapper has a button for"
    } else {
        "a key name the emulator's input API knows"
    };
    CaptureError::Options(format!(
        "--keys: {key:?} is not {known}; names look like \"KBD_left KBD_enter\" \
         (KBD_backspace is bspace to AUTOTYPE, KBD_kp8 is kp_8, and so on)"
    ))
}

/// The two key namespaces, paired.
///
/// `input/sequence` names a key by its `KBD_KEYS` enumerator, `AUTOTYPE` by the
/// mapper's button name, and **they are not the same word with a prefix on
/// one side**: `KBD_backspace` is `bspace`, every keypad key differs
/// (`KBD_kp8` is `kp_8`, `KBD_kpenter` is `kp_enter`), and so do the
/// modifiers and brackets (`KBD_leftshift` is `lshift`, `KBD_leftctrl` is
/// `lctrl`, `KBD_leftbracket` is `lbracket`). `AUTOTYPE` does not validate a
/// name either — an unknown one only reaches `LOG_ERR` in the emulator's log
/// (`gui/mapper.cpp:209-222`), which the capture deletes — so a derivation
/// that is merely plausible would answer the setup screen with nothing and
/// write a capture of the wrong screen, at exit 0.
///
/// This table is the pairing, taken from the emulator's own tables rather than
/// derived from either spelling: the mapper's button layout and its `KeyBlock`
/// tables (`gui/mapper.cpp`, `CreateLayout`) and the API's key list
/// (`webserver/input.cpp`, `key_name_map`), at `dosbox-automation 0.85.1`.
/// `KBD_NONE` is not in it: the API accepts it as a key that presses nothing,
/// and there is no button to press, so a capture cannot use it.
pub const KEY_NAMES: &[(&str, &str)] = &[
    ("KBD_0", "0"),
    ("KBD_1", "1"),
    ("KBD_2", "2"),
    ("KBD_3", "3"),
    ("KBD_4", "4"),
    ("KBD_5", "5"),
    ("KBD_6", "6"),
    ("KBD_7", "7"),
    ("KBD_8", "8"),
    ("KBD_9", "9"),
    ("KBD_a", "a"),
    ("KBD_abnt1", "abnt1"),
    ("KBD_b", "b"),
    ("KBD_backslash", "backslash"),
    ("KBD_backspace", "bspace"),
    ("KBD_c", "c"),
    ("KBD_capslock", "capslock"),
    ("KBD_comma", "comma"),
    ("KBD_d", "d"),
    ("KBD_delete", "delete"),
    ("KBD_down", "down"),
    ("KBD_e", "e"),
    ("KBD_end", "end"),
    ("KBD_enter", "enter"),
    ("KBD_equals", "equals"),
    ("KBD_esc", "esc"),
    ("KBD_f", "f"),
    ("KBD_f1", "f1"),
    ("KBD_f10", "f10"),
    ("KBD_f11", "f11"),
    ("KBD_f12", "f12"),
    ("KBD_f2", "f2"),
    ("KBD_f3", "f3"),
    ("KBD_f4", "f4"),
    ("KBD_f5", "f5"),
    ("KBD_f6", "f6"),
    ("KBD_f7", "f7"),
    ("KBD_f8", "f8"),
    ("KBD_f9", "f9"),
    ("KBD_g", "g"),
    ("KBD_grave", "grave"),
    ("KBD_h", "h"),
    ("KBD_home", "home"),
    ("KBD_i", "i"),
    ("KBD_insert", "insert"),
    ("KBD_j", "j"),
    ("KBD_k", "k"),
    ("KBD_kp0", "kp_0"),
    ("KBD_kp1", "kp_1"),
    ("KBD_kp2", "kp_2"),
    ("KBD_kp3", "kp_3"),
    ("KBD_kp4", "kp_4"),
    ("KBD_kp5", "kp_5"),
    ("KBD_kp6", "kp_6"),
    ("KBD_kp7", "kp_7"),
    ("KBD_kp8", "kp_8"),
    ("KBD_kp9", "kp_9"),
    ("KBD_kpdivide", "kp_divide"),
    ("KBD_kpenter", "kp_enter"),
    ("KBD_kpminus", "kp_minus"),
    ("KBD_kpmultiply", "kp_multiply"),
    ("KBD_kpperiod", "kp_period"),
    ("KBD_kpplus", "kp_plus"),
    ("KBD_l", "l"),
    ("KBD_left", "left"),
    ("KBD_leftalt", "lalt"),
    ("KBD_leftbracket", "lbracket"),
    ("KBD_leftctrl", "lctrl"),
    ("KBD_leftgui", "lgui"),
    ("KBD_leftshift", "lshift"),
    ("KBD_m", "m"),
    ("KBD_minus", "minus"),
    ("KBD_n", "n"),
    ("KBD_numlock", "numlock"),
    ("KBD_o", "o"),
    ("KBD_oem102", "oem102"),
    ("KBD_p", "p"),
    ("KBD_pagedown", "pagedown"),
    ("KBD_pageup", "pageup"),
    ("KBD_pause", "pause"),
    ("KBD_period", "period"),
    ("KBD_printscreen", "printscreen"),
    ("KBD_q", "q"),
    ("KBD_quote", "quote"),
    ("KBD_r", "r"),
    ("KBD_right", "right"),
    ("KBD_rightalt", "ralt"),
    ("KBD_rightbracket", "rbracket"),
    ("KBD_rightctrl", "rctrl"),
    ("KBD_rightgui", "rgui"),
    ("KBD_rightshift", "rshift"),
    ("KBD_s", "s"),
    ("KBD_scrolllock", "scrolllock"),
    ("KBD_semicolon", "semicolon"),
    ("KBD_slash", "slash"),
    ("KBD_space", "space"),
    ("KBD_t", "t"),
    ("KBD_tab", "tab"),
    ("KBD_u", "u"),
    ("KBD_up", "up"),
    ("KBD_v", "v"),
    ("KBD_w", "w"),
    ("KBD_x", "x"),
    ("KBD_y", "y"),
    ("KBD_z", "z"),
];

/// Whether `input/sequence` would accept `key`, as far as the table knows.
pub fn is_sequence_key(key: &str) -> bool {
    KEY_NAMES.iter().any(|(api, _)| *api == key)
}

/// The `AUTOTYPE` button for a `KBD_*` name, or `None` when the emulator's
/// mapper has no button for it (or the name is not one at all).
pub fn autotype_button(key: &str) -> Option<&'static str> {
    KEY_NAMES
        .iter()
        .find(|(api, _)| *api == key)
        .map(|(_, button)| *button)
}

/// `frame_000100` — six digits, so a directory listing sorts by frame.
pub fn frame_file_name(index: u64) -> String {
    format!("frame_{index:06}")
}

/// A digest in the form `sha256sum` prints and `reference-indices.md` records:
/// lowercase hex, 64 digits.
///
/// The hashing is in-process (`sha2`, the crate `assets verify --tree` already
/// digests with) rather than a `sha256sum` child: a capture must produce its
/// manifest on a machine without coreutils, and a digest written by a different
/// program than the one the record was measured with is a digest that can
/// disagree with it.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
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
    /// The `sha256` of the bytes of `rgb`, written where the file is written.
    ///
    /// The digest is recorded here rather than in `docs/fidelity/reference-indices.md`
    /// because the document cannot be checked mechanically (Phase 1's close-out
    /// measured it: two naming shapes, and 24 of the tree's 105 `frame_*` files
    /// with no row by name), while a manifest sits in the capture directory and
    /// says which file its digest is of. `Option` because a manifest filed
    /// before the field existed has none — [`crate::verify::backfill`] fills
    /// those in once, and [`crate::verify::command`] reports a frame that still
    /// has none rather than passing over it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// The `sha256` of the bytes of `idx`, when the frame has indices.
    ///
    /// `compare` prefers the indexed side whenever both sides have it (it is
    /// stricter than widened RGB), so an index plane whose bytes changed is a
    /// change to what every diff is measured against — and the RGB digest
    /// alone would not see it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub idx_sha256: Option<String>,
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
    validate_recipe(
        &options.exe,
        &options.cwd,
        options.cycles,
        &options.keys,
        options.boot,
    )?;
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

/// The checks every command that drives the emulated original makes before it
/// starts one: the recipe half of [`validate_options`], shared with
/// `xtask music trace`, whose trace's indices mean what a capture's indices
/// mean only under the same recipe (D17).
///
/// Every key is checked here, before anything starts, in both recipes: a name
/// `AUTOTYPE` does not know is not an error it reports — it logs and presses
/// nothing — so a command would answer the setup screen with nothing and write
/// the wrong screen at exit 0 (D17).
pub fn validate_recipe(
    exe: &Path,
    cwd: &Path,
    cycles: u32,
    keys: &[String],
    boot: BootRecipe,
) -> Result<(), CaptureError> {
    if cycles == 0 {
        return Err(CaptureError::Oracle(OracleError::UnpinnedCycles));
    }
    for key in keys {
        if !is_sequence_key(key) {
            return Err(unknown_key(key, false));
        }
        if boot == BootRecipe::Autoexec && autotype_button(key).is_none() {
            return Err(unknown_key(key, true));
        }
    }
    if !cwd.is_dir() {
        return Err(CaptureError::Options(format!(
            "{} is not a directory to mount",
            cwd.display()
        )));
    }
    if exe.parent() != Some(cwd) {
        return Err(CaptureError::Options(format!(
            "--exe {} must live in --cwd {}: the guest runs it from the mounted directory, \
             because it loads its data files from the working directory",
            exe.display(),
            cwd.display()
        )));
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
    let answering = !options.keys.is_empty();
    if options.boot == BootRecipe::Typed && answering {
        // The setup screen has to be answered before the demo draws anything.
        // Under the `Autoexec` recipe the guest answers it itself, from the
        // `[autoexec]` section this run was started with (D17).
        oracle.wait_for_text(SETUP_SCREEN_TEXT, DEFAULT_WAIT)?;
        oracle.send_keys(&options.keys.join(" "))?;
    }
    if answering {
        // Keys that leave the setup screen up have not answered it: the
        // capture would be of the menu while the manifest said the demo. The
        // guest is the judge, not the key names — `KBD_kp8` is a real key and
        // a real button, and it answers nothing on this screen — so a run
        // whose keys do not clear it is refused, naming them (D17).
        if let Err(error) = oracle.wait_for_graphics(SETUP_ANSWER_TIMEOUT) {
            let detail = match error {
                OracleError::Timeout(message) => message,
                other => other.to_string(),
            };
            return Err(CaptureError::Oracle(OracleError::Timeout(format!(
                "the keys {:?} did not start the demo: {detail}",
                options.keys.join(" ")
            ))));
        }
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
    // Indices taken but not yet read. Requested frames closer together than one
    // frame's indexed read are all taken before any of them is read: the read
    // costs INDEXED_READ_FRAMES rendered frames, so reading one frame's indices
    // before taking the next loses the next index — which is what made
    // `--frames 2449,2450,2451` uncapturable and what kept a reference from
    // carrying the neighbouring frames the ±1 alignment search needs (D18).
    let mut taken: Vec<u64> = Vec::new();
    for (position, &index) in options.frames.iter().enumerate() {
        // Wait for this frame. The oracle refuses a capture the emulator has
        // clearly run past (see `Oracle::wait_for_frame` for how close it can
        // come, and D15), because a frame taken later would be written under
        // this one's name.
        oracle.wait_for_frame(index)?;
        taken.push(index);

        let next_in_reach = options
            .frames
            .get(position + 1)
            .is_some_and(|next| next.saturating_sub(index) < INDEXED_READ_FRAMES);
        if next_in_reach {
            continue;
        }

        // Every frame in this run has been taken; read and write them now, in
        // the order they were asked for.
        for index in taken.drain(..) {
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

    // The three fields the index plane contributes, so that a frame the
    // coherence check refused them for has neither the file nor its digest:
    // a digest must never name a file the manifest does not (`verify` reads
    // `idx` as the list of the files a frame has).
    let (mut idx, mut palette, mut idx_sha256) = (None, None, None);
    if let (Some(indexed), Some(entries)) = (&frame.indexed, &frame.palette) {
        let file = format!("{name}.idx");
        std::fs::write(out.join(&file), indexed).map_err(|e| CaptureError::Io {
            path: out.join(&file),
            message: e.to_string(),
        })?;
        idx = Some(file);
        palette = Some(used_palette(indexed, entries));
        idx_sha256 = Some(sha256_hex(indexed));
    }

    Ok(FrameRef {
        index,
        idx,
        rgb,
        palette,
        // Digested from the bytes that were just written, so the record and
        // the file cannot be about different moments.
        sha256: Some(sha256_hex(&frame.rgb)),
        idx_sha256,
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

    /// `sha256sum` of the two files `write_frame` writes for the fixtures'
    /// 320×200 indexed frame: `frame_000430.rgb` and `frame_000430.idx` (the
    /// index plane is `fb.bin`). Taken with the shell command the reference
    /// ledger was measured with, so the tool's digests are pinned to coreutils
    /// rather than to each other.
    const SAMPLE_RGB_SHA256: &str = "b7b9c97ef87d182265f9db77817f0ccdb6f6151a387d14b96f1edc6232a983c5";
    const SAMPLE_IDX_SHA256: &str = "d3d93da2885e73108be903779c78d8ffcabd8843faa47807ada4331c61e8661b";

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

    /// A manifest as `capture` writes one, for the tests that are about the
    /// record rather than about an emulator. The frame's digests are
    /// placeholders: what a real one is, [`write_frame`] decides, and
    /// `a_captured_frame_carries_its_digest` is the test that says so.
    fn sample_manifest() -> CaptureManifest {
        CaptureManifest {
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
                sha256: Some("0".repeat(64)),
                idx_sha256: Some("1".repeat(64)),
            }],
        }
    }

    #[test]
    fn the_manifest_round_trips() {
        let m = sample_manifest();
        let json = serde_json::to_string(&m).unwrap();
        assert_eq!(CaptureManifest::from_json(&json).unwrap(), m);
    }

    #[test]
    fn a_captured_frame_carries_its_digest() {
        // The digest is written where the file is, and of the bytes that were
        // written — `write_frame`'s return value *is* the manifest entry — so
        // a capture directory can be checked against its own record later
        // without a second oracle run and without the prose ledger.
        let out = scratch_dir("digest");
        std::fs::create_dir_all(&out).unwrap();
        let frame = indexed_frame();
        let entry = write_frame(&out, 430, &frame).unwrap();

        let rgb = std::fs::read(out.join(&entry.rgb)).unwrap();
        let idx = std::fs::read(out.join(entry.idx.as_deref().unwrap())).unwrap();
        assert_eq!(rgb, frame.rgb, "the rgb file is the frame that was written");
        assert_eq!(idx, frame.indexed.clone().unwrap());

        // The digests are `sha256sum`'s, checked against the command rather
        // than against this module's own arithmetic: these two constants were
        // taken with `sha256sum` on the files this test just produced.
        assert_eq!(entry.sha256.as_deref(), Some(SAMPLE_RGB_SHA256));
        assert_eq!(entry.idx_sha256.as_deref(), Some(SAMPLE_IDX_SHA256));
        assert_eq!(sha256_hex(&rgb), SAMPLE_RGB_SHA256);
        assert_eq!(sha256_hex(&idx), SAMPLE_IDX_SHA256);

        // And the manifest's JSON carries them where a reader looks.
        let manifest = CaptureManifest {
            frames: vec![entry],
            ..sample_manifest()
        };
        let value = serde_json::to_value(manifest).unwrap();
        assert_eq!(value["frames"][0]["sha256"].as_str().unwrap().len(), 64);
        assert_eq!(value["frames"][0]["idx_sha256"].as_str().unwrap().len(), 64);
        assert_eq!(value["frames"][0]["sha256"], SAMPLE_RGB_SHA256);
    }

    #[test]
    fn a_frame_the_coherence_check_refused_records_no_index_digest() {
        // No `.idx` file is written, so no digest may name one: the manifest
        // must not record a file it does not have.
        let out = scratch_dir("digest-rgb-only");
        std::fs::create_dir_all(&out).unwrap();
        let frame = CapturedFrame {
            indexed: None,
            palette: None,
            ..indexed_frame()
        };
        let entry = write_frame(&out, 900, &frame).unwrap();
        assert!(entry.idx.is_none());
        assert!(entry.idx_sha256.is_none());
        assert!(entry.sha256.is_some());
        let value = serde_json::to_value(&entry).unwrap();
        assert!(value.get("idx_sha256").is_none(), "{value}");
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
                sha256: None,
                idx_sha256: None,
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
    fn autotype_buttons_are_the_mappers_names_not_the_kbd_names_less_a_prefix() {
        // The pairs where the two namespaces differ are the point: a capture
        // that pressed `p_8` or `backspace` would answer the setup screen with
        // nothing, silently (D17).
        assert_eq!(autotype_button("KBD_left"), Some("left"));
        assert_eq!(autotype_button("KBD_enter"), Some("enter"));
        assert_eq!(autotype_button("KBD_backspace"), Some("bspace"));
        assert_eq!(autotype_button("KBD_kp8"), Some("kp_8"));
        assert_eq!(autotype_button("KBD_kpenter"), Some("kp_enter"));
        assert_eq!(autotype_button("KBD_leftshift"), Some("lshift"));
        assert_eq!(autotype_button("KBD_leftctrl"), Some("lctrl"));
        assert_eq!(autotype_button("KBD_leftalt"), Some("lalt"));
        assert_eq!(autotype_button("KBD_leftgui"), Some("lgui"));
        assert_eq!(autotype_button("KBD_leftbracket"), Some("lbracket"));
        assert_eq!(autotype_button("KBD_rightbracket"), Some("rbracket"));

        // Names that are not the API's at all have no button, including the
        // two spellings the old prefix rule invented.
        assert_eq!(autotype_button("KBD_kp_8"), None);
        assert_eq!(autotype_button("left"), None);
        assert_eq!(autotype_button("KBD_nope"), None);
        assert_eq!(autotype_button("KBD_NONE"), None);
        assert!(!is_sequence_key("KBD_kp_8"));
        assert!(!is_sequence_key("KBD_NONE"));
        assert!(is_sequence_key("KBD_left"));
    }

    #[test]
    fn a_key_the_emulator_does_not_know_is_refused_before_the_capture_starts() {
        // The reviewer's reproduction: `--keys "KBD_kp8 KBD_enter"` used to
        // answer the setup screen with nothing (AUTOTYPE has no `kp8`) and
        // write the 720x400 text screen at exit 0. Now the name alone is
        // enough to refuse, in both recipes, before an emulator is spawned.
        for key in ["KBD_kp_8", "KBD_nope", "left", "KBD_NONE"] {
            let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
            opts.keys = vec![key.to_owned()];
            for boot in [BootRecipe::Typed, BootRecipe::Autoexec] {
                opts.boot = boot;
                match validate_options(&opts) {
                    Err(CaptureError::Options(message)) => {
                        assert!(
                            message.contains(key),
                            "the refusal must name {key}: {message}"
                        );
                    }
                    other => panic!("{key} under {boot:?}: expected a refusal, got {other:?}"),
                }
            }
        }

        // The keys the reference manifests record still pass, and so does the
        // whole table.
        for boot in [BootRecipe::Typed, BootRecipe::Autoexec] {
            let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
            opts.boot = boot;
            assert!(validate_options(&opts).is_ok());
        }
        for (api, _) in KEY_NAMES {
            let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
            opts.boot = BootRecipe::Autoexec;
            opts.keys = vec![(*api).to_owned()];
            assert!(validate_options(&opts).is_ok(), "{api} must be usable");
        }
    }

    #[test]
    fn the_key_table_is_sorted_and_unique() {
        // It is read by eye against the emulator's own tables; a duplicate or
        // a stray blank line is how a missing key hides.
        for pair in KEY_NAMES.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
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
    fn keys_that_do_not_answer_the_setup_screen_refuse_the_capture() {
        // The reviewer's reproduction, at the oracle boundary: the keys are
        // sent, the setup screen stays up, and the run must not quietly write
        // the menu as if it were the demo. `KBD_kp8` is a real key and a real
        // button — it simply answers nothing on that screen — so no key-name
        // check can catch this; the guest has to.
        let out = scratch_dir("keys-that-answer-nothing");
        let opts = options(vec![5100], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        oracle.setup_screen_clears = false;

        let error = capture(&mut oracle, &opts).unwrap_err();
        match error {
            CaptureError::Oracle(OracleError::Timeout(message)) => {
                assert!(
                    message.contains("KBD_left") && message.contains("did not start the demo"),
                    "the refusal must name the keys and what did not happen: {message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        // Nothing was written: the frames would have been of the menu.
        assert!(!out.join("manifest.json").exists());

        // And when the screen does clear, the capture goes through and waits
        // for exactly that.
        let out = scratch_dir("keys-that-answer");
        let opts = options(vec![5100], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        capture(&mut oracle, &opts).unwrap();
        assert_eq!(
            oracle.waited_for_graphics,
            vec!["the demo's first graphics frame".to_owned()]
        );
        assert!(out.join("manifest.json").is_file());
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
    fn indices_within_a_read_of_each_other_are_all_taken_before_any_is_read() {
        // A frame's indexed read costs about twenty-five rendered frames (D15
        // and the constant above), so reading the frames in turn loses every
        // index within a read of the one before — which is what made a capture
        // carrying a keyframe's neighbours impossible, and what left the ±1
        // alignment search (D18) without a neighbouring frame to find.
        let out = scratch_dir("adjacent-indices");
        let opts = options(vec![5100, 5101, 5102], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame(), indexed_frame(), indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();

        assert_eq!(
            oracle.events,
            vec!["take 5100", "take 5101", "take 5102", "read", "read", "read"]
        );
        assert_eq!(
            manifest.frames.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![5100, 5101, 5102]
        );
    }

    #[test]
    fn indices_further_apart_than_a_read_are_still_taken_and_read_in_turn() {
        // Far-apart keyframes are what every capture in the tree holds today,
        // and their behaviour does not change: taking the next one before
        // reading this one would delay the read by the whole gap, and on a
        // screen that changes slowly the indices would be stale by then.
        let out = scratch_dir("sparse-indices");
        let opts = options(vec![5100, 5200], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame(), indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();

        assert_eq!(oracle.events, vec!["take 5100", "read", "take 5200", "read"]);
        assert_eq!(
            manifest.frames.iter().map(|f| f.index).collect::<Vec<_>>(),
            vec![5100, 5200]
        );
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
        /// Every take and every read, in the order the capture made them: what
        /// a test needs to see that a read did not sit between two takes.
        events: Vec<String>,
        keys: Option<String>,
        waited_for_text: Vec<String>,
        waited_for_graphics: Vec<String>,
        /// Whether the setup screen goes away when the keys are sent, as the
        /// real one does when they answer it.
        setup_screen_clears: bool,
        cycles: Option<u32>,
    }

    impl FakeOracle {
        fn new(frames: Vec<CapturedFrame>) -> FakeOracle {
            FakeOracle {
                frames: frames.into(),
                counter: 0,
                stepped_frames: Vec::new(),
                events: Vec::new(),
                keys: None,
                waited_for_text: Vec::new(),
                waited_for_graphics: Vec::new(),
                setup_screen_clears: true,
                cycles: None,
            }
        }
    }

    impl Oracle for FakeOracle {
        fn read_memory(
            &self,
            _segment: u16,
            _offset: u32,
            _len: usize,
        ) -> Result<Vec<u8>, OracleError> {
            Err(OracleError::Transport(
                "the fake oracle holds no guest memory".into(),
            ))
        }
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
            self.events.push("read".to_owned());
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
        fn wait_for_graphics(&mut self, _timeout: Duration) -> Result<(), OracleError> {
            self.waited_for_graphics
                .push("the demo's first graphics frame".to_owned());
            if self.setup_screen_clears {
                Ok(())
            } else {
                Err(OracleError::Timeout(
                    "the screen is still text after 15s".to_owned(),
                ))
            }
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
            self.events.push(format!("take {index}"));
            self.stepped_frames.push(index);
            self.counter = index;
            Ok(())
        }
    }
}
