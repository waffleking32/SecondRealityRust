//! The reference emulator, behind a trait.
//!
//! The oracle is only needed for verification, so it is a trait rather than a
//! hard dependency: an unusable emulator degrades to a documented alternative
//! instead of blocking a phase (spec §8.1, `docs/decisions.md` D10). The one
//! implementation so far drives `dosbox-automation` over its REST API
//! (`docs/decisions.md` D11, D14).

pub mod dosbox;

use std::fmt;
use std::path::Path;
use std::time::Duration;

/// The frame counter counts rendered frames from the emulator's first frame at
/// process start, and every captured index is counted from there too. Recorded
/// in the manifest so an index means the same thing in two runs.
pub const FRAME_ORIGIN: &str = "boot";

/// The dos rate the emulator paces itself to (`docs/decisions.md` D14).
const FRAME_RATE_HZ: f64 = 70.0;

#[derive(Debug)]
pub enum OracleError {
    /// The API token was rejected.
    Unauthorized,
    /// Any other non-2xx answer, with the body the emulator sent.
    Http { status: u16, body: String },
    /// The emulator could not be reached or did not start.
    Transport(String),
    /// Capture without pinned CPU cycles is not reproducible; refuse it.
    UnpinnedCycles,
    /// No frame is available yet.
    NotBooted,
    /// A response did not have the shape the API documents.
    Decode(String),
    /// A wait for screen text or for a frame ran out.
    Timeout(String),
    /// The emulator ran past the requested frame before a capture could be
    /// taken of it, so the capture would not be of the frame that was asked
    /// for. The capture is refused rather than mislabelled.
    FrameMissed { index: u64, observed: u64 },
}

impl fmt::Display for OracleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            OracleError::Unauthorized => write!(
                f,
                "the emulator rejected the API token (401); the token must be exactly 64 hex \
                 characters and is passed in DOSBOX_API_TOKEN"
            ),
            OracleError::Http { status, body } => {
                write!(f, "the emulator answered {status}: {body}")
            }
            OracleError::Transport(message) => write!(f, "{message}"),
            OracleError::UnpinnedCycles => write!(
                f,
                "refusing to capture without pinned CPU cycles: an unpinned capture is not \
                 reproducible and must never be mistaken for a reference (pass --cycles)"
            ),
            OracleError::NotBooted => write!(f, "the emulator has no frame yet"),
            OracleError::Decode(message) => write!(f, "could not decode: {message}"),
            OracleError::Timeout(message) => write!(f, "timed out: {message}"),
            OracleError::FrameMissed { index, observed } => write!(
                f,
                "the emulator reached frame {observed} before frame {index} could be captured; \
                 the capture would not be of the requested frame"
            ),
        }
    }
}

impl std::error::Error for OracleError {}

/// One frame as the oracle held it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapturedFrame {
    pub width: u16,
    pub height: u16,
    /// Indexed pixels, when the oracle exposes them. Preferred for comparison,
    /// because the DAC widening rule then cannot affect a fidelity measurement.
    pub indexed: Option<Vec<u8>>,
    /// Indexed palette as the DAC held it, 0..=63 per component.
    pub palette: Option<[[u8; 3]; 256]>,
    /// Always present, three bytes per pixel, top row first.
    pub rgb: Vec<u8>,
}

impl CapturedFrame {
    /// True when `rgb` is the indexed pixels widened through `palette`.
    ///
    /// The emulator's palette registers are read a request at a time while the
    /// guest runs, so an indexed capture of a moving screen can mix moments.
    /// It is only written as a reference when it reproduces the RGB frame.
    pub fn indexed_matches_rgb(&self) -> bool {
        match (&self.indexed, &self.palette) {
            (Some(indexed), Some(palette)) => {
                indexed.len() * 3 == self.rgb.len()
                    && indexed
                        .iter()
                        .zip(self.rgb.chunks_exact(3))
                        .all(|(&i, rgb)| widen(palette[i as usize]) == [rgb[0], rgb[1], rgb[2]])
            }
            _ => false,
        }
    }
}

/// The widening the emulator applies to six-bit DAC values: round-to-nearest,
/// `(c * 259 + 33) >> 6` (identical to `round(255c/63)`), not bit replication —
/// see `docs/decisions.md` D14 and `crates/sr-engine/src/palette.rs`.
pub fn widen(component: [u8; 3]) -> [u8; 3] {
    component.map(|c| ((((c & 0x3F) as u16) * 259 + 33) >> 6) as u8)
}

