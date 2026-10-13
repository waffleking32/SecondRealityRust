//! `xtask reference verify`: the reference tree against the digests its own
//! manifests record — and `xtask reference backfill`, the one-shot that writes
//! those digests into a tree filed before `capture` recorded them.
//!
//! Phase 1 pinned the tree's bytes in `docs/fidelity/reference-indices.md`, a
//! prose document, and its close-out said what that leaves open: no test reads
//! `reference/`, and comparing the tree with itself catches a *missing or
//! truncated* file but never **a byte changed in place**. The document cannot
//! be the machine's record: the close-out measured it at 90 rows in two naming
//! shapes against 105 `frame_*` files, 24 of which have no row by name at all,
//! and 10 carrying two or three different digests — a verifier resting on it
//! would have failed on 24 correctly-filed files the day it was written, and
//! would stop checking whenever a sentence was reworded. So the digest is
//! written where the file is: `reference capture` puts the `sha256` of each
//! frame file into that frame's entry in the manifest beside it
//! (`reference.rs`), and this tool walks the tree with `compare`'s
//! all-the-way-down rule and checks every capture against its own manifest.
//!
//! **Three checks per capture**, each failure naming the file it is about:
//!
//! 1. every file the manifest names exists, with the length the manifest's
//!    geometry implies (three bytes a pixel for the RGB frame, one for the
//!    index plane);
//! 2. every `frame_*` file in the directory **is** named — a file the manifest
//!    does not name is a failure, not a curiosity, because that is what a
//!    careless edit looks like;
//! 3. every recorded digest matches the file's bytes. The RGB frame and the
//!    index plane are both covered: `compare` prefers the indexed side, so a
//!    byte changed in an `.idx` is a change to what every later diff is
//!    measured against, and a digest of the RGB frame alone would not see it.
//!
//! **A frame with no recorded digest is a finding, not a pass.** A manifest
//! written before the field existed is the state this tool exists to report,
//! and a checker that skipped it would report a green run over the very hole
//! it was built for; [`backfill`] is the one-shot that fills those in.
//!
//! **The exit code is the contract** (the convention `compare` set): **0**
//! every capture checked out, **1** any finding, **2** the tool could not
//! check — nothing at the tree path, no manifest anywhere below it, a manifest
//! it cannot read or does not know the version of, or a file it cannot read
//! (a file that is merely *absent* is a finding, because a manifest naming a
//! file that is not there is a failed check and not a failure to check).
//!
//! **The reference tree is input-only, and `backfill` is its single narrow
//! exception.** It writes the digest fields into manifests and **touches no
//! frame file**; `verify` and `compare` write nothing at all. The exception is
//! the phase plan's, and it is conditional on the evidence: the frames were
//! hashed out of band before and after the backfill, the two lists are
//! identical, the backfill is idempotent, and the digests it wrote were
//! checked against the pre-change list rather than against its own work.

use std::collections::BTreeSet;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use crate::compare::{capture_directories, CaptureDir, MANIFEST_FILE};
use crate::reference::{sha256_hex, CaptureManifest, FrameRef, MANIFEST_VERSION};

/// What every frame file's name starts with (`reference.rs`'s
/// `frame_file_name` is the one place that builds a full name).
const FRAME_PREFIX: &str = "frame_";

/// Why the tool could not check at all, as opposed to finding something wrong:
/// the difference between exit 2 and exit 1, which a script reads.
#[derive(Debug)]
pub enum VerifyError {
    /// The tree, a manifest or a frame file could not be read, or a backfill
    /// was asked to write a tree it cannot read. Nothing was checked/written.
    Io(String),
    /// A manifest is readable but is not the record this tool reads: not a
    /// manifest at all, or one whose `version` is not the version it knows.
    /// Kept apart from [`VerifyError::Io`] because the repair is different —
    /// the file is there and is the wrong file — even though both exit 2.
    Malformed(String),
}

impl fmt::Display for VerifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyError::Io(message) | VerifyError::Malformed(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for VerifyError {}

/// Which manifest field a file's digest is recorded in.
///
/// An enum rather than the field's name as a string so [`DigestField::record`]
/// cannot be handed a name no field answers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DigestField {
    /// The RGB frame: `FrameRef::sha256`.
    Rgb,
    /// The index plane: `FrameRef::idx_sha256`.
    Idx,
}

