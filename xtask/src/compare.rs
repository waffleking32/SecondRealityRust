//! `xtask compare`: run the port, diff its frames against the reference, report.
//!
//! This is the other half of the harness `xtask reference capture` opened:
//! capture says what the original drew at a frame, compare says how far the
//! port is from it. It turns "the picture is slightly off" into a number.
//!
//! Two manifest shapes meet here (the controller's Ruling 2): the capture
//! manifest written by `reference capture` (`reference.rs`), and the dump
//! manifest written by `secondreality --headless --dump-dir`. They share
//! `version`, `width`, `height` and `frames: [{index, idx, rgb}]`; everything
//! else on either side is ignored.
//!
//! Three properties are the reason the tool exists:
//!
//! - **Indexed comparison when both sides have indices.** The emulator widens
//!   the DAC with `(c*259+33)>>6` and the port adopts the same rule (Task 11's
//!   ruling), but comparing indices never depends on that rule at all — and it
//!   is stricter: two palette entries may widen to the same RGB while the
//!   indices still differ, and an index is what the part actually wrote. RGB
//!   is the fallback, used only when one side lacks the indices (a capture
//!   whose coherence check failed, D15).
//! - **Frames pair by manifest index, not by position in the list, and the
//!   rendered side may sit one frame either side of the index it answers for.**
//!   The oracle's animation phase at one boot index is not reproducible across
//!   emulator sessions (B1/D17), so the alignment is *searched* and the offset
//!   each frame was paired at is reported. A reference index with no rendered
//!   frame within that window is refused rather than paired with something
//!   further away: this widens the alignment, never the pixel comparison, and
//!   what it could hide is recorded in D18.
//! - **Exit code 0 when every frame is identical, 1 when any differs** (and 2
//!   for "could not compare at all", which is not a fidelity measurement), so
//!   a phase's exit criteria can be checked by a script.
//!
//! A geometry mismatch is a hard error, not a truncated comparison: the port's
//! frame is resized from the view every tick, so a run can legitimately hold
//! frames of more than one size, and comparing the overlapping corner of two
//! different geometries would report a number that means nothing.
//!
//! `--out` defaults to `out/compare` (gitignored, and it cannot collide with a
//! `--dump-dir`, which the caller names). The reference directory is
//! **input-only**: an `--out` that resolves inside it is refused, resolved
//! paths and all, because a report written among the captures would dirty the
//! bytes every later diff is measured against — the same principle that makes
//! `reference/original/` read-only.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io::BufWriter;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, ExitCode};

use serde::{Deserialize, Serialize};

/// What a manifest may say it is; anything else is refused rather than read.
const MANIFEST_VERSION: u32 = 1;

/// The file a capture directory is described by, on both sides.
const MANIFEST_FILE: &str = "manifest.json";

/// The report this tool writes next to the diff images.
const REPORT_FILE: &str = "report.json";

/// The report format's own version, independent of the manifests' (`MANIFEST_VERSION`
/// is what both sides must say; this is what this tool writes).
///
/// Version 2 is the alignment search (D18): each frame records the offset it
/// was paired at and the summary counts them, so a reader that knows version 1
/// — where a frame's counterpart was always the frame at its own index — must
/// not read a version-2 report as if that were still true.
const REPORT_VERSION: u32 = 2;

/// How far either side of its reference index a rendered frame may sit.
///
/// One frame, because that is what the oracle's ambiguity was measured to be
/// (B1/D17: a session's animation phase is one frame away from another's, a
/// coin flip across sessions and constant within one). It is a constant and not
/// an option on purpose: this is a *search over alignment*, not a tolerance on
/// the pixels, and a knob that widens it would be the tolerance mistake wearing
/// a different name (D18).
const ALIGNMENT_WINDOW: u32 = 1;

/// The score of a candidate that cannot be compared at all — a rendered frame
/// whose geometry is not the reference's. It ranks below every candidate that
/// can be compared, so a comparable one always wins; if it is the only
/// candidate it is still chosen, and the geometry check refuses it out loud
/// with both pixel counts named rather than the report showing a number nobody
/// can interpret.
const INCOMPARABLE: u64 = u64::MAX;

/// Why a comparison could not produce a number.
#[derive(Debug)]
pub enum CompareError {
    /// The two frames hold different numbers of pixels. Reported in pixels,
    /// the reference side first, so the message names which side is which.
    GeometryMismatch { reference: usize, rendered: usize },
    /// A reference frame has no rendered frame it could honestly be paired
    /// with: nothing unused within the alignment window. The comparison is
    /// refused rather than pairing a frame from further away, which would
    /// report a number that means nothing.
    NoFrameWithinWindow {
        /// The reference frame's manifest index.
        index: u32,
        /// How far either side of `index` a candidate may sit.
        window: u32,
        /// The rendered index nearest to `index`, if the rendered side holds
        /// any frames at all. The distance is in the message.
        nearest: Option<u32>,
        /// How many frames the rendered side holds, so a manifest that is
        /// empty or wildly the wrong shape is visible in the message.
        rendered_frames: usize,
    },
    /// A manifest, a frame file, or the layout of the two sides could not be
    /// read or matched. The message names what and where.
    Io(String),
}

impl fmt::Display for CompareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompareError::GeometryMismatch { reference, rendered } => write!(
                f,
                "geometry differs: the reference frame holds {reference} pixels where the rendered \
                 frame holds {rendered}; comparing the overlap would report a number that means \
                 nothing"
            ),
            CompareError::NoFrameWithinWindow {
                index,
                window,
                nearest,
                rendered_frames,
            } => {
                let nearest = match nearest {
                    Some(nearest) => format!(
                        "the nearest rendered index is {nearest}, {} frames away, and ",
                        nearest.abs_diff(*index)
                    ),
                    None => String::new(),
                };
                write!(
                    f,
                    "reference frame {index} has no rendered frame within ±{window} of it that no \
                     earlier reference frame has taken: {nearest}the rendered side holds \
                     {rendered_frames} frames. Refusing to pair a frame from further away, which \
                     would report a number that means nothing"
                )
            }
            CompareError::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for CompareError {}

/// How far apart one pair of frames is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameDiff {
    /// Pixels whose value differs at all. For an indexed pair that is one
    /// index; for an RGB pair, any of the three channels.
    pub differing_pixels: u64,
    pub total_pixels: u64,
    pub differing_percent: f32,
    /// The largest single-value difference anywhere in the frame — an index
    /// difference for an indexed pair, a channel difference for an RGB one.
    pub max_channel_delta: u8,
}

/// Compare two indexed frames, byte for byte.
///
/// Both slices must be the same length; a mismatch is a hard error, not a
/// truncated comparison.
pub fn compare_indexed(reference: &[u8], rendered: &[u8]) -> Result<FrameDiff, CompareError> {
    compare_pixels(reference, rendered, 1)
}

/// Compare two RGB8 frames, one output pixel per source pixel.
///
/// Used only when one side has no indices: the emulator widens the DAC with
/// `(c*259+33)>>6` and the port adopted the same rule (Task 11's ruling), so
/// both sides widen identically and a one-step channel difference is a real
/// difference. No tolerance is applied — a tolerance would be a claim the
/// manifests cannot support.
pub fn compare_rgb(reference: &[u8], rendered: &[u8]) -> Result<FrameDiff, CompareError> {
    compare_pixels(reference, rendered, 3)
}

/// `stride` bytes per pixel: 1 for indices, 3 for RGB8.
fn compare_pixels(reference: &[u8], rendered: &[u8], stride: usize) -> Result<FrameDiff, CompareError> {
    if reference.len() % stride != 0 || rendered.len() % stride != 0 {
        return Err(CompareError::Io(format!(
            "a frame of {} and {} bytes is not a whole number of {stride}-byte pixels",
            reference.len(),
            rendered.len()
        )));
    }
    let reference_pixels = reference.len() / stride;
    let rendered_pixels = rendered.len() / stride;
    if reference_pixels != rendered_pixels {
        return Err(CompareError::GeometryMismatch {
            reference: reference_pixels,
            rendered: rendered_pixels,
        });
    }

    let (differing, max_channel_delta) = differences(reference, rendered, stride);
    let differing_pixels = differing.iter().filter(|differs| **differs).count() as u64;
    let total_pixels = reference_pixels as u64;
    Ok(FrameDiff {
        differing_pixels,
        total_pixels,
        differing_percent: percent(differing_pixels, total_pixels),
        max_channel_delta,
    })
}

/// One walk over two frames of `stride`-byte pixels: which pixels differ, and
/// the widest single-value difference anywhere in them.
///
/// The counts and the diff image are both built from this walk, so the number
/// and the picture cannot disagree about where the difference is.
fn differences(reference: &[u8], rendered: &[u8], stride: usize) -> (Vec<bool>, u8) {
    let mut max_channel_delta = 0u8;
    let mut differing = Vec::with_capacity(reference.len() / stride);
    for (reference_pixel, rendered_pixel) in reference
        .chunks_exact(stride)
        .zip(rendered.chunks_exact(stride))
    {
        let mut differs = false;
        for (a, b) in reference_pixel.iter().zip(rendered_pixel) {
            let delta = a.abs_diff(*b);
            max_channel_delta = max_channel_delta.max(delta);
            differs |= delta != 0;
        }
        differing.push(differs);
    }
    (differing, max_channel_delta)
}

fn percent(differing: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        differing as f32 * 100.0 / total as f32
    }
}

