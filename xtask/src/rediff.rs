//! `cargo xtask rediff`: every filed reference frame, re-rendered and re-compared in one
//! command.
//!
//! Before this, each part was checked against its filed capture by its own test, and each
//! test carried its own index mapping. This command holds those mappings in one table
//! ([`TABLE`], `rediff/table.rs`) copied from the tests that assert them, runs the release
//! binary once per entry with all of that entry's rendered indices in one `--dump-frames`,
//! compares each frame exactly where the table says (no ±1 search: every test already
//! states its index), and checks each frame's differing-pixel count against the count the
//! table registers for it.
//!
//! **The register is the point.** A frame that differs from the reference is not a failure
//! here when its difference is registered — an A-entry, measured and explained — and a
//! frame that matches is a failure when the register says it should not. Either way the
//! count moved, and a count that moves without the register moving with it is the
//! regression this command exists to catch.
//!
//! Exit codes are `compare`'s contract, reread for counts: **0** every frame at its
//! registered count, **1** any frame off it, **2** could not compare — a missing frame, a
//! geometry mismatch, a capture with neither an entry nor a `PENDING` line, an entry with
//! no capture, a frame an entry leaves out or names wrongly, a run that failed. A geometry
//! mismatch is never a count: two screens of different shape have no pixel difference.
//!
//! `--out` defaults to `out/rediff`, and is refused inside `reference/` and inside the
//! tree being compared, resolved paths and all (`compare::refuse_output_inside`, which
//! collapses a `..` behind a component that does not exist yet — D16's bypass).

mod table;

pub use table::{Entry, Frame, Plane, PENDING, TABLE};

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::compare::{
    capture_directories, compare_indexed, compare_rgb, read_manifest, refuse_output_inside,
    FrameDiff, FrameRef, Manifest,
};

/// Where the per-entry dumps and `report.json` go when `--out` is not given.
pub const DEFAULT_OUT: &str = "out/rediff";

/// The tree the table's capture paths are below.
pub const DEFAULT_REFERENCE: &str = "reference/parts";

/// The input-only tree: nothing this command writes may land in it, whatever `--reference`
/// names.
const REFERENCE_ROOT: &str = "reference";

const REPORT_FILE: &str = "report.json";

/// This report's own format version.
const REPORT_VERSION: u32 = 1;

impl fmt::Display for Plane {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Plane::Indexed => "indexed",
            Plane::Rgb => "rgb",
        })
    }
}

/// What the command line asked for.
pub struct Options {
    /// The binary to run. Without it, `cargo build --release -p secondreality` and that.
    pub binary: Option<PathBuf>,
    /// Only these captures (or the captures under these directories).
    pub only: Vec<String>,
    pub out: Option<PathBuf>,
    /// The tree the table's captures are below; `reference/parts` by default.
    pub reference: Option<PathBuf>,
    /// A JSON table in place of the compiled one — what the command's own tests run.
    pub table: Option<PathBuf>,
}

