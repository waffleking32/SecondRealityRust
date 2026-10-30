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
fn a_recorded_refusal_that_starts_decoding_is_a_failed_check() {
    // The register's inverse direction. If the decoder ever grew lenient enough
    // to read the truncated `COMAN/COMBG.UH` — clipping it instead of refusing
    // it — the file's digest would be unchanged, `--tree` would still pass, and
    // the phase's gate would go green while the premise the record rests on was
    // broken. Nothing but this check can see that, so it has to fail.
    let root = scratch("refusal-decoded");
    let tree = root.join("assets");
    fs::create_dir_all(tree.join("COMAN")).unwrap();
    // Any file that decodes, at the path the record says must not: the point is
    // that a refusal recorded as the shipped outcome has stopped happening.
    fs::copy(
        repo_root().join("assets/DDSTARS/TEXTS.16"),
        tree.join("COMAN/COMBG.UH"),
    )
    .unwrap();
    fs::write(
        tree.join("PROVENANCE.md"),
        format!(
            "| path | sha256 | parts |\n|---|---|---|\n| `COMAN/COMBG.UH` | `{}` | 3DSINFLD |\n",
            "0".repeat(64)
        ),
    )
    .unwrap();

    let run = xtask(&root, &["assets", "verify", "--decode"]);

    assert_eq!(
        run.code,
        1,
        "a recorded refusal that decodes is a failed check:\n{}",
        run.output()
    );
    let output = run.output();
    assert!(output.contains("FAIL COMAN/COMBG.UH"), "{output}");
    assert!(
        output.contains("decoded, where the record says it must be refused"),
        "{output}"
    );
    assert!(output.contains("REFUSED_ON_RECORD"), "{output}");
    assert!(output.contains("verdict: 1 finding(s)"), "{output}");
    // And the summary's own arithmetic, in the run a reviewer reads: the row
    // used to be counted nowhere — `refused on record` dropped to 0 and no
    // other category took it — so a one-row tree printed a line whose parts did
    // not add up to the one it led with. Every row is in exactly one category,
    // and this one is in the category that says the premise broke.
    assert!(
        output.contains(
            "decode: 1 asset(s) — 0 picture(s), 0 animation(s), 0 palette(s), 0 Visu scene file(s), \
             0 headerless with no decoder, 0 refused on record, 1 refused on record that now \
             decode, 1 finding(s)"
        ),
        "{output}"
    );
}

#[test]
fn the_decode_summary_adds_up_when_a_recorded_file_cannot_be_read() {
    // The other way a row leaves the picture/animation/palette/headerless
    // categories: the file is not there to decode at all. It is a finding and
    // it is counted, so the summary still adds up to the number of rows.
    let run = xtask(
        &fixture("assets-missing-file"),
        &["assets", "verify", "--decode"],
    );

    assert_eq!(run.code, 1, "a missing file is a failed check:\n{}", run.output());
    let output = run.output();
    assert!(output.contains("FAIL GONE.TXT"), "{output}");
    assert!(
        output.contains(
            "decode: 1 asset(s) — 0 picture(s), 0 animation(s), 0 palette(s), 0 Visu scene file(s), \
             0 headerless with no decoder, 0 refused on record, 1 that could not be decoded, 1 \
             finding(s)"
        ),
        "{output}"
    );
}