/// Pair each reference frame with the rendered frame that answers for it.
///
/// Frames pair by the index each one carries, never by position in the list,
/// and a rendered frame may sit up to `ALIGNMENT_WINDOW` frames either side of
/// the reference index it answers for — the oracle's animation phase at one
/// boot index is not reproducible across sessions (B1/D17), so the alignment is
/// searched rather than assumed (D18).
///
/// For each reference index, in ascending order, the candidates are the unused
/// rendered frames actually present at that index, one below and one above.
/// `difference` scores a candidate — the differing-pixel count between the two
/// pictures — and the lowest score wins, so the pairing follows the picture and
/// a report that has to fail shows the closest thing rather than an arbitrary
/// neighbour. Equal scores go to the smaller offset and then to the lower
/// index, which is what makes an exact index win whenever it is as good a match
/// as its neighbours: on a still screen several candidates match, and the
/// report must say offset 0 rather than flip a coin.
///
/// A rendered frame answers one reference frame only: the pairs are resolved
/// greedily in reference-index order, which is what makes a capture holding two
/// adjacent indices unambiguous. A reference index with no candidate at all is
/// refused — the window widens the alignment, not the comparison. The rendered
/// side may hold frames the reference does not (the port's dump holds every
/// tick where a capture holds a handful of keyframes); it is the reference's
/// indices that must all be answered.
pub fn pair_indices<F>(
    reference: &[u32],
    rendered: &[u32],
    difference: F,
) -> Result<Vec<(u32, u32)>, CompareError>
where
    F: Fn(u32, u32) -> Result<u64, CompareError>,
{
    let mut reference: Vec<u32> = reference.to_vec();
    let mut rendered: Vec<u32> = rendered.to_vec();
    reference.sort_unstable();
    rendered.sort_unstable();

    let mut taken = vec![false; rendered.len()];
    let mut pairs = Vec::with_capacity(reference.len());
    for index in reference {
        // (score, index distance, position in the sorted list): the distance
        // prefers offset 0 over a neighbour, and the position breaks a tie
        // between −1 and +1 by taking the lower index.
        let mut best: Option<(u64, u32, usize)> = None;
        for (position, candidate) in rendered.iter().enumerate() {
            if taken[position] {
                continue;
            }
            let distance = candidate.abs_diff(index);
            if distance > ALIGNMENT_WINDOW {
                continue;
            }
            let rank = (difference(index, *candidate)?, distance, position);
            if best.is_none_or(|best| rank < best) {
                best = Some(rank);
            }
        }
        let Some((_, _, position)) = best else {
            let nearest = rendered
                .iter()
                .enumerate()
                .filter(|(position, _)| !taken[*position])
                .map(|(_, candidate)| *candidate)
                .min_by_key(|candidate| candidate.abs_diff(index));
            return Err(CompareError::NoFrameWithinWindow {
                index,
                window: ALIGNMENT_WINDOW,
                nearest,
                rendered_frames: rendered.len(),
            });
        };
        taken[position] = true;
        pairs.push((index, rendered[position]));
    }
    Ok(pairs)
}

/// `rendered_index - reference_index`: signed, so the direction is in the
/// report and not only the distance. Every pair this tool makes is 0 or ±1 (the
/// window), but it is computed as a difference rather than a pair of casts, so
/// an index too large for `i32` cannot wrap into a plausible-looking offset.
fn frame_offset(reference_index: u32, rendered_index: u32) -> i32 {
    let distance = i32::try_from(reference_index.abs_diff(rendered_index)).unwrap_or(i32::MAX);
    if rendered_index < reference_index {
        -distance
    } else {
        distance
    }
}

/// `identical` only when every frame matched; one differing frame is enough.
pub fn verdict(percents: &[f32]) -> &'static str {
    if percents.iter().all(|percent| *percent == 0.0) {
        "identical"
    } else {
        "different"
    }
}

// --- the two manifests ----------------------------------------------------

/// The fields both manifests share (Ruling 2). Everything else — the
/// capture's recipe, the dump's part name — is ignored.
#[derive(Debug, Deserialize)]
struct Manifest {
    version: u32,
    width: u16,
    height: u16,
    frames: Vec<FrameRef>,
}

/// One frame on either side.
///
/// `rgb` is on both sides and always a file. `idx` is present only when that
/// side produced indices — a capture whose coherence check failed writes none
/// (D15) — and the port's dump carries each frame's own geometry, because
/// `Engine::tick` resizes the frame from the view and a part can switch to
/// double-scan mid-run.
#[derive(Debug, Deserialize)]
struct FrameRef {
    index: u32,
    #[serde(default)]
    idx: Option<String>,
    rgb: String,
    #[serde(default)]
    width: Option<u16>,
    #[serde(default)]
    height: Option<u16>,
}

impl FrameRef {
    /// This frame's geometry: its own when it carries one, the manifest's
    /// otherwise.
    fn geometry(&self, manifest: &Manifest) -> (u16, u16) {
        (
            self.width.unwrap_or(manifest.width),
            self.height.unwrap_or(manifest.height),
        )
    }
}

/// One capture on disk: a manifest and the frame files it names.
#[derive(Debug)]
struct Capture {
    /// Empty for a directory that holds `manifest.json` itself; otherwise the
    /// capture directory's path below the tree's root, `/`-separated —
    /// `title`, `plzpart/plasma`.
    name: String,
    dir: PathBuf,
    manifest: Manifest,
}

/// Find every capture `dir` holds, at any depth.
///
/// A directory with its own `manifest.json` is a capture, and so is every
/// directory below it that holds one, named by its path relative to `dir`.
/// `reference/check_a` is two captures directly inside it: the demo changes
/// display geometry between the title screen and the part after it, one
/// manifest carries one geometry, so Task 12 captured one screen at a time.
/// `reference/parts` is the shape that made this a walk of the *whole* tree:
/// eighteen of its captures are immediate subdirectories and two parts —
/// `plzpart` and `techno`, the two whose screens have two geometries each —
/// hold theirs one level deeper.
///
/// **The walk goes all the way down because measuring less than was asked for
/// is how a green run lies.** A scan of the first level only found 18 of
/// `parts/`'s 22 captures — skipping both CRTC-writing parts and every one of
/// the keyframes that have no settled content — and still printed `verdict:
/// identical` with exit 0. Nothing below the root is skipped here, and a
/// rendered side laid out differently is refused by [`pair_captures`] (the
/// names have to match) rather than reduced to whatever the two sides happen to
/// share.
fn resolve_captures(dir: &Path) -> Result<Vec<Capture>, CompareError> {
    if !dir.is_dir() {
        return Err(CompareError::Io(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    let mut captures = Vec::new();
    collect_captures(dir, dir, &mut captures)?;
    captures.sort_by(|a, b| a.name.cmp(&b.name));
    if captures.is_empty() {
        return Err(CompareError::Io(format!(
            "{} holds no {MANIFEST_FILE} (looked in it and in every subdirectory)",
            dir.display()
        )));
    }
    Ok(captures)
}

/// Every capture at or below `dir`, named by its path relative to `root`.
///
/// The walk does **not** stop at a capture: a manifest inside a capture's own
/// directory is a capture too, and stopping there is the skipped-capture
/// failure this walk exists to prevent. Subdirectories are visited in sorted
/// order so a tree is walked the same way however the filesystem lists it.
fn collect_captures(
    root: &Path,
    dir: &Path,
    captures: &mut Vec<Capture>,
) -> Result<(), CompareError> {
    if dir.join(MANIFEST_FILE).is_file() {
        captures.push(Capture {
            name: capture_name(root, dir),
            dir: dir.to_path_buf(),
            manifest: read_manifest(dir)?,
        });
    }
    let entries =
        fs::read_dir(dir).map_err(|e| CompareError::Io(format!("{}: {e}", dir.display())))?;
    let mut subdirectories = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| CompareError::Io(format!("{}: {e}", dir.display())))?;
        let path = entry.path();
        if path.is_dir() {
            subdirectories.push(path);
        }
    }
    subdirectories.sort();
    for subdirectory in &subdirectories {
        collect_captures(root, subdirectory, captures)?;
    }
    Ok(())
}

/// A capture's name: its path below `root`, `/`-separated, empty for the root
/// itself. The separator is `/` on every platform so a name from one tree can
/// be matched, and a report written, under another.
fn capture_name(root: &Path, dir: &Path) -> String {
    dir.strip_prefix(root)
        .unwrap_or(Path::new(""))
        .components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn read_manifest(dir: &Path) -> Result<Manifest, CompareError> {
    let path = dir.join(MANIFEST_FILE);
    let json = fs::read_to_string(&path)
        .map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    let manifest: Manifest = serde_json::from_str(&json)
        .map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    if manifest.version != MANIFEST_VERSION {
        return Err(CompareError::Io(format!(
            "{} says version {}; this tool reads version {MANIFEST_VERSION}",
            path.display(),
            manifest.version
        )));
    }
    Ok(manifest)
}

/// Pair the two sides' captures by name. Every capture on either side must
/// have a counterpart: an unmatched name is said out loud rather than skipped,
/// because silently measuring less than was asked for is how a green run lies.
fn pair_captures<'a>(
    reference: &'a [Capture],
    rendered: &'a [Capture],
) -> Result<Vec<(&'a Capture, &'a Capture)>, CompareError> {
    let mut pairs = Vec::with_capacity(reference.len());
    let mut missing = Vec::new();
    for reference_capture in reference {
        match rendered.iter().find(|c| c.name == reference_capture.name) {
            Some(rendered_capture) => pairs.push((reference_capture, rendered_capture)),
            None => missing.push(format!("{:?}", reference_capture.name)),
        }
    }
    if !missing.is_empty() {
        return Err(CompareError::Io(format!(
            "the rendered side holds no capture named {} (it holds {})",
            missing.join(", "),
            names(rendered)
        )));
    }

    let extra: Vec<String> = rendered
        .iter()
        .filter(|c| !reference.iter().any(|r| r.name == c.name))
        .map(|c| format!("{:?}", c.name))
        .collect();
    if !extra.is_empty() {
        return Err(CompareError::Io(format!(
            "the rendered side holds {} where the reference holds {}; a capture with no reference \
             cannot be measured",
            extra.join(", "),
            names(reference)
        )));
    }
    Ok(pairs)
}

fn names(captures: &[Capture]) -> String {
    if captures.is_empty() {
        return "nothing".to_string();
    }
    captures
        .iter()
        .map(|c| format!("{:?}", c.name))
        .collect::<Vec<_>>()
        .join(", ")
}

// --- comparing whole captures ---------------------------------------------

/// The frames of both sides, paired by manifest index within the alignment
/// window, the picture deciding between a reference frame's candidates.
///
/// The pairing needs the pixels — the candidate that matches is the one the
/// report should be about — so this reads the frames it is deciding between.
/// The chosen pair is then compared again by `compare_capture`, which is where
/// the numbers in the report come from; neither the score nor the choice is
/// carried over, so the report and the pairing cannot disagree about what was
/// compared.
fn paired_frames<'a>(
    reference: &'a Capture,
    rendered: &'a Capture,
) -> Result<Vec<(&'a FrameRef, &'a FrameRef)>, CompareError> {
    let mut reference_frames: Vec<&FrameRef> = reference.manifest.frames.iter().collect();
    let mut rendered_frames: Vec<&FrameRef> = rendered.manifest.frames.iter().collect();
    reference_frames.sort_by_key(|f| f.index);
    rendered_frames.sort_by_key(|f| f.index);
    let reference_indices: Vec<u32> = reference_frames.iter().map(|f| f.index).collect();
    let rendered_indices: Vec<u32> = rendered_frames.iter().map(|f| f.index).collect();

    let pairs = pair_indices(
        &reference_indices,
        &rendered_indices,
        |reference_index, rendered_index| {
            let reference_frame = frame_with(&reference_frames, reference_index)?;
            let rendered_frame = frame_with(&rendered_frames, rendered_index)?;
            // A frame of another geometry cannot stand in for this one, so it
            // ranks below every candidate that can. If it is the only one it
            // is chosen, and the geometry check refuses it with both sizes
            // named instead of the report carrying a meaningless number.
            if reference_frame.geometry(&reference.manifest)
                != rendered_frame.geometry(&rendered.manifest)
            {
                return Ok(INCOMPARABLE);
            }
            candidate_difference(reference, reference_frame, rendered, rendered_frame)
        },
    )?;

    // Back to the frames themselves: each pair names a reference index and the
    // rendered index it was matched with, and each rendered frame is named
    // once, so a side that lists the same index twice is consumed in order.
    let mut reference_taken = vec![false; reference_frames.len()];
    let mut rendered_taken = vec![false; rendered_frames.len()];
    let mut paired = Vec::with_capacity(pairs.len());
    for (reference_index, rendered_index) in pairs {
        paired.push((
            take_frame(&reference_frames, &mut reference_taken, reference_index)?,
            take_frame(&rendered_frames, &mut rendered_taken, rendered_index)?,
        ));
    }
    Ok(paired)
}