/// The table a run uses: the compiled one, or a JSON file of the same shape.
struct Table {
    entries: Vec<Entry>,
    pending: Vec<&'static str>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TableFile {
    #[serde(default)]
    pending: Vec<String>,
    entries: Vec<EntryFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EntryFile {
    capture: String,
    args: Vec<String>,
    frames: Vec<FrameFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameFile {
    reference: u32,
    rendered: u32,
    plane: PlaneFile,
    registered: u32,
    register: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum PlaneFile {
    Indexed,
    Rgb,
}

/// A string that lives as long as the process: a table read from a file has to have the
/// compiled table's shape, and this command runs once and exits.
fn leak(text: String) -> &'static str {
    Box::leak(text.into_boxed_str())
}

fn load_table(path: &Path) -> Result<Table, String> {
    let json = fs::read_to_string(path).map_err(|e| format!("--table {}: {e}", path.display()))?;
    let file: TableFile =
        serde_json::from_str(&json).map_err(|e| format!("--table {}: {e}", path.display()))?;
    let entries = file
        .entries
        .into_iter()
        .map(|entry| Entry {
            capture: leak(entry.capture),
            args: Vec::leak(entry.args.into_iter().map(leak).collect()),
            frames: Vec::leak(
                entry
                    .frames
                    .into_iter()
                    .map(|frame| Frame {
                        reference: frame.reference,
                        rendered: frame.rendered,
                        plane: match frame.plane {
                            PlaneFile::Indexed => Plane::Indexed,
                            PlaneFile::Rgb => Plane::Rgb,
                        },
                        registered: frame.registered,
                        register: leak(frame.register),
                    })
                    .collect(),
            ),
        })
        .collect();
    Ok(Table {
        entries,
        pending: file.pending.into_iter().map(leak).collect(),
    })
}

/// Whether `name` — a `PENDING` line or an `--only` — covers `capture`: the capture
/// itself, or a directory above it.
fn covers(name: &str, capture: &str) -> bool {
    capture == name || capture.starts_with(&format!("{name}/"))
}

/// One frame's line in the report.
#[derive(Debug, Serialize)]
struct Row {
    capture: String,
    reference: u32,
    rendered: u32,
    plane: String,
    /// `None` when the frame could not be compared; the reason is in the problems.
    differing_pixels: Option<u64>,
    total_pixels: Option<u64>,
    max_channel_delta: Option<u8>,
    registered: u32,
    register: String,
    /// `ok`, `DIFFERS` or `not compared`.
    verdict: &'static str,
}

#[derive(Debug, Serialize)]
struct PendingRow {
    capture: String,
    pending: String,
}

#[derive(Debug, Serialize)]
struct Summary {
    entries: usize,
    frames: usize,
    ok: usize,
    differs: usize,
    not_compared: usize,
    pending: usize,
    problems: usize,
    exit: u8,
}

#[derive(Debug, Serialize)]
struct Report {
    version: u32,
    binary: String,
    frames: Vec<Row>,
    pending: Vec<PendingRow>,
    problems: Vec<String>,
    summary: Summary,
}

/// What one entry's run produced.
struct Outcome {
    rows: Vec<Row>,
    problems: Vec<String>,
}

/// Refuse an `--out` inside the reference — the repository's, and the tree being compared
/// — before anything is built, run or written.
fn refuse_out(out: &Path, reference: &Path) -> Result<(), String> {
    for input in [Path::new(REFERENCE_ROOT), reference] {
        refuse_output_inside(
            out,
            input,
            "--out",
            "the reference captures are inputs, not outputs, and a dump or a report written \
             among them would dirty the frames every later diff is measured against",
            &format!("Pass --out somewhere else; the default is {DEFAULT_OUT}"),
        )?;
    }
    Ok(())
}

/// `cargo xtask rediff`.
///
/// Returns the exit code: 0, 1 or 2 as the module says. A refusal before anything ran —
/// an `--out` inside the reference, a table that cannot be read, a release build that
/// failed — is an `Err`, which `main` also ends with 2, having written nothing.
pub fn command(options: &Options) -> Result<ExitCode, String> {
    let reference = options
        .reference
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_REFERENCE));
    let out = options
        .out
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_OUT));
    refuse_out(&out, &reference)?;

    let table = match &options.table {
        Some(path) => load_table(path)?,
        None => Table {
            entries: TABLE.to_vec(),
            pending: PENDING.to_vec(),
        },
    };
    let selected = |capture: &str| {
        options.only.is_empty() || options.only.iter().any(|only| covers(only, capture))
    };

    let mut problems = Vec::new();
    let filed: Vec<String> = capture_directories(&reference)
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|found| found.name)
        .collect();

    // The table against the tree: every capture has one entry or is under a PENDING line,
    // and every entry and PENDING line names something filed.
    let mut pending_rows = Vec::new();
    for capture in filed.iter().filter(|capture| selected(capture)) {
        let entries = table
            .entries
            .iter()
            .filter(|e| e.capture == capture)
            .count();
        let pending: Vec<&str> = table
            .pending
            .iter()
            .copied()
            .filter(|pending| covers(pending, capture))
            .collect();
        match (entries, pending.first()) {
            (0, None) => problems.push(format!(
                "{capture}: a filed capture with no entry in the table and no PENDING line"
            )),
            (0, Some(line)) => pending_rows.push(PendingRow {
                capture: capture.clone(),
                pending: line.to_string(),
            }),
            (1, None) => {}
            (_, None) => problems.push(format!("{capture}: {entries} entries in the table")),
            (_, Some(line)) => problems.push(format!(
                "{capture}: an entry in the table and PENDING under {line:?}"
            )),
        }
    }
    for entry in table.entries.iter().filter(|e| selected(e.capture)) {
        if !filed.iter().any(|capture| capture == entry.capture) {
            problems.push(format!(
                "{}: the table's entry names no capture under {}",
                entry.capture,
                reference.display()
            ));
        }
    }
    for pending in &table.pending {
        let in_scope = options.only.is_empty()
            || options
                .only
                .iter()
                .any(|only| covers(only, pending) || covers(pending, only));
        if in_scope && !filed.iter().any(|capture| covers(pending, capture)) {
            problems.push(format!(
                "{pending}: a PENDING line that covers no capture under {}",
                reference.display()
            ));
        }
    }

    let entries: Vec<Entry> = table
        .entries
        .iter()
        .copied()
        .filter(|entry| selected(entry.capture))
        .filter(|entry| filed.iter().any(|capture| capture == entry.capture))
        .collect();
    if !options.only.is_empty() && entries.is_empty() && pending_rows.is_empty() {
        return Err(format!(
            "--only {} names no capture in the table",
            options.only.join(", ")
        ));
    }

    let binary = match &options.binary {
        Some(binary) => binary.clone(),
        None => build_release()?,
    };

    let outcomes = run_entries(&entries, &binary, &reference, &out);
    let mut rows = Vec::new();
    for outcome in outcomes {
        rows.extend(outcome.rows);
        problems.extend(outcome.problems);
    }

    print_rows(&rows);
    for row in &pending_rows {
        println!(
            "{}  not compared: PENDING ({:?}; a later task measures its mapping)",
            row.capture, row.pending
        );
    }
    for problem in &problems {
        println!("cannot compare: {problem}");
    }

    let ok = rows.iter().filter(|row| row.verdict == "ok").count();
    let differs = rows.iter().filter(|row| row.verdict == "DIFFERS").count();
    let not_compared = rows
        .iter()
        .filter(|row| row.verdict == "not compared")
        .count();
    let exit: u8 = if !problems.is_empty() {
        2
    } else if differs > 0 {
        1
    } else {
        0
    };
    println!(
        "entries: {}; frames: {} ({ok} at their registered count, {differs} off it, \
         {not_compared} not compared); pending captures: {}; problems: {}",
        entries.len(),
        rows.len(),
        pending_rows.len(),
        problems.len()
    );
    println!(
        "verdict: {}",
        match exit {
            0 => "every frame at its registered count",
            1 => "a frame is off its registered count",
            _ => "could not compare",
        }
    );

    let summary = Summary {
        entries: entries.len(),
        frames: rows.len(),
        ok,
        differs,
        not_compared,
        pending: pending_rows.len(),
        problems: problems.len(),
        exit,
    };
    let report = Report {
        version: REPORT_VERSION,
        binary: binary.display().to_string(),
        frames: rows,
        pending: pending_rows,
        problems,
        summary,
    };
    fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", out.display()))?;
    let path = out.join(REPORT_FILE);
    let mut json = serde_json::to_string_pretty(&report)
        .map_err(|e| format!("cannot serialise {}: {e}", path.display()))?;
    json.push('\n');
    fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))?;
    println!("report: {}", path.display());
    Ok(ExitCode::from(exit))
}

