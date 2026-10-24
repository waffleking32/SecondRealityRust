//! `cargo xtask rediff` end to end: the table against the tree, and the exit code as the
//! contract.
//!
//! Two kinds of test, and neither builds or runs the demo. The first reads the compiled
//! table (`xtask/src/rediff/table.rs`, included here by path — `xtask` is a binary crate,
//! so this is how an integration test sees its constants) against every `manifest.json`
//! under `reference/parts`: a capture the table forgets, or a frame an entry leaves out,
//! is a frame no command compares, which is the failure this command exists to end. The
//! second runs the built `xtask` over a fixture capture (`tests/fixtures/rediff/parts`)
//! with a stand-in for the binary (`tests/fixtures/rediff/fake-secondreality`), a table
//! written per test, and reads its exit code the way a phase's script will: **0** every
//! frame at its registered count, **1** a count off its register, **2** could not compare.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[allow(dead_code)]
#[path = "../src/rediff/table.rs"]
mod table;

use table::{PENDING, TABLE};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn fixtures() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/rediff")
}

/// Every capture under `root`, by its path below it, with the indices its manifest files.
fn filed_captures(root: &Path) -> BTreeMap<String, BTreeSet<u32>> {
    fn walk(root: &Path, dir: &Path, found: &mut BTreeMap<String, BTreeSet<u32>>) {
        let manifest = dir.join("manifest.json");
        if manifest.is_file() {
            let json: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(&manifest).unwrap()).unwrap();
            let indices = json["frames"]
                .as_array()
                .unwrap()
                .iter()
                .map(|frame| frame["index"].as_u64().unwrap() as u32)
                .collect();
            let name = dir
                .strip_prefix(root)
                .unwrap()
                .components()
                .map(|c| c.as_os_str().to_string_lossy().into_owned())
                .collect::<Vec<_>>()
                .join("/");
            found.insert(name, indices);
        }
        let mut subdirectories: Vec<PathBuf> = fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.is_dir())
            .collect();
        subdirectories.sort();
        for subdirectory in subdirectories {
            walk(root, &subdirectory, found);
        }
    }
    let mut found = BTreeMap::new();
    walk(root, root, &mut found);
    found
}

/// Whether a `PENDING` line covers `capture`: the line itself, or a directory above it.
fn pending_covers(pending: &str, capture: &str) -> bool {
    capture == pending || capture.starts_with(&format!("{pending}/"))
}

#[test]
fn the_table_names_every_filed_capture_once() {
    let filed = filed_captures(&repository_root().join("reference/parts"));
    assert!(filed.len() >= 47, "the tree holds {} captures", filed.len());

    // No capture is named twice, by two entries or by an entry and a PENDING line.
    let mut named = BTreeSet::new();
    for entry in TABLE {
        assert!(
            named.insert(entry.capture),
            "{} has two entries",
            entry.capture
        );
        for pending in PENDING {
            assert!(
                !pending_covers(pending, entry.capture),
                "{} has an entry and is PENDING under {pending}",
                entry.capture
            );
        }
    }

    // Every filed capture is an entry or under a PENDING line, and every entry and every
    // PENDING line names a filed capture.
    for capture in filed.keys() {
        let entries = TABLE
            .iter()
            .filter(|entry| entry.capture == capture)
            .count();
        let pending = PENDING
            .iter()
            .any(|pending| pending_covers(pending, capture));
        assert!(
            entries == 1 || (entries == 0 && pending),
            "{capture}: {entries} entries, pending {pending}"
        );
    }
    for entry in TABLE {
        assert!(
            filed.contains_key(entry.capture),
            "{} is not a filed capture",
            entry.capture
        );
    }
    for pending in PENDING {
        assert!(
            filed.keys().any(|capture| pending_covers(pending, capture)),
            "PENDING {pending} covers no filed capture"
        );
    }

    // Every filed frame of an entry's capture is one of its frames, and every frame it
    // lists is filed: an index the table leaves out is a frame no one compares.
    for entry in TABLE {
        let listed: BTreeSet<u32> = entry.frames.iter().map(|frame| frame.reference).collect();
        assert_eq!(
            &listed, &filed[entry.capture],
            "{}: the entry's reference indices are not the manifest's",
            entry.capture
        );
        assert!(
            !entry.args.is_empty(),
            "{}: an entry with no run",
            entry.capture
        );
        for frame in entry.frames {
            assert!(
                !frame.register.is_empty(),
                "{} {}: no register",
                entry.capture,
                frame.reference
            );
            assert!(
                frame.register != "exact" || frame.registered == 0,
                "{} {}: an \"exact\" frame registers {} pixels",
                entry.capture,
                frame.reference,
                frame.registered
            );
        }
    }
}

