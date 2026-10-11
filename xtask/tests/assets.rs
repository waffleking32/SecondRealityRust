//! `cargo xtask assets verify` end to end: the exit code *is* the contract.
//!
//! The tool's job is to tell a script three things apart — "everything checked
//! out" (0), "a check failed" (1) and "the tool could not run at all" (2) — so
//! the tests here run the built binary and read its exit code, the way the
//! script that gates the phase will. The checks themselves are exercised
//! against the real tree (150 curated files) and against fixture trees that
//! hold exactly the failure each test is about.
//!
//! Fixtures live in `tests/fixtures/assets-*/assets/`, mirroring the real
//! layout: one `PROVENANCE.md` and the files it records. A test that writes
//! anything writes it outside the repository, under the temporary directory —
//! `assets/` is input-only, and a sheet written among the assets would become a
//! file with no provenance row, which is the very thing `--tree` reports.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root: `xtask/..`, where `assets/` lives.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("..")
}

/// `tests/fixtures/<name>` — a tree to point the binary's working directory at.
fn fixture(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name)
}

/// A scratch directory for this test, emptied first.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("xtask-assets-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// Copy a fixture tree into a scratch directory, for a test that has to change
/// a tree of its own: the fixtures themselves are read-only inputs, the same
/// rule `assets/` is under.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            fs::copy(entry.path(), target).unwrap();
        }
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    /// Both streams: the tool prints its report to stdout and its refusals to
    /// stderr, and a test about a message does not care which.
    fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