/// One line a frame, in columns.
fn print_rows(rows: &[Row]) {
    let width = rows
        .iter()
        .map(|row| row.capture.len())
        .max()
        .unwrap_or(0)
        .max(7);
    let register = rows
        .iter()
        .map(|row| row.register.len())
        .max()
        .unwrap_or(0)
        .max(8);
    println!(
        "{:width$}  {:>9}  {:>8}  {:7}  {:>9}  {:>10}  {:register$}  verdict",
        "capture", "reference", "rendered", "plane", "differing", "registered", "register"
    );
    for row in rows {
        let differing = row
            .differing_pixels
            .map_or_else(|| "-".to_string(), |count| count.to_string());
        println!(
            "{:width$}  {:>9}  {:>8}  {:7}  {:>9}  {:>10}  {:register$}  {}",
            row.capture,
            row.reference,
            row.rendered,
            row.plane,
            differing,
            row.registered,
            row.register,
            row.verdict
        );
    }
}

/// `cargo build --release -p secondreality`, and the executable it reports.
fn build_release() -> Result<PathBuf, String> {
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    println!("building: cargo build --release -p secondreality");
    let output = Command::new(&cargo)
        .args([
            "build",
            "--release",
            "-p",
            "secondreality",
            "--bin",
            "secondreality",
            "--message-format=json-render-diagnostics",
        ])
        .stderr(Stdio::inherit())
        .output()
        .map_err(|e| format!("cannot run cargo: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "cargo build --release -p secondreality exited with {}",
            output.status
        ));
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if message["reason"] == "compiler-artifact" && message["target"]["name"] == "secondreality"
        {
            if let Some(executable) = message["executable"].as_str() {
                return Ok(PathBuf::from(executable));
            }
        }
    }
    Err("cargo build --release -p secondreality reported no secondreality executable".into())
}