/// The planar (Mode X) layout the demo's 320×400 title screen uses:
/// `idx(x, y) = planes[x % 4][y * (width / 4) + x / 4]`, verified against the
/// recorded planes and frame with zero conflicts (`docs/decisions.md` D14).
pub fn assemble_planes(
    planes: &[Vec<u8>],
    width: u16,
    height: u16,
) -> Result<Vec<u8>, OracleError> {
    if width == 0 || width % 4 != 0 || height == 0 {
        return Err(OracleError::Decode(format!(
            "planar capture needs a width divisible by four, got {width}x{height}"
        )));
    }
    if planes.len() != 4 {
        return Err(OracleError::Decode(format!(
            "planar capture reads four planes, got {}",
            planes.len()
        )));
    }
    let stride = width as usize / 4;
    if planes.iter().any(|p| p.len() < stride * height as usize) {
        return Err(OracleError::Decode(format!(
            "a plane holds less than the {stride}×{height} bytes {width}x{height} needs"
        )));
    }

    let mut indexed = vec![0u8; width as usize * height as usize];
    for y in 0..height as usize {
        for x in 0..width as usize {
            indexed[y * width as usize + x] = planes[x % 4][y * stride + x / 4];
        }
    }
    Ok(indexed)
}

/// How long to wait for something that should happen within a handful of
/// frames, and how long for the emulator to come up.
pub const DEFAULT_WAIT: Duration = Duration::from_secs(30);

/// The guest's command line for `exe`: its file name and the arguments, as the
/// shell runs it. `None` when the path has no file name to run.
///
/// Both boot recipes run this one line — the typed recipe types it, the
/// `[autoexec]` recipe has the guest's own shell read it (D17) — so they
/// cannot come to disagree about what the manifest's `arguments` mean.
pub fn guest_command_line(exe: &Path, arguments: &[String]) -> Option<String> {
    let mut line = exe.file_name()?.to_string_lossy().into_owned();
    for argument in arguments {
        line.push(' ');
        line.push_str(argument);
    }
    Some(line)
}

/// The reference emulator. [`dosbox::DosboxOracle`] is the implementation of
/// record; a second one is only written if that becomes unusable (D10).
pub trait Oracle {
    /// Name and version of the emulator, for the manifest.
    fn describe(&self) -> Result<String, OracleError>;
    /// Start `exe` under the emulator, from `cwd`: `SECOND.EXE` loads
    /// `REALITY.FC` from the working directory, so the two cannot be
    /// separated. Returns once the command has been issued.
    fn boot(&mut self, exe: &Path, cwd: &Path, args: &[String]) -> Result<(), OracleError>;
    /// Pin the emulated CPU. Must happen before the first captured frame.
    fn set_cycles(&mut self, cycles: u32) -> Result<(), OracleError>;
    /// Run forward `frames` rendered frames.
    fn step_frames(&mut self, frames: u32) -> Result<(), OracleError>;
    /// The frame [`Oracle::wait_for_frame`] settled on, when it kept one, and
    /// otherwise the frame the oracle can identify as the one it is showing.
    /// Either way it is a frame the oracle can name, never a guess at one.
    fn frame(&mut self) -> Result<CapturedFrame, OracleError>;
    /// Send a whitespace-separated list of `KBD_*` key names, in order.
    fn send_keys(&mut self, script: &str) -> Result<(), OracleError>;
    /// Stop the emulator and release its process.
    fn shutdown(&mut self) -> Result<(), OracleError>;

    /// Wait until the screen shows `needle`. The guest is the clock here: the
    /// recorded boot recipe waits for the shell prompt and the setup screen
    /// rather than sleeping for guessed times (D14).
    fn wait_for_text(&mut self, needle: &str, timeout: Duration) -> Result<(), OracleError>;
    /// The frame counter, counted from the emulator's first rendered frame.
    fn frame_counter(&mut self) -> Result<u64, OracleError>;
    /// Wait for `index` to be the current frame, take it, and refuse — rather
    /// than let a capture be mislabelled — when it cannot be taken exactly.
    ///
    /// Pinning a frame is the oracle's business, but the promise is the
    /// strict one: the frame kept for the following [`Oracle::frame`] is the
    /// frame the oracle's own counter names `index`, and a frame that arrives
    /// late is refused. A capture is never written under an index whose
    /// picture it does not hold (D17).
    fn wait_for_frame(&mut self, index: u64) -> Result<(), OracleError>;
}

/// Sleep for the part of a wait that does not need polling: the frame counter
/// advances at the dos rate, so all but the last few frames can be slept off.
fn sleep_until_close(remaining: u64) {
    const TIGHT_POLL_FRAMES: u64 = 30;
    sleep_frames(remaining, TIGHT_POLL_FRAMES);
}

