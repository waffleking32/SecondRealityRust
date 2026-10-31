//! `xtask reference verify`: every capture under a tree against the digests its own manifest
//! records (D23).
//!
//! A frame capture passes three checks: every file the manifest names exists at the length its
//! geometry implies (three bytes a pixel for the RGB frame, one for the index plane); every
//! `frame_*` file in the directory is named; every recorded digest, of the RGB frame and of the
//! index plane, matches. A frame with no recorded digest is a finding, not a pass. A music
//! trace (`kind: music-trace`) is checked by [`inspect_trace`]; a `kind` this tool does not
//! know is refused, never skipped.
//!
//! Exit **0** every capture checked out, **1** any finding (including a file that is absent),
//! **2** could not check: nothing at the tree path, no manifest below it, a manifest it cannot
//! read or whose version it does not know, a capture that names no frames, an unreadable file.
//! Nothing is written.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::capture::{capture_directories, CaptureDir, MANIFEST_FILE};
use crate::reference::{sha256_hex, CaptureManifest, FrameRef, MANIFEST_VERSION};

/// What every frame file's name starts with.
const FRAME_PREFIX: &str = "frame_";

/// One file a frame entry names, with what the manifest says about it.
struct NamedFile<'a> {
    name: &'a str,
    /// Bytes a pixel: 3 for the RGB frame, 1 for the index plane.
    per_pixel: usize,
    /// The manifest field the digest is recorded under, and what the file is.
    field: &'static str,
    what: &'static str,
    digest: Option<&'a str>,
}

/// The files one frame entry names: its RGB frame, and its index plane when it has one.
fn frame_files(frame: &FrameRef) -> Vec<NamedFile<'_>> {
    let mut files = vec![NamedFile {
        name: &frame.rgb,
        per_pixel: 3,
        field: "sha256",
        what: "RGB frame",
        digest: frame.sha256.as_deref(),
    }];
    if let Some(idx) = &frame.idx {
        files.push(NamedFile {
            name: idx,
            per_pixel: 1,
            field: "idx_sha256",
            what: "index plane",
            digest: frame.idx_sha256.as_deref(),
        });
    }
    files
}

/// One thing wrong with a capture: the file it is about, by its path below the
/// tree, and what is wrong with it.
#[derive(Debug, PartialEq, Eq)]
struct Finding {
    path: String,
    what: String,
}

/// What checking one capture found.
#[derive(Debug, Default)]
struct Inspection {
    /// How many named files were readable.
    files: usize,
    bytes: u64,
    /// How many rows a trace holds; zero for a frame capture.
    rows: u64,
    findings: Vec<Finding>,
}

/// What a manifest says it is. No `kind` is a frame capture (D23's additive-field rule).
enum Kind {
    Frames,
    MusicTrace,
}

/// The kind a manifest declares, or a refusal naming it.
fn manifest_kind(path: &Path, text: &str) -> Result<Kind, String> {
    let value: serde_json::Value =
        serde_json::from_str(text).map_err(|e| format!("{}: {e}", path.display()))?;
    match value.get("kind") {
        None => Ok(Kind::Frames),
        Some(serde_json::Value::String(kind)) if kind == crate::music::TRACE_KIND => {
            Ok(Kind::MusicTrace)
        }
        Some(serde_json::Value::String(kind)) if kind == "frames" => Ok(Kind::Frames),
        Some(other) => Err(format!(
            "{} says it is a {other} capture; this tool reads frame captures (which declare no \
             `kind`, or \"frames\") and {} traces. A kind it does not know is refused rather than \
             skipped: a check that reports nothing is indistinguishable from one that passed",
            path.display(),
            crate::music::TRACE_KIND,
        )),
    }
}