/// Every entry's run, several at once, in the table's order.
fn run_entries(entries: &[Entry], binary: &Path, reference: &Path, out: &Path) -> Vec<Outcome> {
    let workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .min(entries.len().max(1));
    let next = AtomicUsize::new(0);
    let outcomes: Mutex<Vec<Option<Outcome>>> =
        Mutex::new((0..entries.len()).map(|_| None).collect());
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(|| loop {
                let at = next.fetch_add(1, Ordering::SeqCst);
                let Some(entry) = entries.get(at) else {
                    break;
                };
                let outcome = run_entry(entry, binary, reference, out);
                outcomes.lock().unwrap()[at] = Some(outcome);
            });
        }
    });
    outcomes
        .into_inner()
        .unwrap()
        .into_iter()
        .map(|outcome| outcome.expect("every entry ran"))
        .collect()
}

/// A row for a frame that could not be compared.
fn not_compared(entry: &Entry, frame: &Frame) -> Row {
    Row {
        capture: entry.capture.to_string(),
        reference: frame.reference,
        rendered: frame.rendered,
        plane: frame.plane.to_string(),
        differing_pixels: None,
        total_pixels: None,
        max_channel_delta: None,
        registered: frame.registered,
        register: frame.register.to_string(),
        verdict: "not compared",
    }
}

/// One entry: read its capture, run the binary once, compare every frame.
fn run_entry(entry: &Entry, binary: &Path, reference: &Path, out: &Path) -> Outcome {
    let not_any = |problem: String| Outcome {
        rows: entry
            .frames
            .iter()
            .map(|frame| not_compared(entry, frame))
            .collect(),
        problems: vec![problem],
    };
    let capture_dir = reference.join(entry.capture);
    let capture = match read_manifest(&capture_dir) {
        Ok(manifest) => manifest,
        Err(error) => return not_any(format!("{}: {error}", entry.capture)),
    };

    let mut problems = Vec::new();
    // The table against the capture's own manifest: no filed frame left out, no frame
    // listed that is not filed.
    let filed: BTreeSet<u32> = capture.frames.iter().map(|frame| frame.index).collect();
    let listed: BTreeSet<u32> = entry.frames.iter().map(|frame| frame.reference).collect();
    for missing in filed.difference(&listed) {
        problems.push(format!(
            "{}: filed frame {missing} is in no row of the table's entry",
            entry.capture
        ));
    }

    let dump = out.join(entry.capture).join("rendered");
    if dump.exists() {
        if let Err(error) = fs::remove_dir_all(&dump) {
            return not_any(format!("{}: {error}", dump.display()));
        }
    }
    if let Err(error) = fs::create_dir_all(&dump) {
        return not_any(format!("{}: {error}", dump.display()));
    }
    let indices: BTreeSet<u32> = entry.frames.iter().map(|frame| frame.rendered).collect();
    let list = indices
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let started = std::time::Instant::now();
    let output = Command::new(binary)
        .args(entry.args)
        .args(["--headless", "--dump-frames", &list, "--dump-dir"])
        .arg(&dump)
        .stdin(Stdio::null())
        .output();
    let command_line = format!(
        "{} {} --headless --dump-frames {list} --dump-dir {}",
        binary.display(),
        entry.args.join(" "),
        dump.display()
    );
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            return not_any(format!(
                "{}: cannot run {command_line}: {error}",
                entry.capture
            ))
        }
    };
    let log = out.join(entry.capture).join("run.log");
    if let Ok(mut file) = fs::File::create(&log) {
        let _ = writeln!(file, "$ {command_line}");
        let _ = file.write_all(&output.stdout);
        let _ = file.write_all(&output.stderr);
        let _ = writeln!(
            file,
            "# {} in {:.1} s",
            output.status,
            started.elapsed().as_secs_f64()
        );
    }
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return not_any(format!(
            "{}: the run exited with {}: {command_line}: {}",
            entry.capture,
            output.status,
            stderr.trim()
        ));
    }
    let rendered = match read_manifest(&dump) {
        Ok(manifest) => manifest,
        Err(error) => return not_any(format!("{}: the dump: {error}", entry.capture)),
    };

    let mut rows = Vec::new();
    for frame in entry.frames {
        match compare_frame(&capture_dir, &capture, &dump, &rendered, frame) {
            Ok(diff) => rows.push(Row {
                capture: entry.capture.to_string(),
                reference: frame.reference,
                rendered: frame.rendered,
                plane: frame.plane.to_string(),
                differing_pixels: Some(diff.differing_pixels),
                total_pixels: Some(diff.total_pixels),
                max_channel_delta: Some(diff.max_channel_delta),
                registered: frame.registered,
                register: frame.register.to_string(),
                verdict: if diff.differing_pixels == u64::from(frame.registered) {
                    "ok"
                } else {
                    "DIFFERS"
                },
            }),
            Err(problem) => {
                problems.push(format!(
                    "{} reference {} at rendered {} ({}): {problem}",
                    entry.capture, frame.reference, frame.rendered, frame.plane
                ));
                rows.push(not_compared(entry, frame));
            }
        }
    }
    Outcome { rows, problems }
}

