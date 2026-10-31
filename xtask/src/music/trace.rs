//! `music trace`: boot the original under the D17 recipe, locate STMIK, and write one row of
//! player state per frame into `reference/music/<name>/` beside a manifest.
//!
//! Every frame of the window is sampled, because a sparse sample cannot see a one-frame
//! quantisation error. Each sample is one memory request for the whole segment (so the words
//! cannot tear against each other), keyed by the guest's own frame counter and bracketed by
//! two reads of it: a sample whose counter moved across the read is dropped and re-taken, so a
//! late sample is a different frame number, never a wrong value under the right one.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::time::{Duration, Instant};

use super::format::{
    frames_in, Behaviour, Coverage, LocateRecord, PlayerState, Skipped, TraceFile, TraceManifest,
    COLUMNS, TRACE_FILE, TRACE_KIND,
};
use super::locate::{find_segment, needles, symbol_map, SymbolMap};
use super::{MusicError, MACHINE_WORDS, MUSIC_ROOT};
use crate::oracle::{sleep_frames, Oracle, FRAME_ORIGIN};
use crate::reference::{sha256_hex, Recipe, MANIFEST_VERSION, REFERENCE_ROOT, UPSTREAM_COMMIT};

/// How far ahead a target frame may be before the wait sleeps rather than polls (~0.35 s).
const POLL_WINDOW_FRAMES: u64 = 25;

/// How long the guest's counter may stand still before the trace is refused.
const STALL_TIMEOUT: Duration = Duration::from_secs(120);

/// Everything `music trace` needs. The recipe's `boot_wait` is always the setup screen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceOptions {
    pub object: PathBuf,
    pub recipe: Recipe,
    /// The window to sample, inclusive at both ends.
    pub window: [u64; 2],
    pub out: PathBuf,
    /// Replace a trace the directory already holds.
    pub force: bool,
}

/// Refuse a trace that cannot be reproduced, before touching the emulator: the recipe checks
/// `reference capture` makes (D17), and `--out` only in a new directory under
/// `reference/music/` when it is inside `reference/` at all.
pub fn validate_trace_options(options: &TraceOptions) -> Result<(), MusicError> {
    options.recipe.validate()?;
    if options.window[1] < options.window[0] {
        return Err(MusicError::Options(format!(
            "the frame window {}..{} is empty: its last frame is before its first",
            options.window[0], options.window[1]
        )));
    }
    if !options.object.is_file() {
        return Err(MusicError::Options(format!(
            "--object {} is not a file; the symbol map has to come from the object the demo was \
             linked from",
            options.object.display()
        )));
    }
    let music_root = Path::new(MUSIC_ROOT);
    if options.out.starts_with(Path::new(REFERENCE_ROOT))
        && (!options.out.starts_with(music_root) || options.out == music_root)
    {
        return Err(MusicError::Options(format!(
            "--out {} is inside {REFERENCE_ROOT}/: that tree holds the mounted original and \
             every capture, and a file written among them is a file every later diff is \
             measured against. The only place inside it a trace may be written is a new \
             directory under {MUSIC_ROOT}/, e.g. {MUSIC_ROOT}/<name>",
            options.out.display()
        )));
    }
    if let Some(capture) = frame_capture_holding(&options.out) {
        return Err(MusicError::Options(format!(
            "--out {} is inside the frame capture {}: the frames and the manifest in it are what \
             every later diff reads. A trace goes beside it, e.g. {MUSIC_ROOT}/<name>",
            options.out.display(),
            capture.display()
        )));
    }
    if !options.force && holds_a_trace(&options.out) {
        return Err(MusicError::Options(format!(
            "{} already holds a trace ({}); pass --force to replace it",
            options.out.display(),
            TRACE_FILE
        )));
    }
    Ok(())
}

