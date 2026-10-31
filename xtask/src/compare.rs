//! `xtask compare`: the port's frames against a reference capture, as a differing-pixel count,
//! a max channel delta and a diff image per frame.
//!
//! - Indices are compared whenever both sides have them: stricter than RGB (two entries may
//!   widen to one colour), and independent of the DAC widening. RGB is the fallback for a side
//!   with no indices (D15).
//! - Frames pair by manifest index, and a rendered frame may sit one frame either side of the
//!   index it answers for, because the oracle's animation phase is not reproducible across
//!   sessions (D17). The offset is reported; anything further is refused (D18).
//! - Exit 0 every frame identical, 1 any differs, 2 could not compare (main.rs). A geometry
//!   mismatch is exit 2, never a count over the overlapping corner.
//! - `--out` (default `out/compare`) may not resolve inside the reference.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use serde::Serialize;

use crate::capture::{capture_directories, read_frame, read_manifest, FrameRef, Manifest};
use crate::paths::refuse_output_inside;

/// The report this tool writes next to the diff images.
const REPORT_FILE: &str = "report.json";

/// The report format's version. 2 is the alignment search (D18): each frame records the offset
/// it was paired at.
const REPORT_VERSION: u32 = 2;

/// How far either side of its reference index a rendered frame may sit: the oracle's measured
/// ambiguity (D17). A constant, not an option: it widens the alignment, never the pixels (D18).
const ALIGNMENT_WINDOW: u32 = 1;

/// The score of a candidate of another geometry: below every comparable candidate, so it is
/// chosen only when it is the only one, and the geometry check then refuses it by name.
const INCOMPARABLE: u64 = u64::MAX;

/// Where the reports go when `--out` is not given.
const DEFAULT_OUT: &str = "out/compare";

/// Why a comparison could not produce a number.
#[derive(Debug)]
pub enum CompareError {
    /// The two frames hold different numbers of pixels, the reference side first.
    GeometryMismatch { reference: usize, rendered: usize },
    /// A reference frame has no unused rendered frame within the alignment window.
    NoFrameWithinWindow {
        index: u32,
        window: u32,
        /// The nearest unused rendered index, if the rendered side holds any.
        nearest: Option<u32>,
        rendered_frames: usize,
    },
    /// A manifest, a frame file, or the layout of the two sides could not be read or matched.
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

impl From<String> for CompareError {
    fn from(message: String) -> Self {
        CompareError::Io(message)
    }
}

/// How far apart one pair of frames is.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameDiff {
    /// Pixels whose value differs at all (for RGB, in any channel).
    pub differing_pixels: u64,
    pub total_pixels: u64,
    pub differing_percent: f32,
    /// The largest single-value difference: an index difference, or a channel difference.
    pub max_channel_delta: u8,
}

/// Compare two indexed frames, byte for byte. Different lengths are a hard error.
pub fn compare_indexed(reference: &[u8], rendered: &[u8]) -> Result<FrameDiff, CompareError> {
    compare_pixels(reference, rendered, 1)
}

/// Compare two RGB8 frames, with no tolerance: both sides widen the DAC with the same rule.
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

/// Which pixels differ, and the widest single-value difference. The counts and the diff image
/// both come from this walk, so they cannot disagree.
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

/// Pair each reference index with the rendered index that answers for it.
///
/// For each reference index, ascending, the candidates are the unused rendered indices within
/// `ALIGNMENT_WINDOW` of it. `difference` scores a candidate (the differing-pixel count) and
/// the lowest score wins; ties go to the smaller offset, then the lower index, so a still
/// screen reports offset 0. A rendered frame answers one reference frame only (greedy, in
/// reference order). A reference index with no candidate is refused. The rendered side may
/// hold frames the reference does not.
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
        // (score, index distance, position in the sorted list).
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