struct Run {
    code: i32,
    stdout: String,
    stderr: String,
}

impl Run {
    fn output(&self) -> String {
        format!("{}{}", self.stdout, self.stderr)
    }
}

fn xtask(args: &[&str]) -> Run {
    xtask_in(&std::env::current_dir().unwrap(), args)
}

/// The built `xtask`, run from `dir` — the repository root, for a run whose defaults
/// (`reference/parts`, the compiled table) are the phase run's.
fn xtask_in(dir: &Path, args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
        .current_dir(dir)
        .args(args)
        .output()
        .unwrap_or_else(|e| panic!("cannot run {}: {e}", env!("CARGO_BIN_EXE_xtask")));
    Run {
        code: output.status.code().unwrap_or(-1),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

/// A scratch directory for this test, emptied first.
fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!("xtask-rediff-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// A table of one entry for the fixture capture `still`, run as `--part <dump>` (the fake
/// binary hands back `rendered/<dump>/`), at `rendered = reference − 10`, with the counts
/// given for its three frames: 10 indexed, 10 RGB, 11 RGB.
fn write_table(dir: &Path, dump: &str, registered: [u32; 3], pending: &[&str]) -> PathBuf {
    let frame = |reference: u32, plane: &str, registered: u32| {
        serde_json::json!({
            "reference": reference,
            "rendered": reference - 10,
            "plane": plane,
            "registered": registered,
            "register": if registered == 0 { "exact" } else { "A0" },
        })
    };
    let table = serde_json::json!({
        "pending": pending,
        "entries": [{
            "capture": "still",
            "args": ["--part", dump],
            "frames": [
                frame(10, "indexed", registered[0]),
                frame(10, "rgb", registered[1]),
                frame(11, "rgb", registered[2]),
            ],
        }],
    });
    let path = dir.join("table.json");
    fs::write(&path, serde_json::to_string_pretty(&table).unwrap()).unwrap();
    path
}

/// `xtask rediff` over the fixture tree with the fake binary.
fn rediff(table: &Path, out: &Path, reference: &Path) -> Run {
    xtask(&[
        "rediff",
        "--binary",
        fixtures().join("fake-secondreality").to_str().unwrap(),
        "--reference",
        reference.to_str().unwrap(),
        "--table",
        table.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ])
}

fn report(out: &Path) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(out.join("report.json")).unwrap()).unwrap()
}

#[test]
fn every_frame_at_its_registered_count_is_exit_0() {
    let dir = scratch("exact");
    let table = write_table(&dir, "same", [0, 0, 0], &[]);
    let out = dir.join("out");
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 0, "{}", run.output());

    // One run for the entry, every rendered index in one `--dump-frames`.
    let argv = fs::read_to_string(out.join("still/rendered/argv.txt")).unwrap();
    assert!(argv.contains("--part same"), "{argv}");
    assert!(argv.contains("--headless"), "{argv}");
    assert!(argv.contains("--dump-frames 0,1 "), "{argv}");

    // One line a frame: capture, reference, rendered, plane, count, registered, verdict.
    let lines: Vec<&str> = run
        .stdout
        .lines()
        .filter(|line| line.starts_with("still "))
        .collect();
    assert_eq!(lines.len(), 3, "{}", run.stdout);
    let words: Vec<&str> = lines[0].split_whitespace().collect();
    assert_eq!(
        words,
        ["still", "10", "0", "indexed", "0", "0", "exact", "ok"]
    );
    let report = report(&out);
    assert_eq!(report["frames"].as_array().unwrap().len(), 3);
    assert_eq!(report["frames"][2]["reference"], 11);
    assert_eq!(report["frames"][2]["rendered"], 1);
    assert_eq!(report["frames"][2]["plane"], "rgb");
    assert_eq!(report["frames"][2]["differing_pixels"], 0);
    assert_eq!(report["frames"][2]["verdict"], "ok");
}

#[test]
fn a_count_off_the_register_is_exit_1() {
    let dir = scratch("off");
    let table = write_table(&dir, "off", [0, 0, 0], &[]);
    let out = dir.join("out");
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 1, "{}", run.output());
    let line = run
        .stdout
        .lines()
        .find(|line| line.starts_with("still ") && line.contains(" 11 "))
        .unwrap_or_else(|| panic!("no line for 11: {}", run.stdout));
    let words: Vec<&str> = line.split_whitespace().collect();
    assert_eq!(
        words,
        ["still", "11", "1", "rgb", "2", "0", "exact", "DIFFERS"]
    );
    assert_eq!(report(&out)["frames"][2]["verdict"], "DIFFERS");

    // The same two pixels, registered: the frame is at its register, and the run is 0.
    let table = write_table(&dir, "off", [0, 0, 2], &[]);
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 0, "{}", run.output());
    // And a register that is not the measured number is off it, in either direction.
    let table = write_table(&dir, "same", [0, 0, 2], &[]);
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 1, "{}", run.output());
}

