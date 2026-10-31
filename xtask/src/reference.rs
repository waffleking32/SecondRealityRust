//! `xtask reference capture`: drive the emulated original, write keyframes under `reference/`,
//! and record what produced them. The manifest says exactly which frame each image is (the
//! index's origin, the arguments, the keys, the cycles, the boot), and a capture that cannot be
//! reproduced is refused instead of written.

use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use std::time::Duration;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::oracle::{CapturedFrame, Oracle, OracleError, DEFAULT_WAIT, FRAME_ORIGIN};

/// The pinned commit of `upstream/SecondReality` the port follows
/// (`.gitmodules`; `docs/decisions.md` D13).
pub const UPSTREAM_COMMIT: &str = "071a82e";

/// The reference tree: the mounted original (`reference/original`) and every capture. It is
/// input-only, so `compare --out` and `--sheet` inside it are refused.
pub const REFERENCE_ROOT: &str = "reference";

/// The original's sound card setup screen. `SECOND.EXE` shows it whatever the
/// block argument, and the default choice fails on this emulator, so the
/// capture waits for this text before sending `--keys` (D14).
pub const SETUP_SCREEN_TEXT: &str = "Soundcard";

/// How long `AUTOTYPE` waits before its first key, in seconds. It only has to be after the
/// setup screen is up (boot frame 16, 0.23 s, under `--boot autoexec`); the steps are on the
/// emulated clock, so a longer wait costs wall-clock time, not determinism (D17).
pub const AUTOTYPE_WAIT_SECONDS: f64 = 5.0;

/// How long the setup screen may take to go once the keys were sent or typed: the boot, the
/// `AUTOTYPE` wait and the 0.26 s from Enter to the first graphics frame (D14), with margin.
pub const SETUP_ANSWER_TIMEOUT: Duration = Duration::from_secs(15);

/// The manifest version `compare` and `reference verify` read. Additive fields (the digests,
/// `boot`, `boot_wait`) do not change it: `serde` defaults them.
pub(crate) const MANIFEST_VERSION: u32 = 1;

/// How long one frame's indexed read costs, in rendered frames: requested indices closer than
/// this are all taken before any is read (D19). Measured at `dosbox-automation 0.85.1`, 30000
/// cycles: 270–326 ms a read (the DAC's 768 port reads and 256 index writes are 260–325 ms of
/// it), 19–23 frames at 70.086 Hz, rounded up.
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
    /// The display changed geometry between two frames of this run, so one manifest cannot
    /// describe them all (D15: one capture, one screen).
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

/// How the guest is taken from the emulator's boot to the demo; part of the recipe, because the
/// same boot index is a different screen under another (D17).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum BootRecipe {
    /// The host types `MOUNT`, `C:` and the program line over the API and answers the setup
    /// screen with a key sequence anchored to when it posted it. Two runs put the same screen
    /// 43-45 frames apart, because every step waits on the host (D15).
    #[default]
    Typed,
    /// The emulator's shell runs the recipe from its `[autoexec]` section and `AUTOTYPE`
    /// answers the setup screen on the PIC clock, so the same boot frame is the same screen in
    /// every run (D17).
    Autoexec,
}

/// What the `[autoexec]` boot waits for before the capture starts: the demo's setup screen,
/// or the first graphics frame for a program that never draws it (the vault's `DIS.EXE` and
/// effect batch, D39). Recorded, because the same boot index is a different screen under a
/// different wait.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, clap::ValueEnum,
)]
#[serde(rename_all = "lowercase")]
pub enum BootWait {
    /// [`SETUP_SCREEN_TEXT`]: the demo's anchor.
    #[default]
    Setup,
    /// The first graphics frame.
    Graphics,
}

impl BootWait {
    /// The word serde and clap use, so a refusal names the flag as it is typed.
    pub fn name(self) -> &'static str {
        match self {
            BootWait::Setup => "setup",
            BootWait::Graphics => "graphics",
        }
    }
}

