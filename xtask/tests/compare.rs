//! `cargo xtask compare` end to end: the exit code *is* the contract.
//!
//! A script gating a phase has to tell three things apart — "every frame is
//! identical" (0), "some frame differs" (1) and "the tool could not compare"
//! (2). The difference between the last two is the one that matters most: a
//! comparison that pairs a reference frame with something 300 frames away and
//! prints a percentage is worse than one that refuses, because the percentage
//! looks like evidence. The tests here run the built binary and read its exit
//! code, the way the script will, and the refusal is the one that is checked in
//! most detail: exit 2, a message naming the index, and **no report on disk**.
//!
//! The fixtures are written here rather than committed: a capture is a manifest
//! and a handful of frame files, and a test that writes anything writes it
//! outside the repository, under the temporary directory.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

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

/// Run the built `xtask` binary.
fn xtask(args: &[&str]) -> Run {
    let output = Command::new(env!("CARGO_BIN_EXE_xtask"))
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
    let path = std::env::temp_dir().join(format!("xtask-compare-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).unwrap();
    path
}

/// Write a one-geometry capture of 4×2 frames, each one a flat index value: two
/// frames with different values differ in every pixel, which is all these tests
/// need. `--reference` accepts the capture manifest's shape and the port
/// dump's, and this is the capture shape.
fn write_capture(dir: &Path, frames: &[(u32, u8)]) {
    fs::create_dir_all(dir).unwrap();
    let entries: Vec<serde_json::Value> = frames
        .iter()
        .map(|(index, value)| {
            let name = format!("frame_{index:06}");
            fs::write(dir.join(format!("{name}.idx")), vec![*value; 8]).unwrap();
            fs::write(dir.join(format!("{name}.rgb")), vec![*value; 24]).unwrap();
            serde_json::json!({
                "index": index,
                "idx": format!("{name}.idx"),
                "rgb": format!("{name}.rgb"),
                "palette": { "0": [0, 0, 0] },
            })
        })
        .collect();
    let manifest = serde_json::json!({
        "version": 1,
        "oracle": "dosbox-automation test",
        "upstream_commit": "071a82e",
        "exe": "SECOND.EXE",
        "arguments": ["2"],
        "cycles": 30000,
        "width": 4,
        "height": 2,
        "frame_origin": "boot",
        "skip_boot_frames": 0,
        "keys": ["KBD_left", "KBD_enter"],
        "frames": entries,
    });
    let json = serde_json::to_string_pretty(&manifest).unwrap();
    fs::write(dir.join("manifest.json"), format!("{json}\n")).unwrap();
}

/// Every `report.json` under `dir`, however the tree was laid out.
fn reports(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return found;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            found.extend(reports(&path));
        } else if path.file_name().is_some_and(|name| name == "report.json") {
            found.push(path);
        }
    }
    found
}