/// A music trace's checks: the file the manifest names is there, hashes as recorded and has the
/// recorded length; every row parses into the declared columns and there are `rows` of them;
/// the frames strictly increase; and the coverage the manifest claims is the coverage
/// recomputed from the file's own frames, field by field.
fn inspect_trace(capture: &CaptureDir, text: &str) -> Result<Inspection, String> {
    let mut inspection = Inspection::default();
    let path = capture.dir.join(MANIFEST_FILE);
    let manifest: crate::music::TraceManifest =
        serde_json::from_str(text).map_err(|e| format!("{}: {e}", path.display()))?;
    check_version(&path, manifest.version)?;

    let trace_path = capture.dir.join(&manifest.trace.file);
    let bytes = match fs::read(&trace_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            inspection.findings.push(Finding {
                path: under(capture, &manifest.trace.file),
                what: "the manifest names it as this trace's rows and there is no such file"
                    .to_string(),
            });
            return Ok(inspection);
        }
        Err(error) => return Err(format!("cannot read {}: {error}", trace_path.display())),
    };
    inspection.files = 1;
    inspection.bytes = bytes.len() as u64;

    let actual = sha256_hex(&bytes);
    if actual != manifest.trace.sha256 {
        inspection.findings.push(Finding {
            path: under(capture, &manifest.trace.file),
            what: format!(
                "the manifest records sha256 {} and the file is {actual}",
                manifest.trace.sha256
            ),
        });
    }
    if bytes.len() as u64 != manifest.trace.bytes {
        inspection.findings.push(Finding {
            path: under(capture, &manifest.trace.file),
            what: format!(
                "the file is {} bytes where the manifest records {}",
                bytes.len(),
                manifest.trace.bytes
            ),
        });
    }

    // A file that is not text is a finding about the file, not a failure to check.
    let expected_header = crate::music::PlayerState::header();
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(error) => {
            inspection.findings.push(Finding {
                path: under(capture, &manifest.trace.file),
                what: format!(
                    "it is not UTF-8 text, so it is not a trace of rows: {error} (the file is \
                     {} bytes)",
                    inspection.bytes
                ),
            });
            return Ok(inspection);
        }
    };
    let mut frames: Vec<u64> = Vec::new();
    let mut rows = 0u64;
    for (number, line) in text.lines().enumerate() {
        if number == 0 {
            let header = line.trim_start_matches('\u{feff}');
            if header != expected_header {
                inspection.findings.push(Finding {
                    path: under(capture, &manifest.trace.file),
                    what: format!(
                        "its first line is {header:?} where the manifest's columns are \
                         {expected_header:?}"
                    ),
                });
            }
            continue;
        }
        if line.trim().is_empty() {
            continue;
        }
        match crate::music::PlayerState::parse(line) {
            Ok(state) => {
                rows += 1;
                if let Some(previous) = frames.last() {
                    if state.frame <= *previous {
                        inspection.findings.push(Finding {
                            path: under(capture, &manifest.trace.file),
                            what: format!(
                                "row {number} is frame {} after frame {previous}: a trace's frames \
                                 are strictly increasing",
                                state.frame
                            ),
                        });
                    }
                }
                frames.push(state.frame);
            }
            Err(error) => inspection.findings.push(Finding {
                path: under(capture, &manifest.trace.file),
                what: format!("row {number} does not read as a trace row: {error}"),
            }),
        }
    }
    inspection.rows = rows;
    if rows != manifest.trace.rows {
        inspection.findings.push(Finding {
            path: under(capture, &manifest.trace.file),
            what: format!(
                "the file holds {rows} row(s) and the manifest records {}",
                manifest.trace.rows
            ),
        });
    }
    if rows == 0 {
        inspection.findings.push(Finding {
            path: under(capture, &manifest.trace.file),
            what: "it holds no rows at all, so there is nothing to check".to_string(),
        });
    }

    if !frames.is_empty() {
        let recomputed = crate::music::Coverage::of(manifest.coverage.window, &frames);
        for disagreement in recomputed.disagreements(&manifest.coverage) {
            inspection.findings.push(Finding {
                path: under(capture, MANIFEST_FILE),
                what: format!("the coverage the manifest records does not match the file — {disagreement}"),
            });
        }
    }
    Ok(inspection)
}

/// `cargo xtask reference verify`: exit 0 or 1 as the module says; `Err` is exit 2.
pub fn command(tree: &Path) -> Result<ExitCode, String> {
    let captures = capture_directories(tree)?;
    let mut findings = 0usize;
    let mut files = 0usize;
    let mut bytes = 0u64;

    for capture in &captures {
        let (path, text) = read_manifest_text(&capture.dir)?;
        let inspection = match manifest_kind(&path, &text)? {
            Kind::Frames => {
                let manifest = parse_manifest(&path, &text)?;
                // Nothing to check is not "checked out": exit 2.
                if manifest.frames.is_empty() {
                    return Err(format!(
                        "{}: the manifest names no frames, so there is nothing to check against \
                         it — a capture that holds nothing checkable is not a capture that \
                         checked out",
                        capture.dir.join(MANIFEST_FILE).display()
                    ));
                }
                let inspection = inspect(capture, &manifest)?;
                if inspection.findings.is_empty() {
                    println!(
                        "ok   {}: {} frame(s), {} file(s)",
                        name_of(capture),
                        manifest.frames.len(),
                        inspection.files
                    );
                }
                inspection
            }
            Kind::MusicTrace => {
                let inspection = inspect_trace(capture, &text)?;
                if inspection.findings.is_empty() {
                    println!(
                        "ok   {}: music trace, {} row(s), {} file(s)",
                        name_of(capture),
                        inspection.rows,
                        inspection.files
                    );
                }
                inspection
            }
        };
        files += inspection.files;
        bytes += inspection.bytes;
        findings += inspection.findings.len();
        for finding in &inspection.findings {
            println!("FAIL {}: {}", finding.path, finding.what);
        }
    }

    if findings == 0 {
        println!(
            "verdict: ok — {} capture(s), {files} file(s), {bytes} bytes",
            captures.len()
        );
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "verdict: {findings} finding(s) — {} capture(s), {files} file(s) checked",
        captures.len()
    );
    Ok(ExitCode::from(1))
}