/// How the guest is taken to the demo: the program, the directory it runs from, the pinned
/// cycles, the keys that answer the setup screen and the boot (D17). `reference capture` and
/// `music trace` share it, because their indices mean the same thing only under the same
/// recipe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    /// The program to run in the guest. `SECOND.EXE` loads `REALITY.FC` from its working
    /// directory, so it has to live in `cwd`.
    pub exe: PathBuf,
    pub cwd: PathBuf,
    pub arguments: Vec<String>,
    pub cycles: u32,
    /// `KBD_*` key names sent once the setup screen appears; under [`BootRecipe::Autoexec`]
    /// they become `AUTOTYPE` buttons ([`autotype_button`]).
    pub keys: Vec<String>,
    pub boot: BootRecipe,
    /// What the autoexec boot waits for. Only the autoexec recipe can choose.
    pub boot_wait: BootWait,
}

impl Recipe {
    /// Refuse a recipe that cannot be reproduced, before anything starts. Every key is checked
    /// in both recipes: `AUTOTYPE` presses nothing for a name it does not know and says so only
    /// in the emulator's log, so a bad key would capture the setup screen at exit 0 (D17).
    pub fn validate(&self) -> Result<(), CaptureError> {
        if self.cycles == 0 {
            return Err(CaptureError::Oracle(OracleError::UnpinnedCycles));
        }
        for key in &self.keys {
            if !is_sequence_key(key) {
                return Err(unknown_key(key, false));
            }
            if self.boot == BootRecipe::Autoexec && autotype_button(key).is_none() {
                return Err(unknown_key(key, true));
            }
        }
        if !self.cwd.is_dir() {
            return Err(CaptureError::Options(format!(
                "{} is not a directory to mount",
                self.cwd.display()
            )));
        }
        if self.exe.parent() != Some(self.cwd.as_path()) {
            return Err(CaptureError::Options(format!(
                "--exe {} must live in --cwd {}: the guest runs it from the mounted directory, \
                 because it loads its data files from the working directory",
                self.exe.display(),
                self.cwd.display()
            )));
        }
        if self.boot_wait != BootWait::Setup && self.boot != BootRecipe::Autoexec {
            return Err(CaptureError::Options(format!(
                "--boot-wait {} needs --boot autoexec: the typed recipe waits for the setup screen \
                 itself, because that is the screen its keys answer",
                self.boot_wait.name()
            )));
        }
        Ok(())
    }

    /// The `[autoexec]` lines the guest's shell runs, or none for the typed recipe. The mount
    /// names the absolute directory, because the guest resolves a relative host path against
    /// the emulator's working directory.
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

    /// Pin the cycles, set the anchor `boot` waits for, boot, and under the typed recipe
    /// answer the setup screen over the API. Under the autoexec recipe the guest answers it.
    pub fn boot(&self, oracle: &mut dyn Oracle) -> Result<(), OracleError> {
        oracle.set_cycles(self.cycles)?;
        oracle.set_boot_wait(self.boot_wait)?;
        oracle.boot(&self.exe, &self.cwd, &self.arguments)?;
        if self.boot == BootRecipe::Typed && !self.keys.is_empty() {
            oracle.wait_for_text(SETUP_SCREEN_TEXT, DEFAULT_WAIT)?;
            oracle.send_keys(&self.keys.join(" "))?;
        }
        Ok(())
    }

    /// When keys were sent, that they started the demo: the guest is the judge, not the key
    /// names (`KBD_kp8` is a real key that answers nothing there), and a screen still in text
    /// is the menu (D17). The refusal names the keys.
    pub fn confirm_answered(&self, oracle: &mut dyn Oracle) -> Result<(), String> {
        if self.keys.is_empty() {
            return Ok(());
        }
        oracle.wait_for_graphics(SETUP_ANSWER_TIMEOUT).map_err(|error| {
            let detail = match error {
                OracleError::Timeout(message) => message,
                other => other.to_string(),
            };
            format!("the keys {:?} did not start the demo: {detail}", self.keys.join(" "))
        })
    }