#[test]
fn a_frame_the_dump_lacks_is_exit_2() {
    let dir = scratch("short");
    let table = write_table(&dir, "short", [0, 0, 0], &[]);
    let out = dir.join("out");
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("still"), "{}", run.output());
    assert!(
        run.output().contains("rendered frame 1"),
        "{}",
        run.output()
    );
    // The frames that could be compared still are, and the report says what was not.
    let report = report(&out);
    assert_eq!(report["frames"][0]["verdict"], "ok");
    assert_eq!(report["frames"][2]["verdict"], "not compared");
    assert!(!report["problems"].as_array().unwrap().is_empty());
}

#[test]
fn a_geometry_mismatch_is_exit_2_not_a_count() {
    // The same 8 pixels on a 2x4 screen: a pixel count would be a number about nothing.
    let dir = scratch("tall");
    let table = write_table(&dir, "tall", [0, 0, 0], &[]);
    let out = dir.join("out");
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("4x2"), "{}", run.output());
    assert!(run.output().contains("2x4"), "{}", run.output());
}

#[test]
fn a_run_that_fails_is_exit_2() {
    let dir = scratch("fails");
    let table = write_table(&dir, "fails", [0, 0, 0], &[]);
    let run = rediff(&table, &dir.join("out"), &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("asked to fail"), "{}", run.output());
}

#[test]
fn a_capture_with_no_entry_and_no_pending_line_is_exit_2() {
    // A second capture beside `still` that the table does not name.
    let dir = scratch("unlisted");
    let reference = dir.join("parts");
    for capture in ["still", "extra"] {
        let to = reference.join(capture);
        fs::create_dir_all(&to).unwrap();
        for file in fs::read_dir(fixtures().join("parts/still")).unwrap() {
            let file = file.unwrap().path();
            fs::copy(&file, to.join(file.file_name().unwrap())).unwrap();
        }
    }
    let table = write_table(&dir, "same", [0, 0, 0], &[]);
    let run = rediff(&table, &dir.join("out"), &reference);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("extra"), "{}", run.output());

    // A PENDING line covers it, and the report names it as not compared.
    let table = write_table(&dir, "same", [0, 0, 0], &["extra"]);
    let run = rediff(&table, &dir.join("out"), &reference);
    assert_eq!(run.code, 0, "{}", run.output());
    assert!(
        run.stdout
            .lines()
            .any(|line| line.starts_with("extra") && line.contains("not compared")),
        "{}",
        run.stdout
    );
    let report = report(&dir.join("out"));
    assert_eq!(report["pending"][0]["capture"], "extra");

    // A substituted table and tree are not the phase run, and say so in both places.
    assert_eq!(report["scope"], "partial");
    assert_eq!(report["table"], table.to_str().unwrap());
    assert_eq!(report["reference"], reference.to_str().unwrap());
    assert_eq!(report["only"], serde_json::json!([]));
    let banner = run
        .stdout
        .lines()
        .find(|line| line.starts_with("NOT THE PHASE RUN: "))
        .unwrap_or_else(|| panic!("no banner: {}", run.stdout));
    assert!(
        banner.contains(&format!("--table {}", table.display())),
        "{banner}"
    );
    assert!(
        banner.contains(&format!("--reference {}", reference.display())),
        "{banner}"
    );
    let verdict = run
        .stdout
        .lines()
        .position(|line| line.starts_with("verdict: "));
    let banner_at = run
        .stdout
        .lines()
        .position(|line| line.starts_with("NOT THE PHASE RUN"));
    assert!(
        banner_at < verdict,
        "the banner comes before the verdict: {}",
        run.stdout
    );
}

