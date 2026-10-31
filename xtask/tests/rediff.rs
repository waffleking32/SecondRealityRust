//! `cargo xtask rediff` end to end: the compiled table against the tree and the register, and
//! the exit code over a fixture capture (`tests/fixtures/rediff/parts`) with a stand-in binary
//! (`tests/fixtures/rediff/fake-secondreality`) and a table written per test: **0** every frame
//! at its registered count, **1** a count off its register, **2** could not compare.

mod common;

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use common::{repo_root, scratch, xtask, Run};

#[allow(dead_code)]
#[path = "../src/rediff/table.rs"]
mod table;

use table::TABLE;

fn fixtures() -> PathBuf {
    common::fixture("rediff")
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

#[test]
fn the_table_names_every_filed_capture_once() {
    let filed = filed_captures(&repo_root().join("reference/parts"));
    assert!(filed.len() >= 47, "the tree holds {} captures", filed.len());

    // Every entry names a filed capture, once, and every filed capture has an entry.
    let named: BTreeSet<&str> = TABLE.iter().map(|entry| entry.capture).collect();
    assert_eq!(named.len(), TABLE.len(), "a capture has two entries");
    assert_eq!(
        named,
        filed.keys().map(String::as_str).collect::<BTreeSet<_>>(),
        "the table's captures are not the tree's"
    );

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

/// Every non-`exact` register names a live `## A<n>` entry of `docs/fidelity/APPROXIMATIONS.md`
/// (a tombstone, `## A<n> — Retired: …` or `— Closed: …`, is not live).
#[test]
fn every_register_names_a_live_approximation() {
    let path = repo_root().join("docs/fidelity/APPROXIMATIONS.md");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    let headings: BTreeMap<u32, &str> = text
        .lines()
        .filter_map(|line| {
            let digits: String = line
                .strip_prefix("## A")?
                .chars()
                .take_while(char::is_ascii_digit)
                .collect();
            Some((digits.parse().ok()?, line))
        })
        .collect();
    for entry in TABLE {
        for frame in entry.frames {
            let register = frame.register;
            if register == "exact" {
                continue;
            }
            let n: u32 = register
                .strip_prefix('A')
                .and_then(|digits| digits.parse().ok())
                .unwrap_or_else(|| panic!("{}: `{register}` names no entry", entry.capture));
            let heading = headings
                .get(&n)
                .unwrap_or_else(|| panic!("{}: A{n} is not in the register", entry.capture));
            assert!(
                !heading.contains("— Retired:") && !heading.contains("— Closed:"),
                "{}: A{n} is a tombstone: {heading}",
                entry.capture
            );
        }
    }
}

/// A table of one entry for the fixture capture `still`, run as `--part <dump>` (the fake
/// binary hands back `rendered/<dump>/`), at `rendered = reference − 10`, with the counts
/// given for its three frames: 10 indexed, 10 RGB, 11 RGB.
fn write_table(dir: &Path, dump: &str, registered: [u32; 3]) -> PathBuf {
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
    let table = write_table(&dir, "same", [0, 0, 0]);
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
    let table = write_table(&dir, "off", [0, 0, 0]);
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
    let table = write_table(&dir, "off", [0, 0, 2]);
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 0, "{}", run.output());
    // And a register that is not the measured number is off it, in either direction.
    let table = write_table(&dir, "same", [0, 0, 2]);
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 1, "{}", run.output());
}

#[test]
fn a_frame_the_dump_lacks_is_exit_2() {
    let dir = scratch("short");
    let table = write_table(&dir, "short", [0, 0, 0]);
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
    let table = write_table(&dir, "tall", [0, 0, 0]);
    let out = dir.join("out");
    let run = rediff(&table, &out, &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("4x2"), "{}", run.output());
    assert!(run.output().contains("2x4"), "{}", run.output());
}

#[test]
fn a_run_that_fails_is_exit_2() {
    let dir = scratch("fails");
    let table = write_table(&dir, "fails", [0, 0, 0]);
    let run = rediff(&table, &dir.join("out"), &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("asked to fail"), "{}", run.output());
}

#[test]
fn a_capture_with_no_entry_is_exit_2() {
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
    let table = write_table(&dir, "same", [0, 0, 0]);
    let run = rediff(&table, &dir.join("out"), &reference);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("extra"), "{}", run.output());

}

#[test]
fn a_substituted_table_and_tree_are_a_partial_run() {
    let dir = scratch("partial");
    let table = write_table(&dir, "same", [0, 0, 0]);
    let reference = fixtures().join("parts");
    let out = dir.join("out");
    let run = rediff(&table, &out, &reference);
    assert_eq!(run.code, 0, "{}", run.output());
    let report = report(&out);
    assert_eq!(report["scope"], "partial");
    assert_eq!(report["table"], table.to_str().unwrap());
    assert_eq!(report["reference"], reference.to_str().unwrap());
    assert_eq!(report["only"], serde_json::json!([]));
    let lines: Vec<&str> = run.stdout.lines().collect();
    let banner = lines
        .iter()
        .position(|line| line.starts_with("partial run: "))
        .unwrap_or_else(|| panic!("no partial-run line: {}", run.stdout));
    assert!(lines[banner].contains(&format!("--table {}", table.display())), "{}", lines[banner]);
    assert!(
        lines[banner].contains(&format!("--reference {}", reference.display())),
        "{}",
        lines[banner]
    );
    let verdict = lines.iter().position(|line| line.starts_with("verdict: ")).unwrap();
    assert!(banner < verdict, "the line comes before the verdict: {}", run.stdout);
}

