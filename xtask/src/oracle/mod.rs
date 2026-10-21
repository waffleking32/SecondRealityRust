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

use crate::reference::BootWait;

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
    /// The screen is real and the frame was captured fine; the *indexed* part
    /// of the read does not describe it, so the frame keeps its RGB and no
    /// indices are written. Deliberate and permanent, and kept apart from
    /// `Decode` so a log says which happened (fix round 2).
    ScreenUnreadable(ScreenRefusal),
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
            OracleError::ScreenUnreadable(refusal) => write!(
                f,
                "the screen is not one the indexed read can describe, so this frame keeps its \
                 RGB: {refusal}"
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

/// What kind of screen the indexed read is looking at.
///
/// The read's arithmetic only describes screens that put **one palette index a
/// byte**: 256-colour planar (four planes, a byte of a plane per four pixels)
/// and 256-colour chained (one byte a pixel, which is what mode 13h and its
/// unchained "Mode X" tweak both mean). A 16-colour or 2-colour planar screen
/// packs *bits*, not bytes, and nothing in the tree has needed it; naming the
/// layout rather than asking "is chain-4 on?" is what keeps that from being
/// assembled at a guessed stride (fix round 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenLayout {
    /// 256 colours, four planes, one byte of a plane per four pixels.
    Planar256,
    /// 256 colours, one layout-wide image, one byte a pixel.
    Chained256,
    /// A screen this read does not model.
    Unsupported,
}

impl ScreenLayout {
    /// From the two things the emulator says about the mode: the colour depth
    /// it names (`webserver/video.cpp`'s `video_mode.color_depth`, whose
    /// strings are `misc/video.cpp:27-41`), and whether the sequencer has
    /// chain-4 on. Anything that is not "256-colour" is unmodelled, and so is
    /// a depth the API did not name at all.
    pub fn of(color_depth: &str, chain4: bool) -> ScreenLayout {
        if color_depth != "256-colour" {
            return ScreenLayout::Unsupported;
        }
        if chain4 {
            ScreenLayout::Chained256
        } else {
            ScreenLayout::Planar256
        }
    }
}

/// What the indexed read will do with a screen, worked out before any of it
/// happens so that the decision can be tested without an emulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReadPlan {
    /// Bytes one scanline advances the address counter, and the row length in
    /// the plane the read walks.
    pub stride: usize,
    /// Bytes one row of *pixels* needs: one a pixel chained, one per four
    /// pixels a plane.
    pub row_bytes: usize,
    /// Bytes to read from the start of each plane.
    pub plane_bytes: usize,
}

/// Why a screen cannot be read as indices.
///
/// This is a deliberate, permanent refusal about a screen the read does not
/// describe — not a decode failure — and it is its own type so a capture log
/// can say so (fix round 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScreenRefusal {
    /// The mode is not one this read models: not 256-colour, or a depth the
    /// API did not name.
    UnmodelledMode,
    /// The screen's own offset register implies rows shorter than the pixels
    /// it shows.
    RowTooShort { stride: usize, row_bytes: usize },
    /// The rows need more bytes than one VGA plane holds; the guest's display
    /// wraps and this read cannot say what it shows.
    PlaneTooSmall {
        stride: usize,
        height: u16,
        plane_bytes: usize,
    },
}

impl fmt::Display for ScreenRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScreenRefusal::UnmodelledMode => write!(
                f,
                "this screen is not one palette index a byte (not 256-colour, or a depth the \
                 emulator did not name), and the indexed read describes only those"
            ),
            ScreenRefusal::RowTooShort { stride, row_bytes } => write!(
                f,
                "this screen's rows are {stride} bytes, shorter than the {row_bytes} bytes its \
                 pixels need"
            ),
            ScreenRefusal::PlaneTooSmall {
                stride,
                height,
                plane_bytes,
            } => write!(
                f,
                "this screen's rows are {stride} bytes and it is {height} rows tall, so it needs \
                 {plane_bytes} bytes a plane; one plane holds {PLANE_BYTES}, and the display wraps"
            ),
        }
    }
}

