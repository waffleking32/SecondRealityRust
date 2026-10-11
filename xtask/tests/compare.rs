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