/// A capture's manifest as it is on disk, with its path for the messages.
fn read_manifest_text(dir: &Path) -> Result<(PathBuf, String), String> {
    let path = dir.join(MANIFEST_FILE);
    let text =
        fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    Ok((path, text))
}

fn check_version(path: &Path, version: u32) -> Result<(), String> {
    if version != MANIFEST_VERSION {
        return Err(format!(
            "{} says version {version}; this tool reads version {MANIFEST_VERSION}",
            path.display()
        ));
    }
    Ok(())
}

fn parse_manifest(path: &Path, text: &str) -> Result<CaptureManifest, String> {
    let manifest: CaptureManifest =
        serde_json::from_str(text).map_err(|e| format!("{}: {e}", path.display()))?;
    check_version(path, manifest.version)?;
    Ok(manifest)
}

/// The three checks, on one frame capture.
fn inspect(capture: &CaptureDir, manifest: &CaptureManifest) -> Result<Inspection, String> {
    let mut inspection = Inspection::default();
    let mut named: BTreeSet<String> = BTreeSet::new();

    for frame in &manifest.frames {
        for file in frame_files(frame) {
            named.insert(file.name.to_owned());
            let path = capture.dir.join(file.name);
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                // Absent is a finding; unreadable is exit 2.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    inspection.findings.push(Finding {
                        path: under(capture, file.name),
                        what: format!(
                            "the manifest names it as frame {}'s {} and there is no such file",
                            frame.index, file.what
                        ),
                    });
                    continue;
                }
                Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
            };
            inspection.files += 1;
            inspection.bytes += bytes.len() as u64;

            let expected = file.per_pixel * usize::from(manifest.width) * usize::from(manifest.height);
            if bytes.len() != expected {
                inspection.findings.push(Finding {
                    path: under(capture, file.name),
                    what: format!(
                        "the file is {} bytes where the manifest's {}x{} at {} byte(s) a pixel \
                         is {expected}",
                        bytes.len(),
                        manifest.width,
                        manifest.height,
                        file.per_pixel
                    ),
                });
            }

            match file.digest {
                None => inspection.findings.push(Finding {
                    path: under(capture, file.name),
                    what: format!(
                        "the manifest records no {} for it, so nothing checks these bytes; \
                         re-capture it with `xtask reference capture`, which records one",
                        file.field
                    ),
                }),
                Some(recorded) => {
                    let actual = sha256_hex(&bytes);
                    if actual != recorded {
                        inspection.findings.push(Finding {
                            path: under(capture, file.name),
                            what: format!(
                                "the manifest records {} {recorded} and the file is {actual}",
                                file.field
                            ),
                        });
                    }
                }
            }
        }
    }

    // Every `frame_*` file in the directory has to be one the manifest names.
    let cannot_list = |e: std::io::Error| format!("cannot list {}: {e}", capture.dir.display());
    let entries = fs::read_dir(&capture.dir).map_err(cannot_list)?;
    let mut loose = Vec::new();
    for entry in entries {
        let entry = entry.map_err(cannot_list)?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(FRAME_PREFIX) && !named.contains(&name) {
            loose.push(name);
        }
    }
    loose.sort();
    for name in loose {
        inspection.findings.push(Finding {
            path: under(capture, &name),
            what: format!(
                "no frame in the manifest names it: a {FRAME_PREFIX}* file the manifest does not \
                 name is what a careless edit looks like"
            ),
        });
    }

    Ok(inspection)
}

/// A capture's name as the report prints it: its path below the tree, or the
/// tree itself when the root holds the manifest.
fn name_of(capture: &CaptureDir) -> &str {
    if capture.name.is_empty() {
        "(the tree's own directory)"
    } else {
        &capture.name
    }
}

/// A file's path as a reader finds it: the capture's name below the tree, then
/// the file.
fn under(capture: &CaptureDir, file: &str) -> String {
    if capture.name.is_empty() {
        file.to_owned()
    } else {
        format!("{}/{}", capture.name, file)
    }
}