#[test]
fn frames_that_cannot_be_paired_exit_2_and_leave_no_report() {
    // The shape of the stale negative check in the tree: `reference/check_a` at
    // 700/900 against an older capture at 400/600. The old pairing zipped them
    // positionally and printed percentages for frames 300 apart — and with
    // these two screens, which happen to hold the same values, it would have
    // printed `identical` and exited 0, a mispairing reported as a match. The
    // window is ±1, so there is nothing to pair and the comparison is refused:
    // this is what makes the search a widening of the alignment rather than a
    // loosening of the check.
    //
    // Widening the window to 10,000 turns this test red (it exits 0), so the
    // refusal is pinned to the window and not to the frame values.
    let reference = scratch("unpairable-reference");
    let rendered = scratch("unpairable-rendered");
    // A path nothing has made yet, so the assertion below can tell a directory
    // the comparison left from one that was there to begin with.
    let out = scratch("unpairable-out").join("reports");
    write_capture(&reference, &[(700, 1), (900, 1)]);
    write_capture(&rendered, &[(400, 1), (600, 1)]);

    let run = xtask(&[
        "compare",
        "--reference",
        reference.to_str().unwrap(),
        "--rendered",
        rendered.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);

    assert_eq!(run.code, 2, "a comparison that cannot run is 2:\n{}", run.output());
    let output = run.output();
    // The refusal names the frame it could not place and the window it looked
    // in, so a reader does not have to run anything again to see why.
    assert!(output.contains("700"), "{output}");
    assert!(output.contains("±1"), "{output}");
    assert!(output.contains("means nothing"), "{output}");
    // Nothing at all: not a report, not a diff image, and not even the empty
    // output directory. A script that found a directory would have to open it
    // to learn that the comparison never ran; the exit code is the answer.
    assert!(
        reports(&out).is_empty(),
        "a refused comparison must not leave a report behind"
    );
    assert!(
        !out.exists(),
        "a refused comparison must not leave so much as an empty directory behind"
    );
}

#[test]
fn frames_that_are_identical_exit_0() {
    let capture = scratch("identical");
    // A path nothing has made yet: a comparison that writes something makes it.
    let out = scratch("identical-out").join("reports");
    write_capture(&capture, &[(700, 1), (900, 2)]);

    let run = xtask(&[
        "compare",
        "--reference",
        capture.to_str().unwrap(),
        "--rendered",
        capture.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);

    assert_eq!(run.code, 0, "identical frames are 0:\n{}", run.output());
    assert!(run.stdout.contains("verdict: identical"), "{}", run.output());
    assert_eq!(reports(&out).len(), 1);
}

#[test]
fn frames_that_differ_at_matching_indices_exit_1() {
    let reference = scratch("differing-reference");
    let rendered = scratch("differing-rendered");
    let out = scratch("differing-out").join("reports");
    write_capture(&reference, &[(700, 1), (900, 1)]);
    write_capture(&rendered, &[(700, 2), (900, 1)]);

    let run = xtask(&[
        "compare",
        "--reference",
        reference.to_str().unwrap(),
        "--rendered",
        rendered.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);

    assert_eq!(run.code, 1, "a difference at matching indices is 1:\n{}", run.output());
    assert!(run.stdout.contains("verdict: different"), "{}", run.output());
    assert!(run.stdout.contains("100.0000% differing"), "{}", run.output());
    assert_eq!(reports(&out).len(), 1);
}

#[test]
fn a_tree_of_captures_is_measured_all_the_way_down() {
    // `reference/parts`' shape, and the failure it hid: eighteen captures in
    // the top directory and two parts that keep two each one level deeper. A
    // scan of the first level compared 18 of the 22, skipped everything below
    // and still printed `verdict: identical` with exit 0 — the run that says a
    // phase is finished. Every capture below the root has to be measured, and
    // the count is printed with the verdict so a partial run cannot look whole.
    let tree = scratch("nested-tree");
    write_capture(&tree.join("title"), &[(700, 1), (900, 2)]);
    write_capture(&tree.join("plzpart/cube"), &[(1350, 3)]);
    write_capture(&tree.join("techno/troll"), &[(8500, 4), (9500, 5)]);
    let out = scratch("nested-out").join("reports");

    let run = xtask(&[
        "compare",
        "--reference",
        tree.to_str().unwrap(),
        "--rendered",
        tree.to_str().unwrap(),
        "--out",
        out.to_str().unwrap(),
    ]);

    assert_eq!(run.code, 0, "every frame is itself:\n{}", run.output());
    assert!(run.stdout.contains("captures: 3 measured"), "{}", run.output());
    let mut written: Vec<String> = reports(&out)
        .into_iter()
        .map(|path| {
            path.parent()
                .unwrap()
                .strip_prefix(&out)
                .unwrap()
                .to_string_lossy()
                .into_owned()
        })
        .collect();
    written.sort();
    assert_eq!(
        written,
        vec!["plzpart/cube", "techno/troll", "title"],
        "one report per capture, named as the tree nests them"
    );
}

#[test]
fn an_alignment_pairs_a_run_numbered_from_zero_with_the_references_indices() {
    // What a part the port cannot anchor needs (TUNNELI: `docs/fidelity/
    // tunneli.md` §7.2). The same two captures as the refusal above — 300
    // frames apart in their numbering — compared with the alignment that maps
    // one numbering onto the other: the port's tick 400 with the reference's
    // 700, and 600 with 900.
    let reference = scratch("align-reference");
    let rendered = scratch("align-rendered");
    let out = scratch("align-out").join("reports");
    write_capture(&reference, &[(700, 5), (900, 5)]);
    write_capture(&rendered, &[(400, 5), (600, 5)]);
    let args = |align: &str| {
        vec![
            "compare".to_string(),
            "--reference".to_string(),
            reference.to_str().unwrap().to_string(),
            "--rendered".to_string(),
            rendered.to_str().unwrap().to_string(),
            "--out".to_string(),
            out.to_str().unwrap().to_string(),
            "--align".to_string(),
            align.to_string(),
        ]
    };
    let run = |align: &str| {
        let args = args(align);
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        xtask(&refs)
    };

    // Without it, the same comparison this tree already refuses.
    let refused = run("0");
    assert_eq!(refused.code, 2, "{}", refused.output());
    assert!(refused.output().contains("no rendered frame within"), "{}", refused.output());
    assert!(reports(&out).is_empty(), "a refused comparison left a report");

    // With it: every frame identical, and the console says the alignment was
    // applied, so a report cannot be mistaken for one made without it.
    let aligned = run("300");
    assert_eq!(aligned.code, 0, "{}", aligned.output());
    assert!(aligned.output().contains("read +300"), "{}", aligned.output());
    assert!(aligned.output().contains("frame 700: 0.0000%"), "{}", aligned.output());

    // And the offset reported is the *net* one: one frame further off, the
    // pair is at +1 and the console says so on the frame's own line.
    let off_by_one = run("301");
    assert_eq!(off_by_one.code, 0, "{}", off_by_one.output());
    assert!(off_by_one.output().contains("offset +1"), "{}", off_by_one.output());
    assert!(off_by_one.output().contains("alignment: 2 of 2 frames paired"), "{}", off_by_one.output());
}