/// One VGA plane's addressable bytes. A screen whose rows need more than this
/// wraps under the guest and cannot be read as one plane.
pub const PLANE_BYTES: usize = 64 * 1024;

/// Bytes one scanline advances the address counter, from the CRTC's offset
/// register (3d4h index 13h), or `None` when the read does not model the
/// screen at all.
///
/// The register is a byte offset divided by a factor the mode sets, and the
/// emulator's own arithmetic is the source: a 256-colour screen draws a line
/// every `scan_len * 8` bytes of its address space (`vga_draw.cpp:1785-1792`,
/// the `M_VGA` case, which is where DOSBox puts mode 13h *and* every
/// unchained "Mode X" tweak of it), and one guest plane byte at offset `o`
/// sits at four-byte group `o` in that space (`vga_memory.cpp:129-135`: an
/// unchained read is `((uint32_t *)linear)[o]` selected by the read map). So
/// one *plane* row is `scan_len * 2` bytes and one chained row is
/// `scan_len * 8`. Only the low byte of the port is the offset.
///
/// Measured against the tree before it was written: 40 × 2 = 80 is what every
/// part whose `.idx` files reproduce their RGB uses, and 88 × 2 = 176 is
/// ALKU's (`ALKU/TWEAK.ASM:46`, `ALKU/MAIN.C:26`).
pub fn row_stride_bytes(offset_register: u16, layout: ScreenLayout) -> Option<usize> {
    let scan_len = (offset_register & 0xFF) as usize;
    match layout {
        ScreenLayout::Planar256 => Some(scan_len * 2),
        ScreenLayout::Chained256 => Some(scan_len * 8),
        ScreenLayout::Unsupported => None,
    }
}

/// Work out what to read for a screen, or why it cannot be read — the whole
/// decision, with no emulator in it (fix round 2's reason for splitting it out
/// of the oracle: a test that calls the real thing is the only kind that
/// notices the arithmetic changing).
pub fn plan_indexed_read(
    width: u16,
    height: u16,
    offset_register: u16,
    layout: ScreenLayout,
) -> Result<ReadPlan, ScreenRefusal> {
    let stride = row_stride_bytes(offset_register, layout).ok_or(ScreenRefusal::UnmodelledMode)?;
    let row_bytes = match layout {
        ScreenLayout::Planar256 => width as usize / 4,
        ScreenLayout::Chained256 => width as usize,
        ScreenLayout::Unsupported => return Err(ScreenRefusal::UnmodelledMode),
    };
    if stride < row_bytes {
        return Err(ScreenRefusal::RowTooShort { stride, row_bytes });
    }
    let plane_bytes = stride * height as usize;
    if plane_bytes > PLANE_BYTES {
        return Err(ScreenRefusal::PlaneTooSmall {
            stride,
            height,
            plane_bytes,
        });
    }
    Ok(ReadPlan {
        stride,
        row_bytes,
        plane_bytes,
    })
}