    /// The program as the manifest records it.
    pub fn exe_name(&self) -> String {
        self.exe
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default()
    }
}

/// Everything a capture needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptureOptions {
    pub recipe: Recipe,
    /// Boot-relative frame indices: counted from the emulator's first rendered frame (D14).
    pub frames: Vec<u64>,
    pub out: PathBuf,
    /// Frames skipped from that same origin before the capture begins.
    pub skip_boot_frames: u64,
    /// Where to write one PNG of the frames, to look at; not part of the reference.
    pub sheet: Option<PathBuf>,
}

/// `--keys "KBD_left KBD_enter"`: whitespace-separated key names, in order.
pub fn keys_from_script(script: &str) -> Vec<String> {
    script.split_whitespace().map(str::to_owned).collect()
}

/// The recipe the guest runs itself: mount the reference read-only, `AUTOTYPE` the
/// setup-screen answer, run the program (D17). `AUTOTYPE` comes first because the program does
/// not return: it schedules its presses on the PIC clock and exits, and the shell runs the demo
/// underneath them.
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

/// The refusal a key gets when the capture cannot press it, naming it.
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

/// The two key namespaces, paired: `input/sequence`'s `KBD_*` enumerators and the mapper's
/// `AUTOTYPE` button names, which are not the same word with a prefix (`KBD_backspace` is
/// `bspace`, `KBD_kp8` is `kp_8`, `KBD_leftshift` is `lshift`). `AUTOTYPE` only logs a name it
/// does not know (`gui/mapper.cpp:209-222`), so the pairing is taken from the emulator's own
/// tables (`gui/mapper.cpp`'s `CreateLayout`, `webserver/input.cpp`'s `key_name_map`,
/// `dosbox-automation 0.85.1`). `KBD_NONE` has no button and is left out.
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

/// Whether `input/sequence` accepts `key`.
pub fn is_sequence_key(key: &str) -> bool {
    KEY_NAMES.iter().any(|(api, _)| *api == key)
}

/// The `AUTOTYPE` button for a `KBD_*` name.
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

/// A digest as `sha256sum` prints it: lowercase hex, 64 digits.
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
    /// The palette entries this frame's indices use, six bits per component. Only those: the
    /// rest of the DAC is rewritten while the demo runs, and a snapshot of it does not repeat.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub palette: Option<BTreeMap<u8, [u8; 3]>>,
    /// The `sha256` of `rgb`'s bytes (D23). Absent in older manifests, which
    /// [`crate::verify::command`] reports as a finding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    /// The `sha256` of `idx`'s bytes: `compare` prefers the indexed side, so the RGB digest
    /// alone would not see a changed index plane.
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
    /// What `index` counts from: `"boot"`, the emulator's first rendered frame.
    pub frame_origin: String,
    pub skip_boot_frames: u64,
    /// Absent in older manifests, which were all taken with the typed recipe.
    #[serde(default)]
    pub boot: BootRecipe,
    /// Absent in older manifests, which all waited for the setup screen.
    #[serde(default)]
    pub boot_wait: BootWait,
    pub keys: Vec<String>,
    pub frames: Vec<FrameRef>,
}