/// `rendered_index - reference_index`, signed, saturating at `±i32::MAX`. Under `--align` the
/// rendered index is the shifted one, so the offset is the net one.
fn frame_offset(reference_index: u32, rendered_index: i64) -> i32 {
    let limit = i64::from(i32::MAX);
    (rendered_index - i64::from(reference_index)).clamp(-limit, limit) as i32
}

/// `identical` only when every frame matched.
pub fn verdict(percents: &[f32]) -> &'static str {
    if percents.iter().all(|percent| *percent == 0.0) {
        "identical"
    } else {
        "different"
    }
}

/// One capture with its manifest read.
#[derive(Debug)]
struct Capture {
    /// The capture's path below the tree's root; empty for the root itself.
    name: String,
    dir: PathBuf,
    manifest: Manifest,
}

/// Every capture at or below `dir`, with its manifest.
fn resolve_captures(dir: &Path) -> Result<Vec<Capture>, CompareError> {
    let mut captures = Vec::new();
    for found in capture_directories(dir)? {
        let manifest = read_manifest(&found.dir)?;
        captures.push(Capture {
            name: found.name,
            dir: found.dir,
            manifest,
        });
    }
    Ok(captures)
}

/// Pair the two sides' captures by name. An unmatched name on either side is refused, never
/// skipped.
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

/// The frames of both sides, paired by index within the alignment window, the picture deciding
/// between candidates. The chosen pairs are compared again by `compare_capture`, which is where
/// the reported numbers come from.
fn paired_frames<'a>(
    reference: &'a Capture,
    rendered: &'a Capture,
    align: i32,
) -> Result<Vec<(&'a FrameRef, &'a FrameRef)>, CompareError> {
    let mut reference_frames: Vec<&FrameRef> = reference.manifest.frames.iter().collect();
    let mut rendered_frames: Vec<&FrameRef> = rendered.manifest.frames.iter().collect();
    reference_frames.sort_by_key(|f| f.index);
    rendered_frames.sort_by_key(|f| f.index);
    let reference_indices: Vec<u32> = reference_frames.iter().map(|f| f.index).collect();
    // `--align N` reads the rendered indices N higher for pairing (`docs/fidelity/tunneli.md`
    // §7.2). An index pushed out of range is refused: clamping would collide frames silently.
    let rendered_indices: Vec<u32> = rendered_frames
        .iter()
        .map(|frame| {
            u32::try_from(i64::from(frame.index) + i64::from(align)).map_err(|_| {
                CompareError::Io(format!(
                    "--align {align:+} puts the rendered frame at index {} outside the index \
                     range; an alignment is a numbering offset, not a window",
                    frame.index
                ))
            })
        })
        .collect::<Result<_, _>>()?;

    let pairs = pair_indices(
        &reference_indices,
        &rendered_indices,
        |reference_index, rendered_index| {
            let reference_frame = find(&reference_frames, &reference_indices, reference_index)?;
            let rendered_frame = find(&rendered_frames, &rendered_indices, rendered_index)?;
            if reference_frame.geometry(&reference.manifest)
                != rendered_frame.geometry(&rendered.manifest)
            {
                return Ok(INCOMPARABLE);
            }
            candidate_difference(reference, reference_frame, rendered, rendered_frame)
        },
    )?;

    // Each rendered frame is taken once, so a side listing one index twice is consumed in order.
    let mut reference_taken = vec![false; reference_frames.len()];
    let mut rendered_taken = vec![false; rendered_frames.len()];
    let mut paired = Vec::with_capacity(pairs.len());
    for (reference_index, rendered_index) in pairs {
        paired.push((
            take(&reference_frames, &reference_indices, &mut reference_taken, reference_index)?,
            take(&rendered_frames, &rendered_indices, &mut rendered_taken, rendered_index)?,
        ));
    }
    Ok(paired)
}