/// Whether a directory holds a trace: the trace file, or a manifest of [`TRACE_KIND`].
fn holds_a_trace(dir: &Path) -> bool {
    if dir.join(TRACE_FILE).is_file() {
        return true;
    }
    std::fs::read_to_string(dir.join("manifest.json"))
        .map(|text| {
            serde_json::from_str::<serde_json::Value>(&text)
                .ok()
                .and_then(|value| value.get("kind").cloned())
                .and_then(|kind| kind.as_str().map(str::to_owned))
                .as_deref()
                == Some(TRACE_KIND)
        })
        .unwrap_or(false)
}

/// The nearest directory at or above `path` holding a manifest that is not a trace's.
fn frame_capture_holding(path: &Path) -> Option<PathBuf> {
    let mut current = Some(path);
    while let Some(directory) = current {
        if directory.join("manifest.json").is_file() && !holds_a_trace(directory) {
            return Some(directory.to_path_buf());
        }
        current = directory.parent();
    }
    None
}

/// Boot the original, locate STMIK, sample the window frame by frame, and write the trace and
/// its manifest. The oracle is left running.
pub fn trace(oracle: &mut dyn Oracle, options: &TraceOptions) -> Result<TraceManifest, MusicError> {
    validate_trace_options(options)?;
    let object = std::fs::read(&options.object).map_err(|e| MusicError::Io {
        path: options.object.clone(),
        message: e.to_string(),
    })?;
    let map = symbol_map(&options.object, &object)?;
    let mut needles = needles(&object)?;

    let oracle_name = oracle.describe()?;
    options.recipe.boot(oracle)?;

    // The locate starts as early as the guest answers, before the demo draws: the window's
    // earliest frames hold the music's start, which is what the trace exists to measure. So
    // whether the keys started the demo is checked after sampling.
    let location = find_segment(oracle, &object, &map, &needles)?;
    let base = location.base;
    let located_at = location.frame;
    for needle in needles.iter_mut() {
        needle.found_at = Some(base + needle.offset);
    }
    let mut locate_record = LocateRecord {
        base,
        views: location.views.clone(),
        aliasing: location.aliasing.clone(),
        segment: 0,
        offset: base,
        frame: located_at,
        words: map
            .words
            .iter()
            .map(|(name, offset)| (name.clone(), base + *offset))
            .collect(),
        needles: needles.to_vec(),
        rerun_base: base,
        rerun_views: location.views.clone(),
        rerun_frame: located_at,
    };
    let machine_at_start = read_words(oracle, base, &map, MACHINE_WORDS)?;

    let window = [options.window[0].max(located_at), options.window[1]];
    if window[0] > window[1] {
        return Err(MusicError::Coherence(format!(
            "the segment was located at frame {located_at}, after the last frame asked for \
             ({}): there is nothing to trace",
            options.window[1]
        )));
    }
    let mut samples: Vec<PlayerState> = Vec::new();
    // One frame past the guest's: the frame it is on is already half gone.
    let mut next = window[0].max(oracle.frame_counter()? + 1);
    let mut stalled_at = u64::MAX;
    let mut stalled_since = Instant::now();
    let mut skipped: Vec<Skipped> = Vec::new();
    let mut seen = next.saturating_sub(1);
    // A request is 0.4 ms and a frame 14.3 ms: distance is slept off, the last stretch polled.
    while next <= window[1] {
        let now = oracle.frame_counter()?;
        if now < next {
            if now + POLL_WINDOW_FRAMES < next {
                sleep_frames(next - now, POLL_WINDOW_FRAMES);
            }
            if now != stalled_at {
                stalled_at = now;
                stalled_since = Instant::now();
            } else if stalled_since.elapsed() > STALL_TIMEOUT {
                return Err(MusicError::Coherence(format!(
                    "the guest's frame counter has been {now} for {}s while the trace waited for \
                     {next}: the guest has stopped, and nothing past it was sampled",
                    stalled_since.elapsed().as_secs()
                )));
            }
            continue;
        }
        if now > next {
            skipped.push(Skipped {
                from: next,
                to: now - 1,
                counter_before: seen,
                counter_after: now,
                why: format!(
                    "the guest's own frame counter was read as {seen} and then as {now} with no \
                     reading of the frames between, so they were never the current frame and no \
                     sample could be taken at them"
                ),
            });
            next = now;
        }
        seen = now;
        let before = oracle.frame_counter()?;
        let bytes = oracle.read_memory(0, base, map.segment_length as usize)?;
        let after = oracle.frame_counter()?;
        if before == after {
            samples.push(parse_sample(&bytes, before, &map, base)?);
            next = before + 1;
        } else {
            if after > next {
                skipped.push(Skipped {
                    from: next,
                    to: after - 1,
                    counter_before: before,
                    counter_after: after,
                    why: if after > before + 1 {
                        format!(
                            "the guest's frame counter went from {before} to {after} across one \
                             read — more than one frame at once — so the frames between were never \
                             the current frame"
                        )
                    } else {
                        format!(
                            "the read that was opened on frame {before} was still open when the \
                             guest moved to {after}, so the bytes belong to neither frame with \
                             certainty and were dropped"
                        )
                    },
                });
            }
            next = after;
        }
    }

    options
        .recipe
        .confirm_answered(oracle)
        .map_err(MusicError::Coherence)?;

    // A segment that moved would make every later sample a different variable.
    let rerun = find_segment(oracle, &object, &map, &needles)?;
    if !rerun.views.contains(&base) {
        return Err(MusicError::Coherence(format!(
            "the STMIK segment moved during the trace: the trace read 0x{base:06X} and the \
             needles are now at {}; every sample after the move would be a different variable, \
             so nothing was written",
            rerun
                .views
                .iter()
                .map(|view| format!("0x{view:06X}"))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    locate_record.rerun_base = rerun.base;
    locate_record.rerun_views = rerun.views.clone();
    locate_record.rerun_frame = rerun.frame;
    let machine_at_end = read_words(oracle, base, &map, MACHINE_WORDS)?;
    if rerun.views != location.views {
        locate_record.aliasing = match (&locate_record.aliasing, &rerun.aliasing) {
            (Some(first), Some(second)) => {
                Some(format!("{first} At the end of the trace: {second}"))
            }
            (Some(first), None) => Some(first.clone()),
            (None, Some(second)) => Some(second.clone()),
            (None, None) => None,
        };
    }

    check_samples(&samples, &map)?;
    let check = behaviour(&samples);

    let text = render_trace(&samples);
    let trace_file = TraceFile {
        file: TRACE_FILE.to_string(),
        columns: COLUMNS.iter().map(|column| (*column).to_string()).collect(),
        rows: samples.len() as u64,
        bytes: text.len() as u64,
        sha256: sha256_hex(text.as_bytes()),
    };
    let (machine, machine_moved) = merge_machine_words(machine_at_start, machine_at_end);

    let manifest = TraceManifest {
        version: MANIFEST_VERSION,
        kind: TRACE_KIND.to_string(),
        oracle: oracle_name,
        upstream_commit: UPSTREAM_COMMIT.to_string(),
        exe: options.recipe.exe_name(),
        arguments: options.recipe.arguments.clone(),
        cycles: options.recipe.cycles,
        frame_origin: FRAME_ORIGIN.to_string(),
        skip_boot_frames: 0,
        boot: options.recipe.boot,
        keys: options.recipe.keys.clone(),
        coverage: Coverage::of(window, &frames_in(&samples)),
        trace: trace_file,
        object: map,
        locate: locate_record,
        machine,
        machine_moved,
        check,
        skipped,
    };

    write_trace(&options.out, &text, &manifest, options.force)?;
    Ok(manifest)
}

/// Read the named 16-bit words at the located base. No range is imposed: `_zmixspeed` is a
/// rate in Hz and `_zbufsize` a size in bytes; the byte-valued words are checked in
/// [`parse_sample`].
fn read_words(
    oracle: &mut dyn Oracle,
    base: u32,
    map: &SymbolMap,
    names: &[&str],
) -> Result<BTreeMap<String, i64>, MusicError> {
    let mut out = BTreeMap::new();
    for name in names {
        let offset = map.offset(name)?;
        let bytes = oracle.read_memory(0, base + offset, 2)?;
        if bytes.len() != 2 {
            return Err(MusicError::Coherence(format!(
                "reading {name} at 0x{:06X} returned {} byte(s)",
                base + offset,
                bytes.len()
            )));
        }
        out.insert(
            name.to_string(),
            i64::from(i16::from_le_bytes([bytes[0], bytes[1]])),
        );
    }
    Ok(out)
}

/// The machine's words as the manifest records them: the reading at the locate, plus, for a
/// word that moved, its reading at the end in the second map ([`TraceManifest::machine_moved`]).
fn merge_machine_words(
    start: BTreeMap<String, i64>,
    end: BTreeMap<String, i64>,
) -> (BTreeMap<String, i64>, BTreeMap<String, i64>) {
    let mut machine = start;
    let mut moved = BTreeMap::new();
    for (name, value) in end {
        match machine.get(&name) {
            Some(first) if *first != value => {
                moved.insert(name, value);
            }
            Some(_) => {}
            None => {
                machine.insert(name, value);
            }
        }
    }
    (machine, moved)
}

/// The player's words out of one read of the whole segment, at the frame both counter reads
/// agreed on.
fn parse_sample(
    bytes: &[u8],
    frame: u64,
    map: &SymbolMap,
    base: u32,
) -> Result<PlayerState, MusicError> {
    let word = |name: &str| -> Result<i16, MusicError> {
        let offset = map.offset(name)? as usize;
        let raw = bytes.get(offset..offset + 2).ok_or_else(|| {
            MusicError::Coherence(format!(
                "{name} at offset {offset} is past the {} bytes the segment read returned",
                bytes.len()
            ))
        })?;
        Ok(i16::from_le_bytes([raw[0], raw[1]]))
    };
    let byte = |name: &str| -> Result<u8, MusicError> {
        let value = word(name)?;
        u8::try_from(value).map_err(|_| {
            MusicError::Coherence(format!(
                "{name} reads {value}, which is not a byte value: the base 0x{base:06X} is not \
                 this segment's"
            ))
        })
    };
    Ok(PlayerState {
        frame,
        ord: byte("_np_ord")?,
        row: byte("_np_row")?,
        pat: byte("_np_pat")?,
        loop_: byte("_np_loop")?,
        speed: byte("_np_speed")?,
        tempo: byte("_np_tempo")?,
        zinfo: word("_np_zinfo")?,
        zplus: word("_np_zplus")?,
        zframe: word("_np_zframe")?,
    })
}

/// The checks a trace has to pass before it is written at all.
fn check_samples(samples: &[PlayerState], map: &SymbolMap) -> Result<(), MusicError> {
    if samples.is_empty() {
        return Err(MusicError::Coherence(
            "no frame was sampled: there is nothing to write".into(),
        ));
    }
    for pair in samples.windows(2) {
        if pair[1].frame <= pair[0].frame {
            return Err(MusicError::Coherence(format!(
                "the frame counter is not strictly increasing: {} then {}",
                pair[0].frame, pair[1].frame
            )));
        }
    }
    for state in samples {
        if state.row > 63 {
            return Err(MusicError::Coherence(format!(
                "at frame {} _np_row reads {}: an S3M row is 0..=63, so this is not the word the \
                 map put at offset 0x{:04X}",
                state.frame, state.row, map.words["_np_row"]
            )));
        }
        if state.speed == 0 || state.tempo == 0 {
            return Err(MusicError::Coherence(format!(
                "at frame {} the player reports speed {} tempo {}: both are 1..=255, so this is \
                 not the player's clock",
                state.frame, state.speed, state.tempo
            )));
        }
    }
    Ok(())
}

fn behaviour(samples: &[PlayerState]) -> Behaviour {
    let first = samples.first().expect("checked not empty");
    let mut row = [first.row, first.row];
    let mut ord = [first.ord, first.ord];
    let mut pat = [first.pat, first.pat];
    let mut speeds: Vec<u8> = Vec::new();
    let mut tempos: Vec<u8> = Vec::new();
    let mut zinfo_min = first.zinfo;
    let mut zinfo_max = first.zinfo;
    let mut first_advance = None;
    let mut previous: Option<&PlayerState> = None;
    let mut zframe_moves = 0u64;
    let mut zframe_total = 0i64;
    for state in samples {
        row = [row[0].min(state.row), row[1].max(state.row)];
        ord = [ord[0].min(state.ord), ord[1].max(state.ord)];
        pat = [pat[0].min(state.pat), pat[1].max(state.pat)];
        if !speeds.contains(&state.speed) {
            speeds.push(state.speed);
        }
        if !tempos.contains(&state.tempo) {
            tempos.push(state.tempo);
        }
        zinfo_min = zinfo_min.min(state.zinfo);
        zinfo_max = zinfo_max.max(state.zinfo);
        if let Some(earlier) = previous {
            if first_advance.is_none() && state.position_moved_from(earlier) {
                first_advance = Some(state.frame);
            }
            if earlier.zframe != state.zframe {
                zframe_moves += 1;
                zframe_total += i64::from(state.zframe) - i64::from(earlier.zframe);
            }
        }
        previous = Some(state);
    }
    Behaviour {
        row,
        ord,
        pat,
        speeds,
        tempos,
        zinfo_min,
        zinfo_max,
        first_advance,
        zframe_moves,
        zframe_total,
    }
}

fn render_trace(samples: &[PlayerState]) -> String {
    let mut text = String::with_capacity(samples.len() * 40 + 64);
    text.push_str(&PlayerState::header());
    text.push('\n');
    for state in samples {
        text.push_str(&state.line());
        text.push('\n');
    }
    text
}

/// Write the trace, then the manifest: a half-written directory has no manifest, and the
/// manifest is what makes it a capture.
fn write_trace(
    out: &Path,
    text: &str,
    manifest: &TraceManifest,
    force: bool,
) -> Result<(), MusicError> {
    let trace_path = out.join(TRACE_FILE);
    let manifest_path = out.join("manifest.json");
    if !force && (trace_path.exists() || manifest_path.exists()) {
        return Err(MusicError::Options(format!(
            "{} already holds a trace ({}); pass --force to replace it",
            out.display(),
            if trace_path.exists() {
                TRACE_FILE
            } else {
                "manifest.json"
            }
        )));
    }
    std::fs::create_dir_all(out).map_err(|e| MusicError::Io {
        path: out.to_path_buf(),
        message: e.to_string(),
    })?;
    std::fs::write(&trace_path, text).map_err(|e| MusicError::Io {
        path: trace_path.clone(),
        message: e.to_string(),
    })?;
    let json = serde_json::to_string_pretty(manifest)
        .map_err(|e| MusicError::Options(format!("cannot serialise the manifest: {e}")))?;
    std::fs::write(&manifest_path, format!("{json}\n")).map_err(|e| MusicError::Io {
        path: manifest_path,
        message: e.to_string(),
    })?;
    Ok(())
}

/// `cargo xtask music trace`.
pub fn trace_command(options: &TraceOptions) -> Result<ExitCode, MusicError> {
    validate_trace_options(options)?;
    let autoexec = options.recipe.autoexec_lines()?;
    let mut oracle = crate::oracle::dosbox::DosboxOracle::spawn(
        options.recipe.cycles,
        &options.recipe.cwd,
        &autoexec,
    )?;
    let manifest = trace(&mut oracle, options)?;
    oracle.shutdown()?;
    println!(
        "traced {} frame(s) into {} (window {}..{}, {}, oracle {}, base 0x{:06X})",
        manifest.trace.rows,
        options.out.display(),
        manifest.coverage.window[0],
        manifest.coverage.window[1],
        manifest.coverage.describe(),
        manifest.oracle,
        manifest.locate.base,
    );
    for skip in &manifest.skipped {
        println!(
            "  not sampled: frames {}..{} (the counter went {} -> {}) — {}",
            skip.from, skip.to, skip.counter_before, skip.counter_after, skip.why
        );
    }
    println!(
        "  music advances first at frame {}; row {}..{}, ord {}..{}, pat {}..{}, speed {:?}, \
         tempo {:?}",
        manifest
            .check
            .first_advance
            .map(|frame| frame.to_string())
            .unwrap_or_else(|| "never".into()),
        manifest.check.row[0],
        manifest.check.row[1],
        manifest.check.ord[0],
        manifest.check.ord[1],
        manifest.check.pat[0],
        manifest.check.pat[1],
        manifest.check.speeds,
        manifest.check.tempos,
    );
    for needle in &manifest.locate.needles {
        println!(
            "  needle at 0x{:04X} found at 0x{:06X} (sha256 {})",
            needle.offset,
            needle.found_at.unwrap_or(0),
            needle.sha256
        );
    }
    Ok(ExitCode::SUCCESS)
}

/// `--frames`: a list, a range, or a list of ranges, as one contiguous window. `A..B` is the
/// Rust range (B excluded), `A..=B` includes B; `0..1300` is frames 0 through 1299.
pub fn parse_window(spec: &str) -> Result<[u64; 2], MusicError> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err(MusicError::Options(
            "--frames is empty: give a range (0..1300), an inclusive range (0..=1299), or a \
             comma-separated list of frames"
                .into(),
        ));
    }
    let mut frames: Vec<u64> = Vec::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            return Err(MusicError::Options(format!(
                "--frames {spec:?} has an empty entry"
            )));
        }
        if let Some((start, end)) = part.split_once("..=") {
            let start = parse_frame(start, spec)?;
            let end = parse_frame(end, spec)?;
            if end < start {
                return Err(MusicError::Options(format!(
                    "--frames {part:?}: {} is before {}",
                    end, start
                )));
            }
            frames.extend(start..=end);
        } else if let Some((start, end)) = part.split_once("..") {
            let start = parse_frame(start, spec)?;
            let end = parse_frame(end, spec)?;
            if end <= start {
                return Err(MusicError::Options(format!(
                    "--frames {part:?} is empty: A..B is A up to but not including B, so B must be \
                     after A (use A..=B to include B)"
                )));
            }
            frames.extend(start..end);
        } else {
            frames.push(parse_frame(part, spec)?);
        }
    }
    frames.sort_unstable();
    for pair in frames.windows(2) {
        if pair[1] == pair[0] {
            return Err(MusicError::Options(format!(
                "--frames {spec:?} names frame {} twice",
                pair[0]
            )));
        }
    }
    let first = frames[0];
    let last = frames[frames.len() - 1];
    if last - first + 1 != frames.len() as u64 {
        return Err(MusicError::Options(format!(
            "--frames {spec:?} is not one contiguous run: {} frame(s) from {first} to {last} \
             would need {}, so there is a hole in the window",
            frames.len(),
            last - first + 1
        )));
    }
    Ok([first, last])
}