/// The frame `index` in `manifest`, or why there is none.
fn frame_at<'a>(manifest: &'a Manifest, index: u32, side: &str) -> Result<&'a FrameRef, String> {
    manifest
        .frames
        .iter()
        .find(|frame| frame.index == index)
        .ok_or_else(|| format!("the {side} holds no frame {index}"))
}

fn read(dir: &Path, file: &str) -> Result<Vec<u8>, String> {
    let path = dir.join(file);
    fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Compare one frame of the table, exactly where it says, in the plane it names.
fn compare_frame(
    capture_dir: &Path,
    capture: &Manifest,
    dump: &Path,
    rendered: &Manifest,
    frame: &Frame,
) -> Result<FrameDiff, String> {
    let reference_frame = frame_at(capture, frame.reference, "capture")?;
    let rendered_frame = frame_at(rendered, frame.rendered, "dump")
        .map_err(|_| format!("the dump holds no rendered frame {}", frame.rendered))?;
    let reference_geometry = reference_frame.geometry(capture);
    let rendered_geometry = rendered_frame.geometry(rendered);
    if reference_geometry != rendered_geometry {
        return Err(format!(
            "geometry mismatch: the reference is {}x{} and the rendered frame is {}x{}",
            reference_geometry.0, reference_geometry.1, rendered_geometry.0, rendered_geometry.1
        ));
    }
    let pixels = usize::from(reference_geometry.0) * usize::from(reference_geometry.1);
    let (reference_bytes, rendered_bytes, expected) = match frame.plane {
        Plane::Indexed => {
            let reference_idx = reference_frame.idx.as_deref().ok_or_else(|| {
                format!("the capture files no .idx for frame {}", frame.reference)
            })?;
            let rendered_idx = rendered_frame
                .idx
                .as_deref()
                .ok_or_else(|| format!("the dump holds no .idx for frame {}", frame.rendered))?;
            (
                read(capture_dir, reference_idx)?,
                read(dump, rendered_idx)?,
                pixels,
            )
        }
        Plane::Rgb => (
            read(capture_dir, &reference_frame.rgb)?,
            read(dump, &rendered_frame.rgb)?,
            pixels * 3,
        ),
    };
    for (side, bytes) in [
        ("reference", &reference_bytes),
        ("rendered", &rendered_bytes),
    ] {
        if bytes.len() != expected {
            return Err(format!(
                "the {side} {} file holds {} bytes; {}x{} needs {expected}",
                frame.plane,
                bytes.len(),
                reference_geometry.0,
                reference_geometry.1
            ));
        }
    }
    match frame.plane {
        Plane::Indexed => compare_indexed(&reference_bytes, &rendered_bytes),
        Plane::Rgb => compare_rgb(&reference_bytes, &rendered_bytes),
    }
    .map_err(|e| e.to_string())
}