/// The first frame carrying `index` that no earlier pair has taken.
fn take_frame<'a>(
    frames: &[&'a FrameRef],
    taken: &mut [bool],
    index: u32,
) -> Result<&'a FrameRef, CompareError> {
    for (position, frame) in frames.iter().enumerate() {
        if frame.index == index && !taken[position] {
            taken[position] = true;
            return Ok(frame);
        }
    }
    Err(CompareError::Io(format!(
        "the pairing named a frame at index {index} that the manifest does not hold"
    )))
}

/// The frame carrying `index`, for scoring a candidate. A manifest that names
/// one index twice is not distinguished here; the pairing takes them in order.
fn frame_with<'a>(frames: &[&'a FrameRef], index: u32) -> Result<&'a FrameRef, CompareError> {
    frames
        .iter()
        .find(|frame| frame.index == index)
        .copied()
        .ok_or_else(|| {
            CompareError::Io(format!(
                "the pairing named a frame at index {index} that the manifest does not hold"
            ))
        })
}

/// How far apart two frames are, by the rule the report will use for them:
/// indices when both sides have them, RGB otherwise (D15's fallback).
fn candidate_difference(
    reference: &Capture,
    reference_frame: &FrameRef,
    rendered: &Capture,
    rendered_frame: &FrameRef,
) -> Result<u64, CompareError> {
    let difference = match (&reference_frame.idx, &rendered_frame.idx) {
        (Some(reference_idx), Some(rendered_idx)) => compare_indexed(
            &read_frame(&reference.dir, reference_idx)?,
            &read_frame(&rendered.dir, rendered_idx)?,
        )?,
        _ => compare_rgb(
            &read_frame(&reference.dir, &reference_frame.rgb)?,
            &read_frame(&rendered.dir, &rendered_frame.rgb)?,
        )?,
    };
    Ok(difference.differing_pixels)
}

/// One capture's report, already written to disk.
struct CaptureOutcome {
    report: Report,
    path: PathBuf,
}

#[derive(Debug, Serialize)]
struct Report {
    version: u32,
    /// `indexed`, `rgb`, or `mixed` when one side had indices for some frames
    /// only. The indexed path is used whenever both sides have indices.
    mode: &'static str,
    frames: Vec<FrameReport>,
    summary: Summary,
}

#[derive(Debug, Serialize)]
struct FrameReport {
    /// The reference frame's manifest index.
    index: u32,
    /// The rendered frame it was paired with. The searches pair by the picture,
    /// so this is the index that answered for the reference frame, not
    /// necessarily the reference frame's own.
    rendered_index: u32,
    /// `rendered_index - index`, spelled out rather than left for a reader to
    /// notice by subtraction: 0 for an exact pairing, ±1 when the oracle's
    /// animation phase put the counterpart a frame either side (D18).
    offset: i32,
    differing_pixels: u64,
    total_pixels: u64,
    differing_percent: f32,
    max_channel_delta: u8,
    /// The diff image beside this report: differing pixels in magenta over a
    /// dimmed copy of the reference.
    diff_image: String,
}

#[derive(Debug, Serialize)]
struct Summary {
    worst_percent: f32,
    mean_percent: f32,
    verdict: &'static str,
    /// How many frames were paired at a non-zero offset.
    offset_frames: usize,
    /// Every offset used and how many frames used it: `{"-1": 1, "0": 2}`.
    /// Scattered non-zero offsets are the oracle's ambiguity; one constant
    /// non-zero offset is a finding about the port (D18), and the distribution
    /// is what tells them apart.
    offsets: BTreeMap<i32, usize>,
}

fn summarise(frames: &[FrameReport]) -> Summary {
    let percents: Vec<f32> = frames.iter().map(|frame| frame.differing_percent).collect();
    let worst_percent = percents.iter().copied().fold(0.0f32, f32::max);
    let mean_percent = if percents.is_empty() {
        0.0
    } else {
        percents.iter().sum::<f32>() / percents.len() as f32
    };
    let mut offsets: BTreeMap<i32, usize> = BTreeMap::new();
    for frame in frames {
        *offsets.entry(frame.offset).or_default() += 1;
    }
    Summary {
        worst_percent,
        mean_percent,
        verdict: verdict(&percents),
        offset_frames: frames.iter().filter(|frame| frame.offset != 0).count(),
        offsets,
    }
}

/// `+1 x2, -1 x1` for the console line: the frames paired at a non-zero offset,
/// by offset. Empty when every frame was exact.
fn offsets_used(offsets: &BTreeMap<i32, usize>) -> String {
    offsets
        .iter()
        .filter(|(offset, _)| **offset != 0)
        .map(|(offset, frames)| format!("{offset:+} x{frames}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Make `out` just before something goes in it.
///
/// A comparison that refuses must leave nothing behind — not even an empty
/// directory, which a script would have to open to find out that the comparison
/// never ran and the exit code, not the tree, is the answer it needs.
fn prepare_out(out: &Path) -> Result<(), CompareError> {
    if out.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(out).map_err(|e| CompareError::Io(format!("{}: {e}", out.display())))
}

/// Compare every frame of one capture pair, write its report and one diff
/// image per frame, and hand back what was written.
fn compare_capture(
    reference: &Capture,
    rendered: &Capture,
    out: &Path,
) -> Result<CaptureOutcome, CompareError> {
    let mut frames = Vec::new();
    let mut modes = Vec::new();
    for (reference_frame, rendered_frame) in paired_frames(reference, rendered)? {
        let reference_geometry = reference_frame.geometry(&reference.manifest);
        let rendered_geometry = rendered_frame.geometry(&rendered.manifest);
        if reference_geometry != rendered_geometry {
            return Err(CompareError::GeometryMismatch {
                reference: reference_geometry.0 as usize * reference_geometry.1 as usize,
                rendered: rendered_geometry.0 as usize * rendered_geometry.1 as usize,
            });
        }
        let (width, height) = reference_geometry;
        let expected_rgb = width as usize * height as usize * 3;

        // The colour frame is always there, and it is what the diff image
        // draws from — even when the comparison itself is by index.
        let reference_rgb = read_frame(&reference.dir, &reference_frame.rgb)?;
        if reference_rgb.len() != expected_rgb {
            return Err(CompareError::Io(format!(
                "{} holds {} bytes; {width}x{height} RGB8 needs {expected_rgb}",
                reference.dir.join(&reference_frame.rgb).display(),
                reference_rgb.len()
            )));
        }

        let (diff, mask) = match (&reference_frame.idx, &rendered_frame.idx) {
            // Indexed when *both* sides have indices: the stricter comparison,
            // and the one the DAC widening rule cannot affect. Either side
            // missing them (a capture whose coherence check failed, D15) puts
            // both on the RGB path.
            (Some(reference_idx), Some(rendered_idx)) => {
                let reference_bytes = read_frame(&reference.dir, reference_idx)?;
                let rendered_bytes = read_frame(&rendered.dir, rendered_idx)?;
                let expected_idx = width as usize * height as usize;
                for (file, bytes) in [
                    (reference.dir.join(reference_idx), &reference_bytes),
                    (rendered.dir.join(rendered_idx), &rendered_bytes),
                ] {
                    if bytes.len() != expected_idx {
                        return Err(CompareError::Io(format!(
                            "{} holds {} bytes; {width}x{height} indices need {expected_idx}",
                            file.display(),
                            bytes.len()
                        )));
                    }
                }
                modes.push("indexed");
                let diff = compare_indexed(&reference_bytes, &rendered_bytes)?;
                // Where the difference is, from the same single-pass walk the
                // numbers are counted from: the number and the picture cannot
                // disagree about the rule.
                let (mask, _) = differences(&reference_bytes, &rendered_bytes, 1);
                (diff, mask)
            }
            _ => {
                let rendered_rgb = read_frame(&rendered.dir, &rendered_frame.rgb)?;
                modes.push("rgb");
                let diff = compare_rgb(&reference_rgb, &rendered_rgb)?;
                let (mask, _) = differences(&reference_rgb, &rendered_rgb, 3);
                (diff, mask)
            }
        };

        let diff_image = format!("diff_{:06}.png", reference_frame.index);
        prepare_out(out)?;
        write_diff_image(&out.join(&diff_image), &reference_rgb, &mask, width, height)?;

        frames.push(FrameReport {
            index: reference_frame.index,
            rendered_index: rendered_frame.index,
            offset: frame_offset(reference_frame.index, rendered_frame.index),
            differing_pixels: diff.differing_pixels,
            total_pixels: diff.total_pixels,
            differing_percent: diff.differing_percent,
            max_channel_delta: diff.max_channel_delta,
            diff_image,
        });
    }

    let mode = match (
        modes.iter().all(|mode| *mode == "indexed"),
        modes.iter().any(|mode| *mode == "indexed"),
    ) {
        (true, _) => "indexed",
        (false, true) => "mixed",
        (false, false) => "rgb",
    };
    let summary = summarise(&frames);
    let report = Report {
        version: REPORT_VERSION,
        mode,
        frames,
        summary,
    };
    prepare_out(out)?;
    let path = out.join(REPORT_FILE);
    let mut json = serde_json::to_string_pretty(&report)
        .map_err(|e| CompareError::Io(format!("cannot serialise {}: {e}", path.display())))?;
    json.push('\n');
    fs::write(&path, json).map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    Ok(CaptureOutcome { report, path })
}

fn read_frame(dir: &Path, file: &str) -> Result<Vec<u8>, CompareError> {
    let path = dir.join(file);
    fs::read(&path).map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))
}

/// Differing pixels in magenta over a dimmed copy of the reference, one output
/// pixel per source pixel.
fn write_diff_image(
    path: &Path,
    reference_rgb: &[u8],
    differing: &[bool],
    width: u16,
    height: u16,
) -> Result<(), CompareError> {
    const MAGENTA: [u8; 3] = [255, 0, 255];
    let mut image = Vec::with_capacity(reference_rgb.len());
    for (pixel, differs) in reference_rgb.chunks_exact(3).zip(differing) {
        if *differs {
            image.extend_from_slice(&MAGENTA);
        } else {
            // Half brightness, so the magenta reads as the difference rather
            // than as picture.
            image.extend(pixel.iter().map(|channel| channel / 2));
        }
    }

    let file =
        fs::File::create(path).map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), u32::from(width), u32::from(height));
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    writer
        .write_image_data(&image)
        .map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    writer
        .finish()
        .map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))?;
    Ok(())
}