/// The first frame whose (shifted) index in `indices` is `index`.
fn find<'a>(frames: &[&'a FrameRef], indices: &[u32], index: u32) -> Result<&'a FrameRef, CompareError> {
    frames
        .iter()
        .zip(indices)
        .find(|(_, at)| **at == index)
        .map(|(frame, _)| *frame)
        .ok_or_else(|| missing_index(index))
}

/// The first frame at `index` that no earlier pair has taken. No fallback to the unshifted
/// index: under `--align` that would be a wrong pair.
fn take<'a>(
    frames: &[&'a FrameRef],
    indices: &[u32],
    taken: &mut [bool],
    index: u32,
) -> Result<&'a FrameRef, CompareError> {
    for (position, frame) in frames.iter().enumerate() {
        if indices[position] == index && !taken[position] {
            taken[position] = true;
            return Ok(frame);
        }
    }
    Err(missing_index(index))
}

fn missing_index(index: u32) -> CompareError {
    CompareError::Io(format!(
        "the pairing named a frame at index {index} that the manifest does not hold"
    ))
}

/// How far apart two frames are, by the rule the report uses: indices when both sides have
/// them, RGB otherwise.
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
    /// `indexed`, `rgb`, or `mixed`.
    mode: &'static str,
    frames: Vec<FrameReport>,
    summary: Summary,
}

#[derive(Debug, Serialize)]
struct FrameReport {
    /// The reference frame's manifest index.
    index: u32,
    /// The rendered frame it was paired with.
    rendered_index: u32,
    /// The net offset of the pairing: 0 exact, ±1 the oracle's phase (D18).
    offset: i32,
    differing_pixels: u64,
    total_pixels: u64,
    differing_percent: f32,
    max_channel_delta: u8,
    /// The diff image beside this report: differing pixels in magenta over a dimmed reference.
    diff_image: String,
}

#[derive(Debug, Serialize)]
struct Summary {
    worst_percent: f32,
    mean_percent: f32,
    verdict: &'static str,
    /// How many frames were paired at a non-zero offset.
    offset_frames: usize,
    /// Every offset used and how often: scattered ±1 is the oracle, one constant offset is a
    /// finding about the port (D18).
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

/// `+1 x2, -1 x1`: the non-zero offsets used, for the console line.
fn offsets_used(offsets: &BTreeMap<i32, usize>) -> String {
    offsets
        .iter()
        .filter(|(offset, _)| **offset != 0)
        .map(|(offset, frames)| format!("{offset:+} x{frames}"))
        .collect::<Vec<_>>()
        .join(", ")
}

/// Make `out` just before something goes in it, so a refused comparison leaves nothing behind.
fn prepare_out(out: &Path) -> Result<(), CompareError> {
    if out.is_dir() {
        return Ok(());
    }
    fs::create_dir_all(out).map_err(|e| CompareError::Io(format!("{}: {e}", out.display())))
}

/// Compare every frame of one capture pair, and write its report and diff images.
fn compare_capture(
    reference: &Capture,
    rendered: &Capture,
    out: &Path,
    align: i32,
) -> Result<CaptureOutcome, CompareError> {
    let mut frames = Vec::new();
    let mut indexed = 0usize;
    for (reference_frame, rendered_frame) in paired_frames(reference, rendered, align)? {
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

        // The diff image is drawn from the reference's RGB frame, even when comparing indices.
        let reference_rgb = read_frame(&reference.dir, &reference_frame.rgb)?;
        if reference_rgb.len() != expected_rgb {
            return Err(CompareError::Io(format!(
                "{} holds {} bytes; {width}x{height} RGB8 needs {expected_rgb}",
                reference.dir.join(&reference_frame.rgb).display(),
                reference_rgb.len()
            )));
        }

        let (diff, mask) = match (&reference_frame.idx, &rendered_frame.idx) {
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
                indexed += 1;
                let diff = compare_indexed(&reference_bytes, &rendered_bytes)?;
                let (mask, _) = differences(&reference_bytes, &rendered_bytes, 1);
                (diff, mask)
            }
            _ => {
                let rendered_rgb = read_frame(&rendered.dir, &rendered_frame.rgb)?;
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
            offset: frame_offset(
                reference_frame.index,
                i64::from(rendered_frame.index) + i64::from(align),
            ),
            differing_pixels: diff.differing_pixels,
            total_pixels: diff.total_pixels,
            differing_percent: diff.differing_percent,
            max_channel_delta: diff.max_channel_delta,
            diff_image,
        });
    }