#[test]
fn every_asset_in_the_tree_decodes() {
    // The real tree, and the full check: every file re-hashed against the
    // provenance and every asset decoded.
    let run = xtask(&repo_root(), &["assets", "verify", "--tree", "--decode"]);

    assert_eq!(run.code, 0, "the real tree checks out:\n{}", run.output());
    let output = run.output();
    assert!(output.contains("ALKU/FONA.UH"), "{output}");
    // The decode census, pinned: every picture, animation and palette, the 51
    // files of the vector engine's packed scene set (U2A's six and U2E's 45,
    // read by construction because none has a magic), and the one recorded
    // refusal. The parts add up to the total by construction of the line, and
    // a file that leaves one bucket for another without the record changing
    // is exactly what this pin is here to catch.
    assert!(
        output.contains(
            "decode: 179 asset(s) — 55 picture(s), 2 animation(s), 5 palette(s), 51 Visu scene \
             file(s), 65 headerless with no decoder, 1 refused on record, 0 finding(s)"
        ),
        "{output}"
    );
    // And the two streams' own totals, through the same run the summary is
    // about: the reader tests pin them again, one file at a time.
    assert!(
        output.contains(
            "VISU/C/SCENE/U2A.0AB: VISU animation: 522 frame(s), 1272 command(s), fov 0x22"
        ),
        "{output}"
    );
    assert!(
        output.contains(
            "VISU/C/SCENE/U2E.0AB: VISU animation: 1802 frame(s), 4047 command(s), fov 0x1C"
        ),
        "{output}"
    );
    // The record's own walk: every provenance row is on disk and hashes, and
    // the tree holds nothing the record does not name. Phase 8 Task 6 added
    // `HARD/FCRZ.U` (the row is in the main table), so the walk's own line is
    // pinned here: a tree that gains or loses a file without the record
    // changing has a problem this test cannot see.
    assert!(
        output.contains("tree: 179 file(s) recorded, 179 in the tree, 0 finding(s)"),
        "{output}"
    );
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
    // The provenance's `parts` column names 24 parts (MINVBALL by its shipped EXE alone, D49;
    // PLZPART's sheet was missing from this list before). One sheet each, named
    // after the part, is what makes "look at the sheets" a thing a person can
    // do — so every one of them is asserted, not a count.
    for part in [
        "3DSINFLD", "ALKU", "BEGLOGO", "CRED", "DDSTARS", "ENDLOGO", "ENDSCRL", "GLENZ", "HARD", "GRAB",
        "JPLOGO", "LNS&ZOOM", "MAIN", "MINVBALL", "MNTSCRL", "PAM", "PANICEND", "PLZPART", "RAYSCRL",
        "START", "TECHNO", "TUNNELI", "U2A", "U2E",
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

    // Every asset the record names a part for is on that part's sheet — the
    // recovered file included, which is the one with no source art beside it
    // and so the one a person most needs to see there.
    let output = run.output();
    let techno = sheet_tiles(&output, "TECHNO.png");
    assert_eq!(techno.len(), 5, "TECHNO's sheet (its shipped EXE among them, D49): {techno:?}");
    assert!(
        techno.iter().any(|tile| tile.contains("CIRCLE2.EGA")),
        "the recovered asset is on TECHNO's sheet: {techno:?}"
    );

    // And nothing is silently on no sheet: the tile maps of all 24 sheets
    // between them name every path the record lists. Every file the record
    // names a part for has a tile somewhere, and this asserts the converse too
    // by counting.
    let mut tiles = Vec::new();
    for part in [
        "3DSINFLD", "ALKU", "BEGLOGO", "CRED", "DDSTARS", "ENDLOGO", "ENDSCRL", "GLENZ", "HARD", "GRAB",
        "JPLOGO", "LNS&ZOOM", "MAIN", "MINVBALL", "MNTSCRL", "PAM", "PANICEND", "PLZPART", "RAYSCRL",
        "START", "TECHNO", "TUNNELI", "U2A", "U2E",
    ] {
        tiles.extend(sheet_tiles(&output, &format!("{part}.png")));
    }
    assert_eq!(tiles.len(), 179, "one tile per recorded file");
    assert!(
        !output.contains("names no part for this file"),
        "every recorded file has a part: {output}"
    );
}

/// The tiles of one sheet, from the map the tool prints beside it: the lines
/// after its `sheet <path>` header and before the next one, each
/// `  r<row>c<column> <label>`.
fn sheet_tiles(output: &str, sheet: &str) -> Vec<String> {
    let mut tiles = Vec::new();
    let mut inside = false;
    for line in output.lines() {
        if line.starts_with("sheet ") {
            inside = line.contains(sheet);
            continue;
        }
        if !inside {
            continue;
        }
        let mut fields = line.trim().splitn(2, ' ');
        let (Some(position), Some(label)) = (fields.next(), fields.next()) else {
            continue;
        };
        // The map's own lines, and nothing else: `r3c2 <path> (…)`.
        let is_position = position
            .strip_prefix('r')
            .and_then(|rest| rest.split_once('c'))
            .is_some_and(|(row, column)| {
                !row.is_empty()
                    && !column.is_empty()
                    && row.chars().all(|c| c.is_ascii_digit())
                    && column.chars().all(|c| c.is_ascii_digit())
            });
        if is_position {
            tiles.push(label.to_owned());
        }
    }
    tiles
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

#[test]
fn a_sheet_path_that_dotdots_behind_a_missing_component_is_refused_too() {
    // The bypass `--sheet` inherited from `compare`'s guard: only the deepest
    // ancestor that exists is canonicalised, so a `..` behind a component
    // nothing has made yet survived as text — outside the tree for the string
    // comparison, inside it for every write, which is how a PNG lands among the
    // assets with no provenance row. `missing` has to sit *beside* the tree and
    // not inside it: a tail that starts inside `assets/` still compares as one
    // (the first mismatching component is what `starts_with` looks for), and a
    // test written that way passes whether or not the guard collapses the path.
    let root = scratch("sheet-dotdot").join("tree");
    copy_tree(&fixture("assets-hash-mismatch"), &root);
    let inside = root.join("missing/../assets/sheets");
    let run = xtask(
        &root,
        &["assets", "verify", "--sheet", inside.to_str().unwrap()],
    );

    assert_eq!(run.code, 2, "a refused --sheet is 2:\n{}", run.output());
    assert!(run.output().contains("--sheet"), "{}", run.output());
    assert!(
        !root.join("assets/missing").exists(),
        "the refusal is before anything is written, so not even the component on the way exists"
    );
    assert!(
        !root.join("assets/sheets").exists(),
        "nothing may be written into the tree by a refused --sheet"
    );
}