/// The name a capture's lines are prefixed with: its own — `title`, `part` —
/// or nothing for a directory that is one capture, whose frame lines start
/// with `frame`.
fn capture_prefix(name: &str) -> String {
    if name.is_empty() {
        String::new()
    } else {
        format!("{name} ")
    }
}

/// Where one capture's report and diff images go.
fn capture_out(out: &Path, name: &str) -> PathBuf {
    if name.is_empty() {
        out.to_path_buf()
    } else {
        out.join(name)
    }
}

/// Where the reports go when `--out` is not given. Gitignored, and it cannot
/// collide with a `--dump-dir`, which the caller names explicitly.
const DEFAULT_OUT: &str = "out/compare";

/// The report directory for a run: `--out` if it was given, the default
/// otherwise.
fn output_root(out: Option<&Path>) -> PathBuf {
    out.map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT))
}

/// `path` with symlinks, `.` and `..` resolved as far as it exists: the
/// deepest ancestor that exists is canonicalised, the rest appended, and the
/// whole of it collapsed lexically.
///
/// The collapse is what closes the `..` that sits behind a component that does
/// not exist yet. `fs::canonicalize` refuses such a path — it resolves through
/// the filesystem, and `missing/..` is not a directory because `missing` is not
/// there — so `<parent>/missing/../ref/nested` used to come back with the
/// `missing/..` still in it, which is inside `<parent>/ref` for every write and
/// outside it for every string comparison. The components after the deepest
/// existing ancestor cannot be symlinks (they do not exist), so collapsing them
/// lexically is what the filesystem does with them when it comes to create the
/// path.
///
/// Shared with the other guards over what this tool writes, because a guard
/// that only compares strings is a guard a trailing `/.` walks through.
pub(crate) fn resolve(path: &Path) -> Result<PathBuf, String> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map_err(|e| format!("cannot read the working directory: {e}"))?
            .join(path)
    };
    for ancestor in absolute.ancestors() {
        if let Ok(resolved) = fs::canonicalize(ancestor) {
            let rest = absolute.strip_prefix(ancestor).unwrap_or(Path::new(""));
            return Ok(collapse(&resolved.join(rest)));
        }
    }
    Ok(collapse(&absolute))
}

/// `path` without its `.` components and with every `..` taken back off the
/// component before it. Purely lexical — no filesystem is read — which is what
/// makes it usable on a path that does not exist.
fn collapse(path: &Path) -> PathBuf {
    let absolute = path.is_absolute();
    let mut collapsed = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                // Nothing left to take it off: `/..` is `/`, and a leading
                // `..` on a relative path has to stay one.
                if !collapsed.pop() && !absolute {
                    collapsed.push(Component::ParentDir.as_os_str());
                }
            }
            component => collapsed.push(component.as_os_str()),
        }
    }
    collapsed
}

/// Refuse to write `out` inside `input`: the inputs are read, never written, and
/// a report or a sheet landing among them would dirty the bytes every later
/// check is measured against. Both paths are resolved first, so a relative
/// path, a trailing slash or a `..` cannot slip past.
///
/// `flag` is the option to pass somewhere else, `why` says what would be
/// damaged, and `elsewhere` tells the caller where to put it instead — the
/// message has to be usable without reading this source.
pub(crate) fn refuse_output_inside(
    out: &Path,
    input: &Path,
    flag: &str,
    why: &str,
    elsewhere: &str,
) -> Result<(), String> {
    let out = resolve(out)?;
    let input = resolve(input)?;
    if out.starts_with(&input) {
        return Err(format!(
            "{flag} {} is inside the input tree {}: {why}. {elsewhere}",
            out.display(),
            input.display()
        ));
    }
    Ok(())
}

/// The reference directory is input-only: a report written among the captures
/// would dirty the bytes every later diff is measured against.
fn refuse_out_inside_reference(out: &Path, reference: &Path) -> Result<(), CompareError> {
    refuse_output_inside(
        out,
        reference,
        "--out",
        "the reference captures are inputs, not outputs, and a report written among them would \
         dirty the frames every later diff is measured against",
        &format!("Pass --out somewhere else; the default is {DEFAULT_OUT}"),
    )
    .map_err(CompareError::Io)
}