/// Run the built `xtask` binary in `dir`.
fn xtask(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .args(args)
        .current_dir(dir)
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", env!("CARGO_BIN_EXE_xtask")));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

#[test]
fn a_tree_whose_hash_does_not_match_the_provenance_is_reported() {
    let run = xtask(
        &fixture("assets-hash-mismatch"),
        &["assets", "verify", "--tree"],
    );

    assert_eq!(run.code, 1, "a failed check is 1:\n{}", run.output());
    let output = run.output();
    assert!(output.contains("BAD.TXT"), "{output}");
    // The message names both digests: the recorded one and the file's own, so a
    // reader does not have to re-run anything to see the difference.
    assert!(
        output.contains("0000000000000000000000000000000000000000000000000000000000000000"),
        "the recorded digest is named:\n{output}"
    );
    assert!(
        output.contains("3a76b1015ac28d126546ff1001a352247c9713a8f585362001797874a0faa6b1"),
        "the digest the file actually has is named:\n{output}"
    );
}

#[test]
fn an_asset_provenance_lists_but_the_tree_lacks_is_reported() {
    let run = xtask(
        &fixture("assets-missing-file"),
        &["assets", "verify", "--tree"],
    );

    assert_eq!(run.code, 1, "a failed check is 1:\n{}", run.output());
    let output = run.output();
    assert!(output.contains("GONE.TXT"), "{output}");
}

#[test]
fn a_file_the_provenance_does_not_record_is_reported() {
    // Every file in `assets/` is supposed to be a copy of something upstream,
    // so a file with no row is a file with no origin: without this, a stray
    // file — or a sheet written into the tree — would pass unnoticed.
    let tree = scratch("stray").join("assets");
    copy_tree(&fixture("assets-hash-mismatch/assets"), &tree);
    fs::write(tree.join("STRAY.TXT"), b"nobody recorded this\n").unwrap();

    let run = xtask(tree.parent().unwrap(), &["assets", "verify", "--tree"]);

    assert_eq!(run.code, 1, "a failed check is 1:\n{}", run.output());
    assert!(run.output().contains("STRAY.TXT"), "{}", run.output());
}

#[test]
fn a_recorded_length_that_does_not_match_the_file_is_reported() {
    // The tree's own fixture: the digest is right and the length is not. The
    // file is provably the right bytes, so the record is what has to change —
    // and the tool says which of the two it is.
    let root = scratch("length");
    let tree = root.join("assets");
    copy_tree(&fixture("assets-hash-mismatch/assets"), &tree);
    let provenance = tree.join("PROVENANCE.md");
    let text = fs::read_to_string(&provenance)
        .unwrap()
        .replace(
            "0000000000000000000000000000000000000000000000000000000000000000",
            "3a76b1015ac28d126546ff1001a352247c9713a8f585362001797874a0faa6b1",
        )
        .replace("| 58 |", "| 59 |");
    fs::write(&provenance, text).unwrap();

    let run = xtask(&root, &["assets", "verify", "--tree"]);

    assert_eq!(run.code, 1, "a failed check is 1:\n{}", run.output());
    let output = run.output();
    assert!(output.contains("59") && output.contains("58"), "{output}");
}

#[test]
fn every_asset_in_the_tree_decodes() {
    // The real tree, and the full check: every file re-hashed against the
    // provenance and every asset decoded.
    let run = xtask(&repo_root(), &["assets", "verify", "--tree", "--decode"]);

    assert_eq!(run.code, 0, "the real tree checks out:\n{}", run.output());
    let output = run.output();
    assert!(output.contains("ALKU/FONA.UH"), "{output}");
    // A1's record: 149 curated files in the main table and one recovered file
    // in the table A5 added. The count is the phase's, and a tree that gains or
    // loses a file without the record changing has a problem this test cannot
    // see — so the number is pinned here too.
    assert!(output.contains("150"), "{output}");
    assert!(!output.contains("FAIL"), "{output}");
}

#[test]
fn a_sheet_is_written_for_every_part_that_has_assets() {
    let sheets = scratch("sheets");
    let run = xtask(
        &repo_root(),
        &["assets", "verify", "--sheet", sheets.to_str().unwrap()],
    );

    assert_eq!(run.code, 0, "sheets are not a check:\n{}", run.output());
    // The provenance's `parts` column names 21 parts. One sheet each, named
    // after the part, is what makes "look at the sheets" a thing a person can
    // do — so every one of them is asserted, not a count.
    for part in [
        "3DSINFLD", "ALKU", "BEGLOGO", "CRED", "DDSTARS", "ENDLOGO", "ENDSCRL", "GLENZ", "GRAB",
        "JPLOGO", "LNS&ZOOM", "MAIN", "MNTSCRL", "PAM", "PANICEND", "RAYSCRL", "START", "TECHNO",
        "TUNNELI", "U2A", "U2E",
    ] {
        let file = sheets.join(format!("{part}.png"));
        let bytes = fs::read(&file)
            .unwrap_or_else(|e| panic!("no sheet for {part} at {}: {e}", file.display()));
        assert!(
            bytes.len() > 100,
            "{} is {} bytes; an empty PNG is not a sheet",
            file.display(),
            bytes.len()
        );
        assert_eq!(
            &bytes[..8],
            b"\x89PNG\r\n\x1a\n",
            "{} is not a PNG",
            file.display()
        );
    }
}

#[test]
fn a_directory_without_an_assets_tree_is_a_tool_failure_not_a_failed_check() {
    // Nothing to check against: the difference this exit code exists for.
    let empty = scratch("no-tree");
    let run = xtask(&empty, &["assets", "verify", "--tree"]);

    assert_eq!(run.code, 2, "could not run is 2:\n{}", run.output());
    assert!(run.output().contains("assets"), "{}", run.output());
}

#[test]
fn a_provenance_whose_tables_have_no_path_column_is_a_tool_failure() {
    // The real file's `Collapsed duplicates` table has a `dropped path` column
    // and a `sha256` column. Read by position, or by taking any digest column
    // as evidence, it would be read as a list of curated files.
    let run = xtask(
        &fixture("assets-no-path-column"),
        &["assets", "verify", "--tree"],
    );

    assert_eq!(run.code, 2, "a malformed record is 2:\n{}", run.output());
    assert!(run.output().contains("path"), "{}", run.output());
}

#[test]
fn a_sheet_directory_inside_the_assets_tree_is_refused() {
    // The precedent `compare` set for `--out`: the tree this tool reads is
    // input-only, and a PNG written among the assets would be a file with no
    // provenance row — the failure `--tree` exists to report.
    let root = fixture("assets-hash-mismatch");
    let inside = root.join("assets/sheets");
    let run = xtask(
        &root,
        &["assets", "verify", "--sheet", inside.to_str().unwrap()],
    );

    assert_eq!(run.code, 2, "a refused --sheet is 2:\n{}", run.output());
    assert!(run.output().contains("--sheet"), "{}", run.output());
    assert!(
        !inside.exists(),
        "the refused directory must not be created: {}",
        inside.display()
    );
}