/// Sleep off all but the last `tail` of `remaining` frames, which the caller
/// polls itself. A wait that has to look at each of the last few frames — to
/// pin one exactly — leaves exactly those frames to its own loop.
///
/// One call sleeps at most a second, so a caller that re-reads the counter
/// between calls lands on its target rather than a frame or two past it: the
/// sleep is computed from the nominal frame rate, and a minute-long sleep
/// overshoots by more than a frame (measured: three, which is where the old
/// capture's one-to-three-frame lateness came from — D17).
pub(crate) fn sleep_frames(remaining: u64, tail: u64) {
    const LONGEST_SLEEP_FRAMES: u64 = 70;
    if remaining > tail {
        let frames = (remaining - tail).min(LONGEST_SLEEP_FRAMES);
        let ms = (frames as f64 * 1000.0 / FRAME_RATE_HZ) as u64;
        std::thread::sleep(Duration::from_millis(ms.max(1)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAME_BIN: &[u8] = include_bytes!("../../tests/fixtures/frame.bin");
    const FB_BIN: &[u8] = include_bytes!("../../tests/fixtures/fb.bin");
    const DAC_BIN: &[u8] = include_bytes!("../../tests/fixtures/dac.bin");
    const MODEX_FRAME: &[u8] = include_bytes!("../../tests/fixtures/modex_title_frame.bin");
    const MODEX_PLANES: &[u8] = include_bytes!("../../tests/fixtures/modex_title_planes.bin");

    fn recorded_palette() -> [[u8; 3]; 256] {
        let mut palette = [[0u8; 3]; 256];
        for (i, entry) in palette.iter_mut().enumerate() {
            entry.copy_from_slice(&DAC_BIN[i * 3..i * 3 + 3]);
        }
        palette
    }

    #[test]
    fn the_guest_command_line_is_the_file_name_and_its_arguments() {
        // Both boot recipes run this line (D17), so its shape is pinned here
        // rather than only under whichever recipe happens to be default.
        let exe = Path::new("/reference/original/SECOND.EXE");
        assert_eq!(
            guest_command_line(exe, &["2".to_owned()]),
            Some("SECOND.EXE 2".to_owned())
        );
        assert_eq!(guest_command_line(exe, &[]), Some("SECOND.EXE".to_owned()));
        assert_eq!(guest_command_line(Path::new(""), &[]), None);
    }

    #[test]
    fn widening_six_bit_dac_values_rounds_rather_than_replicates() {
        assert_eq!(widen([0, 0, 0]), [0, 0, 0]);
        assert_eq!(widen([11, 48, 63]), [45, 194, 255]);
        // The three values where bit replication and round-to-nearest disagree
        // in a way a reference capture would show (D14).
        assert_eq!(widen([15, 15, 15]), [61, 61, 61]);
        assert_eq!(widen([52, 52, 52]), [210, 210, 210]);
    }

    #[test]
    fn the_recorded_index_and_palette_reproduce_the_recorded_frame() {
        let frame = CapturedFrame {
            width: 320,
            height: 200,
            indexed: Some(FB_BIN.to_vec()),
            palette: Some(recorded_palette()),
            rgb: dosbox::decode_frame(FRAME_BIN).unwrap().rgb,
        };
        assert!(frame.indexed_matches_rgb());

        // And the check can fail: one wrong index is not silently accepted.
        let mut broken = frame.clone();
        let mut indexed = FB_BIN.to_vec();
        indexed[320 * 100 + 160] ^= 0xFF;
        broken.indexed = Some(indexed);
        assert!(!broken.indexed_matches_rgb());
    }

    #[test]
    fn assembled_mode_x_planes_agree_with_the_recorded_title_screen() {
        let planes: Vec<Vec<u8>> = (0..4)
            .map(|p| {
                let start = p * 32_000;
                MODEX_PLANES[start..start + 32_000].to_vec()
            })
            .collect();
        let indexed = assemble_planes(&planes, 320, 400).unwrap();
        assert_eq!(indexed.len(), 320 * 400);

        // The title screen's own palette was not recorded, so the check D14
        // made is the one that needs no palette: the planes put the same
        // palette index at every pixel the frame gives the same colour, with
        // no conflicts, and the screen really is 57 colours rather than a
        // flat fill.
        let rgb = dosbox::decode_frame(MODEX_FRAME).unwrap().rgb;
        let mut seen: [Option<[u8; 3]>; 256] = [None; 256];
        let mut distinct = 0;
        for (i, (&index, pixel)) in indexed.iter().zip(rgb.chunks_exact(3)).enumerate() {
            let here = [pixel[0], pixel[1], pixel[2]];
            match seen[index as usize] {
                None => {
                    seen[index as usize] = Some(here);
                    distinct += 1;
                }
                Some(before) => assert_eq!(
                    before, here,
                    "palette index {index} appears with two colours, at pixel {i}"
                ),
            }
        }
        assert_eq!(distinct, 57, "the recorded title screen holds 57 indices");
    }

    #[test]
    fn planes_that_are_too_short_are_refused_not_indexed_out_of_bounds() {
        let short = vec![vec![0u8; 100]; 4];
        assert!(matches!(
            assemble_planes(&short, 320, 400),
            Err(OracleError::Decode(_))
        ));
        assert!(matches!(
            assemble_planes(&[], 320, 400),
            Err(OracleError::Decode(_))
        ));
    }
}