/// `cargo xtask compare`: optionally run a command to produce the rendered
/// frames, diff them against the reference, print and write the report.
///
/// Returns the exit code the process should end with: 0 when every frame of
/// every capture is identical, 1 when any differs. A comparison that cannot
/// run at all is an `Err`, and `main` gives that its own exit code, so a
/// script can tell "the frames differ" from "the tool could not tell".
pub fn command(
    reference: &Path,
    rendered: Option<&Path>,
    run: Option<&str>,
    out: Option<&Path>,
) -> Result<ExitCode, CompareError> {
    if run.is_some() && rendered.is_none() {
        return Err(CompareError::Io(
            "--run needs --rendered: the command has to leave its frames where the comparison can \
             find them, and comparing the reference with itself would say nothing"
                .into(),
        ));
    }
    let out = output_root(out);
    // Before anything is run or written: the harness must not write into its
    // own inputs.
    refuse_out_inside_reference(&out, reference)?;
    if let Some(command) = run {
        // Through `sh -c`, so it can be a whole command line rather than a
        // single path.
        let status = Command::new("sh")
            .arg("-c")
            .arg(command)
            .status()
            .map_err(|e| CompareError::Io(format!("cannot run {command:?}: {e}")))?;
        if !status.success() {
            return Err(CompareError::Io(format!(
                "--run exited with {status}: {command:?}"
            )));
        }
    }

    let rendered = rendered.unwrap_or(reference);
    let reference_captures = resolve_captures(reference)?;
    let rendered_captures = resolve_captures(rendered)?;

    let mut identical = true;
    let pairs = pair_captures(&reference_captures, &rendered_captures)?;
    for (reference_capture, rendered_capture) in &pairs {
        let capture_out = capture_out(&out, &reference_capture.name);
        let outcome = compare_capture(reference_capture, rendered_capture, &capture_out)?;
        for frame in &outcome.report.frames {
            let prefix = format!("{}frame", capture_prefix(&reference_capture.name));
            // A frame paired at a non-zero offset says so on its own line: an
            // offset is the oracle's ambiguity being absorbed, and it must be
            // visible where the number is, not worked out from two indices.
            let alignment = if frame.offset == 0 {
                String::new()
            } else {
                format!(
                    ", rendered index {} (offset {:+})",
                    frame.rendered_index, frame.offset
                )
            };
            println!(
                "{prefix} {}: {:.4}% differing ({}/{} px), max delta {}{alignment}",
                frame.index,
                frame.differing_percent,
                frame.differing_pixels,
                frame.total_pixels,
                frame.max_channel_delta
            );
        }
        if outcome.report.summary.offset_frames > 0 {
            println!(
                "{}alignment: {} of {} frames paired at a non-zero offset ({})",
                capture_prefix(&reference_capture.name),
                outcome.report.summary.offset_frames,
                outcome.report.frames.len(),
                offsets_used(&outcome.report.summary.offsets)
            );
        }
        println!("report: {}", outcome.path.display());
        identical &= outcome.report.summary.verdict == "identical";
    }

    // How many captures were measured is printed with the verdict: a run that
    // measured less than it was asked for has to be visible where the green
    // result is, not worked out from the frame lines above it.
    println!("captures: {} measured", pairs.len());
    let verdict = if identical { "identical" } else { "different" };
    println!("verdict: {verdict}");
    Ok(if identical {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};
    use std::process::ExitCode;

    // --- pixel comparison -------------------------------------------------

    #[test]
    fn identical_indexed_frames_report_zero_difference() {
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 2, 3, 4]).unwrap();
        assert_eq!(r.differing_pixels, 0);
        assert_eq!(r.differing_percent, 0.0);
        assert_eq!(r.max_channel_delta, 0);
        assert_eq!(r.total_pixels, 4);
    }

    #[test]
    fn a_single_differing_index_is_counted_once() {
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 2, 9, 4]).unwrap();
        assert_eq!(r.differing_pixels, 1);
        // Also in the brief's plan is `max_channel_delta == 7` for this fixture.
        // That cannot be: the one differing pixel is 3 against 9, a delta of 6.
        // The delta the brief asks for is pinned by the fixture below.
        assert_eq!(r.max_channel_delta, 6);
        assert_eq!(r.differing_percent, 25.0);
    }

    #[test]
    fn the_widest_differing_index_sets_max_channel_delta() {
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 9, 3, 4]).unwrap();
        assert_eq!(r.differing_pixels, 1);
        assert_eq!(r.max_channel_delta, 7);
        assert_eq!(r.differing_percent, 25.0);
    }

    #[test]
    fn an_index_difference_is_counted_even_when_the_rgb_matches() {
        // Two palette entries may widen to the same RGB. The indices still
        // differ, and the index comparison is the stricter one.
        let r = compare_indexed(&[5], &[6]).unwrap();
        assert_eq!(r.differing_pixels, 1);
    }

    #[test]
    fn mismatched_geometry_is_an_error_not_a_partial_diff() {
        assert!(matches!(
            compare_indexed(&[1, 2, 3, 4], &[1, 2]),
            Err(CompareError::GeometryMismatch { .. })
        ));
    }

    #[test]
    fn identical_rgb_frames_report_zero_difference() {
        let r = compare_rgb(&[1, 2, 3, 4, 5, 6], &[1, 2, 3, 4, 5, 6]).unwrap();
        assert_eq!(r.differing_pixels, 0);
        assert_eq!(r.total_pixels, 2);
        assert_eq!(r.max_channel_delta, 0);
    }

    #[test]
    fn an_rgb_pixel_counts_once_however_many_channels_differ() {
        // One pixel differs in all three channels; the second is equal.
        let r = compare_rgb(&[0, 0, 0, 10, 10, 10], &[1, 2, 3, 10, 10, 10]).unwrap();
        assert_eq!(r.differing_pixels, 1);
        assert_eq!(r.total_pixels, 2);
        assert_eq!(r.max_channel_delta, 3);
        assert_eq!(r.differing_percent, 50.0);
    }

    #[test]
    fn mismatched_rgb_geometry_is_an_error_not_a_partial_diff() {
        assert!(matches!(
            compare_rgb(&[1, 2, 3], &[1, 2, 3, 4, 5, 6]),
            Err(CompareError::GeometryMismatch { .. })
        ));
    }

    #[test]
    fn an_rgb_frame_that_is_not_whole_pixels_is_refused() {
        // A truncated write must be a legible refusal, not a comparison of the
        // pixels that happened to be there.
        let error = compare_rgb(&[1, 2, 3, 4, 5], &[1, 2, 3, 4, 5]).unwrap_err();
        assert!(matches!(error, CompareError::Io(_)), "{error}");
    }

    // --- pairing ----------------------------------------------------------

    /// A scorer for a capture whose pictures are named by their index: two
    /// frames are the same picture only at the same index. The pairing tests
    /// below that are about structure use it; the ones about the search use
    /// real frames through `command`.
    fn same_picture_at_the_same_index(
        reference_index: u32,
        rendered_index: u32,
    ) -> Result<u64, CompareError> {
        Ok(u64::from(reference_index != rendered_index) * 8)
    }

    #[test]
    fn frames_are_paired_by_manifest_index_not_by_position() {
        // The indices are the pairing: two lists of the same size that name
        // *different* frames are refused, not zipped positionally. This is the
        // real shape the name is about — `reference/check_a` at 700/900/1350/
        // 1450 against an older capture at 400/600/1100/1150, which the
        // positional pairing compared frame for frame and printed a percentage
        // for.
        let error =
            pair_indices(&[700, 900], &[400, 600], same_picture_at_the_same_index).unwrap_err();
        match error {
            CompareError::NoFrameWithinWindow { index, window, .. } => {
                assert_eq!((index, window), (700, 1));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }

        // Two lists that do name the same frames pair by index, whatever order
        // each manifest happens to list them in.
        assert_eq!(
            pair_indices(&[900, 700], &[700, 900], same_picture_at_the_same_index).unwrap(),
            vec![(700, 700), (900, 900)]
        );
    }

    #[test]
    fn pairing_sorts_by_index_before_it_pairs() {
        // A manifest may list its frames in any order; the pairing is by the
        // index each frame carries, so the report and the diff images line up.
        assert_eq!(
            pair_indices(&[600, 400], &[400, 600], same_picture_at_the_same_index).unwrap(),
            vec![(400, 400), (600, 600)]
        );
        assert_eq!(
            pair_indices(&[20, 10], &[10, 20], same_picture_at_the_same_index).unwrap(),
            vec![(10, 10), (20, 20)]
        );
    }

    #[test]
    fn a_side_that_skipped_frames_is_refused_not_shifted() {
        // A rendered side that cannot supply a reference index must not let the
        // rest of the capture slide over: reference frame 200 has nothing
        // within ±1, and the refusal names it.
        let error = pair_indices(
            &[100, 200, 300],
            &[100, 300],
            same_picture_at_the_same_index,
        )
        .unwrap_err();
        match error {
            CompareError::NoFrameWithinWindow { index, .. } => assert_eq!(index, 200),
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert!(pair_indices(&[100, 700], &[100, 300], same_picture_at_the_same_index).is_err());
    }

    #[test]
    fn a_rendered_frame_is_taken_by_one_reference_frame_only() {
        // Greedy in reference-index order: two adjacent reference frames pair
        // with the two frames that answer them, and a rendered frame that has
        // already answered one is not offered to the next.
        assert_eq!(
            pair_indices(&[900, 901], &[899, 900, 901], |_, _| Ok(0)).unwrap(),
            vec![(900, 900), (901, 901)]
        );
        // One rendered frame cannot answer two reference frames, however well
        // it matches both: the second is refused rather than handed the same
        // frame again.
        let error = pair_indices(&[900, 901], &[901], |_, _| Ok(0)).unwrap_err();
        match error {
            CompareError::NoFrameWithinWindow { index, .. } => assert_eq!(index, 901),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_pairing_follows_the_picture_and_the_exact_index_breaks_a_tie() {
        // Candidates are scored by how far apart their pictures are; an
        // exact-index candidate wins only when it is as close a match as its
        // neighbours, so a still screen — where several candidates are equally
        // exact — reports offset 0 rather than a coin flip (D18).
        let nearest_picture = |reference_index: u32, rendered_index: u32| {
            Ok(match (reference_index, rendered_index) {
                (900, 899) => 3,
                (900, 900) => 2, // the exact index, but a worse match
                (900, 901) => 0, // the picture the reference actually holds
                _ => 100,
            })
        };
        assert_eq!(
            pair_indices(&[900], &[899, 900, 901], nearest_picture).unwrap(),
            vec![(900, 901)]
        );
        // All three hold the same picture: offset 0 wins the tie.
        assert_eq!(
            pair_indices(&[900], &[899, 900, 901], |_, _| Ok(0)).unwrap(),
            vec![(900, 900)]
        );
        // Two neighbours match equally and offset 0 is absent: the lower index
        // wins, deterministically.
        assert_eq!(
            pair_indices(&[900], &[899, 901], |_, _| Ok(0)).unwrap(),
            vec![(900, 899)]
        );
    }

    #[test]
    fn the_offset_distribution_lists_the_offsets_that_were_used() {
        // The console line's evidence: which offsets the frames were paired
        // at, with offset 0 left out because it is the case that needs no
        // explanation (D18).
        assert_eq!(offsets_used(&BTreeMap::from([(0, 3)])), "");
        assert_eq!(
            offsets_used(&BTreeMap::from([(-1, 1), (0, 2), (1, 2)])),
            "-1 x1, +1 x2"
        );
    }

    #[test]
    fn the_verdict_is_identical_only_when_every_frame_matches() {
        assert_eq!(verdict(&[0.0, 0.0]), "identical");
        assert_eq!(verdict(&[0.0, 0.01]), "different");
        // No frames is vacuously identical; nothing differed.
        assert_eq!(verdict(&[]), "identical");
    }

    // --- whole captures ---------------------------------------------------

    #[test]
    fn comparing_a_capture_with_itself_is_identical() {
        let dir = scratch("self");
        write_reference_capture(&dir, (4, 2), &[flat(400, 7, 8), flat(600, 9, 8)]);
        let out = scratch("self-out");

        let code = command(&dir, Some(&dir), None, Some(&out)).unwrap();

        assert_eq!(code, ExitCode::SUCCESS);
        let report = read_json(&out.join("report.json"));
        // The report's shape changed with the alignment search (D18), so its
        // version moved with it: a reader that knows version 1 must not read a
        // version-2 report as if frames still paired positionally.
        assert_eq!(report["version"], 2);
        assert_eq!(report["mode"], "indexed");
        assert_eq!(report["summary"]["verdict"], "identical");
        assert_eq!(report["summary"]["worst_percent"], 0.0);
        assert_eq!(report["summary"]["mean_percent"], 0.0);
        assert_eq!(report["summary"]["offset_frames"], 0);
        assert_eq!(report["summary"]["offsets"], serde_json::json!({ "0": 2 }));
        assert_eq!(report["frames"].as_array().unwrap().len(), 2);
        for frame in report["frames"].as_array().unwrap() {
            assert_eq!(frame["differing_pixels"], 0);
            assert_eq!(frame["total_pixels"], 8);
            assert_eq!(frame["max_channel_delta"], 0);
            assert_eq!(frame["differing_percent"], 0.0);
            assert_eq!(frame["offset"], 0);
            let image = out.join(frame["diff_image"].as_str().unwrap());
            assert!(image.is_file(), "{} was not written", image.display());
        }
    }

    /// The requirement the whole search exists for: a frame pair that would be
    /// measured as different is measured as identical because the offset was
    /// searched, and the offset is in the report. The two halves of the test
    /// are the two outcomes: the reference frame against the rendered frame at
    /// its own index (the different pictures), then against a rendered side
    /// that holds its picture one frame later.
    #[test]
    fn a_frame_one_index_later_is_found_and_its_offset_is_reported() {
        // A dense rendered side — the port dumps every tick — where index 900
        // holds a different picture and 901 holds the reference's byte for
        // byte. This is what the oracle's phase does between sessions (B1/D17).
        let reference = scratch("offset-reference");
        let at_its_own_index = scratch("offset-own-index");
        let one_later = scratch("offset-one-later");
        write_reference_capture(&reference, (4, 2), &[flat(900, 3, 8)]);
        write_rendered_capture(&at_its_own_index, "testcard", &[(flat(900, 7, 8), (4, 2))]);
        write_rendered_capture(
            &one_later,
            "testcard",
            &[
                (flat(899, 1, 8), (4, 2)),
                (flat(900, 7, 8), (4, 2)),
                (flat(901, 3, 8), (4, 2)),
            ],
        );

        // The pictures really are different: paired at index 900 this is a
        // 100% difference, so the pair the search finds is not an accident of
        // the fixture.
        let out = scratch("offset-own-index-out");
        assert_eq!(
            command(&reference, Some(&at_its_own_index), None, Some(&out)).unwrap(),
            ExitCode::from(1)
        );
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["frames"][0]["differing_percent"], 100.0);

        let out = scratch("offset-one-later-out");
        assert_eq!(
            command(&reference, Some(&one_later), None, Some(&out)).unwrap(),
            ExitCode::SUCCESS
        );
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["summary"]["verdict"], "identical");
        let frame = &report["frames"][0];
        assert_eq!(frame["index"], 900);
        assert_eq!(frame["rendered_index"], 901);
        assert_eq!(frame["offset"], 1);
        assert_eq!(frame["differing_pixels"], 0);
        assert_eq!(frame["differing_percent"], 0.0);
        assert_eq!(report["summary"]["offset_frames"], 1);
        assert_eq!(report["summary"]["offsets"], serde_json::json!({ "1": 1 }));
    }

    #[test]
    fn a_frame_one_index_earlier_is_found_and_its_offset_is_reported() {
        let reference = scratch("offset-back-reference");
        let one_earlier = scratch("offset-one-earlier");
        write_reference_capture(&reference, (4, 2), &[flat(900, 3, 8)]);
        write_rendered_capture(
            &one_earlier,
            "testcard",
            &[
                (flat(899, 3, 8), (4, 2)),
                (flat(900, 7, 8), (4, 2)),
                (flat(901, 1, 8), (4, 2)),
            ],
        );
        let out = scratch("offset-one-earlier-out");

        assert_eq!(
            command(&reference, Some(&one_earlier), None, Some(&out)).unwrap(),
            ExitCode::SUCCESS
        );
        let report = read_json(&out.join("report.json"));
        let frame = &report["frames"][0];
        assert_eq!(frame["rendered_index"], 899);
        assert_eq!(frame["offset"], -1);
        assert_eq!(frame["differing_percent"], 0.0);
        assert_eq!(report["summary"]["offsets"], serde_json::json!({ "-1": 1 }));
    }

    #[test]
    fn an_exact_index_is_preferred_when_it_matches_as_well_as_its_neighbour() {
        // A still screen: all three rendered frames hold the reference's
        // picture, so the choice is a tie and offset 0 must win it. Anything
        // else would report the oracle's ambiguity where there is none.
        let reference = scratch("offset-tie-reference");
        let rendered = scratch("offset-tie-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(900, 3, 8)]);
        write_rendered_capture(
            &rendered,
            "testcard",
            &[
                (flat(899, 3, 8), (4, 2)),
                (flat(900, 3, 8), (4, 2)),
                (flat(901, 3, 8), (4, 2)),
            ],
        );
        let out = scratch("offset-tie-out");

        assert_eq!(
            command(&reference, Some(&rendered), None, Some(&out)).unwrap(),
            ExitCode::SUCCESS
        );
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["frames"][0]["rendered_index"], 900);
        assert_eq!(report["frames"][0]["offset"], 0);
        assert_eq!(report["summary"]["offset_frames"], 0);
        assert_eq!(report["summary"]["offsets"], serde_json::json!({ "0": 1 }));
    }

    #[test]
    fn the_closest_rendered_frame_is_kept_when_none_matches() {
        // A failing report has to show the nearest picture, not an arbitrary
        // neighbour: 900 differs in two pixels of eight, 901 in five.
        let reference = scratch("closest-reference");
        let rendered = scratch("closest-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(900, 3, 8)]);
        write_rendered_capture(
            &rendered,
            "testcard",
            &[
                (flat_but(900, 3, 2, 8), (4, 2)),
                (flat_but(901, 3, 5, 8), (4, 2)),
            ],
        );
        let out = scratch("closest-out");

        assert_eq!(
            command(&reference, Some(&rendered), None, Some(&out)).unwrap(),
            ExitCode::from(1)
        );
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["summary"]["verdict"], "different");
        let frame = &report["frames"][0];
        assert_eq!(frame["rendered_index"], 900);
        assert_eq!(frame["offset"], 0);
        assert_eq!(frame["differing_pixels"], 2);
        assert_eq!(frame["differing_percent"], 25.0);
    }

    #[test]
    fn a_rendered_frame_answers_one_reference_frame_only() {
        // Two reference frames holding the same picture and one rendered frame
        // holding it: the second reference frame must not take it again, so it
        // is measured against what is left — a difference, said out loud,
        // rather than a second identical result off the same frame.
        let reference = scratch("consume-reference");
        let rendered = scratch("consume-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(900, 3, 8), flat(901, 3, 8)]);
        write_rendered_capture(
            &rendered,
            "testcard",
            &[(flat(900, 7, 8), (4, 2)), (flat(901, 3, 8), (4, 2))],
        );
        let out = scratch("consume-out");

        assert_eq!(
            command(&reference, Some(&rendered), None, Some(&out)).unwrap(),
            ExitCode::from(1)
        );
        let report = read_json(&out.join("report.json"));
        let frames = report["frames"].as_array().unwrap();
        let paired = |frame: &serde_json::Value| {
            (frame["index"].as_u64(), frame["rendered_index"].as_u64())
        };
        assert_eq!(paired(&frames[0]), (Some(900), Some(901)));
        assert_eq!(frames[0]["differing_percent"], 0.0);
        assert_eq!(paired(&frames[1]), (Some(901), Some(900)));
        assert_eq!(frames[1]["offset"], -1);
        assert_eq!(frames[1]["differing_percent"], 100.0);
        assert_eq!(report["summary"]["offsets"], serde_json::json!({ "1": 1, "-1": 1 }));
    }

    #[test]
    fn a_differing_frame_is_reported_as_different_with_a_diff_image() {
        let reference = scratch("diff-reference");
        let rendered = scratch("diff-rendered");
        // 4x2: one pixel differs, so the report says one of eight.
        let mut rgb = flat(0, 4, 8).rgb;
        rgb[..3].copy_from_slice(&[8, 8, 8]);
        write_reference_capture(&reference, (4, 2), &[flat(0, 4, 8)]);
        write_reference_capture(&rendered, (4, 2), &[FrameData { index: 0, idx: None, rgb }]);
        let out = scratch("diff-out");

        let code = command(&reference, Some(&rendered), None, Some(&out)).unwrap();

        assert_eq!(code, ExitCode::from(1));
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["summary"]["verdict"], "different");
        assert_eq!(report["summary"]["worst_percent"], 12.5);
        let frame = &report["frames"][0];
        assert_eq!(frame["differing_pixels"], 1);
        assert_eq!(frame["total_pixels"], 8);
        assert_eq!(frame["differing_percent"], 12.5);

        let image = out.join(frame["diff_image"].as_str().unwrap());
        assert!(image.is_file(), "{} was not written", image.display());
        let (width, height, pixels) = read_png(&image);
        assert_eq!((width, height), (4, 2));
        // The differing pixel is magenta; the rest is a dimmed copy of the
        // reference, so a reader can see where the difference is.
        assert_eq!(&pixels[..3], &[255, 0, 255]);
        assert_eq!(&pixels[3..6], &[2, 2, 2]);
    }

    #[test]
    fn the_indices_decide_when_both_sides_have_them() {
        // Same RGB on both sides, different indices: the stricter comparison
        // is the indexed one, and it must be the one that runs.
        let reference = scratch("indexed-reference");
        let rendered = scratch("indexed-rendered");
        write_reference_capture(
            &reference,
            (4, 2),
            &[FrameData {
                index: 0,
                idx: Some(vec![5; 8]),
                rgb: vec![20; 24],
            }],
        );
        write_reference_capture(
            &rendered,
            (4, 2),
            &[FrameData {
                index: 0,
                idx: Some(vec![6; 8]),
                rgb: vec![20; 24],
            }],
        );
        let out = scratch("indexed-out");

        let code = command(&reference, Some(&rendered), None, Some(&out)).unwrap();

        assert_eq!(code, ExitCode::from(1));
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["mode"], "indexed");
        assert_eq!(report["frames"][0]["differing_pixels"], 8);
    }

    #[test]
    fn the_rgb_frames_are_used_when_the_reference_has_no_indices() {
        // A capture whose coherence check failed writes no .idx (D15). The
        // comparison falls back to RGB rather than refusing.
        let reference = scratch("rgb-reference");
        let rendered = scratch("rgb-rendered");
        write_reference_capture(
            &reference,
            (4, 2),
            &[FrameData {
                index: 0,
                idx: None,
                rgb: vec![20; 24],
            }],
        );
        write_rendered_capture(
            &rendered,
            "testcard",
            &[(FrameData { index: 0, idx: Some(vec![9; 8]), rgb: vec![24; 24] }, (4, 2))],
        );
        let out = scratch("rgb-out");

        let code = command(&reference, Some(&rendered), None, Some(&out)).unwrap();

        assert_eq!(code, ExitCode::from(1));
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["mode"], "rgb");
        assert_eq!(report["frames"][0]["differing_pixels"], 8);
        assert_eq!(report["frames"][0]["max_channel_delta"], 4);
    }

    #[test]
    fn a_geometry_mismatch_between_the_sides_is_a_hard_error() {
        let reference = scratch("geometry-reference");
        let rendered = scratch("geometry-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(0, 1, 8)]);
        write_rendered_capture(
            &rendered,
            "testcard",
            &[(flat(0, 1, 16), (4, 4))],
        );
        let out = scratch("geometry-out");

        let error = command(&reference, Some(&rendered), None, Some(&out)).unwrap_err();
        assert!(matches!(error, CompareError::GeometryMismatch { .. }));
        let message = error.to_string();
        assert!(message.contains("8") && message.contains("16"), "{message}");
    }

    #[test]
    fn a_frame_that_changed_geometry_mid_run_is_a_hard_error() {
        // The port resizes the frame from the view every tick, so one run can
        // hold frames of two sizes; the reference holds one size per capture.
        let reference = scratch("doublescan-reference");
        let rendered = scratch("doublescan-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(0, 1, 8), flat(1, 1, 8)]);
        write_rendered_capture(
            &rendered,
            "testcard",
            &[(flat(0, 1, 8), (4, 2)), (flat(1, 1, 16), (4, 4))],
        );
        let out = scratch("doublescan-out");

        assert!(matches!(
            command(&reference, Some(&rendered), None, Some(&out)),
            Err(CompareError::GeometryMismatch { .. })
        ));
    }

    #[test]
    fn a_rendered_side_that_cannot_supply_a_reference_index_is_a_hard_error() {
        // The rendered side holding fewer frames than the reference is caught
        // per reference index, by the alignment window, rather than by a count:
        // reference frame 200 has no rendered frame within ±1 of it.
        let reference = scratch("count-reference");
        let rendered = scratch("count-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(100, 1, 8), flat(200, 1, 8)]);
        write_rendered_capture(&rendered, "testcard", &[(flat(100, 1, 8), (4, 2))]);
        // A path nothing has made yet: the refusal must not make it either.
        let out = scratch("count-out").join("reports");

        let error = command(&reference, Some(&rendered), None, Some(&out)).unwrap_err();
        match error {
            CompareError::NoFrameWithinWindow { index, window, .. } => {
                assert_eq!((index, window), (200, 1));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        // A refusal leaves nothing to read: no report, and no output directory
        // for a script to open and wonder about.
        assert!(!out.exists(), "{} was left behind", out.display());
    }

    #[test]
    fn a_rendered_side_may_hold_frames_the_reference_does_not() {
        // The port's dump holds every tick and a capture holds a handful of
        // keyframes, so the rendered side is allowed to be the denser one. The
        // reference's indices are the ones that must all be answered.
        let reference = scratch("dense-reference");
        let rendered = scratch("dense-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(100, 5, 8), flat(200, 6, 8)]);
        write_rendered_capture(
            &rendered,
            "testcard",
            &[
                (flat(99, 1, 8), (4, 2)),
                (flat(100, 5, 8), (4, 2)),
                (flat(101, 2, 8), (4, 2)),
                (flat(199, 3, 8), (4, 2)),
                (flat(200, 6, 8), (4, 2)),
            ],
        );
        let out = scratch("dense-out");

        assert_eq!(
            command(&reference, Some(&rendered), None, Some(&out)).unwrap(),
            ExitCode::SUCCESS
        );
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["frames"].as_array().unwrap().len(), 2);
        assert_eq!(report["summary"]["offset_frames"], 0);
    }

    #[test]
    fn a_directory_without_a_manifest_is_a_legible_error() {
        let dir = scratch("no-manifest");
        let out = scratch("no-manifest-out");

        let error = command(&dir, Some(&dir), None, Some(&out)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("manifest.json"), "{message}");
        assert!(message.contains(&dir.display().to_string()), "{message}");
    }

    #[test]
    fn a_missing_frame_file_is_a_legible_error() {
        let dir = scratch("missing-frame");
        write_reference_capture(&dir, (4, 2), &[flat(0, 1, 8)]);
        fs::remove_file(dir.join("frame_000000.rgb")).unwrap();
        let out = scratch("missing-frame-out");

        let error = command(&dir, Some(&dir), None, Some(&out)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("frame_000000.rgb"), "{message}");
    }

    #[test]
    fn a_capture_tree_is_matched_by_name() {
        // `reference/check_a` holds two captures — the title screen (320x400
        // Mode X) and a part screen (320x200 chain-4) — because one manifest
        // carries one geometry. Comparing the tree compares each capture.
        let reference = scratch("tree-reference");
        let rendered = scratch("tree-rendered");
        for root in [&reference, &rendered] {
            write_reference_capture(&root.join("title"), (4, 4), &[flat(400, 1, 16)]);
            write_reference_capture(&root.join("part"), (4, 2), &[flat(1100, 1, 8)]);
        }
        let out = scratch("tree-out");

        let code = command(&reference, Some(&rendered), None, Some(&out)).unwrap();

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(out.join("title/report.json").is_file());
        assert!(out.join("part/report.json").is_file());
        let title = read_json(&out.join("title/report.json"));
        assert_eq!(title["frames"][0]["total_pixels"], 16);
        let part = read_json(&out.join("part/report.json"));
        assert_eq!(part["frames"][0]["total_pixels"], 8);
    }

    #[test]
    fn a_capture_tree_that_holds_a_different_screen_is_different() {
        let reference = scratch("tree-diff-reference");
        let rendered = scratch("tree-diff-rendered");
        write_reference_capture(&reference.join("title"), (4, 2), &[flat(400, 1, 8)]);
        write_reference_capture(&rendered.join("title"), (4, 2), &[flat(400, 2, 8)]);
        let out = scratch("tree-diff-out");

        let code = command(&reference, Some(&rendered), None, Some(&out)).unwrap();

        assert_eq!(code, ExitCode::from(1));
        let report = read_json(&out.join("title/report.json"));
        assert_eq!(report["summary"]["verdict"], "different");
        assert_eq!(report["frames"][0]["differing_pixels"], 8);
    }

    #[test]
    fn a_capture_the_other_side_lacks_is_a_legible_error() {
        let reference = scratch("tree-missing-reference");
        let rendered = scratch("tree-missing-rendered");
        write_reference_capture(&reference.join("title"), (4, 2), &[flat(400, 1, 8)]);
        write_reference_capture(&reference.join("part"), (4, 2), &[flat(1100, 1, 8)]);
        write_reference_capture(&rendered.join("title"), (4, 2), &[flat(400, 1, 8)]);
        let out = scratch("tree-missing-out");

        let error = command(&reference, Some(&rendered), None, Some(&out)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("part"), "{message}");
    }

    #[test]
    fn the_run_command_is_a_shell_command_line() {
        // `--run` goes to `sh -c`, so it can be a whole command line rather
        // than a single path.
        let reference = scratch("run-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);
        let produced = scratch("run-produced");
        // What the command will copy into place: the captured frames.
        write_reference_capture(&produced, (4, 2), &[flat(0, 5, 8)]);
        // Where the comparison will look: a path nothing has created.
        let rendered = scratch("run-rendered").join("dump");
        let out = scratch("run-out");
        let shell = format!(
            "mkdir -p {} && cp {}/manifest.json {} && cp {}/frame_000000.idx {} && cp {}/frame_000000.rgb {}",
            rendered.display(),
            produced.display(),
            rendered.display(),
            produced.display(),
            rendered.display(),
            produced.display(),
            rendered.display(),
        );

        let code = command(&reference, Some(&rendered), Some(&shell), Some(&out)).unwrap();

        assert_eq!(code, ExitCode::SUCCESS);
        assert!(out.join("report.json").is_file());
    }

    #[test]
    fn a_run_command_that_fails_is_reported() {
        let reference = scratch("run-fail-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);
        let rendered = scratch("run-fail-rendered");
        let out = scratch("run-fail-out");

        let error = command(&reference, Some(&rendered), Some("exit 3"), Some(&out)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("exit 3"), "{message}");
    }

    #[test]
    fn a_run_command_with_no_rendered_directory_is_refused() {
        // The command has to write its frames somewhere; without --rendered
        // the comparison would silently compare the reference with itself.
        let reference = scratch("run-alone-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);

        let error = command(&reference, None, Some("true"), None).unwrap_err();
        assert!(error.to_string().contains("--rendered"), "{error}");
    }

    #[test]
    fn the_rendered_side_may_be_left_out_and_means_the_reference() {
        let dir = scratch("default-rendered");
        write_reference_capture(&dir, (4, 2), &[flat(0, 5, 8)]);
        let out = scratch("default-rendered-out");

        assert_eq!(command(&dir, None, None, Some(&out)).unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn the_default_report_directory_is_out_compare() {
        // Not the reference directory: nothing this tool writes may land among
        // the captures, and `out/` is already ignored by git.
        assert_eq!(output_root(None), PathBuf::from("out/compare"));
        assert_eq!(
            output_root(Some(Path::new("somewhere/else"))),
            PathBuf::from("somewhere/else")
        );
    }

    #[test]
    fn an_out_directory_inside_the_reference_is_refused() {
        // reference/ is input-only: a report written among the captures would
        // dirty the bytes every later diff is measured against.
        let reference = scratch("guard-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);
        let inside = reference.join("reports");

        let error = command(&reference, Some(&reference), None, Some(&inside)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("--out"), "{message}");
        assert!(message.contains(&inside.display().to_string()), "{message}");
        assert!(message.contains(&reference.display().to_string()), "{message}");
        assert!(!inside.exists(), "the refused directory must not be created");
    }

    #[test]
    fn the_reference_itself_is_refused_as_an_out_directory() {
        let reference = scratch("guard-self-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);

        assert!(command(&reference, Some(&reference), None, Some(&reference)).is_err());
        assert!(!reference.join("report.json").exists());
    }

    #[test]
    fn a_dotted_or_unmade_path_cannot_slip_past_the_out_guard() {
        let reference = scratch("guard-slip-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);

        // A trailing `/.` names the reference itself.
        assert!(command(&reference, Some(&reference), None, Some(&reference.join("."))).is_err());
        // A path under the reference whose first component does not exist yet.
        assert!(command(
            &reference,
            Some(&reference),
            None,
            Some(&reference.join("deep/../deeper"))
        )
        .is_err());
    }

    #[test]
    fn a_dotdot_behind_a_component_nothing_has_made_is_resolved_lexically() {
        // The bypass this guard carried: only the deepest *existing* ancestor
        // canonicalises, so a `..` behind a component that does not exist yet
        // survived as text and the comparison below never saw it. The path
        // below is inside `<parent>/ref` for every write — `create_dir_all`
        // makes `missing` and then resolves `..` through it — and outside it
        // for a string comparison: the report and the diff images landed among
        // the captures, where the plain path was refused.
        let parent = scratch("guard-dotdot-parent");
        let reference = parent.join("ref");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);

        let out = parent.join("missing/../ref/nested");
        // What the filesystem would do with it, named without the `..` — the
        // path the comparison has to see.
        assert_eq!(resolve(&out).unwrap(), resolve(&reference).unwrap().join("nested"));

        let error = command(&reference, Some(&reference), None, Some(&out))
            .expect_err("the path lands inside the reference");
        assert!(error.to_string().contains("ref/nested"), "{error}");
        // Refused before anything is written, so not even the `missing`
        // component is left behind on the way.
        assert!(!parent.join("missing").exists());
        assert!(!reference.join("nested").exists());
    }

    #[test]
    fn a_report_directory_beside_the_reference_is_allowed() {
        let reference = scratch("guard-ok-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);
        let out = scratch("guard-ok-out");

        assert_eq!(
            command(&reference, Some(&reference), None, Some(&out)).unwrap(),
            ExitCode::SUCCESS
        );
        assert!(out.join("report.json").is_file());
    }

    #[test]
    fn the_whole_tree_form_finds_every_capture_the_per_part_form_finds() {
        // `reference/parts` is the tree that hid four captures: eighteen of its
        // capture directories are immediate subdirectories, and `plzpart` and
        // `techno` — the two parts whose screens have two geometries each —
        // hold theirs one level deeper. A scan of the first level measured 18
        // of the 22 and still said `verdict: identical`, so the whole-tree form
        // is pinned against the per-part form: the two have to find the same
        // directories, no more and no fewer.
        let parts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/parts");
        let whole = resolve_captures(&parts).unwrap();
        assert_eq!(whole.len(), 22, "the tree's capture directories, reviewed and pinned");

        let mut per_part: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&parts).unwrap() {
            let path = entry.unwrap().path();
            if !path.is_dir() {
                continue;
            }
            // Every subdirectory of `parts/` holds captures: eighteen hold one
            // each and the two Mode X parts hold two — so a subdirectory with
            // none is a change to the tree this test is about.
            let captures = resolve_captures(&path)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            per_part.extend(captures.into_iter().map(|capture| capture.dir));
        }
        assert_eq!(per_part.len(), 22, "the same 22, one invocation per part");

        // Both directions: a whole-tree run may not miss a capture the per-part
        // form finds, and may not measure one no per-part form finds.
        for capture in &whole {
            assert!(
                per_part.contains(&capture.dir),
                "the whole-tree form found {}, which no --reference <part> run finds",
                capture.dir.display()
            );
        }
        for dir in &per_part {
            assert!(
                whole.iter().any(|capture| &capture.dir == dir),
                "the whole-tree form missed {}, which a per-part run finds",
                dir.display()
            );
        }

        // The names carry the nesting, because that is what pairs the two
        // sides: a rendered tree laid out flat cannot silently match a nested
        // reference capture, it is refused by name.
        let names: Vec<&str> = whole.iter().map(|c| c.name.as_str()).collect();
        for name in ["plzpart/cube", "plzpart/plasma", "techno/interference", "techno/troll"] {
            assert!(names.contains(&name), "{name} is missing from {names:?}");
        }
        for name in ["3dsinfld", "endscrl", "pam"] {
            assert!(names.contains(&name), "{name} is missing from {names:?}");
        }
    }

    #[test]
    fn the_real_check_a_captures_parse() {
        // The real input, not an example: `reference/check_a` holds two
        // captures, and the tool reads their manifests as written.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/check_a");
        let captures = resolve_captures(&root).unwrap();

        let names: Vec<&str> = captures.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["part", "title"]);

        let title = &captures[1];
        assert_eq!((title.manifest.width, title.manifest.height), (320, 400));
        assert_eq!(title.manifest.frames.len(), 2);
        // Re-captured under the autoexec boot and 30000 cycles (D17): the same
        // two screens as the typed-recipe capture, at the indices that boot
        // puts them at.
        assert_eq!(title.manifest.frames[0].index, 700);
        assert!(title.manifest.frames.iter().all(|f| f.idx.is_some()));

        let part = &captures[0];
        assert_eq!((part.manifest.width, part.manifest.height), (320, 200));
        assert_eq!(part.manifest.frames[0].index, 1350);
    }

    #[test]
    fn the_real_check_a_capture_compares_identical_with_no_offset() {
        // The real capture through the real path: four frames of two
        // geometries, every index exactly where the alignment search finds it.
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/check_a");
        let out = scratch("real-check-a-out");

        assert_eq!(command(&root, Some(&root), None, Some(&out)).unwrap(), ExitCode::SUCCESS);
        for capture in ["part", "title"] {
            let report = read_json(&out.join(capture).join("report.json"));
            assert_eq!(report["summary"]["verdict"], "identical");
            assert_eq!(report["summary"]["offset_frames"], 0);
            for frame in report["frames"].as_array().unwrap() {
                assert_eq!(frame["offset"], 0);
                assert_eq!(frame["differing_percent"], 0.0);
            }
        }
    }

    // --- helpers ----------------------------------------------------------

    /// A frame as the tests write it: indices optional, RGB always present.
    struct FrameData {
        index: u32,
        idx: Option<Vec<u8>>,
        rgb: Vec<u8>,
    }

    /// A frame of one repeated index and one repeated RGB value.
    fn flat(index: u32, value: u8, pixels: usize) -> FrameData {
        FrameData {
            index,
            idx: Some(vec![value; pixels]),
            rgb: vec![value; pixels * 3],
        }
    }

    /// A frame that differs from `flat(index, value, pixels)` in exactly
    /// `differing` of its pixels, so a test can say how close a picture is.
    fn flat_but(index: u32, value: u8, differing: usize, pixels: usize) -> FrameData {
        let mut idx = vec![value; pixels];
        for pixel in idx.iter_mut().take(differing) {
            *pixel = value.wrapping_add(1);
        }
        FrameData {
            index,
            idx: Some(idx),
            rgb: vec![value; pixels * 3],
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("xtask-compare-test-{name}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Write a capture in the capture manifest's shape: the recipe fields, one
    /// geometry for the whole capture, palette entries per frame.
    fn write_reference_capture(dir: &Path, geometry: (u16, u16), frames: &[FrameData]) {
        fs::create_dir_all(dir).unwrap();
        let entries: Vec<_> = frames.iter().map(|f| frame_entry(dir, f, None)).collect();
        let manifest = serde_json::json!({
            "version": 1,
            "oracle": "dosbox-automation test",
            "upstream_commit": "071a82e",
            "exe": "SECOND.EXE",
            "arguments": ["2"],
            "cycles": 25000,
            "width": geometry.0,
            "height": geometry.1,
            "frame_origin": "boot",
            "skip_boot_frames": 300,
            "keys": ["KBD_left", "KBD_enter"],
            "frames": entries,
        });
        write_manifest(dir, manifest);
    }

    /// Write a capture in the port's dump shape: the first frame's geometry at
    /// the top level, each frame's own geometry inside it.
    fn write_rendered_capture(dir: &Path, part: &str, frames: &[(FrameData, (u16, u16))]) {
        fs::create_dir_all(dir).unwrap();
        let entries: Vec<_> = frames
            .iter()
            .map(|(f, geometry)| frame_entry(dir, f, Some(*geometry)))
            .collect();
        let first = frames.first().map(|(_, g)| *g).unwrap_or((0, 0));
        let manifest = serde_json::json!({
            "version": 1,
            "part": part,
            "width": first.0,
            "height": first.1,
            "frames": entries,
        });
        write_manifest(dir, manifest);
    }

    fn frame_entry(dir: &Path, frame: &FrameData, geometry: Option<(u16, u16)>) -> serde_json::Value {
        let name = format!("frame_{:06}", frame.index);
        let rgb = format!("{name}.rgb");
        fs::write(dir.join(&rgb), &frame.rgb).unwrap();
        let mut entry = serde_json::json!({ "index": frame.index, "rgb": rgb });
        if let Some(bytes) = &frame.idx {
            let idx = format!("{name}.idx");
            fs::write(dir.join(&idx), bytes).unwrap();
            entry["idx"] = serde_json::Value::String(idx);
        }
        match geometry {
            Some((width, height)) => {
                entry["width"] = width.into();
                entry["height"] = height.into();
            }
            // The capture manifest records the palette the frame uses.
            None => entry["palette"] = serde_json::json!({ "0": [0, 0, 0] }),
        }
        entry
    }

    fn write_manifest(dir: &Path, manifest: serde_json::Value) {
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        fs::write(dir.join("manifest.json"), format!("{json}\n")).unwrap();
    }

    fn read_json(path: &Path) -> serde_json::Value {
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    /// Read a PNG back: `(width, height, RGB8 pixels)`.
    fn read_png(path: &Path) -> (u32, u32, Vec<u8>) {
        let decoder = png::Decoder::new(fs::File::open(path).unwrap());
        let mut reader = decoder.read_info().unwrap();
        let mut buffer = vec![0; reader.output_buffer_size()];
        let info = reader.next_frame(&mut buffer).unwrap();
        buffer.truncate(info.buffer_size());
        (info.width, info.height, buffer)
    }
}