    let mode = if indexed == frames.len() {
        "indexed"
    } else if indexed > 0 {
        "mixed"
    } else {
        "rgb"
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

/// Differing pixels in magenta over a half-brightness copy of the reference.
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
            image.extend(pixel.iter().map(|channel| channel / 2));
        }
    }
    crate::sheet::write_png(path, u32::from(width), u32::from(height), &image)
        .map_err(|e| CompareError::Io(format!("{}: {e}", path.display())))
}

/// `cargo xtask compare`: diff the rendered frames against the reference, print and write the
/// report, and return exit 0 (identical) or 1 (different). `Err` is exit 2.
pub fn command(
    reference: &Path,
    rendered: Option<&Path>,
    out: Option<&Path>,
    align: i32,
) -> Result<ExitCode, CompareError> {
    let out = out.map_or_else(|| PathBuf::from(DEFAULT_OUT), Path::to_path_buf);
    refuse_output_inside(
        &out,
        reference,
        "--out",
        "the reference captures are inputs, not outputs, and a report written among them would \
         dirty the frames every later diff is measured against",
        &format!("Pass --out somewhere else; the default is {DEFAULT_OUT}"),
    )?;

    let rendered = rendered.unwrap_or(reference);
    if align != 0 {
        println!(
            "alignment: the rendered side's indices are read {align:+} for pairing (--align); \
             the offsets below are on top of that"
        );
    }
    let reference_captures = resolve_captures(reference)?;
    let rendered_captures = resolve_captures(rendered)?;

    let mut identical = true;
    let pairs = pair_captures(&reference_captures, &rendered_captures)?;
    for (reference_capture, rendered_capture) in &pairs {
        let name = &reference_capture.name;
        let (prefix, capture_out) = if name.is_empty() {
            (String::new(), out.clone())
        } else {
            (format!("{name} "), out.join(name))
        };
        let outcome = compare_capture(reference_capture, rendered_capture, &capture_out, align)?;
        for frame in &outcome.report.frames {
            let alignment = if frame.offset == 0 {
                String::new()
            } else {
                format!(
                    ", rendered index {} (offset {:+})",
                    frame.rendered_index, frame.offset
                )
            };
            println!(
                "{prefix}frame {}: {:.4}% differing ({}/{} px), max delta {}{alignment}",
                frame.index,
                frame.differing_percent,
                frame.differing_pixels,
                frame.total_pixels,
                frame.max_channel_delta
            );
        }
        if outcome.report.summary.offset_frames > 0 {
            println!(
                "{prefix}alignment: {} of {} frames paired at a non-zero offset ({})",
                outcome.report.summary.offset_frames,
                outcome.report.frames.len(),
                offsets_used(&outcome.report.summary.offsets)
            );
        }
        println!("report: {}", outcome.path.display());
        identical &= outcome.report.summary.verdict == "identical";
    }

    // The count of captures measured sits with the verdict, so a partial run cannot look whole.
    println!("captures: {} measured", pairs.len());
    println!("verdict: {}", if identical { "identical" } else { "different" });
    Ok(if identical {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn the_widest_differing_index_sets_max_channel_delta() {
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 2, 9, 4]).unwrap();
        assert_eq!((r.differing_pixels, r.max_channel_delta, r.differing_percent), (1, 6, 25.0));
        let r = compare_indexed(&[1, 2, 3, 4], &[1, 9, 3, 4]).unwrap();
        assert_eq!((r.differing_pixels, r.max_channel_delta), (1, 7));
    }

    #[test]
    fn an_index_difference_is_counted_even_when_the_rgb_matches() {
        // Two palette entries may widen to the same RGB; the indices still differ.
        assert_eq!(compare_indexed(&[5], &[6]).unwrap().differing_pixels, 1);
    }

    #[test]
    fn mismatched_geometry_is_an_error_not_a_partial_diff() {
        assert!(matches!(
            compare_indexed(&[1, 2, 3, 4], &[1, 2]),
            Err(CompareError::GeometryMismatch { .. })
        ));
    }

    #[test]
    fn an_rgb_pixel_counts_once_however_many_channels_differ() {
        let r = compare_rgb(&[0, 0, 0, 10, 10, 10], &[1, 2, 3, 10, 10, 10]).unwrap();
        assert_eq!(r.differing_pixels, 1);
        assert_eq!(r.total_pixels, 2);
        assert_eq!(r.max_channel_delta, 3);
        assert_eq!(r.differing_percent, 50.0);
    }

    #[test]
    fn an_rgb_frame_that_is_not_whole_pixels_is_refused() {
        let error = compare_rgb(&[1, 2, 3, 4, 5], &[1, 2, 3, 4, 5]).unwrap_err();
        assert!(matches!(error, CompareError::Io(_)), "{error}");
    }

    // --- pairing ----------------------------------------------------------

    /// Two frames are the same picture only at the same index.
    fn same_picture_at_the_same_index(
        reference_index: u32,
        rendered_index: u32,
    ) -> Result<u64, CompareError> {
        Ok(u64::from(reference_index != rendered_index) * 8)
    }

    #[test]
    fn frames_are_paired_by_manifest_index_not_by_position() {
        // Two lists of one size that name different frames are refused, not zipped.
        let error =
            pair_indices(&[700, 900], &[400, 600], same_picture_at_the_same_index).unwrap_err();
        match error {
            CompareError::NoFrameWithinWindow { index, window, .. } => {
                assert_eq!((index, window), (700, 1));
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        // Lists naming the same frames pair by index, whatever order each lists them in.
        assert_eq!(
            pair_indices(&[900, 700], &[700, 900], same_picture_at_the_same_index).unwrap(),
            vec![(700, 700), (900, 900)]
        );
    }

    #[test]
    fn a_rendered_frame_is_taken_by_one_reference_frame_only() {
        assert_eq!(
            pair_indices(&[900, 901], &[899, 900, 901], |_, _| Ok(0)).unwrap(),
            vec![(900, 900), (901, 901)]
        );
        let error = pair_indices(&[900, 901], &[901], |_, _| Ok(0)).unwrap_err();
        match error {
            CompareError::NoFrameWithinWindow { index, .. } => assert_eq!(index, 901),
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn the_pairing_follows_the_picture_and_the_exact_index_breaks_a_tie() {
        let nearest_picture = |reference_index: u32, rendered_index: u32| {
            Ok(match (reference_index, rendered_index) {
                (900, 899) => 3,
                (900, 900) => 2, // the exact index, but a worse match
                (900, 901) => 0, // the picture the reference holds
                _ => 100,
            })
        };
        assert_eq!(
            pair_indices(&[900], &[899, 900, 901], nearest_picture).unwrap(),
            vec![(900, 901)]
        );
        // All three equal: offset 0 wins.
        assert_eq!(
            pair_indices(&[900], &[899, 900, 901], |_, _| Ok(0)).unwrap(),
            vec![(900, 900)]
        );
        // Two neighbours equal and no offset 0: the lower index wins.
        assert_eq!(
            pair_indices(&[900], &[899, 901], |_, _| Ok(0)).unwrap(),
            vec![(900, 899)]
        );
    }

    #[test]
    fn the_offset_distribution_and_the_verdict() {
        assert_eq!(offsets_used(&BTreeMap::from([(0, 3)])), "");
        assert_eq!(
            offsets_used(&BTreeMap::from([(-1, 1), (0, 2), (1, 2)])),
            "-1 x1, +1 x2"
        );
        assert_eq!(verdict(&[0.0, 0.0]), "identical");
        assert_eq!(verdict(&[0.0, 0.01]), "different");
        assert_eq!(verdict(&[]), "identical");
    }

    // --- whole captures ---------------------------------------------------

    #[test]
    fn comparing_a_capture_with_itself_is_identical() {
        let dir = scratch("self");
        write_reference_capture(&dir, (4, 2), &[flat(400, 7, 8), flat(600, 9, 8)]);
        let out = scratch("self-out");

        assert_eq!(command(&dir, None, Some(&out), 0).unwrap(), ExitCode::SUCCESS);
        let report = read_json(&out.join("report.json"));
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
            assert_eq!(frame["offset"], 0);
            let image = out.join(frame["diff_image"].as_str().unwrap());
            assert!(image.is_file(), "{} was not written", image.display());
        }
    }

    #[test]
    fn a_frame_one_index_away_is_found_and_its_offset_is_reported() {
        // Index 900 holds another picture and 901 the reference's, as the oracle's phase does
        // between sessions (D17): paired at 900 it is 100% different, searched it is 0 at +1.
        let reference = scratch("offset-reference");
        let at_its_own_index = scratch("offset-own-index");
        let one_later = scratch("offset-one-later");
        write_reference_capture(&reference, (4, 2), &[flat(900, 3, 8)]);
        write_rendered_capture(&at_its_own_index, &[(flat(900, 7, 8), (4, 2))]);
        write_rendered_capture(
            &one_later,
            &[
                (flat(899, 1, 8), (4, 2)),
                (flat(900, 7, 8), (4, 2)),
                (flat(901, 3, 8), (4, 2)),
            ],
        );

        let out = scratch("offset-own-index-out");
        assert_eq!(
            command(&reference, Some(&at_its_own_index), Some(&out), 0).unwrap(),
            ExitCode::from(1)
        );
        assert_eq!(read_json(&out.join("report.json"))["frames"][0]["differing_percent"], 100.0);

        let out = scratch("offset-one-later-out");
        assert_eq!(
            command(&reference, Some(&one_later), Some(&out), 0).unwrap(),
            ExitCode::SUCCESS
        );
        let report = read_json(&out.join("report.json"));
        let frame = &report["frames"][0];
        assert_eq!(frame["index"], 900);
        assert_eq!(frame["rendered_index"], 901);
        assert_eq!(frame["offset"], 1);
        assert_eq!(frame["differing_pixels"], 0);
        assert_eq!(report["summary"]["offset_frames"], 1);
        assert_eq!(report["summary"]["offsets"], serde_json::json!({ "1": 1 }));
    }

    #[test]
    fn a_rendered_side_may_hold_frames_the_reference_does_not() {
        let reference = scratch("dense-reference");
        let rendered = scratch("dense-rendered");
        write_reference_capture(&reference, (4, 2), &[flat(100, 5, 8), flat(200, 6, 8)]);
        write_rendered_capture(
            &rendered,
            &[
                (flat(99, 1, 8), (4, 2)),
                (flat(100, 5, 8), (4, 2)),
                (flat(101, 2, 8), (4, 2)),
                (flat(199, 3, 8), (4, 2)),
                (flat(200, 6, 8), (4, 2)),
            ],
        );
        let out = scratch("dense-out");

        assert_eq!(command(&reference, Some(&rendered), Some(&out), 0).unwrap(), ExitCode::SUCCESS);
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["frames"].as_array().unwrap().len(), 2);
        assert_eq!(report["summary"]["offset_frames"], 0);
    }

    #[test]
    fn a_differing_frame_is_reported_as_different_with_a_diff_image() {
        let reference = scratch("diff-reference");
        let rendered = scratch("diff-rendered");
        let mut rgb = flat(0, 4, 8).rgb;
        rgb[..3].copy_from_slice(&[8, 8, 8]);
        write_reference_capture(&reference, (4, 2), &[flat(0, 4, 8)]);
        write_reference_capture(&rendered, (4, 2), &[FrameData { index: 0, idx: None, rgb }]);
        let out = scratch("diff-out");

        assert_eq!(command(&reference, Some(&rendered), Some(&out), 0).unwrap(), ExitCode::from(1));
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["summary"]["worst_percent"], 12.5);
        let frame = &report["frames"][0];
        assert_eq!(frame["differing_pixels"], 1);
        assert_eq!(frame["total_pixels"], 8);

        let (width, height, pixels) =
            crate::sheet::read_png(&out.join(frame["diff_image"].as_str().unwrap()));
        assert_eq!((width, height), (4, 2));
        // The differing pixel is magenta; the rest a dimmed copy of the reference.
        assert_eq!(&pixels[..3], &[255, 0, 255]);
        assert_eq!(&pixels[3..6], &[2, 2, 2]);
    }

    #[test]
    fn the_indices_decide_when_both_sides_have_them_and_rgb_otherwise() {
        // Same RGB, different indices: the indexed comparison runs.
        let reference = scratch("indexed-reference");
        let rendered = scratch("indexed-rendered");
        let frame = |idx: Option<u8>, rgb: u8| FrameData {
            index: 0,
            idx: idx.map(|value| vec![value; 8]),
            rgb: vec![rgb; 24],
        };
        write_reference_capture(&reference, (4, 2), &[frame(Some(5), 20)]);
        write_reference_capture(&rendered, (4, 2), &[frame(Some(6), 20)]);
        let out = scratch("indexed-out");
        assert_eq!(command(&reference, Some(&rendered), Some(&out), 0).unwrap(), ExitCode::from(1));
        let report = read_json(&out.join("report.json"));
        assert_eq!(report["mode"], "indexed");
        assert_eq!(report["frames"][0]["differing_pixels"], 8);

        // A reference with no indices (D15) falls back to RGB rather than refusing.
        let reference = scratch("rgb-reference");
        let rendered = scratch("rgb-rendered");
        write_reference_capture(&reference, (4, 2), &[frame(None, 20)]);
        write_rendered_capture(&rendered, &[(frame(Some(9), 24), (4, 2))]);
        let out = scratch("rgb-out");
        assert_eq!(command(&reference, Some(&rendered), Some(&out), 0).unwrap(), ExitCode::from(1));
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
        write_rendered_capture(&rendered, &[(flat(0, 1, 16), (4, 4))]);
        let out = scratch("geometry-out");

        let error = command(&reference, Some(&rendered), Some(&out), 0).unwrap_err();
        assert!(matches!(error, CompareError::GeometryMismatch { .. }));
        let message = error.to_string();
        assert!(message.contains("8") && message.contains("16"), "{message}");
    }

    #[test]
    fn a_missing_manifest_or_frame_file_is_a_legible_error() {
        let dir = scratch("no-manifest");
        let out = scratch("no-manifest-out");
        let message = command(&dir, None, Some(&out), 0).unwrap_err().to_string();
        assert!(message.contains("manifest.json"), "{message}");
        assert!(message.contains(&dir.display().to_string()), "{message}");

        let dir = scratch("missing-frame");
        write_reference_capture(&dir, (4, 2), &[flat(0, 1, 8)]);
        fs::remove_file(dir.join("frame_000000.rgb")).unwrap();
        let message = command(&dir, None, Some(&out), 0).unwrap_err().to_string();
        assert!(message.contains("frame_000000.rgb"), "{message}");
    }

    #[test]
    fn a_capture_tree_is_matched_by_name() {
        // One manifest carries one geometry, so `reference/check_a` is two captures.
        let reference = scratch("tree-reference");
        let rendered = scratch("tree-rendered");
        for root in [&reference, &rendered] {
            write_reference_capture(&root.join("title"), (4, 4), &[flat(400, 1, 16)]);
            write_reference_capture(&root.join("part"), (4, 2), &[flat(1100, 1, 8)]);
        }
        let out = scratch("tree-out");

        assert_eq!(command(&reference, Some(&rendered), Some(&out), 0).unwrap(), ExitCode::SUCCESS);
        assert_eq!(read_json(&out.join("title/report.json"))["frames"][0]["total_pixels"], 16);
        assert_eq!(read_json(&out.join("part/report.json"))["frames"][0]["total_pixels"], 8);

        // A capture the other side lacks is refused by name.
        fs::remove_dir_all(rendered.join("part")).unwrap();
        let message = command(&reference, Some(&rendered), Some(&out), 0).unwrap_err().to_string();
        assert!(message.contains("part"), "{message}");
    }

    #[test]
    fn an_out_directory_inside_the_reference_is_refused() {
        let reference = scratch("guard-reference");
        write_reference_capture(&reference, (4, 2), &[flat(0, 5, 8)]);
        let inside = reference.join("reports");

        let message = command(&reference, None, Some(&inside), 0).unwrap_err().to_string();
        assert!(message.contains("--out"), "{message}");
        assert!(message.contains(&inside.display().to_string()), "{message}");
        assert!(message.contains(&reference.display().to_string()), "{message}");
        assert!(!inside.exists(), "the refused directory must not be created");
        assert!(command(&reference, None, Some(&reference), 0).is_err());
        assert!(!reference.join("report.json").exists());
    }

    #[test]
    fn the_whole_tree_form_finds_every_capture_the_per_part_form_finds() {
        // A scan of the first level once measured 18 of 23 captures and said `identical`, so the
        // whole-tree walk is pinned against one walk per subdirectory
        // (`reference-indices.md` §25–§27 list the nested captures).
        let parts = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/parts");
        let whole = resolve_captures(&parts).unwrap();
        assert_eq!(whole.len(), 51, "the tree's capture directories");

        let mut per_part: Vec<PathBuf> = Vec::new();
        for entry in fs::read_dir(&parts).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                let captures = resolve_captures(&path)
                    .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                per_part.extend(captures.into_iter().map(|capture| capture.dir));
            }
        }
        let mut whole_dirs: Vec<PathBuf> = whole.iter().map(|capture| capture.dir.clone()).collect();
        whole_dirs.sort();
        per_part.sort();
        assert_eq!(whole_dirs, per_part);

        // The names carry the nesting, which is what pairs the two sides.
        let names: Vec<&str> = whole.iter().map(|c| c.name.as_str()).collect();
        for name in ["plzpart/cube", "plzpart/plasma", "techno/interference", "techno/troll", "pam"] {
            assert!(names.contains(&name), "{name} is missing from {names:?}");
        }
    }

    #[test]
    fn the_real_check_a_capture_compares_identical_with_no_offset() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../reference/check_a");
        let out = scratch("real-check-a-out");

        assert_eq!(command(&root, None, Some(&out), 0).unwrap(), ExitCode::SUCCESS);
        for capture in ["part", "title"] {
            let report = read_json(&out.join(capture).join("report.json"));
            assert_eq!(report["summary"]["verdict"], "identical");
            assert_eq!(report["summary"]["offset_frames"], 0);
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
    fn write_rendered_capture(dir: &Path, frames: &[(FrameData, (u16, u16))]) {
        fs::create_dir_all(dir).unwrap();
        let entries: Vec<_> = frames
            .iter()
            .map(|(f, geometry)| frame_entry(dir, f, Some(*geometry)))
            .collect();
        let first = frames.first().map(|(_, g)| *g).unwrap_or((0, 0));
        let manifest = serde_json::json!({
            "version": 1,
            "part": "testcard",
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
}