/// Refuse a capture that cannot be reproduced, before touching the emulator.
pub fn validate_options(options: &CaptureOptions) -> Result<(), CaptureError> {
    options.recipe.validate()?;
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
    if let Some(sheet) = &options.sheet {
        crate::paths::refuse_output_inside(
            sheet,
            &options.recipe.cwd,
            "--sheet",
            "that directory is the mounted original (SECOND.EXE and REALITY.FC), and every frame \
             diff in the project is measured against those exact bytes",
            "Pass --sheet outside the reference, e.g. out/<name>-sheet.png",
        )
        .map_err(CaptureError::Options)?;
        crate::paths::refuse_output_inside(
            sheet,
            Path::new(REFERENCE_ROOT),
            "--sheet",
            "the reference tree holds the mounted original and every capture, and a file written \
             among them is an extra file in the tree every later diff is measured against",
            &format!("Pass --sheet outside {REFERENCE_ROOT}/, e.g. out/<name>-sheet.png"),
        )
        .map_err(CaptureError::Options)?;
        crate::paths::refuse_output_inside(
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
    options.recipe.boot(oracle)?;
    options
        .recipe
        .confirm_answered(oracle)
        .map_err(|message| CaptureError::Oracle(OracleError::Timeout(message)))?;

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
    // Indices taken but not yet read: frames closer than one indexed read are all taken
    // before any is read, or reading one loses the next (D18, D19).
    let mut taken: Vec<u64> = Vec::new();
    for (position, &index) in options.frames.iter().enumerate() {
        oracle.wait_for_frame(index)?;
        taken.push(index);

        let next_in_reach = options
            .frames
            .get(position + 1)
            .is_some_and(|next| next.saturating_sub(index) < INDEXED_READ_FRAMES);
        if next_in_reach {
            continue;
        }

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
        exe: options.recipe.exe_name(),
        arguments: options.recipe.arguments.clone(),
        cycles: options.recipe.cycles,
        width,
        height,
        frame_origin: FRAME_ORIGIN.to_owned(),
        skip_boot_frames: options.skip_boot_frames,
        boot: options.recipe.boot,
        boot_wait: options.recipe.boot_wait,
        keys: options.recipe.keys.clone(),
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

    // A frame the coherence check refused indices for has neither the `.idx` file nor its
    // digest: `verify` reads `idx` as the list of a frame's files.
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
        sha256: Some(sha256_hex(&frame.rgb)),
        idx_sha256,
    })
}

/// One PNG of a capture's frames, in [`crate::sheet`]'s layout, drawn from the files the
/// capture wrote rather than from memory, so it cannot show something that was not written.
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
            label: format!("frame {} ({})", frame.index, frame.rgb),
        });
    }
    crate::sheet::write(sheet, &tiles).map_err(|e| CaptureError::Io {
        path: sheet.to_path_buf(),
        message: e.to_string(),
    })
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
    use crate::oracle::fixtures::{recorded_palette, FB_BIN, FRAME_BIN, MODEX_FRAME};
    use std::collections::VecDeque;
    use std::time::Duration;

    /// `sha256sum` of the two files `write_frame` writes for the fixtures' 320×200 indexed
    /// frame (`frame_000430.rgb`, and `frame_000430.idx`, which is `fb.bin`): pinned to
    /// coreutils, not to this module's own arithmetic.
    const SAMPLE_RGB_SHA256: &str = "b7b9c97ef87d182265f9db77817f0ccdb6f6151a387d14b96f1edc6232a983c5";
    const SAMPLE_IDX_SHA256: &str = "d3d93da2885e73108be903779c78d8ffcabd8843faa47807ada4331c61e8661b";

    /// A frame as the recorded fixtures held it: 320×200 with indices.
    fn indexed_frame() -> CapturedFrame {
        let mut frame = decode_frame(FRAME_BIN).unwrap();
        frame.palette = Some(recorded_palette());
        frame.indexed = Some(FB_BIN.to_vec());
        frame
    }

    fn options(frames: Vec<u64>, out: &Path) -> CaptureOptions {
        // Validation refuses a --cwd that is not there.
        let cwd = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/original");
        CaptureOptions {
            recipe: Recipe {
                exe: cwd.join("SECOND.EXE"),
                cwd,
                arguments: vec!["2".into()],
                cycles: 25000,
                keys: keys_from_script("KBD_left KBD_enter"),
                boot: BootRecipe::default(),
                boot_wait: BootWait::default(),
            },
            frames,
            out: out.to_path_buf(),
            skip_boot_frames: 5000,
            sheet: None,
        }
    }

    fn scratch_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("xtask-capture-test-{name}"));
        let _ = std::fs::remove_dir_all(&path);
        path
    }

    /// A manifest as `capture` writes one, with placeholder digests.
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
            boot_wait: BootWait::Setup,
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

    /// The manifest round-trips, and records the recipe under the keys and words a reader
    /// looks for (D14, D17).
    #[test]
    fn the_manifest_round_trips_and_records_the_recipe() {
        let mut manifest = sample_manifest();
        manifest.boot_wait = BootWait::Graphics;
        let json = serde_json::to_string(&manifest).unwrap();
        assert_eq!(serde_json::from_str::<CaptureManifest>(&json).unwrap(), manifest);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(value["cycles"], 25000);
        assert_eq!(value["frame_origin"], "boot");
        assert_eq!(value["boot"], "autoexec");
        assert_eq!(value["boot_wait"], "graphics");
        assert_eq!(value["keys"][1], "KBD_enter");
    }

    /// A manifest filed before `boot` and `boot_wait` existed is the typed recipe waiting for
    /// the setup screen, which is how every one of them was taken.
    #[test]
    fn an_older_manifest_means_the_typed_recipe_and_the_setup_screen() {
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
        let manifest: CaptureManifest = serde_json::from_str(json).unwrap();
        assert_eq!(manifest.boot, BootRecipe::Typed);
        assert_eq!(manifest.boot_wait, BootWait::Setup);
    }

    #[test]
    fn a_captured_frame_carries_its_digest() {
        // `write_frame`'s return value is the manifest entry, digested from the bytes written.
        let out = scratch_dir("digest");
        std::fs::create_dir_all(&out).unwrap();
        let frame = indexed_frame();
        let entry = write_frame(&out, 430, &frame).unwrap();

        let rgb = std::fs::read(out.join(&entry.rgb)).unwrap();
        let idx = std::fs::read(out.join(entry.idx.as_deref().unwrap())).unwrap();
        assert_eq!(rgb, frame.rgb, "the rgb file is the frame that was written");
        assert_eq!(idx, frame.indexed.clone().unwrap());

        assert_eq!(entry.sha256.as_deref(), Some(SAMPLE_RGB_SHA256));
        assert_eq!(entry.idx_sha256.as_deref(), Some(SAMPLE_IDX_SHA256));
        assert_eq!(sha256_hex(&rgb), SAMPLE_RGB_SHA256);
        assert_eq!(sha256_hex(&idx), SAMPLE_IDX_SHA256);
        let value = serde_json::to_value(&entry).unwrap();
        assert_eq!(value["sha256"], SAMPLE_RGB_SHA256);
        assert_eq!(value["idx_sha256"], SAMPLE_IDX_SHA256);
    }

    #[test]
    fn a_frame_the_coherence_check_refused_records_no_index_digest() {
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
    fn a_boot_wait_without_an_autoexec_boot_is_refused() {
        // The typed recipe waits for the setup screen its keys answer; only the autoexec
        // recipe can choose another anchor.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.recipe.boot = BootRecipe::Typed;
        opts.recipe.boot_wait = BootWait::Graphics;
        match validate_options(&opts) {
            Err(CaptureError::Options(message)) => assert!(
                message.contains("--boot-wait graphics") && message.contains("autoexec"),
                "the refusal must name the flag as clap spells it and the recipe it needs: \
                 {message}"
            ),
            other => panic!("expected a refusal, got {other:?}"),
        }
        opts.recipe.boot_wait = BootWait::Setup;
        assert!(validate_options(&opts).is_ok());
        opts.recipe.boot = BootRecipe::Autoexec;
        opts.recipe.boot_wait = BootWait::Graphics;
        assert!(validate_options(&opts).is_ok());
    }

    #[test]
    fn the_capture_tells_the_oracle_which_anchor_to_wait_for() {
        // `boot` reads the anchor, so it has to be set first; the manifest records it.
        let out = scratch_dir("boot-wait-oracle");
        let mut opts = options(vec![5100], &out);
        opts.recipe.boot = BootRecipe::Autoexec;
        opts.recipe.boot_wait = BootWait::Graphics;
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();
        assert_eq!(oracle.boot_saw_wait, Some(Some(BootWait::Graphics)));
        assert_eq!(manifest.boot_wait, BootWait::Graphics);
        std::fs::remove_dir_all(&out).unwrap();

        let out = scratch_dir("boot-wait-oracle-default");
        let opts = options(vec![5100], &out);
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();
        assert_eq!(oracle.boot_saw_wait, Some(Some(BootWait::Setup)));
        assert_eq!(manifest.boot_wait, BootWait::Setup);
        std::fs::remove_dir_all(&out).unwrap();
    }

    #[test]
    fn the_autoexec_recipe_runs_the_whole_boot_in_the_guest() {
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
        // `AUTOTYPE` with no buttons is an error.
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
        // A name `AUTOTYPE` would silently press nothing for is refused, in both recipes,
        // before an emulator is spawned; every name in the table passes.
        for key in ["KBD_kp_8", "KBD_nope", "left", "KBD_NONE"] {
            let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
            opts.recipe.keys = vec![key.to_owned()];
            for boot in [BootRecipe::Typed, BootRecipe::Autoexec] {
                opts.recipe.boot = boot;
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

        for boot in [BootRecipe::Typed, BootRecipe::Autoexec] {
            let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
            opts.recipe.boot = boot;
            assert!(validate_options(&opts).is_ok());
        }
        for (api, _) in KEY_NAMES {
            let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
            opts.recipe.boot = BootRecipe::Autoexec;
            opts.recipe.keys = vec![(*api).to_owned()];
            assert!(validate_options(&opts).is_ok(), "{api} must be usable");
        }
    }

    #[test]
    fn the_key_table_is_sorted_and_unique() {
        for pair in KEY_NAMES.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} then {}", pair[0].0, pair[1].0);
        }
    }

    #[test]
    fn the_typed_recipe_gives_the_guest_nothing_to_run() {
        let opts = options(vec![5100], Path::new("/tmp/never-written"));
        assert_eq!(opts.recipe.boot, BootRecipe::Typed);
        assert!(opts.recipe.autoexec_lines().unwrap().is_empty());

        // The autoexec mount is absolute, as the emulator's allow-list is.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.recipe.boot = BootRecipe::Autoexec;
        let lines = opts.recipe.autoexec_lines().unwrap();
        assert_eq!(lines[0], format!("MOUNT C -RO {}", opts.recipe.cwd.canonicalize().unwrap().display()));
        assert_eq!(lines.last().unwrap(), "SECOND.EXE 2");
    }

    #[test]
    fn the_guest_answers_the_setup_screen_under_the_autoexec_recipe() {
        // Under the autoexec recipe the guest answers the screen; the typed one waits for it and
        // sends the keys over the API (D17).
        let out = scratch_dir("autoexec-keys");
        let mut opts = options(vec![5100], &out);
        opts.recipe.boot = BootRecipe::Autoexec;
        let mut oracle = FakeOracle::new(vec![indexed_frame()]);
        let manifest = capture(&mut oracle, &opts).unwrap();
        assert_eq!(manifest.boot, BootRecipe::Autoexec);
        assert!(oracle.waited_for_text.is_empty());
        assert_eq!(oracle.keys, None);
        std::fs::remove_dir_all(&out).unwrap();

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
        // The keys are sent and the setup screen stays up: the guest has to judge, since a real
        // key such as `KBD_kp8` can answer nothing there. Nothing is written.
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
        assert!(!out.join("manifest.json").exists());

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
        opts.recipe.cycles = 0;
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Oracle(OracleError::UnpinnedCycles))
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

        // An index inside the skipped boot.
        let opts = options(vec![4999], Path::new("/tmp/never-written"));
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));
    }

    #[test]
    fn the_program_must_live_in_the_directory_that_is_mounted() {
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        opts.recipe.exe = PathBuf::from("elsewhere/SECOND.EXE");
        assert!(matches!(
            validate_options(&opts),
            Err(CaptureError::Options(_))
        ));
    }

    #[test]
    fn a_trailing_slash_on_the_working_directory_still_matches() {
        // The CLI's default is spelled `reference/original/`.
        let mut opts = options(vec![5100], Path::new("/tmp/never-written"));
        let cwd = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/original/");
        opts.recipe.exe = cwd.join("SECOND.EXE");
        opts.recipe.cwd = cwd;
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

        assert_eq!(oracle.counter, 5200);
        assert_eq!(oracle.stepped_frames, vec![5100, 5200]);

        let on_disk: CaptureManifest =
            serde_json::from_str(&std::fs::read_to_string(out.join("manifest.json")).unwrap())
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
            // Only the entries the frame uses.
            let palette = frame.palette.as_ref().unwrap();
            assert_eq!(palette[&1], [11, 48, 63]);
            let distinct: std::collections::BTreeSet<u8> = FB_BIN.iter().copied().collect();
            assert_eq!(palette.len(), distinct.len());
            assert!(palette.len() < 256);
        }
    }

    #[test]
    fn indices_within_a_read_of_each_other_are_all_taken_before_any_is_read() {
        // Closer than INDEXED_READ_FRAMES: every take before any read (D18, D19).
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
        // Far apart: each read follows its take, before the screen moves on.
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
        let at = |x: u32, y: u32| {
            let at = ((y * width + x) * 3) as usize;
            [rgb[at], rgb[at + 1], rgb[at + 2]]
        };
        assert_eq!(at(3, 3), [frame_rgb[0], frame_rgb[1], frame_rgb[2]]);
        assert_eq!(at(327, 3), [frame_rgb[0], frame_rgb[1], frame_rgb[2]]);
        // The gutter between the cells is the sheet's background.
        assert_eq!(at(324, 3), [40, 40, 40]);
        std::fs::remove_dir_all(&out).unwrap();
    }

    /// `reference/` is input-only, and so is the capture this run writes; a sheet goes outside
    /// both.
    #[test]
    fn a_sheet_inside_the_reference_or_the_capture_is_refused() {
        let options_with = |sheet: PathBuf| {
            let mut opts = options(vec![5100], Path::new("out/capture"));
            opts.sheet = Some(sheet);
            opts
        };
        let mounted = options(vec![5100], Path::new("out/capture")).recipe.cwd.join("sheet.png");
        for sheet in [
            mounted,
            PathBuf::from("reference/sheet.png"),
            PathBuf::from("reference/check_a/sheet.png"),
            PathBuf::from("reference/parts/beglogo/sheet.png"),
            PathBuf::from("out/capture/sheet.png"),
        ] {
            assert!(
                matches!(validate_options(&options_with(sheet.clone())), Err(CaptureError::Options(_))),
                "{} must be refused",
                sheet.display()
            );
        }
        assert!(validate_options(&options_with(PathBuf::from("out/beglogo-sheet.png"))).is_ok());
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
            decode_frame(MODEX_FRAME).unwrap(),
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

    /// An oracle that answers from memory, for the capture's ordering, refusals and output.
    struct FakeOracle {
        frames: VecDeque<CapturedFrame>,
        counter: u64,
        stepped_frames: Vec<u64>,
        /// Every take and every read, in order.
        events: Vec<String>,
        keys: Option<String>,
        waited_for_text: Vec<String>,
        waited_for_graphics: Vec<String>,
        /// The anchor `capture` set, and what `boot` saw when it ran.
        boot_wait: Option<BootWait>,
        boot_saw_wait: Option<Option<BootWait>>,
        /// Whether the setup screen goes away when the keys are sent.
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
                boot_wait: None,
                boot_saw_wait: None,
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
            self.boot_saw_wait = Some(self.boot_wait);
            Ok(())
        }
        fn set_boot_wait(&mut self, wait: BootWait) -> Result<(), OracleError> {
            self.boot_wait = Some(wait);
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