#[test]
fn an_only_that_names_only_pending_captures_is_refused() {
    // Comparing nothing is not "every frame at its registered count".
    let dir = scratch("only-pending");
    let reference = dir.join("parts");
    for capture in ["still", "extra"] {
        let to = reference.join(capture);
        fs::create_dir_all(&to).unwrap();
        for file in fs::read_dir(fixtures().join("parts/still")).unwrap() {
            let file = file.unwrap().path();
            fs::copy(&file, to.join(file.file_name().unwrap())).unwrap();
        }
    }
    let table = write_table(&dir, "same", [0, 0, 0], &["extra"]);
    let out = dir.join("out");
    let run = xtask(&[
        "rediff",
        "--binary",
        fixtures().join("fake-secondreality").to_str().unwrap(),
        "--reference",
        reference.to_str().unwrap(),
        "--table",
        table.to_str().unwrap(),
        "--only",
        "extra",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(
        run.output().contains("nothing to compare"),
        "{}",
        run.output()
    );
    assert!(
        !out.join("report.json").exists(),
        "a refused run writes no report"
    );
}

#[test]
fn only_the_default_run_is_the_full_scope() {
    // The compiled table over the default `reference/parts`, from the repository root,
    // with the fake binary: it cannot render the demo, so every entry is a failed run and
    // the exit is 2 — but the report it writes is the phase run's shape, and says `full`.
    let dir = scratch("full-scope");
    let fake = fixtures().join("fake-secondreality");
    let out = dir.join("out");
    let run = xtask_in(
        &repository_root(),
        &[
            "rediff",
            "--binary",
            fake.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(run.code, 2, "{}", run.output());
    let full = report(&out);
    assert_eq!(full["scope"], "full");
    assert_eq!(full["table"], "compiled");
    assert_eq!(full["reference"], "reference/parts");
    assert_eq!(full["only"], serde_json::json!([]));
    assert!(!run.stdout.contains("NOT THE PHASE RUN"), "{}", run.stdout);

    // The same run narrowed by `--only` is partial, on the console and in the report.
    let run = xtask_in(
        &repository_root(),
        &[
            "rediff",
            "--binary",
            fake.to_str().unwrap(),
            "--only",
            "loop/alku",
            "--out",
            out.to_str().unwrap(),
        ],
    );
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(
        run.stdout
            .lines()
            .any(|line| line == "NOT THE PHASE RUN: --only loop/alku"),
        "{}",
        run.stdout
    );
    let partial = report(&out);
    assert_eq!(partial["scope"], "partial");
    assert_eq!(partial["only"], serde_json::json!(["loop/alku"]));
    assert_eq!(partial["table"], "compiled");
}

#[test]
fn an_entry_with_no_capture_is_exit_2() {
    let dir = scratch("no-capture");
    // An entry for a capture the tree does not hold.
    let table = write_table(&dir, "same", [0, 0, 0], &[]);
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&table).unwrap()).unwrap();
    let mut gone = json["entries"][0].clone();
    gone["capture"] = "gone".into();
    json["entries"].as_array_mut().unwrap().push(gone);
    fs::write(&table, json.to_string()).unwrap();
    let run = rediff(&table, &dir.join("out"), &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("gone"), "{}", run.output());

    // And a PENDING line that covers nothing.
    let table = write_table(&dir, "same", [0, 0, 0], &["nowhere"]);
    let run = rediff(&table, &dir.join("out"), &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("nowhere"), "{}", run.output());
}

#[test]
fn the_default_report_directory_is_out_rediff_and_the_reference_is_refused() {
    let dir = scratch("inside");
    let table = write_table(&dir, "same", [0, 0, 0], &[]);
    let reference = dir.join("parts");
    fs::create_dir_all(&reference).unwrap();
    let run = rediff(&table, &reference.join("report"), &reference);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("--out"), "{}", run.output());
    assert!(!reference.join("report").exists());
    let help = xtask(&["rediff", "--help"]);
    assert!(help.stdout.contains("out/rediff"), "{}", help.stdout);
}

#[test]
fn an_out_through_dot_dot_into_the_reference_is_refused() {
    // D16's bypass: a `..` behind a component that does not exist yet survived the guard
    // as text, and the write landed inside the reference. Both writers are refused, and
    // neither leaves the component it would have made on the way.
    let parent = scratch("dotdot");
    let reference = parent.join("ref");
    fs::create_dir_all(reference.join("still")).unwrap();
    for file in fs::read_dir(fixtures().join("parts/still")).unwrap() {
        let file = file.unwrap().path();
        fs::copy(
            &file,
            reference.join("still").join(file.file_name().unwrap()),
        )
        .unwrap();
    }
    let out = parent.join("missing/../ref/nested");

    let table = write_table(&parent, "same", [0, 0, 0], &[]);
    let run = rediff(&table, &out, &reference);
    assert_eq!(run.code, 2, "rediff: {}", run.output());
    assert!(run.output().contains("ref/nested"), "{}", run.output());
    assert!(!parent.join("missing").exists());
    assert!(!reference.join("nested").exists());

    let run = xtask(&[
        "compare",
        "--reference",
        reference.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "compare: {}", run.output());
    assert!(run.output().contains("ref/nested"), "{}", run.output());
    assert!(!parent.join("missing").exists());
    assert!(!reference.join("nested").exists());
}