fn parse_frame(text: &str, spec: &str) -> Result<u64, MusicError> {
    text.trim().parse::<u64>().map_err(|e| {
        MusicError::Options(format!(
            "--frames {spec:?}: {} is not a frame index: {e}",
            text.trim()
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::object;
    use super::super::DEFAULT_OBJECT;
    use super::*;

    #[test]
    fn the_frame_window_is_half_open_and_checked_for_holes() {
        assert_eq!(parse_window("0..1300").unwrap(), [0, 1299]);
        assert_eq!(parse_window("0..=1299").unwrap(), [0, 1299]);
        assert_eq!(parse_window("5").unwrap(), [5, 5]);
        assert_eq!(parse_window("1,2,3").unwrap(), [1, 3]);
        assert_eq!(parse_window(" 0..5 , 5..9 ").unwrap(), [0, 8]);
        for bad in [
            "",
            "10..10",
            "10..0",
            "0,2",
            "0..5,9..12",
            "a..b",
            "0..=0..=3",
        ] {
            assert!(
                parse_window(bad).is_err(),
                "--frames {bad:?} must be refused"
            );
        }
    }

    /// A word whose two readings agree is recorded once; one only the end has is added; one
    /// that moved keeps its first reading and is recorded again in the second map.
    #[test]
    fn machine_readings_that_disagree_are_both_recorded() {
        let start = BTreeMap::from([
            ("_zoutputmode".to_string(), 5),
            ("_zmixspeed".to_string(), 20000),
        ]);
        assert_eq!(
            merge_machine_words(start.clone(), start.clone()),
            (start.clone(), BTreeMap::new())
        );

        let (merged, moved) = merge_machine_words(
            start.clone(),
            BTreeMap::from([("_zspeedadjust".to_string(), 0)]),
        );
        assert_eq!(merged.get("_zspeedadjust"), Some(&0));
        assert_eq!(merged.get("_zoutputmode"), Some(&5));
        assert!(moved.is_empty());

        let (machine, moved) = merge_machine_words(
            start,
            BTreeMap::from([
                ("_zoutputmode".to_string(), 5),
                ("_zmixspeed".to_string(), 40000),
            ]),
        );
        assert_eq!(machine.get("_zmixspeed"), Some(&20000));
        assert_eq!(moved, BTreeMap::from([("_zmixspeed".to_string(), 40000)]));
    }

    /// `parse_sample` reads the words out of one whole-segment read, and refuses a segment
    /// whose base is wrong: a `_np_row` of 300 is not a row.
    #[test]
    fn a_sample_reads_the_words_and_refuses_a_wrong_base() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        let mut bytes = vec![0u8; map.segment_length as usize];
        fn put(bytes: &mut [u8], map: &SymbolMap, name: &str, value: i16) {
            let at = map.offset(name).unwrap() as usize;
            bytes[at..at + 2].copy_from_slice(&value.to_le_bytes());
        }
        for (name, value) in [
            ("_np_ord", 25),
            ("_np_row", 7),
            ("_np_pat", 70),
            ("_np_loop", 1),
            ("_np_speed", 6),
            ("_np_tempo", 120),
            ("_np_zinfo", 0),
            ("_np_zplus", 2),
            ("_np_zframe", 385),
        ] {
            put(&mut bytes, &map, name, value);
        }
        let state = parse_sample(&bytes, 900, &map, 0x1051F0).unwrap();
        assert_eq!(
            state,
            PlayerState {
                frame: 900,
                ord: 25,
                row: 7,
                pat: 70,
                loop_: 1,
                speed: 6,
                tempo: 120,
                zinfo: 0,
                zplus: 2,
                zframe: 385,
            }
        );

        put(&mut bytes, &map, "_np_row", 300);
        match parse_sample(&bytes, 900, &map, 0x1051F0) {
            Err(MusicError::Coherence(message)) => {
                assert!(message.contains("0x1051F0"), "{message}")
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    /// Rows inside 0..=63, a speed and tempo that are 1..=255, and frames that strictly
    /// increase.
    #[test]
    fn the_coherence_checks_refuse_what_is_not_the_player() {
        let object = object();
        let map = symbol_map(Path::new(DEFAULT_OBJECT), &object).unwrap();
        let good = PlayerState {
            frame: 900,
            ord: 25,
            row: 7,
            pat: 70,
            loop_: 0,
            speed: 6,
            tempo: 120,
            zinfo: 0,
            zplus: 0,
            zframe: 0,
        };
        assert!(check_samples(&[good], &map).is_ok());
        assert!(check_samples(&[], &map).is_err(), "nothing to write");
        let mut rowed = good;
        rowed.row = 64;
        assert!(check_samples(&[rowed], &map).is_err());
        let mut stopped = good;
        stopped.speed = 0;
        assert!(check_samples(&[stopped], &map).is_err());
        assert!(
            check_samples(&[good, good], &map).is_err(),
            "frames must increase"
        );
    }
}