/// The planar (Mode X) layout: `idx(x, y) = planes[x % 4][y * stride + x / 4]`,
/// with `stride` the screen's own row length in plane bytes rather than an
/// assumed `width / 4`.
///
/// The packed layout the demo's 320×400 title screen uses is `stride =
/// width / 4` and was verified against the recorded planes and frame with zero
/// conflicts (`docs/decisions.md` D14). Taking the stride as an argument is
/// what B3 block 1 added: ALKU's screen is 176 bytes a row where `width / 4`
/// is 80 (`ALKU/TWEAK.ASM:46` writes CRTC 0x13 = `0x5813`), and assembling it
/// at the packed stride put a sheared plane in the reference tree —
/// `reference/parts/alku/frame_000900.idx` in the first pass, whose picture
/// started at row 110 instead of the row 50 `ALKU/MAIN.C:165-166` blits to.
pub fn assemble_planes(
    planes: &[Vec<u8>],
    width: u16,
    height: u16,
    stride: usize,
) -> Result<Vec<u8>, OracleError> {
    if width == 0 || width % 4 != 0 || height == 0 {
        return Err(OracleError::Decode(format!(
            "planar capture needs a width divisible by four, got {width}x{height}"
        )));
    }
    if stride * 4 < width as usize {
        return Err(OracleError::Decode(format!(
            "a {stride}-byte row is narrower than the {width} pixels this screen shows"
        )));
    }
    if planes.len() != 4 {
        return Err(OracleError::Decode(format!(
            "planar capture reads four planes, got {}",
            planes.len()
        )));
    }
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
    /// Tell the oracle what a [`crate::reference::BootRecipe::Autoexec`] boot
    /// waits for before returning ([`BootWait::Setup`] or
    /// [`BootWait::Graphics`]). Called before [`Oracle::boot`], which reads it.
    ///
    /// The default is a no-op: an oracle that does not run a guest boot of its
    /// own has no anchor to wait for, and a typed boot always waits for the
    /// setup screen its keys answer.
    fn set_boot_wait(&mut self, wait: BootWait) -> Result<(), OracleError> {
        let _ = wait;
        Ok(())
    }
    /// Run forward `frames` rendered frames.
    fn step_frames(&mut self, frames: u32) -> Result<(), OracleError>;
    /// The earliest frame [`Oracle::wait_for_frame`] settled on, when it kept
    /// any, and otherwise the frame the oracle can identify as the one it is
    /// showing. Either way it is a frame the oracle can name, never a guess at
    /// one. A capture that takes the neighbours of a keyframe before reading
    /// any of them holds more than one; they come back in the order they were
    /// waited for.
    fn frame(&mut self) -> Result<CapturedFrame, OracleError>;
    /// Send a whitespace-separated list of `KBD_*` key names, in order.
    fn send_keys(&mut self, script: &str) -> Result<(), OracleError>;
    /// Stop the emulator and release its process.
    fn shutdown(&mut self) -> Result<(), OracleError>;

    /// Wait until the screen shows `needle`. The guest is the clock here: the
    /// recorded boot recipe waits for the shell prompt and the setup screen
    /// rather than sleeping for guessed times (D14).
    fn wait_for_text(&mut self, needle: &str, timeout: Duration) -> Result<(), OracleError>;
    /// Wait until the screen is in a graphics mode, or fail. This is how a
    /// capture judges the keys it sent: the setup screen is text, every screen
    /// the demo draws is graphics, and a key sequence that only reaches the
    /// menu's default leaves the guest in text — either at the menu or back at
    /// the DOS prompt after the soundcard it picked fails to start (D17).
    fn wait_for_graphics(&mut self, timeout: Duration) -> Result<(), OracleError>;
    /// The frame counter, counted from the emulator's first rendered frame.
    fn frame_counter(&mut self) -> Result<u64, OracleError>;
    /// Wait for `index` to be the current frame, take it, and refuse — rather
    /// than let a capture be mislabelled — when it cannot be taken exactly.
    ///
    /// Pinning a frame is the oracle's business, but the promise is the
    /// strict one: the frame kept for the following [`Oracle::frame`] call is
    /// the frame the oracle's own counter names `index`, and a frame that
    /// arrives late is refused. A capture is never written under an index
    /// whose picture it does not hold (D17). Waiting for several frames before
    /// reading any of them is allowed — each wait pins the frame its own index
    /// names, and the frames are handed over in the order they were waited
    /// for.
    fn wait_for_frame(&mut self, index: u64) -> Result<(), OracleError>;
    /// Read `len` bytes of the guest's memory at `segment:offset`, which is the
    /// addressing the emulator's own API takes: `segment * 16 + offset`, with a
    /// **32-bit** offset, so any address in the emulated memory can be named by
    /// `(0, address)`. Used by `xtask music trace`, which reads the player's
    /// own state rather than the screen.
    fn read_memory(&self, segment: u16, offset: u32, len: usize) -> Result<Vec<u8>, OracleError>;
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
        let indexed = assemble_planes(&planes, 320, 400, 320 / 4).unwrap();
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
            assemble_planes(&short, 320, 400, 320 / 4),
            Err(OracleError::Decode(_))
        ));
        assert!(matches!(
            assemble_planes(&[], 320, 400, 320 / 4),
            Err(OracleError::Decode(_))
        ));
    }

    /// The stride is the screen's, not `width / 4`: the same plane bytes
    /// assembled at 80 and at 176 are different planes, and only the one the
    /// screen used puts a byte where the picture has it.
    #[test]
    fn assembly_follows_the_planes_own_row_stride() {
        // Four planes of 176-byte rows, each row stamped with its own number,
        // so a wrong stride reads the wrong row rather than by luck another
        // pixel of the same one.
        let stride = 176usize;
        let planes: Vec<Vec<u8>> = (0..4u8)
            .map(|plane| {
                let mut p = vec![0u8; stride * 400];
                for y in 0..400usize {
                    for c in 0..stride {
                        p[y * stride + c] = (y as u8).wrapping_add(plane);
                    }
                }
                p
            })
            .collect();

        let indexed = assemble_planes(&planes, 320, 400, stride).unwrap();
        for y in 0..400usize {
            for x in 0..320usize {
                assert_eq!(
                    indexed[y * 320 + x],
                    (y as u8).wrapping_add((x % 4) as u8),
                    "pixel ({x},{y}) does not come from row {y}"
                );
            }
        }

        // The packed stride is a different plane, and it shears: the plane's
        // row 50 — where ALKU's picture begins — comes out at screen row 110
        // when the same bytes are read at 80, which is the 110 the first
        // pass's `frame_000900.idx` started its picture at.
        let packed = assemble_planes(&planes, 320, 400, 80).unwrap();
        assert_eq!(packed[110 * 320], indexed[50 * 320]);
        assert_ne!(packed, indexed);

        // A row narrower than the screen's pixels cannot be assembled at all.
        assert!(matches!(
            assemble_planes(&planes, 320, 400, 40),
            Err(OracleError::Decode(_))
        ));
    }

    /// The stride is the emulator's own arithmetic: index 13h in the mode's
    /// units. This calls the production function — the point of fix round 2
    /// was that a test restating the formula cannot notice it changing.
    #[test]
    fn the_row_stride_is_the_crtc_offset_in_the_modes_units() {
        use ScreenLayout::{Chained256, Planar256, Unsupported};

        // The packed row every part but ALKU and PLZPART uses: mode 13h
        // leaves index 13h at 0x28, 40 × 2 = 80 = width / 4 for a 320-wide
        // screen.
        assert_eq!(row_stride_bytes(0x28, Planar256), Some(320 / 4));
        // The two screens this phase has measured in a part's own source.
        assert_eq!(row_stride_bytes(0x58, Planar256), Some(176), "ALKU");
        assert_eq!(row_stride_bytes(0x50, Planar256), Some(160), "PLZPART");
        assert_eq!(row_stride_bytes(0x2E, Planar256), Some(92), "GLENZ");
        // Chain-4 draws a byte a pixel, so its row is eight bytes an offset.
        assert_eq!(row_stride_bytes(0x28, Chained256), Some(320));
        assert_eq!(row_stride_bytes(0x00, Chained256), Some(0));
        // Only the low byte of the port is the offset: the high byte of a
        // 16-bit port read is not part of the register.
        assert_eq!(row_stride_bytes(0xFF58, Planar256), Some(176));
        assert_eq!(row_stride_bytes(0x0158, Planar256), Some(176));
        assert_eq!(row_stride_bytes(0xFF00, Chained256), Some(0));
        // A screen the read does not model has no stride at all.
        assert_eq!(row_stride_bytes(0x28, Unsupported), None);
    }

    /// Which screen is which comes from the emulator's colour depth, not from
    /// chain-4 alone: 16-colour planar is the case that would otherwise be
    /// assembled at a plausible-looking wrong stride.
    #[test]
    fn the_layout_follows_the_modes_colour_depth() {
        assert_eq!(
            ScreenLayout::of("256-colour", false),
            ScreenLayout::Planar256
        );
        assert_eq!(
            ScreenLayout::of("256-colour", true),
            ScreenLayout::Chained256
        );
        for depth in [
            "16-colour",
            "4-colour",
            "2-colour",
            "monochrome",
            "composite",
            "15-bit high colour",
            "16-bit high colour",
            "24-bit true colour",
            "",
        ] {
            assert_eq!(
                ScreenLayout::of(depth, false),
                ScreenLayout::Unsupported,
                "{depth} is not a screen this read can index"
            );
            assert_eq!(ScreenLayout::of(depth, true), ScreenLayout::Unsupported);
        }
    }

    /// Every reason the read can refuse a screen, and the three screens the
    /// tree has: the one it reads, ALKU's, and a screen that is not indexed
    /// content at all.
    #[test]
    fn a_screen_the_read_cannot_describe_is_refused_with_its_reason() {
        // The packed 320×400 screen every `.idx` in the tree came from.
        assert_eq!(
            plan_indexed_read(320, 400, 0x28, ScreenLayout::Planar256),
            Ok(ReadPlan {
                stride: 80,
                row_bytes: 80,
                plane_bytes: 32_000,
            })
        );
        // ALKU's: the stride is right and the plane cannot hold the screen.
        assert_eq!(
            plan_indexed_read(320, 400, 0x58, ScreenLayout::Planar256),
            Err(ScreenRefusal::PlaneTooSmall {
                stride: 176,
                height: 400,
                plane_bytes: 70_400,
            })
        );
        // GLENZ's 320×200 at 92 bytes a row fits a plane, and is why block 2
        // is the first place this path runs for real.
        assert_eq!(
            plan_indexed_read(320, 200, 0x2E, ScreenLayout::Planar256),
            Ok(ReadPlan {
                stride: 92,
                row_bytes: 80,
                plane_bytes: 18_400,
            })
        );
        // A row narrower than the pixels it would have to show.
        assert_eq!(
            plan_indexed_read(320, 400, 0x10, ScreenLayout::Planar256),
            Err(ScreenRefusal::RowTooShort {
                stride: 32,
                row_bytes: 80,
            })
        );
        // A 16-colour screen is refused before any stride is considered.
        assert_eq!(
            plan_indexed_read(320, 400, 0x28, ScreenLayout::Unsupported),
            Err(ScreenRefusal::UnmodelledMode)
        );
        // A chained screen wider than its own rows is refused too, where the
        // old read would have gather past the end of its buffer.
        assert_eq!(
            plan_indexed_read(320, 200, 0x20, ScreenLayout::Chained256),
            Err(ScreenRefusal::RowTooShort {
                stride: 256,
                row_bytes: 320,
            })
        );
        // A chained screen with rows wider than the screen is read as a
        // gather — when it still fits one image. A 320×200 screen cannot: any
        // row wider than 320 bytes overflows the 64 KiB a chain-4 image has,
        // so the read refuses rather than gathering.
        assert_eq!(
            plan_indexed_read(320, 200, 0x2E, ScreenLayout::Chained256),
            Err(ScreenRefusal::PlaneTooSmall {
                stride: 368,
                height: 200,
                plane_bytes: 73_600,
            })
        );
        assert_eq!(
            plan_indexed_read(320, 150, 0x2E, ScreenLayout::Chained256),
            Ok(ReadPlan {
                stride: 368,
                row_bytes: 320,
                plane_bytes: 55_200,
            })
        );
    }
}