impl DigestField {
    /// The name the manifest writes it under, for a finding's message.
    fn name(self) -> &'static str {
        match self {
            DigestField::Rgb => "sha256",
            DigestField::Idx => "idx_sha256",
        }
    }

    /// Record a digest in the entry.
    fn record(self, frame: &mut FrameRef, digest: String) {
        match self {
            DigestField::Rgb => frame.sha256 = Some(digest),
            DigestField::Idx => frame.idx_sha256 = Some(digest),
        }
    }
}

/// One file a frame entry names, with what the manifest says about it.
///
/// The names and digests are owned rather than borrowed from the entry so a
/// caller can hold this list while writing a digest back into the frame.
struct NamedFile {
    /// The file name as the manifest writes it, e.g. `frame_000430.rgb`.
    name: String,
    /// How many bytes of it a pixel takes: three for the RGB frame, one for
    /// the index plane. The manifest's geometry times this is the length the
    /// file must have.
    per_pixel: usize,
    /// The field this file's digest is recorded in.
    field: DigestField,
    /// The digest the entry records for it, when it records one.
    digest: Option<String>,
}

/// The files one frame entry names: its RGB frame always, its index plane when
/// it has one.
fn frame_files(frame: &FrameRef) -> Vec<NamedFile> {
    let mut files = vec![NamedFile {
        name: frame.rgb.clone(),
        per_pixel: 3,
        field: DigestField::Rgb,
        digest: frame.sha256.clone(),
    }];
    if let Some(idx) = &frame.idx {
        files.push(NamedFile {
            name: idx.clone(),
            per_pixel: 1,
            field: DigestField::Idx,
            digest: frame.idx_sha256.clone(),
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
    /// How many named files were readable (so the summary counts what was
    /// actually checked, not what was asked for).
    files: usize,
    bytes: u64,
    findings: Vec<Finding>,
}

/// `cargo xtask reference verify`: walk the tree and check every capture
/// against the digests its manifest records.
///
/// Returns the exit code the process should end with: 0 when every capture
/// checked out, 1 when any check failed. A tree this tool cannot check at all
/// is an `Err`, which `main` turns into exit 2 — a script must be able to tell
/// "the tree is as filed" from "the tree could not be checked".
pub fn command(tree: &Path) -> Result<ExitCode, VerifyError> {
    let captures = captures_in(tree)?;
    let mut findings = 0usize;
    let mut files = 0usize;
    let mut bytes = 0u64;

    for capture in &captures {
        let manifest = read_manifest(&capture.dir)?;
        let inspection = inspect(capture, &manifest, tree)?;
        files += inspection.files;
        bytes += inspection.bytes;
        findings += inspection.findings.len();
        if inspection.findings.is_empty() {
            println!(
                "ok   {}: {} frame(s), {} file(s)",
                name_of(capture),
                manifest.frames.len(),
                inspection.files
            );
        }
        for finding in &inspection.findings {
            println!("FAIL {}: {}", finding.path, finding.what);
        }
    }

    if findings == 0 {
        println!(
            "verdict: ok — {} capture(s), {files} frame file(s), {bytes} bytes",
            captures.len()
        );
        return Ok(ExitCode::SUCCESS);
    }
    println!(
        "verdict: {findings} finding(s) — {} capture(s), {files} frame file(s) checked",
        captures.len()
    );
    Ok(ExitCode::from(1))
}

/// `cargo xtask reference backfill`: record every frame file's digest in the
/// manifest beside it, for a tree that was filed before `capture` wrote them.
///
/// Fills in a digest that is missing and never writes over one that disagrees
/// with the file: a backfill that re-recorded a changed frame would destroy the
/// evidence the digest exists to be — the record would come to describe
/// whatever the tree happens to be, which is worse than no record. A manifest
/// whose recorded digest differs from the file is refused, with the file named
/// and `reference verify` pointed at as the tool that lists every such file.
///
/// Reads, digests and re-serialises the whole tree before writing anything, so
/// a tree this tool cannot read (a missing frame, an unreadable manifest) is
/// left exactly as it was rather than half-recorded.
pub fn backfill(tree: &Path) -> Result<ExitCode, VerifyError> {
    let captures = captures_in(tree)?;
    let mut planned: Vec<(PathBuf, String)> = Vec::new();
    let mut recorded = 0usize;

    for capture in &captures {
        let mut manifest = read_manifest(&capture.dir)?;
        for frame in &mut manifest.frames {
            for file in frame_files(frame) {
                let path = capture.dir.join(&file.name);
                let bytes = fs::read(&path).map_err(|e| {
                    VerifyError::Io(format!(
                        "cannot backfill {}: the manifest names it and it cannot be read ({e}); \
                         nothing was written",
                        under(capture, &file.name)
                    ))
                })?;
                let actual = sha256_hex(&bytes);
                match &file.digest {
                    Some(recorded_digest) if *recorded_digest == actual => {}
                    Some(recorded_digest) => {
                        return Err(VerifyError::Io(format!(
                            "{}: the manifest records {} {recorded_digest} and the file is \
                             {actual}; a backfill fills in a digest that is missing and will not \
                             write over one that disagrees — run `xtask reference verify --tree {}` \
                             to see every file that has changed. Nothing was written.",
                            under(capture, &file.name),
                            file.field.name(),
                            capture.dir.display()
                        )));
                    }
                    None => {
                        file.field.record(frame, actual);
                        recorded += 1;
                    }
                }
            }
        }

        let path = capture.dir.join(MANIFEST_FILE);
        let json = serialise(&manifest, &path)?;
        let current = fs::read_to_string(&path)
            .map_err(|e| VerifyError::Io(format!("cannot read {}: {e}", path.display())))?;
        // Idempotence, in the only form that can be checked: the bytes the
        // tool would write are the bytes that are there, so it writes nothing
        // (not even the same content back, which would move the file's time).
        if current != json {
            planned.push((path, json));
        }
    }

    for (path, json) in &planned {
        fs::write(path, json)
            .map_err(|e| VerifyError::Io(format!("cannot write {}: {e}", path.display())))?;
    }

    if planned.is_empty() {
        println!(
            "backfill: {} capture(s), every digest already recorded and matching — nothing written",
            captures.len()
        );
    } else {
        println!(
            "backfill: {} capture(s), {recorded} digest(s) recorded into {} manifest(s)",
            captures.len(),
            planned.len()
        );
        for (path, _) in &planned {
            println!("  wrote {}", path.display());
        }
    }
    Ok(ExitCode::SUCCESS)
}

/// Every capture at or below `tree`, through `compare`'s walk — the same
/// all-the-way-down rule, so the two tools cannot come to disagree about what
/// a capture is.
fn captures_in(tree: &Path) -> Result<Vec<CaptureDir>, VerifyError> {
    capture_directories(tree).map_err(|e| VerifyError::Io(e.to_string()))
}

/// Read a capture's manifest, refusing one this tool does not know the shape
/// or the version of rather than checking it against the wrong rules.
fn read_manifest(dir: &Path) -> Result<CaptureManifest, VerifyError> {
    let path = dir.join(MANIFEST_FILE);
    let json = fs::read_to_string(&path)
        .map_err(|e| VerifyError::Io(format!("cannot read {}: {e}", path.display())))?;
    let manifest: CaptureManifest = serde_json::from_str(&json)
        .map_err(|e| VerifyError::Malformed(format!("{}: {e}", path.display())))?;
    if manifest.version != MANIFEST_VERSION {
        return Err(VerifyError::Malformed(format!(
            "{} says version {}; this tool reads version {MANIFEST_VERSION}",
            path.display(),
            manifest.version
        )));
    }
    Ok(manifest)
}

/// The three checks, on one capture.
fn inspect(
    capture: &CaptureDir,
    manifest: &CaptureManifest,
    tree: &Path,
) -> Result<Inspection, VerifyError> {
    let mut inspection = Inspection::default();
    let mut named: BTreeSet<String> = BTreeSet::new();

    for frame in &manifest.frames {
        for file in frame_files(frame) {
            named.insert(file.name.clone());
            let path = capture.dir.join(&file.name);
            let bytes = match fs::read(&path) {
                Ok(bytes) => bytes,
                // Absent is a finding — the check is "every file the manifest
                // names exists" — but unreadable is not: a permission error
                // means this tool could not check, which is exit 2.
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    inspection.findings.push(Finding {
                        path: under(capture, &file.name),
                        what: format!(
                            "the manifest names it as frame {}'s {} and there is no such file",
                            frame.index,
                            field_target(file.field)
                        ),
                    });
                    continue;
                }
                Err(error) => {
                    return Err(VerifyError::Io(format!(
                        "cannot read {}: {error}",
                        path.display()
                    )))
                }
            };
            inspection.files += 1;
            inspection.bytes += bytes.len() as u64;

            let expected = file.per_pixel * usize::from(manifest.width) * usize::from(manifest.height);
            if bytes.len() != expected {
                inspection.findings.push(Finding {
                    path: under(capture, &file.name),
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

            match &file.digest {
                None => inspection.findings.push(Finding {
                    path: under(capture, &file.name),
                    what: format!(
                        "the manifest records no {} for it, so nothing checks these bytes; `xtask \
                         reference backfill --tree {}` writes one",
                        file.field.name(),
                        tree.display()
                    ),
                }),
                Some(recorded) => {
                    let actual = sha256_hex(&bytes);
                    if actual != *recorded {
                        inspection.findings.push(Finding {
                            path: under(capture, &file.name),
                            what: format!(
                                "the manifest records {} {recorded} and the file is {actual}",
                                file.field.name()
                            ),
                        });
                    }
                }
            }
        }
    }

    // Every `frame_*` file in the directory has to be one the manifest names.
    let entries = fs::read_dir(&capture.dir)
        .map_err(|e| VerifyError::Io(format!("cannot list {}: {e}", capture.dir.display())))?;
    let mut loose = Vec::new();
    for entry in entries {
        let entry =
            entry.map_err(|e| VerifyError::Io(format!("cannot list {}: {e}", capture.dir.display())))?;
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

/// The manifest's own text for what a file is, for a message about it.
fn field_target(field: DigestField) -> &'static str {
    match field {
        DigestField::Rgb => "RGB frame",
        DigestField::Idx => "index plane",
    }
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

/// The manifest's bytes as `capture` writes them, so a backfilled manifest and
/// a captured one are the same file.
fn serialise(manifest: &CaptureManifest, path: &Path) -> Result<String, VerifyError> {
    let json = serde_json::to_string_pretty(manifest).map_err(|e| {
        VerifyError::Malformed(format!("cannot serialise {}: {e}", path.display()))
    })?;
    Ok(format!("{json}\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reference::BootRecipe;
    use std::collections::BTreeMap;

    /// The capture the tests check: 2×2 pixels, frame 10 with an index plane
    /// and frame 20 without, every digest recorded as `capture` records it.
    const FRAME_A_RGB: &[u8] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    const FRAME_A_IDX: &[u8] = &[1, 2, 3, 4];
    const FRAME_B_RGB: &[u8] = &[12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23];
    const FRAME_A: &str = "frame_000010.rgb";
    const FRAME_A_INDEX: &str = "frame_000010.idx";
    const FRAME_B: &str = "frame_000020.rgb";

    fn capture() -> CaptureManifest {
        CaptureManifest {
            version: MANIFEST_VERSION,
            oracle: "dosbox-automation 0.85.1 (6d100)".into(),
            upstream_commit: "071a82e".into(),
            exe: "SECOND.EXE".into(),
            arguments: vec!["2".into()],
            cycles: 30000,
            width: 2,
            height: 2,
            frame_origin: "boot".into(),
            skip_boot_frames: 0,
            boot: BootRecipe::Autoexec,
            keys: vec!["KBD_left".into(), "KBD_enter".into()],
            frames: vec![
                FrameRef {
                    index: 10,
                    idx: Some(FRAME_A_INDEX.into()),
                    rgb: FRAME_A.into(),
                    palette: Some(BTreeMap::from([(1u8, [1, 2, 3])])),
                    sha256: Some(sha256_hex(FRAME_A_RGB)),
                    idx_sha256: Some(sha256_hex(FRAME_A_IDX)),
                },
                FrameRef {
                    index: 20,
                    idx: None,
                    rgb: FRAME_B.into(),
                    palette: None,
                    sha256: Some(sha256_hex(FRAME_B_RGB)),
                    idx_sha256: None,
                },
            ],
        }
    }

    /// A scratch directory for this test, emptied first.
    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("xtask-verify-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    /// Write the capture's frame files and its manifest, under `tree`, as
    /// `capture` writes them: frames first, manifest last.
    ///
    /// `name` is the capture's path below the tree — `""` is a tree that is
    /// one capture itself, anything else a capture directory below it.
    fn write_capture(tree: &Path, name: &str, manifest: &CaptureManifest) -> PathBuf {
        let dir = tree.join(name);
        fs::create_dir_all(&dir).unwrap();
        for (file, bytes) in [
            (FRAME_A, FRAME_A_RGB),
            (FRAME_A_INDEX, FRAME_A_IDX),
            (FRAME_B, FRAME_B_RGB),
        ] {
            fs::write(dir.join(file), bytes).unwrap();
        }
        write_manifest(&dir, manifest);
        dir
    }

    fn write_manifest(dir: &Path, manifest: &CaptureManifest) {
        let json = serde_json::to_string_pretty(manifest).unwrap();
        fs::write(dir.join(MANIFEST_FILE), format!("{json}\n")).unwrap();
    }

    fn capture_dir(name: &str, dir: &Path) -> CaptureDir {
        CaptureDir {
            name: name.to_owned(),
            dir: dir.to_path_buf(),
        }
    }

    fn check(name: &str, dir: &Path, manifest: &CaptureManifest) -> Inspection {
        inspect(&capture_dir(name, dir), manifest, Path::new("tree")).unwrap()
    }

    /// Every file's bytes under `tree`, so a test can say nothing changed.
    fn tree_digest(tree: &Path) -> Vec<(String, String)> {
        let mut files = Vec::new();
        for capture in capture_directories(tree).unwrap() {
            for entry in fs::read_dir(&capture.dir).unwrap() {
                let entry = entry.unwrap();
                let bytes = fs::read(entry.path()).unwrap();
                files.push((
                    entry.file_name().to_string_lossy().into_owned(),
                    sha256_hex(&bytes),
                ));
            }
        }
        files.sort();
        files
    }

    #[test]
    fn a_capture_whose_files_hash_as_recorded_reports_nothing() {
        let tree = scratch("clean");
        let manifest = capture();
        let dir = write_capture(&tree, "parts/one", &manifest);

        let inspection = check("parts/one", &dir, &manifest);
        assert_eq!(inspection.findings, vec![]);
        assert_eq!(inspection.files, 3, "three files: two rgb, one idx");
        assert_eq!(
            inspection.bytes,
            (FRAME_A_RGB.len() + FRAME_A_IDX.len() + FRAME_B_RGB.len()) as u64
        );
        // The whole tree checks out through the same walk `compare` uses.
        assert_eq!(command(&tree).unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn a_frame_whose_bytes_changed_is_a_finding_that_names_it() {
        let tree = scratch("rgb-flip");
        let manifest = capture();
        let dir = write_capture(&tree, "parts/one", &manifest);
        fs::write(dir.join(FRAME_A), [9u8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]).unwrap();

        let inspection = check("parts/one", &dir, &manifest);
        assert_eq!(inspection.findings.len(), 1, "{:?}", inspection.findings);
        assert_eq!(inspection.findings[0].path, "parts/one/frame_000010.rgb");
        assert!(
            inspection.findings[0].what.contains("sha256"),
            "{}",
            inspection.findings[0].what
        );
    }

    #[test]
    fn an_index_plane_whose_bytes_changed_is_a_finding() {
        // The indexed side is what `compare` prefers; a digest of the RGB
        // frame alone would let a changed index plane through.
        let tree = scratch("idx-flip");
        let manifest = capture();
        let dir = write_capture(&tree, "", &manifest);
        fs::write(dir.join(FRAME_A_INDEX), [9u8, 2, 3, 4]).unwrap();

        let inspection = check("", &dir, &manifest);
        assert_eq!(inspection.findings.len(), 1, "{:?}", inspection.findings);
        assert_eq!(inspection.findings[0].path, FRAME_A_INDEX);
        let what = &inspection.findings[0].what;
        assert!(what.contains("idx_sha256"), "{what}");
        assert!(!what.contains(" idx "), "the RGB frame passed: {what}");
    }

    #[test]
    fn a_frame_file_the_manifest_does_not_name_is_a_finding() {
        // What a careless edit looks like: a file beside the capture that no
        // frame entry mentions, which nothing else in the tree would report.
        let tree = scratch("loose");
        let manifest = capture();
        let dir = write_capture(&tree, "parts/one", &manifest);
        fs::write(dir.join("frame_000999.rgb"), [0u8; 12]).unwrap();

        let inspection = check("parts/one", &dir, &manifest);
        assert_eq!(inspection.findings.len(), 1, "{:?}", inspection.findings);
        assert_eq!(inspection.findings[0].path, "parts/one/frame_000999.rgb");
        assert!(
            inspection.findings[0].what.contains("does not name"),
            "{}",
            inspection.findings[0].what
        );
    }

    #[test]
    fn a_file_the_manifest_names_that_is_not_there_is_a_finding() {
        let tree = scratch("missing");
        let manifest = capture();
        let dir = write_capture(&tree, "", &manifest);
        fs::remove_file(dir.join(FRAME_B)).unwrap();

        let inspection = check("", &dir, &manifest);
        assert_eq!(inspection.findings.len(), 1, "{:?}", inspection.findings);
        assert_eq!(inspection.findings[0].path, FRAME_B);
        assert!(
            inspection.findings[0].what.contains("no such file"),
            "{}",
            inspection.findings[0].what
        );
        assert_eq!(inspection.files, 2, "the two files that are there");
    }

    #[test]
    fn a_frame_whose_length_is_not_its_geometry_is_a_finding() {
        // Two independent checks fail on a truncated file, and both are said:
        // the length the geometry implies, and the digest.
        let tree = scratch("truncated");
        let manifest = capture();
        let dir = write_capture(&tree, "", &manifest);
        fs::write(dir.join(FRAME_A), &FRAME_A_RGB[..8]).unwrap();

        let inspection = check("", &dir, &manifest);
        let said: Vec<&str> = inspection
            .findings
            .iter()
            .map(|finding| finding.what.as_str())
            .collect();
        assert_eq!(inspection.findings.len(), 2, "{said:?}");
        assert!(
            said.iter().any(|what| what.contains("8 bytes")
                && what.contains("2x2")
                && what.contains("12")),
            "{said:?}"
        );
        assert!(said.iter().any(|what| what.contains("sha256")), "{said:?}");
    }

    #[test]
    fn a_frame_with_no_recorded_digest_is_a_finding() {
        // Not a pass: skipping it would report a green run over exactly the
        // hole this tool was built for (the tree as Phase 1 filed it).
        let tree = scratch("no-digest");
        let mut manifest = capture();
        manifest.frames[1].sha256 = None;
        let dir = write_capture(&tree, "", &manifest);
        write_manifest(&dir, &manifest);

        let inspection = check("", &dir, &manifest);
        assert_eq!(inspection.findings.len(), 1, "{:?}", inspection.findings);
        assert_eq!(inspection.findings[0].path, FRAME_B);
        assert!(
            inspection.findings[0].what.contains("backfill"),
            "the message says what writes one: {}",
            inspection.findings[0].what
        );
    }

    #[test]
    fn a_manifest_this_tool_does_not_read_is_refused() {
        let tree = scratch("version");
        let mut manifest = capture();
        let dir = write_capture(&tree, "", &manifest);
        manifest.version = MANIFEST_VERSION + 1;
        write_manifest(&dir, &manifest);

        match command(&tree) {
            Err(VerifyError::Malformed(message)) => {
                assert!(message.contains("version 2"), "{message}")
            }
            other => panic!("a version this tool does not read is not checked: {other:?}"),
        }
    }

    #[test]
    fn a_manifest_that_is_not_a_manifest_is_refused() {
        let tree = scratch("malformed");
        let manifest = capture();
        let dir = write_capture(&tree, "", &manifest);
        fs::write(dir.join(MANIFEST_FILE), "{ not json").unwrap();

        match command(&tree) {
            Err(VerifyError::Malformed(_)) => {}
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn a_tree_with_no_capture_in_it_cannot_be_checked() {
        // The walk error, not a green run: a tree this tool measured nothing
        // in must not be readable as "everything checked out".
        let tree = scratch("empty");
        match command(&tree) {
            Err(VerifyError::Io(message)) => {
                assert!(message.contains("holds no manifest.json"), "{message}")
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    #[test]
    fn backfill_records_every_digest_without_touching_a_frame() {
        let tree = scratch("backfill");
        let mut manifest = capture();
        manifest.frames[0].sha256 = None;
        manifest.frames[0].idx_sha256 = None;
        manifest.frames[1].sha256 = None;
        let dir = write_capture(&tree, "", &manifest);
        write_manifest(&dir, &manifest);
        let frames_before: Vec<_> = [FRAME_A, FRAME_A_INDEX, FRAME_B]
            .iter()
            .map(|file| fs::read(dir.join(file)).unwrap())
            .collect();

        assert_eq!(backfill(&tree).unwrap(), ExitCode::SUCCESS);

        let written = read_manifest(&dir).unwrap();
        assert_eq!(written.frames[0].sha256.as_deref(), Some(sha256_hex(FRAME_A_RGB).as_str()));
        assert_eq!(written.frames[0].idx_sha256.as_deref(), Some(sha256_hex(FRAME_A_IDX).as_str()));
        assert_eq!(written.frames[1].sha256.as_deref(), Some(sha256_hex(FRAME_B_RGB).as_str()));
        assert_eq!(written.frames[1].idx_sha256, None, "frame 20 has no index plane");

        // The guard the plan calls load-bearing: a backfill that rewrote a
        // frame would destroy the thing it is protecting.
        for (file, before) in [FRAME_A, FRAME_A_INDEX, FRAME_B].iter().zip(&frames_before) {
            assert_eq!(&fs::read(dir.join(file)).unwrap(), before, "{file} changed");
        }

        // And what it wrote is what the verifier then agrees with.
        assert_eq!(command(&tree).unwrap(), ExitCode::SUCCESS);
    }

    #[test]
    fn a_second_backfill_writes_no_byte() {
        let tree = scratch("backfill-twice");
        let mut manifest = capture();
        manifest.frames[0].sha256 = None;
        manifest.frames[0].idx_sha256 = None;
        manifest.frames[1].sha256 = None;
        let dir = write_capture(&tree, "parts/one", &manifest);
        write_manifest(&dir, &manifest);

        assert_eq!(backfill(&tree).unwrap(), ExitCode::SUCCESS);
        let once = tree_digest(&tree);
        assert_eq!(backfill(&tree).unwrap(), ExitCode::SUCCESS);
        assert_eq!(tree_digest(&tree), once, "the second run changed a file");
    }

    #[test]
    fn backfill_refuses_a_tree_it_cannot_read_and_writes_nothing() {
        // The other guard: a manifest naming a file that is not there is not a
        // tree to record digests in, and the tree is left exactly as it was —
        // including the second capture, which could have been backfilled.
        let tree = scratch("backfill-unreadable");
        let mut manifest = capture();
        manifest.frames[0].sha256 = None;
        let dir = write_capture(&tree, "", &manifest);
        write_manifest(&dir, &manifest);
        fs::remove_file(dir.join(FRAME_A)).unwrap();
        let before = tree_digest(&tree);

        match backfill(&tree) {
            Err(VerifyError::Io(message)) => {
                assert!(message.contains(FRAME_A), "the file is named: {message}");
                assert!(message.contains("nothing was written"), "{message}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(tree_digest(&tree), before, "a refused backfill wrote something");
    }

    #[test]
    fn backfill_will_not_write_over_a_digest_that_disagrees() {
        // If it did, running it after a frame changed would bless the change:
        // the record would come to describe whatever the tree happens to be,
        // which is worse than no record.
        let tree = scratch("backfill-disagrees");
        let mut manifest = capture();
        manifest.frames[1].sha256 = Some(sha256_hex(b"not this file"));
        let dir = write_capture(&tree, "", &manifest);
        write_manifest(&dir, &manifest);
        let before = tree_digest(&tree);

        match backfill(&tree) {
            Err(VerifyError::Io(message)) => {
                assert!(message.contains(FRAME_B), "the file is named: {message}");
                assert!(message.contains("will not write over"), "{message}");
                assert!(message.contains("reference verify"), "{message}");
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
        assert_eq!(tree_digest(&tree), before, "a refused backfill wrote something");
    }
}