#[test]
fn an_only_that_names_nothing_in_the_table_is_refused() {
    // Comparing nothing is not "every frame at its registered count".
    let dir = scratch("only-nothing");
    let table = write_table(&dir, "same", [0, 0, 0]);
    let out = dir.join("out");
    let run = xtask(&[
        "rediff",
        "--binary",
        fixtures().join("fake-secondreality").to_str().unwrap(),
        "--reference",
        fixtures().join("parts").to_str().unwrap(),
        "--table",
        table.to_str().unwrap(),
        "--only",
        "elsewhere",
        "--out",
        out.to_str().unwrap(),
    ]);
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("names no capture"), "{}", run.output());
    assert!(!out.join("report.json").exists(), "a refused run writes no report");
}

#[test]
fn only_the_default_run_is_the_full_scope() {
    // The compiled table over the default `reference/parts`, with the fake binary: every entry
    // is a failed run and the exit is 2, but the report says `full`.
    let dir = scratch("full-scope");
    let fake = fixtures().join("fake-secondreality");
    let out = dir.join("out");
    let run = xtask(
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
    assert!(!run.stdout.contains("partial run"), "{}", run.stdout);

    // The same run narrowed by `--only` is partial, on the console and in the report.
    let run = xtask(
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
            .any(|line| line == "partial run: --only loop/alku"),
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
    let table = write_table(&dir, "same", [0, 0, 0]);
    let mut json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&table).unwrap()).unwrap();
    let mut gone = json["entries"][0].clone();
    gone["capture"] = "gone".into();
    json["entries"].as_array_mut().unwrap().push(gone);
    fs::write(&table, json.to_string()).unwrap();
    let run = rediff(&table, &dir.join("out"), &fixtures().join("parts"));
    assert_eq!(run.code, 2, "{}", run.output());
    assert!(run.output().contains("gone"), "{}", run.output());
}

#[test]
fn the_default_report_directory_is_out_rediff_and_the_reference_is_refused() {
    let dir = scratch("inside");
    let table = write_table(&dir, "same", [0, 0, 0]);
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
    // D16: a `..` behind a component that does not exist yet lands inside the reference.
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

    let table = write_table(&parent, "same", [0, 0, 0]);
    let run = rediff(&table, &out, &reference);
    assert_eq!(run.code, 2, "rediff: {}", run.output());
    assert!(run.output().contains("ref/nested"), "{}", run.output());
    assert!(!parent.join("missing").exists());
    assert!(!reference.join("nested").exists());

}

/// Every `fullrun/*` row's rendered index is its reference plus its part's start difference —
/// the part being the last one whose original first frame is at or before the reference
/// (`reference-indices.md` §26.3's rows) — except the frames named in
/// `FULLRUN_LOADER_FRAMES`, which sit on the port's last frame before the next part starts.
/// A row whose rendered index falls past the next part's port start lies across a handover,
/// and must be named in `FULLRUN_ACROSS_HANDOVER`. `bins/secondreality/tests/fullrun.rs`
/// includes the same file and asserts `FULLRUN_SEAMS` against the run, so the two cannot
/// disagree.
#[test]
fn every_fullrun_row_is_at_its_parts_start_difference() {
    use table::{FULLRUN_ACROSS_HANDOVER, FULLRUN_LOADER_FRAMES, FULLRUN_SEAMS};
    let seams = FULLRUN_SEAMS;
    assert!(
        seams.windows(2).all(|pair| pair[0].original < pair[1].original
            && pair[0].start < pair[1].start),
        "the seam table is in the run's order"
    );
    assert!(seams.iter().all(|seam| seam.start <= seam.port), "a part's first frame is not before its start");
    let part_of = |reference: u32| {
        seams.iter().rposition(|seam| seam.original <= reference).expect("a frame after 404")
    };
    let loader: BTreeMap<u32, usize> =
        FULLRUN_LOADER_FRAMES.iter().map(|(reference, next)| (*reference, *next as usize)).collect();
    assert_eq!(loader.keys().copied().collect::<Vec<_>>(), [7920, 16636, 34110]);

    let mut loader_seen = BTreeSet::new();
    let mut across_seen = BTreeSet::new();
    let mut rows = 0;
    for entry in TABLE.iter().filter(|entry| entry.capture.starts_with("fullrun/")) {
        assert_eq!(entry.args, ["--block", "1"], "{}", entry.capture);
        for frame in entry.frames {
            rows += 1;
            let part = part_of(frame.reference);
            let expected = match loader.get(&frame.reference) {
                Some(&next) => {
                    assert_eq!(next, part + 1, "{}: the loader stretch before the next part", frame.reference);
                    loader_seen.insert(frame.reference);
                    seams[next].start - 1
                }
                None => {
                    let seam = seams[part];
                    (i64::from(frame.reference) + i64::from(seam.port) - i64::from(seam.original)) as u32
                }
            };
            assert_eq!(
                frame.rendered, expected,
                "{} {}: {} is not at {}'s start difference",
                entry.capture, frame.reference, frame.rendered, seams[part].part
            );
            assert!(frame.rendered >= seams[part].start, "{}: before {}'s start", frame.reference, seams[part].part);
            let across = seams.get(part + 1).is_some_and(|next| frame.rendered >= next.start);
            if across {
                across_seen.insert(frame.reference);
            }
            assert_eq!(
                across,
                FULLRUN_ACROSS_HANDOVER.contains(&frame.reference),
                "{}: across a handover, and listed, must agree",
                frame.reference
            );
        }
    }
    assert_eq!(rows, 60, "the fullrun captures' 44 frames and 16 index planes");
    assert_eq!(loader_seen.len(), 3, "every loader frame is a row");
    assert_eq!(across_seen.into_iter().collect::<Vec<_>>(), FULLRUN_ACROSS_HANDOVER);
}
